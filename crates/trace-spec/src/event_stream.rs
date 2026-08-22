//! The metaharness event-stream adapter: `metaharness.event/1` lines in, [`TraceIr`] out.
//!
//! A driven run's transcript is no longer the vendor's own `stream-json`. Every `llm` step spawns
//! through `metaharness run <kind>`, and what the driver writes down is the seam's event stream —
//! one JSON object per line, each carrying its own `format` tag, `seq`, `run` and the vendor's
//! timestamp where there was one. This module lifts that stream into the same neutral IR
//! [`crate::adapter`] produces from Claude Code's format, so one `trace-spec/1` document can judge
//! a run whichever harness produced it.
//!
//! # One adapter for every harness metaharness ever drives
//!
//! That is the point of reading the *seam's* stream rather than each vendor's: the seam already
//! normalised the vendor, so a second vendor behind metaharness is a metaharness adapter and not a
//! third reader here. What this module knows is the seam's wire, and nothing about Claude Code,
//! Codex or whatever comes next.
//!
//! # The projection is total, and "no IR event" is a decision
//!
//! metaharness publishes which of its nineteen events project into which `trace-ir/1` family and
//! which project into none — its `CONTROL_PLANE_EVENTS`, mirrored here as
//! [`CONTROL_PLANE_EVENTS`]. Those eight are recognised and produce **no IR event**, and that is
//! deliberately not the same thing as an event this build could not read:
//!
//! * a **control-plane** event is understood — a step boundary, a turn boundary, a decision, a
//!   command acknowledgement, a warning, an expired credential — and the IR has no family for it,
//!   so nothing about it is uncertain;
//! * an **unknown event name** is uncertain, and becomes [`EventKind::Opaque`] exactly as an
//!   unrecognised `stream-json` event does, which makes every expectation whose truth could depend
//!   on it `unk`.
//!
//! Collapsing the two would be expensive in the direction that matters: routing `turn.started`
//! through the opaque path would poison every count in every driven run, and a checker that
//! reports `unk` for everything has stopped checking. Routing an unknown name through the control
//! plane would do the reverse and report a run as clean because it stopped being readable.
//!
//! # The denial record
//!
//! metaharness keeps two denial populations apart on purpose: `session.ended.permission_denials`
//! is the **vendor's** own list, passed through and never added to, and `tool.decided` is the
//! **seam's** per-call decision — the one the driver's `decide_tool` policy answers. The IR has
//! one `permission_denials` field, so the reader joins them by call id rather than summing them:
//! a call the seam denied *and* the vendor listed is one denial, not two. The `permission_denials`
//! function carries the precedence and what makes the field [`None`].
//!
//! A denied call gets **no synthesised result**. metaharness's own argument applies unchanged: a
//! decision is not a result, and inventing a failed result for a call that never ran would be
//! evidence this reader manufactured. Where the vendor tells the model about the refusal it does
//! so with a real `tool.result`, and that is what `tool.failed` reads.
//!
//! # Absent stays absent
//!
//! Invariant 5, and this wire makes it sharper than the vendor's does: metaharness serialises an
//! absent payload field as an explicit `null` rather than omitting the key, so *every* optional
//! field arrives as a present key. `null` is read as absence throughout — that is the one rule the
//! crate's private `json` module exists to hold in one place — and nothing here turns an
//! unrecorded quantity into a zero.
//!
//! What the wire does not carry at all is named where it is read rather than left to be
//! discovered: `run_usage` for the three usage figures, `model_usage` for per-model cost, and
//! `tool_result` for the per-tool result fields. Each becomes `unk` in a verdict, never a pass.
//!
//! # What refuses, and what does not
//!
//! | code | for |
//! |---|---|
//! | `TRACE-ADAPT-001` | bytes that are not UTF-8, a line that is not JSON, or a line whose `format` tag is not this wire's |
//! | `TRACE-ADAPT-002` | a stream with no events at all |
//!
//! The format tag is checked on **every line**, not on the first, because that is what the tag is
//! for: metaharness writes it per line so a truncated capture stays self-describing, and a reader
//! that checked it once would happily read the second half of a file somebody concatenated.
//! Refusals accumulate (invariant 3).
//!
//! # No clock, and no correlation here
//!
//! Timestamps are the vendor's `at`, passed through verbatim; nothing is measured (invariant 9).
//! Correlating a result to its call is [`TraceIr::new`]'s job, as it is for the other adapter, so
//! there is one owner of the pairing.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;
use trace_domain::code::{TraceCode, ValidationErrors};
use trace_domain::digest::digest_of_bytes;
use trace_domain::ir::{
    AdapterRef, AssistantRequest, EventKind, ModelUsage, OpaqueEvent, RateLimitState, Recorded,
    RunOutcome, RunUsage, SessionStart, ToolCall, ToolResult, TraceEvent, TraceIr,
};

use crate::json::{compact, i64_at, mcp_servers_at, names_at, plugins_at, str_at, text_at, u64_at};

/// The format tag every line of this wire carries.
pub const EVENT_STREAM_FORMAT: &str = "metaharness.event/1";

/// This adapter, and the wire version it was written against.
///
/// Named after the *seam* rather than after a vendor, because that is what it reads. A report says
/// which adapter judged a run, so a verdict that changed because the reader changed is visible as
/// such rather than as a change in the agent's behaviour.
pub const METAHARNESS_EVENT_STREAM: AdapterRef = AdapterRef {
    name: "metaharness/event-stream",
    written_against: &[EVENT_STREAM_FORMAT],
};

/// The events that are recognised and project into no `trace-ir/1` family.
///
/// Mirrors metaharness's own `CONTROL_PLANE_EVENTS`, which is published there for exactly this
/// reason: the projection is a **total** function, so "this event becomes no IR event" has to be a
/// decision somebody wrote down rather than an omission a reader can fall into. A name that is in
/// neither this list nor `read_event`'s match is unknown, and unknown goes opaque.
///
/// `tool.decided` is here and still load-bearing: it produces no event of its own and its denials
/// are counted into the terminal record by `permission_denials`.
pub const CONTROL_PLANE_EVENTS: [&str; 8] = [
    "step.entered",
    "step.left",
    "turn.started",
    "turn.ended",
    "tool.decided",
    "command.result",
    "warning",
    "auth.expired",
];

