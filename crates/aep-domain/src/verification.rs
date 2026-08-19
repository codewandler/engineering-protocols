//! Verification: establishing that a claim holds.
//!
//! Generation and verification are deliberately separate. An agent may write the code, the
//! tests, the design and the hypothesis; it does not get to be the thing that decides whether
//! its own output is correct. Wherever a deterministic or independent system can answer the
//! question, the protocol routes the question there:
//!
//! | claim | verifier |
//! |---|---|
//! | the code builds | [`Verifier::Compiler`] |
//! | the tests pass | [`Verifier::TestRunner`] |
//! | the API is compatible | [`Verifier::ContractRunner`] |
//! | the property holds | [`Verifier::PropertyTester`] |
//! | the release is healthy | [`Verifier::TelemetryQuery`] |
//! | the design was accepted | [`Verifier::HumanReview`] |
//! | the change is permitted | [`Verifier::PolicyEngine`] |
//!
//! A failure is most useful when it hands back a [`Counterexample`]: an input the agent can
//! act on, without being able to edit the criterion it failed.

use std::fmt;

use crate::error::ParseError;
use crate::ids::{ClaimId, SubjectRef, ToolRef};
use crate::node::Node;

/// A class of system that can establish facts.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum Verifier {
    /// A compiler or type checker.
    Compiler,
    /// A test runner.
    TestRunner,
    /// A consumer/provider contract checker.
    ContractRunner,
    /// A linter or static analyser.
    StaticAnalyzer,
    /// A property-based testing engine.
    PropertyTester,
    /// A model checker.
    ModelChecker,
    /// A telemetry query against a running system.
    TelemetryQuery,
    /// A policy engine.
    PolicyEngine,
    /// A human granting approval.
    HumanApproval,
    /// A human reviewing an artifact or change.
    HumanReview,
    /// A validator for artifact structure and relationships.
    ArtifactValidator,
    /// Anything else, named by tool.
    ExternalTool(ToolRef),
}

impl Verifier {
    /// Every verifier class that is not an external tool.
    pub const NAMED: &'static [Self] = &[
        Self::Compiler,
        Self::TestRunner,
        Self::ContractRunner,
        Self::StaticAnalyzer,
        Self::PropertyTester,
        Self::ModelChecker,
        Self::TelemetryQuery,
        Self::PolicyEngine,
        Self::HumanApproval,
        Self::HumanReview,
        Self::ArtifactValidator,
    ];

    /// The verifier as written in documents.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Compiler => "compiler",
            Self::TestRunner => "test-runner",
            Self::ContractRunner => "contract-runner",
            Self::StaticAnalyzer => "static-analyzer",
            Self::PropertyTester => "property-tester",
            Self::ModelChecker => "model-checker",
            Self::TelemetryQuery => "telemetry-query",
            Self::PolicyEngine => "policy-engine",
            Self::HumanApproval => "human-approval",
            Self::HumanReview => "human-review",
            Self::ArtifactValidator => "artifact-validator",
            Self::ExternalTool(tool) => tool.as_str(),
        }
    }

    /// Parses a verifier name; anything unrecognised becomes an external tool.
    pub fn parse(value: &str) -> Result<Self, ParseError> {
        if let Some(known) = Self::NAMED
            .iter()
            .find(|verifier| verifier.as_str() == value)
        {
            return Ok(known.clone());
        }
        Ok(Self::ExternalTool(ToolRef::new(value)?))
    }

    /// `true` when this verifier is a person, and therefore cannot be scheduled by a harness.
    pub fn is_human(&self) -> bool {
        matches!(self, Self::HumanApproval | Self::HumanReview)
    }
}

impl fmt::Display for Verifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for Verifier {
    type Err = ParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

impl serde::Serialize for Verifier {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> serde::Deserialize<'de> for Verifier {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        Self::parse(&raw).map_err(serde::de::Error::custom)
    }
}

impl schemars::JsonSchema for Verifier {
    fn schema_name() -> String {
        "Verifier".to_owned()
    }

