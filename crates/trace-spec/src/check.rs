//! Evaluating a specification against a run: three values, and a reason for the third.
//!
//! # Accumulating, and citing
//!
//! Every expectation is evaluated and every verdict is reported; the checker does not stop at the
//! first gap. Every verdict cites what produced it — a gap names the events that should not have
//! been there or the field that disagreed, an `unk` names the event it could not read or the
//! field the transcript does not carry — and [`Outcome`] has no shape for a verdict with nothing
//! to cite.
//!
//! # An unread event can only make a count bigger
//!
//! The design's rule is that an opaque event makes a tool expectation `unk`, because the checker
//! cannot know whether it was a tool call. Applied bluntly that would be harsher than the truth:
//! an unread event can *add* calls and never remove one, so a bound of `at_least: 1` that has
//! already seen two calls holds whatever the unread event was.
//!
//! The checker does the three-valued reasoning properly (`decide_count`, private). The observed count is a **lower
//! bound** on the real one when something could not be read, and the decision is:
//!
//! | the bound | holds for every value ≥ observed | holds for none of them | otherwise |
//! |---|---|---|---|
//! | verdict | `ok` | `gap` | `unk` |
//!
//! So `at_most: 2` with three calls observed is a gap even under uncertainty — reading the
//! unread event could only make it worse — and `exactly: 0` with one call observed is a gap for
//! the same reason. That is the difference between a checker that is careful and one that is
//! merely timid.
//!
//! # Kleene, not "the worst of the three"
//!
//! Where an expectation folds several subjects — every matched call's result, every selected
//! step's duration — a gap beside an unknown is still a **gap**, because something *was* observed
//! to be wrong. That is the same fold `Truth::and` performs in `aep-domain` and the same rule
//! `infra-spec` states, and it is deliberately not a severity ordering.

use std::collections::BTreeMap;

use trace_domain::ir::{ModelUsage, RunOutcome, RunUsage, SessionStart, Step, ToolCall, TraceIr};
use trace_domain::matcher::{CallSelector, CountBound, FieldMatcher, RangeBound, ResultMatcher};
use trace_domain::spec::{Aggregate, ApiErrorStatus, ExpectationKind, ToolAvailability, TraceSpec};

use crate::report::{
    CheckReport, Citation, ExpectationReport, Outcome, UnknownReason, REPORT_FORMAT,
};

/// Checks a run against a specification.
///
/// `advisory_overrides` is the list of expectation ids a caller downgraded to
/// [`Severity::Advisory`](trace_domain::spec::Severity::Advisory) on the command line, recorded in
/// the report so a reader can see that the run gated on something narrower than the document
/// says. Applying the downgrade is
/// [`TraceSpec::mark_advisory`](trace_domain::spec::TraceSpec::mark_advisory)'s job; this only
/// carries the record of it.
///
/// Deterministic: same specification and same transcript in, byte-identical report out. No clock
/// is read.
pub fn check(spec: &TraceSpec, ir: &TraceIr, advisory_overrides: &[String]) -> CheckReport {
    debug_assert_eq!(REPORT_FORMAT, "trace-report/1");
    let expectations = spec
        .expectations
        .iter()
        .map(|expectation| {
            ExpectationReport::new(
                expectation.id.clone(),
                expectation.statement.clone(),
                expectation.kind.name(),
                expectation.severity,
                expectation.on_unknown,
                evaluate(&expectation.kind, ir),
            )
        })
        .collect();
    CheckReport::new(
        spec.id.clone(),
        spec.title.clone(),
        spec.digest.clone(),
        ir.transcript_digest.clone(),
        ir.adapter.clone(),
        advisory_overrides.to_vec(),
        expectations,
    )
}

/// One expectation against one run.
///
/// The exhaustive dispatch. [`ExpectationKind`] is deliberately not `#[non_exhaustive]`, so a kind
/// added to the vocabulary and not evaluated here fails to compile.
#[allow(clippy::too_many_lines)] // One arm per kind; splitting the dispatch would hide the map.
fn evaluate(kind: &ExpectationKind, ir: &TraceIr) -> Outcome {
    match kind {
        ExpectationKind::EnvPluginLoaded {
            plugin,
            version,
            source,
        } => env_plugin_loaded(ir, plugin, version.as_deref(), source.as_deref()),
        ExpectationKind::EnvExclusive { plugins } => env_exclusive(ir, plugins),
        ExpectationKind::EnvOutputStyle { equals } => {
            env_field(ir, "output_style", equals, |start| {
                start.output_style.as_deref()
            })
        }
        ExpectationKind::EnvSkillAvailable { skill } => {
            env_offers(ir, "skills", "skill", skill, |start| {
                start.skills.as_deref()
            })
        }
        ExpectationKind::EnvAgentAvailable { agent } => {
            env_offers(ir, "agents", "agent", agent, |start| {
                start.agents.as_deref()
            })
        }
        ExpectationKind::EnvToolAvailable { availability } => env_tool_available(ir, availability),
        ExpectationKind::EnvMcpServers { count } => env_mcp_servers(ir, *count),
        ExpectationKind::EnvModel { equals } => {
            env_field(ir, "model", equals, |start| start.model.as_deref())
        }
        ExpectationKind::EnvPermissionMode { equals } => {
            env_field(ir, "permission_mode", equals, |start| {
                start.permission_mode.as_deref()
            })
        }
        ExpectationKind::EnvApiKeySource { equals } => {
            env_field(ir, "api_key_source", equals, |start| {
                start.api_key_source.as_deref()
            })
        }
        ExpectationKind::SkillInvoked { skill, count } => skill_invoked(ir, skill, *count),
        ExpectationKind::SkillCompleted { skill, count } => skill_completed(ir, skill, *count),
        ExpectationKind::ToolCalled { selector, count } => tool_called(ir, selector, *count),
        ExpectationKind::ToolAbsent { selector } => tool_absent(ir, selector),
        ExpectationKind::ToolResultMatches { selector, result } => {
            tool_result_matches(ir, selector, result)
        }
        ExpectationKind::ToolResultBytes {
            selector,
            bytes,
            per,
        } => tool_result_bytes(ir, selector, *bytes, *per),
        ExpectationKind::ToolFailed { selector, count } => tool_failed(ir, selector, *count),
        ExpectationKind::ToolErrorRate { selector, rate } => tool_error_rate(ir, selector, *rate),
        ExpectationKind::ToolRepeated { selector, count } => tool_repeated(ir, selector, *count),
        ExpectationKind::Order { first, before } => order(ir, first, before),
        ExpectationKind::RunResult {
            is_error,
            subtype,
            stop_reason,
            terminal_reason,
            api_error_status,
        } => run_result(
            ir,
            *is_error,
            subtype.as_deref(),
            stop_reason.as_deref(),
            terminal_reason.as_deref(),
            api_error_status.as_ref(),
        ),
        ExpectationKind::PermissionDenied { count } => {
            outcome_count(ir, "permission_denials", *count, |run| {
                run.permission_denials
            })
        }
        ExpectationKind::SubagentSpawned { count } => {
            outcome_count(ir, "subagents_spawned", *count, |run| run.subagents_spawned)
        }
        ExpectationKind::TextMatches { matcher } => text_matches(ir, matcher),
        ExpectationKind::RateLimitStatus { allowed } => rate_limit_status(ir, allowed),
        ExpectationKind::RateLimitOverage { equals } => rate_limit_overage(ir, *equals),
        ExpectationKind::RateLimitUtilization { utilization } => {
            rate_limit_utilization(ir, *utilization)
        }
        ExpectationKind::Turns { count } => {
            outcome_count(ir, "num_turns", *count, |run| run.num_turns)
        }
        ExpectationKind::ApiRequests { count } => whole_run_count(
            "api requests",
            *count,
            u64::try_from(ir.api_request_count()).unwrap_or(u64::MAX),
        ),
        ExpectationKind::EventsAssistant { count } => whole_run_count(
            "assistant events",
            *count,
            u64::try_from(ir.assistant_event_count()).unwrap_or(u64::MAX),
        ),
        ExpectationKind::Iterations { count } => {
            usage_count(ir, "usage.iterations", *count, |usage| {
                usage.iterations.and_then(|it| u64::try_from(it).ok())
            })
        }
        ExpectationKind::TokensInput { count, model } => {
            tokens(ir, "input_tokens", *count, model.as_deref(), Token::Input)
        }
        ExpectationKind::TokensOutput { count, model } => {
            tokens(ir, "output_tokens", *count, model.as_deref(), Token::Output)
        }
        ExpectationKind::TokensTotal { count, model } => tokens(
            ir,
            "input_tokens + output_tokens",
            *count,
            model.as_deref(),
            Token::Total,
        ),
        ExpectationKind::TokensThinking { count } => {
            usage_count(ir, "usage.thinking_tokens", *count, |usage| {
                usage.thinking_tokens
            })
        }
        ExpectationKind::ThinkingEstimated { count } => thinking_estimated(ir, *count),
        ExpectationKind::CostTotal { usd, model } => cost_total(ir, *usd, model.as_deref()),
        ExpectationKind::CacheUsed { equals } => cache_used(ir, *equals),
        ExpectationKind::CacheReadTokens { count, model } => tokens(
            ir,
            "cache_read_input_tokens",
            *count,
            model.as_deref(),
            Token::CacheRead,
        ),
        ExpectationKind::CacheCreatedTokens { count, model } => tokens(
            ir,
            "cache_creation_input_tokens",
            *count,
            model.as_deref(),
            Token::CacheCreated,
        ),
        ExpectationKind::CacheHitRatio { ratio } => cache_hit_ratio(ir, *ratio),
        ExpectationKind::DurationTotal { ms } => {
            outcome_count(ir, "duration_ms", *ms, |run| run.duration_ms)
        }
        ExpectationKind::DurationApi { ms } => {
            outcome_count(ir, "duration_api_ms", *ms, |run| run.duration_api_ms)
        }
        ExpectationKind::Ttft { ms } => outcome_count(ir, "ttft_ms", *ms, |run| run.ttft_ms),
        ExpectationKind::TimeToRequest { ms } => {
            outcome_count(ir, "time_to_request_ms", *ms, |run| run.time_to_request_ms)
        }
        ExpectationKind::StepGenTime { selector, ms } => step_time(ir, selector, *ms, Phase::Gen),
        ExpectationKind::StepExecTime { selector, ms } => step_time(ir, selector, *ms, Phase::Exec),
        ExpectationKind::TimeInferenceTotal { ms } => step_total(ir, *ms, Phase::Gen),
        ExpectationKind::TimeToolExecTotal { ms } => step_total(ir, *ms, Phase::Exec),
        ExpectationKind::Speed { equals } => {
            usage_text(ir, "usage.speed", equals, |usage| usage.speed.as_deref())
        }
        ExpectationKind::ServiceTier { equals } => {
            usage_text(ir, "usage.service_tier", equals, |usage| {
                usage.service_tier.as_deref()
            })
        }
    }
}

/// Which of a step's two derived intervals a kind reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    /// The model thinking and emitting the call.
    Gen,
    /// The tool doing the work.
    Exec,
}

impl Phase {
    /// The word a report prints.
    fn as_str(self) -> &'static str {
        match self {
            Self::Gen => "gen",
            Self::Exec => "exec",
        }
    }
}

/// Which token quantity a kind reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Token {
    /// Uncached input tokens.
    Input,
    /// Output tokens.
    Output,
    /// Input plus output, excluding cache reads.
    Total,
    /// Tokens read from the cache.
    CacheRead,
    /// Tokens written to the cache.
    CacheCreated,
}

