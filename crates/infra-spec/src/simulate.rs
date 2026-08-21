//! Evaluating a desired-state specification against a compiled snapshot: three values, and a
//! reason for the third.
//!
//! # A simulation is a report, not a gate
//!
//! IW2 decision 6, carried forward without change. `protocol infra simulate` exits 0 whatever the
//! verdicts say — a cluster that fails nine expectations has been *successfully* simulated, and
//! that report is the product. Exit 1 stays where it was: an input that could not be simulated at
//! all, a refused specification, a refused bundle, a tampered IR document. Anything else would
//! make "simulate in CI" turn a declared aspiration into a broken build on the day somebody
//! writes the aspiration down, which is precisely the day it is least true.
//!
//! # The shape that makes invariant 5 structural
//!
//! [`Outcome`] has three variants and each carries what that verdict owes:
//!
//! | outcome | carries | so it is impossible to |
//! |---|---|---|
//! | [`Holds`](Outcome::Holds) | nothing | — |
//! | [`Gap`](Outcome::Gap) | the typed have-versus-want | report a failure nobody can act on |
//! | [`Undecidable`](Outcome::Undecidable) | the reason the snapshot cannot decide | write `Unknown` and mean "false" |
//!
//! There is no `Outcome::from_bool`, no `Option<Gap>` beside a separate verdict field, and the
//! verdict is *derived* from the variant ([`Outcome::verdict`]) rather than stored beside it. So a
//! future rule cannot produce a `False` without saying what would have to change, and cannot
//! produce an `Unknown` without saying why — which is the enforcement invariant 5 asks for,
//! expressed the way [`Truth`] itself expresses it: by having no other shape available.
//!
//! An expectation's verdict is the Kleene conjunction over its subjects
//! ([`Truth::and`]), so a single `False` subject decides the expectation and an undecidable
//! subject only surfaces when nothing beside it is false. That is the same fold
//! [`Predicate::All`] already uses, and it is deliberately
//! not "the worst of the three": `Unknown` beside `False` is still `False`, because something
//! *was* observed to be wrong.

use std::collections::BTreeMap;

use aep_domain::predicate::{Predicate, Truth};
use infra_analyze::{parse_image, properties_with, InfraGraph, WorkloadProperties};
use infra_compiler::{InfraIr, ResolvedWorkload, UnresolvedReference, UnresolvedTarget};
use infra_domain::network::Service;
use infra_domain::workload::WorkloadKind;
use serde::Serialize;

use crate::facts::{workload_facts, WorkloadFacts};
use crate::spec::{Expectation, ExpectationKind, InfraSpec, Scope};

/// The format string a persisted simulation document carries.
pub const SIMULATION_FORMAT: &str = "infra-simulation/1";

/// Why a snapshot cannot decide an expectation about a subject.
///
/// Closed, and every variant names something a reader can go and change: run the scanner with
/// another kind, declare the field, observe the namespace, or accept that the pod really is
/// unattributable. "Unknown" on its own would name none of them.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(tag = "reason", rename_all = "snake_case")]
pub enum UnknownReason {
    /// The scope selected no subject at all.
    ///
    /// Deliberately not vacuous truth. "Every container in `payments` declares limits" over a
    /// namespace holding no workloads is a sentence the snapshot has no subject for, and
    /// answering `True` would let an expectation pass by selecting nothing — the one way an
    /// expectation can be green and mean nothing.
    NoSubjectInScope,
    /// The scope names a namespace the scan did not observe.
    NamespaceUnobserved {
        /// The namespace named.
        namespace: String,
    },
    /// The bundle did not scan the kind the expectation reads.
    ///
    /// Unobserved is not absent — `INFRA-BUNDLE-002`'s argument, honoured in the direction that
    /// costs a verdict.
    KindUnscanned {
        /// The bundle key that was not scanned, such as `poddisruptionbudgets`.
        kind: String,
    },
    /// The subject does not carry the field the expectation reads — a daemonset has no declared
    /// replica count, an image names no registry, a service has no selector.
    FieldAbsent {
        /// The subject's IR key.
        subject: String,
        /// The field that is not there.
        field: String,
    },
    /// A pod in the subject's namespace has a controller the graph refuses to guess, so no pod
    /// count in that namespace is a count.
    OwnershipUnderivable {
        /// The pod whose controller stayed underived.
        pod: String,
    },
    /// A predicate read a fact the projection withheld, and this is why it was withheld.
    FactWithheld {
        /// The fact path the predicate reads.
        path: String,
        /// Why the projection declined to state it.
        because: Box<UnknownReason>,
    },
}

