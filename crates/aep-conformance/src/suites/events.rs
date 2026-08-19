//! A command reports the facts it produced.
//!
//! §49 draws the line entities do not cross: an entity is the current state of an addressable thing,
//! an event is an immutable fact that something occurred. §78.15 asks that a command report the
//! events it emitted, and the reason is that everything downstream — triggered work (§51), a
//! projection, a notification, a second agent waiting for a design to be approved — is driven by
//! those facts and by nothing else. A backend that applies the change and reports no event looks
//! perfectly correct to whoever issued the command and silently stops the rest of the system:
//! nothing is wrong with the data, the work simply never happens. The converse matters as much — a
//! command that changed nothing must report nothing, or every retry becomes a fan-out.

use aep_domain::command::{Command, CreateEntity};
use aep_domain::entity::EntityType;
use aep_domain::ids::EventId;
use aep_domain::node::Node;

use crate::harness::{Backend, Harness};
use crate::report::{Check, SuiteReport};

/// Runs the events suite.
// The four checks are one ordered scenario — a create, a second create, a create that changes
// nothing, and a replay of the first — each reading the state the previous one left. Splitting it
// into helpers would hide that sequence, which is the thing a reader needs to follow.
#[allow(clippy::too_many_lines)]
pub fn run<B: Backend>(backend: &B) -> SuiteReport {
    let harness = Harness::new("events");
    let mut report = SuiteReport::new("events");

    let emitted = "a create reports at least one emitted event";
    let distinct = "no event is reported by two different commands";
    let silent = "a command that changes nothing reports no events";
    let replayed = "a replay reports the events of the original rather than new ones";

    let Ok(entity_type) = EntityType::parse("aep.design/v1") else {
        report.aborted(
            emitted,
            "the suite's entity type `aep.design/v1` is not well formed",
        );
        return report;
    };
    let taken = harness.locator("design");
    let make = |locator| {
        Command::CreateEntity(CreateEntity {
            entity_type: entity_type.clone(),
            locator,
            data: Node::Map(
                [
                    (
                        "title".to_owned(),
                        Node::from("A design worth telling people about"),
                    ),
                    ("status".to_owned(), Node::from("in_review")),
                ]
                .into(),
            ),
        })
    };

    // Keep this command's own identifiers: the replay at the end has to be the same logical command
    // rather than a second one that merely looks like it.
    let command_id = harness.command_id();
    let context = harness.context();
    let key = context.idempotency_key.clone();

    let first = match harness.execute(
        backend,
        harness.envelope(command_id.clone(), make(taken.clone()), context),
    ) {
        Ok(result) => result,
        Err(error) => {
            report.aborted(emitted, format!("the entity could not be created: {error}"));
            return report;
        }
    };

    report.expect(
        emitted,
        !first.events.is_empty(),
        "creating an entity emitted no event, so nothing downstream of a creation can be triggered \
         by one",
    );

    match harness.run(backend, make(harness.locator("design"))) {
        Ok(second) => {
            let shared: Vec<&EventId> = first
                .events
                .iter()
                .filter(|event| second.events.contains(event))
                .collect();
            report.expect(
                distinct,
                shared.is_empty(),
                format!(
                    "two separate creates both report {shared:?}; the first reported {:?} and the \
                     second {:?}",
                    first.events, second.events
                ),
            );
        }
        Err(error) => report.aborted(
            distinct,
            format!("a second entity could not be created: {error}"),
        ),
    }

    // The same address twice: nothing is created, so there is no fact to report.
    match harness.run(backend, make(taken.clone())) {
        Ok(result) => report.expect(
            silent,
            result.events.is_empty(),
            format!(
                "a create at an address already taken was accepted and reported {:?}",
                result.events
            ),
        ),
        Err(error) => report.record(Check {
            name: silent.to_owned(),
            passed: true,
            detail: Some(format!(
                "the second create at the same address was refused ({error}), so it reported \
                 nothing at all"
            )),
        }),
    }

    // The same logical command again: same command id, same idempotency key, same payload.
    match harness.execute(
        backend,
        harness.envelope(command_id, make(taken), harness.context_with_key(&key)),
    ) {
        Ok(replay) => report.expect(
            replayed,
            replay.events == first.events,
            format!(
                "the original reported {:?} and the replay reported {:?}, so whatever is listening \
                 acts twice on one command",
                first.events, replay.events
            ),
        ),
        // A backend that does not recognise the replay fails the idempotency suite; saying so twice
        // would only send an implementer looking in two places for one fault.
        Err(error) => report.record(Check {
            name: replayed.to_owned(),
            passed: true,
            detail: Some(format!(
                "the replay was not recognised as one ({error}); whether a replay is recognised is \
                 the idempotency suite's property"
            )),
        }),
    }

    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use aep_backend_memory::MemoryBackend;

    #[test]
    fn the_reference_backend_reports_what_it_emitted() {
        let report = run(&MemoryBackend::new());
        assert!(report.passed(), "{report}");
    }

    #[test]
    fn a_backend_that_reports_no_events_fails() {
        let backend = crate::faulty::FaultyBackend::new(
            MemoryBackend::new(),
            crate::faulty::Fault::DropEvents,
        );
        let report = run(&backend);
        assert!(!report.passed(), "{report}");
    }
}
