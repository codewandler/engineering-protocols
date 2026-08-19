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

use aep_contract::command::CommandEnvelope;
use aep_domain::command::{Command, CreateEntity};
use aep_domain::entity::{EntityLocator, EntityType};
use aep_domain::ids::{CommandId, EventId, IdempotencyKey};
use aep_domain::node::Node;

use crate::harness::{Backend, Harness, ORGANISATION, SPACE};
use crate::report::{Check, SuiteReport};

/// Runs the events suite.
// A suite is one ordered list of checks, each depending on the state the last one left. Splitting it
// into helpers would hide that sequence, which is the thing a reader needs to follow.
#[allow(clippy::too_many_lines)]
pub fn run<B: Backend>(backend: &B) -> SuiteReport {
    let harness = Harness::new(SUITE);
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
    let taken = match address("design", "first") {
        Ok(locator) => locator,
        Err(detail) => {
            report.aborted(emitted, detail);
            return report;
        }
    };
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

    let first = match envelope(&harness, "first", make(taken.clone())).and_then(|envelope| {
        harness
            .execute(backend, envelope)
            .map_err(|e| e.to_string())
    }) {
        Ok(result) => result,
        Err(detail) => {
            report.aborted(
                emitted,
                format!("the entity could not be created: {detail}"),
            );
            return report;
        }
    };

    report.expect(
        emitted,
        !first.events.is_empty(),
        "creating an entity emitted no event, so nothing downstream of a creation can be triggered \
         by one",
    );

    match address("design", "second")
        .and_then(|locator| envelope(&harness, "second", make(locator)))
        .and_then(|envelope| {
            harness
                .execute(backend, envelope)
                .map_err(|e| e.to_string())
        }) {
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
        Err(detail) => report.aborted(
            distinct,
            format!("a second entity could not be created: {detail}"),
        ),
    }

    // The same address twice: nothing is created, so there is no fact to report.
    match envelope(&harness, "duplicate", make(taken.clone())).and_then(|envelope| {
        harness
            .execute(backend, envelope)
            .map_err(|e| e.to_string())
    }) {
        Ok(result) => report.expect(
            silent,
            result.events.is_empty(),
            format!(
                "a create at an address already taken was accepted and reported {:?}",
                result.events
            ),
        ),
        Err(detail) => report.record(Check {
            name: silent.to_owned(),
            passed: true,
            detail: Some(format!(
                "the second create at the same address was refused ({detail}), so it reported \
                 nothing at all"
            )),
        }),
    }

    // The same logical command again: the same command id, the same key, the same payload.
    match envelope(&harness, "first", make(taken))
        .and_then(|envelope| harness.execute(backend, envelope).map_err(|e| e.to_string()))
    {
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
        Err(detail) => report.record(Check {
            name: replayed.to_owned(),
            passed: true,
            detail: Some(format!(
                "the replay was not recognised as one ({detail}); whether a replay is recognised is \
                 the idempotency suite's property"
            )),
        }),
    }

    report
}

/// The name this suite mints into every identifier it creates.
///
/// A full run drives all sixteen suites against one backend, and the harness numbers its generated
/// identifiers from zero for each of them. Two suites using them raw issue the same idempotency key
/// for different commands and create entities at the same address; both are refused, and the failure
/// reads as a fault in the backend rather than as a collision between suites. Here the naming does
/// double duty: reusing one tag is exactly how the replay is made to be the same logical command.
const SUITE: &str = "events";

/// An address no other suite in the same run uses.
fn address(kind: &str, tag: &str) -> Result<EntityLocator, String> {
    EntityLocator::new(ORGANISATION, SPACE, kind, format!("{kind}-{SUITE}-{tag}"))
        .map_err(|error| error.to_string())
}

/// An envelope whose command id and idempotency key no other suite in the same run uses.
fn envelope(
    harness: &Harness,
    tag: &str,
    payload: Command,
) -> Result<CommandEnvelope<Command>, String> {
    let command_id = CommandId::new(format!("cmd-{SUITE}-{tag}")).map_err(|e| e.to_string())?;
    let key = IdempotencyKey::new(format!("key-{SUITE}-{tag}")).map_err(|e| e.to_string())?;
    Ok(harness.envelope(command_id, payload, harness.context_with_key(&key)))
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
