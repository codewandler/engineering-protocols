//! The validated observation, and the one door into it.
//!
//! [`Observation`] and everything it holds implement [`serde::Serialize`] and deliberately not
//! `Deserialize`: the only way to obtain one is [`TryFrom<RawBundle>`], which is where every rule
//! runs. The conversion accumulates — a bundle with forty defects reports forty refusals — and a
//! bundle with any refusal yields no observation at all.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;
use serde_json::Value;

use crate::code::{InfraCode, ValidationErrors};
use crate::config::{ConfigMap, Secret};
use crate::controller::{CronJob, Job, ReplicaSet};
use crate::network::{Ingress, Service};
use crate::policy::{HorizontalPodAutoscaler, PodDisruptionBudget};
use crate::raw::{
    items, optional_items, RawBundle, RawClaim, RawMeta, RawNamespace, RawNode, RawPod,
    RawServiceAccount,
};
use crate::workload::{Workload, WorkloadKind};

/// The format string this model reads.
pub const OBSERVATION_FORMAT: &str = "infra-observation/1";

/// The kind keys a bundle must carry, in the scanner's order.
pub const KINDS: &[&str] = &[
    "namespaces",
    "nodes",
    "deployments",
    "statefulsets",
    "daemonsets",
    "pods",
    "services",
    "ingresses",
    "configmaps",
    "secrets",
    "serviceaccounts",
    "persistentvolumeclaims",
];

/// The kind keys the scanner grew after `infra-observation/1` shipped, tolerated when absent.
///
/// A bundle written before the scanner collected these still validates: absence means "the scan
/// did not look" and is carried as `None` on [`Observation`], never rewritten into "none exist".
/// See the crate documentation for why this is the compatible reading and
/// [`InfraCode::MissingKind`] for why the original twelve stay required.
pub const OPTIONAL_KINDS: &[&str] = &[
    "replicasets",
    "jobs",
    "cronjobs",
    "poddisruptionbudgets",
    "horizontalpodautoscalers",
];

/// An object's identity: what everything downstream keys on.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct Identity {
    /// The namespace, absent on cluster-scoped kinds.
    pub namespace: Option<String>,
    /// The name.
    pub name: String,
    /// The API server's uid — carried as provenance of the observation, not used as a key,
    /// because a redeployed object keeps its identity and changes its uid.
    pub uid: String,
}

/// A namespace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Namespace {
    /// Identity.
    pub identity: Identity,
    /// Labels.
    pub labels: BTreeMap<String, String>,
}

/// A node, reduced to what scheduling and diagnosis read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Node {
    /// Identity.
    pub identity: Identity,
    /// Labels.
    pub labels: BTreeMap<String, String>,
    /// Resource capacity, quantities as the API states them.
    pub capacity: BTreeMap<String, String>,
    /// Runtime and OS identification.
    pub info: NodeInfo,
}

/// The slice of `nodeInfo` the model keeps.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NodeInfo {
    /// CPU architecture.
    pub architecture: Option<String>,
    /// Container runtime and version.
    pub container_runtime: Option<String>,
    /// Kernel version.
    pub kernel: Option<String>,
    /// Kubelet version.
    pub kubelet: Option<String>,
    /// Operating system.
    pub operating_system: Option<String>,
    /// OS image.
    pub os_image: Option<String>,
}

/// A service account: pure identity in this subset.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ServiceAccount {
    /// Identity.
    pub identity: Identity,
    /// Labels.
    pub labels: BTreeMap<String, String>,
}

/// A persistent volume claim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PersistentVolumeClaim {
    /// Identity.
    pub identity: Identity,
    /// Labels.
    pub labels: BTreeMap<String, String>,
    /// The storage class.
    pub storage_class: Option<String>,
    /// Access modes, sorted: the API treats them as a set, so the model does.
    pub access_modes: Vec<String>,
    /// The requested size, as the API states it.
    pub requested_storage: Option<String>,
    /// The observed lifecycle phase.
    pub phase: ClaimPhase,
}