/// What a bound says about an observed value that might be a lower bound on the real one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Decision {
    /// It holds however many unread events there were.
    Holds,
    /// It fails however many unread events there were.
    Fails,
    /// An unread event could change the answer.
    Undecided,
}

/// Three-valued reasoning over a count that may be a lower bound.
///
/// See the module documentation for the table. `may_grow` is `true` when something the checker
/// could not read might have contributed to the count.
fn decide_count(bound: CountBound, observed: u64, may_grow: bool) -> Decision {
    if !may_grow {
        return if bound.holds(observed) {
            Decision::Holds
        } else {
            Decision::Fails
        };
    }
    let holds_now = bound.holds(observed);
    // A bound with no ceiling and no `exactly` that holds now holds for every larger value too.
    if holds_now && bound.at_most.is_none() && bound.exactly.is_none() {
        return Decision::Holds;
    }
    // A ceiling already exceeded cannot be met by a larger value.
    let exceeded = match (bound.exactly, bound.at_most) {
        (Some(exactly), _) => observed > exactly,
        (None, Some(ceiling)) => observed > ceiling,
        (None, None) => false,
    };
    if exceeded {
        Decision::Fails
    } else {
        Decision::Undecided
    }
}

/// The events the adapter could not read, as an `unk` reason.
fn opaque_reason(ir: &TraceIr) -> UnknownReason {
    let opaque = ir.opaque_events();
    UnknownReason::OpaqueEvents {
        events: opaque.iter().map(|(index, _)| *index).collect(),
        types: opaque
            .iter()
            .filter_map(|(_, event)| event.event_type.clone())
            .collect(),
    }
}

/// `true` when the adapter met an event it could not read.
///
/// Any such event poisons a count, because the checker cannot know what it was — a `tool_use`
/// under a renamed wrapper counts, and reading it as absent would be the lie this whole design
/// exists to prevent. [`decide_count`] narrows that to the cases where it can actually change the
/// answer.
fn has_unread(ir: &TraceIr) -> bool {
    !ir.opaque_events().is_empty()
}

/// The run's opening record, or the reason there is none.
fn session(ir: &TraceIr) -> Result<(usize, &SessionStart), Outcome> {
    match (ir.session_start_event(), ir.session_start()) {
        (Some(at), Some(start)) => Ok((at, start)),
        _ => Err(Outcome::Undecidable(UnknownReason::NoSessionStart)),
    }
}

/// The run's terminal record, or the reason there is none.
fn terminal(ir: &TraceIr) -> Result<(usize, &RunOutcome), Outcome> {
    match (ir.run_outcome_event(), ir.run_outcome()) {
        (Some(at), Some(run)) => Ok((at, run)),
        _ => Err(Outcome::Undecidable(UnknownReason::NoRunOutcome)),
    }
}

/// The run's aggregate usage, or the reason there is none.
fn usage(ir: &TraceIr) -> Result<(usize, &RunUsage), Outcome> {
    let (at, run) = terminal(ir)?;
    match &run.usage {
        Some(usage) => Ok((at, usage)),
        None => Err(unknown_field("usage")),
    }
}

/// One model's usage, or the reason it cannot be read.
fn model_usage<'a>(run: &'a RunOutcome, model: &str) -> Result<&'a ModelUsage, Outcome> {
    let Some(per_model) = &run.model_usage else {
        return Err(unknown_field("model_usage"));
    };
    per_model.get(model).ok_or_else(|| {
        Outcome::Undecidable(UnknownReason::ModelNotUsed {
            model: model.to_owned(),
        })
    })
}

/// An `unk` because a field is not there.
fn unknown_field(field: &str) -> Outcome {
    Outcome::Undecidable(UnknownReason::FieldAbsent {
        field: field.to_owned(),
    })
}

/// A pass citing one event.
fn holds(at: usize, note: impl Into<String>) -> Outcome {
    Outcome::Ok(Citation::new(vec![at], note))
}

/// A contradiction citing one event.
fn contradicted(at: usize, note: impl Into<String>) -> Outcome {
    Outcome::Gap(Citation::new(vec![at], note))
}

/// Turns a [`Decision`] into an outcome with the citation the caller assembled.
fn from_decision(
    decision: Decision,
    events: Vec<usize>,
    note: String,
    undecided: UnknownReason,
) -> Outcome {
    match decision {
        Decision::Holds => Outcome::Ok(Citation::new(events, note)),
        Decision::Fails => Outcome::Gap(Citation::new(events, note)),
        Decision::Undecided => Outcome::Undecidable(undecided),
    }
}

// --- the environment -----------------------------------------------------------------------

/// One text field of the opening record, compared for equality.
fn env_field(
    ir: &TraceIr,
    field: &str,
    expected: &str,
    read: impl Fn(&SessionStart) -> Option<&str>,
) -> Outcome {
    let (at, start) = match session(ir) {
        Ok(found) => found,
        Err(outcome) => return outcome,
    };
    match read(start) {
        None => unknown_field(field),
        Some(actual) if actual == expected => holds(at, format!("{field} = {actual}")),
        Some(actual) => contradicted(at, format!("{field} = {actual}, expected {expected}")),
    }
}

/// A name that must appear in one of the opening record's offered lists.
fn env_offers(
    ir: &TraceIr,
    field: &str,
    noun: &str,
    wanted: &str,
    read: impl Fn(&SessionStart) -> Option<&[String]>,
) -> Outcome {
    let (at, start) = match session(ir) {
        Ok(found) => found,
        Err(outcome) => return outcome,
    };
    match read(start) {
        None => unknown_field(field),
        Some(offered) if offered.iter().any(|name| name == wanted) => holds(
            at,
            format!("{noun} {wanted} is among {} offered", offered.len()),
        ),
        Some(offered) => contradicted(
            at,
            format!("{noun} {wanted} is not among the {} offered", offered.len()),
        ),
    }
}

/// A named plugin is loaded, optionally at a pinned version and from a pinned source.
fn env_plugin_loaded(
    ir: &TraceIr,
    plugin: &str,
    version: Option<&str>,
    source: Option<&str>,
) -> Outcome {
    let (at, start) = match session(ir) {
        Ok(found) => found,
        Err(outcome) => return outcome,
    };
    let Some(plugins) = &start.plugins else {
        return unknown_field("plugins");
    };
    let Some(loaded) = plugins.iter().find(|entry| entry.name == plugin) else {
        return contradicted(at, format!("{plugin} is not loaded"));
    };
    // A pinned version or source the harness does not record is `unk`, not a mismatch: the
    // difference between "it is at another version" and "nobody wrote the version down" is
    // exactly what the third value is for.
    for (pinned, actual, field) in [
        (version, loaded.version.as_deref(), "plugins[].version"),
        (source, loaded.source.as_deref(), "plugins[].source"),
    ] {
        let Some(pinned) = pinned else { continue };
        match actual {
            None => return unknown_field(field),
            Some(actual) if actual == pinned => {}
            Some(actual) => {
                return contradicted(
                    at,
                    format!("{plugin} {field} is {actual}, expected {pinned}"),
                )
            }
        }
    }
    holds(
        at,
        format!(
            "{plugin}{}{} is loaded",
            loaded
                .version
                .as_deref()
                .map(|version| format!(" {version}"))
                .unwrap_or_default(),
            loaded
                .source
                .as_deref()
                .map(|source| format!(" from {source}"))
                .unwrap_or_default()
        ),
    )
}

/// The loaded plugins are exactly the named set.
fn env_exclusive(ir: &TraceIr, expected: &std::collections::BTreeSet<String>) -> Outcome {
    let (at, start) = match session(ir) {
        Ok(found) => found,
        Err(outcome) => return outcome,
    };
    let Some(plugins) = &start.plugins else {
        return unknown_field("plugins");
    };
    let loaded: std::collections::BTreeSet<&str> =
        plugins.iter().map(|entry| entry.name.as_str()).collect();
    let unexpected: Vec<&str> = loaded
        .iter()
        .copied()
        .filter(|name| !expected.contains(*name))
        .collect();
    let missing: Vec<&str> = expected
        .iter()
        .map(String::as_str)
        .filter(|name| !loaded.contains(name))
        .collect();
    if unexpected.is_empty() && missing.is_empty() {
        return holds(
            at,
            format!(
                "exactly {} loaded",
                loaded.into_iter().collect::<Vec<_>>().join(", ")
            ),
        );
    }
    let mut parts = Vec::new();
    if !unexpected.is_empty() {
        parts.push(format!(
            "{} unexpected plugin(s): {}",
            unexpected.len(),
            unexpected.join(", ")
        ));
    }
    if !missing.is_empty() {
        parts.push(format!("{} not loaded", missing.join(", ")));
    }
    contradicted(at, parts.join("; "))
}

/// The offered tool list, in the three claims [`ToolAvailability`] can make about it.
///
/// Every one reads `SessionStart.tools` and nothing else. What the model *called* is a different
/// question with its own kinds, and conflating them is the mistake this kind exists to make
/// impossible: an allowlist that offers a tool nobody reaches for is invisible to `tool.absent`.
fn env_tool_available(ir: &TraceIr, availability: &ToolAvailability) -> Outcome {
    match availability {
        ToolAvailability::Offered { tool } => {
            env_offers(ir, "tools", "tool", tool, |start| start.tools.as_deref())
        }
        ToolAvailability::NotOffered { tool } => {
            env_withholds(ir, "tools", "tool", tool, |start| start.tools.as_deref())
        }
        ToolAvailability::Only { tools } => env_tools_only(ir, tools),
    }
}

/// A name that must **not** appear in one of the opening record's offered lists.
///
/// The negation of [`env_offers`], and its own function rather than a flag on that one, because
/// the sentence it writes is a different sentence: here *not offered* is the claim that held, and
/// there it was the contradiction.
fn env_withholds(
    ir: &TraceIr,
    field: &str,
    noun: &str,
    unwanted: &str,
    read: impl Fn(&SessionStart) -> Option<&[String]>,
) -> Outcome {
    let (at, start) = match session(ir) {
        Ok(found) => found,
        Err(outcome) => return outcome,
    };
    match read(start) {
        None => unknown_field(field),
        Some(offered) if offered.iter().any(|name| name == unwanted) => contradicted(
            at,
            format!("{noun} {unwanted} is among the {} offered", offered.len()),
        ),
        Some(offered) => holds(
            at,
            format!(
                "{noun} {unwanted} is not among the {} offered",
                offered.len()
            ),
        ),
    }
}

/// The offered tools are exactly the named set.
///
/// [`env_exclusive`] pointed at tools rather than plugins, and reported the same way: what leaked
/// in and what was expected and missing, because an author fixing one wants both halves at once.
fn env_tools_only(ir: &TraceIr, expected: &std::collections::BTreeSet<String>) -> Outcome {
    let (at, start) = match session(ir) {
        Ok(found) => found,
        Err(outcome) => return outcome,
    };
    let Some(tools) = &start.tools else {
        return unknown_field("tools");
    };
    let offered: std::collections::BTreeSet<&str> = tools.iter().map(String::as_str).collect();
    let unexpected: Vec<&str> = offered
        .iter()
        .copied()
        .filter(|name| !expected.contains(*name))
        .collect();
    let missing: Vec<&str> = expected
        .iter()
        .map(String::as_str)
        .filter(|name| !offered.contains(name))
        .collect();
    if unexpected.is_empty() && missing.is_empty() {
        return holds(
            at,
            format!("exactly the {} tools expected were offered", offered.len()),
        );
    }
    let mut parts = Vec::new();
    if !unexpected.is_empty() {
        parts.push(format!(
            "{} unexpected tool(s): {}",
            unexpected.len(),
            unexpected.join(", ")
        ));
    }
    if !missing.is_empty() {
        parts.push(format!("{} not offered", missing.join(", ")));
    }
    contradicted(at, parts.join("; "))
}

