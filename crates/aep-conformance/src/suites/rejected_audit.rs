//! A refused command changes nothing and is still recorded.
//!
//! §55 is what separates an audit trail from a success log. "An agent attempted a production write
//! and policy denied it" is the row that matters in a security review, an incident post-mortem and
//! an access audit — and it is precisely the row a system that only writes on success does not have.
//! The refusal has to be findable *as* a refusal (`AuditQuery::rejected()`), it has to carry a
//! machine-readable reason, and it must not carry a change record: a row claiming a mutation that
//! never happened is worse than no row at all, which is why `AuditRecord::validate` refuses one that
//! claims both. This suite drives two refusals a backend cannot talk its way out of — a write
//! against a revision that has moved on, and a second entity at an address already taken — and then
//! asks the trail about them.

use aep_contract::query::AuditQuery;
use aep_contract::testing::block_on;
use aep_domain::audit::AuditRecord;
use aep_domain::command::{Command, CreateEntity, UpdateEntity};
use aep_domain::entity::{
    EntityLocator, EntityRef, EntityRevision, EntityType, VersionedEntityRef,
};
use aep_domain::ids::AuditId;
use aep_domain::node::Node;

use crate::harness::{Backend, Harness};
use crate::report::SuiteReport;

/// Runs the rejected-action audit suite.
pub fn run<B: Backend>(backend: &B) -> SuiteReport {
    let harness = Harness::new("rejected-audit");
    let mut report = SuiteReport::new("rejected-audit");

    let left = "a refused command leaves an audit record";
    let only = "a rejection query returns refusals and nothing else";
    let no_change = "a refused command's record carries no change record";
    let explained = "a refusal records a decision that says it was not allowed, and why";
    let separated = "a rejection query does not return the commands that were accepted";

    let accepted = match refuse_two_commands(&harness, backend) {
        Ok(accepted) => accepted,
        Err(detail) => {
            report.aborted(left, detail);
            return report;
        }
    };

    let rejections = match block_on(
        backend.audit(&AuditQuery::for_correlation(harness.correlation().clone()).rejected()),
    ) {
        Ok(page) => page.items,
        Err(error) => {
            report.aborted(left, format!("rejections could not be queried: {error}"));
            return report;
        }
    };

    report.expect(
        left,
        !rejections.is_empty(),
        "two commands were refused and the trail has nothing to say about either, so an attempt \
         that was stopped is indistinguishable from an attempt that never happened",
    );

    if rejections.is_empty() {
        report.aborted(only, "no refusal was returned");
        report.aborted(no_change, "no refusal was returned");
        report.aborted(explained, "no refusal was returned");
        report.aborted(separated, "no refusal was returned");
        return report;
    }

    let not_refusals: Vec<String> = rejections
        .iter()
        .filter(|record| !record.is_rejection())
        .map(|record| format!("{} ({})", record.audit_id, record.kind))
        .collect();
    report.expect(
        only,
        not_refusals.is_empty(),
        format!("a query for refusals also returned {not_refusals:?}"),
    );

    let mutating: Vec<String> = rejections
        .iter()
        .filter(|record| record.mutated())
        .map(|record| record.audit_id.to_string())
        .collect();
    report.expect(
        no_change,
        mutating.is_empty(),
        format!(
            "{} refusal record(s) ({mutating:?}) claim a change, but a refused command changes \
             nothing",
            mutating.len()
        ),
    );

    let unexplained: Vec<String> = rejections
        .iter()
        .filter(|record| !explains_itself(record))
        .map(|record| record.audit_id.to_string())
        .collect();
    report.expect(
        explained,
        unexplained.is_empty(),
        format!(
            "{unexplained:?} record a refusal without a decision saying `allowed: false` and a \
             reason a reader can act on"
        ),
    );

    let leaked: Vec<String> = rejections
        .iter()
        .filter(|record| accepted.contains(&record.audit_id))
        .map(|record| record.audit_id.to_string())
        .collect();
    report.expect(
        separated,
        leaked.is_empty(),
        format!(
            "asking for refusals returned {leaked:?}, which are the records of commands that were \
             accepted"
        ),
    );

    report
}

/// `true` when a record carries a decision that refuses something and says why.
fn explains_itself(record: &AuditRecord) -> bool {
    record.decision.as_ref().is_some_and(|decision| {
        !decision.allowed && (decision.rule.is_some() || !decision.missing.is_empty())
    })
}

/// Drives two accepted commands and two refusals, returning the audit ids of the accepted ones.
///
/// Two refusals rather than one, on purpose. A stale `expected_revision` is the refusal §78.11
/// names, but a backend that ignores revision assertions turns it into an acceptance and leaves this
/// suite with nothing to look at — which is the concurrency suite's finding, not this one's. A
/// second entity at an address already taken is refused by any backend that can address entities at
/// all.
fn refuse_two_commands<B: Backend>(harness: &Harness, backend: &B) -> Result<Vec<AuditId>, String> {
    let entity_type = EntityType::parse("aep.design/v1").map_err(|error| error.to_string())?;
    let locator = harness.locator("design");
    let body = |title: &str| {
        Node::Map(
            [
                ("title".to_owned(), Node::from(title)),
                ("status".to_owned(), Node::from("in_review")),
            ]
            .into(),
        )
    };

    let created = harness
        .run(
            backend,
            Command::CreateEntity(CreateEntity {
                entity_type: entity_type.clone(),
                locator: locator.clone(),
                data: body("A design two commands will be refused against"),
            }),
        )
        .map_err(|error| format!("the entity could not be created: {error}"))?;
    let design = settle(harness, backend, created.affected.first(), &locator)?;

    let updated = harness
        .run(
            backend,
            Command::UpdateEntity(UpdateEntity {
                target: design.unversioned(),
                changes: [("title".to_owned(), Node::from("A design that has moved on"))].into(),
            }),
        )
        .map_err(|error| format!("the entity could not be updated: {error}"))?;

    // Refusal one: a write against the revision the design was at before that update.
    let stale = harness
        .envelope(
            harness.command_id(),
            Command::UpdateEntity(UpdateEntity {
                target: design.unversioned(),
                changes: [(
                    "title".to_owned(),
                    Node::from("Rewritten behind the update's back"),
                )]
                .into(),
            }),
            harness.context(),
        )
        .expecting(EntityRevision::INITIAL);
    drop(harness.execute(backend, stale));

    // Refusal two: a second entity at an address that is already taken.
    drop(harness.run(
        backend,
        Command::CreateEntity(CreateEntity {
            entity_type,
            locator,
            data: body("A design at somebody else's address"),
        }),
    ));

    Ok(created.audit.into_iter().chain(updated.audit).collect())
}

/// Where a create landed, falling back to the locator when the backend reports nothing.
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

#[cfg(test)]
mod tests {
    use super::*;
    use aep_backend_memory::MemoryBackend;

    #[test]
    fn the_reference_backend_records_what_it_refused() {
        let report = run(&MemoryBackend::new());
        assert!(report.passed(), "{report}");
    }

    #[test]
    fn a_backend_that_forgets_refusals_fails() {
        let backend = crate::faulty::FaultyBackend::new(
            MemoryBackend::new(),
            crate::faulty::Fault::DropRejectionAudit,
        );
        let report = run(&backend);
        assert!(
            !report.passed(),
            "a refusal that leaves no trace is the whole point of this suite:\n{report}"
        );
    }
}
