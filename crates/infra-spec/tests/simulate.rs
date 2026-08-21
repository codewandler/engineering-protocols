//! What a snapshot decides about a specification — every expectation kind with the case that
//! holds and the case that does not, and every reason an expectation can be undecidable.
//!
//! The committed example carries most of it; the four cases it deliberately does not hold — a
//! digest-pinned image, a bundle that never scanned disruption budgets, a service with no
//! selector at all, and the two workload kinds beside `deployment` — arrive as small bundles
//! built here.

mod support;

use aep_domain::predicate::Truth;
use infra_spec::simulate::{Gap, Outcome, UnknownReason};
use infra_spec::{read_spec, simulate, Simulation};

/// Simulates a one-expectation specification against a bundle's text.
fn verdict_of(body: &str, bundle: &str) -> (Truth, Vec<Outcome>) {
    let spec = read_spec(&format!(
        "format: infra-spec/1\nname: fixture\nexpectations:\n{body}"
    ))
    .expect("the fixture specification is valid");
    let ir = support::compile(bundle);
    let simulation = simulate(&spec, &ir);
    let report = &simulation.reports[0];
    (
        report.verdict,
        report
            .outcomes
            .iter()
            .map(|outcome| outcome.outcome.clone())
            .collect(),
    )
}

/// The committed example's report for one expectation id.
fn example() -> Simulation {
    simulate(&support::example_spec(), &support::example_ir())
}

/// The verdict the committed example reaches for an id.
fn example_verdict(simulation: &Simulation, id: &str) -> Truth {
    simulation
        .reports
        .iter()
        .find(|report| report.id == id)
        .unwrap_or_else(|| panic!("the committed specification declares no `{id}`"))
        .verdict
}

#[test]
fn the_committed_example_reaches_all_three_verdicts_and_the_counts_are_the_documented_ones() {
    let simulation = example();
    assert_eq!(simulation.format, infra_spec::SIMULATION_FORMAT);
    assert_eq!(simulation.snapshot.context, "k3d-dev-cluster");
    assert_eq!(
        (
            simulation.summary.holds,
            simulation.summary.gaps,
            simulation.summary.undecidable
        ),
        (11, 12, 5),
        "the fixture is built to exercise all three verdicts; if this moved, say why in its README"
    );
    assert_eq!(
        simulation.summary.expectations,
        simulation.reports.len(),
        "every expectation gets exactly one report"
    );
}

#[test]
fn every_expectation_kind_holds_somewhere_on_the_fixture_and_fails_somewhere_on_it() {
    let simulation = example();
    let mut held = std::collections::BTreeSet::new();
    let mut failed = std::collections::BTreeSet::new();
    for report in &simulation.reports {
        match report.verdict {
            Truth::True => held.insert(report.kind.clone()),
            Truth::False => failed.insert(report.kind.clone()),
            Truth::Unknown => false,
        };
    }
    // One kind has no holding case on this cluster — nothing here is digest-pinned — and it is
    // covered by a purpose-built bundle below. Naming it keeps the gap deliberate rather than
    // accidental; every other kind is load-bearing in both directions on the one fixture.
    let holds_only_elsewhere = ["image_pinned_by_digest"];
    for kind in infra_spec::ExpectationKind::ALL {
        if !holds_only_elsewhere.contains(kind) {
            assert!(held.contains(*kind), "`{kind}` never holds on the fixture");
        }
        assert!(
            failed.contains(*kind),
            "`{kind}` never fails on the fixture, so its negative case is untested there"
        );
    }
}

#[test]
fn each_undecidable_expectation_on_the_fixture_carries_the_reason_its_name_promises() {
    let simulation = example();
    let reason_of = |id: &str| -> UnknownReason {
        let report = simulation
            .reports
            .iter()
            .find(|report| report.id == id)
            .expect("the id is declared");
        assert_eq!(report.verdict, Truth::Unknown, "`{id}` is not undecidable");
        match &report.outcomes[0].outcome {
            Outcome::Undecidable(reason) => reason.clone(),
            other => panic!("`{id}` reports {other:?} rather than an undecidable outcome"),
        }
    };

    assert!(matches!(
        reason_of("kube-system-replicas"),
        UnknownReason::FieldAbsent { ref field, .. } if field == "replicas"
    ));
    assert!(matches!(
        reason_of("payments-resources"),
        UnknownReason::NamespaceUnobserved { ref namespace } if namespace == "payments"
    ));
    assert!(matches!(
        reason_of("shop-registry"),
        UnknownReason::FieldAbsent { ref field, .. } if field.ends_with("image registry")
    ));
    assert!(matches!(
        reason_of("shop-ready-pods"),
        UnknownReason::FactWithheld { ref path, ref because }
            if path == "workload.ready_pods"
                && matches!(**because, UnknownReason::OwnershipUnderivable { .. })
    ));
    assert!(matches!(
        reason_of("retired-replicas"),
        UnknownReason::NoSubjectInScope
    ));
}

