//! What moved between two compiled snapshots of one cluster.
//!
//! ```text
//! scan A                              scan B
//!   |                                   |
//! validate + compile                validate + compile
//!   |                                   |
//! InfraIr A                          InfraIr B
//!        \                            /
//!         +---------- drift ---------+
//!                      |
//!                  InfraDrift
//! ```
//!
//! # Declared state, not the runtime layer
//!
//! Pods are deliberately absent from the change vocabulary. A deployment's pods are renamed on
//! every rollout, and a drift report listing 1186 pods added and 1186 removed after a restart is
//! a report nobody reads twice — the same argument IW2 decision 5 made when it kept pods out of
//! the Mermaid rendering and left them in the canonical JSON. What a pod's churn *means* for the
//! declared state already shows up here: the workload's image, replica count and configuration
//! digests are all in the vocabulary, and `protocol infra diagnose` answers the runtime question
//! about either snapshot on its own.
//!
//! # No catch-all
//!
//! Every variant of [`InfraChange`] names a field or a member. Where a construct has more
//! comparable fields than are worth one variant each — a workload's labels, selector, service
//! account, governing service and volumes — the change is
//! [`WorkloadFieldChanged`](InfraChange::WorkloadFieldChanged) carrying a
//! [`WorkloadField`], which is a closed enum and not a string: a reader can enumerate what this
//! build can report, and a new comparable field cannot arrive as prose.
//!
//! # One refusal
//!
//! [`DriftRefusal::DifferentContext`], mirroring `ess-diff`'s single
//! `DifferentSystem`. Comparing two scans of one cluster is what this answers; comparing
//! `k3d-dev` with `production` produces a change list where every object was added and every
//! object was removed, which is true and useless.
//!
//! # Determinism
//!
//! Changes are sorted by their own ordering — category, then subject, then member — so the same
//! pair produces byte-identical bytes, and the ordering is a property of the document rather than
//! of the order the maps happened to be walked in.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use infra_compiler::{InfraIr, ResolvedContainer, ResolvedWorkload};
use infra_domain::config::{ConfigMap, Secret};
use infra_domain::network::Service;
use infra_domain::observation::PersistentVolumeClaim;
use serde::Serialize;

/// The format string a persisted drift document carries.
pub const DRIFT_FORMAT: &str = "infra-drift/1";

/// Why two snapshots cannot be compared at all.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "refusal", rename_all = "snake_case")]
pub enum DriftRefusal {
    /// The two scans targeted different kubeconfig contexts.
    DifferentContext {
        /// The context the `from` snapshot was scanned in.
        from: String,
        /// The context the `to` snapshot was scanned in.
        to: String,
    },
}

impl fmt::Display for DriftRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DifferentContext { from, to } => write!(
                f,
                "the two snapshots were scanned in different contexts, `{from}` and `{to}`: \
                 drift answers what moved between two scans of one cluster"
            ),
        }
    }
}

/// A comparable field of a workload that has no variant of its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkloadField {
    /// The labels on the workload object.
    Labels,
    /// The pod selector.
    Selector,
    /// The pod template's labels.
    TemplateLabels,
    /// The service account the pods run as.
    ServiceAccount,
    /// A statefulset's governing service.
    GoverningService,
    /// The template's volumes.
    Volumes,
}

impl WorkloadField {
    /// The field as it is printed.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Labels => "labels",
            Self::Selector => "selector",
            Self::TemplateLabels => "template labels",
            Self::ServiceAccount => "service account",
            Self::GoverningService => "governing service",
            Self::Volumes => "volumes",
        }
    }
}

impl fmt::Display for WorkloadField {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A comparable field of a service that has no variant of its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ServiceField {
    /// The pod selector.
    Selector,
    /// The declared ports.
    Ports,
    /// The service type.
    ServiceType,
    /// The labels.
    Labels,
}

impl ServiceField {
    /// The field as it is printed.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Selector => "selector",
            Self::Ports => "ports",
            Self::ServiceType => "type",
            Self::Labels => "labels",
        }
    }
}

impl fmt::Display for ServiceField {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Which kind of object an add or a remove is about, so the two verbs need one variant each
/// rather than one per kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MemberKind {
    /// A namespace.
    Namespace,
    /// A cluster node.
    Node,
    /// A workload.
    Workload,
    /// A service.
    Service,
    /// An ingress.
    Ingress,
    /// A configmap.
    ConfigMap,
    /// A secret.
    Secret,
    /// A persistent volume claim.
    Claim,
}

