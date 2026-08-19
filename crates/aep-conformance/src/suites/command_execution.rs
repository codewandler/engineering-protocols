//! What a command must report, and what must be true afterwards.
//!
//! Every state change in AEP goes through one boundary, so the result of a command is the only
//! thing a caller has to reason with: what changed, which revision it changed to, and a token that
//! lets the next read see it. A backend that applies the change but reports none of that is not
//! merely terse — a caller cannot pin an approval to a revision it was not told about, cannot read
//! its own write back, and cannot tell an accepted command from a silently discarded one. That is
//! the fault [`crate::faulty::Fault::DropAffected`] injects, and this suite exists to catch it.
//!
//! The other half is that the report must be true: the entity is readable afterwards, it holds the
//! body that was sent, and a command against something that does not exist is refused with a code a
//! caller can branch on rather than a message it has to parse.

use aep_contract::command::CommandOutcome;
use aep_domain::command::{Command, CreateEntity, UpdateEntity};
use aep_domain::entity::{EntityId, EntityRef, EntityRevision, EntityType};
use aep_domain::node::Node;

use crate::harness::{Backend, Harness};
use crate::report::SuiteReport;

/// The type the entity under test is created with.
const ENTITY_TYPE: &str = "aep.design/v1";
/// An identity that is well formed and was never created, for asking what happens to a command
/// addressed at nothing.
const ABSENT_ENTITY: &str = "aep-conformance-absent-entity";

/// Runs the command-execution suite.
// One entity, driven from creation through update to a command that misses. Splitting it would hide
// that each check is asking about the state the previous one left behind.
#[allow(clippy::too_many_lines)]
pub fn run<B: Backend>(backend: &B) -> SuiteReport {
    let harness = Harness::new("command-execution");
    let mut report = SuiteReport::new("command-execution");

    let Ok(entity_type) = ENTITY_TYPE.parse::<EntityType>() else {
        report.aborted(
            "a command can be issued at all",
            format!("the suite's own entity type `{ENTITY_TYPE}` is not well formed"),
        );
        return report;
    };
    let locator = harness.locator("design");
    let body = Node::Map(
        [
            ("title".to_owned(), Node::from("A command under test")),
            ("status".to_owned(), Node::from("in_review")),
        ]
        .into(),
    );

    let created = match harness.run(
        backend,
        Command::CreateEntity(CreateEntity {
            entity_type,
            locator: locator.clone(),
            data: body.clone(),
        }),
    ) {
        Ok(result) => result,
        Err(error) => {
            report.aborted("an entity can be created", error.to_string());
            return report;
        }
    };

    report.expect(
        "a creation reports exactly one affected entity",
        created.affected.len() == 1,
        format!(
            "creating `{locator}` reported {} affected entities",
            created.affected.len()
        ),
    );
    report.expect(
        "a creation says it applied the command for the first time",
        created.outcome == CommandOutcome::Accepted,
        format!(
            "creating `{locator}` reported the outcome `{:?}`",
            created.outcome
        ),
    );
    report.expect(
        "a command result carries the consistency token a later read can demand",
        !created.consistency.as_str().is_empty(),
        "the creation returned an empty consistency token, so no read can ask to see this write"
            .to_owned(),
    );

    let Some(reference) = created.affected.first().cloned() else {
        report.aborted(
            "a created entity is readable afterwards",
            format!("the creation of `{locator}` reported no entity to read"),
        );
        return report;
    };

    report.expect(
        "a created entity starts at revision 1",
        reference.revision == EntityRevision::INITIAL,
        format!("`{locator}` was created at revision {}", reference.revision),
    );

    match harness.read(backend, &reference.unversioned()) {
        Ok(entity) => report.expect(
            "a created entity is readable afterwards, and holds the body that was sent",
            entity.data == body,
            format!(
                "`{locator}` was created with {body:?} and reads back as {:?}",
                entity.data
            ),
        ),
        Err(error) => report.aborted(
            "a created entity is readable afterwards, and holds the body that was sent",
            error,
        ),
    }

    let revised = Node::from("A command under test, revised");
    match harness.run(
        backend,
        Command::UpdateEntity(UpdateEntity {
            target: reference.unversioned(),
            changes: [("title".to_owned(), revised.clone())].into(),
        }),
    ) {
        Ok(result) => match result.revision_of(&reference.unversioned()) {
            Some(revision) => report.expect(
                "an update advances the revision by exactly one",
                revision.get() == reference.revision.get() + 1,
                format!(
                    "the entity was at revision {} and the update reported revision {revision}",
                    reference.revision
                ),
            ),
            None => report.expect(
                "an update advances the revision by exactly one",
                false,
                "the update reported no revision for the entity it changed, so a caller cannot \
                     pin anything to what it did"
                    .to_owned(),
            ),
        },
        Err(error) => report.aborted(
            "an update advances the revision by exactly one",
            error.to_string(),
        ),
    }

    match harness.field(backend, &reference.unversioned(), "title") {
        Some(title) => report.expect(
            "an accepted update is visible in the entity, not only in the result",
            title == revised,
            format!("the update set the title to {revised:?} and the entity reads back {title:?}"),
        ),
        None => report.expect(
            "an accepted update is visible in the entity, not only in the result",
            false,
            "the entity has no title after an update that set one".to_owned(),
        ),
    }

    let Ok(absent) = EntityId::new(ABSENT_ENTITY) else {
        report.aborted(
            "a command against an entity that does not exist is refused as `not_found`",
            format!("the suite's own placeholder identity `{ABSENT_ENTITY}` is not well formed"),
        );
        return report;
    };
    match harness.run(
        backend,
        Command::UpdateEntity(UpdateEntity {
            target: EntityRef::new(absent),
            changes: [("title".to_owned(), Node::from("Written to nothing"))].into(),
        }),
    ) {
        Ok(result) => report.expect(
            "a command against an entity that does not exist is refused as `not_found`",
            false,
            format!(
                "the backend accepted a change to `{ABSENT_ENTITY}`, which was never created, and \
                 reported `{:?}`",
                result.outcome
            ),
        ),
        Err(error) => report.expect(
            "a command against an entity that does not exist is refused as `not_found`",
            error.code() == "not_found",
            format!(
                "the backend refused with `{}` rather than `not_found`: {error}",
                error.code()
            ),
        ),
    }

    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::faulty::{Fault, FaultyBackend};
    use aep_backend_memory::MemoryBackend;

    #[test]
    fn the_reference_backend_reports_what_its_commands_changed() {
        let report = run(&MemoryBackend::new());
        assert!(report.passed(), "{report}");
    }

    #[test]
    fn a_backend_that_hides_what_it_changed_does_not_pass() {
        let report = run(&FaultyBackend::new(
            MemoryBackend::new(),
            Fault::DropAffected,
        ));
        assert!(
            !report.passed(),
            "a command that does not say what it changed leaves a caller unable to read its own \
             write, and this suite must say so: {report}"
        );
    }
}
