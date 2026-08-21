//! IW2.5's analysis surface over the committed example observation: per-workload properties
//! with observed replicas and coverage, the invariant candidates with their exceptions, the
//! directions summary, and the HTML component view.
//!
//! The mutation register for this file lives on the wave's plan page:
//! `docs/plan/infra-wave-2-analyze.md` § "Refinement".

use std::path::Path;

use infra_analyze::{
    candidates, candidates_to_text, diagnose, directions, directions_to_text, properties,
    render_html, InfraGraph, PropCode, Severity,
};
use infra_compiler::InfraIr;
use infra_domain::observation::Observation;
use infra_domain::raw::RawBundle;

fn example_ir() -> InfraIr {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/k3d-dev-cluster/observation.json");
    let text = std::fs::read_to_string(&path).expect("the committed observation is readable");
    let raw: RawBundle = serde_json::from_str(&text).expect("the committed observation is JSON");
    let observation = Observation::try_from(raw).expect("the committed observation is valid");
    infra_compiler::compile(&observation)
}

// ---- properties -----------------------------------------------------------------------------

#[test]
fn properties_carry_declared_and_observed_replicas_per_workload() {
    let ir = example_ir();
    let all = properties(&ir);
    let coredns = all
        .iter()
        .find(|entry| entry.workload == "kube-system/deployment/coredns")
        .expect("coredns has properties");
    assert_eq!(coredns.replicas, Some(2), "declared");
    assert_eq!(coredns.observed_pods, 1, "one pod was observed");
    assert_eq!(coredns.ready_pods, 1, "and it is ready");

    let storefront = all
        .iter()
        .find(|entry| entry.workload == "shop/deployment/storefront-server")
        .expect("storefront-server has properties");
    assert_eq!(storefront.replicas, Some(2));
    assert_eq!(storefront.observed_pods, 2);
    assert_eq!(storefront.ready_pods, 0, "neither pod passes readiness");
}

#[test]
fn properties_name_the_budgets_and_autoscalers_covering_each_workload() {
    let ir = example_ir();
    let all = properties(&ir);
    let switchboard = all
        .iter()
        .find(|entry| entry.workload == "shop/statefulset/switchboard")
        .expect("switchboard has properties");
    assert_eq!(
        switchboard.pod_disruption_budgets.as_deref(),
        Some(&["shop/switchboard".to_owned()][..]),
        "the switchboard budget covers the switchboard template"
    );
    assert_eq!(
        switchboard.horizontal_pod_autoscalers.as_deref(),
        Some(&["shop/switchboard".to_owned()][..]),
        "the pinned autoscaler targets the statefulset"
    );

    let storefront = all
        .iter()
        .find(|entry| entry.workload == "shop/deployment/storefront-server")
        .expect("storefront-server has properties");
    assert_eq!(
        storefront.pod_disruption_budgets.as_deref(),
        Some(&[][..]),
        "scanned and uncovered is an empty list, never None"
    );
    assert_eq!(
        storefront.horizontal_pod_autoscalers.as_deref(),
        Some(&["shop/storefront-server".to_owned()][..])
    );
}

#[test]
fn properties_on_an_old_format_bundle_carry_coverage_as_unscanned_not_as_uncovered() {
    let bundle = serde_json::json!({
        "format": "infra-observation/1",
        "context": "old-format",
        "scanned_at": "2026-08-21T08:00:00Z",
        "scout_version": "0.1.0",
        "kinds": {
            "namespaces": { "items": [ { "metadata": { "name": "app", "uid": "ns-1" } } ] },
            "nodes": { "items": [] },
            "deployments": { "items": [
                { "metadata": { "name": "web", "namespace": "app", "uid": "d-1" },
                  "spec": { "replicas": 2,
                            "selector": { "matchLabels": { "app": "web" } },
                            "template": { "metadata": { "labels": { "app": "web" } },
                                          "spec": { "containers": [
                                              { "name": "web", "image": "web:1" } ] } } } }
            ] },
            "statefulsets": { "items": [] }, "daemonsets": { "items": [] },
            "pods": { "items": [] }, "services": { "items": [] }, "ingresses": { "items": [] },
            "configmaps": { "items": [] }, "secrets": { "items": [] },
            "serviceaccounts": { "items": [] }, "persistentvolumeclaims": { "items": [] }
        }
    });
    let raw: RawBundle = serde_json::from_value(bundle).expect("the bundle parses");
    let observation = Observation::try_from(raw).expect("the bundle validates");
    let all = properties(&infra_compiler::compile(&observation));
    assert_eq!(all[0].pod_disruption_budgets, None, "unscanned is None");
    assert_eq!(all[0].horizontal_pod_autoscalers, None);
}

