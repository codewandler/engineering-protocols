//! Reading a `trace-spec/1` document: the permissive half, and the rules deserialization cannot
//! see.
//!
//! Invariant 2, in the third observation domain. A specification is a file somebody writes,
//! reviews and commits, so it becomes a domain type by *validating* rather than by deserializing:
//! [`TraceSpec`] does not implement `Deserialize`, this module holds the type that does, and
//! [`TryFrom`] is the only door.
//!
//! # What the shape already refuses, and what it cannot
//!
//! The wire form is **externally tagged under `expect:`** and keeps the design's dotted kind
//! names verbatim — `expect: {tool.called: {tool: Bash}}` — which buys two refusals for free: a
//! kind this build does not implement is refused *by name*, and every variant's parameter struct
//! carries `deny_unknown_fields`, so `at_leats: 1` is refused rather than read as "unbounded".
//!
//! That nesting is the reason the kind is **not** flattened beside `id:`, which is how the design
//! document's own examples are written: serde's `deny_unknown_fields` does not survive `flatten`,
//! which is the note `infra-spec`'s reader carries for the same construction
//! (`crates/infra-spec/src/raw.rs`), and a specification where a misspelt parameter silently
//! became a default is worse than one that is a line longer.
//!
//! A document written the flat way is refused too, though not helpfully: `expect:
//! env.plugin_loaded` with the parameters beside `id:` produces `TRACE-SPEC-002` carrying serde's
//! *"invalid type: unit variant, expected newtype variant"*, which names neither the kind nor the
//! fix. That is a known rough edge of this wire form, and the place to smooth it is a hint in the
//! reader — never a second accepted shape, which would put `deny_unknown_fields` back on the
//! flattened path where it does not work.
//!
//! What no deserializer can see is everything about the document as a whole, and about whether an
//! expectation can decide anything at all:
//!
//! | rule | code |
//! |---|---|
//! | the format is the one this build reads | `TRACE-SPEC-001` |
//! | the text reads as the `trace-spec/1` shape at all | `TRACE-SPEC-002` |
//! | no two expectations share an id | `TRACE-SPEC-003` |
//! | at least one expectation is declared | `TRACE-SPEC-004` |
//! | every kind's own parameters can decide something | `TRACE-SPEC-005` |
//! | every id is a stable identifier | `TRACE-SPEC-006` |
//! | every bound can be satisfied | `TRACE-SPEC-007` |
//! | no matcher this build does not implement was asked for | `TRACE-SPEC-008` |
//!
//! `TRACE-SPEC-005` is the one worth reading the code for, because "can decide something" is a
//! judgement and every call of it is written down beside the check that makes it. A `tool.absent`
//! with no tool and no argument matcher forbids *every* tool call; an `order` whose two sides
//! select the same calls asks for a call to precede itself; a `result` that states none of its
//! five fields is a verdict about nothing. Each of those is a document somebody meant to finish.
//!
//! # The one default, and why it is the only one
//!
//! `count:` may be omitted on `skill.invoked`, `skill.completed` and `tool.called`, where it
//! means `{at_least: 1}` — *"this happened"*, which is what every author of those three means. No
//! other bound defaults, and an explicitly written `count: {}` is refused rather than read as
//! that default: an author who typed a bound and got no bound should be told.
//!
//! # Two spellings, one kind
//!
//! `skill.available` is an accepted spelling of `env.skill_available`; the design § 3.2 calls one
//! the alias of the other, and both produce [`ExpectationKind::EnvSkillAvailable`]. Aliases are
//! accepted on input only, which is the workspace's standing rule for wire-format aliases
//! (`AGENTS.md` § *Conventions*) — a report prints the canonical name. The generated JSON schema
//! describes the canonical spelling alone, because `schemars` reads `rename` and not `alias`;
//! that is a schema which is stricter than the reader, never looser, which is the safe direction.
//!
//! # `regex:` deserializes and is then refused
//!
//! Design § 3.4 lists a `regex` matcher and this build has no regular-expression engine to run
//! one with. It is accepted by the *parser* and refused by *validation*
//! ([`TraceCode::SpecUnsupportedMatcher`]) with a message naming `glob` as what to write instead.
//! Both alternatives are worse: refusing it as an unknown field would tell the author that
//! `regex` is a typo, and reading it as `contains` would produce a specification that means
//! something other than what it says.
//!
//! # Errors accumulate
//!
//! Invariant 3. A document with four broken expectations reports four refusals in one run, and
//! the tests at the bottom of this file assert an exact count per code rather than "is an error",
//! which is the only thing that enforces it.

use std::collections::{BTreeMap, BTreeSet};

use crate::code::{TraceCode, ValidationErrors};
use crate::matcher::{
    CallSelector, CountBound, FieldMatcher, RangeBound, ResultMatcher, ScalarValue,
};
use crate::spec::{
    Aggregate, ApiErrorStatus, Expectation, ExpectationKind, OnUnknown, Severity, ToolAvailability,
    TraceSpec, SPEC_FORMAT,
};

/// Reads a specification's text — YAML or JSON — through its validation.
///
/// The one reader, so a command line, a harness and a test cannot disagree about what a document
/// means. A text that does not deserialize at all is a single [`TraceCode::SpecMalformed`]
/// refusal rather than a raw serde sentence: a caller matches on a code here exactly as it does
/// on a document that deserialized and then broke a rule.
///
/// # Errors
///
/// Every rule that failed, accumulated.
pub fn read_spec(text: &str) -> Result<TraceSpec, ValidationErrors> {
    // Through `serde_json::Value`: see the manifest's note beside `serde_yaml`. An externally
    // tagged enum is a single-key *map* in the JSON data model and a YAML *tag* (`!tool.called`)
    // in `serde_yaml` 0.9, and a specification is written in the first spelling. One conversion
    // buys the readable wire form and `deny_unknown_fields` on every variant at once — and JSON
    // parses through the same path, because JSON is YAML.
    let value: serde_json::Value = serde_yaml::from_str(text).map_err(malformed)?;
    let raw: RawTraceSpec = serde_json::from_value(value).map_err(malformed)?;
    TraceSpec::try_from(raw)
}

/// The one refusal in this family that cannot accumulate with others: a document that did not
/// deserialize has no expectations to go on and check.
fn malformed(error: impl std::fmt::Display) -> ValidationErrors {
    let mut errors = ValidationErrors::new();
    errors.refuse(TraceCode::SpecMalformed, "document", error.to_string());
    errors
}

/// A specification as it is written, before anything has checked what it claims.
#[derive(Debug, Clone, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RawTraceSpec {
    /// The shape the document says it is written in.
    pub format: String,
    /// What the specification is about, such as `planning-plugin/eval`.
    pub id: String,
    /// A human sentence for a report's heading.
    #[serde(default)]
    pub title: Option<String>,
    /// The expectations it declares.
    #[serde(default)]
    pub expectations: Vec<RawExpectation>,
}

/// One expectation as written.
#[derive(Debug, Clone, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RawExpectation {
    /// The id a verdict is reported under.
    pub id: String,
    /// The author's own sentence, carried into the report unchanged.
    #[serde(default)]
    pub statement: Option<String>,
    /// Whether a gap moves the exit code. `gate` when the document omits it.
    #[serde(default)]
    pub severity: RawSeverity,
    /// What an undecidable verdict means here. `unknown` when the document omits it.
    #[serde(default)]
    pub on_unknown: RawOnUnknown,
    /// What it claims.
    pub expect: RawExpectationKind,
}

/// `gate` or `advisory`, as written.
#[derive(Debug, Clone, Copy, Default, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RawSeverity {
    /// The default: a gap fails the check.
    #[default]
    Gate,
    /// A gap is reported and printed, and the exit code does not move.
    Advisory,
}

impl From<RawSeverity> for Severity {
    fn from(raw: RawSeverity) -> Self {
        match raw {
            RawSeverity::Gate => Self::Gate,
            RawSeverity::Advisory => Self::Advisory,
        }
    }
}

/// `unknown` or `gap`, as written.
#[derive(Debug, Clone, Copy, Default, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RawOnUnknown {
    /// The default: report `unk`.
    #[default]
    Unknown,
    /// Report a gap — the transcript must have carried the field.
    Gap,
}

impl From<RawOnUnknown> for OnUnknown {
    fn from(raw: RawOnUnknown) -> Self {
        match raw {
            RawOnUnknown::Unknown => Self::Unknown,
            RawOnUnknown::Gap => Self::Gap,
        }
    }
}

/// `total` or `each`, as written.
#[derive(Debug, Clone, Copy, Default, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RawAggregate {
    /// The default: the sum across every selected call.
    #[default]
    Total,
    /// Every selected call on its own.
    Each,
}

impl From<RawAggregate> for Aggregate {
    fn from(raw: RawAggregate) -> Self {
        match raw {
            RawAggregate::Total => Self::Total,
            RawAggregate::Each => Self::Each,
        }
    }
}

/// A bound over a whole number as written: `{at_least: 1}`, `{at_most: 3}`, `{exactly: 0}`, or a
/// floor and a ceiling together.
///
/// Never a bare number. `count: 1` cannot be read as "at least once" by one author and "exactly
/// once" by the next, so there is no shorthand to read it as either.
#[derive(Debug, Clone, Copy, Default, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RawCountBound {
    /// The lowest acceptable value.
    #[serde(default)]
    pub at_least: Option<u64>,
    /// The highest acceptable value.
    #[serde(default)]
    pub at_most: Option<u64>,
    /// The only acceptable value; validation refuses it beside either of the other two.
    #[serde(default)]
    pub exactly: Option<u64>,
}

/// A bound over a fractional quantity as written: `{at_least: 0.9}`, `{at_most: 0.62}`, or both.
///
/// No `exactly`, by construction — design decision D6, and the same absence [`RangeBound`] has.
#[derive(Debug, Clone, Copy, Default, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RawRangeBound {
    /// The lowest acceptable value.
    #[serde(default)]
    pub at_least: Option<f64>,
    /// The highest acceptable value.
    #[serde(default)]
    pub at_most: Option<f64>,
}

/// A matcher over one named field as written: `{contains: "protocol artifact new"}`.
///
/// Externally tagged, so a matcher name this build does not know is refused by name rather than
/// defaulted away — and so `regex:` can be *read* and then refused with advice, which an
/// unknown-field refusal could not do.
#[derive(Debug, Clone, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RawFieldMatcher {
    /// `{exact: "…"}` — the whole field, character for character.
    Exact(String),
    /// `{contains: "…"}` — a substring of the field.
    Contains(String),
    /// `{glob: "…"}` — `*` for any run of characters, `?` for one.
    Glob(String),
    /// `{equals: <bool|integer|string>}` — a scalar field, compared like with like.
    Equals(RawScalar),
    /// `{regex: "…"}` — accepted by the parser and refused by validation
    /// ([`TraceCode::SpecUnsupportedMatcher`]), with `glob` named as what to write instead.
    Regex(String),
}

/// A scalar under `equals:` as written.
///
/// [`ScalarValue`] has no fractional variant, deliberately; this one does, so that a fraction is
/// refused with the advice to write a bound rather than with a serde sentence about which of four
/// shapes it failed to be.
#[derive(Debug, Clone, serde::Deserialize, schemars::JsonSchema)]
#[serde(untagged)]
pub enum RawScalar {
    /// A boolean, such as `userModified: {equals: false}`.
    Bool(bool),
    /// A whole number.
    Integer(i64),
    /// A fraction, which validation refuses.
    Fraction(f64),
    /// A string.
    Text(String),
}

