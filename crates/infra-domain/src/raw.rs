//! The permissive half: what an `infra-observation/1` bundle deserializes into.
//!
//! Everything here implements [`Deserialize`] and nothing here is trusted. A
//! live cluster always carries more than the model — unknown fields are tolerated everywhere,
//! because refusing a field this crate has never heard of would make the model a denylist of the
//! Kubernetes API's future. What *is* checked is checked in one place, the [`TryFrom`] conversions
//! beside the validated types, and checked accumulating: one run reports every defect.
//!
//! Fields that could arrive malformed in ways the validation wants to name precisely — labels,
//! selectors, secret data values — deserialize as [`serde_json::Value`] maps rather than as typed
//! maps, so a non-string label is a refusal with a location instead of a serde error that aborts
//! the whole item.

use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::Value;

use crate::code::{InfraCode, ValidationErrors};

/// A whole observation bundle, exactly as the scanner wrote it.
#[derive(Debug, Clone, Deserialize)]
pub struct RawBundle {
    /// The format claim, `infra-observation/1`.
    #[serde(default)]
    pub format: String,
    /// The kubeconfig context the scan targeted.
    #[serde(default)]
    pub context: String,
    /// When the scan ran, RFC 3339 UTC.
    #[serde(default)]
    pub scanned_at: String,
    /// The scanner's version.
    #[serde(default)]
    pub scout_version: String,
    /// One `kubectl get -o json` list per kind, keyed by the scanner's kind name.
    ///
    /// A map of [`Value`]s rather than twelve typed fields, so a kind this model does not read is
    /// tolerated and a kind that is absent is *absent* — distinguishable from empty, which
    /// [`crate::code::InfraCode::MissingKind`] depends on.
    #[serde(default)]
    pub kinds: BTreeMap<String, Value>,
}

/// The `metadata` block every object carries.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct RawMeta {
    /// The object's name.
    #[serde(default)]
    pub name: Option<String>,
    /// The object's namespace, absent on cluster-scoped kinds.
    #[serde(default)]
    pub namespace: Option<String>,
    /// The API server's identity for the object.
    #[serde(default)]
    pub uid: Option<String>,
    /// Labels, values unchecked until validation.
    #[serde(default)]
    pub labels: BTreeMap<String, Value>,
    /// Owner references, read for the controller link on pods.
    #[serde(default, rename = "ownerReferences")]
    pub owner_references: Vec<RawOwnerRef>,
}

/// One entry of `metadata.ownerReferences`.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct RawOwnerRef {
    /// The owner's kind, such as `ReplicaSet`.
    #[serde(default)]
    pub kind: String,
    /// The owner's name.
    #[serde(default)]
    pub name: String,
    /// Whether this owner is the managing controller.
    #[serde(default)]
    pub controller: bool,
}

/// A namespace.
#[derive(Debug, Clone, Deserialize)]
pub struct RawNamespace {
    /// Identity and labels.
    #[serde(default)]
    pub metadata: RawMeta,
}

/// A node.
#[derive(Debug, Clone, Deserialize)]
pub struct RawNode {
    /// Identity and labels.
    #[serde(default)]
    pub metadata: RawMeta,
    /// The observed status; only capacity and node info are read.
    #[serde(default)]
    pub status: RawNodeStatus,
}

/// The slice of a node's status the model reads.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct RawNodeStatus {
    /// Resource capacity, quantities kept as the API's strings.
    #[serde(default)]
    pub capacity: BTreeMap<String, String>,
    /// Runtime and OS identification.
    #[serde(default, rename = "nodeInfo")]
    pub node_info: RawNodeInfo,
}

/// The `status.nodeInfo` block.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct RawNodeInfo {
    /// CPU architecture, such as `amd64`.
    #[serde(default)]
    pub architecture: Option<String>,
    /// Container runtime and version.
    #[serde(default, rename = "containerRuntimeVersion")]
    pub container_runtime_version: Option<String>,
    /// Kernel version.
    #[serde(default, rename = "kernelVersion")]
    pub kernel_version: Option<String>,
    /// Kubelet version.
    #[serde(default, rename = "kubeletVersion")]
    pub kubelet_version: Option<String>,
    /// Operating system, such as `linux`.
    #[serde(default, rename = "operatingSystem")]
    pub operating_system: Option<String>,
    /// OS image, such as a distribution name.
    #[serde(default, rename = "osImage")]
    pub os_image: Option<String>,
}

