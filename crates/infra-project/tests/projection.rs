//! One row per gap kind: which disposition it gets, and the case beside it that gets the other
//! one.
//!
//! The table in `docs/plan/infra-wave-4-project-back.md` is what this file checks. A gap kind with
//! a patch and no negative case beside it is a rule nothing is holding to its line — the moment
//! somebody widens "we can derive this" the negative case is the only thing that notices.

mod support;

use std::collections::BTreeSet;

use infra_project::{Disposition, ObligationReason, Projection, RefusalReason};

/// The disposition of one (expectation, subject) pair, or a panic naming what is there instead.
fn disposition<'a>(
    projection: &'a Projection,
    expectation: &str,
    subject: &str,
) -> &'a Disposition {
    projection
        .entries
        .iter()
        .find(|entry| entry.expectation == expectation && entry.subject == subject)
        .map_or_else(
            || {
                panic!(
                    "no entry for `{expectation}` on `{subject}`; the projection holds {:?}",
                    projection
                        .entries
                        .iter()
                        .map(|entry| (&entry.expectation, &entry.subject))
                        .collect::<Vec<_>>()
                )
            },
            |entry| &entry.disposition,
        )
}

/// The generated change for a pair, or a panic saying what it got instead.
fn generated<'a>(projection: &'a Projection, expectation: &str, subject: &str) -> &'a str {
    match disposition(projection, expectation, subject) {
        Disposition::Generated(change) => &change.change,
        other => panic!("`{expectation}` on `{subject}` is {other:?}, not a generated change"),
    }
}

/// The obligation reason for a pair, or a panic saying what it got instead.
fn owed<'a>(projection: &'a Projection, expectation: &str, subject: &str) -> &'a ObligationReason {
    match disposition(projection, expectation, subject) {
        Disposition::Obligation(obligation) => &obligation.reason,
        other => panic!("`{expectation}` on `{subject}` is {other:?}, not an obligation"),
    }
}

/// The projection of the committed fixture.
fn fixture() -> Projection {
    infra_project::project(&support::example_spec(), &support::example_ir())
}

// -------------------------------------------------------------------------------------------
// What gets a patch, on the committed fixture.
// -------------------------------------------------------------------------------------------

#[test]
fn a_replica_count_below_the_range_is_raised_to_the_floor_and_nothing_more() {
    let projection = fixture();
    assert_eq!(
        generated(
            &projection,
            "shop-replicas",
            "workloads/shop/deployment/flaky-agent"
        ),
        "spec.replicas: 1 -> 2",
        "the nearest acceptable count, which the range decides; anything else would be a number \
         this crate chose"
    );
}

#[test]
fn a_replica_count_above_the_range_is_lowered_to_the_ceiling() {
    let bundle = support::bundle(
        "nearest",
        &[
            (
                "namespaces",
                serde_json::json!([support::namespace("shop")]),
            ),
            (
                "deployments",
                serde_json::json!([support::deployment(
                    "shop",
                    "wide",
                    9,
                    &serde_json::json!([support::container("main", "registry.example.com/x:1")])
                )]),
            ),
        ],
    );
    let spec = support::spec(
        "  - id: ceiling\n    scope: {namespace: shop}\n    expect:\n      replicas_within: \
         {min: 2, max: 4}\n",
    );
    let projection = infra_project::project(&spec, &support::compile(&bundle));
    assert_eq!(
        generated(&projection, "ceiling", "workloads/shop/deployment/wide"),
        "spec.replicas: 9 -> 4"
    );
}

#[test]
fn a_resource_gap_is_patched_only_because_the_specification_states_the_quantities() {
    let projection = fixture();
    let change = generated(
        &projection,
        "shop-resources",
        "workloads/shop/deployment/queue-redis",
    );
    assert!(
        change.contains("cpu=25m") && change.contains("cpu=500m"),
        "the patch carries the quantities `expected.yaml` states and no others: {change}"
    );
}

