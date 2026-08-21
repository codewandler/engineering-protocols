//! The infrastructure IR: one cluster's semantic state, normalized and content-addressed.
//!
//! # The same property `ess-compiler` holds, in the shape a cluster forces
//!
//! In the specification IR a dangling reference is *unrepresentable*: a handle can only be minted
//! by the compiler, and the compiler refuses a specification that references what is not
//! declared. An observed cluster cannot be held to that bar — a selector that matches nothing and
//! an env var reading a configmap that is not there are things a live cluster legitimately
//! contains, and refusing to build an IR of a degraded cluster would make the tool useless
//! exactly when IW2's diagnosis needs it. So the property splits:
//!
//! * **Where the model resolves a reference, dangling is still unrepresentable.** A
//!   [`Reference::Resolved`] holds a handle, a handle has no public constructor, and the lookups
//!   are total: [`InfraIr::config_map`] returns `&ConfigMap`, not an `Option`.
//! * **What did not resolve is a typed fact, not an error.** [`Reference::Unresolved`] keeps the
//!   name at the site, and every one is also aggregated in [`InfraModel::unresolved`] — openly
//!   carried data for IW2 to diagnose, sorted so the IR stays canonical.
//!
//! # Keys
//!
//! Every map is a [`BTreeMap`] keyed by identity, never a `HashMap` (invariant 9):
//! cluster-scoped kinds by `name`, namespaced kinds by `namespace/name`, and workloads by
//! `namespace/kind/name` because the three workload kinds share one map and a deployment may
//! legally share a name with a statefulset. `uid` is deliberately not the key: a redeployed
//! object keeps its identity and changes its uid, and the digest must survive a redeploy that
//! changes nothing semantic.
//!
//! # The digest
//!
//! [`InfraIr::digest`] is the full SHA-256, 64 hex characters, over the canonical JSON of
//! [`InfraModel`] alone — compact, keys sorted, so any reader of the persisted document can
//! recompute it. `context`, `scanned_at` and `scout_version` live in [`Provenance`],
//! *outside* the digested bytes: two scans of an unchanged cluster produce the same digest, which
//! is the entire point of hashing — a digest that changed with the clock would detect nothing but
//! the passage of time.

use std::collections::BTreeMap;
use std::fmt;

use infra_domain::config::{ConfigMap, Secret};
use infra_domain::controller::{CronJob, Job, ReplicaSet};
use infra_domain::network::Service;
use infra_domain::observation::{
    ContainerStatus, Identity, Namespace, Node, OwnerRef, PersistentVolumeClaim, PodPhase,
    ServiceAccount,
};
use infra_domain::policy::{HorizontalPodAutoscaler, PodDisruptionBudget};
use infra_domain::workload::{Probes, Resources, VolumeMount, WorkloadKind};
use serde::Serialize;

/// The format string a persisted IR document carries.
pub const IR_FORMAT: &str = "infra-ir/1";

