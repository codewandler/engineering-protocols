//! `trace-spec/1` — what a run must have looked like.
//!
//! One document, a list of expectations, each with an id a verdict is reported under. It is
//! `infra-spec/1` pointed at a third observation domain and it reuses that family's shape
//! deliberately rather than inventing a parallel one: an author who has met one meets no new idea
//! in the other.
//!
//! # The bar for admitting a kind
//!
//! Not "somebody might want to assert it" — **can a transcript decide it, and can the report say
//! what it saw**. Every kind below reads a field the harness recorded. None is measured by the
//! checker, none reads a clock, and none calls a model. `ttft` is the clearest case: the brief for
//! the design assumed it would be derived as *first assistant timestamp minus first event*, and
//! the real transcript shows why that does not work — the first four events carry no timestamp at
//! all, so the subtraction would compute zero. The kind reads the recorded field or reports
//! `unk`.
//!
//! # Every kind names its own `unk`
//!
//! The `unk` arm is on each variant's documentation, because it is the part a reader will
//! otherwise assume away. Two rules run through all of them:
//!
//! * **a missing field is `unk`, never `false`** — every field here belongs to a format that is
//!   not a stable public schema (design D1), and an absent one means this transcript cannot
//!   answer the question;
//! * **a scope that selects nothing is `unk`, never `ok`** — the `infra-spec` rule, for the same
//!   reason: an expectation must not be able to pass by selecting nothing.
//!
//! An expectation may override the first with [`OnUnknown::Gap`], which is how a specification
//! says *"if this transcript cannot tell me, that is itself the failure"*. The default is
//! [`OnUnknown::Unknown`], and the default is what the exit codes are built around.
//!
//! # Severity, and why an advisory verdict is not a disabled one
//!
//! Design § 3.6 documents `speed` and `service_tier` "with `enabled: false` in every example",
//! and the eval's metrics block computes twelve numbers it is not allowed to have an opinion
//! about. Both want the same thing and neither should be spelled as *off*: a check that is
//! switched off reads exactly like a check that passed, which is the failure mode `AGENTS.md`
//! § *Gate* names.
//!
//! [`Severity::Advisory`] is the answer. An advisory expectation is evaluated, reported and
//! printed like any other — it simply does not move the exit code. That is what lets a cost bound
//! or a cache-ratio bound live in the document, in front of a reader, without a cold cache
//! turning a merge red.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use crate::digest::digest_of_canonical;
use crate::matcher::{CallSelector, CountBound, FieldMatcher, RangeBound, ResultMatcher};

/// The format string a specification carries.
pub const SPEC_FORMAT: &str = "trace-spec/1";

/// Whether an expectation's verdict moves the exit code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// A gap fails the check. The default, because a specification that gates nothing is a
    /// document nobody has to keep true.
    #[default]
    Gate,
    /// A gap is reported and printed, and the exit code does not move.
    ///
    /// For the quantities design D6 warns about — cost, tokens, duration, cache state — which
    /// vary run to run with model routing and load. Not a disabled check: the verdict is in the
    /// report, and a reader sees it.
    Advisory,
}

/// What an undecidable verdict means for this expectation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OnUnknown {
    /// Report `unk`. The default, and what exit code 3 is built around.
    #[default]
    Unknown,
    /// Report a gap.
    ///
    /// For the expectation whose whole point is that the transcript must carry the field —
    /// *"this run must record its own cost"* — where silence is the defect rather than an
    /// obstacle to finding one.
    Gap,
}

/// How a bound over a set of per-call values is applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Aggregate {
    /// The sum across every selected call. The context-budget reading, and the default.
    #[default]
    Total,
    /// Every selected call on its own. One call over the bound is a gap.
    Each,
}

/// What the terminal record's `api_error_status` must say.
///
/// Two spellings because the healthy case is *absence* and the interesting case is a particular
/// value, and collapsing them would make "no API error" unwritable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "api_error_status", rename_all = "snake_case")]
pub enum ApiErrorStatus {
    /// There must be no API error status at all.
    Absent,
    /// There must be one, and it must be this.
    Equals {
        /// The status expected.
        value: String,
    },
}

