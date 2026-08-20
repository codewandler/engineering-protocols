//! Workloads: deployments, statefulsets and daemonsets, reduced to pod-template essentials.
//!
//! The three kinds share one validated shape with the kind carried as data, because everything
//! IW2–IW4 ask of them — which containers, wired to what, running as whom, at how many replicas —
//! is the same question for all three. What differs (update strategies, claim retention,
//! pod management policy) is rollout mechanics, excluded from the subset.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::code::{InfraCode, ValidationErrors};
use crate::observation::{identity, port_string, string_map, Identity};
use crate::raw::{RawContainer, RawEnvVar, RawProbe, RawVolume, RawWorkload};

/// Which API kind a workload came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkloadKind {
    /// A deployment.
    Deployment,
    /// A statefulset.
    StatefulSet,
    /// A daemonset.
    DaemonSet,
}

impl WorkloadKind {
    /// The singular form used in IR keys, such as `deployment`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Deployment => "deployment",
            Self::StatefulSet => "statefulset",
            Self::DaemonSet => "daemonset",
        }
    }

    /// The bundle's kind key, such as `deployments`.
    pub fn plural(self) -> &'static str {
        match self {
            Self::Deployment => "deployments",
            Self::StatefulSet => "statefulsets",
            Self::DaemonSet => "daemonsets",
        }
    }
}

impl std::fmt::Display for WorkloadKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A workload: identity, scale, selection and the pod template essentials.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Workload {
    /// Which of the three kinds.
    pub kind: WorkloadKind,
    /// Identity.
    pub identity: Identity,
    /// Labels on the workload object itself.
    pub labels: BTreeMap<String, String>,
    /// Desired replicas. Absent on daemonsets, whose count is the node set's.
    pub replicas: Option<u32>,
    /// The pod selector's `matchLabels`.
    pub selector: BTreeMap<String, String>,
    /// A statefulset's governing service name.
    pub service_name: Option<String>,
    /// The pod template.
    pub template: PodTemplate,
}

/// The essentials of a pod template.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PodTemplate {
    /// The template's labels — what the workload's own selector matches.
    pub labels: BTreeMap<String, String>,
    /// The service account the pods run as; `default` was not filled in here, deliberately —
    /// the compiler resolves the absence, so the model records what the document said.
    pub service_account: Option<String>,
    /// The containers, in declared order.
    pub containers: Vec<Container>,
    /// The volumes, in declared order.
    pub volumes: Vec<Volume>,
}

/// One container of a template.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Container {
    /// The container's name.
    pub name: String,
    /// The image reference.
    pub image: String,
    /// Environment variables, in declared order.
    pub env: Vec<EnvVar>,
    /// Whole-map environment sources, in declared order.
    pub env_from: Vec<EnvFrom>,
    /// Volume mounts, in declared order.
    pub volume_mounts: Vec<VolumeMount>,
    /// The probes.
    pub probes: Probes,
    /// Requests and limits.
    pub resources: Resources,
}

/// One environment variable and where its value comes from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EnvVar {
    /// The variable's name.
    pub name: String,
    /// The source.
    pub source: EnvSource,
}

/// Where an environment variable's value comes from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EnvSource {
    /// A literal value, as declared. An env var with neither value nor reference is a literal
    /// empty string, which is what the kubelet gives it.
    Literal {
        /// The value.
        value: String,
    },
    /// A key of a configmap.
    ConfigMapKey {
        /// The configmap's name.
        name: String,
        /// The key.
        key: String,
        /// Whether the reference may dangle without failing the pod.
        optional: bool,
    },
    /// A key of a secret. The *reference* is semantic wiring; the value it would resolve to
    /// never appears anywhere in this model.
    SecretKey {
        /// The secret's name.
        name: String,
        /// The key.
        key: String,
        /// Whether the reference may dangle without failing the pod.
        optional: bool,
    },
    /// A downward-API field of the pod.
    FieldRef {
        /// The field path, such as `metadata.name`.
        path: String,
    },
    /// A resource quantity of a container; the details are not modelled.
    ResourceFieldRef,
}