/// Declares every handle kind and its total accessor on [`InfraIr`] from one line each — the
/// `ess-compiler` idiom, kept because its argument transfers whole: a handle is only mintable by
/// this crate, therefore the map contains it, therefore the lookup does not return an `Option`.
macro_rules! handles {
    (
        $(
            $(#[$attribute:meta])*
            $handle:ident => $accessor:ident : $resolved:ident in $field:ident,
            $what:literal;
        )*
    ) => {
        $(
            $(#[$attribute])*
            ///
            /// Mintable only by [`compile`](crate::compile::compile). A consumer cannot construct
            /// one, so it cannot hold a reference the compiler did not check against the model.
            /// The key it wraps is the entry's map key.
            #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
            #[serde(transparent)]
            pub struct $handle(String);

            impl $handle {
                /// Records a checked reference. Crate-private: minting is the check.
                pub(crate) fn new(key: String) -> Self {
                    Self(key)
                }

                /// The map key it carries.
                pub fn key(&self) -> &str {
                    &self.0
                }
            }

            impl fmt::Display for $handle {
                fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                    f.write_str(&self.0)
                }
            }

            impl InfraIr {
                #[doc = concat!("The ", $what, " a handle names.")]
                ///
                /// Total. The one way to reach the panic is to use a handle from one
                /// [`InfraIr`] against another, which is a programming mistake and not an
                /// observation's problem.
                pub fn $accessor(&self, handle: &$handle) -> &$resolved {
                    self.model.$field.get(&handle.0).unwrap_or_else(|| {
                        panic!(
                            "`{handle}` is not a {} this IR holds: a handle belongs to the IR \
                             that minted it",
                            $what
                        )
                    })
                }
            }
        )*
    };
}

handles! {
    // No `NamespaceHandle`, `WorkloadHandle` or `PodHandle` — deliberately. A handle exists
    // where a *site in the model* resolves a reference, and in this wave no site references a
    // namespace, a workload or a pod (namespace membership and empty selections are aggregate
    // facts). IW2's graph adds those handles when it adds the sites that need them; declaring
    // them now would be six accessors nothing can mint a key for.
    /// A node that was observed.
    NodeHandle => node : Node in nodes, "node";
    /// A service that was observed.
    ServiceHandle => service : Service in services, "service";
    /// A configmap that was observed.
    ConfigMapHandle => config_map : ConfigMap in config_maps, "configmap";
    /// A secret that was observed.
    SecretHandle => secret : Secret in secrets, "secret";
    /// A service account that was observed.
    ServiceAccountHandle => service_account : ServiceAccount in service_accounts, "service account";
    /// A persistent volume claim that was observed.
    ClaimHandle => claim : PersistentVolumeClaim in claims, "persistent volume claim";
}

/// A reference as the IR carries it: checked, and honest about the outcome.
///
/// The unresolved arm is data, not an error — see the module doc. Every unresolved arm is also
/// listed in [`InfraModel::unresolved`], so a consumer that only wants the dangling set never
/// walks the tree.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Reference<H> {
    /// The target was observed; the handle's lookup is total.
    Resolved {
        /// The minted handle.
        key: H,
    },
    /// The target was not observed. The name is kept so the site stays readable on its own.
    Unresolved {
        /// The name the cluster declared.
        name: String,
    },
}

/// A workload with every modelled reference checked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResolvedWorkload {
    /// Which of the three kinds.
    pub kind: WorkloadKind,
    /// Identity.
    pub identity: Identity,
    /// Labels on the workload object.
    pub labels: BTreeMap<String, String>,
    /// Desired replicas; absent on daemonsets.
    pub replicas: Option<u32>,
    /// The pod selector's `matchLabels`.
    pub selector: BTreeMap<String, String>,
    /// A statefulset's governing service, checked against the observed services.
    pub governing_service: Option<Reference<ServiceHandle>>,
    /// The service account the pods run as. The document's absence means `default`, and the
    /// compiler resolves that spelling — so this is always present, unlike the model's field.
    pub service_account: Reference<ServiceAccountHandle>,
    /// The template's labels.
    pub template_labels: BTreeMap<String, String>,
    /// The containers, in declared order.
    pub containers: Vec<ResolvedContainer>,
    /// The volumes, in declared order.
    pub volumes: Vec<ResolvedVolume>,
}

/// A container with its configuration references checked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResolvedContainer {
    /// The container's name.
    pub name: String,
    /// The image reference.
    pub image: String,
    /// Environment variables, in declared order.
    pub env: Vec<ResolvedEnvVar>,
    /// Whole-map environment sources, in declared order.
    pub env_from: Vec<ResolvedEnvFrom>,
    /// Volume mounts, in declared order.
    pub volume_mounts: Vec<VolumeMount>,
    /// The probes, carried as validated.
    pub probes: Probes,
    /// Requests and limits, carried as validated.
    pub resources: Resources,
}

/// One environment variable, its source checked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResolvedEnvVar {
    /// The variable's name.
    pub name: String,
    /// The source.
    pub source: ResolvedEnvSource,
}

