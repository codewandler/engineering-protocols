//! The step map: the half of a run a workflow deliberately does not state.
//!
//! A step map is a document like the four already in the tree — parsed into a `Raw*` type,
//! validated into a domain type, published as a generated schema, cross-validated against the
//! documents it names. It says, per workflow state, what a harness should *do* while it is there.
//! Order inside a state is the map author's; order **between** states is the workflow's, and
//! nothing here can override it.
//!
//! # Cross-validation runs in two phases, because the two halves are knowable at different times
//!
//! | phase | what it checks | when |
//! |---|---|---|
//! | [`cross_validate`](StepMap::cross_validate) | every state the map names is a state of the workflow; every **named** verifier a step names can produce the kind it claims | at load, against the registry |
//! | [`check_run`](StepMap::check_run) | every evidence kind a step declares is declared by the protocol in force; the resolved workflow's id and version equal the pin | at run start, against the resolved plan |
//!
//! The second is not a duplicate of the first and cannot be folded into it: the protocol in force
//! comes from the **task**, which no document loader has seen. A loader that guessed would let a
//! map validate and then fail at the transition that needed the evidence — the exact failure the
//! check exists to prevent.
//!
//! **A named verifier is checked; an external tool is not.** `kinds_for_verifier` filters on
//! `default_verifiers()`, which is a table of **defaults** rather than of constraints — `diff`
//! defaults to `[compiler, static-analyzer]`, so a diff produced by `git` would be refused for a
//! defect that is not one. `Verifier::ExternalTool` is what `Verifier::parse` falls through to for
//! anything unrecognised, it appears in no row of that table, and its kind is still checked at run
//! start against the protocol. Review finding **F5**.
//!
//! # An `llm` step cannot carry an evidence block, and the type is what makes that true
//!
//! Not a validation rule that could be relaxed later: [`LlmStep`] has no evidence field. An
//! agent's own statement never satisfies an independence requirement, so a step kind that could
//! mint evidence from a model's output would be the single change that unpicks the whole loop.
//! Anything an `llm` step is supposed to have achieved that is *checkable* is observed by a
//! subsequent [`CommandStep`]: the model writes the code, and `cargo test` says whether it works.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::str::FromStr;

use aep_domain::error::{ParseError, ValidationCode, ValidationError, ValidationErrors};
use aep_domain::evidence::{EvidenceKind, TestSuite};
use aep_domain::ids::{StateId, SubjectRef, ToolRef};
use aep_domain::protocol::Protocol;
use aep_domain::verification::Verifier;
use aep_domain::version::WorkflowRef;
use aep_domain::workflow::Workflow;

use crate::digest::digest_of_canonical;
use crate::pin::PinnedWorkflowRef;
use crate::STEP_MAP_FORMAT;

/// How many times a state may be entered before the driver stops, when the map does not say.
///
/// `adp/default` has a deliberate `verify → implement` back-edge, and the workflow's own comment
/// explains why: *"a workflow that can only go forwards is a lie about how engineering works"*. So
/// a driver must be able to go round again — and must not go round forever.
pub const DEFAULT_VISIT_BUDGET: u32 = 3;

/// How many times a `command` step is retried before its budget is spent, when the map is silent.
///
/// A retry is for *a process died*, not for *the suite is red*: a suite that ran and failed
/// produced a verdict, and the verdict is submitted rather than retried.
pub const DEFAULT_COMMAND_RETRIES: u32 = 2;

/// How many times an `llm` step is retried.
///
/// Once, and not configurable: a model call that errored is worth one more attempt, and a second
/// one is a token budget nobody stated.
pub const LLM_RETRIES: u32 = 1;

/// Identifier of a step map, such as `development/default`.
///
/// The same charset rule as a workflow identifier — lowercase kebab segments joined by `.` or `/`,
/// never ending in a numeric segment, because that form is reserved for a version reference.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct StepMapId(String);

impl StepMapId {
    /// The pattern published in generated JSON Schema.
    pub const PATTERN: &'static str = "^[a-z][a-z0-9]*(-[a-z0-9]+)*([./][a-z0-9]+(-[a-z0-9]+)*)*$";

    /// Parses and validates an identifier.
    pub fn new(value: impl Into<String>) -> Result<Self, ParseError> {
        let value = value.into();
        let reject = |reason: String| Err(ParseError::identifier("step map", &value, reason));
        if value.is_empty() {
            return reject("must not be empty".to_owned());
        }
        if value.len() > 200 {
            return reject(format!(
                "must be at most 200 characters, got {}",
                value.len()
            ));
        }
        for segment in value.split(['.', '/', '-']) {
            if segment.is_empty() {
                return reject(
                    "has an empty segment; separators (./-) must not lead, trail or repeat"
                        .to_owned(),
                );
            }
            if let Some(bad) = segment
                .chars()
                .find(|c| !(c.is_ascii_lowercase() || c.is_ascii_digit()))
            {
                return reject(format!("contains disallowed character {bad:?}"));
            }
        }
        if !value.starts_with(|c: char| c.is_ascii_lowercase()) {
            return reject("must start with a lowercase letter".to_owned());
        }
        if value
            .rsplit(['.', '/'])
            .next()
            .is_some_and(|last| last.chars().all(|c| c.is_ascii_digit()))
        {
            return reject(
                "must not end in a numeric segment; that form is reserved for version references"
                    .to_owned(),
            );
        }
        Ok(Self(value))
    }