impl MemberKind {
    /// The kind as it is printed, and as the IR's map name spells it.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Namespace => "namespace",
            Self::Node => "node",
            Self::Workload => "workload",
            Self::Service => "service",
            Self::Ingress => "ingress",
            Self::ConfigMap => "configmap",
            Self::Secret => "secret",
            Self::Claim => "claim",
        }
    }
}

impl fmt::Display for MemberKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One typed difference between two snapshots.
///
/// Ordered by declaration, then by subject: `Ord` is derived and the variant order below is the
/// document's ordering contract, which is why additions and removals lead — a reader wants the
/// membership change before the field change inside a member that survived.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(tag = "change", rename_all = "snake_case")]
pub enum InfraChange {
    /// An object the `to` snapshot holds and the `from` snapshot did not.
    Added {
        /// Which kind.
        kind: MemberKind,
        /// Its IR key.
        subject: String,
    },
    /// An object the `from` snapshot held and the `to` snapshot does not.
    Removed {
        /// Which kind.
        kind: MemberKind,
        /// Its IR key.
        subject: String,
    },
    /// A workload's declared replica count moved.
    ReplicasChanged {
        /// The workload's IR key.
        subject: String,
        /// What it declared before; absent on a daemonset.
        from: Option<u32>,
        /// What it declares now.
        to: Option<u32>,
    },
    /// A container appeared in a workload that survived.
    ContainerAdded {
        /// The workload's IR key.
        subject: String,
        /// The container's name.
        container: String,
    },
    /// A container disappeared from a workload that survived.
    ContainerRemoved {
        /// The workload's IR key.
        subject: String,
        /// The container's name.
        container: String,
    },
    /// A container's image reference moved — the change a deploy is.
    ImageChanged {
        /// The workload's IR key.
        subject: String,
        /// The container's name.
        container: String,
        /// The image before.
        from: String,
        /// The image now.
        to: String,
    },
    /// A container's requests or limits moved.
    ResourcesChanged {
        /// The workload's IR key.
        subject: String,
        /// The container's name.
        container: String,
    },
    /// A container's probes moved.
    ProbesChanged {
        /// The workload's IR key.
        subject: String,
        /// The container's name.
        container: String,
    },
    /// A container's environment — variables or whole-map imports — moved.
    EnvironmentChanged {
        /// The workload's IR key.
        subject: String,
        /// The container's name.
        container: String,
    },
    /// A workload field with no variant of its own moved.
    WorkloadFieldChanged {
        /// The workload's IR key.
        subject: String,
        /// Which field.
        field: WorkloadField,
    },
    /// A service field moved.
    ServiceFieldChanged {
        /// The service's IR key.
        subject: String,
        /// Which field.
        field: ServiceField,
    },
    /// An ingress's rules or default backend moved.
    IngressRoutingChanged {
        /// The ingress's IR key.
        subject: String,
    },
    /// A configmap's or secret's key set or content digests moved — the config-hash change.
    ///
    /// Content, never values: the IR carries `{sha256, length}` per key and nothing else, so this
    /// says *that* a value changed and can never say what it changed to.
    ConfigContentChanged {
        /// Which kind — configmap or secret.
        kind: MemberKind,
        /// Its IR key.
        subject: String,
        /// Keys the `to` snapshot has and the `from` snapshot did not.
        added_keys: Vec<String>,
        /// Keys the `from` snapshot had and the `to` snapshot does not.
        removed_keys: Vec<String>,
        /// Keys both hold whose digests differ.
        changed_keys: Vec<String>,
    },
    /// A claim's binding phase moved.
    ClaimPhaseChanged {
        /// The claim's IR key.
        subject: String,
        /// The phase before, as the API stated it.
        from: String,
        /// The phase now.
        to: String,
    },
    /// A reference that resolved in the `from` snapshot dangles in the `to` snapshot.
    ReferenceBroke {
        /// The IR entry holding the reference.
        subject: String,
        /// Where inside it.
        site: String,
        /// What it names.
        target: String,
    },
    /// A reference that dangled in the `from` snapshot resolves in the `to` snapshot.
    ReferenceHealed {
        /// The IR entry holding the reference.
        subject: String,
        /// Where inside it.
        site: String,
        /// What it named.
        target: String,
    },
}