/// A deployment, statefulset or daemonset — one raw shape, because the API gives them one.
#[derive(Debug, Clone, Deserialize)]
pub struct RawWorkload {
    /// Identity and labels.
    #[serde(default)]
    pub metadata: RawMeta,
    /// The declared spec; only the modelled essentials are read.
    #[serde(default)]
    pub spec: RawWorkloadSpec,
}

/// The slice of a workload's spec the model reads.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct RawWorkloadSpec {
    /// Desired replicas. Absent on daemonsets, whose count is the node set's.
    #[serde(default)]
    pub replicas: Option<u32>,
    /// The pod selector.
    #[serde(default)]
    pub selector: Option<RawSelector>,
    /// A statefulset's governing (headless) service.
    #[serde(default, rename = "serviceName")]
    pub service_name: Option<String>,
    /// The pod template.
    #[serde(default)]
    pub template: Option<RawPodTemplate>,
}

/// A label selector; only `matchLabels` is modelled in v1 of the subset.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct RawSelector {
    /// Equality requirements, values unchecked until validation.
    #[serde(default, rename = "matchLabels")]
    pub match_labels: BTreeMap<String, Value>,
}

/// A workload's pod template.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct RawPodTemplate {
    /// The template's metadata; its labels are what selectors match.
    #[serde(default)]
    pub metadata: RawMeta,
    /// The pod spec.
    #[serde(default)]
    pub spec: RawPodSpec,
}

/// The slice of a pod spec the model reads.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct RawPodSpec {
    /// The service account the pods run as; `default` when absent.
    #[serde(default, rename = "serviceAccountName")]
    pub service_account_name: Option<String>,
    /// The containers.
    #[serde(default)]
    pub containers: Vec<RawContainer>,
    /// The volumes containers may mount.
    #[serde(default)]
    pub volumes: Vec<RawVolume>,
    /// Which node a pod was scheduled to. Read on pods, not on templates.
    #[serde(default, rename = "nodeName")]
    pub node_name: Option<String>,
}

/// One container of a pod template.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct RawContainer {
    /// The container's name.
    #[serde(default)]
    pub name: Option<String>,
    /// The image reference.
    #[serde(default)]
    pub image: Option<String>,
    /// Environment variables.
    #[serde(default)]
    pub env: Vec<RawEnvVar>,
    /// Whole-map environment sources.
    #[serde(default, rename = "envFrom")]
    pub env_from: Vec<RawEnvFrom>,
    /// Volume mounts.
    #[serde(default, rename = "volumeMounts")]
    pub volume_mounts: Vec<RawVolumeMount>,
    /// Liveness probe.
    #[serde(default, rename = "livenessProbe")]
    pub liveness_probe: Option<RawProbe>,
    /// Readiness probe.
    #[serde(default, rename = "readinessProbe")]
    pub readiness_probe: Option<RawProbe>,
    /// Startup probe.
    #[serde(default, rename = "startupProbe")]
    pub startup_probe: Option<RawProbe>,
    /// Requests and limits.
    #[serde(default)]
    pub resources: RawResources,
}

/// One environment variable.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct RawEnvVar {
    /// The variable's name.
    #[serde(default)]
    pub name: Option<String>,
    /// A literal value.
    #[serde(default)]
    pub value: Option<String>,
    /// A reference the value is read from instead.
    #[serde(default, rename = "valueFrom")]
    pub value_from: Option<RawEnvVarSource>,
}

/// Where an environment variable's value comes from.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct RawEnvVarSource {
    /// A key of a configmap.
    #[serde(default, rename = "configMapKeyRef")]
    pub config_map_key_ref: Option<RawKeySelector>,
    /// A key of a secret.
    #[serde(default, rename = "secretKeyRef")]
    pub secret_key_ref: Option<RawKeySelector>,
    /// A field of the pod itself.
    #[serde(default, rename = "fieldRef")]
    pub field_ref: Option<RawFieldRef>,
    /// A resource quantity of a container.
    #[serde(default, rename = "resourceFieldRef")]
    pub resource_field_ref: Option<Value>,
}

/// A `{name, key, optional}` reference into a configmap or secret.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct RawKeySelector {
    /// The configmap's or secret's name.
    #[serde(default)]
    pub name: Option<String>,
    /// The key inside it.
    #[serde(default)]
    pub key: Option<String>,
    /// Whether the reference may dangle without failing the pod.
    #[serde(default)]
    pub optional: bool,
}

/// A downward-API field reference.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct RawFieldRef {
    /// The referenced field path, such as `metadata.name`.
    #[serde(default, rename = "fieldPath")]
    pub field_path: Option<String>,
}