/// Reads a `metaharness.event/1` stream into the IR.
///
/// # Errors
///
/// `TRACE-ADAPT-001` when the bytes are not UTF-8, when a line is not JSON, or when a line carries
/// another wire's format tag — one refusal per bad line, accumulated. `TRACE-ADAPT-002` when the
/// stream holds no events at all. An event name this build does not know is not an error; it is an
/// opaque record in the returned IR.
pub fn read_event_stream(bytes: &[u8]) -> Result<TraceIr, ValidationErrors> {
    let text = match std::str::from_utf8(bytes) {
        Ok(text) => text,
        Err(error) => {
            // A byte sequence that is not text has no lines to go on with, which is why this is
            // the one refusal here that cannot accumulate with others.
            let mut errors = ValidationErrors::new();
            errors.refuse(
                TraceCode::AdapterMalformedTranscript,
                "transcript",
                format!("the event stream's bytes are not UTF-8: {error}"),
            );
            return Err(errors);
        }
    };
    read_text(bytes, text)
}

/// Reads an event stream that is already text.
///
/// The digest is still taken over the bytes of that text, so a run named by a report is the same
/// run whichever entry point read it.
///
/// # Errors
///
/// As [`read_event_stream`], less the not-UTF-8 case that a `&str` cannot be in.
pub fn read_event_stream_str(text: &str) -> Result<TraceIr, ValidationErrors> {
    read_text(text.as_bytes(), text)
}

/// One line of the stream, parsed, with the file line it came from.
struct Line {
    /// The 1-based line of the file, counted over blank lines too.
    number: usize,
    /// The line's JSON.
    value: Value,
}

/// What the seam decided, gathered before any event is built.
///
/// A separate pass because a decision may be recorded after the terminal record — nothing on this
/// wire promises otherwise — and a reader that counted denials as it went would produce a
/// different number for the same run depending on line order.
#[derive(Default)]
struct Seam {
    /// The calls the seam refused, by call id.
    denied: BTreeSet<String>,
    /// Whether the stream carried any decision at all.
    ///
    /// The difference between *nothing was refused* and *this stream does not record decisions*,
    /// which is the difference between `Some(0)` and [`None`].
    decided: bool,
}

/// The read itself: refuse what is not this wire, then lift what is.
fn read_text(bytes: &[u8], text: &str) -> Result<TraceIr, ValidationErrors> {
    let mut errors = ValidationErrors::new();
    let mut lines: Vec<Line> = Vec::new();

    for (offset, raw) in text.lines().enumerate() {
        // 1-based, and counted over blank lines too: a `source_line` a report prints has to be the
        // number `sed -n '<n>p'` takes.
        let number = offset + 1;
        if raw.trim().is_empty() {
            continue;
        }
        let value = match serde_json::from_str::<Value>(raw) {
            Ok(value) => value,
            Err(error) => {
                errors.refuse(
                    TraceCode::AdapterMalformedTranscript,
                    format!("line[{number}]"),
                    format!("line {number} is not JSON: {error}"),
                );
                continue;
            }
        };
        match str_at(&value, "format") {
            Some(tag) if tag == EVENT_STREAM_FORMAT => lines.push(Line { number, value }),
            other => errors.refuse(
                TraceCode::AdapterMalformedTranscript,
                format!("line[{number}]"),
                format!(
                    "line {number} carries the format tag `{}` and this reader reads \
                     `{EVENT_STREAM_FORMAT}`: the tag is on every line so a truncated capture \
                     stays self-describing, and guessing at an untagged line is how two wires \
                     become one silently",
                    other.unwrap_or("<none>")
                ),
            ),
        }
    }

    if !errors.is_empty() {
        return Err(errors);
    }
    if lines.is_empty() {
        // Judging an empty stream would report every expectation `unk` — true, and useless.
        errors.refuse(
            TraceCode::AdapterEmptyTranscript,
            "transcript",
            "the event stream holds no events at all: there is nothing to judge",
        );
        return Err(errors);
    }

    let seam = seam_ledger(&lines);
    let mut events: Vec<TraceEvent> = Vec::new();
    let mut requests: Vec<AssistantRequest> = Vec::new();
    for line in &lines {
        read_event(line, &seam, &mut events, &mut requests);
    }

    Ok(TraceIr::new(
        digest_of_bytes(bytes),
        METAHARNESS_EVENT_STREAM,
        events,
        requests,
    ))
}

/// Every call the seam refused, and whether it decided anything at all.
fn seam_ledger(lines: &[Line]) -> Seam {
    let mut seam = Seam::default();
    for line in lines {
        if str_at(&line.value, "event") != Some("tool.decided") {
            continue;
        }
        seam.decided = true;
        if str_at(&line.value["decision"], "decision") == Some("deny") {
            if let Some(call) = text_at(&line.value, "call_id") {
                seam.denied.insert(call);
            }
        }
    }
    seam
}