#[test]
fn the_same_gap_without_a_stated_value_is_an_obligation_that_names_what_is_missing() {
    // The negative case for the resources patch, and the whole rule in one comparison: the gap is
    // identical, the specification is one `remedy:` block shorter, and the answer changes from a
    // patch to a decision.
    let bundle = support::bundle(
        "unstated",
        &[
            (
                "namespaces",
                serde_json::json!([support::namespace("shop")]),
            ),
            (
                "deployments",
                serde_json::json!([support::deployment(
                    "shop",
                    "bare",
                    1,
                    &serde_json::json!([support::container("main", "registry.example.com/x:1")])
                )]),
            ),
        ],
    );
    let ir = support::compile(&bundle);

    let stated = support::spec(
        "  - id: envelope\n    scope: {namespace: shop}\n    expect: resources_declared\n    \
         remedy:\n      resources:\n        requests: {cpu: 25m}\n        limits: {cpu: 500m}\n",
    );
    let projection = infra_project::project(&stated, &ir);
    assert!(matches!(
        disposition(&projection, "envelope", "workloads/shop/deployment/bare"),
        Disposition::Generated(_)
    ));

    let unstated = support::spec(
        "  - id: envelope\n    scope: {namespace: shop}\n    expect: resources_declared\n",
    );
    let projection = infra_project::project(&unstated, &ir);
    let reason = owed(&projection, "envelope", "workloads/shop/deployment/bare");
    assert_eq!(
        reason,
        &ObligationReason::ValueUnstated {
            fields: vec![
                "resources.requests".to_owned(),
                "resources.limits".to_owned()
            ],
        },
        "the obligation names both halves nobody stated"
    );
}

#[test]
fn a_remedy_that_states_only_one_missing_half_leaves_the_whole_gap_owed() {
    // A half-written patch would close nothing and read as if it had: the expectation wants
    // requests *and* limits, so a container patched with limits alone still gaps. Owed, not
    // partially generated.
    let bundle = support::bundle(
        "half",
        &[
            (
                "namespaces",
                serde_json::json!([support::namespace("shop")]),
            ),
            (
                "deployments",
                serde_json::json!([support::deployment(
                    "shop",
                    "bare",
                    1,
                    &serde_json::json!([support::container("main", "registry.example.com/x:1")])
                )]),
            ),
        ],
    );
    let spec = support::spec(
        "  - id: envelope\n    scope: {namespace: shop}\n    expect: resources_declared\n    \
         remedy:\n      resources:\n        limits: {cpu: 500m}\n",
    );
    let projection = infra_project::project(&spec, &support::compile(&bundle));
    assert_eq!(
        owed(&projection, "envelope", "workloads/shop/deployment/bare"),
        &ObligationReason::ValueUnstated {
            fields: vec!["resources.requests".to_owned()],
        },
        "only the half nobody stated is owed, and the gap is not half-closed"
    );
}

#[test]
fn a_stated_probe_is_written_into_the_container_that_lacks_it() {
    // The probe patch's positive case is here rather than on the committed fixture, deliberately:
    // `shop-probes` is namespace-scoped over four unrelated containers, and one liveness probe for
    // all four would be a guess written as a specification. Where one container is in scope, the
    // author can state a probe that means something — and this is that case.
    let bundle = support::bundle(
        "probes",
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
                    &serde_json::json!([support::container("main", "registry.example.com/x:1")])
                )]),
            ),
        ],
    );
    let spec = support::spec(
        r"  - id: liveness
    scope: {workload_selector: {app: api}}
    expect:
      probes_declared: {liveness: true}
    remedy:
      probes:
        liveness:
          http_get: {path: /healthz, port: 8080}
          period_seconds: 10
",
    );
    let projection = infra_project::project(&spec, &support::compile(&bundle));
    assert_eq!(
        generated(&projection, "liveness", "workloads/shop/deployment/api"),
        "containers[main]: livenessProbe written"
    );

    let patch = &projection.patches[0];
    assert_eq!(patch.patch_type, infra_project::PatchType::Strategic);
    let probe = &patch.patch["spec"]["template"]["spec"]["containers"][0]["livenessProbe"];
    assert_eq!(probe["httpGet"]["path"], "/healthz");
    assert_eq!(
        probe["httpGet"]["port"],
        serde_json::Value::from(8080),
        "a numeric port stays a number; a string would name a port called `8080`"
    );
    assert_eq!(probe["periodSeconds"], serde_json::Value::from(10));
}