/// The scope half of the tool family as written, where a kind nests it: `{tool: Bash, args: {…}}`.
///
/// The other tool-family kinds spell `tool:` and `args:` as their own fields rather than nesting
/// a selector, because `deny_unknown_fields` does not survive `flatten` and a shared selector
/// struct would have to be flattened to keep the design's wire form. The duplication is the price
/// of refusing `toool: Bash`.
#[derive(Debug, Clone, Default, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RawSelector {
    /// The tool's name. Absent selects every tool.
    #[serde(default)]
    pub tool: Option<String>,
    /// Matchers over named arguments, all of which must hold.
    #[serde(default)]
    pub args: BTreeMap<String, RawFieldMatcher>,
}

/// What the terminal record's `api_error_status` must say: `absent`, or `{equals: "429"}`.
#[derive(Debug, Clone, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RawApiErrorStatus {
    /// `absent` — there must be no API error status at all.
    Absent,
    /// `{equals: "429"}` — there must be one, and it must be this.
    Equals(String),
}

/// An expectation kind as written, externally tagged so the parameter names are checked.
///
/// The variant names are the design's dotted kind names verbatim, and
/// [`ExpectationKind::NAMES`] is the same vocabulary on the validated side. A test at the bottom
/// of this file builds a document for every published name and refuses to let the two lists
/// drift.
#[derive(Debug, Clone, serde::Deserialize, schemars::JsonSchema)]
pub enum RawExpectationKind {
    /// `{env.plugin_loaded: {plugin: …, version: …, source: …}}`
    #[serde(rename = "env.plugin_loaded")]
    EnvPluginLoaded(RawPluginLoaded),
    /// `{env.exclusive: {plugins: [...]}}`
    #[serde(rename = "env.exclusive")]
    EnvExclusive(RawPlugins),
    /// `{env.output_style: {equals: …}}`
    #[serde(rename = "env.output_style")]
    EnvOutputStyle(RawEquals),
    /// `{env.skill_available: {skill: …}}`, also spelled `{skill.available: {skill: …}}`.
    #[serde(rename = "env.skill_available", alias = "skill.available")]
    EnvSkillAvailable(RawSkill),
    /// `{env.agent_available: {agent: …}}`
    #[serde(rename = "env.agent_available")]
    EnvAgentAvailable(RawAgent),
    /// `{env.tool_available: {tool: Bash}}`, `{env.tool_available: {tool: Task, available:
    /// false}}`, or `{env.tool_available: {only: [Read, Glob, Grep]}}`.
    #[serde(rename = "env.tool_available")]
    EnvToolAvailable(RawToolAvailable),
    /// `{env.model: {equals: …}}`
    #[serde(rename = "env.model")]
    EnvModel(RawEquals),
    /// `{env.permission_mode: {equals: …}}`
    #[serde(rename = "env.permission_mode")]
    EnvPermissionMode(RawEquals),
    /// `{env.api_key_source: {equals: none}}`
    #[serde(rename = "env.api_key_source")]
    EnvApiKeySource(RawEquals),

    /// `{skill.invoked: {skill: …, count: {…}}}`
    #[serde(rename = "skill.invoked")]
    SkillInvoked(RawSkillCount),
    /// `{skill.completed: {skill: …, count: {…}}}`
    #[serde(rename = "skill.completed")]
    SkillCompleted(RawSkillCount),

    /// `{tool.called: {tool: …, args: {…}, count: {…}}}`
    #[serde(rename = "tool.called")]
    ToolCalled(RawToolCalled),
    /// `{tool.absent: {tool: …, args: {…}}}`
    #[serde(rename = "tool.absent")]
    ToolAbsent(RawSelector),
    /// `{tool.result: {tool: …, args: {…}, result: {<field>: <matcher>}}}`
    #[serde(rename = "tool.result")]
    ToolResult(RawToolResult),
    /// `{tool.result_bytes: {tool: …, args: {…}, bytes: {…}, per: total|each}}`
    #[serde(rename = "tool.result_bytes")]
    ToolResultBytes(RawToolResultBytes),
    /// `{tool.failed: {tool: …, args: {…}, count: {…}}}`
    #[serde(rename = "tool.failed")]
    ToolFailed(RawScopedCount),
    /// `{tool.error_rate: {tool: …, args: {…}, rate: {at_most: …}}}`
    #[serde(rename = "tool.error_rate")]
    ToolErrorRate(RawToolErrorRate),
    /// `{tool.repeated: {tool: …, args: {…}, count: {…}}}`
    #[serde(rename = "tool.repeated")]
    ToolRepeated(RawScopedCount),
    /// `{order: {first: {tool: …}, before: {tool: …}}}`
    #[serde(rename = "order")]
    Order(RawOrder),
    /// `{result: {is_error: …, subtype: …, stop_reason: …, terminal_reason: …,
    /// api_error_status: …}}`
    #[serde(rename = "result")]
    RunResult(RawRunResult),
    /// `{permission.denied: {count: {…}}}`
    #[serde(rename = "permission.denied")]
    PermissionDenied(RawCount),
    /// `{subagent.spawned: {count: {…}}}`
    #[serde(rename = "subagent.spawned")]
    SubagentSpawned(RawCount),
    /// `{text.matches: {contains: "…"}}` — the matcher's own keys, inline.
    #[serde(rename = "text.matches")]
    TextMatches(RawFieldMatcher),

    /// `{rate_limit.status: {allowed: [...]}}`
    #[serde(rename = "rate_limit.status")]
    RateLimitStatus(RawAllowed),
    /// `{rate_limit.overage: {equals: false}}`
    #[serde(rename = "rate_limit.overage")]
    RateLimitOverage(RawEqualsBool),
    /// `{rate_limit.utilization: {at_most: 0.9}}` — the range bound's own keys, inline.
    #[serde(rename = "rate_limit.utilization")]
    RateLimitUtilization(RawRangeBound),

    /// `{turns: {count: {…}}}`
    #[serde(rename = "turns")]
    Turns(RawCount),
    /// `{api_requests: {count: {…}}}`
    #[serde(rename = "api_requests")]
    ApiRequests(RawCount),
    /// `{events.assistant: {count: {…}}}`
    #[serde(rename = "events.assistant")]
    EventsAssistant(RawCount),
    /// `{iterations: {count: {…}}}`
    #[serde(rename = "iterations")]
    Iterations(RawCount),

    /// `{tokens.input: {count: {…}, model: …}}`
    #[serde(rename = "tokens.input")]
    TokensInput(RawModelCount),
    /// `{tokens.output: {count: {…}, model: …}}`
    #[serde(rename = "tokens.output")]
    TokensOutput(RawModelCount),
    /// `{tokens.total: {count: {…}, model: …}}`
    #[serde(rename = "tokens.total")]
    TokensTotal(RawModelCount),
    /// `{tokens.thinking: {count: {…}}}`
    #[serde(rename = "tokens.thinking")]
    TokensThinking(RawCount),
    /// `{thinking.estimated: {count: {…}}}`
    #[serde(rename = "thinking.estimated")]
    ThinkingEstimated(RawCount),
    /// `{cost.total: {at_most_usd: 1.0, model: …}}`
    #[serde(rename = "cost.total")]
    CostTotal(RawCostTotal),
    /// `{cache.used: {equals: true}}`
    #[serde(rename = "cache.used")]
    CacheUsed(RawEqualsBool),
    /// `{cache.read_tokens: {count: {…}, model: …}}`
    #[serde(rename = "cache.read_tokens")]
    CacheReadTokens(RawModelCount),
    /// `{cache.created_tokens: {count: {…}, model: …}}`
    #[serde(rename = "cache.created_tokens")]
    CacheCreatedTokens(RawModelCount),
    /// `{cache.hit_ratio: {at_least: 0.9}}` — the range bound's own keys, inline.
    #[serde(rename = "cache.hit_ratio")]
    CacheHitRatio(RawRangeBound),

    /// `{duration.total: {ms: {…}}}`
    #[serde(rename = "duration.total")]
    DurationTotal(RawMs),
    /// `{duration.api: {ms: {…}}}`
    #[serde(rename = "duration.api")]
    DurationApi(RawMs),
    /// `{ttft: {ms: {…}}}`
    #[serde(rename = "ttft")]
    Ttft(RawMs),
    /// `{time_to_request: {ms: {…}}}`
    #[serde(rename = "time_to_request")]
    TimeToRequest(RawMs),
    /// `{step.gen_time: {tool: …, args: {…}, ms: {…}}}`
    #[serde(rename = "step.gen_time")]
    StepGenTime(RawStepMs),
    /// `{step.exec_time: {tool: …, args: {…}, ms: {…}}}`
    #[serde(rename = "step.exec_time")]
    StepExecTime(RawStepMs),
    /// `{time.inference_total: {ms: {…}}}`
    #[serde(rename = "time.inference_total")]
    TimeInferenceTotal(RawMs),
    /// `{time.tool_exec_total: {ms: {…}}}`
    #[serde(rename = "time.tool_exec_total")]
    TimeToolExecTotal(RawMs),

    /// `{speed: {equals: standard}}`
    #[serde(rename = "speed")]
    Speed(RawEquals),
    /// `{service_tier: {equals: standard}}`
    #[serde(rename = "service_tier")]
    ServiceTier(RawEquals),
}

/// The parameters of `env.plugin_loaded`.
#[derive(Debug, Clone, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RawPluginLoaded {
    /// The plugin's name.
    pub plugin: String,
    /// The version it must be at, where the specification pins one.
    #[serde(default)]
    pub version: Option<String>,
    /// The source it must have come from, where the specification pins one.
    #[serde(default)]
    pub source: Option<String>,
}

/// The parameters of `env.exclusive`.
#[derive(Debug, Clone, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RawPlugins {
    /// Every plugin that may be loaded, and no others.
    pub plugins: Vec<String>,
}

/// The parameters of the six kinds that compare one recorded string.
#[derive(Debug, Clone, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RawEquals {
    /// The value expected.
    pub equals: String,
}

/// The parameters of the two kinds that compare one recorded boolean.
#[derive(Debug, Clone, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RawEqualsBool {
    /// What it must be.
    pub equals: bool,
}

/// The parameters of `env.skill_available`.
#[derive(Debug, Clone, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RawSkill {
    /// The skill's name, as the harness lists it.
    pub skill: String,
}

/// The parameters of `env.agent_available`.
#[derive(Debug, Clone, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RawAgent {
    /// The agent's name, as the harness lists it.
    pub agent: String,
}

/// The parameters of `env.tool_available`, in its two forms.
///
/// `tool` and `only` are two different claims — *"this one was on the table"* against *"exactly
/// these were"* — so exactly one of them is written. Both is two expectations reported under one
/// id; neither names no tool at all. `available` belongs to `tool` alone: beside `only` it has no
/// reading, because `only` already says of every tool whether it was offered.
///
/// Three optional fields rather than a tagged union, because that is what an author writes and
/// `deny_unknown_fields` still catches a misspelt one. The combinations that mean nothing are
/// refused by name in validation, which is where the message can say which line to delete.
#[derive(Debug, Clone, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RawToolAvailable {
    /// One tool's name, as the harness lists it.
    #[serde(default)]
    pub tool: Option<String>,
    /// Whether that tool must have been offered. `true` when the document omits it.
    #[serde(default)]
    pub available: Option<bool>,
    /// Every tool that may be offered, and no others.
    #[serde(default)]
    pub only: Option<Vec<String>>,
}