/// A persistent volume claim's lifecycle phase.
///
/// The API defines exactly these three. Anything else — including an absent status — maps to
/// [`Self::Unknown`], for [`PodPhase`]'s reason: a phase is runtime observation, not document
/// structure, and an unobserved phase is not the same claim as an unbound one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimPhase {
    /// Not yet bound to a volume.
    Pending,
    /// Bound to a volume.
    Bound,
    /// The bound volume is gone.
    Lost,
    /// The state could not be obtained.
    Unknown,
}

impl ClaimPhase {
    /// The phase as the API spells it, which is also how a report prints it.
    ///
    /// [`Self::Unknown`] renders as `Unknown`, the API's own word for "the state could not be
    /// obtained" — not as an absence, because the claim was observed and its phase was not.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "Pending",
            Self::Bound => "Bound",
            Self::Lost => "Lost",
            Self::Unknown => "Unknown",
        }
    }

    fn parse(phase: Option<&str>) -> Self {
        match phase {
            Some("Pending") => Self::Pending,
            Some("Bound") => Self::Bound,
            Some("Lost") => Self::Lost,
            _ => Self::Unknown,
        }
    }
}

/// A pod's lifecycle phase.
///
/// The API defines exactly these five. A phase string outside them maps to [`Self::Unknown`],
/// which is also what the API reports when the kubelet cannot be reached — tolerated rather than
/// refused, because a phase is runtime observation, not document structure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PodPhase {
    /// Accepted, not yet running.
    Pending,
    /// Bound to a node, at least one container running.
    Running,
    /// All containers terminated successfully.
    Succeeded,
    /// All containers terminated, at least one in failure.
    Failed,
    /// The state could not be obtained.
    Unknown,
}

impl PodPhase {
    fn parse(phase: Option<&str>) -> Self {
        match phase {
            Some("Pending") => Self::Pending,
            Some("Running") => Self::Running,
            Some("Succeeded") => Self::Succeeded,
            Some("Failed") => Self::Failed,
            _ => Self::Unknown,
        }
    }
}

/// The controller that owns a pod, as its owner reference states it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OwnerRef {
    /// The owner's kind, such as `ReplicaSet` or `StatefulSet`.
    pub kind: String,
    /// The owner's name.
    pub name: String,
}

/// One container's observed status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ContainerStatus {
    /// The container's name.
    pub name: String,
    /// Whether it currently passes readiness.
    pub ready: bool,
    /// How often it restarted.
    pub restart_count: u32,
    /// Why the container is waiting instead of running, when it is — `CrashLoopBackOff`,
    /// `ImagePullBackOff` and their kind. `None` for a running or terminated container.
    ///
    /// Kept verbatim rather than as a closed enum: the reason strings are kubelet vocabulary
    /// that grows between releases, and a diagnosis that mapped an unknown reason to `Unknown`
    /// would erase exactly the word an operator needs to read.
    pub waiting_reason: Option<String>,
}

/// A pod, reduced to runtime essentials: what IW2's diagnosis reads.
///
/// Deliberately *not* a second copy of the workload model — a pod's spec is its template's, and
/// carrying it twice would put two spellings of every container in the IR.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Pod {
    /// Identity.
    pub identity: Identity,
    /// Labels — what service selectors match.
    pub labels: BTreeMap<String, String>,
    /// The lifecycle phase.
    pub phase: PodPhase,
    /// `true` when the pod has containers and every one passes readiness.
    pub ready: bool,
    /// The node the pod was scheduled to, absent while pending.
    pub node: Option<String>,
    /// The managing controller, when one is declared.
    pub owner: Option<OwnerRef>,
    /// Per-container readiness and restarts, in the API's order.
    pub containers: Vec<ContainerStatus>,
}

