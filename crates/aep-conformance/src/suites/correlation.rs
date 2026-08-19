//! One activity is reassembled from one identifier.
//!
//! §38 keeps correlation and causation apart because they answer different questions: causation says
//! what produced this one step, correlation says what belongs together. A release, an incident or a
//! single user request is a dozen commands, events and decisions spread over minutes and services,
//! and the only thing that makes it one story afterwards is that every record carries the same
//! `correlation_id`. Lose it and the trail still contains everything that happened, in the sense
//! that a shredded document still contains every word: "show me everything about that release" stops
//! having an answer, and the questions people actually ask after an outage are all that question.

use aep_contract::command::{CommandEnvelope, CommandResult};
use aep_contract::query::AuditQuery;
use aep_contract::testing::block_on;
use aep_domain::command::{Command, CreateEntity, UpdateEntity};
use aep_domain::entity::{EntityLocator, EntityRef, EntityType, VersionedEntityRef};
use aep_domain::ids::{CommandId, CorrelationId, IdempotencyKey};
use aep_domain::node::Node;

use crate::harness::{Backend, Harness, ORGANISATION, SPACE};
use crate::report::SuiteReport;

/// How many commands the suite issues under the activity it then asks about.
const ISSUED: usize = 3;

/// Runs the correlation suite.
pub fn run<B: Backend>(backend: &B) -> SuiteReport {
    // Two activities against one backend, because "this activity's records" is only a claim if
    // there is another activity that could have been returned and was not.
    let harness = Harness::new(SUITE);
    let elsewhere = Harness::new("correlation-elsewhere");
    let mut report = SuiteReport::new("correlation");

    let carried = "every record of an activity carries that activity's identifier";
    let whole = "one activity is recoverable from a single identifier";
    let unused = "an identifier no command used returns an empty page";
    let separate = "another activity's records are not returned by this one's identifier";

    if let Err(detail) = drive(&harness, backend) {
        report.aborted(whole, detail);
        return report;
    }
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
        records.len() >= ISSUED,
        format!(
            "{ISSUED} command(s) were issued under {} and the trail returns {} record(s)",
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
                    "{nobody} was never issued and yet {} record(s) came back, so the filter is \
                     not being applied",
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
            "the other activity {} has {theirs} record(s) of its own ({:?}), and {} of them came \
             back for {}",
            elsewhere.correlation(),
            elsewhere_ran.as_ref().err(),
            strangers.len(),
            harness.correlation()
        ),
    );

    report
}

/// Issues [`ISSUED`] commands under one activity.
fn drive<B: Backend>(harness: &Harness, backend: &B) -> Result<(), String> {
    let entity_type = EntityType::parse("aep.design/v1").map_err(|error| error.to_string())?;
    let locator = address("design", "subject")?;
    let created = issue(
        harness,
        backend,
        "create",
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

    for (tag, note) in [
        ("first-edit", "a first revision of the summary"),
        ("second-edit", "a second one"),
    ] {
        issue(
            harness,
            backend,
            tag,
            Command::UpdateEntity(UpdateEntity {
                target: design.unversioned(),
                changes: [("summary".to_owned(), Node::from(note))].into(),
            }),
        )
        .map_err(|error| format!("the entity could not be updated: {error}"))?;
    }

    Ok(())
}

/// Runs one command under a second activity, so the first activity's query has something to exclude.
fn second_activity<B: Backend>(harness: &Harness, backend: &B) -> Result<(), String> {
    let entity_type = EntityType::parse("aep.story/v1").map_err(|error| error.to_string())?;
    issue(
        harness,
        backend,
        "elsewhere",
        Command::CreateEntity(CreateEntity {
            entity_type,
            locator: address("story", "elsewhere")?,
            data: Node::Map([("title".to_owned(), Node::from("Somebody else's work"))].into()),
        }),
    )
    .map_err(|error| format!("the second activity's command failed: {error}"))?;
    Ok(())
}

/// Where a create landed, falling back to the address when the backend reports nothing.
fn settle<B: Backend>(
    harness: &Harness,
    backend: &B,
    affected: Option<&VersionedEntityRef>,
    locator: &EntityLocator,
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

/// The name this suite mints into every identifier it creates.
///
/// A full run drives all sixteen suites against one backend, and the harness numbers its generated
/// identifiers from zero for each of them. Two suites using them raw issue the same idempotency key
/// for different commands and create entities at the same address; both are refused, and the failure
/// reads as a fault in the backend rather than as a collision between suites. The same applies to
/// the two harnesses here, which are two activities and not two runs.
const SUITE: &str = "correlation";

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

/// Issues a command under identifiers this suite owns.
fn issue<B: Backend>(
    harness: &Harness,
    backend: &B,
    tag: &str,
    payload: Command,
) -> Result<CommandResult, String> {
    harness
        .execute(backend, envelope(harness, tag, payload)?)
        .map_err(|error| error.to_string())
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
