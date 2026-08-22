//! The Claude Code `stream-json` adapter: transcript bytes in, [`TraceIr`] out.
//!
//! One JSONL line is one harness event, and one harness event is *zero or more* IR events — an
//! `assistant` line carrying a text block and two tool calls is three facts, and the IR says so
//! (design § 2.1). [`TraceIr::new`] assigns the indices and correlates each result to its call;
//! this module's whole job is to get a harness's spelling into the neutral vocabulary, or to
//! admit that it could not.
//!
//! # Unknown is not dropped, and it is not false
//!
//! Invariant 5, in the shape a transcript forces (design § 2.9). An event `type` this build does
//! not recognise — or a content block type inside a recognised event — becomes
//! [`EventKind::Opaque`] carrying its declared `type`, its declared `subtype` and the digest of
//! the raw line. It is never discarded and never guessed at, because a checker that dropped it
//! would report *"the tool was never called"* when what happened is that it stopped being able to
//! see tool calls. An event that this adapter recognised the envelope of but could read nothing
//! out of becomes opaque too, for the same reason: an event that produced no IR event at all has
//! vanished, whatever the intention was.
//!
//! # Unknown *fields*, by contrast, are tolerated in silence
//!
//! Every authored document in this workspace deserializes with `deny_unknown_fields`, and that is
//! right for a document somebody wrote against a published schema: a misspelled key there is a
//! mistake the author wants to be told about, immediately, by name.
//!
//! **This is the opposite case, and it takes the opposite rule.** A `stream-json` transcript is
//! not an authored document and its shape is not a stable public schema (design D1) — it is the
//! output of a harness that adds keys between patch releases without telling anybody. A reader
//! that refused a transcript for carrying `fast_mode_state`, `inference_geo` or
//! `context_management` would be a reader that stopped working on the next Tuesday, and it would
//! fail in the worst available way: refusing the whole run because of a key nothing asserts on.
//! So an unrecognised *field* on a recognised event is ignored without comment, while an
//! unrecognised *event* is preserved opaque and makes the expectations that depend on it `unk`.
//! The distinction is exactly where the format is authored and where it is observed.
//!
//! # What refuses, and what does not
//!
//! | code | for |
//! |---|---|
//! | `TRACE-ADAPT-001` | bytes that are not UTF-8, or a line that is not JSON — a file that is not a transcript |
//! | `TRACE-ADAPT-002` | a transcript with no events at all |
//!
//! An unrecognised event is neither of those; it is an opaque record and a successful read.
//! Refusals accumulate (invariant 3): a transcript with four unparseable lines reports four.
//!
//! # The measures, stated rather than implied
//!
//! A byte count whose definition is folklore is a number two people compute differently, so both
//! are written down here and nowhere else:
//!
//! * [`ToolCall::input_bytes`] — the byte length of the **compact JSON of the call's `input`
//!   object**, as the adapter stored it. A call recording no arguments measures `{}`, two bytes.
//!   This is model *output*: the model wrote those bytes and paid output prices for them.
//! * [`ToolResult::content_bytes`] — the byte length of the **result content rendered as text**: a
//!   string content is itself, an array of blocks is its compact JSON, an absent content is zero.
//!   This is injected into the *next* request, where it costs input tokens and then sits in the
//!   context for the rest of the run (design § 2.5).
//!
//! Byte length, not character count — `String::len`. The eval's own `jq` metrics block uses
//! `length`, which counts codepoints, so a transcript containing any non-ASCII character produces
//! a slightly larger number here than the shell pipeline reports. Bytes are the measure that
//! matches what was sent.
//!
//! # Lines, and where a reported line points
//!
//! [`TraceEvent::source_line`] is the 1-based line of the file, counted over every line including
//! the blank ones that are skipped, so `sed -n '<n>p'` prints the line a report names.
//!
//! # No clock, and no correlation here
//!
//! Timestamps are read off the event and passed through verbatim; nothing is measured (invariant
//! 9). Correlating a result to its call is [`TraceIr::new`]'s job and deliberately not this
//! module's, so there is one owner of the pairing rather than one per adapter.

use std::collections::BTreeMap;