/// Where an environment variable's value comes from, references checked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ResolvedEnvSource {
    /// A literal value, as declared.
    Literal {
        /// The value.
        value: String,
    },
    /// A key of a configmap.
    ConfigMapKey {
        /// The configmap.
        config_map: Reference<ConfigMapHandle>,
        /// The key inside it.
        key: String,
        /// Whether the reference may dangle without failing the pod.
        optional: bool,
    },
    /// A key of a secret — the reference, never the value.
    SecretKey {
        /// The secret.
        secret: Reference<SecretHandle>,
        /// The key inside it.
        key: String,
        /// Whether the reference may dangle without failing the pod.
        optional: bool,
    },
    /// A downward-API field of the pod.
    FieldRef {
        /// The field path.
        path: String,
    },
    /// A resource quantity of a container.
    ResourceFieldRef,
}

/// One `envFrom` source, its reference checked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResolvedEnvFrom {
    /// A prefix prepended to every imported key.
    pub prefix: Option<String>,
    /// What is imported.
    pub source: ResolvedEnvFromSource,
}

/// What an `envFrom` entry imports, reference checked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ResolvedEnvFromSource {
    /// A whole configmap.
    ConfigMap {
        /// The configmap.
        config_map: Reference<ConfigMapHandle>,
        /// Whether the reference may dangle without failing the pod.
        optional: bool,
    },
    /// A whole secret.
    Secret {
        /// The secret.
        secret: Reference<SecretHandle>,
        /// Whether the reference may dangle without failing the pod.
        optional: bool,
    },
}

/// One volume, its source's reference checked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResolvedVolume {
    /// The volume's name.
    pub name: String,
    /// The source.
    pub source: ResolvedVolumeSource,
}

/// A volume's source, references checked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ResolvedVolumeSource {
    /// A configmap.
    ConfigMap {
        /// The configmap.
        config_map: Reference<ConfigMapHandle>,
        /// Whether the reference may dangle without failing the pod.
        optional: bool,
    },
    /// A secret.
    Secret {
        /// The secret.
        secret: Reference<SecretHandle>,
        /// Whether the reference may dangle without failing the pod.
        optional: bool,
    },
    /// A persistent volume claim.
    Claim {
        /// The claim.
        claim: Reference<ClaimHandle>,
    },
    /// An ephemeral empty directory.
    EmptyDir,
    /// A path on the node.
    HostPath {
        /// The path.
        path: Option<String>,
    },
    /// A source the subset does not model.
    Other,
}

/// An ingress with its backends checked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResolvedIngress {
    /// Identity.
    pub identity: Identity,
    /// Labels.
    pub labels: BTreeMap<String, String>,
    /// The rules, in declared order.
    pub rules: Vec<ResolvedIngressRule>,
    /// The backend used when no rule matches.
    pub default_backend: Option<ResolvedIngressBackend>,
}

/// One ingress rule, backends checked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResolvedIngressRule {
    /// The host.
    pub host: Option<String>,
    /// The paths, in declared order.
    pub paths: Vec<ResolvedIngressPath>,
}

/// One routed path, backend checked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResolvedIngressPath {
    /// The URL path.
    pub path: Option<String>,
    /// How the path matches.
    pub path_type: Option<String>,
    /// The backend.
    pub backend: ResolvedIngressBackend,
}

/// A backend, its service reference checked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResolvedIngressBackend {
    /// The service.
    pub service: Reference<ServiceHandle>,
    /// The service port, by name or number.
    pub port: Option<String>,
}

/// A pod with its node assignment checked.
///
/// The owner reference stays plain data, deliberately: deployment pods are owned through
/// `ReplicaSet`s, a kind the observation surface does not include, so resolving owners here would
/// mark nearly every pod of a healthy cluster unresolved. Building the ownership chain is IW2's
/// graph work, on top of the owner *data* this carries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResolvedPod {
    /// Identity.
    pub identity: Identity,
    /// Labels.
    pub labels: BTreeMap<String, String>,
    /// The lifecycle phase.
    pub phase: PodPhase,
    /// `true` when the pod has containers and every one passes readiness.
    pub ready: bool,
    /// The node the pod was scheduled to; absent while pending.
    pub node: Option<Reference<NodeHandle>>,
    /// The managing controller, as declared.
    pub owner: Option<OwnerRef>,
    /// Per-container readiness and restarts.
    pub containers: Vec<ContainerStatus>,
}

