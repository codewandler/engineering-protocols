//! What rests on what in an observed cluster, and the site that says so.
//!
//! The `ess-diff` dependency-graph idiom, instantiated on infrastructure: a closed edge
//! vocabulary rather than strings, the edge pointing from the dependent to the dependency —
//! the direction a manifest is written in, since a declaration names what it references — and
//! every edge carrying the sites in the dependent that state it, so any line of the rendered
//! graph is checkable against the IR without re-deriving anything.
//!
//! # Only observed objects are nodes
//!
//! An edge exists where a reference *resolved*. What did not resolve is already carried by the
//! IR as a typed [`UnresolvedReference`](infra_compiler::UnresolvedReference) and diagnosed by
//! [`diagnose`](crate::diagnose::diagnose) (`INFRA-DIAG-002`/`003`); drawing a node for an
//! absent object would put something nobody observed on the map of what was observed.
//!
//! # Pod ownership is derived, not guessed
//!
//! A deployment's pods are owned through a `ReplicaSet`, a kind the observation surface does
//! not include. The chain is still derivable without it: a controller-managed pod carries the
//! `pod-template-hash` label, and its owning replicaset's name is the deployment's name with
//! exactly that hash appended — so stripping `-<hash>` off the owner's name and finding that
//! deployment in the pod's namespace closes the chain on evidence the IR already holds.
//! Statefulset and daemonset pods name their workload directly. Everything else — a bare pod,
//! a Job's pod, a replicaset whose deployment is gone — is an [`UnderivedOwner`]: a typed fact
//! with the reason derivation stopped, never a best guess.
//!
//! # Determinism
//!
//! [`BTreeMap`]/[`BTreeSet`] throughout; nodes, edges and facts all sort by their own
//! [`Ord`]. Two constructions over one IR render byte-identical documents —
//! `tests/determinism.rs` holds the claim, and its source scan keeps unordered maps and clocks
//! out of the crate (invariant 9).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use infra_compiler::{
    InfraIr, Reference, ResolvedEnvFromSource, ResolvedEnvSource, ResolvedVolumeSource,
};
use infra_domain::workload::WorkloadKind;
use serde::Serialize;

/// The format string of the JSON graph document.
pub const GRAPH_FORMAT: &str = "infra-graph/1";

/// Which kind of observed object a node stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    /// A deployment, statefulset or daemonset.
    Workload,
    /// A service.
    Service,
    /// An ingress.
    Ingress,
    /// A configmap.
    ConfigMap,
    /// A secret.
    Secret,
    /// A service account.
    ServiceAccount,
    /// A persistent volume claim.
    Claim,
    /// A pod.
    Pod,
    /// A cluster node.
    Node,
}

impl NodeKind {
    /// The prefix a Mermaid identifier of this kind carries, so an identifier read out of a
    /// rendered diagram says which kind it is without looking it up — `ess-gen`'s convention.
    fn mermaid_prefix(self) -> &'static str {
        match self {
            Self::Workload => "wl",
            Self::Service => "svc",
            Self::Ingress => "ing",
            Self::ConfigMap => "cm",
            Self::Secret => "sec",
            Self::ServiceAccount => "sa",
            Self::Claim => "pvc",
            Self::Pod => "pod",
            Self::Node => "node",
        }
    }

    /// What the kind is called in a label or a sentence.
    pub fn noun(self) -> &'static str {
        match self {
            Self::Workload => "workload",
            Self::Service => "service",
            Self::Ingress => "ingress",
            Self::ConfigMap => "configmap",
            Self::Secret => "secret",
            Self::ServiceAccount => "service account",
            Self::Claim => "claim",
            Self::Pod => "pod",
            Self::Node => "node",
        }
    }

    /// Whether objects of this kind live in a namespace. Only cluster nodes do not.
    fn namespaced(self) -> bool {
        !matches!(self, Self::Node)
    }
}

/// One observed object: its kind and its IR map key.
///
/// The key is the IR's — `namespace/name`, workloads `namespace/kind/name`, cluster nodes bare
/// `name` — so every node resolves against the IR it was built from by a map lookup.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct GraphNode {
    /// Which kind of object.
    pub kind: NodeKind,
    /// The IR map key.
    pub key: String,
}