/// The parameters of `skill.invoked` and `skill.completed`.
#[derive(Debug, Clone, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RawSkillCount {
    /// The skill's name.
    pub skill: String,
    /// How many are acceptable. `{at_least: 1}` when the document omits it.
    #[serde(default)]
    pub count: Option<RawCountBound>,
}

/// The parameters of `tool.called`.
#[derive(Debug, Clone, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RawToolCalled {
    /// The tool's name. Absent selects every tool.
    #[serde(default)]
    pub tool: Option<String>,
    /// Matchers over named arguments, all of which must hold.
    #[serde(default)]
    pub args: BTreeMap<String, RawFieldMatcher>,
    /// How many are acceptable. `{at_least: 1}` when the document omits it.
    #[serde(default)]
    pub count: Option<RawCountBound>,
}

/// The parameters of `tool.failed` and `tool.repeated`, where the bound is not optional.
#[derive(Debug, Clone, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RawScopedCount {
    /// The tool's name. Absent selects every tool.
    #[serde(default)]
    pub tool: Option<String>,
    /// Matchers over named arguments, all of which must hold.
    #[serde(default)]
    pub args: BTreeMap<String, RawFieldMatcher>,
    /// How many are acceptable.
    pub count: RawCountBound,
}

/// The parameters of `tool.result`.
#[derive(Debug, Clone, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RawToolResult {
    /// The tool's name. Absent selects every tool.
    #[serde(default)]
    pub tool: Option<String>,
    /// Matchers over named arguments, all of which must hold.
    #[serde(default)]
    pub args: BTreeMap<String, RawFieldMatcher>,
    /// What the matched calls' results must say, by field name.
    pub result: BTreeMap<String, RawFieldMatcher>,
}

/// The parameters of `tool.result_bytes`.
#[derive(Debug, Clone, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RawToolResultBytes {
    /// The tool's name. Absent selects every tool.
    #[serde(default)]
    pub tool: Option<String>,
    /// Matchers over named arguments, all of which must hold.
    #[serde(default)]
    pub args: BTreeMap<String, RawFieldMatcher>,
    /// The byte bound.
    pub bytes: RawCountBound,
    /// Whether the bound is over the sum or over each call. `total` when the document omits it.
    #[serde(default)]
    pub per: RawAggregate,
}

/// The parameters of `tool.error_rate`.
#[derive(Debug, Clone, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RawToolErrorRate {
    /// The tool's name. Absent selects every tool.
    #[serde(default)]
    pub tool: Option<String>,
    /// Matchers over named arguments, all of which must hold.
    #[serde(default)]
    pub args: BTreeMap<String, RawFieldMatcher>,
    /// The acceptable rate, from 0 to 1.
    pub rate: RawRangeBound,
}

/// The parameters of `order`.
#[derive(Debug, Clone, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RawOrder {
    /// The call that must come first.
    pub first: RawSelector,
    /// The call it must come before.
    pub before: RawSelector,
}

/// The parameters of `result`.
#[derive(Debug, Clone, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RawRunResult {
    /// Whether the run must be flagged as an error.
    #[serde(default)]
    pub is_error: Option<bool>,
    /// The record's subtype.
    #[serde(default)]
    pub subtype: Option<String>,
    /// Why the model must have stopped.
    #[serde(default)]
    pub stop_reason: Option<String>,
    /// Why the run must have ended.
    #[serde(default)]
    pub terminal_reason: Option<String>,
    /// What the API error status must say.
    #[serde(default)]
    pub api_error_status: Option<RawApiErrorStatus>,
}

/// The parameters of the eight kinds that bound one whole number and nothing else.
#[derive(Debug, Clone, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RawCount {
    /// How many are acceptable.
    pub count: RawCountBound,
}

/// The parameters of `rate_limit.status`.
#[derive(Debug, Clone, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RawAllowed {
    /// The acceptable statuses.
    pub allowed: Vec<String>,
}

/// The parameters of the five token and cache kinds that may be scoped to one model.
#[derive(Debug, Clone, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RawModelCount {
    /// How many are acceptable.
    pub count: RawCountBound,
    /// The model to scope to, where the specification names one.
    #[serde(default)]
    pub model: Option<String>,
}

/// The parameters of `cost.total`.
///
/// The two keys carry their unit — `at_most_usd`, not `at_most` — because that is how the design
/// writes them, and because a number of dollars beside a number of tokens on the next line should
/// not be spelled identically.
#[derive(Debug, Clone, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RawCostTotal {
    /// The lowest acceptable cost, in US dollars.
    #[serde(default)]
    pub at_least_usd: Option<f64>,
    /// The highest acceptable cost, in US dollars.
    #[serde(default)]
    pub at_most_usd: Option<f64>,
    /// The model to scope to, where the specification names one.
    #[serde(default)]
    pub model: Option<String>,
}

/// The parameters of the six kinds that bound one recorded duration.
#[derive(Debug, Clone, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RawMs {
    /// The acceptable duration, in milliseconds.
    pub ms: RawCountBound,
}

/// The parameters of `step.gen_time` and `step.exec_time`.
#[derive(Debug, Clone, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RawStepMs {
    /// The tool's name. Absent selects every tool.
    #[serde(default)]
    pub tool: Option<String>,
    /// Matchers over named arguments, all of which must hold.
    #[serde(default)]
    pub args: BTreeMap<String, RawFieldMatcher>,
    /// The acceptable interval, in milliseconds.
    pub ms: RawCountBound,
}

impl TryFrom<RawTraceSpec> for TraceSpec {
    type Error = ValidationErrors;

    fn try_from(raw: RawTraceSpec) -> Result<Self, Self::Error> {
        let mut errors = ValidationErrors::new();

        if raw.format != SPEC_FORMAT {
            errors.refuse(
                TraceCode::SpecUnsupportedFormat,
                "format",
                format!(
                    "this build reads `{SPEC_FORMAT}`, and the document is written in `{}`",
                    raw.format
                ),
            );
        }

        if !is_document_id(&raw.id) {
            errors.refuse(
                TraceCode::SpecMalformedId,
                "id",
                format!(
                    "`{}` is not a specification id: lowercase letters, digits and dashes, with \
                     at most one `/` between a namespace and a name",
                    raw.id
                ),
            );
        }

        if raw.expectations.is_empty() {
            errors.refuse(
                TraceCode::SpecEmptyExpectations,
                "expectations",
                "a specification that expects nothing judges nothing, and a report with no \
                 content reads exactly like a report with no gaps",
            );
        }

        let mut seen: BTreeMap<String, usize> = BTreeMap::new();
        let mut expectations = Vec::with_capacity(raw.expectations.len());
        for (index, written) in raw.expectations.into_iter().enumerate() {
            let location = format!("expectations[{index}]");

            if !is_identifier(&written.id) {
                errors.refuse(
                    TraceCode::SpecMalformedId,
                    format!("{location}.id"),
                    format!(
                        "`{}` is not an expectation id: lowercase letters, digits and dashes",
                        written.id
                    ),
                );
            }
            if let Some(first) = seen.insert(written.id.clone(), index) {
                errors.refuse(
                    TraceCode::SpecDuplicateExpectation,
                    format!("{location}.id"),
                    format!(
                        "`{}` is already declared at expectations[{first}], and a report names a \
                         verdict by its id",
                        written.id
                    ),
                );
            }

            // The kind is validated whatever the id did, so a document with a bad id and a bad
            // bound reports both in one run (invariant 3).
            let Some(kind) = kind_of(written.expect, &location, &mut errors) else {
                continue;
            };
            expectations.push(Expectation {
                id: written.id,
                statement: written.statement,
                severity: written.severity.into(),
                on_unknown: written.on_unknown.into(),
                kind,
            });
        }

        errors.into_result(Self::new(raw.id, raw.title, expectations))
    }
}

/// Where a defect is: `expectations[2].expect.tool.called.count`.
fn at(location: &str, kind: &str, field: &str) -> String {
    format!("{location}.expect.{kind}.{field}")
}

/// Where a defect is when it is about the kind itself rather than one of its parameters.
fn at_kind(location: &str, kind: &str) -> String {
    format!("{location}.expect.{kind}")
}

