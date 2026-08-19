//! The command side of the interaction contract: every operation that changes state.
//!
//! Nothing in AEP mutates except through a [`Command`] (§35). That is what makes the audit trail
//! complete rather than best-effort — there is no second path by which a design quietly becomes
//! approved — and it is why this module is small on purpose: the vocabulary of change is meant to
//! be readable in one sitting.
//!
//! # Why the semantic commands exist
//!
//! A generic `PATCH status = "approved"` is trivial to offer and impossible to check. The backend
//! sees a field name and a value; it cannot ask whether anybody actually reviewed anything, so the
//! only answer it can give is "yes". [`ApproveDesign`] carries the review itself, and the same
//! transition becomes a list of questions that have answers (§42):
//!
//! * the review exists;
//! * it targets *this* revision of the design, not the one that has since been rewritten;
//! * its disposition is approval, rather than approval-with-changes;
//! * the actor holds the capability to approve;
//! * the workflow permits an approval from the state the design is in.
//!
//! None of those can be asked of a field assignment. [`UpdateEntity`] therefore stays deliberately
//! dull — a title, an owner, a tag, the ordinary mutable fields — and every lifecycle transition
//! gets a command that names it.
//!
//! # Why nothing is deleted
//!
//! There is no delete command, and adding one would be a design change rather than a feature
//! (§43). An engineering record whose history can be erased is not a record: the worth of an ADR
//! is precisely that the decision it reversed is still readable, and the worth of a superseded
//! design is that the reader can see what was tried. [`ArchiveEntity`] takes an entity out of
//! active use and [`SupersedeEntity`] says what replaced it. Both leave it addressable and its
//! history intact. Reclaiming physical storage is a backend's business and no part of the logical
//! contract.
//!
//! # Structure and semantics
//!
//! A [`Command`] deserializes structurally: shape, types, no unknown fields.
//! [`Command::validate`] is the semantic pass that runs without a backend, and it can only catch
//! the contradictions visible in the command itself — an entity superseding itself, an update that
//! updates nothing. Everything in the list above needs stored state, and belongs to the layer that
//! has it.

use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;

use crate::artifact::RelationKind;
use crate::capability::Capability;
use crate::entity::{
    ActorRef, EntityLocator, EntityRef, EntityRevision, EntityType, VersionedEntityRef,
};
use crate::error::{ParseError, ValidationCode, ValidationError, ValidationErrors};
use crate::ids::RelationId;
use crate::node::Node;

/// The versioned wire name of a command, such as `aep.entity.create/v1`.
///
/// Command types are versioned because they are a published interface (§36): a backend that
/// implements `aep.design.approve/v1` keeps implementing it after `v2` adds a field, and a client
/// that only speaks `v1` is told so instead of having its payload silently reinterpreted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum CommandKind {
    /// Create an entity.
    CreateEntity,
    /// Change an entity's ordinary mutable fields.
    UpdateEntity,
    /// Record a relation between two entities.
    CreateRelation,
    /// Remove a relation.
    RemoveRelation,
    /// Take an entity out of active use.
    ArchiveEntity,
    /// Replace an entity with a successor.
    SupersedeEntity,
    /// Ask for a review of a design revision.
    SubmitDesignReview,
    /// Approve a design revision on the strength of a review.
    ApproveDesign,
    /// Accept an architecture decision record.
    AcceptAdr,
}