/// Normalizes one line into zero or one IR event, plus a request record where it carries one.
///
/// Zero is a legitimate outcome here, unlike in the `stream-json` adapter: there, an envelope that
/// produced nothing had vanished and was recorded opaque, because every `stream-json` event is
/// *supposed* to carry content. Here the events that produce nothing are named in
/// [`CONTROL_PLANE_EVENTS`] and in the `usage` arm, so a line that produces no event has been
/// decided about rather than lost.
fn read_event(
    line: &Line,
    seam: &Seam,
    events: &mut Vec<TraceEvent>,
    requests: &mut Vec<AssistantRequest>,
) {
    let value = &line.value;
    let at = text_at(value, "at");
    let name = str_at(value, "event");
    let mut push = |kind: EventKind| events.push(TraceEvent::new(line.number, at.clone(), kind));

    match name {
        Some("session.started") => push(EventKind::SessionStart(Box::new(session_start(value)))),
        Some("session.ended") => push(EventKind::RunOutcome(Box::new(run_outcome(value, seam)))),
        Some("text") => push(EventKind::AssistantText {
            text: text_at(value, "text").unwrap_or_default(),
            request_id: text_at(value, "request_id"),
        }),
        // The reasoning's `request_id` has no home in the IR and is dropped: `text.matches` reads
        // what the model said to the operator, and no expectation kind reads reasoning at all.
        Some("thinking") => push(EventKind::AssistantThinking {
            text: text_at(value, "text").unwrap_or_default(),
        }),
        Some("thinking.estimate") => push(EventKind::ThinkingEstimate {
            estimated_tokens: u64_at(value, "estimate"),
            estimated_tokens_delta: i64_at(value, "delta"),
        }),
        Some("injection") => push(EventKind::SyntheticInjection {
            text: text_at(value, "text").unwrap_or_default(),
        }),
        // A request with no tool name is a call this build cannot name, and a nameless `ToolCall`
        // would be a call every `tool.called` expectation silently misses.
        Some("tool.requested") => push(tool_call(value).map_or_else(
            || opaque_line(Some("tool.requested"), line),
            |call| EventKind::ToolCall(Box::new(call)),
        )),
        Some("tool.result") => push(EventKind::ToolResult(Box::new(tool_result(value)))),
        Some("rate_limit") => push(EventKind::RateLimit(Box::new(rate_limit(value)))),
        // The vendor said something metaharness could not map. It stays opaque one layer up too,
        // carrying the *vendor's* type and digest rather than this line's, because that is what a
        // reader has to look for in the retained transcript.
        Some("opaque") => push(EventKind::Opaque(Box::new(OpaqueEvent {
            event_type: text_at(value, "vendor_type"),
            subtype: text_at(value, "vendor_subtype"),
            digest: text_at(value, "digest")
                .unwrap_or_else(|| digest_of_bytes(compact(value).as_bytes())),
        }))),
        // One API request's usage. It is a request record and not an event: the IR's `requests`
        // is what `api_requests`, `events.assistant` and the per-request series read, and an event
        // family for it would be a second copy of numbers the terminal record already totals.
        Some("usage") => {
            let usage = value.get("usage");
            requests.push(AssistantRequest {
                source_line: line.number,
                request_id: text_at(value, "request_id"),
                model: text_at(value, "model"),
                input_tokens: usage.and_then(|usage| u64_at(usage, "input_tokens")),
                output_tokens: usage.and_then(|usage| u64_at(usage, "output_tokens")),
                cache_read_input_tokens: usage
                    .and_then(|usage| u64_at(usage, "cache_read_input_tokens")),
                cache_creation_input_tokens: usage
                    .and_then(|usage| u64_at(usage, "cache_creation_input_tokens")),
            });
        }
        Some(known) if CONTROL_PLANE_EVENTS.contains(&known) => {}
        // An event name this build does not know. Unknown, and therefore opaque: it may have been
        // a tool call, and reading it as absent is the lie this whole design exists to prevent.
        other => push(opaque_line(other, line)),
    }
}

/// Reads a `session.started` event: the run's opening record.
///
/// What the event carries and the IR has no home for is dropped rather than kept opaque —
/// `adapter`, `adapter_class`, `session_id`, `inputs_digest`, the retained `transcript` reference
/// and the `hermetic` attestation. That is the same rule the other adapter states for an
/// unrecognised *field*: the IR is the judged form, no expectation kind reads any of them, and the
/// stream itself remains the record for a reader who wants them. An unrecognised *event* is the
/// case that stays opaque, because that one could have been a tool call.
fn session_start(value: &Value) -> SessionStart {
    SessionStart {
        model: text_at(value, "model"),
        permission_mode: text_at(value, "permission_mode"),
        // The seam's `credential_source` is the IR's `api_key_source`: same question — who paid —
        // asked in two vocabularies, and an adapter is where that stops being anybody else's
        // problem.
        api_key_source: text_at(value, "credential_source"),
        harness_version: text_at(value, "harness_version"),
        output_style: text_at(value, "output_style"),
        cwd: text_at(value, "cwd"),
        tools: names_at(value, "offered_tools"),
        slash_commands: names_at(value, "slash_commands"),
        skills: names_at(value, "skills"),
        agents: names_at(value, "agents"),
        plugins: plugins_at(value),
        mcp_servers: mcp_servers_at(value),
    }
}

/// Reads a `session.ended` event: the terminal record, and the source of every resource fact.
fn run_outcome(value: &Value, seam: &Seam) -> RunOutcome {
    RunOutcome {
        is_error: value.get("is_error").and_then(Value::as_bool),
        subtype: text_at(value, "subtype"),
        stop_reason: text_at(value, "stop_reason"),
        terminal_reason: text_at(value, "terminal_reason"),
        // Recorded as `null` in a healthy run, and `null` is absence.
        api_error_status: text_at(value, "api_error_status"),
        num_turns: u64_at(value, "num_turns"),
        duration_ms: u64_at(value, "duration_ms"),
        duration_api_ms: u64_at(value, "duration_api_ms"),
        ttft_ms: u64_at(value, "ttft_ms"),
        time_to_request_ms: u64_at(value, "time_to_request_ms"),
        total_cost_usd: value.get("total_cost_usd").and_then(Value::as_f64),
        permission_denials: permission_denials(value, seam),
        subagents_spawned: u64_at(value, "subagents_spawned"),
        usage: value
            .get("usage")
            .filter(|usage| usage.is_object())
            .map(run_usage),
        model_usage: model_usage(value),
    }
}