use serde_json::Value;
use trace_domain::code::{TraceCode, ValidationErrors};
use trace_domain::digest::digest_of_bytes;
use trace_domain::ir::{
    AdapterRef, AssistantRequest, EventKind, ModelUsage, OpaqueEvent, RateLimitState, Recorded,
    RunOutcome, RunUsage, SessionStart, ToolCall, ToolResult, TraceEvent, TraceIr,
};

use crate::json::{
    compact, count_at, i64_at, mcp_servers_at, names_at, plugins_at, str_at, text_at, u64_at,
};

/// This adapter, and the harness versions it was written against.
///
/// Versioned because the format is not a stable public schema (design D1): a report says which
/// adapter judged a run, so a verdict that changed because the *reader* changed is visible as
/// such rather than as a change in the agent's behaviour.
pub const CLAUDE_CODE_STREAM_JSON: AdapterRef = AdapterRef {
    name: "claude-code/stream-json",
    written_against: &["2.1.238"],
};

/// Reads a Claude Code `stream-json` transcript into the IR.
///
/// # Errors
///
/// `TRACE-ADAPT-001` when the bytes are not UTF-8 or a line is not JSON — one refusal per bad
/// line, accumulated. `TRACE-ADAPT-002` when the transcript holds no events at all. An event this
/// build does not recognise is not an error; it is an opaque record in the returned IR.
pub fn read_transcript(bytes: &[u8]) -> Result<TraceIr, ValidationErrors> {
    let text = match std::str::from_utf8(bytes) {
        Ok(text) => text,
        Err(error) => {
            // Not a transcript at all. There is no partial read to offer: a byte sequence that is
            // not text has no lines to go on with, which is why this is the one refusal here that
            // cannot accumulate with others.
            let mut errors = ValidationErrors::new();
            errors.refuse(
                TraceCode::AdapterMalformedTranscript,
                "transcript",
                format!("the transcript's bytes are not UTF-8: {error}"),
            );
            return Err(errors);
        }
    };
    read_text(bytes, text)
}

/// Reads a transcript that is already text.
///
/// The digest is still taken over the bytes of that text, so a run named by a report is the same
/// run whichever entry point read it.
///
/// # Errors
///
/// As [`read_transcript`], less the not-UTF-8 case that a `&str` cannot be in.
pub fn read_transcript_str(text: &str) -> Result<TraceIr, ValidationErrors> {
    read_text(text.as_bytes(), text)
}

/// The read itself: line by line, refusing nothing it can turn into a record.
fn read_text(bytes: &[u8], text: &str) -> Result<TraceIr, ValidationErrors> {
    let mut errors = ValidationErrors::new();
    let mut events: Vec<TraceEvent> = Vec::new();
    let mut requests: Vec<AssistantRequest> = Vec::new();

    for (offset, line) in text.lines().enumerate() {
        // 1-based, and counted over blank lines too: a `source_line` a report prints has to be
        // the number `sed -n '<n>p'` takes.
        let source_line = offset + 1;
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<Value>(line) {
            Ok(value) => read_event(&value, line, source_line, &mut events, &mut requests),
            Err(error) => errors.refuse(
                TraceCode::AdapterMalformedTranscript,
                format!("line[{source_line}]"),
                format!("line {source_line} is not JSON: {error}"),
            ),
        }
    }

    if !errors.is_empty() {
        return Err(errors);
    }
    if events.is_empty() {
        // Judging an empty transcript would report every expectation `unk` — true, and useless.
        errors.refuse(
            TraceCode::AdapterEmptyTranscript,
            "transcript",
            "the transcript holds no events at all: there is nothing to judge",
        );
        return Err(errors);
    }

    Ok(TraceIr::new(
        digest_of_bytes(bytes),
        CLAUDE_CODE_STREAM_JSON,
        events,
        requests,
    ))
}