/// A validated observation: one cluster, one scan, every rule already enforced.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Observation {
    /// The kubeconfig context the scan targeted. Provenance, not semantic state.
    pub context: String,
    /// When the scan ran. Provenance, not semantic state.
    pub scanned_at: String,
    /// The scanner's version. Provenance, not semantic state.
    pub scout_version: String,
    /// Namespaces, in observed order; the compiler normalizes.
    pub namespaces: Vec<Namespace>,
    /// Nodes.
    pub nodes: Vec<Node>,
    /// Deployments, statefulsets and daemonsets, kind carried on each.
    pub workloads: Vec<Workload>,
    /// Services.
    pub services: Vec<Service>,
    /// Ingresses.
    pub ingresses: Vec<Ingress>,
    /// Configmaps: keys and value digests, never values.
    pub config_maps: Vec<ConfigMap>,
    /// Secrets: keys and value digests, never values.
    pub secrets: Vec<Secret>,
    /// Service accounts.
    pub service_accounts: Vec<ServiceAccount>,
    /// Persistent volume claims.
    pub claims: Vec<PersistentVolumeClaim>,
    /// Pods, runtime essentials only.
    pub pods: Vec<Pod>,
    /// Replicasets; `None` when the bundle predates the kind ([`OPTIONAL_KINDS`]).
    pub replica_sets: Option<Vec<ReplicaSet>>,
    /// Jobs; `None` when the bundle predates the kind.
    pub jobs: Option<Vec<Job>>,
    /// Cronjobs; `None` when the bundle predates the kind.
    pub cron_jobs: Option<Vec<CronJob>>,
    /// Pod disruption budgets; `None` when the bundle predates the kind.
    pub pod_disruption_budgets: Option<Vec<PodDisruptionBudget>>,
    /// Horizontal pod autoscalers; `None` when the bundle predates the kind.
    pub horizontal_pod_autoscalers: Option<Vec<HorizontalPodAutoscaler>>,
}

impl TryFrom<RawBundle> for Observation {
    type Error = ValidationErrors;

    // Long because a bundle has seventeen kinds, not because any step is deep: each block is
    // one kind's collection, and splitting them apart would scatter the one accumulator they
    // share.
    #[allow(clippy::too_many_lines)]
    fn try_from(raw: RawBundle) -> Result<Self, Self::Error> {
        let mut errors = ValidationErrors::new();

        if raw.format != OBSERVATION_FORMAT {
            errors.refuse(
                InfraCode::UnsupportedFormat,
                "format",
                format!(
                    "`{}` is not a format this build reads; expected `{OBSERVATION_FORMAT}`",
                    raw.format
                ),
            );
        }

        let namespaces = items::<RawNamespace>(&raw, "namespaces", &mut errors)
            .into_iter()
            .filter_map(|(location, item)| Namespace::from_raw(&item, &location, &mut errors))
            .collect();
        let nodes = items::<RawNode>(&raw, "nodes", &mut errors)
            .into_iter()
            .filter_map(|(location, item)| Node::from_raw(&item, &location, &mut errors))
            .collect();

        let mut workloads = Vec::new();
        for (kind_key, kind) in [
            ("deployments", WorkloadKind::Deployment),
            ("statefulsets", WorkloadKind::StatefulSet),
            ("daemonsets", WorkloadKind::DaemonSet),
        ] {
            workloads.extend(
                items::<crate::raw::RawWorkload>(&raw, kind_key, &mut errors)
                    .into_iter()
                    .filter_map(|(location, item)| {
                        Workload::from_raw(&item, kind, &location, &mut errors)
                    }),
            );
        }

        let pods = items::<RawPod>(&raw, "pods", &mut errors)
            .into_iter()
            .filter_map(|(location, item)| Pod::from_raw(&item, &location, &mut errors))
            .collect();
        let services = items::<crate::raw::RawService>(&raw, "services", &mut errors)
            .into_iter()
            .filter_map(|(location, item)| Service::from_raw(&item, &location, &mut errors))
            .collect();
        let ingresses = items::<crate::raw::RawIngress>(&raw, "ingresses", &mut errors)
            .into_iter()
            .filter_map(|(location, item)| Ingress::from_raw(&item, &location, &mut errors))
            .collect();
        let config_maps = items::<crate::raw::RawConfigMap>(&raw, "configmaps", &mut errors)
            .into_iter()
            .filter_map(|(location, item)| ConfigMap::from_raw(&item, &location, &mut errors))
            .collect();
        let secrets = items::<crate::raw::RawSecret>(&raw, "secrets", &mut errors)
            .into_iter()
            .filter_map(|(location, item)| Secret::from_raw(&item, &location, &mut errors))
            .collect();
        let service_accounts = items::<RawServiceAccount>(&raw, "serviceaccounts", &mut errors)
            .into_iter()
            .filter_map(|(location, item)| ServiceAccount::from_raw(&item, &location, &mut errors))
            .collect();
        let claims = items::<RawClaim>(&raw, "persistentvolumeclaims", &mut errors)
            .into_iter()
            .filter_map(|(location, item)| {
                PersistentVolumeClaim::from_raw(&item, &location, &mut errors)
            })
            .collect();

        let replica_sets = optional_items::<crate::raw::RawReplicaSet>(
            &raw,
            "replicasets",
            &mut errors,
        )
        .map(|list| {
            list.into_iter()
                .filter_map(|(location, item)| ReplicaSet::from_raw(&item, &location, &mut errors))
                .collect()
        });
        let jobs = optional_items::<crate::raw::RawJob>(&raw, "jobs", &mut errors).map(|list| {
            list.into_iter()
                .filter_map(|(location, item)| Job::from_raw(&item, &location, &mut errors))
                .collect()
        });
        let cron_jobs = optional_items::<crate::raw::RawCronJob>(&raw, "cronjobs", &mut errors)
            .map(|list| {
                list.into_iter()
                    .filter_map(|(location, item)| CronJob::from_raw(&item, &location, &mut errors))
                    .collect()
            });
        let pod_disruption_budgets = optional_items::<crate::raw::RawPodDisruptionBudget>(
            &raw,
            "poddisruptionbudgets",
            &mut errors,
        )
        .map(|list| {
            list.into_iter()
                .filter_map(|(location, item)| {
                    PodDisruptionBudget::from_raw(&item, &location, &mut errors)
                })
                .collect()
        });
        let horizontal_pod_autoscalers = optional_items::<crate::raw::RawHorizontalPodAutoscaler>(
            &raw,
            "horizontalpodautoscalers",
            &mut errors,
        )
        .map(|list| {
            list.into_iter()
                .filter_map(|(location, item)| {
                    HorizontalPodAutoscaler::from_raw(&item, &location, &mut errors)
                })
                .collect()
        });

        let observation = Self {
            context: raw.context,
            scanned_at: raw.scanned_at,
            scout_version: raw.scout_version,
            namespaces,
            nodes,
            workloads,
            services,
            ingresses,
            config_maps,
            secrets,
            service_accounts,
            claims,
            pods,
            replica_sets,
            jobs,
            cron_jobs,
            pod_disruption_budgets,
            horizontal_pod_autoscalers,
        };
        observation.refuse_duplicates(&mut errors);

        if errors.is_empty() {
            Ok(observation)
        } else {
            Err(errors)
        }
    }
}

