//! What is wrong with an observed cluster — typed, coded, explainable, and never a refusal.
//!
//! # A diagnosis is a report, not a gate
//!
//! The invariant IW1 set for compilation carries through: observed infrastructure is allowed to
//! be wrong. Validation refuses a bundle that lies about its own shape; diagnosis *describes* a
//! cluster that is honestly in a bad state. So [`diagnose`] is total, a diagnosis full of
//! errors is a successful diagnosis, and the CLI exits 0 unless the input itself was invalid.
//!
//! # One rule, one function, one code
//!
//! Every rule below is a function that pushes [`Finding`]s, every finding carries a stable
//! [`DiagCode`] whose severity is registered with the code, and `tests/diagnosis.rs` holds a
//! positive and a negative fixture case per rule — disabling a rule fails the test naming its
//! code, which is what makes the rule set mutation-checkable instead of merely listed.

use std::collections::{BTreeMap, BTreeSet};

use infra_compiler::{InfraIr, UnresolvedTarget};
use serde::Serialize;

use crate::code::{DiagCode, Severity};
use crate::graph::{EdgeRelation, GraphNode, InfraGraph, NodeKind};
use crate::properties::parse_image;

/// A container restarting this often is diagnosed (`INFRA-DIAG-009`).
///
/// Five, deterministically: below it, a restart or two is a node reboot or a rollout; at five,
/// something inside the container keeps dying. A threshold in the registry rather than a flag,
/// so two runs of one build cannot disagree about what "high" means.
pub const HIGH_RESTART_THRESHOLD: u32 = 5;

/// Waiting reasons that mean "starting", not "stuck" (`INFRA-DIAG-008` does not fire on them).
const BENIGN_WAITING: [&str; 2] = ["ContainerCreating", "PodInitializing"];

/// The secret type the token controller manages; nothing mounts it explicitly, so the orphan
/// rule (`INFRA-DIAG-011`) skips it by *type* — a typed exemption, not a name pattern.
const SERVICE_ACCOUNT_TOKEN: &str = "kubernetes.io/service-account-token";

/// One diagnosed fact: which rule, how serious, about what, on which evidence.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct Finding {
    /// The stable code a harness matches on.
    pub code: DiagCode,
    /// The code's registered severity, repeated here so a serialized finding stands alone.
    pub severity: Severity,
    /// The object the finding is about, as an IR path: `workloads/sbf/deployment/api`.
    pub subject: String,
    /// Where inside the subject, when the finding is narrower than the object:
    /// `containers[main]`, `containers[main].env[TOKEN]`.
    pub site: Option<String>,
    /// What is wrong, for a human.
    pub message: String,
    /// The fields the finding rests on, each named: `image`, `restarts`, `phase`.
    pub evidence: BTreeMap<String, String>,
}

impl Finding {
    /// Builds a finding; the severity comes from the code and nowhere else.
    fn new(
        code: DiagCode,
        subject: impl Into<String>,
        site: Option<String>,
        message: impl Into<String>,
        evidence: BTreeMap<String, String>,
    ) -> Self {
        Self {
            code,
            severity: code.severity(),
            subject: subject.into(),
            site,
            message: message.into(),
            evidence,
        }
    }
}

/// The full diagnosis of one IR.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Diagnosis {
    /// Every finding, sorted by code, then subject, then site.
    pub findings: Vec<Finding>,
}

impl Diagnosis {
    /// How many findings carry each severity, highest first: `(errors, warnings, infos)`.
    #[must_use]
    pub fn counts(&self) -> (usize, usize, usize) {
        let mut errors = 0;
        let mut warnings = 0;
        let mut infos = 0;
        for finding in &self.findings {
            match finding.severity {
                Severity::Error => errors += 1,
                Severity::Warning => warnings += 1,
                Severity::Info => infos += 1,
            }
        }
        (errors, warnings, infos)
    }

    /// The findings at or above a severity, in the same order.
    #[must_use]
    pub fn at_least(&self, floor: Severity) -> Vec<&Finding> {
        self.findings
            .iter()
            .filter(|finding| finding.severity >= floor)
            .collect()
    }
}

/// Diagnoses one IR. Total: a degraded cluster diagnoses successfully — that is the point.
///
/// The graph is built internally because half the rules read it (orphans, ownership,
/// duplicate selectors); a caller that already has one can pass it via [`diagnose_with`].
#[must_use]
pub fn diagnose(ir: &InfraIr) -> Diagnosis {
    diagnose_with(ir, &InfraGraph::of(ir))
}

