//! The closure from a delta to what a committed suite owes again.
//!
//! The fixture pair under `examples/revision-pair/` differs by exactly six changes, and its
//! specification obliges ten scenarios — so this file can assert *which* scenarios each change
//! puts back to owed, rather than that some number of them did. That distinction is the whole
//! reason the fixture exists: an impact engine's failure mode is a plausible answer nobody checks.
//!
//! It also breaks the four fail-closed mechanisms that can be broken from outside the crate and
//! watches each one fail: a suite from the wrong revision, a suite from another system, a scenario
//! resting on a construct no graph has a node for, and a model that moved in a family the delta
//! still does not compare — the topology, and a domain's naming, now that W7.2 moved entities,
//! commands, views and bindings out of that arm and into named change entries.

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

/// The impact of the fixture pair's six changes on the suite the earlier revision obliges.
fn catalog_impact() -> EssImpact {
    impact(
        &compiled("examples/revision-pair/before"),
        &compiled("examples/revision-pair/after"),
        Some(&catalog_suite()),
        None,
    )
    .expect("the pair is two revisions of one system and the suite is the earlier one's")
}

/// Every scenario one change puts back to owed.
fn owed_by(report: &EssImpact, change: &str) -> BTreeSet<String> {
    let Some(Invalidation::Narrowed { scenarios }) = &report.invalidation else {
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
fn the_suite_the_fixture_obliges_is_ten_scenarios_and_the_delta_is_six_changes() {
    // Asserted before anything is measured against it. Every count below is a fraction of these two
    // numbers, so a fixture that quietly stopped obliging scenarios would make every other test in
    // this file pass by reporting nothing.
    let report = catalog_impact();

    assert_eq!(report.churn.conformance_scenarios_total, Some(10));
    assert_eq!(report.churn.semantic_changes_total, 6);
    assert_eq!(report.churn.actor_grants_changed, 2);
}

#[test]
fn an_edited_entity_invariant_owes_every_scenario_that_rests_on_the_entity_and_no_other() {
    // The entity-side half of what W7.2 delivers: this edit used to arrive as an empty delta and
    // fall to the fail-closed catch-all, owing all ten scenarios with no name. Now it is a change
    // entry, the closure seeds at the entity, and the one scenario that never touches a price
    // list — the rejected creation, which brings nothing into existence — is not owed by it.
    let report = catalog_impact();
    let owed = owed_by(
        &report,
        "entity/catalog.pricing.PriceList/invariants-changed",
    );

    assert_eq!(
        owed.len(),
        9,
        "nine of the ten scenarios create, move or refuse-to-move a price list: {owed:?}"
    );
    assert!(
        !owed.contains("catalog.pricing.CreatePriceList/outcome/rejected"),
        "the rejected creation touches no instance, so the invariant edit does not reach it - \
         this absence is the narrowing: {owed:?}"
    );
    assert!(owed.contains("catalog.pricing.CreatePriceList/outcome/created"));
    assert!(owed.contains(
        "catalog.pricing.PriceList/transition/retire/by/catalog.pricing.RetirePriceList/retired"
    ));
}

#[test]
fn an_edited_outcome_guard_owes_every_scenario_because_every_scenario_creates_through_it() {
    // The command-side half. Every scenario in this fixture either exercises `CreatePriceList` or
    // needs an instance it created, so the honest narrowing for this particular change is no
    // narrowing at all — asserted as the full set *with the dependency checked per scenario*, so
    // this fails if the closure ever gets there by accident rather than through the graph.
    let report = catalog_impact();
    let owed = owed_by(
        &report,
        "command/catalog.pricing.CreatePriceList/outcome-condition-changed/created",
    );

    assert_eq!(owed.len(), 10, "{owed:?}");
    let suite = catalog_suite();
    for id in &owed {
        let scenario = suite
            .scenario(&ScenarioId::parse(id).expect("a scenario id"))
            .expect("the suite holds it");
        assert!(
            scenario.source.iter().any(|construct| {
                construct.to_string() == "command catalog.pricing.CreatePriceList"
                    || construct
                        .to_string()
                        .starts_with("outcome catalog.pricing.CreatePriceList/")
            }),
            "`{id}` is owed by the guard edit, so its own dependency set must name the command \
             or one of its branches"
        );
    }
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
        "four of the ten scenarios act as the auditor: {owed:?}"
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
fn a_variant_removed_from_an_enum_reaches_the_entity_that_holds_it_transitively() {
    // Design §24's requirement, on the fixture: `Currency` is not named by the entity, by the
    // command, or by most of the ten scenarios. It is reached through `Money`, and the path is
    // what makes the answer checkable rather than assertable.
    let report = catalog_impact();
    let change = "type/catalog.pricing.Currency/variant-removed/GBP";
    let owed = owed_by(&report, change);

    assert_eq!(
        owed.len(),
        10,
        "every scenario in this fixture either handles a price list priced in a currency or \
         submits a floor price in one: {owed:?}"
    );

    let Some(Invalidation::Narrowed { scenarios }) = &report.invalidation else {
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
            "entity catalog.pricing.PriceList has a field of type type catalog.pricing.Money"
                .to_owned(),
        ],
        "the answer has to be readable as an argument, not as a verdict — and it is the shortest \
         argument, through the floor price, not the longer one through the headline's wrapper"
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
        .as_ref()
        .expect("a suite was given")
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
fn a_suite_whose_contract_digest_its_model_does_not_compute_is_refused() {
    // The state the rule is load-bearing in: the suite is genuinely the `before` revision's — its
    // `spec_digest` matches, so the older refusal provably cannot fire — and only its contract
    // digest has been rewritten, which is a hand edit or a corruption. Narrowing on a false claim
    // of derivation would produce a short list that looks exactly like a correct one.
    let before = compiled("examples/revision-pair/before");
    let after = compiled("examples/revision-pair/after");
    let mut forged = catalog_suite();
    assert_eq!(
        forged.provenance.spec_digest,
        synthesize(&before).suite.provenance.spec_digest,
        "the suite is the earlier revision's, so only the contract check can refuse it"
    );
    forged.provenance.contract_digest = aep_domain::evidence::SpecDigest::new(
        "0000000000000000000000000000000000000000000000000000000000000000",
    )
    .expect("a well-formed digest");

    let refusal =
        impact(&before, &after, Some(&forged), None).expect_err("a false claim is refused");

    assert!(
        matches!(refusal, ImpactRefusal::SuiteContractMismatch { .. }),
        "{refusal:?}"
    );
    assert!(
        refusal.to_string().contains("false"),
        "the refusal says the claim is false, not merely different: {refusal}"
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
    assert_eq!(later.len(), 10, "the later suite is a real suite");

    let refusal =
        impact(&before, &after, Some(&later), None).expect_err("the wrong suite is refused");

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

    let refusal = impact(&before, &after, Some(&elsewhere), None)
        .expect_err("another system's suite is refused");

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
        Some(&catalog_suite()),
        None,
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

    let report =
        impact(&before, &after, Some(&suite), None).expect("the suite is still the earlier one's");

    assert!(
        report
            .invalidation
            .as_ref()
            .expect("a suite was given")
            .is_whole(),
        "a suite the graph cannot see all of is owed whole: {:?}",
        report.invalidation
    );
    assert_eq!(
        report.churn.conformance_scenarios_invalidated,
        Some(suite.len()),
        "every scenario, not the ones a partial walk happened to reach"
    );
}

#[test]
fn an_erased_payload_mapping_is_named_by_the_delta_and_narrowed_rather_than_owing_everything() {
    // The mutation the wave-5 fail-closed test used to make, with W7.2's opposite outcome: an
    // outcome's `payload:` lives in the command family, which the delta now compares, so erasing
    // one arrives as a named change entry and a *narrowing* — the catch-all no longer fires for
    // it. This is the shrink of mechanism 6, proven by the construct that motivated the mechanism.
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
    let report = impact(&before, &after, Some(&suite), None)
        .expect("one system, and the earlier one's suite");

    let ids: Vec<String> = report
        .delta
        .changes()
        .iter()
        .map(|change| change.id().to_string())
        .collect();
    assert_eq!(
        ids,
        vec!["command/billing.invoice.PayInvoice/outcome-payload-changed/settled".to_owned()],
        "the edit that used to arrive as an empty delta now has a name"
    );
    let Some(Invalidation::Narrowed { .. }) = &report.invalidation else {
        panic!(
            "a change the delta can name is narrowed through the graph, not owed whole: {:?}",
            report.invalidation
        );
    };
    assert!(
        report
            .churn
            .conformance_scenarios_invalidated
            .expect("a suite was given")
            > 0,
        "the command moved, so something that rests on it is owed"
    );
}

#[test]
fn a_change_in_a_family_the_delta_still_does_not_compare_owes_the_whole_suite() {
    // Fail-closed mechanism 6, on what remains after W7.2's shrink: the topology has no change
    // family, so flipping one workload's statelessness produces two different models and an
    // **empty** delta — no change entry has a vocabulary for it — and an empty delta narrowing to
    // nothing would be a survival claim about a model that moved. So it is not narrowed at all.
    let before = compiled("examples/billing");
    let mut after = compiled("examples/billing");
    let workload = after
        .workloads
        .values_mut()
        .next()
        .expect("billing declares workloads");
    workload.stateless = !workload.stateless;

    let suite = synthesize(&before).suite;
    let report = impact(&before, &after, Some(&suite), None)
        .expect("one system, and the earlier one's suite");

    assert!(
        report.delta.is_empty(),
        "the delta has no entry for a topology change: {:?}",
        report.delta
    );
    let Some(Invalidation::Whole { because }) = &report.invalidation else {
        panic!(
            "a model that moved where no comparison reads owes everything: {:?}",
            report.invalidation
        );
    };
    assert_eq!(*because, ess_diff::WholeAnswer::UncomparedFamilyChanged);
    assert_eq!(
        report.churn.conformance_scenarios_invalidated,
        Some(suite.len()),
        "every scenario, because none of them can honestly be said to stand"
    );
}

#[test]
fn a_domains_naming_moving_owes_the_whole_suite_because_no_family_compares_a_domain() {
    // The gap W7.2 closed while shrinking the catch-all: a domain document can set a wire name, a
    // display name and a summary, no family compares a domain, and until this slice the
    // uncompared-family check did not read domain naming either — so this exact edit produced an
    // empty delta *and* an empty narrowing, which was a survival claim about a model that moved.
    // Only the naming is checked, deliberately: a domain's membership sets are derived from the
    // constructs' own declarations, each compared by its own family, and folding them in here
    // would send every added type to `Whole` and erase the narrowing.
    let before = compiled("examples/billing");
    let mut after = compiled("examples/billing");
    let domain = after
        .domains
        .values_mut()
        .next()
        .expect("billing declares domains");
    domain.naming.display = Some("Renamed on the page".to_owned());

    let suite = synthesize(&before).suite;
    let report = impact(&before, &after, Some(&suite), None)
        .expect("one system, and the earlier one's suite");

    assert!(
        report.delta.is_empty(),
        "the delta has no entry for a domain naming change: {:?}",
        report.delta
    );
    let Some(Invalidation::Whole { because }) = &report.invalidation else {
        panic!(
            "a model that moved where no comparison reads owes everything: {:?}",
            report.invalidation
        );
    };
    assert_eq!(*because, ess_diff::WholeAnswer::UncomparedFamilyChanged);
}

#[test]
fn a_narrowed_answer_never_reports_more_scenarios_than_the_suite_holds() {
    // The arithmetic that keeps `owed` honest: it is a set of the suite's own keys, so it cannot
    // name a scenario the suite does not hold, and it cannot exceed it.
    let report = catalog_impact();
    let suite = catalog_suite();
    let owed = report
        .invalidation
        .as_ref()
        .expect("a suite was given")
        .owed(&suite);

    assert!(owed.len() <= suite.len());
    for id in &owed {
        assert!(suite.scenario(id).is_some(), "`{id}` is not in the suite");
    }
    assert_eq!(
        Some(owed.len()),
        report.churn.conformance_scenarios_invalidated
    );
}
