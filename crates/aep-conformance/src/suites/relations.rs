//! Edges, and the fact that both ends of one must be answerable.
//!
//! The graph is where an engineering record stops being a pile of documents. "Which review covers
//! this design?", "what supersedes this decision?", "what does this story depend on?" are all one
//! question asked from different ends of the same edge, and a backend that can only answer it in the
//! direction it happened to index has not implemented relations — it has implemented a foreign key.
//!
//! [`crate::faulty::Fault::DropRelations`] is the blunt version of getting this wrong: edges are
//! accepted and never returned. It looks like an empty graph rather than an error, so every
//! traversal answers "nothing related", every gate that asks for supporting evidence finds none, and
//! nothing anywhere reports a failure.
//!
//! Two edges of the property are less obvious. An edge to an entity that does not exist must be
//! refused at creation — a dangling edge is a lie that only shows up when someone follows it. And a
//! removed edge must stop being returned: relations are the one thing in AEP that is genuinely
//! removed, because an edge asserted in error carries no history worth keeping, while the entities
//! it joined keep theirs.

use aep_contract::query::{Page, Relation, RelationQuery};
use aep_contract::testing::block_on;
use aep_domain::artifact::RelationKind;
use aep_domain::command::{Command, CreateRelation, RemoveRelation};
use aep_domain::entity::{EntityId, EntityRef, VersionedEntityRef};
use aep_domain::ids::RelationId;
use aep_domain::node::Node;

use crate::harness::{Backend, Harness};
use crate::report::{Check, SuiteReport};

/// An identity that is well formed and was never created, for asking about a dangling edge.
const ABSENT_ENTITY: &str = "aep-conformance-absent-relation-target";
/// The property a removal establishes, named once because two arms report it.
const REMOVAL_TAKES_EFFECT: &str = "a removed relation stops being returned";

/// Runs the relations suite.
// One small graph, asked about from both ends and then unpicked. The checks only mean anything
// against the same three entities, so they stay together.
#[allow(clippy::too_many_lines)]
pub fn run<B: Backend>(backend: &B) -> SuiteReport {
    let harness = Harness::new("relations");
    let mut report = SuiteReport::new("relations");

    let (design, specification, story) = match fixture(&harness, backend) {
        Ok(fixture) => fixture,
        Err(error) => {
            report.aborted("entities can be created to relate", error);
            return report;
        }
    };

    if let Err(error) = relate(
        &harness,
        backend,
        RelationKind::Designs,
        &design,
        &specification,
    ) {
        report.aborted("a relation can be created", error);
        return report;
    }
    if let Err(error) = relate(&harness, backend, RelationKind::DependsOn, &design, &story) {
        report.aborted("a second relation can be created", error);
        return report;
    }

    let designs = |relation: &Relation| {
        relation.kind == RelationKind::Designs
            && relation.source.id == design.id
            && relation.target.id == specification.id
    };

    let mut edge: Option<RelationId> = None;
    match edges(backend, &RelationQuery::from(design.unversioned())) {
        Ok(page) => {
            edge = page
                .items
                .iter()
                .find(|item| designs(item))
                .map(|item| item.id.clone());
            report.expect(
                "a created relation is returned when relations are asked from its source",
                edge.is_some(),
                format!(
                    "`{} designs {}` was created, and asking from the source returned {} \
                     relations, none of them that one",
                    design.id,
                    specification.id,
                    page.len()
                ),
            );
        }
        Err(error) => report.aborted(
            "a created relation is returned when relations are asked from its source",
            error,
        ),
    }

    // The inverse question. A design's neighbourhood is discoverable from the design; what a
    // specification is designed by is only discoverable from the specification.
    match edges(backend, &RelationQuery::to(specification.unversioned())) {
        Ok(page) => report.expect(
            "the same relation is returned when relations are asked to its target",
            page.items.iter().any(designs),
            format!(
                "`{} designs {}` was created, and asking what points at the target returned {} \
                 relations, none of them that one",
                design.id,
                specification.id,
                page.len()
            ),
        ),
        Err(error) => report.aborted(
            "the same relation is returned when relations are asked to its target",
            error,
        ),
    }

    match edges(
        backend,
        &RelationQuery::from(design.unversioned()).of_kind(RelationKind::Designs),
    ) {
        Ok(page) => {
            let other: Vec<String> = page
                .items
                .iter()
                .filter(|item| item.kind != RelationKind::Designs)
                .map(|item| item.kind.as_str().to_owned())
                .collect();
            report.expect(
                "a kind filter returns only relations of that kind",
                other.is_empty() && page.items.iter().any(designs),
                format!(
                    "the source has a `designs` edge and a `depends_on` edge; filtering for \
                     `designs` returned {} relations, including {:?}",
                    page.len(),
                    other
                ),
            );
        }
        Err(error) => report.aborted("a kind filter returns only relations of that kind", error),
    }

    let Ok(absent) = EntityId::new(ABSENT_ENTITY) else {
        report.aborted(
            "a relation to an entity that does not exist is refused",
            format!("the suite's own placeholder identity `{ABSENT_ENTITY}` is not well formed"),
        );
        return report;
    };
    match harness.run(
        backend,
        Command::CreateRelation(CreateRelation {
            kind: RelationKind::DependsOn,
            source: design.unversioned(),
            target: EntityRef::new(absent),
        }),
    ) {
        Ok(_) => report.expect(
            "a relation to an entity that does not exist is refused",
            false,
            format!(
                "the backend recorded an edge to `{ABSENT_ENTITY}`, which was never created; the \
                 lie only surfaces when someone follows it"
            ),
        ),
        Err(error) => report.expect(
            "a relation to an entity that does not exist is refused",
            error.code() == "not_found",
            format!(
                "the backend refused with `{}` rather than `not_found`: {error}",
                error.code()
            ),
        ),
    }

    let Some(edge) = edge else {
        report.aborted(
            REMOVAL_TAKES_EFFECT,
            "the relation to remove was never returned, so its removal cannot be observed"
                .to_owned(),
        );
        return report;
    };
    match harness.run(
        backend,
        Command::RemoveRelation(RemoveRelation {
            relation: edge.clone(),
        }),
    ) {
        Ok(_) => match edges(backend, &RelationQuery::from(design.unversioned())) {
            Ok(page) => report.expect(
                REMOVAL_TAKES_EFFECT,
                !page.items.iter().any(designs)
                    && page
                        .items
                        .iter()
                        .any(|item| item.kind == RelationKind::DependsOn),
                format!(
                    "`{edge}` was removed, and asking from the source still returns {} relations",
                    page.len()
                ),
            ),
            Err(error) => report.aborted(REMOVAL_TAKES_EFFECT, error),
        },
        Err(error) => report.record(Check::failed(
            REMOVAL_TAKES_EFFECT,
            format!(
                "removing `{edge}` was refused with `{}`: {error}",
                error.code()
            ),
        )),
    }

    report
}