impl CommandKind {
    /// Every command kind, in the order §103 lists them.
    ///
    /// Generic commands first, then domain commands; diagnostics and generated vocabulary
    /// listings read in that order too.
    pub const ALL: &'static [Self] = &[
        Self::CreateEntity,
        Self::UpdateEntity,
        Self::CreateRelation,
        Self::RemoveRelation,
        Self::ArchiveEntity,
        Self::SupersedeEntity,
        Self::SubmitDesignReview,
        Self::ApproveDesign,
        Self::AcceptAdr,
    ];

    /// The versioned name as it appears on the wire.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CreateEntity => "aep.entity.create/v1",
            Self::UpdateEntity => "aep.entity.update/v1",
            Self::CreateRelation => "aep.relation.create/v1",
            Self::RemoveRelation => "aep.relation.remove/v1",
            Self::ArchiveEntity => "aep.entity.archive/v1",
            Self::SupersedeEntity => "aep.entity.supersede/v1",
            Self::SubmitDesignReview => "aep.design.submit-review/v1",
            Self::ApproveDesign => "aep.design.approve/v1",
            Self::AcceptAdr => "aep.adr.accept/v1",
        }
    }

    /// Parses a versioned command name.
    ///
    /// The rejection lists the whole vocabulary, because the usual cause is a command that does
    /// not exist — `aep.entity.delete/v1` above all — and the fix is to pick one that does.
    pub fn parse(value: &str) -> Result<Self, ParseError> {
        Self::ALL
            .iter()
            .copied()
            .find(|kind| kind.as_str() == value)
            .ok_or_else(|| {
                ParseError::reference(
                    "command",
                    value,
                    format!(
                        "expected one of {}",
                        Self::ALL
                            .iter()
                            .map(|kind| kind.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                )
            })
    }
}

impl fmt::Display for CommandKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for CommandKind {
    type Err = ParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

impl serde::Serialize for CommandKind {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> serde::Deserialize<'de> for CommandKind {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        Self::parse(&raw).map_err(serde::de::Error::custom)
    }
}

impl schemars::JsonSchema for CommandKind {
    fn schema_name() -> String {
        "CommandKind".to_owned()
    }

    fn json_schema(_: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        let mut schema = schemars::schema::SchemaObject {
            instance_type: Some(schemars::schema::InstanceType::String.into()),
            enum_values: Some(
                Self::ALL
                    .iter()
                    .map(|kind| serde_json::Value::String(kind.as_str().to_owned()))
                    .collect(),
            ),
            ..Default::default()
        };
        schema.metadata().description = Some(
            "The versioned name of a command type, such as `aep.entity.create/v1`.".to_owned(),
        );
        schema.into()
    }
}

/// Bringing an entity into existence.
///
/// The identity is assigned by the backend, not by the caller: a client that could choose an
/// entity id could also reuse one, and identity that can be reused is not identity. The caller
/// supplies the locator — the address the organisation knows the thing by — and the backend
/// answers with the id.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct CreateEntity {
    /// What kind of entity it is, which decides its schema, lifecycle and permitted relations.
    pub entity_type: EntityType,
    /// The address it is to be known by.
    pub locator: EntityLocator,
    /// The body, in whatever shape the entity type declares.
    pub data: Node,
}

/// Changing an entity's ordinary mutable fields.
///
/// This is the structural update §42 permits, and it is not a lifecycle transition. A `status`
/// key here is a mistake: statuses move through the commands that name the move, which is what
/// gives the engine something to validate.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct UpdateEntity {
    /// The entity to change.
    pub target: EntityRef,
    /// The fields to set, keyed by field name.
    pub changes: BTreeMap<String, Node>,
}

/// Recording a relation between two entities.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct CreateRelation {
    /// What the edge means.
    pub kind: RelationKind,
    /// Where the edge starts; relation kinds are read source-first, `source specifies target`.
    pub source: EntityRef,
    /// Where the edge ends.
    pub target: EntityRef,
}

/// Removing a relation.
///
/// The edge goes; both entities stay. Relations are the one thing the protocol does remove
/// outright, because an edge asserted in error carries no history worth keeping — the entities it
/// joined keep theirs.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct RemoveRelation {
    /// The edge to remove.
    pub relation: RelationId,
}

/// Taking an entity out of active use without erasing it.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct ArchiveEntity {
    /// The entity to archive.
    pub target: EntityRef,
    /// Why, recorded in the audit trail.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Replacing an entity with a successor.