impl GraphNode {
    fn new(kind: NodeKind, key: impl Into<String>) -> Self {
        Self {
            kind,
            key: key.into(),
        }
    }

    /// The namespace the object sits in; `None` for a cluster-scoped node.
    ///
    /// Read off the key's first segment, which the IR defines as the namespace for every
    /// namespaced kind — a property of the key format, not a parse of an opaque identity.
    pub fn namespace(&self) -> Option<&str> {
        if self.kind.namespaced() {
            self.key.split('/').next()
        } else {
            None
        }
    }

    /// The object's own name: the key's last segment.
    pub fn name(&self) -> &str {
        self.key.rsplit('/').next().unwrap_or(&self.key)
    }
}

impl fmt::Display for GraphNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.kind.noun(), self.key)
    }
}

/// What one object does to the object it references.
///
/// A closed vocabulary, for the reason `ess-diff` closes its own: a graph whose edges are
/// strings is a graph whose meaning is a convention. Every variant is produced by
/// [`InfraGraph::of`] from a named field of the IR, and `tests/graph.rs` checks each one is
/// reachable from the committed example observation — a relation nothing mints is not
/// vocabulary, it is decoration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum EdgeRelation {
    /// A service's selector matches a workload's pod template.
    Selects,
    /// An ingress backend routes to a service.
    RoutesTo,
    /// A statefulset names its governing (headless) service.
    GovernedBy,
    /// A container env var reads one key of a configmap or secret.
    ReadsKeyOf,
    /// A container `envFrom` imports every key of a configmap or secret.
    ImportsAllOf,
    /// A template volume mounts a configmap or secret.
    Mounts,
    /// A template volume is backed by a persistent volume claim.
    Claims,
    /// A workload's pods run as a service account.
    RunsAs,
    /// A pod is managed by a workload — declared for statefulsets and daemonsets, derived
    /// through the template hash for deployments.
    OwnedBy,
    /// A pod was scheduled onto a cluster node.
    ScheduledOn,
}

impl EdgeRelation {
    /// Every relation, so a test can insist each one is minted from a real observation.
    pub const ALL: [Self; 10] = [
        Self::Selects,
        Self::RoutesTo,
        Self::GovernedBy,
        Self::ReadsKeyOf,
        Self::ImportsAllOf,
        Self::Mounts,
        Self::Claims,
        Self::RunsAs,
        Self::OwnedBy,
        Self::ScheduledOn,
    ];

    /// The phrase an edge reads with: `<dependent> <verb> <dependency>`.
    pub const fn verb(self) -> &'static str {
        match self {
            Self::Selects => "selects",
            Self::RoutesTo => "routes to",
            Self::GovernedBy => "is governed by",
            Self::ReadsKeyOf => "reads a key of",
            Self::ImportsAllOf => "imports all of",
            Self::Mounts => "mounts",
            Self::Claims => "claims",
            Self::RunsAs => "runs as",
            Self::OwnedBy => "is owned by",
            Self::ScheduledOn => "is scheduled on",
        }
    }
}

impl fmt::Display for EdgeRelation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.verb())
    }
}

/// One dependency, with every site in the dependent that states it.
///
/// One edge per `(from, relation, to)` triple rather than one per site: a workload reading
/// three keys of one configmap is one arrow on any legible drawing, and the three sites are the
/// arrow's evidence, not three arrows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GraphEdge {
    /// The object that references the other.
    pub from: GraphNode,
    /// What it does to it.
    pub relation: EdgeRelation,
    /// The object it references.
    pub to: GraphNode,
    /// Where in `from` the reference is stated, sorted: `containers[main].env[MODE]`,
    /// `volumes[config]`, `selector[app=web]`.
    pub sites: Vec<String>,
}

impl fmt::Display for GraphEdge {
    /// `workload sbf/deployment/api reads a key of secret sbf/creds`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {} {}", self.from, self.relation.verb(), self.to)
    }
}

/// Why a pod's controller could not be tied to an observed workload.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(tag = "reason", rename_all = "snake_case")]
pub enum UnderivedReason {
    /// The pod declares no controller at all — a bare pod.
    NoOwnerDeclared,
    /// The controller's kind is outside the observed subset — a `Job`, a `Node`.
    KindOutsideModel {
        /// The declared kind.
        kind: String,
    },
    /// The owner is a replicaset but the pod carries no `pod-template-hash` label, or the
    /// owner's name does not end in that hash, so no deployment name can be recovered.
    TemplateHashUnderivable {
        /// The replicaset's name as declared.
        name: String,
    },
    /// Derivation produced a workload name, but no such workload was observed.
    NoMatchingWorkload {
        /// The workload kind derivation arrived at.
        kind: String,
        /// The name it arrived at.
        name: String,
    },
}