/// The v1 expectation vocabulary.
///
/// Forty-nine kinds across five families, each decidable from a transcript alone. The wire form
/// is externally tagged under `expect:` and keeps the design's dotted names verbatim —
/// `expect: {tool.called: {…}}` — so a kind this build does not implement is refused *by name*
/// and every kind's own parameters get `deny_unknown_fields`, which a flattened form cannot have
/// (serde's `deny_unknown_fields` does not survive `flatten`). A specification where `at_leats: 1`
/// silently became "unbounded" is worse than one that is a line longer.
///
/// Deliberately **not** `#[non_exhaustive]`. The checker in `trace-spec` matches this enum
/// exhaustively from another crate, and that is the point: a kind added here and not evaluated
/// there fails to compile, where a catch-all arm would silently report the new kind as something
/// it is not. Additivity for a downstream consumer is worth less than a compiler error for the
/// next person who adds a kind.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "expect")]
pub enum ExpectationKind {
    // --- the environment the run actually got --------------------------------------------
    /// A plugin with this name is loaded, optionally at this version and from this source.
    ///
    /// `unk` when the harness records no plugin list.
    #[serde(rename = "env.plugin_loaded")]
    EnvPluginLoaded {
        /// The plugin's name.
        plugin: String,
        /// The version it must be at, where the specification pins one.
        version: Option<String>,
        /// The source it must have come from, where the specification pins one.
        source: Option<String>,
    },
    /// The loaded plugins are **exactly** this set — nothing else leaked in.
    ///
    /// The only kind here that can fail on a correctly-behaving agent, which is the point: it
    /// reports on the *experiment*, not on the subject. Prevention is harness-specific and
    /// detection is not — a CI image that cannot redirect a config directory has no
    /// `CLAUDE_CONFIG_DIR`, and for those this assertion is the only control available. And a
    /// guard is verified by what it refuses: isolation that silently stops working is
    /// indistinguishable from isolation that works, because the run goes green either way.
    ///
    /// `unk` when the harness records no plugin list.
    #[serde(rename = "env.exclusive")]
    EnvExclusive {
        /// Every plugin that may be loaded, and no others.
        plugins: BTreeSet<String>,
    },
    /// The output style is the expected one, usually the default.
    ///
    /// `unk` when the harness records no output style. A non-default style is the operator's own
    /// configuration leaking into a run that was supposed to be sealed.
    #[serde(rename = "env.output_style")]
    EnvOutputStyle {
        /// The style expected.
        equals: String,
    },
    /// The named skill is among those the harness offered.
    ///
    /// Available is a different fact from invoked, and available-but-never-invoked is a real and
    /// interesting outcome: the plugin loaded and the model did not reach for it. `skill.available`
    /// is an accepted spelling of this kind.
    ///
    /// `unk` when the harness records no skill list.
    #[serde(rename = "env.skill_available")]
    EnvSkillAvailable {
        /// The skill's name, as the harness lists it.
        skill: String,
    },
    /// The named agent is among those the harness offered.
    ///
    /// `unk` when the harness records no agent list.
    #[serde(rename = "env.agent_available")]
    EnvAgentAvailable {
        /// The agent's name, as the harness lists it.
        agent: String,
    },
    /// The **resolved** model matches — what the alias on the command line turned into, not what
    /// was typed.
    ///
    /// `unk` when the harness records no model.
    #[serde(rename = "env.model")]
    EnvModel {
        /// The resolved model expected.
        equals: String,
    },
    /// The permission mode is the expected one.
    ///
    /// `unk` when the harness records no permission mode. Catches a run that silently asked for
    /// permissions, or one more permissive than the eval intended.
    #[serde(rename = "env.permission_mode")]
    EnvPermissionMode {
        /// The mode expected.
        equals: String,
    },
    /// The credential source is the expected one — `none` for a run that must bill the
    /// logged-in session.
    ///
    /// The one that has already bitten: an exported API key took precedence over the login and
    /// billed an account with no credits. This expectation would have caught it in the first
    /// event, before a turn was spent.
    ///
    /// `unk` when the harness records no credential source.
    #[serde(rename = "env.api_key_source")]
    EnvApiKeySource {
        /// The source expected.
        equals: String,
    },

    // --- the skill, in the two levels beyond availability --------------------------------
    /// The model **chose** the skill: a tool call naming it.
    ///
    /// `unk` when an opaque event could have been the call.
    #[serde(rename = "skill.invoked")]
    SkillInvoked {
        /// The skill's name.
        skill: String,
        /// How many invocations are acceptable.
        count: CountBound,
    },
    /// The skill ran to completion: its correlated result names it and reports success.
    ///
    /// **Structural, not textual.** The observed result object is a boolean the harness set, not
    /// a sentence the model wrote — which is what makes this the strongest claim in the family.
    ///
    /// `unk` when the invocation matched but no result was correlated to it.
    #[serde(rename = "skill.completed")]
    SkillCompleted {
        /// The skill's name.
        skill: String,
        /// How many completions are acceptable.
        count: CountBound,
    },