/// One dangling reference, carried openly for IW2 to diagnose.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct UnresolvedReference {
    /// Which IR entry holds the reference: its map name and key, such as
    /// `workloads/sbf/deployment/frontend`.
    pub from: String,
    /// Where inside that entry, such as `containers[main].env[DB_PASSWORD]` or `selector`.
    pub site: String,
    /// What was referenced and not observed.
    pub target: UnresolvedTarget,
}

/// What a dangling reference points at.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum UnresolvedTarget {
    /// A configmap that was not observed.
    ConfigMap {
        /// Its name.
        name: String,
        /// Whether the reference declared itself optional.
        optional: bool,
    },
    /// A configmap that was observed but lacks the referenced key.
    ConfigMapKey {
        /// The configmap's name.
        name: String,
        /// The missing key.
        key: String,
        /// Whether the reference declared itself optional.
        optional: bool,
    },
    /// A secret that was not observed.
    Secret {
        /// Its name.
        name: String,
        /// Whether the reference declared itself optional.
        optional: bool,
    },
    /// A secret that was observed but lacks the referenced key.
    SecretKey {
        /// The secret's name.
        name: String,
        /// The missing key.
        key: String,
        /// Whether the reference declared itself optional.
        optional: bool,
    },
    /// A service account that was not observed.
    ServiceAccount {
        /// Its name.
        name: String,
    },
    /// A persistent volume claim that was not observed.
    Claim {
        /// Its name.
        name: String,
    },
    /// A service that was not observed.
    Service {
        /// Its name.
        name: String,
    },
    /// A node that was not observed.
    Node {
        /// Its name.
        name: String,
    },
    /// A namespace an object sits in that the scan did not observe.
    Namespace {
        /// Its name.
        name: String,
    },
    /// A service selector no observed pod matches.
    PodsMatchingSelector {
        /// The selector that matched nothing.
        selector: BTreeMap<String, String>,
    },
}

/// The digested content: everything semantic, nothing about when or how it was observed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InfraModel {
    /// Namespaces, keyed by name.
    pub namespaces: BTreeMap<String, Namespace>,
    /// Nodes, keyed by name.
    pub nodes: BTreeMap<String, Node>,
    /// Workloads, keyed by `namespace/kind/name`.
    pub workloads: BTreeMap<String, ResolvedWorkload>,
    /// Services, keyed by `namespace/name`.
    pub services: BTreeMap<String, Service>,
    /// Ingresses, keyed by `namespace/name`.
    pub ingresses: BTreeMap<String, ResolvedIngress>,
    /// Configmaps, keyed by `namespace/name`.
    pub config_maps: BTreeMap<String, ConfigMap>,
    /// Secrets, keyed by `namespace/name`.
    pub secrets: BTreeMap<String, Secret>,
    /// Service accounts, keyed by `namespace/name`.
    pub service_accounts: BTreeMap<String, ServiceAccount>,
    /// Persistent volume claims, keyed by `namespace/name`.
    pub claims: BTreeMap<String, PersistentVolumeClaim>,
    /// Pods, keyed by `namespace/name`.
    pub pods: BTreeMap<String, ResolvedPod>,
    /// Replicasets, keyed by `namespace/name`; `None` when the bundle predates the kind.
    ///
    /// `Option` here and on the four maps below is the compatibility choice carried through
    /// from [`infra_domain::observation::OPTIONAL_KINDS`]: an unscanned kind stays
    /// distinguishable from a scanned-and-empty one all the way into the digested model, so a
    /// consumer can refuse to reason about what nobody looked at. Older bundles therefore
    /// digest differently from newer ones even when the twelve original kinds agree — the model
    /// genuinely grew.
    pub replica_sets: Option<BTreeMap<String, ReplicaSet>>,
    /// Jobs, keyed by `namespace/name`; `None` when the bundle predates the kind.
    pub jobs: Option<BTreeMap<String, Job>>,
    /// Cronjobs, keyed by `namespace/name`; `None` when the bundle predates the kind.
    pub cron_jobs: Option<BTreeMap<String, CronJob>>,
    /// Pod disruption budgets, keyed by `namespace/name`; `None` when the bundle predates
    /// the kind.
    pub pod_disruption_budgets: Option<BTreeMap<String, PodDisruptionBudget>>,
    /// Horizontal pod autoscalers, keyed by `namespace/name`; `None` when the bundle predates
    /// the kind.
    ///
    /// A `scaleTargetRef` stays plain data rather than a handle, for [`ResolvedPod::owner`]'s
    /// reason: an autoscaler aimed at something unobserved is a *diagnosis* about the cluster
    /// (`INFRA-DIAG-018`), not a resolution failure of the document.
    pub horizontal_pod_autoscalers: Option<BTreeMap<String, HorizontalPodAutoscaler>>,
    /// Every dangling reference, sorted — the aggregate of every [`Reference::Unresolved`] in
    /// the maps above, plus the checks that have no site to live at (a selector matching
    /// nothing, a namespace nobody observed).
    pub unresolved: Vec<UnresolvedReference>,
}