impl std::fmt::Display for UnknownReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoSubjectInScope => f.write_str("the scope selects no subject in this snapshot"),
            Self::NamespaceUnobserved { namespace } => {
                write!(f, "namespace `{namespace}` was not observed")
            }
            Self::KindUnscanned { kind } => write!(f, "the bundle did not scan `{kind}`"),
            Self::FieldAbsent { subject, field } => {
                write!(f, "`{subject}` declares no `{field}`")
            }
            Self::OwnershipUnderivable { pod } => write!(
                f,
                "`{pod}` has an underivable controller, so pod counts in its namespace are a \
                 lower bound"
            ),
            Self::FactWithheld { path, because } => write!(f, "`{path}` is not stated: {because}"),
        }
    }
}

/// What would have to change for a subject to satisfy an expectation: have, beside want.
///
/// One variant per expectation kind, and no catch-all. A gap a reader cannot act on is a finding
/// that will not be acted on, which is the whole argument `INFRA-DIAG-*` evidence maps already
/// make on the observed side.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(tag = "gap", rename_all = "snake_case")]
pub enum Gap {
    /// No workload of that kind and name is in that namespace.
    WorkloadAbsent {
        /// The namespace expected to hold it.
        namespace: String,
        /// The kind expected.
        kind: WorkloadKind,
        /// The name expected.
        name: String,
    },
    /// The declared replica count is outside the range.
    ReplicasOutsideRange {
        /// What the snapshot declares.
        have: u32,
        /// The lowest acceptable count.
        want_min: u32,
        /// The highest acceptable count.
        want_max: u32,
    },
    /// A container declares no requests, no limits, or neither.
    ResourcesAbsent {
        /// Which container.
        container: String,
        /// `true` when requests are missing.
        requests_missing: bool,
        /// `true` when limits are missing.
        limits_missing: bool,
    },
    /// A container is missing a probe the expectation asks for.
    ProbeAbsent {
        /// Which container.
        container: String,
        /// `true` when a liveness probe was asked for and is missing.
        liveness_missing: bool,
        /// `true` when a readiness probe was asked for and is missing.
        readiness_missing: bool,
    },
    /// A container's image names a registry outside the allowlist.
    ImageRegistryNotAllowed {
        /// Which container.
        container: String,
        /// The image reference as declared.
        image: String,
        /// The registry it names.
        have: String,
        /// The registries the expectation allows.
        allowed: Vec<String>,
    },
    /// A container's image resolves to `latest`.
    ImageTagIsLatest {
        /// Which container.
        container: String,
        /// The image reference as declared.
        image: String,
        /// The tag it states, absent when the reference is untagged — which resolves to `latest`
        /// by the runtimes' own rule.
        tag: Option<String>,
    },
    /// A container's image is not pinned by digest.
    ImageNotPinned {
        /// Which container.
        container: String,
        /// The image reference as declared.
        image: String,
    },
    /// A multi-replica workload has no covering disruption budget.
    DisruptionBudgetAbsent {
        /// How many replicas it declares.
        replicas: u32,
    },
    /// A service's selector matches no observed pod.
    SelectorMatchesNoPod {
        /// The selector that matched nothing.
        selector: BTreeMap<String, String>,
    },
    /// A required configmap or secret reference names something the scan did not observe.
    ReferenceUnresolved {
        /// Where in the workload the reference sits.
        site: String,
        /// What it names.
        target: String,
    },
    /// A workload sits in a namespace outside the allowlist.
    NamespaceNotAllowed {
        /// The namespace it sits in.
        have: String,
        /// The namespaces the expectation allows.
        allowed: Vec<String>,
    },
    /// A predicate evaluated `False` against the subject's facts.
    PredicateFalse {
        /// The predicate as written.
        predicate: String,
        /// The facts it read, with the values it read them at — the evidence behind the verdict.
        facts: BTreeMap<String, String>,
    },
}

