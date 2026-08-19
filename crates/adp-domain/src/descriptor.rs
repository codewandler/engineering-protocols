//! Type discovery for the development types (§47).
//!
//! A harness that has never heard of a test plan still has to be able to ask what one is: which
//! commands may target it, which relations it may hold, whether it can be changed at all. Answering
//! that from data rather than from a match arm is what lets an organisation add a type this
//! repository never shipped — and it is why the answers live here, next to the types, rather than
//! inside `aep-contract`, which has no business knowing what development work is.
//!
//! # What a descriptor's `commands` list means
//!
//! It lists the commands that **target** the type, which is a narrower claim than "commands you
//! might issue while working with one". Two consequences worth stating, because both look like
//! omissions:
//!
//! * `aep.entity.create/v1` appears nowhere. Creation has no target — the type is named in its
//!   payload — so listing it here would be answering a different question. A harness learns a type
//!   is creatable from its lifecycle's initial status.
//! * `adp.test-plan.record/v1` appears on the **specification** and not on the test plan. The
//!   command attaches a plan to a subject; the subject is what changes. A descriptor that listed it
//!   under `adp.test-plan/v1` would send a harness to ask the plan for permission to record itself.
//!
//! # What is not described here
//!
//! [`crate::command::CommandKind::StartStory`] and [`CompleteStory`](crate::command::CompleteStory)
//! target `aep.story/v1`, which is a base-protocol type: a profile adding commands to an existing
//! type does not thereby own its descriptor. Those two are listed in this crate's command
//! vocabulary and will need to reach the story's descriptor wherever it comes to live.

use aep_contract::registry::{
    CommandDescriptor, LifecycleDescriptor, RelationDescriptor, TypeDescriptor,
};
use aep_domain::artifact::{ArtifactStatus, RelationKind};
use aep_domain::entity::{EntityBody, EntityType};

use crate::body::{AcceptanceCriteria, ChangeSet, Specification, TestPlan};
use crate::command::CommandKind;

/// Everything a harness needs in order to work with ADP's four entity types.
///
/// Ordered specification → test plan → acceptance criteria → change, which is the order the work
/// produces them in.
pub fn type_descriptors() -> Vec<TypeDescriptor> {
    vec![
        specification(),
        test_plan(),
        acceptance_criteria(),
        change(),
    ]
}

/// `adp.specification/v1`.
fn specification() -> TypeDescriptor {
    let mut descriptor = TypeDescriptor::new(
        Specification::entity_type(),
        "What the work must do, as individually addressable requirements.",
    );
    // The published ladder from `artifacts/lifecycles/specification.yaml`. It is reproduced rather
    // than invented: review is the only way out of draft, so a specification cannot become approved
    // by being edited.
    descriptor.lifecycle = Some(LifecycleDescriptor {
        initial: ArtifactStatus::Draft,
        statuses: vec![
            ArtifactStatus::Draft,
            ArtifactStatus::InReview,
            ArtifactStatus::Approved,
            ArtifactStatus::Implemented,
            ArtifactStatus::Rejected,
            ArtifactStatus::Superseded,
            ArtifactStatus::Archived,
        ],
        transitions: vec![
            (
                ArtifactStatus::Draft,
                vec![ArtifactStatus::InReview, ArtifactStatus::Archived],
            ),
            (
                ArtifactStatus::InReview,
                vec![
                    ArtifactStatus::Draft,
                    ArtifactStatus::Approved,
                    ArtifactStatus::Rejected,
                ],
            ),
            (
                ArtifactStatus::Approved,
                vec![
                    ArtifactStatus::Implemented,
                    ArtifactStatus::Superseded,
                    ArtifactStatus::Archived,
                ],
            ),
            (
                ArtifactStatus::Implemented,
                vec![ArtifactStatus::Superseded, ArtifactStatus::Archived],
            ),
            (ArtifactStatus::Rejected, vec![ArtifactStatus::Archived]),
            (ArtifactStatus::Superseded, vec![ArtifactStatus::Archived]),
            (ArtifactStatus::Archived, Vec::new()),
        ],
    });
    descriptor.commands = vec![
        development(
            CommandKind::SatisfySpecification,
            "Declare it satisfied, naming the evidence.",
        ),
        development(
            CommandKind::RecordTestPlan,
            "Attach the test plan that will judge it.",
        ),
        generic(
            "aep.entity.update/v1",
            "Change its ordinary mutable fields — never its status.",
        ),
        generic("aep.entity.archive/v1", "Take it out of active use."),
        generic("aep.entity.supersede/v1", "Replace it with a successor."),
    ];
    descriptor.relations = vec![
        RelationDescriptor {
            kind: RelationKind::Specifies,
            target_types: vec![story_type(), design_type()],
            required: false,
        },
        RelationDescriptor {
            kind: RelationKind::Supersedes,
            target_types: vec![Specification::entity_type()],
            required: false,
        },
    ];
    descriptor
}