/// Where the observation came from — outside the digested bytes, deliberately.
///
/// Everything here changes between two scans of an unchanged cluster, and none of it changes
/// what the cluster *is*. See the module doc.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Provenance {
    /// The kubeconfig context the scan targeted.
    pub context: String,
    /// When the scan ran.
    pub scanned_at: String,
    /// The scanner's version.
    pub scout_version: String,
    /// This compiler's version.
    pub compiler_version: String,
}

/// The compiled IR: the model, its provenance, and the total lookups.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InfraIr {
    /// Where the observation came from.
    pub provenance: Provenance,
    /// The digested content.
    pub model: InfraModel,
}

/// The persisted form of an IR: what `protocol infra compile --out` writes.
#[derive(Debug, Clone, Serialize)]
pub struct InfraIrDocument<'a> {
    /// The format claim, `infra-ir/1`.
    pub format: &'static str,
    /// Where the observation came from.
    pub provenance: &'a Provenance,
    /// The content digest of `model` — over the model's canonical bytes alone, so it is equal
    /// for two scans of an unchanged cluster.
    pub digest: String,
    /// The digested content.
    pub model: &'a InfraModel,
}

impl InfraIr {
    /// The content digest: the full SHA-256, 64 hex characters, over the model's canonical JSON.
    ///
    /// The full width and not a truncation, for the reason gap register D-4 settled for the
    /// specification digest: the moment a digest becomes an acceptance criterion, 64 bits is
    /// fine against drift and weak against construction.
    pub fn digest(&self) -> String {
        // Through `Value` first, deliberately: `serde_json::Value` maps are ordered by key, so
        // the canonical form is *key-sorted* compact JSON — reproducible by anyone who parses
        // the persisted document, where direct struct serialization would bake this crate's
        // field declaration order into the digest and no reader could recompute it.
        let value =
            serde_json::to_value(&self.model).expect("the model has no non-serializable state");
        let canonical = serde_json::to_vec(&value).expect("a value serializes");
        digest_of_canonical(&canonical)
    }

    /// The persistable document, digest computed.
    pub fn document(&self) -> InfraIrDocument<'_> {
        InfraIrDocument {
            format: IR_FORMAT,
            provenance: &self.provenance,
            digest: self.digest(),
            model: &self.model,
        }
    }
}

/// The digest of already-canonical model bytes — the construction [`InfraIr::digest`] uses,
/// exposed so a consumer verifying a persisted document computes the same one instead of
/// spelling its own. One owner per algorithm: a second spelling is a second place for the two
/// to disagree.
#[must_use]
pub fn digest_of_canonical(canonical: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    use std::fmt::Write as _;

    let hash = Sha256::digest(canonical);
    let mut rendered = String::with_capacity(64);
    for byte in &hash {
        let _ = write!(rendered, "{byte:02x}");
    }
    rendered
}