    /// The identifier as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for StepMapId {
    type Err = ParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

impl TryFrom<String> for StepMapId {
    type Error = ParseError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<StepMapId> for String {
    fn from(value: StepMapId) -> Self {
        value.0
    }
}

impl fmt::Display for StepMapId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl fmt::Debug for StepMapId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "StepMapId({:?})", self.0)
    }
}

impl schemars::JsonSchema for StepMapId {
    fn schema_name() -> String {
        "StepMapId".to_owned()
    }

    fn json_schema(_: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        let mut schema = schemars::schema::SchemaObject {
            instance_type: Some(schemars::schema::InstanceType::String.into()),
            ..Default::default()
        };
        schema.string().pattern = Some(Self::PATTERN.to_owned());
        schema.metadata().description =
            Some("Identifier of a step map, such as `development/default`.".to_owned());
        schema.into()
    }
}

/// A step map as it is written, before anything has checked what it claims.
#[derive(Debug, Clone, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RawStepMap {
    /// The format version the document says it is written in.
    pub format: String,
    /// What this map is, such as `development/default`.
    pub id: StepMapId,
    /// The workflow it is written against, pinned to a major version.
    ///
    /// Read as a [`WorkflowRef`] so an unpinned one reaches validation and is refused there,
    /// **accumulating** with every other problem in the document rather than short-circuiting as a
    /// deserialization error would. The published schema is
    /// [`PinnedWorkflowRef`]'s, which is the strict one — the two
    /// halves of review finding F6.
    #[schemars(with = "PinnedWorkflowRef")]
    pub workflow: WorkflowRef,
    /// A human sentence for a report's heading.
    #[serde(default)]
    pub title: Option<String>,
    /// What to do in each state, by state name.
    #[serde(default)]
    pub states: BTreeMap<StateId, RawStateSteps>,
}

/// One state's steps, as written.
#[derive(Debug, Clone, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RawStateSteps {
    /// How many times this state may be entered before the run stops.
    #[serde(default)]
    pub visit_budget: Option<u32>,
    /// The steps, in the order the author wants them run.
    #[serde(default)]
    pub steps: Vec<RawStep>,
}

/// One step, as written.
#[derive(Debug, Clone, serde::Deserialize, schemars::JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum RawStep {
    /// Run a program and map its result to evidence.
    Command(RawCommandStep),
    /// Ask a model, with a tool set the protocol derived.
    Llm(RawLlmStep),
    /// Stop and hand the run to a person.
    Operator(RawOperatorStep),
}

/// A `command` step, as written.
#[derive(Debug, Clone, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RawCommandStep {
    /// The program and its arguments.
    pub run: Vec<String>,
    /// What this step is for, in one line.
    #[serde(default)]
    pub description: Option<String>,
    /// What to record when the program produces a verdict.
    #[serde(default)]
    pub evidence: Option<RawEvidenceMapping>,
    /// How many times to retry a step that could not run at all.
    #[serde(default)]
    pub retries: Option<u32>,
}

/// An `llm` step, as written.
///
/// It has no `evidence` key and cannot be given one: see the module documentation.
#[derive(Debug, Clone, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RawLlmStep {
    /// What the model is asked to do.
    pub prompt: String,
    /// Skills the harness should make available, by name.
    #[serde(default)]
    pub skills: Vec<String>,
    /// Which harness runs it. `claude-code` when the document is silent.
    #[serde(default)]
    pub harness: Option<String>,
    /// What this step is for, in one line.
    #[serde(default)]
    pub description: Option<String>,
}

/// An `operator` step, as written.
#[derive(Debug, Clone, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RawOperatorStep {
    /// What the person is being asked for.
    pub prompt: String,
    /// What this step is for, in one line.
    #[serde(default)]
    pub description: Option<String>,
}

/// How a command's result becomes evidence, as written.
#[derive(Debug, Clone, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RawEvidenceMapping {
    /// Which kind of evidence the program establishes.
    pub kind: EvidenceKind,
    /// Which class of verifier it is.
    pub verifier: Verifier,
    /// Which suite ran, for a `test_result`.
    #[serde(default)]
    pub suite: Option<TestSuite>,
    /// What the evidence is about.
    #[serde(default)]
    pub subject: Option<SubjectRef>,
    /// The tool that produced it, for provenance.
    #[serde(default)]
    pub tool: Option<ToolRef>,
    /// Where the program writes the evidence record, when it writes one itself.
    #[serde(default)]
    pub record: Option<String>,
}

/// Which sort of step this is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StepKind {
    /// Runs a program.
    Command,
    /// Asks a model.
    Llm,
    /// Stops for a person.
    Operator,
}

impl StepKind {
    /// The kind as written in documents and reports.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Command => "command",
            Self::Llm => "llm",
            Self::Operator => "operator",
        }
    }
}

impl fmt::Display for StepKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A validated step.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Step {
    /// Run a program and map its result to evidence.
    Command(CommandStep),
    /// Ask a model, with a tool set the protocol derived.
    Llm(LlmStep),
    /// Stop and hand the run to a person.
    Operator(OperatorStep),
}

impl Step {
    /// Which sort of step this is.
    pub fn kind(&self) -> StepKind {
        match self {
            Self::Command(_) => StepKind::Command,
            Self::Llm(_) => StepKind::Llm,
            Self::Operator(_) => StepKind::Operator,
        }
    }