/// Normalizes one transcript event into zero or more IR events.
///
/// Zero is not an outcome this leaves standing: an envelope that yielded nothing recognisable is
/// pushed as an opaque record before returning, so no line of the file is unrepresented.
fn read_event(
    value: &Value,
    line: &str,
    source_line: usize,
    events: &mut Vec<TraceEvent>,
    requests: &mut Vec<AssistantRequest>,
) {
    let timestamp = text_at(value, "timestamp");
    let event_type = str_at(value, "type");
    let subtype = str_at(value, "subtype");
    let before = events.len();

    let single = |kind: EventKind| TraceEvent::new(source_line, timestamp.clone(), kind);

    match (event_type, subtype) {
        (Some("system"), Some("init")) => {
            events.push(single(EventKind::SessionStart(Box::new(session_start(
                value,
            )))));
        }
        (Some("system"), Some("thinking_tokens")) => {
            events.push(single(EventKind::ThinkingEstimate {
                estimated_tokens: u64_at(value, "estimated_tokens"),
                estimated_tokens_delta: i64_at(value, "estimated_tokens_delta"),
            }));
        }
        (Some("rate_limit_event"), _) => {
            events.push(single(EventKind::RateLimit(Box::new(rate_limit(value)))));
        }
        (Some("assistant"), _) => {
            read_assistant(
                value,
                line,
                source_line,
                timestamp.as_ref(),
                events,
                requests,
            );
        }
        (Some("user"), _) => read_user(value, line, source_line, timestamp.as_ref(), events),
        (Some("result"), _) => {
            events.push(single(EventKind::RunOutcome(Box::new(run_outcome(value)))));
        }
        _ => events.push(single(opaque(event_type, subtype, line))),
    }

    if events.len() == before {
        // A recognised envelope carrying nothing this build could read — an `assistant` event
        // whose content is a shape we do not know, say. Recording it opaque keeps the census
        // honest; letting it produce nothing would be the silent drop invariant 5 forbids.
        events.push(TraceEvent::new(
            source_line,
            timestamp,
            opaque(event_type, subtype, line),
        ));
    }
}

/// Reads a `system`/`init` event: the run's opening record (design § 2.3).
///
/// Every field is optional and an absent one stays absent. This is where a class of eval defect
/// is visible before the first turn is spent — the model an alias resolved to, the permission
/// mode, and `apiKeySource`, which has already billed the wrong account once.
fn session_start(value: &Value) -> SessionStart {
    SessionStart {
        model: text_at(value, "model"),
        permission_mode: text_at(value, "permissionMode"),
        api_key_source: text_at(value, "apiKeySource"),
        harness_version: text_at(value, "claude_code_version"),
        output_style: text_at(value, "output_style"),
        cwd: text_at(value, "cwd"),
        tools: names_at(value, "tools"),
        slash_commands: names_at(value, "slash_commands"),
        skills: names_at(value, "skills"),
        agents: names_at(value, "agents"),
        plugins: plugins_at(value),
        mcp_servers: mcp_servers_at(value),
    }
}

/// Reads the account's rate-limit state out of `rate_limit_info`.
///
/// An event of this type with no `rate_limit_info` is still a rate-limit event; it yields a state
/// that answers nothing, which is the honest record of a harness that emitted the envelope and no
/// content.
fn rate_limit(value: &Value) -> RateLimitState {
    let info = value.get("rate_limit_info");
    let field = |key: &str| info.and_then(|info| info.get(key));
    RateLimitState {
        status: field("status")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        limit_type: field("rateLimitType")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        utilization: field("utilization").and_then(Value::as_f64),
        is_using_overage: field("isUsingOverage").and_then(Value::as_bool),
        resets_at: field("resetsAt").and_then(Value::as_i64),
    }
}

