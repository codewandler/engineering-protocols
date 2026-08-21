//! Every diagnosis rule, load-bearing on the committed example observation — one positive and
//! one negative case per rule, asserted by code, so disabling any rule fails here naming it.
//!
//! The mutation register for this file (each applied, watched fail, reverted — the convention
//! `AGENTS.md` § Conventions demands) lives on the wave's plan page:
//! `docs/plan/infra-wave-2-analyze.md` § "Mutations run".

use std::path::Path;

use infra_analyze::{diagnose, DiagCode, Diagnosis, Finding, Severity};
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

fn example_diagnosis() -> Diagnosis {
    diagnose(&example_ir())
}

/// The findings under one code.
fn of_code(diagnosis: &Diagnosis, code: DiagCode) -> Vec<&Finding> {
    diagnosis
        .findings
        .iter()
        .filter(|finding| finding.code == code)
        .collect()
}

/// Whether any finding under `code` is about `subject`.
fn fires_on(diagnosis: &Diagnosis, code: DiagCode, subject: &str) -> bool {
    of_code(diagnosis, code)
        .iter()
        .any(|finding| finding.subject == subject)
}

#[test]
fn every_registered_code_fires_at_least_once_on_the_example_observation() {
    // The registry-level guard: a rule cannot be registered without being load-bearing on the
    // committed fixture, and a disabled rule fails here naming its code.
    let diagnosis = example_diagnosis();
    for code in DiagCode::ALL {
        assert!(
            !of_code(&diagnosis, *code).is_empty(),
            "{code} ({code:?}) produced no finding on the example observation — its rule is \
             disabled or its fixture case is gone"
        );
    }
}

#[test]
fn findings_arrive_sorted_and_each_carries_its_codes_registered_severity() {
    let diagnosis = example_diagnosis();
    assert!(
        diagnosis.findings.windows(2).all(|pair| pair[0] <= pair[1]),
        "findings must arrive in canonical order"
    );
    for finding in &diagnosis.findings {
        assert_eq!(
            finding.severity,
            finding.code.severity(),
            "{}: a finding's severity is a function of its code and of nothing else",
            finding.code
        );
    }
}

#[test]
fn a_selector_matching_nothing_is_diagnosed_and_a_matching_one_is_not() {
    let diagnosis = example_diagnosis();
    let dangling = of_code(&diagnosis, DiagCode::DanglingSelector);
    assert_eq!(dangling.len(), 1, "exactly the lost-lookup service");
    assert_eq!(dangling[0].subject, "services/shop/lost-lookup");
    assert_eq!(
        dangling[0].evidence.get("selector").map(String::as_str),
        Some("app=retired")
    );
}

#[test]
fn a_required_missing_reference_is_an_error_and_an_optional_one_is_info() {
    let diagnosis = example_diagnosis();

    let required = of_code(&diagnosis, DiagCode::MissingReference);
    assert_eq!(
        required.len(),
        2,
        "the flaky-agent secret and the retired ingress backend: {required:?}"
    );
    assert!(
        fires_on(
            &diagnosis,
            DiagCode::MissingReference,
            "workloads/shop/deployment/flaky-agent"
        ),
        "the required `agent-credentials` secret is absent and must be an error"
    );
    assert!(
        fires_on(
            &diagnosis,
            DiagCode::MissingReference,
            "ingresses/shop/edge"
        ),
        "the `retired-api` backend service is absent and must be an error"
    );

    let optional = of_code(&diagnosis, DiagCode::MissingOptionalReference);
    assert_eq!(
        optional.len(),
        1,
        "exactly coredns's optional custom configmap"
    );
    assert_eq!(
        optional[0].subject, "workloads/kube-system/deployment/coredns",
        "the optional `coredns-custom` volume is info, not error — the distinction is the rule"
    );
}

#[test]
fn a_container_without_bounds_fires_and_the_bounded_coredns_container_does_not() {
    let diagnosis = example_diagnosis();
    assert!(
        fires_on(
            &diagnosis,
            DiagCode::NoResourceBounds,
            "workloads/shop/deployment/flaky-agent"
        ),
        "flaky-agent declares neither requests nor limits"
    );
    assert!(
        !fires_on(
            &diagnosis,
            DiagCode::NoResourceBounds,
            "workloads/kube-system/deployment/coredns"
        ),
        "coredns declares requests and limits; firing on it would be a false claim"
    );
}

