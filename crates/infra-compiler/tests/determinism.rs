//! The determinism claims, asserted rather than stated (invariant 9).
//!
//! Three properties, each with the mutation that would break it named in its test:
//!
//! 1. compiling the same observation twice yields byte-identical documents;
//! 2. a bundle whose `kinds` object or item lists arrive in another order yields the identical
//!    IR — order of observation is not semantic state;
//! 3. editing `scanned_at` (or `context`, or `scout_version`) changes provenance and not the
//!    digest — two scans of an unchanged cluster address the same content.
//!
//! Plus the source scan that keeps unordered maps and clocks out of the crate, in the
//! comment-skipping, boundary-aware form `ess-gen/tests/determinism.rs` argues for.

use std::path::Path;

use infra_domain::observation::Observation;
use infra_domain::raw::RawBundle;

/// A bundle exercising every kind: two namespaces, a node, a deployment with configuration
/// references, a statefulset, services (one selecting nothing), an ingress, and the stores.
/// Long because it is data, not logic.
#[allow(clippy::too_many_lines)]
fn bundle() -> serde_json::Value {
    serde_json::json!({
        "format": "infra-observation/1",
        "context": "test-cluster",
        "scanned_at": "2026-08-20T22:30:30Z",
        "scout_version": "0.1.0",
        "kinds": {
            "namespaces": { "items": [
                { "metadata": { "name": "app", "uid": "ns-1" } },
                { "metadata": { "name": "infra", "uid": "ns-2" } }
            ] },
            "nodes": { "items": [
                { "metadata": { "name": "node-a", "uid": "no-1" },
                  "status": { "capacity": { "cpu": "8", "memory": "32Gi" },
                              "nodeInfo": { "architecture": "amd64" } } }
            ] },
            "deployments": { "items": [
                // Two deployments, each dangling: reversing this list must flip the order
                // their facts are *pushed* in, so a compiler that stopped sorting them shows.
                { "metadata": { "name": "api", "namespace": "app", "uid": "d-0" },
                  "spec": {
                    "replicas": 1,
                    "selector": { "matchLabels": { "app": "api" } },
                    "template": { "metadata": { "labels": { "app": "api" } }, "spec": {
                      "containers": [ { "name": "api", "image": "api:1",
                        "envFrom": [ { "configMapRef": { "name": "also-vanished" } } ] } ]
                    } }
                  } },
                { "metadata": { "name": "web", "namespace": "app", "uid": "d-1",
                                "labels": { "app": "web" } },
                  "spec": {
                    "replicas": 2,
                    "selector": { "matchLabels": { "app": "web" } },
                    "template": {
                      "metadata": { "labels": { "app": "web" } },
                      "spec": {
                        "serviceAccountName": "runner",
                        "containers": [{
                          "name": "main", "image": "web:1",
                          "env": [
                            { "name": "MODE", "valueFrom": { "configMapKeyRef": {
                                "name": "settings", "key": "mode" } } },
                            { "name": "TOKEN", "valueFrom": { "secretKeyRef": {
                                "name": "creds", "key": "token" } } }
                          ],
                          "envFrom": [ { "configMapRef": { "name": "vanished" } } ]
                        }],
                        "volumes": [
                          { "name": "state", "persistentVolumeClaim": { "claimName": "data" } }
                        ]
                      }
                    }
                  } }
            ] },
            "statefulsets": { "items": [
                { "metadata": { "name": "db", "namespace": "app", "uid": "s-1" },
                  "spec": {
                    "replicas": 1,
                    "serviceName": "db-headless",
                    "selector": { "matchLabels": { "app": "db" } },
                    "template": { "metadata": { "labels": { "app": "db" } }, "spec": {
                      "containers": [ { "name": "db", "image": "db:2" } ]
                    } }
                  } }
            ] },
            "daemonsets": { "items": [] },
            "pods": { "items": [
                { "metadata": { "name": "web-1", "namespace": "app", "uid": "p-1",
                                "labels": { "app": "web" },
                                "ownerReferences": [ { "kind": "ReplicaSet", "name": "web-abc",
                                                       "controller": true } ] },
                  "spec": { "nodeName": "node-a" },
                  "status": { "phase": "Running", "containerStatuses": [
                      { "name": "main", "ready": true, "restartCount": 1 } ] } },
                { "metadata": { "name": "web-2", "namespace": "app", "uid": "p-2",
                                "labels": { "app": "web" } },
                  "spec": { "nodeName": "node-gone" },
                  "status": { "phase": "Pending" } }
            ] },
            "services": { "items": [
                { "metadata": { "name": "web", "namespace": "app", "uid": "sv-1" },
                  "spec": { "type": "ClusterIP", "selector": { "app": "web" },
                            "ports": [ { "port": 80, "targetPort": 8080 } ] } },
                { "metadata": { "name": "orphan", "namespace": "app", "uid": "sv-2" },
                  "spec": { "selector": { "app": "ghost" },
                            "ports": [ { "port": 9090 } ] } }
            ] },
            "ingresses": { "items": [
                { "metadata": { "name": "edge", "namespace": "app", "uid": "i-1" },
                  "spec": { "rules": [ { "host": "web.test", "http": { "paths": [
                      { "path": "/", "pathType": "Prefix",
                        "backend": { "service": { "name": "web",
                                                  "port": { "number": 80 } } } },
                      { "path": "/old", "pathType": "Prefix",
                        "backend": { "service": { "name": "retired",
                                                  "port": { "number": 80 } } } }
                  ] } } ] } }
            ] },
            "configmaps": { "items": [
                { "metadata": { "name": "settings", "namespace": "app", "uid": "c-1" },
                  "data": { "mode": "fast", "verbose": "false" } }
            ] },
            "secrets": { "items": [
                { "metadata": { "name": "creds", "namespace": "app", "uid": "se-1" },
                  "type": "Opaque",
                  "data": { "password": {
                      "sha256": "8a94462377096e0657f57b6e6bc0e29000464398727091d7863726ce50974968",
                      "length": 12 } } }
            ] },
            "serviceaccounts": { "items": [
                { "metadata": { "name": "default", "namespace": "app", "uid": "sa-1" } }
            ] },
            "persistentvolumeclaims": { "items": [
                { "metadata": { "name": "data", "namespace": "app", "uid": "pv-1" },
                  "spec": { "storageClassName": "local-path",
                            "accessModes": ["ReadWriteOnce"],
                            "resources": { "requests": { "storage": "1Gi" } } } }
            ] }
        }
    })
}

