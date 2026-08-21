//! What a desired-state specification is refused for — every `INFRA-SPEC-*` code, each with the
//! document that fires it and a document beside it that does not.
//!
//! Codes, never message text (invariant 4), and one run reports every defect (invariant 3).

mod support;

use infra_domain::code::InfraCode;
use infra_spec::spec::{ExpectationKind, Scope, SubjectClass};
use infra_spec::{read_spec, InfraSpec};

/// The refusals a text produces, or a panic if it validates.
fn refusals(text: &str) -> infra_domain::ValidationErrors {
    read_spec(text).expect_err("this document is meant to be refused")
}

/// A one-expectation document around a body.
fn document(body: &str) -> String {
    format!("format: infra-spec/1\nname: fixture\nexpectations:\n{body}")
}

#[test]
fn the_committed_example_specification_validates_and_declares_every_kind() {
    let spec = support::example_spec();
    assert_eq!(spec.format, infra_spec::SPEC_FORMAT);
    let declared: std::collections::BTreeSet<&str> = spec
        .expectations
        .iter()
        .map(|expectation| expectation.kind.as_str())
        .collect();
    for kind in ExpectationKind::ALL {
        assert!(
            declared.contains(kind),
            "the committed fixture declares no `{kind}`, so that kind is not load-bearing on it"
        );
    }
}

#[test]
fn a_format_this_build_does_not_read_is_refused_with_its_own_code() {
    let refused = refusals(
        "format: infra-spec/2\nname: fixture\nexpectations:\n  - id: a\n    expect: resources_declared\n",
    );
    assert!(
        refused.contains(InfraCode::SpecUnsupportedFormat),
        "{refused}"
    );
}

#[test]
fn a_specification_with_no_expectations_is_refused_rather_than_read_as_satisfied() {
    let refused = refusals("format: infra-spec/1\nname: fixture\nexpectations: []\n");
    assert!(
        refused.contains(InfraCode::SpecEmptyExpectations),
        "a report with no content reads exactly like a report with no gaps: {refused}"
    );
}

#[test]
fn a_document_that_does_not_deserialize_is_one_coded_refusal_and_not_a_serde_sentence() {
    let refused = refusals("format: infra-spec/1\nname: fixture\nexpectations: 7\n");
    assert_eq!(refused.len(), 1, "{refused}");
    assert!(refused.contains(InfraCode::SpecMalformed), "{refused}");

    let unknown_kind = refusals(&document("  - id: a\n    expect: teleports_gracefully\n"));
    assert!(
        unknown_kind.contains(InfraCode::SpecMalformed),
        "an expectation kind this build does not implement is refused by name: {unknown_kind}"
    );

    let misspelt = refusals(&document(
        "  - id: a\n    expect:\n      replicas_within: {mim: 2, max: 4}\n",
    ));
    assert!(
        misspelt.contains(InfraCode::SpecMalformed),
        "`mim` must be refused rather than defaulted to zero: {misspelt}"
    );
}

#[test]
fn an_id_that_is_not_an_identifier_is_refused_and_a_dashed_lowercase_one_is_not() {
    let refused = refusals(&document(
        "  - id: Shop Replicas\n    expect: resources_declared\n",
    ));
    assert!(refused.contains(InfraCode::SpecMalformedId), "{refused}");
    read_spec(&document(
        "  - id: shop-replicas-2\n    expect: resources_declared\n",
    ))
    .expect("a lowercase dashed id is an id");
}

#[test]
fn two_expectations_sharing_an_id_are_refused_because_a_report_names_a_verdict_by_it() {
    let refused = refusals(&document(
        "  - id: same\n    expect: resources_declared\n  - id: same\n    expect: image_tag_not_latest\n",
    ));
    assert!(
        refused.contains(InfraCode::SpecDuplicateExpectation),
        "{refused}"
    );
}