/// What a snapshot decided about one subject.
///
/// The verdict is the variant. See the module doc for why it is not a field.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum Outcome {
    /// The subject satisfies the expectation.
    Holds,
    /// It does not, and this is what would have to change.
    Gap(Gap),
    /// The snapshot cannot decide, and this is why.
    Undecidable(UnknownReason),
}

impl Outcome {
    /// The truth value this outcome is.
    ///
    /// Derived, not stored: there is no way to construct an [`Outcome`] whose verdict disagrees
    /// with what it carries.
    pub fn verdict(&self) -> Truth {
        match self {
            Self::Holds => Truth::True,
            Self::Gap(_) => Truth::False,
            Self::Undecidable(_) => Truth::Unknown,
        }
    }
}

/// One subject's outcome, named.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct SubjectOutcome {
    /// The subject's IR key — `workloads/shop/deployment/api`, `services/shop/lost-lookup`, or
    /// the scope itself when the expectation could not reach a subject at all.
    pub subject: String,
    /// What the snapshot decided.
    #[serde(flatten)]
    pub outcome: Outcome,
}

/// One expectation's verdict, with the subjects behind it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExpectationReport {
    /// The expectation's id.
    pub id: String,
    /// What it claims, rendered from the kind — so a report is readable without the
    /// specification beside it.
    pub claim: String,
    /// The author's own statement, when the document carries one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub statement: Option<String>,
    /// The scope, rendered.
    pub scope: String,
    /// The expectation kind's wire name, for a harness that matches rather than reads.
    pub kind: String,
    /// The Kleene conjunction over the subjects.
    pub verdict: Truth,
    /// Every subject the scope selected, in IR-key order — the evidence behind a `True` as much
    /// as behind a `False`, because "it held" is a different claim over eight workloads than
    /// over none.
    pub subjects: Vec<String>,
    /// Every subject that did not simply hold, in the same order. A holding subject is named in
    /// `subjects` and carries nothing else to say.
    pub outcomes: Vec<SubjectOutcome>,
}

/// How many expectations landed on each verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Summary {
    /// How many expectations were evaluated.
    pub expectations: usize,
    /// How many hold.
    pub holds: usize,
    /// How many are contradicted by the snapshot.
    pub gaps: usize,
    /// How many the snapshot cannot decide.
    pub undecidable: usize,
}

/// Which snapshot a simulation was run against.
///
/// The context and the digest, and deliberately not `scanned_at`: a report that carries the scan
/// clock is a report whose bytes change when nothing did, and this document is drift-checked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SnapshotRef {
    /// The kubeconfig context the scan targeted.
    pub context: String,
    /// The IR's content digest.
    pub digest: String,
}

/// The whole report: what was expected, of which snapshot, and what it decided.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Simulation {
    /// The format claim, `infra-simulation/1`.
    pub format: &'static str,
    /// The specification's name, as it declares itself.
    pub specification: String,
    /// The snapshot it was evaluated against.
    pub snapshot: SnapshotRef,
    /// One report per expectation, in the specification's own order.
    pub reports: Vec<ExpectationReport>,
    /// The counts.
    pub summary: Summary,
}

impl Simulation {
    /// The canonical JSON document, key-sorted, with a trailing newline.
    ///
    /// Through [`serde_json::Value`] for the reason [`InfraIr::digest`] goes through one: a
    /// key-sorted rendering is reproducible by any reader, where struct field order is this
    /// crate's private business.
    #[must_use]
    pub fn to_json(&self) -> String {
        let value = serde_json::to_value(self).expect("a simulation has no non-serializable state");
        let mut rendered =
            serde_json::to_string_pretty(&value).expect("a JSON value renders as JSON");
        rendered.push('\n');
        rendered
    }
}