    /// How many times this step may be retried after producing no verdict at all.
    ///
    /// Per step kind, as decision D5 takes it: a `command` step retries because a process died, an
    /// `llm` step retries once because a model call errored, and an `operator` step never retries
    /// because a person is not a flaky dependency.
    pub fn retry_budget(&self) -> u32 {
        match self {
            Self::Command(step) => step.retries,
            Self::Llm(_) => LLM_RETRIES,
            Self::Operator(_) => 0,
        }
    }

    /// One line naming the step, for a report.
    pub fn label(&self) -> String {
        match self {
            Self::Command(step) => step
                .description
                .clone()
                .unwrap_or_else(|| step.run.join(" ")),
            Self::Llm(step) => step
                .description
                .clone()
                .unwrap_or_else(|| step.prompt.clone()),
            Self::Operator(step) => step
                .description
                .clone()
                .unwrap_or_else(|| step.prompt.clone()),
        }
    }
}

/// A validated `command` step.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct CommandStep {
    /// The program and its arguments, never empty.
    pub run: Vec<String>,
    /// What this step is for, in one line.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// What to record when the program produces a verdict.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evidence: Option<EvidenceMapping>,
    /// How many times to retry a step that could not run at all.
    pub retries: u32,
}

impl CommandStep {
    /// The names a `{placeholder}` in [`Self::run`] or in `evidence.record` may have.
    ///
    /// Two, and both are things only the run knows: where its directory is, and which transcript
    /// the `llm` step before this one wrote. A map is a document in the repository and a run
    /// directory is allocated when the run starts, so a step that has to name one has no way to
    /// write it down — which is why `protocol trace check` could not be a step of a map before
    /// these existed.
    ///
    /// The list is closed and an unknown name is refused at load, because the alternative is a
    /// command line containing the literal characters `{transcirpt}` and a checker reporting that
    /// it cannot read that file, halfway through a run.
    pub const PLACEHOLDERS: &'static [&'static str] = &["run_directory", "transcript"];

    /// The program, which a validated step always has.
    pub fn program(&self) -> &str {
        self.run.first().map_or("", String::as_str)
    }

    /// The arguments after the program.
    pub fn arguments(&self) -> &[String] {
        self.run.get(1..).unwrap_or_default()
    }

    /// Every word of this step that a placeholder may appear in.
    pub fn expandable(&self) -> impl Iterator<Item = &str> {
        self.run.iter().map(String::as_str).chain(
            self.evidence
                .as_ref()
                .and_then(|mapping| mapping.record.as_deref()),
        )
    }
}

/// Every `{placeholder}` in `word`, in the order they appear.
///
/// A placeholder is `{` then one or more of `a-z` and `_` then `}` — deliberately narrow, so that
/// the argument `{}` that `find -exec` takes, and a jq program's `{a: .b}`, are ordinary text
/// rather than a refusal. Anything that *does* match the shape is checked against
/// [`CommandStep::PLACEHOLDERS`], so a misspelling is refused rather than passed to a program as
/// literal braces.
pub fn placeholders_in(word: &str) -> Vec<&str> {
    let mut found = Vec::new();
    let mut rest = word;
    while let Some(open) = rest.find('{') {
        let after = &rest[open + 1..];
        let Some(close) = after.find('}') else {
            break;
        };
        let name = &after[..close];
        if !name.is_empty() && name.chars().all(|c| c.is_ascii_lowercase() || c == '_') {
            found.push(name);
        }
        rest = &after[close + 1..];
    }
    found
}

/// A validated `llm` step.
///
/// There is no `evidence` field, and that absence is the mechanism rather than an omission.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct LlmStep {
    /// What the model is asked to do.
    pub prompt: String,
    /// Skills the harness makes available, by name.
    pub skills: Vec<String>,
    /// Which harness runs it.
    ///
    /// The selection seam § 4.9 point 3 names: a second adapter is a second free function chosen by
    /// this name, not a trait added before there is a second implementation to design it against.
    pub harness: String,
    /// What this step is for, in one line.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl LlmStep {
    /// The harness a step runs under when the document does not say.
    pub const DEFAULT_HARNESS: &'static str = "claude-code";
}

/// A validated `operator` step.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct OperatorStep {
    /// What the person is being asked for.
    pub prompt: String,
    /// What this step is for, in one line.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// A validated evidence mapping.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct EvidenceMapping {
    /// Which kind of evidence the program establishes.
    pub kind: EvidenceKind,
    /// Which class of verifier it is.
    pub verifier: Verifier,
    /// Which suite ran, for a `test_result`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suite: Option<TestSuite>,
    /// What the evidence is about.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject: Option<SubjectRef>,
    /// The tool that produced it, for provenance.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool: Option<ToolRef>,
    /// Where the program writes the evidence record, when it writes one itself.
    ///
    /// Without it a `command` step's evidence is minted from the exit status, which is why
    /// [`Self::MINTABLE`] is short: an exit status carries *did it produce a verdict, and was it
    /// yes or no*, and a kind whose record holds digests and counts cannot be built from that
    /// without inventing them. With it the driver mints nothing — it reads the document the
    /// verifier wrote, exactly as `protocol evaluate --evidence` reads one, and submits what the
    /// document says. `protocol trace evidence` is the case it was added for: the record carries
    /// the specification's digest, the transcript's digest and three counts, all of which are
    /// facts about a check this process did not run.
    ///
    /// The path admits the same placeholders [`CommandStep::run`] does.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub record: Option<String>,
}

