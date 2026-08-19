//! Applying commands.
//!
//! The order of the checks is the contract:
//!
//! ```text
//! 1  idempotency   a replay returns the original result and applies nothing
//! 2  validation    a malformed command is refused before anything is read
//! 3  existence     the target must be there
//! 4  revision      expected_revision must match, or it is a conflict — never a merge
//! 5  apply         mutate, bump the revision, record history
//! 6  record        audit and events, including for a refusal
//! ```
//!
//! Step 6 is the one implementations get wrong. A refusal that leaves no trace is indistinguishable
//! from an attempt that never happened, and "an agent tried to change production and was stopped" is
//! precisely the thing an audit trail exists to show.

use aep_contract::command::{CommandEnvelope, CommandOutcome, CommandResult, CommandService};
use aep_contract::error::CommandError;
use aep_contract::query::{Relation, RevisionRecord};
use aep_domain::audit::{AuditKind, AuditRecord, CausationRef, ChangeRecord, DecisionRecord};
use aep_domain::command::Command;
use aep_domain::domain_event::{DomainEvent, DomainEventEnvelope, DomainEventType};
use aep_domain::entity::{ActorRef, EntityMetadata, EntityRef, EntityRevision, VersionedEntityRef};
use aep_domain::ids::{AuditId, EventId};
use aep_domain::node::Node;
use aep_domain::time::Timestamp;

use crate::store::{AppliedCommand, Store, StoredEntity};
use crate::MemoryBackend;

/// The body key a lifecycle status is kept under.
///
/// The contract carries untyped bodies, so status is a convention rather than a field. It is spelled
/// out here because two backends disagreeing about where status lives would silently break every
/// query that filters on it.
pub const STATUS_KEY: &str = "status";

impl CommandService for MemoryBackend {
    type Command = Command;

    async fn execute(
        &self,
        envelope: CommandEnvelope<Self::Command>,
    ) -> Result<CommandResult, CommandError> {
        self.with_store_mut(|store| apply(store, &envelope))
    }
}

/// Applies one command, recording what happened either way.
fn apply(
    store: &mut Store,
    envelope: &CommandEnvelope<Command>,
) -> Result<CommandResult, CommandError> {
    let key = &envelope.context.idempotency_key;

    // 1. Idempotency. A replay of the same logical command returns what it returned before; the same
    //    key on a *different* command is a client bug, and accepting it would make the key useless.
    if let Some(applied) = store.applied(key) {
        if applied.command_id == envelope.command_id {
            let mut result = applied.result.clone();
            result.outcome = CommandOutcome::Replayed;
            return Ok(result);
        }
        let error = CommandError::IdempotencyMismatch {
            key: key.clone(),
            original: applied.command_id.clone(),
        };
        record_rejection(store, envelope, &error);
        return Err(error);
    }

    // 2. Validation, before anything is read.
    if let Err(errors) = envelope.payload.validate() {
        let error = CommandError::Invalid { errors };
        record_rejection(store, envelope, &error);
        return Err(error);
    }

    match apply_valid(store, envelope) {
        Ok(result) => {
            store.remember(
                key,
                AppliedCommand {
                    command_id: envelope.command_id.clone(),
                    result: result.clone(),
                },
            );
            Ok(result)
        }
        Err(error) => {
            record_rejection(store, envelope, &error);
            Err(error)
        }
    }
}