/// Reads an `assistant` event: one request record, plus one IR event per content block.
///
/// The request record goes in whatever the blocks turn out to be, because it is a fact about the
/// API call and not about what the model said on it — that is what keeps `events.assistant` and
/// `api_requests` two different numbers (design § 2.7).
fn read_assistant(
    value: &Value,
    line: &str,
    source_line: usize,
    timestamp: Option<&String>,
    events: &mut Vec<TraceEvent>,
    requests: &mut Vec<AssistantRequest>,
) {
    let request_id = text_at(value, "request_id");
    let message = value.get("message");
    let usage = message.and_then(|message| message.get("usage"));
    requests.push(AssistantRequest {
        source_line,
        request_id: request_id.clone(),
        model: message.and_then(|message| text_at(message, "model")),
        input_tokens: usage.and_then(|usage| u64_at(usage, "input_tokens")),
        output_tokens: usage.and_then(|usage| u64_at(usage, "output_tokens")),
        cache_read_input_tokens: usage.and_then(|usage| u64_at(usage, "cache_read_input_tokens")),
        cache_creation_input_tokens: usage
            .and_then(|usage| u64_at(usage, "cache_creation_input_tokens")),
    });

    let Some(blocks) = message
        .and_then(|message| message.get("content"))
        .and_then(Value::as_array)
    else {
        return;
    };
    for block in blocks {
        let declared = str_at(block, "type");
        let kind = match declared {
            Some("text") => EventKind::AssistantText {
                text: text_at(block, "text").unwrap_or_default(),
                request_id: request_id.clone(),
            },
            Some("thinking") => EventKind::AssistantThinking {
                text: text_at(block, "thinking").unwrap_or_default(),
            },
            // A `tool_use` with no name is a call this build cannot name, and a nameless
            // `ToolCall` would be a call every `tool.called` expectation silently misses.
            Some("tool_use") => tool_call(block).map_or_else(
                || opaque(declared, str_at(block, "subtype"), line),
                |call| EventKind::ToolCall(Box::new(call)),
            ),
            _ => opaque(declared, str_at(block, "subtype"), line),
        };
        events.push(TraceEvent::new(source_line, timestamp.cloned(), kind));
    }
}

/// Reads one `tool_use` block, or [`None`] where it declares no name.
fn tool_call(block: &Value) -> Option<ToolCall> {
    let name = str_at(block, "name")?.to_owned();
    let input: BTreeMap<String, Recorded> = block
        .get("input")
        .and_then(Value::as_object)
        .map(|object| {
            object
                .iter()
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect()
        })
        .unwrap_or_default();
    // The module doc's definition: the compact JSON of the stored input object.
    let input_bytes = serde_json::to_string(&input).map_or(0, |json| json.len());
    Some(ToolCall {
        call_id: text_at(block, "id"),
        name,
        input,
        input_bytes,
        result_event: None,
    })
}

/// Reads a `user` event: tool results, or the harness injecting text of its own.
///
/// A `user` event in a headless run is the harness speaking, not a person — either handing back
/// what a tool produced, or loading a skill's own content into the conversation (design § 2.8).
fn read_user(
    value: &Value,
    line: &str,
    source_line: usize,
    timestamp: Option<&String>,
    events: &mut Vec<TraceEvent>,
) {
    let synthetic = value
        .get("isSynthetic")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    // The per-tool fields sit on the *user event*, beside the content, not inside the block.
    let fields = result_fields(value);
    let content = value
        .get("message")
        .and_then(|message| message.get("content"));

    match content {
        Some(Value::String(text)) if synthetic => events.push(TraceEvent::new(
            source_line,
            timestamp.cloned(),
            EventKind::SyntheticInjection { text: text.clone() },
        )),
        Some(Value::Array(blocks)) => {
            for block in blocks {
                let declared = str_at(block, "type");
                let kind = match declared {
                    Some("tool_result") => {
                        EventKind::ToolResult(Box::new(tool_result(block, &fields)))
                    }
                    Some("text") if synthetic => EventKind::SyntheticInjection {
                        text: text_at(block, "text").unwrap_or_default(),
                    },
                    _ => opaque(declared, str_at(block, "subtype"), line),
                };
                events.push(TraceEvent::new(source_line, timestamp.cloned(), kind));
            }
        }
        // Neither shape. The caller's "produced nothing" rule records it opaque.
        _ => {}
    }
}