/// The three entities this suite draws edges between.
fn fixture<B: Backend>(
    harness: &Harness,
    backend: &B,
) -> Result<(VersionedEntityRef, VersionedEntityRef, VersionedEntityRef), String> {
    let design = entity(harness, backend, "aep.design/v1", "design")?;
    let specification = entity(harness, backend, "aep.specification/v1", "specification")?;
    let story = entity(harness, backend, "aep.story/v1", "story")?;
    Ok((design, specification, story))
}

/// Creates one entity for the fixture.
fn entity<B: Backend>(
    harness: &Harness,
    backend: &B,
    entity_type: &str,
    kind: &str,
) -> Result<VersionedEntityRef, String> {
    harness
        .create(
            backend,
            entity_type,
            kind,
            &[
                ("title", Node::from("An end of an edge")),
                ("status", Node::from("active")),
            ],
        )
        .map_err(|error| error.to_string())
}

/// Records one edge.
fn relate<B: Backend>(
    harness: &Harness,
    backend: &B,
    kind: RelationKind,
    source: &VersionedEntityRef,
    target: &VersionedEntityRef,
) -> Result<(), String> {
    harness
        .run(
            backend,
            Command::CreateRelation(CreateRelation {
                kind,
                source: source.unversioned(),
                target: target.unversioned(),
            }),
        )
        .map(|_| ())
        .map_err(|error| error.to_string())
}

/// Asks for relations, reporting the backend's own explanation when it will not answer.
fn edges<B: Backend>(backend: &B, query: &RelationQuery) -> Result<Page<Relation>, String> {
    block_on(backend.relations(query)).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::faulty::{Fault, FaultyBackend};
    use aep_backend_memory::MemoryBackend;

    #[test]
    fn the_reference_backend_answers_an_edge_from_both_ends() {
        let report = run(&MemoryBackend::new());
        assert!(report.passed(), "{report}");
    }

    #[test]
    fn a_backend_that_returns_no_edges_does_not_pass() {
        let report = run(&FaultyBackend::new(
            MemoryBackend::new(),
            Fault::DropRelations,
        ));
        assert!(
            !report.passed(),
            "an empty graph is indistinguishable from an unrelated one, and every traversal quietly \
             answers `nothing`: {report}"
        );
    }
}
