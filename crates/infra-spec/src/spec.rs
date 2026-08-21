//! The desired-state model: what somebody expects an observed cluster to be.
//!
//! # Small on purpose
//!
//! Twelve expectation kinds, each with its own typed form, each decidable from a compiled
//! snapshot alone. The bar for admitting a kind is not "an operator might want it" — it is
//! **can a snapshot decide it, and can the report say what would have to change**. A kind that
//! answers the first and not the second produces a verdict nobody can act on, which is the
//! failure mode `INFRA-DIAG-*` already avoids by carrying evidence on every finding.
//!
//! # No wall clock, anywhere
//!
//! Review finding I7. Not one expectation compares a timestamp, and there is no vocabulary for a
//! duration: everything here is **snapshot-relative**. `now() − observed_at < 15m` is the first
//! predicate whose verdict changes when nothing changed, and a report that is not a function of
//! its inputs cannot be re-run, cached, committed or drift-checked. Freshness, if it is ever
//! wanted, arrives as a property of the *provenance* a caller decides about before simulating —
//! not as an expectation this crate evaluates.
//!
//! # Scopes select subjects; kinds decide about them
//!
//! An expectation is a (scope, kind) pair. The scope picks the subjects out of the snapshot, the
//! kind decides `True`/`False`/`Unknown` per subject, and the expectation's verdict is the Kleene
//! conjunction over them — so one `False` subject makes the expectation `False`, and an
//! undecidable subject with no false one beside it makes it `Unknown`. There is no majority rule
//! and no percentage: `infra_analyze::invariants` already answers "what does this cluster
//! *almost* do", and that is a different question from "does it do what I declared".

use std::collections::BTreeMap;
use std::fmt;

use aep_domain::predicate::Predicate;
use infra_domain::workload::WorkloadKind;
use serde::Serialize;

/// The format string a desired-state specification carries.
pub const SPEC_FORMAT: &str = "infra-spec/1";

/// What an expectation is about, which is what decides whether a scope can select it.
///
/// Not written in a document — derived from the kind, so a specification cannot claim a subject
/// class its kind does not have.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SubjectClass {
    /// Workloads: deployments, statefulsets, daemonsets.
    Workload,
    /// Services.
    Service,
    /// The cluster itself — the expectation names its own subject and a scope selects nothing.
    Cluster,
}

impl SubjectClass {
    /// The class as it appears in a refusal message.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Workload => "workloads",
            Self::Service => "services",
            Self::Cluster => "the cluster",
        }
    }
}

impl fmt::Display for SubjectClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Which subjects of a snapshot an expectation is about.
///
/// A namespace nobody observed is not an empty namespace, and a selector nothing matches is not a
/// satisfied expectation — both are [`Unknown`](aep_domain::predicate::Truth::Unknown) at
/// evaluation, with the reason attached. See [`crate::simulate::UnknownReason`].
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Scope {
    /// Every subject of the class in the snapshot.
    Cluster,
    /// Every subject of the class in one namespace.
    Namespace {
        /// The namespace's name.
        name: String,
    },
    /// Every workload whose own labels carry all of these pairs — `matchLabels` semantics, AND
    /// across the pairs, the same rule `pdb_covers` uses.
    WorkloadSelector {
        /// The label pairs a workload must all carry.
        labels: BTreeMap<String, String>,
    },
}

impl Scope {
    /// `true` when this scope can pick subjects of `class` out of a snapshot.
    ///
    /// A workload-label selector cannot select a service — services carry labels too, but a
    /// specification saying "every service labelled `tier: edge`" would be selecting by a
    /// different map than the one the name says, and silently agreeing to that is how a scope
    /// stops meaning one thing.
    pub fn selects(&self, class: SubjectClass) -> bool {
        match self {
            // Cluster scope is the only one that can carry an expectation naming its own
            // subject, and it selects every other class too.
            Self::Cluster => true,
            Self::Namespace { .. } => {
                matches!(class, SubjectClass::Workload | SubjectClass::Service)
            }
            Self::WorkloadSelector { .. } => matches!(class, SubjectClass::Workload),
        }
    }

    /// The namespace this scope restricts to, when it restricts to one.
    pub fn namespace(&self) -> Option<&str> {
        match self {
            Self::Namespace { name } => Some(name),
            Self::Cluster | Self::WorkloadSelector { .. } => None,
        }
    }
}

