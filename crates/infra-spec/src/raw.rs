//! Reading a desired-state specification: the permissive half, and the rules deserialization
//! cannot see.
//!
//! Invariant 2, the second instance of it on the infrastructure side. A specification is a file
//! somebody writes, reviews and commits, so it becomes a domain type by *validating* rather than
//! by deserializing: [`InfraSpec`] does not implement `Deserialize`, this module holds the type
//! that does, and [`TryFrom`] is the only door.
//!
//! # What the shape already refuses, and what it cannot
//!
//! The wire form is externally tagged throughout — `expect: {replicas_within: {min: 2, max: 5}}`,
//! `scope: {namespace: shop}` — which buys two refusals for free: an expectation kind this build
//! does not implement is refused by name, and every variant carries `deny_unknown_fields`, so a
//! misspelt parameter is refused rather than defaulted. That is deliberate and it is the reason
//! the kind is nested under `expect:` instead of flattened beside `id:`: serde's
//! `deny_unknown_fields` does not survive `flatten`, and a specification where `mim: 2` silently
//! becomes `min: 0` is worse than one that is a line longer.
//!
//! What no deserializer can see is everything about the document as a whole, and about whether an
//! expectation can decide anything at all:
//!
//! | rule | code |
//! |---|---|
//! | the format is one this build implements | `INFRA-SPEC-001` |
//! | it declares at least one expectation | `INFRA-SPEC-004` |
//! | every id is a stable identifier | `INFRA-SPEC-007` |
//! | no id appears twice | `INFRA-SPEC-003` |
//! | every scope can select its expectation's subjects | `INFRA-SPEC-006` |
//! | every kind's own parameters can decide something | `INFRA-SPEC-005` |
//! | every predicate reads facts the projection produces | `INFRA-SPEC-008` |
//!
//! The last one is the specification-side twin of the IR's dangling-handle check: a predicate over
//! `workload.replica` would evaluate `Unknown` on every workload forever, and the report would say
//! "the snapshot cannot decide" about a typo. Refusing it at validation is the only place the
//! difference is still visible.
//!
//! # Errors accumulate
//!
//! Invariant 3. A specification with four broken expectations reports four refusals in one run.

use std::collections::BTreeMap;

use aep_domain::predicate::Predicate;
use infra_domain::code::{InfraCode, ValidationErrors};
use infra_domain::workload::WorkloadKind;

use crate::facts::{is_projected, WORKLOAD_FACTS};
use crate::spec::{Expectation, ExpectationKind, InfraSpec, Scope, SPEC_FORMAT};

/// Reads a specification's text — YAML or JSON — through its validation.
///
/// The one reader, so the command line and a harness cannot disagree about what a document
/// means. A text that does not deserialize at all is a single `INFRA-SPEC-002` refusal rather
/// than a raw serde sentence: a caller matches on a code here exactly as it does on a bundle.
///
/// # Errors
///
/// Every rule that failed, accumulated.
pub fn read_spec(text: &str) -> Result<InfraSpec, ValidationErrors> {
    // Through `serde_json::Value`: see the manifest's note beside `serde_yaml`. YAML's superset
    // shapes this model does not have — non-string keys, tags, aliases into maps — fail here with
    // the same code a malformed document does, which is the honest answer for a document this
    // build cannot read.
    let value: serde_json::Value = serde_yaml::from_str(text).map_err(|error| {
        let mut errors = ValidationErrors::new();
        errors.refuse(InfraCode::SpecMalformed, "document", error.to_string());
        errors
    })?;
    let raw: RawInfraSpec = serde_json::from_value(value).map_err(|error| {
        let mut errors = ValidationErrors::new();
        errors.refuse(InfraCode::SpecMalformed, "document", error.to_string());
        errors
    })?;
    InfraSpec::try_from(raw)
}

/// A specification as it is written, before anything has checked what it claims.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawInfraSpec {
    /// The shape the document says it is written in.
    pub format: String,
    /// What the specification is about, for a human reading a report.
    pub name: String,
    /// The expectations it declares.
    #[serde(default)]
    pub expectations: Vec<RawExpectation>,
}

