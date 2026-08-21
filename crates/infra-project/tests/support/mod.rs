//! Shared fixtures: the committed example observation and specification, and small purpose-built
//! bundles for the cases the example deliberately does not carry.
//!
//! In a subdirectory so `cargo test` does not treat it as a test binary of its own. Each test
//! binary compiles the whole module and uses part of it, which is what the allowance below is
//! for — it is scoped to this file and to nothing that ships.

#![allow(dead_code)]

use std::path::{Path, PathBuf};

use infra_compiler::InfraIr;
use infra_domain::observation::Observation;
use infra_domain::raw::RawBundle;
use infra_spec::InfraSpec;

/// The repository root.
pub fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the workspace root exists")
}

/// A repository file's text.
pub fn read(relative: &str) -> String {
    let path = root().join(relative);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{} is readable: {error}", path.display()))
}

/// Compiles a bundle's text into an IR, refusing to guess at anything invalid.
pub fn compile(text: &str) -> InfraIr {
    let raw: RawBundle = serde_json::from_str(text).expect("the bundle is JSON");
    let observation = Observation::try_from(raw).expect("the bundle is a valid observation");
    infra_compiler::compile(&observation)
}

/// The committed example observation, compiled.
pub fn example_ir() -> InfraIr {
    compile(&read("examples/k3d-dev-cluster/observation.json"))
}

/// The committed example specification, validated.
pub fn example_spec() -> InfraSpec {
    infra_spec::read_spec(&read("examples/k3d-dev-cluster/expected.yaml"))
        .expect("the committed specification is valid")
}

/// Validates a specification written inline.
pub fn spec(body: &str) -> InfraSpec {
    infra_spec::read_spec(&format!(
        "format: infra-spec/1\nname: fixture\nexpectations:\n{body}"
    ))
    .expect("the inline specification is valid")
}

/// The twelve kinds every bundle must carry, so a hand-built fixture is a valid observation.
const REQUIRED_KINDS: &[&str] = &[
    "namespaces",
    "nodes",
    "deployments",
    "statefulsets",
    "daemonsets",
    "services",
    "ingresses",
    "configmaps",
    "secrets",
    "serviceaccounts",
    "persistentvolumeclaims",
    "pods",
];

/// Builds a minimal valid bundle around the kinds a test cares about.
///
/// Every kind not named arrives as an empty list — *observed and empty*, which is a different
/// claim from unscanned. A test about an unscanned kind simply does not name it.
pub fn bundle(context: &str, kinds: &[(&str, serde_json::Value)]) -> String {
    let mut map = serde_json::Map::new();
    for kind in REQUIRED_KINDS {
        map.insert(
            (*kind).to_owned(),
            serde_json::json!({ "items": serde_json::Value::Array(Vec::new()) }),
        );
    }
    for (kind, items) in kinds {
        map.insert(
            (*kind).to_owned(),
            serde_json::json!({ "items": items.clone() }),
        );
    }
    serde_json::to_string(&serde_json::json!({
        "format": "infra-observation/1",
        "context": context,
        "scanned_at": "2026-08-21T00:00:00Z",
        "scout_version": "0.1.0",
        "kinds": serde_json::Value::Object(map),
    }))
    .expect("the fixture serializes")
}

/// One namespace object.
pub fn namespace(name: &str) -> serde_json::Value {
    serde_json::json!({"metadata": {"name": name, "uid": format!("uid-ns-{name}")}})
}

/// One deployment, with whatever container list a test needs.
pub fn deployment(
    namespace: &str,
    name: &str,
    replicas: u32,
    containers: &serde_json::Value,
) -> serde_json::Value {
    serde_json::json!({
        "metadata": {"name": name, "namespace": namespace, "uid": format!("uid-{namespace}-{name}"),
                     "labels": {"app": name}},
        "spec": {
            "replicas": replicas,
            "selector": {"matchLabels": {"app": name}},
            "template": {"metadata": {"labels": {"app": name}}, "spec": {"containers": containers}},
        }
    })
}

/// One container with an image and nothing else — no probes, no resources.
pub fn container(name: &str, image: &str) -> serde_json::Value {
    serde_json::json!({"name": name, "image": image, "resources": {}})
}

/// One disruption budget.
pub fn budget(namespace: &str, name: &str, selector: &serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "metadata": {"name": name, "namespace": namespace,
                     "uid": format!("uid-pdb-{namespace}-{name}")},
        "spec": {"maxUnavailable": 1, "selector": {"matchLabels": selector}}
    })
}
