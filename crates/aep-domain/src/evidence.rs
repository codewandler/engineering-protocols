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

use crate::artifact::{Artifact, ArtifactKind, ArtifactRef, Revision};
use crate::capability::Environment;
use crate::error::ParseError;
use crate::facts::{FactPath, FactValue, Number};
use crate::ids::{ApprovalId, ClaimId, EvidenceId, ServiceId, SubjectRef, ToolRef};
use crate::review::ReviewResult;
use crate::time::Timestamp;
use crate::verification::{Counterexample, Seed, VerificationStatus, Verifier};

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
/// Facts: `property_test.<property>.{result,passed,cases,seed}`, plus
/// `property_test.<property>.seed.exists` — which is the one a rule should read, because a policy
/// can require that a failing property run be reproducible and cannot usefully compare a seed to a
/// literal.
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
    /// What the run was generated from, when the tool that ran it has a seed.
    ///
    /// Optional, because not every property checker is randomised: an exhaustive or symbolic one
    /// has nothing to seed and should not be made to invent one. But a randomised run that reports
    /// a counterexample and no seed hands back a failure nobody can make happen again, and
    /// reproducing the failure is the entire value of the report.
    ///
    /// On the run rather than on each [`Counterexample`] deliberately: the counterexample already
    /// carries its own `input` verbatim, so the thing that cannot be recovered from the record is
    /// not the failing case but the *search* that found it — and that is a property of the run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<Seed>,
    /// The outcome.
    pub status: VerificationStatus,
    /// Inputs that break the property.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub counterexamples: Vec<Counterexample>,
}

impl PropertyTestResult {
    /// `true` when somebody handed this record could run it again.
    ///
    /// A failing run that is not reproducible names a defect and withholds the way to see it.
    pub fn is_reproducible(&self) -> bool {
        self.seed.is_some()
    }
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
    ///
    /// Spelt `artifact_kind` on the wire. The enclosing [`Evidence`] is internally tagged by `kind`,
    /// so serde consumes that key as the tag before this field is ever read — a document written
    /// with `kind: artifact` and a second `kind:` inside it failed with `missing field 'kind'`, and
    /// no spelling of an artifact evidence record could be written at all. `kind` stays accepted as
    /// an alias, since a record embedded in Rust rather than parsed from a document was always fine.
    #[serde(rename = "artifact_kind", alias = "kind")]
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

/// The one hex-digest validation, shared by every digest newtype in this module.
///
/// Extracted rather than copied when [`TranscriptDigest`] joined [`SpecDigest`]: two digest types
/// that disagreed about whether upper case, a non-hex character or a length were acceptable would
/// be two definitions of "a digest" in one file, and the second one would be discovered by a
/// record that one type accepted and the other refused.
///
/// `kind` names the digest in the refusal, so the reason says which field was wrong, and `hint`
/// carries the part that is specific to that field.
fn parse_hex_digest(
    kind: &'static str,
    value: String,
    min: usize,
    max: usize,
    hint: &'static str,
) -> Result<String, ParseError> {
    if value
        .chars()
        .any(|character| character.is_ascii_uppercase())
    {
        return Err(ParseError::reference(
            kind,
            &value,
            "digests are written in lower case, so one value has one spelling",
        ));
    }
    if !value.chars().all(|character| character.is_ascii_hexdigit()) {
        return Err(ParseError::reference(kind, &value, hint));
    }
    let length = value.chars().count();
    if !(min..=max).contains(&length) {
        return Err(ParseError::reference(
            kind,
            &value,
            format!("expected {min} to {max} hexadecimal characters, found {length}"),
        ));
    }
    Ok(value)
}

/// A digest of the resolved specification a conformance suite was generated from.
///
/// Lower-case hexadecimal. `ess-gen` writes the full 64-character SHA-256 of the resolved model
/// into every generated artifact's provenance header, so 64 is the length this workspace produces.
/// It wrote a 16-character truncation before the widening (gap register D-4: once suite acceptance
/// and completion decisions rest on the digest, 64 bits is weak against construction), and records
/// from before are still *parsed* — a digest from 16 to 64 characters is accepted — so a stale
/// record fails at the digest comparison that names both digests, not at parse where the refusal
/// could name only one. A truncated digest can never equal a full one, so nothing accepted here
/// weakens the comparison.
///
/// Case is fixed rather than folded, for the reason this repository keeps arriving at: two
/// spellings of one value are two documents that disagree in text and agree in meaning, and here
/// the disagreement would be silent — an upper-case digest would simply fail to match a lower-case
/// one and the record would read as evidence about a different specification.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
#[serde(transparent)]
pub struct SpecDigest(String);

impl SpecDigest {
    /// The shortest accepted digest: what `ess-gen` wrote before the D-4 widening.
    pub const MIN_LENGTH: usize = 16;

    /// The longest accepted digest: a full SHA-256 in hex, which is what `ess-gen` writes.
    pub const MAX_LENGTH: usize = 64;

    /// Builds a digest, refusing anything that is not one.
    pub fn new(value: impl Into<String>) -> Result<Self, ParseError> {
        Ok(Self(parse_hex_digest(
            "specification digest",
            value.into(),
            Self::MIN_LENGTH,
            Self::MAX_LENGTH,
            "expected hexadecimal; a name such as `billing/v3` is not a digest",
        )?))
    }

    /// The digest as written.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SpecDigest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl<'de> serde::Deserialize<'de> for SpecDigest {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        Self::new(raw).map_err(serde::de::Error::custom)
    }
}

impl schemars::JsonSchema for SpecDigest {
    fn schema_name() -> String {
        "SpecDigest".to_owned()
    }