/// One `envFrom` source.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct RawEnvFrom {
    /// A prefix prepended to every imported key.
    #[serde(default)]
    pub prefix: Option<String>,
    /// A whole configmap.
    #[serde(default, rename = "configMapRef")]
    pub config_map_ref: Option<RawNameRef>,
    /// A whole secret.
    #[serde(default, rename = "secretRef")]
    pub secret_ref: Option<RawNameRef>,
}

/// A `{name, optional}` reference to a configmap or secret.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct RawNameRef {
    /// The referenced object's name.
    #[serde(default)]
    pub name: Option<String>,
    /// Whether the reference may dangle without failing the pod.
    #[serde(default)]
    pub optional: bool,
}

/// One volume mount of a container.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct RawVolumeMount {
    /// The volume's name.
    #[serde(default)]
    pub name: Option<String>,
    /// Where it is mounted.
    #[serde(default, rename = "mountPath")]
    pub mount_path: Option<String>,
    /// Whether the mount is read-only.
    #[serde(default, rename = "readOnly")]
    pub read_only: bool,
}

/// One volume of a pod template.
///
/// The sources are optional fields rather than an enum because that is the API's own shape; the
/// validated model turns them into one. A volume with a source this model does not read becomes
/// `Other` there — tolerated, not refused.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct RawVolume {
    /// The volume's name, what mounts refer to.
    #[serde(default)]
    pub name: Option<String>,
    /// A configmap source.
    #[serde(default, rename = "configMap")]
    pub config_map: Option<RawConfigMapVolume>,
    /// A secret source.
    #[serde(default)]
    pub secret: Option<RawSecretVolume>,
    /// A persistent volume claim source.
    #[serde(default, rename = "persistentVolumeClaim")]
    pub persistent_volume_claim: Option<RawClaimSource>,
    /// An ephemeral empty directory.
    #[serde(default, rename = "emptyDir")]
    pub empty_dir: Option<Value>,
    /// A host path.
    #[serde(default, rename = "hostPath")]
    pub host_path: Option<RawHostPath>,
}

/// A configmap volume source.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct RawConfigMapVolume {
    /// The configmap's name.
    #[serde(default)]
    pub name: Option<String>,
    /// Whether the reference may dangle without failing the pod.
    #[serde(default)]
    pub optional: bool,
}

/// A secret volume source.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct RawSecretVolume {
    /// The secret's name.
    #[serde(default, rename = "secretName")]
    pub secret_name: Option<String>,
    /// Whether the reference may dangle without failing the pod.
    #[serde(default)]
    pub optional: bool,
}

/// A persistent volume claim volume source.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct RawClaimSource {
    /// The claim's name.
    #[serde(default, rename = "claimName")]
    pub claim_name: Option<String>,
}

/// A host path volume source.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct RawHostPath {
    /// The path on the node.
    #[serde(default)]
    pub path: Option<String>,
}

/// A probe, any of the three kinds.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct RawProbe {
    /// An HTTP GET handler.
    #[serde(default, rename = "httpGet")]
    pub http_get: Option<RawHttpGet>,
    /// A TCP handler.
    #[serde(default, rename = "tcpSocket")]
    pub tcp_socket: Option<RawTcpSocket>,
    /// An exec handler; the command itself is not modelled.
    #[serde(default)]
    pub exec: Option<Value>,
    /// A gRPC handler.
    #[serde(default)]
    pub grpc: Option<Value>,
    /// Seconds before the first check.
    #[serde(default, rename = "initialDelaySeconds")]
    pub initial_delay_seconds: Option<u32>,
    /// Seconds between checks.
    #[serde(default, rename = "periodSeconds")]
    pub period_seconds: Option<u32>,
    /// Seconds before a check times out.
    #[serde(default, rename = "timeoutSeconds")]
    pub timeout_seconds: Option<u32>,
    /// Failures before the probe is considered failed.
    #[serde(default, rename = "failureThreshold")]
    pub failure_threshold: Option<u32>,
}

/// An HTTP GET probe handler.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct RawHttpGet {
    /// The request path.
    #[serde(default)]
    pub path: Option<String>,
    /// The port, a number or a named port.
    #[serde(default)]
    pub port: Option<Value>,
}

/// A TCP probe handler.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct RawTcpSocket {
    /// The port, a number or a named port.
    #[serde(default)]
    pub port: Option<Value>,
}

/// A container's resource requests and limits, quantities kept as the API's strings.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct RawResources {
    /// Requested quantities per resource.
    #[serde(default)]
    pub requests: BTreeMap<String, String>,
    /// Limit quantities per resource.
    #[serde(default)]
    pub limits: BTreeMap<String, String>,
}

