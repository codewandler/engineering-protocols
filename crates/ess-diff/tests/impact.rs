//! The closure from a delta to what a committed suite owes again.
//!
//! The fixture pair under `examples/revision-pair/` differs by exactly four changes, and its
//! specification obliges nine scenarios — so this file can assert *which* scenarios each of the four
//! puts back to owed, rather than that some number of them did. That distinction is the whole reason
//! the fixture exists: an impact engine's failure mode is a plausible answer nobody checks.
//!
//! It also breaks the four fail-closed mechanisms that can be broken from outside the crate and
//! watches each one fail: a suite from the wrong revision, a suite from another system, a scenario
//! resting on a construct no graph has a node for, and a model that moved in a family the delta
//! does not compare.

mod support;

use std::collections::BTreeSet;

use ess_conformance::scenario::{
    ConformanceScenario, ConformanceSuite, EssSemanticRef, ScenarioId, ScenarioPurpose, ViewRef,
};
use ess_conformance::synthesize;
use ess_diff::graph::ImpactClass;
use ess_diff::{impact, EssImpact, ImpactRefusal, Invalidation};
use ess_domain::name::QualifiedName;
use support::compiled;

/// The suite the `before` half of the fixture pair obliges.
///
/// Synthesised in-process rather than read from a file, and that is not only convenience: the suite
/// a narrowing is checked against has to be the one the `--from` revision produced, and deriving it
/// here makes that true by construction instead of by a path in a test.
fn catalog_suite() -> ConformanceSuite {
    synthesize(&compiled("examples/revision-pair/before")).suite
}

/// The impact of the fixture pair's four changes on the suite the earlier revision obliges.
fn catalog_impact() -> EssImpact {
    impact(
        &compiled("examples/revision-pair/before"),
        &compiled("examples/revision-pair/after"),
        &catalog_suite(),
    )
    .expect("the pair is two revisions of one system and the suite is the earlier one's")
}

/// Every scenario one change puts back to owed.
fn owed_by(report: &EssImpact, change: &str) -> BTreeSet<String> {
    let Invalidation::Narrowed { scenarios } = &report.invalidation else {
        panic!(
            "this fixture narrows; it reported {:?}",
            report.invalidation
        );
    };
    scenarios
        .iter()
        .filter(|(_, impact)| {
            impact
                .reasons()
                .iter()
                .any(|reason| reason.change.to_string() == change)
        })
        .map(|(id, _)| id.to_string())
        .collect()
}

#[test]
fn the_suite_the_fixture_obliges_is_nine_scenarios_and_the_delta_is_four_changes() {
    // Asserted before anything is measured against it. Every count below is a fraction of these two
    // numbers, so a fixture that quietly stopped obliging scenarios would make every other test in
    // this file pass by reporting nothing.
    let report = catalog_impact();

    assert_eq!(report.churn.conformance_scenarios_total, 9);
    assert_eq!(report.churn.semantic_changes_total, 4);
    assert_eq!(report.churn.actor_grants_changed, 2);
}

#[test]
fn taking_a_grant_from_an_actor_owes_only_the_scenarios_that_act_as_that_actor() {
    // The narrowing that makes the wave worth building. Nothing in a specification depends on an
    // actor, so a closure from one reaches exactly that actor — and the scenarios owed again are the
    // ones whose own dependency set names it.
    let report = catalog_impact();
    let owed = owed_by(
        &report,
        "actor/catalog.pricing.Auditor/grant-removed/catalog.pricing.RetirePriceList",
    );

    assert_eq!(
        owed.len(),
        4,
        "four of the nine scenarios act as the auditor: {owed:?}"
    );
    assert!(owed.contains("catalog.pricing.RetirePriceList/outcome/retired"));
    assert!(
        !owed.contains("catalog.pricing.PublishPriceList/outcome/published"),
        "publishing is the pricing manager's, and this change is about the auditor: {owed:?}"
    );

    let suite = catalog_suite();
    for id in &owed {
        let scenario = suite
            .scenario(&ScenarioId::parse(id).expect("a scenario id"))
            .expect("the suite holds it");
        assert!(
            scenario
                .source
                .iter()
                .any(|construct| { construct.to_string() == "actor catalog.pricing.Auditor" }),
            "`{id}` is owed again, so it has to be one that rests on the actor that moved"
        );
    }
}

