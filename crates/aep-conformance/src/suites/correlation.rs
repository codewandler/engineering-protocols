//! One activity is reassembled from one identifier.
//!
//! §38 keeps correlation and causation apart because they answer different questions: causation says
//! what produced this one step, correlation says what belongs together. A release, an incident or a
//! single user request is a dozen commands, events and decisions spread across minutes and services,
//! and the only thing that makes it one story afterwards is that every record carries the same
//! `correlation_id`. Lose it and the trail still contains everything that happened, in the sense
//! that a shredded document still contains every word: the question "show me everything about that
//! release" stops having an answer, and the questions people actually ask after an outage are all
//! that question.

use aep_contract::query::AuditQuery;
use aep_contract::testing::block_on;
use aep_domain::command::{Command, CreateEntity, UpdateEntity};
use aep_domain::entity::{EntityRef, EntityType, VersionedEntityRef};
use aep_domain::ids::{CommandId, CorrelationId, IdempotencyKey};
use aep_domain::node::Node;

use crate::harness::{Backend, Harness};
use crate::report::SuiteReport;

/// Runs the correlation suite.
// A suite is one ordered list of checks, each depending on the state the last one left. Splitting it
// into helpers would hide that sequence, which is the thing a reader needs to follow.
#[allow(clippy::too_many_lines)]
pub fn run<B: Backend>(backend: &B) -> SuiteReport {
    // Two activities against one backend, because "this activity's records" is only a claim if
    // there is another activity that could have been returned and was not.
    let harness = Harness::new("correlation");
    let elsewhere = Harness::new("correlation-elsewhere");
    let mut report = SuiteReport::new("correlation");

    let carried = "every record of an activity carries that activity's identifier";
    let whole = "one activity is recoverable from a single identifier";
    let unused = "an identifier no command used returns an empty page";
    let separate = "another activity's records are not returned by this one's identifier";

    let commands = match drive(&harness, backend) {
        Ok(commands) => commands,
        Err(detail) => {
            report.aborted(whole, detail);
            return report;
        }
    };
    let elsewhere_ran = second_activity(&elsewhere, backend);

    let records = match block_on(
        backend.audit(&AuditQuery::for_correlation(harness.correlation().clone())),
    ) {
        Ok(page) => page.items,
        Err(error) => {
            report.aborted(whole, format!("audit could not be queried: {error}"));
            return report;
        }
    };

    let foreign: Vec<String> = records
        .iter()
        .filter(|record| &record.correlation_id != harness.correlation())
        .map(|record| format!("{} carries {}", record.audit_id, record.correlation_id))
        .collect();
    report.expect(
        carried,
        foreign.is_empty(),
        format!(
            "asking for activity {} returned {} record(s) belonging to something else: {foreign:?}",
            harness.correlation(),
            foreign.len()
        ),
    );

    report.expect(
        whole,
        records.len() >= commands,
        format!(
            "{commands} command(s) were issued under {} and the trail returns {} record(s)",
            harness.correlation(),
            records.len()
        ),
    );

    match CorrelationId::new("corr-correlation-nobody-used-this") {
        Ok(nobody) => match block_on(backend.audit(&AuditQuery::for_correlation(nobody.clone()))) {
            Ok(page) => report.expect(
                unused,
                page.is_empty(),
                format!(
                    "{} was never issued and yet {} record(s) came back, so the filter is not \
                     being applied",
                    nobody,
                    page.len()
                ),
            ),
            Err(error) => report.aborted(unused, error.to_string()),
        },
        Err(error) => report.aborted(unused, error.to_string()),
    }

    let strangers: Vec<String> = records
        .iter()
        .filter(|record| &record.actor == elsewhere.actor())
        .map(|record| record.audit_id.to_string())
        .collect();
    let theirs = block_on(backend.audit(&AuditQuery::for_correlation(
        elsewhere.correlation().clone(),
    )))
    .map_or(0, |page| page.len());
    report.expect(
        separate,
        strangers.is_empty() && elsewhere_ran.is_ok() && theirs > 0,
        format!(
            "the other activity {} has {theirs} record(s) of its own ({:?}), and {} of them were \
             returned for {}",
            elsewhere.correlation(),
            elsewhere_ran.as_ref().err(),
            strangers.len(),
            harness.correlation()
        ),
    );

    report
}