/// One `envFrom` source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EnvFrom {
    /// A prefix prepended to every imported key.
    pub prefix: Option<String>,
    /// What is imported.
    pub source: EnvFromSource,
}

/// What an `envFrom` entry imports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EnvFromSource {
    /// A whole configmap.
    ConfigMap {
        /// Its name.
        name: String,
        /// Whether the reference may dangle without failing the pod.
        optional: bool,
    },
    /// A whole secret.
    Secret {
        /// Its name.
        name: String,
        /// Whether the reference may dangle without failing the pod.
        optional: bool,
    },
}

/// One volume mount.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VolumeMount {
    /// The volume's name.
    pub name: String,
    /// Where it is mounted.
    pub path: String,
    /// Whether the mount is read-only.
    pub read_only: bool,
}

/// A container's three probes, each optional.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Probes {
    /// Liveness.
    pub liveness: Option<Probe>,
    /// Readiness.
    pub readiness: Option<Probe>,
    /// Startup.
    pub startup: Option<Probe>,
}

/// One probe: its handler and its timing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Probe {
    /// What the probe does.
    pub handler: ProbeHandler,
    /// Seconds before the first check.
    pub initial_delay_seconds: Option<u32>,
    /// Seconds between checks.
    pub period_seconds: Option<u32>,
    /// Seconds before a check times out.
    pub timeout_seconds: Option<u32>,
    /// Failures before the probe is considered failed.
    pub failure_threshold: Option<u32>,
}

/// A probe's handler.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProbeHandler {
    /// An HTTP GET.
    HttpGet {
        /// The request path.
        path: Option<String>,
        /// The port, a number or a named port, rendered as text.
        port: Option<String>,
    },
    /// A TCP connect.
    Tcp {
        /// The port, a number or a named port, rendered as text.
        port: Option<String>,
    },
    /// An exec; the command is not modelled.
    Exec,
    /// A gRPC health check.
    Grpc,
    /// A handler shape this model does not read.
    Unknown,
}

/// Requests and limits, quantities kept as the API's strings.
///
/// Strings rather than parsed quantities, deliberately: `100m` and `0.1` are the same amount and
/// different bytes, and normalizing them is a semantic claim about Kubernetes quantity arithmetic
/// this wave does not need to make. What the API stated is what the digest covers.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Resources {
    /// Requested quantities per resource.
    pub requests: BTreeMap<String, String>,
    /// Limit quantities per resource.
    pub limits: BTreeMap<String, String>,
}

/// One volume of a template.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Volume {
    /// The volume's name, what mounts refer to.
    pub name: String,
    /// The source.
    pub source: VolumeSource,
}

/// A volume's source, reduced to the kinds that carry references.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum VolumeSource {
    /// A configmap.
    ConfigMap {
        /// Its name.
        name: String,
        /// Whether the reference may dangle without failing the pod.
        optional: bool,
    },
    /// A secret.
    Secret {
        /// Its name.
        name: String,
        /// Whether the reference may dangle without failing the pod.
        optional: bool,
    },
    /// A persistent volume claim.
    Claim {
        /// The claim's name.
        claim: String,
    },
    /// An ephemeral empty directory.
    EmptyDir,
    /// A path on the node.
    HostPath {
        /// The path.
        path: Option<String>,
    },
    /// A source this model does not read — projected volumes, CSI, downward API and their kind.
    /// Tolerated, because refusing them would make the subset a denylist of volume drivers.
    Other,
}