///
/// Distinct from archiving: archiving says "no longer in use", superseding says "use this
/// instead". The successor is required, so a reader who arrives at the old entity is never left
/// without the forwarding address.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct SupersedeEntity {
    /// The entity being replaced.
    pub target: EntityRef,
    /// What replaces it.
    pub successor: EntityRef,
    /// Why, recorded in the audit trail.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Asking for a review of one design revision.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct SubmitDesignReview {
    /// The exact revision to be reviewed; a review of "the design" would not survive the next
    /// edit.
    pub design: VersionedEntityRef,
    /// Who is asked.
    pub reviewer: ActorRef,
}

/// Approving one design revision on the strength of a recorded review.
///
/// The review reference is what separates this from `status = "approved"`: it is the thing the
/// engine checks against — that it exists, that it reviewed this revision, and that it says
/// approved.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct ApproveDesign {
    /// The exact revision being approved.
    pub design: VersionedEntityRef,
    /// The review that supports the approval.
    pub review: EntityRef,
}

/// Accepting an architecture decision record.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct AcceptAdr {
    /// The exact revision being accepted.
    pub adr: VersionedEntityRef,
    /// The decision this one reverses, when it reverses one.
    ///
    /// Carried on the acceptance rather than issued as a separate [`SupersedeEntity`] because the
    /// two facts are one decision: an ADR that supersedes another does so *by being accepted*, and
    /// splitting it in two leaves a window in which both are current.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supersedes: Option<EntityRef>,
}

/// A state-changing operation.
///
/// The generic variants are the ones every entity type supports; the domain variants exist
/// because their transitions carry conditions a field assignment cannot express. See the module
/// documentation for why both kinds are needed.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(tag = "command", rename_all = "kebab-case")]
#[non_exhaustive]
pub enum Command {
    /// Create an entity.
    CreateEntity(CreateEntity),
    /// Change an entity's ordinary mutable fields.
    UpdateEntity(UpdateEntity),
    /// Record a relation.
    CreateRelation(CreateRelation),
    /// Remove a relation.
    RemoveRelation(RemoveRelation),
    /// Take an entity out of active use.
    ArchiveEntity(ArchiveEntity),
    /// Replace an entity with a successor.
    SupersedeEntity(SupersedeEntity),
    /// Ask for a review of a design revision.
    SubmitDesignReview(SubmitDesignReview),
    /// Approve a design revision.
    ApproveDesign(ApproveDesign),
    /// Accept an architecture decision record.
    AcceptAdr(AcceptAdr),
}

impl Command {
    /// The versioned command type, for the envelope and for routing.
    pub fn kind(&self) -> CommandKind {
        match self {
            Self::CreateEntity(_) => CommandKind::CreateEntity,
            Self::UpdateEntity(_) => CommandKind::UpdateEntity,
            Self::CreateRelation(_) => CommandKind::CreateRelation,
            Self::RemoveRelation(_) => CommandKind::RemoveRelation,
            Self::ArchiveEntity(_) => CommandKind::ArchiveEntity,
            Self::SupersedeEntity(_) => CommandKind::SupersedeEntity,
            Self::SubmitDesignReview(_) => CommandKind::SubmitDesignReview,
            Self::ApproveDesign(_) => CommandKind::ApproveDesign,
            Self::AcceptAdr(_) => CommandKind::AcceptAdr,
        }
    }

    /// The entity this command mutates, where it mutates an existing one.
    ///
    /// `None` has two causes and they are different: [`Command::CreateEntity`] has no target
    /// because the entity does not exist yet, and [`Command::RemoveRelation`] has none because a
    /// relation is addressed by its own identifier rather than as an entity. A relation *created*
    /// does name its source, because the source is the entity whose neighbourhood changes and
    /// whose authorisation therefore applies.
    pub fn target(&self) -> Option<EntityRef> {
        match self {
            Self::CreateEntity(_) | Self::RemoveRelation(_) => None,
            Self::UpdateEntity(UpdateEntity { target, .. })
            | Self::ArchiveEntity(ArchiveEntity { target, .. })
            | Self::SupersedeEntity(SupersedeEntity { target, .. }) => Some(target.clone()),
            Self::CreateRelation(CreateRelation { source, .. }) => Some(source.clone()),
            Self::SubmitDesignReview(SubmitDesignReview { design, .. })
            | Self::ApproveDesign(ApproveDesign { design, .. }) => Some(design.unversioned()),
            Self::AcceptAdr(AcceptAdr { adr, .. }) => Some(adr.unversioned()),
        }
    }