/// Validates one kind's own parameters.
///
/// `None` when the kind cannot decide anything, in which case a refusal has already been
/// recorded and the expectation is dropped — a validated [`ExpectationKind`] is one a checker can
/// evaluate.
///
/// One `match` over all fifty, deliberately: this is the seam where the wire vocabulary and
/// [`ExpectationKind::NAMES`] meet, and splitting it into families would hide the exhaustiveness
/// that makes a missing kind a compile error rather than a silent gap. It is long for that
/// reason and no other.
#[allow(clippy::too_many_lines)]
fn kind_of(
    raw: RawExpectationKind,
    location: &str,
    errors: &mut ValidationErrors,
) -> Option<ExpectationKind> {
    match raw {
        RawExpectationKind::EnvPluginLoaded(written) => plugin_loaded(written, location, errors),
        RawExpectationKind::EnvExclusive(written) => Some(ExpectationKind::EnvExclusive {
            plugins: names_of(
                written.plugins,
                &at(location, "env.exclusive", "plugins"),
                "plugin",
                "an `env.exclusive` listing no plugin is an unfinished document far more often \
                 than it is the claim that nothing at all was loaded, and the two read the same",
                errors,
            )?,
        }),
        RawExpectationKind::EnvOutputStyle(written) => Some(ExpectationKind::EnvOutputStyle {
            equals: stated(
                written.equals,
                &at(location, "env.output_style", "equals"),
                "output style",
                errors,
            )?,
        }),
        RawExpectationKind::EnvSkillAvailable(written) => {
            Some(ExpectationKind::EnvSkillAvailable {
                skill: stated(
                    written.skill,
                    &at(location, "env.skill_available", "skill"),
                    "skill name",
                    errors,
                )?,
            })
        }
        RawExpectationKind::EnvAgentAvailable(written) => {
            Some(ExpectationKind::EnvAgentAvailable {
                agent: stated(
                    written.agent,
                    &at(location, "env.agent_available", "agent"),
                    "agent name",
                    errors,
                )?,
            })
        }
        RawExpectationKind::EnvToolAvailable(written) => tool_available(written, location, errors),
        RawExpectationKind::EnvModel(written) => Some(ExpectationKind::EnvModel {
            equals: stated(
                written.equals,
                &at(location, "env.model", "equals"),
                "model",
                errors,
            )?,
        }),
        RawExpectationKind::EnvPermissionMode(written) => {
            Some(ExpectationKind::EnvPermissionMode {
                equals: stated(
                    written.equals,
                    &at(location, "env.permission_mode", "equals"),
                    "permission mode",
                    errors,
                )?,
            })
        }
        RawExpectationKind::EnvApiKeySource(written) => Some(ExpectationKind::EnvApiKeySource {
            equals: stated(
                written.equals,
                &at(location, "env.api_key_source", "equals"),
                "credential source",
                errors,
            )?,
        }),

        RawExpectationKind::SkillInvoked(written) => {
            let (skill, count) = skill_count(written, "skill.invoked", location, errors)?;
            Some(ExpectationKind::SkillInvoked { skill, count })
        }
        RawExpectationKind::SkillCompleted(written) => {
            let (skill, count) = skill_count(written, "skill.completed", location, errors)?;
            Some(ExpectationKind::SkillCompleted { skill, count })
        }

        RawExpectationKind::ToolCalled(written) => tool_called(written, location, errors),
        RawExpectationKind::ToolAbsent(written) => tool_absent(written, location, errors),
        RawExpectationKind::ToolResult(written) => tool_result(written, location, errors),
        RawExpectationKind::ToolResultBytes(written) => {
            tool_result_bytes(written, location, errors)
        }
        RawExpectationKind::ToolFailed(written) => {
            let (selector, count) = scoped_count(written, "tool.failed", location, errors)?;
            Some(ExpectationKind::ToolFailed { selector, count })
        }
        RawExpectationKind::ToolErrorRate(written) => tool_error_rate(written, location, errors),
        RawExpectationKind::ToolRepeated(written) => {
            let (selector, count) = scoped_count(written, "tool.repeated", location, errors)?;
            Some(ExpectationKind::ToolRepeated { selector, count })
        }
        RawExpectationKind::Order(written) => order(written, location, errors),
        RawExpectationKind::RunResult(written) => run_result(written, location, errors),
        RawExpectationKind::PermissionDenied(written) => Some(ExpectationKind::PermissionDenied {
            count: count_of(
                written.count,
                &at(location, "permission.denied", "count"),
                errors,
            )?,
        }),
        RawExpectationKind::SubagentSpawned(written) => Some(ExpectationKind::SubagentSpawned {
            count: count_of(
                written.count,
                &at(location, "subagent.spawned", "count"),
                errors,
            )?,
        }),
        RawExpectationKind::TextMatches(written) => text_matches(written, location, errors),

        RawExpectationKind::RateLimitStatus(written) => Some(ExpectationKind::RateLimitStatus {
            allowed: names_of(
                written.allowed,
                &at(location, "rate_limit.status", "allowed"),
                "status",
                "an empty allowlist permits no rate-limit status at all, so every run fails it",
                errors,
            )?,
        }),
        RawExpectationKind::RateLimitOverage(written) => Some(ExpectationKind::RateLimitOverage {
            equals: written.equals,
        }),
        RawExpectationKind::RateLimitUtilization(written) => {
            Some(ExpectationKind::RateLimitUtilization {
                utilization: range_of(
                    written,
                    &at_kind(location, "rate_limit.utilization"),
                    errors,
                )?,
            })
        }

        RawExpectationKind::Turns(written) => Some(ExpectationKind::Turns {
            count: count_of(written.count, &at(location, "turns", "count"), errors)?,
        }),
        RawExpectationKind::ApiRequests(written) => Some(ExpectationKind::ApiRequests {
            count: count_of(
                written.count,
                &at(location, "api_requests", "count"),
                errors,
            )?,
        }),
        RawExpectationKind::EventsAssistant(written) => Some(ExpectationKind::EventsAssistant {
            count: count_of(
                written.count,
                &at(location, "events.assistant", "count"),
                errors,
            )?,
        }),
        RawExpectationKind::Iterations(written) => Some(ExpectationKind::Iterations {
            count: count_of(written.count, &at(location, "iterations", "count"), errors)?,
        }),

        RawExpectationKind::TokensInput(written) => {
            let (count, model) = model_count(written, "tokens.input", location, errors)?;
            Some(ExpectationKind::TokensInput { count, model })
        }
        RawExpectationKind::TokensOutput(written) => {
            let (count, model) = model_count(written, "tokens.output", location, errors)?;
            Some(ExpectationKind::TokensOutput { count, model })
        }
        RawExpectationKind::TokensTotal(written) => {
            let (count, model) = model_count(written, "tokens.total", location, errors)?;
            Some(ExpectationKind::TokensTotal { count, model })
        }
        RawExpectationKind::TokensThinking(written) => Some(ExpectationKind::TokensThinking {
            count: count_of(
                written.count,
                &at(location, "tokens.thinking", "count"),
                errors,
            )?,
        }),
        RawExpectationKind::ThinkingEstimated(written) => {
            Some(ExpectationKind::ThinkingEstimated {
                count: count_of(
                    written.count,
                    &at(location, "thinking.estimated", "count"),
                    errors,
                )?,
            })
        }
        RawExpectationKind::CostTotal(written) => cost_total(written, location, errors),
        RawExpectationKind::CacheUsed(written) => Some(ExpectationKind::CacheUsed {
            equals: written.equals,
        }),
        RawExpectationKind::CacheReadTokens(written) => {
            let (count, model) = model_count(written, "cache.read_tokens", location, errors)?;
            Some(ExpectationKind::CacheReadTokens { count, model })
        }
        RawExpectationKind::CacheCreatedTokens(written) => {
            let (count, model) = model_count(written, "cache.created_tokens", location, errors)?;
            Some(ExpectationKind::CacheCreatedTokens { count, model })
        }
        RawExpectationKind::CacheHitRatio(written) => Some(ExpectationKind::CacheHitRatio {
            ratio: range_of(written, &at_kind(location, "cache.hit_ratio"), errors)?,
        }),

        RawExpectationKind::DurationTotal(written) => Some(ExpectationKind::DurationTotal {
            ms: count_of(written.ms, &at(location, "duration.total", "ms"), errors)?,
        }),
        RawExpectationKind::DurationApi(written) => Some(ExpectationKind::DurationApi {
            ms: count_of(written.ms, &at(location, "duration.api", "ms"), errors)?,
        }),
        RawExpectationKind::Ttft(written) => Some(ExpectationKind::Ttft {
            ms: count_of(written.ms, &at(location, "ttft", "ms"), errors)?,
        }),
        RawExpectationKind::TimeToRequest(written) => Some(ExpectationKind::TimeToRequest {
            ms: count_of(written.ms, &at(location, "time_to_request", "ms"), errors)?,
        }),
        RawExpectationKind::StepGenTime(written) => {
            let (selector, ms) = step_ms(written, "step.gen_time", location, errors)?;
            Some(ExpectationKind::StepGenTime { selector, ms })
        }
        RawExpectationKind::StepExecTime(written) => {
            let (selector, ms) = step_ms(written, "step.exec_time", location, errors)?;
            Some(ExpectationKind::StepExecTime { selector, ms })
        }
        RawExpectationKind::TimeInferenceTotal(written) => {
            Some(ExpectationKind::TimeInferenceTotal {
                ms: count_of(
                    written.ms,
                    &at(location, "time.inference_total", "ms"),
                    errors,
                )?,
            })
        }
        RawExpectationKind::TimeToolExecTotal(written) => {
            Some(ExpectationKind::TimeToolExecTotal {
                ms: count_of(
                    written.ms,
                    &at(location, "time.tool_exec_total", "ms"),
                    errors,
                )?,
            })
        }

        RawExpectationKind::Speed(written) => Some(ExpectationKind::Speed {
            equals: stated(
                written.equals,
                &at(location, "speed", "equals"),
                "speed tier",
                errors,
            )?,
        }),
        RawExpectationKind::ServiceTier(written) => Some(ExpectationKind::ServiceTier {
            equals: stated(
                written.equals,
                &at(location, "service_tier", "equals"),
                "service tier",
                errors,
            )?,
        }),
    }
}

/// Validates `env.plugin_loaded`: all three parameters are checked before any is propagated.
fn plugin_loaded(
    written: RawPluginLoaded,
    location: &str,
    errors: &mut ValidationErrors,
) -> Option<ExpectationKind> {
    let kind = "env.plugin_loaded";
    let mut usable = true;
    let plugin = stated(
        written.plugin,
        &at(location, kind, "plugin"),
        "plugin name",
        errors,
    );
    let version = optionally_stated(
        written.version,
        &at(location, kind, "version"),
        "version",
        &mut usable,
        errors,
    );
    let source = optionally_stated(
        written.source,
        &at(location, kind, "source"),
        "source",
        &mut usable,
        errors,
    );
    let plugin = plugin?;
    usable.then_some(ExpectationKind::EnvPluginLoaded {
        plugin,
        version,
        source,
    })
}

/// Validates `env.tool_available`: exactly one of `tool` and `only`, and `available` only where
/// `tool` names something for it to be about.
///
/// The three refusals here are the whole reason this kind is not two lines like the two beside
/// it. Each one is a document somebody meant to finish, and each message names the line to
/// delete rather than the rule that was broken.
fn tool_available(
    written: RawToolAvailable,
    location: &str,
    errors: &mut ValidationErrors,
) -> Option<ExpectationKind> {
    let kind = "env.tool_available";
    let availability = match (written.tool, written.only) {
        (Some(_), Some(_)) => {
            errors.refuse(
                TraceCode::SpecInvalidExpectation,
                at_kind(location, kind),
                "`tool` and `only` are two different claims — one tool was offered, against \
                 exactly these were — and an expectation holding both reports one verdict for two",
            );
            return None;
        }
        (None, None) => {
            errors.refuse(
                TraceCode::SpecInvalidExpectation,
                at_kind(location, kind),
                "an `env.tool_available` with neither `tool` nor `only` names no tool, so it can \
                 only report a gap",
            );
            return None;
        }
        (Some(tool), None) => {
            let tool = stated(tool, &at(location, kind, "tool"), "tool name", errors)?;
            if written.available.unwrap_or(true) {
                ToolAvailability::Offered { tool }
            } else {
                ToolAvailability::NotOffered { tool }
            }
        }
        (None, Some(only)) => {
            if written.available.is_some() {
                errors.refuse(
                    TraceCode::SpecInvalidExpectation,
                    at(location, kind, "available"),
                    "`available` is about the one tool `tool` names; beside `only` it has no \
                     reading, because `only` already says of every tool whether it was offered",
                );
                return None;
            }
            ToolAvailability::Only {
                tools: names_of(
                    only,
                    &at(location, kind, "only"),
                    "tool",
                    "an `env.tool_available` listing no tool under `only` is an unfinished \
                     document far more often than it is the claim that no tool at all was \
                     offered, and the two read the same",
                    errors,
                )?,
            }
        }
    };
    Some(ExpectationKind::EnvToolAvailable { availability })
}

/// Validates the two skill kinds, whose `count` defaults to `{at_least: 1}`.
fn skill_count(
    written: RawSkillCount,
    kind: &str,
    location: &str,
    errors: &mut ValidationErrors,
) -> Option<(String, CountBound)> {
    let skill = stated(
        written.skill,
        &at(location, kind, "skill"),
        "skill name",
        errors,
    );
    let count = default_count(written.count, &at(location, kind, "count"), errors);
    Some((skill?, count?))
}

/// `true` when every text there is satisfies the matcher, the empty one included.
fn matches_every_text(matcher: &FieldMatcher) -> bool {
    match matcher {
        FieldMatcher::Contains(pattern) => pattern.is_empty(),
        FieldMatcher::Glob(pattern) => {
            !pattern.is_empty() && pattern.chars().all(|character| character == '*')
        }
        _ => false,
    }
}

/// Validates `tool.called`, whose `count` defaults to `{at_least: 1}`.
///
/// An unscoped selector is **allowed** here, unlike on `tool.absent`: *"the agent called at least
/// one tool"* is a claim a transcript decides and a run can fail.
fn tool_called(
    written: RawToolCalled,
    location: &str,
    errors: &mut ValidationErrors,
) -> Option<ExpectationKind> {
    let kind = "tool.called";
    let selector = selector_of(written.tool, written.args, &at_kind(location, kind), errors);
    let count = default_count(written.count, &at(location, kind, "count"), errors);
    Some(ExpectationKind::ToolCalled {
        selector: selector?,
        count: count?,
    })
}