/// Evaluates every expectation of `spec` against `ir`.
///
/// Total: there is no failure mode. A specification is validated before it exists and an
/// [`InfraIr`] exists because it compiled, so the only things left to say are verdicts.
#[must_use]
pub fn simulate(spec: &InfraSpec, ir: &InfraIr) -> Simulation {
    let graph = InfraGraph::of(ir);
    let properties: BTreeMap<String, WorkloadProperties> = properties_with(ir, &graph)
        .into_iter()
        .map(|sheet| (sheet.workload.clone(), sheet))
        .collect();
    let facts = workload_facts(ir, &graph);

    let reports: Vec<ExpectationReport> = spec
        .expectations
        .iter()
        .map(|expectation| report(expectation, ir, &properties, &facts))
        .collect();

    let mut summary = Summary {
        expectations: reports.len(),
        holds: 0,
        gaps: 0,
        undecidable: 0,
    };
    for report in &reports {
        match report.verdict {
            Truth::True => summary.holds += 1,
            Truth::False => summary.gaps += 1,
            Truth::Unknown => summary.undecidable += 1,
        }
    }

    Simulation {
        format: SIMULATION_FORMAT,
        specification: spec.name.clone(),
        snapshot: SnapshotRef {
            context: ir.provenance.context.clone(),
            digest: ir.digest(),
        },
        reports,
        summary,
    }
}

/// Evaluates one expectation.
fn report(
    expectation: &Expectation,
    ir: &InfraIr,
    properties: &BTreeMap<String, WorkloadProperties>,
    facts: &BTreeMap<String, WorkloadFacts>,
) -> ExpectationReport {
    let (subjects, outcomes) = evaluate(expectation, ir, properties, facts);
    // The Kleene fold, and the reason it is a fold rather than a max: `Unknown` beside `False` is
    // `False`, because something was observed to be wrong. `Truth::and` already says so.
    let verdict = outcomes.iter().fold(Truth::True, |accumulated, outcome| {
        accumulated.and(outcome.outcome.verdict())
    });
    ExpectationReport {
        id: expectation.id.clone(),
        claim: expectation.kind.to_string(),
        statement: expectation.statement.clone(),
        scope: expectation.scope.to_string(),
        kind: expectation.kind.as_str().to_owned(),
        verdict,
        subjects,
        outcomes: outcomes
            .into_iter()
            .filter(|outcome| outcome.outcome != Outcome::Holds)
            .collect(),
    }
}

/// The subjects a scope selected and what each of them decided.
fn evaluate(
    expectation: &Expectation,
    ir: &InfraIr,
    properties: &BTreeMap<String, WorkloadProperties>,
    facts: &BTreeMap<String, WorkloadFacts>,
) -> (Vec<String>, Vec<SubjectOutcome>) {
    // The one kind that names its own subject: cluster-scoped, so there is nothing to select.
    if let ExpectationKind::WorkloadExists {
        namespace,
        kind,
        name,
    } = &expectation.kind
    {
        let key = format!("{namespace}/{}/{name}", kind.as_str());
        let subject = format!("workloads/{key}");
        let outcome = if ir.model.workloads.contains_key(&key) {
            Outcome::Holds
        } else {
            Outcome::Gap(Gap::WorkloadAbsent {
                namespace: namespace.clone(),
                kind: *kind,
                name: name.clone(),
            })
        };
        return (
            vec![subject.clone()],
            vec![SubjectOutcome { subject, outcome }],
        );
    }

    if let ExpectationKind::ServiceSelectorResolves = &expectation.kind {
        return match select_services(ir, &expectation.scope) {
            Err(reason) => undecidable_scope(&expectation.scope, reason),
            Ok(services) => {
                let subjects: Vec<String> = services
                    .iter()
                    .map(|(key, _)| format!("services/{key}"))
                    .collect();
                let outcomes = services
                    .into_iter()
                    .map(|(key, service)| SubjectOutcome {
                        subject: format!("services/{key}"),
                        outcome: selector_outcome(ir, &key, service),
                    })
                    .collect();
                (subjects, outcomes)
            }
        };
    }

    let selected = match select_workloads(ir, &expectation.scope) {
        Err(reason) => return undecidable_scope(&expectation.scope, reason),
        Ok(keys) => keys,
    };
    if selected.is_empty() {
        return undecidable_scope(&expectation.scope, UnknownReason::NoSubjectInScope);
    }

    let subjects: Vec<String> = selected
        .iter()
        .map(|key| format!("workloads/{key}"))
        .collect();
    let outcomes = selected
        .into_iter()
        .map(|key| {
            let workload = &ir.model.workloads[&key];
            SubjectOutcome {
                subject: format!("workloads/{key}"),
                outcome: workload_outcome(&expectation.kind, ir, &key, workload, properties, facts),
            }
        })
        .collect();
    (subjects, outcomes)
}