/// Reads one `tool_result` block, with the per-tool fields from its enclosing event.
///
/// `is_error` encodes **this harness's convention**, which is why it is decided here and not in
/// the model: Claude Code writes the flag only where it means something. Four of the eleven
/// results in the committed `7hTYjT` fixture carry `is_error: false` explicitly and seven omit the
/// key, and all eleven succeeded — so an absent key is `Some(false)`, not [`None`]. [`None`] in the
/// IR means *no adapter could tell*, and here the adapter can tell. Mapping absence to [`None`]
/// would make `tool.failed` and `tool.error_rate` report `unk` on every healthy run, which is a
/// checker that has stopped checking. Harness-specific knowledge belongs in the adapter — that is
/// what the seam in design § 2.1 is for.
fn tool_result(block: &Value, fields: &BTreeMap<String, Recorded>) -> ToolResult {
    let content = match block.get("content") {
        Some(Value::String(text)) => Some(text.clone()),
        Some(Value::Null) | None => None,
        Some(other) => Some(compact(other)),
    };
    ToolResult {
        call_id: text_at(block, "tool_use_id"),
        is_error: Some(
            block
                .get("is_error")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        ),
        content_bytes: content.as_ref().map_or(0, String::len),
        content,
        fields: fields.clone(),
    }
}

/// Flattens the `tool_use_result` sibling of a `user` event into the result's field map.
///
/// Open by construction: `Skill` records `commandName` and `success`, `Bash` records `stdout`,
/// `stderr` and `interrupted`, `Edit` records `filePath` and `userModified`, and a tool this
/// adapter has never heard of records whatever it records (design § 2.4). A `tool_use_result` that
/// is not an object is kept under its own key rather than dropped, because a matcher that finds
/// nothing must mean *the harness recorded nothing*.
fn result_fields(value: &Value) -> BTreeMap<String, Recorded> {
    match value.get("tool_use_result") {
        Some(Value::Object(object)) => object
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect(),
        Some(Value::Null) | None => BTreeMap::new(),
        Some(other) => {
            let mut fields = BTreeMap::new();
            fields.insert("tool_use_result".to_owned(), other.clone());
            fields
        }
    }
}

/// Reads a `result` event: the terminal record, and the source of every resource fact.
fn run_outcome(value: &Value) -> RunOutcome {
    RunOutcome {
        is_error: value.get("is_error").and_then(Value::as_bool),
        subtype: text_at(value, "subtype"),
        stop_reason: text_at(value, "stop_reason"),
        terminal_reason: text_at(value, "terminal_reason"),
        // Recorded as `null` in a healthy run, and `null` is absence: `text_at` yields `None`.
        api_error_status: text_at(value, "api_error_status"),
        num_turns: u64_at(value, "num_turns"),
        duration_ms: u64_at(value, "duration_ms"),
        duration_api_ms: u64_at(value, "duration_api_ms"),
        ttft_ms: u64_at(value, "ttft_ms"),
        time_to_request_ms: u64_at(value, "time_to_request_ms"),
        total_cost_usd: value.get("total_cost_usd").and_then(Value::as_f64),
        // A *length*, and only where the list is there. An absent list is `None` — the harness did
        // not say — while an empty list is `Some(0)`, which is the harness saying "none". Absence
        // is not zero, and the two must stay distinguishable all the way to the verdict.
        permission_denials: count_at(value, "permission_denials"),
        subagents_spawned: value
            .get("subagent_stats")
            .and_then(|stats| u64_at(stats, "spawned")),
        usage: value
            .get("usage")
            .filter(|usage| usage.is_object())
            .map(run_usage),
        model_usage: model_usage(value),
    }
}

/// Reads the run's aggregate usage.
fn run_usage(usage: &Value) -> RunUsage {
    RunUsage {
        input_tokens: u64_at(usage, "input_tokens"),
        output_tokens: u64_at(usage, "output_tokens"),
        cache_read_input_tokens: u64_at(usage, "cache_read_input_tokens"),
        cache_creation_input_tokens: u64_at(usage, "cache_creation_input_tokens"),
        // The **billed** figure, from the API's own breakdown — never the mid-stream
        // `thinking_tokens` estimate, which is a different quantity wearing a similar name.
        thinking_tokens: usage
            .get("output_tokens_details")
            .and_then(|details| u64_at(details, "thinking_tokens")),
        // An array's length, not a counter (design § 3.5).
        iterations: usage
            .get("iterations")
            .and_then(Value::as_array)
            .map(Vec::len),
        speed: text_at(usage, "speed"),
        service_tier: text_at(usage, "service_tier"),
    }
}

