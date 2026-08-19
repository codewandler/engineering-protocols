//! Evidence: observable facts produced during execution.
//!
//! A workflow does not advance because an agent says a step is done. It advances because
//! evidence satisfies a predicate. Evidence therefore carries *provenance* — who produced it,
//! from what, with which command — and the engine evaluates it without manufacturing it.
//!
//! # Fact projection
//!
//! Each piece of evidence projects into [facts](crate::facts). A unit test run projects:
//!
//! ```text
//! tests.unit.passed = 12
//! tests.unit.failed = 0
//! tests.unit.result = passed
//! tests.unit.exists = true
//! unit_tests.failed = 0        # alias, so `unit_tests.failed == 0` also works
//! test.result = passed         # the most recent test run, whichever suite
//! ```
//!
//! Aliases exist because both spellings read naturally in a principle document and neither is
//! worth forcing on an author. The canonical spelling is the `tests.<suite>.*` one; aliases
//! are listed on each payload type.

use std::fmt;

use crate::artifact::{ArtifactKind, ArtifactRef, Revision};
use crate::capability::Environment;
use crate::error::ParseError;
use crate::facts::{FactPath, FactValue, Number};
use crate::ids::{ApprovalId, ClaimId, EvidenceId, ServiceId, SubjectRef, ToolRef};
use crate::review::ReviewResult;
use crate::time::Timestamp;
use crate::verification::{Counterexample, VerificationStatus, Verifier};

/// Which body of tests was run.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum TestSuite {
    /// Unit tests.
    Unit,
    /// Integration tests.
    Integration,
    /// Consumer/provider contract tests.
    Contract,
    /// Property-based tests.
    Property,
    /// The existing suite, run to detect regressions.
    Regression,
    /// Mutation testing.
    Mutation,
    /// Fuzzing.
    Fuzz,
    /// Differential testing against a reference implementation.
    Differential,
    /// End-to-end tests.
    EndToEnd,
    /// Smoke tests.
    Smoke,
    /// A suite this vocabulary does not name.
    Named(String),
}

impl TestSuite {
    /// Every named suite.
    pub const NAMED: &'static [Self] = &[
        Self::Unit,
        Self::Integration,
        Self::Contract,
        Self::Property,
        Self::Regression,
        Self::Mutation,
        Self::Fuzz,
        Self::Differential,
        Self::EndToEnd,
        Self::Smoke,
    ];

    /// The suite as written in documents and fact paths.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Unit => "unit",
            Self::Integration => "integration",
            Self::Contract => "contract",
            Self::Property => "property",
            Self::Regression => "regression",
            Self::Mutation => "mutation",
            Self::Fuzz => "fuzz",
            Self::Differential => "differential",
            Self::EndToEnd => "e2e",
            Self::Smoke => "smoke",
            Self::Named(name) => name,
        }
    }

    /// Parses a suite name; anything kebab-case is accepted.
    pub fn parse(value: &str) -> Result<Self, ParseError> {
        if let Some(known) = Self::NAMED.iter().find(|suite| suite.as_str() == value) {
            return Ok(known.clone());
        }
        Ok(match value {
            "end-to-end" | "end_to_end" => Self::EndToEnd,
            other => {
                let named = crate::ids::PrincipleId::new(other).map_err(|_| {
                    ParseError::identifier(
                        "test suite",
                        other,
                        "test suite names are lower-case kebab-case".to_owned(),
                    )
                })?;
                Self::Named(named.as_str().to_owned())
            }
        })
    }
}

impl fmt::Display for TestSuite {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for TestSuite {
    type Err = ParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

impl serde::Serialize for TestSuite {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> serde::Deserialize<'de> for TestSuite {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        Self::parse(&raw).map_err(serde::de::Error::custom)
    }
}

impl schemars::JsonSchema for TestSuite {
    fn schema_name() -> String {
        "TestSuite".to_owned()
    }

    fn json_schema(_: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        let mut schema = schemars::schema::SchemaObject {
            instance_type: Some(schemars::schema::InstanceType::String.into()),
            ..Default::default()
        };
        schema.string().pattern = Some("^[a-z][a-z0-9-]*$".to_owned());
        schema.metadata().description = Some("Which body of tests was run.".to_owned());
        schema.metadata().examples = TestSuite::NAMED
            .iter()
            .map(|suite| serde_json::Value::String(suite.as_str().to_owned()))
            .collect();
        schema.into()
    }
}

/// The outcome of running a test suite.
///
/// Facts: `tests.<suite>.{passed,failed,skipped,total,result,exists}`, with aliases
/// `<suite>_tests.*`, `test.{result,exists}` and, for the regression suite,
/// `regression_suite.result`.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct TestResult {
    /// Which suite ran.
    pub suite: TestSuite,
    /// How many tests passed.
    #[serde(default)]
    pub passed: usize,
    /// How many failed.
    #[serde(default)]
    pub failed: usize,
    /// How many were skipped.
    #[serde(default)]
    pub skipped: usize,
    /// How long the run took.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    /// The selector the run was narrowed by, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selector: Option<String>,
}