impl Observation {
    /// Refuses every pair of same-kind objects sharing a namespace and name.
    ///
    /// After collection rather than during it, so a duplicate is reported *as* a duplicate and
    /// not as whatever second-order defect the collision would have caused downstream.
    /// One `scan` call per kind is what makes the length; the logic is the closure.
    #[allow(clippy::too_many_lines)]
    fn refuse_duplicates(&self, errors: &mut ValidationErrors) {
        fn scan<'a>(
            kind: &str,
            identities: impl Iterator<Item = &'a Identity>,
            errors: &mut ValidationErrors,
        ) {
            let mut seen = BTreeSet::new();
            for identity in identities {
                if !seen.insert((identity.namespace.clone(), identity.name.clone())) {
                    let place = match &identity.namespace {
                        Some(namespace) => format!("{namespace}/{}", identity.name),
                        None => identity.name.clone(),
                    };
                    errors.refuse(
                        InfraCode::DuplicateIdentity,
                        format!("kinds.{kind}"),
                        format!("`{place}` appears twice; identity must be unique per kind"),
                    );
                }
            }
        }

        scan(
            "namespaces",
            self.namespaces.iter().map(|item| &item.identity),
            errors,
        );
        scan(
            "nodes",
            self.nodes.iter().map(|item| &item.identity),
            errors,
        );
        // Workloads deduplicate per kind, not across the three: a deployment and a statefulset
        // may legally share a name.
        for kind in [
            WorkloadKind::Deployment,
            WorkloadKind::StatefulSet,
            WorkloadKind::DaemonSet,
        ] {
            scan(
                kind.plural(),
                self.workloads
                    .iter()
                    .filter(|workload| workload.kind == kind)
                    .map(|item| &item.identity),
                errors,
            );
        }
        scan(
            "services",
            self.services.iter().map(|item| &item.identity),
            errors,
        );
        scan(
            "ingresses",
            self.ingresses.iter().map(|item| &item.identity),
            errors,
        );
        scan(
            "configmaps",
            self.config_maps.iter().map(|item| &item.identity),
            errors,
        );
        scan(
            "secrets",
            self.secrets.iter().map(|item| &item.identity),
            errors,
        );
        scan(
            "serviceaccounts",
            self.service_accounts.iter().map(|item| &item.identity),
            errors,
        );
        scan(
            "persistentvolumeclaims",
            self.claims.iter().map(|item| &item.identity),
            errors,
        );
        scan("pods", self.pods.iter().map(|item| &item.identity), errors);
        if let Some(replica_sets) = &self.replica_sets {
            scan(
                "replicasets",
                replica_sets.iter().map(|item| &item.identity),
                errors,
            );
        }
        if let Some(jobs) = &self.jobs {
            scan("jobs", jobs.iter().map(|item| &item.identity), errors);
        }
        if let Some(cron_jobs) = &self.cron_jobs {
            scan(
                "cronjobs",
                cron_jobs.iter().map(|item| &item.identity),
                errors,
            );
        }
        if let Some(budgets) = &self.pod_disruption_budgets {
            scan(
                "poddisruptionbudgets",
                budgets.iter().map(|item| &item.identity),
                errors,
            );
        }
        if let Some(autoscalers) = &self.horizontal_pod_autoscalers {
            scan(
                "horizontalpodautoscalers",
                autoscalers.iter().map(|item| &item.identity),
                errors,
            );
        }
    }
}