/// How many MCP servers the opening record listed, against a bound.
///
/// The count is exact and never a lower bound, so [`decide_count`] is not reached for: the
/// opening record either lists the servers or does not, and an opaque event later in the run
/// cannot add one to a list that was written before the first turn.
///
/// The gap names both numbers. A reader of *"3 MCP server(s), expected at most 0"* knows what to
/// go and look at without opening the transcript, and the names are deliberately **not** in the
/// note: they are account-level, they end up in a committed report, and the count is the whole
/// claim.
fn env_mcp_servers(ir: &TraceIr, bound: CountBound) -> Outcome {
    let (at, start) = match session(ir) {
        Ok(found) => found,
        Err(outcome) => return outcome,
    };
    let Some(servers) = &start.mcp_servers else {
        return unknown_field("mcp_servers");
    };
    let observed = servers.len() as u64;
    if bound.holds(observed) {
        holds(at, format!("{observed} MCP server(s), bound {bound}"))
    } else {
        contradicted(at, format!("{observed} MCP server(s), expected {bound}"))
    }
}

// --- the skill -----------------------------------------------------------------------------

/// Tool calls that invoke a named skill.
fn skill_calls<'a>(ir: &'a TraceIr, skill: &str) -> Vec<(usize, &'a ToolCall)> {
    ir.tool_calls()
        .into_iter()
        .filter(|(_, call)| {
            call.name == "Skill"
                && call
                    .argument("skill")
                    .and_then(|value| value.as_str())
                    .is_some_and(|named| named == skill)
        })
        .collect()
}

/// The model chose the skill.
fn skill_invoked(ir: &TraceIr, skill: &str, count: CountBound) -> Outcome {
    let calls = skill_calls(ir, skill);
    let events: Vec<usize> = calls.iter().map(|(at, _)| *at).collect();
    let observed = events.len() as u64;
    from_decision(
        decide_count(count, observed, has_unread(ir)),
        events,
        format!("{skill} invoked {observed} time(s), {count}"),
        opaque_reason(ir),
    )
}

/// The skill ran to completion — structurally, from the result the harness set.
fn skill_completed(ir: &TraceIr, skill: &str, count: CountBound) -> Outcome {
    let mut events = Vec::new();
    let mut completed = 0u64;
    let mut undecided: Option<UnknownReason> = None;
    for (at, call) in skill_calls(ir, skill) {
        let Some((result_at, result)) = ir.result_of(call) else {
            undecided.get_or_insert(UnknownReason::NoResultCorrelated { call_event: at });
            continue;
        };
        let named = result.field("commandName").and_then(|value| value.as_str());
        let succeeded = result.field("success").and_then(serde_json::Value::as_bool);
        match (named, succeeded) {
            (Some(named), Some(true)) if named == skill => {
                completed += 1;
                events.push(at);
                events.push(result_at);
            }
            (None, _) => {
                undecided.get_or_insert(UnknownReason::ResultFieldAbsent {
                    call_event: at,
                    result_event: result_at,
                    field: "commandName".to_owned(),
                });
            }
            (_, None) => {
                undecided.get_or_insert(UnknownReason::ResultFieldAbsent {
                    call_event: at,
                    result_event: result_at,
                    field: "success".to_owned(),
                });
            }
            _ => {}
        }
    }
    let may_grow = has_unread(ir) || undecided.is_some();
    from_decision(
        decide_count(count, completed, may_grow),
        events,
        format!("{skill} completed {completed} time(s) with success=true, {count}"),
        undecided.unwrap_or_else(|| opaque_reason(ir)),
    )
}

// --- the tool family --------------------------------------------------------------------

/// The calls a selector picks, with their event indices.
fn selected<'a>(ir: &'a TraceIr, selector: &CallSelector) -> Vec<(usize, &'a ToolCall)> {
    ir.tool_calls()
        .into_iter()
        .filter(|(_, call)| selector.matches(call))
        .collect()
}

/// Matched calls fall within a bound.
fn tool_called(ir: &TraceIr, selector: &CallSelector, count: CountBound) -> Outcome {
    let calls = selected(ir, selector);
    let events: Vec<usize> = calls.iter().map(|(at, _)| *at).collect();
    let observed = events.len() as u64;
    from_decision(
        decide_count(count, observed, has_unread(ir)),
        events,
        format!("{selector} called {observed} time(s), {count}"),
        opaque_reason(ir),
    )
}

/// No call matches.
fn tool_absent(ir: &TraceIr, selector: &CallSelector) -> Outcome {
    let calls = selected(ir, selector);
    if calls.is_empty() {
        return if has_unread(ir) {
            Outcome::Undecidable(opaque_reason(ir))
        } else {
            Outcome::Ok(Citation::run(format!("no call matches {selector}")))
        };
    }
    Outcome::Gap(Citation::new(
        calls.iter().map(|(at, _)| *at).collect(),
        format!("{} call(s) match {selector}", calls.len()),
    ))
}

/// Every matched call's result satisfies the matcher.
fn tool_result_matches(ir: &TraceIr, selector: &CallSelector, matcher: &ResultMatcher) -> Outcome {
    let calls = selected(ir, selector);
    if calls.is_empty() {
        return Outcome::Undecidable(UnknownReason::NothingInScope {
            selector: selector.to_string(),
        });
    }
    let mut satisfied = Vec::new();
    let mut failed = Vec::new();
    let mut undecided: Option<UnknownReason> = None;
    for (at, call) in calls {
        let Some((result_at, result)) = ir.result_of(call) else {
            undecided.get_or_insert(UnknownReason::NoResultCorrelated { call_event: at });
            continue;
        };
        if let Some(field) = matcher.absent_fields(result).first() {
            undecided.get_or_insert(UnknownReason::ResultFieldAbsent {
                call_event: at,
                result_event: result_at,
                field: field.clone(),
            });
            continue;
        }
        if matcher.matches(result) {
            satisfied.push(result_at);
        } else {
            failed.push(result_at);
        }
    }
    if !failed.is_empty() {
        // A gap beside an unknown is still a gap: something was observed to be wrong.
        return Outcome::Gap(Citation::new(
            failed,
            format!("{} result(s) do not satisfy {matcher}", satisfied.len() + 1),
        ));
    }
    match undecided {
        Some(reason) => Outcome::Undecidable(reason),
        None => Outcome::Ok(Citation::new(
            satisfied,
            format!("every matched result satisfies {matcher}"),
        )),
    }
}

/// The bytes a call's result carried, per call and in total.
fn result_bytes(
    ir: &TraceIr,
    calls: &[(usize, &ToolCall)],
) -> (Vec<(usize, usize, u64)>, Option<UnknownReason>) {
    let mut measured = Vec::new();
    let mut undecided = None;
    for (at, call) in calls {
        match ir.result_of(call) {
            Some((result_at, result)) => measured.push((
                *at,
                result_at,
                u64::try_from(result.content_bytes).unwrap_or(u64::MAX),
            )),
            None => {
                if undecided.is_none() {
                    undecided = Some(UnknownReason::NoResultCorrelated { call_event: *at });
                }
            }
        }
    }
    (measured, undecided)
}

/// Result bytes stay within a bound.
fn tool_result_bytes(
    ir: &TraceIr,
    selector: &CallSelector,
    bytes: CountBound,
    per: Aggregate,
) -> Outcome {
    let calls = selected(ir, selector);
    if calls.is_empty() {
        return Outcome::Undecidable(UnknownReason::NothingInScope {
            selector: selector.to_string(),
        });
    }
    let (measured, undecided) = result_bytes(ir, &calls);
    if let Some(reason) = undecided {
        return Outcome::Undecidable(reason);
    }
    match per {
        Aggregate::Total => {
            let total: u64 = measured.iter().map(|(_, _, size)| size).sum();
            let events = measured.iter().map(|(_, result, _)| *result).collect();
            from_decision(
                decide_count(bytes, total, has_unread(ir)),
                events,
                format!("{selector} returned {total}B in total, {bytes}"),
                opaque_reason(ir),
            )
        }
        Aggregate::Each => {
            let over: Vec<usize> = measured
                .iter()
                .filter(|(_, _, size)| !bytes.holds(*size))
                .map(|(_, result, _)| *result)
                .collect();
            if over.is_empty() {
                Outcome::Ok(Citation::new(
                    measured.iter().map(|(_, result, _)| *result).collect(),
                    format!("every {selector} result is {bytes} bytes"),
                ))
            } else {
                Outcome::Gap(Citation::new(
                    over.clone(),
                    format!("{} result(s) are outside {bytes} bytes", over.len()),
                ))
            }
        }
    }
}

/// How many selected calls came back flagged as errors, and whether that could be read.
fn failures(
    ir: &TraceIr,
    calls: &[(usize, &ToolCall)],
) -> (u64, Vec<usize>, Option<UnknownReason>) {
    let mut failed = 0u64;
    let mut events = Vec::new();
    let mut undecided = None;
    for (at, call) in calls {
        let Some((result_at, result)) = ir.result_of(call) else {
            if undecided.is_none() {
                undecided = Some(UnknownReason::NoResultCorrelated { call_event: *at });
            }
            continue;
        };
        match result.is_error {
            Some(true) => {
                failed += 1;
                events.push(result_at);
            }
            Some(false) => {}
            None => {
                if undecided.is_none() {
                    undecided = Some(UnknownReason::ResultFieldAbsent {
                        call_event: *at,
                        result_event: result_at,
                        field: "is_error".to_owned(),
                    });
                }
            }
        }
    }
    (failed, events, undecided)
}

/// Failed calls stay within a bound.
fn tool_failed(ir: &TraceIr, selector: &CallSelector, count: CountBound) -> Outcome {
    let calls = selected(ir, selector);
    if calls.is_empty() {
        return Outcome::Undecidable(UnknownReason::NothingInScope {
            selector: selector.to_string(),
        });
    }
    let (failed, events, undecided) = failures(ir, &calls);
    let may_grow = has_unread(ir) || undecided.is_some();
    from_decision(
        decide_count(count, failed, may_grow),
        events,
        format!(
            "{failed} of {} {selector} call(s) failed, {count}",
            calls.len()
        ),
        undecided.unwrap_or_else(|| opaque_reason(ir)),
    )
}

/// Failed calls over total calls stays within a bound.
fn tool_error_rate(ir: &TraceIr, selector: &CallSelector, rate: RangeBound) -> Outcome {
    let calls = selected(ir, selector);
    if calls.is_empty() {
        // A rate over zero is not zero.
        return Outcome::Undecidable(UnknownReason::NothingInScope {
            selector: selector.to_string(),
        });
    }
    let (failed, events, undecided) = failures(ir, &calls);
    if let Some(reason) = undecided {
        return Outcome::Undecidable(reason);
    }
    #[allow(clippy::cast_precision_loss)] // Counts here are event counts: far below 2^53.
    let observed = failed as f64 / calls.len() as f64;
    let note = format!(
        "{failed} of {} {selector} call(s) failed — rate {observed:.3}, {rate}",
        calls.len()
    );
    if rate.holds(observed) {
        Outcome::Ok(Citation::new(events, note))
    } else {
        Outcome::Gap(Citation::new(events, note))
    }
}