impl TestResult {
    /// A passing run of `passed` tests.
    pub fn passing(suite: TestSuite, passed: usize) -> Self {
        Self {
            suite,
            passed,
            failed: 0,
            skipped: 0,
            duration_ms: None,
            selector: None,
        }
    }

    /// A failing run.
    pub fn failing(suite: TestSuite, passed: usize, failed: usize) -> Self {
        Self {
            suite,
            passed,
            failed,
            skipped: 0,
            duration_ms: None,
            selector: None,
        }
    }

    /// Total tests observed.
    pub fn total(&self) -> usize {
        self.passed + self.failed + self.skipped
    }

    /// `passed` when nothing failed, `failed` otherwise.
    ///
    /// A run with no tests at all is *not* a pass: an empty suite must not read as a green one.
    pub fn status(&self) -> VerificationStatus {
        if self.total() == 0 {
            VerificationStatus::Inconclusive
        } else if self.failed == 0 {
            VerificationStatus::Passed
        } else {
            VerificationStatus::Failed
        }
    }
}

/// The outcome of static analysis.
///
/// Facts: `static_analysis.{errors,warnings,result,exists}`.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct StaticAnalysisResult {
    /// Which analyser ran.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool: Option<ToolRef>,
    /// How many errors it reported.
    #[serde(default)]
    pub errors: usize,
    /// How many warnings.
    #[serde(default)]
    pub warnings: usize,
}

/// The outcome of contract verification.
///
/// Facts: `contracts.{checked,failed,breaking_changes,result}`, with aliases
/// `tests.contract.failed` and `contract_tests.failed`.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct ContractResult {
    /// How many contracts were checked.
    #[serde(default)]
    pub checked: usize,
    /// How many failed.
    #[serde(default)]
    pub failed: usize,
    /// How many changes break an existing consumer.
    #[serde(default)]
    pub breaking_changes: usize,
    /// The consumer, when the check is consumer-specific.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub consumer: Option<String>,
    /// The provider.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
}

/// The outcome of one property test.
///
/// Facts: `property_test.<property>.{result,passed,cases}`.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct PropertyTestResult {
    /// Which property was tested.
    pub property: ClaimId,
    /// How many cases were generated.
    #[serde(default)]
    pub cases: usize,
    /// The outcome.
    pub status: VerificationStatus,
    /// Inputs that break the property.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub counterexamples: Vec<Counterexample>,
}

/// How a deployment ended.
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
pub enum DeploymentStatus {
    /// Still going.
    InProgress,
    /// Completed.
    Succeeded,
    /// Did not complete.
    Failed,
    /// Was undone.
    RolledBack,
}

impl DeploymentStatus {
    /// The status as written in documents and facts.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InProgress => "in_progress",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::RolledBack => "rolled_back",
        }
    }
}

impl fmt::Display for DeploymentStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The outcome of a deployment.
///
/// Facts: `deployment.{status,succeeded,environment,revision}`,
/// `deployment.previous_revision.exists`, `deployment.<environment>.status`.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct DeploymentResult {
    /// Where it went.
    pub environment: Environment,
    /// What was deployed.
    pub revision: String,
    /// What was running before, which is what a rollback needs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_revision: Option<String>,
    /// The rollout strategy used.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strategy: Option<String>,
    /// How it ended.
    pub status: DeploymentStatus,
}

/// One measured value from telemetry.
///
/// Facts: `metric.<metric>` and the bare `<metric>` alias, so both
/// `metric.error_rate < 0.01` and `error_rate < 0.01` work.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct MetricObservation {
    /// The metric's name, as a dotted path.
    pub metric: FactPath,
    /// Its value.
    pub value: Number,
    /// Its unit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    /// The window it was measured over, such as `5m`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window: Option<String>,
    /// The service it describes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service: Option<ServiceId>,
}

/// How healthy a service is.
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
pub enum HealthStatus {
    /// Serving normally.
    Healthy,
    /// Serving, with impairment.
    Degraded,
    /// Not serving.
    Unhealthy,
}

impl HealthStatus {
    /// The status as written in documents and facts.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Healthy => "healthy",
            Self::Degraded => "degraded",
            Self::Unhealthy => "unhealthy",
        }
    }

    /// The health names in rank order, for a protocol's `health` scale.
    pub fn scale() -> Vec<String> {
        [Self::Unhealthy, Self::Degraded, Self::Healthy]
            .iter()
            .map(|status| status.as_str().to_owned())
            .collect()
    }
}

impl fmt::Display for HealthStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// An observation of a service's health.
///
/// Facts: `service.health`, and `service.<service>.health` when a service is named.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct HealthObservation {
    /// Which service.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service: Option<ServiceId>,
    /// How healthy it is.
    pub status: HealthStatus,
    /// What was checked.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// Whether an approval was given.
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
pub enum ApprovalDecision {
    /// Approved.
    Granted,
    /// Refused.
    Denied,
}