    // --- what the agent did ---------------------------------------------------------------
    /// Tool calls matching the selector fall within the bound.
    ///
    /// `unk` when the adapter met an opaque event that could have been a tool call.
    #[serde(rename = "tool.called")]
    ToolCalled {
        /// Which calls are in scope.
        selector: CallSelector,
        /// How many are acceptable.
        count: CountBound,
    },
    /// No tool call matches the selector.
    ///
    /// A kind of its own rather than `count: {exactly: 0}`, because *"this must never happen"* is
    /// the assertion people get wrong when they have to spell it as a bound.
    ///
    /// `unk` when an opaque event could have been such a call.
    #[serde(rename = "tool.absent")]
    ToolAbsent {
        /// Which calls must not have happened.
        selector: CallSelector,
    },
    /// Every matched call's **result** satisfies the result matcher.
    ///
    /// `tool.called` matches the request and this matches what came back; the two are different
    /// claims. A `Bash` call whose command matched and whose `interrupted` is `true` satisfies
    /// the first and should fail the second.
    ///
    /// `unk` when a call matched but no result was correlated, or when the result does not carry
    /// the field the matcher names — a truncated transcript is not a bad result, and a renamed
    /// field is not a broken agent.
    #[serde(rename = "tool.result")]
    ToolResultMatches {
        /// Which calls are in scope.
        selector: CallSelector,
        /// What their results must say.
        result: ResultMatcher,
    },
    /// The result bytes of the selected calls stay within a bound.
    ///
    /// A **context-budget guard**: a tool result is injected into the next request, where it
    /// costs input tokens and then sits in the context for the rest of the run. An observed run
    /// pushed about 1 848 tokens of tool output into its window with no aggregate in the terminal
    /// record accounting for it.
    ///
    /// `unk` when a selected call has no correlated result, or when nothing is in scope.
    #[serde(rename = "tool.result_bytes")]
    ToolResultBytes {
        /// Which calls are in scope.
        selector: CallSelector,
        /// The byte bound.
        bytes: CountBound,
        /// Whether the bound is over the sum or over each call.
        per: Aggregate,
    },
    /// How many selected calls came back flagged as errors stays within a bound.
    ///
    /// Scoped on purpose. A refusal this project designed is correct behaviour: `protocol
    /// artifact move` exits 1 when the move is illegal, and a run that asked, was refused and
    /// relayed the refusal behaved exactly right — and contains a failed tool call.
    ///
    /// `unk` when a selected call has no correlated result, or when nothing is in scope.
    #[serde(rename = "tool.failed")]
    ToolFailed {
        /// Which calls are in scope.
        selector: CallSelector,
        /// How many failures are acceptable.
        count: CountBound,
    },
    /// Failed calls over total calls, in the same scope, stays within a bound.
    ///
    /// Measures how well the model understood the tooling, which is a different question from
    /// whether the job got done.
    ///
    /// `unk` when no call is in scope at all — a rate over zero is not zero.
    #[serde(rename = "tool.error_rate")]
    ToolErrorRate {
        /// Which calls are in scope.
        selector: CallSelector,
        /// The acceptable rate, from 0 to 1.
        rate: RangeBound,
    },
    /// How many groups of byte-identical `(tool, input)` calls the run made.
    ///
    /// A confusion signal rather than a correctness one — two identical reads of one file is a
    /// model that lost track, three identical invocations is a retry loop — which is why it is a
    /// bound and not a prohibition.
    ///
    /// `unk` when an opaque event could have been a tool call.
    #[serde(rename = "tool.repeated")]
    ToolRepeated {
        /// Which calls are counted.
        selector: CallSelector,
        /// How many repeated groups are acceptable.
        count: CountBound,
    },
    /// The **first** occurrence of one call precedes the **first** occurrence of another.
    ///
    /// The same fact AEP already models: `evidence.first_seq.test_result <
    /// evidence.first_seq.diff` is how red-before-green is checked in the protocol, and it is
    /// first-occurrence ordering over a submission sequence. This is first-occurrence ordering
    /// over an event sequence, spelled the same way on purpose.
    ///
    /// `unk` when either side never occurs — "A before B" is undecidable when there is no A, and
    /// reporting it as a failure blames the wrong thing.
    #[serde(rename = "order")]
    Order {
        /// The call that must come first.
        first: CallSelector,
        /// The call it must come before.
        before: CallSelector,
    },
    /// The run's terminal record matches.
    ///
    /// `unk` when there is no terminal record — a transcript truncated by a crash has none, and
    /// that is exactly the case that must not read as a failed assertion.
    #[serde(rename = "result")]
    RunResult {
        /// Whether the run must be flagged as an error.
        is_error: Option<bool>,
        /// The record's subtype.
        subtype: Option<String>,
        /// Why the model must have stopped.
        stop_reason: Option<String>,
        /// Why the run must have ended.
        terminal_reason: Option<String>,
        /// What the API error status must say.
        api_error_status: Option<ApiErrorStatus>,
    },
    /// How many permission requests were denied stays within a bound.
    ///
    /// `unk` when the harness records no denial list.
    #[serde(rename = "permission.denied")]
    PermissionDenied {
        /// How many denials are acceptable.
        count: CountBound,
    },
    /// How many subagents were spawned stays within a bound.
    ///
    /// `unk` when the field is absent from this harness version.
    #[serde(rename = "subagent.spawned")]
    SubagentSpawned {
        /// How many are acceptable.
        count: CountBound,
    },
    /// The final assistant text matches.
    ///
    /// **The weakest kind on the list, and marked as such deliberately.** The eval asserts on
    /// files and events rather than on wording, because wording is allowed to vary and an
    /// assertion on it is a test of a sentence. It exists because *"the refusal was relayed to
    /// the operator"* has no other observable form today, and it should be avoided everywhere
    /// else.
    ///
    /// `unk` when there is no final assistant text.
    #[serde(rename = "text.matches")]
    TextMatches {
        /// How the final text is compared.
        matcher: FieldMatcher,
    },