/// Groups of byte-identical calls stay within a bound.
fn tool_repeated(ir: &TraceIr, selector: &CallSelector, count: CountBound) -> Outcome {
    let calls = selected(ir, selector);
    let mut groups: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (at, call) in calls {
        groups.entry(call.repetition_key()).or_default().push(at);
    }
    let repeated: Vec<&Vec<usize>> = groups.values().filter(|events| events.len() > 1).collect();
    let observed = repeated.len() as u64;
    let events: Vec<usize> = repeated.into_iter().flatten().copied().collect();
    from_decision(
        decide_count(count, observed, has_unread(ir)),
        events,
        format!("{observed} identical {selector} call group(s), {count}"),
        opaque_reason(ir),
    )
}

/// The first occurrence of one call precedes the first occurrence of another.
fn order(ir: &TraceIr, first: &CallSelector, before: &CallSelector) -> Outcome {
    let Some((first_at, _)) = selected(ir, first).into_iter().next() else {
        return Outcome::Undecidable(UnknownReason::NeverOccurred {
            selector: first.to_string(),
        });
    };
    let Some((before_at, _)) = selected(ir, before).into_iter().next() else {
        return Outcome::Undecidable(UnknownReason::NeverOccurred {
            selector: before.to_string(),
        });
    };
    let note = format!("first {first} at {first_at}, first {before} at {before_at}");
    if first_at < before_at {
        Outcome::Ok(Citation::new(vec![first_at, before_at], note))
    } else {
        Outcome::Gap(Citation::new(vec![first_at, before_at], note))
    }
}

// --- the terminal record ------------------------------------------------------------------

/// The terminal record matches, field by declared field.
fn run_result(
    ir: &TraceIr,
    is_error: Option<bool>,
    subtype: Option<&str>,
    stop_reason: Option<&str>,
    terminal_reason: Option<&str>,
    api_error_status: Option<&ApiErrorStatus>,
) -> Outcome {
    let (at, run) = match terminal(ir) {
        Ok(found) => found,
        Err(outcome) => return outcome,
    };
    let mut satisfied = Vec::new();
    let mut disagreed = Vec::new();

    if let Some(expected) = is_error {
        match run.is_error {
            None => return unknown_field("is_error"),
            Some(actual) if actual == expected => satisfied.push(format!("is_error={actual}")),
            Some(actual) => disagreed.push(format!("is_error={actual}, expected {expected}")),
        }
    }
    for (expected, actual, field) in [
        (subtype, run.subtype.as_deref(), "subtype"),
        (stop_reason, run.stop_reason.as_deref(), "stop_reason"),
        (
            terminal_reason,
            run.terminal_reason.as_deref(),
            "terminal_reason",
        ),
    ] {
        let Some(expected) = expected else { continue };
        match actual {
            None => return unknown_field(field),
            Some(actual) if actual == expected => satisfied.push(format!("{field}={actual}")),
            Some(actual) => disagreed.push(format!("{field}={actual}, expected {expected}")),
        }
    }
    if let Some(expected) = api_error_status {
        match (expected, run.api_error_status.as_deref()) {
            (ApiErrorStatus::Absent, None) => satisfied.push("api_error_status absent".to_owned()),
            (ApiErrorStatus::Absent, Some(actual)) => {
                disagreed.push(format!("api_error_status={actual}, expected none"));
            }
            (ApiErrorStatus::Equals { value }, Some(actual)) if actual == value => {
                satisfied.push(format!("api_error_status={actual}"));
            }
            (ApiErrorStatus::Equals { value }, Some(actual)) => {
                disagreed.push(format!("api_error_status={actual}, expected {value}"));
            }
            (ApiErrorStatus::Equals { value }, None) => {
                disagreed.push(format!("api_error_status absent, expected {value}"));
            }
        }
    }

    if disagreed.is_empty() {
        holds(at, satisfied.join(", "))
    } else {
        contradicted(at, disagreed.join(", "))
    }
}

/// A whole number the terminal record carries, against a bound.
fn outcome_count(
    ir: &TraceIr,
    field: &str,
    bound: CountBound,
    read: impl Fn(&RunOutcome) -> Option<u64>,
) -> Outcome {
    let (at, run) = match terminal(ir) {
        Ok(found) => found,
        Err(outcome) => return outcome,
    };
    match read(run) {
        None => unknown_field(field),
        Some(value) if bound.holds(value) => holds(at, format!("{field} = {value}, {bound}")),
        Some(value) => contradicted(at, format!("{field} = {value}, {bound}")),
    }
}

/// A whole number the run's aggregate usage carries, against a bound.
fn usage_count(
    ir: &TraceIr,
    field: &str,
    bound: CountBound,
    read: impl Fn(&RunUsage) -> Option<u64>,
) -> Outcome {
    let (at, usage) = match usage(ir) {
        Ok(found) => found,
        Err(outcome) => return outcome,
    };
    match read(usage) {
        None => unknown_field(field),
        Some(value) if bound.holds(value) => holds(at, format!("{field} = {value}, {bound}")),
        Some(value) => contradicted(at, format!("{field} = {value}, {bound}")),
    }
}

/// A text field the run's aggregate usage carries, compared for equality.
fn usage_text(
    ir: &TraceIr,
    field: &str,
    expected: &str,
    read: impl Fn(&RunUsage) -> Option<&str>,
) -> Outcome {
    let (at, usage) = match usage(ir) {
        Ok(found) => found,
        Err(outcome) => return outcome,
    };
    match read(usage) {
        None => unknown_field(field),
        Some(actual) if actual == expected => holds(at, format!("{field} = {actual}")),
        Some(actual) => contradicted(at, format!("{field} = {actual}, expected {expected}")),
    }
}

/// A count of what is in the transcript itself, which is never undecidable.
fn whole_run_count(what: &str, bound: CountBound, observed: u64) -> Outcome {
    let note = format!("{observed} {what}, {bound}");
    if bound.holds(observed) {
        Outcome::Ok(Citation::run(note))
    } else {
        Outcome::Gap(Citation::run(note))
    }
}

/// The last thinking estimate the harness emitted.
fn thinking_estimated(ir: &TraceIr, bound: CountBound) -> Outcome {
    let Some((at, estimate)) = ir.last_thinking_estimate() else {
        return Outcome::Undecidable(UnknownReason::NoThinkingEstimate);
    };
    let note = format!("last thinking estimate {estimate}, {bound}");
    if bound.holds(estimate) {
        holds(at, note)
    } else {
        contradicted(at, note)
    }
}

/// The final assistant text matches.
fn text_matches(ir: &TraceIr, matcher: &FieldMatcher) -> Outcome {
    let Some((at, text)) = ir.final_assistant_text() else {
        return Outcome::Undecidable(UnknownReason::NoFinalText);
    };
    // The note names the matcher, never the text: the weakest kind on the list should not also be
    // the one that pastes a model's prose into a report.
    let note = format!("the final assistant text ({} chars) {matcher}", text.len());
    if matcher.matches_text(text) {
        holds(at, note)
    } else {
        contradicted(
            at,
            format!("the final assistant text does not match {matcher}"),
        )
    }
}

// --- the rate-limit family -----------------------------------------------------------------

/// The rate-limit status is in an allowed set.
fn rate_limit_status(ir: &TraceIr, allowed: &std::collections::BTreeSet<String>) -> Outcome {
    let Some((at, state)) = ir.rate_limit() else {
        return Outcome::Undecidable(UnknownReason::NoRateLimitEvent);
    };
    match &state.status {
        None => unknown_field("rate_limit.status"),
        Some(status) if allowed.contains(status) => holds(at, format!("status = {status}")),
        Some(status) => contradicted(
            at,
            format!(
                "status = {status}, allowed {}",
                allowed.iter().cloned().collect::<Vec<_>>().join(", ")
            ),
        ),
    }
}

/// Whether the run was paid for out of overage.
fn rate_limit_overage(ir: &TraceIr, expected: bool) -> Outcome {
    let Some((at, state)) = ir.rate_limit() else {
        return Outcome::Undecidable(UnknownReason::NoRateLimitEvent);
    };
    match state.is_using_overage {
        None => unknown_field("rate_limit.is_using_overage"),
        Some(actual) if actual == expected => holds(at, format!("is_using_overage = {actual}")),
        Some(actual) => contradicted(
            at,
            format!("is_using_overage = {actual}, expected {expected}"),
        ),
    }
}

/// How much of the rate-limit window was used.
fn rate_limit_utilization(ir: &TraceIr, bound: RangeBound) -> Outcome {
    let Some((at, state)) = ir.rate_limit() else {
        return Outcome::Undecidable(UnknownReason::NoRateLimitEvent);
    };
    match state.utilization {
        None => unknown_field("rate_limit.utilization"),
        Some(value) => {
            let note = format!("utilization = {value}, {bound}");
            if bound.holds(value) {
                holds(at, note)
            } else {
                contradicted(at, note)
            }
        }
    }
}

// --- tokens, cost and cache -----------------------------------------------------------------

/// Reads one token quantity, run-wide or for one model.
fn read_tokens(
    run: &RunOutcome,
    model: Option<&str>,
    which: Token,
) -> Result<Option<u64>, Outcome> {
    if let Some(model) = model {
        let scoped = model_usage(run, model)?;
        return Ok(match which {
            Token::Input => scoped.input_tokens,
            Token::Output => scoped.output_tokens,
            Token::Total => scoped.input_tokens.zip(scoped.output_tokens).map(sum),
            Token::CacheRead => scoped.cache_read_input_tokens,
            Token::CacheCreated => scoped.cache_creation_input_tokens,
        });
    }
    let Some(usage) = &run.usage else {
        return Err(unknown_field("usage"));
    };
    Ok(match which {
        Token::Input => usage.input_tokens,
        Token::Output => usage.output_tokens,
        Token::Total => usage.input_tokens.zip(usage.output_tokens).map(sum),
        Token::CacheRead => usage.cache_read_input_tokens,
        Token::CacheCreated => usage.cache_creation_input_tokens,
    })
}

/// Adds a pair, saturating — a token total cannot overflow a `u64` from a real run, and
/// saturating beats panicking on a corrupt one.
fn sum(pair: (u64, u64)) -> u64 {
    pair.0.saturating_add(pair.1)
}

/// A token quantity against a bound, run-wide or for one model.
fn tokens(
    ir: &TraceIr,
    field: &str,
    bound: CountBound,
    model: Option<&str>,
    which: Token,
) -> Outcome {
    let (at, run) = match terminal(ir) {
        Ok(found) => found,
        Err(outcome) => return outcome,
    };
    let value = match read_tokens(run, model, which) {
        Ok(value) => value,
        Err(outcome) => return outcome,
    };
    let scope = model.map_or_else(String::new, |model| format!(" for {model}"));
    match value {
        None => unknown_field(field),
        Some(value) if bound.holds(value) => {
            holds(at, format!("{field}{scope} = {value}, {bound}"))
        }
        Some(value) => contradicted(at, format!("{field}{scope} = {value}, {bound}")),
    }
}

/// What the run cost, run-wide or for one model.
fn cost_total(ir: &TraceIr, bound: RangeBound, model: Option<&str>) -> Outcome {
    let (at, run) = match terminal(ir) {
        Ok(found) => found,
        Err(outcome) => return outcome,
    };
    let value = match model {
        Some(model) => match model_usage(run, model) {
            Ok(scoped) => scoped.cost_usd,
            Err(outcome) => return outcome,
        },
        None => run.total_cost_usd,
    };
    let scope = model.map_or_else(String::new, |model| format!(" for {model}"));
    match value {
        None => unknown_field("total_cost_usd"),
        Some(value) => {
            let note = format!("cost{scope} = ${value:.4}, {bound}");
            if bound.holds(value) {
                holds(at, note)
            } else {
                contradicted(at, note)
            }
        }
    }
}