/// One expectation as written.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawExpectation {
    /// The id a verdict is reported under.
    pub id: String,
    /// The author's own sentence, carried into the report unchanged.
    #[serde(default)]
    pub statement: Option<String>,
    /// Which subjects it is about; cluster-wide when the document omits it.
    #[serde(default)]
    pub scope: RawScope,
    /// What it decides about each of them.
    pub expect: RawExpectationKind,
}

/// A scope as written: `cluster`, `{namespace: shop}` or `{workload_selector: {app: shop}}`.
#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RawScope {
    /// Every subject in the snapshot.
    #[default]
    Cluster,
    /// Every subject in one namespace.
    Namespace(String),
    /// Every workload carrying all of these labels.
    WorkloadSelector(BTreeMap<String, String>),
}

/// An expectation kind as written, externally tagged so the parameter names are checked.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RawExpectationKind {
    /// `{workload_exists: {namespace: …, workload_kind: …, name: …}}`
    WorkloadExists(RawWorkloadExists),
    /// `{replicas_within: {min: …, max: …}}`
    ReplicasWithin(RawReplicasWithin),
    /// `resources_declared`
    ResourcesDeclared,
    /// `{probes_declared: {liveness: true, readiness: true}}`
    ProbesDeclared(RawProbesDeclared),
    /// `{image_registry: {allowed: [...]}}`
    ImageRegistry(RawAllowed),
    /// `image_tag_not_latest`
    ImageTagNotLatest,
    /// `image_pinned_by_digest`
    ImagePinnedByDigest,
    /// `pdb_covers_multi_replica`
    PdbCoversMultiReplica,
    /// `service_selector_resolves`
    ServiceSelectorResolves,
    /// `config_references_resolve`
    ConfigReferencesResolve,
    /// `{namespace_allowlist: {allowed: [...]}}`
    NamespaceAllowlist(RawAllowed),
    /// `{workload_predicate: "workload.ready_pods >= 1"}`
    WorkloadPredicate(Predicate),
}

/// The parameters of `workload_exists`.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawWorkloadExists {
    /// The namespace it should be in.
    pub namespace: String,
    /// `deployment`, `statefulset` or `daemonset`.
    pub workload_kind: String,
    /// Its name.
    pub name: String,
}

/// The parameters of `replicas_within`.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawReplicasWithin {
    /// The lowest acceptable declared replica count.
    pub min: u32,
    /// The highest acceptable declared replica count.
    pub max: u32,
}

/// The parameters of `probes_declared`.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawProbesDeclared {
    /// Require a liveness probe.
    #[serde(default)]
    pub liveness: bool,
    /// Require a readiness probe.
    #[serde(default)]
    pub readiness: bool,
}

/// The parameters of the two allowlist kinds.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawAllowed {
    /// What is permitted.
    pub allowed: Vec<String>,
}

impl TryFrom<RawInfraSpec> for InfraSpec {
    type Error = ValidationErrors;

    fn try_from(raw: RawInfraSpec) -> Result<Self, Self::Error> {
        let mut errors = ValidationErrors::new();

        if raw.format != SPEC_FORMAT {
            errors.refuse(
                InfraCode::SpecUnsupportedFormat,
                "format",
                format!(
                    "this build reads `{SPEC_FORMAT}`, and the document is written in `{}`",
                    raw.format
                ),
            );
        }

        if raw.expectations.is_empty() {
            errors.refuse(
                InfraCode::SpecEmptyExpectations,
                "expectations",
                "a specification that expects nothing simulates to nothing, and a report with no \
                 content reads exactly like a report with no gaps",
            );
        }

        let mut seen: BTreeMap<String, usize> = BTreeMap::new();
        let mut expectations = Vec::with_capacity(raw.expectations.len());
        for (index, written) in raw.expectations.into_iter().enumerate() {
            let location = format!("expectations[{index}]");

            if !is_identifier(&written.id) {
                errors.refuse(
                    InfraCode::SpecMalformedId,
                    format!("{location}.id"),
                    format!(
                        "`{}` is not an expectation id: lowercase letters, digits and dashes, \
                         starting with a letter",
                        written.id
                    ),
                );
            }
            if let Some(first) = seen.insert(written.id.clone(), index) {
                errors.refuse(
                    InfraCode::SpecDuplicateExpectation,
                    format!("{location}.id"),
                    format!(
                        "`{}` is already declared at expectations[{first}], and a report names a \
                         verdict by its id",
                        written.id
                    ),
                );
            }

            let scope = scope_of(written.scope, &location, &mut errors);
            let Some(kind) = kind_of(written.expect, &location, &mut errors) else {
                continue;
            };

            let class = kind.subject_class();
            if !scope.selects(class) {
                errors.refuse(
                    InfraCode::SpecScopeNotApplicable,
                    format!("{location}.scope"),
                    format!(
                        "`{}` is about {class}, and the scope `{scope}` cannot select one",
                        kind.as_str()
                    ),
                );
            }

            expectations.push(Expectation {
                id: written.id,
                statement: written.statement,
                scope,
                kind,
            });
        }

        errors.into_result(Self::assembled(raw.name, expectations))
    }
}

