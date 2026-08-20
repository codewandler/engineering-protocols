//! Principles: enforceable engineering rules.
//!
//! A principle is the difference between "use test-driven development" and something a machine
//! can check. It says *when* it applies, *what* must be true and *by when*, *what evidence*
//! counts, *which verifiers* must have spoken, *which capabilities* it takes away, and *what
//! happens on failure*.
//!
//! ```yaml
//! id: test-driven
//! version: 1
//! applies_when:
//!   task.kind: {any_of: [feature, bugfix]}
//! requires:
//!   before_implementation:
//!     - test.exists
//!     - test.first_result == failed
//!   before_completion:
//!     - tests.unit.failed == 0
//!     - regression_suite.result == passed
//! evidence:
//!   - test_result
//!   - source_diff
//! ```
//!
//! # Timing
//!
//! Obligations are timed against workflow *phases*, not against state names, so one principle
//! works with any workflow that declares the phase. `before_implementation` holds for any state
//! tagged `implementation`, whether the workflow calls that state `implement`, `build` or
//! `mitigate`. An obligation with no stated timing defaults to
//! [`ObligationTiming::default`] — before completion — which never blocks early and always
//! blocks finishing.

use std::collections::BTreeMap;
use std::fmt;

use crate::artifact::ArtifactKind;
use crate::capability::CapabilityPolicy;
use crate::error::{ParseError, ValidationCode, ValidationError, ValidationErrors};
use crate::ids::{ClaimId, ObligationId, PhaseId, PrincipleId, StateId};
use crate::node::Node;
use crate::predicate::Predicate;
use crate::requirement::{EvidenceRequirement, RequirementSet};
use crate::verification::Verifier;
use crate::version::{MajorVersion, PrincipleRef};

/// The phase name a workflow's terminal states are expected to declare.
pub const COMPLETION_PHASE: &str = "completion";

/// What an obligation is timed against.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum PhaseRef {
    /// A phase, which any number of states may declare.
    Phase(PhaseId),
    /// One specific state.
    State(StateId),
}

impl PhaseRef {
    /// Parses `{phase: x}`, `{state: y}` or a bare phase name.
    pub fn from_node(node: &Node) -> Result<Self, ParseError> {
        match node {
            Node::Text(phase) => Ok(Self::Phase(PhaseId::new(phase.as_str())?)),
            Node::Map(entries) => {
                if let Some(state) = entries.get("state").and_then(Node::as_text) {
                    return Ok(Self::State(StateId::new(state)?));
                }
                if let Some(phase) = entries.get("phase").and_then(Node::as_text) {
                    return Ok(Self::Phase(PhaseId::new(phase)?));
                }
                Err(ParseError::shape(
                    "obligation timing",
                    "`phase` or `state`",
                    format!("keys {:?}", entries.keys().collect::<Vec<_>>()),
                ))
            }
            other => Err(ParseError::shape(
                "obligation timing",
                "a phase name or a mapping",
                other.type_name(),
            )),
        }
    }
}

impl fmt::Display for PhaseRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Phase(phase) => write!(f, "phase {phase}"),
            Self::State(state) => write!(f, "state {state}"),
        }
    }
}

/// When an obligation must hold.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, schemars::JsonSchema,
)]
#[serde(tag = "timing", rename_all = "snake_case")]
pub enum ObligationTiming {
    /// At every transition.
    Always,
    /// Before entering a matching state.
    Before {
        /// What must not be entered until this holds.
        target: PhaseRef,
    },
    /// While in a matching state, checked when leaving it.
    During {
        /// Where this applies.
        target: PhaseRef,
    },
}

impl ObligationTiming {
    /// Before completion: the default when a principle does not say.
    pub fn default_timing() -> Self {
        Self::Before {
            target: PhaseRef::Phase(
                PhaseId::new(COMPLETION_PHASE).expect("the completion phase name is valid"),
            ),
        }
    }