/// A recorded human approval.
///
/// Facts: `approval.<approval>.{granted,decision}`.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct ApprovalRecord {
    /// Which approval this is.
    pub approval: ApprovalId,
    /// Who gave or refused it.
    pub approver: Producer,
    /// What they decided.
    pub decision: ApprovalDecision,
    /// What the approval is about.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject: Option<SubjectRef>,
    /// Anything they said.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// A set of changes to source.
///
/// Facts: `diff.{exists,files_changed,lines_added,lines_removed}`, alias `source_diff.exists`.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct ChangeSet {
    /// How many files changed.
    #[serde(default)]
    pub files_changed: usize,
    /// How many lines were added.
    #[serde(default)]
    pub lines_added: usize,
    /// How many lines were removed.
    #[serde(default)]
    pub lines_removed: usize,
    /// The revision before the change.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision_before: Option<Revision>,
    /// The revision after it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision_after: Option<Revision>,
    /// Which paths changed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub paths: Vec<String>,
}

/// What was observed about an artifact.
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
pub enum ArtifactObservation {
    /// It is there.
    Exists,
    /// It conforms to its schema.
    SchemaValid,
    /// It has the sections its kind requires.
    SectionsPresent,
    /// It has been approved.
    Approved,
    /// It has been reviewed, whatever the outcome.
    Reviewed,
    /// It still applies to the current revision.
    Current,
    /// Its relationships resolve and are of legal kinds.
    RelationshipValid,
}

impl ArtifactObservation {
    /// The observation as written in documents and facts.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Exists => "exists",
            Self::SchemaValid => "schema_valid",
            Self::SectionsPresent => "sections_present",
            Self::Approved => "approved",
            Self::Reviewed => "reviewed",
            Self::Current => "current",
            Self::RelationshipValid => "relationship_valid",
        }
    }
}

impl fmt::Display for ArtifactObservation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// An observation about one artifact.
///
/// Facts: `artifact.<kind>.<observation>`, for example `artifact.design.schema_valid`.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct ArtifactEvidence {
    /// Which artifact.
    pub artifact: ArtifactRef,
    /// What kind it is.
    pub kind: ArtifactKind,
    /// What was observed.
    pub observation: ArtifactObservation,
    /// Whether the observation holds.
    #[serde(default = "default_true")]
    pub holds: bool,
    /// What was checked, or why it does not hold.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// Serde default for a boolean that defaults to `true`.
fn default_true() -> bool {
    true
}

/// A verifier's statement about a named claim.
///
/// Facts: `verification.<claim>.{status,passed}` and the `<claim>_verified` alias, which is
/// what makes `recovery_verified == true` work.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct VerificationRecord {
    /// What was claimed.
    pub claim: ClaimId,
    /// Who established it.
    pub verifier: Verifier,
    /// The outcome.
    pub status: VerificationStatus,
    /// What the claim is about.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject: Option<SubjectRef>,
    /// Inputs that break the claim.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub counterexamples: Vec<Counterexample>,
}

/// How much of a specification the implementation satisfies.
///
/// Facts: `specification.satisfied`, `specification.requirements.{total,satisfied}`,
/// `specification.unsatisfied.count`.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct SpecificationRecord {
    /// Which specification.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact: Option<ArtifactRef>,
    /// Whether every requirement is satisfied.
    pub satisfied: bool,
    /// How many requirements it has.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requirements_total: Option<usize>,
    /// How many are satisfied.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requirements_satisfied: Option<usize>,
    /// Which are not.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unsatisfied: Vec<String>,
}

/// What produced a piece of evidence.
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
#[serde(tag = "producer", rename_all = "snake_case")]
pub enum Producer {
    /// An agent.
    Agent {
        /// Its identifier, such as a model or session name.
        id: String,
    },
    /// A person.
    Human {
        /// Their identifier.
        id: String,
    },
    /// A tool.
    Tool {
        /// Which tool.
        tool: ToolRef,
    },
    /// The harness itself.
    Harness {
        /// Its identifier.
        id: String,
    },
    /// A verifier.
    Verifier {
        /// Which class of verifier.
        verifier: Verifier,
    },
}

impl Producer {
    /// `true` when a person produced this.
    pub fn is_human(&self) -> bool {
        matches!(self, Self::Human { .. })
    }

    /// `true` when an agent produced this.
    ///
    /// Agent-produced evidence is not thereby untrustworthy; what it means is that a principle
    /// requiring independent verification is not satisfied by it alone.
    pub fn is_agent(&self) -> bool {
        matches!(self, Self::Agent { .. })
    }
}

impl fmt::Display for Producer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Agent { id } => write!(f, "agent {id}"),
            Self::Human { id } => write!(f, "human {id}"),
            Self::Tool { tool } => write!(f, "tool {tool}"),
            Self::Harness { id } => write!(f, "harness {id}"),
            Self::Verifier { verifier } => write!(f, "verifier {verifier}"),
        }
    }
}

