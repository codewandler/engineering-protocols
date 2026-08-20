//! Reading a persisted `infra-ir/1` document back into a typed [`InfraIr`].
//!
//! # Why this is a validation and not a `Deserialize`
//!
//! The IR's guarantees are relational: a [`Reference::Resolved`] holds a handle whose lookup is
//! total, and a handle exists only because the compiler checked the key against the model it was
//! minting into. A derived `Deserialize` on the IR types would open a second door — any JSON
//! file could claim `"state": "resolved"` for a key no map holds, and the first total lookup
//! would panic on a document's say-so. So the persisted form comes back the same way a bundle
//! comes in: through private mirror types and an accumulating validation
//! ([`read_document`]), and the handles are re-minted only after their keys are checked.
//!
//! The digest is verified too, but it answers a *different* question. A digest matches whenever
//! it was computed over the bytes beside it, so it proves the document was not edited after it
//! was written — not that a compiler wrote it. Tamper detection is the digest's job
//! (`INFRA-IR-002`); keeping lookups total is the relational check's (`INFRA-IR-004`); neither
//! covers the other.
//!
//! # What is deliberately not re-checked
//!
//! Value-level rules that `infra-domain` enforced on the way in — digest well-formedness of
//! configmap keys, label-map string-ness — are not re-run here. They are content the digest
//! already covers for any honestly produced document, and none of them can turn a total lookup
//! into a panic. The unresolved-fact aggregate is likewise read as data, not re-derived: it is
//! the compiler's statement about the observation, and re-deriving it here would be a second
//! implementation of resolution for the two to disagree over.

use std::collections::BTreeMap;

use infra_domain::code::{InfraCode, ValidationErrors};
use infra_domain::config::{ConfigMap, Secret, ValueDigest};
use infra_domain::network::{Service, ServicePort};
use infra_domain::observation::{
    ClaimPhase, ContainerStatus, Identity, Namespace, Node, NodeInfo, OwnerRef,
    PersistentVolumeClaim, PodPhase, ServiceAccount,
};
use infra_domain::workload::{Probe, ProbeHandler, Probes, Resources, VolumeMount, WorkloadKind};
use serde::Deserialize;

use crate::ir::{
    digest_of_canonical, ClaimHandle, ConfigMapHandle, InfraIr, InfraModel, NodeHandle, Provenance,
    Reference, ResolvedContainer, ResolvedEnvFrom, ResolvedEnvFromSource, ResolvedEnvSource,
    ResolvedEnvVar, ResolvedIngress, ResolvedIngressBackend, ResolvedIngressPath,
    ResolvedIngressRule, ResolvedPod, ResolvedVolume, ResolvedVolumeSource, ResolvedWorkload,
    SecretHandle, ServiceAccountHandle, ServiceHandle, UnresolvedReference, UnresolvedTarget,
    IR_FORMAT,
};