    // --- the rate-limit family: a billing guard, not a performance one ---------------------
    /// The rate-limit status is in an allowed set.
    ///
    /// `unk` when the transcript records no rate-limit event.
    #[serde(rename = "rate_limit.status")]
    RateLimitStatus {
        /// The acceptable statuses.
        allowed: BTreeSet<String>,
    },
    /// Whether the run was paid for out of overage.
    ///
    /// A fact about money that no other part of the record carries, and one a CI job running an
    /// eval on every merge should be allowed to assert.
    ///
    /// `unk` when the transcript records no rate-limit event.
    #[serde(rename = "rate_limit.overage")]
    RateLimitOverage {
        /// What `isUsingOverage` must be — `false` in every sane specification.
        equals: bool,
    },
    /// How much of the rate-limit window was used stays within a bound.
    ///
    /// `unk` when the transcript records no rate-limit event.
    #[serde(rename = "rate_limit.utilization")]
    RateLimitUtilization {
        /// The acceptable utilization, from 0 to 1.
        utilization: RangeBound,
    },

    // --- counting a run: four quantities, four kinds ---------------------------------------
    /// The harness's own notion of a turn, from the terminal record.
    ///
    /// The only one of the four run quantities the harness itself names. A runaway loop is this
    /// one; a cost surprise is `api_requests` or `cost.total`.
    ///
    /// `unk` when there is no terminal record, or it records no turn count.
    #[serde(rename = "turns")]
    Turns {
        /// How many turns are acceptable.
        count: CountBound,
    },
    /// Distinct API requests across assistant events.
    ///
    /// The closest thing to "how many times did we call the model" — fewer than the event count,
    /// because one API response arrives as several events.
    ///
    /// Never `unk`: it is a count of what is in the transcript.
    #[serde(rename = "api_requests")]
    ApiRequests {
        /// How many requests are acceptable.
        count: CountBound,
    },
    /// Assistant events.
    ///
    /// An artefact of streaming: text and each tool call arrive as separate events sharing one
    /// request id. Bound it to catch a run that fragmented, not to bound cost — this is almost
    /// never what anyone means.
    ///
    /// Never `unk`.
    #[serde(rename = "events.assistant")]
    EventsAssistant {
        /// How many are acceptable.
        count: CountBound,
    },
    /// Per-iteration usage records in the terminal record.
    ///
    /// An **array's length**, not a counter — and nothing like the other three.
    ///
    /// `unk` when there is no terminal record, or it records no iterations.
    #[serde(rename = "iterations")]
    Iterations {
        /// How many are acceptable.
        count: CountBound,
    },