impl fmt::Display for Scope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cluster => f.write_str("cluster"),
            Self::Namespace { name } => write!(f, "namespace {name}"),
            Self::WorkloadSelector { labels } => {
                write!(f, "workloads matching {}", render_labels(labels))
            }
        }
    }
}

/// Renders a label map the way a `matchLabels` block reads.
fn render_labels(labels: &BTreeMap<String, String>) -> String {
    labels
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join(",")
}

/// The v1 expectation vocabulary: twelve kinds, each decidable from a snapshot.
///
/// Every kind's semantics — including exactly when it is `Unknown` — is on its variant, because
/// the `Unknown` arm is the part a reader will otherwise assume away.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "expect", rename_all = "snake_case")]
pub enum ExpectationKind {
    /// A workload with this namespace, kind and name was observed.
    ///
    /// Never `Unknown`: the three workload kinds are required in every bundle
    /// (`INFRA-BUNDLE-002`), so absence here is absence, not silence.
    WorkloadExists {
        /// The namespace it should be in.
        namespace: String,
        /// Which of the three kinds.
        kind: WorkloadKind,
        /// Its name.
        name: String,
    },
    /// Every workload in scope declares a replica count within `[min, max]`.
    ///
    /// `Unknown` for a daemonset, which declares no replica count at all — one pod per node is
    /// not a number the snapshot holds, and reading the absent field as zero would fail an
    /// expectation about something nobody stated.
    ReplicasWithin {
        /// The lowest acceptable declared replica count.
        min: u32,
        /// The highest acceptable declared replica count.
        max: u32,
    },
    /// Every container of every workload in scope declares both resource requests and limits.
    ///
    /// Never `Unknown`: a container's `resources` block is either in the snapshot or is not.
    ResourcesDeclared,
    /// Every container of every workload in scope declares the probes asked for.
    ///
    /// Never `Unknown`, for the reason [`ResourcesDeclared`](Self::ResourcesDeclared) is not.
    ProbesDeclared {
        /// Require a liveness probe.
        liveness: bool,
        /// Require a readiness probe.
        readiness: bool,
    },
    /// Every container image in scope names a registry from this set.
    ///
    /// `Unknown` when an image names no registry — `redis:7-alpine` is resolved by whatever the
    /// runtime's default registry is, and the snapshot does not carry that. Calling it a
    /// violation would blame an image for a fact about the node's configuration.
    ImageRegistry {
        /// The registry hosts an image may name, such as `registry.example.com`.
        allowed: Vec<String>,
    },
    /// No container image in scope resolves to `latest`.
    ///
    /// An untagged image is `latest` by the runtimes' own rule, so it is `False` too — unless the
    /// reference is pinned by digest, which makes the tag decoration. Never `Unknown`.
    ImageTagNotLatest,
    /// Every container image in scope is pinned by a `sha256:` digest. Never `Unknown`.
    ImagePinnedByDigest,
    /// Every workload in scope declaring more than one replica is covered by a disruption budget.
    ///
    /// `Unknown` twice over: when the bundle did not scan disruption budgets — unobserved is not
    /// uncovered, `INFRA-BUNDLE-002`'s argument in the other direction — and for a daemonset,
    /// whose replica count is absent, so whether the rule even applies is undecidable.
    PdbCoversMultiReplica,
    /// Every service in scope has a selector that matches at least one observed pod.
    ///
    /// `Unknown` for a service with an empty selector: its endpoints are managed by hand or by a
    /// controller, which is legal, and there is no selector to resolve.
    ServiceSelectorResolves,
    /// Every configmap and secret reference in scope resolves to an observed object.
    ///
    /// An *optional* reference that dangles is `True`: the cluster declared that the object may
    /// be absent, which is the same split `INFRA-DIAG-002` and `-003` make. Never `Unknown`.
    ConfigReferencesResolve,
    /// Every workload in scope sits in one of these namespaces. Never `Unknown`.
    NamespaceAllowlist {
        /// The namespaces a workload may live in.
        allowed: Vec<String>,
    },
    /// A labelled predicate over the workload facts the snapshot projects.
    ///
    /// The escape hatch, and the reason there is no second predicate language here: this is
    /// [`aep_domain::predicate::Predicate`] evaluated against
    /// [`workload facts`](crate::facts::WORKLOAD_FACTS) with
    /// [`Truth`](aep_domain::predicate::Truth) semantics unchanged. `Unknown` exactly when a fact
    /// the predicate reads is absent from a subject's projection, and the projection records why
    /// each absent fact is absent — so the report says "the bundle did not scan budgets", not
    /// "unknown".
    ///
    /// A predicate reading a path the projection never produces is refused at validation
    /// (`INFRA-SPEC-008`), because a typo that evaluates `Unknown` forever is indistinguishable
    /// from a snapshot that cannot decide.
    WorkloadPredicate {
        /// The condition, over the `workload.*` facts.
        predicate: Predicate,
    },
}