/// How many permission requests this run refused, across both populations that can refuse one.
///
/// metaharness records two, and keeps them apart on purpose: `permission_denials` is the
/// **vendor's** own list, passed through untouched, and a `tool.decided` carrying a `deny` is the
/// **seam's** decision — in a driven run, this repository's own `decide_tool` policy answering a
/// call before it runs. They are different populations, and one of them is usually empty: a call
/// the seam refuses may never reach the vendor's permission pipeline at all.
///
/// The IR has one field, so they are joined rather than summed, by call id:
///
/// * every call the seam denied counts once, and `census.denied` raises that count where the seam
///   says it took more decisions than this stream carried — a truncated capture must not read as a
///   quieter run;
/// * a vendor entry whose `tool_use_id` is already a seam denial adds nothing, because one refused
///   call is one denial however many layers wrote it down. That case is real: a hook deny on Claude
///   Code `2.1.238` was observed appearing in the vendor's array one-for-one;
/// * a vendor entry the seam never decided adds one, because the vendor refused something the seam
///   claimed nothing about.
///
/// [`None`] — *this stream does not say* — when the terminal record carries neither census nor
/// vendor list and no decision was recorded anywhere in the stream. A run whose seam decided and
/// refused nothing is `Some(0)`, which is a different answer and the one `permission.denied` needs
/// in order to mean anything.
fn permission_denials(value: &Value, seam: &Seam) -> Option<u64> {
    let census = value
        .get("census")
        .and_then(|census| u64_at(census, "denied"));
    let vendor = value.get("permission_denials").and_then(Value::as_array);
    if census.is_none() && vendor.is_none() && !seam.decided {
        return None;
    }
    let observed = u64::try_from(seam.denied.len()).unwrap_or(u64::MAX);
    let by_seam = observed.max(census.unwrap_or(0));
    let by_vendor = vendor.map_or(0, |entries| {
        entries
            .iter()
            .filter(|entry| match str_at(entry, "tool_use_id") {
                // An entry the seam already counted is the same refusal seen twice.
                Some(call) => !seam.denied.contains(call),
                // An entry naming no call cannot be attributed, and dropping it would be the
                // quieter mistake.
                None => true,
            })
            .count()
            .try_into()
            .unwrap_or(u64::MAX)
    });
    Some(by_seam.saturating_add(by_vendor))
}

/// Reads the run's aggregate usage.
///
/// Three of [`RunUsage`]'s fields have no key on this wire and stay [`None`]: `thinking_tokens`
/// (metaharness carries the vendor's usage figures and not the API's `output_tokens_details`
/// breakdown), `iterations` and `speed`. So `tokens.thinking`, `iterations` and `speed` read `unk`
/// against an event stream — the honest verdict for a quantity the record does not carry, and the
/// thing to fix at the seam rather than here if it is ever wanted.
fn run_usage(usage: &Value) -> RunUsage {
    RunUsage {
        input_tokens: u64_at(usage, "input_tokens"),
        output_tokens: u64_at(usage, "output_tokens"),
        cache_read_input_tokens: u64_at(usage, "cache_read_input_tokens"),
        cache_creation_input_tokens: u64_at(usage, "cache_creation_input_tokens"),
        thinking_tokens: None,
        iterations: None,
        speed: None,
        service_tier: text_at(usage, "service_tier"),
    }
}

/// Reads the terminal record's per-model breakdown, where it has one.
///
/// Snake-cased keys, unlike the `stream-json` adapter's camel-cased ones, and with no per-model
/// cost at all: the wire's per-model record is the same usage shape as the aggregate, so
/// [`ModelUsage::cost_usd`] stays [`None`] and a `cost.total` scoped to a model reads `unk`. The
/// run's own `total_cost_usd` is unaffected.
fn model_usage(value: &Value) -> Option<BTreeMap<String, ModelUsage>> {
    let models = value.get("model_usage")?.as_object()?;
    Some(
        models
            .iter()
            .map(|(model, usage)| {
                (
                    model.clone(),
                    ModelUsage {
                        input_tokens: u64_at(usage, "input_tokens"),
                        output_tokens: u64_at(usage, "output_tokens"),
                        cache_read_input_tokens: u64_at(usage, "cache_read_input_tokens"),
                        cache_creation_input_tokens: u64_at(usage, "cache_creation_input_tokens"),
                        cost_usd: None,
                    },
                )
            })
            .collect(),
    )
}

/// Reads one `tool.requested` event, or [`None`] where it declares no name.
///
/// The call is recorded whatever the seam went on to decide about it: a call the policy refused was
/// still a call the model made, which is what `tool.called` and `tool.absent` ask about. Whether it
/// ran is the result's business.
fn tool_call(value: &Value) -> Option<ToolCall> {
    let name = str_at(value, "name")?.to_owned();
    let input: BTreeMap<String, Recorded> = value
        .get("input")
        .and_then(Value::as_object)
        .map(|object| {
            object
                .iter()
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect()
        })
        .unwrap_or_default();
    // The same measure the other adapter documents: the compact JSON of the stored input object.
    let input_bytes = serde_json::to_string(&input).map_or(0, |json| json.len());
    Some(ToolCall {
        call_id: text_at(value, "call_id"),
        name,
        input,
        input_bytes,
        result_event: None,
    })
}

/// Reads one `tool.result` event.
///
/// Two decisions worth naming, because both are places a reader could quietly invent a fact:
///
/// * **`is_error` absent stays [`None`].** The other adapter maps an absent flag to `Some(false)`,
///   on the observation that Claude Code writes it only where it means something — but that is a
///   fact about *that* vendor's transcript, and this wire is a seam that may be carrying any of
///   them. metaharness states that an absent payload field is the `unk` verdict, so `tool.failed`
///   and `tool.error_rate` report `unk` over results whose flag was not recorded rather than
///   reporting success. Acceptance is exactly this: an absent field stays absent, never zero.
/// * **The per-tool result fields are empty.** Claude Code's `tool_use_result` sibling — where
///   `Skill` records `commandName` and `success`, `Bash` its `stdout` and `interrupted` — is not
///   carried on this wire, so `skill.completed` and any `tool.result` matcher naming such a field
///   read `unk` against an event stream. Where the *content itself* is a JSON object its keys are
///   addressable, which is the one case this reader can honestly offer.
fn tool_result(value: &Value) -> ToolResult {
    let recorded = value.get("content");
    let content = match recorded {
        Some(Value::String(text)) => Some(text.clone()),
        Some(Value::Null) | None => None,
        Some(other) => Some(compact(other)),
    };
    let fields: BTreeMap<String, Recorded> = match recorded {
        Some(Value::Object(object)) => object
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect(),
        _ => BTreeMap::new(),
    };
    ToolResult {
        call_id: text_at(value, "call_id"),
        is_error: value.get("is_error").and_then(Value::as_bool),
        // The rendered content's byte length, so one definition of the measure holds across both
        // adapters. The wire's own `bytes` answers only where there is no content to measure — a
        // harness that recorded the size without the text still said something.
        content_bytes: content.as_ref().map_or_else(
            || usize::try_from(u64_at(value, "bytes").unwrap_or(0)).unwrap_or(usize::MAX),
            String::len,
        ),
        content,
        fields,
    }
}