/// One pod whose controller the graph refuses to guess.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct UnderivedOwner {
    /// The pod's IR key.
    pub pod: String,
    /// The controller as the pod declares it, when it declares one.
    pub owner_kind: Option<String>,
    /// The declared controller's name, when there is one.
    pub owner_name: Option<String>,
    /// Why derivation stopped.
    pub reason: UnderivedReason,
}

/// The identity of an edge: what the map of sites is keyed by.
type EdgeId = (GraphNode, EdgeRelation, GraphNode);

/// The dependency graph of one compiled IR.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InfraGraph {
    /// Every observed object, whether or not anything references it.
    nodes: BTreeSet<GraphNode>,
    /// The sites of every edge, keyed by the edge's identity.
    edges: BTreeMap<EdgeId, BTreeSet<String>>,
    /// The derived controller of each pod that has one.
    pod_owners: BTreeMap<String, String>,
    /// Every pod whose controller stayed underived, sorted.
    underived: Vec<UnderivedOwner>,
}

impl InfraGraph {
    /// Builds the graph of one IR. A walk, not a resolution: every reference here is already a
    /// checked handle, so this only reads keys and records edges.
    #[must_use]
    pub fn of(ir: &InfraIr) -> Self {
        let mut graph = Self {
            nodes: BTreeSet::new(),
            edges: BTreeMap::new(),
            pod_owners: BTreeMap::new(),
            underived: Vec::new(),
        };

        for key in ir.model.nodes.keys() {
            graph.node(NodeKind::Node, key);
        }
        for key in ir.model.services.keys() {
            graph.node(NodeKind::Service, key);
        }
        for key in ir.model.ingresses.keys() {
            graph.node(NodeKind::Ingress, key);
        }
        for key in ir.model.config_maps.keys() {
            graph.node(NodeKind::ConfigMap, key);
        }
        for key in ir.model.secrets.keys() {
            graph.node(NodeKind::Secret, key);
        }
        for key in ir.model.service_accounts.keys() {
            graph.node(NodeKind::ServiceAccount, key);
        }
        for key in ir.model.claims.keys() {
            graph.node(NodeKind::Claim, key);
        }

        graph.walk_workloads(ir);
        graph.walk_services(ir);
        graph.walk_ingresses(ir);
        graph.walk_pods(ir);
        graph.underived.sort();
        graph
    }

    /// Every node, in canonical order.
    pub fn nodes(&self) -> &BTreeSet<GraphNode> {
        &self.nodes
    }