    // --- what it cost -----------------------------------------------------------------------
    /// Uncached input tokens, run-wide or for one model.
    ///
    /// `unk` when the field is absent, or when the named model was never used.
    #[serde(rename = "tokens.input")]
    TokensInput {
        /// How many are acceptable.
        count: CountBound,
        /// The model to scope to, where the specification names one.
        model: Option<String>,
    },
    /// Output tokens, run-wide or for one model.
    ///
    /// `unk` when the field is absent, or when the named model was never used.
    #[serde(rename = "tokens.output")]
    TokensOutput {
        /// How many are acceptable.
        count: CountBound,
        /// The model to scope to, where the specification names one.
        model: Option<String>,
    },
    /// Input plus output tokens, **excluding cache reads**, run-wide or for one model.
    ///
    /// The definition is in the document rather than in the reader's head, because a total whose
    /// terms are folklore is a number two people compute differently and then argue about.
    ///
    /// `unk` when either term is absent, or when the named model was never used.
    #[serde(rename = "tokens.total")]
    TokensTotal {
        /// How many are acceptable.
        count: CountBound,
        /// The model to scope to, where the specification names one.
        model: Option<String>,
    },
    /// The **billed** thinking tokens the API reported.
    ///
    /// Not the harness's live estimate — see [`ThinkingEstimated`](Self::ThinkingEstimated),
    /// which is a different source and a different number.
    ///
    /// `unk` when the field is absent.
    #[serde(rename = "tokens.thinking")]
    TokensThinking {
        /// How many are acceptable.
        count: CountBound,
    },
    /// The harness's last live thinking estimate.
    ///
    /// A different source and a different number from [`TokensThinking`](Self::TokensThinking):
    /// the estimate restarts per stretch and is emitted mid-stream, where the other is what the
    /// API reported. Conflating them in one kind would make a bound mean two things.
    ///
    /// `unk` when the transcript carries no thinking estimate.
    #[serde(rename = "thinking.estimated")]
    ThinkingEstimated {
        /// How many are acceptable.
        count: CountBound,
    },
    /// What the run cost, in US dollars, run-wide or for one model.
    ///
    /// Bounds only, never equality — the type has no `exactly`. A cost expectation exists to
    /// catch a run that looped for forty minutes, not to detect a 12% regression.
    ///
    /// `unk` when the field is absent, or when the named model was never used.
    #[serde(rename = "cost.total")]
    CostTotal {
        /// The acceptable cost.
        usd: RangeBound,
        /// The model to scope to, where the specification names one.
        model: Option<String>,
    },
    /// Whether the run read anything from the cache at all.
    ///
    /// The simple form, and the one most specifications want.
    ///
    /// `unk` when the field is absent.
    #[serde(rename = "cache.used")]
    CacheUsed {
        /// What it must be.
        equals: bool,
    },
    /// Tokens read from the cache, run-wide or for one model.
    ///
    /// `unk` when the field is absent, or when the named model was never used.
    #[serde(rename = "cache.read_tokens")]
    CacheReadTokens {
        /// How many are acceptable.
        count: CountBound,
        /// The model to scope to, where the specification names one.
        model: Option<String>,
    },
    /// Tokens written to the cache, run-wide or for one model.
    ///
    /// Worth a kind: a run that re-creates a large cache it should have read is a real
    /// regression, and it is invisible in cost until it is expensive.
    ///
    /// `unk` when the field is absent, or when the named model was never used.
    #[serde(rename = "cache.created_tokens")]
    CacheCreatedTokens {
        /// How many are acceptable.
        count: CountBound,
        /// The model to scope to, where the specification names one.
        model: Option<String>,
    },
    /// The cache hit ratio, with its denominator carried in the specification and not in the
    /// reader's head:
    ///
    /// ```text
    /// hit_ratio = cache_read_input_tokens / (cache_read_input_tokens + input_tokens)
    /// ```
    ///
    /// Cache *creation* tokens are excluded from the denominator: writing the cache is not a miss
    /// against it. Writing the formula down is the point.
    ///
    /// `unk` when either term is absent, or when the denominator is zero — a ratio over nothing
    /// is not a ratio.
    #[serde(rename = "cache.hit_ratio")]
    CacheHitRatio {
        /// The acceptable ratio, from 0 to 1.
        ratio: RangeBound,
    },

    // --- where the wall clock went ------------------------------------------------------
    /// The run's recorded wall-clock duration.
    ///
    /// `unk` when there is no terminal record, or it records no duration.
    #[serde(rename = "duration.total")]
    DurationTotal {
        /// The acceptable duration, in milliseconds.
        ms: CountBound,
    },
    /// The duration the harness attributed to API calls.
    ///
    /// Observed *exceeding* the run's own duration in a real transcript, which is why it is its
    /// own kind and not derived from the other.
    ///
    /// `unk` when there is no terminal record, or it records no API duration.
    #[serde(rename = "duration.api")]
    DurationApi {
        /// The acceptable duration, in milliseconds.
        ms: CountBound,
    },
    /// The recorded time to first token.
    ///
    /// Read, never derived. Where the harness does not record it the verdict is `unk` with the
    /// reason *"this transcript records no time to first token"* — it is never obtained by a
    /// subtraction the harness did not authorise.
    #[serde(rename = "ttft")]
    Ttft {
        /// The acceptable time, in milliseconds.
        ms: CountBound,
    },
    /// Recorded startup overhead before the first API request.
    ///
    /// The one latency number that is about the harness rather than the model, which makes it the
    /// one worth bounding in CI: it catches a plugin that got slow to load.
    ///
    /// `unk` when there is no terminal record, or it records no such time.
    #[serde(rename = "time_to_request")]
    TimeToRequest {
        /// The acceptable time, in milliseconds.
        ms: CountBound,
    },
    /// Every selected call's **generation** interval — the model thinking and emitting it.
    ///
    /// Derived from recorded timestamps, never measured. Scoped by tool or by argument matcher,
    /// and applied to each selected step: one step over the bound is a gap.
    ///
    /// `unk` when a selected step's neighbours carry no timestamp, or when nothing is in scope.
    #[serde(rename = "step.gen_time")]
    StepGenTime {
        /// Which calls are in scope.
        selector: CallSelector,
        /// The acceptable interval, in milliseconds.
        ms: CountBound,
    },
    /// Every selected call's **execution** interval — the tool doing the work.
    ///
    /// The one that is a real guard on this repository's own CLI: a verb that got slow shows up
    /// as a step, not as a percentage of a total that is dominated by inference. Every
    /// `protocol artifact` call in the observed run returned in ≤ 187 ms.
    ///
    /// `unk` when a selected step's result carries no timestamp, or when nothing is in scope.
    #[serde(rename = "step.exec_time")]
    StepExecTime {
        /// Which calls are in scope.
        selector: CallSelector,
        /// The acceptable interval, in milliseconds.
        ms: CountBound,
    },
    /// The sum of every step's generation interval.
    ///
    /// `unk` when any step's interval could not be derived — a total that silently omitted one
    /// would be a smaller number wearing the same name.
    #[serde(rename = "time.inference_total")]
    TimeInferenceTotal {
        /// The acceptable total, in milliseconds.
        ms: CountBound,
    },
    /// The sum of every step's execution interval.
    ///
    /// Observed at 1.5% of the two combined: the wall clock of an agent run is about 98.5% model.
    ///
    /// `unk` when any step's interval could not be derived.
    #[serde(rename = "time.tool_exec_total")]
    TimeToolExecTotal {
        /// The acceptable total, in milliseconds.
        ms: CountBound,
    },