#[test]
fn a_scope_that_cannot_select_the_expectations_subject_is_refused_in_both_directions() {
    let service_by_workload_labels = refusals(&document(
        "  - id: a\n    scope:\n      workload_selector: {app: shop}\n    expect: service_selector_resolves\n",
    ));
    assert!(
        service_by_workload_labels.contains(InfraCode::SpecScopeNotApplicable),
        "a workload's labels are a different map from a service's: {service_by_workload_labels}"
    );

    let named_subject_under_a_namespace = refusals(&document(
        "  - id: a\n    scope: {namespace: shop}\n    expect:\n      workload_exists: {namespace: shop, workload_kind: deployment, name: api}\n",
    ));
    assert!(
        named_subject_under_a_namespace.contains(InfraCode::SpecScopeNotApplicable),
        "an expectation that names its own subject takes no scope: {named_subject_under_a_namespace}"
    );

    read_spec(&document(
        "  - id: a\n    scope: {namespace: shop}\n    expect: service_selector_resolves\n",
    ))
    .expect("a namespace scope selects services");
}

#[test]
fn every_kind_whose_parameters_can_decide_nothing_is_refused_with_one_code() {
    for (label, body) in [
        (
            "min above max",
            "  - id: a\n    expect:\n      replicas_within: {min: 5, max: 2}\n",
        ),
        (
            "neither probe asked for",
            "  - id: a\n    expect:\n      probes_declared: {liveness: false, readiness: false}\n",
        ),
        (
            "an empty registry allowlist",
            "  - id: a\n    expect:\n      image_registry: {allowed: []}\n",
        ),
        (
            "a blank namespace in an allowlist",
            "  - id: a\n    expect:\n      namespace_allowlist: {allowed: [\"\"]}\n",
        ),
        (
            "a workload kind that is not one of the three",
            "  - id: a\n    expect:\n      workload_exists: {namespace: shop, workload_kind: replicaset, name: api}\n",
        ),
        (
            "a blank workload name",
            "  - id: a\n    expect:\n      workload_exists: {namespace: shop, workload_kind: deployment, name: \"\"}\n",
        ),
        (
            "an empty workload selector",
            "  - id: a\n    scope:\n      workload_selector: {}\n    expect: resources_declared\n",
        ),
        (
            "a predicate that holds without observing anything",
            "  - id: a\n    expect:\n      workload_predicate: true\n",
        ),
    ] {
        let refused = refusals(&document(body));
        assert!(
            refused.contains(InfraCode::SpecInvalidExpectation),
            "{label} must be refused: {refused}"
        );
    }
}

#[test]
fn a_predicate_reading_a_fact_the_projection_never_states_is_refused_as_a_typo() {
    let refused = refusals(&document(
        "  - id: a\n    expect:\n      workload_predicate: workload.replica >= 2\n",
    ));
    assert!(
        refused.contains(InfraCode::SpecUnknownFact),
        "a near-miss path evaluates unknown forever, which is a lie about a typo: {refused}"
    );
    read_spec(&document(
        "  - id: a\n    expect:\n      workload_predicate: workload.replicas >= 2\n",
    ))
    .expect("the projected spelling is accepted");
}

#[test]
fn a_specification_with_four_defects_reports_four_refusals_in_one_run() {
    let refused = refusals(
        "format: infra-spec/9\nname: fixture\nexpectations:\n\
         \x20 - id: Bad Id\n    expect: resources_declared\n\
         \x20 - id: dup\n    expect: image_tag_not_latest\n\
         \x20 - id: dup\n    expect:\n      replicas_within: {min: 9, max: 1}\n",
    );
    assert_eq!(
        refused.len(),
        4,
        "the format, the id, the duplicate and the range are four separate defects: {refused}"
    );
    for code in [
        InfraCode::SpecUnsupportedFormat,
        InfraCode::SpecMalformedId,
        InfraCode::SpecDuplicateExpectation,
        InfraCode::SpecInvalidExpectation,
    ] {
        assert!(refused.contains(code), "{code} is missing from {refused}");
    }
}