impl EvidenceMapping {
    /// The evidence kinds a driver can mint from a program's exit status.
    ///
    /// A closed set, and short on purpose. Everything here is establishable from *did the verifier
    /// produce a verdict, and was it yes or no* — which is all an exit status carries. A kind whose
    /// payload needs numbers nobody read out of the output (a metric, a deployment, an approval)
    /// is refused at load rather than minted with invented values, because a fabricated count is
    /// worse than a missing record: the engine cannot tell it apart from an observed one.
    pub const MINTABLE: &'static [EvidenceKind] = &[
        EvidenceKind::TestResult,
        EvidenceKind::StaticAnalysis,
        EvidenceKind::ContractResult,
        EvidenceKind::Diff,
    ];
}

/// One state's steps, validated.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct StateSteps {
    /// How many times this state may be entered before the run stops.
    pub visit_budget: u32,
    /// The steps, in the order the author wrote them.
    pub steps: Vec<Step>,
}

/// A validated step map.
///
/// Obtained only through [`TryFrom<RawStepMap>`], which is what makes possession of one the
/// evidence that its format version, its pin and its steps were checked.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct StepMap {
    /// What this map is.
    pub id: StepMapId,
    /// The workflow it is written against, pinned to a major version.
    pub workflow: PinnedWorkflowRef,
    /// A human sentence for a report's heading.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// What to do in each state.
    pub states: BTreeMap<StateId, StateSteps>,
}

impl StepMap {
    /// The format version this map is written in.
    ///
    /// A constant rather than a field: a value of this type is one whose version was checked, and
    /// a field holding it would be a second place for it to be wrong.
    pub fn format(&self) -> &'static str {
        STEP_MAP_FORMAT
    }

    /// The steps of one state, or nothing when the map is silent about it.
    ///
    /// A state with no steps is not an error: a workflow state whose transition is unguarded needs
    /// no work done in it, and a map that had to say so for every such state would be noise.
    pub fn steps_for(&self, state: &StateId) -> &[Step] {
        self.states
            .get(state)
            .map_or(&[], |entry| entry.steps.as_slice())
    }

    /// How many times `state` may be entered before the run stops.
    pub fn visit_budget(&self, state: &StateId) -> u32 {
        self.states
            .get(state)
            .map_or(DEFAULT_VISIT_BUDGET, |entry| entry.visit_budget)
    }

    /// Every evidence kind any step of this map declares.
    pub fn declared_evidence_kinds(&self) -> BTreeSet<EvidenceKind> {
        self.states
            .values()
            .flat_map(|entry| entry.steps.iter())
            .filter_map(|step| match step {
                Step::Command(command) => command.evidence.as_ref().map(|mapping| mapping.kind),
                Step::Llm(_) | Step::Operator(_) => None,
            })
            .collect()
    }

    /// The digest of this map's canonical JSON, which the cursor pins a run to.
    pub fn digest(&self) -> String {
        digest_of_canonical(self)
    }

    /// Phase one: the map against the workflow it names, at load time.
    ///
    /// Accumulating, and it does not stop at the first bad state: a map with four renamed states
    /// reports four, because fixing a document one error per run is how a validation step becomes
    /// something people avoid running.
    pub fn cross_validate(&self, workflow: &Workflow) -> ValidationErrors {
        let mut errors = ValidationErrors::new();

        if workflow.id != *self.workflow.id() {
            errors.push(ValidationError::new(
                ValidationCode::UnknownWorkflow,
                format!("driver-steps[{}].workflow", self.id),
                format!(
                    "the map is written against `{}` and was checked against `{}`",
                    self.workflow, workflow.id
                ),
            ));
        } else if !self.workflow.accepts(workflow.version) {
            errors.push(
                ValidationError::new(
                    ValidationCode::VersionMismatch,
                    format!("driver-steps[{}].workflow", self.id),
                    format!(
                        "the map pins `{}` and the workflow in the tree is at version {}",
                        self.workflow, workflow.version
                    ),
                )
                .with_hint(
                    "a major version exists because the change could not be expressed additively, \
                     so the map is rewritten against the new state graph rather than migrated",
                ),
            );
        }

        let known: Vec<&str> = workflow.states.keys().map(StateId::as_str).collect();
        for state in self.states.keys() {
            if !workflow.states.contains_key(state) {
                errors.push(
                    ValidationError::new(
                        ValidationCode::UnknownState,
                        format!("driver-steps[{}].states.{state}", self.id),
                        format!("`{state}` is not a state of workflow `{}`", workflow.id),
                    )
                    .with_hint(format!("the workflow declares: {}", known.join(", "))),
                );
            }
        }

        for (state, entry) in &self.states {
            for (index, step) in entry.steps.iter().enumerate() {
                let Step::Command(command) = step else {
                    continue;
                };
                let Some(mapping) = &command.evidence else {
                    continue;
                };
                // An external tool is exempt: `default_verifiers` is a table of defaults, not of
                // constraints, and it has no row for a tool the protocol has never heard of.
                // Review finding F5.
                if !Verifier::NAMED.contains(&mapping.verifier) {
                    continue;
                }
                if !mapping.kind.default_verifiers().contains(&mapping.verifier) {
                    let can: Vec<&str> = EvidenceKind::ALL
                        .iter()
                        .filter(|kind| kind.default_verifiers().contains(&mapping.verifier))
                        .map(|kind| kind.as_str())
                        .collect();
                    errors.push(
                        ValidationError::new(
                            ValidationCode::NoVerifierForEvidence,
                            format!(
                                "driver-steps[{}].states.{state}.steps[{index}].evidence",
                                self.id
                            ),
                            format!(
                                "a `{}` does not establish `{}`",
                                mapping.verifier,
                                mapping.kind.as_str()
                            ),
                        )
                        .with_hint(format!(
                            "`{}` establishes: {}",
                            mapping.verifier,
                            if can.is_empty() {
                                "nothing this protocol names".to_owned()
                            } else {
                                can.join(", ")
                            }
                        )),
                    );
                }
            }
        }

        errors
    }

    /// Phase two: the map against the protocol in force and the workflow the plan resolved to.
    ///
    /// Run before the first step executes, because the alternative is failing at the transition
    /// that needed the evidence — halfway through a run that has already spent a token budget.
    pub fn check_run(&self, protocol: &Protocol, workflow: &Workflow) -> ValidationErrors {
        let mut errors = self.cross_validate(workflow);

        for (state, entry) in &self.states {
            for (index, step) in entry.steps.iter().enumerate() {
                let Step::Command(command) = step else {
                    continue;
                };
                let Some(mapping) = &command.evidence else {
                    continue;
                };
                if !protocol.declares_evidence(mapping.kind) {
                    errors.push(
                        ValidationError::new(
                            ValidationCode::UndeclaredEvidenceKind,
                            format!(
                                "driver-steps[{}].states.{state}.steps[{index}].evidence.kind",
                                self.id
                            ),
                            format!(
                                "protocol `{}` does not declare `{}`",
                                protocol.reference(),
                                mapping.kind.as_str()
                            ),
                        )
                        .with_hint(format!(
                            "the protocol declares: {}",
                            protocol
                                .evidence_kinds
                                .iter()
                                .map(ToString::to_string)
                                .collect::<Vec<_>>()
                                .join(", ")
                        )),
                    );
                }
            }
        }

        errors
    }
}