/// The report shape for a scope that could not reach a subject: the scope itself is the subject.
fn undecidable_scope(scope: &Scope, reason: UnknownReason) -> (Vec<String>, Vec<SubjectOutcome>) {
    let subject = scope.to_string();
    (
        Vec::new(),
        vec![SubjectOutcome {
            subject,
            outcome: Outcome::Undecidable(reason),
        }],
    )
}

/// Every workload key a scope selects, or the reason the scope cannot be resolved at all.
fn select_workloads(ir: &InfraIr, scope: &Scope) -> Result<Vec<String>, UnknownReason> {
    if let Some(namespace) = scope.namespace() {
        if !ir.model.namespaces.contains_key(namespace) {
            return Err(UnknownReason::NamespaceUnobserved {
                namespace: namespace.to_owned(),
            });
        }
    }
    Ok(ir
        .model
        .workloads
        .iter()
        .filter(|(_, workload)| match scope {
            Scope::Cluster => true,
            Scope::Namespace { name } => workload.identity.namespace.as_deref() == Some(name),
            Scope::WorkloadSelector { labels } => labels
                .iter()
                .all(|(key, value)| workload.labels.get(key) == Some(value)),
        })
        .map(|(key, _)| key.clone())
        .collect())
}

/// Every service a scope selects, or the reason the scope cannot be resolved at all.
fn select_services<'a>(
    ir: &'a InfraIr,
    scope: &Scope,
) -> Result<Vec<(String, &'a Service)>, UnknownReason> {
    if let Some(namespace) = scope.namespace() {
        if !ir.model.namespaces.contains_key(namespace) {
            return Err(UnknownReason::NamespaceUnobserved {
                namespace: namespace.to_owned(),
            });
        }
    }
    let selected: Vec<(String, &Service)> = ir
        .model
        .services
        .iter()
        .filter(|(_, service)| match scope {
            Scope::Cluster => true,
            Scope::Namespace { name } => service.identity.namespace.as_deref() == Some(name),
            // Validation refuses this pairing (`INFRA-SPEC-006`), so it is unreachable from a
            // document; selecting nothing is the honest answer if it is ever reached from code.
            Scope::WorkloadSelector { .. } => false,
        })
        .map(|(key, service)| (key.clone(), service))
        .collect();
    if selected.is_empty() {
        return Err(UnknownReason::NoSubjectInScope);
    }
    Ok(selected)
}

/// Whether a service's selector resolves to at least one observed pod.
fn selector_outcome(ir: &InfraIr, key: &str, service: &Service) -> Outcome {
    if service.selector.is_empty() {
        return Outcome::Undecidable(UnknownReason::FieldAbsent {
            subject: format!("services/{key}"),
            field: "selector".to_owned(),
        });
    }
    let from = format!("services/{key}");
    let dangling = ir.model.unresolved.iter().any(|reference| {
        reference.from == from
            && matches!(
                reference.target,
                UnresolvedTarget::PodsMatchingSelector { .. }
            )
    });
    if dangling {
        Outcome::Gap(Gap::SelectorMatchesNoPod {
            selector: service.selector.clone(),
        })
    } else {
        Outcome::Holds
    }
}