/// Reads the terminal record's per-model breakdown, where it has one.
///
/// Camel-cased keys, unlike the aggregate's — the harness spells the same quantities two ways in
/// one event, and an adapter is where that stops being anybody else's problem.
fn model_usage(value: &Value) -> Option<BTreeMap<String, ModelUsage>> {
    let models = value.get("modelUsage")?.as_object()?;
    Some(
        models
            .iter()
            .map(|(model, usage)| {
                (
                    model.clone(),
                    ModelUsage {
                        input_tokens: u64_at(usage, "inputTokens"),
                        output_tokens: u64_at(usage, "outputTokens"),
                        cache_read_input_tokens: u64_at(usage, "cacheReadInputTokens"),
                        cache_creation_input_tokens: u64_at(usage, "cacheCreationInputTokens"),
                        cost_usd: usage.get("costUSD").and_then(Value::as_f64),
                    },
                )
            })
            .collect(),
    )
}

/// An opaque record: what it called itself, and the digest of the line it was on.
///
/// Used for an unrecognised top-level event and for an unrecognised content block alike. A block's
/// record names the *block's* type — that is the thing this build did not recognise — and its
/// [`TraceEvent::source_line`] still points at the enclosing event's line, which is how a reader
/// finds it in the file.
fn opaque(event_type: Option<&str>, subtype: Option<&str>, line: &str) -> EventKind {
    EventKind::Opaque(Box::new(OpaqueEvent {
        event_type: event_type.map(ToOwned::to_owned),
        subtype: subtype.map(ToOwned::to_owned),
        digest: digest_of_bytes(line.as_bytes()),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One transcript event, written readably here and fed as the single line a transcript holds.
    ///
    /// Compacted through [`Value`] rather than written on one line, so a fixture stays legible
    /// without the test accidentally asserting about JSONL line splitting — which
    /// [`a_blank_line_advances_the_line_counter_so_a_reported_line_matches_the_file`] does on
    /// purpose, on raw bytes.
    fn read(event: &str) -> TraceIr {
        let value: Value = serde_json::from_str(event).expect("the fixture is JSON");
        read_transcript_str(&compact(&value)).expect("the fixture is a readable transcript")
    }

    #[test]
    fn an_event_type_this_build_does_not_recognise_is_kept_opaque_rather_than_dropped() {
        let ir = read(r#"{"type":"telemetry_flush","subtype":"periodic","count":3}"#);
        assert_eq!(
            ir.events.len(),
            1,
            "the unrecognised event is still an event: dropping it is how a checker starts \
             reporting that a tool was never called"
        );
        let (index, opaque) = ir.opaque_events()[0];
        assert_eq!(index, 0);
        assert_eq!(opaque.event_type.as_deref(), Some("telemetry_flush"));
        assert_eq!(opaque.subtype.as_deref(), Some("periodic"));
        assert_eq!(opaque.digest.len(), 64, "the raw line's digest, in full");
        assert!(opaque.digest.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn an_unrecognised_content_block_goes_opaque_while_its_siblings_still_normalize() {
        let ir = read(
            r#"{"type":"assistant","request_id":"req_1","message":{"model":"m","content":[
                 {"type":"text","text":"before"},
                 {"type":"redacted_thinking","data":"…"},
                 {"type":"tool_use","id":"toolu_1","name":"Bash","input":{"command":"ls"}}
               ]}}"#,
        );
        assert_eq!(ir.events.len(), 3, "three blocks, three IR events");
        assert_eq!(
            ir.events[0].kind.family(),
            "assistant_text",
            "a sibling of an unreadable block is still read"
        );
        assert_eq!(ir.events[1].kind.family(), "opaque");
        assert_eq!(
            ir.opaque_events()[0].1.event_type.as_deref(),
            Some("redacted_thinking"),
            "the record names the block type, which is what was not recognised"
        );
        assert_eq!(ir.events[2].tool_call().expect("a call").name, "Bash");
    }

    #[test]
    fn an_unknown_field_on_a_recognised_event_is_tolerated_and_the_event_still_normalizes() {
        // The deliberate opposite of the `deny_unknown_fields` rule every *authored* document in
        // this workspace follows. A transcript is observed, not authored: its shape is not a
        // stable public schema, and a reader that refused a run for a key added in the next patch
        // release would refuse the whole run over a field nothing asserts on.
        let ir = read(
            r#"{"type":"result","subtype":"success","is_error":false,"num_turns":4,
                "fast_mode_state":"off","inference_geo":"not_available","a_key_from_2027":{"x":1}}"#,
        );
        let outcome = ir.run_outcome().expect("the terminal record was read");
        assert_eq!(outcome.is_error, Some(false));
        assert_eq!(outcome.num_turns, Some(4));
        assert_eq!(
            ir.opaque_events().len(),
            0,
            "three unknown fields, and not one of them makes the event unreadable"
        );
    }

    #[test]
    fn a_tool_result_with_no_is_error_key_is_not_an_error() {
        // This harness records the flag only where it is meaningful: seven of the eleven results
        // in the committed `7hTYjT` fixture omit it and all eleven succeeded. `None` would mean
        // "no adapter could tell", and would turn `tool.failed` into `unk` on every healthy run.
        let ir = read(
            r#"{"type":"user","message":{"content":[
                 {"type":"tool_result","tool_use_id":"toolu_1","content":"ok"}
               ]},"tool_use_result":{"stdout":"ok","interrupted":false}}"#,
        );
        let result = ir.events[0].tool_result().expect("a result");
        assert_eq!(result.is_error, Some(false), "absent is not unknown here");
        assert_eq!(result.content.as_deref(), Some("ok"));
        assert_eq!(result.content_bytes, 2);
        assert_eq!(
            result.field("interrupted"),
            Some(&Value::Bool(false)),
            "the per-tool fields come off the user event, beside the block"
        );
    }

    #[test]
    fn a_line_that_is_not_json_is_refused_once_and_the_message_names_the_line() {
        let errors =
            read_transcript(b"{\"type\":\"system\",\"subtype\":\"init\"}\nnot json at all\n")
                .expect_err("a line that is not JSON is a file that is not a transcript");
        assert_eq!(errors.count(TraceCode::AdapterMalformedTranscript), 1);
        assert_eq!(errors.len(), 1);
        assert!(
            errors.as_slice()[0].message.contains("line 2"),
            "a refusal a reader cannot locate is a refusal they cannot act on: {}",
            errors.as_slice()[0].message
        );
    }

    #[test]
    fn bytes_that_are_not_utf8_are_refused_as_a_file_that_is_not_a_transcript() {
        let errors = read_transcript(&[0x7b, 0xff, 0xfe, 0x7d])
            .expect_err("a byte sequence that is not text has no lines to read");
        assert_eq!(errors.count(TraceCode::AdapterMalformedTranscript), 1);
        assert_eq!(errors.len(), 1);
    }

    #[test]
    fn a_transcript_with_no_events_is_refused_rather_than_judged() {
        for empty in ["", "\n\n   \n\t\n"] {
            let errors = read_transcript(empty.as_bytes())
                .expect_err("an empty transcript has nothing to judge");
            assert_eq!(
                errors.count(TraceCode::AdapterEmptyTranscript),
                1,
                "{empty:?}"
            );
            assert_eq!(errors.len(), 1, "{empty:?}");
        }
    }

    #[test]
    fn an_empty_permission_denial_list_is_zero_and_an_absent_one_is_unknown() {
        // The distinction the whole `Option` discipline exists for: "it says none" and "it does
        // not say" are different answers, and collapsing them is how a checker passes an
        // expectation it could not evaluate.
        let stated = read(r#"{"type":"result","subtype":"success","permission_denials":[]}"#);
        assert_eq!(
            stated.run_outcome().expect("an outcome").permission_denials,
            Some(0)
        );

        let silent = read(r#"{"type":"result","subtype":"success"}"#);
        assert_eq!(
            silent.run_outcome().expect("an outcome").permission_denials,
            None,
            "absence is not zero"
        );

        let two = read(
            r#"{"type":"result","subtype":"success","permission_denials":[{"tool":"Bash"},{"tool":"Write"}]}"#,
        );
        assert_eq!(
            two.run_outcome().expect("an outcome").permission_denials,
            Some(2),
            "the list's length, not a counter the harness maintains"
        );
    }

    #[test]
    fn a_null_api_error_status_reads_as_absent_rather_than_as_the_word_null() {
        let ir = read(r#"{"type":"result","subtype":"success","api_error_status":null}"#);
        assert_eq!(
            ir.run_outcome().expect("an outcome").api_error_status,
            None,
            "a healthy run records `null` here, and `null` is not a status"
        );
    }

    #[test]
    fn a_recognised_envelope_that_carries_nothing_readable_is_still_recorded() {
        let ir = read(r#"{"type":"user","message":{"content":[]}}"#);
        assert_eq!(ir.events.len(), 1, "the line did not vanish");
        assert_eq!(ir.events[0].kind.family(), "opaque");
        assert_eq!(ir.opaque_events()[0].1.event_type.as_deref(), Some("user"));
    }

    #[test]
    fn a_blank_line_advances_the_line_counter_so_a_reported_line_matches_the_file() {
        let ir = read_transcript_str(
            "\n\n{\"type\":\"system\",\"subtype\":\"thinking_tokens\",\"estimated_tokens\":50}\n",
        )
        .expect("one event on line 3");
        assert_eq!(ir.events.len(), 1);
        assert_eq!(
            ir.events[0].source_line, 3,
            "`sed -n '3p'` has to print the line the report names"
        );
    }

    #[test]
    fn the_byte_measures_are_the_compact_json_of_the_input_and_the_text_of_the_result() {
        let ir = read(
            r#"{"type":"assistant","message":{"content":[
                 {"type":"tool_use","id":"toolu_1","name":"Read","input":{"file_path":"/x"}}
               ]}}"#,
        );
        let call = ir.events[0].tool_call().expect("a call");
        assert_eq!(
            call.input_bytes,
            r#"{"file_path":"/x"}"#.len(),
            "the compact JSON of the input object, and nothing around it"
        );

        let blocks = read(
            r#"{"type":"user","message":{"content":[
                 {"type":"tool_result","tool_use_id":"toolu_1","content":[{"type":"text","text":"hi"}]}
               ]}}"#,
        );
        let result = blocks.events[0].tool_result().expect("a result");
        assert_eq!(
            result.content.as_deref(),
            Some(r#"[{"text":"hi","type":"text"}]"#),
            "an array content is rendered as its compact JSON"
        );
        assert_eq!(result.content_bytes, 29);
    }

    #[test]
    fn a_name_list_reads_bare_strings_and_objects_alike() {
        // The observed harness writes strings; the next one may write objects. Reading both is
        // cheaper than a refusal, and neither shape may shorten the list.
        let ir = read(
            r#"{"type":"system","subtype":"init","skills":["a",{"name":"b"},{"unnamed":true}]}"#,
        );
        let start = ir.session_start().expect("an opening record");
        assert_eq!(
            start.skills.as_deref(),
            Some(
                &[
                    "a".to_owned(),
                    "b".to_owned(),
                    r#"{"unnamed":true}"#.to_owned()
                ][..]
            ),
            "an entry this build cannot name keeps its JSON rather than disappearing"
        );
    }

    #[test]
    fn a_thinking_estimate_keeps_a_negative_delta_rather_than_refusing_it() {
        let ir = read(
            r#"{"type":"system","subtype":"thinking_tokens","estimated_tokens":80,"estimated_tokens_delta":-20}"#,
        );
        assert_eq!(
            ir.events[0].kind,
            EventKind::ThinkingEstimate {
                estimated_tokens: Some(80),
                estimated_tokens_delta: Some(-20),
            }
        );
    }

    #[test]
    fn the_adapter_names_itself_and_the_harness_it_was_written_against() {
        let ir = read(r#"{"type":"result","subtype":"success"}"#);
        assert_eq!(ir.adapter.name, "claude-code/stream-json");
        assert_eq!(ir.adapter.written_against, &["2.1.238"]);
        assert_eq!(ir.format, "trace-ir/1");
    }
}