/// [`diagnose`] over a graph the caller already built.
#[must_use]
pub fn diagnose_with(ir: &InfraIr, graph: &InfraGraph) -> Diagnosis {
    let mut findings = Vec::new();

    rule_dangling_selector(ir, &mut findings);
    rule_missing_references(ir, &mut findings);
    rule_resource_bounds(ir, &mut findings);
    rule_probes(ir, &mut findings);
    rule_unpinned_image(ir, &mut findings);
    rule_single_replica(ir, &mut findings);
    rule_stuck_waiting(ir, &mut findings);
    rule_high_restarts(ir, &mut findings);
    rule_pod_not_ready(ir, graph, &mut findings);
    rule_orphaned_config(ir, graph, &mut findings);
    rule_unbound_claim(ir, &mut findings);
    rule_orphaned_claim(ir, graph, &mut findings);
    rule_duplicate_selectors(graph, &mut findings);

    findings.sort();
    Diagnosis { findings }
}

/// `INFRA-DIAG-001` — a service selector that matches no pod, read off the IR's unresolved
/// facts, where the compiler already ran the match.
fn rule_dangling_selector(ir: &InfraIr, findings: &mut Vec<Finding>) {
    for fact in &ir.model.unresolved {
        if let UnresolvedTarget::PodsMatchingSelector { selector } = &fact.target {
            let rendered = selector
                .iter()
                .map(|(label, value)| format!("{label}={value}"))
                .collect::<Vec<_>>()
                .join(",");
            findings.push(Finding::new(
                DiagCode::DanglingSelector,
                fact.from.clone(),
                Some(fact.site.clone()),
                format!("the selector `{rendered}` matches no observed pod"),
                BTreeMap::from([("selector".to_owned(), rendered)]),
            ));
        }
    }
}

/// `INFRA-DIAG-002`/`003` — every other unresolved fact the IR carries, split by whether the
/// reference site declared itself optional. The kinds that cannot declare optionality —
/// service account, claim, service, node, namespace — are required by construction.
fn rule_missing_references(ir: &InfraIr, findings: &mut Vec<Finding>) {
    for fact in &ir.model.unresolved {
        let (what, name, optional) = match &fact.target {
            UnresolvedTarget::PodsMatchingSelector { .. } => continue,
            UnresolvedTarget::ConfigMap { name, optional } => {
                ("configmap", name.clone(), *optional)
            }
            UnresolvedTarget::ConfigMapKey {
                name,
                key,
                optional,
            } => ("configmap key", format!("{name}[{key}]"), *optional),
            UnresolvedTarget::Secret { name, optional } => ("secret", name.clone(), *optional),
            UnresolvedTarget::SecretKey {
                name,
                key,
                optional,
            } => ("secret key", format!("{name}[{key}]"), *optional),
            UnresolvedTarget::ServiceAccount { name } => ("service account", name.clone(), false),
            UnresolvedTarget::Claim { name } => ("persistent volume claim", name.clone(), false),
            UnresolvedTarget::Service { name } => ("service", name.clone(), false),
            UnresolvedTarget::Node { name } => ("node", name.clone(), false),
            UnresolvedTarget::Namespace { name } => ("namespace", name.clone(), false),
        };
        let code = if optional {
            DiagCode::MissingOptionalReference
        } else {
            DiagCode::MissingReference
        };
        let message = if optional {
            format!("the optional {what} `{name}` was not observed; tolerated, and worth knowing")
        } else {
            format!("the required {what} `{name}` was not observed")
        };
        findings.push(Finding::new(
            code,
            fact.from.clone(),
            Some(fact.site.clone()),
            message,
            BTreeMap::from([
                ("kind".to_owned(), what.to_owned()),
                ("name".to_owned(), name),
            ]),
        ));
    }
}

/// `INFRA-DIAG-004` — a container without requests, limits, or either.
fn rule_resource_bounds(ir: &InfraIr, findings: &mut Vec<Finding>) {
    for (key, workload) in &ir.model.workloads {
        for container in &workload.containers {
            let mut missing = Vec::new();
            if container.resources.requests.is_empty() {
                missing.push("requests");
            }
            if container.resources.limits.is_empty() {
                missing.push("limits");
            }
            if missing.is_empty() {
                continue;
            }
            let missing = missing.join(" and ");
            findings.push(Finding::new(
                DiagCode::NoResourceBounds,
                format!("workloads/{key}"),
                Some(format!("containers[{}]", container.name)),
                format!(
                    "container `{}` declares no resource {missing}",
                    container.name
                ),
                BTreeMap::from([("missing".to_owned(), missing.clone())]),
            ));
        }
    }
}