    fn json_schema(_: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        let mut schema = schemars::schema::SchemaObject {
            instance_type: Some(schemars::schema::InstanceType::String.into()),
            ..Default::default()
        };
        schema.string().pattern = Some(format!(
            "^[0-9a-f]{{{},{}}}$",
            Self::MIN_LENGTH,
            Self::MAX_LENGTH
        ));
        schema.metadata().description = Some(
            "A digest of the resolved specification a conformance suite was generated from, in \
             lower-case hexadecimal."
                .to_owned(),
        );
        schema.metadata().examples =
            ["4e1d3f8a9b2c1d0e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e"]
                .iter()
                .map(|value| serde_json::Value::String((*value).to_owned()))
                .collect();
        schema.into()
    }
}

/// A digest of the transcript a trace-conformance check was run over.
///
/// The full SHA-256 of the transcript file's raw bytes, in 64 lowercase hexadecimal characters —
/// exactly what `sha256sum` prints for the same file, which is the property that makes the record
/// checkable by a reader who does not run this code.
///
/// # Why this is not a [`SpecDigest`]
///
/// Both are lowercase hex and a compiler cannot tell them apart as strings, and
/// [`TraceConformanceResult`] holds one of each — so a builder that transposed them would produce
/// a record claiming a transcript's digest identified the specification, which is *false* and
/// which nothing downstream could detect. Two types make the transposition a compile error
/// instead. They also disagree about width on purpose: a `SpecDigest` may be a 16-character
/// truncation because `ess-gen` wrote those before the D-4 widening, and a transcript digest never
/// was one, so it demands all 64.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
#[serde(transparent)]
pub struct TranscriptDigest(String);

impl TranscriptDigest {
    /// The only accepted length: a full SHA-256 in hex.
    ///
    /// Exact rather than a range. Nothing in this workspace has ever written a truncated
    /// transcript digest, so accepting one would widen the type for no record that exists.
    pub const LENGTH: usize = 64;

    /// Builds a digest, refusing anything that is not one.
    pub fn new(value: impl Into<String>) -> Result<Self, ParseError> {
        Ok(Self(parse_hex_digest(
            "transcript digest",
            value.into(),
            Self::LENGTH,
            Self::LENGTH,
            "expected hexadecimal; a file path is not a digest",
        )?))
    }

    /// The digest as written.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for TranscriptDigest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl<'de> serde::Deserialize<'de> for TranscriptDigest {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        Self::new(raw).map_err(serde::de::Error::custom)
    }
}

impl schemars::JsonSchema for TranscriptDigest {
    fn schema_name() -> String {
        "TranscriptDigest".to_owned()
    }

    fn json_schema(_: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        let mut schema = schemars::schema::SchemaObject {
            instance_type: Some(schemars::schema::InstanceType::String.into()),
            ..Default::default()
        };
        schema.string().pattern = Some(format!("^[0-9a-f]{{{}}}$", Self::LENGTH));
        schema.metadata().description = Some(
            "A digest of the transcript a trace-conformance check read, as `sha256sum` writes it: \
             64 lower-case hexadecimal characters."
                .to_owned(),
        );
        schema.metadata().examples =
            ["e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"]
                .iter()
                .map(|value| serde_json::Value::String((*value).to_owned()))
                .collect();
        schema.into()
    }
}

/// What a trace-conformance check found when it judged a harness transcript against a trace
/// specification.
///
/// The body of an [`Evidence::TraceConformance`] record, and deliberately **not** the check
/// report: the report carries every expectation with the events it cites and the text those events
/// said, which is transcript-derived content that a protocol never predicates over and that a
/// record pasted into a pull request should not carry. What survives the handoff is the verdict,
/// the three counts, the id of every expectation that gapped — so a failure names something
/// actionable — and the digest pair that makes the claim mean anything.
///
/// # The digest pair is the record
///
/// *"Some agent passed some behavioural specification"* is worthless. *"The run whose transcript
/// digests to this satisfied the specification that digests to that"* is a claim a reader holding
/// the two files can check, and it is the only reason this record is worth minting. Both digests
/// are required, and both are typed so they cannot be swapped.
///
/// Facts: `trace_conformance.{status,passed,specification,spec_digest,transcript_digest}`,
/// `trace_conformance.expectations.{total,gapped,unknown}`.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct TraceConformanceResult {
    /// Which specification, by the id it declares, such as `planning-plugin/eval`.
    ///
    /// What a person reads. It is not what identifies the document — see [`Self::spec_digest`].
    pub specification: String,
    /// A digest of the specification as authored.
    pub spec_digest: SpecDigest,
    /// A digest of the transcript's raw bytes.
    pub transcript_digest: TranscriptDigest,
    /// The outcome, over the gating expectations alone.
    ///
    /// `Passed` when every gating expectation held, `Failed` when the run contradicted one, and
    /// `Inconclusive` when nothing was contradicted and something could not be judged. The last is
    /// not a softer failure: *"the agent did the wrong thing"* and *"the transcript format moved
    /// under us"* want different people to be woken up, and a record that flattened them would
    /// open a defect report against a run nobody managed to read.
    pub status: VerificationStatus,
    /// How many expectations were evaluated.
    #[serde(default)]
    pub expectations_total: usize,
    /// How many the transcript contradicted — **every** one, including any the caller downgraded
    /// to advisory for the run.
    ///
    /// See [`Self::advisory_overrides`] for why the downgrade does not shrink this number.
    #[serde(default)]
    pub expectations_gapped: usize,
    /// How many it could not decide, on the same all-rows basis.
    #[serde(default)]
    pub expectations_unknown: usize,
    /// Expectation ids the caller downgraded to advisory on the command line, so they were
    /// reported and gated nothing.
    ///
    /// Recorded rather than folded away, because the specification's digest is the digest of the
    /// document *as authored*: without this list a reader of the record could not tell that the
    /// run gated on something narrower than the document says.
    ///
    /// A downgrade moves the checker's exit code and deliberately does **not** move
    /// `trace_conformance.passed`. `--advisory` exists so a cost bound that drifted with model
    /// routing cannot turn a CI job red (design D6); it is a property of the invocation, not of
    /// the protocol's requirement, and a requirement that a caller's own flag could satisfy would
    /// not be a requirement. So the fact stays strictly stronger than exit 0, on the same polarity
    /// as everything else here: unproven is not proven.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub advisory_overrides: Vec<String>,
    /// Which adapter read the transcript, where the check named one.
    ///
    /// A harness output format is not a stable public schema (design D1), so which adapter — and
    /// which harness versions it was written against — is part of what a later reader needs to
    /// know why a verdict came out as it did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub adapter: Option<String>,
    /// Which expectations gapped, so a failure names something actionable.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub gapped_expectations: Vec<String>,
}

