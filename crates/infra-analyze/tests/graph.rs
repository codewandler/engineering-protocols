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
fn a_deployment_pod_is_owned_through_the_template_hash_without_any_replicaset_observed() {
    let graph = InfraGraph::of(&example_ir());
    assert_eq!(
        graph.owner_of("kube-system/coredns-ccb96694c-jkz7w"),
        Some("kube-system/deployment/coredns"),
        "the replicaset name minus the pod-template-hash is the deployment"
    );
    assert_eq!(
        graph.owner_of("sbf/asterisk-0"),
        Some("sbf/statefulset/asterisk"),
        "a statefulset pod names its workload directly"
    );
}

#[test]
fn a_job_pod_and_a_bare_pod_are_typed_facts_not_guesses() {
    let graph = InfraGraph::of(&example_ir());
    let underived = graph.underived_owners();

    let job_pod = underived
        .iter()
        .find(|fact| fact.pod == "sbf/cache-warm-jc7dd")
        .expect("the Job's pod must be an underived-owner fact");
    assert_eq!(
        job_pod.reason,
        UnderivedReason::KindOutsideModel {
            kind: "Job".to_owned()
        },
        "the reason names the kind the model does not hold"
    );

    let bare_pod = underived
        .iter()
        .find(|fact| fact.pod == "sbf/debug-shell")
        .expect("the bare pod must be an underived-owner fact");
    assert_eq!(bare_pod.reason, UnderivedReason::NoOwnerDeclared);

    // And neither got an ownership edge — a fact is instead of a guess, not beside one.
    assert_eq!(graph.owner_of("sbf/cache-warm-jc7dd"), None);
    assert_eq!(graph.owner_of("sbf/debug-shell"), None);
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