    /// Every edge with its sites, in canonical order.
    pub fn edges(&self) -> impl Iterator<Item = GraphEdge> + '_ {
        self.edges
            .iter()
            .map(|((from, relation, to), sites)| GraphEdge {
                from: from.clone(),
                relation: *relation,
                to: to.clone(),
                sites: sites.iter().cloned().collect(),
            })
    }

    /// How many distinct edges there are.
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    /// The workload key a pod's controller was derived to, when it was.
    pub fn owner_of(&self, pod_key: &str) -> Option<&str> {
        self.pod_owners.get(pod_key).map(String::as_str)
    }

    /// Every pod whose controller stayed underived, with the reason.
    pub fn underived_owners(&self) -> &[UnderivedOwner] {
        &self.underived
    }

    /// This graph restricted to one namespace: its objects, their edges, and any cluster-scoped
    /// node one of those edges reaches.
    ///
    /// A namespace that was never observed yields an empty graph rather than an error — asking
    /// for it is a query, and the honest answer to "what is in a namespace nobody observed" is
    /// nothing.
    #[must_use]
    pub fn restricted_to(&self, namespace: &str) -> Self {
        let mut kept = Self {
            nodes: self
                .nodes
                .iter()
                .filter(|node| node.namespace() == Some(namespace))
                .cloned()
                .collect(),
            edges: BTreeMap::new(),
            pod_owners: BTreeMap::new(),
            underived: self
                .underived
                .iter()
                .filter(|fact| fact.pod.split('/').next() == Some(namespace))
                .cloned()
                .collect(),
        };
        for ((from, relation, to), sites) in &self.edges {
            if from.namespace() == Some(namespace) {
                kept.nodes.insert(to.clone());
                kept.edges
                    .insert((from.clone(), *relation, to.clone()), sites.clone());
            }
        }
        for (pod, workload) in &self.pod_owners {
            if pod.split('/').next() == Some(namespace) {
                kept.pod_owners.insert(pod.clone(), workload.clone());
            }
        }
        kept
    }

    // ---- building ---------------------------------------------------------------------------

    fn node(&mut self, kind: NodeKind, key: &str) {
        self.nodes.insert(GraphNode::new(kind, key));
    }

    fn edge(&mut self, from: GraphNode, relation: EdgeRelation, to: GraphNode, site: String) {
        self.nodes.insert(from.clone());
        self.nodes.insert(to.clone());
        self.edges
            .entry((from, relation, to))
            .or_default()
            .insert(site);
    }

    #[allow(clippy::too_many_lines)]
    fn walk_workloads(&mut self, ir: &InfraIr) {
        for (key, workload) in &ir.model.workloads {
            let subject = GraphNode::new(NodeKind::Workload, key);
            self.nodes.insert(subject.clone());

            if let Reference::Resolved { key: account } = &workload.service_account {
                self.edge(
                    subject.clone(),
                    EdgeRelation::RunsAs,
                    GraphNode::new(NodeKind::ServiceAccount, account.key()),
                    "serviceAccountName".to_owned(),
                );
            }
            if let Some(Reference::Resolved { key: service }) = &workload.governing_service {
                self.edge(
                    subject.clone(),
                    EdgeRelation::GovernedBy,
                    GraphNode::new(NodeKind::Service, service.key()),
                    "serviceName".to_owned(),
                );
            }

            for container in &workload.containers {
                for variable in &container.env {
                    let site = format!("containers[{}].env[{}]", container.name, variable.name);
                    match &variable.source {
                        ResolvedEnvSource::ConfigMapKey {
                            config_map: Reference::Resolved { key: target },
                            ..
                        } => self.edge(
                            subject.clone(),
                            EdgeRelation::ReadsKeyOf,
                            GraphNode::new(NodeKind::ConfigMap, target.key()),
                            site,
                        ),
                        ResolvedEnvSource::SecretKey {
                            secret: Reference::Resolved { key: target },
                            ..
                        } => self.edge(
                            subject.clone(),
                            EdgeRelation::ReadsKeyOf,
                            GraphNode::new(NodeKind::Secret, target.key()),
                            site,
                        ),
                        _ => {}
                    }
                }
                for (index, entry) in container.env_from.iter().enumerate() {
                    let site = format!("containers[{}].envFrom[{index}]", container.name);
                    match &entry.source {
                        ResolvedEnvFromSource::ConfigMap {
                            config_map: Reference::Resolved { key: target },
                            ..
                        } => self.edge(
                            subject.clone(),
                            EdgeRelation::ImportsAllOf,
                            GraphNode::new(NodeKind::ConfigMap, target.key()),
                            site,
                        ),
                        ResolvedEnvFromSource::Secret {
                            secret: Reference::Resolved { key: target },
                            ..
                        } => self.edge(
                            subject.clone(),
                            EdgeRelation::ImportsAllOf,
                            GraphNode::new(NodeKind::Secret, target.key()),
                            site,
                        ),
                        _ => {}
                    }
                }
            }

            for volume in &workload.volumes {
                let site = format!("volumes[{}]", volume.name);
                match &volume.source {
                    ResolvedVolumeSource::ConfigMap {
                        config_map: Reference::Resolved { key: target },
                        ..
                    } => self.edge(
                        subject.clone(),
                        EdgeRelation::Mounts,
                        GraphNode::new(NodeKind::ConfigMap, target.key()),
                        site,
                    ),
                    ResolvedVolumeSource::Secret {
                        secret: Reference::Resolved { key: target },
                        ..
                    } => self.edge(
                        subject.clone(),
                        EdgeRelation::Mounts,
                        GraphNode::new(NodeKind::Secret, target.key()),
                        site,
                    ),
                    ResolvedVolumeSource::Claim {
                        claim: Reference::Resolved { key: target },
                    } => self.edge(
                        subject.clone(),
                        EdgeRelation::Claims,
                        GraphNode::new(NodeKind::Claim, target.key()),
                        site,
                    ),
                    _ => {}
                }
            }
        }
    }

    /// A service selects the workloads whose pod template its selector matches, in its own
    /// namespace — the same match the compiler ran against *pods*, run against what pods are
    /// made from, so the edge exists even while a deployment is scaled to zero.
    fn walk_services(&mut self, ir: &InfraIr) {
        for (service_key, service) in &ir.model.services {
            if service.selector.is_empty() {
                continue;
            }
            let site = format!(
                "selector[{}]",
                service
                    .selector
                    .iter()
                    .map(|(label, value)| format!("{label}={value}"))
                    .collect::<Vec<_>>()
                    .join(",")
            );
            for (workload_key, workload) in &ir.model.workloads {
                if workload.identity.namespace != service.identity.namespace {
                    continue;
                }
                let matches = service
                    .selector
                    .iter()
                    .all(|(label, value)| workload.template_labels.get(label) == Some(value));
                if matches {
                    self.edge(
                        GraphNode::new(NodeKind::Service, service_key),
                        EdgeRelation::Selects,
                        GraphNode::new(NodeKind::Workload, workload_key),
                        site.clone(),
                    );
                }
            }
        }
    }

    fn walk_ingresses(&mut self, ir: &InfraIr) {
        for (key, ingress) in &ir.model.ingresses {
            let subject = GraphNode::new(NodeKind::Ingress, key);
            for (rule_index, rule) in ingress.rules.iter().enumerate() {
                for (path_index, path) in rule.paths.iter().enumerate() {
                    if let Reference::Resolved { key: service } = &path.backend.service {
                        self.edge(
                            subject.clone(),
                            EdgeRelation::RoutesTo,
                            GraphNode::new(NodeKind::Service, service.key()),
                            format!("rules[{rule_index}].paths[{path_index}]"),
                        );
                    }
                }
            }
            if let Some(backend) = &ingress.default_backend {
                if let Reference::Resolved { key: service } = &backend.service {
                    self.edge(
                        subject.clone(),
                        EdgeRelation::RoutesTo,
                        GraphNode::new(NodeKind::Service, service.key()),
                        "defaultBackend".to_owned(),
                    );
                }
            }
        }
    }

    fn walk_pods(&mut self, ir: &InfraIr) {
        for (key, pod) in &ir.model.pods {
            let subject = GraphNode::new(NodeKind::Pod, key);
            self.nodes.insert(subject.clone());

            if let Some(Reference::Resolved { key: node }) = &pod.node {
                self.edge(
                    subject.clone(),
                    EdgeRelation::ScheduledOn,
                    GraphNode::new(NodeKind::Node, node.key()),
                    "nodeName".to_owned(),
                );
            }

            let namespace = pod.identity.namespace.as_deref().unwrap_or_default();
            match &pod.owner {
                None => self.underived.push(UnderivedOwner {
                    pod: key.clone(),
                    owner_kind: None,
                    owner_name: None,
                    reason: UnderivedReason::NoOwnerDeclared,
                }),
                Some(owner) => {
                    let derived = match owner.kind.as_str() {
                        "StatefulSet" => Some((WorkloadKind::StatefulSet, owner.name.clone())),
                        "DaemonSet" => Some((WorkloadKind::DaemonSet, owner.name.clone())),
                        "ReplicaSet" => {
                            let Some(deployment) = deployment_of_replicaset(
                                &owner.name,
                                pod.labels.get("pod-template-hash").map(String::as_str),
                            ) else {
                                self.underived.push(UnderivedOwner {
                                    pod: key.clone(),
                                    owner_kind: Some(owner.kind.clone()),
                                    owner_name: Some(owner.name.clone()),
                                    reason: UnderivedReason::TemplateHashUnderivable {
                                        name: owner.name.clone(),
                                    },
                                });
                                continue;
                            };
                            Some((WorkloadKind::Deployment, deployment))
                        }
                        other => {
                            self.underived.push(UnderivedOwner {
                                pod: key.clone(),
                                owner_kind: Some(other.to_owned()),
                                owner_name: Some(owner.name.clone()),
                                reason: UnderivedReason::KindOutsideModel {
                                    kind: other.to_owned(),
                                },
                            });
                            continue;
                        }
                    };
                    if let Some((kind, name)) = derived {
                        let workload_key = format!("{namespace}/{}/{name}", kind.as_str());
                        if ir.model.workloads.contains_key(&workload_key) {
                            self.edge(
                                subject.clone(),
                                EdgeRelation::OwnedBy,
                                GraphNode::new(NodeKind::Workload, &workload_key),
                                "ownerReferences".to_owned(),
                            );
                            self.pod_owners.insert(key.clone(), workload_key);
                        } else {
                            self.underived.push(UnderivedOwner {
                                pod: key.clone(),
                                owner_kind: Some(owner.kind.clone()),
                                owner_name: Some(owner.name.clone()),
                                reason: UnderivedReason::NoMatchingWorkload {
                                    kind: kind.as_str().to_owned(),
                                    name,
                                },
                            });
                        }
                    }
                }
            }
        }
    }
}

