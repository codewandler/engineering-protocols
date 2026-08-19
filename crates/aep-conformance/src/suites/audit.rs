//! Every mutation leaves a record that names who, what and which revision.
//!
//! §52 makes audit a cross-cutting concern rather than a feature, and §78.10 fixes what a record
//! has to be able to answer about a successful change: who authorised it, what executed it, which
//! entity moved, and from which revision to which. This is not logging. A log line is written for
//! whoever is watching now; an audit record is written for whoever asks in six months, by which time
//! the only thing left is the record. A backend that writes nothing, or writes a single `user` field
//! that conflates a person with the agent acting for them, can answer "something changed" and
//! nothing else — which is exactly the answer that makes an access review or an incident review
//! impossible.

use aep_contract::query::AuditQuery;
use aep_contract::testing::block_on;
use aep_domain::audit::AuditRecord;
use aep_domain::command::{Command, CreateEntity};
use aep_domain::entity::{EntityRef, EntityType, VersionedEntityRef};
use aep_domain::node::Node;

use crate::harness::{Backend, Harness};
use crate::report::SuiteReport;

/// Runs the audit suite.
// A suite is one ordered list of checks, each depending on the state the last one left. Splitting it
// into helpers would hide that sequence, which is the thing a reader needs to follow.
#[allow(clippy::too_many_lines)]
pub fn run<B: Backend>(backend: &B) -> SuiteReport {
    let harness = Harness::new("audit");
    let mut report = SuiteReport::new("audit");

    let left = "a create leaves at least one audit record";
    let names_actor = "an audit record names the actor the command ran on behalf of";
    let names_executor = "an audit record names the executor separately from the actor";
    let names_change = "an audit record says which entity changed and which revision it reached";
    let scoped = "audit by entity returns that entity's records and no others";

    let design = match plant(&harness, backend, "design") {
        Ok(reference) => reference,
        Err(detail) => {
            report.aborted(left, detail);
            return report;
        }
    };
    // A second entity, so that "only this entity's records" is a claim about filtering rather than
    // a claim about there being nothing else to return.
    let elsewhere = match plant(&harness, backend, "story") {
        Ok(reference) => reference,
        Err(detail) => {
            report.aborted(scoped, detail);
            return report;
        }
    };

    let records = match block_on(backend.audit(&AuditQuery::for_entity(design.unversioned()))) {
        Ok(page) => page.items,
        Err(error) => {
            report.aborted(left, format!("audit could not be queried: {error}"));
            return report;
        }
    };

    report.expect(
        left,
        !records.is_empty(),
        "creating an entity produced no audit record, so the change cannot be explained afterwards",
    );

    let Some(record) = records.first() else {
        report.aborted(names_actor, "there was no record to read");
        report.aborted(names_executor, "there was no record to read");
        report.aborted(names_change, "there was no record to read");
        report.aborted(scoped, "there was no record to read");
        return report;
    };

    report.expect(
        names_actor,
        &record.actor == harness.actor(),
        format!(
            "the command was issued by {} and the record is attributed to {}",
            harness.actor(),
            record.actor
        ),
    );

    let executor = record.executor.as_ref();
    report.expect(
        names_executor,
        executor == Some(harness.executor()) && executor != Some(&record.actor),
        format!(
            "the command named executor {} and the record carries {}",
            harness.executor(),
            executor.map_or_else(|| "nothing".to_owned(), ToString::to_string)
        ),
    );

    report.expect(
        names_change,
        records.iter().any(|candidate| covers(candidate, &design)),
        format!(
            "no record covers entity {} at revision {}; the records for it carry {:?}",
            design.id,
            design.revision.get(),
            records
                .iter()
                .map(|candidate| candidate.change.as_ref().map(|change| change
                    .after
                    .map_or(0, aep_domain::entity::EntityRevision::get)))
                .collect::<Vec<_>>()
        ),
    );

    let strays: Vec<String> = records
        .iter()
        .filter(|candidate| {
            candidate
                .subject
                .as_ref()
                .is_none_or(|subject| subject.id != design.id)
        })
        .map(|candidate| candidate.audit_id.to_string())
        .collect();
    report.expect(
        scoped,
        strays.is_empty(),
        format!(
            "asking for {}'s records also returned {} record(s) about something else ({strays:?}); \
             a second entity {} exists in the same trail",
            design.id,
            strays.len(),
            elsewhere.id
        ),
    );

    report
}

/// `true` when `record` reports the change that brought `reference` to its revision.
fn covers(record: &AuditRecord, reference: &VersionedEntityRef) -> bool {
    record.change.as_ref().is_some_and(|change| {
        change.entity.id == reference.id && change.after == Some(reference.revision)
    })
}

/// Creates an entity and reports where it landed.
///
/// It falls back to resolving the locator when a backend reports no affected entity, so that a
/// backend failing the `command-execution` suite is not failed here for the same reason twice.
fn plant<B: Backend>(
    harness: &Harness,
    backend: &B,
    kind: &str,
) -> Result<VersionedEntityRef, String> {
    let entity_type = EntityType::parse(&format!("aep.{kind}/v1")).map_err(|e| e.to_string())?;
    let locator = harness.locator(kind);
    let data = Node::Map(
        [
            ("title".to_owned(), Node::from("An entity under audit")),
            ("status".to_owned(), Node::from("active")),
        ]
        .into(),
    );
    let result = harness
        .run(
            backend,
            Command::CreateEntity(CreateEntity {
                entity_type,
                locator: locator.clone(),
                data,
            }),
        )
        .map_err(|error| format!("the entity could not be created: {error}"))?;
    if let Some(reference) = result.affected.first() {
        return Ok(reference.clone());
    }
    let id = block_on(backend.resolve(&locator)).map_err(|error| error.to_string())?;
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
    fn the_reference_backend_records_who_changed_what() {
        let report = run(&MemoryBackend::new());
        assert!(report.passed(), "{report}");
    }

    #[test]
    fn a_backend_that_audits_nothing_fails() {
        let backend = crate::faulty::FaultyBackend::new(
            MemoryBackend::new(),
            crate::faulty::Fault::DropAudit,
        );
        let report = run(&backend);
        assert!(!report.passed(), "{report}");
    }
}