#[test]
fn a_variant_removed_from_an_enum_reaches_the_entity_that_holds_it_three_declarations_away() {
    // Design §24's requirement, on the fixture: `Currency` is not named by the entity, by the
    // command, or by seven of the nine scenarios. It is reached through `Money` and `Headline`, and
    // the path is what makes the answer checkable rather than assertable.
    let report = catalog_impact();
    let change = "type/catalog.pricing.Currency/variant-removed/GBP";
    let owed = owed_by(&report, change);

    assert_eq!(
        owed.len(),
        9,
        "every scenario in this fixture creates or moves a price list, and a price list holds a \
         headline priced in a currency: {owed:?}"
    );

    let Invalidation::Narrowed { scenarios } = &report.invalidation else {
        panic!("this fixture narrows");
    };
    let publish = scenarios
        .get(
            &ScenarioId::parse("catalog.pricing.PublishPriceList/outcome/published")
                .expect("an id"),
        )
        .expect("the scenario is owed again");
    let reason = publish
        .reasons()
        .iter()
        .find(|reason| reason.change.to_string() == change)
        .expect("the variant removal is one of the reasons");

    assert_eq!(reason.class(), ImpactClass::TransitivelyImpacted);
    let hops: Vec<String> = reason.edges().iter().map(ToString::to_string).collect();
    assert_eq!(
        hops,
        vec![
            "type catalog.pricing.Money has a field of type type catalog.pricing.Currency"
                .to_owned(),
            "type catalog.pricing.Headline wraps type catalog.pricing.Money".to_owned(),
            "entity catalog.pricing.PriceList has a field of type type catalog.pricing.Headline"
                .to_owned(),
        ],
        "the answer has to be readable as an argument, not as a verdict"
    );
}

#[test]
fn every_scenario_resting_directly_on_a_changed_construct_is_owed_again() {
    // The floor a narrowing may never go under. Computed here from the suite and the delta without
    // consulting the closure at all, so it fails if the closure ever misses the easiest case there
    // is: the scenario names the changed construct itself.
    let report = catalog_impact();
    let suite = catalog_suite();
    let owed: BTreeSet<String> = report
        .invalidation
        .owed(&suite)
        .into_iter()
        .map(ToString::to_string)
        .collect();

    let mut floor: BTreeSet<String> = BTreeSet::new();
    for change in report.delta.changes() {
        let Some(subject) = change.subject() else {
            continue;
        };
        for (id, scenario) in &suite.scenarios {
            if scenario.depends_on(&subject) {
                floor.insert(id.to_string());
            }
        }
    }

    assert!(
        !floor.is_empty(),
        "the fixture must contain at least one scenario that names a changed construct outright, \
         or this test passes whether the rule holds or not"
    );
    assert!(
        floor.is_subset(&owed),
        "these rest directly on something that moved and were not owed again: {:?}",
        floor.difference(&owed).collect::<Vec<_>>()
    );
}

#[test]
fn analysing_the_same_pair_twice_produces_byte_identical_json() {
    // Four independent compilations, two independent syntheses, two independent closures. An
    // unordered map or an address-dependent walk anywhere in the graph, the breadth-first search or
    // the report would show up here as a diff.
    let first = catalog_impact().to_canonical_json();
    let second = catalog_impact().to_canonical_json();

    assert_eq!(
        first.as_bytes(),
        second.as_bytes(),
        "the same delta and the same suite must be the same bytes"
    );
    assert!(first.ends_with('\n'), "one trailing newline");
    assert!(
        first.len() > 1000,
        "the report is not empty: {}",
        first.len()
    );
}

#[test]
fn a_suite_produced_from_the_later_revision_is_refused_rather_than_narrowed() {
    // The state the rule is load-bearing in: this suite is a real suite for a real revision of the
    // same system, so a narrowing against it would run, terminate and produce a short, plausible,
    // meaningless list. What makes it wrong is that the results it stands for were never produced
    // against it.
    let before = compiled("examples/revision-pair/before");
    let after = compiled("examples/revision-pair/after");
    let later = synthesize(&after).suite;
    assert_eq!(later.len(), 9, "the later suite is a real suite");

    let refusal = impact(&before, &after, &later).expect_err("the wrong suite is refused");

    assert!(
        matches!(refusal, ImpactRefusal::SuiteFromAnotherRevision { .. }),
        "{refusal:?}"
    );
    assert!(
        refusal.to_string().contains("`--to`"),
        "the refusal names the mistake rather than the mismatch: {refusal}"
    );
}

#[test]
fn a_suite_for_another_system_is_refused() {
    let before = compiled("examples/revision-pair/before");
    let after = compiled("examples/revision-pair/after");
    let elsewhere = synthesize(&compiled("examples/billing")).suite;
    assert!(!elsewhere.is_empty(), "billing obliges scenarios");

    let refusal =
        impact(&before, &after, &elsewhere).expect_err("another system's suite is refused");

    assert!(
        matches!(refusal, ImpactRefusal::SuiteFromAnotherSystem { .. }),
        "{refusal:?}"
    );
    assert!(
        refusal.to_string().contains("billing") && refusal.to_string().contains("catalog"),
        "the refusal names both systems: {refusal}"
    );
}