/// `INFRA-DIAG-005` — a container without a liveness probe, a readiness probe, or either.
fn rule_probes(ir: &InfraIr, findings: &mut Vec<Finding>) {
    for (key, workload) in &ir.model.workloads {
        for container in &workload.containers {
            let mut missing = Vec::new();
            if container.probes.liveness.is_none() {
                missing.push("liveness");
            }
            if container.probes.readiness.is_none() {
                missing.push("readiness");
            }
            if missing.is_empty() {
                continue;
            }
            let missing = missing.join(" and ");
            findings.push(Finding::new(
                DiagCode::NoProbes,
                format!("workloads/{key}"),
                Some(format!("containers[{}]", container.name)),
                format!("container `{}` has no {missing} probe", container.name),
                BTreeMap::from([("missing".to_owned(), missing.clone())]),
            ));
        }
    }
}

/// `INFRA-DIAG-006` — `:latest`, untagged, and not digest-pinned images.
fn rule_unpinned_image(ir: &InfraIr, findings: &mut Vec<Finding>) {
    for (key, workload) in &ir.model.workloads {
        for container in &workload.containers {
            let image = parse_image(&container.image);
            if image.digest.is_some() {
                continue;
            }
            let complaint = match image.tag.as_deref() {
                None => "no tag",
                Some("latest") => "the `latest` tag",
                Some(_) => continue,
            };
            findings.push(Finding::new(
                DiagCode::UnpinnedImage,
                format!("workloads/{key}"),
                Some(format!("containers[{}]", container.name)),
                format!(
                    "image `{}` has {complaint} and no digest; what runs depends on when it \
                     was pulled",
                    container.image
                ),
                BTreeMap::from([("image".to_owned(), container.image.clone())]),
            ));
        }
    }
}

/// `INFRA-DIAG-007` — a workload that wants exactly one replica. Daemonsets have no replica
/// count and cannot fire.
fn rule_single_replica(ir: &InfraIr, findings: &mut Vec<Finding>) {
    for (key, workload) in &ir.model.workloads {
        if workload.replicas == Some(1) {
            findings.push(Finding::new(
                DiagCode::SingleReplica,
                format!("workloads/{key}"),
                None,
                "one replica: any disruption is an outage",
                BTreeMap::from([("replicas".to_owned(), "1".to_owned())]),
            ));
        }
    }
}

/// `INFRA-DIAG-008` — a container waiting for a reason that is not part of normal startup.
fn rule_stuck_waiting(ir: &InfraIr, findings: &mut Vec<Finding>) {
    for (key, pod) in &ir.model.pods {
        for container in &pod.containers {
            let Some(reason) = &container.waiting_reason else {
                continue;
            };
            if BENIGN_WAITING.contains(&reason.as_str()) {
                continue;
            }
            findings.push(Finding::new(
                DiagCode::PodStuckWaiting,
                format!("pods/{key}"),
                Some(format!("containers[{}]", container.name)),
                format!("container `{}` is stuck waiting: {reason}", container.name),
                BTreeMap::from([("reason".to_owned(), reason.clone())]),
            ));
        }
    }
}

/// `INFRA-DIAG-009` — a container at or above the restart threshold.
fn rule_high_restarts(ir: &InfraIr, findings: &mut Vec<Finding>) {
    for (key, pod) in &ir.model.pods {
        for container in &pod.containers {
            if container.restart_count < HIGH_RESTART_THRESHOLD {
                continue;
            }
            findings.push(Finding::new(
                DiagCode::HighRestartCount,
                format!("pods/{key}"),
                Some(format!("containers[{}]", container.name)),
                format!(
                    "container `{}` has restarted {} times",
                    container.name, container.restart_count
                ),
                BTreeMap::from([("restarts".to_owned(), container.restart_count.to_string())]),
            ));
        }
    }
}

/// `INFRA-DIAG-010` — a pod that is not ready, not done, and whose controller derives to an
/// observed workload — the workload's existence *is* the readiness expectation.
///
/// Scoped to derived-owner pods deliberately: a bare pod states no expectation, a `Job`'s pod
/// finishing is its job, and both already surface as typed underived-owner facts on the graph.
fn rule_pod_not_ready(ir: &InfraIr, graph: &InfraGraph, findings: &mut Vec<Finding>) {
    use infra_domain::observation::PodPhase;

    for (key, pod) in &ir.model.pods {
        if pod.ready || pod.phase == PodPhase::Succeeded {
            continue;
        }
        let Some(workload) = graph.owner_of(key) else {
            continue;
        };
        let phase = format!("{:?}", pod.phase).to_lowercase();
        findings.push(Finding::new(
            DiagCode::PodNotReady,
            format!("pods/{key}"),
            None,
            format!(
                "the pod is not ready (phase {phase}) and workload `{workload}` expects it to be"
            ),
            BTreeMap::from([
                ("phase".to_owned(), phase),
                ("workload".to_owned(), workload.to_owned()),
            ]),
        ));
    }
}

/// The set of nodes at least one edge points at.
fn referenced_targets(graph: &InfraGraph) -> BTreeSet<GraphNode> {
    graph.edges().map(|edge| edge.to).collect()
}