/// Validates a `metadata` block into an [`Identity`], refusing what is missing.
pub(crate) fn identity(
    meta: &RawMeta,
    namespaced: bool,
    location: &str,
    errors: &mut ValidationErrors,
) -> Option<Identity> {
    let mut complete = true;
    for (field, present) in [
        (
            "name",
            meta.name.as_deref().is_some_and(|name| !name.is_empty()),
        ),
        (
            "uid",
            meta.uid.as_deref().is_some_and(|uid| !uid.is_empty()),
        ),
        (
            "namespace",
            !namespaced
                || meta
                    .namespace
                    .as_deref()
                    .is_some_and(|namespace| !namespace.is_empty()),
        ),
    ] {
        if !present {
            errors.refuse(
                InfraCode::MissingIdentity,
                format!("{location}.metadata.{field}"),
                format!(
                    "`{field}` is missing or empty; identity is what everything downstream keys on"
                ),
            );
            complete = false;
        }
    }
    if !complete {
        return None;
    }
    Some(Identity {
        namespace: if namespaced {
            meta.namespace.clone()
        } else {
            None
        },
        name: meta.name.clone().unwrap_or_default(),
        uid: meta.uid.clone().unwrap_or_default(),
    })
}

/// Validates a map whose values must all be strings — labels and selectors.
///
/// Each non-string value is refused with its own location and dropped; the strings survive, so
/// one bad label does not hide the rest of the pass's findings.
pub(crate) fn string_map(
    raw: &BTreeMap<String, Value>,
    location: &str,
    errors: &mut ValidationErrors,
) -> BTreeMap<String, String> {
    let mut validated = BTreeMap::new();
    for (key, value) in raw {
        match value.as_str() {
            Some(text) => {
                validated.insert(key.clone(), text.to_owned());
            }
            None => {
                errors.refuse(
                    InfraCode::NonStringSelector,
                    format!("{location}.{key}"),
                    format!(
                        "the value of `{key}` is {}, not a string",
                        value_kind(value)
                    ),
                );
            }
        }
    }
    validated
}

/// Names a JSON value's kind for an error message without echoing the value.
pub(crate) fn value_kind(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "a boolean",
        Value::Number(_) => "a number",
        Value::String(_) => "a string",
        Value::Array(_) => "an array",
        Value::Object(_) => "an object",
    }
}