/// Reads a persisted `infra-ir/1` document back into a typed IR, or refuses it.
///
/// One run reports every problem it can still reach: a digest mismatch and every dangling
/// handle arrive together. A document whose shape does not read as the format at all is refused
/// with `INFRA-IR-003` alone, because nothing behind an unreadable shape is checkable.
///
/// # Errors
///
/// [`ValidationErrors`] carrying `INFRA-IR-001` (wrong format), `INFRA-IR-002` (digest does not
/// match content), `INFRA-IR-003` (not the `infra-ir/1` shape) or `INFRA-IR-004` (a `resolved`
/// reference whose key its map does not hold).
pub fn read_document(value: &serde_json::Value) -> Result<InfraIr, ValidationErrors> {
    let mut errors = ValidationErrors::new();

    let declared = value.get("format").and_then(serde_json::Value::as_str);
    if declared != Some(IR_FORMAT) {
        errors.refuse(
            InfraCode::IrUnsupportedFormat,
            "format",
            format!(
                "`{}` is not a format this build reads; expected `{IR_FORMAT}`",
                declared.unwrap_or("<none>")
            ),
        );
        return Err(errors);
    }

    let document: DocumentMirror = match serde_json::from_value(value.clone()) {
        Ok(document) => document,
        Err(error) => {
            errors.refuse(InfraCode::IrMalformed, "<document>", error.to_string());
            return Err(errors);
        }
    };

    // The digest first, over the model exactly as persisted: compact key-sorted re-serialization
    // of the parsed value reproduces the canonical bytes the writer hashed.
    let canonical = match serde_json::to_vec(&document.model) {
        Ok(canonical) => canonical,
        Err(error) => {
            errors.refuse(InfraCode::IrMalformed, "model", error.to_string());
            return Err(errors);
        }
    };
    let recomputed = digest_of_canonical(&canonical);
    if recomputed != document.digest {
        errors.refuse(
            InfraCode::IrDigestMismatch,
            "digest",
            format!(
                "the digest does not match the content — claimed {}, computed {recomputed}; \
                 the document was edited after it was compiled",
                document.digest
            ),
        );
    }

    let model: ModelMirror = match serde_json::from_value(document.model) {
        Ok(model) => model,
        Err(error) => {
            errors.refuse(InfraCode::IrMalformed, "model", error.to_string());
            return Err(errors);
        }
    };

    let ir = model.into_ir(document.provenance, &mut errors);
    if errors.is_empty() {
        Ok(ir)
    } else {
        Err(errors)
    }
}

// ---------------------------------------------------------------------------------------------
// The mirrors. Private, field-for-field images of what `Serialize` writes, with
// `deny_unknown_fields` throughout: the document is machine-written, so an unknown field is not
// forward tolerance, it is a document some other version or some hand produced.
// ---------------------------------------------------------------------------------------------