#[test]
fn an_expectation_the_snapshot_cannot_decide_never_becomes_a_gap() {
    // Invariant 5, at the level a report is read at. Every undecidable expectation on the
    // fixture must be absent from the gap count, and every one of its subject outcomes must be
    // an `Undecidable`, not a `Gap` wearing an unknown reason.
    let simulation = example();
    for report in &simulation.reports {
        if report.verdict != Truth::Unknown {
            continue;
        }
        for outcome in &report.outcomes {
            assert!(
                !matches!(outcome.outcome, Outcome::Gap(_)),
                "`{}` is undecidable overall and reports a gap for {}: a false subject would \
                 have made the whole expectation false",
                report.id,
                outcome.subject
            );
        }
    }
    assert_eq!(
        simulation.summary.holds + simulation.summary.gaps + simulation.summary.undecidable,
        simulation.summary.expectations,
        "the three counts partition the expectations; a fourth bucket would be a collapsed value"
    );
}

#[test]
fn a_gap_beside_an_undecidable_subject_still_decides_the_expectation_false() {
    // The Kleene fold where it bites: `shop-replicas` has two workloads outside the range and
    // none it cannot read, while `shop-registry` has none outside and one it cannot read.
    let simulation = example();
    assert_eq!(example_verdict(&simulation, "shop-replicas"), Truth::False);
    assert_eq!(
        example_verdict(&simulation, "shop-registry"),
        Truth::Unknown
    );
}

#[test]
fn one_scope_holding_both_a_contradicted_and_an_undecidable_subject_reads_false() {
    // The Kleene table where `Unknown` and `False` actually meet, which the committed fixture
    // never reaches: `False` dominates, because something *was* observed to be wrong. Reading it
    // as `Unknown` would let one daemonset hide a deployment that is genuinely out of range.
    let mixed = support::bundle(
        "mixed",
        &[
            (
                "namespaces",
                serde_json::json!([support::namespace("shop")]),
            ),
            (
                "deployments",
                serde_json::json!([support::deployment(
                    "shop",
                    "api",
                    1,
                    &serde_json::json!([support::container("api", "registry.example.com/api:3")])
                )]),
            ),
            (
                "daemonsets",
                serde_json::json!([support::daemonset(
                    "shop",
                    "agent",
                    &serde_json::json!([support::container(
                        "agent",
                        "registry.example.com/agent:3"
                    )])
                )]),
            ),
        ],
    );
    let (verdict, outcomes) = verdict_of(
        "  - id: a\n    scope: {namespace: shop}\n    expect:\n      replicas_within: {min: 2, max: 4}\n",
        &mixed,
    );
    // Assert the fixture reached the state the rule is load-bearing in, before asserting the rule.
    assert!(
        outcomes
            .iter()
            .any(|outcome| matches!(outcome, Outcome::Gap(Gap::ReplicasOutsideRange { .. })))
            && outcomes.iter().any(|outcome| matches!(
                outcome,
                Outcome::Undecidable(UnknownReason::FieldAbsent { ref field, .. }) if field == "replicas"
            )),
        "this fixture needs one contradicted subject and one undecidable subject: {outcomes:?}"
    );
    assert_eq!(
        verdict,
        Truth::False,
        "`Unknown` beside `False` is `False`; a report that said otherwise would let an \
         undecidable subject bury an observed one"
    );
}