impl TraceConformanceResult {
    /// `true` when this check is evidence about the transcript `digest` identifies.
    ///
    /// The digest, never the file name. Two runs of one eval write two transcripts to one path,
    /// and a record naming the path would be a true statement about whichever run happened last.
    pub fn attests(&self, digest: &TranscriptDigest) -> bool {
        &self.transcript_digest == digest
    }
}

/// What a conformance run found when it checked an implementation against a specification.
///
/// This is the join between the two halves of the project: a specification generates its own
/// conformance suite, an implementation is checked against it, and the result becomes a fact the
/// protocol can decide on. The versions are all recorded because a passing result means nothing
/// without them — "conformant" is a claim about one implementation against one specification,
/// checked by one suite, and each of those moves independently.
///
/// Facts: `ess_conformance.{status,passed,spec_version,spec_digest}`,
/// `ess_conformance.scenarios.{total,failed}`.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct EssConformanceResult {
    /// Which specification, at which version, such as `billing/v3`.
    ///
    /// What a person reads. It is not what identifies the specification — see [`Self::spec_digest`].
    pub specification: String,
    /// A digest of the resolved specification the suite was generated from.
    ///
    /// Required, and the reason the whole record is worth anything. Without it this says that
    /// *some* implementation passed *some* suite; with it, the record names the exact model the
    /// suite came from, and a reader can check that it is the model in front of them. `billing/v3`
    /// is a label two different resolutions can share; a digest is not.
    pub spec_digest: SpecDigest,
    /// Which implementation was checked.
    pub implementation: String,
    /// The outcome.
    pub status: VerificationStatus,
    /// How many scenarios the suite ran.
    #[serde(default)]
    pub scenarios_total: usize,
    /// How many did not hold.
    #[serde(default)]
    pub scenarios_failed: usize,
    /// The version of the suite that ran.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suite_version: Option<String>,
    /// The compiler that produced the suite.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compiler_version: Option<String>,
    /// The generator that produced it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generator_version: Option<String>,
    /// Which scenarios failed, so a failure names something actionable.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub failed_scenarios: Vec<String>,
}

impl EssConformanceResult {
    /// `true` when this run is evidence about the specification `digest` identifies.
    ///
    /// The digest, never the label. `billing/v3` names a version of a system; it does not name the
    /// resolved model a suite was generated from, and two resolutions that differ by one field are
    /// two specifications wearing one label. A run against a different digest is a true statement
    /// about a different specification — which is exactly as useful here as an approval of version
    /// three is to version seven.
    pub fn attests(&self, digest: &SpecDigest) -> bool {
        &self.spec_digest == digest
    }

