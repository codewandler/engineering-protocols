//! What the drift report says about the committed pair of observations, and the one thing it
//! refuses.
//!
//! The pair is a fixture and its documented mutation, so each assertion below is a claim about a
//! mutation somebody can read in `examples/k3d-dev-cluster/README.md` — not about a shape the
//! test invented for itself.

mod support;

use infra_spec::drift::{InfraChange, MemberKind, ServiceField, WorkloadField};
use infra_spec::{drift, DriftRefusal};

/// The committed pair, compared.
fn example_drift() -> infra_spec::InfraDrift {
    drift(&support::example_ir(), &support::drifted_ir()).expect("one cluster, two scans")
}

#[test]
fn every_change_kind_the_pair_was_built_to_exercise_appears_exactly_where_it_should() {
    let report = example_drift();
    let counts = infra_spec::drift_counts(&report);
    for (kind, expected) in [
        ("added", 4),
        ("removed", 2),
        ("replicas_changed", 1),
        ("container_added", 1),
        ("container_removed", 1),
        ("image_changed", 1),
        ("resources_changed", 1),
        ("probes_changed", 1),
        ("environment_changed", 2),
        ("workload_field_changed", 1),
        ("service_field_changed", 1),
        ("ingress_routing_changed", 1),
        ("config_content_changed", 2),
        ("claim_phase_changed", 1),
        ("reference_broke", 1),
        ("reference_healed", 1),
    ] {
        assert_eq!(
            counts.get(kind).copied().unwrap_or_default(),
            expected,
            "`{kind}` on the committed pair; the whole report is {:#?}",
            report.changes
        );
    }
    assert_eq!(
        counts.len(),
        16,
        "the pair exercises every change kind this build can report, and nothing else"
    );
}

#[test]
fn a_workloads_replica_count_image_and_labels_each_arrive_as_their_own_typed_change() {
    let report = example_drift();
    assert!(report.changes.contains(&InfraChange::ReplicasChanged {
        subject: "shop/statefulset/switchboard".to_owned(),
        from: Some(2),
        to: Some(3),
    }));
    assert!(report.changes.contains(&InfraChange::ImageChanged {
        subject: "shop/deployment/storefront-server".to_owned(),
        container: "storefront-server".to_owned(),
        from: "localhost:31721/apps/storefront-server:AtqQlTV".to_owned(),
        to: "localhost:31721/apps/storefront-server:Bx7pQr2".to_owned(),
    }));
    assert!(report.changes.contains(&InfraChange::WorkloadFieldChanged {
        subject: "kube-system/deployment/coredns".to_owned(),
        field: WorkloadField::Labels,
    }));
    assert!(report.changes.contains(&InfraChange::ServiceFieldChanged {
        subject: "shop/queue-redis-master".to_owned(),
        field: ServiceField::Ports,
    }));
}

#[test]
fn a_configuration_change_names_the_keys_and_never_a_value() {
    let report = example_drift();
    let content = report
        .changes
        .iter()
        .find_map(|change| match change {
            InfraChange::ConfigContentChanged {
                kind: MemberKind::ConfigMap,
                subject,
                changed_keys,
                ..
            } if subject == "shop/storefront-env" => Some(changed_keys.clone()),
            _ => None,
        })
        .expect("the fixture changes one configmap value");
    assert_eq!(content, vec!["REGION".to_owned()]);

    // The report names *which* keys moved and nothing about what they hold — not the value, and
    // not even the digest of one, which the IR does carry and this deliberately does not repeat.
    let rendered = format!("{}{}", infra_spec::drift_to_text(&report), report.to_json());
    for leak in [
        "eu-central",
        "9c1185a5c5e9fc54612808977ee8f548b2258d31ddadef4a5b7c1e0dbf6e0b40",
    ] {
        assert!(
            !rendered.contains(leak),
            "a drift report says which keys moved, never what they hold, and it printed {leak:?}"
        );
    }
    assert!(
        rendered.contains("REGION"),
        "the key that moved has to be nameable, or the report is a bare `something changed`"
    );
}

