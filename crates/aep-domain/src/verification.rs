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
//! act on, without being able to edit the criterion it failed. Where the run that found it was
//! generated rather than written, a [`Seed`] says how to generate it again — a counterexample that
//! cannot be reproduced is a claim about a run nobody can repeat.

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
    /// A runner that checks an implementation against a specification's own conformance suite.
    ConformanceRunner,
    /// A checker that reads a harness transcript and decides it against a trace specification.
    ///
    /// Distinct from [`Verifier::ConformanceRunner`] because the two observe different things: a
    /// conformance runner executes an implementation against a suite, and a trace checker reads a
    /// recording of how an agent worked. Distinct from [`Verifier::ArtifactValidator`] because a
    /// transcript is not an artifact of the work — it is a record of the worker — and letting any
    /// artifact validator establish a behavioural claim about an agent would make the claim
    /// indistinguishable from every other file check, which is the whole thing it is for.
    TraceChecker,
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
        Self::ConformanceRunner,
        Self::TraceChecker,
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
            Self::ConformanceRunner => "conformance-runner",
            Self::TraceChecker => "trace-checker",
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

/// What a generated run has to be given to happen again.
///
/// A string, and opaque to this protocol, because every tool that produces one spells it
/// differently: `proptest` and `quickcheck` hand out a 64-bit integer, Hypothesis a base64
/// `@reproduce_failure` blob, `fast-check` a 32-bit integer, `go test -fuzz` a corpus entry, a
/// fuzzer a file name. A `u64` would fit two of those and force the rest to encode something that
/// *looks* like a seed and reproduces nothing — worse than leaving the field out, because an
/// absent seed is visible and a lossy one is not.
///
/// So the protocol carries the token and never reads structure out of it, for the same reason an
/// entity id is never parsed for meaning. What can be spelled back is a question for the tool, and
/// the tool is already named: the evidence envelope records its producer and its provenance.
///
/// What *is* checked is only that a person could use it — non-empty, one line, and short enough to
/// be a seed rather than a corpus. A failure whose reproduction needs a whole input file has that
/// input in [`Counterexample::input`] or in an artifact; it does not belong in a field somebody is
/// expected to paste after `--seed`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
#[serde(transparent)]
pub struct Seed(String);

impl Seed {
    /// The longest seed this protocol carries.
    ///
    /// Generous for every spelling above and far short of an input corpus, which is a different
    /// thing recorded in a different place.
    pub const MAX_LENGTH: usize = 256;

    /// Builds a seed, refusing one nobody could act on.
    pub fn new(value: impl Into<String>) -> Result<Self, ParseError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(ParseError::reference(
                "seed",
                &value,
                "a blank seed claims a run can be reproduced and does not say how",
            ));
        }
        if value.chars().any(char::is_control) {
            return Err(ParseError::reference(
                "seed",
                &value,
                "a seed is one line: it is printed in a report and pasted into a command",
            ));
        }
        if value.chars().count() > Self::MAX_LENGTH {
            return Err(ParseError::reference(
                "seed",
                &value,
                format!(
                    "a seed is at most {} characters; an input this large is a corpus, and belongs \
                     in the counterexample or an artifact",
                    Self::MAX_LENGTH
                ),
            ));
        }
        Ok(Self(value))
    }

    /// The seed as written by the tool that produced it.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Seed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl<'de> serde::Deserialize<'de> for Seed {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        // A number reads naturally in a document for the tools that use one, and quoting it would
        // be a trap: `seed: 12345` must not become a shape error.
        let node = crate::node::Node::deserialize(deserializer)?;
        match &node {
            Node::Text(text) => Self::new(text.clone()).map_err(serde::de::Error::custom),
            Node::Number(number) => Self::new(number.to_string()).map_err(serde::de::Error::custom),
            other => Err(serde::de::Error::custom(format!(
                "expected a seed, found {}",
                other.type_name()
            ))),
        }
    }
}

impl schemars::JsonSchema for Seed {
    fn schema_name() -> String {
        "Seed".to_owned()
    }

    fn json_schema(_: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        let mut schema = schemars::schema::SchemaObject {
            instance_type: Some(schemars::schema::InstanceType::String.into()),
            ..Default::default()
        };
        schema.string().min_length = Some(1);
        schema.string().max_length =
            Some(u32::try_from(Self::MAX_LENGTH).expect("the limit is small enough for a schema"));
        schema.metadata().description = Some(
            "Whatever the producing tool needs to generate the same run again, in that tool's own \
             spelling. Never interpreted."
                .to_owned(),
        );
        schema.metadata().examples = ["17650292319862362387", "AXicY2BkYGAAAAANAAI="]
            .iter()
            .map(|value| serde_json::Value::String((*value).to_owned()))
            .collect();
        schema.into()
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
    fn a_trace_checker_is_a_named_class_and_not_an_external_tool() {
        // `EvidenceKind::TraceConformance::default_verifiers` names this class in a `'static`
        // slice, and only a named variant can appear in one. Were `trace-checker` to fall through
        // to `ExternalTool`, a protocol declaring it would compare unequal to the class the kind
        // demands, and `can_establish` would report the kind establishable by nobody.
        assert_eq!(
            Verifier::parse("trace-checker").expect("named"),
            Verifier::TraceChecker,
            "`trace-checker` must resolve to the named class, not to an external tool"
        );
        assert_eq!(Verifier::TraceChecker.as_str(), "trace-checker");
        assert!(
            Verifier::NAMED.contains(&Verifier::TraceChecker),
            "a class absent from `NAMED` is absent from the published schema's examples"
        );
        assert!(
            !Verifier::TraceChecker.is_human(),
            "reading a transcript mechanically is not a person reviewing one"
        );
    }

    #[test]
    fn inconclusive_is_not_a_pass() {
        assert!(VerificationStatus::Passed.is_pass());
        assert!(!VerificationStatus::Inconclusive.is_pass());
        assert!(!VerificationStatus::Skipped.is_pass());
        assert!(!VerificationStatus::Failed.is_pass());
    }

    #[test]
    fn a_seed_is_taken_in_whatever_spelling_its_tool_uses() {
        // Four real spellings from four real tools. Any of them would be lost by a `u64` field, and
        // a lost seed is a counterexample nobody can reproduce.
        for spelling in [
            "17650292319862362387",
            "AXicY2BkYGAAAAANAAI=",
            "0xdeadbeef",
            "corpus/crash-6f1c2b",
        ] {
            let seed = Seed::new(spelling).expect("a usable seed");
            assert_eq!(seed.as_str(), spelling, "the seed must survive verbatim");
        }
        let from_number: Seed = serde_json::from_str("12345").expect("a numeric seed");
        assert_eq!(
            from_number.as_str(),
            "12345",
            "`seed: 12345` is how half these tools write it, and quoting it must not be required"
        );
    }

    #[test]
    fn a_seed_nobody_could_act_on_is_refused() {
        // The field exists so a failing run can be run again. A blank or unusable value would say
        // "reproducible" and deliver nothing, which is worse than saying nothing at all.
        for refused in ["", "   ", "seed\nwith a newline"] {
            Seed::new(refused).expect_err(&format!("`{refused}` is not a usable seed"));
        }
        let corpus = "a".repeat(Seed::MAX_LENGTH + 1);
        let error = Seed::new(corpus).expect_err("a corpus is not a seed");
        assert!(
            error.to_string().contains("corpus"),
            "the reader has to be told where a large input goes instead: {error}"
        );
    }
}