impl fmt::Display for InfraChange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Added { kind, subject } => write!(f, "{kind} {subject} added"),
            Self::Removed { kind, subject } => write!(f, "{kind} {subject} removed"),
            Self::ReplicasChanged { subject, from, to } => write!(
                f,
                "{subject} replicas {} -> {}",
                render_replicas(*from),
                render_replicas(*to)
            ),
            Self::ContainerAdded { subject, container } => {
                write!(f, "{subject} container {container} added")
            }
            Self::ContainerRemoved { subject, container } => {
                write!(f, "{subject} container {container} removed")
            }
            Self::ImageChanged {
                subject,
                container,
                from,
                to,
            } => write!(f, "{subject} container {container} image {from} -> {to}"),
            Self::ResourcesChanged { subject, container } => {
                write!(f, "{subject} container {container} resources changed")
            }
            Self::ProbesChanged { subject, container } => {
                write!(f, "{subject} container {container} probes changed")
            }
            Self::EnvironmentChanged { subject, container } => {
                write!(f, "{subject} container {container} environment changed")
            }
            Self::WorkloadFieldChanged { subject, field } => {
                write!(f, "{subject} {field} changed")
            }
            Self::ServiceFieldChanged { subject, field } => {
                write!(f, "{subject} {field} changed")
            }
            Self::IngressRoutingChanged { subject } => write!(f, "{subject} routing changed"),
            Self::ConfigContentChanged {
                kind,
                subject,
                added_keys,
                removed_keys,
                changed_keys,
            } => write!(
                f,
                "{kind} {subject} content changed (+{} -{} ~{})",
                added_keys.len(),
                removed_keys.len(),
                changed_keys.len()
            ),
            Self::ClaimPhaseChanged { subject, from, to } => {
                write!(f, "{subject} phase {from} -> {to}")
            }
            Self::ReferenceBroke {
                subject,
                site,
                target,
            } => write!(f, "{subject} {site} no longer resolves {target}"),
            Self::ReferenceHealed {
                subject,
                site,
                target,
            } => write!(f, "{subject} {site} now resolves {target}"),
        }
    }
}

/// A replica count, or the word for a workload that declares none.
fn render_replicas(replicas: Option<u32>) -> String {
    replicas.map_or_else(|| "undeclared".to_owned(), |count| count.to_string())
}

/// Which snapshot a side of a drift report is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DriftSideRef {
    /// The kubeconfig context the scan targeted.
    pub context: String,
    /// The IR's content digest.
    pub digest: String,
}

/// What moved between two snapshots of one cluster.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InfraDrift {
    /// The format claim, `infra-drift/1`.
    pub format: &'static str,
    /// The snapshot compared from.
    pub from: DriftSideRef,
    /// The snapshot compared to.
    pub to: DriftSideRef,
    /// Every change, in the format's own order.
    pub changes: Vec<InfraChange>,
}

impl InfraDrift {
    /// The canonical JSON document, key-sorted, with a trailing newline.
    #[must_use]
    pub fn to_json(&self) -> String {
        let value =
            serde_json::to_value(self).expect("a drift report has no non-serializable state");
        let mut rendered =
            serde_json::to_string_pretty(&value).expect("a JSON value renders as JSON");
        rendered.push('\n');
        rendered
    }

    /// `true` when the two snapshots are semantically identical.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }
}

/// Compares two snapshots of one cluster.
///
/// # Errors
///
/// [`DriftRefusal::DifferentContext`] when the two scans targeted different clusters.
pub fn drift(from: &InfraIr, to: &InfraIr) -> Result<InfraDrift, DriftRefusal> {
    if from.provenance.context != to.provenance.context {
        return Err(DriftRefusal::DifferentContext {
            from: from.provenance.context.clone(),
            to: to.provenance.context.clone(),
        });
    }

    let mut changes = Vec::new();
    membership(
        MemberKind::Namespace,
        &keys(&from.model.namespaces),
        &keys(&to.model.namespaces),
        &mut changes,
    );
    membership(
        MemberKind::Node,
        &keys(&from.model.nodes),
        &keys(&to.model.nodes),
        &mut changes,
    );
    membership(
        MemberKind::Claim,
        &keys(&from.model.claims),
        &keys(&to.model.claims),
        &mut changes,
    );
    workloads(from, to, &mut changes);
    services(from, to, &mut changes);
    ingresses(from, to, &mut changes);
    config_maps(from, to, &mut changes);
    secrets(from, to, &mut changes);
    claims(from, to, &mut changes);
    references(from, to, &mut changes);

    changes.sort();
    changes.dedup();
    Ok(InfraDrift {
        format: DRIFT_FORMAT,
        from: DriftSideRef {
            context: from.provenance.context.clone(),
            digest: from.digest(),
        },
        to: DriftSideRef {
            context: to.provenance.context.clone(),
            digest: to.digest(),
        },
        changes,
    })
}