#[test]
fn a_container_without_probes_fires_and_the_probed_coredns_container_does_not() {
    let diagnosis = example_diagnosis();
    assert!(
        fires_on(
            &diagnosis,
            DiagCode::NoProbes,
            "workloads/shop/statefulset/switchboard"
        ),
        "switchboard has no probes at all"
    );
    assert!(
        !fires_on(
            &diagnosis,
            DiagCode::NoProbes,
            "workloads/kube-system/deployment/coredns"
        ),
        "coredns has liveness and readiness probes"
    );
}

#[test]
fn latest_and_untagged_images_fire_and_a_pinned_tag_does_not() {
    let diagnosis = example_diagnosis();
    let unpinned = of_code(&diagnosis, DiagCode::UnpinnedImage);
    assert!(
        unpinned.iter().any(
            |finding| finding.subject == "workloads/shop/statefulset/switchboard"
                && finding.message.contains("`latest` tag")
        ),
        "switchboard runs :latest: {unpinned:?}"
    );
    assert!(
        unpinned.iter().any(
            |finding| finding.subject == "workloads/shop/deployment/flaky-agent"
                && finding.message.contains("no tag")
        ),
        "flaky-agent's image is untagged: {unpinned:?}"
    );
    assert!(
        !fires_on(
            &diagnosis,
            DiagCode::UnpinnedImage,
            "workloads/kube-system/deployment/coredns"
        ),
        "coredns is pinned to 1.12.0"
    );
}

#[test]
fn one_replica_is_info_and_two_replicas_or_a_daemonset_are_not() {
    let diagnosis = example_diagnosis();
    assert!(
        fires_on(
            &diagnosis,
            DiagCode::SingleReplica,
            "workloads/shop/deployment/flaky-agent"
        ),
        "one replica is the finding"
    );
    assert!(
        !fires_on(
            &diagnosis,
            DiagCode::SingleReplica,
            "workloads/shop/statefulset/switchboard"
        ),
        "switchboard wants two replicas; firing on it would make the rule mean `has replicas`"
    );
    assert!(
        !fires_on(
            &diagnosis,
            DiagCode::SingleReplica,
            "workloads/kube-system/daemonset/svclb-traefik-2290261f"
        ),
        "a daemonset has no replica count to judge"
    );
}

#[test]
fn a_crashlooping_container_is_an_error_and_a_creating_one_is_not() {
    let diagnosis = example_diagnosis();
    let stuck = of_code(&diagnosis, DiagCode::PodStuckWaiting);
    assert_eq!(
        stuck.len(),
        1,
        "exactly the crash-looping flaky-agent pod: {stuck:?}"
    );
    assert_eq!(stuck[0].subject, "pods/shop/flaky-agent-6d8f9c7b44-x1q2z");
    assert_eq!(
        stuck[0].evidence.get("reason").map(String::as_str),
        Some("CrashLoopBackOff")
    );
    assert!(
        !fires_on(
            &diagnosis,
            DiagCode::PodStuckWaiting,
            "pods/shop/switchboard-0"
        ),
        "switchboard-0 waits as ContainerCreating — normal startup, not a defect"
    );
}

#[test]
fn repeated_restarts_fire_and_a_stable_container_does_not() {
    let diagnosis = example_diagnosis();
    assert!(
        fires_on(
            &diagnosis,
            DiagCode::HighRestartCount,
            "pods/shop/flaky-agent-6d8f9c7b44-x1q2z"
        ),
        "seventeen restarts are over any threshold"
    );
    assert!(
        fires_on(
            &diagnosis,
            DiagCode::HighRestartCount,
            "pods/kube-system/coredns-ccb96694c-jkz7w"
        ),
        "coredns's ten restarts are a real observation of the real cluster"
    );
    assert!(
        !fires_on(
            &diagnosis,
            DiagCode::HighRestartCount,
            "pods/shop/queue-redis-66f544d5b-mplh5"
        ),
        "zero restarts are not high"
    );
}

#[test]
fn a_pod_its_workload_expects_ready_fires_and_a_finished_job_pod_does_not() {
    let diagnosis = example_diagnosis();
    assert!(
        fires_on(
            &diagnosis,
            DiagCode::PodNotReady,
            "pods/shop/flaky-agent-6d8f9c7b44-x1q2z"
        ),
        "the deployment expects this pod ready and it is not"
    );
    assert!(
        !fires_on(
            &diagnosis,
            DiagCode::PodNotReady,
            "pods/shop/cache-warm-jc7dd"
        ),
        "a Succeeded Job pod is done, not broken"
    );
    assert!(
        !fires_on(&diagnosis, DiagCode::PodNotReady, "pods/shop/debug-shell"),
        "a ready pod is not a finding"
    );
}