impl TryFrom<RawStepMap> for StepMap {
    type Error = ValidationErrors;

    fn try_from(raw: RawStepMap) -> Result<Self, Self::Error> {
        let mut errors = ValidationErrors::new();

        if raw.format != STEP_MAP_FORMAT {
            errors.push(
                ValidationError::new(
                    ValidationCode::UnsupportedFormatVersion,
                    "driver-steps.format",
                    format!(
                        "this build reads `{STEP_MAP_FORMAT}`, and the document is written in `{}`",
                        raw.format
                    ),
                )
                .with_hint(
                    "a step map from a newer driver may name step kinds this one cannot run, and \
                     running the rest would be a run that silently skipped them",
                ),
            );
        }

        let workflow = match PinnedWorkflowRef::try_from(raw.workflow.clone()) {
            Ok(pin) => Some(pin),
            Err(refusals) => {
                errors.extend(refusals);
                None
            }
        };

        if raw.states.is_empty() {
            errors.push(ValidationError::new(
                ValidationCode::EmptyDeclaration,
                format!("driver-steps[{}].states", raw.id),
                "a step map with no states tells a driver nothing about how to run anything",
            ));
        }

        let mut states = BTreeMap::new();
        for (state, entry) in raw.states {
            let location = format!("driver-steps[{}].states.{state}", raw.id);
            if entry.visit_budget == Some(0) {
                errors.push(ValidationError::new(
                    ValidationCode::EmptyDeclaration,
                    format!("{location}.visit_budget"),
                    "a visit budget of zero forbids entering a state the workflow can reach",
                ));
            }
            let mut steps = Vec::with_capacity(entry.steps.len());
            for (index, step) in entry.steps.into_iter().enumerate() {
                if let Some(step) =
                    validate_step(step, &format!("{location}.steps[{index}]"), &mut errors)
                {
                    steps.push(step);
                }
            }
            // `{transcript}` is the transcript of the `llm` step this step follows, so a state
            // with no `llm` step before the one that names it has nothing to expand. Decidable
            // here, from the document alone, which is where a step map's mistakes belong: the
            // alternative is a run that has already paid for a model session discovering that the
            // step after it cannot name what to check.
            let mut preceded_by_llm = false;
            for (index, step) in steps.iter().enumerate() {
                match step {
                    Step::Llm(_) => preceded_by_llm = true,
                    Step::Command(command)
                        if !preceded_by_llm
                            && command
                                .expandable()
                                .any(|word| placeholders_in(word).contains(&"transcript")) =>
                    {
                        errors.push(
                            ValidationError::new(
                                ValidationCode::MissingDeclaration,
                                format!("{location}.steps[{index}].run"),
                                format!(
                                    "`{{transcript}}` is the transcript of the `llm` step this \
                                         one follows, and no step of `{state}` before it is one"
                                ),
                            )
                            .with_hint(
                                "put the step after the `llm` step whose transcript it reads, \
                                 in the same state: a transcript from another state belongs to \
                                 another session and another prompt",
                            ),
                        );
                    }
                    _ => {}
                }
            }

            states.insert(
                state,
                StateSteps {
                    visit_budget: entry.visit_budget.unwrap_or(DEFAULT_VISIT_BUDGET),
                    steps,
                },
            );
        }

        let Some(workflow) = workflow else {
            return Err(errors);
        };

        errors.into_result(Self {
            id: raw.id,
            workflow,
            title: raw.title,
            states,
        })
    }
}