/// The key set of one of the IR's maps.
fn keys<T>(map: &BTreeMap<String, T>) -> BTreeSet<&str> {
    map.keys().map(String::as_str).collect()
}

/// Records what appeared and what disappeared between two key sets.
fn membership(
    kind: MemberKind,
    before: &BTreeSet<&str>,
    after: &BTreeSet<&str>,
    changes: &mut Vec<InfraChange>,
) {
    for key in after.difference(before) {
        changes.push(InfraChange::Added {
            kind,
            subject: (*key).to_owned(),
        });
    }
    for key in before.difference(after) {
        changes.push(InfraChange::Removed {
            kind,
            subject: (*key).to_owned(),
        });
    }
}

/// Workload membership, then every field of the ones that survived.
fn workloads(from: &InfraIr, to: &InfraIr, changes: &mut Vec<InfraChange>) {
    membership(
        MemberKind::Workload,
        &keys(&from.model.workloads),
        &keys(&to.model.workloads),
        changes,
    );
    for (key, before) in &from.model.workloads {
        let Some(after) = to.model.workloads.get(key) else {
            continue;
        };
        workload_fields(key, before, after, changes);
        containers(key, before, after, changes);
    }
}

/// The workload fields that are not a container's.
fn workload_fields(
    key: &str,
    before: &ResolvedWorkload,
    after: &ResolvedWorkload,
    changes: &mut Vec<InfraChange>,
) {
    if before.replicas != after.replicas {
        changes.push(InfraChange::ReplicasChanged {
            subject: key.to_owned(),
            from: before.replicas,
            to: after.replicas,
        });
    }
    for (field, moved) in [
        (WorkloadField::Labels, before.labels != after.labels),
        (WorkloadField::Selector, before.selector != after.selector),
        (
            WorkloadField::TemplateLabels,
            before.template_labels != after.template_labels,
        ),
        (
            WorkloadField::ServiceAccount,
            before.service_account != after.service_account,
        ),
        (
            WorkloadField::GoverningService,
            before.governing_service != after.governing_service,
        ),
        (WorkloadField::Volumes, before.volumes != after.volumes),
    ] {
        if moved {
            changes.push(InfraChange::WorkloadFieldChanged {
                subject: key.to_owned(),
                field,
            });
        }
    }
}

/// Containers are compared by name, not by position: reordering a template's containers changes
/// nothing about the system, and a positional comparison would report every one of them moved.
fn containers(
    key: &str,
    before: &ResolvedWorkload,
    after: &ResolvedWorkload,
    changes: &mut Vec<InfraChange>,
) {
    let old: BTreeMap<&str, &ResolvedContainer> = before
        .containers
        .iter()
        .map(|container| (container.name.as_str(), container))
        .collect();
    let new: BTreeMap<&str, &ResolvedContainer> = after
        .containers
        .iter()
        .map(|container| (container.name.as_str(), container))
        .collect();

    for name in new.keys().filter(|name| !old.contains_key(*name)) {
        changes.push(InfraChange::ContainerAdded {
            subject: key.to_owned(),
            container: (*name).to_owned(),
        });
    }
    for name in old.keys().filter(|name| !new.contains_key(*name)) {
        changes.push(InfraChange::ContainerRemoved {
            subject: key.to_owned(),
            container: (*name).to_owned(),
        });
    }
    for (name, old_container) in &old {
        let Some(new_container) = new.get(name) else {
            continue;
        };
        if old_container.image != new_container.image {
            changes.push(InfraChange::ImageChanged {
                subject: key.to_owned(),
                container: (*name).to_owned(),
                from: old_container.image.clone(),
                to: new_container.image.clone(),
            });
        }
        if old_container.resources != new_container.resources {
            changes.push(InfraChange::ResourcesChanged {
                subject: key.to_owned(),
                container: (*name).to_owned(),
            });
        }
        if old_container.probes != new_container.probes {
            changes.push(InfraChange::ProbesChanged {
                subject: key.to_owned(),
                container: (*name).to_owned(),
            });
        }
        if old_container.env != new_container.env
            || old_container.env_from != new_container.env_from
        {
            changes.push(InfraChange::EnvironmentChanged {
                subject: key.to_owned(),
                container: (*name).to_owned(),
            });
        }
    }
}