/// Whether the run read anything from the cache.
fn cache_used(ir: &TraceIr, expected: bool) -> Outcome {
    let (at, usage) = match usage(ir) {
        Ok(found) => found,
        Err(outcome) => return outcome,
    };
    match usage.cache_read_input_tokens {
        None => unknown_field("usage.cache_read_input_tokens"),
        Some(read) => {
            let used = read > 0;
            let note = format!("cache read {read} token(s), used = {used}");
            if used == expected {
                holds(at, note)
            } else {
                contradicted(at, format!("{note}, expected {expected}"))
            }
        }
    }
}

/// The cache hit ratio, with the denominator the specification states.
fn cache_hit_ratio(ir: &TraceIr, bound: RangeBound) -> Outcome {
    let (at, usage) = match usage(ir) {
        Ok(found) => found,
        Err(outcome) => return outcome,
    };
    let (Some(read), Some(input)) = (usage.cache_read_input_tokens, usage.input_tokens) else {
        return unknown_field("usage.cache_read_input_tokens + usage.input_tokens");
    };
    let denominator = read.saturating_add(input);
    if denominator == 0 {
        return Outcome::Undecidable(UnknownReason::RatioUndefined {
            denominator: "cache_read_input_tokens + input_tokens".to_owned(),
        });
    }
    #[allow(clippy::cast_precision_loss)] // Token counts are far below 2^53.
    let ratio = read as f64 / denominator as f64;
    let note = format!("hit ratio {ratio:.5} = {read} / ({read} + {input}), {bound}");
    if bound.holds(ratio) {
        holds(at, note)
    } else {
        contradicted(at, note)
    }
}

// --- derived timings --------------------------------------------------------------------

/// The interval a phase reads off a step.
fn interval(step: &Step, phase: Phase) -> Option<i64> {
    match phase {
        Phase::Gen => step.gen_ms,
        Phase::Exec => step.exec_ms,
    }
}

/// Every selected step's interval is within a bound.
fn step_time(ir: &TraceIr, selector: &CallSelector, bound: CountBound, phase: Phase) -> Outcome {
    let selected: Vec<usize> = self::selected(ir, selector)
        .into_iter()
        .map(|(at, _)| at)
        .collect();
    let steps: Vec<Step> = ir
        .steps()
        .into_iter()
        .filter(|step| selected.contains(&step.call_event))
        .collect();
    if steps.is_empty() {
        return Outcome::Undecidable(UnknownReason::NothingInScope {
            selector: selector.to_string(),
        });
    }
    let mut over = Vec::new();
    let mut measured = Vec::new();
    for step in &steps {
        // A non-monotonic pair is treated exactly like an unrecorded one: the transcript cannot
        // state the duration, and inventing a magnitude for it would be measuring rather than
        // reading.
        let Some(value) = interval(step, phase).and_then(|ms| u64::try_from(ms).ok()) else {
            return Outcome::Undecidable(UnknownReason::TimestampAbsent {
                event: step.call_event,
            });
        };
        measured.push(step.call_event);
        if !bound.holds(value) {
            over.push(step.call_event);
        }
    }
    let what = phase.as_str();
    if over.is_empty() {
        Outcome::Ok(Citation::new(
            measured,
            format!(
                "every {selector} step's {what} is {bound} ms across {} step(s)",
                steps.len()
            ),
        ))
    } else {
        Outcome::Gap(Citation::new(
            over.clone(),
            format!(
                "{} of {} {selector} step(s) are outside {bound} ms of {what}",
                over.len(),
                steps.len()
            ),
        ))
    }
}