    /// `true` when this run was produced against the revision `artifact` describes *now*.
    ///
    /// The conformance counterpart of
    /// [`ReviewResult::covers`](crate::review::ReviewResult::covers), and the same defect it
    /// refuses one layer down: an approval of version three does not cover version seven, and a
    /// suite run against yesterday's model does not attest today's. Without this, a task closes
    /// having proven conformance to a specification nobody is building against any more.
    ///
    /// # This fails closed, deliberately
    ///
    /// An artifact that records no digest returns `false`, not `true`. Evidence that *cannot
    /// demonstrate* it was produced against the current revision does not satisfy the requirement;
    /// it is not presumed current until something proves otherwise.
    ///
    /// That polarity is a decision, and the next reader will meet the opposite one:
    /// `docs/design/ess-semantic-diff-impact-evolution-design-v0.1.md` proposes an invalidation
    /// model in which a record stays valid until a diff proves it stale — evidence failing *open*.
    /// `docs/reviews/2026-08-20-semantic-diff-feasibility-review.md` (finding S1) concluded that
    /// design had the polarity backwards and needs this rule as a precondition. Unproven is not
    /// proven, here as everywhere else in this protocol: this is the same reasoning as
    /// [`Truth::Unknown`](crate::predicate::Truth::Unknown) never collapsing to `true`.
    ///
    /// The one declared exception is the artifact's own
    /// [`FreshnessPolicy::AlwaysValid`](crate::artifact::FreshnessPolicy::AlwaysValid), which is
    /// applied by [`ArtifactGraph::governing_models`](crate::artifact::ArtifactGraph::governing_models)
    /// rather than here, so that the opt-out is one written statement in a manifest rather than a
    /// branch scattered through every check.
    pub fn covers(&self, artifact: &Artifact) -> bool {
        artifact.is_at_revision(&self.spec_digest)
    }
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum EvidenceKind {
    /// A test run.
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
    Diff,
    /// An observation about an artifact.
    Artifact,
    /// A review outcome.
    Review,
    /// A verifier's statement about a claim.
    Verification,
    /// Specification satisfaction.
    Specification,
    /// An implementation checked against an executable system specification.
    EssConformance,
    /// A harness transcript checked against a trace specification.
    ///
    /// What it attests: a `trace-spec` document was decided against the recorded transcript of an
    /// agent run — which tools were called, in which order, with which arguments and results, in
    /// which environment, and at what cost — by the trace checker reading that file. The producer
    /// is therefore [`Producer::Verifier`]: the checker observed a recording, it did not ask the
    /// agent how the run went.
    ///
    /// What its provenance must carry: **the digest of the transcript** and **the digest of the
    /// specification**, in [`Provenance::digest`] and [`Provenance::inputs`]. Without both, the
    /// record says only that *some* run satisfied *some* specification, which establishes nothing;
    /// with both it says that the run with this digest satisfied the specification with that one,
    /// and any reader holding the two files can check it.
    ///
    /// What it is not: an agent's own account of how it worked. That is a claim by the subject
    /// about the subject, and no amount of confidence in it makes it this kind — which is the
    /// reason the kind exists separately from [`EvidenceKind::Verification`] rather than being
    /// folded into it. The only class that can establish it is [`Verifier::TraceChecker`], and
    /// that is what makes a behavioural claim about an LLM step admissible without the LLM minting
    /// anything.
    TraceConformance,
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
        Self::EssConformance,
        Self::TraceConformance,
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
            Self::EssConformance => "ess_conformance",
            Self::TraceConformance => "trace_conformance",
        }
    }

    /// Spellings accepted alongside [`EvidenceKind::as_str`], kept for documents written against
    /// earlier drafts.
    ///
    /// One list, read by [`EvidenceKind::parse`], by `Deserialize` through it, and by the generated
    /// schema. It used to be three — a `match` arm, a `#[serde(alias)]` and nothing at all in the
    /// schema — which is how the published vocabulary came to be smaller than the parser's.
    pub const ALIASES: &'static [(&'static str, Self)] = &[
        ("test_execution", Self::TestResult),
        ("source_diff", Self::Diff),
        ("artifact_evidence", Self::Artifact),
        ("review_result", Self::Review),
        ("verification_record", Self::Verification),
        ("specification_record", Self::Specification),
        ("ess_conformance_result", Self::EssConformance),
    ];

    /// Parses a kind name, accepting the documented aliases.
    pub fn parse(value: &str) -> Result<Self, ParseError> {
        if let Some(known) = Self::ALL.iter().find(|kind| kind.as_str() == value) {
            return Ok(*known);
        }
        if let Some((_, kind)) = Self::ALIASES.iter().find(|(alias, _)| *alias == value) {
            return Ok(*kind);
        }
        Err(ParseError::identifier(
            "evidence kind",
            value,
            format!(
                "expected one of {}",
                Self::ALL
                    .iter()
                    .map(|kind| kind.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        ))
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
            Self::EssConformance => &[Verifier::ConformanceRunner],
            Self::TraceConformance => &[Verifier::TraceChecker],
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

impl<'de> serde::Deserialize<'de> for EvidenceKind {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        Self::parse(&raw).map_err(serde::de::Error::custom)
    }
}

impl schemars::JsonSchema for EvidenceKind {
    fn schema_name() -> String {
        "EvidenceKind".to_owned()
    }

    // Written by hand because [`EvidenceKind::parse`] is: it accepts the aliases as well as the
    // canonical names, and a derived schema publishes only what the variants are called — so an
    // editor marks `source_diff` invalid in a document the engine reads without complaint.
    fn json_schema(_: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        let spellings = Self::ALL
            .iter()
            .map(|kind| kind.as_str().to_owned())
            .chain(Self::ALIASES.iter().map(|(alias, _)| (*alias).to_owned()))
            .map(serde_json::Value::String)
            .collect();
        let mut schema = schemars::schema::SchemaObject {
            instance_type: Some(schemars::schema::InstanceType::String.into()),
            ..Default::default()
        };
        schema.enum_values = Some(spellings);
        schema.metadata().description = Some(
            "A kind of evidence. The canonical names come first; the rest are older spellings \
             still accepted."
                .to_owned(),
        );
        schema.into()
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
    /// An implementation checked against an executable system specification.
    EssConformance(EssConformanceResult),
    /// A harness transcript checked against a trace specification.
    TraceConformance(TraceConformanceResult),
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
            Self::EssConformance(_) => EvidenceKind::EssConformance,
            Self::TraceConformance(_) => EvidenceKind::TraceConformance,
        }
    }

    /// The resolved-model digest this evidence was produced against, where it has one.
    ///
    /// The other half of the scoping on the revision binding: it applies where the evidence
    /// carries a digest *and* the graph holds an artifact that pins one. Most evidence kinds
    /// carry none — a unit-test run is not about a compiled model — and the binding must leave
    /// them alone rather than refusing them for missing something they never had.
    ///
    /// A match arm rather than a downcast, so a new digest-carrying evidence kind opts in by
    /// being listed here and cannot acquire the binding by accident.
    pub fn spec_digest(&self) -> Option<&SpecDigest> {
        match self {
            Self::EssConformance(result) => Some(&result.spec_digest),
            // `TraceConformance` also holds a `SpecDigest` and deliberately does **not** opt in,
            // which is worth saying because a reader will otherwise take it for an oversight. This
            // binding asks whether evidence was produced against the resolved model an artifact
            // pins *now*; a trace specification's digest is the digest of an authored YAML
            // document about an agent's behaviour, and no ESS artifact will ever pin one.
            // Returning it would make every trace record fail the revision comparison for a reason
            // that has nothing to do with the revision.
            _ => None,
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
            Self::EssConformance(result) => format!(
                "{} against {}: {} ({}/{} scenarios failed)",
                result.implementation,
                result.specification,
                result.status,
                result.scenarios_failed,
                result.scenarios_total
            ),
            Self::TraceConformance(result) => format!(
                "transcript against {}: {} ({} ok, {} gap, {} unk of {})",
                result.specification,
                result.status,
                result
                    .expectations_total
                    .saturating_sub(result.expectations_gapped + result.expectations_unknown),
                result.expectations_gapped,
                result.expectations_unknown,
                result.expectations_total
            ),
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
                // The seed itself is projected so a report can print it, but `seed.exists` is what
                // a rule reads: "this run can be reproduced" is a decidable requirement, and
                // comparing a seed to a literal is not.
                if let Some(seed) = &result.seed {
                    facts.push((base.child("seed"), FactValue::text(seed.as_str())));
                }
                facts.push((
                    base.child("seed").child("exists"),
                    FactValue::bool(result.is_reproducible()),
                ));
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
            Self::EssConformance(result) => {
                let base = path(&["ess_conformance"]);
                facts.push((
                    base.child("status"),
                    FactValue::text(result.status.as_str()),
                ));
                facts.push((
                    base.child("passed"),
                    FactValue::bool(result.status.is_pass() && result.scenarios_failed == 0),
                ));
                facts.push((
                    base.child("spec_version"),
                    FactValue::text(result.specification.clone()),
                ));
                // The digest, beside the label, so a rule can pin the specification a task is
                // governed by rather than trusting a name two models can share.
                facts.push((
                    base.child("spec_digest"),
                    FactValue::text(result.spec_digest.as_str()),
                ));
                facts.push((
                    base.child("scenarios").child("total"),
                    FactValue::count(result.scenarios_total),
                ));
                facts.push((
                    base.child("scenarios").child("failed"),
                    FactValue::count(result.scenarios_failed),
                ));
            }
            Self::TraceConformance(result) => {
                let base = path(&["trace_conformance"]);
                facts.push((
                    base.child("status"),
                    FactValue::text(result.status.as_str()),
                ));
                // The pessimistic reading, as `ess_conformance.passed` takes: a check reporting a
                // pass alongside gapped expectations is contradicting itself, and this fact does
                // not take the optimistic half of that.
                facts.push((
                    base.child("passed"),
                    FactValue::bool(result.status.is_pass() && result.expectations_gapped == 0),
                ));
                facts.push((
                    base.child("specification"),
                    FactValue::text(result.specification.clone()),
                ));
                facts.push((
                    base.child("spec_digest"),
                    FactValue::text(result.spec_digest.as_str()),
                ));
                // The transcript's digest is a fact in its own right: it is how a rule pins the
                // *run* a claim is about, where `spec_digest` pins only the document it was
                // judged against.
                facts.push((
                    base.child("transcript_digest"),
                    FactValue::text(result.transcript_digest.as_str()),
                ));
                facts.push((
                    base.child("expectations").child("total"),
                    FactValue::count(result.expectations_total),
                ));
                facts.push((
                    base.child("expectations").child("gapped"),
                    FactValue::count(result.expectations_gapped),
                ));
                facts.push((
                    base.child("expectations").child("unknown"),
                    FactValue::count(result.expectations_unknown),
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

    #[test]
    fn an_artifact_evidence_record_can_be_written_in_a_document_at_all() {
        // `Evidence` is internally tagged by `kind`, and this variant also had a field called
        // `kind` — so serde ate the tag and then reported the field it had just consumed as
        // missing. Every artifact evidence document failed with `missing field 'kind'`, which meant
        // no `development.critical` task could satisfy `design-by-contract` or `preserve-evidence`
        // through a document at all. The variant existed and was unreachable from the one place a
        // user writes evidence.
        let written = r#"
kind: artifact
artifact: "design:passkeys-auth"
artifact_kind: design
observation: sections_present
"#;
        let evidence: Evidence =
            serde_yaml::from_str(written).expect("an artifact record parses from a document");
        let Evidence::Artifact(artifact) = &evidence else {
            panic!("the tag selects the artifact variant: {evidence:?}");
        };
        assert_eq!(artifact.kind, ArtifactKind::Design);

        // Inside the enum the old spelling cannot work and never did: two `kind:` keys in one
        // mapping are a duplicate field, which is the defect itself. The alias earns its place one
        // level down, where the struct is read on its own and there is no tag to collide with.
        let alone: ArtifactEvidence = serde_yaml::from_str(
            "artifact: \"design:passkeys-auth\"\nkind: design\nobservation: sections_present\n",
        )
        .expect("the struct alone still accepts the original spelling");
        assert_eq!(alone.kind, ArtifactKind::Design);

        let duplicated = written.replace("artifact_kind:", "kind:");
        let error = serde_yaml::from_str::<Evidence>(&duplicated)
            .expect_err("two `kind:` keys in one mapping is not a record");
        assert!(
            error.to_string().contains("duplicate field"),
            "the refusal should name the collision rather than a missing field: {error}"
        );
    }

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
    fn a_property_run_hands_back_the_seed_that_reproduces_its_counterexample() {
        // The point of a counterexample is that somebody can go and see it. A record that reports
        // one and cannot say how the run was generated proves a defect exists and withholds the
        // only way to look at it.
        let evidence = Evidence::PropertyTestResult(PropertyTestResult {
            property: "session-isolation".parse().expect("claim"),
            cases: 10_000,
            seed: Some(Seed::new("17650292319862362387").expect("a seed")),
            status: VerificationStatus::Failed,
            counterexamples: vec![Counterexample {
                verifier: Verifier::PropertyTester,
                property: Some("session-isolation".parse().expect("claim")),
                note: Some("two sessions shared a cache key".to_owned()),
                ..Counterexample::default()
            }],
        });
        let facts = store(&evidence);

        assert_eq!(
            value(&facts, "property_test.session-isolation.seed"),
            Some(FactValue::text("17650292319862362387")),
            "the seed must survive verbatim: it is the tool's spelling, not ours"
        );
        assert_eq!(
            value(&facts, "property_test.session-isolation.seed.exists"),
            Some(FactValue::bool(true))
        );
    }

    #[test]
    fn a_property_run_without_a_seed_says_so_rather_than_looking_reproducible() {
        // An exhaustive checker has nothing to seed and must not be made to invent one — but the
        // difference has to be readable, so a rule can insist that a *randomised* failure carries
        // one without banning the checkers that do not need it.
        let evidence = Evidence::PropertyTestResult(PropertyTestResult {
            property: "session-isolation".parse().expect("claim"),
            cases: 10_000,
            seed: None,
            status: VerificationStatus::Passed,
            counterexamples: Vec::new(),
        });
        let facts = store(&evidence);

        assert_eq!(
            value(&facts, "property_test.session-isolation.seed.exists"),
            Some(FactValue::bool(false))
        );
        assert_eq!(
            value(&facts, "property_test.session-isolation.seed"),
            None,
            "an absent seed must be absent, not an empty string that reads as one"
        );
    }

    #[test]
    fn a_conformance_result_projects_a_fact_a_completion_condition_can_read() {
        let evidence = Evidence::EssConformance(EssConformanceResult {
            specification: "billing/v3".to_owned(),
            spec_digest: SpecDigest::new("4e1d3f8a9b2c1d0e").expect("a digest"),
            implementation: "invoice-service@rev-4711".to_owned(),
            status: VerificationStatus::Passed,
            scenarios_total: 24,
            scenarios_failed: 0,
            suite_version: Some("1".to_owned()),
            compiler_version: Some("0.3.0".to_owned()),
            generator_version: Some("0.3.0".to_owned()),
            failed_scenarios: Vec::new(),
        });
        let facts = store(&evidence);

        assert_eq!(
            value(&facts, "ess_conformance.passed"),
            Some(FactValue::bool(true))
        );
        assert_eq!(
            value(&facts, "ess_conformance.spec_version"),
            Some(FactValue::text("billing/v3"))
        );
        assert_eq!(
            value(&facts, "ess_conformance.scenarios.total"),
            Some(FactValue::count(24))
        );
        assert_eq!(
            value(&facts, "ess_conformance.spec_digest"),
            Some(FactValue::text("4e1d3f8a9b2c1d0e")),
            "a rule has to be able to pin the specification, not just read its label"
        );
    }

    #[test]
    fn conformance_evidence_for_one_specification_does_not_attest_another() {
        // The same shape as `an_approval_of_version_three_does_not_cover_version_seven`: a true
        // statement about one thing is not a statement about the thing in front of you. Here both
        // records say `billing/v3` and both are honest; they were produced against two different
        // resolutions of it, and only the digest can tell anyone that.
        let run = |digest: &str| EssConformanceResult {
            specification: "billing/v3".to_owned(),
            spec_digest: SpecDigest::new(digest).expect("a digest"),
            implementation: "invoice-service@rev-4711".to_owned(),
            status: VerificationStatus::Passed,
            scenarios_total: 24,
            scenarios_failed: 0,
            suite_version: Some("1".to_owned()),
            compiler_version: Some("0.3.0".to_owned()),
            generator_version: Some("0.3.0".to_owned()),
            failed_scenarios: Vec::new(),
        };
        let governing = SpecDigest::new("4e1d3f8a9b2c1d0e").expect("a digest");

        assert!(run("4e1d3f8a9b2c1d0e").attests(&governing));
        assert!(
            !run("0badc0ffee123456").attests(&governing),
            "a passing run against a different model must not read as conformance to this one"
        );
    }

    #[test]
    fn a_run_against_yesterdays_revision_does_not_cover_the_specification_in_the_graph() {
        // `attests` compares a run with a digest somebody already found. `covers` is the question a
        // requirement actually asks: is this run about the specification *as the graph holds it
        // now*. The third case is the one the polarity argument turns on — an artifact recording no
        // digest is covered by nothing, because unproven is not proven.
        use crate::artifact::{ArtifactId, ArtifactKind, ArtifactLocation, ArtifactStatus};

        let specification = |digest: Option<&str>| {
            let mut artifact = crate::artifact::Artifact::new(
                ArtifactId::new("ess:billing/v3").expect("id"),
                ArtifactKind::ExecutableSystemSpecification,
                ArtifactStatus::Approved,
                ArtifactLocation::Inline,
            );
            artifact.model_digest = digest.map(|value| SpecDigest::new(value).expect("a digest"));
            artifact
        };
        let run = |digest: &str| EssConformanceResult {
            specification: "billing/v3".to_owned(),
            spec_digest: SpecDigest::new(digest).expect("a digest"),
            implementation: "invoice-service@rev-4711".to_owned(),
            status: VerificationStatus::Passed,
            scenarios_total: 24,
            scenarios_failed: 0,
            suite_version: Some("1".to_owned()),
            compiler_version: Some("0.3.0".to_owned()),
            generator_version: Some("0.3.0".to_owned()),
            failed_scenarios: Vec::new(),
        };

        assert!(run("4e1d3f8a9b2c1d0e").covers(&specification(Some("4e1d3f8a9b2c1d0e"))));
        assert!(
            !run("0badc0ffee123456").covers(&specification(Some("4e1d3f8a9b2c1d0e"))),
            "a green run against yesterday's resolution is not conformance to today's"
        );
        assert!(
            !run("4e1d3f8a9b2c1d0e").covers(&specification(None)),
            "a specification recording no digest is covered by nothing: this fails closed"
        );
    }

    #[test]
    fn only_evidence_that_was_produced_against_a_model_carries_a_digest() {
        // The scope of the revision binding on the evidence side. A unit-test run is not about a
        // compiled model, so the binding must not refuse it for lacking a digest it never had.
        assert_eq!(
            Evidence::EssConformance(EssConformanceResult {
                specification: "billing/v3".to_owned(),
                spec_digest: SpecDigest::new("4e1d3f8a9b2c1d0e").expect("a digest"),
                implementation: "invoice-service".to_owned(),
                status: VerificationStatus::Passed,
                scenarios_total: 24,
                scenarios_failed: 0,
                suite_version: None,
                compiler_version: None,
                generator_version: None,
                failed_scenarios: Vec::new(),
            })
            .spec_digest()
            .map(SpecDigest::as_str),
            Some("4e1d3f8a9b2c1d0e")
        );
        assert_eq!(
            Evidence::TestResult(TestResult::passing(TestSuite::Unit, 12)).spec_digest(),
            None
        );
    }

    #[test]
    fn a_conformance_record_that_cannot_name_its_specification_is_refused() {
        // The field is required, and this is why: a record without it proves that some
        // implementation passed some suite. Nobody can act on that.
        let error = serde_yaml::from_str::<EssConformanceResult>(
            r"
specification: billing/v3
implementation: invoice-service@rev-4711
status: passed
scenarios_total: 24
",
        )
        .expect_err("no digest, no claim");
        assert!(error.to_string().contains("spec_digest"), "{error}");

        for refused in ["billing/v3", "4E1D3F8A9B2C1D0E", "4e1d", "zzzzzzzzzzzzzzzz"] {
            SpecDigest::new(refused)
                .expect_err(&format!("`{refused}` is not a specification digest"));
        }
    }

    #[test]
    fn a_conformance_run_with_failures_does_not_pass_however_it_reports_its_status() {
        // A runner that returned `passed` alongside failing scenarios is contradicting itself, and
        // the fact a completion condition reads must not take the optimistic half of that.
        let evidence = Evidence::EssConformance(EssConformanceResult {
            specification: "billing/v3".to_owned(),
            spec_digest: SpecDigest::new("4e1d3f8a9b2c1d0e").expect("a digest"),
            implementation: "invoice-service@rev-4711".to_owned(),
            status: VerificationStatus::Passed,
            scenarios_total: 24,
            scenarios_failed: 2,
            suite_version: None,
            compiler_version: None,
            generator_version: None,
            failed_scenarios: vec!["invoice-creation".to_owned(), "cancel-paid".to_owned()],
        });
        let facts = store(&evidence);

        assert_eq!(
            value(&facts, "ess_conformance.passed"),
            Some(FactValue::bool(false)),
            "two failing scenarios is not conformance, whatever the status field says"
        );
        assert_eq!(
            value(&facts, "ess_conformance.scenarios.failed"),
            Some(FactValue::count(2))
        );
        assert!(
            evidence.summary().contains("2/24"),
            "{}",
            evidence.summary()
        );
    }

    #[test]
    fn only_a_conformance_runner_establishes_conformance() {
        assert_eq!(
            EvidenceKind::EssConformance.default_verifiers(),
            &[Verifier::ConformanceRunner],
            "an agent reporting that its own implementation conforms is not a conformance run"
        );
    }

    fn trace_result() -> TraceConformanceResult {
        TraceConformanceResult {
            specification: "planning-plugin/eval".to_owned(),
            spec_digest: SpecDigest::new(
                "9f3ca1b2c3d4e5f60718293a4b5c6d7e8f90a1b2c3d4e5f60718293a4b5c6d7e",
            )
            .expect("a digest"),
            transcript_digest: TranscriptDigest::new(
                "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            )
            .expect("a digest"),
            status: VerificationStatus::Failed,
            expectations_total: 10,
            expectations_gapped: 2,
            expectations_unknown: 1,
            adapter: Some("claude-code/stream-json".to_owned()),
            advisory_overrides: Vec::new(),
            gapped_expectations: vec![
                "nothing-else-loaded".to_owned(),
                "no-hand-edited-frontmatter".to_owned(),
            ],
        }
    }

    #[test]
    fn a_trace_check_projects_the_verdict_the_counts_and_both_digests() {
        let evidence = Evidence::TraceConformance(trace_result());
        let facts = store(&evidence);

        assert_eq!(
            value(&facts, "trace_conformance.status"),
            Some(FactValue::text("failed"))
        );
        assert_eq!(
            value(&facts, "trace_conformance.specification"),
            Some(FactValue::text("planning-plugin/eval"))
        );
        assert_eq!(
            value(&facts, "trace_conformance.expectations.total"),
            Some(FactValue::count(10))
        );
        assert_eq!(
            value(&facts, "trace_conformance.expectations.gapped"),
            Some(FactValue::count(2))
        );
        assert_eq!(
            value(&facts, "trace_conformance.expectations.unknown"),
            Some(FactValue::count(1))
        );
        // Both digests are facts, and they are different facts: one pins the document the run was
        // judged against, the other pins the run. A record carrying only the first would say that
        // *some* run satisfied this specification.
        assert_eq!(
            value(&facts, "trace_conformance.spec_digest"),
            Some(FactValue::text(
                "9f3ca1b2c3d4e5f60718293a4b5c6d7e8f90a1b2c3d4e5f60718293a4b5c6d7e"
            ))
        );
        assert_eq!(
            value(&facts, "trace_conformance.transcript_digest"),
            Some(FactValue::text(
                "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
            ))
        );
    }

    #[test]
    fn a_trace_check_reporting_a_pass_beside_a_gap_does_not_pass() {
        // The pessimistic reading `ess_conformance.passed` takes, one domain over: a record that
        // says `passed` and then lists a contradicted expectation is contradicting itself, and the
        // fact a principle predicates over does not take the optimistic half of that.
        let mut result = trace_result();
        result.status = VerificationStatus::Passed;
        assert_eq!(
            result.expectations_gapped, 2,
            "the fixture reaches the state"
        );
        let facts = store(&Evidence::TraceConformance(result));
        assert_eq!(
            value(&facts, "trace_conformance.passed"),
            Some(FactValue::bool(false)),
            "a self-contradicting record must not project a pass"
        );

        let mut clean = trace_result();
        clean.status = VerificationStatus::Passed;
        clean.expectations_gapped = 0;
        clean.gapped_expectations.clear();
        let facts = store(&Evidence::TraceConformance(clean));
        assert_eq!(
            value(&facts, "trace_conformance.passed"),
            Some(FactValue::bool(true))
        );
    }

    #[test]
    fn an_undecided_run_is_not_a_failed_one_in_the_record() {
        // Exit 3, not a softer exit 1. `Inconclusive` is not a pass either, so the requirement
        // stays owed — but a reader can tell "the agent did the wrong thing" from "nobody could
        // read the transcript", which is the distinction the whole three-valued checker exists for.
        let mut result = trace_result();
        result.status = VerificationStatus::Inconclusive;
        result.expectations_gapped = 0;
        result.gapped_expectations.clear();
        let facts = store(&Evidence::TraceConformance(result));
        assert_eq!(
            value(&facts, "trace_conformance.status"),
            Some(FactValue::text("inconclusive"))
        );
        assert_eq!(
            value(&facts, "trace_conformance.passed"),
            Some(FactValue::bool(false)),
            "undecided is not proven, here as everywhere else in this protocol"
        );
    }

    #[test]
    fn a_trace_record_does_not_claim_to_be_about_an_ess_revision() {
        // `Evidence::spec_digest` is the resolved-model digest the ESS revision binding compares
        // against an artifact. A trace specification's digest is a digest of an authored YAML
        // document about behaviour, and no ESS artifact pins one — so opting in would make every
        // trace record fail that comparison for a reason unrelated to the revision.
        let evidence = Evidence::TraceConformance(trace_result());
        assert!(
            evidence.spec_digest().is_none(),
            "the trace record must not be dragged into the ESS revision binding"
        );
        assert_eq!(evidence.kind(), EvidenceKind::TraceConformance);
        assert!(
            evidence.summary().contains("2 gap"),
            "the summary names what went wrong: {}",
            evidence.summary()
        );
    }

    #[test]
    fn a_trace_record_round_trips_through_the_document_form_the_engine_reads() {
        let evidence = Evidence::TraceConformance(trace_result());
        let written = serde_yaml::to_string(&evidence).expect("serialises");
        assert!(
            written.contains("kind: trace_conformance"),
            "the tag is the wire name the protocol declares: {written}"
        );
        let read: Evidence = serde_yaml::from_str(&written).expect("a written record parses back");
        assert_eq!(read, evidence, "the document form is lossless");
    }

    #[test]
    fn a_transcript_digest_is_not_interchangeable_with_a_specification_digest() {
        // Both are lowercase hex and a `String` field would let a builder transpose them, which
        // would produce a record whose digest pair is false and which nothing downstream could
        // detect. The types are what make that a compile error; the widths differ as well.
        assert!(
            TranscriptDigest::new("e3b0c44298fc1c14").is_err(),
            "a 16-hex truncation is a legacy `SpecDigest`, and never a transcript digest"
        );
        assert!(
            SpecDigest::new("e3b0c44298fc1c14").is_ok(),
            "the fixture reaches the state: the same string is a valid `SpecDigest`"
        );
        let error = TranscriptDigest::new(
            "E3B0C44298FC1C149AFBF4C8996FB92427AE41E4649B934CA495991B7852B855",
        )
        .expect_err("upper case is refused");
        assert!(
            error.to_string().contains("lower case"),
            "the refusal says why: {error}"
        );
        assert!(
            TranscriptDigest::new("integrations/claude-code/eval/result.jsonl").is_err(),
            "a path is not a digest, and a record naming a path would name whichever run was last"
        );
    }

    #[test]
    fn only_a_trace_checker_establishes_transcript_conformance() {
        assert_eq!(
            EvidenceKind::TraceConformance.default_verifiers(),
            &[Verifier::TraceChecker],
            "an agent's own account of how it worked is not a check of the transcript it produced"
        );
        assert!(
            !EvidenceKind::TraceConformance
                .default_verifiers()
                .iter()
                .any(Verifier::is_human),
            "a transcript check is mechanical: the same transcript and spec give the same verdict"
        );
    }

    #[test]
    fn trace_conformance_is_spelled_the_way_the_checker_writes_it() {
        // The wire name is pinned: it is what the trace checker writes into an evidence record,
        // what `protocols/adp/1.yaml` declares, and what a document written by hand says. A rename
        // here silently stops matching a declaration that is not in this repository's control.
        assert_eq!(
            EvidenceKind::TraceConformance.as_str(),
            "trace_conformance",
            "the wire name is a contract with the checker and with every protocol declaring it"
        );
        assert_eq!(
            EvidenceKind::TraceConformance.to_string(),
            "trace_conformance",
            "`Display` and `as_str` are one spelling, not two"
        );
        assert_eq!(
            EvidenceKind::parse("trace_conformance").expect("the canonical name parses"),
            EvidenceKind::TraceConformance
        );
        assert_eq!(
            "trace_conformance"
                .parse::<EvidenceKind>()
                .expect("`FromStr` goes through `parse`"),
            EvidenceKind::TraceConformance
        );
        assert!(
            EvidenceKind::parse("trace-conformance").is_err(),
            "evidence kinds are snake_case; the hyphenated spelling is a verifier name, not a kind"
        );
        assert!(
            !EvidenceKind::ALIASES
                .iter()
                .any(|(_, kind)| *kind == EvidenceKind::TraceConformance),
            "a kind with no earlier drafts has no older spellings to accept"
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
    fn deserializing_an_evidence_kind_accepts_exactly_what_parsing_it_accepts() {
        // The generated schema publishes `ALL` and `ALIASES`; `Deserialize` goes through `parse`,
        // so the vocabulary an editor is shown and the one a document is read with are one list.
        for kind in EvidenceKind::ALL {
            let parsed: EvidenceKind = serde_json::from_str(&format!("\"{}\"", kind.as_str()))
                .expect("serde reads the canonical name");
            assert_eq!(parsed, *kind);
        }
        for (alias, kind) in EvidenceKind::ALIASES {
            let parsed: EvidenceKind =
                serde_json::from_str(&format!("\"{alias}\"")).expect("serde reads the alias");
            assert_eq!(parsed, *kind, "serde reads `{alias}` as something else");
        }
        assert!(
            serde_json::from_str::<EvidenceKind>("\"vibes\"").is_err(),
            "serde must refuse a kind `parse` refuses"
        );
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
