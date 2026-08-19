//! Filters that are applied, rather than accepted and ignored.
//!
//! A query filter is a promise about what is *not* in the answer. "All approved designs for this
//! epic" is the question a release gate asks, and a backend that returns everything and lets the
//! caller sort it out has not answered it — it has moved the decision to a client that will
//! reimplement the filter slightly differently, and the release gate will pass on a design nobody
//! approved.
//!
//! The failure mode is quiet, which is why it needs a suite: a query that ignores its filters still
//! returns results, still has the right shape, and looks correct until someone counts. That is
//! [`crate::faulty::Fault::IgnoreQueryFilters`].
//!
//! Every check here is phrased so that other suites' entities cannot affect it. The store is shared
//! across a conformance run, so "returns exactly three" would be a statement about run order; what
//! is checked instead is that everything returned satisfies the filter, and that nothing which
//! satisfies it is missing.

use aep_contract::query::{EntityEnvelope, EntityQuery, Page};
use aep_contract::testing::block_on;
use aep_domain::artifact::RelationKind;
use aep_domain::command::{Command, CreateRelation};
use aep_domain::entity::{EntityType, VersionedEntityRef};
use aep_domain::node::Node;

use crate::harness::{Backend, Harness};
use crate::report::SuiteReport;

/// The body key that marks an entity as belonging to this suite.
///
/// A conformance run shares one backend, so a filter that does not narrow to this suite's own
/// entities would be answered partly by whatever the suites before it created.
const MARKER: &str = "conformance_query_subject";
/// The type two of the three subjects are created with.
const DESIGN: &str = "aep.design/v1";
/// The type the third is created with, so a type filter has something to leave out.
const STORY: &str = "aep.story/v1";

/// Runs the query suite.
// Eight filters over one fixture. Splitting them would mean creating the fixture three times, and
// the checks are only meaningful against the same set of entities.
#[allow(clippy::too_many_lines)]
pub fn run<B: Backend>(backend: &B) -> SuiteReport {
    let harness = Harness::new("query");
    let mut report = SuiteReport::new("query");

    let approved_design = match subject(&harness, backend, DESIGN, "design", "approved") {
        Ok(reference) => reference,
        Err(error) => {
            report.aborted("entities can be created to query for", error);
            return report;
        }
    };
    let draft_design = match subject(&harness, backend, DESIGN, "design", "in_review") {
        Ok(reference) => reference,
        Err(error) => {
            report.aborted("entities can be created to query for", error);
            return report;
        }
    };
    let approved_story = match subject(&harness, backend, STORY, "story", "approved") {
        Ok(reference) => reference,
        Err(error) => {
            report.aborted("entities can be created to query for", error);
            return report;
        }
    };

    let Ok(design_type) = DESIGN.parse::<EntityType>() else {
        report.aborted(
            "a type filter returns only entities of that type",
            format!("the suite's own entity type `{DESIGN}` is not well formed"),
        );
        return report;
    };

    match find(backend, &EntityQuery::of_type(design_type.clone())) {
        Ok(page) => {
            let wrong: Vec<String> = page
                .items
                .iter()
                .filter(|entity| entity.metadata.entity_type != design_type)
                .map(|entity| {
                    format!(
                        "{} is a {}",
                        entity.metadata.id, entity.metadata.entity_type
                    )
                })
                .collect();
            report.expect(
                "a type filter returns only entities of that type",
                wrong.is_empty(),
                format!(
                    "a query for `{DESIGN}` returned {} entities of other types: {}",
                    wrong.len(),
                    wrong.join(", ")
                ),
            );
            report.expect(
                "a type filter returns the entities that have that type",
                holds(&page, &approved_design) && holds(&page, &draft_design),
                format!(
                    "a query for `{DESIGN}` returned {} entities, and left out one of the two this \
                     suite created",
                    page.len()
                ),
            );
        }
        Err(error) => {
            report.aborted("a type filter returns only entities of that type", error);
        }
    }

    let approved = EntityQuery::default()
        .matching(MARKER, Node::from(MARKER))
        .matching("status", Node::from("approved"));
    match find(backend, &approved) {
        Ok(page) => {
            let wrong: Vec<String> = page
                .items
                .iter()
                .filter(|entity| !matches_body(entity, "approved"))
                .map(|entity| entity.metadata.id.to_string())
                .collect();
            report.expect(
                "a body filter returns only entities whose body matches every clause",
                wrong.is_empty(),
                format!(
                    "a query for status `approved` returned {} entities that do not have it: {}",
                    wrong.len(),
                    wrong.join(", ")
                ),
            );
            report.expect(
                "a body filter keeps the entities that do match",
                holds(&page, &approved_design) && holds(&page, &approved_story),
                format!(
                    "a query for status `approved` returned {} entities and left out one of the \
                     two approved ones this suite created",
                    page.len()
                ),
            );
        }
        Err(error) => report.aborted(
            "a body filter returns only entities whose body matches every clause",
            error,
        ),
    }

    // A filter nothing satisfies. Returning everything here is the same bug as returning everything
    // for a filter that does narrow, but it is the version a caller notices last.
    let unsatisfiable = EntityQuery::default()
        .matching(MARKER, Node::from(MARKER))
        .matching(
            "status",
            Node::from("no-entity-in-this-run-has-this-status"),
        );
    match find(backend, &unsatisfiable) {
        Ok(page) => report.expect(
            "a filter that nothing satisfies returns an empty page rather than everything",
            page.is_empty(),
            format!(
                "a query for a status no entity holds returned {} entities",
                page.len()
            ),
        ),
        Err(error) => report.aborted(
            "a filter that nothing satisfies returns an empty page rather than everything",
            error,
        ),
    }

    let mut truncated = EntityQuery::default().matching(MARKER, Node::from(MARKER));
    truncated.limit = Some(2);
    match find(backend, &truncated) {
        Ok(page) => report.expect(
            "a limit truncates the page and says where to continue from",
            page.len() == 2 && page.has_more(),
            format!(
                "this suite created three entities; a limit of 2 returned {} of them and {} a \
                 continuation",
                page.len(),
                if page.has_more() {
                    "reported"
                } else {
                    "reported no"
                }
            ),
        ),
        Err(error) => report.aborted(
            "a limit truncates the page and says where to continue from",
            error,
        ),
    }

    let mut complete = EntityQuery::default().matching(MARKER, Node::from(MARKER));
    complete.limit = Some(64);
    match find(backend, &complete) {
        Ok(page) => report.expect(
            "a page that holds the whole answer reports nothing to continue from",
            !page.has_more(),
            format!(
                "a limit of 64 returned {} entities and still reported a continuation, which would \
                 send a caller round a loop that never ends",
                page.len()
            ),
        ),
        Err(error) => report.aborted(
            "a page that holds the whole answer reports nothing to continue from",
            error,
        ),
    }

    // Relation traversal is a filter too, and the same fault strips it.
    let related = harness.run(
        backend,
        Command::CreateRelation(CreateRelation {
            kind: RelationKind::Designs,
            source: approved_design.unversioned(),
            target: approved_story.unversioned(),
        }),
    );
    match related {
        Ok(_) => {
            let traversal = EntityQuery::default()
                .related_to(approved_design.unversioned(), RelationKind::Designs);
            match find(backend, &traversal) {
                Ok(page) => report.expect(
                    "a relation filter returns the entities at the far end of that relation, and \
                     no others",
                    page.len() == 1 && holds(&page, &approved_story),
                    format!(
                        "one entity is `designs`-related to the design, and the traversal returned \
                         {}",
                        page.len()
                    ),
                ),
                Err(error) => report.aborted(
                    "a relation filter returns the entities at the far end of that relation, and \
                     no others",
                    error,
                ),
            }
        }
        Err(error) => report.aborted(
            "a relation filter returns the entities at the far end of that relation, and no others",
            error.to_string(),
        ),
    }

    report
}