#[test]
fn unreferenced_config_fires_and_referenced_or_token_managed_config_does_not() {
    let diagnosis = example_diagnosis();
    assert!(
        fires_on(
            &diagnosis,
            DiagCode::OrphanedConfig,
            "config_maps/shop/abandoned-config"
        ),
        "nothing references abandoned-config"
    );
    assert!(
        fires_on(
            &diagnosis,
            DiagCode::OrphanedConfig,
            "secrets/shop/devspace-cache-acd"
        ),
        "nothing references the devspace cache secret"
    );
    assert!(
        !fires_on(
            &diagnosis,
            DiagCode::OrphanedConfig,
            "config_maps/kube-system/coredns"
        ),
        "coredns mounts its configmap; calling it orphaned would be a false claim"
    );
    assert!(
        !fires_on(
            &diagnosis,
            DiagCode::OrphanedConfig,
            "secrets/shop/sa-token-legacy"
        ),
        "a service-account token secret is the token controller's, exempted by type"
    );
}

#[test]
fn a_pending_claim_fires_and_a_bound_one_does_not() {
    let diagnosis = example_diagnosis();
    let unbound = of_code(&diagnosis, DiagCode::UnboundClaim);
    assert_eq!(unbound.len(), 1, "exactly the pending orphan-cache claim");
    assert_eq!(unbound[0].subject, "claims/shop/orphan-cache");
    assert!(
        !fires_on(
            &diagnosis,
            DiagCode::UnboundClaim,
            "claims/shop/queue-redis-data"
        ),
        "a bound claim is healthy"
    );
}

#[test]
fn an_unreferenced_claim_fires_and_the_mounted_one_does_not() {
    let diagnosis = example_diagnosis();
    assert!(
        fires_on(
            &diagnosis,
            DiagCode::OrphanedClaim,
            "claims/shop/orphan-cache"
        ),
        "no workload volume references orphan-cache"
    );
    assert!(
        !fires_on(
            &diagnosis,
            DiagCode::OrphanedClaim,
            "claims/shop/queue-redis-data"
        ),
        "queue-redis mounts this claim"
    );
}

#[test]
fn two_services_selecting_one_workload_set_are_reported_once_together() {
    let diagnosis = example_diagnosis();
    let duplicates = of_code(&diagnosis, DiagCode::DuplicateSelectors);
    assert_eq!(duplicates.len(), 1, "one group: {duplicates:?}");
    let services = duplicates[0]
        .evidence
        .get("services")
        .expect("the group names its services");
    assert!(
        services.contains("shop/switchboard-client")
            && services.contains("shop/switchboard-headless"),
        "both services of the group are named: {services}"
    );
    assert!(
        !services.contains("storefront"),
        "a service with its own target set is not in the group: {services}"
    );
}

#[test]
fn a_budget_guarding_nothing_fires_and_the_one_guarding_switchboard_does_not() {
    let diagnosis = example_diagnosis();
    let dangling = of_code(&diagnosis, DiagCode::PdbSelectsNothing);
    assert_eq!(dangling.len(), 1, "exactly the retired-workers budget");
    assert_eq!(
        dangling[0].subject,
        "pod_disruption_budgets/shop/retired-workers"
    );
    assert!(
        !fires_on(
            &diagnosis,
            DiagCode::PdbSelectsNothing,
            "pod_disruption_budgets/shop/switchboard"
        ),
        "the switchboard budget matches switchboard-0; firing on it would be a false claim"
    );
}

#[test]
fn a_multi_replica_workload_without_a_budget_fires_and_a_covered_one_does_not() {
    let diagnosis = example_diagnosis();
    assert!(
        fires_on(
            &diagnosis,
            DiagCode::NoPdbCoverage,
            "workloads/shop/deployment/storefront-server"
        ),
        "two replicas, no covering budget"
    );
    assert!(
        !fires_on(
            &diagnosis,
            DiagCode::NoPdbCoverage,
            "workloads/shop/statefulset/switchboard"
        ),
        "the switchboard budget covers the switchboard template; firing would be a false claim"
    );
    assert!(
        !fires_on(
            &diagnosis,
            DiagCode::NoPdbCoverage,
            "workloads/shop/deployment/flaky-agent"
        ),
        "one replica is DIAG-007's finding, not a coverage gap"
    );
}