#[test]
fn a_probe_gap_with_nothing_stated_is_owed_and_says_what_to_write_where() {
    let projection = fixture();
    assert_eq!(
        owed(
            &projection,
            "shop-probes",
            "workloads/shop/deployment/storefront-server"
        ),
        &ObligationReason::ValueUnstated {
            fields: vec!["probes.liveness".to_owned()],
        }
    );
}

// -------------------------------------------------------------------------------------------
// New objects.
// -------------------------------------------------------------------------------------------

#[test]
fn a_missing_disruption_budget_becomes_a_manifest_built_from_the_workloads_own_selector() {
    let projection = fixture();
    let object = projection
        .objects
        .iter()
        .find(|object| object.target.name == "storefront-server")
        .expect("the fixture's multi-replica workload has no budget");
    assert_eq!(object.target.api_version, "policy/v1");
    assert_eq!(
        object.manifest["spec"]["maxUnavailable"],
        serde_json::Value::from(1),
        "the weakest budget the gap itself determines; any other number is a decision"
    );
    let selector = &object.manifest["spec"]["selector"]["matchLabels"];
    assert_eq!(selector["app.kubernetes.io/name"], "storefront-server");
    assert!(
        object.manifest["metadata"].get("uid").is_none(),
        "a manifest claims no identity the API server has not assigned"
    );
}

#[test]
fn a_budget_whose_name_is_taken_is_owed_rather_than_written_over() {
    // The negative case for the new-object kind. A budget already exists under the name the
    // projection would use, and it covers something else — so writing one is a collision, not a
    // fix, and the collision is the operator's to resolve.
    let bundle = support::bundle(
        "taken",
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
                    &serde_json::json!([support::container("main", "registry.example.com/x:1")])
                )]),
            ),
            (
                "poddisruptionbudgets",
                serde_json::json!([support::budget(
                    "shop",
                    "api",
                    &serde_json::json!({"app": "something-else"})
                )]),
            ),
        ],
    );
    let spec = support::spec(
        "  - id: budgets\n    scope: {namespace: shop}\n    expect: pdb_covers_multi_replica\n",
    );
    let projection = infra_project::project(&spec, &support::compile(&bundle));
    assert_eq!(
        owed(&projection, "budgets", "workloads/shop/deployment/api"),
        &ObligationReason::NameTaken {
            object: "poddisruptionbudget shop/api".to_owned(),
        }
    );
    assert!(
        projection.objects.is_empty(),
        "nothing is written when the name is taken"
    );
}

// -------------------------------------------------------------------------------------------
// What is always owed, on the committed fixture.
// -------------------------------------------------------------------------------------------

#[test]
fn every_gap_kind_that_needs_a_decision_gets_one_with_the_class_that_names_it() {
    let projection = fixture();
    for (expectation, subject, expected) in [
        (
            "checkout-exists",
            "workloads/shop/deployment/checkout-api",
            ObligationReason::ObjectUndefined,
        ),
        (
            "flaky-agent-registry",
            "workloads/shop/deployment/flaky-agent",
            ObligationReason::ImageChoice,
        ),
        (
            "shop-tags",
            "workloads/shop/statefulset/switchboard",
            ObligationReason::ImageChoice,
        ),
        (
            "shop-digests",
            "workloads/shop/deployment/queue-redis",
            ObligationReason::ImageChoice,
        ),
        (
            "shop-selectors",
            "services/shop/lost-lookup",
            ObligationReason::TargetUnknown,
        ),
        (
            "shop-config-refs",
            "workloads/shop/deployment/flaky-agent",
            ObligationReason::TargetUnknown,
        ),
        (
            "shop-only",
            "workloads/kube-system/deployment/coredns",
            ObligationReason::FieldImmutable {
                field: "metadata.namespace".to_owned(),
            },
        ),
    ] {
        assert_eq!(
            owed(&projection, expectation, subject),
            &expected,
            "`{expectation}` on `{subject}`"
        );
    }
}

