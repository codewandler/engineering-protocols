//! The graph over the committed example observation: every relation minted, ownership derived
//! and refused on evidence, the namespace restriction honest.

use std::path::Path;

use infra_analyze::{EdgeRelation, GraphDocument, InfraGraph, NodeKind, UnderivedReason};
use infra_compiler::InfraIr;
use infra_domain::observation::Observation;
use infra_domain::raw::RawBundle;

/// Compiles the committed example observation — the same fixture the gate drift-checks.
fn example_ir() -> InfraIr {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/k3d-dev-cluster/observation.json");
    let text = std::fs::read_to_string(&path).expect("the committed observation is readable");
    let raw: RawBundle = serde_json::from_str(&text).expect("the committed observation is JSON");
    let observation = Observation::try_from(raw).expect("the committed observation is valid");
    infra_compiler::compile(&observation)
}

#[test]
fn every_edge_relation_is_minted_from_the_committed_observation() {
    // A relation nothing produces is decoration, not vocabulary — the `ess-diff` rule, held
    // here against the example cluster.
    let graph = InfraGraph::of(&example_ir());
    let minted: std::collections::BTreeSet<EdgeRelation> =
        graph.edges().map(|edge| edge.relation).collect();
    for relation in EdgeRelation::ALL {
        assert!(
            minted.contains(&relation),
            "no edge in the example graph carries `{}` ({relation:?}); either the fixture \
             stopped exercising it or the walk stopped producing it",
            relation.verb()
        );
    }
}

#[test]
fn a_deployment_pod_is_owned_exactly_through_its_observed_replicaset() {
    let graph = InfraGraph::of(&example_ir());
    assert_eq!(
        graph.owner_of("kube-system/coredns-ccb96694c-jkz7w"),
        Some("kube-system/deployment/coredns"),
        "pod -> replicaset -> deployment, every rung observed"
    );
    assert_eq!(
        graph.owner_of("sbf/asterisk-0"),
        Some("sbf/statefulset/asterisk"),
        "a statefulset pod names its workload directly"
    );

    // The chain is drawn as it is declared: the pod's edge lands on the replicaset, the
    // replicaset's on the deployment, and both sites name the mechanism — `ownerReferences`,
    // never the hash heuristic, because on this bundle the heuristic is not needed.
    let pod_edge = graph
        .edges()
        .find(|edge| {
            edge.relation == EdgeRelation::OwnedBy
                && edge.from.key == "kube-system/coredns-ccb96694c-jkz7w"
        })
        .expect("the coredns pod has an ownership edge");
    assert_eq!(pod_edge.to.kind, NodeKind::ReplicaSet);
    assert_eq!(pod_edge.to.key, "kube-system/coredns-ccb96694c");
    assert_eq!(pod_edge.sites, vec!["ownerReferences".to_owned()]);

    let replicaset_edge = graph
        .edges()
        .find(|edge| {
            edge.relation == EdgeRelation::OwnedBy
                && edge.from.key == "kube-system/coredns-ccb96694c"
        })
        .expect("the coredns replicaset has an ownership edge");
    assert_eq!(replicaset_edge.to.key, "kube-system/deployment/coredns");
    assert_eq!(replicaset_edge.sites, vec!["ownerReferences".to_owned()]);
}

#[test]
fn a_job_pod_chains_to_its_job_and_cronjob_and_a_bare_pod_stays_a_typed_fact() {
    let graph = InfraGraph::of(&example_ir());
    let underived = graph.underived_owners();

    // The Job's pod, an underived fact in IW2, now derives: the job kind is observed.
    assert!(
        !underived
            .iter()
            .any(|fact| fact.pod == "sbf/cache-warm-jc7dd"),
        "a pod whose job was observed is no longer underived: {underived:?}"
    );
    let job_edge = graph
        .edges()
        .find(|edge| {
            edge.relation == EdgeRelation::OwnedBy && edge.from.key == "sbf/cache-warm-jc7dd"
        })
        .expect("the job pod's ownership edge exists");
    assert_eq!(job_edge.to.kind, NodeKind::Job);
    assert_eq!(job_edge.to.key, "sbf/cache-warm");
    assert_eq!(
        graph.owner_of("sbf/cache-warm-jc7dd"),
        None,
        "a job is not a workload: DIAG-010's readiness expectation must not reach job pods"
    );

    // And the cronjob-spawned job chains one rung further.
    let cron_edge = graph
        .edges()
        .find(|edge| {
            edge.relation == EdgeRelation::OwnedBy && edge.from.key == "sbf/reindex-29301120"
        })
        .expect("the cronjob's job has an ownership edge");
    assert_eq!(cron_edge.to.kind, NodeKind::CronJob);
    assert_eq!(cron_edge.to.key, "sbf/reindex");

    let bare_pod = underived
        .iter()
        .find(|fact| fact.pod == "sbf/debug-shell")
        .expect("the bare pod must be an underived-owner fact");
    assert_eq!(bare_pod.reason, UnderivedReason::NoOwnerDeclared);
    assert_eq!(graph.owner_of("sbf/debug-shell"), None);
}