/// Applies a command that has passed idempotency and validation.
// One arm per command. Splitting it would scatter the effect of the command vocabulary across a
// dozen functions, and the thing a reader needs is to see all of them side by side.
#[allow(clippy::too_many_lines)]
fn apply_valid(
    store: &mut Store,
    envelope: &CommandEnvelope<Command>,
) -> Result<CommandResult, CommandError> {
    let at = envelope.context.issued_at;
    let actor = envelope.context.actor.clone();
    let executor = envelope.context.executor.clone();

    let affected = match &envelope.payload {
        Command::CreateEntity(create) => {
            if store.locator_taken(&create.locator) {
                return Err(CommandError::Conflict {
                    reason: format!("`{}` already addresses an entity", create.locator),
                });
            }
            let id = store.next_entity_id();
            let metadata = EntityMetadata::new(
                id.clone(),
                create.locator.clone(),
                create.entity_type.clone(),
                at,
                actor.clone(),
            );
            let reference = metadata.versioned_reference();
            store.insert_entity(StoredEntity {
                metadata,
                data: create.data.clone(),
                archived: false,
            });
            record_history(store, &reference, envelope, at, &actor, executor.as_ref());
            emit(
                store,
                envelope,
                "aep.entity.created/v1",
                DomainEvent::Custom {
                    event_type: parse_type("aep.entity.created/v1"),
                    data: create.data.clone(),
                },
                &reference,
                at,
                &actor,
            );
            vec![reference]
        }

        Command::UpdateEntity(update) => {
            // A generic patch is the one command that could quietly edit evidence, so the type's
            // own answer to "may this be changed?" is asked before anything is touched.
            require_mutable(store, &update.target)?;
            let reference = mutate(
                store,
                envelope,
                &update.target,
                at,
                &actor,
                executor.as_ref(),
                |entity| {
                    merge(&mut entity.data, &update.changes);
                },
            )?;
            emit(
                store,
                envelope,
                "aep.entity.updated/v1",
                DomainEvent::Custom {
                    event_type: parse_type("aep.entity.updated/v1"),
                    data: Node::Map(update.changes.clone()),
                },
                &reference,
                at,
                &actor,
            );
            vec![reference]
        }

        Command::CreateRelation(create) => {
            require_entity(store, &create.source)?;
            require_entity(store, &create.target)?;
            let id = store.next_relation_id();
            store.insert_relation(Relation {
                id,
                kind: create.kind,
                source: create.source.clone(),
                target: create.target.clone(),
                created_at: at,
                created_by: actor.clone(),
            });
            Vec::new()
        }

        Command::RemoveRelation(remove) => {
            if store.remove_relation(&remove.relation).is_none() {
                return Err(CommandError::Conflict {
                    reason: format!("relation `{}` does not exist", remove.relation),
                });
            }
            Vec::new()
        }

        Command::ArchiveEntity(archive) => {
            // Archiving is a state change, not a delete: the entity stays readable, which is the
            // whole point of not having a delete.
            let reference = mutate(
                store,
                envelope,
                &archive.target,
                at,
                &actor,
                executor.as_ref(),
                |entity| {
                    entity.archived = true;
                    set_status(&mut entity.data, "archived");
                },
            )?;
            vec![reference]
        }

        Command::SupersedeEntity(supersede) => {
            require_entity(store, &supersede.successor)?;
            let reference = mutate(
                store,
                envelope,
                &supersede.target,
                at,
                &actor,
                executor.as_ref(),
                |entity| {
                    set_status(&mut entity.data, "superseded");
                },
            )?;
            let id = store.next_relation_id();
            store.insert_relation(Relation {
                id,
                kind: aep_domain::artifact::RelationKind::Supersedes,
                source: supersede.successor.clone(),
                target: supersede.target.clone(),
                created_at: at,
                created_by: actor.clone(),
            });
            vec![reference]
        }

        Command::SubmitDesignReview(submit) => {
            require_revision(store, &submit.design)?;
            let reference = mutate(
                store,
                envelope,
                &submit.design.unversioned(),
                at,
                &actor,
                executor.as_ref(),
                |entity| set_status(&mut entity.data, "in_review"),
            )?;
            emit(
                store,
                envelope,
                "aep.design.submitted-for-review/v1",
                DomainEvent::Custom {
                    event_type: parse_type("aep.design.submitted-for-review/v1"),
                    data: Node::Text(submit.reviewer.to_string()),
                },
                &reference,
                at,
                &actor,
            );
            vec![reference]
        }

        Command::ApproveDesign(approve) => {
            // The review must exist and must be about this exact revision. This is the check a
            // generic `PATCH status = approved` cannot make.
            require_entity(store, &approve.review)?;
            require_revision(store, &approve.design)?;
            let reference = mutate(
                store,
                envelope,
                &approve.design.unversioned(),
                at,
                &actor,
                executor.as_ref(),
                |entity| set_status(&mut entity.data, "approved"),
            )?;
            emit(
                store,
                envelope,
                "aep.design.approved/v1",
                DomainEvent::Custom {
                    event_type: parse_type("aep.design.approved/v1"),
                    data: Node::Text(approve.review.to_string()),
                },
                &reference,
                at,
                &actor,
            );
            vec![reference]
        }

        Command::AcceptAdr(accept) => {
            require_revision(store, &accept.adr)?;
            let reference = mutate(
                store,
                envelope,
                &accept.adr.unversioned(),
                at,
                &actor,
                executor.as_ref(),
                |entity| set_status(&mut entity.data, "accepted"),
            )?;
            if let Some(superseded) = &accept.supersedes {
                require_entity(store, superseded)?;
                let id = store.next_relation_id();
                store.insert_relation(Relation {
                    id,
                    kind: aep_domain::artifact::RelationKind::Supersedes,
                    source: accept.adr.unversioned(),
                    target: superseded.clone(),
                    created_at: at,
                    created_by: actor.clone(),
                });
            }
            emit(
                store,
                envelope,
                "aep.adr.accepted/v1",
                DomainEvent::Custom {
                    event_type: parse_type("aep.adr.accepted/v1"),
                    data: Node::Null,
                },
                &reference,
                at,
                &actor,
            );
            vec![reference]
        }

        // The command vocabulary is open, so a backend must be able to say "not me" without
        // implying a permission problem.
        other => {
            return Err(CommandError::Unsupported {
                command_type: other.kind().as_str().to_owned(),
            })
        }
    };

    let audit_id = store.next_audit_id();
    let change = affected.first().map(|reference| ChangeRecord {
        entity: reference.unversioned(),
        before: previous_revision(reference.revision),
        after: Some(reference.revision),
        command: Some(envelope.command_type.clone()),
        payload: None,
        redacted: false,
        redaction_reason: None,
    });
    store.record_audit(AuditRecord {
        audit_id: audit_id.clone(),
        kind: AuditKind::CommandAccepted,
        occurred_at: at,
        actor: actor.clone(),
        executor: executor.clone(),
        subject: affected.first().map(VersionedEntityRef::unversioned),
        request_id: Some(envelope.context.request_id.clone()),
        command_id: Some(envelope.command_id.clone()),
        event_id: None,
        correlation_id: envelope.context.correlation_id.clone(),
        causation: envelope
            .context
            .causation
            .as_ref()
            .map(|causation| CausationRef::Decision {
                decision: audit_id_from(&causation.0),
            }),
        execution_id: envelope.context.execution_id.clone(),
        task: envelope.context.task.clone(),
        decision: None,
        change,
        evidence: Vec::new(),
    });

    Ok(CommandResult {
        command_id: envelope.command_id.clone(),
        outcome: CommandOutcome::Accepted,
        affected,
        events: store
            .events()
            .last()
            .map(|event| vec![event.event_id.clone()])
            .unwrap_or_default(),
        audit: vec![audit_id],
        consistency: store.token(),
    })
}