    /// The phase or state this timing refers to, if any.
    pub fn target(&self) -> Option<&PhaseRef> {
        match self {
            Self::Always => None,
            Self::Before { target } | Self::During { target } => Some(target),
        }
    }

    /// A short slug, used to build obligation identifiers.
    fn slug(&self) -> String {
        match self {
            Self::Always => "always".to_owned(),
            Self::Before { target } => format!("before-{}", slug_of(target)),
            Self::During { target } => format!("during-{}", slug_of(target)),
        }
    }
}

impl Default for ObligationTiming {
    fn default() -> Self {
        Self::default_timing()
    }
}

/// Renders a phase reference as an identifier-safe slug.
fn slug_of(target: &PhaseRef) -> String {
    match target {
        PhaseRef::Phase(phase) => phase.as_str().to_owned(),
        PhaseRef::State(state) => state.as_str().replace(['.', '/'], "-"),
    }
}

impl fmt::Display for ObligationTiming {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Always => f.write_str("always"),
            Self::Before { target } => write!(f, "before {target}"),
            Self::During { target } => write!(f, "during {target}"),
        }
    }
}

/// One timed obligation of a principle.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, schemars::JsonSchema)]
pub struct Obligation {
    /// Its identifier, unique within the principle.
    pub id: ObligationId,
    /// What it is for, in one line.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// When it must hold.
    pub timing: ObligationTiming,
    /// What must hold.
    pub requires: RequirementSet,
}

impl fmt::Display for Obligation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.description {
            Some(description) => write!(f, "{} ({}, {})", description, self.id, self.timing),
            None => write!(f, "{} ({})", self.id, self.timing),
        }
    }
}

/// A verifier that must have spoken.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, schemars::JsonSchema)]
pub struct VerificationRequirement {
    /// Which verifier.
    pub verifier: Verifier,
    /// Which claim it must have established.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub claim: Option<ClaimId>,
    /// What kind of artifact it must have been about.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject_kind: Option<ArtifactKind>,
    /// When it must have happened.
    pub timing: ObligationTiming,
}

impl VerificationRequirement {
    /// Parses the document form: a bare verifier name, or a mapping.
    pub fn from_node(node: &Node) -> Result<Self, ParseError> {
        match node {
            Node::Text(verifier) => Ok(Self {
                verifier: Verifier::parse(verifier)?,
                claim: None,
                subject_kind: None,
                timing: ObligationTiming::default_timing(),
            }),
            Node::Map(entries) => {
                let verifier = entries
                    .get("verifier")
                    .and_then(Node::as_text)
                    .ok_or_else(|| {
                        ParseError::shape("verification[]", "a `verifier` field", "no `verifier`")
                    })
                    .and_then(Verifier::parse)?;
                let claim = match entries.get("claim").and_then(Node::as_text) {
                    Some(claim) => Some(ClaimId::new(claim)?),
                    None => None,
                };
                let subject_kind = match entries.get("subject_kind").and_then(Node::as_text) {
                    Some(kind) => Some(ArtifactKind::parse(kind)?),
                    None => None,
                };
                let timing = match (entries.get("before"), entries.get("during")) {
                    (Some(before), _) => ObligationTiming::Before {
                        target: PhaseRef::from_node(before)?,
                    },
                    (None, Some(during)) => ObligationTiming::During {
                        target: PhaseRef::from_node(during)?,
                    },
                    (None, None) => ObligationTiming::default_timing(),
                };
                Ok(Self {
                    verifier,
                    claim,
                    subject_kind,
                    timing,
                })
            }
            other => Err(ParseError::shape(
                "verification[]",
                "a verifier name or a mapping",
                other.type_name(),
            )),
        }
    }
}

impl fmt::Display for VerificationRequirement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} must run", self.verifier)?;
        if let Some(claim) = &self.claim {
            write!(f, " for {claim}")?;
        }
        if let Some(kind) = &self.subject_kind {
            write!(f, " on a {kind}")?;
        }
        Ok(())
    }
}

