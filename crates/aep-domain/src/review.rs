//! Reviews.
//!
//! A review is an *activity* that produces evidence, not primarily a document. The protocol
//! does not care whether it happened in GitHub, in Gerrit, in a meeting or as a signed
//! artifact; it needs a [`ReviewResult`] it can check.
//!
//! # Why the reviewed revision matters
//!
//! An approval names the version it was given against. Without that, an approval of version 3
//! of a design silently authorises version 7 — the reviewer's name ends up attached to a
//! decision they never saw. [`ReviewResult::covers`] is the check that prevents it.

use std::fmt;

use crate::artifact::{
    Artifact, ArtifactKind, ArtifactRef, ArtifactVersion, FreshnessPolicy, Revision,
};

/// What a review concluded.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ReviewDisposition {
    /// Accepted as it stands.
    Approved,
    /// Not accepted yet; specific changes are wanted.
    ChangesRequested,
    /// Not accepted.
    Rejected,
}

impl ReviewDisposition {
    /// The disposition as written in documents and facts.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Approved => "approved",
            Self::ChangesRequested => "changes_requested",
            Self::Rejected => "rejected",
        }
    }

    /// `true` only for [`ReviewDisposition::Approved`].
    pub fn is_approved(self) -> bool {
        self == Self::Approved
    }
}

impl fmt::Display for ReviewDisposition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Who reviewed.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[serde(tag = "reviewer", rename_all = "snake_case")]
pub enum Reviewer {
    /// A person.
    Human {
        /// Their identifier in whatever system recorded the review.
        id: String,
    },
    /// A group.
    Team {
        /// The team's name.
        name: String,
    },
    /// An agent.
    Agent {
        /// The agent's identifier.
        id: String,
    },
    /// An automated check standing in for a reviewer.
    Automated {
        /// What ran.
        tool: crate::ids::ToolRef,
    },
}

impl Reviewer {
    /// `true` when a person or team was accountable for this review.
    ///
    /// Some principles require a human review specifically; an agent reviewing its own
    /// team's work does not satisfy those.
    pub fn is_human(&self) -> bool {
        matches!(self, Self::Human { .. } | Self::Team { .. })
    }
}

impl fmt::Display for Reviewer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Human { id } => write!(f, "human {id}"),
            Self::Team { name } => write!(f, "team {name}"),
            Self::Agent { id } => write!(f, "agent {id}"),
            Self::Automated { tool } => write!(f, "tool {tool}"),
        }
    }
}

/// How serious a review finding is.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// Worth knowing.
    Info,
    /// Minor.
    Low,
    /// Should be addressed.
    Medium,
    /// Must be addressed.
    High,
    /// Stops everything.
    Critical,
}

impl Severity {
    /// The severity as written in documents.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Critical => "critical",
        }
    }

    /// The severity names in rank order, for a protocol's `severity` scale.
    pub fn scale() -> Vec<String> {
        [
            Self::Info,
            Self::Low,
            Self::Medium,
            Self::High,
            Self::Critical,
        ]
        .iter()
        .map(|severity| severity.as_str().to_owned())
        .collect()
    }
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One thing a review found.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct Finding {
    /// How serious it is.
    pub severity: Severity,
    /// What it is.
    pub summary: String,
    /// Where it is, such as a `file:line` or a document section.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    /// Whether it must be resolved before the work proceeds.
    #[serde(default)]
    pub blocking: bool,
}

/// The outcome of one review.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct ReviewResult {
    /// What was reviewed.
    pub subject: ArtifactRef,
    /// What kind of thing that was, when known without consulting the graph.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject_kind: Option<ArtifactKind>,
    /// Who reviewed it.
    pub reviewer: Reviewer,
    /// What they concluded.
    pub disposition: ReviewDisposition,
    /// What they found.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub findings: Vec<Finding>,
    /// Which version of the artifact they saw.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reviewed_version: Option<ArtifactVersion>,
    /// Which source revision the subject was at.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reviewed_revision: Option<Revision>,
}

impl ReviewResult {
    /// Findings that must be resolved before proceeding.
    pub fn blocking_findings(&self) -> impl Iterator<Item = &Finding> {
        self.findings.iter().filter(|finding| finding.blocking)
    }

    /// `true` when the review approved and left nothing blocking.
    pub fn is_clean_approval(&self) -> bool {
        self.disposition.is_approved() && self.blocking_findings().next().is_none()
    }

    /// `true` when this review still applies to `artifact`.
    ///
    /// A review with no recorded version applies until something supersedes the artifact; a
    /// review of a specific version stops applying when the artifact moves on, unless the
    /// artifact's freshness policy says otherwise.
    pub fn covers(&self, artifact: &Artifact) -> bool {
        if self.subject.id() != &artifact.id {
            return false;
        }
        if artifact.freshness == FreshnessPolicy::AlwaysValid {
            return true;
        }
        match (&self.reviewed_version, &artifact.version) {
            (Some(reviewed), Some(current)) => reviewed == current,
            (Some(_), None) | (None, Some(_)) => {
                artifact.freshness != FreshnessPolicy::BoundToRevision
            }
            (None, None) => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::artifact::{ArtifactId, ArtifactLocation, ArtifactStatus};

    fn design(version: Option<&str>, freshness: FreshnessPolicy) -> Artifact {
        let mut artifact = Artifact::new(
            ArtifactId::new("design:passkeys").expect("id"),
            ArtifactKind::Design,
            ArtifactStatus::Approved,
            ArtifactLocation::Inline,
        );
        artifact.version = version.map(ArtifactVersion::new);
        artifact.freshness = freshness;
        artifact
    }

    fn review(version: Option<&str>) -> ReviewResult {
        ReviewResult {
            subject: ArtifactRef::parse("design:passkeys").expect("reference"),
            subject_kind: Some(ArtifactKind::Design),
            reviewer: Reviewer::Human {
                id: "ada".to_owned(),
            },
            disposition: ReviewDisposition::Approved,
            findings: Vec::new(),
            reviewed_version: version.map(ArtifactVersion::new),
            reviewed_revision: None,
        }
    }

    #[test]
    fn an_approval_of_version_three_does_not_cover_version_seven() {
        assert!(review(Some("3")).covers(&design(Some("3"), FreshnessPolicy::UntilSuperseded)));
        assert!(!review(Some("3")).covers(&design(Some("7"), FreshnessPolicy::UntilSuperseded)));
        assert!(
            review(Some("3")).covers(&design(Some("7"), FreshnessPolicy::AlwaysValid)),
            "an explicitly version-independent artifact stays covered"
        );
    }

    #[test]
    fn a_revision_bound_artifact_needs_a_versioned_review() {
        assert!(!review(None).covers(&design(Some("7"), FreshnessPolicy::BoundToRevision)));
        assert!(review(None).covers(&design(Some("7"), FreshnessPolicy::UntilSuperseded)));
    }

    #[test]
    fn blocking_findings_defeat_an_approval() {
        let mut result = review(Some("1"));
        assert!(result.is_clean_approval());
        result.findings.push(Finding {
            severity: Severity::High,
            summary: "credential scope is unbounded".to_owned(),
            location: Some("docs/designs/passkeys.md#storage".to_owned()),
            blocking: true,
        });
        assert!(!result.is_clean_approval());
        assert_eq!(result.blocking_findings().count(), 1);
    }
}