#[test]
fn on_a_bundle_without_replicasets_the_hash_fallback_derives_and_names_itself() {
    // The IW2 bundle format: no replicaset kind. The heuristic still closes the chain, and the
    // edge's site says `pod-template-hash` so nobody mistakes it for the declared mechanism.
    let bundle = serde_json::json!({
        "format": "infra-observation/1",
        "context": "fallback",
        "scanned_at": "2026-08-21T08:00:00Z",
        "scout_version": "0.1.0",
        "kinds": {
            "namespaces": { "items": [ { "metadata": { "name": "app", "uid": "ns-1" } } ] },
            "nodes": { "items": [] },
            "deployments": { "items": [
                { "metadata": { "name": "web", "namespace": "app", "uid": "d-1" },
                  "spec": { "replicas": 1,
                            "selector": { "matchLabels": { "app": "web" } },
                            "template": { "metadata": { "labels": { "app": "web" } },
                                          "spec": { "containers": [
                                              { "name": "web", "image": "web:1" } ] } } } }
            ] },
            "statefulsets": { "items": [] }, "daemonsets": { "items": [] },
            "services": { "items": [] }, "ingresses": { "items": [] },
            "configmaps": { "items": [] }, "secrets": { "items": [] },
            "serviceaccounts": { "items": [] }, "persistentvolumeclaims": { "items": [] },
            "pods": { "items": [
                { "metadata": { "name": "web-5d9c8b7a6f-aaaaa", "namespace": "app",
                                "uid": "p-1",
                                "labels": { "app": "web", "pod-template-hash": "5d9c8b7a6f" },
                                "ownerReferences": [ { "kind": "ReplicaSet",
                                                       "name": "web-5d9c8b7a6f",
                                                       "controller": true } ] },
                  "status": { "phase": "Running" } }
            ] }
        }
    });
    let raw: RawBundle = serde_json::from_value(bundle).expect("the bundle parses");
    let observation = Observation::try_from(raw).expect("the bundle is valid");
    let graph = InfraGraph::of(&infra_compiler::compile(&observation));

    assert_eq!(
        graph.owner_of("app/web-5d9c8b7a6f-aaaaa"),
        Some("app/deployment/web"),
        "the fallback still derives on an older bundle"
    );
    let edge = graph
        .edges()
        .find(|edge| edge.relation == EdgeRelation::OwnedBy)
        .expect("the fallback edge exists");
    assert_eq!(
        edge.sites,
        vec!["pod-template-hash".to_owned()],
        "the heuristic names itself on the edge"
    );
    assert!(
        graph.underived_owners().is_empty(),
        "nothing stayed underived"
    );
}

