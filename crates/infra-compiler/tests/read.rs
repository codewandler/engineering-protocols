//! The read-back guarantees: a persisted document round-trips, and every way a document can
//! lie — edited content, a hand-written `resolved` claim, a foreign format — is refused with
//! its own code.

use infra_domain::observation::Observation;
use infra_domain::raw::RawBundle;
use infra_domain::InfraCode;

/// A bundle whose compilation mints every handle kind: node, service, configmap, secret,
/// service account and claim all have at least one resolved reference site.
fn bundle() -> serde_json::Value {
    serde_json::json!({
        "format": "infra-observation/1",
        "context": "read-back",
        "scanned_at": "2026-08-21T08:00:00Z",
        "scout_version": "0.1.0",
        "kinds": {
            "namespaces": { "items": [ { "metadata": { "name": "app", "uid": "ns-1" } } ] },
            "nodes": { "items": [ { "metadata": { "name": "node-a", "uid": "no-1" } } ] },
            "deployments": { "items": [
                { "metadata": { "name": "web", "namespace": "app", "uid": "d-1" },
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
                          ]
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
                                "labels": { "app": "web" } },
                  "spec": { "nodeName": "node-a" },
                  "status": { "phase": "Running", "containerStatuses": [
                      { "name": "main", "ready": true, "restartCount": 0 } ] } }
            ] },
            "services": { "items": [
                { "metadata": { "name": "web", "namespace": "app", "uid": "sv-1" },
                  "spec": { "selector": { "app": "web" },
                            "ports": [ { "port": 80 } ] } },
                { "metadata": { "name": "db-headless", "namespace": "app", "uid": "sv-2" },
                  "spec": { "clusterIP": "None", "selector": { "app": "db" },
                            "ports": [ { "port": 5432 } ] } }
            ] },
            "ingresses": { "items": [
                { "metadata": { "name": "edge", "namespace": "app", "uid": "i-1" },
                  "spec": { "rules": [ { "host": "web.test", "http": { "paths": [
                      { "path": "/", "pathType": "Prefix",
                        "backend": { "service": { "name": "web",
                                                  "port": { "number": 80 } } } }
                  ] } } ] } }
            ] },
            "configmaps": { "items": [
                { "metadata": { "name": "settings", "namespace": "app", "uid": "c-1" },
                  "data": { "mode": "fast" } }
            ] },
            "secrets": { "items": [
                { "metadata": { "name": "creds", "namespace": "app", "uid": "se-1" },
                  "type": "Opaque",
                  "data": { "token": {
                      "sha256": "8a94462377096e0657f57b6e6bc0e29000464398727091d7863726ce50974968",
                      "length": 12 } } }
            ] },
            "serviceaccounts": { "items": [
                { "metadata": { "name": "runner", "namespace": "app", "uid": "sa-1" } },
                { "metadata": { "name": "default", "namespace": "app", "uid": "sa-2" } }
            ] },
            "persistentvolumeclaims": { "items": [
                { "metadata": { "name": "data", "namespace": "app", "uid": "pv-1" },
                  "spec": { "accessModes": ["ReadWriteOnce"] },
                  "status": { "phase": "Bound" } }
            ] }
        }
    })
}

fn compiled() -> infra_compiler::InfraIr {
    let raw: RawBundle = serde_json::from_value(bundle()).expect("the bundle parses");
    let observation = Observation::try_from(raw).expect("the fixture is valid");
    infra_compiler::compile(&observation)
}

/// The persisted document as a JSON value, exactly as `compile --out` writes it.
fn persisted() -> serde_json::Value {
    let ir = compiled();
    serde_json::to_value(ir.document()).expect("the document serializes")
}

/// Recomputes the document's digest over its (possibly edited) model — what a *dishonest*
/// producer would do, used here to prove the relational check does not hide behind the digest.
fn restamp_digest(document: &mut serde_json::Value) {
    let canonical = serde_json::to_vec(&document["model"]).expect("the model serializes");
    document["digest"] = serde_json::json!(infra_compiler::digest_of_canonical(&canonical));
}