/// Reads the account's rate-limit state out of `info`.
///
/// An event with no `info` is still a rate-limit event; it yields a state that answers nothing,
/// which is the honest record of a harness that emitted the envelope and no content.
fn rate_limit(value: &Value) -> RateLimitState {
    let info = value.get("info");
    let field = |key: &str| info.and_then(|info| info.get(key));
    RateLimitState {
        status: field("status")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        limit_type: field("window")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        utilization: field("utilization").and_then(Value::as_f64),
        is_using_overage: field("using_overage").and_then(Value::as_bool),
        resets_at: field("resets_at").and_then(Value::as_i64),
    }
}

/// An opaque record for a line this build did not understand: what it called itself, and the
/// digest of the line.
fn opaque_line(event_type: Option<&str>, line: &Line) -> EventKind {
    EventKind::Opaque(Box::new(OpaqueEvent {
        event_type: event_type.map(ToOwned::to_owned),
        subtype: None,
        digest: digest_of_bytes(compact(&line.value).as_bytes()),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One event, framed as the wire frames it and read as a one-line stream.
    fn read(event: &str) -> TraceIr {
        read_event_stream_str(&framed(event)).expect("the fixture is a readable event stream")
    }

    /// A fixture event with the envelope every line carries.
    fn framed(event: &str) -> String {
        let mut value: Value = serde_json::from_str(event).expect("the fixture is JSON");
        let object = value.as_object_mut().expect("an event is an object");
        object.insert("format".to_owned(), Value::from(EVENT_STREAM_FORMAT));
        object.insert("seq".to_owned(), Value::from(1));
        object.insert("run".to_owned(), Value::from("T-1/1"));
        compact(&value)
    }

    /// Several events as one stream.
    fn stream(events: &[&str]) -> TraceIr {
        let text: Vec<String> = events.iter().map(|event| framed(event)).collect();
        read_event_stream_str(&text.join("\n")).expect("the fixture is a readable event stream")
    }

    #[test]
    fn the_opening_record_lifts_every_field_the_ir_has_a_home_for() {
        let ir = read(
            r#"{"event":"session.started","adapter":"claude","adapter_class":"harness",
                "harness_version":"2.1.239","session_id":"s-1","model":"claude-opus-4",
                "permission_mode":"dontAsk","credential_source":"none","output_style":"default",
                "cwd":"/work","offered_tools":["Bash","Write"],"slash_commands":["/help"],
                "skills":["engineering-protocols:planning"],"agents":["a"],
                "plugins":[{"name":"engineering-protocols","version":"0.1.0","source":"inline"}],
                "mcp_servers":[],"inputs_digest":"d","transcript":{"path":"/t","digest":"e","bytes":9},
                "hermetic":{"mode":"strict"}}"#,
        );
        let start = ir.session_start().expect("an opening record");
        assert_eq!(start.model.as_deref(), Some("claude-opus-4"));
        assert_eq!(start.permission_mode.as_deref(), Some("dontAsk"));
        assert_eq!(
            start.api_key_source.as_deref(),
            Some("none"),
            "`credential_source` is the same question the IR calls `api_key_source`"
        );
        assert_eq!(start.harness_version.as_deref(), Some("2.1.239"));
        assert_eq!(start.output_style.as_deref(), Some("default"));
        assert_eq!(start.cwd.as_deref(), Some("/work"));
        assert_eq!(
            start.tools.as_deref(),
            Some(&["Bash".to_owned(), "Write".to_owned()][..]),
            "`offered_tools` is the inventory `env.tool_available` reads"
        );
        assert_eq!(
            start.skills.as_deref(),
            Some(&["engineering-protocols:planning".to_owned()][..])
        );
        let plugins = start.plugins.as_deref().expect("the plugins were listed");
        assert_eq!(plugins[0].name, "engineering-protocols");
        assert_eq!(plugins[0].version.as_deref(), Some("0.1.0"));
        assert_eq!(
            plugins[0].path, None,
            "the wire carries no plugin path, and an invented one would answer a question nobody \
             observed"
        );
        assert_eq!(
            start.mcp_servers.as_deref(),
            Some(&[][..]),
            "an empty list is the harness saying `none`, which is what a hermetic run looks like"
        );
        assert_eq!(ir.opaque_events().len(), 0);
    }

    #[test]
    fn a_field_the_stream_records_as_null_stays_absent_rather_than_becoming_zero() {
        // metaharness writes an absent payload field as an explicit `null`. Every one of these
        // would read as a confident zero under a reader that took the key's presence for an
        // answer, and each is a bound somebody gates on.
        let ir = read(
            r#"{"event":"session.ended","is_error":null,"subtype":null,"stop_reason":null,
                "terminal_reason":null,"api_error_status":null,"num_turns":null,"duration_ms":null,
                "duration_api_ms":null,"ttft_ms":null,"time_to_request_ms":null,
                "total_cost_usd":null,"permission_denials":null,"subagents_spawned":null,
                "usage":null,"model_usage":null}"#,
        );
        let outcome = ir.run_outcome().expect("the terminal record was read");
        assert_eq!(outcome.is_error, None);
        assert_eq!(outcome.num_turns, None);
        assert_eq!(outcome.duration_ms, None);
        assert_eq!(outcome.total_cost_usd, None);
        assert_eq!(outcome.subagents_spawned, None);
        assert!(outcome.usage.is_none());
        assert!(outcome.model_usage.is_none());
        assert_eq!(
            outcome.permission_denials, None,
            "no census, no vendor list and no decision recorded anywhere: this stream does not \
             say, which is not the same as `nothing was refused`"
        );
    }

    #[test]
    fn a_seam_denial_is_the_denial_record_even_when_the_vendor_listed_nothing() {
        // The story's load-bearing case. In a driven run the seam refuses the call before the
        // vendor's own permission pipeline sees it, so the vendor's array can be empty precisely
        // when enforcement worked.
        let ir = stream(&[
            r#"{"event":"tool.requested","call_id":"c-1","name":"Bash","input":{"command":"ls /"},"decision_required":true,"seam":"control_request"}"#,
            r#"{"event":"tool.decided","call_id":"c-1","decision":{"decision":"deny","reason":"outside the driven surface"},"decided_by":"embedder","seam":"control_request","latency_ms":2}"#,
            r#"{"event":"session.ended","is_error":false,"permission_denials":[],"census":{"allowed":3,"denied":1,"replaced":0,"abstained":0,"by_seam":{},"by_decider":{}}}"#,
        ]);
        assert_eq!(
            ir.run_outcome()
                .expect("a terminal record")
                .permission_denials,
            Some(1),
            "the seam's own decision is what `permission.denied` counts here"
        );
        assert_eq!(
            ir.census().events_by_family.get("tool_call"),
            Some(&1),
            "a refused call is still a call the model made"
        );
        assert_eq!(
            ir.opaque_events().len(),
            0,
            "a decision is understood, not unread: routing it through the opaque path would \
             poison every count in every driven run"
        );
    }

    #[test]
    fn one_refused_call_written_down_twice_is_one_denial() {
        // Claude Code at 2.1.238 was observed listing a hook deny in its own array one-for-one.
        // Summing the two populations would report one refusal as two, and a bound of `at_most: 1`
        // would go red for a run that behaved exactly as designed.
        let ir = stream(&[
            r#"{"event":"tool.decided","call_id":"c-1","decision":{"decision":"deny","reason":"no"},"decided_by":"embedder","seam":"hook","latency_ms":1}"#,
            r#"{"event":"session.ended","permission_denials":[{"tool_name":"Bash","tool_use_id":"c-1","tool_input":{}}],"census":{"allowed":0,"denied":1,"replaced":0,"abstained":0,"by_seam":{},"by_decider":{}}}"#,
        ]);
        assert_eq!(
            ir.run_outcome()
                .expect("a terminal record")
                .permission_denials,
            Some(1)
        );
    }

    #[test]
    fn a_denial_the_seam_never_decided_is_counted_beside_the_seams_own() {
        // The other direction: the vendor refused a call the seam claimed nothing about. Dropping
        // it would report a run with two refusals as a run with one.
        let ir = stream(&[
            r#"{"event":"tool.decided","call_id":"c-1","decision":{"decision":"deny","reason":"no"},"decided_by":"embedder","seam":"hook","latency_ms":1}"#,
            r#"{"event":"session.ended","permission_denials":[{"tool_name":"Write","tool_use_id":"c-9","tool_input":{}}],"census":{"allowed":0,"denied":1,"replaced":0,"abstained":0,"by_seam":{},"by_decider":{}}}"#,
        ]);
        assert_eq!(
            ir.run_outcome()
                .expect("a terminal record")
                .permission_denials,
            Some(2)
        );
    }

    #[test]
    fn a_seam_that_decided_and_refused_nothing_says_zero_rather_than_nothing() {
        let ir = stream(&[
            r#"{"event":"tool.decided","call_id":"c-1","decision":{"decision":"allow"},"decided_by":"embedder","seam":"control_request","latency_ms":1}"#,
            r#"{"event":"session.ended","is_error":false,"permission_denials":null}"#,
        ]);
        assert_eq!(
            ir.run_outcome()
                .expect("a terminal record")
                .permission_denials,
            Some(0),
            "`nothing was refused` and `this stream does not record decisions` are different \
             answers, and only one of them lets `permission.denied` mean anything"
        );
    }

    #[test]
    fn a_census_that_counted_more_decisions_than_the_stream_carried_wins() {
        // A capture that lost lines must not read as the quieter run.
        let ir = read(
            r#"{"event":"session.ended","permission_denials":[],"census":{"allowed":1,"denied":4,"replaced":0,"abstained":0,"by_seam":{},"by_decider":{}}}"#,
        );
        assert_eq!(
            ir.run_outcome()
                .expect("a terminal record")
                .permission_denials,
            Some(4)
        );
    }

    #[test]
    fn a_control_plane_event_produces_no_ir_event_and_poisons_no_count() {
        let ir = stream(&[
            r#"{"event":"step.entered","step":{"workflow":"w","state":"s","index":1,"attempt":1},"frame_digest":"d"}"#,
            r#"{"event":"turn.started","turn":1,"frame_digest":"d"}"#,
            r#"{"event":"text","text":"working","request_id":"r-1"}"#,
            r#"{"event":"turn.ended","turn":1,"stop_reason":"end_turn"}"#,
            r#"{"event":"command.result","id":"decide-c-1","outcome":{"outcome":"accepted"}}"#,
            r#"{"event":"warning","code":"VERSION_OFF_PIN","message":"2.1.240 is off the pin"}"#,
            r#"{"event":"auth.expired","credential_source":"none","detail":"session expired","source_line":9}"#,
            r#"{"event":"step.left","step":{"workflow":"w","state":"s","index":1,"attempt":1},"outcome":{"outcome":"completed"}}"#,
        ]);
        assert_eq!(
            ir.events.len(),
            1,
            "eight of the nine lines are control plane: understood, and with no IR family to land \
             in"
        );
        assert_eq!(
            ir.opaque_events().len(),
            0,
            "an event with no family is not an event nobody could read"
        );
        assert_eq!(ir.events[0].source_line, 3, "the line a report names");
    }

    #[test]
    fn an_event_name_this_build_does_not_know_is_kept_opaque_rather_than_dropped() {
        // The wire grows. A twentieth event name must not read as absence: it may have been a tool
        // call, and a checker that dropped it would report "the tool was never called" when what
        // happened is that it stopped being able to see tool calls.
        let ir = read(r#"{"event":"tool.streamed","call_id":"c-1","chunk":"…"}"#);
        assert_eq!(ir.events.len(), 1);
        let (index, opaque) = ir.opaque_events()[0];
        assert_eq!(index, 0);
        assert_eq!(opaque.event_type.as_deref(), Some("tool.streamed"));
        assert_eq!(opaque.digest.len(), 64, "the line's digest, in full");
    }

    #[test]
    fn a_vendor_record_metaharness_could_not_read_stays_opaque_one_layer_up() {
        let ir = read(
            r#"{"event":"opaque","vendor_type":"quantum_entanglement_event","vendor_subtype":"periodic",
                "digest":"695fc789d8ad9d9a41c17c757915c593473d564b97701b62abf43f65dd3aaac8","source_line":4}"#,
        );
        let (_, opaque) = ir.opaque_events()[0];
        assert_eq!(
            opaque.event_type.as_deref(),
            Some("quantum_entanglement_event"),
            "the record names what the *vendor* called it, which is what a reader has to look for"
        );
        assert_eq!(opaque.subtype.as_deref(), Some("periodic"));
        assert_eq!(
            opaque.digest, "695fc789d8ad9d9a41c17c757915c593473d564b97701b62abf43f65dd3aaac8",
            "the vendor record's own digest, so it is citable against the retained transcript"
        );
    }

    #[test]
    fn a_result_that_does_not_say_whether_it_errored_stays_unknown() {
        // The opposite rule from the `stream-json` adapter, and deliberately: that one knows its
        // vendor writes the flag only where it means something. This wire is a seam that may be
        // carrying any vendor, and an absent field is the `unk` verdict it states.
        let ir = read(r#"{"event":"tool.result","call_id":"c-1","content":"ok","bytes":2}"#);
        let result = ir.events[0].tool_result().expect("a result");
        assert_eq!(result.is_error, None, "absence is not success");
        assert_eq!(result.content.as_deref(), Some("ok"));
        assert_eq!(result.content_bytes, 2);
        assert!(
            result.fields.is_empty(),
            "the vendor's per-tool result sibling is not carried on this wire"
        );
    }

    #[test]
    fn a_structured_result_content_is_addressable_field_by_field() {
        let ir = read(
            r#"{"event":"tool.result","call_id":"c-1","is_error":false,"content":{"commandName":"planning","success":true},"bytes":41}"#,
        );
        let result = ir.events[0].tool_result().expect("a result");
        assert_eq!(result.field("success"), Some(&Value::Bool(true)));
        assert_eq!(
            result.content.as_deref(),
            Some(r#"{"commandName":"planning","success":true}"#),
            "an object content is rendered as its compact JSON, as the other adapter renders an \
             array one"
        );
        assert_eq!(result.content_bytes, 41);
    }

    #[test]
    fn a_result_with_no_content_falls_back_to_the_size_the_harness_recorded() {
        let ir = read(
            r#"{"event":"tool.result","call_id":"c-1","is_error":true,"content":null,"bytes":4096}"#,
        );
        let result = ir.events[0].tool_result().expect("a result");
        assert_eq!(result.content, None);
        assert_eq!(
            result.content_bytes, 4096,
            "a harness that recorded the size without the text still said something"
        );
    }

    #[test]
    fn a_usage_event_is_a_request_record_and_not_an_event() {
        let ir = stream(&[
            r#"{"event":"usage","request_id":"r-1","model":"m","usage":{"input_tokens":12,"output_tokens":3,"cache_read_input_tokens":900,"cache_creation_input_tokens":0,"service_tier":"standard"}}"#,
            r#"{"event":"usage","request_id":"r-1","model":"m","usage":{"input_tokens":14,"output_tokens":5,"cache_read_input_tokens":901,"cache_creation_input_tokens":0,"service_tier":"standard"}}"#,
            r#"{"event":"usage","request_id":null,"model":"m","usage":{"input_tokens":1,"output_tokens":1,"cache_read_input_tokens":0,"cache_creation_input_tokens":0,"service_tier":null}}"#,
        ]);
        assert_eq!(ir.events.len(), 0, "usage folds into the request series");
        assert_eq!(ir.assistant_event_count(), 3);
        assert_eq!(
            ir.api_request_count(),
            2,
            "one shared id, one unlabelled — an unlabelled record is its own request rather than \
             folded into another"
        );
        assert_eq!(ir.requests[0].input_tokens, Some(12));
        assert_eq!(ir.requests[2].request_id, None);
    }

    #[test]
    fn the_terminal_record_reads_the_usage_the_wire_carries_and_no_more() {
        let ir = read(
            r#"{"event":"session.ended","is_error":false,"num_turns":7,"duration_ms":4200,
                "duration_api_ms":3900,"ttft_ms":700,"time_to_request_ms":80,"total_cost_usd":0.0123,
                "subagents_spawned":0,
                "usage":{"input_tokens":1200,"output_tokens":48,"cache_read_input_tokens":900,"cache_creation_input_tokens":0,"service_tier":"standard"},
                "model_usage":{"m":{"input_tokens":1200,"output_tokens":48,"cache_read_input_tokens":900,"cache_creation_input_tokens":0,"service_tier":null}},
                "census":{"allowed":2,"denied":0,"replaced":0,"abstained":0,"by_seam":{},"by_decider":{}}}"#,
        );
        let outcome = ir.run_outcome().expect("a terminal record");
        let usage = outcome.usage.as_ref().expect("the aggregate usage");
        assert_eq!(usage.input_tokens, Some(1200));
        assert_eq!(usage.service_tier.as_deref(), Some("standard"));
        assert_eq!(
            (usage.thinking_tokens, usage.iterations, usage.speed.clone()),
            (None, None, None),
            "three quantities this wire does not carry: `unk` in a verdict, never a pass"
        );
        let per_model = outcome.model_usage.as_ref().expect("the per-model record");
        assert_eq!(per_model["m"].input_tokens, Some(1200));
        assert_eq!(
            per_model["m"].cost_usd, None,
            "the wire's per-model record carries no cost, so a cost scoped to a model is `unk`"
        );
        assert_eq!(outcome.subagents_spawned, Some(0));
        assert_eq!(outcome.num_turns, Some(7));
    }

    #[test]
    fn a_rate_limit_window_reads_the_seams_spelling_of_it() {
        let ir = read(
            r#"{"event":"rate_limit","info":{"status":"allowed","window":"seven_day","resets_at":1787000000,"utilization":0.25,"using_overage":false}}"#,
        );
        let (_, state) = ir.rate_limit().expect("a rate-limit state");
        assert_eq!(state.status.as_deref(), Some("allowed"));
        assert_eq!(state.limit_type.as_deref(), Some("seven_day"));
        assert_eq!(state.is_using_overage, Some(false));
        assert_eq!(state.resets_at, Some(1_787_000_000));
    }

    #[test]
    fn a_line_carrying_another_wires_format_tag_is_refused_by_name() {
        // The tag is on every line so a truncated capture stays self-describing; a reader that
        // trusted the first line would read the second half of a concatenated file as this wire.
        let stream = format!(
            "{}\n{}\n",
            framed(r#"{"event":"text","text":"hello"}"#),
            r#"{"format":"metaharness.command/1","id":"decide-c-1","command":"tool.decide"}"#
        );
        let errors =
            read_event_stream_str(&stream).expect_err("a command line is not an event line");
        assert_eq!(errors.count(TraceCode::AdapterMalformedTranscript), 1);
        assert!(
            errors.as_slice()[0]
                .message
                .contains("metaharness.command/1"),
            "the refusal names the tag it found: {}",
            errors.as_slice()[0].message
        );
    }

    #[test]
    fn a_line_that_is_not_json_is_refused_once_and_the_message_names_the_line() {
        let stream = format!(
            "{}\nnot json at all\n",
            framed(r#"{"event":"text","text":"x"}"#)
        );
        let errors = read_event_stream_str(&stream).expect_err("a line that is not JSON");
        assert_eq!(errors.count(TraceCode::AdapterMalformedTranscript), 1);
        assert!(
            errors.as_slice()[0].message.contains("line 2"),
            "a refusal a reader cannot locate is a refusal they cannot act on: {}",
            errors.as_slice()[0].message
        );
    }

    #[test]
    fn four_bad_lines_are_four_refusals_rather_than_the_first_one() {
        // Invariant 3, on a transcript.
        let errors = read_event_stream_str("a\nb\nc\nd\n").expect_err("nothing here is JSON");
        assert_eq!(errors.count(TraceCode::AdapterMalformedTranscript), 4);
    }

    #[test]
    fn a_stream_with_no_events_is_refused_rather_than_judged() {
        for empty in ["", "\n\n   \n\t\n"] {
            let errors =
                read_event_stream_str(empty).expect_err("an empty stream has nothing to judge");
            assert_eq!(
                errors.count(TraceCode::AdapterEmptyTranscript),
                1,
                "{empty:?}"
            );
        }
    }

    #[test]
    fn bytes_that_are_not_utf8_are_refused_as_a_file_that_is_not_a_stream() {
        let errors = read_event_stream(&[0x7b, 0xff, 0xfe, 0x7d])
            .expect_err("a byte sequence that is not text has no lines to read");
        assert_eq!(errors.count(TraceCode::AdapterMalformedTranscript), 1);
        assert_eq!(errors.len(), 1);
    }

    #[test]
    fn a_blank_line_advances_the_line_counter_so_a_reported_line_matches_the_file() {
        let ir = read_event_stream_str(&format!(
            "\n\n{}\n",
            framed(r#"{"event":"text","text":"hello"}"#)
        ))
        .expect("one event on line 3");
        assert_eq!(ir.events[0].source_line, 3);
    }

    #[test]
    fn a_timestamp_is_the_vendors_and_a_line_without_one_derives_no_duration() {
        let ir = stream(&[
            r#"{"event":"tool.requested","at":"2026-08-22T10:00:00.000Z","call_id":"c-1","name":"Bash","input":{"command":"ls"},"decision_required":true,"seam":"hook"}"#,
            r#"{"event":"tool.result","at":"2026-08-22T10:00:02.000Z","call_id":"c-1","is_error":false,"content":"x","bytes":1}"#,
            r#"{"event":"tool.requested","call_id":"c-2","name":"Bash","input":{"command":"pwd"},"decision_required":true,"seam":"hook"}"#,
        ]);
        let steps = ir.steps();
        assert_eq!(steps[0].exec_ms, Some(2_000), "recorded, never measured");
        assert_eq!(
            steps[1].exec_ms, None,
            "a step with no recorded timestamp has no duration rather than a zero one"
        );
    }

    #[test]
    fn the_adapter_names_itself_and_the_wire_it_reads() {
        let ir = read(r#"{"event":"session.ended","is_error":false}"#);
        assert_eq!(ir.adapter.name, "metaharness/event-stream");
        assert_eq!(ir.adapter.written_against, &["metaharness.event/1"]);
        assert_eq!(ir.format, "trace-ir/1");
    }

    #[test]
    fn the_control_plane_list_is_the_one_metaharness_publishes() {
        // The projection is total only while both sides agree about which events land nowhere. If
        // metaharness adds a control-plane event and this list does not follow, the new name falls
        // through to the opaque arm — noisy, and visible, which is the failure this ordering picks.
        assert_eq!(CONTROL_PLANE_EVENTS.len(), 8);
        for name in [
            "step.entered",
            "step.left",
            "turn.started",
            "turn.ended",
            "tool.decided",
            "command.result",
            "warning",
            "auth.expired",
        ] {
            assert!(CONTROL_PLANE_EVENTS.contains(&name), "{name}");
        }
    }
}