/// How a piece of evidence was obtained.
#[derive(
    Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct Provenance {
    /// The command that produced it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    /// The tool that produced it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool: Option<ToolRef>,
    /// The source revision it describes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision: Option<Revision>,
    /// Where it was produced.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace: Option<String>,
    /// The environment it describes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub environment: Option<Environment>,
    /// A digest of the raw output, for tamper detection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub digest: Option<String>,
    /// Inputs the observation depended on.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub inputs: Vec<String>,
}

/// A kind of evidence.
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
#[non_exhaustive]
pub enum EvidenceKind {
    /// A test run.
    #[serde(alias = "test_execution")]
    TestResult,
    /// Static analysis output.
    StaticAnalysis,
    /// Contract verification output.
    ContractResult,
    /// A property test.
    PropertyTestResult,
    /// A deployment.
    DeploymentResult,
    /// A measured value.
    MetricObservation,
    /// A health check.
    HealthObservation,
    /// A human approval.
    Approval,
    /// A set of source changes.
    #[serde(alias = "source_diff")]
    Diff,
    /// An observation about an artifact.
    #[serde(alias = "artifact_evidence")]
    Artifact,
    /// A review outcome.
    #[serde(alias = "review_result")]
    Review,
    /// A verifier's statement about a claim.
    #[serde(alias = "verification_record")]
    Verification,
    /// Specification satisfaction.
    #[serde(alias = "specification_record")]
    Specification,
}

impl EvidenceKind {
    /// Every evidence kind.
    pub const ALL: &'static [Self] = &[
        Self::TestResult,
        Self::StaticAnalysis,
        Self::ContractResult,
        Self::PropertyTestResult,
        Self::DeploymentResult,
        Self::MetricObservation,
        Self::HealthObservation,
        Self::Approval,
        Self::Diff,
        Self::Artifact,
        Self::Review,
        Self::Verification,
        Self::Specification,
    ];

    /// The kind as written in documents and fact paths.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::TestResult => "test_result",
            Self::StaticAnalysis => "static_analysis",
            Self::ContractResult => "contract_result",
            Self::PropertyTestResult => "property_test_result",
            Self::DeploymentResult => "deployment_result",
            Self::MetricObservation => "metric_observation",
            Self::HealthObservation => "health_observation",
            Self::Approval => "approval",
            Self::Diff => "diff",
            Self::Artifact => "artifact",
            Self::Review => "review",
            Self::Verification => "verification",
            Self::Specification => "specification",
        }
    }

    /// Parses a kind name, accepting the documented aliases.
    pub fn parse(value: &str) -> Result<Self, ParseError> {
        if let Some(known) = Self::ALL.iter().find(|kind| kind.as_str() == value) {
            return Ok(*known);
        }
        match value {
            "test_execution" => Ok(Self::TestResult),
            "source_diff" => Ok(Self::Diff),
            "artifact_evidence" => Ok(Self::Artifact),
            "review_result" => Ok(Self::Review),
            "verification_record" => Ok(Self::Verification),
            "specification_record" => Ok(Self::Specification),
            other => Err(ParseError::identifier(
                "evidence kind",
                other,
                format!(
                    "expected one of {}",
                    Self::ALL
                        .iter()
                        .map(|kind| kind.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            )),
        }
    }

    /// Verifier classes that can establish this kind of evidence.
    ///
    /// Used to check that required evidence is actually obtainable: a protocol that requires
    /// contract results but declares no contract runner is misconfigured, and saying so at
    /// validation time is better than discovering it when a workflow will not advance.
    pub fn default_verifiers(self) -> &'static [Verifier] {
        match self {
            Self::TestResult => &[Verifier::TestRunner],
            Self::StaticAnalysis => &[Verifier::StaticAnalyzer, Verifier::Compiler],
            Self::ContractResult => &[Verifier::ContractRunner],
            Self::PropertyTestResult => &[Verifier::PropertyTester, Verifier::ModelChecker],
            Self::DeploymentResult => &[Verifier::PolicyEngine, Verifier::TelemetryQuery],
            Self::MetricObservation | Self::HealthObservation => &[Verifier::TelemetryQuery],
            Self::Approval => &[Verifier::HumanApproval],
            Self::Diff => &[Verifier::Compiler, Verifier::StaticAnalyzer],
            Self::Artifact => &[Verifier::ArtifactValidator],
            Self::Review => &[Verifier::HumanReview],
            Self::Verification => &[Verifier::PolicyEngine, Verifier::ModelChecker],
            Self::Specification => &[Verifier::TestRunner, Verifier::HumanReview],
        }
    }
}