/// What to do when a requirement is not met.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum FailurePolicy {
    /// Do not advance. The default, and the safe one.
    #[default]
    Block,
    /// Try again, then fall back.
    Retry {
        /// How many attempts in total.
        max_attempts: u32,
        /// What to do once attempts are exhausted.
        then: Box<FailurePolicy>,
    },
    /// Undo the change, once the stated precondition holds.
    Rollback {
        /// What must be true for a rollback to be possible, such as a previous revision.
        require: Predicate,
    },
    /// Hand off to a person or team.
    Escalate {
        /// Who to.
        to: String,
    },
    /// Stop the execution.
    Abort,
}

impl FailurePolicy {
    /// `true` when this policy, or one it falls back to, rolls back.
    pub fn involves_rollback(&self) -> bool {
        match self {
            Self::Rollback { .. } => true,
            Self::Retry { then, .. } => then.involves_rollback(),
            _ => false,
        }
    }

    /// The rollback precondition, when this policy rolls back.
    pub fn rollback_requirement(&self) -> Option<&Predicate> {
        match self {
            Self::Rollback { require } => Some(require),
            Self::Retry { then, .. } => then.rollback_requirement(),
            _ => None,
        }
    }

    /// Parses the document form.
    ///
    /// Accepts a bare action name (`block`, `abort`, `rollback`, `escalate`) or a mapping with
    /// an `action` key and the action's parameters.
    pub fn from_node(node: &Node) -> Result<Self, ParseError> {
        match node {
            Node::Text(action) => Self::from_action(action, &BTreeMap::new()),
            Node::Map(entries) => {
                let action = entries
                    .get("action")
                    .and_then(Node::as_text)
                    .ok_or_else(|| {
                        ParseError::shape("on_failure", "an `action` field", "no `action`")
                    })?;
                Self::from_action(action, entries)
            }
            other => Err(ParseError::shape(
                "on_failure",
                "an action name or a mapping",
                other.type_name(),
            )),
        }
    }

    fn from_action(action: &str, entries: &BTreeMap<String, Node>) -> Result<Self, ParseError> {
        match action {
            "block" => Ok(Self::Block),
            "abort" => Ok(Self::Abort),
            "retry" => {
                let max_attempts = match entries.get("max_attempts") {
                    Some(Node::Number(number)) if number.is_integral() && number.get() >= 1.0 => {
                        // Guarded above: integral and at least 1.
                        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                        let attempts = number.get() as u32;
                        attempts
                    }
                    None => 1,
                    Some(other) => {
                        return Err(ParseError::shape(
                            "on_failure.max_attempts",
                            "an integer of at least 1",
                            other.type_name(),
                        ))
                    }
                };
                let then = match entries.get("then") {
                    Some(then) => Box::new(Self::from_node(then)?),
                    None => Box::new(Self::Block),
                };
                Ok(Self::Retry { max_attempts, then })
            }
            "rollback" => {
                let require = match entries
                    .get("rollback")
                    .and_then(|rollback| rollback.as_map())
                    .and_then(|rollback| rollback.get("require"))
                    .or_else(|| entries.get("require"))
                {
                    Some(require) => Predicate::from_node(require)?,
                    None => Predicate::Always,
                };
                Ok(Self::Rollback { require })
            }
            "escalate" => {
                let to = entries
                    .get("to")
                    .and_then(Node::as_text)
                    .ok_or_else(|| {
                        ParseError::shape("on_failure.to", "who to escalate to", "no `to`")
                    })?
                    .to_owned();
                Ok(Self::Escalate { to })
            }
            other => Err(ParseError::shape(
                "on_failure.action",
                "block, abort, retry, rollback or escalate",
                other.to_owned(),
            )),
        }
    }