/// A service.
#[derive(Debug, Clone, Deserialize)]
pub struct RawService {
    /// Identity and labels.
    #[serde(default)]
    pub metadata: RawMeta,
    /// The declared spec; type, selector and ports are read.
    #[serde(default)]
    pub spec: RawServiceSpec,
}

/// The slice of a service's spec the model reads.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct RawServiceSpec {
    /// The service type, such as `ClusterIP`.
    #[serde(default, rename = "type")]
    pub service_type: Option<String>,
    /// The pod selector, values unchecked until validation.
    #[serde(default)]
    pub selector: BTreeMap<String, Value>,
    /// The declared ports.
    #[serde(default)]
    pub ports: Vec<RawServicePort>,
}

/// One declared service port.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct RawServicePort {
    /// The port's name, required when a service declares several.
    #[serde(default)]
    pub name: Option<String>,
    /// The exposed port.
    #[serde(default)]
    pub port: Option<i64>,
    /// The pod-side port, a number or a named container port.
    #[serde(default, rename = "targetPort")]
    pub target_port: Option<Value>,
    /// The protocol, `TCP` when absent.
    #[serde(default)]
    pub protocol: Option<String>,
}

/// An ingress, `networking.k8s.io/v1` shape.
#[derive(Debug, Clone, Deserialize)]
pub struct RawIngress {
    /// Identity and labels.
    #[serde(default)]
    pub metadata: RawMeta,
    /// The declared rules.
    #[serde(default)]
    pub spec: RawIngressSpec,
}

/// The slice of an ingress spec the model reads.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct RawIngressSpec {
    /// The routing rules.
    #[serde(default)]
    pub rules: Vec<RawIngressRule>,
    /// The backend used when no rule matches.
    #[serde(default, rename = "defaultBackend")]
    pub default_backend: Option<RawIngressBackend>,
}

/// One ingress rule.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct RawIngressRule {
    /// The host the rule applies to.
    #[serde(default)]
    pub host: Option<String>,
    /// The HTTP paths.
    #[serde(default)]
    pub http: Option<RawIngressHttp>,
}

/// The `http` block of an ingress rule.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct RawIngressHttp {
    /// The paths.
    #[serde(default)]
    pub paths: Vec<RawIngressPath>,
}

/// One ingress path.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct RawIngressPath {
    /// The URL path.
    #[serde(default)]
    pub path: Option<String>,
    /// How the path matches.
    #[serde(default, rename = "pathType")]
    pub path_type: Option<String>,
    /// Where matching traffic goes.
    #[serde(default)]
    pub backend: Option<RawIngressBackend>,
}

/// An ingress backend.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct RawIngressBackend {
    /// The service form of a backend; resource backends are not modelled.
    #[serde(default)]
    pub service: Option<RawIngressServiceBackend>,
}

/// The service half of an ingress backend.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct RawIngressServiceBackend {
    /// The service's name.
    #[serde(default)]
    pub name: Option<String>,
    /// The service port, by name or number.
    #[serde(default)]
    pub port: Option<RawServiceBackendPort>,
}

/// An ingress backend's port.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct RawServiceBackendPort {
    /// The port's name.
    #[serde(default)]
    pub name: Option<String>,
    /// The port's number.
    #[serde(default)]
    pub number: Option<i64>,
}

/// A configmap.
#[derive(Debug, Clone, Deserialize)]
pub struct RawConfigMap {
    /// Identity and labels.
    #[serde(default)]
    pub metadata: RawMeta,
    /// Text entries; the values are hashed at validation and never enter the model.
    #[serde(default)]
    pub data: BTreeMap<String, Value>,
    /// Binary entries, base64 on the wire; hashed exactly as text entries are.
    #[serde(default, rename = "binaryData")]
    pub binary_data: BTreeMap<String, Value>,
}

/// A secret, already sanitized by the scanner: values are `{sha256, length}` digests.
///
/// Values deserialize as [`Value`] because whether each one *is* a digest object is the hard rule
/// [`crate::code::InfraCode::UnsanitizedSecret`] enforces, and a rule needs to see what it refuses.
#[derive(Debug, Clone, Deserialize)]
pub struct RawSecret {
    /// Identity and labels.
    #[serde(default)]
    pub metadata: RawMeta,
    /// The secret's type, such as `Opaque`.
    #[serde(default, rename = "type")]
    pub secret_type: Option<String>,
    /// Digest entries from `data`.
    #[serde(default)]
    pub data: BTreeMap<String, Value>,
    /// Digest entries from `stringData`.
    #[serde(default, rename = "stringData")]
    pub string_data: BTreeMap<String, Value>,
}

