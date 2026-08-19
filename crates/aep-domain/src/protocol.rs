//! Protocol declarations: the vocabulary a profile is allowed to use.
//!
//! A protocol document says which capabilities exist, which evidence kinds exist, which
//! verifiers are available, which facts are observable, and which ordered scales are defined.
//! Profiles, principles and workflows are then checked against that vocabulary, which is what
//! turns a typo into a validation error instead of a completion condition that can never be
//! met.
//!
//! ```yaml
//! id: aep
//! version: 1
//! title: Agentic Engineering Protocol
//! capabilities: [repository.read, repository.write, tests.execute]
//! evidence_kinds: [test_result, static_analysis, approval]
//! verifiers: [test-runner, static-analyzer, human-approval]
//! observables: ["tests.**", "static_analysis.**", "task.**"]
//! scales:
//!   risk: [low, medium, high, critical]
//! ```
//!
//! `adp/1` and `aop/1` extend `aep/1` rather than restating it: see [`Protocol::extend`].

use std::collections::BTreeSet;

use crate::artifact::ArtifactKind;
use crate::capability::Capability;
use crate::error::{ValidationCode, ValidationError, ValidationErrors};
use crate::evidence::EvidenceKind;
use crate::facts::{FactPath, FactPattern, Scales};
use crate::ids::{PhaseId, ProtocolId};
use crate::principle::FailurePolicy;
use crate::verification::Verifier;
use crate::version::{MajorVersion, ProtocolRef};

/// Protocol major versions this build implements.
///
/// An engine rejects anything else rather than interpreting it: a document written against a
/// later major version may mean something different by the same words.
pub const SUPPORTED_MAJORS: &[u32] = &[1];

/// `true` when this build implements `major`.
pub fn is_supported_major(major: MajorVersion) -> bool {
    SUPPORTED_MAJORS.contains(&major.get())
}

/// The vocabulary and defaults of one protocol version.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Protocol {
    /// Its identifier.
    pub id: ProtocolId,
    /// Its major version.
    pub version: MajorVersion,
    /// A short human title.
    pub title: String,
    /// What it covers.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    /// The protocol this one builds on.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extends: Option<ProtocolRef>,
    /// Capabilities a profile may mention.
    pub capabilities: BTreeSet<Capability>,
    /// Capabilities no profile may grant outright: they must be behind approval, or denied.
    ///
    /// This is the protocol-level floor under every profile. It is why a resolved plan cannot
    /// authorise an agent to write production without an approval gate — the check does not depend
    /// on anyone remembering to add one.
    #[serde(skip_serializing_if = "BTreeSet::is_empty")]
    pub approval_floor: BTreeSet<Capability>,
    /// Evidence kinds a requirement may mention.
    pub evidence_kinds: BTreeSet<EvidenceKind>,
    /// Verifiers available to establish evidence.
    pub verifiers: BTreeSet<Verifier>,
    /// Artifact kinds this protocol reasons about.
    #[serde(skip_serializing_if = "BTreeSet::is_empty")]
    pub artifact_kinds: BTreeSet<ArtifactKind>,
    /// Phases workflows are expected to declare.
    #[serde(skip_serializing_if = "BTreeSet::is_empty")]
    pub phases: BTreeSet<PhaseId>,
    /// Fact families a predicate may read.
    pub observables: Vec<FactPattern>,
    /// Ordered scales for non-numeric comparison.
    #[serde(skip_serializing_if = "Scales::is_empty")]
    pub scales: Scales,
    /// What happens on failure when nothing more specific says.
    pub default_failure_policy: FailurePolicy,
}

impl Protocol {
    /// A reference to this protocol at its own version.
    pub fn reference(&self) -> ProtocolRef {
        ProtocolRef::new(self.id.clone(), self.version)
    }

    /// `true` when a profile may mention `capability`.
    pub fn declares_capability(&self, capability: &Capability) -> bool {
        self.capabilities
            .iter()
            .any(|declared| declared.covers(capability))
    }

    /// `true` when a requirement may mention `kind`.
    pub fn declares_evidence(&self, kind: EvidenceKind) -> bool {
        self.evidence_kinds.contains(&kind)
    }

    /// `true` when some declared verifier can establish `kind`.
    pub fn can_establish(&self, kind: EvidenceKind) -> bool {
        kind.default_verifiers()
            .iter()
            .any(|verifier| self.verifiers.contains(verifier))
    }

    /// `true` when `capability` must never be granted outright.
    ///
    /// Overlap is enough, in either direction: a floor on `deployment.create:production` is
    /// violated by granting `deployment.create` for every environment, not only by granting
    /// production specifically.
    pub fn needs_approval_floor(&self, capability: &Capability) -> bool {
        self.approval_floor
            .iter()
            .any(|floor| floor.covers(capability) || capability.covers(floor))
    }

    /// `true` when a predicate may read `path`.
    pub fn is_observable(&self, path: &FactPath) -> bool {
        self.observables.iter().any(|pattern| pattern.matches(path))
    }