/// The sum of every step's interval is within a bound.
fn step_total(ir: &TraceIr, bound: CountBound, phase: Phase) -> Outcome {
    let steps = ir.steps();
    if steps.is_empty() {
        return Outcome::Undecidable(UnknownReason::NothingInScope {
            selector: "any tool".to_owned(),
        });
    }
    let mut total = 0u64;
    for step in &steps {
        let Some(value) = interval(step, phase).and_then(|ms| u64::try_from(ms).ok()) else {
            return Outcome::Undecidable(UnknownReason::TimestampAbsent {
                event: step.call_event,
            });
        };
        total = total.saturating_add(value);
    }
    let what = phase.as_str();
    let note = format!(
        "{what} total {total} ms across {} step(s), {bound}",
        steps.len()
    );
    let events = steps.iter().map(|step| step.call_event).collect();
    if bound.holds(total) {
        Outcome::Ok(Citation::new(events, note))
    } else {
        Outcome::Gap(Citation::new(events, note))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use trace_domain::ir::{
        AdapterRef, EventKind, McpServer, OpaqueEvent, RunOutcome, RunUsage, ToolResult, TraceEvent,
    };
    use trace_domain::matcher::{FieldMatcher, ScalarValue};

    use super::*;
    use crate::report::Verdict;

    fn adapter() -> AdapterRef {
        AdapterRef {
            name: "test",
            written_against: &["0"],
        }
    }

    fn call_event(line: usize, at: Option<&str>, id: &str, name: &str, input: &str) -> TraceEvent {
        let parsed: serde_json::Value = serde_json::from_str(input).expect("the fixture is JSON");
        let map = parsed
            .as_object()
            .expect("a tool input is an object")
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();
        TraceEvent::new(
            line,
            at.map(ToOwned::to_owned),
            EventKind::ToolCall(Box::new(ToolCall {
                call_id: Some(id.to_owned()),
                name: name.to_owned(),
                input: map,
                input_bytes: input.len(),
                result_event: None,
            })),
        )
    }

    fn result_event(
        line: usize,
        at: Option<&str>,
        id: &str,
        is_error: Option<bool>,
        fields: &str,
    ) -> TraceEvent {
        let parsed: serde_json::Value = serde_json::from_str(fields).expect("the fixture is JSON");
        let map = parsed
            .as_object()
            .expect("result fields are an object")
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();
        TraceEvent::new(
            line,
            at.map(ToOwned::to_owned),
            EventKind::ToolResult(Box::new(ToolResult {
                call_id: Some(id.to_owned()),
                is_error,
                content_bytes: fields.len(),
                content: None,
                fields: map,
            })),
        )
    }

    fn opaque_event(line: usize) -> TraceEvent {
        TraceEvent::new(
            line,
            None,
            EventKind::Opaque(Box::new(OpaqueEvent {
                event_type: Some("tool_stream".to_owned()),
                subtype: None,
                digest: "0".repeat(64),
            })),
        )
    }

    fn ir(events: Vec<TraceEvent>) -> TraceIr {
        TraceIr::new("digest".to_owned(), adapter(), events, Vec::new())
    }

    fn bash(command: &str) -> CallSelector {
        let mut selector = CallSelector::tool("Bash");
        selector.args.insert(
            "command".to_owned(),
            FieldMatcher::Contains(command.to_owned()),
        );
        selector
    }

    // ---------------------------------------------------------------------------------------
    // The whole vocabulary, against a real committed run, with a negative case beside each.
    //
    // The pairing is the design's own T2 acceptance criterion and the standard `infra-spec` sets
    // for itself, and it is the only shape that catches the failure this family exists to
    // prevent: a kind that reports `ok` because it is not looking. A positive case alone would
    // pass for a checker whose every arm returned `ok`.
    //
    // That the *document* can express all fifty-one is a separate claim, checked where it
    // belongs — in `trace_domain::raw`'s own tests, against the wire form.

    /// The committed transcript of eval run `7hTYjT`: 36 events, 2026-08-21.
    const SEVEN_H: &[u8] = include_bytes!("../tests/fixtures/plugin-eval-7hTYjT.jsonl");

    /// The skill the eval is about.
    const SKILL: &str = "engineering-protocols:planning";

    /// The plugin the eval is about.
    const PLUGIN: &str = "engineering-protocols";

    /// Every tool run `7hTYjT` was offered, in the order the opening record lists them.
    ///
    /// Written out rather than derived from the fixture, because a set derived from the thing
    /// under test cannot contradict it: this is the literal an exactness expectation would be
    /// authored with, and `Task` being in it is the hole the driver's enforcement mapping names.
    const OFFERED: &[&str] = &[
        "Task",
        "Bash",
        "CronCreate",
        "CronDelete",
        "CronList",
        "DesignSync",
        "Edit",
        "EnterWorktree",
        "ExitWorktree",
        "Glob",
        "Grep",
        "ListAgents",
        "Monitor",
        "NotebookEdit",
        "PushNotification",
        "Read",
        "RemoteTrigger",
        "ReportFindings",
        "ScheduleWakeup",
        "SendMessage",
        "Skill",
        "TaskCreate",
        "TaskGet",
        "TaskList",
        "TaskOutput",
        "TaskStop",
        "TaskUpdate",
        "ToolSearch",
        "WebFetch",
        "WebSearch",
        "Workflow",
        "Write",
    ];

    fn real_run() -> TraceIr {
        crate::adapter::read_transcript(SEVEN_H)
            .expect("the committed fixture is a transcript this build reads")
    }

    fn user_modified(value: bool) -> ResultMatcher {
        let mut matcher = ResultMatcher::default();
        matcher.fields.insert(
            "userModified".to_owned(),
            FieldMatcher::Equals(ScalarValue::Bool(value)),
        );
        matcher
    }

    fn names(set: &[&str]) -> BTreeSet<String> {
        set.iter().map(|name| (*name).to_owned()).collect()
    }
    /// One row of the table: the kind, and what this transcript says about it.
    struct Case {
        kind: ExpectationKind,
        expected: Verdict,
    }

    fn case(kind: ExpectationKind, expected: Verdict) -> Case {
        Case { kind, expected }
    }

    /// The whole vocabulary, positive and negative, against run `7hTYjT`.
    ///
    /// Observed values, all read from the fixture: 13 turns, 8 API requests, 19 assistant events, 1
    /// iteration, 16 input tokens, 3 824 output, 34 thinking, 313 513 cache reads, 20 168 cache
    /// creation, `$0.2736589`, 42 167 ms total, 42 955 ms API, 1 915 ms to first token, 50 ms to first
    /// request, 11 tool calls (Bash 4, Edit 3, Read 3, Skill 1), 0 failures, 0 repeated groups,
    /// 27 761 ms of inference and 415 ms of tool execution.
    #[allow(clippy::too_many_lines)] // A table. Splitting it would hide the pairing it exists to show.
    fn table() -> Vec<Case> {
        vec![
            // --- the environment -----------------------------------------------------------
            case(
                ExpectationKind::EnvPluginLoaded {
                    plugin: PLUGIN.to_owned(),
                    version: Some("0.1.0".to_owned()),
                    source: Some("engineering-protocols@inline".to_owned()),
                },
                Verdict::Ok,
            ),
            case(
                ExpectationKind::EnvPluginLoaded {
                    plugin: PLUGIN.to_owned(),
                    version: Some("9.9.9".to_owned()),
                    source: None,
                },
                Verdict::Gap,
            ),
            case(
                ExpectationKind::EnvExclusive {
                    plugins: names(&[PLUGIN]),
                },
                Verdict::Ok,
            ),
            case(
                ExpectationKind::EnvExclusive {
                    plugins: names(&[PLUGIN, "track"]),
                },
                Verdict::Gap,
            ),
            case(
                ExpectationKind::EnvOutputStyle {
                    equals: "default".to_owned(),
                },
                Verdict::Ok,
            ),
            case(
                ExpectationKind::EnvOutputStyle {
                    equals: "Operator Report".to_owned(),
                },
                Verdict::Gap,
            ),
            case(
                ExpectationKind::EnvSkillAvailable {
                    skill: SKILL.to_owned(),
                },
                Verdict::Ok,
            ),
            case(
                ExpectationKind::EnvSkillAvailable {
                    skill: "engineering-protocols:nonesuch".to_owned(),
                },
                Verdict::Gap,
            ),
            case(
                ExpectationKind::EnvAgentAvailable {
                    agent: "engineering-protocols:decomposer".to_owned(),
                },
                Verdict::Ok,
            ),
            case(
                ExpectationKind::EnvAgentAvailable {
                    agent: "engineering-protocols:nonesuch".to_owned(),
                },
                Verdict::Gap,
            ),
            case(
                ExpectationKind::EnvToolAvailable {
                    availability: ToolAvailability::Offered {
                        tool: "Bash".to_owned(),
                    },
                },
                Verdict::Ok,
            ),
            case(
                ExpectationKind::EnvToolAvailable {
                    availability: ToolAvailability::Offered {
                        tool: "Nonesuch".to_owned(),
                    },
                },
                Verdict::Gap,
            ),
            case(
                ExpectationKind::EnvToolAvailable {
                    availability: ToolAvailability::NotOffered {
                        tool: "Nonesuch".to_owned(),
                    },
                },
                Verdict::Ok,
            ),
            // The one the driver's enforcement mapping is about: `Task` maps to no protocol
            // action, and this run was offered it. A specification that forbids it gaps here,
            // which is the point — the run never called `Task`, so `tool.absent` would be green.
            case(
                ExpectationKind::EnvToolAvailable {
                    availability: ToolAvailability::NotOffered {
                        tool: "Task".to_owned(),
                    },
                },
                Verdict::Gap,
            ),
            case(
                ExpectationKind::EnvToolAvailable {
                    availability: ToolAvailability::Only {
                        tools: names(OFFERED),
                    },
                },
                Verdict::Ok,
            ),
            case(
                ExpectationKind::EnvToolAvailable {
                    availability: ToolAvailability::Only {
                        tools: names(&["Read", "Glob", "Grep"]),
                    },
                },
                Verdict::Gap,
            ),
            // Run `7hTYjT` listed `mcp_servers: []` — an empty list, not an absent field, which
            // is what makes the hermetic bound decidable here at all.
            case(
                ExpectationKind::EnvMcpServers {
                    count: CountBound::at_most(0),
                },
                Verdict::Ok,
            ),
            case(
                ExpectationKind::EnvMcpServers {
                    count: CountBound::at_least(1),
                },
                Verdict::Gap,
            ),
            case(
                ExpectationKind::EnvModel {
                    equals: "claude-sonnet-5".to_owned(),
                },
                Verdict::Ok,
            ),
            case(
                ExpectationKind::EnvModel {
                    equals: "sonnet".to_owned(),
                },
                Verdict::Gap,
            ),
            case(
                ExpectationKind::EnvPermissionMode {
                    equals: "dontAsk".to_owned(),
                },
                Verdict::Ok,
            ),
            case(
                ExpectationKind::EnvPermissionMode {
                    equals: "acceptEdits".to_owned(),
                },
                Verdict::Gap,
            ),
            case(
                ExpectationKind::EnvApiKeySource {
                    equals: "none".to_owned(),
                },
                Verdict::Ok,
            ),
            case(
                ExpectationKind::EnvApiKeySource {
                    equals: "ANTHROPIC_API_KEY".to_owned(),
                },
                Verdict::Gap,
            ),
            // --- the skill -----------------------------------------------------------------
            case(
                ExpectationKind::SkillInvoked {
                    skill: SKILL.to_owned(),
                    count: CountBound::exactly(1),
                },
                Verdict::Ok,
            ),
            case(
                ExpectationKind::SkillInvoked {
                    skill: SKILL.to_owned(),
                    count: CountBound::at_least(2),
                },
                Verdict::Gap,
            ),
            case(
                ExpectationKind::SkillCompleted {
                    skill: SKILL.to_owned(),
                    count: CountBound::at_least(1),
                },
                Verdict::Ok,
            ),
            case(
                ExpectationKind::SkillCompleted {
                    skill: "engineering-protocols:nonesuch".to_owned(),
                    count: CountBound::at_least(1),
                },
                Verdict::Gap,
            ),
            // --- what the agent did ---------------------------------------------------------
            case(
                ExpectationKind::ToolCalled {
                    selector: bash("protocol artifact new"),
                    count: CountBound::at_least(1),
                },
                Verdict::Ok,
            ),
            case(
                ExpectationKind::ToolCalled {
                    selector: bash("protocol artifact new"),
                    count: CountBound::at_least(9),
                },
                Verdict::Gap,
            ),
            case(
                ExpectationKind::ToolAbsent {
                    selector: bash("rm -rf"),
                },
                Verdict::Ok,
            ),
            case(
                ExpectationKind::ToolAbsent {
                    selector: CallSelector::tool("Edit"),
                },
                Verdict::Gap,
            ),
            case(
                ExpectationKind::ToolResultMatches {
                    selector: CallSelector::tool("Edit"),
                    result: user_modified(false),
                },
                Verdict::Ok,
            ),
            case(
                ExpectationKind::ToolResultMatches {
                    selector: CallSelector::tool("Edit"),
                    result: user_modified(true),
                },
                Verdict::Gap,
            ),
            case(
                ExpectationKind::ToolResultBytes {
                    selector: CallSelector::tool("Read"),
                    bytes: CountBound::at_most(100_000),
                    per: Aggregate::Total,
                },
                Verdict::Ok,
            ),
            case(
                ExpectationKind::ToolResultBytes {
                    selector: CallSelector::tool("Read"),
                    bytes: CountBound::at_most(1),
                    per: Aggregate::Each,
                },
                Verdict::Gap,
            ),
            case(
                ExpectationKind::ToolFailed {
                    selector: CallSelector::tool("Bash"),
                    count: CountBound::exactly(0),
                },
                Verdict::Ok,
            ),
            case(
                ExpectationKind::ToolFailed {
                    selector: CallSelector::tool("Bash"),
                    count: CountBound::at_least(1),
                },
                Verdict::Gap,
            ),
            case(
                ExpectationKind::ToolErrorRate {
                    selector: CallSelector::tool("Bash"),
                    rate: RangeBound::at_most(0.0),
                },
                Verdict::Ok,
            ),
            case(
                ExpectationKind::ToolErrorRate {
                    selector: CallSelector::tool("Bash"),
                    rate: RangeBound::at_least(0.5),
                },
                Verdict::Gap,
            ),
            case(
                ExpectationKind::ToolRepeated {
                    selector: CallSelector::default(),
                    count: CountBound::at_most(0),
                },
                Verdict::Ok,
            ),
            case(
                ExpectationKind::ToolRepeated {
                    selector: CallSelector::default(),
                    count: CountBound::at_least(1),
                },
                Verdict::Gap,
            ),
            case(
                ExpectationKind::Order {
                    first: bash("protocol artifact"),
                    before: CallSelector::tool("Edit"),
                },
                Verdict::Ok,
            ),
            case(
                ExpectationKind::Order {
                    first: CallSelector::tool("Edit"),
                    before: bash("protocol artifact"),
                },
                Verdict::Gap,
            ),
            case(
                ExpectationKind::RunResult {
                    is_error: Some(false),
                    subtype: Some("success".to_owned()),
                    stop_reason: None,
                    terminal_reason: Some("completed".to_owned()),
                    api_error_status: Some(ApiErrorStatus::Absent),
                },
                Verdict::Ok,
            ),
            case(
                ExpectationKind::RunResult {
                    is_error: Some(true),
                    subtype: None,
                    stop_reason: None,
                    terminal_reason: None,
                    api_error_status: None,
                },
                Verdict::Gap,
            ),
            case(
                ExpectationKind::PermissionDenied {
                    count: CountBound::exactly(0),
                },
                Verdict::Ok,
            ),
            case(
                ExpectationKind::PermissionDenied {
                    count: CountBound::at_least(1),
                },
                Verdict::Gap,
            ),
            case(
                ExpectationKind::SubagentSpawned {
                    count: CountBound::exactly(0),
                },
                Verdict::Ok,
            ),
            case(
                ExpectationKind::SubagentSpawned {
                    count: CountBound::at_least(1),
                },
                Verdict::Gap,
            ),
            case(
                ExpectationKind::TextMatches {
                    matcher: FieldMatcher::Contains("Validation".to_owned()),
                },
                Verdict::Ok,
            ),
            case(
                ExpectationKind::TextMatches {
                    matcher: FieldMatcher::Contains("catastrophic failure".to_owned()),
                },
                Verdict::Gap,
            ),
            // --- the rate-limit family --------------------------------------------------------
            case(
                ExpectationKind::RateLimitStatus {
                    allowed: names(&["allowed", "allowed_warning"]),
                },
                Verdict::Ok,
            ),
            case(
                ExpectationKind::RateLimitStatus {
                    allowed: names(&["allowed"]),
                },
                Verdict::Gap,
            ),
            case(
                ExpectationKind::RateLimitOverage { equals: false },
                Verdict::Ok,
            ),
            case(
                ExpectationKind::RateLimitOverage { equals: true },
                Verdict::Gap,
            ),
            case(
                ExpectationKind::RateLimitUtilization {
                    utilization: RangeBound::at_most(1.0),
                },
                Verdict::Ok,
            ),
            case(
                ExpectationKind::RateLimitUtilization {
                    utilization: RangeBound::at_most(0.1),
                },
                Verdict::Gap,
            ),
            // --- counting a run ---------------------------------------------------------------
            case(
                ExpectationKind::Turns {
                    count: CountBound::exactly(13),
                },
                Verdict::Ok,
            ),
            case(
                ExpectationKind::Turns {
                    count: CountBound::at_most(2),
                },
                Verdict::Gap,
            ),
            case(
                ExpectationKind::ApiRequests {
                    count: CountBound::exactly(8),
                },
                Verdict::Ok,
            ),
            case(
                ExpectationKind::ApiRequests {
                    count: CountBound::at_most(2),
                },
                Verdict::Gap,
            ),
            case(
                ExpectationKind::EventsAssistant {
                    count: CountBound::exactly(19),
                },
                Verdict::Ok,
            ),
            case(
                ExpectationKind::EventsAssistant {
                    count: CountBound::at_most(2),
                },
                Verdict::Gap,
            ),
            case(
                ExpectationKind::Iterations {
                    count: CountBound::exactly(1),
                },
                Verdict::Ok,
            ),
            case(
                ExpectationKind::Iterations {
                    count: CountBound::at_least(2),
                },
                Verdict::Gap,
            ),
            // --- what it cost -------------------------------------------------------------------
            case(
                ExpectationKind::TokensInput {
                    count: CountBound::exactly(16),
                    model: None,
                },
                Verdict::Ok,
            ),
            case(
                ExpectationKind::TokensInput {
                    count: CountBound::at_least(1_000),
                    model: None,
                },
                Verdict::Gap,
            ),
            case(
                ExpectationKind::TokensOutput {
                    count: CountBound::exactly(3_824),
                    model: Some("claude-sonnet-5".to_owned()),
                },
                Verdict::Ok,
            ),
            case(
                ExpectationKind::TokensOutput {
                    count: CountBound::at_most(10),
                    model: None,
                },
                Verdict::Gap,
            ),
            case(
                ExpectationKind::TokensTotal {
                    count: CountBound::exactly(3_840),
                    model: None,
                },
                Verdict::Ok,
            ),
            case(
                ExpectationKind::TokensTotal {
                    count: CountBound::at_most(100),
                    model: None,
                },
                Verdict::Gap,
            ),
            case(
                ExpectationKind::TokensThinking {
                    count: CountBound::exactly(34),
                },
                Verdict::Ok,
            ),
            case(
                ExpectationKind::TokensThinking {
                    count: CountBound::at_least(1_000),
                },
                Verdict::Gap,
            ),
            case(
                ExpectationKind::ThinkingEstimated {
                    count: CountBound::exactly(80),
                },
                Verdict::Ok,
            ),
            case(
                ExpectationKind::ThinkingEstimated {
                    count: CountBound::at_least(1_000),
                },
                Verdict::Gap,
            ),
            case(
                ExpectationKind::CostTotal {
                    usd: RangeBound::at_most(1.00),
                    model: None,
                },
                Verdict::Ok,
            ),
            case(
                ExpectationKind::CostTotal {
                    usd: RangeBound::at_most(0.01),
                    model: None,
                },
                Verdict::Gap,
            ),
            case(ExpectationKind::CacheUsed { equals: true }, Verdict::Ok),
            case(ExpectationKind::CacheUsed { equals: false }, Verdict::Gap),
            case(
                ExpectationKind::CacheReadTokens {
                    count: CountBound::exactly(313_513),
                    model: None,
                },
                Verdict::Ok,
            ),
            case(
                ExpectationKind::CacheReadTokens {
                    count: CountBound::at_most(10),
                    model: None,
                },
                Verdict::Gap,
            ),
            case(
                ExpectationKind::CacheCreatedTokens {
                    count: CountBound::at_most(100_000),
                    model: None,
                },
                Verdict::Ok,
            ),
            case(
                ExpectationKind::CacheCreatedTokens {
                    count: CountBound::at_most(10),
                    model: None,
                },
                Verdict::Gap,
            ),
            case(
                ExpectationKind::CacheHitRatio {
                    ratio: RangeBound::at_least(0.99),
                },
                Verdict::Ok,
            ),
            case(
                ExpectationKind::CacheHitRatio {
                    ratio: RangeBound::at_least(0.999_999),
                },
                Verdict::Gap,
            ),
            // --- where the wall clock went ----------------------------------------------------
            case(
                ExpectationKind::DurationTotal {
                    ms: CountBound::exactly(42_167),
                },
                Verdict::Ok,
            ),
            case(
                ExpectationKind::DurationTotal {
                    ms: CountBound::at_most(100),
                },
                Verdict::Gap,
            ),
            case(
                ExpectationKind::DurationApi {
                    ms: CountBound::exactly(42_955),
                },
                Verdict::Ok,
            ),
            case(
                ExpectationKind::DurationApi {
                    ms: CountBound::at_most(100),
                },
                Verdict::Gap,
            ),
            case(
                ExpectationKind::Ttft {
                    ms: CountBound::exactly(1_915),
                },
                Verdict::Ok,
            ),
            case(
                ExpectationKind::Ttft {
                    ms: CountBound::at_most(10),
                },
                Verdict::Gap,
            ),
            case(
                ExpectationKind::TimeToRequest {
                    ms: CountBound::exactly(50),
                },
                Verdict::Ok,
            ),
            case(
                ExpectationKind::TimeToRequest {
                    ms: CountBound::at_most(1),
                },
                Verdict::Gap,
            ),
            case(
                ExpectationKind::StepGenTime {
                    selector: CallSelector::tool("Bash"),
                    ms: CountBound::at_most(5_000),
                },
                Verdict::Ok,
            ),
            case(
                ExpectationKind::StepGenTime {
                    selector: CallSelector::tool("Bash"),
                    ms: CountBound::at_most(100),
                },
                Verdict::Gap,
            ),
            case(
                ExpectationKind::StepExecTime {
                    selector: CallSelector::tool("Bash"),
                    ms: CountBound::at_most(200),
                },
                Verdict::Ok,
            ),
            case(
                ExpectationKind::StepExecTime {
                    selector: CallSelector::tool("Bash"),
                    ms: CountBound::at_most(10),
                },
                Verdict::Gap,
            ),
            case(
                ExpectationKind::TimeInferenceTotal {
                    ms: CountBound::exactly(27_761),
                },
                Verdict::Ok,
            ),
            case(
                ExpectationKind::TimeInferenceTotal {
                    ms: CountBound::at_most(100),
                },
                Verdict::Gap,
            ),
            case(
                ExpectationKind::TimeToolExecTotal {
                    ms: CountBound::exactly(415),
                },
                Verdict::Ok,
            ),
            case(
                ExpectationKind::TimeToolExecTotal {
                    ms: CountBound::at_most(10),
                },
                Verdict::Gap,
            ),
            // --- environment-dependent ----------------------------------------------------------
            case(
                ExpectationKind::Speed {
                    equals: "standard".to_owned(),
                },
                Verdict::Ok,
            ),
            case(
                ExpectationKind::Speed {
                    equals: "fast".to_owned(),
                },
                Verdict::Gap,
            ),
            case(
                ExpectationKind::ServiceTier {
                    equals: "standard".to_owned(),
                },
                Verdict::Ok,
            ),
            case(
                ExpectationKind::ServiceTier {
                    equals: "priority".to_owned(),
                },
                Verdict::Gap,
            ),
        ]
    }

    #[test]
    fn every_kind_holds_on_the_real_run_and_a_negative_case_beside_it_does_not() {
        let ir = real_run();
        let mut covered: BTreeMap<&'static str, (bool, bool)> = BTreeMap::new();
        for Case { kind, expected } in table() {
            let outcome = evaluate(&kind, &ir);
            assert_eq!(
                outcome.verdict(),
                expected,
                "{} expected {expected} and got {}: {}",
                kind.name(),
                outcome.verdict(),
                outcome.detail()
            );
            let entry = covered.entry(kind.name()).or_insert((false, false));
            match expected {
                Verdict::Ok => entry.0 = true,
                Verdict::Gap => entry.1 = true,
                Verdict::Unknown => {}
            }
        }

        // The coverage guard. A kind added to the vocabulary and not exercised here would
        // otherwise ship untested, and this is the one file that would have caught it.
        for name in ExpectationKind::NAMES {
            let (positive, negative) = covered.get(name).copied().unwrap_or((false, false));
            assert!(positive, "{name} has no case that holds on the real run");
            assert!(negative, "{name} has no negative case beside it");
        }
        assert_eq!(
            covered.len(),
            ExpectationKind::NAMES.len(),
            "the table covers exactly the published vocabulary"
        );
    }

    #[test]
    fn an_unread_event_cannot_unmeet_a_lower_bound_that_is_already_met() {
        // The refinement the module doc argues for. An opaque event can only *add* calls, so an
        // `at_least` bound already satisfied is `ok` and not `unk` — a checker that reported
        // `unk` here would be timid rather than careful.
        assert_eq!(
            decide_count(CountBound::at_least(1), 2, true),
            Decision::Holds
        );
        assert_eq!(
            decide_count(CountBound::at_least(3), 2, true),
            Decision::Undecided,
            "an unread event could be the third call"
        );
        assert_eq!(
            decide_count(CountBound::at_most(2), 3, true),
            Decision::Fails,
            "a ceiling already exceeded cannot be met by a larger value"
        );
        assert_eq!(
            decide_count(CountBound::at_most(2), 1, true),
            Decision::Undecided
        );
        assert_eq!(
            decide_count(CountBound::exactly(0), 1, true),
            Decision::Fails
        );
        assert_eq!(
            decide_count(CountBound::exactly(1), 1, true),
            Decision::Undecided,
            "an unread event could be a second one"
        );
        assert_eq!(
            decide_count(CountBound::exactly(1), 1, false),
            Decision::Holds,
            "with nothing unread, the same numbers decide"
        );
    }

    #[test]
    fn an_event_the_adapter_could_not_read_makes_this_must_never_happen_undecidable() {
        let with_unread = ir(vec![opaque_event(1)]);
        let outcome = evaluate(
            &ExpectationKind::ToolAbsent {
                selector: bash("rm -rf"),
            },
            &with_unread,
        );
        assert_eq!(
            outcome.verdict(),
            Verdict::Unknown,
            "reporting `ok` here would be the checker saying \"the tool was never called\" when \
             it had stopped being able to see tool calls"
        );
        let clean = ir(Vec::new());
        assert_eq!(
            evaluate(
                &ExpectationKind::ToolAbsent {
                    selector: bash("rm -rf"),
                },
                &clean,
            )
            .verdict(),
            Verdict::Ok
        );
    }

    #[test]
    fn a_result_field_that_disagrees_is_a_gap_and_one_that_is_absent_is_undecidable() {
        let mut matcher = ResultMatcher::default();
        matcher.fields.insert(
            "userModified".to_owned(),
            FieldMatcher::Equals(ScalarValue::Bool(false)),
        );
        let kind = ExpectationKind::ToolResultMatches {
            selector: CallSelector::tool("Edit"),
            result: matcher,
        };

        let touched = ir(vec![
            call_event(1, None, "a", "Edit", r#"{"file_path":"/x"}"#),
            result_event(2, None, "a", Some(false), r#"{"userModified":true}"#),
        ]);
        assert_eq!(evaluate(&kind, &touched).verdict(), Verdict::Gap);

        let silent = ir(vec![
            call_event(1, None, "a", "Edit", r#"{"file_path":"/x"}"#),
            result_event(2, None, "a", Some(false), r#"{"filePath":"/x"}"#),
        ]);
        assert_eq!(
            evaluate(&kind, &silent).verdict(),
            Verdict::Unknown,
            "a harness that renamed the field is not an agent that misbehaved"
        );

        let truncated = ir(vec![call_event(
            1,
            None,
            "a",
            "Edit",
            r#"{"file_path":"/x"}"#,
        )]);
        assert_eq!(
            evaluate(&kind, &truncated).verdict(),
            Verdict::Unknown,
            "a truncated transcript is not a bad result"
        );
    }

    #[test]
    fn a_gap_beside_an_unknown_is_still_a_gap_because_something_was_observed_to_be_wrong() {
        let mut matcher = ResultMatcher::default();
        matcher.fields.insert(
            "userModified".to_owned(),
            FieldMatcher::Equals(ScalarValue::Bool(false)),
        );
        // Two Edits: one whose result contradicts the matcher, one whose result never arrived.
        let mixed = ir(vec![
            call_event(1, None, "a", "Edit", r#"{"file_path":"/x"}"#),
            result_event(2, None, "a", Some(false), r#"{"userModified":true}"#),
            call_event(3, None, "b", "Edit", r#"{"file_path":"/y"}"#),
        ]);
        assert_eq!(
            evaluate(
                &ExpectationKind::ToolResultMatches {
                    selector: CallSelector::tool("Edit"),
                    result: matcher,
                },
                &mixed,
            )
            .verdict(),
            Verdict::Gap
        );
    }

    #[test]
    fn an_expectation_cannot_pass_by_selecting_nothing() {
        // The `infra-spec` rule, carried over: a rate over zero calls is not zero, and a result
        // matcher over no call is not satisfied.
        let empty = ir(vec![call_event(
            1,
            None,
            "a",
            "Read",
            r#"{"file_path":"/x"}"#,
        )]);
        for kind in [
            ExpectationKind::ToolErrorRate {
                selector: CallSelector::tool("Bash"),
                rate: RangeBound::at_most(0.0),
            },
            ExpectationKind::ToolResultMatches {
                selector: CallSelector::tool("Bash"),
                result: ResultMatcher::default(),
            },
            ExpectationKind::ToolResultBytes {
                selector: CallSelector::tool("Bash"),
                bytes: CountBound::at_most(10),
                per: Aggregate::Total,
            },
            ExpectationKind::StepExecTime {
                selector: CallSelector::tool("Bash"),
                ms: CountBound::at_most(500),
            },
        ] {
            assert_eq!(
                evaluate(&kind, &empty).verdict(),
                Verdict::Unknown,
                "{} passed by selecting nothing",
                kind.name()
            );
        }
    }

    #[test]
    fn an_ordering_with_a_side_that_never_happened_is_undecidable_rather_than_a_failure() {
        let one_sided = ir(vec![call_event(
            1,
            None,
            "a",
            "Bash",
            r#"{"command":"protocol artifact new"}"#,
        )]);
        let kind = ExpectationKind::Order {
            first: bash("protocol artifact"),
            before: CallSelector::tool("Edit"),
        };
        assert_eq!(
            evaluate(&kind, &one_sided).verdict(),
            Verdict::Unknown,
            "\"A before B\" is undecidable when there is no B, and failing it blames the wrong \
             thing"
        );

        let both = ir(vec![
            call_event(
                1,
                None,
                "a",
                "Bash",
                r#"{"command":"protocol artifact new"}"#,
            ),
            call_event(2, None, "b", "Edit", r#"{"file_path":"/x"}"#),
        ]);
        assert_eq!(evaluate(&kind, &both).verdict(), Verdict::Ok);

        let wrong_way = ir(vec![
            call_event(1, None, "b", "Edit", r#"{"file_path":"/x"}"#),
            call_event(
                2,
                None,
                "a",
                "Bash",
                r#"{"command":"protocol artifact new"}"#,
            ),
        ]);
        assert_eq!(evaluate(&kind, &wrong_way).verdict(), Verdict::Gap);
    }

    /// A terminal record with the usage a resource expectation reads.
    fn outcome_ir(usage: RunUsage, cost: Option<f64>) -> TraceIr {
        ir(vec![TraceEvent::new(
            1,
            None,
            EventKind::RunOutcome(Box::new(RunOutcome {
                is_error: Some(false),
                total_cost_usd: cost,
                usage: Some(usage),
                ..RunOutcome::default()
            })),
        )])
    }

    #[test]
    fn a_ratio_with_no_denominator_is_undecidable_rather_than_zero() {
        let nothing = outcome_ir(
            RunUsage {
                cache_read_input_tokens: Some(0),
                input_tokens: Some(0),
                ..RunUsage::default()
            },
            None,
        );
        assert_eq!(
            evaluate(
                &ExpectationKind::CacheHitRatio {
                    ratio: RangeBound::at_least(0.9)
                },
                &nothing,
            )
            .verdict(),
            Verdict::Unknown
        );

        let observed = outcome_ir(
            RunUsage {
                cache_read_input_tokens: Some(313_513),
                input_tokens: Some(16),
                ..RunUsage::default()
            },
            None,
        );
        assert_eq!(
            evaluate(
                &ExpectationKind::CacheHitRatio {
                    ratio: RangeBound::at_least(0.9)
                },
                &observed,
            )
            .verdict(),
            Verdict::Ok
        );
    }

    #[test]
    fn an_expectation_scoped_to_a_model_the_run_never_used_is_undecidable_not_satisfied() {
        let mut per_model = BTreeMap::new();
        per_model.insert(
            "claude-sonnet-5".to_owned(),
            trace_domain::ir::ModelUsage {
                input_tokens: Some(16),
                output_tokens: Some(3_824),
                ..trace_domain::ir::ModelUsage::default()
            },
        );
        let run = ir(vec![TraceEvent::new(
            1,
            None,
            EventKind::RunOutcome(Box::new(RunOutcome {
                model_usage: Some(per_model),
                usage: Some(RunUsage::default()),
                ..RunOutcome::default()
            })),
        )]);
        assert_eq!(
            evaluate(
                &ExpectationKind::TokensOutput {
                    count: CountBound::at_most(10_000),
                    model: Some("claude-opus-4".to_owned()),
                },
                &run,
            )
            .verdict(),
            Verdict::Unknown,
            "an expectation must not be able to pass by selecting nothing"
        );
        assert_eq!(
            evaluate(
                &ExpectationKind::TokensOutput {
                    count: CountBound::at_most(10_000),
                    model: Some("claude-sonnet-5".to_owned()),
                },
                &run,
            )
            .verdict(),
            Verdict::Ok
        );
    }

    #[test]
    fn a_duration_around_an_event_with_no_timestamp_is_undecidable_and_never_zero() {
        let untimed = ir(vec![
            call_event(
                1,
                None,
                "a",
                "Bash",
                r#"{"command":"protocol artifact new"}"#,
            ),
            result_event(2, None, "a", Some(false), "{}"),
        ]);
        assert_eq!(
            evaluate(
                &ExpectationKind::StepExecTime {
                    selector: CallSelector::tool("Bash"),
                    ms: CountBound::at_most(500),
                },
                &untimed,
            )
            .verdict(),
            Verdict::Unknown
        );
        assert_eq!(
            evaluate(
                &ExpectationKind::TimeToolExecTotal {
                    ms: CountBound::at_most(5_000)
                },
                &untimed,
            )
            .verdict(),
            Verdict::Unknown,
            "a total that omitted an unmeasurable step would be a smaller number wearing the same \
             name"
        );

        let timed = ir(vec![
            call_event(
                1,
                Some("2026-08-21T12:00:00.000Z"),
                "a",
                "Bash",
                r#"{"command":"protocol artifact new"}"#,
            ),
            result_event(2, Some("2026-08-21T12:00:00.187Z"), "a", Some(false), "{}"),
        ]);
        assert_eq!(
            evaluate(
                &ExpectationKind::StepExecTime {
                    selector: CallSelector::tool("Bash"),
                    ms: CountBound::at_most(500),
                },
                &timed,
            )
            .verdict(),
            Verdict::Ok
        );
    }

    #[test]
    fn a_tool_expectation_over_a_run_that_recorded_no_tool_list_is_undecidable_not_a_gap() {
        // The second `unk` rule of the module doc, on the newest kind: a harness that records an
        // opening record without `tools` cannot answer the question, and *"the tool was not
        // offered"* is a different sentence from *"this transcript does not say"*. Reading the
        // absence as a gap would make an enforcement audit fail on a harness upgrade.
        let silent = ir(vec![TraceEvent::new(
            1,
            None,
            EventKind::SessionStart(Box::new(SessionStart {
                model: Some("claude-sonnet-5".to_owned()),
                ..SessionStart::default()
            })),
        )]);
        for availability in [
            ToolAvailability::Offered {
                tool: "Bash".to_owned(),
            },
            ToolAvailability::NotOffered {
                tool: "Task".to_owned(),
            },
            ToolAvailability::Only {
                tools: BTreeSet::from(["Bash".to_owned()]),
            },
        ] {
            let outcome = evaluate(&ExpectationKind::EnvToolAvailable { availability }, &silent);
            assert!(
                matches!(
                    &outcome,
                    Outcome::Undecidable(UnknownReason::FieldAbsent { field }) if field == "tools"
                ),
                "an absent tool list decided something: {}",
                outcome.detail()
            );
        }
        // And the same record with a list decides all three, so the `unk` above is about the
        // field and not about the shape of the test.
        let offered = ir(vec![TraceEvent::new(
            1,
            None,
            EventKind::SessionStart(Box::new(SessionStart {
                tools: Some(vec!["Bash".to_owned()]),
                ..SessionStart::default()
            })),
        )]);
        assert_eq!(
            evaluate(
                &ExpectationKind::EnvToolAvailable {
                    availability: ToolAvailability::Only {
                        tools: BTreeSet::from(["Bash".to_owned()]),
                    },
                },
                &offered,
            )
            .verdict(),
            Verdict::Ok
        );
    }

    /// An opening record listing `count` MCP servers under synthetic names.
    ///
    /// Synthetic deliberately. The run this kind was written from listed three account-level
    /// servers by name, and a committed fixture is not where an account's server names, ids or
    /// addresses belong — the count is the whole claim, so the count is all the fixture carries.
    fn session_with_mcp_servers(count: usize) -> TraceIr {
        let servers = (0..count)
            .map(|index| McpServer {
                name: format!("server-{index}"),
                status: Some("needs-auth".to_owned()),
            })
            .collect();
        ir(vec![TraceEvent::new(
            1,
            None,
            EventKind::SessionStart(Box::new(SessionStart {
                mcp_servers: Some(servers),
                ..SessionStart::default()
            })),
        )])
    }

    #[test]
    fn the_hermetic_mcp_bound_holds_at_zero_and_names_what_it_saw_when_it_does_not() {
        let hermetic = evaluate(
            &ExpectationKind::EnvMcpServers {
                count: CountBound::at_most(0),
            },
            &session_with_mcp_servers(0),
        );
        assert_eq!(
            hermetic.verdict(),
            Verdict::Ok,
            "a session given no MCP server is the hermetic case: {}",
            hermetic.detail()
        );

        // The observation the register row was opened for: two of the four sessions of governed
        // run `W4-1/1` listed three account-level servers, every one `needs-auth`, in a run whose
        // `CLAUDE_CONFIG_DIR` was a scratch directory with no `mcpServers` key and whose tree had
        // no `.mcp.json`. They arrive with the login, so no directory the runner controls can
        // exclude them — which is why the bound has to be asserted rather than assumed.
        let leaked = evaluate(
            &ExpectationKind::EnvMcpServers {
                count: CountBound::at_most(0),
            },
            &session_with_mcp_servers(3),
        );
        assert_eq!(leaked.verdict(), Verdict::Gap);
        assert!(
            leaked.detail().contains("3 MCP server(s)") && leaked.detail().contains("at most 0"),
            "a gap must name the count it saw and the bound it broke, or the reader has to open \
             the transcript to learn either: {}",
            leaked.detail()
        );
    }

    #[test]
    fn an_opening_record_that_lists_no_mcp_servers_at_all_is_undecidable_not_hermetic() {
        // The whole lesson of the row. A harness that stopped recording `mcp_servers` would turn
        // this bound into the greenest row in the report while saying nothing, and *"the field is
        // gone"* is exactly the case a hermeticity claim must not read as *"nothing was there"*.
        let silent = ir(vec![TraceEvent::new(
            1,
            None,
            EventKind::SessionStart(Box::new(SessionStart {
                model: Some("claude-sonnet-5".to_owned()),
                ..SessionStart::default()
            })),
        )]);
        let outcome = evaluate(
            &ExpectationKind::EnvMcpServers {
                count: CountBound::at_most(0),
            },
            &silent,
        );
        assert!(
            matches!(
                &outcome,
                Outcome::Undecidable(UnknownReason::FieldAbsent { field }) if field == "mcp_servers"
            ),
            "an absent server list decided something: {}",
            outcome.detail()
        );
    }

    #[test]
    fn an_environment_expectation_over_a_transcript_with_no_session_start_is_undecidable() {
        let headless = ir(Vec::new());
        for kind in [
            ExpectationKind::EnvApiKeySource {
                equals: "none".to_owned(),
            },
            ExpectationKind::EnvExclusive {
                plugins: BTreeSet::from(["engineering-protocols".to_owned()]),
            },
            ExpectationKind::EnvPluginLoaded {
                plugin: "engineering-protocols".to_owned(),
                version: None,
                source: None,
            },
            ExpectationKind::EnvToolAvailable {
                availability: ToolAvailability::NotOffered {
                    tool: "Task".to_owned(),
                },
            },
            ExpectationKind::EnvMcpServers {
                count: CountBound::at_most(0),
            },
        ] {
            assert!(
                matches!(
                    evaluate(&kind, &headless),
                    Outcome::Undecidable(UnknownReason::NoSessionStart)
                ),
                "{} decided something from a transcript with no opening record",
                kind.name()
            );
        }
    }
}