/// The deployment name a replicaset's name encodes, when the pod's template hash confirms it.
///
/// `coredns-ccb96694c` with hash `ccb96694c` derives `coredns`; a name that does not end in
/// `-<hash>`, or a pod without the label, derives nothing — that is the typed fact, not an
/// error.
fn deployment_of_replicaset(replicaset: &str, template_hash: Option<&str>) -> Option<String> {
    let hash = template_hash?;
    if hash.is_empty() {
        return None;
    }
    let suffix = format!("-{hash}");
    replicaset
        .strip_suffix(suffix.as_str())
        .filter(|prefix| !prefix.is_empty())
        .map(ToOwned::to_owned)
}

// ---------------------------------------------------------------------------------------------
// Rendering. The JSON document is canonical; Mermaid is a projection of part of it.
// ---------------------------------------------------------------------------------------------

/// The graph as a persistable JSON document.
#[derive(Debug, Clone, Serialize)]
pub struct GraphDocument {
    /// The format claim, `infra-graph/1`.
    pub format: &'static str,
    /// The kubeconfig context the underlying observation targeted.
    pub context: String,
    /// The digest of the IR the graph was built from, so the two documents chain.
    pub source_digest: String,
    /// The namespace the graph was restricted to, when it was.
    pub namespace: Option<String>,
    /// Every node.
    pub nodes: Vec<GraphNode>,
    /// Every edge with its sites.
    pub edges: Vec<GraphEdge>,
    /// Every pod whose controller stayed underived.
    pub underived_owners: Vec<UnderivedOwner>,
}