    /// The revision this command asserts the target is currently at, where it asserts one.
    ///
    /// A domain command that names a revision *is* an optimistic-concurrency assertion (§41):
    /// approving `design@3` is a claim that 3 is still current, and if the design has moved to 4
    /// the approval must fail rather than land on text nobody approved. No separate
    /// `expected_revision` field is needed for those commands, and none should be accepted —
    /// two sources for one assertion is one source too many.
    pub fn expected_revision(&self) -> Option<EntityRevision> {
        match self {
            Self::CreateEntity(_)
            | Self::UpdateEntity(_)
            | Self::CreateRelation(_)
            | Self::RemoveRelation(_)
            | Self::ArchiveEntity(_)
            | Self::SupersedeEntity(_) => None,
            Self::SubmitDesignReview(SubmitDesignReview { design, .. })
            | Self::ApproveDesign(ApproveDesign { design, .. }) => Some(design.revision),
            Self::AcceptAdr(AcceptAdr { adr, .. }) => Some(adr.revision),
        }
    }

    /// `true` for every command there is.
    ///
    /// Reads as a tautology and is meant to: commands are the mutating half of the contract and
    /// queries are the other half (§44), so the contract layer can ask this instead of keeping its
    /// own list of which operations write. The arm is exhaustive rather than a bare `true` so that
    /// a future read-only command has to answer the question here instead of inheriting an answer.
    pub fn is_mutating(&self) -> bool {
        match self {
            Self::CreateEntity(_)
            | Self::UpdateEntity(_)
            | Self::CreateRelation(_)
            | Self::RemoveRelation(_)
            | Self::ArchiveEntity(_)
            | Self::SupersedeEntity(_)
            | Self::SubmitDesignReview(_)
            | Self::ApproveDesign(_)
            | Self::AcceptAdr(_) => true,
        }
    }

    /// The single capability this command requires.
    ///
    /// [`Command::ApproveDesign`] and [`Command::AcceptAdr`] need `artifact.write`, not
    /// `approval.request`: `approval.request` is the capability to *ask a human* to decide, while
    /// these two record a decision already taken and change what the artifact says. Granting an
    /// agent the right to raise approval requests must never also grant it the right to answer
    /// them. [`Command::SubmitDesignReview`] is the one command here that only asks.
    pub fn required_capability(&self) -> Capability {
        match self {
            Self::SubmitDesignReview(_) => Capability::ReviewRequest,
            Self::CreateEntity(_)
            | Self::UpdateEntity(_)
            | Self::CreateRelation(_)
            | Self::RemoveRelation(_)
            | Self::ArchiveEntity(_)
            | Self::SupersedeEntity(_)
            | Self::ApproveDesign(_)
            | Self::AcceptAdr(_) => Capability::ArtifactWrite,
        }
    }

