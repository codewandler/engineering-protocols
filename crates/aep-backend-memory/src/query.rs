//! Answering queries.
//!
//! Every method here is read-only, and that is a property worth stating rather than assuming: a
//! query that mutated would produce a change nothing in the audit trail could explain.
//!
//! The store is immediately consistent, so `AtLeast(token)` is satisfied without waiting — except
//! for a token this store never issued, which is reported rather than quietly accepted. A projected
//! backend would block here instead.

use aep_contract::consistency::QueryConsistency;
use aep_contract::error::QueryError;
use aep_contract::query::{
    AuditQuery, EntityEnvelope, EntityQuery, Page, QueryService, Relation, RelationQuery,
    RevisionRecord,
};
use aep_contract::registry::{CommandDescriptor, TypeDescriptor};
use aep_domain::artifact::ArtifactKind;
use aep_domain::audit::AuditRecord;
use aep_domain::entity::{Entity, EntityId, EntityLocator, EntityRef, EntityType};
use aep_domain::node::Node;

use crate::store::{Store, StoredEntity};
use crate::MemoryBackend;

impl QueryService for MemoryBackend {
    type AuditRecord = AuditRecord;

    async fn get(
        &self,
        reference: &EntityRef,
        consistency: QueryConsistency,
    ) -> Result<EntityEnvelope, QueryError> {
        self.with_store(|store| {
            check_consistency(store, &consistency)?;
            store
                .entity(&reference.id)
                .map(envelope)
                .ok_or_else(|| QueryError::NotFound {
                    what: format!("entity `{reference}`"),
                })
        })
    }

    async fn resolve(&self, locator: &EntityLocator) -> Result<EntityId, QueryError> {
        self.with_store(|store| {
            store
                .resolve(locator)
                .cloned()
                .ok_or_else(|| QueryError::NotFound {
                    what: format!("locator `{locator}`"),
                })
        })
    }

    async fn query(&self, query: &EntityQuery) -> Result<Page<EntityEnvelope>, QueryError> {
        self.with_store(|store| {
            check_consistency(store, &query.consistency)?;

            let related: Option<Vec<EntityId>> = query.related_to.as_ref().map(|anchor| {
                store
                    .relations()
                    .filter(|relation| {
                        query.relation.is_none_or(|kind| relation.kind == kind)
                            && relation.source.id == anchor.id
                    })
                    .map(|relation| relation.target.id.clone())
                    .collect()
            });

            let items: Vec<EntityEnvelope> = store
                .entities()
                .filter(|entity| {
                    query
                        .entity_type
                        .as_ref()
                        .is_none_or(|wanted| &entity.metadata.entity_type == wanted)
                })
                .filter(|entity| {
                    query
                        .organisation
                        .as_ref()
                        .is_none_or(|wanted| entity.metadata.locator.organisation() == wanted)
                })
                .filter(|entity| {
                    query
                        .space
                        .as_ref()
                        .is_none_or(|wanted| entity.metadata.locator.space() == wanted)
                })
                .filter(|entity| matches_body(entity, query))
                .filter(|entity| {
                    related
                        .as_ref()
                        .is_none_or(|ids| ids.contains(&entity.metadata.id))
                })
                .map(envelope)
                .collect();

            Ok(paginate(items, query.limit))
        })
    }

    async fn relations(&self, query: &RelationQuery) -> Result<Page<Relation>, QueryError> {
        self.with_store(|store| {
            check_consistency(store, &query.consistency)?;
            let items: Vec<Relation> = store
                .relations()
                .filter(|relation| {
                    query
                        .source
                        .as_ref()
                        .is_none_or(|source| relation.source.id == source.id)
                })
                .filter(|relation| {
                    query
                        .target
                        .as_ref()
                        .is_none_or(|target| relation.target.id == target.id)
                })
                .filter(|relation| query.kind.is_none_or(|kind| relation.kind == kind))
                .cloned()
                .collect();
            Ok(paginate(items, query.limit))
        })
    }

    async fn history(&self, reference: &EntityRef) -> Result<Vec<RevisionRecord>, QueryError> {
        self.with_store(|store| {
            if store.entity(&reference.id).is_none() {
                return Err(QueryError::NotFound {
                    what: format!("entity `{reference}`"),
                });
            }
            Ok(store.history(&reference.id).to_vec())
        })
    }

