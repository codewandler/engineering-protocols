//! Every revision an entity passed through, in the order it passed through them.
//!
//! Current state answers "what does this design say?". History answers "what did it say when it was
//! approved, and who changed it afterwards?" — which is the question an engineering record exists
//! for. Without it a revision number is a counter rather than a position: nothing can be
//! reconstructed, an approval pinned to revision 3 cannot be checked against what revision 3
//! actually contained, and an audit trail that names revisions points at states nobody kept.
//!
//! [`crate::faulty::Fault::LoseHistory`] keeps only the most recent record. That is the shape this
//! goes wrong in practice — a backend that stores the entity and calls the latest row its history —
//! and it survives every check that only looks at a freshly created entity, because a freshly
//! created entity has exactly one revision. So this suite asks after a change as well as before one.
//!
//! The last check is about a different failure: history for an entity that does not exist must fail.
//! An empty answer says "this thing was never changed", which is a statement about a record that is
//! not there.

use aep_contract::error::QueryError;
use aep_contract::query::RevisionRecord;
use aep_contract::testing::block_on;
use aep_domain::command::{Command, UpdateEntity};
use aep_domain::entity::{EntityId, EntityRef, EntityRevision};
use aep_domain::node::Node;

use crate::harness::{Backend, Harness};
use crate::report::SuiteReport;

/// An identity that is well formed and was never created.
const ABSENT_ENTITY: &str = "aep-conformance-absent-history";

/// Runs the history suite.
// The whole point is that the answer changes as the entity does, so the checks before and after the
// update belong to one sequence.
#[allow(clippy::too_many_lines)]
pub fn run<B: Backend>(backend: &B) -> SuiteReport {
    let harness = Harness::new("history");
    let mut report = SuiteReport::new("history");

    let design = match harness.create_design(backend) {
        Ok(design) => design,
        Err(error) => {
            report.aborted(
                "an entity can be created to have a history",
                error.to_string(),
            );
            return report;
        }
    };
    let target = design.unversioned();

    match history(backend, &target) {
        Ok(records) => report.expect(
            "history after a creation holds exactly one record, at revision 1",
            records.len() == 1
                && records
                    .first()
                    .is_some_and(|record| record.revision == EntityRevision::INITIAL),
            format!(
                "the entity has only ever been created, and its history holds {} records: {}",
                records.len(),
                revisions(&records)
            ),
        ),
        Err(error) => report.aborted(
            "history after a creation holds exactly one record, at revision 1",
            error.to_string(),
        ),
    }

    if let Err(error) = harness.run(
        backend,
        Command::UpdateEntity(UpdateEntity {
            target: target.clone(),
            changes: [("title".to_owned(), Node::from("Changed once"))].into(),
        }),
    ) {
        report.aborted(
            "history after an update holds one record per revision",
            error.to_string(),
        );
        return report;
    }

    let records = match history(backend, &target) {
        Ok(records) => records,
        Err(error) => {
            report.aborted(
                "history after an update holds one record per revision",
                error.to_string(),
            );
            return report;
        }
    };

    report.expect(
        "history after an update holds one record per revision",
        records.len() == 2,
        format!(
            "the entity was created and then changed once, and its history holds {} records: {}",
            records.len(),
            revisions(&records)
        ),
    );
    report.expect(
        "history is ordered by ascending revision",
        records
            .windows(2)
            .all(|pair| pair[0].revision < pair[1].revision),
        format!(
            "the records arrived in the order {}, so a reader cannot tell which state came first",
            revisions(&records)
        ),
    );

    let anonymous: Vec<String> = records
        .iter()
        .filter(|record| record.actor != *harness.actor())
        .map(|record| {
            format!(
                "revision {} is attributed to {}",
                record.revision, record.actor
            )
        })
        .collect();
    report.expect(
        "every history record names the actor the change was attributed to",
        anonymous.is_empty(),
        format!(
            "every change here was made by `{}`, and the history says otherwise: {}",
            harness.actor(),
            anonymous.join(", ")
        ),
    );

    match harness.read(backend, &target) {
        Ok(entity) => report.expect(
            "the newest history record is the revision the entity is at",
            records
                .last()
                .is_some_and(|record| record.revision == entity.metadata.revision),
            format!(
                "the entity is at revision {} and its history ends at {}",
                entity.metadata.revision,
                revisions(&records)
            ),
        ),
        Err(error) => report.aborted(
            "the newest history record is the revision the entity is at",
            error,
        ),
    }

    let Ok(absent) = EntityId::new(ABSENT_ENTITY) else {
        report.aborted(
            "history of an entity that does not exist fails rather than answering with nothing",
            format!("the suite's own placeholder identity `{ABSENT_ENTITY}` is not well formed"),
        );
        return report;
    };
    match history(backend, &EntityRef::new(absent)) {
        Ok(records) => report.expect(
            "history of an entity that does not exist fails rather than answering with nothing",
            false,
            format!(
                "the backend answered with {} records for `{ABSENT_ENTITY}`, which was never \
                 created; an empty history says the entity exists and was never changed",
                records.len()
            ),
        ),
        Err(error) => report.expect(
            "history of an entity that does not exist fails rather than answering with nothing",
            error.code() == "not_found",
            format!(
                "the backend refused with `{}` rather than `not_found`: {error}",
                error.code()
            ),
        ),
    }

    report
}

/// Asks for an entity's history.
///
/// The error is kept whole rather than rendered, so the last check can match on the backend's code
/// instead of on the words in its message.
fn history<B: Backend>(
    backend: &B,
    reference: &EntityRef,
) -> Result<Vec<RevisionRecord>, QueryError> {
    block_on(backend.history(reference))
}

/// The revisions a history covers, for a detail line that says what was actually seen.
fn revisions(records: &[RevisionRecord]) -> String {
    if records.is_empty() {
        return "no revisions at all".to_owned();
    }
    records
        .iter()
        .map(|record| record.revision.to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::faulty::{Fault, FaultyBackend};
    use aep_backend_memory::MemoryBackend;

    #[test]
    fn the_reference_backend_keeps_every_revision_it_passed_through() {
        let report = run(&MemoryBackend::new());
        assert!(report.passed(), "{report}");
    }

    #[test]
    fn a_backend_that_keeps_only_the_latest_revision_does_not_pass() {
        let report = run(&FaultyBackend::new(
            MemoryBackend::new(),
            Fault::LoseHistory,
        ));
        assert!(
            !report.passed(),
            "a history of one row looks correct for anything that has never changed, which is why \
             this suite asks after a change: {report}"
        );
    }
}