#[test]
fn the_validated_type_is_only_reachable_through_validation() {
    // Invariant 2, asserted the way `aep-domain`'s own scan asserts it: the source of this crate
    // must not put `Deserialize` on the validated type, and *must* have it on the raw one.
    // Comment-skipping, because prose about the rule is not a breach of it — the same shape
    // `infra-analyze`'s determinism scan uses, and the same reason.
    let derives = |relative: &str| -> Vec<String> {
        support::read(relative)
            .lines()
            .filter(|line| !line.trim().starts_with("//") && line.contains("Deserialize"))
            .map(str::to_owned)
            .collect()
    };
    assert!(
        derives("crates/infra-spec/src/spec.rs").is_empty(),
        "`InfraSpec` and its members must not deserialize; the only door is `TryFrom<RawInfraSpec>`"
    );
    assert!(
        !derives("crates/infra-spec/src/raw.rs").is_empty(),
        "the raw half must deserialize, or this scan has stopped looking at the right file"
    );
}

#[test]
fn a_scope_selects_exactly_the_subject_classes_its_shape_can_reach() {
    // The rule the refusal above rests on, stated over the type rather than over a document.
    let namespaced = Scope::Namespace {
        name: "shop".to_owned(),
    };
    assert!(namespaced.selects(SubjectClass::Workload));
    assert!(namespaced.selects(SubjectClass::Service));
    assert!(!namespaced.selects(SubjectClass::Cluster));
    assert!(Scope::Cluster.selects(SubjectClass::Cluster));
}

#[test]
fn a_specification_reads_from_json_too_because_json_is_yaml() {
    let spec: InfraSpec = read_spec(
        r#"{"format": "infra-spec/1", "name": "fixture",
            "expectations": [{"id": "a", "expect": "resources_declared"}]}"#,
    )
    .expect("a JSON specification is a YAML specification");
    assert_eq!(spec.expectations.len(), 1);
    assert!(spec.expectation("a").is_some());
}

// -------------------------------------------------------------------------------------------
// Remedies — `INFRA-SPEC-009` and `INFRA-SPEC-010`, and the rule that a remedy decides nothing.
// -------------------------------------------------------------------------------------------

#[test]
fn a_remedy_beside_a_kind_that_never_finds_an_empty_field_is_refused_rather_than_carried() {
    let refused = refusals(&document(
        "  - id: a\n    expect: image_tag_not_latest\n    remedy:\n      resources:\n        \
         limits: {cpu: 500m}\n",
    ));
    assert!(
        refused.contains(InfraCode::SpecRemedyNotApplicable),
        "a resources remedy on an image expectation is a patch that will never be written: \
         {refused}"
    );
}

#[test]
fn a_probes_remedy_for_a_probe_the_expectation_never_asks_for_is_refused() {
    let refused = refusals(&document(
        "  - id: a\n    expect:\n      probes_declared: {liveness: true}\n    remedy:\n      \
         probes:\n        readiness:\n          http_get: {path: /ready, port: 8080}\n",
    ));
    assert!(
        refused.contains(InfraCode::SpecInvalidRemedy),
        "an unasked-for probe can never become a gap, so it can never become a patch: {refused}"
    );
}

#[test]
fn a_remedy_that_states_nothing_is_refused_because_it_leaves_the_gap_where_it_was() {
    for body in [
        "  - id: a\n    expect: resources_declared\n    remedy:\n      resources: {}\n",
        "  - id: a\n    expect:\n      probes_declared: {liveness: true}\n    remedy:\n      \
         probes: {}\n",
    ] {
        let refused = refusals(&document(body));
        assert!(
            refused.contains(InfraCode::SpecInvalidRemedy),
            "an empty remedy is not a remedy: {refused}"
        );
    }
}

#[test]
fn a_probe_remedy_states_exactly_one_handler_and_neither_is_refused_the_same_way_as_both() {
    for handlers in [
        "          initial_delay_seconds: 5\n",
        "          http_get: {path: /healthz, port: 8080}\n          tcp_socket: {port: 8080}\n",
    ] {
        let refused = refusals(&document(&format!(
            "  - id: a\n    expect:\n      probes_declared: {{liveness: true}}\n    remedy:\n      \
             probes:\n        liveness:\n{handlers}"
        )));
        assert!(
            refused.contains(InfraCode::SpecInvalidRemedy),
            "a probe does one thing: {refused}"
        );
    }
}