fn observe(value: &serde_json::Value) -> Observation {
    let raw: RawBundle = serde_json::from_value(value.clone()).expect("the bundle parses");
    Observation::try_from(raw).expect("the fixture is valid")
}

fn document_bytes(observation: &Observation) -> String {
    let ir = infra_compiler::compile(observation);
    serde_json::to_string_pretty(&ir.document()).expect("the document serializes")
}

#[test]
fn compiling_the_same_observation_twice_yields_byte_identical_documents() {
    let observation = observe(&bundle());
    let first = document_bytes(&observation);
    let second = document_bytes(&observation);
    assert_eq!(
        first, second,
        "two compilations of one observation must not differ in a single byte"
    );
}

#[test]
fn a_bundle_with_reordered_kinds_and_reordered_item_lists_compiles_to_the_identical_ir() {
    // Mutation this must catch: any map keyed by arrival instead of identity, or the
    // `facts.sort()` in `compile` removed — pods arrive reversed here, so their node-reference
    // facts are pushed in the opposite order.
    let ordered = bundle();
    let mut reordered = serde_json::Map::new();
    // Reverse the key order of `kinds` itself…
    let kinds = ordered["kinds"].as_object().expect("kinds is an object");
    for (kind, list) in kinds.iter().rev() {
        let mut list = list.clone();
        // …and the items inside every kind.
        if let Some(items) = list.get_mut("items").and_then(|items| items.as_array_mut()) {
            items.reverse();
        }
        reordered.insert(kind.clone(), list);
    }
    let mut shuffled = ordered.clone();
    shuffled["kinds"] = serde_json::Value::Object(reordered);

    // The reordering must actually have happened, or this test guards nothing.
    assert_ne!(
        serde_json::to_string(&ordered).expect("serializes"),
        serde_json::to_string(&shuffled).expect("serializes"),
        "the fixture must present a genuinely different byte order"
    );
    // The state where the sort is load-bearing must actually be reached: at least two facts
    // from the same push phase (two dangling deployments), or removing `facts.sort()` would
    // pass this test while breaking the property.
    let ir = infra_compiler::compile(&observe(&ordered));
    let workload_facts = ir
        .model
        .unresolved
        .iter()
        .filter(|fact| fact.from.starts_with("workloads/"))
        .count();
    assert!(
        workload_facts >= 2,
        "the fixture must dangle from two workloads, found {workload_facts}"
    );
    assert_eq!(
        document_bytes(&observe(&ordered)),
        document_bytes(&observe(&shuffled)),
        "the order a scanner happens to emit objects in is not semantic state"
    );
}