/// Service membership and fields.
fn services(from: &InfraIr, to: &InfraIr, changes: &mut Vec<InfraChange>) {
    membership(
        MemberKind::Service,
        &keys(&from.model.services),
        &keys(&to.model.services),
        changes,
    );
    for (key, before) in &from.model.services {
        let Some(after) = to.model.services.get(key) else {
            continue;
        };
        service_fields(key, before, after, changes);
    }
}

/// The comparable fields of a service.
fn service_fields(key: &str, before: &Service, after: &Service, changes: &mut Vec<InfraChange>) {
    for (field, moved) in [
        (ServiceField::Selector, before.selector != after.selector),
        (ServiceField::Ports, before.ports != after.ports),
        (
            ServiceField::ServiceType,
            before.service_type != after.service_type,
        ),
        (ServiceField::Labels, before.labels != after.labels),
    ] {
        if moved {
            changes.push(InfraChange::ServiceFieldChanged {
                subject: key.to_owned(),
                field,
            });
        }
    }
}

/// Ingress membership and routing.
fn ingresses(from: &InfraIr, to: &InfraIr, changes: &mut Vec<InfraChange>) {
    membership(
        MemberKind::Ingress,
        &keys(&from.model.ingresses),
        &keys(&to.model.ingresses),
        changes,
    );
    for (key, before) in &from.model.ingresses {
        let Some(after) = to.model.ingresses.get(key) else {
            continue;
        };
        // Routing only: an ingress's labels move without a request going anywhere else, and
        // there is no `IngressField` because rules and the default backend are one answer to one
        // question — where does traffic go.
        if before.rules != after.rules || before.default_backend != after.default_backend {
            changes.push(InfraChange::IngressRoutingChanged {
                subject: key.to_owned(),
            });
        }
    }
}

/// Configmap membership and content.
fn config_maps(from: &InfraIr, to: &InfraIr, changes: &mut Vec<InfraChange>) {
    membership(
        MemberKind::ConfigMap,
        &keys(&from.model.config_maps),
        &keys(&to.model.config_maps),
        changes,
    );
    for (key, before) in &from.model.config_maps {
        let Some(after) = to.model.config_maps.get(key) else {
            continue;
        };
        content(
            MemberKind::ConfigMap,
            key,
            &digests(before),
            &digests(after),
            changes,
        );
    }
}

/// Secret membership and content.
fn secrets(from: &InfraIr, to: &InfraIr, changes: &mut Vec<InfraChange>) {
    membership(
        MemberKind::Secret,
        &keys(&from.model.secrets),
        &keys(&to.model.secrets),
        changes,
    );
    for (key, before) in &from.model.secrets {
        let Some(after) = to.model.secrets.get(key) else {
            continue;
        };
        content(
            MemberKind::Secret,
            key,
            &secret_digests(before),
            &secret_digests(after),
            changes,
        );
    }
}

/// A configmap's keys and the digest of each value.
fn digests(config_map: &ConfigMap) -> BTreeMap<&str, &str> {
    config_map
        .keys
        .iter()
        .map(|(key, digest)| (key.as_str(), digest.sha256.as_str()))
        .collect()
}

/// A secret's keys and the digest of each value. Values never existed here to compare.
fn secret_digests(secret: &Secret) -> BTreeMap<&str, &str> {
    secret
        .keys
        .iter()
        .map(|(key, digest)| (key.as_str(), digest.sha256.as_str()))
        .collect()
}

/// Records one config-content change when the key sets or any digest moved.
fn content(
    kind: MemberKind,
    key: &str,
    before: &BTreeMap<&str, &str>,
    after: &BTreeMap<&str, &str>,
    changes: &mut Vec<InfraChange>,
) {
    let added_keys: Vec<String> = after
        .keys()
        .filter(|name| !before.contains_key(*name))
        .map(|name| (*name).to_owned())
        .collect();
    let removed_keys: Vec<String> = before
        .keys()
        .filter(|name| !after.contains_key(*name))
        .map(|name| (*name).to_owned())
        .collect();
    let changed_keys: Vec<String> = before
        .iter()
        .filter(|(name, digest)| after.get(*name).is_some_and(|current| current != *digest))
        .map(|(name, _)| (*name).to_owned())
        .collect();
    if added_keys.is_empty() && removed_keys.is_empty() && changed_keys.is_empty() {
        return;
    }
    changes.push(InfraChange::ConfigContentChanged {
        kind,
        subject: key.to_owned(),
        added_keys,
        removed_keys,
        changed_keys,
    });
}