impl Workload {
    pub(crate) fn from_raw(
        raw: &RawWorkload,
        kind: WorkloadKind,
        location: &str,
        errors: &mut ValidationErrors,
    ) -> Option<Self> {
        let identity = identity(&raw.metadata, true, location, errors)?;
        let selector = raw
            .spec
            .selector
            .as_ref()
            .map(|selector| {
                string_map(
                    &selector.match_labels,
                    &format!("{location}.spec.selector.matchLabels"),
                    errors,
                )
            })
            .unwrap_or_default();

        let template = if let Some(template) = &raw.spec.template {
            let template_location = format!("{location}.spec.template");
            let containers: Vec<Container> = template
                .spec
                .containers
                .iter()
                .enumerate()
                .filter_map(|(index, container)| {
                    Container::from_raw(
                        container,
                        &format!("{template_location}.spec.containers[{index}]"),
                        errors,
                    )
                })
                .collect();
            if template.spec.containers.is_empty() {
                errors.refuse(
                    InfraCode::EmptyWorkload,
                    format!("{template_location}.spec.containers"),
                    "a workload whose template declares no containers runs nothing",
                );
            }
            PodTemplate {
                labels: string_map(
                    &template.metadata.labels,
                    &format!("{template_location}.metadata.labels"),
                    errors,
                ),
                service_account: template.spec.service_account_name.clone(),
                containers,
                volumes: template
                    .spec
                    .volumes
                    .iter()
                    .enumerate()
                    .filter_map(|(index, volume)| {
                        Volume::from_raw(
                            volume,
                            &format!("{template_location}.spec.volumes[{index}]"),
                            errors,
                        )
                    })
                    .collect(),
            }
        } else {
            errors.refuse(
                InfraCode::EmptyWorkload,
                format!("{location}.spec.template"),
                "a workload without a pod template runs nothing",
            );
            PodTemplate {
                labels: BTreeMap::new(),
                service_account: None,
                containers: Vec::new(),
                volumes: Vec::new(),
            }
        };

        Some(Self {
            kind,
            identity,
            labels: string_map(
                &raw.metadata.labels,
                &format!("{location}.metadata.labels"),
                errors,
            ),
            replicas: raw.spec.replicas,
            selector,
            service_name: raw.spec.service_name.clone(),
            template,
        })
    }
}

impl Container {
    fn from_raw(raw: &RawContainer, location: &str, errors: &mut ValidationErrors) -> Option<Self> {
        let mut complete = true;
        for (field, present) in [
            (
                "name",
                raw.name.as_deref().is_some_and(|name| !name.is_empty()),
            ),
            (
                "image",
                raw.image.as_deref().is_some_and(|image| !image.is_empty()),
            ),
        ] {
            if !present {
                errors.refuse(
                    InfraCode::MissingContainerField,
                    format!("{location}.{field}"),
                    format!("`{field}` is missing or empty"),
                );
                complete = false;
            }
        }
        if !complete {
            return None;
        }

        let env = raw
            .env
            .iter()
            .enumerate()
            .filter_map(|(index, variable)| {
                EnvVar::from_raw(variable, &format!("{location}.env[{index}]"), errors)
            })
            .collect();
        let env_from = raw
            .env_from
            .iter()
            .filter_map(|entry| {
                let source = if let Some(reference) = &entry.config_map_ref {
                    reference.name.clone().map(|name| EnvFromSource::ConfigMap {
                        name,
                        optional: reference.optional,
                    })
                } else {
                    entry.secret_ref.as_ref().and_then(|reference| {
                        reference.name.clone().map(|name| EnvFromSource::Secret {
                            name,
                            optional: reference.optional,
                        })
                    })
                };
                source.map(|source| EnvFrom {
                    prefix: entry.prefix.clone(),
                    source,
                })
            })
            .collect();
        let volume_mounts = raw
            .volume_mounts
            .iter()
            .filter_map(|mount| {
                Some(VolumeMount {
                    name: mount.name.clone()?,
                    path: mount.mount_path.clone().unwrap_or_default(),
                    read_only: mount.read_only,
                })
            })
            .collect();

        Some(Self {
            name: raw.name.clone().unwrap_or_default(),
            image: raw.image.clone().unwrap_or_default(),
            env,
            env_from,
            volume_mounts,
            probes: Probes {
                liveness: raw
                    .liveness_probe
                    .as_ref()
                    .map(|probe| Probe::from_raw(probe, location, errors)),
                readiness: raw
                    .readiness_probe
                    .as_ref()
                    .map(|probe| Probe::from_raw(probe, location, errors)),
                startup: raw
                    .startup_probe
                    .as_ref()
                    .map(|probe| Probe::from_raw(probe, location, errors)),
            },
            resources: Resources {
                requests: raw.resources.requests.clone(),
                limits: raw.resources.limits.clone(),
            },
        })
    }
}