/// The document envelope; the model stays a [`serde_json::Value`] until the digest is checked.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DocumentMirror {
    #[allow(
        dead_code,
        reason = "checked before the mirror parse, kept so the shape is complete"
    )]
    format: String,
    provenance: ProvenanceMirror,
    digest: String,
    model: serde_json::Value,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProvenanceMirror {
    context: String,
    scanned_at: String,
    scout_version: String,
    compiler_version: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ModelMirror {
    namespaces: BTreeMap<String, NamespaceMirror>,
    nodes: BTreeMap<String, NodeMirror>,
    workloads: BTreeMap<String, WorkloadMirror>,
    services: BTreeMap<String, ServiceMirror>,
    ingresses: BTreeMap<String, IngressMirror>,
    config_maps: BTreeMap<String, ConfigMapMirror>,
    secrets: BTreeMap<String, SecretMirror>,
    service_accounts: BTreeMap<String, ServiceAccountMirror>,
    claims: BTreeMap<String, ClaimMirror>,
    pods: BTreeMap<String, PodMirror>,
    unresolved: Vec<UnresolvedMirror>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct IdentityMirror {
    namespace: Option<String>,
    name: String,
    uid: String,
}

impl IdentityMirror {
    fn into_identity(self) -> Identity {
        Identity {
            namespace: self.namespace,
            name: self.name,
            uid: self.uid,
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NamespaceMirror {
    identity: IdentityMirror,
    labels: BTreeMap<String, String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NodeMirror {
    identity: IdentityMirror,
    labels: BTreeMap<String, String>,
    capacity: BTreeMap<String, String>,
    info: NodeInfoMirror,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NodeInfoMirror {
    architecture: Option<String>,
    container_runtime: Option<String>,
    kernel: Option<String>,
    kubelet: Option<String>,
    operating_system: Option<String>,
    os_image: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ServiceMirror {
    identity: IdentityMirror,
    labels: BTreeMap<String, String>,
    service_type: String,
    selector: BTreeMap<String, String>,
    ports: Vec<ServicePortMirror>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ServicePortMirror {
    name: Option<String>,
    port: u16,
    target_port: String,
    protocol: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ConfigMapMirror {
    identity: IdentityMirror,
    labels: BTreeMap<String, String>,
    keys: BTreeMap<String, ValueDigestMirror>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ValueDigestMirror {
    sha256: String,
    length: u64,
}

impl ValueDigestMirror {
    fn into_digest(self) -> ValueDigest {
        ValueDigest {
            sha256: self.sha256,
            length: self.length,
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SecretMirror {
    identity: IdentityMirror,
    labels: BTreeMap<String, String>,
    secret_type: String,
    keys: BTreeMap<String, ValueDigestMirror>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ServiceAccountMirror {
    identity: IdentityMirror,
    labels: BTreeMap<String, String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ClaimMirror {
    identity: IdentityMirror,
    labels: BTreeMap<String, String>,
    storage_class: Option<String>,
    access_modes: Vec<String>,
    requested_storage: Option<String>,
    phase: ClaimPhaseMirror,
}

#[derive(Deserialize, Clone, Copy)]
#[serde(rename_all = "snake_case")]
enum ClaimPhaseMirror {
    Pending,
    Bound,
    Lost,
    Unknown,
}

impl ClaimPhaseMirror {
    fn into_phase(self) -> ClaimPhase {
        match self {
            Self::Pending => ClaimPhase::Pending,
            Self::Bound => ClaimPhase::Bound,
            Self::Lost => ClaimPhase::Lost,
            Self::Unknown => ClaimPhase::Unknown,
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WorkloadMirror {
    kind: WorkloadKindMirror,
    identity: IdentityMirror,
    labels: BTreeMap<String, String>,
    replicas: Option<u32>,
    selector: BTreeMap<String, String>,
    governing_service: Option<ReferenceMirror>,
    service_account: ReferenceMirror,
    template_labels: BTreeMap<String, String>,
    containers: Vec<ContainerMirror>,
    volumes: Vec<VolumeMirror>,
}

#[derive(Deserialize, Clone, Copy)]
#[serde(rename_all = "snake_case")]
enum WorkloadKindMirror {
    Deployment,
    StatefulSet,
    DaemonSet,
}

impl WorkloadKindMirror {
    fn into_kind(self) -> WorkloadKind {
        match self {
            Self::Deployment => WorkloadKind::Deployment,
            Self::StatefulSet => WorkloadKind::StatefulSet,
            Self::DaemonSet => WorkloadKind::DaemonSet,
        }
    }
}

/// A reference as persisted: either arm is only a claim until the key is checked.
#[derive(Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
enum ReferenceMirror {
    Resolved { key: String },
    Unresolved { name: String },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ContainerMirror {
    name: String,
    image: String,
    env: Vec<EnvVarMirror>,
    env_from: Vec<EnvFromMirror>,
    volume_mounts: Vec<VolumeMountMirror>,
    probes: ProbesMirror,
    resources: ResourcesMirror,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EnvVarMirror {
    name: String,
    source: EnvSourceMirror,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum EnvSourceMirror {
    Literal {
        value: String,
    },
    ConfigMapKey {
        config_map: ReferenceMirror,
        key: String,
        optional: bool,
    },
    SecretKey {
        secret: ReferenceMirror,
        key: String,
        optional: bool,
    },
    FieldRef {
        path: String,
    },
    ResourceFieldRef,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EnvFromMirror {
    prefix: Option<String>,
    source: EnvFromSourceMirror,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum EnvFromSourceMirror {
    ConfigMap {
        config_map: ReferenceMirror,
        optional: bool,
    },
    Secret {
        secret: ReferenceMirror,
        optional: bool,
    },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct VolumeMountMirror {
    name: String,
    path: String,
    read_only: bool,
}

impl VolumeMountMirror {
    fn into_mount(self) -> VolumeMount {
        VolumeMount {
            name: self.name,
            path: self.path,
            read_only: self.read_only,
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProbesMirror {
    liveness: Option<ProbeMirror>,
    readiness: Option<ProbeMirror>,
    startup: Option<ProbeMirror>,
}

impl ProbesMirror {
    fn into_probes(self) -> Probes {
        Probes {
            liveness: self.liveness.map(ProbeMirror::into_probe),
            readiness: self.readiness.map(ProbeMirror::into_probe),
            startup: self.startup.map(ProbeMirror::into_probe),
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProbeMirror {
    handler: ProbeHandlerMirror,
    initial_delay_seconds: Option<u32>,
    period_seconds: Option<u32>,
    timeout_seconds: Option<u32>,
    failure_threshold: Option<u32>,
}

impl ProbeMirror {
    fn into_probe(self) -> Probe {
        Probe {
            handler: self.handler.into_handler(),
            initial_delay_seconds: self.initial_delay_seconds,
            period_seconds: self.period_seconds,
            timeout_seconds: self.timeout_seconds,
            failure_threshold: self.failure_threshold,
        }
    }
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum ProbeHandlerMirror {
    HttpGet {
        path: Option<String>,
        port: Option<String>,
    },
    Tcp {
        port: Option<String>,
    },
    Exec,
    Grpc,
    Unknown,
}

impl ProbeHandlerMirror {
    fn into_handler(self) -> ProbeHandler {
        match self {
            Self::HttpGet { path, port } => ProbeHandler::HttpGet { path, port },
            Self::Tcp { port } => ProbeHandler::Tcp { port },
            Self::Exec => ProbeHandler::Exec,
            Self::Grpc => ProbeHandler::Grpc,
            Self::Unknown => ProbeHandler::Unknown,
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ResourcesMirror {
    requests: BTreeMap<String, String>,
    limits: BTreeMap<String, String>,
}

impl ResourcesMirror {
    fn into_resources(self) -> Resources {
        Resources {
            requests: self.requests,
            limits: self.limits,
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct VolumeMirror {
    name: String,
    source: VolumeSourceMirror,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum VolumeSourceMirror {
    ConfigMap {
        config_map: ReferenceMirror,
        optional: bool,
    },
    Secret {
        secret: ReferenceMirror,
        optional: bool,
    },
    Claim {
        claim: ReferenceMirror,
    },
    EmptyDir,
    HostPath {
        path: Option<String>,
    },
    Other,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct IngressMirror {
    identity: IdentityMirror,
    labels: BTreeMap<String, String>,
    rules: Vec<IngressRuleMirror>,
    default_backend: Option<IngressBackendMirror>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct IngressRuleMirror {
    host: Option<String>,
    paths: Vec<IngressPathMirror>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct IngressPathMirror {
    path: Option<String>,
    path_type: Option<String>,
    backend: IngressBackendMirror,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct IngressBackendMirror {
    service: ReferenceMirror,
    port: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PodMirror {
    identity: IdentityMirror,
    labels: BTreeMap<String, String>,
    phase: PodPhaseMirror,
    ready: bool,
    node: Option<ReferenceMirror>,
    owner: Option<OwnerRefMirror>,
    containers: Vec<ContainerStatusMirror>,
}

#[derive(Deserialize, Clone, Copy)]
#[serde(rename_all = "snake_case")]
enum PodPhaseMirror {
    Pending,
    Running,
    Succeeded,
    Failed,
    Unknown,
}

impl PodPhaseMirror {
    fn into_phase(self) -> PodPhase {
        match self {
            Self::Pending => PodPhase::Pending,
            Self::Running => PodPhase::Running,
            Self::Succeeded => PodPhase::Succeeded,
            Self::Failed => PodPhase::Failed,
            Self::Unknown => PodPhase::Unknown,
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OwnerRefMirror {
    kind: String,
    name: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ContainerStatusMirror {
    name: String,
    ready: bool,
    restart_count: u32,
    waiting_reason: Option<String>,
}

impl ContainerStatusMirror {
    fn into_status(self) -> ContainerStatus {
        ContainerStatus {
            name: self.name,
            ready: self.ready,
            restart_count: self.restart_count,
            waiting_reason: self.waiting_reason,
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UnresolvedMirror {
    from: String,
    site: String,
    target: UnresolvedTargetMirror,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum UnresolvedTargetMirror {
    ConfigMap {
        name: String,
        optional: bool,
    },
    ConfigMapKey {
        name: String,
        key: String,
        optional: bool,
    },
    Secret {
        name: String,
        optional: bool,
    },
    SecretKey {
        name: String,
        key: String,
        optional: bool,
    },
    ServiceAccount {
        name: String,
    },
    Claim {
        name: String,
    },
    Service {
        name: String,
    },
    Node {
        name: String,
    },
    Namespace {
        name: String,
    },
    PodsMatchingSelector {
        selector: BTreeMap<String, String>,
    },
}

impl UnresolvedTargetMirror {
    fn into_target(self) -> UnresolvedTarget {
        match self {
            Self::ConfigMap { name, optional } => UnresolvedTarget::ConfigMap { name, optional },
            Self::ConfigMapKey {
                name,
                key,
                optional,
            } => UnresolvedTarget::ConfigMapKey {
                name,
                key,
                optional,
            },
            Self::Secret { name, optional } => UnresolvedTarget::Secret { name, optional },
            Self::SecretKey {
                name,
                key,
                optional,
            } => UnresolvedTarget::SecretKey {
                name,
                key,
                optional,
            },
            Self::ServiceAccount { name } => UnresolvedTarget::ServiceAccount { name },
            Self::Claim { name } => UnresolvedTarget::Claim { name },
            Self::Service { name } => UnresolvedTarget::Service { name },
            Self::Node { name } => UnresolvedTarget::Node { name },
            Self::Namespace { name } => UnresolvedTarget::Namespace { name },
            Self::PodsMatchingSelector { selector } => {
                UnresolvedTarget::PodsMatchingSelector { selector }
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Conversion: mint handles only where the key is present; refuse every claim that is not.
// ---------------------------------------------------------------------------------------------

/// Re-mints one reference against the map that must hold its key.
///
/// `exists` is a lookup into the already-parsed mirror maps and `mint` the crate-private handle
/// constructor; a `resolved` claim whose key the map does not hold is refused with the location,
/// because reading it as-is would turn a total lookup into a panic.
fn reference<H>(
    mirror: ReferenceMirror,
    exists: impl Fn(&str) -> bool,
    mint: impl Fn(String) -> H,
    what: &str,
    location: &str,
    errors: &mut ValidationErrors,
) -> Reference<H> {
    match mirror {
        ReferenceMirror::Resolved { key } => {
            if !exists(&key) {
                errors.refuse(
                    InfraCode::IrDanglingHandle,
                    location.to_owned(),
                    format!(
                        "the document claims `{key}` is a resolved {what}, but the model holds \
                         no such entry; no compilation produces this"
                    ),
                );
            }
            Reference::Resolved { key: mint(key) }
        }
        ReferenceMirror::Unresolved { name } => Reference::Unresolved { name },
    }
}

impl ModelMirror {
    /// Rebuilds the typed model, accumulating a refusal for every dangling `resolved` claim.
    ///
    /// The returned IR is only handed out by [`read_document`] when no refusal accumulated; the
    /// construction still completes either way so that one run reports every problem.
    #[allow(clippy::too_many_lines)]
    fn into_ir(self, provenance: ProvenanceMirror, errors: &mut ValidationErrors) -> InfraIr {
        let namespaces: BTreeMap<String, Namespace> = self
            .namespaces
            .into_iter()
            .map(|(key, mirror)| {
                (
                    key,
                    Namespace {
                        identity: mirror.identity.into_identity(),
                        labels: mirror.labels,
                    },
                )
            })
            .collect();
        let nodes: BTreeMap<String, Node> = self
            .nodes
            .into_iter()
            .map(|(key, mirror)| {
                (
                    key,
                    Node {
                        identity: mirror.identity.into_identity(),
                        labels: mirror.labels,
                        capacity: mirror.capacity,
                        info: NodeInfo {
                            architecture: mirror.info.architecture,
                            container_runtime: mirror.info.container_runtime,
                            kernel: mirror.info.kernel,
                            kubelet: mirror.info.kubelet,
                            operating_system: mirror.info.operating_system,
                            os_image: mirror.info.os_image,
                        },
                    },
                )
            })
            .collect();
        let services: BTreeMap<String, Service> = self
            .services
            .into_iter()
            .map(|(key, mirror)| {
                (
                    key,
                    Service {
                        identity: mirror.identity.into_identity(),
                        labels: mirror.labels,
                        service_type: mirror.service_type,
                        selector: mirror.selector,
                        ports: mirror
                            .ports
                            .into_iter()
                            .map(|port| ServicePort {
                                name: port.name,
                                port: port.port,
                                target_port: port.target_port,
                                protocol: port.protocol,
                            })
                            .collect(),
                    },
                )
            })
            .collect();
        let config_maps: BTreeMap<String, ConfigMap> = self
            .config_maps
            .into_iter()
            .map(|(key, mirror)| {
                (
                    key,
                    ConfigMap {
                        identity: mirror.identity.into_identity(),
                        labels: mirror.labels,
                        keys: mirror
                            .keys
                            .into_iter()
                            .map(|(name, digest)| (name, digest.into_digest()))
                            .collect(),
                    },
                )
            })
            .collect();
        let secrets: BTreeMap<String, Secret> = self
            .secrets
            .into_iter()
            .map(|(key, mirror)| {
                (
                    key,
                    Secret {
                        identity: mirror.identity.into_identity(),
                        labels: mirror.labels,
                        secret_type: mirror.secret_type,
                        keys: mirror
                            .keys
                            .into_iter()
                            .map(|(name, digest)| (name, digest.into_digest()))
                            .collect(),
                    },
                )
            })
            .collect();
        let service_accounts: BTreeMap<String, ServiceAccount> = self
            .service_accounts
            .into_iter()
            .map(|(key, mirror)| {
                (
                    key,
                    ServiceAccount {
                        identity: mirror.identity.into_identity(),
                        labels: mirror.labels,
                    },
                )
            })
            .collect();
        let claims: BTreeMap<String, PersistentVolumeClaim> = self
            .claims
            .into_iter()
            .map(|(key, mirror)| {
                (
                    key,
                    PersistentVolumeClaim {
                        identity: mirror.identity.into_identity(),
                        labels: mirror.labels,
                        storage_class: mirror.storage_class,
                        access_modes: mirror.access_modes,
                        requested_storage: mirror.requested_storage,
                        phase: mirror.phase.into_phase(),
                    },
                )
            })
            .collect();

        let workloads: BTreeMap<String, ResolvedWorkload> = self
            .workloads
            .into_iter()
            .map(|(key, mirror)| {
                let location = format!("model.workloads[{key}]");
                let governing_service = mirror.governing_service.map(|governing| {
                    reference(
                        governing,
                        |key| services.contains_key(key),
                        ServiceHandle::new,
                        "service",
                        &format!("{location}.governing_service"),
                        errors,
                    )
                });
                let service_account = reference(
                    mirror.service_account,
                    |key| service_accounts.contains_key(key),
                    ServiceAccountHandle::new,
                    "service account",
                    &format!("{location}.service_account"),
                    errors,
                );
                let containers = mirror
                    .containers
                    .into_iter()
                    .map(|container| {
                        container_from(container, &location, &config_maps, &secrets, errors)
                    })
                    .collect();
                let volumes = mirror
                    .volumes
                    .into_iter()
                    .map(|volume| {
                        let site = format!("{location}.volumes[{}]", volume.name);
                        let source = match volume.source {
                            VolumeSourceMirror::ConfigMap {
                                config_map,
                                optional,
                            } => ResolvedVolumeSource::ConfigMap {
                                config_map: reference(
                                    config_map,
                                    |key| config_maps.contains_key(key),
                                    ConfigMapHandle::new,
                                    "configmap",
                                    &site,
                                    errors,
                                ),
                                optional,
                            },
                            VolumeSourceMirror::Secret { secret, optional } => {
                                ResolvedVolumeSource::Secret {
                                    secret: reference(
                                        secret,
                                        |key| secrets.contains_key(key),
                                        SecretHandle::new,
                                        "secret",
                                        &site,
                                        errors,
                                    ),
                                    optional,
                                }
                            }
                            VolumeSourceMirror::Claim { claim } => ResolvedVolumeSource::Claim {
                                claim: reference(
                                    claim,
                                    |key| claims.contains_key(key),
                                    ClaimHandle::new,
                                    "persistent volume claim",
                                    &site,
                                    errors,
                                ),
                            },
                            VolumeSourceMirror::EmptyDir => ResolvedVolumeSource::EmptyDir,
                            VolumeSourceMirror::HostPath { path } => {
                                ResolvedVolumeSource::HostPath { path }
                            }
                            VolumeSourceMirror::Other => ResolvedVolumeSource::Other,
                        };
                        ResolvedVolume {
                            name: volume.name,
                            source,
                        }
                    })
                    .collect();
                (
                    key,
                    ResolvedWorkload {
                        kind: mirror.kind.into_kind(),
                        identity: mirror.identity.into_identity(),
                        labels: mirror.labels,
                        replicas: mirror.replicas,
                        selector: mirror.selector,
                        governing_service,
                        service_account,
                        template_labels: mirror.template_labels,
                        containers,
                        volumes,
                    },
                )
            })
            .collect();

        let ingresses: BTreeMap<String, ResolvedIngress> = self
            .ingresses
            .into_iter()
            .map(|(key, mirror)| {
                let location = format!("model.ingresses[{key}]");
                let mut backend =
                    |mirror: IngressBackendMirror, site: String| ResolvedIngressBackend {
                        service: reference(
                            mirror.service,
                            |key| services.contains_key(key),
                            ServiceHandle::new,
                            "service",
                            &site,
                            errors,
                        ),
                        port: mirror.port,
                    };
                let rules = mirror
                    .rules
                    .into_iter()
                    .enumerate()
                    .map(|(rule_index, rule)| ResolvedIngressRule {
                        host: rule.host,
                        paths: rule
                            .paths
                            .into_iter()
                            .enumerate()
                            .map(|(path_index, path)| ResolvedIngressPath {
                                path: path.path,
                                path_type: path.path_type,
                                backend: backend(
                                    path.backend,
                                    format!("{location}.rules[{rule_index}].paths[{path_index}]"),
                                ),
                            })
                            .collect(),
                    })
                    .collect();
                let default_backend = mirror
                    .default_backend
                    .map(|mirror| backend(mirror, format!("{location}.default_backend")));
                (
                    key,
                    ResolvedIngress {
                        identity: mirror.identity.into_identity(),
                        labels: mirror.labels,
                        rules,
                        default_backend,
                    },
                )
            })
            .collect();

        let pods: BTreeMap<String, ResolvedPod> = self
            .pods
            .into_iter()
            .map(|(key, mirror)| {
                let location = format!("model.pods[{key}]");
                let node = mirror.node.map(|node| {
                    reference(
                        node,
                        |key| nodes.contains_key(key),
                        NodeHandle::new,
                        "node",
                        &format!("{location}.node"),
                        errors,
                    )
                });
                (
                    key,
                    ResolvedPod {
                        identity: mirror.identity.into_identity(),
                        labels: mirror.labels,
                        phase: mirror.phase.into_phase(),
                        ready: mirror.ready,
                        node,
                        owner: mirror.owner.map(|owner| OwnerRef {
                            kind: owner.kind,
                            name: owner.name,
                        }),
                        containers: mirror
                            .containers
                            .into_iter()
                            .map(ContainerStatusMirror::into_status)
                            .collect(),
                    },
                )
            })
            .collect();

        let unresolved = self
            .unresolved
            .into_iter()
            .map(|mirror| UnresolvedReference {
                from: mirror.from,
                site: mirror.site,
                target: mirror.target.into_target(),
            })
            .collect();

        InfraIr {
            provenance: Provenance {
                context: provenance.context,
                scanned_at: provenance.scanned_at,
                scout_version: provenance.scout_version,
                compiler_version: provenance.compiler_version,
            },
            model: InfraModel {
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
                unresolved,
            },
        }
    }
}

/// Rebuilds one container, re-minting its configuration references. Long for `compile`'s
/// reason: a container has many reference sites, not a deep algorithm.
#[allow(clippy::too_many_lines)]
fn container_from(
    mirror: ContainerMirror,
    location: &str,
    config_maps: &BTreeMap<String, ConfigMap>,
    secrets: &BTreeMap<String, Secret>,
    errors: &mut ValidationErrors,
) -> ResolvedContainer {
    let env = mirror
        .env
        .into_iter()
        .map(|variable| {
            let site = format!(
                "{location}.containers[{}].env[{}]",
                mirror.name, variable.name
            );
            let source = match variable.source {
                EnvSourceMirror::Literal { value } => ResolvedEnvSource::Literal { value },
                EnvSourceMirror::ConfigMapKey {
                    config_map,
                    key,
                    optional,
                } => ResolvedEnvSource::ConfigMapKey {
                    config_map: reference(
                        config_map,
                        |key| config_maps.contains_key(key),
                        ConfigMapHandle::new,
                        "configmap",
                        &site,
                        errors,
                    ),
                    key,
                    optional,
                },
                EnvSourceMirror::SecretKey {
                    secret,
                    key,
                    optional,
                } => ResolvedEnvSource::SecretKey {
                    secret: reference(
                        secret,
                        |key| secrets.contains_key(key),
                        SecretHandle::new,
                        "secret",
                        &site,
                        errors,
                    ),
                    key,
                    optional,
                },
                EnvSourceMirror::FieldRef { path } => ResolvedEnvSource::FieldRef { path },
                EnvSourceMirror::ResourceFieldRef => ResolvedEnvSource::ResourceFieldRef,
            };
            ResolvedEnvVar {
                name: variable.name,
                source,
            }
        })
        .collect();
    let env_from = mirror
        .env_from
        .into_iter()
        .enumerate()
        .map(|(index, entry)| {
            let site = format!("{location}.containers[{}].envFrom[{index}]", mirror.name);
            let source = match entry.source {
                EnvFromSourceMirror::ConfigMap {
                    config_map,
                    optional,
                } => ResolvedEnvFromSource::ConfigMap {
                    config_map: reference(
                        config_map,
                        |key| config_maps.contains_key(key),
                        ConfigMapHandle::new,
                        "configmap",
                        &site,
                        errors,
                    ),
                    optional,
                },
                EnvFromSourceMirror::Secret { secret, optional } => ResolvedEnvFromSource::Secret {
                    secret: reference(
                        secret,
                        |key| secrets.contains_key(key),
                        SecretHandle::new,
                        "secret",
                        &site,
                        errors,
                    ),
                    optional,
                },
            };
            ResolvedEnvFrom {
                prefix: entry.prefix,
                source,
            }
        })
        .collect();
    ResolvedContainer {
        name: mirror.name,
        image: mirror.image,
        env,
        env_from,
        volume_mounts: mirror
            .volume_mounts
            .into_iter()
            .map(VolumeMountMirror::into_mount)
            .collect(),
        probes: mirror.probes.into_probes(),
        resources: mirror.resources.into_resources(),
    }
}
