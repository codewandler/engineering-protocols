//! What resolves, what dangles, and how each is carried.
//!
//! The property under test is the split the IR's module doc states: a resolved reference is a
//! handle with a total lookup, and a dangling one is a typed fact the IR carries openly — never
//! an error, because an observed cluster legitimately contains them.

use infra_compiler::{
    InfraIr, Reference, ResolvedEnvFromSource, ResolvedEnvSource, ResolvedVolumeSource,
    UnresolvedTarget,
};
use infra_domain::observation::Observation;
use infra_domain::raw::RawBundle;

/// A bundle with one of everything and four deliberate danglings: a vanished configmap
/// (`envFrom`), a missing configmap *key*, a service selecting nothing, and an ingress path to
/// a retired service.
fn compiled() -> InfraIr {
    let bundle = serde_json::json!({
        "format": "infra-observation/1",
        "context": "test",
        "scanned_at": "2026-08-20T22:30:30Z",
        "scout_version": "0.1.0",
        "kinds": {
            "namespaces": { "items": [ { "metadata": { "name": "app", "uid": "ns-1" } } ] },
            "nodes": { "items": [ { "metadata": { "name": "node-a", "uid": "no-1" } } ] },
            "deployments": { "items": [
                { "metadata": { "name": "web", "namespace": "app", "uid": "d-1" },
                  "spec": {
                    "replicas": 1,
                    "selector": { "matchLabels": { "app": "web" } },
                    "template": {
                      "metadata": { "labels": { "app": "web" } },
                      "spec": {
                        "containers": [{
                          "name": "main", "image": "web:1",
                          "env": [
                            { "name": "MODE", "valueFrom": { "configMapKeyRef": {
                                "name": "settings", "key": "mode" } } },
                            { "name": "GONE", "valueFrom": { "configMapKeyRef": {
                                "name": "settings", "key": "no-such-key" } } },
                            { "name": "TOKEN", "valueFrom": { "secretKeyRef": {
                                "name": "creds", "key": "token", "optional": true } } }
                          ],
                          "envFrom": [ { "configMapRef": { "name": "vanished" } } ]
                        }],
                        "volumes": [
                          { "name": "state", "persistentVolumeClaim": { "claimName": "data" } },
                          { "name": "cfg", "configMap": { "name": "settings" } }
                        ]
                      }
                    }
                  } }
            ] },
            "statefulsets": { "items": [] },
            "daemonsets": { "items": [] },
            "pods": { "items": [
                { "metadata": { "name": "web-1", "namespace": "app", "uid": "p-1",
                                "labels": { "app": "web" } },
                  "spec": { "nodeName": "node-a" },
                  "status": { "phase": "Running", "containerStatuses": [
                      { "name": "main", "ready": true, "restartCount": 0 } ] } }
            ] },
            "services": { "items": [
                { "metadata": { "name": "web", "namespace": "app", "uid": "sv-1" },
                  "spec": { "selector": { "app": "web" }, "ports": [ { "port": 80 } ] } },
                { "metadata": { "name": "orphan", "namespace": "app", "uid": "sv-2" },
                  "spec": { "selector": { "app": "ghost" }, "ports": [ { "port": 1 } ] } }
            ] },
            "ingresses": { "items": [
                { "metadata": { "name": "edge", "namespace": "app", "uid": "i-1" },
                  "spec": { "rules": [ { "host": "web.test", "http": { "paths": [
                      { "path": "/", "backend": { "service": { "name": "web" } } },
                      { "path": "/old", "backend": { "service": { "name": "retired" } } }
                  ] } } ] } }
            ] },
            "configmaps": { "items": [
                { "metadata": { "name": "settings", "namespace": "app", "uid": "c-1" },
                  "data": { "mode": "fast" } }
            ] },
            "secrets": { "items": [
                { "metadata": { "name": "creds", "namespace": "app", "uid": "se-1" },
                  "data": { "token": {
                      "sha256": "8a94462377096e0657f57b6e6bc0e29000464398727091d7863726ce50974968",
                      "length": 12 } } }
            ] },
            "serviceaccounts": { "items": [
                { "metadata": { "name": "default", "namespace": "app", "uid": "sa-1" } }
            ] },
            "persistentvolumeclaims": { "items": [
                { "metadata": { "name": "data", "namespace": "app", "uid": "pv-1" },
                  "spec": {} }
            ] }
        }
    });
    let raw: RawBundle = serde_json::from_value(bundle).expect("the bundle parses");
    let observation = Observation::try_from(raw).expect("the fixture is valid");
    infra_compiler::compile(&observation)
}