impl GraphDocument {
    /// Builds the document of a graph over the IR it came from.
    #[must_use]
    pub fn of(graph: &InfraGraph, ir: &InfraIr, namespace: Option<&str>) -> Self {
        Self {
            format: GRAPH_FORMAT,
            context: ir.provenance.context.clone(),
            source_digest: ir.digest(),
            namespace: namespace.map(ToOwned::to_owned),
            nodes: graph.nodes.iter().cloned().collect(),
            edges: graph.edges().collect(),
            underived_owners: graph.underived.clone(),
        }
    }

    /// The document as pretty JSON with a trailing newline — what the CLI prints and what a
    /// determinism test compares byte for byte.
    #[must_use]
    pub fn to_json(&self) -> String {
        let mut rendered = serde_json::to_string_pretty(self)
            .expect("the graph document has no non-serializable state");
        rendered.push('\n');
        rendered
    }
}

/// Which node kinds the Mermaid rendering draws.
///
/// The configuration topology — workloads, services, ingresses and the stores they reference —
/// and deliberately not the runtime layer: a real cluster observes a thousand pods, and a
/// flowchart with a thousand pod boxes is not a rendering anyone reads. The JSON document is
/// canonical and carries everything; the diagram is the part of it a person looks at.
const MERMAID_KINDS: [NodeKind; 7] = [
    NodeKind::Workload,
    NodeKind::Service,
    NodeKind::Ingress,
    NodeKind::ConfigMap,
    NodeKind::Secret,
    NodeKind::ServiceAccount,
    NodeKind::Claim,
];