/// The revision before `revision`, or `None` for a creation.
fn previous_revision(revision: EntityRevision) -> Option<EntityRevision> {
    EntityRevision::new(revision.get().saturating_sub(1)).ok()
}

/// Reads a causation string back as an audit identifier, falling back to a placeholder.
fn audit_id_from(value: &str) -> AuditId {
    AuditId::new(value).unwrap_or_else(|_| AuditId::new("aud-unknown").expect("well formed"))
}

/// Parses a known-good event type.
fn parse_type(value: &str) -> DomainEventType {
    value.parse().expect("a built-in event type is well formed")
}

/// Fails when an entity does not exist.
fn require_entity(store: &Store, reference: &EntityRef) -> Result<(), CommandError> {
    if store.entity(&reference.id).is_none() {
        return Err(CommandError::NotFound {
            entity: reference.clone(),
        });
    }
    Ok(())
}

/// Fails when an entity's type does not permit editing.
fn require_mutable(store: &Store, reference: &EntityRef) -> Result<(), CommandError> {
    let entity = store
        .entity(&reference.id)
        .ok_or_else(|| CommandError::NotFound {
            entity: reference.clone(),
        })?;
    if crate::query::is_mutable(&entity.metadata.entity_type) {
        return Ok(());
    }
    Err(CommandError::Conflict {
        reason: format!(
            "`{}` is a {}, which is immutable: a record that can be edited after the fact is not \
             evidence. Archive it, or supersede it with a new one.",
            reference, entity.metadata.entity_type
        ),
    })
}

/// Fails when an entity is not at the revision a command names.
fn require_revision(store: &Store, reference: &VersionedEntityRef) -> Result<(), CommandError> {
    let entity = store
        .entity(&reference.id)
        .ok_or_else(|| CommandError::NotFound {
            entity: reference.unversioned(),
        })?;
    if entity.metadata.revision != reference.revision {
        return Err(CommandError::RevisionConflict {
            entity: reference.unversioned(),
            expected: reference.revision,
            actual: entity.metadata.revision,
        });
    }
    Ok(())
}