#[test]
fn editing_scanned_at_changes_provenance_and_not_the_digest() {
    // Mutation this must catch: `digest()` hashing the document (provenance included) instead
    // of the model, or any provenance field migrating into `InfraModel`.
    let mut rescanned = bundle();
    rescanned["scanned_at"] = serde_json::json!("2026-08-21T09:00:00Z");
    rescanned["scout_version"] = serde_json::json!("0.2.0");
    rescanned["context"] = serde_json::json!("same-cluster-other-kubeconfig-name");

    let first = infra_compiler::compile(&observe(&bundle()));
    let second = infra_compiler::compile(&observe(&rescanned));
    assert_ne!(
        first.provenance, second.provenance,
        "the fixture must actually differ in provenance"
    );
    assert_eq!(
        first.digest(),
        second.digest(),
        "two scans of an unchanged cluster must address the same content"
    );
}

#[test]
fn the_digest_is_the_full_sha256_all_64_hex_characters() {
    let digest = infra_compiler::compile(&observe(&bundle())).digest();
    assert_eq!(digest.len(), 64, "no truncation: {digest}");
    assert!(
        digest.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "hex only: {digest}"
    );
}

#[test]
fn a_semantic_change_does_change_the_digest() {
    // The inverse guard: a digest that never changes would pass every stability test above.
    let mut scaled = bundle();
    scaled["kinds"]["deployments"]["items"][0]["spec"]["replicas"] = serde_json::json!(3);
    assert_ne!(
        infra_compiler::compile(&observe(&bundle())).digest(),
        infra_compiler::compile(&observe(&scaled)).digest(),
        "a replica change is semantic and must move the digest"
    );
}

/// What a deterministic crate must not mention in code.
const BANNED: &[&str] = &[
    "HashMap",
    "HashSet",
    "SystemTime",
    "Instant::now",
    "rand::",
    "getrandom",
    "thread_rng",
];

/// Every banned token `text` uses in code, as `(line number, token)`.
fn banned_uses(text: &str) -> Vec<(usize, &'static str)> {
    let mut found = Vec::new();
    for (number, line) in text.lines().enumerate() {
        if line.trim().starts_with("//") {
            continue;
        }
        for token in BANNED {
            let mut from = 0;
            while let Some(at) = line[from..].find(token) {
                let start = from + at;
                let boundary = line[..start]
                    .chars()
                    .next_back()
                    .is_none_or(|before| !before.is_alphanumeric() && before != '_');
                if boundary {
                    found.push((number + 1, *token));
                }
                from = start + token.len();
            }
        }
    }
    found
}

#[test]
fn the_compiler_uses_no_unordered_map_and_reads_no_clock() {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut checked = 0;
    let mut violations = Vec::new();
    for entry in std::fs::read_dir(&directory).expect("the crate has sources") {
        let path = entry.expect("an entry").path();
        if path.extension().is_none_or(|it| it != "rs") {
            continue;
        }
        let text = std::fs::read_to_string(&path).expect("a readable source file");
        for (line, token) in banned_uses(&text) {
            violations.push(format!("{}:{line}: `{token}`", path.display()));
        }
        checked += 1;
    }
    assert!(
        checked >= 3,
        "only {checked} source files were read; the scan is looking in the wrong place"
    );
    assert!(
        violations.is_empty(),
        "this crate promises byte-identical output for the same observation, and these lines \
         can break that between two runs:\n{}",
        violations.join("\n")
    );
}

#[test]
fn the_scan_sees_a_real_violation_and_ignores_prose_and_substrings() {
    assert_eq!(
        banned_uses("use std::collections::HashMap;"),
        vec![(1, "HashMap")],
        "a real use must trip the scan"
    );
    assert!(
        banned_uses("// a HashMap here would wobble the digest").is_empty(),
        "a comment about the rule must not trip it"
    );
}