/// Issues several commands under one activity, returning how many were issued.
fn drive<B: Backend>(harness: &Harness, backend: &B) -> Result<usize, String> {
    let entity_type = EntityType::parse("aep.design/v1").map_err(|error| error.to_string())?;
    let locator = harness.locator("design");
    let created = harness
        .run(
            backend,
            Command::CreateEntity(CreateEntity {
                entity_type,
                locator: locator.clone(),
                data: Node::Map(
                    [
                        (
                            "title".to_owned(),
                            Node::from("A design under one activity"),
                        ),
                        ("status".to_owned(), Node::from("in_review")),
                    ]
                    .into(),
                ),
            }),
        )
        .map_err(|error| format!("the entity could not be created: {error}"))?;
    let design = settle(harness, backend, created.affected.first(), &locator)?;

    for note in ["a first revision of the summary", "a second one"] {
        harness
            .run(
                backend,
                Command::UpdateEntity(UpdateEntity {
                    target: design.unversioned(),
                    changes: [("summary".to_owned(), Node::from(note))].into(),
                }),
            )
            .map_err(|error| format!("the entity could not be updated: {error}"))?;
    }

    Ok(3)
}

/// Runs one command under a second activity, so the first activity's query has something to exclude.
fn second_activity<B: Backend>(harness: &Harness, backend: &B) -> Result<(), String> {
    let entity_type = EntityType::parse("aep.story/v1").map_err(|error| error.to_string())?;
    // Both harnesses generate identifiers from their own counter, so the second one is given keys of
    // its own: reusing a key for a different command is refused, and rightly so.
    let key = IdempotencyKey::new("key-correlation-elsewhere").map_err(|e| e.to_string())?;
    let command_id = CommandId::new("cmd-correlation-elsewhere").map_err(|e| e.to_string())?;
    let payload = Command::CreateEntity(CreateEntity {
        entity_type,
        locator: harness.locator("story"),
        data: Node::Map([("title".to_owned(), Node::from("Somebody else's work"))].into()),
    });
    harness
        .execute(
            backend,
            harness.envelope(command_id, payload, harness.context_with_key(&key)),
        )
        .map_err(|error| format!("the second activity's command failed: {error}"))?;
    Ok(())
}

/// Where a create landed, falling back to the locator when the backend reports nothing.
fn settle<B: Backend>(
    harness: &Harness,
    backend: &B,
    affected: Option<&VersionedEntityRef>,
    locator: &aep_domain::entity::EntityLocator,
) -> Result<VersionedEntityRef, String> {
    if let Some(reference) = affected {
        return Ok(reference.clone());
    }
    let id = block_on(backend.resolve(locator)).map_err(|error| error.to_string())?;
    Ok(harness
        .read(backend, &EntityRef::new(id))?
        .metadata
        .versioned_reference())
}

#[cfg(test)]
mod tests {
    use super::*;
    use aep_backend_memory::MemoryBackend;

    #[test]
    fn the_reference_backend_keeps_one_activity_together() {
        let report = run(&MemoryBackend::new());
        assert!(report.passed(), "{report}");
    }

    #[test]
    fn a_backend_that_scrambles_the_activity_fails() {
        let backend = crate::faulty::FaultyBackend::new(
            MemoryBackend::new(),
            crate::faulty::Fault::ScrambleCorrelation,
        );
        let report = run(&backend);
        assert!(
            !report.passed(),
            "records that cannot be reassembled into one activity:\n{report}"
        );
    }
}