/// A claim's binding phase.
fn claims(from: &InfraIr, to: &InfraIr, changes: &mut Vec<InfraChange>) {
    for (key, before) in &from.model.claims {
        let Some(after) = to.model.claims.get(key) else {
            continue;
        };
        if before.phase != after.phase {
            changes.push(InfraChange::ClaimPhaseChanged {
                subject: key.to_owned(),
                from: phase(before),
                to: phase(after),
            });
        }
    }
}

/// A claim's phase as the API spells it.
fn phase(claim: &PersistentVolumeClaim) -> String {
    claim.phase.as_str().to_owned()
}

/// References that broke and references that healed.
fn references(from: &InfraIr, to: &InfraIr, changes: &mut Vec<InfraChange>) {
    let before: BTreeSet<_> = from.model.unresolved.iter().collect();
    let after: BTreeSet<_> = to.model.unresolved.iter().collect();
    // Both directions are scoped to holders present in *both* snapshots, and for the same
    // reason: an object that arrived brings its references with it and an object that left took
    // its references with it, so the membership change already says what happened. Reporting the
    // reference too would count one event twice, and would make a clean deployment of a new
    // service read as a cluster that just broke.
    for reference in after.difference(&before) {
        if !still_present(from, &reference.from) {
            continue;
        }
        changes.push(InfraChange::ReferenceBroke {
            subject: reference.from.clone(),
            site: reference.site.clone(),
            target: crate::simulate::describe_target(&reference.target),
        });
    }
    for reference in before.difference(&after) {
        if !still_present(to, &reference.from) {
            continue;
        }
        changes.push(InfraChange::ReferenceHealed {
            subject: reference.from.clone(),
            site: reference.site.clone(),
            target: crate::simulate::describe_target(&reference.target),
        });
    }
}

/// Whether an IR path — `workloads/<key>`, `services/<key>`, … — still names something observed.
fn still_present(ir: &InfraIr, path: &str) -> bool {
    let Some((map, key)) = path.split_once('/') else {
        return false;
    };
    match map {
        "workloads" => ir.model.workloads.contains_key(key),
        "services" => ir.model.services.contains_key(key),
        "ingresses" => ir.model.ingresses.contains_key(key),
        "pods" => ir.model.pods.contains_key(key),
        "configmaps" => ir.model.config_maps.contains_key(key),
        "secrets" => ir.model.secrets.contains_key(key),
        "claims" => ir.model.claims.contains_key(key),
        "namespaces" => ir.model.namespaces.contains_key(key),
        "nodes" => ir.model.nodes.contains_key(key),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn membership_reports_each_side_once_and_nothing_for_a_shared_key() {
        let before = BTreeSet::from(["a", "b"]);
        let after = BTreeSet::from(["b", "c"]);
        let mut changes = Vec::new();
        membership(MemberKind::Workload, &before, &after, &mut changes);
        assert_eq!(
            changes,
            vec![
                InfraChange::Added {
                    kind: MemberKind::Workload,
                    subject: "c".to_owned()
                },
                InfraChange::Removed {
                    kind: MemberKind::Workload,
                    subject: "a".to_owned()
                },
            ],
            "`b` is in both snapshots and is not a change"
        );
    }

    #[test]
    fn the_change_ordering_puts_membership_before_the_fields_of_a_surviving_member() {
        let mut changes = vec![
            InfraChange::ImageChanged {
                subject: "shop/deployment/api".to_owned(),
                container: "api".to_owned(),
                from: "api:1".to_owned(),
                to: "api:2".to_owned(),
            },
            InfraChange::Removed {
                kind: MemberKind::Workload,
                subject: "shop/deployment/old".to_owned(),
            },
            InfraChange::Added {
                kind: MemberKind::Workload,
                subject: "shop/deployment/new".to_owned(),
            },
        ];
        changes.sort();
        assert!(
            matches!(changes[0], InfraChange::Added { .. })
                && matches!(changes[1], InfraChange::Removed { .. }),
            "membership leads: {changes:?}"
        );
    }
}