/// Creates one entity of this suite's fixture, marked as belonging to it.
fn subject<B: Backend>(
    harness: &Harness,
    backend: &B,
    entity_type: &str,
    kind: &str,
    status: &str,
) -> Result<VersionedEntityRef, String> {
    harness
        .create(
            backend,
            entity_type,
            kind,
            &[
                ("title", Node::from("A subject to query for")),
                ("status", Node::from(status)),
                (MARKER, Node::from(MARKER)),
            ],
        )
        .map_err(|error| error.to_string())
}

/// Runs a query, reporting the backend's own explanation when it will not answer.
fn find<B: Backend>(backend: &B, query: &EntityQuery) -> Result<Page<EntityEnvelope>, String> {
    block_on(backend.query(query)).map_err(|error| error.to_string())
}

/// `true` when a page holds `entity`.
fn holds(page: &Page<EntityEnvelope>, entity: &VersionedEntityRef) -> bool {
    page.items.iter().any(|item| item.metadata.id == entity.id)
}

/// `true` when an entity's body carries this suite's marker and `status`.
fn matches_body(entity: &EntityEnvelope, status: &str) -> bool {
    let Some(fields) = entity.data.as_map() else {
        return false;
    };
    fields.get(MARKER) == Some(&Node::from(MARKER))
        && fields.get("status") == Some(&Node::from(status))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::faulty::{Fault, FaultyBackend};
    use aep_backend_memory::MemoryBackend;

    #[test]
    fn the_reference_backend_applies_the_filters_it_is_given() {
        let report = run(&MemoryBackend::new());
        assert!(report.passed(), "{report}");
    }

    #[test]
    fn a_backend_that_ignores_its_filters_does_not_pass() {
        let report = run(&FaultyBackend::new(
            MemoryBackend::new(),
            Fault::IgnoreQueryFilters,
        ));
        assert!(
            !report.passed(),
            "a filter that is accepted and ignored answers a question nobody asked, and the caller \
             cannot tell: {report}"
        );
    }
}