impl fmt::Display for EvidenceKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for EvidenceKind {
    type Err = ParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

/// An observable fact produced during execution.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[non_exhaustive]
pub enum Evidence {
    /// A test run.
    #[serde(alias = "test_execution")]
    TestResult(TestResult),
    /// Static analysis output.
    StaticAnalysis(StaticAnalysisResult),
    /// Contract verification output.
    ContractResult(ContractResult),
    /// A property test.
    PropertyTestResult(PropertyTestResult),
    /// A deployment.
    DeploymentResult(DeploymentResult),
    /// A measured value.
    MetricObservation(MetricObservation),
    /// A health check.
    HealthObservation(HealthObservation),
    /// A human approval.
    Approval(ApprovalRecord),
    /// A set of source changes.
    #[serde(alias = "source_diff")]
    Diff(ChangeSet),
    /// An observation about an artifact.
    Artifact(ArtifactEvidence),
    /// A review outcome.
    Review(ReviewResult),
    /// A verifier's statement about a claim.
    Verification(VerificationRecord),
    /// Specification satisfaction.
    Specification(SpecificationRecord),
}

impl Evidence {
    /// Which kind of evidence this is.
    pub fn kind(&self) -> EvidenceKind {
        match self {
            Self::TestResult(_) => EvidenceKind::TestResult,
            Self::StaticAnalysis(_) => EvidenceKind::StaticAnalysis,
            Self::ContractResult(_) => EvidenceKind::ContractResult,
            Self::PropertyTestResult(_) => EvidenceKind::PropertyTestResult,
            Self::DeploymentResult(_) => EvidenceKind::DeploymentResult,
            Self::MetricObservation(_) => EvidenceKind::MetricObservation,
            Self::HealthObservation(_) => EvidenceKind::HealthObservation,
            Self::Approval(_) => EvidenceKind::Approval,
            Self::Diff(_) => EvidenceKind::Diff,
            Self::Artifact(_) => EvidenceKind::Artifact,
            Self::Review(_) => EvidenceKind::Review,
            Self::Verification(_) => EvidenceKind::Verification,
            Self::Specification(_) => EvidenceKind::Specification,
        }
    }

    /// A one-line description, for audit records.
    pub fn summary(&self) -> String {
        match self {
            Self::TestResult(result) => format!(
                "{} tests: {} passed, {} failed",
                result.suite, result.passed, result.failed
            ),
            Self::StaticAnalysis(result) => {
                format!(
                    "static analysis: {} errors, {} warnings",
                    result.errors, result.warnings
                )
            }
            Self::ContractResult(result) => format!(
                "contracts: {} checked, {} failed, {} breaking",
                result.checked, result.failed, result.breaking_changes
            ),
            Self::PropertyTestResult(result) => {
                format!("property {}: {}", result.property, result.status)
            }
            Self::DeploymentResult(result) => {
                format!("deployment to {}: {}", result.environment, result.status)
            }
            Self::MetricObservation(observation) => {
                format!("{} = {}", observation.metric, observation.value)
            }
            Self::HealthObservation(observation) => match &observation.service {
                Some(service) => format!("{service} is {}", observation.status),
                None => format!("service is {}", observation.status),
            },
            Self::Approval(record) => format!(
                "approval {}: {:?} by {}",
                record.approval, record.decision, record.approver
            ),
            Self::Diff(change) => format!(
                "diff: {} files, +{} -{}",
                change.files_changed, change.lines_added, change.lines_removed
            ),
            Self::Artifact(evidence) => format!(
                "{} {} is {}{}",
                evidence.kind,
                evidence.artifact,
                if evidence.holds { "" } else { "not " },
                evidence.observation
            ),
            Self::Review(result) => format!(
                "review of {} by {}: {}",
                result.subject, result.reviewer, result.disposition
            ),
            Self::Verification(record) => {
                format!(
                    "{} verified by {}: {}",
                    record.claim, record.verifier, record.status
                )
            }
            Self::Specification(record) => format!(
                "specification satisfied: {}{}",
                record.satisfied,
                match (record.requirements_satisfied, record.requirements_total) {
                    (Some(satisfied), Some(total)) => format!(" ({satisfied}/{total})"),
                    _ => String::new(),
                }
            ),
        }
    }