#[test]
fn an_autoscaler_pinned_to_one_size_fires_and_a_real_range_does_not() {
    let diagnosis = example_diagnosis();
    let pinned = of_code(&diagnosis, DiagCode::HpaFixedRange);
    assert_eq!(pinned.len(), 1, "exactly the switchboard autoscaler");
    assert_eq!(
        pinned[0].subject,
        "horizontal_pod_autoscalers/shop/switchboard"
    );
    assert!(
        !fires_on(
            &diagnosis,
            DiagCode::HpaFixedRange,
            "horizontal_pod_autoscalers/shop/storefront-server"
        ),
        "1..3 is a range"
    );
}

#[test]
fn an_autoscaler_aimed_at_nothing_is_an_error_and_an_aimed_one_is_not() {
    let diagnosis = example_diagnosis();
    let missing = of_code(&diagnosis, DiagCode::HpaTargetMissing);
    assert_eq!(missing.len(), 1, "exactly the ghost-scaler");
    assert_eq!(
        missing[0].subject,
        "horizontal_pod_autoscalers/shop/ghost-scaler"
    );
    assert_eq!(
        missing[0].evidence.get("target_name").map(String::as_str),
        Some("retired-api-server")
    );
    assert!(
        !fires_on(
            &diagnosis,
            DiagCode::HpaTargetMissing,
            "horizontal_pod_autoscalers/shop/switchboard"
        ),
        "the switchboard statefulset exists; firing on its autoscaler would be a false claim"
    );
}

#[test]
fn a_job_short_of_its_completions_with_failures_fires_and_a_completed_one_does_not() {
    let diagnosis = example_diagnosis();
    let failed = of_code(&diagnosis, DiagCode::JobFailed);
    assert_eq!(failed.len(), 1, "exactly the reindex job");
    assert_eq!(failed[0].subject, "jobs/shop/reindex-29301120");
    assert_eq!(
        failed[0].evidence.get("failed").map(String::as_str),
        Some("3")
    );
    assert!(
        !fires_on(&diagnosis, DiagCode::JobFailed, "jobs/shop/cache-warm"),
        "a job that reached its completions succeeded, whatever the retries cost"
    );
}

#[test]
fn a_suspended_cronjob_is_info_and_a_running_one_is_not() {
    let diagnosis = example_diagnosis();
    let suspended = of_code(&diagnosis, DiagCode::CronJobSuspended);
    assert_eq!(suspended.len(), 1, "exactly nightly-report");
    assert_eq!(suspended[0].subject, "cron_jobs/shop/nightly-report");
    assert!(
        !fires_on(
            &diagnosis,
            DiagCode::CronJobSuspended,
            "cron_jobs/shop/reindex"
        ),
        "an unsuspended cronjob is not a finding"
    );
}

#[test]
fn the_new_rules_stay_silent_on_a_bundle_that_did_not_scan_their_kinds() {
    // The compatibility half of every IW2.5 rule: unobserved is not unbound. An IW2-format
    // bundle (no replicasets, jobs, cronjobs, budgets or autoscalers) must produce none of the
    // six new findings — a coverage gap must never be manufactured out of a scan gap.
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
                  "spec": { "replicas": 3,
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
    let observation = Observation::try_from(raw).expect("an IW2-format bundle still validates");
    let diagnosis = diagnose(&infra_compiler::compile(&observation));
    for code in [
        DiagCode::PdbSelectsNothing,
        DiagCode::NoPdbCoverage,
        DiagCode::HpaFixedRange,
        DiagCode::HpaTargetMissing,
        DiagCode::JobFailed,
        DiagCode::CronJobSuspended,
    ] {
        assert!(
            of_code(&diagnosis, code).is_empty(),
            "{code} fired on a bundle that never scanned its kind — a defect out of a gap"
        );
    }
}

#[test]
fn the_severity_floor_filters_out_exactly_what_is_below_it() {
    let diagnosis = example_diagnosis();
    let (errors, warnings, infos) = diagnosis.counts();
    assert!(
        errors > 0 && warnings > 0 && infos > 0,
        "the fixture exercises all three severities: {errors}/{warnings}/{infos}"
    );
    assert_eq!(
        diagnosis.at_least(Severity::Warning).len(),
        errors + warnings,
        "the floor keeps errors and warnings and drops info"
    );
    assert_eq!(
        diagnosis.at_least(Severity::Info).len(),
        diagnosis.findings.len(),
        "the lowest floor keeps everything"
    );
}