// ---- invariant candidates -------------------------------------------------------------------

#[test]
fn all_three_candidates_are_mined_from_the_committed_observation_in_code_order() {
    let mined = candidates(&example_ir());
    let codes: Vec<PropCode> = mined.iter().map(|candidate| candidate.code).collect();
    assert_eq!(
        codes,
        vec![
            PropCode::UniformRegistry,
            PropCode::UniformPdbCoverage,
            PropCode::UniformResourceBounds
        ],
        "each candidate rule is load-bearing on the fixture"
    );
}

#[test]
fn the_registry_candidate_names_the_dominant_registry_and_lists_every_exception() {
    let mined = candidates(&example_ir());
    let registry = &mined[0];
    assert_eq!(
        registry.statement,
        "all images pull from registry `(default)`"
    );
    assert_eq!((registry.holds_for, registry.population), (4, 7));
    assert_eq!(
        registry.exceptions.len(),
        3,
        "the example.com, localhost and registry.local images: {:?}",
        registry.exceptions
    );
    assert!(
        registry.exceptions.iter().any(|exception| exception.subject
            == "workloads/shop/statefulset/switchboard"
            && exception.detail.contains("registry.example.com")),
        "an exception names its subject and what it does instead: {:?}",
        registry.exceptions
    );
    assert!(
        registry
            .exceptions
            .windows(2)
            .all(|pair| pair[0] <= pair[1]),
        "exceptions arrive sorted, or the rendering is registry-grouping in disguise: {:?}",
        registry.exceptions
    );
}

#[test]
fn a_candidate_with_exceptions_reads_as_uniformity_with_exceptions_not_as_violations() {
    let mined = candidates(&example_ir());
    let coverage = &mined[1];
    assert_eq!(coverage.code, PropCode::UniformPdbCoverage);
    assert_eq!((coverage.holds_for, coverage.population), (2, 3));
    assert_eq!(
        coverage.exceptions[0].subject,
        "workloads/shop/deployment/storefront-server"
    );

    let text = candidates_to_text(&mined);
    assert!(
        text.contains(
            "INFRA-PROP-002 every multi-replica workload has a disruption budget — holds for \
             2 of 3; except:"
        ),
        "the rendering states the uniformity and its counts: {text}"
    );
    assert!(
        !text.contains("violat"),
        "an exception is not a violation, and the rendering must not call it one: {text}"
    );
}

#[test]
fn a_cluster_without_majority_uniformity_yields_no_candidate() {
    // Two registries at one image each: no majority, no candidate — uniformity is observed,
    // never manufactured.
    let bundle = serde_json::json!({
        "format": "infra-observation/1",
        "context": "split",
        "scanned_at": "2026-08-21T08:00:00Z",
        "scout_version": "0.1.0",
        "kinds": {
            "namespaces": { "items": [ { "metadata": { "name": "app", "uid": "ns-1" } } ] },
            "nodes": { "items": [] },
            "deployments": { "items": [
                { "metadata": { "name": "a", "namespace": "app", "uid": "d-1" },
                  "spec": { "replicas": 1,
                            "selector": { "matchLabels": { "app": "a" } },
                            "template": { "metadata": { "labels": { "app": "a" } },
                                          "spec": { "containers": [
                                              { "name": "a", "image": "one.example/a:1" } ] } } } },
                { "metadata": { "name": "b", "namespace": "app", "uid": "d-2" },
                  "spec": { "replicas": 1,
                            "selector": { "matchLabels": { "app": "b" } },
                            "template": { "metadata": { "labels": { "app": "b" } },
                                          "spec": { "containers": [
                                              { "name": "b", "image": "two.example/b:1" } ] } } } }
            ] },
            "statefulsets": { "items": [] }, "daemonsets": { "items": [] },
            "pods": { "items": [] }, "services": { "items": [] }, "ingresses": { "items": [] },
            "configmaps": { "items": [] }, "secrets": { "items": [] },
            "serviceaccounts": { "items": [] }, "persistentvolumeclaims": { "items": [] }
        }
    });
    let raw: RawBundle = serde_json::from_value(bundle).expect("the bundle parses");
    let observation = Observation::try_from(raw).expect("the bundle validates");
    let mined = candidates(&infra_compiler::compile(&observation));
    assert!(
        !mined
            .iter()
            .any(|candidate| candidate.code == PropCode::UniformRegistry),
        "a 1-vs-1 split is not uniformity: {mined:?}"
    );
}

// ---- directions -----------------------------------------------------------------------------