#[test]
fn a_dangling_reference_is_carried_as_a_fact_and_never_refuses_compilation() {
    let ir = compiled();
    // The four deliberate danglings, and nothing else: an IR that also flagged healthy
    // references would bury IW2 in noise.
    let targets: Vec<&UnresolvedTarget> = ir
        .model
        .unresolved
        .iter()
        .map(|fact| &fact.target)
        .collect();
    assert_eq!(
        targets.len(),
        4,
        "exactly the four deliberate danglings: {:#?}",
        ir.model.unresolved
    );
    assert!(
        targets.iter().any(|target| matches!(target,
            UnresolvedTarget::ConfigMap { name, optional: false } if name == "vanished")),
        "the vanished envFrom configmap is a fact"
    );
    assert!(
        targets.iter().any(|target| matches!(target,
            UnresolvedTarget::ConfigMapKey { name, key, .. }
                if name == "settings" && key == "no-such-key")),
        "a key missing from an observed configmap is its own kind of fact"
    );
    assert!(
        targets.iter().any(|target| matches!(target,
            UnresolvedTarget::Service { name } if name == "retired")),
        "the ingress path to a retired service is a fact"
    );
    assert!(
        targets.iter().any(|target| matches!(target,
            UnresolvedTarget::PodsMatchingSelector { selector }
                if selector.get("app").map(String::as_str) == Some("ghost"))),
        "a selector matching no pod is a fact, not an error"
    );
}

#[test]
fn a_resolved_reference_is_a_handle_whose_lookup_is_total() {
    let ir = compiled();
    let workload = &ir.model.workloads["app/deployment/web"];
    let ResolvedEnvSource::ConfigMapKey {
        config_map: Reference::Resolved { key },
        ..
    } = &workload.containers[0].env[0].source
    else {
        panic!("MODE reads an observed configmap and must resolve");
    };
    // No `Option`, no second lookup that could disagree: the handle answers.
    let settings = ir.config_map(key);
    assert!(
        settings.keys.contains_key("mode"),
        "the handle leads to the configmap that holds the key"
    );
}

#[test]
fn the_unresolved_site_keeps_the_declared_name_so_the_ir_reads_on_its_own() {
    let ir = compiled();
    let workload = &ir.model.workloads["app/deployment/web"];
    let ResolvedEnvFromSource::ConfigMap {
        config_map: Reference::Unresolved { name },
        ..
    } = &workload.containers[0].env_from[0].source
    else {
        panic!("the vanished configmap cannot resolve");
    };
    assert_eq!(name, "vanished", "the site names what the cluster declared");
}

#[test]
fn an_absent_service_account_name_resolves_as_default_because_that_is_what_the_kubelet_does() {
    let ir = compiled();
    let workload = &ir.model.workloads["app/deployment/web"];
    let Reference::Resolved { key } = &workload.service_account else {
        panic!("`default` exists in the fixture and must resolve");
    };
    assert_eq!(ir.service_account(key).identity.name, "default");
}

#[test]
fn volume_claims_and_optional_secret_references_resolve_with_their_flags_kept() {
    let ir = compiled();
    let workload = &ir.model.workloads["app/deployment/web"];
    assert!(
        matches!(
            &workload.volumes[0].source,
            ResolvedVolumeSource::Claim {
                claim: Reference::Resolved { .. }
            }
        ),
        "the claim exists and resolves"
    );
    let ResolvedEnvSource::SecretKey {
        secret: Reference::Resolved { .. },
        optional,
        ..
    } = &workload.containers[0].env[2].source
    else {
        panic!("the secret exists and must resolve");
    };
    assert!(optional, "the declared optionality survives into the IR");
}