/// Renders an int-or-string port as its string form, refusing anything else.
pub(crate) fn port_string(
    value: &Value,
    location: &str,
    errors: &mut ValidationErrors,
) -> Option<String> {
    match value {
        Value::Number(number) => Some(number.to_string()),
        Value::String(name) => Some(name.clone()),
        other => {
            errors.refuse(
                InfraCode::MalformedObject,
                location.to_owned(),
                format!(
                    "a port must be a number or a name, found {}",
                    value_kind(other)
                ),
            );
            None
        }
    }
}

impl Namespace {
    fn from_raw(raw: &RawNamespace, location: &str, errors: &mut ValidationErrors) -> Option<Self> {
        let identity = identity(&raw.metadata, false, location, errors)?;
        Some(Self {
            identity,
            labels: string_map(
                &raw.metadata.labels,
                &format!("{location}.metadata.labels"),
                errors,
            ),
        })
    }
}

impl Node {
    fn from_raw(raw: &RawNode, location: &str, errors: &mut ValidationErrors) -> Option<Self> {
        let identity = identity(&raw.metadata, false, location, errors)?;
        Some(Self {
            identity,
            labels: string_map(
                &raw.metadata.labels,
                &format!("{location}.metadata.labels"),
                errors,
            ),
            capacity: raw.status.capacity.clone(),
            info: NodeInfo {
                architecture: raw.status.node_info.architecture.clone(),
                container_runtime: raw.status.node_info.container_runtime_version.clone(),
                kernel: raw.status.node_info.kernel_version.clone(),
                kubelet: raw.status.node_info.kubelet_version.clone(),
                operating_system: raw.status.node_info.operating_system.clone(),
                os_image: raw.status.node_info.os_image.clone(),
            },
        })
    }
}

impl ServiceAccount {
    fn from_raw(
        raw: &RawServiceAccount,
        location: &str,
        errors: &mut ValidationErrors,
    ) -> Option<Self> {
        let identity = identity(&raw.metadata, true, location, errors)?;
        Some(Self {
            identity,
            labels: string_map(
                &raw.metadata.labels,
                &format!("{location}.metadata.labels"),
                errors,
            ),
        })
    }
}

impl PersistentVolumeClaim {
    fn from_raw(raw: &RawClaim, location: &str, errors: &mut ValidationErrors) -> Option<Self> {
        let identity = identity(&raw.metadata, true, location, errors)?;
        let mut access_modes = raw.spec.access_modes.clone();
        access_modes.sort();
        access_modes.dedup();
        Some(Self {
            identity,
            labels: string_map(
                &raw.metadata.labels,
                &format!("{location}.metadata.labels"),
                errors,
            ),
            storage_class: raw.spec.storage_class_name.clone(),
            access_modes,
            requested_storage: raw.spec.resources.requests.get("storage").cloned(),
            phase: ClaimPhase::parse(raw.status.phase.as_deref()),
        })
    }
}