#[test]
fn a_quoted_number_is_refused_as_a_port_name_because_it_is_one() {
    let refused = refusals(&document(
        "  - id: a\n    expect:\n      probes_declared: {liveness: true}\n    remedy:\n      \
         probes:\n        liveness:\n          tcp_socket: {port: \"8080\"}\n",
    ));
    assert!(
        refused.contains(InfraCode::SpecInvalidRemedy),
        "`\"8080\"` names a port called `8080`, and a container that declares none never becomes \
         ready: {refused}"
    );
}

#[test]
fn a_remedy_that_validates_is_carried_on_the_expectation_and_a_document_without_one_carries_none() {
    let with = read_spec(&document(
        "  - id: a\n    expect: resources_declared\n    remedy:\n      resources:\n        \
         requests: {cpu: 25m}\n        limits: {cpu: 500m, memory: 256Mi}\n",
    ))
    .expect("the document is valid");
    let stated = with.expectations[0]
        .remedy
        .as_ref()
        .expect("the remedy survived validation");
    let (requests, limits) = stated.resource_quantities();
    assert_eq!(requests["cpu"], "25m");
    assert_eq!(limits["memory"], "256Mi");

    let without = read_spec(&document("  - id: a\n    expect: resources_declared\n"))
        .expect("the document is valid");
    assert!(
        without.expectations[0].remedy.is_none(),
        "a remedy is never invented for an expectation that states none"
    );
}

#[test]
fn a_remedy_changes_no_verdict_because_nothing_evaluates_one() {
    // The rule that makes a remedy safe to add to a committed specification: it is written into a
    // patch, never read by the evaluator. Two documents differing only in remedies must simulate
    // to the same bytes, or `simulation.json` has started depending on a projection's input.
    let ir = support::example_ir();
    let bare = read_spec(&document(
        "  - id: a\n    scope: {namespace: shop}\n    expect: resources_declared\n",
    ))
    .expect("valid");
    let remedied = read_spec(&document(
        r"  - id: a
    scope: {namespace: shop}
    expect: resources_declared
    remedy:
      resources:
        requests: {cpu: 25m}
        limits: {cpu: 500m}
",
    ))
    .expect("valid");

    assert_ne!(
        bare.digest(),
        remedied.digest(),
        "the two specifications differ, so their digests must too — otherwise this test compares \
         one document with itself"
    );
    assert_eq!(
        infra_spec::simulate(&bare, &ir).to_json(),
        infra_spec::simulate(&remedied, &ir).to_json(),
        "a remedy is what a projection writes, never what an evaluator reads"
    );
}

#[test]
fn the_committed_example_specification_simulates_identically_with_and_without_its_remedies() {
    // The same rule, on the document the gate drift-checks: `simulation.json` must not move
    // because somebody added a remedy to `expected.yaml`.
    let ir = support::example_ir();
    let spec = support::example_spec();
    assert!(
        spec.expectations
            .iter()
            .any(|expectation| expectation.remedy.is_some()),
        "the committed fixture is meant to carry remedies; without one this test compares two \
         identical documents"
    );

    let text = support::read("examples/k3d-dev-cluster/expected.yaml");
    let stripped = strip_remedies(&text);
    let without = read_spec(&stripped).expect("the stripped specification is still valid");
    assert!(
        without
            .expectations
            .iter()
            .all(|expectation| expectation.remedy.is_none()),
        "the strip left a remedy behind, so this test proves nothing"
    );
    assert_eq!(
        infra_spec::simulate(&spec, &ir).to_json(),
        infra_spec::simulate(&without, &ir).to_json()
    );
}

/// Removes every `remedy:` block from a specification's text, by indentation.
///
/// Textual because the point is to compare the *committed file* with itself minus one feature; a
/// structural strip would go through the validated type this test is checking.
fn strip_remedies(text: &str) -> String {
    let mut kept = Vec::new();
    let mut dropping = None;
    for line in text.lines() {
        let indent = line.len() - line.trim_start().len();
        if let Some(depth) = dropping {
            if line.trim().is_empty() || indent > depth {
                continue;
            }
            dropping = None;
        }
        if line.trim_start().starts_with("remedy:") {
            dropping = Some(indent);
            continue;
        }
        kept.push(line);
    }
    let mut rendered = kept.join("\n");
    rendered.push('\n');
    rendered
}