impl EnvVar {
    fn from_raw(raw: &RawEnvVar, location: &str, errors: &mut ValidationErrors) -> Option<Self> {
        let Some(name) = raw.name.clone().filter(|name| !name.is_empty()) else {
            errors.refuse(
                InfraCode::MissingIdentity,
                format!("{location}.name"),
                "an environment variable without a name sets nothing",
            );
            return None;
        };
        let source = match &raw.value_from {
            None => EnvSource::Literal {
                value: raw.value.clone().unwrap_or_default(),
            },
            Some(reference) => {
                if let Some(key_ref) = &reference.config_map_key_ref {
                    EnvSource::ConfigMapKey {
                        name: key_ref.name.clone().unwrap_or_default(),
                        key: key_ref.key.clone().unwrap_or_default(),
                        optional: key_ref.optional,
                    }
                } else if let Some(key_ref) = &reference.secret_key_ref {
                    EnvSource::SecretKey {
                        name: key_ref.name.clone().unwrap_or_default(),
                        key: key_ref.key.clone().unwrap_or_default(),
                        optional: key_ref.optional,
                    }
                } else if let Some(field_ref) = &reference.field_ref {
                    EnvSource::FieldRef {
                        path: field_ref.field_path.clone().unwrap_or_default(),
                    }
                } else if reference.resource_field_ref.is_some() {
                    EnvSource::ResourceFieldRef
                } else {
                    // A `valueFrom` with no recognised branch: the kubelet would reject it, and
                    // treating it as an empty literal would invent a value nobody declared.
                    EnvSource::Literal {
                        value: raw.value.clone().unwrap_or_default(),
                    }
                }
            }
        };
        Some(Self { name, source })
    }
}

impl Probe {
    fn from_raw(raw: &RawProbe, location: &str, errors: &mut ValidationErrors) -> Self {
        let handler = if let Some(http) = &raw.http_get {
            ProbeHandler::HttpGet {
                path: http.path.clone(),
                port: http
                    .port
                    .as_ref()
                    .and_then(|port| port_string(port, location, errors)),
            }
        } else if let Some(tcp) = &raw.tcp_socket {
            ProbeHandler::Tcp {
                port: tcp
                    .port
                    .as_ref()
                    .and_then(|port| port_string(port, location, errors)),
            }
        } else if raw.exec.is_some() {
            ProbeHandler::Exec
        } else if raw.grpc.is_some() {
            ProbeHandler::Grpc
        } else {
            ProbeHandler::Unknown
        };
        Self {
            handler,
            initial_delay_seconds: raw.initial_delay_seconds,
            period_seconds: raw.period_seconds,
            timeout_seconds: raw.timeout_seconds,
            failure_threshold: raw.failure_threshold,
        }
    }
}