    /// Renders this policy back into document form.
    pub fn to_node(&self) -> Node {
        match self {
            Self::Block => Node::Text("block".to_owned()),
            Self::Abort => Node::Text("abort".to_owned()),
            Self::Escalate { to } => Node::Map(
                [
                    ("action".to_owned(), Node::Text("escalate".to_owned())),
                    ("to".to_owned(), Node::Text(to.clone())),
                ]
                .into(),
            ),
            Self::Retry { max_attempts, then } => Node::Map(
                [
                    ("action".to_owned(), Node::Text("retry".to_owned())),
                    (
                        "max_attempts".to_owned(),
                        Node::Number(crate::facts::Number::from(*max_attempts)),
                    ),
                    ("then".to_owned(), then.to_node()),
                ]
                .into(),
            ),
            Self::Rollback { require } => Node::Map(
                [
                    ("action".to_owned(), Node::Text("rollback".to_owned())),
                    (
                        "rollback".to_owned(),
                        Node::Map([("require".to_owned(), require.to_node())].into()),
                    ),
                ]
                .into(),
            ),
        }
    }
}

impl fmt::Display for FailurePolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Block => f.write_str("block"),
            Self::Abort => f.write_str("abort"),
            Self::Escalate { to } => write!(f, "escalate to {to}"),
            Self::Retry { max_attempts, then } => write!(f, "retry {max_attempts}x then {then}"),
            Self::Rollback { require } => write!(f, "roll back (requires {require})"),
        }
    }
}

impl serde::Serialize for FailurePolicy {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.to_node().serialize(serializer)
    }
}

impl<'de> serde::Deserialize<'de> for FailurePolicy {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let node = Node::deserialize(deserializer)?;
        Self::from_node(&node).map_err(serde::de::Error::custom)
    }
}

impl schemars::JsonSchema for FailurePolicy {
    fn schema_name() -> String {
        "FailurePolicy".to_owned()
    }

    fn json_schema(generator: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        let mut schema = schemars::schema::SchemaObject::default();
        schema.subschemas().any_of = Some(vec![
            <String>::json_schema(generator),
            <BTreeMap<String, Node>>::json_schema(generator),
        ]);
        schema.metadata().description = Some(
            "What to do when a requirement is not met: `block`, `abort`, or a mapping with \
             `action: retry|rollback|escalate` and that action's parameters."
                .to_owned(),
        );
        schema.into()
    }
}

/// An enforceable engineering rule.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Principle {
    /// Its identifier.
    pub id: PrincipleId,
    /// Its major version.
    pub version: MajorVersion,
    /// A short human title.
    pub title: String,
    /// What it is for.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    /// When it applies.
    pub applicability: Predicate,
    /// What it obliges, and when.
    pub obligations: Vec<Obligation>,
    /// Evidence that must exist by completion.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub evidence: Vec<EvidenceRequirement>,
    /// Verifiers that must have spoken.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub verification: Vec<VerificationRequirement>,
    /// What it takes away or puts behind approval.
    #[serde(skip_serializing_if = "CapabilityPolicy::is_empty")]
    pub capabilities: CapabilityPolicy,
    /// What happens when one of its requirements is not met.
    pub failure_policy: FailurePolicy,
}

impl Principle {
    /// `true` when this principle applies to a task with these facts.
    pub fn applies(&self, facts: &dyn crate::facts::FactSource) -> bool {
        // An unobservable applicability condition applies: a principle that cannot rule
        // itself out stays in force, because the alternative is silently dropping a rule.
        self.applicability.evaluate(facts) != crate::predicate::Truth::False
    }

    /// Obligations that must hold at every transition, plus those timed at `timing`.
    pub fn obligations_for(
        &self,
        predicate: impl Fn(&ObligationTiming) -> bool,
    ) -> Vec<&Obligation> {
        self.obligations
            .iter()
            .filter(|obligation| predicate(&obligation.timing))
            .collect()
    }