/// Validates `tool.absent`, the one kind an unscoped selector makes senseless.
///
/// No tool and no argument matcher forbids *every* tool call, which no useful agent run
/// satisfies. It is refused rather than evaluated, because the specification that meant it is
/// vanishingly rarer than the one that lost its `tool:` line.
fn tool_absent(
    written: RawSelector,
    location: &str,
    errors: &mut ValidationErrors,
) -> Option<ExpectationKind> {
    let kind = "tool.absent";
    let selector = selector_of(written.tool, written.args, &at_kind(location, kind), errors)?;
    if selector.is_unscoped() {
        errors.refuse(
            TraceCode::SpecInvalidExpectation,
            at_kind(location, kind),
            "a `tool.absent` with no tool and no argument matcher forbids every tool call, which \
             is almost always a lost `tool:` line rather than the claim it makes",
        );
        return None;
    }
    Some(ExpectationKind::ToolAbsent { selector })
}

/// Validates `tool.result`. The result matcher must name at least one field: a `tool.result` over
/// no field is `tool.called` written at greater length.
fn tool_result(
    written: RawToolResult,
    location: &str,
    errors: &mut ValidationErrors,
) -> Option<ExpectationKind> {
    let kind = "tool.result";
    let selector = selector_of(written.tool, written.args, &at_kind(location, kind), errors);
    let result = result_of(written.result, &at(location, kind, "result"), errors);
    Some(ExpectationKind::ToolResultMatches {
        selector: selector?,
        result: result?,
    })
}

/// Validates `tool.result_bytes`.
fn tool_result_bytes(
    written: RawToolResultBytes,
    location: &str,
    errors: &mut ValidationErrors,
) -> Option<ExpectationKind> {
    let kind = "tool.result_bytes";
    let selector = selector_of(written.tool, written.args, &at_kind(location, kind), errors);
    let bytes = count_of(written.bytes, &at(location, kind, "bytes"), errors);
    Some(ExpectationKind::ToolResultBytes {
        selector: selector?,
        bytes: bytes?,
        per: written.per.into(),
    })
}

/// Validates `tool.error_rate`.
fn tool_error_rate(
    written: RawToolErrorRate,
    location: &str,
    errors: &mut ValidationErrors,
) -> Option<ExpectationKind> {
    let kind = "tool.error_rate";
    let selector = selector_of(written.tool, written.args, &at_kind(location, kind), errors);
    let rate = range_of(written.rate, &at(location, kind, "rate"), errors);
    Some(ExpectationKind::ToolErrorRate {
        selector: selector?,
        rate: rate?,
    })
}

/// Validates the two scoped kinds whose bound is mandatory.
fn scoped_count(
    written: RawScopedCount,
    kind: &str,
    location: &str,
    errors: &mut ValidationErrors,
) -> Option<(CallSelector, CountBound)> {
    let selector = selector_of(written.tool, written.args, &at_kind(location, kind), errors);
    let count = count_of(written.count, &at(location, kind, "count"), errors);
    Some((selector?, count?))
}

/// Validates the two step-timing kinds.
fn step_ms(
    written: RawStepMs,
    kind: &str,
    location: &str,
    errors: &mut ValidationErrors,
) -> Option<(CallSelector, CountBound)> {
    let selector = selector_of(written.tool, written.args, &at_kind(location, kind), errors);
    let ms = count_of(written.ms, &at(location, kind, "ms"), errors);
    Some((selector?, ms?))
}

/// Validates the five kinds that may be scoped to one model.
fn model_count(
    written: RawModelCount,
    kind: &str,
    location: &str,
    errors: &mut ValidationErrors,
) -> Option<(CountBound, Option<String>)> {
    let mut usable = true;
    let count = count_of(written.count, &at(location, kind, "count"), errors);
    let model = optionally_stated(
        written.model,
        &at(location, kind, "model"),
        "model",
        &mut usable,
        errors,
    );
    let count = count?;
    usable.then_some((count, model))
}

/// Validates `cost.total`, whose two keys carry their unit.
fn cost_total(
    written: RawCostTotal,
    location: &str,
    errors: &mut ValidationErrors,
) -> Option<ExpectationKind> {
    let kind = "cost.total";
    let mut usable = true;
    let usd = range_of(
        RawRangeBound {
            at_least: written.at_least_usd,
            at_most: written.at_most_usd,
        },
        &at_kind(location, kind),
        errors,
    );
    let model = optionally_stated(
        written.model,
        &at(location, kind, "model"),
        "model",
        &mut usable,
        errors,
    );
    let usd = usd?;
    usable.then_some(ExpectationKind::CostTotal { usd, model })
}

/// Validates `order`, refusing the pair that asks a call to precede itself.
///
/// Both sides are validated before either refusal is propagated, so a document with two broken
/// selectors reports two refusals (invariant 3).
fn order(
    written: RawOrder,
    location: &str,
    errors: &mut ValidationErrors,
) -> Option<ExpectationKind> {
    let kind = "order";
    let first = selector_of(
        written.first.tool,
        written.first.args,
        &at(location, kind, "first"),
        errors,
    );
    let before = selector_of(
        written.before.tool,
        written.before.args,
        &at(location, kind, "before"),
        errors,
    );
    let (first, before) = (first?, before?);
    if first == before {
        errors.refuse(
            TraceCode::SpecInvalidExpectation,
            at_kind(location, kind),
            format!(
                "both sides select the same calls ({first}), and the first occurrence of a call \
                 cannot precede itself"
            ),
        );
        return None;
    }
    Some(ExpectationKind::Order { first, before })
}

/// Validates `result`.
///
/// A record expectation that states none of its five fields decides nothing: it holds for every
/// transcript that has a terminal record at all, which is a green verdict about the transcript's
/// existence.
fn run_result(
    written: RawRunResult,
    location: &str,
    errors: &mut ValidationErrors,
) -> Option<ExpectationKind> {
    let kind = "result";
    if written.is_error.is_none()
        && written.subtype.is_none()
        && written.stop_reason.is_none()
        && written.terminal_reason.is_none()
        && written.api_error_status.is_none()
    {
        errors.refuse(
            TraceCode::SpecInvalidExpectation,
            at_kind(location, kind),
            "a `result` that states none of `is_error`, `subtype`, `stop_reason`, \
             `terminal_reason` or `api_error_status` holds for every run that produced a terminal \
             record, which is a green verdict about nothing",
        );
        return None;
    }
    let mut usable = true;
    let subtype = optionally_stated(
        written.subtype,
        &at(location, kind, "subtype"),
        "subtype",
        &mut usable,
        errors,
    );
    let stop_reason = optionally_stated(
        written.stop_reason,
        &at(location, kind, "stop_reason"),
        "stop reason",
        &mut usable,
        errors,
    );
    let terminal_reason = optionally_stated(
        written.terminal_reason,
        &at(location, kind, "terminal_reason"),
        "terminal reason",
        &mut usable,
        errors,
    );
    let api_error_status = match written.api_error_status {
        None => None,
        Some(RawApiErrorStatus::Absent) => Some(ApiErrorStatus::Absent),
        Some(RawApiErrorStatus::Equals(value)) => {
            let value = optionally_stated(
                Some(value),
                &at(location, kind, "api_error_status.equals"),
                "API error status",
                &mut usable,
                errors,
            );
            value.map(|value| ApiErrorStatus::Equals { value })
        }
    };
    usable.then_some(ExpectationKind::RunResult {
        is_error: written.is_error,
        subtype,
        stop_reason,
        terminal_reason,
        api_error_status,
    })
}

/// Validates `text.matches`, the weakest kind in the vocabulary — which must at least be able to
/// fail.
///
/// `{contains: ""}` and `{glob: "*"}` hold for every text there is, including the empty one, so
/// they are refused: an expectation that cannot fail is a check that stopped checking, and this
/// is the one kind where the mistake is easy to make and invisible in the report.
fn text_matches(
    written: RawFieldMatcher,
    location: &str,
    errors: &mut ValidationErrors,
) -> Option<ExpectationKind> {
    let kind = "text.matches";
    let matcher = matcher_of(written, &at_kind(location, kind), errors)?;
    if matches_every_text(&matcher) {
        errors.refuse(
            TraceCode::SpecInvalidExpectation,
            at_kind(location, kind),
            format!("every text satisfies `{matcher}`, so this expectation can only report `ok`"),
        );
        return None;
    }
    Some(ExpectationKind::TextMatches { matcher })
}

/// Validates a call selector. Every argument matcher is checked before any refusal is propagated.
fn selector_of(
    tool: Option<String>,
    args: BTreeMap<String, RawFieldMatcher>,
    location: &str,
    errors: &mut ValidationErrors,
) -> Option<CallSelector> {
    let mut usable = true;
    let tool = match tool {
        Some(name) if name.trim().is_empty() => {
            errors.refuse(
                TraceCode::SpecInvalidExpectation,
                format!("{location}.tool"),
                "a blank tool name matches no call, and reads like the omission that would have \
                 matched every one",
            );
            usable = false;
            None
        }
        other => other,
    };
    let mut matchers = BTreeMap::new();
    for (field, written) in args {
        let at = format!("{location}.args.{field}");
        if field.trim().is_empty() {
            errors.refuse(
                TraceCode::SpecInvalidExpectation,
                format!("{location}.args"),
                "an argument matcher names the argument it is about, and a blank name names none",
            );
            usable = false;
            continue;
        }
        match matcher_of(written, &at, errors) {
            Some(matcher) => {
                matchers.insert(field, matcher);
            }
            None => usable = false,
        }
    }
    usable.then_some(CallSelector {
        tool,
        args: matchers,
    })
}

/// Validates a result matcher, which must name at least one field.
fn result_of(
    fields: BTreeMap<String, RawFieldMatcher>,
    location: &str,
    errors: &mut ValidationErrors,
) -> Option<ResultMatcher> {
    if fields.is_empty() {
        errors.refuse(
            TraceCode::SpecInvalidExpectation,
            location.to_owned(),
            "a result matcher over no field claims nothing about a result; `tool.called` is the \
             kind that asserts the call happened",
        );
        return None;
    }
    let mut usable = true;
    let mut matchers = BTreeMap::new();
    for (field, written) in fields {
        let at = format!("{location}.{field}");
        if field.trim().is_empty() {
            errors.refuse(
                TraceCode::SpecInvalidExpectation,
                location.to_owned(),
                "a result matcher names the field it is about, and a blank name names none",
            );
            usable = false;
            continue;
        }
        match matcher_of(written, &at, errors) {
            Some(matcher) => {
                matchers.insert(field, matcher);
            }
            None => usable = false,
        }
    }
    usable.then_some(ResultMatcher { fields: matchers })
}

/// Validates one field matcher, and refuses `regex:` by name.
fn matcher_of(
    raw: RawFieldMatcher,
    location: &str,
    errors: &mut ValidationErrors,
) -> Option<FieldMatcher> {
    match raw {
        RawFieldMatcher::Exact(value) => Some(FieldMatcher::Exact(value)),
        RawFieldMatcher::Contains(value) => Some(FieldMatcher::Contains(value)),
        RawFieldMatcher::Glob(value) => Some(FieldMatcher::Glob(value)),
        RawFieldMatcher::Equals(value) => {
            scalar_of(value, location, errors).map(FieldMatcher::Equals)
        }
        RawFieldMatcher::Regex(pattern) => {
            errors.refuse(
                TraceCode::SpecUnsupportedMatcher,
                location.to_owned(),
                format!(
                    "`regex: {pattern:?}` is a matcher this build does not implement, and it is \
                     refused by name rather than read as `contains:` — which would be a \
                     specification that means something other than what it says. Write `glob:` \
                     instead: `*` for any run of characters, `?` for one, anchored at both ends"
                ),
            );
            None
        }
    }
}

