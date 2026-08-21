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
