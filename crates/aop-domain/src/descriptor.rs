//! Type discovery for the operations types.
//!
//! A harness that has never heard of an incident should still be able to ask what one is, which
//! commands may target it and whether it can be changed (§47). [`type_descriptors`] answers from
//! data, so an operations harness needs no match arm per type.
//!
//! # Why two of the three publish no lifecycle
//!
//! [`aep_contract::registry::LifecycleDescriptor`] is expressed in
//! [`ArtifactStatus`](aep_domain::artifact::ArtifactStatus) — `draft`, `in_review`, `approved`,
//! `superseded` — which is the right vocabulary for a document and has no word for `canary`. A
//! [`Runbook`] is a document and gets a real lifecycle. An [`Incident`] and a [`Release`] are not:
//! their ladders are operational states, and mapping `triaged` onto `proposed` or `promoted` onto
//! `implemented` would publish a lifecycle a harness could act on and be wrong about. They
//! therefore publish none, and [`incident_transitions`] and [`release_transitions`] carry the
//! ladders in their own vocabulary until the contract has a descriptor that can hold them.

use aep_contract::registry::{
    CommandDescriptor, LifecycleDescriptor, RelationDescriptor, TypeDescriptor,
};
use aep_domain::artifact::{ArtifactStatus, RelationKind};
use aep_domain::entity::EntityBody;

use crate::body::{Incident, IncidentStatus, Release, ReleaseStatus, Runbook};
use crate::command::CommandKind;

/// Describes one AOP command, which always pins the revision it names.
fn operations_command(kind: CommandKind, summary: &str) -> CommandDescriptor {
    CommandDescriptor {
        command_type: kind.as_str().to_owned(),
        summary: summary.to_owned(),
        // Every operations command carries a `VersionedEntityRef`, so every one of them is an
        // optimistic-concurrency assertion and none can silently overwrite newer state.
        revision_guarded: true,
    }
}

/// Describes one of AEP's generic commands, which address an entity without pinning a revision.
fn generic_command(command_type: &str, summary: &str) -> CommandDescriptor {
    CommandDescriptor {
        command_type: command_type.to_owned(),
        summary: summary.to_owned(),
        revision_guarded: false,
    }
}

/// The incident ladder, as `from -> [to]`.
///
/// Published separately from the type descriptor because the contract's lifecycle descriptor cannot
/// hold operational statuses; see the module documentation.
pub fn incident_transitions() -> Vec<(IncidentStatus, Vec<IncidentStatus>)> {
    IncidentStatus::ALL
        .iter()
        .map(|status| (*status, status.successors()))
        .collect()
}

/// The release ladder, as `from -> [to]`, including the sideways step to `rolled_back`.
pub fn release_transitions() -> Vec<(ReleaseStatus, Vec<ReleaseStatus>)> {
    ReleaseStatus::ALL
        .iter()
        .map(|status| (*status, status.successors()))
        .collect()
}

/// The lifecycle of a runbook.
///
/// A runbook is a document, so the artifact vocabulary fits it exactly. There is no `rejected`: a
/// runbook that fails review is rewritten rather than abandoned, and one that is genuinely not
/// wanted is archived. A revision of an active runbook is a new revision of the same entity through
/// `aep.entity.update/v1`, not a trip back down the lifecycle — the status says whether the runbook
/// is the one to open at 3am, not how recently it was edited.
fn runbook_lifecycle() -> LifecycleDescriptor {
    LifecycleDescriptor {
        initial: ArtifactStatus::Draft,
        statuses: vec![
            ArtifactStatus::Draft,
            ArtifactStatus::InReview,
            ArtifactStatus::Approved,
            ArtifactStatus::Active,
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
                vec![ArtifactStatus::Draft, ArtifactStatus::Approved],
            ),
            (
                ArtifactStatus::Approved,
                vec![
                    ArtifactStatus::Active,
                    ArtifactStatus::Superseded,
                    ArtifactStatus::Archived,
                ],
            ),
            (
                ArtifactStatus::Active,
                vec![ArtifactStatus::Superseded, ArtifactStatus::Archived],
            ),
            (ArtifactStatus::Superseded, vec![ArtifactStatus::Archived]),
            (ArtifactStatus::Archived, Vec::new()),
        ],
    }
}