impl ExpectationKind {
    /// The wire discriminant, which is also what a report prints.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::WorkloadExists { .. } => "workload_exists",
            Self::ReplicasWithin { .. } => "replicas_within",
            Self::ResourcesDeclared => "resources_declared",
            Self::ProbesDeclared { .. } => "probes_declared",
            Self::ImageRegistry { .. } => "image_registry",
            Self::ImageTagNotLatest => "image_tag_not_latest",
            Self::ImagePinnedByDigest => "image_pinned_by_digest",
            Self::PdbCoversMultiReplica => "pdb_covers_multi_replica",
            Self::ServiceSelectorResolves => "service_selector_resolves",
            Self::ConfigReferencesResolve => "config_references_resolve",
            Self::NamespaceAllowlist { .. } => "namespace_allowlist",
            Self::WorkloadPredicate { .. } => "workload_predicate",
        }
    }

    /// What this kind is about, which decides which scopes may carry it.
    pub fn subject_class(&self) -> SubjectClass {
        match self {
            Self::WorkloadExists { .. } => SubjectClass::Cluster,
            Self::ServiceSelectorResolves => SubjectClass::Service,
            Self::ReplicasWithin { .. }
            | Self::ResourcesDeclared
            | Self::ProbesDeclared { .. }
            | Self::ImageRegistry { .. }
            | Self::ImageTagNotLatest
            | Self::ImagePinnedByDigest
            | Self::PdbCoversMultiReplica
            | Self::ConfigReferencesResolve
            | Self::NamespaceAllowlist { .. }
            | Self::WorkloadPredicate { .. } => SubjectClass::Workload,
        }
    }

    /// Every kind's wire discriminant, in declaration order — the vocabulary, generated rather
    /// than listed, so a new kind cannot be added without appearing here.
    pub const ALL: &'static [&'static str] = &[
        "workload_exists",
        "replicas_within",
        "resources_declared",
        "probes_declared",
        "image_registry",
        "image_tag_not_latest",
        "image_pinned_by_digest",
        "pdb_covers_multi_replica",
        "service_selector_resolves",
        "config_references_resolve",
        "namespace_allowlist",
        "workload_predicate",
    ];
}

impl fmt::Display for ExpectationKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WorkloadExists {
                namespace,
                kind,
                name,
            } => write!(f, "{}/{namespace}/{name} exists", kind.as_str()),
            Self::ReplicasWithin { min, max } => write!(f, "replicas within [{min}, {max}]"),
            Self::ResourcesDeclared => f.write_str("requests and limits declared"),
            Self::ProbesDeclared {
                liveness,
                readiness,
            } => {
                let mut wanted = Vec::new();
                if *liveness {
                    wanted.push("liveness");
                }
                if *readiness {
                    wanted.push("readiness");
                }
                write!(f, "{} probe declared", wanted.join(" and "))
            }
            Self::ImageRegistry { allowed } => {
                write!(f, "image registry in [{}]", allowed.join(", "))
            }
            Self::ImageTagNotLatest => f.write_str("image tag is not `latest`"),
            Self::ImagePinnedByDigest => f.write_str("image pinned by digest"),
            Self::PdbCoversMultiReplica => {
                f.write_str("a disruption budget covers every multi-replica workload")
            }
            Self::ServiceSelectorResolves => f.write_str("service selector matches a pod"),
            Self::ConfigReferencesResolve => {
                f.write_str("every required configmap and secret reference resolves")
            }
            Self::NamespaceAllowlist { allowed } => {
                write!(f, "namespace in [{}]", allowed.join(", "))
            }
            Self::WorkloadPredicate { predicate } => write!(f, "{predicate}"),
        }
    }
}