    /// Every phase this principle's obligations refer to.
    pub fn referenced_phases(&self) -> Vec<&PhaseId> {
        let timings = self
            .obligations
            .iter()
            .map(|obligation| &obligation.timing)
            .chain(
                self.verification
                    .iter()
                    .map(|requirement| &requirement.timing),
            );

        let mut phases: Vec<&PhaseId> = Vec::new();
        for timing in timings {
            if let Some(PhaseRef::Phase(phase)) = timing.target() {
                if !phases.contains(&phase) {
                    phases.push(phase);
                }
            }
        }
        phases
    }

    /// Every state this principle's obligations refer to by name.
    pub fn referenced_states(&self) -> Vec<&StateId> {
        let mut states = Vec::new();
        for obligation in &self.obligations {
            if let Some(PhaseRef::State(state)) = obligation.timing.target() {
                if !states.contains(&state) {
                    states.push(state);
                }
            }
        }
        states
    }
}

/// A principle document, as parsed.
#[derive(Debug, Clone, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RawPrinciple {
    /// Its identifier.
    pub id: PrincipleId,
    /// Its major version.
    #[serde(default = "default_version")]
    pub version: MajorVersion,
    /// A short human title.
    pub title: String,
    /// What it is for.
    #[serde(default)]
    pub summary: Option<String>,
    /// When it applies; omitted means always.
    #[serde(default, alias = "applicability")]
    pub applies_when: Option<Predicate>,
    /// Timed requirements, keyed by `always`, `before_<phase>`, `during_<phase>`, or given as
    /// requirement keys alongside a `before`/`during` selector.
    #[serde(default)]
    pub requires: Option<Node>,
    /// Explicitly identified obligations.
    #[serde(default)]
    pub obligations: Vec<RawObligation>,
    /// Evidence that must exist by completion.
    #[serde(default)]
    pub evidence: Vec<EvidenceRequirement>,
    /// Verifiers that must have spoken.
    #[serde(default)]
    pub verification: Vec<Node>,
    /// What it takes away or puts behind approval.
    #[serde(default)]
    pub capabilities: CapabilityPolicy,
    /// What happens when one of its requirements is not met.
    #[serde(default, alias = "failure_policy")]
    pub on_failure: Option<FailurePolicy>,
}

/// Serde default for a document's major version.
fn default_version() -> MajorVersion {
    MajorVersion::V1
}

/// An explicitly identified obligation, as parsed.
#[derive(Debug, Clone, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RawObligation {
    /// Its identifier within the principle.
    pub id: String,
    /// What it is for.
    #[serde(default)]
    pub description: Option<String>,
    /// Before entering this phase or state.
    #[serde(default)]
    pub before: Option<Node>,
    /// While in this phase or state.
    #[serde(default)]
    pub during: Option<Node>,
    /// Checked at every transition.
    #[serde(default)]
    pub always: bool,
    /// What must hold.
    #[serde(default, alias = "require")]
    pub requires: RequirementSet,
}

impl TryFrom<RawPrinciple> for Principle {
    type Error = ValidationErrors;

