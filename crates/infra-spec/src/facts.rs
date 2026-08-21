//! The workload fact sheet the predicate escape hatch reads — and, for every fact that is *not*
//! there, why.
//!
//! # Reusing the evaluator means reusing its `Unknown`
//!
//! [`Predicate::evaluate`](aep_domain::predicate::Predicate::evaluate) already returns
//! [`Truth::Unknown`](aep_domain::predicate::Truth::Unknown) when a fact it reads has no value.
//! That is the whole mechanism: this module decides **which facts a snapshot can state about a
//! workload**, and every fact it declines to state becomes an honest `Unknown` in the evaluator
//! that already exists. Nothing here re-implements three-valued logic, and nothing here can
//! collapse the third value, because there is no boolean anywhere in the path.
//!
//! What this module adds is the sentence the report needs beside the verdict. A bare "unknown"
//! sends a reader to the cluster; "the bundle did not scan disruption budgets" sends them to the
//! scanner. So a projection is a pair: the [`FactStore`] the evaluator reads, and a map from each
//! *withheld* path to the [`UnknownReason`] it was withheld for.
//!
//! # Why a withheld fact is not a zero
//!
//! Every absence here is a case where writing a number would be a claim nobody made:
//!
//! | fact | withheld when | writing a number instead would claim |
//! |---|---|---|
//! | `workload.replicas` | the workload is a daemonset | that somebody declared a replica count |
//! | `workload.observed_pods`, `workload.ready_pods` | a pod in the same namespace has an underivable controller | that the counted pods are all of them |
//! | `workload.pdb_count`, `workload.hpa_count` | the bundle did not scan that kind | that none exist |
//!
//! The middle row is the one worth arguing for. Pod ownership is derived
//! ([`InfraGraph`]), and an underived pod is a pod that *might* belong
//! to any workload in its namespace. Counting the derivable ones and calling the result the
//! workload's pod count turns a lower bound into a measurement, and a predicate like
//! `workload.ready_pods >= 2` would then read `False` on a cluster where it may well be true.

use std::collections::BTreeMap;

use aep_domain::facts::{FactPath, FactStore, FactValue};
use infra_analyze::{properties_with, InfraGraph, WorkloadProperties};
use infra_compiler::InfraIr;
use infra_domain::workload::WorkloadKind;

use crate::simulate::UnknownReason;

/// Every fact path a workload projection can carry, in the order they are documented.
///
/// The closed vocabulary of the escape hatch, and the list validation checks a predicate's paths
/// against (`INFRA-SPEC-008`). A path that is not here is a typo or a fact nobody projects, and
/// both are better refused than silently `Unknown` forever.
pub const WORKLOAD_FACTS: &[&str] = &[
    "workload.kind",
    "workload.namespace",
    "workload.name",
    "workload.service_account",
    "workload.replicas",
    "workload.containers",
    "workload.containers_with_requests",
    "workload.containers_with_limits",
    "workload.containers_with_liveness",
    "workload.containers_with_readiness",
    "workload.containers_digest_pinned",
    "workload.containers_latest_tag",
    "workload.observed_pods",
    "workload.ready_pods",
    "workload.pdb_count",
    "workload.hpa_count",
    "workload.unresolved_references",
    "workload.required_unresolved_references",
];

/// One workload's facts, and the reason behind every fact that is missing.
#[derive(Debug, Clone)]
pub struct WorkloadFacts {
    /// The workload's IR key, `namespace/kind/name`.
    pub workload: String,
    /// What the snapshot can state.
    pub store: FactStore,
    /// What it cannot, and why — keyed by the path, so a predicate's own
    /// [`fact_paths`](aep_domain::predicate::Predicate::fact_paths) look the reason up directly.
    pub withheld: BTreeMap<String, UnknownReason>,
}

/// Projects every workload of a snapshot into its fact sheet, keyed by IR key.
///
/// Takes the graph rather than building one, because the caller already needs it: the derived
/// ownership that decides `observed_pods` is the same walk `properties_with` reads.
#[must_use]
pub fn workload_facts(ir: &InfraIr, graph: &InfraGraph) -> BTreeMap<String, WorkloadFacts> {
    let ownership_blind = namespaces_with_underived_pods(ir, graph);
    properties_with(ir, graph)
        .into_iter()
        .map(|properties| {
            let facts = project(ir, &properties, &ownership_blind);
            (properties.workload.clone(), facts)
        })
        .collect()
}