#[test]
fn a_reference_change_is_only_reported_for_a_holder_present_in_both_snapshots() {
    let report = example_drift();
    // `flaky-agent` was removed and took its required dangling secret with it; `lost-lookup` was
    // removed and took its dangling selector. Neither is a reference event — the removal already
    // says what happened, and counting both would double-count one change.
    for change in &report.changes {
        if let InfraChange::ReferenceHealed { subject, .. }
        | InfraChange::ReferenceBroke { subject, .. } = change
        {
            assert!(
                !subject.contains("flaky-agent") && !subject.contains("lost-lookup"),
                "{change} names a holder that only exists on one side"
            );
        }
    }
    assert!(report.changes.iter().any(|change| matches!(
        change,
        InfraChange::ReferenceHealed { subject, .. } if subject == "ingresses/shop/edge"
    )));
    assert!(report.changes.iter().any(|change| matches!(
        change,
        InfraChange::ReferenceBroke { subject, .. }
            if subject == "workloads/shop/deployment/queue-redis"
    )));
}

#[test]
fn comparing_a_snapshot_with_itself_reports_no_change_at_all() {
    let ir = support::example_ir();
    let report = drift(&ir, &ir).expect("one snapshot is one cluster");
    assert!(
        report.is_empty(),
        "a snapshot has not drifted from itself: {:#?}",
        report.changes
    );
    assert_eq!(report.from.digest, report.to.digest);
}

#[test]
fn two_snapshots_of_different_clusters_are_refused_rather_than_compared() {
    let other = support::compile(&support::bundle(
        "production",
        &[(
            "namespaces",
            serde_json::json!([support::namespace("shop")]),
        )],
    ));
    let refusal = drift(&support::example_ir(), &other)
        .expect_err("two contexts is the one thing drift refuses");
    assert!(matches!(
        refusal,
        DriftRefusal::DifferentContext { ref from, ref to }
            if from == "k3d-dev-cluster" && to == "production"
    ));
    assert!(
        refusal.to_string().contains("different contexts"),
        "the refusal says what it refused: {refusal}"
    );
}

#[test]
fn a_pods_churn_is_not_drift_because_drift_is_over_declared_state() {
    // Every pod renamed, nothing declared touched: a rollout. The report must be silent, or a
    // restart of a healthy cluster reads as a thousand changes.
    let original = support::read("examples/k3d-dev-cluster/observation.json");
    let mut document: serde_json::Value =
        serde_json::from_str(&original).expect("the fixture is JSON");
    let pods = document["kinds"]["pods"]["items"]
        .as_array_mut()
        .expect("the fixture scans pods");
    for pod in pods.iter_mut() {
        let name = pod["metadata"]["name"]
            .as_str()
            .expect("a pod has a name")
            .to_owned();
        pod["metadata"]["name"] = serde_json::Value::String(format!("{name}-rolled"));
        pod["metadata"]["uid"] = serde_json::Value::String(format!("rolled-{name}"));
    }
    let rolled = support::compile(&document.to_string());
    let report = drift(&support::example_ir(), &rolled).expect("one cluster");
    assert!(
        report.is_empty(),
        "renaming every pod changed no declared state: {:#?}",
        report.changes
    );
}

#[test]
fn reordering_a_templates_containers_is_not_a_change_because_containers_compare_by_name() {
    let original = support::read("examples/k3d-dev-cluster/observation.json");
    let mut document: serde_json::Value =
        serde_json::from_str(&original).expect("the fixture is JSON");
    let daemonsets = document["kinds"]["daemonsets"]["items"]
        .as_array_mut()
        .expect("the fixture scans daemonsets");
    let containers = daemonsets[0]["spec"]["template"]["spec"]["containers"]
        .as_array_mut()
        .expect("the daemonset has containers");
    assert_eq!(
        containers.len(),
        2,
        "this test needs two containers to reorder"
    );
    containers.reverse();
    let reordered = support::compile(&document.to_string());
    let report = drift(&support::example_ir(), &reordered).expect("one cluster");
    assert!(
        report.is_empty(),
        "a positional comparison would have reported both containers moved: {:#?}",
        report.changes
    );
}
