//! Retrying a command that may already have been applied.
//!
//! A client that issues a mutation and then loses the connection does not know whether it happened.
//! It has exactly one safe move: send it again. The contract makes that safe by separating the three
//! identifiers a retry needs — a fresh `request_id`, the same `command_id`, the same
//! `idempotency_key` — and requiring the backend to recognise the second attempt and hand back what
//! the first one produced.
//!
//! Get this wrong and the failure is silent and expensive: the retry applies a second time, the
//! entity is at a revision nobody asked for, and the story, the approval or the payment exists
//! twice. That is [`crate::faulty::Fault::ReplayApplies`], and it is what this suite catches.
//!
//! The other edge is the one implementers forget. A key that has been used by a *different* command
//! must be refused, not honoured: if reusing a key silently applies whatever arrives with it, the
//! key guarantees nothing and a client bug becomes a data-loss bug.

use aep_contract::command::CommandOutcome;
use aep_domain::command::{Command, UpdateEntity};
use aep_domain::entity::{EntityRevision, VersionedEntityRef};
use aep_domain::node::Node;

use crate::harness::{Backend, Harness};
use crate::report::SuiteReport;

/// Runs the idempotency suite.
// A command, its retry, and a key reused by something else — in that order, against the same entity.
// Each step is only meaningful because of the state the previous one left.
#[allow(clippy::too_many_lines)]
pub fn run<B: Backend>(backend: &B) -> SuiteReport {
    let harness = Harness::new("idempotency");
    let mut report = SuiteReport::new("idempotency");

    let design = match harness.create_design(backend) {
        Ok(design) => design,
        Err(error) => {
            report.aborted(
                "an entity can be created to retry against",
                error.to_string(),
            );
            return report;
        }
    };

    let command = Command::UpdateEntity(UpdateEntity {
        target: design.unversioned(),
        changes: [("title".to_owned(), Node::from("Applied once"))].into(),
    });
    let command_id = harness.command_id();
    let context = harness.context();
    let key = context.idempotency_key.clone();

    let first = match harness.execute(
        backend,
        harness.envelope(command_id.clone(), command.clone(), context),
    ) {
        Ok(result) => result,
        Err(error) => {
            report.aborted("a command can be applied a first time", error.to_string());
            return report;
        }
    };

    let Some(applied_at) = revision(&harness, backend, &design) else {
        report.aborted(
            "the entity is readable after the command it was changed by",
            "the entity could not be read after the first attempt".to_owned(),
        );
        return report;
    };

    // The retry: a new transport attempt for the same logical command, carrying the same key. This
    // is exactly what a client does after a timeout, and it is the only thing it can do.
    let replay = match harness.execute(
        backend,
        harness.envelope(
            command_id.clone(),
            command.clone(),
            harness.context_with_key(&key),
        ),
    ) {
        Ok(result) => result,
        Err(error) => {
            report.aborted(
                "a retry of an already applied command is answered rather than refused",
                error.to_string(),
            );
            return report;
        }
    };

    report.expect(
        "a retry of the same logical command is recognised as a replay",
        replay.outcome == CommandOutcome::Replayed,
        format!(
            "the retry reported `{:?}` rather than `Replayed`, so the caller cannot tell a second \
             application from the first",
            replay.outcome
        ),
    );
    report.expect(
        "a replay returns what the original command returned",
        replay.affected == first.affected,
        format!(
            "the command reported {:?} and its replay reported {:?}",
            first.affected, replay.affected
        ),
    );
    report.expect(
        "a replay answers for the logical command it replays",
        replay.command_id == first.command_id,
        format!(
            "the command was `{}` and its replay answered for `{}`",
            first.command_id, replay.command_id
        ),
    );

    match revision(&harness, backend, &design) {
        Some(after) => report.expect(
            "a replay does not advance the revision",
            after == applied_at,
            format!(
                "the command left the entity at revision {applied_at}, and after replaying it the \
                 entity is at revision {after}"
            ),
        ),
        None => report.aborted(
            "a replay does not advance the revision",
            "the entity could not be read after the replay".to_owned(),
        ),
    }

    // A different command under the same key. Honouring this would make the key meaningless: the
    // backend would be promising "at most once" while applying whatever turned up second.
    let different = Command::UpdateEntity(UpdateEntity {
        target: design.unversioned(),
        changes: [(
            "title".to_owned(),
            Node::from("A different intention entirely"),
        )]
        .into(),
    });
    match harness.execute(
        backend,
        harness.envelope(
            harness.command_id(),
            different,
            harness.context_with_key(&key),
        ),
    ) {
        Ok(result) => report.expect(
            "an idempotency key reused by a different command is refused",
            false,
            format!(
                "the backend accepted a second, different command under key `{key}` and reported \
                 `{:?}`",
                result.outcome
            ),
        ),
        Err(error) => report.expect(
            "an idempotency key reused by a different command is refused",
            error.code() == "idempotency_mismatch",
            format!(
                "the backend refused with `{}` rather than `idempotency_mismatch`: {error}",
                error.code()
            ),
        ),
    }

    match revision(&harness, backend, &design) {
        Some(after) => report.expect(
            "a refused key reuse leaves the entity where it was",
            after == applied_at,
            format!(
                "the entity was at revision {applied_at} before the reused key was refused, and is \
                 at revision {after} after it"
            ),
        ),
        None => report.aborted(
            "a refused key reuse leaves the entity where it was",
            "the entity could not be read after the refusal".to_owned(),
        ),
    }

    report
}

/// The revision an entity is at now, or `None` when it cannot be read.
fn revision<B: Backend>(
    harness: &Harness,
    backend: &B,
    entity: &VersionedEntityRef,
) -> Option<EntityRevision> {
    harness
        .read(backend, &entity.unversioned())
        .ok()
        .map(|entity| entity.metadata.revision)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::faulty::{Fault, FaultyBackend};
    use aep_backend_memory::MemoryBackend;

    #[test]
    fn the_reference_backend_applies_a_replayed_command_once() {
        let report = run(&MemoryBackend::new());
        assert!(report.passed(), "{report}");
    }

    #[test]
    fn a_backend_that_applies_a_replay_twice_does_not_pass() {
        let report = run(&FaultyBackend::new(
            MemoryBackend::new(),
            Fault::ReplayApplies,
        ));
        assert!(
            !report.passed(),
            "a retry after a timeout is the one move a client has, and a suite that lets a backend \
             apply it twice is worse than no suite: {report}"
        );
    }
}
