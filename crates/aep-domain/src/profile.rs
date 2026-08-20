//! Profiles: a reusable bundle of protocol, workflow, principles and completion.
//!
//! A task selects a profile instead of enumerating three dozen rules:
//!
//! ```yaml
//! id: development.standard
//! protocol: aep/1
//! workflow: adp/default
//! principles:
//!   - spec-driven
//!   - test-driven
//!   - contract-testing
//!   - static-analysis
//!   - least-privilege
//! capabilities:
//!   allow: [repository.read, repository.write, tests.execute]
//!   require_approval: [production.write]
//!   deny: [secret.read]
//! completion:
//!   all:
//!     - specification.satisfied
//!     - tests.unit.failed == 0
//!     - evidence.missing == 0
//! ```
//!
//! Profiles may extend another profile, which is how `development.fast`,
//! `development.standard` and `development.critical` stay honest about being three points on one
//! scale rather than three unrelated documents.

use std::collections::BTreeMap;

use crate::capability::CapabilityPolicy;
use crate::error::{ValidationCode, ValidationError, ValidationErrors};
use crate::facts::{FactPath, FactValue};
use crate::ids::{PrincipleId, ProfileId};
use crate::requirement::RequirementSet;
use crate::version::{MajorVersion, PrincipleRef, ProfileVersionedRef, ProtocolRef, WorkflowRef};

/// A reusable bundle of protocol, workflow, principles and completion condition.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Profile {
    /// Its identifier.
    pub id: ProfileId,
    /// Its major version.
    pub version: MajorVersion,
    /// A short human title.
    pub title: String,
    /// What it is for, and when to pick it over its neighbours.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    /// The protocol version it is written against.
    pub protocol: ProtocolRef,
    /// The profile it builds on.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extends: Option<ProfileVersionedRef>,
    /// The workflow to execute.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workflow: Option<WorkflowRef>,
    /// The principles in force.
    pub principles: Vec<PrincipleRef>,
    /// Principles inherited from `extends` that this profile drops.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub without_principles: Vec<PrincipleId>,
    /// What it grants, and what it puts behind approval.
    pub capabilities: CapabilityPolicy,
    /// What being finished means.
    pub completion: RequirementSet,
    /// Facts the profile declares.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub facts: BTreeMap<FactPath, FactValue>,
}

impl Profile {
    /// Merges this profile onto the one it extends.
    ///
    /// Principles union then drop anything in `without_principles`; capabilities are granted
    /// from the base and then restricted by this profile's denials and approval requirements;
    /// completion conditions are conjoined, so extending a profile can only ever make finishing
    /// harder.
    #[must_use]
    pub fn extend(&self, base: &Self) -> Self {
        let mut merged = self.clone();

        let mut principles = base.principles.clone();
        for reference in &self.principles {
            if !principles
                .iter()
                .any(|existing| existing.id() == reference.id())
            {
                principles.push(reference.clone());
            }
        }
        principles.retain(|reference| !self.without_principles.contains(reference.id()));
        merged.principles = principles;

        let mut capabilities = base.capabilities.clone();
        capabilities.grant(&self.capabilities);
        capabilities
            .deny
            .extend(self.capabilities.deny.iter().cloned());
        merged.capabilities = capabilities;

        let mut completion = base.completion.clone();
        completion.extend(self.completion.clone());
        merged.completion = completion;

        let mut facts = base.facts.clone();
        facts.extend(self.facts.clone());
        merged.facts = facts;

        if merged.workflow.is_none() {
            merged.workflow.clone_from(&base.workflow);
        }

        merged
    }
}

/// A profile document, as parsed.
#[derive(Debug, Clone, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RawProfile {
    /// Its identifier.
    pub id: ProfileId,
    /// Its major version.
    #[serde(default = "default_version")]
    pub version: MajorVersion,
    /// A short human title.
    pub title: String,
    /// What it is for.
    #[serde(default)]
    pub summary: Option<String>,
    /// The protocol version it is written against.
    pub protocol: ProtocolRef,
    /// The profile it builds on.
    #[serde(default)]
    pub extends: Option<ProfileVersionedRef>,
    /// The workflow to execute.
    #[serde(default)]
    pub workflow: Option<WorkflowRef>,
    /// The principles in force.
    #[serde(default)]
    pub principles: Vec<PrincipleRef>,
    /// Inherited principles this profile drops.
    #[serde(default, alias = "remove_principles")]
    pub without_principles: Vec<PrincipleId>,
    /// What it grants, and what it puts behind approval.
    #[serde(default)]
    pub capabilities: CapabilityPolicy,
    /// What being finished means.
    #[serde(default)]
    pub completion: RequirementSet,
    /// Facts the profile declares.
    #[serde(default)]
    pub facts: BTreeMap<FactPath, FactValue>,
}

/// Serde default for a document's major version.
fn default_version() -> MajorVersion {
    MajorVersion::V1
}

impl TryFrom<RawProfile> for Profile {
    type Error = ValidationErrors;