    async fn audit(&self, query: &AuditQuery) -> Result<Page<Self::AuditRecord>, QueryError> {
        self.with_store(|store| {
            let items: Vec<AuditRecord> = store
                .audit()
                .iter()
                .filter(|record| {
                    query.entity.as_ref().is_none_or(|entity| {
                        record
                            .subject
                            .as_ref()
                            .is_some_and(|subject| subject.id == entity.id)
                    })
                })
                .filter(|record| {
                    query
                        .correlation_id
                        .as_ref()
                        .is_none_or(|correlation| &record.correlation_id == correlation)
                })
                .filter(|record| {
                    query
                        .command_id
                        .as_ref()
                        .is_none_or(|command| record.command_id.as_ref() == Some(command))
                })
                .filter(|record| {
                    query
                        .actor
                        .as_ref()
                        .is_none_or(|actor| &record.actor == actor)
                })
                .filter(|record| {
                    query
                        .kind
                        .as_ref()
                        .is_none_or(|kind| record.kind.as_str() == kind)
                })
                .filter(|record| query.since.is_none_or(|since| record.occurred_at >= since))
                .filter(|record| query.until.is_none_or(|until| record.occurred_at < until))
                .filter(|record| !query.rejected_only || record.is_rejection())
                .cloned()
                .collect();
            Ok(paginate(items, query.limit))
        })
    }

    async fn describe_type(&self, entity_type: &EntityType) -> Result<TypeDescriptor, QueryError> {
        let kind = ArtifactKind::NAMED
            .iter()
            .find(|kind| &kind.entity_type() == entity_type)
            .ok_or_else(|| QueryError::NotFound {
                what: format!("type `{entity_type}`"),
            })?;

        let mut descriptor = TypeDescriptor::new(
            entity_type.clone(),
            format!("An artifact of kind `{kind}`."),
        );
        // A review result records what someone concluded at a moment. A record that can be edited
        // afterwards is not evidence, so the type says so and the commands stop at archival.
        descriptor.mutable = *kind != ArtifactKind::ReviewResult;
        descriptor.commands = vec![
            CommandDescriptor {
                command_type: "aep.entity.update/v1".to_owned(),
                summary: "Change fields of the entity.".to_owned(),
                revision_guarded: true,
            },
            CommandDescriptor {
                command_type: "aep.entity.archive/v1".to_owned(),
                summary: "Archive the entity; nothing is deleted.".to_owned(),
                revision_guarded: true,
            },
        ];
        if kind.is_a(&ArtifactKind::Design) {
            descriptor.commands.push(CommandDescriptor {
                command_type: "aep.design.approve/v1".to_owned(),
                summary: "Approve the design against a review of this revision.".to_owned(),
                revision_guarded: true,
            });
        }
        if *kind == ArtifactKind::ArchitectureDecisionRecord {
            descriptor.commands.push(CommandDescriptor {
                command_type: "aep.adr.accept/v1".to_owned(),
                summary: "Accept the decision, optionally superseding an earlier one.".to_owned(),
                revision_guarded: true,
            });
        }
        Ok(descriptor)
    }
}

/// Wraps a stored entity for the wire.
fn envelope(entity: &StoredEntity) -> EntityEnvelope {
    Entity::new(entity.metadata.clone(), entity.data.clone())
}

/// `true` when an entity's body matches every `matching` clause.
fn matches_body(entity: &StoredEntity, query: &EntityQuery) -> bool {
    if query.matching.is_empty() {
        return true;
    }
    let Node::Map(entries) = &entity.data else {
        return false;
    };
    query
        .matching
        .iter()
        .all(|(key, wanted)| entries.get(key) == Some(wanted))
}

/// Refuses a read that demands a point this store has not reached.
fn check_consistency(store: &Store, consistency: &QueryConsistency) -> Result<(), QueryError> {
    match consistency.token() {
        None => Ok(()),
        Some(token) if store.has_reached(token) => Ok(()),
        Some(token) => Err(QueryError::ConsistencyTimeout {
            token: token.to_string(),
        }),
    }
}

/// Applies a limit, reporting whether more results were available.
fn paginate<T>(mut items: Vec<T>, limit: Option<usize>) -> Page<T> {
    match limit {
        Some(limit) if items.len() > limit => {
            items.truncate(limit);
            Page {
                items,
                next: Some(aep_contract::query::Cursor(format!("offset-{limit}"))),
            }
        }
        _ => Page::complete(items),
    }
}
