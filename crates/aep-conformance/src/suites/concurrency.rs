//! Two writers, one entity, and the refusal that keeps them from overwriting each other.
//!
//! Optimistic concurrency is the whole of the protection here. A caller reads revision 4, decides
//! what to do about what it read, and asserts revision 4 when it writes. If the entity has moved on,
//! its decision was made about text that no longer exists and the write must be refused — with a
//! machine-readable error carrying both revisions, so the caller can refetch, decide again, and
//! reissue.
//!
//! A backend that accepts the assertion and ignores it is the worst kind of wrong: every write
//! succeeds, nothing errors, and one agent's work disappears under another's. Nobody notices until
//! someone asks why the approved design says something no reviewer read. That is
//! [`crate::faulty::Fault::IgnoreExpectedRevision`], and this suite exists to catch it.
//!
//! The last check says the opposite thing on purpose: an unguarded write is legal. A backend must
//! not invent a guard nobody asked for, and a command that carries no assertion must say so rather
//! than implying one.

use aep_contract::error::CommandError;
use aep_domain::command::{Command, UpdateEntity};
use aep_domain::entity::EntityRevision;
use aep_domain::node::Node;

use crate::harness::{Backend, Harness};
use crate::report::{Check, SuiteReport};

/// What the writer that wins puts in the title.
const FIRST_WRITER: &str = "Written by the writer that asserted correctly";
/// What the writer that lost the race tries to put there.
const SECOND_WRITER: &str = "Rewritten behind the first writer's back";
/// The property the first, correctly asserted write establishes.
const ASSERTION_HOLDS: &str = "an update asserting the revision the entity is at is accepted";

/// Runs the concurrency suite.
// The three writes belong together: each one asks about the state the previous left behind, and a
// stale assertion is only stale because an earlier write succeeded.
#[allow(clippy::too_many_lines)]
pub fn run<B: Backend>(backend: &B) -> SuiteReport {
    let harness = Harness::new("concurrency");
    let mut report = SuiteReport::new("concurrency");

    let design = match harness.create_design(backend) {
        Ok(design) => design,
        Err(error) => {
            report.aborted(
                "an entity can be created to write against",
                error.to_string(),
            );
            return report;
        }
    };
    let target = design.unversioned();

    let guarded = harness
        .envelope(
            harness.command_id(),
            Command::UpdateEntity(UpdateEntity {
                target: target.clone(),
                changes: [("title".to_owned(), Node::from(FIRST_WRITER))].into(),
            }),
            harness.context(),
        )
        .expecting(design.revision);
    let accepted = match harness.execute(backend, guarded) {
        Ok(result) => {
            report.record(Check::passed(ASSERTION_HOLDS));
            result
        }
        Err(error) => {
            report.record(Check::failed(
                ASSERTION_HOLDS,
                format!(
                    "the entity was at revision {} and an update asserting it was refused with \
                     `{}`: {error}",
                    design.revision,
                    error.code()
                ),
            ));
            return report;
        }
    };
    let current = match accepted.revision_of(&target) {
        Some(revision) => revision,
        None => design.revision.next(),
    };

    // The second writer read the same revision the first one did, and has not noticed that the first
    // one already landed.
    let stale = harness
        .envelope(
            harness.command_id(),
            Command::UpdateEntity(UpdateEntity {
                target: target.clone(),
                changes: [("title".to_owned(), Node::from(SECOND_WRITER))].into(),
            }),
            harness.context(),
        )
        .expecting(design.revision);
    match harness.execute(backend, stale) {
        Ok(result) => {
            report.expect(
                "an update asserting a revision the entity has moved past is refused",
                false,
                format!(
                    "the entity had moved from revision {} to {current}, and the backend accepted a \
                     write asserting {} anyway, reporting `{:?}`",
                    design.revision, design.revision, result.outcome
                ),
            );
            report.expect(
                "a refusal reports both the revision asserted and the revision held",
                false,
                "there was no refusal to report, because the stale write was accepted".to_owned(),
            );
        }
        Err(error) => {
            report.expect(
                "an update asserting a revision the entity has moved past is refused",
                error.code() == "revision_conflict",
                format!(
                    "the backend refused with `{}` rather than `revision_conflict`: {error}",
                    error.code()
                ),
            );
            match &error {
                CommandError::RevisionConflict {
                    expected, actual, ..
                } => report.expect(
                    "a refusal reports both the revision asserted and the revision held",
                    *expected == design.revision && *actual == current,
                    format!(
                        "the write asserted revision {} against an entity at {current}, and the \
                         refusal reported expected {expected} and actual {actual}",
                        design.revision
                    ),
                ),
                other => report.expect(
                    "a refusal reports both the revision asserted and the revision held",
                    false,
                    format!(
                        "the refusal was `{}`, which carries no revisions for the caller to \
                         refetch from: {other}",
                        other.code()
                    ),
                ),
            }
        }
    }

    match harness.read(backend, &target) {
        Ok(entity) => {
            let title = entity
                .data
                .as_map()
                .and_then(|fields| fields.get("title"))
                .and_then(Node::as_text)
                .unwrap_or_default();
            report.expect(
                "a refused update leaves the entity exactly as it was",
                title == FIRST_WRITER && entity.metadata.revision == current,
                format!(
                    "after the refusal the entity is at revision {} with title {title:?}; it was at \
                     revision {current} with title {FIRST_WRITER:?}",
                    entity.metadata.revision
                ),
            );
        }
        Err(error) => report.aborted(
            "a refused update leaves the entity exactly as it was",
            error,
        ),
    }

    // Unguarded is legal. A backend that refuses this has invented a rule; one that reports the
    // command as guarded has hidden from the caller that nothing was being asserted.
    let unguarded = harness.envelope(
        harness.command_id(),
        Command::UpdateEntity(UpdateEntity {
            target: target.clone(),
            changes: [(
                "title".to_owned(),
                Node::from("Written without an assertion"),
            )]
            .into(),
        }),
        harness.context(),
    );
    let guarded_flag = unguarded.is_revision_guarded();
    match harness.execute(backend, unguarded) {
        Ok(result) => report.expect(
            "an update that asserts no revision is accepted, and says it asserted none",
            !guarded_flag
                && result
                    .revision_of(&target)
                    .is_none_or(|revision| revision > current),
            format!(
                "the unguarded write reported guarded={guarded_flag} and left the entity at {:?}",
                result.revision_of(&target).map(EntityRevision::get)
            ),
        ),
        Err(error) => report.expect(
            "an update that asserts no revision is accepted, and says it asserted none",
            false,
            format!(
                "a write that asserts nothing has nothing to conflict with, and this one was \
                 refused with `{}`: {error}",
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
    fn the_reference_backend_refuses_a_stale_write() {
        let report = run(&MemoryBackend::new());
        assert!(report.passed(), "{report}");
    }

    #[test]
    fn a_backend_that_ignores_a_revision_assertion_does_not_pass() {
        let report = run(&FaultyBackend::new(
            MemoryBackend::new(),
            Fault::IgnoreExpectedRevision,
        ));
        assert!(
            !report.passed(),
            "a merged stale write loses one agent's work with no error anywhere, which is exactly \
             the failure this suite exists to make loud: {report}"
        );
    }
}