/// What a harness needs in order to work with the three operations types.
///
/// The generic commands are listed alongside the operational ones because they are genuinely
/// available on all three: `aep.entity.update/v1` for ordinary mutable fields, `aep.entity.archive/v1`
/// because §43 makes archiving the vocabulary for retirement and there is no delete. Nothing else is
/// advertised — a command in this list that no crate implements is a promise a harness will try to
/// keep.
///
/// No relations are declared for [`Incident`] or [`Release`]. None of the shipped documents names
/// one, and an edge advertised here is an edge a backend has to accept and a query has to answer.
pub fn type_descriptors() -> Vec<TypeDescriptor> {
    vec![
        TypeDescriptor {
            entity_type: Incident::entity_type(),
            summary: "A live service impairment and the response to it.".to_owned(),
            schema: None,
            // See the module documentation: the incident ladder is operational, and the contract's
            // lifecycle descriptor speaks artifact statuses.
            lifecycle: None,
            commands: vec![
                operations_command(
                    CommandKind::AcknowledgeIncident,
                    "Take responsibility for a live incident.",
                ),
                operations_command(
                    CommandKind::MitigateIncident,
                    "Record an action taken against production to stop the bleeding.",
                ),
                operations_command(
                    CommandKind::ResolveIncident,
                    "Close an incident on the strength of a recorded verification.",
                ),
                generic_command(
                    "aep.entity.update/v1",
                    "Change the incident's ordinary mutable fields, such as its blast radius.",
                ),
                generic_command(
                    "aep.entity.archive/v1",
                    "Retire an incident record without erasing it.",
                ),
            ],
            relations: Vec::new(),
            mutable: true,
        },
        TypeDescriptor {
            entity_type: Runbook::entity_type(),
            summary: "Operational instructions for one service, written before they are needed."
                .to_owned(),
            schema: None,
            lifecycle: Some(runbook_lifecycle()),
            commands: vec![
                generic_command(
                    "aep.entity.update/v1",
                    "Change the runbook's steps, verification or escalation path.",
                ),
                generic_command(
                    "aep.entity.supersede/v1",
                    "Replace a runbook with the one that took its place.",
                ),
                generic_command(
                    "aep.entity.archive/v1",
                    "Retire a runbook for a service that no longer exists.",
                ),
            ],
            relations: vec![RelationDescriptor {
                kind: RelationKind::Supersedes,
                target_types: vec![Runbook::entity_type()],
                // A first runbook supersedes nothing, so the relation cannot be required.
                required: false,
            }],
            mutable: true,
        },
        TypeDescriptor {
            entity_type: Release::entity_type(),
            summary: "One revision on its way to an environment.".to_owned(),
            schema: None,
            // See the module documentation: `canary` and `observed` have no artifact-status
            // counterpart, and inventing one would misdescribe the release rather than describe it.
            lifecycle: None,
            commands: vec![
                operations_command(
                    CommandKind::PromoteRelease,
                    "Move a release into an environment, naming its approval where one is required.",
                ),
                operations_command(
                    CommandKind::RollbackRelease,
                    "Return an environment to an earlier revision.",
                ),
                generic_command(
                    "aep.entity.update/v1",
                    "Change the release's ordinary mutable fields, such as its rollout strategy.",
                ),
                generic_command(
                    "aep.entity.archive/v1",
                    "Retire a release record without erasing it.",
                ),
            ],
            relations: Vec::new(),
            mutable: true,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_registry_describes_all_three_operations_types() {
        let described: Vec<String> = type_descriptors()
            .iter()
            .map(|descriptor| descriptor.entity_type.to_string())
            .collect();
        assert_eq!(
            described,
            vec![
                "aop.incident/v1".to_owned(),
                "aop.runbook/v1".to_owned(),
                "aop.release/v1".to_owned(),
            ]
        );
    }

    #[test]
    fn every_advertised_command_exists_in_one_of_the_two_vocabularies() {
        for descriptor in type_descriptors() {
            for command in &descriptor.commands {
                let name = &command.command_type;
                let known = CommandKind::parse(name).is_ok()
                    || aep_domain::command::CommandKind::parse(name).is_ok();
                assert!(
                    known,
                    "{} advertises {name}, which no crate implements",
                    descriptor.entity_type
                );
                assert!(
                    !command.summary.is_empty(),
                    "{name} is advertised without saying what it does"
                );
            }
        }
    }

    #[test]
    fn only_the_operations_commands_are_advertised_as_revision_guarded() {
        for descriptor in type_descriptors() {
            for command in &descriptor.commands {
                let is_operational = CommandKind::parse(&command.command_type).is_ok();
                assert_eq!(
                    command.revision_guarded, is_operational,
                    "{} claims the wrong concurrency guarantee for {}",
                    descriptor.entity_type, command.command_type
                );
            }
        }
    }

    #[test]
    fn an_incident_advertises_the_three_commands_that_move_it() {
        let descriptors = type_descriptors();
        let incident = descriptors
            .iter()
            .find(|descriptor| descriptor.entity_type == Incident::entity_type())
            .expect("the registry describes incidents");

        assert!(incident.accepts("aop.incident.acknowledge/v1"));
        assert!(incident.accepts("aop.incident.mitigate/v1"));
        assert!(incident.accepts("aop.incident.resolve/v1"));
        assert!(
            !incident.accepts("aop.release.promote/v1"),
            "a release command must not be offered on an incident"
        );
        assert!(
            !incident.accepts("aep.entity.delete/v1"),
            "there is no delete command anywhere in the protocol"
        );
    }

    #[test]
    fn a_release_advertises_both_of_its_commands_and_neither_incident_command() {
        let descriptors = type_descriptors();
        let release = descriptors
            .iter()
            .find(|descriptor| descriptor.entity_type == Release::entity_type())
            .expect("the registry describes releases");

        assert!(release.accepts("aop.release.promote/v1"));
        assert!(release.accepts("aop.release.rollback/v1"));
        assert!(!release.accepts("aop.incident.mitigate/v1"));
    }

    #[test]
    fn the_operational_types_publish_no_artifact_lifecycle() {
        for descriptor in type_descriptors() {
            let is_runbook = descriptor.entity_type == Runbook::entity_type();
            assert_eq!(
                descriptor.lifecycle.is_some(),
                is_runbook,
                "{} publishes the wrong kind of lifecycle: an incident and a release move through \
                 operational statuses, which the artifact vocabulary cannot spell",
                descriptor.entity_type
            );
        }
    }

    #[test]
    fn a_runbook_lifecycle_starts_in_draft_and_ends_archived() {
        let lifecycle = runbook_lifecycle();
        assert_eq!(lifecycle.initial, ArtifactStatus::Draft);

        let terminal = lifecycle
            .transitions
            .iter()
            .filter(|(_, to)| to.is_empty())
            .map(|(from, _)| *from)
            .collect::<Vec<_>>();
        assert_eq!(
            terminal,
            vec![ArtifactStatus::Archived],
            "archived is the only status a runbook cannot leave"
        );

        for (from, to) in &lifecycle.transitions {
            assert!(
                lifecycle.statuses.contains(from),
                "{from} moves but is not a declared status"
            );
            for target in to {
                assert!(
                    lifecycle.statuses.contains(target),
                    "{from} can reach {target}, which is not a declared status"
                );
            }
        }
    }

    #[test]
    fn a_runbook_may_supersede_another_runbook_and_nothing_else() {
        let descriptors = type_descriptors();
        let runbook = descriptors
            .iter()
            .find(|descriptor| descriptor.entity_type == Runbook::entity_type())
            .expect("the registry describes runbooks");

        assert!(runbook.permits_relation(RelationKind::Supersedes));
        assert!(!runbook.permits_relation(RelationKind::Verifies));
        let supersedes = runbook
            .relations
            .iter()
            .find(|relation| relation.kind == RelationKind::Supersedes)
            .expect("the supersedes relation is declared");
        assert_eq!(supersedes.target_types, vec![Runbook::entity_type()]);
        assert!(
            !supersedes.required,
            "a first runbook supersedes nothing, so requiring the edge would make it unusable"
        );
    }

    #[test]
    fn the_published_ladders_match_the_status_types_they_come_from() {
        let incident = incident_transitions();
        assert_eq!(incident.len(), IncidentStatus::ALL.len());
        assert_eq!(
            incident.first().map(|(from, _)| *from),
            Some(IncidentStatus::INITIAL)
        );
        assert_eq!(
            incident.last().map(|(_, to)| to.clone()),
            Some(Vec::new()),
            "resolved is terminal"
        );

        let release = release_transitions();
        assert_eq!(release.len(), ReleaseStatus::ALL.len());
        let rolled_back = release
            .iter()
            .find(|(from, _)| *from == ReleaseStatus::RolledBack)
            .expect("rolled_back is published");
        assert!(rolled_back.1.is_empty(), "rolled_back is terminal");
        let canary = release
            .iter()
            .find(|(from, _)| *from == ReleaseStatus::Canary)
            .expect("canary is published");
        assert_eq!(
            canary.1,
            vec![ReleaseStatus::Observed, ReleaseStatus::RolledBack],
            "a canary either gets observed or gets rolled back"
        );
    }

    #[test]
    fn every_operations_type_is_mutable_and_says_so() {
        for descriptor in type_descriptors() {
            assert!(
                descriptor.mutable,
                "{} is edited throughout its life: an incident acquires hypotheses, a release \
                 advances its status, a runbook is rewritten after it fails somebody at 3am",
                descriptor.entity_type
            );
        }
    }
}