    // --- environment-dependent, and documented as such --------------------------------------
    /// The speed tier the account was served at.
    ///
    /// **Environment-dependent**: a specification that pins this fails on somebody else's account
    /// rather than on the agent's behaviour. Write it [`Severity::Advisory`].
    ///
    /// `unk` when the field is absent.
    #[serde(rename = "speed")]
    Speed {
        /// The tier expected.
        equals: String,
    },
    /// The service tier the account was served at.
    ///
    /// **Environment-dependent**, for [`Speed`](Self::Speed)'s reason.
    ///
    /// `unk` when the field is absent.
    #[serde(rename = "service_tier")]
    ServiceTier {
        /// The tier expected.
        equals: String,
    },
}

impl ExpectationKind {
    /// The kind's name, as the document spells it and a report prints it.
    ///
    /// Derived from one `match` rather than stored beside the variant, so a new kind cannot be
    /// added without naming it.
    pub fn name(&self) -> &'static str {
        match self {
            Self::EnvPluginLoaded { .. } => "env.plugin_loaded",
            Self::EnvExclusive { .. } => "env.exclusive",
            Self::EnvOutputStyle { .. } => "env.output_style",
            Self::EnvSkillAvailable { .. } => "env.skill_available",
            Self::EnvAgentAvailable { .. } => "env.agent_available",
            Self::EnvModel { .. } => "env.model",
            Self::EnvPermissionMode { .. } => "env.permission_mode",
            Self::EnvApiKeySource { .. } => "env.api_key_source",
            Self::SkillInvoked { .. } => "skill.invoked",
            Self::SkillCompleted { .. } => "skill.completed",
            Self::ToolCalled { .. } => "tool.called",
            Self::ToolAbsent { .. } => "tool.absent",
            Self::ToolResultMatches { .. } => "tool.result",
            Self::ToolResultBytes { .. } => "tool.result_bytes",
            Self::ToolFailed { .. } => "tool.failed",
            Self::ToolErrorRate { .. } => "tool.error_rate",
            Self::ToolRepeated { .. } => "tool.repeated",
            Self::Order { .. } => "order",
            Self::RunResult { .. } => "result",
            Self::PermissionDenied { .. } => "permission.denied",
            Self::SubagentSpawned { .. } => "subagent.spawned",
            Self::TextMatches { .. } => "text.matches",
            Self::RateLimitStatus { .. } => "rate_limit.status",
            Self::RateLimitOverage { .. } => "rate_limit.overage",
            Self::RateLimitUtilization { .. } => "rate_limit.utilization",
            Self::Turns { .. } => "turns",
            Self::ApiRequests { .. } => "api_requests",
            Self::EventsAssistant { .. } => "events.assistant",
            Self::Iterations { .. } => "iterations",
            Self::TokensInput { .. } => "tokens.input",
            Self::TokensOutput { .. } => "tokens.output",
            Self::TokensTotal { .. } => "tokens.total",
            Self::TokensThinking { .. } => "tokens.thinking",
            Self::ThinkingEstimated { .. } => "thinking.estimated",
            Self::CostTotal { .. } => "cost.total",
            Self::CacheUsed { .. } => "cache.used",
            Self::CacheReadTokens { .. } => "cache.read_tokens",
            Self::CacheCreatedTokens { .. } => "cache.created_tokens",
            Self::CacheHitRatio { .. } => "cache.hit_ratio",
            Self::DurationTotal { .. } => "duration.total",
            Self::DurationApi { .. } => "duration.api",
            Self::Ttft { .. } => "ttft",
            Self::TimeToRequest { .. } => "time_to_request",
            Self::StepGenTime { .. } => "step.gen_time",
            Self::StepExecTime { .. } => "step.exec_time",
            Self::TimeInferenceTotal { .. } => "time.inference_total",
            Self::TimeToolExecTotal { .. } => "time.tool_exec_total",
            Self::Speed { .. } => "speed",
            Self::ServiceTier { .. } => "service_tier",
        }
    }

    /// Every kind name this build implements, sorted.
    ///
    /// Published so a refusal can list what *is* accepted, and so a test can assert that the
    /// document form and the validated form agree about the vocabulary rather than drifting into
    /// two lists.
    pub const NAMES: &'static [&'static str] = &[
        "api_requests",
        "cache.created_tokens",
        "cache.hit_ratio",
        "cache.read_tokens",
        "cache.used",
        "cost.total",
        "duration.api",
        "duration.total",
        "env.agent_available",
        "env.api_key_source",
        "env.exclusive",
        "env.model",
        "env.output_style",
        "env.permission_mode",
        "env.plugin_loaded",
        "env.skill_available",
        "events.assistant",
        "iterations",
        "order",
        "permission.denied",
        "rate_limit.overage",
        "rate_limit.status",
        "rate_limit.utilization",
        "result",
        "service_tier",
        "skill.completed",
        "skill.invoked",
        "speed",
        "step.exec_time",
        "step.gen_time",
        "subagent.spawned",
        "text.matches",
        "thinking.estimated",
        "time.inference_total",
        "time.tool_exec_total",
        "time_to_request",
        "tokens.input",
        "tokens.output",
        "tokens.thinking",
        "tokens.total",
        "tool.absent",
        "tool.called",
        "tool.error_rate",
        "tool.failed",
        "tool.repeated",
        "tool.result",
        "tool.result_bytes",
        "ttft",
        "turns",
    ];
}