/// Decides one workload-class expectation about one workload.
fn workload_outcome(
    kind: &ExpectationKind,
    ir: &InfraIr,
    key: &str,
    workload: &ResolvedWorkload,
    properties: &BTreeMap<String, WorkloadProperties>,
    facts: &BTreeMap<String, WorkloadFacts>,
) -> Outcome {
    let subject = format!("workloads/{key}");
    match kind {
        // Handled before selection; a scope never reaches them.
        ExpectationKind::WorkloadExists { .. } | ExpectationKind::ServiceSelectorResolves => {
            Outcome::Holds
        }
        ExpectationKind::ReplicasWithin { min, max } => match workload.replicas {
            None => Outcome::Undecidable(UnknownReason::FieldAbsent {
                subject,
                field: "replicas".to_owned(),
            }),
            Some(replicas) if replicas < *min || replicas > *max => {
                Outcome::Gap(Gap::ReplicasOutsideRange {
                    have: replicas,
                    want_min: *min,
                    want_max: *max,
                })
            }
            Some(_) => Outcome::Holds,
        },
        ExpectationKind::ResourcesDeclared => workload
            .containers
            .iter()
            .find_map(|container| {
                let requests_missing = container.resources.requests.is_empty();
                let limits_missing = container.resources.limits.is_empty();
                (requests_missing || limits_missing).then(|| {
                    Outcome::Gap(Gap::ResourcesAbsent {
                        container: container.name.clone(),
                        requests_missing,
                        limits_missing,
                    })
                })
            })
            .unwrap_or(Outcome::Holds),
        ExpectationKind::ProbesDeclared {
            liveness,
            readiness,
        } => workload
            .containers
            .iter()
            .find_map(|container| {
                let liveness_missing = *liveness && container.probes.liveness.is_none();
                let readiness_missing = *readiness && container.probes.readiness.is_none();
                (liveness_missing || readiness_missing).then(|| {
                    Outcome::Gap(Gap::ProbeAbsent {
                        container: container.name.clone(),
                        liveness_missing,
                        readiness_missing,
                    })
                })
            })
            .unwrap_or(Outcome::Holds),
        ExpectationKind::ImageRegistry { .. }
        | ExpectationKind::ImageTagNotLatest
        | ExpectationKind::ImagePinnedByDigest => image_outcome(kind, &subject, workload),
        ExpectationKind::PdbCoversMultiReplica => {
            let sheet = &properties[key];
            let Some(replicas) = sheet.replicas else {
                return Outcome::Undecidable(UnknownReason::FieldAbsent {
                    subject,
                    field: "replicas".to_owned(),
                });
            };
            let Some(covering) = sheet.pod_disruption_budgets.as_ref() else {
                return Outcome::Undecidable(UnknownReason::KindUnscanned {
                    kind: "poddisruptionbudgets".to_owned(),
                });
            };
            if replicas > 1 && covering.is_empty() {
                Outcome::Gap(Gap::DisruptionBudgetAbsent { replicas })
            } else {
                Outcome::Holds
            }
        }
        ExpectationKind::ConfigReferencesResolve => ir
            .model
            .unresolved
            .iter()
            .filter(|reference| reference.from == subject)
            .find_map(|reference| {
                is_config_reference(&reference.target)
                    .then(|| unresolved_gap(reference))
                    .flatten()
            })
            .map_or(Outcome::Holds, Outcome::Gap),
        ExpectationKind::NamespaceAllowlist { allowed } => {
            let namespace = workload.identity.namespace.clone().unwrap_or_default();
            if allowed.contains(&namespace) {
                Outcome::Holds
            } else {
                Outcome::Gap(Gap::NamespaceNotAllowed {
                    have: namespace,
                    allowed: allowed.clone(),
                })
            }
        }
        ExpectationKind::WorkloadPredicate { predicate } => {
            predicate_outcome(predicate, &facts[key])
        }
    }
}

