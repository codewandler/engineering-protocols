//! The artifact half of `ess impact`: which generated artifacts a delta owes, proven on the
//! fixture pair.
//!
//! Wave 7 (W7.1). The fixture's four changes must narrow the owed artifacts to a **strict subset**
//! of what the model derives, with a path explaining each — a closure that owed everything for
//! every change would be indistinguishable from one that owed nothing, which is the same argument
//! the scenario tests already make one granularity up. And a change to the system header must owe
//! everything, because no artifact's slice can name the system itself.
//!
//! The committed-tree checks break each fail-closed arm from outside: a tampered contract digest,
//! an unreadable stamp, a missing file and an unfollowable one each arrive as *owed, stated as
//! such* — never as an absence a reader could mistake for health.

mod support;

use std::collections::BTreeMap;

use ess_diff::{
    impact, ArtifactAnswer, ArtifactId, ArtifactObligation, GeneratedTree, WholeAnswer,
};
use support::compiled;

/// The four-change report over the fixture pair, with no suite and no committed tree: the artifact
/// answer must stand on the models alone.
fn catalog_report() -> ess_diff::EssImpact {
    impact(
        &compiled("examples/revision-pair/before"),
        &compiled("examples/revision-pair/after"),
        None,
        None,
    )
    .expect("the pair is two revisions of one system")
}

/// A projection id, spelled once.
fn projection(path: &str) -> ArtifactId {
    ArtifactId::Projection {
        path: path.to_owned(),
    }
}

/// The owed map of a report that must have narrowed.
fn owed(report: &ess_diff::EssImpact) -> &BTreeMap<ArtifactId, ArtifactObligation> {
    let ArtifactAnswer::Narrowed { .. } = &report.artifacts else {
        panic!("this fixture narrows; it reported {:?}", report.artifacts);
    };
    report.artifacts.owed().expect("narrowed")
}

#[test]
fn the_four_change_delta_owes_a_strict_subset_of_the_artifacts() {
    let report = catalog_report();
    let owed = owed(&report);

    // The fixture is load-bearing only while both sides of "strict" hold: something owed,
    // something not.
    assert_eq!(report.churn.semantic_changes_total, 4);
    assert!(
        !owed.is_empty(),
        "four changes that owe no artifact would mean the slices miss everything"
    );
    assert!(
        owed.len() < report.churn.generated_artifacts_total,
        "{} of {} owed — a closure that owes everything for every change is indistinguishable \
         from one that owes nothing",
        owed.len(),
        report.churn.generated_artifacts_total
    );
    assert_eq!(report.churn.generated_artifacts_owed, owed.len());
}

#[test]
fn the_artifacts_the_currency_changes_reach_are_owed_and_named() {
    // By name, not by count: the failure mode of an impact engine is a plausible answer nobody
    // checks. `Money` holds a `Currency` field, `Headline` wraps `Money`, and every command's
    // outcome acts on the entity that holds a `Headline` — so those artifacts are owed, and the
    // constructs that never reach `Currency` are not.
    let report = catalog_report();
    let owed = owed(&report);

    for path in [
        "schema/types/catalog.pricing.Currency.schema.json",
        "schema/types/catalog.pricing.Money.schema.json",
        "schema/types/catalog.pricing.Headline.schema.json",
        "schema/commands/catalog.pricing.CreatePriceList.schema.json",
        "docs/domains/catalog.pricing.md",
        "openapi/pricing-service.yaml",
        "asyncapi/pricing-service.yaml",
    ] {
        assert!(
            owed.contains_key(&projection(path)),
            "`{path}` derives from `Currency` and must be owed; owed: {:?}",
            owed.keys().collect::<Vec<_>>()
        );
    }
}

#[test]
fn an_artifact_whose_slice_nothing_reached_is_absent_from_the_answer() {
    // The narrowing itself. `PriceListId` wraps a `String`, the events carry only a
    // `PriceListId`, and the error carries the entity's state — none of them rests on `Currency`
    // or on an actor, so none of the four changes reaches their slices.
    let report = catalog_report();
    let owed = owed(&report);

    for path in [
        "schema/types/catalog.pricing.PriceListId.schema.json",
        "schema/events/catalog.pricing.PriceListCreated.schema.json",
        "schema/events/catalog.pricing.PriceListPublished.schema.json",
        "schema/events/catalog.pricing.PriceListRetired.schema.json",
        "schema/errors/catalog.pricing.PriceListStateConflict.schema.json",
    ] {
        assert!(
            !owed.contains_key(&projection(path)),
            "nothing in `{path}`'s slice moved, and listing it anyway would drown the answer"
        );
    }
}

