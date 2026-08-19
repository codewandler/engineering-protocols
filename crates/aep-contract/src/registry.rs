//! Type discovery.
//!
//! A generic harness has to be able to ask what it is looking at:
//!
//! ```text
//! What is a Design?
//! Which fields are required?
//! Which commands can target it?
//! Which relations may it have?
//! Is it immutable?
//! Which lifecycle states exist?
//! ```
//!
//! Answering those from data rather than from a match arm is what keeps a harness working when an
//! organisation adds an entity type this repository never heard of.

use aep_domain::artifact::{ArtifactStatus, RelationKind};
use aep_domain::entity::EntityType;

/// What a type's lifecycle allows.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct LifecycleDescriptor {
    /// Where a new one starts.
    pub initial: ArtifactStatus,
    /// Every status it may hold.
    pub statuses: Vec<ArtifactStatus>,
    /// Which moves are legal, as `from -> [to]`.
    pub transitions: Vec<(ArtifactStatus, Vec<ArtifactStatus>)>,
}

/// A command this type accepts.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct CommandDescriptor {
    /// Its versioned wire name, such as `aep.design.approve/v1`.
    pub command_type: String,
    /// What it does, in one line.
    pub summary: String,
    /// Whether it asserts a revision, and so cannot silently overwrite.
    pub revision_guarded: bool,
}

/// A relation this type may have.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct RelationDescriptor {
    /// Which relation.
    pub kind: RelationKind,
    /// What it may point at.
    pub target_types: Vec<EntityType>,
    /// Whether the relation is required for the entity to be usable.
    #[serde(default)]
    pub required: bool,
}

/// Everything a harness needs in order to work with a type it has never seen.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct TypeDescriptor {
    /// Which type this describes.
    pub entity_type: EntityType,
    /// A one-line description.
    pub summary: String,
    /// Where its JSON Schema can be fetched, when one is published.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,
    /// Its lifecycle, for types that have one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lifecycle: Option<LifecycleDescriptor>,
    /// The commands that may target it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub commands: Vec<CommandDescriptor>,
    /// The relations it may have.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub relations: Vec<RelationDescriptor>,
    /// Whether it may be changed at all.
    ///
    /// A review result is immutable: it records what someone concluded at a moment, and a record
    /// that can be edited afterwards is not evidence.
    pub mutable: bool,
}

impl TypeDescriptor {
    /// A minimal descriptor for a mutable type.
    pub fn new(entity_type: EntityType, summary: impl Into<String>) -> Self {
        Self {
            entity_type,
            summary: summary.into(),
            schema: None,
            lifecycle: None,
            commands: Vec::new(),
            relations: Vec::new(),
            mutable: true,
        }
    }

    /// `true` when `command_type` may target this type.
    pub fn accepts(&self, command_type: &str) -> bool {
        self.commands
            .iter()
            .any(|command| command.command_type == command_type)
    }

    /// `true` when `kind` is a legal relation for this type.
    pub fn permits_relation(&self, kind: RelationKind) -> bool {
        self.relations.iter().any(|relation| relation.kind == kind)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_descriptor_answers_what_may_target_a_type() {
        let mut descriptor = TypeDescriptor::new(
            "aep.design/v1".parse().expect("type"),
            "A proposed solution to a specification.",
        );
        descriptor.commands.push(CommandDescriptor {
            command_type: "aep.design.approve/v1".to_owned(),
            summary: "Approve a design against a review.".to_owned(),
            revision_guarded: true,
        });
        descriptor.relations.push(RelationDescriptor {
            kind: RelationKind::Designs,
            target_types: vec!["aep.specification/v1".parse().expect("type")],
            required: true,
        });

        assert!(descriptor.accepts("aep.design.approve/v1"));
        assert!(!descriptor.accepts("aep.adr.accept/v1"));
        assert!(descriptor.permits_relation(RelationKind::Designs));
        assert!(!descriptor.permits_relation(RelationKind::Decides));
    }

    #[test]
    fn an_immutable_type_says_so() {
        let mut review = TypeDescriptor::new(
            "aep.review-result/v1".parse().expect("type"),
            "What a reviewer concluded, at a moment.",
        );
        review.mutable = false;
        assert!(!review.mutable);

        let json = serde_json::to_value(&review).expect("serialises");
        assert_eq!(json["mutable"], false);
    }
}