    fn try_from(raw: RawPrinciple) -> Result<Self, Self::Error> {
        let mut errors = ValidationErrors::new();
        let mut obligations: Vec<Obligation> = Vec::new();

        if let Some(node) = &raw.requires {
            match parse_timed_requirements(&raw.id, node) {
                Ok(parsed) => obligations.extend(parsed),
                Err(error) => errors.push(ValidationError::new(
                    ValidationCode::UnknownPhase,
                    format!("principle {}.requires", raw.id),
                    error.to_string(),
                )),
            }
        }

        for (index, obligation) in raw.obligations.iter().enumerate() {
            match build_obligation(&raw.id, index, obligation) {
                Ok(parsed) => obligations.push(parsed),
                Err(error) => errors.push(ValidationError::new(
                    ValidationCode::UnknownPhase,
                    format!("principle {}.obligations[{index}]", raw.id),
                    error.to_string(),
                )),
            }
        }

        deduplicate_ids(&mut obligations);

        let mut verification = Vec::new();
        for (index, node) in raw.verification.iter().enumerate() {
            match VerificationRequirement::from_node(node) {
                Ok(parsed) => verification.push(parsed),
                Err(error) => errors.push(ValidationError::new(
                    ValidationCode::NoVerifierForEvidence,
                    format!("principle {}.verification[{index}]", raw.id),
                    error.to_string(),
                )),
            }
        }

        if obligations.is_empty()
            && raw.evidence.is_empty()
            && verification.is_empty()
            && raw.capabilities.is_empty()
        {
            errors.push(
                ValidationError::new(
                    ValidationCode::EmptyDeclaration,
                    format!("principle {}", raw.id),
                    "declares no obligations, evidence, verification or capability policy, so it \
                     cannot change any outcome"
                        .to_owned(),
                )
                .with_hint("give the principle something to enforce, or delete it"),
            );
        }

        let principle = Self {
            id: raw.id,
            version: raw.version,
            title: raw.title,
            summary: raw.summary,
            applicability: raw.applies_when.unwrap_or(Predicate::Always),
            obligations,
            evidence: raw.evidence,
            verification,
            capabilities: raw.capabilities,
            failure_policy: raw.on_failure.unwrap_or_default(),
        };
        errors.into_result(principle)
    }
}

/// Parses the `requires` mapping into timed obligations.
fn parse_timed_requirements(
    principle: &PrincipleId,
    node: &Node,
) -> Result<Vec<Obligation>, ParseError> {
    let mut obligations = Vec::new();

    let Some(entries) = node.as_map() else {
        // `requires: [...]` with no timing: everything is owed before completion.
        let requires = RequirementSet::from_node(node)?;
        if !requires.is_empty() {
            obligations.push(build(
                principle,
                ObligationTiming::default_timing(),
                requires,
            ));
        }
        return Ok(obligations);
    };

    let mut selector: Option<ObligationTiming> = None;
    let mut untimed: BTreeMap<String, Node> = BTreeMap::new();

    for (key, value) in entries {
        if key == "before" {
            selector = Some(ObligationTiming::Before {
                target: PhaseRef::from_node(value)?,
            });
            continue;
        }
        if key == "during" {
            selector = Some(ObligationTiming::During {
                target: PhaseRef::from_node(value)?,
            });
            continue;
        }
        if key == "always" {
            let requires = RequirementSet::from_node(value)?;
            obligations.push(build(principle, ObligationTiming::Always, requires));
            continue;
        }
        if let Some(phase) = key.strip_prefix("before_") {
            let timing = ObligationTiming::Before {
                target: PhaseRef::Phase(PhaseId::new(phase.replace('_', "-"))?),
            };
            obligations.push(build(principle, timing, RequirementSet::from_node(value)?));
            continue;
        }
        if let Some(phase) = key.strip_prefix("during_") {
            let timing = ObligationTiming::During {
                target: PhaseRef::Phase(PhaseId::new(phase.replace('_', "-"))?),
            };
            obligations.push(build(principle, timing, RequirementSet::from_node(value)?));
            continue;
        }
        untimed.insert(key.clone(), value.clone());
    }

    if !untimed.is_empty() {
        let requires = RequirementSet::from_node(&Node::Map(untimed))?;
        let timing = selector
            .clone()
            .unwrap_or_else(ObligationTiming::default_timing);
        obligations.push(build(principle, timing, requires));
    } else if let Some(timing) = selector {
        // `before:` on its own says when but not what; that is almost certainly a mistake.
        return Err(ParseError::shape(
            format!("principle {principle}.requires"),
            "requirement keys alongside the timing selector",
            format!("only `{timing}`"),
        ));
    }

    Ok(obligations)
}