/// One expectation: an id, what it claims, and what its verdict is allowed to do.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Expectation {
    /// The id a verdict is reported under.
    pub id: String,
    /// The author's own sentence, carried into the report unchanged.
    pub statement: Option<String>,
    /// Whether a gap moves the exit code.
    pub severity: Severity,
    /// What an undecidable verdict means here.
    pub on_unknown: OnUnknown,
    /// What it claims.
    pub kind: ExpectationKind,
}

/// A specification: what a run must have looked like.
///
/// Validated. There is no `Deserialize` here (invariant 2) — [`crate::raw::RawTraceSpec`] is the
/// type that deserializes, and `TryFrom` is the only door.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TraceSpec {
    /// What the specification is about, such as `planning-plugin/eval`.
    pub id: String,
    /// A human sentence for a report's heading.
    pub title: Option<String>,
    /// The expectations it declares.
    pub expectations: Vec<Expectation>,
    /// The content digest of the document **as authored**.
    ///
    /// Computed once, at validation, over the format, the id, the title and the expectations —
    /// and deliberately *not* recomputed afterwards. A command-line override that downgrades an
    /// expectation to advisory changes what the run gated on, and the report says so in its own
    /// field; it does not get to change the name of the specification it checked.
    pub digest: String,
}

/// The view the specification digest is computed over.
///
/// A separate type rather than serializing [`TraceSpec`] itself, because the digest is a field of
/// that type and a value cannot digest itself.
#[derive(Serialize)]
struct DigestView<'a> {
    format: &'static str,
    id: &'a str,
    title: Option<&'a str>,
    expectations: &'a [Expectation],
}

impl TraceSpec {
    /// Builds a validated specification and computes its digest.
    ///
    /// Crate-private: `TryFrom<RawTraceSpec>` is the only door, which is invariant 2. A consumer
    /// that could construct one directly could construct one that never passed a rule.
    pub(crate) fn new(id: String, title: Option<String>, expectations: Vec<Expectation>) -> Self {
        let digest = digest_of_canonical(&DigestView {
            format: SPEC_FORMAT,
            id: &id,
            title: title.as_deref(),
            expectations: &expectations,
        });
        Self {
            id,
            title,
            expectations,
            digest,
        }
    }