    /// Merges this protocol onto `base`, which it extends.
    ///
    /// Vocabulary is additive: a profile written against `adp/1` may use everything `aep/1`
    /// declares. Nothing is ever removed by extension, because a principle written against the
    /// base protocol has to keep working.
    #[must_use]
    pub fn extend(&self, base: &Self) -> Self {
        let mut merged = self.clone();
        merged
            .capabilities
            .extend(base.capabilities.iter().cloned());
        merged
            .evidence_kinds
            .extend(base.evidence_kinds.iter().copied());
        merged.verifiers.extend(base.verifiers.iter().cloned());
        // The floor is inherited, and inheriting it is the whole point: a derived protocol that
        // forgot to restate it would silently let a profile grant production access outright.
        merged
            .approval_floor
            .extend(base.approval_floor.iter().cloned());
        merged
            .artifact_kinds
            .extend(base.artifact_kinds.iter().cloned());
        merged.phases.extend(base.phases.iter().cloned());
        for pattern in &base.observables {
            if !merged.observables.contains(pattern) {
                merged.observables.push(pattern.clone());
            }
        }
        merged.scales.extend(&base.scales);
        merged
    }
}

/// A protocol document, as parsed.
#[derive(Debug, Clone, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RawProtocol {
    /// Its identifier.
    pub id: ProtocolId,
    /// Its major version.
    #[serde(default = "default_version")]
    pub version: MajorVersion,
    /// A short human title.
    pub title: String,
    /// What it covers.
    #[serde(default)]
    pub summary: Option<String>,
    /// The protocol this one builds on.
    #[serde(default)]
    pub extends: Option<ProtocolRef>,
    /// Capabilities a profile may mention.
    #[serde(default)]
    pub capabilities: BTreeSet<Capability>,
    /// Capabilities no profile may grant outright.
    #[serde(default, alias = "approval_required")]
    pub approval_floor: BTreeSet<Capability>,
    /// Evidence kinds a requirement may mention.
    #[serde(default)]
    pub evidence_kinds: BTreeSet<EvidenceKind>,
    /// Verifiers available to establish evidence.
    #[serde(default)]
    pub verifiers: BTreeSet<Verifier>,
    /// Artifact kinds this protocol reasons about.
    #[serde(default)]
    pub artifact_kinds: BTreeSet<ArtifactKind>,
    /// Phases workflows are expected to declare.
    #[serde(default)]
    pub phases: BTreeSet<PhaseId>,
    /// Fact families a predicate may read.
    #[serde(default)]
    pub observables: Vec<FactPattern>,
    /// Ordered scales for non-numeric comparison.
    #[serde(default)]
    pub scales: Scales,
    /// What happens on failure when nothing more specific says.
    #[serde(default, alias = "on_failure")]
    pub default_failure_policy: Option<FailurePolicy>,
}

/// Serde default for a document's major version.
fn default_version() -> MajorVersion {
    MajorVersion::V1
}

impl TryFrom<RawProtocol> for Protocol {
    type Error = ValidationErrors;