/// The namespaces holding at least one pod whose controller could not be derived, with the reason
/// the first such pod gave.
///
/// Namespace-wide rather than pod-wide on purpose: an underived pod could belong to any workload
/// in its namespace, so it is every one of their counts that stops being a count.
fn namespaces_with_underived_pods(
    ir: &InfraIr,
    graph: &InfraGraph,
) -> BTreeMap<String, UnknownReason> {
    let mut blind = BTreeMap::new();
    for underived in graph.underived_owners() {
        let Some(pod) = ir.model.pods.get(&underived.pod) else {
            continue;
        };
        let Some(namespace) = pod.identity.namespace.clone() else {
            continue;
        };
        blind
            .entry(namespace)
            .or_insert_with(|| UnknownReason::OwnershipUnderivable {
                pod: underived.pod.clone(),
            });
    }
    blind
}

/// Builds one workload's fact sheet.
fn project(
    ir: &InfraIr,
    properties: &WorkloadProperties,
    ownership_blind: &BTreeMap<String, UnknownReason>,
) -> WorkloadFacts {
    let mut store = FactStore::new();
    let mut withheld = BTreeMap::new();
    let workload = ir
        .model
        .workloads
        .get(&properties.workload)
        .expect("properties are extracted from the model's own workloads");
    let namespace = workload.identity.namespace.clone().unwrap_or_default();

    store.set_path("workload.kind", FactValue::text(properties.kind.as_str()));
    store.set_path("workload.namespace", FactValue::text(namespace.clone()));
    store.set_path(
        "workload.name",
        FactValue::text(workload.identity.name.clone()),
    );
    store.set_path(
        "workload.service_account",
        FactValue::text(reference_name(&workload.service_account)),
    );

    match properties.replicas {
        Some(replicas) => store.set_path("workload.replicas", FactValue::count(replicas as usize)),
        None => {
            withheld.insert(
                "workload.replicas".to_owned(),
                UnknownReason::FieldAbsent {
                    subject: properties.workload.clone(),
                    field: "replicas".to_owned(),
                },
            );
        }
    }

    container_counts(ir, properties, &mut store);

    if let Some(reason) = ownership_blind.get(&namespace) {
        withheld.insert("workload.observed_pods".to_owned(), reason.clone());
        withheld.insert("workload.ready_pods".to_owned(), reason.clone());
    } else {
        store.set_path(
            "workload.observed_pods",
            FactValue::count(properties.observed_pods as usize),
        );
        store.set_path(
            "workload.ready_pods",
            FactValue::count(properties.ready_pods as usize),
        );
    }

    optional_count(
        &mut store,
        &mut withheld,
        "workload.pdb_count",
        "poddisruptionbudgets",
        properties.pod_disruption_budgets.as_ref(),
    );
    optional_count(
        &mut store,
        &mut withheld,
        "workload.hpa_count",
        "horizontalpodautoscalers",
        properties.horizontal_pod_autoscalers.as_ref(),
    );

    let prefix = format!("workloads/{}", properties.workload);
    let unresolved: Vec<_> = ir
        .model
        .unresolved
        .iter()
        .filter(|reference| reference.from == prefix)
        .collect();
    store.set_path(
        "workload.unresolved_references",
        FactValue::count(unresolved.len()),
    );
    store.set_path(
        "workload.required_unresolved_references",
        FactValue::count(
            unresolved
                .iter()
                .filter(|reference| crate::simulate::is_required(&reference.target))
                .count(),
        ),
    );

    WorkloadFacts {
        workload: properties.workload.clone(),
        store,
        withheld,
    }
}