/// Applies a change to an entity, after checking existence and the asserted revision.
fn mutate(
    store: &mut Store,
    envelope: &CommandEnvelope<Command>,
    target: &EntityRef,
    at: Timestamp,
    actor: &ActorRef,
    executor: Option<&ActorRef>,
    change: impl FnOnce(&mut StoredEntity),
) -> Result<VersionedEntityRef, CommandError> {
    let entity = store
        .entity(&target.id)
        .ok_or_else(|| CommandError::NotFound {
            entity: target.clone(),
        })?;

    if let Some(expected) = envelope.expected_revision {
        if entity.metadata.revision != expected {
            return Err(CommandError::RevisionConflict {
                entity: target.clone(),
                expected,
                actual: entity.metadata.revision,
            });
        }
    }

    let entity = store
        .entity_mut(&target.id)
        .expect("the entity was present a moment ago");
    change(entity);
    entity
        .metadata
        .advance(at, actor.clone(), executor.cloned());
    let reference = entity.metadata.versioned_reference();
    record_history(store, &reference, envelope, at, actor, executor);
    Ok(reference)
}

/// Appends a history entry for a revision.
fn record_history(
    store: &mut Store,
    reference: &VersionedEntityRef,
    envelope: &CommandEnvelope<Command>,
    at: Timestamp,
    actor: &ActorRef,
    executor: Option<&ActorRef>,
) {
    store.record_revision(
        &reference.id,
        RevisionRecord {
            revision: reference.revision,
            at,
            actor: actor.clone(),
            executor: executor.cloned(),
            command_id: Some(envelope.command_id.clone()),
            audit_id: None,
        },
    );
}

/// Records a domain event caused by this command.
fn emit(
    store: &mut Store,
    envelope: &CommandEnvelope<Command>,
    event_type: &str,
    payload: DomainEvent,
    subject: &VersionedEntityRef,
    at: Timestamp,
    actor: &ActorRef,
) {
    let event_id: EventId = store.next_event_id();
    store.record_event(DomainEventEnvelope {
        event_id,
        event_type: parse_type(event_type),
        subject: Some(subject.unversioned()),
        entity_revision: Some(subject.revision),
        payload,
        command_id: Some(envelope.command_id.clone()),
        correlation_id: envelope.context.correlation_id.clone(),
        causation: Some(envelope.command_id.to_string()),
        execution_id: envelope.context.execution_id.clone(),
        occurred_at: at,
        actor: actor.clone(),
    });
}

/// Records that a command was refused, with the reason.
fn record_rejection(store: &mut Store, envelope: &CommandEnvelope<Command>, error: &CommandError) {
    let audit_id = store.next_audit_id();
    store.record_audit(AuditRecord {
        audit_id,
        kind: AuditKind::CommandRejected,
        occurred_at: envelope.context.issued_at,
        actor: envelope.context.actor.clone(),
        executor: envelope.context.executor.clone(),
        subject: envelope.target.clone(),
        request_id: Some(envelope.context.request_id.clone()),
        command_id: Some(envelope.command_id.clone()),
        event_id: None,
        correlation_id: envelope.context.correlation_id.clone(),
        causation: None,
        execution_id: envelope.context.execution_id.clone(),
        task: envelope.context.task.clone(),
        decision: Some(DecisionRecord {
            allowed: false,
            operation: envelope.payload.summary(),
            capability: Some(envelope.payload.required_capability()),
            decision: None,
            source: None,
            rule: Some(error.code().to_owned()),
            missing: vec![error.to_string()],
            state: None,
        }),
        // A rejected command changed nothing, so it carries no change record. The audit type
        // enforces this; recording one here would be a lie about what happened.
        change: None,
        evidence: Vec::new(),
    });
}

/// Sets the conventional status key in an entity body.
fn set_status(data: &mut Node, status: &str) {
    match data {
        Node::Map(entries) => {
            entries.insert(STATUS_KEY.to_owned(), Node::Text(status.to_owned()));
        }
        other => {
            let mut entries = std::collections::BTreeMap::new();
            entries.insert("value".to_owned(), other.clone());
            entries.insert(STATUS_KEY.to_owned(), Node::Text(status.to_owned()));
            *other = Node::Map(entries);
        }
    }
}

/// Merges changes into an entity body.
fn merge(data: &mut Node, changes: &std::collections::BTreeMap<String, Node>) {
    match data {
        Node::Map(entries) => {
            for (key, value) in changes {
                entries.insert(key.clone(), value.clone());
            }
        }
        other => {
            let mut entries = std::collections::BTreeMap::new();
            entries.insert("value".to_owned(), other.clone());
            for (key, value) in changes {
                entries.insert(key.clone(), value.clone());
            }
            *other = Node::Map(entries);
        }
    }
}
