//! Nothing is deleted, and what was written as evidence stays as it was written.
//!
//! Two properties that look like housekeeping and are not. §43 gives the contract no physical
//! delete: an entity leaves active use by being archived or superseded and remains addressable
//! afterwards, so a question asked six months later still has something to read. §78.9 adds the
//! other half — a type declared immutable refuses an edit instead of quietly accepting one. Without
//! the first, an archived design that answers `not_found` is a deletion with better manners, and
//! every audit record, review and history entry pointing at it becomes a dangling reference. Without
//! the second, a review result can be rewritten after the fact, which means it no longer records
//! what anybody concluded and cannot be used as evidence for anything.

use aep_contract::testing::block_on;
use aep_domain::command::{ArchiveEntity, Command, CreateEntity, UpdateEntity};
use aep_domain::entity::{EntityRef, EntityType, VersionedEntityRef};
use aep_domain::node::Node;

use crate::harness::{Backend, Harness};
use crate::report::SuiteReport;

/// Runs the immutability suite.
// A suite is one ordered list of checks, each depending on the state the last one left. Splitting it
// into helpers would hide that sequence, which is the thing a reader needs to follow.
#[allow(clippy::too_many_lines)]
pub fn run<B: Backend>(backend: &B) -> SuiteReport {
    let harness = Harness::new("immutability");
    let mut report = SuiteReport::new("immutability");

    archiving_is_not_deletion(&harness, backend, &mut report);
    an_immutable_type_refuses_an_edit(&harness, backend, &mut report);

    report
}

/// Establishes that archiving moves an entity on rather than making it go away.
fn archiving_is_not_deletion<B: Backend>(harness: &Harness, backend: &B, report: &mut SuiteReport) {
    let readable = "an archived entity is still readable, because there is no delete";
    let advances = "archiving advances the revision rather than removing anything";
    let states = "archiving records a status rather than a disappearance";

    let story = match plant(
        harness,
        backend,
        "aep.story/v1",
        "story",
        &[
            ("title", Node::from("A story that has been overtaken")),
            ("status", Node::from("active")),
        ],
    ) {
        Ok(reference) => reference,
        Err(detail) => {
            report.aborted(readable, detail);
            return;
        }
    };

    if let Err(error) = harness.run(
        backend,
        Command::ArchiveEntity(ArchiveEntity {
            target: story.unversioned(),
            reason: Some("overtaken by later work".to_owned()),
        }),
    ) {
        report.aborted(readable, format!("archiving was refused: {error}"));
        return;
    }

    let archived = harness.read(backend, &story.unversioned());
    report.expect(
        readable,
        archived.is_ok(),
        archived.as_ref().err().map_or_else(String::new, |detail| {
            format!("reading the archived entity back failed with {detail}")
        }),
    );

    let Ok(entity) = archived else {
        report.aborted(advances, "the archived entity could not be read back");
        report.aborted(states, "the archived entity could not be read back");
        return;
    };

    report.expect(
        advances,
        entity.metadata.revision.get() > story.revision.get(),
        format!(
            "the entity was at revision {} before archiving and is at {} after",
            story.revision.get(),
            entity.metadata.revision.get()
        ),
    );

    let recorded_status = entity
        .data
        .as_map()
        .and_then(|fields| fields.get("status"))
        .and_then(Node::as_text);
    report.expect(
        states,
        recorded_status == Some("archived"),
        format!("the archived entity's `status` reads {recorded_status:?}"),
    );
}

/// Establishes that an edit to an immutable type is refused, and refused for the right reason.
fn an_immutable_type_refuses_an_edit<B: Backend>(
    harness: &Harness,
    backend: &B,
    report: &mut SuiteReport,
) {
    let refused = "an update to an immutable type is refused";
    let named = "the refusal names immutability rather than looking like a missing permission";
    let unchanged = "a refused edit leaves the immutable record at the revision it was written at";

    let review = match plant(
        harness,
        backend,
        "aep.review-result/v1",
        "review-result",
        &[
            ("title", Node::from("What the reviewer concluded")),
            ("status", Node::from("active")),
        ],
    ) {
        Ok(reference) => reference,
        Err(detail) => {
            report.aborted(refused, detail);
            return;
        }
    };

    let attempt = harness.run(
        backend,
        Command::UpdateEntity(UpdateEntity {
            target: review.unversioned(),
            changes: [(
                "title".to_owned(),
                Node::from("What the reviewer would rather have concluded"),
            )]
            .into(),
        }),
    );

    match &attempt {
        Ok(result) => {
            report.expect(
                refused,
                false,
                format!(
                    "the edit was accepted, with outcome {:?}; a record that can be rewritten \
                     afterwards is not evidence",
                    result.outcome
                ),
            );
            report.aborted(named, "the edit was not refused at all");
        }
        Err(error) => {
            report.expect(refused, true, String::new());
            let code = error.code();
            let message = error.to_string();
            report.expect(
                named,
                code != "unauthorised" && message.to_lowercase().contains("immutable"),
                format!(
                    "the refusal came back as `{code}`: {message} — an implementer reading this \
                     cannot tell immutability from a missing capability"
                ),
            );
        }
    }

    match harness.read(backend, &review.unversioned()) {
        Ok(entity) => report.expect(
            unchanged,
            entity.metadata.revision.get() == review.revision.get(),
            format!(
                "the record was written at revision {} and is now at {}",
                review.revision.get(),
                entity.metadata.revision.get()
            ),
        ),
        Err(detail) => report.aborted(unchanged, detail),
    }
}

/// Creates an entity and reports where it landed.
///
/// It falls back to resolving the locator when a backend reports no affected entity, so that a
/// backend failing the `command-execution` suite is not failed here for the same reason twice.
fn plant<B: Backend>(
    harness: &Harness,
    backend: &B,
    entity_type: &str,
    kind: &str,
    fields: &[(&str, Node)],
) -> Result<VersionedEntityRef, String> {
    let entity_type = EntityType::parse(entity_type).map_err(|error| error.to_string())?;
    let locator = harness.locator(kind);
    let data = Node::Map(
        fields
            .iter()
            .map(|(key, value)| ((*key).to_owned(), value.clone()))
            .collect(),
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
    fn the_reference_backend_deletes_nothing_and_refuses_to_edit_evidence() {
        let report = run(&MemoryBackend::new());
        assert!(report.passed(), "{report}");
    }

    #[test]
    fn a_backend_that_hides_archived_entities_fails() {
        let backend = crate::faulty::FaultyBackend::new(
            MemoryBackend::new(),
            crate::faulty::Fault::HideArchived,
        );
        let report = run(&backend);
        assert!(
            !report.passed(),
            "hiding an archived entity is deletion by another name:\n{report}"
        );
    }
}