impl Pod {
    fn from_raw(raw: &RawPod, location: &str, errors: &mut ValidationErrors) -> Option<Self> {
        let identity = identity(&raw.metadata, true, location, errors)?;
        let mut containers = Vec::with_capacity(raw.status.container_statuses.len());
        for (index, status) in raw.status.container_statuses.iter().enumerate() {
            match status.name.as_deref() {
                Some(name) if !name.is_empty() => containers.push(ContainerStatus {
                    name: name.to_owned(),
                    ready: status.ready,
                    restart_count: status.restart_count,
                    waiting_reason: status
                        .state
                        .waiting
                        .as_ref()
                        .and_then(|waiting| waiting.reason.clone()),
                }),
                _ => errors.refuse(
                    InfraCode::MissingIdentity,
                    format!("{location}.status.containerStatuses[{index}].name"),
                    "a container status without a name cannot be attributed",
                ),
            }
        }
        let ready = !containers.is_empty() && containers.iter().all(|status| status.ready);
        Some(Self {
            identity,
            labels: string_map(
                &raw.metadata.labels,
                &format!("{location}.metadata.labels"),
                errors,
            ),
            phase: PodPhase::parse(raw.status.phase.as_deref()),
            ready,
            node: raw.spec.node_name.clone(),
            owner: raw
                .metadata
                .owner_references
                .iter()
                .find(|reference| reference.controller)
                .map(|reference| OwnerRef {
                    kind: reference.kind.clone(),
                    name: reference.name.clone(),
                }),
            containers,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_bundle() -> serde_json::Value {
        let mut kinds = serde_json::Map::new();
        for kind in KINDS {
            kinds.insert((*kind).to_owned(), serde_json::json!({ "items": [] }));
        }
        serde_json::json!({
            "format": OBSERVATION_FORMAT,
            "context": "test",
            "scanned_at": "2026-08-20T22:30:30Z",
            "scout_version": "0.1.0",
            "kinds": kinds,
        })
    }

    fn validate(bundle: serde_json::Value) -> Result<Observation, ValidationErrors> {
        let raw: RawBundle = serde_json::from_value(bundle).expect("the bundle parses");
        Observation::try_from(raw)
    }

    #[test]
    fn an_empty_but_complete_bundle_is_a_valid_observation_of_nothing() {
        let observation = validate(minimal_bundle()).expect("an empty cluster is observable");
        assert_eq!(observation.context, "test");
        assert!(observation.pods.is_empty());
    }

    #[test]
    fn a_wrong_format_string_is_refused_with_its_own_code() {
        let mut bundle = minimal_bundle();
        bundle["format"] = serde_json::json!("infra-observation/2");
        let errors = validate(bundle).expect_err("an unknown format is refused");
        assert!(
            errors.contains(InfraCode::UnsupportedFormat),
            "expected INFRA-BUNDLE-001, got: {errors}"
        );
    }

    #[test]
    fn an_absent_kind_is_refused_because_not_scanned_is_not_the_same_as_none_exist() {
        let mut bundle = minimal_bundle();
        bundle["kinds"]
            .as_object_mut()
            .expect("kinds is an object")
            .remove("ingresses");
        let errors = validate(bundle).expect_err("a missing kind is refused");
        assert!(
            errors.contains(InfraCode::MissingKind),
            "expected INFRA-BUNDLE-002, got: {errors}"
        );
    }

    #[test]
    fn a_kind_the_model_never_heard_of_is_tolerated() {
        let mut bundle = minimal_bundle();
        bundle["kinds"]["networkpolicies"] = serde_json::json!({ "items": [{}] });
        validate(bundle).expect("an unknown kind is a scanner ahead of the model, not a defect");
    }

    #[test]
    fn a_bundle_without_the_optional_kinds_validates_and_carries_their_absence_as_absence() {
        // The compatibility choice: `minimal_bundle()` is the *original* twelve-kind format, and
        // it must stay a valid observation after the scanner grew — refusing it would invalidate
        // every scan taken before replicasets, jobs, cronjobs, budgets and autoscalers joined.
        let observation = validate(minimal_bundle()).expect("an older bundle still validates");
        assert!(
            observation.replica_sets.is_none()
                && observation.jobs.is_none()
                && observation.cron_jobs.is_none()
                && observation.pod_disruption_budgets.is_none()
                && observation.horizontal_pod_autoscalers.is_none(),
            "an unscanned kind is None — not an empty list, which would claim nobody has any"
        );
    }

    #[test]
    fn an_optional_kind_that_is_present_is_validated_not_waved_through() {
        let mut bundle = minimal_bundle();
        // A cronjob without a schedule: present, so the rules run and refuse it.
        bundle["kinds"]["cronjobs"] = serde_json::json!({ "items": [
            { "metadata": { "name": "broken", "namespace": "app", "uid": "cj-1" } }
        ] });
        let errors = validate(bundle).expect_err("a present optional kind is checked");
        assert!(
            errors.contains(InfraCode::MalformedObject),
            "expected INFRA-OBJECT-001, got: {errors}"
        );
    }

    #[test]
    fn an_empty_optional_kind_is_an_observation_of_none_distinguishable_from_unscanned() {
        let mut bundle = minimal_bundle();
        bundle["kinds"]["poddisruptionbudgets"] = serde_json::json!({ "items": [] });
        let observation = validate(bundle).expect("an empty list is observable");
        assert_eq!(
            observation.pod_disruption_budgets,
            Some(Vec::new()),
            "scanned-and-empty is Some([]), never None"
        );
    }

    #[test]
    fn two_replicasets_sharing_namespace_and_name_are_refused_as_a_duplicate() {
        let mut bundle = minimal_bundle();
        bundle["kinds"]["replicasets"] = serde_json::json!({ "items": [
            { "metadata": { "name": "web-abc", "namespace": "app", "uid": "rs-1" } },
            { "metadata": { "name": "web-abc", "namespace": "app", "uid": "rs-2" } }
        ] });
        let errors = validate(bundle).expect_err("a duplicate identity is refused");
        assert!(
            errors.contains(InfraCode::DuplicateIdentity),
            "expected INFRA-OBJECT-003, got: {errors}"
        );
    }

    #[test]
    fn every_identity_defect_in_one_object_is_reported_in_one_run() {
        let mut bundle = minimal_bundle();
        // No name, no uid, no namespace: three refusals from one pod, not one.
        bundle["kinds"]["pods"]["items"] = serde_json::json!([{ "metadata": {} }]);
        let errors = validate(bundle).expect_err("an unidentifiable pod is refused");
        assert_eq!(
            errors.len(),
            3,
            "name, uid and namespace are each refused: {errors}"
        );
        assert!(errors.contains(InfraCode::MissingIdentity));
    }

    #[test]
    fn two_pods_sharing_namespace_and_name_are_refused_as_a_duplicate() {
        let mut bundle = minimal_bundle();
        let pod = serde_json::json!({
            "metadata": { "name": "web-0", "namespace": "app", "uid": "aaa" }
        });
        let mut second = pod.clone();
        second["metadata"]["uid"] = serde_json::json!("bbb");
        bundle["kinds"]["pods"]["items"] = serde_json::json!([pod, second]);
        let errors = validate(bundle).expect_err("a duplicate identity is refused");
        assert!(
            errors.contains(InfraCode::DuplicateIdentity),
            "expected INFRA-OBJECT-003, got: {errors}"
        );
    }

    #[test]
    fn a_non_string_label_is_refused_and_named_by_key() {
        let mut bundle = minimal_bundle();
        bundle["kinds"]["namespaces"]["items"] = serde_json::json!([{
            "metadata": { "name": "app", "uid": "aaa", "labels": { "tier": 3 } }
        }]);
        let errors = validate(bundle).expect_err("a numeric label is refused");
        assert!(
            errors.contains(InfraCode::NonStringSelector),
            "expected INFRA-SELECTOR-001, got: {errors}"
        );
        assert!(
            errors.as_slice()[0].location.ends_with("labels.tier"),
            "the refusal names the key: {errors}"
        );
    }

    #[test]
    fn an_unknown_phase_string_maps_to_unknown_rather_than_refusing_the_pod() {
        let mut bundle = minimal_bundle();
        bundle["kinds"]["pods"]["items"] = serde_json::json!([{
            "metadata": { "name": "web-0", "namespace": "app", "uid": "aaa" },
            "status": { "phase": "Evicted" }
        }]);
        let observation = validate(bundle).expect("a strange phase is observation, not structure");
        assert_eq!(observation.pods[0].phase, PodPhase::Unknown);
    }

    #[test]
    fn readiness_requires_every_container_ready_and_at_least_one() {
        let mut bundle = minimal_bundle();
        bundle["kinds"]["pods"]["items"] = serde_json::json!([
            {
                "metadata": { "name": "a", "namespace": "app", "uid": "u1" },
                "status": { "phase": "Running", "containerStatuses": [
                    { "name": "main", "ready": true, "restartCount": 0 },
                    { "name": "sidecar", "ready": false, "restartCount": 4 }
                ] }
            },
            {
                "metadata": { "name": "b", "namespace": "app", "uid": "u2" },
                "status": { "phase": "Pending" }
            }
        ]);
        let observation = validate(bundle).expect("both pods validate");
        assert!(
            !observation.pods[0].ready,
            "one unready container is enough"
        );
        assert!(
            !observation.pods[1].ready,
            "no containers is not the same as all ready"
        );
    }
}