    /// Downgrades named expectations to [`Severity::Advisory`], returning the ids it did not find.
    ///
    /// The escape hatch a caller needs when one expectation is about the *environment the run was
    /// given* rather than about the agent — the eval's `env.api_key_source` under
    /// `EVAL_USE_API_KEY=1` is the motivating case. It is deliberately **not** a way to skip a
    /// check: the expectation is still evaluated, still printed, and still in the report, and the
    /// report names every id that was downgraded.
    ///
    /// Unknown ids are returned rather than ignored, so a caller can refuse a typo instead of
    /// silently downgrading nothing — which would be a check that stopped checking.
    pub fn mark_advisory(&mut self, ids: &BTreeSet<String>) -> Vec<String> {
        let declared: BTreeSet<&str> = self
            .expectations
            .iter()
            .map(|expectation| expectation.id.as_str())
            .collect();
        let unknown: Vec<String> = ids
            .iter()
            .filter(|id| !declared.contains(id.as_str()))
            .cloned()
            .collect();
        for expectation in &mut self.expectations {
            if ids.contains(&expectation.id) {
                expectation.severity = Severity::Advisory;
            }
        }
        unknown
    }

    /// How many expectations of each kind the document declares.
    ///
    /// For a reader of a report who wants to know what was checked without reading the document.
    pub fn kind_census(&self) -> BTreeMap<&'static str, usize> {
        let mut census = BTreeMap::new();
        for expectation in &self.expectations {
            *census.entry(expectation.kind.name()).or_default() += 1;
        }
        census
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn expectation(id: &str, kind: ExpectationKind) -> Expectation {
        Expectation {
            id: id.to_owned(),
            statement: None,
            severity: Severity::Gate,
            on_unknown: OnUnknown::Unknown,
            kind,
        }
    }

    fn spec() -> TraceSpec {
        TraceSpec::new(
            "planning-plugin/eval".to_owned(),
            None,
            vec![
                expectation(
                    "billed-to-the-session",
                    ExpectationKind::EnvApiKeySource {
                        equals: "none".to_owned(),
                    },
                ),
                expectation(
                    "within-budget",
                    ExpectationKind::CostTotal {
                        usd: RangeBound::at_most(1.0),
                        model: None,
                    },
                ),
            ],
        )
    }

    #[test]
    fn every_kind_names_itself_and_the_published_list_holds_them_all() {
        assert_eq!(
            ExpectationKind::NAMES.len(),
            49,
            "the vocabulary is forty-nine kinds; a new one must be published here to be writable"
        );
        let mut sorted = ExpectationKind::NAMES.to_vec();
        sorted.sort_unstable();
        assert_eq!(
            sorted,
            ExpectationKind::NAMES,
            "the list is sorted, so a refusal that prints it reads alphabetically"
        );
        let mut seen = BTreeSet::new();
        for name in ExpectationKind::NAMES {
            assert!(seen.insert(*name), "{name} is published twice");
        }
    }

    #[test]
    fn the_name_a_kind_reports_is_the_name_the_document_writes() {
        let kind = ExpectationKind::ToolCalled {
            selector: CallSelector::tool("Bash"),
            count: CountBound::at_least(1),
        };
        assert_eq!(kind.name(), "tool.called");
        let wire = serde_json::to_value(&kind).expect("a kind serializes");
        assert_eq!(
            wire["expect"].as_str(),
            Some("tool.called"),
            "the serialized tag and `name()` must be the same word, or a JSON report and a text \
             report would name one expectation two ways"
        );
    }

    #[test]
    fn the_digest_is_over_the_document_and_survives_a_command_line_downgrade() {
        let mut first = spec();
        let authored = first.digest.clone();
        let unknown = first.mark_advisory(&BTreeSet::from(["within-budget".to_owned()]));
        assert!(unknown.is_empty());
        assert_eq!(
            first.expectations[1].severity,
            Severity::Advisory,
            "the named expectation is downgraded"
        );
        assert_eq!(
            first.expectations[0].severity,
            Severity::Gate,
            "and nothing else is"
        );
        assert_eq!(
            first.digest, authored,
            "the digest names the document as authored; the override is reported separately"
        );
    }

    #[test]
    fn downgrading_an_id_the_document_does_not_declare_is_reported_rather_than_ignored() {
        // A silently-ignored typo is a check the caller believes it relaxed and did not, or —
        // worse — believes it kept and did not.
        let mut spec = spec();
        let unknown = spec.mark_advisory(&BTreeSet::from(["within-budgets".to_owned()]));
        assert_eq!(unknown, vec!["within-budgets"]);
        assert!(spec
            .expectations
            .iter()
            .all(|expectation| expectation.severity == Severity::Gate));
    }

    #[test]
    fn two_documents_that_differ_only_in_a_title_are_not_the_same_specification() {
        let plain = spec();
        let titled = TraceSpec::new(
            plain.id.clone(),
            Some("the planning plugin behaves as its skill says".to_owned()),
            plain.expectations.clone(),
        );
        assert_ne!(plain.digest, titled.digest);
        assert_eq!(plain.digest.len(), 64);
    }

    #[test]
    fn a_kind_census_counts_what_was_checked() {
        assert_eq!(
            spec().kind_census(),
            BTreeMap::from([("env.api_key_source", 1), ("cost.total", 1)])
        );
    }
}