#[test]
fn directions_rank_errors_first_and_lead_with_the_autoscaler_aimed_at_nothing() {
    let ir = example_ir();
    let ranked = directions(&diagnose(&ir), &candidates(&ir));
    assert!(!ranked.is_empty());
    assert_eq!(ranked[0].severity, Severity::Error, "errors lead");
    let error_codes: Vec<&str> = ranked
        .iter()
        .take_while(|direction| direction.severity == Severity::Error)
        .map(|direction| direction.code.as_str())
        .collect();
    assert!(
        error_codes.contains(&"INFRA-DIAG-018"),
        "the ghost autoscaler is among the leading errors: {error_codes:?}"
    );
    // Deduplication: every subject appears once per direction.
    for direction in &ranked {
        let mut seen = std::collections::BTreeSet::new();
        for subject in &direction.subjects {
            assert!(
                seen.insert(subject),
                "{subject} listed twice under {}",
                direction.code
            );
        }
    }
}

#[test]
fn the_directions_text_states_candidate_exceptions_without_prescribing() {
    let ir = example_ir();
    let ranked = directions(&diagnose(&ir), &candidates(&ir));
    let text = directions_to_text(&ranked);
    assert!(
        text.contains("INFRA-PROP-002")
            && text.contains("holds for 2 of 3; 1 exception(s) break the uniformity"),
        "a candidate's direction restates the fact: {text}"
    );
    assert!(
        text.contains("-> workloads/shop/deployment/storefront-server"),
        "the exception is the subject to look at: {text}"
    );
}

// ---- the HTML component view ----------------------------------------------------------------

fn example_html(namespace: Option<&str>) -> String {
    let ir = example_ir();
    let graph = InfraGraph::of(&ir);
    let diagnosis = diagnose(&ir);
    let all = infra_analyze::properties_with(&ir, &graph);
    render_html(&graph, &diagnosis, &all, namespace)
}

#[test]
fn the_html_page_sections_by_namespace_aggregates_pods_and_badges_by_worst_finding() {
    let page = example_html(None);
    assert!(page.contains("<h2>namespace kube-system</h2>"));
    assert!(page.contains("<h2>namespace shop</h2>"));
    assert!(
        page.contains("deployment storefront-server — 0/2 ready"),
        "pods appear only aggregated on their workload: {}",
        &page[..600]
    );
    assert!(
        !page.contains("debug-shell") || page.contains("pods/shop/debug-shell"),
        "no pod boxes; a pod name may appear only inside a finding subject"
    );
    assert!(
        page.contains("deployment flaky-agent — 0/1 ready\"]:::sevError"),
        "the crash-looping pod's error rolls up to its workload's badge"
    );
    assert!(
        page.contains(
            "<script src=\"https://cdn.jsdelivr.net/npm/mermaid@11.4.1/dist/mermaid.min.js\""
        ),
        "the renderer is version-pinned"
    );
    assert!(
        page.contains("class=\"mermaid\""),
        "the Mermaid source is embedded for the browser to render"
    );
    assert!(
        page.contains("<h2>directions</h2>"),
        "directions lead the page"
    );
}

#[test]
fn the_namespace_filter_scopes_sections_findings_and_directions_alike() {
    let page = example_html(Some("kube-system"));
    assert!(page.contains("<h2>namespace kube-system</h2>"));
    assert!(
        !page.contains("<h2>namespace shop</h2>"),
        "only the requested namespace is rendered"
    );
    assert!(
        !page.contains("INFRA-DIAG-008"),
        "shop's crash loop must not leak into kube-system's page"
    );
    assert!(
        !page.contains("flaky-agent"),
        "no shop subject appears anywhere on the filtered page"
    );

    let nowhere = example_html(Some("never-observed"));
    assert!(
        nowhere.contains("nothing observed in scope."),
        "an unobserved namespace is an honest empty page, not an error"
    );
}

#[test]
fn the_html_page_writes_out_as_one_self_contained_file() {
    // The artifact the orchestrator will wire behind `--format html`: written here so the
    // shape is proven end to end before the CLI flag exists.
    let page = example_html(None);
    let directory = std::env::temp_dir().join("infra-analyze-html-test");
    std::fs::create_dir_all(&directory).expect("a scratch directory");
    let path = directory.join("cluster.html");
    std::fs::write(&path, &page).expect("the page writes");
    let read_back = std::fs::read_to_string(&path).expect("and reads back");
    assert_eq!(read_back, page, "one file, byte-complete");
    assert!(
        read_back.starts_with("<!DOCTYPE html>") && read_back.ends_with("</html>\n"),
        "a whole document"
    );
    let external: Vec<&str> = read_back
        .match_indices("https://")
        .map(|(at, _)| &read_back[at..read_back.len().min(at + 60)])
        .collect();
    assert_eq!(
        external.len(),
        1,
        "exactly one external reference, the pinned renderer: {external:?}"
    );
    std::fs::remove_dir_all(&directory).ok();
}