    /// The facts this evidence establishes.
    ///
    /// Canonical paths come first, aliases after; a later binding for the same path wins, so
    /// aliases never shadow a canonical fact.
    // One arm per evidence kind: splitting it would scatter the fact vocabulary across a dozen
    // functions, which is exactly the thing this table is here to make readable.
    #[allow(clippy::too_many_lines)]
    pub fn facts(&self) -> Vec<(FactPath, FactValue)> {
        let path = |segments: &[&str]| FactPath::from_segments(segments);
        let mut facts: Vec<(FactPath, FactValue)> = Vec::new();

        match self {
            Self::TestResult(result) => {
                let suite = result.suite.as_str();
                let status = result.status();
                let base = path(&["tests", suite]);
                facts.push((base.child("passed"), FactValue::count(result.passed)));
                facts.push((base.child("failed"), FactValue::count(result.failed)));
                facts.push((base.child("skipped"), FactValue::count(result.skipped)));
                facts.push((base.child("total"), FactValue::count(result.total())));
                facts.push((base.child("result"), FactValue::text(status.as_str())));
                facts.push((base.child("exists"), FactValue::bool(true)));

                // Aliases: `unit_tests.failed`, `test.result`, `regression_suite.result`.
                let alias = path(&[&format!("{suite}_tests")]);
                facts.push((alias.child("failed"), FactValue::count(result.failed)));
                facts.push((alias.child("passed"), FactValue::count(result.passed)));
                facts.push((alias.child("result"), FactValue::text(status.as_str())));
                facts.push((path(&["test", "result"]), FactValue::text(status.as_str())));
                facts.push((path(&["test", "exists"]), FactValue::bool(true)));
                if result.suite == TestSuite::Regression {
                    facts.push((
                        path(&["regression_suite", "result"]),
                        FactValue::text(status.as_str()),
                    ));
                }
            }
            Self::StaticAnalysis(result) => {
                let base = path(&["static_analysis"]);
                facts.push((base.child("errors"), FactValue::count(result.errors)));
                facts.push((base.child("warnings"), FactValue::count(result.warnings)));
                facts.push((base.child("exists"), FactValue::bool(true)));
                facts.push((
                    base.child("result"),
                    FactValue::text(if result.errors == 0 {
                        "passed"
                    } else {
                        "failed"
                    }),
                ));
            }
            Self::ContractResult(result) => {
                let base = path(&["contracts"]);
                facts.push((base.child("checked"), FactValue::count(result.checked)));
                facts.push((base.child("failed"), FactValue::count(result.failed)));
                facts.push((
                    base.child("breaking_changes"),
                    FactValue::count(result.breaking_changes),
                ));
                let passed = result.failed == 0 && result.breaking_changes == 0;
                facts.push((
                    base.child("result"),
                    FactValue::text(if passed { "passed" } else { "failed" }),
                ));
                facts.push((base.child("exists"), FactValue::bool(true)));
                facts.push((
                    path(&["tests", "contract", "failed"]),
                    FactValue::count(result.failed),
                ));
                facts.push((
                    path(&["contract_tests", "failed"]),
                    FactValue::count(result.failed),
                ));
            }
            Self::PropertyTestResult(result) => {
                let base = path(&["property_test", result.property.as_str()]);
                facts.push((
                    base.child("result"),
                    FactValue::text(result.status.as_str()),
                ));
                facts.push((
                    base.child("passed"),
                    FactValue::bool(result.status.is_pass()),
                ));
                facts.push((base.child("cases"), FactValue::count(result.cases)));
                facts.push((base.child("exists"), FactValue::bool(true)));
            }
            Self::DeploymentResult(result) => {
                let base = path(&["deployment"]);
                facts.push((
                    base.child("status"),
                    FactValue::text(result.status.as_str()),
                ));
                facts.push((
                    base.child("succeeded"),
                    FactValue::bool(result.status == DeploymentStatus::Succeeded),
                ));
                facts.push((
                    base.child("environment"),
                    FactValue::text(result.environment.as_str()),
                ));
                facts.push((
                    base.child("revision"),
                    FactValue::text(result.revision.clone()),
                ));
                facts.push((
                    base.child("previous_revision").child("exists"),
                    FactValue::bool(result.previous_revision.is_some()),
                ));
                if let Some(previous) = &result.previous_revision {
                    facts.push((
                        base.child("previous_revision"),
                        FactValue::text(previous.clone()),
                    ));
                }
                if let Environment::Named(_) | Environment::Any = result.environment {
                } else {
                    facts.push((
                        path(&["deployment", result.environment.as_str(), "status"]),
                        FactValue::text(result.status.as_str()),
                    ));
                }
            }
            Self::MetricObservation(observation) => {
                let mut segments = vec!["metric".to_owned()];
                segments.extend(observation.metric.segments().iter().cloned());
                facts.push((
                    FactPath::from_segments(segments),
                    FactValue::Number(observation.value),
                ));
                facts.push((
                    observation.metric.clone(),
                    FactValue::Number(observation.value),
                ));
            }
            Self::HealthObservation(observation) => {
                facts.push((
                    path(&["service", "health"]),
                    FactValue::text(observation.status.as_str()),
                ));
                if let Some(service) = &observation.service {
                    let mut segments = vec!["service".to_owned()];
                    segments.extend(service.as_str().split(['.', '/']).map(ToOwned::to_owned));
                    segments.push("health".to_owned());
                    facts.push((
                        FactPath::from_segments(segments),
                        FactValue::text(observation.status.as_str()),
                    ));
                }
            }
            Self::Approval(record) => {
                let base = path(&["approval", record.approval.as_str()]);
                facts.push((
                    base.child("granted"),
                    FactValue::bool(record.decision == ApprovalDecision::Granted),
                ));
                facts.push((
                    base.child("decision"),
                    FactValue::text(match record.decision {
                        ApprovalDecision::Granted => "granted",
                        ApprovalDecision::Denied => "denied",
                    }),
                ));
                facts.push((
                    base.child("by_human"),
                    FactValue::bool(record.approver.is_human()),
                ));
            }
            Self::Diff(change) => {
                let base = path(&["diff"]);
                facts.push((base.child("exists"), FactValue::bool(true)));
                facts.push((
                    base.child("files_changed"),
                    FactValue::count(change.files_changed),
                ));
                facts.push((
                    base.child("lines_added"),
                    FactValue::count(change.lines_added),
                ));
                facts.push((
                    base.child("lines_removed"),
                    FactValue::count(change.lines_removed),
                ));
                facts.push((path(&["source_diff", "exists"]), FactValue::bool(true)));
            }
            Self::Artifact(evidence) => {
                for kind in evidence.kind.lineage() {
                    facts.push((
                        path(&["artifact", kind.as_str(), evidence.observation.as_str()]),
                        FactValue::bool(evidence.holds),
                    ));
                }
            }
            Self::Review(result) => {
                let kinds = result
                    .subject_kind
                    .as_ref()
                    .map(ArtifactKind::lineage)
                    .unwrap_or_default();
                let approved = result.is_clean_approval();
                for kind in kinds {
                    let base = path(&["review", kind.as_str()]);
                    facts.push((
                        base.child("result"),
                        FactValue::text(result.disposition.as_str()),
                    ));
                    facts.push((base.child("approved"), FactValue::bool(approved)));
                    facts.push((
                        base.child("blocking_findings"),
                        FactValue::count(result.blocking_findings().count()),
                    ));
                    facts.push((
                        base.child("by_human"),
                        FactValue::bool(result.reviewer.is_human()),
                    ));
                }
                let base = path(&["review"]);
                facts.push((
                    base.child("result"),
                    FactValue::text(result.disposition.as_str()),
                ));
                facts.push((base.child("approved"), FactValue::bool(approved)));
            }
            Self::Verification(record) => {
                let base = path(&["verification", record.claim.as_str()]);
                facts.push((
                    base.child("status"),
                    FactValue::text(record.status.as_str()),
                ));
                facts.push((
                    base.child("passed"),
                    FactValue::bool(record.status.is_pass()),
                ));
                facts.push((
                    path(&[&format!("{}_verified", record.claim.as_str())]),
                    FactValue::bool(record.status.is_pass()),
                ));
            }
            Self::Specification(record) => {
                let base = path(&["specification"]);
                facts.push((base.child("satisfied"), FactValue::bool(record.satisfied)));
                facts.push((base.child("exists"), FactValue::bool(true)));
                if let Some(total) = record.requirements_total {
                    facts.push((
                        base.child("requirements").child("total"),
                        FactValue::count(total),
                    ));
                }
                if let Some(satisfied) = record.requirements_satisfied {
                    facts.push((
                        base.child("requirements").child("satisfied"),
                        FactValue::count(satisfied),
                    ));
                }
                facts.push((
                    base.child("unsatisfied").child("count"),
                    FactValue::count(record.unsatisfied.len()),
                ));
            }
        }

        facts
    }
}

/// Evidence with its provenance.
///
/// The envelope is what makes evidence auditable: the same `tests.unit.failed = 0` means
/// something different when a test runner produced it than when an agent asserted it, and the
/// envelope is where that difference is recorded.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct EvidenceEnvelope<T> {
    /// Its identifier.
    pub id: EvidenceId,
    /// When it was produced.
    pub produced_at: Timestamp,
    /// What produced it.
    pub producer: Producer,
    /// What it is about.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject: Option<SubjectRef>,
    /// The evidence itself.
    pub value: T,
    /// How it was obtained.
    #[serde(default, skip_serializing_if = "is_default_provenance")]
    pub provenance: Provenance,
}

