//! Every entity says who made it and who touched it last.
//!
//! §78.14 asks four things of an entity's own metadata, and §58 says why the fourth is separate:
//! creation attribution, update attribution, executor attribution where one was given, and no
//! collapsing of `actor` into `executor`. The audit trail can reconstruct all of this by replaying
//! records, but that is the expensive answer to a question asked constantly — "who owns this, who
//! last changed it, and was it a person or an agent?" — and a backend that keeps a single
//! `modified_by` field has quietly overwritten the creator of everything that was ever edited. In
//! agentic work `actor: human:alice, executor: agent:release-agent-17` is the ordinary case, and a
//! backend that stores one of them can say neither who is accountable nor what actually ran.

use aep_contract::command::CommandEnvelope;
use aep_contract::testing::block_on;
use aep_domain::command::{Command, CreateEntity, UpdateEntity};
use aep_domain::entity::{EntityLocator, EntityRef, EntityType, VersionedEntityRef};
use aep_domain::ids::{CommandId, IdempotencyKey};
use aep_domain::node::Node;

use crate::harness::{Backend, Harness, ORGANISATION, SPACE};
use crate::report::SuiteReport;

/// Runs the provenance suite.
pub fn run<B: Backend>(backend: &B) -> SuiteReport {
    // Two actors, because "the creator survives an edit" is only a claim when somebody else edits.
    let creator = Harness::new(SUITE);
    let editor = Harness::new("provenance-editor");
    let mut report = SuiteReport::new("provenance");

    let names_creator = "a created entity names the actor who created it";
    let keeps_creator = "a change by a second actor leaves the creator unchanged";
    let names_editor = "a change names the actor who made it";
    let keeps_executor = "the executor of a change is recorded separately from the actor";
    let ordered = "an entity is never updated before it was created";

    let design = match plant(&creator, backend) {
        Ok(reference) => reference,
        Err(detail) => {
            report.aborted(names_creator, detail);
            return report;
        }
    };

    match creator.read(backend, &design.unversioned()) {
        Ok(entity) => report.expect(
            names_creator,
            entity.metadata.provenance.created_by == *creator.actor(),
            format!(
                "{} created the entity and its provenance credits {}",
                creator.actor(),
                entity.metadata.provenance.created_by
            ),
        ),
        Err(detail) => report.aborted(names_creator, detail),
    }

    if let Err(detail) = amend(&creator, &editor, backend, &design) {
        for property in [keeps_creator, names_editor, keeps_executor, ordered] {
            report.aborted(property, detail.clone());
        }
        return report;
    }

    let entity = match creator.read(backend, &design.unversioned()) {
        Ok(entity) => entity,
        Err(detail) => {
            for property in [keeps_creator, names_editor, keeps_executor, ordered] {
                report.aborted(property, detail.clone());
            }
            return report;
        }
    };
    let provenance = &entity.metadata.provenance;

    report.expect(
        keeps_creator,
        provenance.created_by == *creator.actor(),
        format!(
            "{} created the entity, {} then changed it, and the entity now credits {} with \
             creating it",
            creator.actor(),
            editor.actor(),
            provenance.created_by
        ),
    );

    report.expect(
        names_editor,
        provenance.updated_by == *editor.actor(),
        format!(
            "{} made the last change and the entity credits {}",
            editor.actor(),
            provenance.updated_by
        ),
    );

    let executor = provenance.updated_executor.as_ref();
    report.expect(
        keeps_executor,
        executor == Some(editor.executor()) && executor != Some(&provenance.updated_by),
        format!(
            "the change named executor {} and the entity carries {}",
            editor.executor(),
            executor.map_or_else(|| "nothing".to_owned(), ToString::to_string)
        ),
    );

    report.expect(
        ordered,
        entity.metadata.created_at <= entity.metadata.updated_at,
        format!(
            "the entity reports created_at {:?} and updated_at {:?}",
            entity.metadata.created_at, entity.metadata.updated_at
        ),
    );

    report
}

/// Creates the entity whose provenance the suite is about.
fn plant<B: Backend>(harness: &Harness, backend: &B) -> Result<VersionedEntityRef, String> {
    let entity_type = EntityType::parse("aep.design/v1").map_err(|error| error.to_string())?;
    let locator = address("design", "subject")?;
    let result = harness
        .execute(
            backend,
            envelope(
                harness,
                "create",
                Command::CreateEntity(CreateEntity {
                    entity_type,
                    locator: locator.clone(),
                    data: Node::Map(
                        [
                            (
                                "title".to_owned(),
                                Node::from("A design with a history of hands"),
                            ),
                            ("status".to_owned(), Node::from("in_review")),
                        ]
                        .into(),
                    ),
                }),
            )?,
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

/// Changes the entity as a second actor, later than it was created.
fn amend<B: Backend>(
    creator: &Harness,
    editor: &Harness,
    backend: &B,
    design: &VersionedEntityRef,
) -> Result<(), String> {
    let mut change = envelope(
        editor,
        "amend",
        Command::UpdateEntity(UpdateEntity {
            target: design.unversioned(),
            changes: [(
                "title".to_owned(),
                Node::from("A design somebody else has been at"),
            )]
            .into(),
        }),
    )?;
    // The two harnesses keep separate clocks. Taking this timestamp from the creator's puts the
    // change observably after the creation, which is what makes the ordering check mean something
    // without anybody sleeping.
    change.context.issued_at = creator.now();

    editor
        .execute(backend, change)
        .map_err(|error| format!("the second actor's change failed: {error}"))?;
    Ok(())
}

/// The name this suite mints into every identifier it creates.
///
/// A full run drives all sixteen suites against one backend, and the harness numbers its generated
/// identifiers from zero for each of them. Two suites using them raw issue the same idempotency key
/// for different commands and create entities at the same address; both are refused, and the failure
/// reads as a fault in the backend rather than as a collision between suites. The same applies to
/// the two harnesses here, which are two actors and not two runs.
const SUITE: &str = "provenance";

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
    fn the_reference_backend_remembers_whose_work_this_is() {
        let report = run(&MemoryBackend::new());
        assert!(report.passed(), "{report}");
    }

    #[test]
    fn a_backend_that_forgets_who_created_an_entity_fails() {
        let backend = crate::faulty::FaultyBackend::new(
            MemoryBackend::new(),
            crate::faulty::Fault::ForgetProvenance,
        );
        let report = run(&backend);
        assert!(!report.passed(), "{report}");
    }
}