#[test]
fn every_obligation_names_a_decision_rather_than_repeating_the_gap() {
    let projection = fixture();
    for entry in projection.owed() {
        let Disposition::Obligation(obligation) = &entry.disposition else {
            continue;
        };
        assert!(
            obligation.decision.len() > entry.reads.len(),
            "`{}` on `{}` says no more than the gap did: {:?}",
            entry.expectation,
            entry.subject,
            obligation.decision
        );
    }
}

// -------------------------------------------------------------------------------------------
// What is refused.
// -------------------------------------------------------------------------------------------

#[test]
fn a_false_predicate_is_refused_because_a_condition_names_no_field() {
    // Not on the committed fixture: its one false predicate is `workload.replicas >= 2`, which the
    // replica patch closes on the way past. A predicate over a fact no patch in this build can
    // move is what reaches the refusal.
    let bundle = support::bundle(
        "predicate",
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
                    2,
                    &serde_json::json!([support::container("main", "registry.example.com/x:1")])
                )]),
            ),
        ],
    );
    let spec = support::spec(
        "  - id: sidecar\n    scope: {namespace: shop}\n    expect:\n      workload_predicate: \
         workload.containers == 2\n",
    );
    let projection = infra_project::project(&spec, &support::compile(&bundle));
    match disposition(&projection, "sidecar", "workloads/shop/deployment/api") {
        Disposition::Refused(refusal) => {
            assert_eq!(refusal.reason, RefusalReason::NotAField);
            assert!(
                refusal.detail.contains("workload.containers=1"),
                "the refusal carries the evidence the verdict was reached on: {}",
                refusal.detail
            );
        }
        other => panic!("a predicate gap must be refused, not {other:?}"),
    }
}

#[test]
fn two_expectations_that_disagree_leave_one_of_them_refused_rather_than_silently_lost() {
    // The self-check. `[2, 4]` and `[6, 8]` over one workload cannot both hold; the projection
    // writes one patch, and the expectation that patch does not satisfy is refused by name rather
    // than reported as closed.
    let bundle = support::bundle(
        "disagree",
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
                    &serde_json::json!([support::container("main", "registry.example.com/x:1")])
                )]),
            ),
        ],
    );
    let spec = support::spec(
        "  - id: small\n    scope: {namespace: shop}\n    expect:\n      replicas_within: \
         {min: 2, max: 4}\n  - id: large\n    scope: {namespace: shop}\n    expect:\n      \
         replicas_within: {min: 6, max: 8}\n",
    );
    let projection = infra_project::project(&spec, &support::compile(&bundle));
    let refused: Vec<&str> = projection
        .entries
        .iter()
        .filter(|entry| matches!(entry.disposition, Disposition::Refused(_)))
        .map(|entry| entry.expectation.as_str())
        .collect();
    assert_eq!(
        refused,
        vec!["small"],
        "exactly one of the two disagreeing expectations is refused, and it is the one the \
         emitted patch does not satisfy"
    );
    match disposition(&projection, "small", "workloads/shop/deployment/api") {
        Disposition::Refused(refusal) => assert_eq!(
            refusal.reason,
            RefusalReason::Contradicted {
                by: "large".to_owned()
            }
        ),
        other => panic!("expected a contradiction refusal, got {other:?}"),
    }
}

// -------------------------------------------------------------------------------------------
// Accounting.
// -------------------------------------------------------------------------------------------