#[test]
fn a_pod_whose_scanned_replicaset_is_absent_or_deploymentless_is_handled_exactly() {
    // Replicasets ARE scanned here, so the heuristic must not run: a pod naming a replicaset
    // outside the scanned set is a typed fact naming the replicaset, and a pod whose observed
    // replicaset has no deployment derives to the replicaset with no workload and no fact.
    let bundle = serde_json::json!({
        "format": "infra-observation/1",
        "context": "exact",
        "scanned_at": "2026-08-21T08:00:00Z",
        "scout_version": "0.1.0",
        "kinds": {
            "namespaces": { "items": [ { "metadata": { "name": "app", "uid": "ns-1" } } ] },
            "nodes": { "items": [] }, "deployments": { "items": [] },
            "statefulsets": { "items": [] }, "daemonsets": { "items": [] },
            "services": { "items": [] }, "ingresses": { "items": [] },
            "configmaps": { "items": [] }, "secrets": { "items": [] },
            "serviceaccounts": { "items": [] }, "persistentvolumeclaims": { "items": [] },
            "replicasets": { "items": [
                { "metadata": { "name": "bare-rs", "namespace": "app", "uid": "rs-1" },
                  "spec": { "replicas": 1 } },
                { "metadata": { "name": "orphan-rs", "namespace": "app", "uid": "rs-2",
                                "ownerReferences": [ { "kind": "Deployment", "name": "gone",
                                                       "controller": true } ] },
                  "spec": { "replicas": 1 } }
            ] },
            "pods": { "items": [
                { "metadata": { "name": "ghost-pod", "namespace": "app", "uid": "p-1",
                                "labels": { "pod-template-hash": "5d9c8b7a6f" },
                                "ownerReferences": [ { "kind": "ReplicaSet",
                                                       "name": "ghost-5d9c8b7a6f",
                                                       "controller": true } ] },
                  "status": { "phase": "Running" } },
                { "metadata": { "name": "bare-rs-pod", "namespace": "app", "uid": "p-2",
                                "ownerReferences": [ { "kind": "ReplicaSet", "name": "bare-rs",
                                                       "controller": true } ] },
                  "status": { "phase": "Running" } },
                { "metadata": { "name": "orphan-rs-pod", "namespace": "app", "uid": "p-3",
                                "ownerReferences": [ { "kind": "ReplicaSet", "name": "orphan-rs",
                                                       "controller": true } ] },
                  "status": { "phase": "Running" } }
            ] }
        }
    });
    let raw: RawBundle = serde_json::from_value(bundle).expect("the bundle parses");
    let observation = Observation::try_from(raw).expect("the bundle is valid");
    let graph = InfraGraph::of(&infra_compiler::compile(&observation));

    let reasons: Vec<(&str, &UnderivedReason)> = graph
        .underived_owners()
        .iter()
        .map(|fact| (fact.pod.as_str(), &fact.reason))
        .collect();
    assert!(
        reasons.contains(&(
            "app/ghost-pod",
            &UnderivedReason::NoMatchingWorkload {
                kind: "replicaset".to_owned(),
                name: "ghost-5d9c8b7a6f".to_owned()
            }
        )),
        "with replicasets scanned, an absent one is the fact — not a hash derivation: {reasons:?}"
    );
    assert!(
        reasons.contains(&(
            "app/orphan-rs-pod",
            &UnderivedReason::NoMatchingWorkload {
                kind: "deployment".to_owned(),
                name: "gone".to_owned()
            }
        )),
        "an observed replicaset whose deployment is gone names the deployment: {reasons:?}"
    );
    assert!(
        !reasons.iter().any(|(pod, _)| *pod == "app/bare-rs-pod"),
        "a bare replicaset ends the chain legitimately; nothing is missing: {reasons:?}"
    );
    assert_eq!(
        graph.owner_of("app/bare-rs-pod"),
        None,
        "a bare replicaset is not a workload"
    );
}

#[test]
fn a_replicaset_whose_deployment_is_gone_and_a_hashless_pod_both_stay_underived() {
    // The two remaining reasons need a cluster the committed fixture does not model, so they
    // get a minimal bundle of their own.
    let bundle = serde_json::json!({
        "format": "infra-observation/1",
        "context": "underived",
        "scanned_at": "2026-08-21T08:00:00Z",
        "scout_version": "0.1.0",
        "kinds": {
            "namespaces": { "items": [ { "metadata": { "name": "app", "uid": "ns-1" } } ] },
            "nodes": { "items": [] }, "deployments": { "items": [] },
            "statefulsets": { "items": [] }, "daemonsets": { "items": [] },
            "services": { "items": [] }, "ingresses": { "items": [] },
            "configmaps": { "items": [] }, "secrets": { "items": [] },
            "serviceaccounts": { "items": [] }, "persistentvolumeclaims": { "items": [] },
            "pods": { "items": [
                { "metadata": { "name": "ghost-5d9c8b7a6f-aaaaa", "namespace": "app",
                                "uid": "p-1",
                                "labels": { "pod-template-hash": "5d9c8b7a6f" },
                                "ownerReferences": [ { "kind": "ReplicaSet",
                                                       "name": "ghost-5d9c8b7a6f",
                                                       "controller": true } ] },
                  "status": { "phase": "Running" } },
                { "metadata": { "name": "hashless-bbbbb", "namespace": "app", "uid": "p-2",
                                "ownerReferences": [ { "kind": "ReplicaSet",
                                                       "name": "hashless-c4d5e6f7a8",
                                                       "controller": true } ] },
                  "status": { "phase": "Running" } }
            ] }
        }
    });
    let raw: RawBundle = serde_json::from_value(bundle).expect("the bundle parses");
    let observation = Observation::try_from(raw).expect("the bundle is valid");
    let graph = InfraGraph::of(&infra_compiler::compile(&observation));

    let reasons: Vec<&UnderivedReason> = graph
        .underived_owners()
        .iter()
        .map(|fact| &fact.reason)
        .collect();
    assert!(
        reasons.contains(&&UnderivedReason::NoMatchingWorkload {
            kind: "deployment".to_owned(),
            name: "ghost".to_owned()
        }),
        "a derivation that lands on nothing observed is a typed fact naming what it derived: \
         {reasons:?}"
    );
    assert!(
        reasons.contains(&&UnderivedReason::TemplateHashUnderivable {
            name: "hashless-c4d5e6f7a8".to_owned()
        }),
        "a pod without the template-hash label derives nothing: {reasons:?}"
    );
}