/// Decides the three image expectations about one workload.
///
/// One owner for "what is this image", reading [`parse_image`] exactly as `INFRA-DIAG-006` does —
/// a second parser here could disagree with the diagnosis about the same reference.
fn image_outcome(kind: &ExpectationKind, subject: &str, workload: &ResolvedWorkload) -> Outcome {
    for container in &workload.containers {
        let parsed = parse_image(&container.image);
        match kind {
            ExpectationKind::ImageRegistry { allowed } => {
                let Some(registry) = parsed.registry else {
                    // The default registry resolves it, and the snapshot does not carry which one
                    // that is. Blaming the image for the node's configuration would be a verdict
                    // about something nobody observed.
                    return Outcome::Undecidable(UnknownReason::FieldAbsent {
                        subject: subject.to_owned(),
                        field: format!("containers[{}].image registry", container.name),
                    });
                };
                if !allowed.contains(&registry) {
                    return Outcome::Gap(Gap::ImageRegistryNotAllowed {
                        container: container.name.clone(),
                        image: container.image.clone(),
                        have: registry,
                        allowed: allowed.clone(),
                    });
                }
            }
            // Untagged is `latest` by the runtimes' own rule, so both spell the same gap; a
            // digest pin makes whatever tag rides along decoration.
            ExpectationKind::ImageTagNotLatest
                if parsed.digest.is_none()
                    && parsed.tag.as_deref().unwrap_or("latest") == "latest" =>
            {
                return Outcome::Gap(Gap::ImageTagIsLatest {
                    container: container.name.clone(),
                    image: container.image.clone(),
                    tag: parsed.tag.clone(),
                });
            }
            ExpectationKind::ImagePinnedByDigest if parsed.digest.is_none() => {
                return Outcome::Gap(Gap::ImageNotPinned {
                    container: container.name.clone(),
                    image: container.image.clone(),
                });
            }
            _ => {}
        }
    }
    Outcome::Holds
}

/// Evaluates the escape hatch against one workload's projection.
///
/// The `Unknown` arm is not written here — [`Predicate::evaluate`] produces it whenever a fact it
/// reads has no value, and this function's only job is to attach *which* withheld fact caused it.
/// Deleting the lookup would lose the reason, not the verdict, which is why the mutation register
/// breaks the arm rather than the lookup.
fn predicate_outcome(predicate: &Predicate, facts: &WorkloadFacts) -> Outcome {
    match predicate.evaluate(&facts.store) {
        Truth::True => Outcome::Holds,
        Truth::False => Outcome::Gap(Gap::PredicateFalse {
            predicate: predicate.to_string(),
            facts: read_facts(predicate, facts),
        }),
        Truth::Unknown => {
            let withheld = predicate.fact_paths().into_iter().find_map(|path| {
                let rendered = path.to_string();
                facts
                    .withheld
                    .get(&rendered)
                    .map(|reason| UnknownReason::FactWithheld {
                        path: rendered,
                        because: Box::new(reason.clone()),
                    })
            });
            Outcome::Undecidable(
                withheld.unwrap_or(UnknownReason::FieldAbsent {
                    subject: format!("workloads/{}", facts.workload),
                    field: predicate
                        .fact_paths()
                        .first()
                        .map_or_else(|| "fact".to_owned(), ToString::to_string),
                }),
            )
        }
    }
}

/// The values a predicate read, as the evidence behind a `False`.
fn read_facts(predicate: &Predicate, facts: &WorkloadFacts) -> BTreeMap<String, String> {
    use aep_domain::facts::FactSource as _;
    predicate
        .fact_paths()
        .into_iter()
        .filter_map(|path| {
            facts
                .store
                .fact(path)
                .map(|value| (path.to_string(), value.to_string()))
        })
        .collect()
}

/// `true` when an unresolved target is a configmap or secret reference.
fn is_config_reference(target: &UnresolvedTarget) -> bool {
    matches!(
        target,
        UnresolvedTarget::ConfigMap { .. }
            | UnresolvedTarget::ConfigMapKey { .. }
            | UnresolvedTarget::Secret { .. }
            | UnresolvedTarget::SecretKey { .. }
    )
}

/// The gap a dangling reference is — or `None` when the cluster declared it may dangle.
///
/// The optional split is `INFRA-DIAG-002` versus `-003`, kept identical: a reference marked
/// `optional: true` is a statement that the object need not exist, and failing an expectation on
/// one would contradict what the cluster itself declared.
fn unresolved_gap(reference: &UnresolvedReference) -> Option<Gap> {
    is_required(&reference.target).then(|| Gap::ReferenceUnresolved {
        site: reference.site.clone(),
        target: describe_target(&reference.target),
    })
}