/// Validates a scope's own parameters; an unusable one degrades to cluster scope so that the rest
/// of the expectation still reports its refusals in the same run.
fn scope_of(raw: RawScope, location: &str, errors: &mut ValidationErrors) -> Scope {
    match raw {
        RawScope::Cluster => Scope::Cluster,
        RawScope::Namespace(name) => {
            if name.trim().is_empty() {
                errors.refuse(
                    InfraCode::SpecInvalidExpectation,
                    format!("{location}.scope.namespace"),
                    "a namespace scope with no namespace selects the whole cluster by accident",
                );
                return Scope::Cluster;
            }
            Scope::Namespace { name }
        }
        RawScope::WorkloadSelector(labels) => {
            if labels.is_empty() {
                errors.refuse(
                    InfraCode::SpecInvalidExpectation,
                    format!("{location}.scope.workload_selector"),
                    "an empty selector matches every workload, which is cluster scope written \
                     confusingly",
                );
                return Scope::Cluster;
            }
            Scope::WorkloadSelector { labels }
        }
    }
}

/// Validates one kind's own parameters. `None` when the kind cannot decide anything at all, in
/// which case a refusal has already been recorded.
fn kind_of(
    raw: RawExpectationKind,
    location: &str,
    errors: &mut ValidationErrors,
) -> Option<ExpectationKind> {
    let at = |field: &str| format!("{location}.expect.{field}");
    match raw {
        RawExpectationKind::WorkloadExists(written) => workload_exists(written, location, errors),
        RawExpectationKind::ReplicasWithin(written) => {
            if written.min > written.max {
                errors.refuse(
                    InfraCode::SpecInvalidExpectation,
                    at("replicas_within"),
                    format!(
                        "min {} is above max {}, so no replica count can satisfy it",
                        written.min, written.max
                    ),
                );
                return None;
            }
            Some(ExpectationKind::ReplicasWithin {
                min: written.min,
                max: written.max,
            })
        }
        RawExpectationKind::ResourcesDeclared => Some(ExpectationKind::ResourcesDeclared),
        RawExpectationKind::ProbesDeclared(written) => {
            if !written.liveness && !written.readiness {
                errors.refuse(
                    InfraCode::SpecInvalidExpectation,
                    at("probes_declared"),
                    "asking for neither probe is an expectation every container satisfies, which \
                     is a green verdict about nothing",
                );
                return None;
            }
            Some(ExpectationKind::ProbesDeclared {
                liveness: written.liveness,
                readiness: written.readiness,
            })
        }
        RawExpectationKind::ImageRegistry(written) => {
            allowlist(written, "image_registry", "registry", location, errors)
                .map(|allowed| ExpectationKind::ImageRegistry { allowed })
        }
        RawExpectationKind::ImageTagNotLatest => Some(ExpectationKind::ImageTagNotLatest),
        RawExpectationKind::ImagePinnedByDigest => Some(ExpectationKind::ImagePinnedByDigest),
        RawExpectationKind::PdbCoversMultiReplica => Some(ExpectationKind::PdbCoversMultiReplica),
        RawExpectationKind::ServiceSelectorResolves => {
            Some(ExpectationKind::ServiceSelectorResolves)
        }
        RawExpectationKind::ConfigReferencesResolve => {
            Some(ExpectationKind::ConfigReferencesResolve)
        }
        RawExpectationKind::NamespaceAllowlist(written) => allowlist(
            written,
            "namespace_allowlist",
            "namespace",
            location,
            errors,
        )
        .map(|allowed| ExpectationKind::NamespaceAllowlist { allowed }),
        RawExpectationKind::WorkloadPredicate(predicate) => {
            if predicate.is_trivially_true() {
                errors.refuse(
                    InfraCode::SpecInvalidExpectation,
                    at("workload_predicate"),
                    "a predicate that holds without observing anything is not an expectation",
                );
                return None;
            }
            let mut projected = true;
            for path in predicate.fact_paths() {
                if !is_projected(path) {
                    projected = false;
                    errors.refuse(
                        InfraCode::SpecUnknownFact,
                        at("workload_predicate"),
                        format!(
                            "`{path}` is not a fact the workload projection states; the {} it \
                             does are {}",
                            WORKLOAD_FACTS.len(),
                            WORKLOAD_FACTS.join(", ")
                        ),
                    );
                }
            }
            projected.then_some(ExpectationKind::WorkloadPredicate { predicate })
        }
    }
}