/// `adp.test-plan/v1`.
fn test_plan() -> TypeDescriptor {
    let mut descriptor = TypeDescriptor::new(
        TestPlan::entity_type(),
        "How the work will be shown to satisfy what it was asked to do.",
    );
    descriptor.commands = vec![
        generic(
            "aep.entity.update/v1",
            "Change its ordinary mutable fields as the plan is refined.",
        ),
        generic("aep.entity.archive/v1", "Take it out of active use."),
        generic(
            "aep.entity.supersede/v1",
            "Replace it with a rewritten plan.",
        ),
    ];
    descriptor.relations = vec![RelationDescriptor {
        kind: RelationKind::Verifies,
        target_types: vec![
            Specification::entity_type(),
            AcceptanceCriteria::entity_type(),
            story_type(),
        ],
        // Required, for the same reason a review with no `reviews` edge is rejected: a plan that
        // does not say what it tests cannot be evidence for anything.
        required: true,
    }];
    descriptor
}

/// `adp.acceptance-criteria/v1`.
fn acceptance_criteria() -> TypeDescriptor {
    let mut descriptor = TypeDescriptor::new(
        AcceptanceCriteria::entity_type(),
        "The conditions under which one story is accepted as finished.",
    );
    descriptor.commands = vec![
        development(
            CommandKind::RecordTestPlan,
            "Attach the test plan that will judge the criteria.",
        ),
        generic(
            "aep.entity.update/v1",
            "Change its ordinary mutable fields.",
        ),
        generic("aep.entity.archive/v1", "Take it out of active use."),
    ];
    descriptor.relations = vec![
        RelationDescriptor {
            kind: RelationKind::Specifies,
            target_types: vec![story_type()],
            // Criteria that name no story accept nothing.
            required: true,
        },
        RelationDescriptor {
            kind: RelationKind::DerivedFrom,
            target_types: vec![Specification::entity_type()],
            required: false,
        },
    ];
    descriptor
}

/// `adp.change/v1`.
fn change() -> TypeDescriptor {
    let mut descriptor = TypeDescriptor::new(
        ChangeSet::entity_type(),
        "A recorded implementation: what changed, and what it was for.",
    );
    // Not mutable, for the reason a review result is not: it records what happened at a moment, and
    // a record that can be improved afterwards cannot be provenance. `aep.entity.update/v1` and
    // `aep.entity.supersede/v1` are therefore absent — a later change is a new record, not an
    // edit — while archiving stays, because taking a record out of active use does not rewrite it.
    descriptor.mutable = false;
    descriptor.commands = vec![generic(
        "aep.entity.archive/v1",
        "Take the record out of active use without altering it.",
    )];
    descriptor.relations = vec![RelationDescriptor {
        kind: RelationKind::Implements,
        target_types: vec![
            Specification::entity_type(),
            AcceptanceCriteria::entity_type(),
            design_type(),
        ],
        required: false,
    }];
    descriptor
}

/// A development command, which is always revision-guarded.
fn development(kind: CommandKind, summary: &str) -> CommandDescriptor {
    CommandDescriptor {
        command_type: kind.as_str().to_owned(),
        summary: summary.to_owned(),
        revision_guarded: true,
    }
}

/// A generic AEP command.
///
/// None of them are revision-guarded: they carry no versioned reference, so a caller that needs the
/// guarantee sends `expected_revision` in the envelope (§41).
fn generic(command_type: &str, summary: &str) -> CommandDescriptor {
    CommandDescriptor {
        command_type: command_type.to_owned(),
        summary: summary.to_owned(),
        revision_guarded: false,
    }
}

/// The base protocol's story type, which ADP's commands target but do not define.
fn story_type() -> EntityType {
    "aep.story/v1".parse().expect("a well-formed entity type")
}