/// `true` when a dangling reference is one the cluster did *not* declare optional.
pub(crate) fn is_required(target: &UnresolvedTarget) -> bool {
    match target {
        UnresolvedTarget::ConfigMap { optional, .. }
        | UnresolvedTarget::ConfigMapKey { optional, .. }
        | UnresolvedTarget::Secret { optional, .. }
        | UnresolvedTarget::SecretKey { optional, .. } => !optional,
        UnresolvedTarget::ServiceAccount { .. }
        | UnresolvedTarget::Claim { .. }
        | UnresolvedTarget::Service { .. }
        | UnresolvedTarget::Node { .. }
        | UnresolvedTarget::Namespace { .. }
        | UnresolvedTarget::PodsMatchingSelector { .. } => true,
    }
}

/// One line naming what a dangling reference pointed at.
pub(crate) fn describe_target(target: &UnresolvedTarget) -> String {
    match target {
        UnresolvedTarget::ConfigMap { name, .. } => format!("configmap {name}"),
        UnresolvedTarget::ConfigMapKey { name, key, .. } => format!("configmap {name} key {key}"),
        UnresolvedTarget::Secret { name, .. } => format!("secret {name}"),
        UnresolvedTarget::SecretKey { name, key, .. } => format!("secret {name} key {key}"),
        UnresolvedTarget::ServiceAccount { name } => format!("service account {name}"),
        UnresolvedTarget::Claim { name } => format!("claim {name}"),
        UnresolvedTarget::Service { name } => format!("service {name}"),
        UnresolvedTarget::Node { name } => format!("node {name}"),
        UnresolvedTarget::Namespace { name } => format!("namespace {name}"),
        UnresolvedTarget::PodsMatchingSelector { selector } => format!(
            "pods matching {}",
            selector
                .iter()
                .map(|(key, value)| format!("{key}={value}"))
                .collect::<Vec<_>>()
                .join(",")
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_verdict_is_the_outcome_variant_and_not_a_field_beside_it() {
        assert_eq!(Outcome::Holds.verdict(), Truth::True);
        assert_eq!(
            Outcome::Gap(Gap::DisruptionBudgetAbsent { replicas: 3 }).verdict(),
            Truth::False
        );
        assert_eq!(
            Outcome::Undecidable(UnknownReason::NoSubjectInScope).verdict(),
            Truth::Unknown
        );
    }

    #[test]
    fn an_undecidable_subject_beside_a_gap_still_reads_false() {
        // The Kleene fold, stated as a test because it is the one place `Unknown` and `False`
        // meet: something *was* observed to be wrong, so the expectation is wrong.
        let folded = [
            Outcome::Undecidable(UnknownReason::NoSubjectInScope),
            Outcome::Gap(Gap::DisruptionBudgetAbsent { replicas: 2 }),
        ]
        .iter()
        .fold(Truth::True, |accumulated, outcome| {
            accumulated.and(outcome.verdict())
        });
        assert_eq!(folded, Truth::False);
    }

    #[test]
    fn an_undecidable_subject_beside_only_holding_ones_reads_unknown_not_true() {
        let folded = [
            Outcome::Holds,
            Outcome::Undecidable(UnknownReason::KindUnscanned {
                kind: "poddisruptionbudgets".to_owned(),
            }),
            Outcome::Holds,
        ]
        .iter()
        .fold(Truth::True, |accumulated, outcome| {
            accumulated.and(outcome.verdict())
        });
        assert_eq!(
            folded,
            Truth::Unknown,
            "an expectation nothing contradicted and something could not decide is not satisfied"
        );
    }

    #[test]
    fn an_optional_dangling_reference_is_not_required_and_a_plain_one_is() {
        assert!(!is_required(&UnresolvedTarget::ConfigMap {
            name: "coredns-custom".to_owned(),
            optional: true,
        }));
        assert!(is_required(&UnresolvedTarget::Secret {
            name: "agent-credentials".to_owned(),
            optional: false,
        }));
        assert!(is_required(&UnresolvedTarget::Claim {
            name: "orphan-cache".to_owned(),
        }));
    }
}