/// Validates one step, pushing what is wrong with it and returning it when nothing is.
fn validate_step(raw: RawStep, location: &str, errors: &mut ValidationErrors) -> Option<Step> {
    match raw {
        RawStep::Command(command) => {
            let mut usable = true;
            if command.run.is_empty() || command.run.iter().all(|word| word.trim().is_empty()) {
                errors.push(ValidationError::new(
                    ValidationCode::EmptyDeclaration,
                    format!("{location}.run"),
                    "a command step with nothing to run cannot observe anything",
                ));
                usable = false;
            }
            let evidence = command.evidence.and_then(|mapping| {
                let validated = validate_mapping(mapping, location, errors);
                usable &= validated.is_some();
                validated
            });
            let step = CommandStep {
                run: command.run,
                description: command.description,
                evidence,
                retries: command.retries.unwrap_or(DEFAULT_COMMAND_RETRIES),
            };
            for word in step.expandable() {
                for name in placeholders_in(word) {
                    if CommandStep::PLACEHOLDERS.contains(&name) {
                        continue;
                    }
                    errors.push(
                        ValidationError::new(
                            ValidationCode::UndeclaredReference,
                            format!("{location}.run"),
                            format!("nothing in a run expands `{{{name}}}`"),
                        )
                        .with_hint(format!(
                            "a step may name: {}",
                            CommandStep::PLACEHOLDERS
                                .iter()
                                .map(|name| format!("{{{name}}}"))
                                .collect::<Vec<_>>()
                                .join(", ")
                        )),
                    );
                    usable = false;
                }
            }
            usable.then_some(Step::Command(step))
        }
        RawStep::Llm(step) => {
            if step.prompt.trim().is_empty() {
                errors.push(ValidationError::new(
                    ValidationCode::EmptyDeclaration,
                    format!("{location}.prompt"),
                    "an llm step with an empty prompt asks for nothing",
                ));
                return None;
            }
            Some(Step::Llm(LlmStep {
                prompt: step.prompt,
                skills: step.skills,
                harness: step
                    .harness
                    .unwrap_or_else(|| LlmStep::DEFAULT_HARNESS.to_owned()),
                description: step.description,
            }))
        }
        RawStep::Operator(step) => {
            if step.prompt.trim().is_empty() {
                errors.push(ValidationError::new(
                    ValidationCode::EmptyDeclaration,
                    format!("{location}.prompt"),
                    "an operator step with an empty prompt tells a person nothing",
                ));
                return None;
            }
            Some(Step::Operator(OperatorStep {
                prompt: step.prompt,
                description: step.description,
            }))
        }
    }
}