/// The base protocol's design type.
fn design_type() -> EntityType {
    "aep.design/v1".parse().expect("a well-formed entity type")
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    fn descriptor_for(entity_type: &EntityType) -> TypeDescriptor {
        type_descriptors()
            .into_iter()
            .find(|descriptor| &descriptor.entity_type == entity_type)
            .unwrap_or_else(|| panic!("no descriptor for {entity_type}"))
    }

    #[test]
    fn every_body_this_crate_defines_is_discoverable() {
        let described: BTreeSet<String> = type_descriptors()
            .iter()
            .map(|descriptor| descriptor.entity_type.to_string())
            .collect();
        let defined: BTreeSet<String> = [
            Specification::entity_type(),
            TestPlan::entity_type(),
            AcceptanceCriteria::entity_type(),
            ChangeSet::entity_type(),
        ]
        .iter()
        .map(ToString::to_string)
        .collect();
        assert_eq!(
            described, defined,
            "a body with no descriptor is invisible to a generic harness"
        );
    }

    #[test]
    fn every_advertised_command_exists_in_a_command_vocabulary() {
        for descriptor in type_descriptors() {
            for command in &descriptor.commands {
                let known = CommandKind::parse(&command.command_type).is_ok()
                    || aep_domain::CommandKind::parse(&command.command_type).is_ok();
                assert!(
                    known,
                    "{} advertises `{}`, which no command implements — a harness would build a \
                     request nobody can answer",
                    descriptor.entity_type, command.command_type
                );
            }
        }
    }

    #[test]
    fn every_development_command_advertised_is_revision_guarded() {
        for descriptor in type_descriptors() {
            for command in &descriptor.commands {
                if CommandKind::parse(&command.command_type).is_ok() {
                    assert!(
                        command.revision_guarded,
                        "`{}` names a versioned reference, so the descriptor must say so",
                        command.command_type
                    );
                }
            }
        }
    }

    #[test]
    fn recording_a_test_plan_is_advertised_on_the_subject_not_on_the_plan() {
        let plan = descriptor_for(&TestPlan::entity_type());
        assert!(
            !plan.accepts(CommandKind::RecordTestPlan.as_str()),
            "the command targets the subject; a plan cannot record itself"
        );
        assert!(descriptor_for(&Specification::entity_type())
            .accepts(CommandKind::RecordTestPlan.as_str()));
        assert!(descriptor_for(&AcceptanceCriteria::entity_type())
            .accepts(CommandKind::RecordTestPlan.as_str()));
    }

    #[test]
    fn only_a_specification_can_be_declared_satisfied() {
        let satisfy = CommandKind::SatisfySpecification.as_str();
        assert!(descriptor_for(&Specification::entity_type()).accepts(satisfy));
        for other in [
            TestPlan::entity_type(),
            AcceptanceCriteria::entity_type(),
            ChangeSet::entity_type(),
        ] {
            assert!(
                !descriptor_for(&other).accepts(satisfy),
                "{other} is not a specification"
            );
        }
    }

    #[test]
    fn a_recorded_change_cannot_be_edited_afterwards() {
        let change = descriptor_for(&ChangeSet::entity_type());
        assert!(
            !change.mutable,
            "a record that can be improved later is not provenance"
        );
        assert!(!change.accepts("aep.entity.update/v1"));
        assert!(!change.accepts("aep.entity.supersede/v1"));
        assert!(
            change.accepts("aep.entity.archive/v1"),
            "retiring a record does not rewrite it"
        );
    }

    #[test]
    fn a_test_plan_must_say_what_it_verifies() {
        let plan = descriptor_for(&TestPlan::entity_type());
        assert!(plan.permits_relation(RelationKind::Verifies));
        let verifies = plan
            .relations
            .iter()
            .find(|relation| relation.kind == RelationKind::Verifies)
            .expect("the relation is declared");
        assert!(
            verifies.required,
            "a plan that names no subject is evidence for nothing"
        );
        assert!(verifies
            .target_types
            .contains(&Specification::entity_type()));
    }

    #[test]
    fn a_specification_cannot_become_approved_by_being_edited() {
        let lifecycle = descriptor_for(&Specification::entity_type())
            .lifecycle
            .expect("the specification lifecycle is published");
        assert_eq!(lifecycle.initial, ArtifactStatus::Draft);
        let from_draft = lifecycle
            .transitions
            .iter()
            .find(|(from, _)| *from == ArtifactStatus::Draft)
            .map(|(_, to)| to.clone())
            .expect("draft has outgoing transitions");
        assert!(from_draft.contains(&ArtifactStatus::InReview));
        assert!(
            !from_draft.contains(&ArtifactStatus::Approved),
            "review is the only way out of draft"
        );
    }

    #[test]
    fn no_descriptor_advertises_creation_because_creation_targets_nothing() {
        for descriptor in type_descriptors() {
            assert!(
                !descriptor.accepts("aep.entity.create/v1"),
                "{} lists a command that has no target",
                descriptor.entity_type
            );
        }
    }
}
