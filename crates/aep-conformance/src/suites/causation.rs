//! The immediate cause of each step is recoverable.
//!
//! Correlation gathers an activity into a bag; causation is what gives the bag an order. §38 draws
//! the chain — a command causes an event, the event causes a protocol decision, the decision causes
//! the next command — and §78.13 asks that it be walkable afterwards. The difference shows up the
//! first time somebody asks *why* rather than *what*: with correlation alone the answer is "these
//! forty things happened during the release", with causation it is "this deploy happened because
//! that approval was granted because that verification passed". A backend that records a cause it
//! cannot resolve, or drops the command a record came from, leaves an implementer with a pile of
//! rows and no way to order them.

use aep_contract::command::CausationRef as StatedCause;
use aep_contract::query::AuditQuery;
use aep_contract::testing::block_on;
use aep_domain::audit::{AuditRecord, CausationRef};
use aep_domain::command::{Command, CreateEntity, UpdateEntity};
use aep_domain::entity::{EntityRef, EntityType, VersionedEntityRef};
use aep_domain::ids::CommandId;
use aep_domain::node::Node;

use crate::harness::{Backend, Harness};
use crate::report::{Check, SuiteReport};

/// What one run of the scenario issued, so the trail can be asked about it by name.
struct Chain {
    /// The command that created the entity.
    first: CommandId,
    /// The command issued as a consequence of the first.
    second: CommandId,
    /// A command the backend was expected to refuse.
    refused: CommandId,
}

/// Runs the causation suite.
// A suite is one ordered list of checks, each depending on the state the last one left. Splitting it
// into helpers would hide that sequence, which is the thing a reader needs to follow.
#[allow(clippy::too_many_lines)]
pub fn run<B: Backend>(backend: &B) -> SuiteReport {
    let harness = Harness::new("causation");
    let mut report = SuiteReport::new("causation");

    let names_command = "an accepted command's audit record names the command it came from";
    let carries_cause = "a command issued with a stated cause produces a record that carries one";
    let walkable = "a chain of two commands can be walked from the second back to the first";
    let refusal_named = "the record for a refusal still names the command that was refused";

    let chain = match drive(&harness, backend) {
        Ok(chain) => chain,
        Err(detail) => {
            report.aborted(names_command, detail);
            return report;
        }
    };

    let records = match block_on(
        backend.audit(&AuditQuery::for_correlation(harness.correlation().clone())),
    ) {
        Ok(page) => page.items,
        Err(error) => {
            report.aborted(
                names_command,
                format!("audit could not be queried: {error}"),
            );
            return report;
        }
    };

    report.expect(
        names_command,
        find(&records, &chain.first).is_some(),
        format!(
            "no record in the trail names command {}; the trail names {:?}",
            chain.first,
            named_commands(&records)
        ),
    );

    let second = find(&records, &chain.second);
    report.expect(
        carries_cause,
        second.is_some_and(|record| record.causation.is_some()),
        format!(
            "command {} was issued naming its cause, and its record carries {}",
            chain.second,
            second.map_or_else(
                || "no record at all".to_owned(),
                |record| record
                    .causation
                    .as_ref()
                    .map_or_else(|| "no cause".to_owned(), ToString::to_string)
            )
        ),
    );

    report.expect(
        walkable,
        second.is_some_and(|record| leads_to(&records, record, &chain.first)),
        format!(
            "walking back from {} does not arrive at {}",
            chain.second, chain.first
        ),
    );

    // A backend that records no refusal at all fails the rejected-audit suite, and failing it here
    // as well would only tell an implementer to look in two places for one fault.
    let refusals: Vec<&AuditRecord> = records
        .iter()
        .filter(|record| record.is_rejection())
        .collect();
    if refusals.is_empty() {
        report.record(Check {
            name: refusal_named.to_owned(),
            passed: true,
            detail: Some(
                "the trail returned no refusal to inspect; whether a refusal is recorded at all is \
                 the rejected-audit suite's property"
                    .to_owned(),
            ),
        });
    } else {
        report.expect(
            refusal_named,
            refusals
                .iter()
                .any(|record| record.command_id.as_ref() == Some(&chain.refused)),
            format!(
                "command {} was refused and the {} refusal record(s) in the trail name {:?}",
                chain.refused,
                refusals.len(),
                refusals
                    .iter()
                    .map(|record| record.command_id.as_ref().map(ToString::to_string))
                    .collect::<Vec<_>>()
            ),
        );
    }

    report
}

/// The record a command produced, if the trail names it.
fn find<'a>(records: &'a [AuditRecord], command: &CommandId) -> Option<&'a AuditRecord> {
    records
        .iter()
        .find(|record| record.command_id.as_ref() == Some(command))
}

/// Every command the trail names, for a failure detail worth reading.
fn named_commands(records: &[AuditRecord]) -> Vec<String> {
    records
        .iter()
        .filter_map(|record| record.command_id.as_ref().map(ToString::to_string))
        .collect()
}

/// `true` when `record`'s stated cause resolves, directly or through its record, to `command`.
fn leads_to(records: &[AuditRecord], record: &AuditRecord, command: &CommandId) -> bool {
    match record.causation.as_ref() {
        Some(CausationRef::Command { command: named }) => named == command,
        Some(CausationRef::Decision { decision }) => records.iter().any(|earlier| {
            &earlier.audit_id == decision && earlier.command_id.as_ref() == Some(command)
        }),
        _ => false,
    }
}

/// Issues a command, a second command caused by it, and a third the backend should refuse.
fn drive<B: Backend>(harness: &Harness, backend: &B) -> Result<Chain, String> {
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

    let first = harness.command_id();
    let created = harness
        .execute(
            backend,
            harness.envelope(
                first.clone(),
                Command::CreateEntity(CreateEntity {
                    entity_type: entity_type.clone(),
                    locator: locator.clone(),
                    data: body("A design at the head of a causal chain"),
                }),
                harness.context(),
            ),
        )
        .map_err(|error| format!("the entity could not be created: {error}"))?;
    let design = settle(harness, backend, created.affected.first(), &locator)?;

    // Name the first step's own audit record as the cause where the backend reported one; a backend
    // that reports none can still be asked to carry the command forward.
    let cause = created
        .audit
        .first()
        .map_or_else(|| first.to_string(), ToString::to_string);

    let second = harness.command_id();
    harness
        .execute(
            backend,
            harness.envelope(
                second.clone(),
                Command::UpdateEntity(UpdateEntity {
                    target: design.unversioned(),
                    changes: [("status".to_owned(), Node::from("approved"))].into(),
                }),
                harness.context().caused_by(StatedCause(cause)),
            ),
        )
        .map_err(|error| format!("the second command failed: {error}"))?;

    // A second entity at an address already taken: refused by any backend that can address entities.
    let refused = harness.command_id();
    drop(harness.execute(
        backend,
        harness.envelope(
            refused.clone(),
            Command::CreateEntity(CreateEntity {
                entity_type,
                locator,
                data: body("A design at somebody else's address"),
            }),
            harness.context(),
        ),
    ));

    Ok(Chain {
        first,
        second,
        refused,
    })
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
    fn the_reference_backend_keeps_the_chain_walkable() {
        let report = run(&MemoryBackend::new());
        assert!(report.passed(), "{report}");
    }

    #[test]
    fn a_backend_that_forgets_what_caused_a_step_fails() {
        let backend = crate::faulty::FaultyBackend::new(
            MemoryBackend::new(),
            crate::faulty::Fault::DropCausation,
        );
        let report = run(&backend);
        assert!(!report.passed(), "{report}");
    }
}