impl InfraGraph {
    /// The configuration topology as a Mermaid `flowchart`, one subgraph per namespace,
    /// without the Markdown fence — `ess-gen`'s convention, for `ess-gen`'s reason: a CLI
    /// writing to a pipe must not fence, or the first thing anyone does is delete three
    /// characters off each end.
    #[must_use]
    pub fn mermaid(&self) -> String {
        use std::fmt::Write as _;

        let drawn: Vec<&GraphNode> = self
            .nodes
            .iter()
            .filter(|node| MERMAID_KINDS.contains(&node.kind))
            .collect();

        // Identifiers are indices into the canonical node order, prefixed by kind so an
        // identifier read out of the diagram says what it is.
        let mut ids: BTreeMap<&GraphNode, String> = BTreeMap::new();
        let mut per_kind: BTreeMap<NodeKind, usize> = BTreeMap::new();
        for node in &drawn {
            let index = per_kind.entry(node.kind).or_default();
            ids.insert(node, format!("{}{index}", node.kind.mermaid_prefix()));
            *index += 1;
        }

        let mut namespaces: BTreeMap<&str, Vec<&GraphNode>> = BTreeMap::new();
        for node in &drawn {
            namespaces
                .entry(node.namespace().unwrap_or_default())
                .or_default()
                .push(node);
        }

        let mut out = String::from("flowchart TB\n");
        for (index, (namespace, members)) in namespaces.iter().enumerate() {
            let _ = writeln!(
                out,
                "    subgraph ns{index}[\"namespace {}\"]",
                label(namespace)
            );
            for node in members {
                let text = match node.kind {
                    NodeKind::Workload => {
                        // The key is `namespace/kind/name`; the label reads `kind name`.
                        let mut parts = node.key.splitn(3, '/');
                        let _ = parts.next();
                        let kind = parts.next().unwrap_or_default();
                        format!("{kind} {}", parts.next().unwrap_or_default())
                    }
                    other => format!("{} {}", other.noun(), node.name()),
                };
                let _ = writeln!(out, "        {}[\"{}\"]", ids[*node], label(&text));
            }
            out.push_str("    end\n");
        }
        for ((from, relation, to), sites) in &self.edges {
            let (Some(from_id), Some(to_id)) = (ids.get(from), ids.get(to)) else {
                continue;
            };
            let text = if sites.len() > 1 {
                format!("{} ({})", relation.verb(), sites.len())
            } else {
                relation.verb().to_owned()
            };
            let _ = writeln!(out, "    {from_id} -->|\"{}\"| {to_id}", label(&text));
        }
        out
    }
}

/// Defuses what would end a quoted Mermaid string early — `ess-gen`'s escaping, kept
/// identical so the two Mermaid emitters in this workspace cannot disagree about quoting.
fn label(text: &str) -> String {
    text.replace('"', "#quot;").replace('\n', " ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_replicaset_name_derives_its_deployment_only_when_the_hash_confirms_it() {
        assert_eq!(
            deployment_of_replicaset("coredns-ccb96694c", Some("ccb96694c")),
            Some("coredns".to_owned())
        );
        assert_eq!(
            deployment_of_replicaset("coredns-ccb96694c", Some("othertag")),
            None,
            "a hash the name does not end in derives nothing"
        );
        assert_eq!(
            deployment_of_replicaset("coredns-ccb96694c", None),
            None,
            "no template hash, no derivation"
        );
        assert_eq!(
            deployment_of_replicaset("-ccb96694c", Some("ccb96694c")),
            None,
            "an empty deployment name is not a derivation"
        );
        assert_eq!(
            deployment_of_replicaset("coredns-ccb96694c", Some("")),
            None,
            "an empty hash proves nothing"
        );
    }

    #[test]
    fn a_mermaid_label_cannot_close_the_quoted_string_it_sits_in() {
        assert_eq!(label("a\"b\nc"), "a#quot;b c");
    }

    #[test]
    fn a_graph_node_reads_its_namespace_off_the_key_and_a_cluster_node_has_none() {
        let workload = GraphNode::new(NodeKind::Workload, "sbf/deployment/api");
        assert_eq!(workload.namespace(), Some("sbf"));
        assert_eq!(workload.name(), "api");
        let node = GraphNode::new(NodeKind::Node, "k3d-server-0");
        assert_eq!(node.namespace(), None);
        assert_eq!(node.name(), "k3d-server-0");
    }
}