/// Whether provenance is empty, for output suppression.
fn is_default_provenance(provenance: &Provenance) -> bool {
    provenance == &Provenance::default()
}

impl<T> EvidenceEnvelope<T> {
    /// Wraps a value with the minimum metadata.
    pub fn new(id: EvidenceId, produced_at: Timestamp, producer: Producer, value: T) -> Self {
        Self {
            id,
            produced_at,
            producer,
            subject: None,
            value,
            provenance: Provenance::default(),
        }
    }

    /// Attaches a subject, builder-style.
    #[must_use]
    pub fn with_subject(mut self, subject: SubjectRef) -> Self {
        self.subject = Some(subject);
        self
    }

    /// Attaches provenance, builder-style.
    #[must_use]
    pub fn with_provenance(mut self, provenance: Provenance) -> Self {
        self.provenance = provenance;
        self
    }
}

/// A submitted piece of evidence.
pub type EvidenceRecord = EvidenceEnvelope<Evidence>;

impl EvidenceRecord {
    /// Which kind of evidence this record carries.
    pub fn kind(&self) -> EvidenceKind {
        self.value.kind()
    }

    /// The facts this record establishes.
    pub fn facts(&self) -> Vec<(FactPath, FactValue)> {
        self.value.facts()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::facts::{FactSource, FactStore};

    fn store(evidence: &Evidence) -> FactStore {
        let mut store = FactStore::new();
        store.extend_facts(evidence.facts());
        store
    }

    fn value(store: &FactStore, path: &str) -> Option<FactValue> {
        store.fact(&FactPath::new(path).expect("path"))
    }

    #[test]
    fn a_test_run_projects_canonical_facts_and_documented_aliases() {
        let evidence = Evidence::TestResult(TestResult::passing(TestSuite::Unit, 12));
        let facts = store(&evidence);

        assert_eq!(
            value(&facts, "tests.unit.passed"),
            Some(FactValue::count(12))
        );
        assert_eq!(
            value(&facts, "tests.unit.failed"),
            Some(FactValue::count(0))
        );
        assert_eq!(
            value(&facts, "tests.unit.result"),
            Some(FactValue::text("passed"))
        );
        assert_eq!(
            value(&facts, "unit_tests.failed"),
            Some(FactValue::count(0))
        );
        assert_eq!(
            value(&facts, "test.result"),
            Some(FactValue::text("passed"))
        );
    }

    #[test]
    fn an_empty_test_run_is_inconclusive_not_green() {
        let empty = TestResult::passing(TestSuite::Unit, 0);
        assert_eq!(empty.status(), VerificationStatus::Inconclusive);
        let facts = store(&Evidence::TestResult(empty));
        assert_eq!(
            value(&facts, "tests.unit.result"),
            Some(FactValue::text("inconclusive"))
        );
    }

    #[test]
    fn metrics_project_both_the_namespaced_and_bare_path() {
        let evidence = Evidence::MetricObservation(MetricObservation {
            metric: "error_rate".parse().expect("path"),
            value: Number::new(0.004).expect("finite"),
            unit: Some("ratio".to_owned()),
            window: Some("5m".to_owned()),
            service: None,
        });
        let facts = store(&evidence);
        let expected = FactValue::number(0.004).expect("finite");
        assert_eq!(value(&facts, "metric.error_rate"), Some(expected.clone()));
        assert_eq!(value(&facts, "error_rate"), Some(expected));
    }

    #[test]
    fn a_verification_record_projects_the_claim_verified_alias() {
        let evidence = Evidence::Verification(VerificationRecord {
            claim: "recovery".parse().expect("claim"),
            verifier: Verifier::TelemetryQuery,
            status: VerificationStatus::Passed,
            subject: None,
            counterexamples: Vec::new(),
        });
        let facts = store(&evidence);
        assert_eq!(
            value(&facts, "recovery_verified"),
            Some(FactValue::bool(true))
        );
        assert_eq!(
            value(&facts, "verification.recovery.status"),
            Some(FactValue::text("passed"))
        );
    }

    #[test]
    fn a_deployment_records_whether_a_rollback_target_exists() {
        let evidence = Evidence::DeploymentResult(DeploymentResult {
            environment: Environment::Production,
            revision: "rev-4711".to_owned(),
            previous_revision: Some("rev-4710".to_owned()),
            strategy: Some("canary".to_owned()),
            status: DeploymentStatus::Succeeded,
        });
        let facts = store(&evidence);
        assert_eq!(
            value(&facts, "deployment.previous_revision.exists"),
            Some(FactValue::bool(true))
        );
        assert_eq!(
            value(&facts, "deployment.production.status"),
            Some(FactValue::text("succeeded"))
        );
    }

    #[test]
    fn a_review_projects_facts_for_the_subject_kind_hierarchy() {
        let evidence = Evidence::Review(ReviewResult {
            subject: "design:passkeys".parse().expect("reference"),
            subject_kind: Some(ArtifactKind::ArchitectureDesign),
            reviewer: crate::review::Reviewer::Human {
                id: "ada".to_owned(),
            },
            disposition: crate::review::ReviewDisposition::Approved,
            findings: Vec::new(),
            reviewed_version: None,
            reviewed_revision: None,
        });
        let facts = store(&evidence);
        assert_eq!(
            value(&facts, "review.architecture-design.approved"),
            Some(FactValue::bool(true))
        );
        assert_eq!(
            value(&facts, "review.design.approved"),
            Some(FactValue::bool(true)),
            "an architecture design review is also a design review"
        );
    }

    #[test]
    fn evidence_kind_aliases_parse() {
        assert_eq!(
            EvidenceKind::parse("test_execution").expect("alias"),
            EvidenceKind::TestResult
        );
        assert_eq!(
            EvidenceKind::parse("source_diff").expect("alias"),
            EvidenceKind::Diff
        );
        assert!(EvidenceKind::parse("vibes").is_err());
    }

    #[test]
    fn every_evidence_kind_names_at_least_one_verifier() {
        for kind in EvidenceKind::ALL {
            assert!(
                !kind.default_verifiers().is_empty(),
                "{kind} has no verifier that can establish it"
            );
        }
    }
}