/// The six per-container counts, which are always statable: a container list is in the snapshot
/// or the workload is not.
fn container_counts(ir: &InfraIr, properties: &WorkloadProperties, store: &mut FactStore) {
    let containers = &properties.containers;
    store.set_path("workload.containers", FactValue::count(containers.len()));
    store.set_path(
        "workload.containers_with_requests",
        FactValue::count(
            containers
                .iter()
                .filter(|container| !container.requests.is_empty())
                .count(),
        ),
    );
    store.set_path(
        "workload.containers_with_limits",
        FactValue::count(
            containers
                .iter()
                .filter(|container| !container.limits.is_empty())
                .count(),
        ),
    );
    store.set_path(
        "workload.containers_digest_pinned",
        FactValue::count(
            containers
                .iter()
                .filter(|container| container.image.digest.is_some())
                .count(),
        ),
    );
    store.set_path(
        "workload.containers_latest_tag",
        FactValue::count(
            containers
                .iter()
                .filter(|container| {
                    // Untagged resolves to `latest` by the runtimes' own rule, and a digest pin
                    // makes the tag decoration — one owner for that judgement, here and in the
                    // `image_tag_not_latest` expectation.
                    container.image.digest.is_none()
                        && container.image.tag.as_deref().unwrap_or("latest") == "latest"
                })
                .count(),
        ),
    );
    let (liveness, readiness) = probe_counts(ir, &properties.workload);
    store.set_path(
        "workload.containers_with_liveness",
        FactValue::count(liveness),
    );
    store.set_path(
        "workload.containers_with_readiness",
        FactValue::count(readiness),
    );
}

/// Sets a count that only exists when its kind was scanned, and records the reason when it was
/// not.
fn optional_count(
    store: &mut FactStore,
    withheld: &mut BTreeMap<String, UnknownReason>,
    path: &str,
    kind: &str,
    covering: Option<&Vec<String>>,
) {
    match covering {
        Some(keys) => store.set_path(path, FactValue::count(keys.len())),
        None => {
            withheld.insert(
                path.to_owned(),
                UnknownReason::KindUnscanned {
                    kind: kind.to_owned(),
                },
            );
        }
    }
}

/// How many of a workload's containers declare a liveness and a readiness probe.
///
/// Read off the IR rather than off [`WorkloadProperties`], which does not carry probes — the
/// properties sheet is the resource envelope, and widening it to satisfy one fact path would put
/// the same field in two places.
fn probe_counts(ir: &InfraIr, workload_key: &str) -> (usize, usize) {
    let Some(workload) = ir.model.workloads.get(workload_key) else {
        return (0, 0);
    };
    let liveness = workload
        .containers
        .iter()
        .filter(|container| container.probes.liveness.is_some())
        .count();
    let readiness = workload
        .containers
        .iter()
        .filter(|container| container.probes.readiness.is_some())
        .count();
    (liveness, readiness)
}

/// The name a reference carries, resolved or not — both spellings name the same thing.
fn reference_name<H: std::fmt::Display>(reference: &infra_compiler::Reference<H>) -> String {
    match reference {
        infra_compiler::Reference::Resolved { key } => key
            .to_string()
            .rsplit_once('/')
            .map_or_else(|| key.to_string(), |(_, name)| name.to_owned()),
        infra_compiler::Reference::Unresolved { name } => name.clone(),
    }
}

/// The fact path for a documented name.
///
/// # Panics
///
/// Panics when `path` is not a valid fact path, which would be a defect in [`WORKLOAD_FACTS`]
/// rather than in any document.
#[must_use]
pub fn fact_path(path: &str) -> FactPath {
    FactPath::new(path).unwrap_or_else(|error| panic!("`{path}` is not a fact path: {error}"))
}

/// `true` when `path` is one this projection can produce.
#[must_use]
pub fn is_projected(path: &FactPath) -> bool {
    let rendered = path.to_string();
    WORKLOAD_FACTS.contains(&rendered.as_str())
}

/// The workload kind as a fact value, for a predicate comparing `workload.kind == deployment`.
#[must_use]
pub fn kind_value(kind: WorkloadKind) -> FactValue {
    FactValue::text(kind.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_documented_fact_path_parses_and_the_membership_check_agrees_with_the_list() {
        for path in WORKLOAD_FACTS {
            let parsed = fact_path(path);
            assert!(
                is_projected(&parsed),
                "{path} is documented but not recognised"
            );
        }
        assert!(
            !is_projected(&fact_path("workload.replica")),
            "a near-miss path must not be recognised, or the typo rule cannot fire"
        );
    }
}