#[test]
fn every_gap_the_snapshot_reports_gets_exactly_one_entry_and_no_gap_is_lost() {
    let spec = support::example_spec();
    let ir = support::example_ir();
    let simulation = infra_spec::simulate(&spec, &ir);
    let projection = infra_project::project(&spec, &ir);

    let reported: BTreeSet<(String, String)> = simulation
        .reports
        .iter()
        .flat_map(|report| {
            report
                .outcomes
                .iter()
                .filter(|outcome| matches!(outcome.outcome, infra_spec::Outcome::Gap(_)))
                .map(move |outcome| (report.id.clone(), outcome.subject.clone()))
        })
        .collect();
    let entries: BTreeSet<(String, String)> = projection
        .entries
        .iter()
        .map(|entry| (entry.expectation.clone(), entry.subject.clone()))
        .collect();

    assert!(
        reported.is_subset(&entries),
        "these gaps are in the simulation and in no entry: {:?}",
        reported.difference(&entries).collect::<Vec<_>>()
    );
    assert_eq!(
        entries.len(),
        projection.entries.len(),
        "an (expectation, subject) pair carries at most one entry"
    );
    assert_eq!(
        projection.summary.gaps_observed,
        reported.len(),
        "the summary's observed-gap count is the simulation's"
    );
    assert_eq!(
        projection.summary.generated + projection.summary.obligations + projection.summary.refusals,
        projection.entries.len(),
        "the three dispositions partition the entries; a fourth bucket would be a lost gap"
    );
}

#[test]
fn a_gap_this_projections_own_changes_open_is_marked_as_such_and_closed_in_the_same_tree() {
    let projection = fixture();
    let induced: Vec<&str> = projection
        .entries
        .iter()
        .filter(|entry| entry.origin == infra_project::GapOrigin::Induced)
        .map(|entry| entry.subject.as_str())
        .collect();
    assert_eq!(
        induced,
        vec![
            "workloads/shop/deployment/flaky-agent",
            "workloads/shop/deployment/queue-redis"
        ],
        "raising these two to two replicas is what makes them multi-replica workloads a budget \
         has to cover"
    );
    for subject in induced {
        assert!(
            matches!(
                disposition(&projection, "shop-pdb", subject),
                Disposition::Generated(_)
            ),
            "a projection that opens a gap and does not close it trades one for another"
        );
    }
}

#[test]
fn one_object_gets_one_patch_file_and_its_type_is_the_one_that_carries_every_change_in_it() {
    let projection = fixture();
    let mut seen = BTreeSet::new();
    for patch in &projection.patches {
        assert!(
            seen.insert(patch.target.slug()),
            "`{}` has two patch files, which is two things to apply in an order nobody wrote \
             down",
            patch.target
        );
        assert!(
            patch.path.ends_with(&format!(".{}.json", patch.patch_type)),
            "the filename carries the type it must be applied with: {}",
            patch.path
        );
    }
    let flaky = projection
        .patches
        .iter()
        .find(|patch| patch.target.name == "flaky-agent")
        .expect("the fixture patches it");
    assert_eq!(
        flaky.patch_type,
        infra_project::PatchType::Strategic,
        "it carries a replica change and a container change, and the container change decides"
    );
    assert_eq!(flaky.patch["spec"]["replicas"], serde_json::Value::from(2));
}

#[test]
fn the_tree_holds_a_summary_an_obligations_list_and_nothing_it_did_not_generate() {
    let projection = fixture();
    let files = projection.artifacts();
    let paths: Vec<&str> = files.keys().map(String::as_str).collect();
    assert!(paths.contains(&"SUMMARY.md") && paths.contains(&"OBLIGATIONS.md"));
    assert_eq!(
        files.len(),
        2 + projection.patches.len() + projection.objects.len(),
        "the tree is the two documents plus one file per patch and per generated object: {paths:?}"
    );
    for patch in &projection.patches {
        assert!(files.contains_key(&patch.path));
    }
    let summary = &files["SUMMARY.md"];
    assert!(
        summary.contains(&projection.provenance.specification_digest)
            && summary.contains(&projection.provenance.snapshot_digest),
        "the summary names both inputs it was computed from"
    );
    assert!(
        summary.contains("Nothing here has been applied"),
        "the tree says out loud that it is a proposal"
    );
}

#[test]
fn the_obligations_document_names_every_gap_the_tree_does_not_close_and_no_others() {
    let projection = fixture();
    let document = infra_project::obligations_markdown(&projection);
    for entry in projection.owed() {
        assert!(
            document.contains(&entry.subject),
            "`{}` is owed and not in OBLIGATIONS.md",
            entry.subject
        );
    }
    assert!(
        document.contains(&format!(
            "## Decisions owed ({})",
            projection.summary.obligations
        )),
        "the count in the heading is the count in the summary"
    );
}