/// `INFRA-DIAG-011` — configmaps and secrets nothing references through a modelled site.
///
/// "Modelled site" is part of the finding, not small print: a projected service-account token
/// or a kubelet-mounted root CA reaches pods through machinery outside the subset, so the rule
/// says "no env, envFrom or volume site", never "unused". Service-account token secrets are
/// exempted by type for exactly that reason.
fn rule_orphaned_config(ir: &InfraIr, graph: &InfraGraph, findings: &mut Vec<Finding>) {
    let referenced = referenced_targets(graph);
    for key in ir.model.config_maps.keys() {
        let node = GraphNode {
            kind: NodeKind::ConfigMap,
            key: key.clone(),
        };
        if !referenced.contains(&node) {
            findings.push(Finding::new(
                DiagCode::OrphanedConfig,
                format!("config_maps/{key}"),
                None,
                "no env, envFrom or volume site of any observed workload references this configmap",
                BTreeMap::new(),
            ));
        }
    }
    for (key, secret) in &ir.model.secrets {
        if secret.secret_type == SERVICE_ACCOUNT_TOKEN {
            continue;
        }
        let node = GraphNode {
            kind: NodeKind::Secret,
            key: key.clone(),
        };
        if !referenced.contains(&node) {
            findings.push(Finding::new(
                DiagCode::OrphanedConfig,
                format!("secrets/{key}"),
                None,
                "no env, envFrom or volume site of any observed workload references this secret",
                BTreeMap::from([("type".to_owned(), secret.secret_type.clone())]),
            ));
        }
    }
}

/// `INFRA-DIAG-012` — a claim observed in a phase other than `Bound`.
///
/// An *unobserved* phase does not fire: unknown is not unbound, and claiming a claim is broken
/// because a scanner did not look would be manufacturing a defect out of a gap.
fn rule_unbound_claim(ir: &InfraIr, findings: &mut Vec<Finding>) {
    use infra_domain::observation::ClaimPhase;

    for (key, claim) in &ir.model.claims {
        let phase = match claim.phase {
            ClaimPhase::Pending => "pending",
            ClaimPhase::Lost => "lost",
            ClaimPhase::Bound | ClaimPhase::Unknown => continue,
        };
        findings.push(Finding::new(
            DiagCode::UnboundClaim,
            format!("claims/{key}"),
            None,
            format!("the claim is {phase}, not bound; pods mounting it cannot start"),
            BTreeMap::from([("phase".to_owned(), phase.to_owned())]),
        ));
    }
}

/// `INFRA-DIAG-013` — a claim no workload volume references.
///
/// A statefulset's `volumeClaimTemplates` claims are outside the modelled reference surface,
/// so such a claim fires here with the wording scoped to what was checked.
fn rule_orphaned_claim(ir: &InfraIr, graph: &InfraGraph, findings: &mut Vec<Finding>) {
    let referenced = referenced_targets(graph);
    for key in ir.model.claims.keys() {
        let node = GraphNode {
            kind: NodeKind::Claim,
            key: key.clone(),
        };
        if !referenced.contains(&node) {
            findings.push(Finding::new(
                DiagCode::OrphanedClaim,
                format!("claims/{key}"),
                None,
                "no volume of any observed workload references this claim",
                BTreeMap::new(),
            ));
        }
    }
}

/// `INFRA-DIAG-014` — services whose selectors resolve to exactly the same workload set.
fn rule_duplicate_selectors(graph: &InfraGraph, findings: &mut Vec<Finding>) {
    let mut targets_of: BTreeMap<GraphNode, BTreeSet<String>> = BTreeMap::new();
    for edge in graph.edges() {
        if edge.relation == EdgeRelation::Selects {
            targets_of
                .entry(edge.from.clone())
                .or_default()
                .insert(edge.to.key.clone());
        }
    }
    let mut groups: BTreeMap<BTreeSet<String>, Vec<GraphNode>> = BTreeMap::new();
    for (service, targets) in targets_of {
        groups.entry(targets).or_default().push(service);
    }
    for (targets, services) in groups {
        if services.len() < 2 {
            continue;
        }
        let names: Vec<String> = services.iter().map(|node| node.key.clone()).collect();
        let subject = format!("services/{}", names[0]);
        findings.push(Finding::new(
            DiagCode::DuplicateSelectors,
            subject,
            None,
            format!(
                "services {} all select the same workload set {{{}}}",
                names.join(", "),
                targets.iter().cloned().collect::<Vec<_>>().join(", ")
            ),
            BTreeMap::from([
                ("services".to_owned(), names.join(", ")),
                (
                    "workloads".to_owned(),
                    targets.iter().cloned().collect::<Vec<_>>().join(", "),
                ),
            ]),
        ));
    }
}