#[test]
fn a_grant_change_owes_the_documents_that_read_grants_and_not_the_ones_that_do_not() {
    // The asymmetry that proves the slices differ per artifact: `OpenAPI` renders who may invoke
    // what, so every actor is in its slice; `AsyncAPI` renders channels and reads no actor at
    // all. One grant change must therefore appear in the first document's reasons and not in the
    // second's.
    let report = catalog_report();
    let owed = owed(&report);
    let grant_change = "actor/catalog.pricing.Auditor/grant-removed";

    let reasons_naming = |id: &ArtifactId| -> usize {
        match owed.get(id) {
            Some(ArtifactObligation::SliceMoved { reasons }) => reasons
                .iter()
                .filter(|reason| reason.change.to_string().starts_with(grant_change))
                .count(),
            _ => 0,
        }
    };

    assert!(
        reasons_naming(&projection("openapi/pricing-service.yaml")) > 0,
        "the OpenAPI document renders grants, so the grant change must be among its reasons"
    );
    assert_eq!(
        reasons_naming(&projection("asyncapi/pricing-service.yaml")),
        0,
        "the AsyncAPI document reads no actor, so the grant change must not reach it"
    );
}

#[test]
fn an_owed_artifacts_path_explains_the_membership_hop_by_hop() {
    // Design §24 at artifact granularity: `Money` is owed by the `Currency` changes through
    // exactly one hop — its own field — and the path must say so, because the path is what a
    // reviewer checks when the answer surprises them.
    let report = catalog_report();
    let owed = owed(&report);

    let Some(ArtifactObligation::SliceMoved { reasons }) = owed.get(&projection(
        "schema/types/catalog.pricing.Money.schema.json",
    )) else {
        panic!("the Money schema is owed with reasons");
    };
    let reason = reasons
        .iter()
        .find(|reason| reason.change.to_string().contains("variant-added"))
        .expect("the CHF addition reaches Money");
    assert_eq!(
        reason.edges().len(),
        1,
        "one hop — Money's own field: {reason:?}"
    );
    assert_eq!(
        reason.edges()[0].to_string(),
        "type catalog.pricing.Money has a field of type type catalog.pricing.Currency"
    );
}

#[test]
fn whole_model_artifacts_are_owed_by_any_change_at_all() {
    // The index, the system pages, the suite and the workspace derive from the whole model, so
    // every change is in their slice at no distance. Absence here would be a survival claim about
    // documents that render everything.
    let report = catalog_report();
    let owed = owed(&report);

    for id in [
        projection("docs/README.md"),
        projection("docs/interactions.md"),
        projection("docs/crossings.md"),
        projection("docs/topology.md"),
        ArtifactId::Suite,
        ArtifactId::Workspace {
            path: "rust".to_owned(),
        },
    ] {
        let Some(ArtifactObligation::SliceMoved { reasons }) = owed.get(&id) else {
            panic!("`{id}` derives from the whole model and must be owed with reasons");
        };
        assert_eq!(
            reasons.len(),
            4,
            "one reason per change for `{id}`: {reasons:?}"
        );
    }
}

#[test]
fn a_change_to_the_system_header_owes_every_artifact() {
    // No artifact's slice can name the system itself, so there is nothing to narrow by — the same
    // mechanism-3 argument the scenario answer makes, at artifact granularity.
    let before = compiled("examples/revision-pair/before");
    let mut after = before.clone();
    after.version = ess_domain::name::Version::new(9).expect("v9");

    let report = impact(&before, &after, None, None).expect("two revisions of one system");

    let ArtifactAnswer::Whole { because } = &report.artifacts else {
        panic!(
            "a header change cannot be narrowed and must owe everything: {:?}",
            report.artifacts
        );
    };
    assert!(
        matches!(because, WholeAnswer::SystemChanged { .. }),
        "{because:?}"
    );
    assert_eq!(
        report.churn.generated_artifacts_owed,
        report.churn.generated_artifacts_total
    );
}

