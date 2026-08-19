//! Read-your-writes, without a sleep anywhere.
//!
//! A conformance suite that waits 200 milliseconds and hopes is testing the machine it runs on. The
//! contract removes the need: every accepted mutation returns an opaque token, and a read may demand
//! a view no older than that token. An immediately consistent backend satisfies the demand for free;
//! one with a projection behind it blocks until the projection catches up. Neither has to say which
//! it is, and the caller writes the same code either way.
//!
//! The demand only means something if it can fail. A backend that treats `AtLeast(token)` as a
//! suggestion — answering from whatever is at hand — passes every test that asks it for data it
//! already has, and loses exactly the case the token exists for: the read that arrives before the
//! projection. That is [`crate::faulty::Fault::AnswerStaleReads`], and the check that catches it is
//! the one that hands the backend a token it never issued and requires it to say no.
//!
//! Both surfaces are checked. A backend that honours freshness on `get` and quietly drops it on
//! `query` has a hole exactly where the expensive questions are asked.

use aep_contract::consistency::{ConsistencyToken, QueryConsistency};
use aep_contract::query::EntityQuery;
use aep_contract::testing::block_on;
use aep_domain::command::{Command, UpdateEntity};
use aep_domain::node::Node;

use crate::harness::{Backend, Harness};
use crate::report::{Check, SuiteReport};

/// The title the write under test sets, and the value the query looks for.
const WRITTEN: &str = "Written, and demanded back";
/// A token no backend can have issued, because it names the suite that made it up.
const NEVER_ISSUED: &str = "aep-conformance-token-no-backend-ever-issued";
/// The property that makes the demand a demand, asked of a single read.
const READ_REFUSES: &str =
    "a read demanding a token the backend never issued is refused rather than answered";
/// The same property, asked of the surface where the expensive questions are asked.
const QUERY_REFUSES: &str =
    "a query demanding a token the backend never issued is refused rather than answered";

/// Runs the consistency suite.
// One write, then four demands made of it across both read surfaces. They share the token, so they
// share a function.
#[allow(clippy::too_many_lines)]
pub fn run<B: Backend>(backend: &B) -> SuiteReport {
    let harness = Harness::new("consistency");
    let mut report = SuiteReport::new("consistency");

    let design = match harness.create_design(backend) {
        Ok(design) => design,
        Err(error) => {
            report.aborted("an entity can be created to write to", error.to_string());
            return report;
        }
    };
    let target = design.unversioned();

    let written = match harness.run(
        backend,
        Command::UpdateEntity(UpdateEntity {
            target: target.clone(),
            changes: [("title".to_owned(), Node::from(WRITTEN))].into(),
        }),
    ) {
        Ok(result) => result,
        Err(error) => {
            report.aborted(
                "a write returns a token to demand it back",
                error.to_string(),
            );
            return report;
        }
    };
    let token = written.consistency.clone();

    match block_on(backend.get(&target, QueryConsistency::at_least(token.clone()))) {
        Ok(entity) => {
            let title = entity
                .data
                .as_map()
                .and_then(|fields| fields.get("title"))
                .and_then(Node::as_text)
                .unwrap_or_default();
            report.expect(
                "a read demanding the token a write returned sees that write",
                title == WRITTEN,
                format!(
                    "the write set the title to {WRITTEN:?} and returned the token `{token}`; a \
                     read demanding that token saw {title:?}"
                ),
            );
        }
        Err(error) => report.expect(
            "a read demanding the token a write returned sees that write",
            false,
            format!(
                "the backend issued the token `{token}` for its own write and then could not reach \
                 it: {error}"
            ),
        ),
    }

    report.expect(
        "a read demanding nothing is always answered",
        block_on(backend.get(&target, QueryConsistency::Current)).is_ok(),
        "a read with no freshness demand was refused, though there is nothing for it to wait for"
            .to_owned(),
    );

    let Ok(foreign) = ConsistencyToken::new(NEVER_ISSUED) else {
        report.aborted(
            READ_REFUSES,
            format!("the suite's own placeholder token `{NEVER_ISSUED}` is not well formed"),
        );
        return report;
    };

    match block_on(backend.get(&target, QueryConsistency::at_least(foreign.clone()))) {
        Ok(entity) => report.record(Check::failed(
            READ_REFUSES,
            format!(
                "the backend answered a read demanding `{foreign}`, which it never issued, with \
                 revision {} — so a caller cannot tell a satisfied demand from an ignored one",
                entity.metadata.revision
            ),
        )),
        Err(_) => report.record(Check::passed(READ_REFUSES)),
    }

    let demanded = EntityQuery::default()
        .matching("title", Node::from(WRITTEN))
        .with_consistency(QueryConsistency::at_least(token.clone()));
    match block_on(backend.query(&demanded)) {
        Ok(page) => report.expect(
            "a query demanding the token a write returned sees that write",
            page.items
                .iter()
                .any(|entity| entity.metadata.id == target.id),
            format!(
                "the write is not among the {} entities a query demanding `{token}` returned",
                page.len()
            ),
        ),
        Err(error) => report.expect(
            "a query demanding the token a write returned sees that write",
            false,
            format!("the query could not reach the backend's own token `{token}`: {error}"),
        ),
    }

    let unreachable =
        EntityQuery::default().with_consistency(QueryConsistency::at_least(foreign.clone()));
    match block_on(backend.query(&unreachable)) {
        Ok(page) => report.record(Check::failed(
            QUERY_REFUSES,
            format!(
                "the backend answered a query demanding `{foreign}`, which it never issued, with \
                 {} entities",
                page.len()
            ),
        )),
        Err(_) => report.record(Check::passed(QUERY_REFUSES)),
    }

    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::faulty::{Fault, FaultyBackend};
    use aep_backend_memory::MemoryBackend;

    #[test]
    fn the_reference_backend_answers_a_freshness_demand_it_can_satisfy() {
        let report = run(&MemoryBackend::new());
        assert!(report.passed(), "{report}");
    }

    #[test]
    fn a_backend_that_ignores_a_freshness_demand_does_not_pass() {
        let report = run(&FaultyBackend::new(
            MemoryBackend::new(),
            Fault::AnswerStaleReads,
        ));
        assert!(
            !report.passed(),
            "a demand that is never refused is not a demand, and the read it exists for is the one \
             that arrives before the projection: {report}"
        );
    }
}