    fn try_from(raw: RawProfile) -> Result<Self, Self::Error> {
        let mut errors = ValidationErrors::new();
        let location = format!("profile {}", raw.id);

        if raw.workflow.is_none() && raw.extends.is_none() {
            errors.push(
                ValidationError::new(
                    ValidationCode::EmptyDeclaration,
                    format!("{location}.workflow"),
                    "a profile must name a workflow, or extend a profile that does".to_owned(),
                )
                .with_hint("add `workflow: adp/default`"),
            );
        }

        if raw.completion.is_empty() && raw.extends.is_none() {
            errors.push(
                ValidationError::new(
                    ValidationCode::EmptyDeclaration,
                    format!("{location}.completion"),
                    "a profile must say what being finished means, or extend a profile that does"
                        .to_owned(),
                )
                .with_hint(
                    "without a completion condition, nothing distinguishes finished work from \
                     abandoned work",
                ),
            );
        }

        let mut seen: Vec<&PrincipleId> = Vec::new();
        for reference in &raw.principles {
            if seen.contains(&reference.id()) {
                errors.push(ValidationError::new(
                    ValidationCode::DuplicatePrinciple,
                    format!("{location}.principles"),
                    format!("`{}` is listed twice", reference.id()),
                ));
            } else {
                seen.push(reference.id());
            }
        }

        for dropped in &raw.without_principles {
            if seen.contains(&dropped) {
                errors.push(ValidationError::new(
                    ValidationCode::ConflictingDeclaration,
                    format!("{location}.without_principles"),
                    format!("`{dropped}` is both listed and dropped"),
                ));
            }
        }

        let profile = Self {
            id: raw.id,
            version: raw.version,
            title: raw.title,
            summary: raw.summary,
            protocol: raw.protocol,
            extends: raw.extends,
            workflow: raw.workflow,
            principles: raw.principles,
            without_principles: raw.without_principles,
            capabilities: raw.capabilities,
            completion: raw.completion,
            facts: raw.facts,
        };
        errors.into_result(profile)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capability::Capability;

    fn profile(yaml: &str) -> Result<Profile, ValidationErrors> {
        let raw: RawProfile = serde_yaml::from_str(yaml).expect("document parses");
        Profile::try_from(raw)
    }

    const STANDARD: &str = r"
id: development.standard
title: Standard development
protocol: aep/1
workflow: adp/default
principles: [spec-driven, test-driven, static-analysis]
capabilities:
  allow: [repository.read, repository.write, tests.execute]
  require_approval: [production.write]
completion:
  all:
    - specification.satisfied
    - tests.unit.failed == 0
";

    #[test]
    fn accepts_a_complete_profile() {
        let parsed = profile(STANDARD).expect("validates");
        assert_eq!(parsed.principles.len(), 3);
        assert_eq!(
            parsed.workflow.map(|reference| reference.to_string()),
            Some("adp/default".to_owned())
        );
        assert_eq!(parsed.completion.predicates.len(), 1);
    }

    #[test]
    fn requires_a_workflow_and_a_completion_condition() {
        let errors = profile(
            r"
id: development.vague
title: Vague
protocol: aep/1
principles: [test-driven]
",
        )
        .expect_err("incomplete");
        assert!(
            errors.contains(ValidationCode::EmptyDeclaration),
            "{errors}"
        );
        let locations: Vec<&str> = errors
            .as_slice()
            .iter()
            .map(|error| error.location.as_str())
            .collect();
        assert!(
            locations.contains(&"profile development.vague.workflow"),
            "the missing workflow was not reported: {errors}"
        );
        assert!(
            locations.contains(&"profile development.vague.completion"),
            "the missing completion condition was not reported: {errors}"
        );
    }

    #[test]
    fn extending_adds_principles_and_can_only_tighten_completion() {
        let base = profile(STANDARD).expect("validates");
        let critical = profile(
            r"
id: development.critical
title: Critical development
protocol: aep/1
extends: development.standard
principles: [mutation-testing, approval-gates]
capabilities:
  require_approval: [repository.write]
completion:
  - property_test.session_isolation.passed
",
        )
        .expect("validates");

        let merged = critical.extend(&base);
        assert_eq!(
            merged
                .principles
                .iter()
                .map(|reference| reference.id().to_string())
                .collect::<Vec<_>>(),
            vec![
                "spec-driven",
                "test-driven",
                "static-analysis",
                "mutation-testing",
                "approval-gates"
            ]
        );
        assert_eq!(
            merged.completion.predicates.len(),
            2,
            "the base condition is kept and the extension's is added"
        );
        assert_eq!(
            merged.capabilities.decide(&Capability::RepositoryWrite),
            crate::capability::CapabilityDecision::RequiresApproval,
            "an extension can put an inherited grant behind approval"
        );
        assert_eq!(
            merged.workflow.map(|reference| reference.to_string()),
            Some("adp/default".to_owned()),
            "the workflow is inherited"
        );
    }

    #[test]
    fn a_profile_can_drop_an_inherited_principle() {
        let base = profile(STANDARD).expect("validates");
        let fast = profile(
            r"
id: development.fast
title: Fast development
protocol: aep/1
extends: development.standard
without_principles: [spec-driven]
",
        )
        .expect("validates");

        let merged = fast.extend(&base);
        assert!(
            !merged
                .principles
                .iter()
                .any(|reference| reference.id().as_str() == "spec-driven"),
            "{:?}",
            merged.principles
        );
        assert_eq!(merged.principles.len(), 2);
    }

    #[test]
    fn rejects_a_principle_that_is_both_listed_and_dropped() {
        let errors = profile(
            r"
id: development.confused
title: Confused
protocol: aep/1
workflow: adp/default
principles: [test-driven]
without_principles: [test-driven]
completion:
  - tests.unit.failed == 0
",
        )
        .expect_err("contradiction");
        assert!(
            errors.contains(ValidationCode::ConflictingDeclaration),
            "{errors}"
        );
    }
}
