//! Identity, and its distance from the address a thing was created at.
//!
//! An entity id is what the backend calls a record; a locator is what the organisation calls it. A
//! backend that derives one from the other — handing back the locator key as the id, or minting an
//! id by hashing the address — has turned identity into a key again: it can be guessed, it can be
//! reused, and two records that were once addressed alike quietly become one. Everything else in
//! this crate is anchored to the id, so a backend that gets this wrong fails elsewhere in ways that
//! look like unrelated bugs.
//!
//! Nothing in [`crate::faulty`] can break this suite, and that is a fact about identity rather than
//! a gap in the fault list: the wrapper can only perturb what crosses the boundary, and ids are
//! minted inside. A wrapper that rewrote them would be a second backend, not a faulty one. So this
//! suite runs against a working backend and earns its keep on the mistakes implementers actually
//! make — an id built out of the locator, an address that resolves to nothing, an identity that
//! moves when the entity changes.

use aep_contract::testing::block_on;
use aep_domain::command::{Command, CreateEntity, UpdateEntity};
use aep_domain::entity::{EntityLocator, EntityType, VersionedEntityRef};
use aep_domain::node::Node;

use crate::harness::{Backend, Harness};
use crate::report::SuiteReport;

/// The type every entity in this suite is created with.
///
/// One type throughout, so that two entities differ only by the address they were created at — which
/// is what makes "they were given different identities" a statement about identity rather than about
/// types.
const ENTITY_TYPE: &str = "aep.story/v1";

/// Runs the identity suite.
// Six questions about one pair of entities. Splitting them would mean creating the fixture again
// per question, and "the same entity still has the same id" is not a question about a fresh one.
#[allow(clippy::too_many_lines)]
pub fn run<B: Backend>(backend: &B) -> SuiteReport {
    let harness = Harness::new("identity");
    let mut report = SuiteReport::new("identity");

    let (address, created) = match create(&harness, backend) {
        Ok(created) => created,
        Err(error) => {
            report.aborted("an entity can be created at all", error);
            return report;
        }
    };

    match harness.read(backend, &created.unversioned()) {
        Ok(entity) => report.expect(
            "a created entity is addressable by the identity it was given",
            entity.metadata.id == created.id && entity.metadata.locator == address,
            format!(
                "created `{address}` as `{}`; that identity reads back as `{}` at `{}`",
                created.id, entity.metadata.id, entity.metadata.locator
            ),
        ),
        Err(error) => report.aborted(
            "a created entity is addressable by the identity it was given",
            error,
        ),
    }

    match create(&harness, backend) {
        Ok((second_address, second)) => {
            report.expect(
                "two entities created the same way are given different identities",
                second.id != created.id,
                format!(
                    "`{address}` and `{second_address}` hold the same body, and both were given \
                     `{}`",
                    created.id
                ),
            );
        }
        Err(error) => report.aborted(
            "two entities created the same way are given different identities",
            error,
        ),
    }

    let identity = created.id.as_str();
    let key = address.key();
    report.expect(
        "an identity is not the key it is addressed by",
        identity != key && !identity.contains(key),
        format!("`{address}` has key `{key}`, and the identity handed back is `{identity}`"),
    );

    match block_on(backend.resolve(&address)) {
        Ok(resolved) => report.expect(
            "a locator resolves to the identity of the entity created at it",
            resolved == created.id,
            format!(
                "`{address}` resolves to `{resolved}`, and was created as `{}`",
                created.id
            ),
        ),
        Err(error) => report.aborted(
            "a locator resolves to the identity of the entity created at it",
            error.to_string(),
        ),
    }

    // An address nothing was ever created at. A backend that answers here has invented an identity,
    // which is worse than failing: the caller goes on to read, write and audit against a record that
    // does not exist.
    let unknown = harness.locator("story");
    match block_on(backend.resolve(&unknown)) {
        Ok(invented) => report.expect(
            "a locator nothing was created at does not resolve",
            false,
            format!("`{unknown}` was never created, and resolved to `{invented}`"),
        ),
        Err(error) => report.expect(
            "a locator nothing was created at does not resolve",
            error.code() == "not_found",
            format!(
                "`{unknown}` was refused with `{}` rather than `not_found`",
                error.code()
            ),
        ),
    }

    let renamed = harness.run(
        backend,
        Command::UpdateEntity(UpdateEntity {
            target: created.unversioned(),
            changes: [(
                "title".to_owned(),
                Node::from("Renamed, still the same thing"),
            )]
            .into(),
        }),
    );
    match renamed {
        Ok(_) => match (
            harness.read(backend, &created.unversioned()),
            block_on(backend.resolve(&address)).map_err(|error| error.to_string()),
        ) {
            (Ok(entity), Ok(resolved)) => {
                report.expect(
                    "an entity keeps its identity when its contents change",
                    entity.metadata.id == created.id && resolved == created.id,
                    format!(
                        "`{}` changed to revision {} and is now `{}`, resolving from `{address}` to \
                         `{resolved}`",
                        created.id, entity.metadata.revision, entity.metadata.id
                    ),
                );
                report.expect(
                    "an entity keeps its type when its contents change",
                    entity.metadata.entity_type.to_string() == ENTITY_TYPE,
                    format!(
                        "created as `{ENTITY_TYPE}`, and is now `{}`",
                        entity.metadata.entity_type
                    ),
                );
                report.expect(
                    "a change advances the revision rather than the identity",
                    entity.metadata.revision > created.revision,
                    format!(
                        "the entity was at revision {} before the change and {} after it",
                        created.revision, entity.metadata.revision
                    ),
                );
            }
            (Err(error), _) | (_, Err(error)) => {
                report.aborted(
                    "an entity keeps its identity when its contents change",
                    error,
                );
            }
        },
        Err(error) => report.aborted(
            "an entity keeps its identity when its contents change",
            error.to_string(),
        ),
    }

    report
}

/// Creates one entity, reporting both the address it was created at and where it landed.
///
/// The suite needs the locator as well as the reference, which is why it issues the command itself
/// rather than going through [`Harness::create`].
fn create<B: Backend>(
    harness: &Harness,
    backend: &B,
) -> Result<(EntityLocator, VersionedEntityRef), String> {
    let entity_type = ENTITY_TYPE
        .parse::<EntityType>()
        .map_err(|error| format!("the suite's own entity type is not well formed: {error}"))?;
    let locator = harness.locator("story");
    let result = harness
        .run(
            backend,
            Command::CreateEntity(CreateEntity {
                entity_type,
                locator: locator.clone(),
                data: Node::Map(
                    [
                        ("title".to_owned(), Node::from("Indistinguishable")),
                        ("status".to_owned(), Node::from("active")),
                    ]
                    .into(),
                ),
            }),
        )
        .map_err(|error| error.to_string())?;
    let reference = result.affected.first().cloned().ok_or_else(|| {
        format!("the backend accepted a creation at `{locator}` and reported no entity")
    })?;
    Ok((locator, reference))
}

#[cfg(test)]
mod tests {
    use super::*;
    use aep_backend_memory::MemoryBackend;

    #[test]
    fn the_reference_backend_keeps_identity_apart_from_address() {
        let report = run(&MemoryBackend::new());
        assert!(report.passed(), "{report}");
    }

    #[test]
    fn the_suite_checks_more_than_one_edge_of_identity() {
        let report = run(&MemoryBackend::new());
        assert!(
            report.len() >= 4,
            "identity has several edges — uniqueness, opacity, resolution and stability — and a \
             suite that checks one of them is not a suite: {report}"
        );
    }
}