/// Builds an obligation with a generated identifier.
fn build(
    principle: &PrincipleId,
    timing: ObligationTiming,
    requires: RequirementSet,
) -> Obligation {
    let id = ObligationId::new(format!("{}/{}", principle, timing.slug()))
        .unwrap_or_else(|error| panic!("generated obligation id is invalid: {error}"));
    Obligation {
        id,
        description: None,
        timing,
        requires,
    }
}

/// Builds an explicitly identified obligation.
fn build_obligation(
    principle: &PrincipleId,
    index: usize,
    raw: &RawObligation,
) -> Result<Obligation, ParseError> {
    let timing = match (raw.always, &raw.before, &raw.during) {
        (true, None, None) => ObligationTiming::Always,
        (false, Some(before), None) => ObligationTiming::Before {
            target: PhaseRef::from_node(before)?,
        },
        (false, None, Some(during)) => ObligationTiming::During {
            target: PhaseRef::from_node(during)?,
        },
        (false, None, None) => ObligationTiming::default_timing(),
        _ => {
            return Err(ParseError::shape(
                format!("principle {principle}.obligations[{index}]"),
                "exactly one of `always`, `before` or `during`",
                "more than one",
            ))
        }
    };
    let id = ObligationId::new(format!("{principle}/{}", raw.id))?;
    Ok(Obligation {
        id,
        description: raw.description.clone(),
        timing,
        requires: raw.requires.clone(),
    })
}

/// Makes generated obligation identifiers unique by appending an index to collisions.
fn deduplicate_ids(obligations: &mut [Obligation]) {
    let mut seen: BTreeMap<String, usize> = BTreeMap::new();
    for obligation in obligations.iter_mut() {
        let key = obligation.id.to_string();
        let count = seen.entry(key.clone()).or_default();
        *count += 1;
        if *count > 1 {
            obligation.id = ObligationId::new(format!("{key}-{}", *count))
                .unwrap_or_else(|error| panic!("generated obligation id is invalid: {error}"));
        }
    }
}

/// Which principles a task adds to or removes from its profile.
#[derive(
    Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct PrincipleOverrides {
    /// Principles to add.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub add: Vec<PrincipleRef>,
    /// Principles to drop.
    ///
    /// Removal is recorded in the audit trail: dropping `mutation-testing` for one task is a
    /// decision someone should be able to find later.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub remove: Vec<PrincipleId>,
}