/// Validates the one kind that names its own subject.
fn workload_exists(
    written: RawWorkloadExists,
    location: &str,
    errors: &mut ValidationErrors,
) -> Option<ExpectationKind> {
    let mut usable = true;
    if written.namespace.trim().is_empty() || written.name.trim().is_empty() {
        errors.refuse(
            InfraCode::SpecInvalidExpectation,
            format!("{location}.expect.workload_exists"),
            "a workload is named by a namespace and a name, and neither may be blank",
        );
        usable = false;
    }
    let Some(kind) = workload_kind(&written.workload_kind) else {
        errors.refuse(
            InfraCode::SpecInvalidExpectation,
            format!("{location}.expect.workload_exists.workload_kind"),
            format!(
                "`{}` is not a workload kind; the three are `deployment`, `statefulset` and \
                 `daemonset`",
                written.workload_kind
            ),
        );
        return None;
    };
    usable.then_some(ExpectationKind::WorkloadExists {
        namespace: written.namespace,
        kind,
        name: written.name,
    })
}

/// Validates an allowlist: present, non-empty, no blank entries.
fn allowlist(
    written: RawAllowed,
    kind: &str,
    noun: &str,
    location: &str,
    errors: &mut ValidationErrors,
) -> Option<Vec<String>> {
    if written.allowed.is_empty() {
        errors.refuse(
            InfraCode::SpecInvalidExpectation,
            format!("{location}.expect.{kind}.allowed"),
            format!("an empty allowlist permits no {noun} at all, which fails every subject"),
        );
        return None;
    }
    if written.allowed.iter().any(|entry| entry.trim().is_empty()) {
        errors.refuse(
            InfraCode::SpecInvalidExpectation,
            format!("{location}.expect.{kind}.allowed"),
            format!("a blank entry is not a {noun}"),
        );
        return None;
    }
    Some(written.allowed)
}

/// The three workload kinds, by the spelling the IR keys use.
fn workload_kind(written: &str) -> Option<WorkloadKind> {
    match written {
        "deployment" => Some(WorkloadKind::Deployment),
        "statefulset" => Some(WorkloadKind::StatefulSet),
        "daemonset" => Some(WorkloadKind::DaemonSet),
        _ => None,
    }
}

/// `true` when `value` is a stable identifier: a letter, then lowercase letters, digits, dashes.
fn is_identifier(value: &str) -> bool {
    value.starts_with(|character: char| character.is_ascii_lowercase())
        && value.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_id_is_lowercase_dashed_and_starts_with_a_letter() {
        assert!(is_identifier("shop-replicas-2"));
        assert!(!is_identifier("Shop"), "upper case is not an id");
        assert!(!is_identifier("2-shop"), "a digit cannot lead");
        assert!(
            !is_identifier("shop_replicas"),
            "an underscore is not a dash"
        );
        assert!(!is_identifier(""), "an empty id names nothing");
    }

    #[test]
    fn the_three_workload_kinds_parse_by_their_ir_spelling_and_nothing_else_does() {
        assert_eq!(workload_kind("deployment"), Some(WorkloadKind::Deployment));
        assert_eq!(
            workload_kind("statefulset"),
            Some(WorkloadKind::StatefulSet)
        );
        assert_eq!(workload_kind("daemonset"), Some(WorkloadKind::DaemonSet));
        assert_eq!(
            workload_kind("Deployment"),
            None,
            "the API's casing is not the IR's key spelling"
        );
        assert_eq!(workload_kind("replicaset"), None);
    }
}