#[test]
fn two_specifications_of_different_systems_are_refused_here_too() {
    // The rule `diff` refuses on, at this door as well. `impact` runs the comparison itself, so the
    // refusal has to travel rather than be restated — one rule, one spelling.
    let refusal = impact(
        &compiled("examples/billing"),
        &compiled("examples/revision-pair/before"),
        &catalog_suite(),
    )
    .expect_err("two systems are refused");

    assert!(matches!(refusal, ImpactRefusal::Pair { .. }), "{refusal:?}");
}

#[test]
fn a_suite_resting_on_a_construct_no_graph_has_a_node_for_owes_the_whole_suite() {
    // Fail-closed mechanism 4, reached from outside the crate. The suite is otherwise the right
    // suite — same system, same digest — and one scenario is added that depends on a view neither
    // revision declares. A closure can never reach it, so narrowing would leave it out of every
    // answer while looking exactly like a correct narrowing.
    let before = compiled("examples/revision-pair/before");
    let after = compiled("examples/revision-pair/after");
    let mut suite = catalog_suite();
    let total = suite.len();
    suite
        .insert(
            // A well-formed id the suite does not already hold: publishing from `Draft` is a legal
            // move, so no refusal scenario exists for it.
            ScenarioId::parse(
                "catalog.pricing.PriceList/state/Draft/refuses/catalog.pricing.PublishPriceList",
            )
            .expect("a scenario id"),
            ConformanceScenario::new(
                ScenarioPurpose::new("rests on a construct the model does not declare")
                    .expect("a purpose"),
                Vec::new(),
                [EssSemanticRef::from(ViewRef::new(
                    QualifiedName::new("catalog.pricing.PriceListSummary").expect("a name"),
                ))],
            ),
        )
        .expect("the id is new");
    assert_eq!(suite.len(), total + 1, "the fixture reached the state");

    let report = impact(&before, &after, &suite).expect("the suite is still the earlier one's");

    assert!(
        report.invalidation.is_whole(),
        "a suite the graph cannot see all of is owed whole: {:?}",
        report.invalidation
    );
    assert_eq!(
        report.churn.conformance_scenarios_invalidated,
        suite.len(),
        "every scenario, not the ones a partial walk happened to reach"
    );
}

#[test]
fn a_change_in_a_family_the_delta_does_not_compare_owes_the_whole_suite() {
    // Fail-closed mechanism 6, reached with the construct that motivated it: an outcome's
    // `payload:` mapping lives in the command family, which wave 5 deliberately does not compare.
    // Erasing one produces two different models and an **empty** delta — no change entry has a
    // vocabulary for it — and an empty delta narrowing to nothing would be a survival claim about
    // a model that moved. So it is not narrowed at all.
    let before = compiled("examples/billing");
    let mut after = compiled("examples/billing");
    let settled = after
        .commands
        .get_mut(&QualifiedName::new("billing.invoice.PayInvoice").expect("a name"))
        .expect("the fixture declares it")
        .outcomes
        .iter_mut()
        .find(|outcome| outcome.name.as_str() == "settled")
        .expect("the branch exists");
    assert!(
        !settled.payload.is_empty(),
        "the fixture declares where `InvoicePaid`'s fields come from, or there is nothing to erase"
    );
    settled.payload.clear();

    let suite = synthesize(&before).suite;
    let report = impact(&before, &after, &suite).expect("one system, and the earlier one's suite");

    assert!(
        report.delta.is_empty(),
        "the delta has no entry for it, which is exactly why narrowing is not entitled to an \
         answer: {:?}",
        report.delta
    );
    let Invalidation::Whole { because } = &report.invalidation else {
        panic!(
            "a model that moved where no comparison reads owes everything: {:?}",
            report.invalidation
        );
    };
    assert_eq!(*because, ess_diff::WholeSuite::UncomparedFamilyChanged);
    assert_eq!(
        report.churn.conformance_scenarios_invalidated,
        suite.len(),
        "every scenario, because none of them can honestly be said to stand"
    );
}

#[test]
fn a_narrowed_answer_never_reports_more_scenarios_than_the_suite_holds() {
    // The arithmetic that keeps `owed` honest: it is a set of the suite's own keys, so it cannot
    // name a scenario the suite does not hold, and it cannot exceed it.
    let report = catalog_impact();
    let suite = catalog_suite();
    let owed = report.invalidation.owed(&suite);

    assert!(owed.len() <= suite.len());
    for id in &owed {
        assert!(suite.scenario(id).is_some(), "`{id}` is not in the suite");
    }
    assert_eq!(owed.len(), report.churn.conformance_scenarios_invalidated);
}