/// Validates a scalar under `equals:`, refusing the fraction [`ScalarValue`] deliberately cannot
/// hold.
fn scalar_of(raw: RawScalar, location: &str, errors: &mut ValidationErrors) -> Option<ScalarValue> {
    match raw {
        RawScalar::Bool(value) => Some(ScalarValue::Bool(value)),
        RawScalar::Integer(value) => Some(ScalarValue::Integer(value)),
        RawScalar::Text(value) => Some(ScalarValue::Text(value)),
        RawScalar::Fraction(value) => {
            errors.refuse(
                TraceCode::SpecInvalidExpectation,
                location.to_owned(),
                format!(
                    "`equals: {value}` asks for equality against a fraction, which is the \
                     comparison design decision D6 refuses: every fractional quantity here \
                     varies run to run. Write a bound — `{{at_most: …}}` or `{{at_least: …}}` — \
                     on the kind that carries one"
                ),
            );
            None
        }
    }
}

/// Validates a count bound: it must state a side, and it must be satisfiable.
fn count_of(
    raw: RawCountBound,
    location: &str,
    errors: &mut ValidationErrors,
) -> Option<CountBound> {
    let bound = CountBound {
        at_least: raw.at_least,
        at_most: raw.at_most,
        exactly: raw.exactly,
    };
    if bound.is_empty() {
        errors.refuse(
            TraceCode::SpecInvalidExpectation,
            location.to_owned(),
            "a bound with no side accepts every number, which is not a bound; write `at_least`, \
             `at_most` or `exactly`",
        );
        return None;
    }
    if bound.exactly.is_some() && (bound.at_least.is_some() || bound.at_most.is_some()) {
        errors.refuse(
            TraceCode::SpecUnsatisfiableBound,
            location.to_owned(),
            format!(
                "`exactly` states the only acceptable value, so it cannot be combined with a \
                 floor or a ceiling; this bound says `{bound}` and silently ignores the rest"
            ),
        );
        return None;
    }
    if bound.is_unsatisfiable() {
        errors.refuse(
            TraceCode::SpecUnsatisfiableBound,
            location.to_owned(),
            format!(
                "the floor is above the ceiling ({bound}), so no observed value can satisfy it"
            ),
        );
        return None;
    }
    Some(bound)
}

/// Validates a count bound that may be omitted, where omission means `{at_least: 1}`.
///
/// An explicitly written `count: {}` is refused rather than read as that default: an author who
/// typed a bound and got none should be told which of the two happened.
fn default_count(
    raw: Option<RawCountBound>,
    location: &str,
    errors: &mut ValidationErrors,
) -> Option<CountBound> {
    match raw {
        Some(written) => count_of(written, location, errors),
        None => Some(CountBound::at_least(1)),
    }
}

/// Validates a range bound: it must state a side, and it must be satisfiable.
fn range_of(
    raw: RawRangeBound,
    location: &str,
    errors: &mut ValidationErrors,
) -> Option<RangeBound> {
    let bound = RangeBound {
        at_least: raw.at_least,
        at_most: raw.at_most,
    };
    if bound.is_empty() {
        errors.refuse(
            TraceCode::SpecInvalidExpectation,
            location.to_owned(),
            "a bound with no side accepts every value, which is not a bound; write `at_least` or \
             `at_most`",
        );
        return None;
    }
    if bound.is_unsatisfiable() {
        errors.refuse(
            TraceCode::SpecUnsatisfiableBound,
            location.to_owned(),
            format!(
                "the floor is above the ceiling ({bound}), so no observed value can satisfy it"
            ),
        );
        return None;
    }
    Some(bound)
}

/// Validates a name an expectation is about: present, and not blank.
fn stated(
    value: String,
    location: &str,
    noun: &str,
    errors: &mut ValidationErrors,
) -> Option<String> {
    if value.trim().is_empty() {
        errors.refuse(
            TraceCode::SpecInvalidExpectation,
            location.to_owned(),
            format!("a blank {noun} names nothing, so this expectation can only report a gap"),
        );
        return None;
    }
    Some(value)
}

/// Validates an optional name: absent is fine, blank is not.
///
/// "The document did not pin it" and "the document pinned nothing at all" are both `None` here
/// and they are not the same outcome, so the second is reported through `usable` rather than
/// through the return — a nested option would make every call site read the difference wrong.
fn optionally_stated(
    value: Option<String>,
    location: &str,
    noun: &str,
    usable: &mut bool,
    errors: &mut ValidationErrors,
) -> Option<String> {
    match value {
        None => None,
        Some(value) => {
            let stated = stated(value, location, noun, errors);
            *usable &= stated.is_some();
            stated
        }
    }
}

/// Validates a set of names: non-empty, with no blank entry.
fn names_of(
    values: Vec<String>,
    location: &str,
    noun: &str,
    empty: &str,
    errors: &mut ValidationErrors,
) -> Option<BTreeSet<String>> {
    if values.is_empty() {
        errors.refuse(
            TraceCode::SpecInvalidExpectation,
            location.to_owned(),
            empty.to_owned(),
        );
        return None;
    }
    if values.iter().any(|value| value.trim().is_empty()) {
        errors.refuse(
            TraceCode::SpecInvalidExpectation,
            location.to_owned(),
            format!("a blank entry is not a {noun}"),
        );
        return None;
    }
    Some(values.into_iter().collect())
}

/// `true` when `value` is a stable identifier: lowercase letters, digits and dashes, not starting
/// with a dash.
fn is_identifier(value: &str) -> bool {
    value
        .starts_with(|character: char| character.is_ascii_lowercase() || character.is_ascii_digit())
        && value.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
        })
}