#[test]
fn restricting_to_a_namespace_keeps_its_objects_their_edges_and_the_nodes_they_reach() {
    let ir = example_ir();
    let graph = InfraGraph::of(&ir).restricted_to("sbf");

    assert!(
        graph
            .nodes()
            .iter()
            .all(|node| node.namespace() != Some("kube-system")),
        "nothing from another namespace survives the restriction"
    );
    assert!(
        graph
            .nodes()
            .iter()
            .any(|node| node.kind == NodeKind::Node && node.key == "k3d-example-server-0"),
        "the cluster node an sbf pod is scheduled on is kept as an edge endpoint"
    );
    assert!(
        graph.edges().count() > 0,
        "the namespace has edges of its own"
    );
    assert!(
        graph
            .underived_owners()
            .iter()
            .all(|fact| fact.pod.starts_with("sbf/")),
        "underived-owner facts are filtered with their pods"
    );

    let nowhere = InfraGraph::of(&ir).restricted_to("never-observed");
    assert!(
        nowhere.nodes().is_empty() && nowhere.edge_count() == 0,
        "a namespace nobody observed holds nothing, and asking is not an error"
    );
}

#[test]
fn the_selector_edge_carries_the_selector_and_the_env_edge_carries_its_site() {
    let graph = InfraGraph::of(&example_ir());
    let selects = graph
        .edges()
        .find(|edge| {
            edge.relation == EdgeRelation::Selects && edge.to.key == "sbf/statefulset/asterisk"
        })
        .expect("a service selects the asterisk statefulset");
    assert!(
        selects.sites[0].starts_with("selector["),
        "the edge's evidence is the selector itself: {:?}",
        selects.sites
    );

    let reads = graph
        .edges()
        .find(|edge| {
            edge.relation == EdgeRelation::ReadsKeyOf
                && edge.from.key == "sbf/deployment/storefront-server"
        })
        .expect("storefront-server reads a key of its secret");
    assert!(
        reads
            .sites
            .iter()
            .any(|site| site.starts_with("containers[") && site.contains(".env[")),
        "the edge's evidence names the container and variable: {:?}",
        reads.sites
    );
}

#[test]
fn the_mermaid_rendering_groups_by_namespace_and_leaves_the_runtime_layer_to_the_json() {
    let ir = example_ir();
    let graph = InfraGraph::of(&ir);
    let mermaid = graph.mermaid();

    assert!(mermaid.starts_with("flowchart TB\n"), "{mermaid}");
    assert!(
        mermaid.contains("subgraph ns0[\"namespace kube-system\"]")
            && mermaid.contains("subgraph ns1[\"namespace sbf\"]"),
        "one subgraph per namespace, in name order: {mermaid}"
    );
    assert!(
        mermaid.contains("[\"deployment coredns\"]"),
        "a workload's label reads kind then name: {mermaid}"
    );
    assert!(
        !mermaid.contains("pod0") && !mermaid.contains("node0"),
        "pods and cluster nodes are the JSON document's, not the diagram's: {mermaid}"
    );
    assert!(
        mermaid.contains("-->|\"routes to\"|"),
        "edges are labelled with the relation's verb: {mermaid}"
    );
}

#[test]
fn the_json_document_chains_to_the_ir_it_was_built_from() {
    let ir = example_ir();
    let graph = InfraGraph::of(&ir);
    let document = GraphDocument::of(&graph, &ir, None);
    assert_eq!(document.format, "infra-graph/1");
    assert_eq!(
        document.source_digest,
        ir.digest(),
        "the graph names the exact model it explains"
    );
    assert_eq!(document.nodes.len(), graph.nodes().len());
    assert_eq!(document.edges.len(), graph.edge_count());
}