impl PrincipleOverrides {
    /// `true` when nothing is overridden.
    pub fn is_empty(&self) -> bool {
        self.add.is_empty() && self.remove.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn principle(yaml: &str) -> Principle {
        let raw: RawPrinciple = serde_yaml::from_str(yaml).expect("document parses");
        Principle::try_from(raw).expect("document validates")
    }

    #[test]
    fn parses_the_phase_keyed_requires_form() {
        let parsed = principle(
            r"
id: test-driven
title: Test-driven development
applies_when:
  task.kind: {any_of: [feature, bugfix]}
requires:
  before_implementation:
    - test.exists
    - test.first_result == failed
  before_completion:
    - tests.unit.failed == 0
evidence:
  - test_result
  - source_diff
",
        );

        assert_eq!(parsed.obligations.len(), 2);
        // Obligations are ordered by their timing key, which keeps output deterministic
        // regardless of how the document happened to be written.
        let ids: Vec<String> = parsed
            .obligations
            .iter()
            .map(|obligation| obligation.id.to_string())
            .collect();
        assert_eq!(
            ids,
            vec![
                "test-driven/before-completion",
                "test-driven/before-implementation"
            ]
        );
        let implementation = parsed
            .obligations
            .iter()
            .find(|obligation| {
                obligation.timing
                    == ObligationTiming::Before {
                        target: PhaseRef::Phase(PhaseId::new("implementation").expect("phase")),
                    }
            })
            .expect("the implementation obligation is present");
        assert_eq!(implementation.requires.predicates.len(), 2);
        assert_eq!(parsed.evidence.len(), 2);
    }

    #[test]
    fn parses_the_artifact_requires_form_with_a_before_selector() {
        let parsed = principle(
            r"
id: spec-driven
title: Specification before implementation
requires:
  before:
    state: implement
  artifacts:
    - kind: specification
      status: approved
",
        );

        assert_eq!(parsed.obligations.len(), 1);
        assert_eq!(
            parsed.obligations[0].timing,
            ObligationTiming::Before {
                target: PhaseRef::State(StateId::new("implement").expect("state"))
            }
        );
        assert_eq!(parsed.obligations[0].requires.artifacts.len(), 1);
    }

    #[test]
    fn requirements_with_no_timing_default_to_before_completion() {
        let parsed = principle(
            r"
id: architecture-decision-records
title: Record durable decisions
applies_when:
  change.architectural: true
requires:
  artifacts:
    - kind: architecture-decision-record
",
        );
        assert_eq!(
            parsed.obligations[0].timing,
            ObligationTiming::default_timing()
        );
    }

    #[test]
    fn a_timing_selector_without_requirements_is_rejected() {
        let raw: RawPrinciple = serde_yaml::from_str(
            r"
id: broken
title: Broken
requires:
  before:
    state: implement
",
        )
        .expect("document parses");
        let errors = Principle::try_from(raw).expect_err("no requirements");
        assert!(errors.to_string().contains("requirement keys"), "{errors}");
    }

    #[test]
    fn a_principle_that_enforces_nothing_is_rejected() {
        let raw: RawPrinciple = serde_yaml::from_str(
            r"
id: vibes
title: Good vibes
summary: Feels right.
",
        )
        .expect("document parses");
        let errors = Principle::try_from(raw).expect_err("nothing enforced");
        assert!(errors.contains(ValidationCode::EmptyDeclaration));
        assert!(
            errors.to_string().contains("cannot change any outcome"),
            "{errors}"
        );
    }

    #[test]
    fn colliding_generated_obligation_ids_are_made_unique() {
        let parsed = principle(
            r"
id: layered
title: Layered
requires:
  before_completion:
    - a.done
obligations:
  - id: before-completion
    requires:
      - b.done
",
        );
        let ids: Vec<String> = parsed
            .obligations
            .iter()
            .map(|obligation| obligation.id.to_string())
            .collect();
        assert_eq!(
            ids,
            vec!["layered/before-completion", "layered/before-completion-2"]
        );
    }

    #[test]
    fn parses_failure_policies_in_both_forms() {
        assert_eq!(
            FailurePolicy::from_node(&Node::from("block")).expect("parses"),
            FailurePolicy::Block
        );

        let rollback: FailurePolicy = serde_yaml::from_str(
            r"
action: rollback
rollback:
  require:
    - deployment.previous_revision.exists
",
        )
        .expect("parses");
        assert!(rollback.involves_rollback());
        assert_eq!(
            rollback.rollback_requirement().map(ToString::to_string),
            Some("deployment.previous_revision.exists".to_owned())
        );

        let retry: FailurePolicy =
            serde_yaml::from_str("action: retry\nmax_attempts: 3\nthen: abort").expect("parses");
        assert_eq!(
            retry,
            FailurePolicy::Retry {
                max_attempts: 3,
                then: Box::new(FailurePolicy::Abort)
            }
        );
        assert!(!retry.involves_rollback());
    }

    #[test]
    fn an_unobservable_applicability_condition_keeps_the_principle_in_force() {
        let parsed = principle(
            r"
id: least-privilege
title: Least privilege
applies_when:
  change.production: true
requires:
  always:
    - capability.audit.recorded
",
        );
        let empty = crate::facts::FactStore::new();
        assert!(
            parsed.applies(&empty),
            "a principle must not fall away just because nothing has been observed yet"
        );

        let mut ruled_out = crate::facts::FactStore::new();
        ruled_out.set_path("change.production", crate::facts::FactValue::bool(false));
        assert!(!parsed.applies(&ruled_out));
    }
}