    fn try_from(raw: RawProtocol) -> Result<Self, Self::Error> {
        let mut errors = ValidationErrors::new();
        let location = format!("protocol {}/{}", raw.id, raw.version);

        if !is_supported_major(raw.version) {
            errors.push(
                ValidationError::new(
                    ValidationCode::UnsupportedProtocolVersion,
                    format!("{location}.version"),
                    format!(
                        "this build implements protocol major version(s) {SUPPORTED_MAJORS:?}, not \
                         {}",
                        raw.version
                    ),
                )
                .with_hint("upgrade the engine rather than reinterpreting the document"),
            );
        }

        if let Some(extends) = &raw.extends {
            if extends.protocol() == &raw.id {
                errors.push(ValidationError::new(
                    ValidationCode::UnknownProtocol,
                    format!("{location}.extends"),
                    format!("`{extends}` cannot extend itself"),
                ));
            }
        }

        // A base protocol carries the observables; an extension may add none of its own.
        if raw.observables.is_empty() && raw.extends.is_none() {
            errors.push(
                ValidationError::new(
                    ValidationCode::UnobservableFact,
                    format!("{location}.observables"),
                    "a protocol that declares no observables makes every predicate unreadable"
                        .to_owned(),
                )
                .with_hint("declare fact families such as `tests.**` and `task.**`"),
            );
        }

        let protocol = Self {
            id: raw.id,
            version: raw.version,
            title: raw.title,
            summary: raw.summary,
            extends: raw.extends,
            capabilities: raw.capabilities,
            approval_floor: raw.approval_floor,
            evidence_kinds: raw.evidence_kinds,
            verifiers: raw.verifiers,
            artifact_kinds: raw.artifact_kinds,
            phases: raw.phases,
            observables: raw.observables,
            scales: raw.scales,
            default_failure_policy: raw.default_failure_policy.unwrap_or_default(),
        };

        // Only a self-contained protocol can be checked here; an extension is checked once its
        // base has been merged in, during resolution.
        if protocol.extends.is_none() {
            for kind in &protocol.evidence_kinds {
                if !protocol.can_establish(*kind) {
                    errors.push(
                        ValidationError::new(
                            ValidationCode::NoVerifierForEvidence,
                            format!("{location}.evidence_kinds"),
                            format!(
                                "`{kind}` is declared but no declared verifier can establish it"
                            ),
                        )
                        .with_hint(format!(
                            "declare one of: {}",
                            kind.default_verifiers()
                                .iter()
                                .map(ToString::to_string)
                                .collect::<Vec<_>>()
                                .join(", ")
                        )),
                    );
                }
            }
        }

        errors.into_result(protocol)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn protocol(yaml: &str) -> Result<Protocol, ValidationErrors> {
        let raw: RawProtocol = serde_yaml::from_str(yaml).expect("document parses");
        Protocol::try_from(raw)
    }

    const BASE: &str = r"
id: aep
version: 1
title: Agentic Engineering Protocol
capabilities: [repository.read, repository.write, tests.execute, production.write, secret.read]
approval_floor: [production.write]
evidence_kinds: [test_result, approval]
verifiers: [test-runner, human-approval]
observables: ['tests.**', 'task.**']
scales:
  risk: [low, medium, high, critical]
";

    #[test]
    fn accepts_a_self_contained_protocol() {
        let parsed = protocol(BASE).expect("validates");
        assert_eq!(parsed.reference().to_string(), "aep/1");
        assert!(parsed.declares_capability(&Capability::TestExecution));
        assert!(!parsed.declares_capability(&Capability::TelemetryRead));
        assert!(
            parsed.needs_approval_floor(&Capability::ProductionWrite),
            "declared, but never grantable outright"
        );
        assert!(parsed.declares_evidence(EvidenceKind::TestResult));
        assert!(parsed.is_observable(&"tests.unit.failed".parse().expect("path")));
        assert!(!parsed.is_observable(&"metric.error_rate".parse().expect("path")));
    }

    #[test]
    fn rejects_evidence_no_declared_verifier_can_establish() {
        let errors = protocol(
            r"
id: aep
title: Base
evidence_kinds: [contract_result]
verifiers: [test-runner]
observables: ['tests.**']
",
        )
        .expect_err("no contract runner");
        assert!(
            errors.contains(ValidationCode::NoVerifierForEvidence),
            "{errors}"
        );
        assert!(errors.to_string().contains("contract-runner"), "{errors}");
    }

    #[test]
    fn rejects_an_unsupported_major_version() {
        let errors = protocol(
            r"
id: aep
version: 4
title: From the future
observables: ['tests.**']
",
        )
        .expect_err("unsupported version");
        assert!(
            errors.contains(ValidationCode::UnsupportedProtocolVersion),
            "{errors}"
        );
    }

    #[test]
    fn extension_adds_to_the_base_vocabulary_without_removing_anything() {
        let base = protocol(BASE).expect("validates");
        let extension = protocol(
            r"
id: aop
title: Agentic Operations Protocol
extends: aep/1
capabilities: [telemetry.read, production.read]
evidence_kinds: [metric_observation]
verifiers: [telemetry-query]
observables: ['metric.**', 'service.**']
",
        )
        .expect("validates");

        let merged = extension.extend(&base);
        assert!(
            merged.declares_capability(&Capability::TestExecution),
            "base capability kept"
        );
        assert!(
            merged.declares_capability(&Capability::TelemetryRead),
            "own capability present"
        );
        assert!(merged.can_establish(EvidenceKind::MetricObservation));
        assert!(merged.can_establish(EvidenceKind::TestResult));
        assert!(merged.is_observable(&"metric.error_rate".parse().expect("path")));
        assert!(merged.is_observable(&"tests.unit.failed".parse().expect("path")));
        assert_eq!(
            merged.scales.compare("high", "low"),
            Some(std::cmp::Ordering::Greater),
            "scales are inherited"
        );
        assert!(
            merged.needs_approval_floor(&Capability::ProductionWrite),
            "the base protocol's approval floor is inherited; a derived protocol that did not \
             restate it would silently let a profile grant production access outright"
        );
    }

    #[test]
    fn a_derived_protocol_may_add_to_the_floor_but_not_escape_it() {
        let base = protocol(BASE).expect("validates");
        let extension = protocol(
            r"
id: adp
title: Development protocol
extends: aep/1
capabilities: [secret.read]
approval_floor: [secret.read]
observables: ['build.**']
",
        )
        .expect("validates");

        let merged = extension.extend(&base);
        assert!(
            merged.needs_approval_floor(&Capability::SecretRead),
            "its own addition"
        );
        assert!(
            merged.needs_approval_floor(&Capability::ProductionWrite),
            "and the base's, which it cannot drop"
        );
    }
}