impl Volume {
    fn from_raw(raw: &RawVolume, location: &str, errors: &mut ValidationErrors) -> Option<Self> {
        let Some(name) = raw.name.clone().filter(|name| !name.is_empty()) else {
            errors.refuse(
                InfraCode::MissingIdentity,
                format!("{location}.name"),
                "a volume without a name cannot be mounted",
            );
            return None;
        };
        let source = if let Some(config_map) = &raw.config_map {
            VolumeSource::ConfigMap {
                name: config_map.name.clone().unwrap_or_default(),
                optional: config_map.optional,
            }
        } else if let Some(secret) = &raw.secret {
            VolumeSource::Secret {
                name: secret.secret_name.clone().unwrap_or_default(),
                optional: secret.optional,
            }
        } else if let Some(claim) = &raw.persistent_volume_claim {
            VolumeSource::Claim {
                claim: claim.claim_name.clone().unwrap_or_default(),
            }
        } else if raw.empty_dir.is_some() {
            VolumeSource::EmptyDir
        } else if let Some(host_path) = &raw.host_path {
            VolumeSource::HostPath {
                path: host_path.path.clone(),
            }
        } else {
            VolumeSource::Other
        };
        Some(Self { name, source })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workload(spec: &serde_json::Value) -> Result<Workload, ValidationErrors> {
        let raw: RawWorkload = serde_json::from_value(serde_json::json!({
            "metadata": { "name": "web", "namespace": "app", "uid": "u1" },
            "spec": spec,
        }))
        .expect("the raw workload parses");
        let mut errors = ValidationErrors::new();
        let validated = Workload::from_raw(&raw, WorkloadKind::Deployment, "w", &mut errors);
        if errors.is_empty() {
            Ok(validated.expect("no errors means a workload"))
        } else {
            Err(errors)
        }
    }

    #[test]
    fn a_template_without_containers_is_refused_as_an_empty_workload() {
        let errors = workload(&serde_json::json!({
            "template": { "spec": { "containers": [] } }
        }))
        .expect_err("nothing to run is refused");
        assert!(
            errors.contains(InfraCode::EmptyWorkload),
            "expected INFRA-WORKLOAD-001, got: {errors}"
        );
    }

    #[test]
    fn a_container_without_name_and_without_image_reports_both_fields_in_one_run() {
        let errors = workload(&serde_json::json!({
            "template": { "spec": { "containers": [ {} ] } }
        }))
        .expect_err("a faceless container is refused");
        let count = errors
            .as_slice()
            .iter()
            .filter(|error| error.code == InfraCode::MissingContainerField)
            .count();
        assert_eq!(count, 2, "name and image each get a refusal: {errors}");
    }

    #[test]
    fn env_and_volume_references_survive_validation_with_their_optionality() {
        let validated = workload(&serde_json::json!({
            "selector": { "matchLabels": { "app": "web" } },
            "template": { "spec": {
                "containers": [{
                    "name": "main", "image": "img:1",
                    "env": [
                        { "name": "PLAIN", "value": "x" },
                        { "name": "FROM_SECRET", "valueFrom": { "secretKeyRef": { "name": "creds", "key": "token", "optional": true } } }
                    ],
                    "envFrom": [ { "configMapRef": { "name": "settings" } } ]
                }],
                "volumes": [ { "name": "cfg", "configMap": { "name": "settings", "optional": true } } ]
            } }
        }))
        .expect("a well-formed workload validates");
        let container = &validated.template.containers[0];
        assert_eq!(
            container.env[1].source,
            EnvSource::SecretKey {
                name: "creds".into(),
                key: "token".into(),
                optional: true
            }
        );
        assert_eq!(
            validated.template.volumes[0].source,
            VolumeSource::ConfigMap {
                name: "settings".into(),
                optional: true
            }
        );
    }

    #[test]
    fn an_unrecognised_volume_source_is_tolerated_as_other_rather_than_refused() {
        let validated = workload(&serde_json::json!({
            "template": { "spec": {
                "containers": [{ "name": "main", "image": "img:1" }],
                "volumes": [ { "name": "creds", "projected": { "sources": [] } } ]
            } }
        }))
        .expect("an exotic volume driver is not a defect");
        assert_eq!(validated.template.volumes[0].source, VolumeSource::Other);
    }
}