/// A service account.
#[derive(Debug, Clone, Deserialize)]
pub struct RawServiceAccount {
    /// Identity and labels.
    #[serde(default)]
    pub metadata: RawMeta,
}

/// A persistent volume claim.
#[derive(Debug, Clone, Deserialize)]
pub struct RawClaim {
    /// Identity and labels.
    #[serde(default)]
    pub metadata: RawMeta,
    /// The declared spec.
    #[serde(default)]
    pub spec: RawClaimSpec,
    /// The observed status; only the phase is read.
    #[serde(default)]
    pub status: RawClaimStatus,
}

/// The slice of a claim's status the model reads.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct RawClaimStatus {
    /// The lifecycle phase, `Bound` on a healthy claim.
    #[serde(default)]
    pub phase: Option<String>,
}

/// The slice of a claim's spec the model reads.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct RawClaimSpec {
    /// The storage class.
    #[serde(default, rename = "storageClassName")]
    pub storage_class_name: Option<String>,
    /// The requested access modes.
    #[serde(default, rename = "accessModes")]
    pub access_modes: Vec<String>,
    /// The resource request block; only `requests.storage` is read.
    #[serde(default)]
    pub resources: RawClaimResources,
}

/// A claim's resource block.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct RawClaimResources {
    /// Requested quantities, `storage` among them.
    #[serde(default)]
    pub requests: BTreeMap<String, String>,
}

/// A pod.
#[derive(Debug, Clone, Deserialize)]
pub struct RawPod {
    /// Identity, labels and owner references.
    #[serde(default)]
    pub metadata: RawMeta,
    /// The pod spec; only the node assignment is read here.
    #[serde(default)]
    pub spec: RawPodSpec,
    /// The observed status essentials.
    #[serde(default)]
    pub status: RawPodStatus,
}

/// The slice of a pod's status the model reads.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct RawPodStatus {
    /// The lifecycle phase.
    #[serde(default)]
    pub phase: Option<String>,
    /// Per-container readiness and restarts.
    #[serde(default, rename = "containerStatuses")]
    pub container_statuses: Vec<RawContainerStatus>,
}

/// One container's observed status.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct RawContainerStatus {
    /// The container's name.
    #[serde(default)]
    pub name: Option<String>,
    /// Whether it currently passes readiness.
    #[serde(default)]
    pub ready: bool,
    /// How often it restarted.
    #[serde(default, rename = "restartCount")]
    pub restart_count: u32,
    /// The current state block; only a waiting state's reason is read.
    #[serde(default)]
    pub state: RawContainerState,
}

/// A container status's `state` block.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct RawContainerState {
    /// Present while the container waits instead of running.
    #[serde(default)]
    pub waiting: Option<RawContainerWaiting>,
}

/// The waiting state's essentials; the free-text `message` is deliberately not read.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct RawContainerWaiting {
    /// The machine-readable reason, such as `CrashLoopBackOff`.
    #[serde(default)]
    pub reason: Option<String>,
}

/// Pulls one kind's items out of the bundle, refusing an absent kind and any item that does not
/// read as the kind's raw shape — and continuing past both.
pub(crate) fn items<T: serde::de::DeserializeOwned>(
    raw: &RawBundle,
    kind: &str,
    errors: &mut ValidationErrors,
) -> Vec<(String, T)> {
    let Some(list) = raw.kinds.get(kind) else {
        errors.refuse(
            InfraCode::MissingKind,
            format!("kinds.{kind}"),
            format!("the bundle carries no `{kind}` list; a scan that did not look is not an observation of nothing"),
        );
        return Vec::new();
    };
    let Some(entries) = list.get("items").and_then(Value::as_array) else {
        errors.refuse(
            InfraCode::MalformedObject,
            format!("kinds.{kind}"),
            "expected a Kubernetes List with an `items` array",
        );
        return Vec::new();
    };
    let mut parsed = Vec::with_capacity(entries.len());
    for (index, entry) in entries.iter().enumerate() {
        let location = format!("kinds.{kind}.items[{index}]");
        match serde_json::from_value::<T>(entry.clone()) {
            Ok(item) => parsed.push((location, item)),
            Err(error) => {
                errors.refuse(InfraCode::MalformedObject, location, error.to_string());
            }
        }
    }
    parsed
}