    /// A one-line description for audit records and explanations.
    pub fn summary(&self) -> String {
        match self {
            Self::CreateEntity(CreateEntity {
                entity_type,
                locator,
                ..
            }) => format!("create {entity_type} at {locator}"),
            Self::UpdateEntity(UpdateEntity { target, changes }) => format!(
                "update {target} ({})",
                changes
                    .keys()
                    .map(String::as_str)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Self::CreateRelation(CreateRelation {
                kind,
                source,
                target,
            }) => format!("relate {source} {kind} {target}"),
            Self::RemoveRelation(RemoveRelation { relation }) => {
                format!("remove relation {relation}")
            }
            Self::ArchiveEntity(ArchiveEntity { target, reason }) => match reason {
                Some(reason) => format!("archive {target}: {reason}"),
                None => format!("archive {target}"),
            },
            Self::SupersedeEntity(SupersedeEntity {
                target,
                successor,
                reason,
            }) => match reason {
                Some(reason) => format!("supersede {target} by {successor}: {reason}"),
                None => format!("supersede {target} by {successor}"),
            },
            Self::SubmitDesignReview(SubmitDesignReview { design, reviewer }) => {
                format!("submit {design} for review by {reviewer}")
            }
            Self::ApproveDesign(ApproveDesign { design, review }) => {
                format!("approve {design} on review {review}")
            }
            Self::AcceptAdr(AcceptAdr { adr, supersedes }) => match supersedes {
                Some(superseded) => format!("accept ADR {adr}, superseding {superseded}"),
                None => format!("accept ADR {adr}"),
            },
        }
    }

    /// Checks what can be checked without a backend.
    ///
    /// Deliberately a short list. Everything interesting about a command — does the review exist,
    /// does the workflow permit this transition, is the revision still current — needs stored
    /// state, and belongs to the layer that has it. What is left is the set of commands that
    /// contradict themselves, and those are worth refusing at the edge because the diagnostic is
    /// so much better there than after a round trip.
    ///
    /// Validation accumulates, as everywhere else: the result reports every problem found.
    pub fn validate(&self) -> Result<(), ValidationErrors> {
        let mut errors = ValidationErrors::new();
        match self {
            Self::UpdateEntity(UpdateEntity { changes, .. }) => {
                if changes.is_empty() {
                    errors.push(
                        ValidationError::new(
                            ValidationCode::EmptyChange,
                            "command.update-entity.changes",
                            "an update must change at least one field",
                        )
                        .with_hint(
                            "an accepted empty update still advances the revision, and nobody \
                             reading the history afterwards can say what it did",
                        ),
                    );
                }
            }
            Self::CreateRelation(CreateRelation { source, target, .. }) => {
                if source == target {
                    errors.push(
                        ValidationError::new(
                            ValidationCode::SelfReference,
                            "command.create-relation.target",
                            format!("{source} cannot hold a relation to itself"),
                        )
                        .with_hint(
                            "every relation kind reads source-first; a self-edge asserts nothing",
                        ),
                    );
                }
            }
            Self::SupersedeEntity(SupersedeEntity {
                target, successor, ..
            }) => {
                if target == successor {
                    errors.push(
                        ValidationError::new(
                            ValidationCode::SelfReference,
                            "command.supersede-entity.successor",
                            format!("{target} cannot supersede itself"),
                        )
                        .with_hint(
                            "to record a new version of the same entity use an update; \
                             supersession points at a different entity",
                        ),
                    );
                }
            }
            Self::CreateEntity(_)
            | Self::RemoveRelation(_)
            | Self::ArchiveEntity(_)
            | Self::SubmitDesignReview(_)
            | Self::ApproveDesign(_)
            | Self::AcceptAdr(_) => {}
        }
        errors.into_result(())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;
    use crate::entity::EntityId;

    /// The design under discussion throughout these tests.
    const DESIGN: &str = "design-passkeys-0001";
    /// Its replacement.
    const SUCCESSOR: &str = "design-passkeys-0002";

    fn reference(id: &str) -> EntityRef {
        EntityRef::new(EntityId::new(id).expect("test entity ids are well formed"))
    }

    fn revision(value: u64) -> EntityRevision {
        EntityRevision::new(value).expect("test revisions are non-zero")
    }

    fn design_at(value: u64) -> VersionedEntityRef {
        reference(DESIGN).at(revision(value))
    }

    /// One command of every kind, so a new variant makes the coverage test fail.
    fn samples() -> Vec<Command> {
        vec![
            Command::CreateEntity(CreateEntity {
                entity_type: EntityType::new("aep", "design", 1).expect("a valid entity type"),
                locator: EntityLocator::new("acme", "payments", "design", "passkeys-auth")
                    .expect("a valid locator"),
                data: Node::Map(BTreeMap::from([(
                    "title".to_owned(),
                    Node::from("Passkey authentication"),
                )])),
            }),
            Command::UpdateEntity(UpdateEntity {
                target: reference(DESIGN),
                changes: BTreeMap::from([("owner".to_owned(), Node::from("human:alice"))]),
            }),
            Command::CreateRelation(CreateRelation {
                kind: RelationKind::Supersedes,
                source: reference(SUCCESSOR),
                target: reference(DESIGN),
            }),
            Command::RemoveRelation(RemoveRelation {
                relation: RelationId::new("rel-0001").expect("a valid relation id"),
            }),
            Command::ArchiveEntity(ArchiveEntity {
                target: reference(DESIGN),
                reason: Some("the feature was cancelled".to_owned()),
            }),
            Command::SupersedeEntity(SupersedeEntity {
                target: reference(DESIGN),
                successor: reference(SUCCESSOR),
                reason: Some("rewritten around device-bound keys".to_owned()),
            }),
            Command::SubmitDesignReview(SubmitDesignReview {
                design: design_at(3),
                reviewer: ActorRef::parse("human:bea").expect("a valid actor"),
            }),
            Command::ApproveDesign(ApproveDesign {
                design: design_at(3),
                review: reference("review-passkeys-0007"),
            }),
            Command::AcceptAdr(AcceptAdr {
                adr: reference("adr-passkeys-00001").at(revision(3)),
                supersedes: Some(reference("adr-passwords-0001")),
            }),
        ]
    }

    #[test]
    fn the_sample_set_covers_every_command_kind() {
        let covered: BTreeSet<CommandKind> = samples().iter().map(Command::kind).collect();
        assert_eq!(
            covered.len(),
            CommandKind::ALL.len(),
            "the samples miss a command kind: {covered:?}"
        );
    }

    #[test]
    fn wire_names_are_the_versioned_names_the_specification_lists() {
        assert_eq!(CommandKind::CreateEntity.as_str(), "aep.entity.create/v1");
        assert_eq!(CommandKind::UpdateEntity.as_str(), "aep.entity.update/v1");
        assert_eq!(
            CommandKind::CreateRelation.as_str(),
            "aep.relation.create/v1"
        );
        assert_eq!(
            CommandKind::RemoveRelation.as_str(),
            "aep.relation.remove/v1"
        );
        assert_eq!(CommandKind::ArchiveEntity.as_str(), "aep.entity.archive/v1");
        assert_eq!(
            CommandKind::SupersedeEntity.as_str(),
            "aep.entity.supersede/v1"
        );
        assert_eq!(
            CommandKind::SubmitDesignReview.as_str(),
            "aep.design.submit-review/v1"
        );
        assert_eq!(CommandKind::ApproveDesign.as_str(), "aep.design.approve/v1");
        assert_eq!(CommandKind::AcceptAdr.as_str(), "aep.adr.accept/v1");
    }

    #[test]
    fn every_wire_name_round_trips_through_parsing_and_serde() {
        for kind in CommandKind::ALL {
            let name = kind.as_str();
            assert_eq!(CommandKind::parse(name).expect(name), *kind);
            assert_eq!(name.parse::<CommandKind>().expect(name), *kind);
            assert_eq!(kind.to_string(), name);

            let json = serde_json::to_string(kind).expect("a command kind serializes");
            assert_eq!(json, format!("\"{name}\""));
            let read: CommandKind =
                serde_json::from_str(&json).expect("a command kind deserializes");
            assert_eq!(read, *kind);
        }
    }

    #[test]
    fn an_unrecognised_command_type_is_rejected_and_the_vocabulary_named() {
        let error =
            CommandKind::parse("aep.entity.delete/v1").expect_err("there is no delete command");
        let message = error.to_string();
        assert!(message.contains("aep.entity.delete/v1"), "{message}");
        assert!(message.contains("aep.entity.archive/v1"), "{message}");
        assert!(message.contains("aep.entity.supersede/v1"), "{message}");
    }

    #[test]
    fn every_command_round_trips_through_json_under_its_own_tag() {
        for command in samples() {
            let json = serde_json::to_value(&command).expect("a command serializes");
            let tag = json
                .get("command")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_else(|| panic!("no `command` tag in {json}"))
                .to_owned();
            assert!(
                tag.chars().all(|c| c.is_ascii_lowercase() || c == '-'),
                "the tag {tag:?} is not kebab-case"
            );
            let read: Command = serde_json::from_value(json.clone())
                .unwrap_or_else(|error| panic!("cannot read back {json}: {error}"));
            assert_eq!(read, command, "the round trip changed {json}");
        }
    }

    #[test]
    fn the_serde_tag_names_the_variant_in_kebab_case() {
        let json = serde_json::to_value(Command::SubmitDesignReview(SubmitDesignReview {
            design: design_at(3),
            reviewer: ActorRef::parse("human:bea").expect("a valid actor"),
        }))
        .expect("a command serializes");
        assert_eq!(json["command"], "submit-design-review");
        assert_eq!(json["design"], format!("{DESIGN}@3"));
        assert_eq!(json["reviewer"], "human:bea");
    }

    #[test]
    fn a_payload_field_the_protocol_does_not_define_is_rejected() {
        let json = serde_json::json!({
            "command": "archive-entity",
            "target": DESIGN,
            "urgency": "high",
        });
        let error =
            serde_json::from_value::<Command>(json).expect_err("unknown fields are rejected");
        assert!(error.to_string().contains("urgency"), "{error}");
    }

    #[test]
    fn only_a_command_naming_a_revision_asserts_one() {
        for command in samples() {
            let asserted = command.expected_revision();
            match command.kind() {
                CommandKind::SubmitDesignReview
                | CommandKind::ApproveDesign
                | CommandKind::AcceptAdr => assert_eq!(
                    asserted,
                    Some(revision(3)),
                    "{} should pin the revision it names",
                    command.summary()
                ),
                other => assert_eq!(
                    asserted, None,
                    "{other} names no revision, so it must not assert one"
                ),
            }
        }
    }

    #[test]
    fn approving_a_stale_revision_is_a_different_command_from_approving_the_current_one() {
        let stale = Command::ApproveDesign(ApproveDesign {
            design: design_at(3),
            review: reference("review-passkeys-0007"),
        });
        let current = Command::ApproveDesign(ApproveDesign {
            design: design_at(7),
            review: reference("review-passkeys-0007"),
        });
        assert_ne!(stale, current);
        assert_eq!(stale.expected_revision(), Some(revision(3)));
        assert_eq!(current.expected_revision(), Some(revision(7)));
        // Both address the same entity: the concurrency assertion lives in the revision, not in
        // the target.
        assert_eq!(stale.target(), current.target());
    }

    #[test]
    fn creating_an_entity_names_no_target_because_nothing_exists_yet() {
        let create = samples()
            .into_iter()
            .find(|command| command.kind() == CommandKind::CreateEntity)
            .expect("the samples include a creation");
        assert_eq!(create.target(), None);
    }

    #[test]
    fn a_relation_command_targets_the_entity_whose_edges_change() {
        let relate = Command::CreateRelation(CreateRelation {
            kind: RelationKind::Supersedes,
            source: reference(SUCCESSOR),
            target: reference(DESIGN),
        });
        assert_eq!(relate.target(), Some(reference(SUCCESSOR)));

        let remove = Command::RemoveRelation(RemoveRelation {
            relation: RelationId::new("rel-0001").expect("a valid relation id"),
        });
        assert_eq!(remove.target(), None);
    }

    #[test]
    fn asking_for_a_review_is_the_only_command_that_does_not_write_an_artifact() {
        for command in samples() {
            let expected = if command.kind() == CommandKind::SubmitDesignReview {
                Capability::ReviewRequest
            } else {
                Capability::ArtifactWrite
            };
            assert_eq!(
                command.required_capability(),
                expected,
                "wrong capability for `{}`",
                command.summary()
            );
        }
    }

    #[test]
    fn approving_a_design_needs_artifact_write_not_approval_request() {
        let approve = Command::ApproveDesign(ApproveDesign {
            design: design_at(3),
            review: reference("review-passkeys-0007"),
        });
        // `approval.request` is the right to ask a human to decide; approving records a decision
        // already taken. An agent that may raise approval requests must not thereby answer them.
        assert_eq!(approve.required_capability(), Capability::ArtifactWrite);
        assert_ne!(approve.required_capability(), Capability::ApprovalRequest);
    }

    #[test]
    fn every_command_mutates_state() {
        for command in samples() {
            assert!(
                command.is_mutating(),
                "`{}` is a command, so it changes state",
                command.summary()
            );
        }
    }

    #[test]
    fn a_summary_names_what_the_command_touched() {
        let supersede = Command::SupersedeEntity(SupersedeEntity {
            target: reference(DESIGN),
            successor: reference(SUCCESSOR),
            reason: Some("rewritten around device-bound keys".to_owned()),
        });
        let summary = supersede.summary();
        assert!(summary.contains(DESIGN), "{summary}");
        assert!(summary.contains(SUCCESSOR), "{summary}");
        assert!(summary.contains("device-bound keys"), "{summary}");

        let update = Command::UpdateEntity(UpdateEntity {
            target: reference(DESIGN),
            changes: BTreeMap::from([
                ("owner".to_owned(), Node::from("human:alice")),
                ("title".to_owned(), Node::from("Passkeys")),
            ]),
        });
        // The field names are what an audit reader needs: "update X" alone says nothing.
        assert_eq!(update.summary(), format!("update {DESIGN} (owner, title)"));
    }

    #[test]
    fn a_well_formed_command_has_nothing_to_report() {
        for command in samples() {
            command.validate().unwrap_or_else(|errors| {
                panic!("`{}` should validate: {errors}", command.summary())
            });
        }
    }

    #[test]
    fn an_entity_cannot_supersede_itself() {
        let command = Command::SupersedeEntity(SupersedeEntity {
            target: reference(DESIGN),
            successor: reference(DESIGN),
            reason: None,
        });
        let errors = command
            .validate()
            .expect_err("self-supersession is refused");
        assert_eq!(errors.len(), 1);
        let error = &errors.as_slice()[0];
        assert_eq!(error.code, ValidationCode::SelfReference);
        assert_eq!(error.location, "command.supersede-entity.successor");
        assert!(error.message.contains("supersede itself"), "{error}");
    }

    #[test]
    fn an_update_that_changes_nothing_is_refused() {
        let command = Command::UpdateEntity(UpdateEntity {
            target: reference(DESIGN),
            changes: BTreeMap::new(),
        });
        let errors = command.validate().expect_err("an empty update is refused");
        assert_eq!(errors.len(), 1);
        let error = &errors.as_slice()[0];
        assert_eq!(error.code, ValidationCode::EmptyChange);
        assert_eq!(error.location, "command.update-entity.changes");
        assert!(
            error
                .hint
                .as_deref()
                .is_some_and(|hint| hint.contains("revision")),
            "the hint should say why an empty update is harmful: {error}"
        );
    }

    #[test]
    fn an_entity_cannot_hold_a_relation_to_itself() {
        let command = Command::CreateRelation(CreateRelation {
            kind: RelationKind::DerivedFrom,
            source: reference(DESIGN),
            target: reference(DESIGN),
        });
        let errors = command.validate().expect_err("a self-edge is refused");
        assert_eq!(errors.len(), 1);
        let error = &errors.as_slice()[0];
        assert_eq!(error.code, ValidationCode::SelfReference);
        assert_eq!(error.location, "command.create-relation.target");
        assert!(error.message.contains(DESIGN), "{error}");
    }
}