#[test]
fn a_model_that_moved_in_an_uncompared_family_owes_every_artifact() {
    // Mechanism 6 for artifacts. A binding's payload mapping is not compared by the delta, so no
    // change entry can name it — and although every slice's *digest* would move, no closure can
    // be seeded at a construct the delta does not know changed.
    let before = compiled("examples/billing");
    let mut after = before.clone();
    let settled = after
        .bindings
        .values_mut()
        .next()
        .expect("billing declares bindings");
    settled.mapping.clear();

    let report = impact(&before, &after, None, None).expect("one system");
    assert!(
        report.delta.is_empty(),
        "the delta has no entry for a mapping change: {:?}",
        report.delta
    );
    let ArtifactAnswer::Whole { because } = &report.artifacts else {
        panic!("an uncompared move owes everything: {:?}", report.artifacts);
    };
    assert_eq!(*because, WholeAnswer::UncomparedFamilyChanged);
}

#[test]
fn a_committed_artifact_with_a_false_contract_digest_is_owed_as_a_false_claim() {
    // The committed tree, verified. The fixture writes a real projection tree in memory, then
    // tampers with one artifact's stamped digest — the state the drift check exists for, reached
    // here through the impact door.
    let before = compiled("examples/revision-pair/before");
    let after = compiled("examples/revision-pair/after");

    let mut files: BTreeMap<String, String> = ess_gen::generate_all(&before)
        .expect("the projections generate")
        .into_iter()
        .map(|(path, artifact)| (path, artifact.contents))
        .collect();
    let target = "schema/types/catalog.pricing.PriceListId.schema.json";
    let honest = files.get(target).expect("the projection exists").clone();
    let tampered = {
        let read = ess_gen::Provenance::read_digests(&honest).expect("the stamp reads");
        honest.replace(
            &read.contract_digest,
            "0000000000000000000000000000000000000000000000000000000000000000",
        )
    };
    files.insert(target.to_owned(), tampered);

    let tree = GeneratedTree { files };
    let report = impact(&before, &after, None, Some(&tree)).expect("one system");
    let owed = owed(&report);

    // The load-bearing part: nothing in PriceListId's slice moved, so without the tamper this
    // artifact would be absent — the false claim alone is what owes it.
    let Some(ArtifactObligation::ContractMismatch {
        committed,
        expected,
    }) = owed.get(&projection(target))
    else {
        panic!(
            "a tampered contract digest is owed as a mismatch: {:?}",
            owed.get(&projection(target))
        );
    };
    assert_eq!(
        committed,
        "0000000000000000000000000000000000000000000000000000000000000000"
    );
    assert_eq!(expected.len(), 64);
}

#[test]
fn a_committed_tree_is_answered_for_fail_closed_file_by_file() {
    let before = compiled("examples/revision-pair/before");
    let after = compiled("examples/revision-pair/after");

    let mut files: BTreeMap<String, String> = ess_gen::generate_all(&before)
        .expect("the projections generate")
        .into_iter()
        .map(|(path, artifact)| (path, artifact.contents))
        .collect();
    // An artifact whose provenance cannot be read: pre-wave-7 output, or damage.
    files.insert(
        "schema/types/catalog.pricing.PriceListId.schema.json".to_owned(),
        "{}".to_owned(),
    );
    // A file the model derives nothing at.
    files.insert(
        "README.md".to_owned(),
        "# an index someone committed".to_owned(),
    );
    // A projection the model derives and the tree lacks.
    files.remove("schema/errors/catalog.pricing.PriceListStateConflict.schema.json");

    let tree = GeneratedTree { files };
    let report = impact(&before, &after, None, Some(&tree)).expect("one system");
    let owed = owed(&report);

    assert_eq!(
        owed.get(&projection(
            "schema/types/catalog.pricing.PriceListId.schema.json"
        )),
        Some(&ArtifactObligation::ProvenanceUnreadable),
        "an unreadable claim is owed, stated as such"
    );
    assert_eq!(
        owed.get(&projection("README.md")),
        Some(&ArtifactObligation::Unfollowed),
        "a file the analysis cannot follow is owed, never silently skipped"
    );
    assert_eq!(
        owed.get(&projection(
            "schema/errors/catalog.pricing.PriceListStateConflict.schema.json"
        )),
        Some(&ArtifactObligation::Missing),
        "a derived artifact the tree lacks is owed its own creation"
    );
}

#[test]
fn the_artifact_answer_is_byte_identical_between_runs() {
    // Invariant 9 for the new document section: the report is committed and quoted, so two runs
    // over one pair must be one sequence of bytes.
    let first = catalog_report().to_canonical_json();
    let second = catalog_report().to_canonical_json();
    assert_eq!(first, second);
    assert!(
        first.contains("\"artifact\""),
        "the document carries the artifact section"
    );
}