/// `true` when `value` is a specification id: an identifier, or two separated by one `/`.
///
/// The `/` is the namespace the design writes — `planning-plugin/eval` — and exactly one is
/// allowed, because a path with two is a filename and a specification id is not a filename
/// (invariant 10).
fn is_document_id(value: &str) -> bool {
    let mut segments = value.split('/');
    let first = segments.next().unwrap_or_default();
    match (segments.next(), segments.next()) {
        (None, _) => is_identifier(first),
        (Some(second), None) => is_identifier(first) && is_identifier(second),
        (Some(_), Some(_)) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A document around an expectation list.
    fn document(body: &str) -> String {
        format!("format: trace-spec/1\nid: planning-plugin/eval\nexpectations:\n{body}")
    }

    /// A one-expectation document around an `expect:` value.
    fn one(expect: &str) -> String {
        document(&format!("  - id: only\n    expect: {expect}\n"))
    }

    /// The refusals a text produces, or a panic if it validates.
    fn refusals(text: &str) -> ValidationErrors {
        read_spec(text).expect_err("this document is meant to be refused")
    }

    /// The specification a text produces, or a panic naming what was refused.
    fn accepted(text: &str) -> TraceSpec {
        read_spec(text).unwrap_or_else(|errors| panic!("this document must validate: {errors}"))
    }

    /// The design's own example, in the wire form this build reads.
    const REALISTIC_YAML: &str = r#"
format: trace-spec/1
id: planning-plugin/eval
title: The planning plugin behaves as its skill says it does

expectations:
  - id: our-plugin-loaded
    statement: only our plugin was loaded
    expect:
      env.plugin_loaded:
        plugin: engineering-protocols
        version: "0.1.0"
        source: engineering-protocols@inline

  - id: nothing-else-loaded
    expect:
      env.exclusive:
        plugins: [engineering-protocols]

  - id: billed-to-the-session
    expect:
      env.api_key_source: {equals: none}

  - id: skill-completed
    expect:
      skill.completed:
        skill: "engineering-protocols:planning"
        count: {at_least: 1}

  - id: created-through-the-cli
    expect:
      tool.called:
        tool: Bash
        args: {command: {contains: "protocol artifact new"}}
        count: {at_least: 1}

  - id: no-hand-edited-frontmatter
    expect:
      tool.absent:
        tool: Edit
        args: {file_path: {glob: "*/.engineering/planning/*.md"}}

  - id: no-edit-was-touched-by-a-human
    expect:
      tool.result:
        tool: Edit
        result: {userModified: {equals: false}}

  - id: asked-before-writing
    expect:
      order:
        first: {tool: Bash, args: {command: {contains: "protocol artifact"}}}
        before: {tool: Edit}

  - id: within-budget
    severity: advisory
    expect:
      cost.total: {at_most_usd: 1.0}

  - id: not-paid-from-overage
    expect:
      rate_limit.overage: {equals: false}

  - id: the-run-records-its-own-turns
    on_unknown: gap
    expect:
      turns: {count: {at_most: 30}}

  - id: read-results-stayed-small
    expect:
      tool.result_bytes:
        tool: Read
        bytes: {at_most: 200000}
        per: each
"#;

    /// The same document, in the other syntax the one reader accepts.
    const REALISTIC_JSON: &str = r#"
{
  "format": "trace-spec/1",
  "id": "planning-plugin/eval",
  "title": "The planning plugin behaves as its skill says it does",
  "expectations": [
    { "id": "our-plugin-loaded",
      "statement": "only our plugin was loaded",
      "expect": { "env.plugin_loaded": { "plugin": "engineering-protocols",
                                         "version": "0.1.0",
                                         "source": "engineering-protocols@inline" } } },
    { "id": "nothing-else-loaded",
      "expect": { "env.exclusive": { "plugins": ["engineering-protocols"] } } },
    { "id": "billed-to-the-session",
      "expect": { "env.api_key_source": { "equals": "none" } } },
    { "id": "skill-completed",
      "expect": { "skill.completed": { "skill": "engineering-protocols:planning",
                                       "count": { "at_least": 1 } } } },
    { "id": "created-through-the-cli",
      "expect": { "tool.called": { "tool": "Bash",
                                   "args": { "command": { "contains": "protocol artifact new" } },
                                   "count": { "at_least": 1 } } } },
    { "id": "no-hand-edited-frontmatter",
      "expect": { "tool.absent": { "tool": "Edit",
                                   "args": { "file_path": { "glob": "*/.engineering/planning/*.md" } } } } },
    { "id": "no-edit-was-touched-by-a-human",
      "expect": { "tool.result": { "tool": "Edit",
                                   "result": { "userModified": { "equals": false } } } } },
    { "id": "asked-before-writing",
      "expect": { "order": { "first": { "tool": "Bash",
                                        "args": { "command": { "contains": "protocol artifact" } } },
                             "before": { "tool": "Edit" } } } },
    { "id": "within-budget",
      "severity": "advisory",
      "expect": { "cost.total": { "at_most_usd": 1.0 } } },
    { "id": "not-paid-from-overage",
      "expect": { "rate_limit.overage": { "equals": false } } },
    { "id": "the-run-records-its-own-turns",
      "on_unknown": "gap",
      "expect": { "turns": { "count": { "at_most": 30 } } } },
    { "id": "read-results-stayed-small",
      "expect": { "tool.result_bytes": { "tool": "Read",
                                         "bytes": { "at_most": 200000 },
                                         "per": "each" } } }
  ]
}
"#;

    /// A minimal valid document for every published kind name.
    ///
    /// The table that stops the wire vocabulary and [`ExpectationKind::NAMES`] drifting apart. A
    /// kind added to one and not the other fails
    /// `every_published_kind_name_is_reachable_from_a_document`, which is the only thing that
    /// notices.
    const EVERY_KIND: &[(&str, &str)] = &[
        ("api_requests", "{api_requests: {count: {at_most: 20}}}"),
        (
            "cache.created_tokens",
            "{cache.created_tokens: {count: {at_most: 30000}}}",
        ),
        ("cache.hit_ratio", "{cache.hit_ratio: {at_least: 0.9}}"),
        (
            "cache.read_tokens",
            "{cache.read_tokens: {count: {at_least: 1}, model: claude-sonnet-5}}",
        ),
        ("cache.used", "{cache.used: {equals: true}}"),
        ("cost.total", "{cost.total: {at_most_usd: 1.0}}"),
        ("duration.api", "{duration.api: {ms: {at_most: 120000}}}"),
        (
            "duration.total",
            "{duration.total: {ms: {at_most: 120000}}}",
        ),
        (
            "env.agent_available",
            "{env.agent_available: {agent: reviewer}}",
        ),
        ("env.api_key_source", "{env.api_key_source: {equals: none}}"),
        (
            "env.exclusive",
            "{env.exclusive: {plugins: [engineering-protocols]}}",
        ),
        ("env.model", "{env.model: {equals: claude-sonnet-5}}"),
        ("env.output_style", "{env.output_style: {equals: default}}"),
        (
            "env.permission_mode",
            "{env.permission_mode: {equals: default}}",
        ),
        (
            "env.plugin_loaded",
            "{env.plugin_loaded: {plugin: engineering-protocols}}",
        ),
        (
            "env.skill_available",
            "{env.skill_available: {skill: planning}}",
        ),
        ("env.tool_available", "{env.tool_available: {tool: Bash}}"),
        (
            "events.assistant",
            "{events.assistant: {count: {at_most: 40}}}",
        ),
        ("iterations", "{iterations: {count: {exactly: 1}}}"),
        (
            "order",
            "{order: {first: {tool: Bash}, before: {tool: Edit}}}",
        ),
        (
            "permission.denied",
            "{permission.denied: {count: {exactly: 0}}}",
        ),
        (
            "rate_limit.overage",
            "{rate_limit.overage: {equals: false}}",
        ),
        (
            "rate_limit.status",
            "{rate_limit.status: {allowed: [allowed]}}",
        ),
        (
            "rate_limit.utilization",
            "{rate_limit.utilization: {at_most: 0.9}}",
        ),
        ("result", "{result: {is_error: false}}"),
        ("service_tier", "{service_tier: {equals: standard}}"),
        ("skill.completed", "{skill.completed: {skill: planning}}"),
        ("skill.invoked", "{skill.invoked: {skill: planning}}"),
        ("speed", "{speed: {equals: standard}}"),
        (
            "step.exec_time",
            "{step.exec_time: {tool: Bash, ms: {at_most: 200}}}",
        ),
        (
            "step.gen_time",
            "{step.gen_time: {tool: Edit, ms: {at_most: 20000}}}",
        ),
        (
            "subagent.spawned",
            "{subagent.spawned: {count: {at_most: 2}}}",
        ),
        ("text.matches", "{text.matches: {contains: refused}}"),
        (
            "thinking.estimated",
            "{thinking.estimated: {count: {at_least: 1}}}",
        ),
        (
            "time.inference_total",
            "{time.inference_total: {ms: {at_most: 60000}}}",
        ),
        (
            "time.tool_exec_total",
            "{time.tool_exec_total: {ms: {at_most: 5000}}}",
        ),
        ("time_to_request", "{time_to_request: {ms: {at_most: 500}}}"),
        ("tokens.input", "{tokens.input: {count: {at_most: 100000}}}"),
        (
            "tokens.output",
            "{tokens.output: {count: {at_most: 20000}}}",
        ),
        (
            "tokens.thinking",
            "{tokens.thinking: {count: {at_least: 1}}}",
        ),
        (
            "tokens.total",
            "{tokens.total: {count: {at_most: 200000}, model: claude-sonnet-5}}",
        ),
        ("tool.absent", "{tool.absent: {tool: Edit}}"),
        ("tool.called", "{tool.called: {tool: Bash}}"),
        (
            "tool.error_rate",
            "{tool.error_rate: {tool: Read, rate: {at_most: 0.1}}}",
        ),
        (
            "tool.failed",
            "{tool.failed: {tool: Read, count: {exactly: 0}}}",
        ),
        (
            "tool.repeated",
            "{tool.repeated: {tool: Read, count: {at_most: 1}}}",
        ),
        (
            "tool.result",
            "{tool.result: {tool: Edit, result: {userModified: {equals: false}}}}",
        ),
        (
            "tool.result_bytes",
            "{tool.result_bytes: {tool: Read, bytes: {at_most: 100000}}}",
        ),
        ("ttft", "{ttft: {ms: {at_most: 5000}}}"),
        ("turns", "{turns: {count: {at_most: 20}}}"),
    ];

    #[test]
    fn a_realistic_specification_validates_and_keeps_its_expectations_in_document_order() {
        let spec = accepted(REALISTIC_YAML);
        assert_eq!(spec.id, "planning-plugin/eval");
        assert_eq!(
            spec.title.as_deref(),
            Some("The planning plugin behaves as its skill says it does")
        );
        let ids: Vec<&str> = spec
            .expectations
            .iter()
            .map(|expectation| expectation.id.as_str())
            .collect();
        assert_eq!(
            ids,
            vec![
                "our-plugin-loaded",
                "nothing-else-loaded",
                "billed-to-the-session",
                "skill-completed",
                "created-through-the-cli",
                "no-hand-edited-frontmatter",
                "no-edit-was-touched-by-a-human",
                "asked-before-writing",
                "within-budget",
                "not-paid-from-overage",
                "the-run-records-its-own-turns",
                "read-results-stayed-small",
            ],
            "a report names verdicts in the order the document declares them"
        );
        assert_eq!(
            spec.expectations[0].statement.as_deref(),
            Some("only our plugin was loaded")
        );
        assert_eq!(
            spec.expectations[4].kind,
            ExpectationKind::ToolCalled {
                selector: {
                    let mut selector = CallSelector::tool("Bash");
                    selector.args.insert(
                        "command".to_owned(),
                        FieldMatcher::Contains("protocol artifact new".to_owned()),
                    );
                    selector
                },
                count: CountBound::at_least(1),
            }
        );
        assert_eq!(
            spec.expectations[11].kind,
            ExpectationKind::ToolResultBytes {
                selector: CallSelector::tool("Read"),
                bytes: CountBound::at_most(200_000),
                per: Aggregate::Each,
            }
        );
        assert_eq!(spec.kind_census().len(), 12, "twelve kinds, one each");
        assert_eq!(spec.digest.len(), 64);
    }

    #[test]
    fn the_same_document_written_as_json_is_the_same_specification() {
        // One reader, two syntaxes: `read_spec` parses JSON through the YAML entry point, so a
        // harness that emits JSON and a human who writes YAML cannot produce two specifications
        // that a report would name differently.
        let from_yaml = accepted(REALISTIC_YAML);
        let from_json = accepted(REALISTIC_JSON);
        assert_eq!(
            from_yaml.digest, from_json.digest,
            "the digest is over the document's content, and the content is the same"
        );
        assert_eq!(from_yaml, from_json);
    }

    #[test]
    fn four_defects_in_one_document_are_four_refusals_in_one_pass() {
        // Invariant 3, and the only thing that enforces it is an exact count per code: a
        // validator that returned on the first defect would report one.
        let refused = refusals(
            "format: trace-spec/2\n\
             id: planning-plugin/eval\n\
             expectations:\n\
             \x20 - id: same-id\n\
             \x20   expect: {tool.called: {tool: Bash, count: {at_least: 5, at_most: 2}}}\n\
             \x20 - id: same-id\n\
             \x20   expect: {tool.absent: {tool: Edit, args: {file_path: {regex: \"x\"}}}}\n",
        );
        assert_eq!(refused.len(), 4, "{refused}");
        assert_eq!(
            refused.count(TraceCode::SpecUnsupportedFormat),
            1,
            "{refused}"
        );
        assert_eq!(
            refused.count(TraceCode::SpecDuplicateExpectation),
            1,
            "{refused}"
        );
        assert_eq!(
            refused.count(TraceCode::SpecUnsatisfiableBound),
            1,
            "{refused}"
        );
        assert_eq!(
            refused.count(TraceCode::SpecUnsupportedMatcher),
            1,
            "{refused}"
        );
    }

    #[test]
    fn a_format_this_build_does_not_read_is_refused_with_its_own_code() {
        let refused = refusals(
            "format: trace-spec/2\nid: eval\nexpectations:\n  - id: a\n    expect: {turns: {count: {at_most: 5}}}\n",
        );
        assert_eq!(
            refused.count(TraceCode::SpecUnsupportedFormat),
            1,
            "{refused}"
        );
        assert_eq!(refused.len(), 1, "and nothing else is wrong: {refused}");
    }

    #[test]
    fn two_expectations_sharing_an_id_are_refused_because_a_report_names_a_verdict_by_it() {
        let refused = refusals(&document(
            "  - id: same\n    expect: {turns: {count: {at_most: 5}}}\n  - id: same\n    expect: {ttft: {ms: {at_most: 5000}}}\n",
        ));
        assert_eq!(
            refused.count(TraceCode::SpecDuplicateExpectation),
            1,
            "{refused}"
        );
        assert_eq!(refused.len(), 1, "{refused}");
    }

    #[test]
    fn a_specification_that_expects_nothing_is_refused_rather_than_read_as_satisfied() {
        let refused =
            refusals("format: trace-spec/1\nid: planning-plugin/eval\nexpectations: []\n");
        assert_eq!(
            refused.count(TraceCode::SpecEmptyExpectations),
            1,
            "a report with no content reads exactly like a report with no gaps: {refused}"
        );
        assert_eq!(refused.len(), 1, "{refused}");
    }

    #[test]
    fn an_expectation_kind_this_build_does_not_implement_is_refused_by_name() {
        let refused = refusals(&one("{tool.kalled: {tool: Bash}}"));
        assert_eq!(refused.count(TraceCode::SpecMalformed), 1, "{refused}");
        assert_eq!(refused.len(), 1, "a malformed document refuses once, alone");
        assert!(
            refused.as_slice()[0].message.contains("tool.kalled"),
            "the refusal must name what was written, or the author cannot find it: {refused}"
        );
    }

    #[test]
    fn a_misspelt_parameter_inside_a_known_kind_is_refused_rather_than_defaulted() {
        // This is what nesting the kind under `expect:` buys. Flattened beside `id:`, the kind's
        // parameters would have to be read through `#[serde(flatten)]`, which silently drops
        // `deny_unknown_fields` — and `toool: Bash` would become "every tool", quietly widening
        // the expectation instead of refusing it.
        let refused = refusals(&one("{tool.called: {toool: Bash}}"));
        assert_eq!(refused.count(TraceCode::SpecMalformed), 1, "{refused}");
        assert!(refused.as_slice()[0].message.contains("toool"), "{refused}");

        let misspelt_bound = refusals(&one("{turns: {count: {at_leats: 1}}}"));
        assert_eq!(
            misspelt_bound.count(TraceCode::SpecMalformed),
            1,
            "`at_leats` must be refused rather than read as an unbounded count: {misspelt_bound}"
        );
    }

    #[test]
    fn a_regex_matcher_is_refused_by_name_and_the_message_says_to_write_a_glob() {
        let refused = refusals(&one(
            "{tool.absent: {tool: Edit, args: {file_path: {regex: \"\\\\.md$\"}}}}",
        ));
        assert_eq!(
            refused.count(TraceCode::SpecUnsupportedMatcher),
            1,
            "{refused}"
        );
        assert_eq!(refused.len(), 1, "{refused}");
        assert!(
            refused.as_slice()[0].message.contains("glob"),
            "a refusal that does not say what to write instead sends the author to the source: \
             {refused}"
        );
    }

    #[test]
    fn a_bound_that_cannot_be_satisfied_is_refused_whichever_way_it_was_written() {
        let inverted = refusals(&one("{turns: {count: {at_least: 5, at_most: 2}}}"));
        assert_eq!(
            inverted.count(TraceCode::SpecUnsatisfiableBound),
            1,
            "{inverted}"
        );

        let both_spellings = refusals(&one("{turns: {count: {exactly: 1, at_least: 1}}}"));
        assert_eq!(
            both_spellings.count(TraceCode::SpecUnsatisfiableBound),
            1,
            "`exactly` beside a floor is two bounds where the type holds one: {both_spellings}"
        );

        let range = refusals(&one("{cache.hit_ratio: {at_least: 0.9, at_most: 0.5}}"));
        assert_eq!(range.count(TraceCode::SpecUnsatisfiableBound), 1, "{range}");
    }

    #[test]
    fn an_equals_over_a_fraction_is_refused_with_the_advice_to_write_a_bound() {
        let refused = refusals(&one(
            "{tool.result: {tool: Bash, result: {duration: {equals: 0.5}}}}",
        ));
        assert_eq!(
            refused.count(TraceCode::SpecInvalidExpectation),
            1,
            "{refused}"
        );
        assert_eq!(refused.len(), 1, "{refused}");
        assert!(
            refused.as_slice()[0].message.contains("at_most"),
            "the refusal names the shape that would have worked: {refused}"
        );
    }

    #[test]
    fn a_bound_with_no_side_is_refused_but_an_omitted_count_means_it_happened() {
        let empty = refusals(&one("{tool.called: {tool: Bash, count: {}}}"));
        assert_eq!(
            empty.count(TraceCode::SpecInvalidExpectation),
            1,
            "an author who typed a bound and got none should be told: {empty}"
        );

        let omitted = accepted(&one("{tool.called: {tool: Bash}}"));
        assert_eq!(
            omitted.expectations[0].kind,
            ExpectationKind::ToolCalled {
                selector: CallSelector::tool("Bash"),
                count: CountBound::at_least(1),
            },
            "an omitted count is `{{at_least: 1}}`, which is what every author of it means"
        );
    }

    #[test]
    fn an_unscoped_tool_absent_forbids_every_tool_call_and_is_refused() {
        let refused = refusals(&one("{tool.absent: {}}"));
        assert_eq!(
            refused.count(TraceCode::SpecInvalidExpectation),
            1,
            "no tool and no argument matcher is a lost `tool:` line, not a claim: {refused}"
        );
        // The same selector on `tool.called` is a real claim — "the agent called at least one
        // tool" — so the refusal is about the kind, not about the selector.
        accepted(&one("{tool.called: {}}"));
    }

    #[test]
    fn an_order_whose_two_sides_select_the_same_calls_is_refused() {
        let refused = refusals(&one("{order: {first: {tool: Bash}, before: {tool: Bash}}}"));
        assert_eq!(
            refused.count(TraceCode::SpecInvalidExpectation),
            1,
            "no call precedes itself: {refused}"
        );
        accepted(&one("{order: {first: {tool: Bash}, before: {tool: Edit}}}"));
    }

    #[test]
    fn an_expectation_whose_parameters_decide_nothing_is_refused_in_each_of_its_shapes() {
        for (body, why) in [
            ("{env.exclusive: {plugins: []}}", "an empty exclusive set"),
            (
                "{env.tool_available: {only: []}}",
                "an empty offered-tool set",
            ),
            ("{rate_limit.status: {allowed: []}}", "an empty allowlist"),
            ("{env.plugin_loaded: {plugin: \"\"}}", "a blank plugin name"),
            (
                "{tool.result: {tool: Edit, result: {}}}",
                "a result matcher over no field",
            ),
            (
                "{result: {}}",
                "a terminal record expectation that states nothing",
            ),
            (
                "{text.matches: {contains: \"\"}}",
                "a matcher every text satisfies",
            ),
        ] {
            let refused = refusals(&one(body));
            assert_eq!(
                refused.count(TraceCode::SpecInvalidExpectation),
                1,
                "{why} must be refused: {refused}"
            );
        }
    }

    #[test]
    fn an_id_that_is_not_a_stable_identifier_is_refused_wherever_it_appears() {
        let refused = refusals(
            "format: trace-spec/1\nid: Planning Plugin\nexpectations:\n  - id: Not An Id\n    expect: {turns: {count: {at_most: 5}}}\n",
        );
        assert_eq!(
            refused.count(TraceCode::SpecMalformedId),
            2,
            "the document's id and the expectation's id are both refused in one pass: {refused}"
        );
        let too_deep = refusals(
            "format: trace-spec/1\nid: a/b/c\nexpectations:\n  - id: a\n    expect: {turns: {count: {at_most: 5}}}\n",
        );
        assert_eq!(
            too_deep.count(TraceCode::SpecMalformedId),
            1,
            "a specification id is a namespace and a name, not a path: {too_deep}"
        );
    }

    #[test]
    fn a_text_that_is_not_a_document_at_all_is_one_coded_refusal_and_not_a_serde_sentence() {
        let refused = refusals("format: trace-spec/1\nid: eval\nexpectations: 7\n");
        assert_eq!(refused.len(), 1, "{refused}");
        assert_eq!(refused.count(TraceCode::SpecMalformed), 1, "{refused}");

        let not_yaml = refusals("format: [unterminated\n");
        assert_eq!(not_yaml.count(TraceCode::SpecMalformed), 1, "{not_yaml}");
    }

    #[test]
    fn env_tool_available_reads_its_three_claims_and_refuses_the_ways_they_can_be_written_wrong() {
        // The 50th kind. `tool:` is the presence claim, `available: false` its negation, and
        // `only:` the exactness claim `env.exclusive` makes about plugins.
        for (body, expected) in [
            (
                "{env.tool_available: {tool: Bash}}",
                ToolAvailability::Offered {
                    tool: "Bash".to_owned(),
                },
            ),
            (
                "{env.tool_available: {tool: Bash, available: true}}",
                ToolAvailability::Offered {
                    tool: "Bash".to_owned(),
                },
            ),
            (
                "{env.tool_available: {tool: Task, available: false}}",
                ToolAvailability::NotOffered {
                    tool: "Task".to_owned(),
                },
            ),
            (
                "{env.tool_available: {only: [Read, Glob, Read]}}",
                ToolAvailability::Only {
                    tools: BTreeSet::from(["Glob".to_owned(), "Read".to_owned()]),
                },
            ),
        ] {
            let spec = accepted(&one(body));
            assert_eq!(
                spec.expectations[0].kind,
                ExpectationKind::EnvToolAvailable {
                    availability: expected
                },
                "`{body}` did not read as the claim it writes"
            );
            assert_eq!(spec.expectations[0].kind.name(), "env.tool_available");
        }

        for (body, why) in [
            (
                "{env.tool_available: {tool: Bash, only: [Bash]}}",
                "one tool and an exact set are two claims under one id",
            ),
            (
                "{env.tool_available: {}}",
                "neither `tool` nor `only` names a tool",
            ),
            (
                "{env.tool_available: {only: [Bash], available: false}}",
                "`available` has no reading beside `only`",
            ),
            (
                "{env.tool_available: {tool: \"\"}}",
                "a blank tool name names nothing",
            ),
            (
                "{env.tool_available: {only: [Bash, \"\"]}}",
                "a blank entry is not a tool",
            ),
        ] {
            let refused = refusals(&one(body));
            assert_eq!(
                refused.count(TraceCode::SpecInvalidExpectation),
                1,
                "{why} — `{body}` must be refused: {refused}"
            );
        }
    }

    #[test]
    fn skill_available_is_an_accepted_spelling_of_env_skill_available() {
        let canonical = accepted(&one("{env.skill_available: {skill: planning}}"));
        let alias = accepted(&one("{skill.available: {skill: planning}}"));
        assert_eq!(
            canonical.expectations[0].kind, alias.expectations[0].kind,
            "the design calls one the alias of the other, and a report must not name them apart"
        );
        assert_eq!(
            alias.expectations[0].kind.name(),
            "env.skill_available",
            "an alias is accepted on input; the canonical name is what is printed"
        );
        assert_eq!(
            canonical.digest, alias.digest,
            "two spellings of one document are one specification"
        );
    }

    #[test]
    fn severity_and_on_unknown_round_trip_and_default_to_gate_and_unknown() {
        let stated = accepted(&document(
            "  - id: a\n    severity: advisory\n    on_unknown: gap\n    expect: {speed: {equals: standard}}\n",
        ));
        assert_eq!(stated.expectations[0].severity, Severity::Advisory);
        assert_eq!(stated.expectations[0].on_unknown, OnUnknown::Gap);

        let omitted = accepted(&one("{speed: {equals: standard}}"));
        assert_eq!(
            omitted.expectations[0].severity,
            Severity::Gate,
            "a specification that gates nothing is a document nobody has to keep true"
        );
        assert_eq!(omitted.expectations[0].on_unknown, OnUnknown::Unknown);
    }

    #[test]
    fn every_published_kind_name_is_reachable_from_a_document_and_names_itself() {
        let mut covered = BTreeSet::new();
        for (name, expect) in EVERY_KIND {
            let spec = read_spec(&one(expect)).unwrap_or_else(|errors| {
                panic!("the document for `{name}` must validate: {errors}")
            });
            assert_eq!(
                spec.expectations.len(),
                1,
                "`{name}` produced no expectation"
            );
            assert_eq!(
                spec.expectations[0].kind.name(),
                *name,
                "the fragment written for `{name}` produced a different kind"
            );
            assert!(
                covered.insert(*name),
                "`{name}` is listed twice in the table"
            );
        }
        let published: BTreeSet<&str> = ExpectationKind::NAMES.iter().copied().collect();
        assert_eq!(
            covered, published,
            "the wire vocabulary and `ExpectationKind::NAMES` have drifted apart: a kind is \
             writable and unpublished, or published and unwritable"
        );
    }

    #[test]
    fn an_id_is_lowercase_dashed_and_a_document_id_may_carry_one_namespace() {
        assert!(is_identifier("our-plugin-loaded"));
        assert!(
            is_identifier("3-turns"),
            "a digit may lead an expectation id"
        );
        assert!(!is_identifier("Our-Plugin"), "upper case is not an id");
        assert!(!is_identifier("our_plugin"), "an underscore is not a dash");
        assert!(!is_identifier("-leading"), "a dash cannot lead");
        assert!(!is_identifier(""), "an empty id names nothing");

        assert!(is_document_id("planning-plugin/eval"));
        assert!(is_document_id("eval"));
        assert!(!is_document_id("a/b/c"), "a specification id is not a path");
        assert!(!is_document_id("planning/"), "the name half is missing");
    }
}