    fn json_schema(_: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        let mut schema = schemars::schema::SchemaObject {
            instance_type: Some(schemars::schema::InstanceType::String.into()),
            ..Default::default()
        };
        schema.string().pattern = Some("^[a-z][a-z0-9-]*([./][a-z0-9-]+)*$".to_owned());
        schema.metadata().description = Some(
            "A verifier class; the named classes are listed in `examples`, and any other name is \
             treated as an external tool."
                .to_owned(),
        );
        schema.metadata().examples = Verifier::NAMED
            .iter()
            .map(|verifier| serde_json::Value::String(verifier.as_str().to_owned()))
            .collect();
        schema.into()
    }
}

/// The outcome of a verification.
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
pub enum VerificationStatus {
    /// The claim holds.
    Passed,
    /// The claim does not hold.
    Failed,
    /// The verifier could not decide; this is not a pass.
    Inconclusive,
    /// The verifier did not run.
    Skipped,
}

impl VerificationStatus {
    /// The status as written in documents and facts.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Passed => "passed",
            Self::Failed => "failed",
            Self::Inconclusive => "inconclusive",
            Self::Skipped => "skipped",
        }
    }

    /// `true` only for [`VerificationStatus::Passed`].
    ///
    /// `Inconclusive` and `Skipped` are deliberately not passes: "the verifier could not tell"
    /// must never read the same as "the verifier said yes".
    pub fn is_pass(self) -> bool {
        self == Self::Passed
    }
}

impl fmt::Display for VerificationStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A concrete input that breaks a claim.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct Counterexample {
    /// Which verifier produced it.
    pub verifier: Verifier,
    /// The property or claim that failed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub property: Option<ClaimId>,
    /// The input that broke it.
    #[serde(default, skip_serializing_if = "is_null_node")]
    pub input: Node,
    /// What should have happened.
    #[serde(default, skip_serializing_if = "is_null_node")]
    pub expected: Node,
    /// What did happen.
    #[serde(default, skip_serializing_if = "is_null_node")]
    pub observed: Node,
    /// Anything else worth handing back to the agent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// Whether a node is null, for output suppression.
fn is_null_node(node: &Node) -> bool {
    node == &Node::Null
}

impl Default for Counterexample {
    fn default() -> Self {
        Self {
            verifier: Verifier::TestRunner,
            property: None,
            input: Node::Null,
            expected: Node::Null,
            observed: Node::Null,
            note: None,
        }
    }
}

/// What a verifier established.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct VerificationResult {
    /// Which verifier ran.
    pub verifier: Verifier,
    /// What it was asked about.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject: Option<SubjectRef>,
    /// The claim, when the verifier checks a named claim.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub claim: Option<ClaimId>,
    /// The outcome.
    pub status: VerificationStatus,
    /// Evidence produced along the way.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence: Vec<crate::evidence::EvidenceRecord>,
    /// Inputs that break the claim, for a failure.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub counterexamples: Vec<Counterexample>,
}

impl VerificationResult {
    /// A passing result with no counterexamples.
    pub fn passed(verifier: Verifier) -> Self {
        Self {
            verifier,
            subject: None,
            claim: None,
            status: VerificationStatus::Passed,
            evidence: Vec::new(),
            counterexamples: Vec::new(),
        }
    }

    /// A failing result.
    pub fn failed(verifier: Verifier, counterexamples: Vec<Counterexample>) -> Self {
        Self {
            verifier,
            subject: None,
            claim: None,
            status: VerificationStatus::Failed,
            evidence: Vec::new(),
            counterexamples,
        }
    }

    /// `true` when the claim was established.
    pub fn is_pass(&self) -> bool {
        self.status.is_pass()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_verifiers_become_external_tools() {
        assert_eq!(
            Verifier::parse("compiler").expect("named"),
            Verifier::Compiler
        );
        assert_eq!(
            Verifier::parse("cargo-mutants").expect("external"),
            Verifier::ExternalTool(ToolRef::new("cargo-mutants").expect("tool"))
        );
        assert!(Verifier::parse("Cargo Mutants").is_err());
    }

    #[test]
    fn inconclusive_is_not_a_pass() {
        assert!(VerificationStatus::Passed.is_pass());
        assert!(!VerificationStatus::Inconclusive.is_pass());
        assert!(!VerificationStatus::Skipped.is_pass());
        assert!(!VerificationStatus::Failed.is_pass());
    }
}