/// One expectation: an id a report names it by, what it is about, and where it applies.
///
/// Validated: the only way to hold one is through
/// [`TryFrom<RawInfraSpec>`](crate::raw::RawInfraSpec), so an [`Expectation`] whose scope cannot
/// select its own subject class does not exist (invariant 2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Expectation {
    /// The stable id a verdict is reported under. Lowercase letters, digits and dashes.
    pub id: String,
    /// What a reader should understand the expectation to mean, when the kind's own rendering is
    /// not enough. Free text, carried through to the report.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub statement: Option<String>,
    /// Which subjects it is about.
    pub scope: Scope,
    /// What it decides about each of them.
    #[serde(flatten)]
    pub kind: ExpectationKind,
}

/// A desired-state specification: expectations over one observed cluster.
///
/// Does not implement [`Deserialize`](serde::Deserialize) — invariant 2. The only route from a
/// document to one of these is [`RawInfraSpec`](crate::raw::RawInfraSpec) and its [`TryFrom`],
/// which is what makes "every expectation's scope can select its subject" a property of the type
/// rather than a rule somebody remembers to run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InfraSpec {
    /// The format claim, `infra-spec/1`.
    pub format: &'static str,
    /// What the specification is about, for a human reading a report. Free text.
    pub name: String,
    /// Every expectation, in the order the document declares them — which is the order a report
    /// prints them, so a reviewer can read the two side by side.
    pub expectations: Vec<Expectation>,
}

impl InfraSpec {
    /// Assembles a validated specification. Crate-private: validation is the constructor.
    pub(crate) fn assembled(name: String, expectations: Vec<Expectation>) -> Self {
        Self {
            format: SPEC_FORMAT,
            name,
            expectations,
        }
    }

    /// The expectation with this id, if the specification declares one.
    pub fn expectation(&self, id: &str) -> Option<&Expectation> {
        self.expectations
            .iter()
            .find(|expectation| expectation.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_workload_label_selector_cannot_select_a_service_or_the_cluster() {
        let selector = Scope::WorkloadSelector {
            labels: BTreeMap::from([("app".to_owned(), "shop".to_owned())]),
        };
        assert!(selector.selects(SubjectClass::Workload));
        assert!(
            !selector.selects(SubjectClass::Service),
            "a service's labels are a different map from a workload's"
        );
        assert!(!selector.selects(SubjectClass::Cluster));
    }

    #[test]
    fn only_cluster_scope_selects_an_expectation_that_names_its_own_subject() {
        let namespaced = Scope::Namespace {
            name: "shop".to_owned(),
        };
        assert!(Scope::Cluster.selects(SubjectClass::Cluster));
        assert!(!namespaced.selects(SubjectClass::Cluster));
        assert!(namespaced.selects(SubjectClass::Workload));
        assert!(namespaced.selects(SubjectClass::Service));
    }

    #[test]
    fn every_kind_declares_its_wire_name_in_the_generated_vocabulary() {
        let kinds = [
            ExpectationKind::WorkloadExists {
                namespace: "shop".to_owned(),
                kind: WorkloadKind::Deployment,
                name: "api".to_owned(),
            },
            ExpectationKind::ReplicasWithin { min: 1, max: 3 },
            ExpectationKind::ResourcesDeclared,
            ExpectationKind::ProbesDeclared {
                liveness: true,
                readiness: true,
            },
            ExpectationKind::ImageRegistry {
                allowed: vec!["registry.example.com".to_owned()],
            },
            ExpectationKind::ImageTagNotLatest,
            ExpectationKind::ImagePinnedByDigest,
            ExpectationKind::PdbCoversMultiReplica,
            ExpectationKind::ServiceSelectorResolves,
            ExpectationKind::ConfigReferencesResolve,
            ExpectationKind::NamespaceAllowlist {
                allowed: vec!["shop".to_owned()],
            },
            ExpectationKind::WorkloadPredicate {
                predicate: Predicate::Always,
            },
        ];
        assert_eq!(
            kinds.len(),
            ExpectationKind::ALL.len(),
            "the vocabulary list and the enum have to hold the same number of kinds"
        );
        for (kind, wire) in kinds.iter().zip(ExpectationKind::ALL) {
            assert_eq!(&kind.as_str(), wire, "{kind:?} renders as a different name");
        }
    }
}