#[test]
fn a_digest_pinned_image_satisfies_the_pin_expectation_and_a_tagged_one_does_not() {
    let pinned = support::bundle(
        "pinned",
        &[
            (
                "namespaces",
                serde_json::json!([support::namespace("shop")]),
            ),
            (
                "deployments",
                serde_json::json!([support::deployment(
                    "shop",
                    "api",
                    1,
                    &serde_json::json!([support::container(
                        "api",
                        "registry.example.com/api@sha256:\
                         0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                    )])
                )]),
            ),
        ],
    );
    let (verdict, _) = verdict_of("  - id: a\n    expect: image_pinned_by_digest\n", &pinned);
    assert_eq!(verdict, Truth::True);
    // The same expectation over a tagged image is the negative case, and its gap names the image.
    let (verdict, outcomes) = verdict_of(
        "  - id: a\n    expect: image_pinned_by_digest\n",
        &support::bundle(
            "pinned",
            &[
                (
                    "namespaces",
                    serde_json::json!([support::namespace("shop")]),
                ),
                (
                    "deployments",
                    serde_json::json!([support::deployment(
                        "shop",
                        "api",
                        1,
                        &serde_json::json!([support::container(
                            "api",
                            "registry.example.com/api:3"
                        )])
                    )]),
                ),
            ],
        ),
    );
    assert_eq!(verdict, Truth::False);
    assert!(matches!(
        outcomes[0],
        Outcome::Gap(Gap::ImageNotPinned { ref image, .. }) if image == "registry.example.com/api:3"
    ));
    // A digest-pinned image is also not `latest`, whatever tag rides along.
    let (verdict, _) = verdict_of("  - id: a\n    expect: image_tag_not_latest\n", &pinned);
    assert_eq!(
        verdict,
        Truth::True,
        "a digest pins the image, so the tag beside it decorates nothing"
    );
}

#[test]
fn a_bundle_that_never_scanned_disruption_budgets_is_undecidable_and_not_uncovered() {
    // `INFRA-BUNDLE-002`'s argument in the direction that costs a verdict: the twelve required
    // kinds are here and `poddisruptionbudgets` is not, which is a scan that did not look.
    let unscanned = support::bundle(
        "unscanned",
        &[
            (
                "namespaces",
                serde_json::json!([support::namespace("shop")]),
            ),
            (
                "deployments",
                serde_json::json!([support::deployment(
                    "shop",
                    "api",
                    3,
                    &serde_json::json!([support::container("api", "registry.example.com/api:3")])
                )]),
            ),
        ],
    );
    let (verdict, outcomes) = verdict_of(
        "  - id: a\n    expect: pdb_covers_multi_replica\n",
        &unscanned,
    );
    assert_eq!(verdict, Truth::Unknown);
    assert!(matches!(
        outcomes[0],
        Outcome::Undecidable(UnknownReason::KindUnscanned { ref kind }) if kind == "poddisruptionbudgets"
    ));

    // The same bundle with an empty budget list is a scan that *did* look and found none, which
    // is a gap.
    let scanned_and_empty = support::bundle(
        "unscanned",
        &[
            (
                "namespaces",
                serde_json::json!([support::namespace("shop")]),
            ),
            (
                "deployments",
                serde_json::json!([support::deployment(
                    "shop",
                    "api",
                    3,
                    &serde_json::json!([support::container("api", "registry.example.com/api:3")])
                )]),
            ),
            ("poddisruptionbudgets", serde_json::json!([])),
        ],
    );
    let (verdict, outcomes) = verdict_of(
        "  - id: a\n    expect: pdb_covers_multi_replica\n",
        &scanned_and_empty,
    );
    assert_eq!(
        verdict,
        Truth::False,
        "scanned-and-empty is an observation, and unscanned is silence"
    );
    assert!(matches!(
        outcomes[0],
        Outcome::Gap(Gap::DisruptionBudgetAbsent { replicas: 3 })
    ));
}

#[test]
fn a_service_with_no_selector_is_undecidable_rather_than_failing_a_resolution_it_never_claimed() {
    let handmade = support::bundle(
        "endpoints",
        &[
            (
                "namespaces",
                serde_json::json!([support::namespace("shop")]),
            ),
            (
                "services",
                serde_json::json!([{
                    "metadata": {"name": "external", "namespace": "shop", "uid": "uid-svc-external"},
                    "spec": {"ports": [{"port": 443}], "type": "ClusterIP"}
                }]),
            ),
        ],
    );
    let (verdict, outcomes) = verdict_of(
        "  - id: a\n    expect: service_selector_resolves\n",
        &handmade,
    );
    assert_eq!(verdict, Truth::Unknown);
    assert!(matches!(
        outcomes[0],
        Outcome::Undecidable(UnknownReason::FieldAbsent { ref field, .. }) if field == "selector"
    ));
}

#[test]
fn workload_exists_holds_for_each_of_the_three_kinds_and_fails_when_the_kind_is_the_wrong_one() {
    let ir = support::example_ir();
    for (kind, namespace, name) in [
        ("deployment", "shop", "storefront-server"),
        ("statefulset", "shop", "switchboard"),
        ("daemonset", "kube-system", "svclb-traefik-2290261f"),
    ] {
        let spec = read_spec(&format!(
            "format: infra-spec/1\nname: fixture\nexpectations:\n  - id: a\n    expect:\n      \
             workload_exists: {{namespace: {namespace}, workload_kind: {kind}, name: {name}}}\n"
        ))
        .expect("valid");
        assert_eq!(
            simulate(&spec, &ir).reports[0].verdict,
            Truth::True,
            "`{kind}/{namespace}/{name}` is in the fixture"
        );
    }
    // The same name under the wrong kind is absent: workloads are keyed by namespace, kind and
    // name because a deployment may legally share a name with a statefulset.
    let spec = read_spec(
        "format: infra-spec/1\nname: fixture\nexpectations:\n  - id: a\n    expect:\n      \
         workload_exists: {namespace: shop, workload_kind: statefulset, name: storefront-server}\n",
    )
    .expect("valid");
    assert_eq!(simulate(&spec, &ir).reports[0].verdict, Truth::False);
}

#[test]
fn an_optional_dangling_reference_holds_and_a_required_one_does_not() {
    // The `INFRA-DIAG-002`/`-003` split, carried into the expectation: the cluster itself
    // declared `coredns-custom` may be absent, and failing an expectation on that would
    // contradict what the cluster said.
    let simulation = example();
    assert_eq!(
        example_verdict(&simulation, "kube-system-config-refs"),
        Truth::True
    );
    assert_eq!(
        example_verdict(&simulation, "shop-config-refs"),
        Truth::False
    );
}

#[test]
fn a_predicate_reads_the_projections_facts_and_a_false_one_carries_the_values_it_read() {
    let simulation = example();
    let report = simulation
        .reports
        .iter()
        .find(|report| report.id == "shop-replica-floor")
        .expect("declared");
    assert_eq!(report.verdict, Truth::False);
    match &report.outcomes[0].outcome {
        Outcome::Gap(Gap::PredicateFalse { predicate, facts }) => {
            assert_eq!(predicate, "workload.replicas >= 2");
            assert_eq!(
                facts.get("workload.replicas").map(String::as_str),
                Some("1"),
                "a false predicate must carry the values it read: {facts:?}"
            );
        }
        other => panic!("expected a false predicate, got {other:?}"),
    }
}

#[test]
fn a_scope_naming_an_observed_but_empty_namespace_is_undecidable_not_vacuously_satisfied() {
    let empty = support::bundle(
        "empty",
        &[(
            "namespaces",
            serde_json::json!([support::namespace("shop"), support::namespace("payments")]),
        )],
    );
    let (verdict, outcomes) = verdict_of(
        "  - id: a\n    scope: {namespace: payments}\n    expect: resources_declared\n",
        &empty,
    );
    assert_eq!(
        verdict,
        Truth::Unknown,
        "an expectation that passes by selecting nothing is the one way a green report means \
         nothing"
    );
    assert!(matches!(
        outcomes[0],
        Outcome::Undecidable(UnknownReason::NoSubjectInScope)
    ));
}

#[test]
fn a_report_names_every_subject_the_scope_selected_including_the_ones_that_held() {
    let simulation = example();
    let report = simulation
        .reports
        .iter()
        .find(|report| report.id == "shop-replicas")
        .expect("declared");
    assert_eq!(
        report.subjects.len(),
        4,
        "`shop` holds four workloads and all four are the evidence: {:?}",
        report.subjects
    );
    assert_eq!(
        report.outcomes.len(),
        2,
        "only the two outside the range have anything more to say"
    );
    assert!(report
        .subjects
        .contains(&"workloads/shop/deployment/storefront-server".to_owned()));
}