#[test]
fn a_persisted_document_reads_back_into_the_identical_ir() {
    let original = compiled();
    let read = infra_compiler::read_document(&persisted())
        .expect("what the compiler wrote must read back");
    assert_eq!(
        read, original,
        "the round trip must lose nothing and invent nothing"
    );
    assert_eq!(
        read.digest(),
        original.digest(),
        "and the two must address the same content"
    );
}

#[test]
fn the_fixture_mints_every_handle_kind_or_the_round_trip_proves_too_little() {
    // The state where the re-minting is load-bearing: one resolved reference per handle kind.
    let text = serde_json::to_string(&persisted()).expect("serializes");
    for (kind, key) in [
        ("node", "node-a"),
        ("service", "app/db-headless"),
        ("configmap", "app/settings"),
        ("secret", "app/creds"),
        ("service account", "app/runner"),
        ("claim", "app/data"),
    ] {
        // Through `Value`, object keys are sorted, so `key` precedes `state` in the text.
        assert!(
            text.contains(&format!("\"key\":\"{key}\",\"state\":\"resolved\"")),
            "no resolved {kind} reference for `{key}` — the fixture stopped exercising it"
        );
    }
}

#[test]
fn an_edited_document_is_refused_for_its_digest() {
    let mut document = persisted();
    document["model"]["workloads"]["app/deployment/web"]["replicas"] = serde_json::json!(7);
    let errors = infra_compiler::read_document(&document)
        .expect_err("an edited document must not read back");
    assert!(
        errors.contains(InfraCode::IrDigestMismatch),
        "expected INFRA-IR-002, got: {errors}"
    );
}

#[test]
fn a_hand_written_resolved_claim_is_refused_even_when_its_digest_is_freshly_stamped() {
    // The attacker recomputes the digest, so INFRA-IR-002 cannot fire; only the relational
    // check stands between this document and a panicking total lookup.
    let mut document = persisted();
    document["model"]["workloads"]["app/deployment/web"]["containers"][0]["env"][0]["source"]
        ["config_map"] = serde_json::json!({ "state": "resolved", "key": "app/no-such-map" });
    restamp_digest(&mut document);
    let errors = infra_compiler::read_document(&document)
        .expect_err("a dangling resolved claim must not read back");
    assert!(
        errors.contains(InfraCode::IrDanglingHandle),
        "expected INFRA-IR-004, got: {errors}"
    );
    assert!(
        !errors.contains(InfraCode::IrDigestMismatch),
        "the digest was honestly restamped; the refusal must come from the relational check \
         alone: {errors}"
    );
}

#[test]
fn an_edited_document_with_a_dangling_claim_reports_both_defects_in_one_run() {
    let mut document = persisted();
    document["model"]["pods"]["app/web-1"]["node"] =
        serde_json::json!({ "state": "resolved", "key": "node-gone" });
    let errors = infra_compiler::read_document(&document)
        .expect_err("an edited document with a dangling claim must not read back");
    assert!(
        errors.contains(InfraCode::IrDigestMismatch)
            && errors.contains(InfraCode::IrDanglingHandle),
        "validation accumulates: both INFRA-IR-002 and INFRA-IR-004 arrive together, got: {errors}"
    );
}

#[test]
fn a_foreign_format_is_refused_before_anything_else_is_believed() {
    let mut document = persisted();
    document["format"] = serde_json::json!("infra-ir/2");
    let errors = infra_compiler::read_document(&document).expect_err("a foreign format");
    assert!(
        errors.contains(InfraCode::IrUnsupportedFormat),
        "expected INFRA-IR-001, got: {errors}"
    );
}

#[test]
fn a_document_that_does_not_read_as_the_shape_is_refused_as_malformed() {
    let mut document = persisted();
    document["model"]["pods"] = serde_json::json!([1, 2, 3]);
    restamp_digest(&mut document);
    let errors = infra_compiler::read_document(&document).expect_err("not the shape");
    assert!(
        errors.contains(InfraCode::IrMalformed),
        "expected INFRA-IR-003, got: {errors}"
    );
}