/// Validates an evidence mapping against what a driver can mint from an exit status.
///
/// `record:` changes which question is being asked. Without it the driver builds the record from
/// the exit status, so the kind has to be one an exit status can carry; with it the verifier wrote
/// the record and the driver only reads it, so every kind the protocol declares is admissible and
/// the suite comes out of the document rather than out of this file.
fn validate_mapping(
    raw: RawEvidenceMapping,
    location: &str,
    errors: &mut ValidationErrors,
) -> Option<EvidenceMapping> {
    let mut usable = true;
    let record = match raw.record {
        Some(path) if path.trim().is_empty() => {
            errors.push(ValidationError::new(
                ValidationCode::EmptyDeclaration,
                format!("{location}.evidence.record"),
                "a record path with nothing in it names no document for the driver to read",
            ));
            usable = false;
            None
        }
        other => other,
    };
    let written = record.is_some();
    if !written && !EvidenceMapping::MINTABLE.contains(&raw.kind) {
        errors.push(
            ValidationError::new(
                ValidationCode::UnsupportedConstruct,
                format!("{location}.evidence.kind"),
                format!(
                    "a driver reads an exit status, which cannot establish `{}`",
                    raw.kind.as_str()
                ),
            )
            .with_hint(format!(
                "a command step can mint: {}. A verifier that writes the record itself declares \
                 `record:` beside `kind:`, and then the driver reads the document instead of \
                 minting anything",
                EvidenceMapping::MINTABLE
                    .iter()
                    .map(|kind| kind.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )),
        );
        usable = false;
    }
    if !written && raw.kind == EvidenceKind::TestResult && raw.suite.is_none() {
        errors.push(
            ValidationError::new(
                ValidationCode::MissingDeclaration,
                format!("{location}.evidence.suite"),
                "a test result names the suite that ran, and nothing in an exit status says which",
            )
            .with_hint("write `suite: unit`, `suite: contract` or another declared suite"),
        );
        usable = false;
    }
    usable.then_some(EvidenceMapping {
        kind: raw.kind,
        verifier: raw.verifier,
        suite: raw.suite,
        subject: raw.subject,
        tool: raw.tool,
        record,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(text: &str) -> Result<StepMap, ValidationErrors> {
        let raw: RawStepMap = serde_yaml_free(text);
        StepMap::try_from(raw)
    }

    /// Deserializes a document without a YAML dependency in this crate: JSON is the wire form the
    /// tests use, and `aep-schema` is what reads YAML in production.
    fn serde_yaml_free(json: &str) -> RawStepMap {
        serde_json::from_str(json).expect("the fixture deserializes")
    }

    fn map_json(workflow: &str, states: &str) -> String {
        format!(
            r#"{{"format":"aep.driver-steps/1","id":"development/default",
                "workflow":"{workflow}","states":{states}}}"#
        )
    }

    const ONE_COMMAND: &str = r#"{"implement":{"steps":[
        {"kind":"command","run":["cargo","test"],
         "evidence":{"kind":"test_result","verifier":"test-runner","suite":"unit"}}]}}"#;

    #[test]
    fn a_map_validates_and_carries_its_pin() {
        let map = read(&map_json("adp/default/1", ONE_COMMAND)).expect("valid");
        assert_eq!(map.workflow.to_string(), "adp/default/1");
        assert_eq!(map.format(), STEP_MAP_FORMAT);
        assert_eq!(map.steps_for(&StateId::new("implement").unwrap()).len(), 1);
        assert_eq!(
            map.visit_budget(&StateId::new("implement").unwrap()),
            DEFAULT_VISIT_BUDGET
        );
    }

    #[test]
    fn an_unpinned_workflow_is_refused() {
        let errors = read(&map_json("adp/default", ONE_COMMAND)).expect_err("refused");
        assert_eq!(errors.len(), 1);
        assert!(errors.contains(ValidationCode::MissingDeclaration));
    }

    #[test]
    fn a_wrong_format_version_is_refused() {
        let text = r#"{"format":"aep.driver-steps/2","id":"development/default",
                       "workflow":"adp/default/1","states":{"implement":{"steps":[]}}}"#;
        let errors = read(text).expect_err("refused");
        assert!(errors.contains(ValidationCode::UnsupportedFormatVersion));
    }

    #[test]
    fn four_broken_steps_report_four_errors() {
        // Invariant 3, asserted as an exact count rather than as "is an error": a validator that
        // returned on the first failure passes an `is_err` test and fails this one.
        let states = r#"{"implement":{"steps":[
            {"kind":"command","run":[]},
            {"kind":"command","run":["x"],"evidence":{"kind":"metric_observation","verifier":"compiler"}},
            {"kind":"command","run":["x"],"evidence":{"kind":"test_result","verifier":"test-runner"}},
            {"kind":"llm","prompt":"  "}]}}"#;
        let errors = read(&map_json("adp/default/1", states)).expect_err("refused");
        assert_eq!(
            errors.len(),
            4,
            "one error per broken step, not one for the document: {errors}"
        );
    }

    /// A kind an exit status cannot carry loads when the verifier writes the record itself.
    ///
    /// `trace_conformance` is the case: its record holds two digests and three counts, which is
    /// why it is not in `MINTABLE` and why it was unreachable from a step map until `record:`
    /// existed. The suite rule relaxes with it — a `test_result` a verifier wrote names its own
    /// suite in the document, and requiring the map to name it too would let the two disagree.
    #[test]
    fn a_kind_no_exit_status_can_carry_loads_when_the_verifier_writes_the_record() {
        let states = r#"{"implement":{"steps":[
            {"kind":"llm","prompt":"do the work"},
            {"kind":"command","run":["protocol","trace","evidence","--transcript","{transcript}"],
             "evidence":{"kind":"trace_conformance","verifier":"trace-checker",
                         "record":"{run_directory}/trace.yaml"}}]}}"#;
        let map = read(&map_json("adp/default/1", states)).expect("valid");
        let steps = map.steps_for(&StateId::new("implement").unwrap());
        let Step::Command(command) = &steps[1] else {
            panic!("the second step is the command");
        };
        assert_eq!(
            command
                .evidence
                .as_ref()
                .and_then(|mapping| mapping.record.clone())
                .as_deref(),
            Some("{run_directory}/trace.yaml")
        );

        // And without `record:` the same step is refused, because then the driver would have to
        // invent the digests.
        let minted = r#"{"implement":{"steps":[
            {"kind":"command","run":["protocol","trace","check"],
             "evidence":{"kind":"trace_conformance","verifier":"trace-checker"}}]}}"#;
        let errors = read(&map_json("adp/default/1", minted)).expect_err("refused");
        assert!(
            errors.contains(ValidationCode::UnsupportedConstruct),
            "{errors}"
        );
    }

    /// A misspelled placeholder is refused at load, where it costs nothing to fix.
    #[test]
    fn a_placeholder_nothing_expands_is_refused_and_a_brace_that_is_not_one_is_left_alone() {
        let states = r#"{"implement":{"steps":[
            {"kind":"command","run":["check","--at","{transcirpt}"]}]}}"#;
        let errors = read(&map_json("adp/default/1", states)).expect_err("refused");
        assert!(
            errors.contains(ValidationCode::UndeclaredReference),
            "{errors}"
        );

        // `{}` is `find -exec`'s argument and `{a: .b}` is a jq program: neither is a placeholder,
        // and a validator that refused them would refuse ordinary command lines.
        let literal = r#"{"implement":{"steps":[
            {"kind":"command","run":["find",".","-exec","rm","{}",";"]},
            {"kind":"command","run":["jq","{a: .b}"]}]}}"#;
        read(&map_json("adp/default/1", literal)).expect("braces that name nothing are text");
    }

    /// `{transcript}` in a state with no `llm` step before it names a session that never happened.
    #[test]
    fn a_transcript_placeholder_with_no_session_before_it_is_refused() {
        let states = r#"{"implement":{"steps":[
            {"kind":"command","run":["protocol","trace","check","--transcript","{transcript}"]},
            {"kind":"llm","prompt":"too late to help the step above"}]}}"#;
        let errors = read(&map_json("adp/default/1", states)).expect_err("refused");
        assert!(
            errors.contains(ValidationCode::MissingDeclaration),
            "{errors}"
        );
    }

    #[test]
    fn an_empty_map_is_refused() {
        let errors = read(&map_json("adp/default/1", "{}")).expect_err("refused");
        assert!(errors.contains(ValidationCode::EmptyDeclaration));
    }

    #[test]
    fn an_llm_step_has_nowhere_to_put_evidence() {
        // The type is the mechanism: `deny_unknown_fields` refuses the key outright, so this is a
        // deserialization failure rather than a validation one, and no relaxation of a rule can
        // reintroduce it.
        let text = r#"{"format":"aep.driver-steps/1","id":"a","workflow":"adp/default/1",
            "states":{"implement":{"steps":[{"kind":"llm","prompt":"go",
            "evidence":{"kind":"test_result","verifier":"test-runner"}}]}}}"#;
        let outcome: Result<RawStepMap, _> = serde_json::from_str(text);
        assert!(
            outcome.is_err(),
            "an llm step must not accept an evidence block"
        );
    }

    #[test]
    fn retry_budgets_are_per_step_kind() {
        let states = r#"{"implement":{"steps":[
            {"kind":"command","run":["x"]},
            {"kind":"command","run":["y"],"retries":0},
            {"kind":"llm","prompt":"go"},
            {"kind":"operator","prompt":"decide"}]}}"#;
        let map = read(&map_json("adp/default/1", states)).expect("valid");
        let steps = map.steps_for(&StateId::new("implement").unwrap());
        assert_eq!(steps[0].retry_budget(), DEFAULT_COMMAND_RETRIES);
        assert_eq!(steps[1].retry_budget(), 0);
        assert_eq!(steps[2].retry_budget(), LLM_RETRIES);
        assert_eq!(
            steps[3].retry_budget(),
            0,
            "a person is not a flaky dependency"
        );
    }

    #[test]
    fn the_digest_ignores_a_reordered_document_and_moves_with_a_changed_step() {
        let first = read(&map_json("adp/default/1", ONE_COMMAND)).expect("valid");
        let second = read(&map_json("adp/default/1", ONE_COMMAND)).expect("valid");
        assert_eq!(first.digest(), second.digest());
        let other = r#"{"implement":{"steps":[
            {"kind":"command","run":["cargo","test","--workspace"],
             "evidence":{"kind":"test_result","verifier":"test-runner","suite":"unit"}}]}}"#;
        let changed = read(&map_json("adp/default/1", other)).expect("valid");
        assert_ne!(first.digest(), changed.digest());
    }

    #[test]
    fn an_external_tool_verifier_loads_where_a_named_one_would_be_refused() {
        let external = r#"{"implement":{"steps":[
            {"kind":"command","run":["ruff"],
             "evidence":{"kind":"static_analysis","verifier":"ruff"}}]}}"#;
        let map = read(&map_json("adp/default/1", external)).expect("valid");
        let workflow = fixture_workflow();
        assert!(
            map.cross_validate(&workflow).is_empty(),
            "an external tool has no row in the defaults table, so there is nothing to refuse it \
             against"
        );
    }

    #[test]
    fn a_named_verifier_that_cannot_produce_the_kind_is_refused_at_load() {
        let wrong = r#"{"implement":{"steps":[
            {"kind":"command","run":["x"],
             "evidence":{"kind":"contract_result","verifier":"test-runner"}}]}}"#;
        let map = read(&map_json("adp/default/1", wrong)).expect("valid");
        let errors = map.cross_validate(&fixture_workflow());
        assert_eq!(errors.len(), 1);
        assert!(errors.contains(ValidationCode::NoVerifierForEvidence));
    }

    #[test]
    fn a_renamed_state_and_a_moved_major_accumulate_rather_than_short_circuit() {
        let states = r#"{"implement":{"steps":[]},"polish":{"steps":[]}}"#;
        let map = read(&map_json("adp/default/1", states)).expect("valid");
        let mut workflow = fixture_workflow();
        workflow.version = MajorVersion::new(2).expect("a major version");
        let errors = map.cross_validate(&workflow);
        assert_eq!(errors.len(), 2, "{errors}");
        assert!(errors.contains(ValidationCode::VersionMismatch));
        assert!(errors.contains(ValidationCode::UnknownState));
    }

    use aep_domain::version::MajorVersion;

    /// A workflow with one state, built through its own validator.
    fn fixture_workflow() -> Workflow {
        let raw: aep_domain::raw::RawWorkflow = serde_json::from_str(
            r#"{"id":"adp/default","version":1,"title":"t","initial":"implement",
                "states":{"implement":{"title":"Implement","terminal":true}},
                "transitions":[]}"#,
        )
        .expect("the fixture deserializes");
        Workflow::try_from(raw).expect("the fixture validates")
    }
}
