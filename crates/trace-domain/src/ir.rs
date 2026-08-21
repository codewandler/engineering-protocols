//! `trace-ir/1` — one agent run, normalized and content-addressed.
//!
//! # Harness-neutral, on purpose
//!
//! An **adapter** per harness format reads a raw transcript and produces this. Nothing here
//! mentions `stream-json`, `tool_use_result` or any other spelling a particular runner happens to
//! use: the expectation kinds in [`crate::spec`] are phrased against these types, so a second
//! harness is a second adapter and not a second specification language. That is the same seam
//! `ess-synth` draws between a plan and its emitters, for the same reason.
//!
//! # Unknown is not dropped, and it is not false
//!
//! Invariant 5, in the shape a transcript forces. An event the adapter does not recognise is
//! retained as [`EventKind::Opaque`] — its index, its `type`/`subtype` if it had one, and the
//! digest of its raw bytes — and is never discarded. Dropping it would produce the failure mode
//! this whole family exists to prevent: a checker reporting *"the tool was never called"* when
//! what happened is that it stopped being able to see tool calls.
//!
//! Every field a harness might not record is an [`Option`], down to the leaves, for the same
//! reason: absence has to stay distinguishable from zero all the way to the verdict, because
//! "this transcript does not say" and "it says none" are different answers.
//!
//! # Indices, and why they are not line numbers
//!
//! A verdict cites the indices of the events that produced it, which is what makes a report
//! checkable by a human against the transcript it names. One *line* of a `stream-json` transcript
//! can carry several IR events — an `assistant` event with a text block and a tool call is two
//! facts — so [`TraceEvent::index`] is the event's position in this IR and **not** its line
//! number. [`TraceEvent::source_line`] carries the 1-based line beside it, so the mapping back to
//! the file is in the record rather than in the reader's head.
//!
//! # No clock, anywhere
//!
//! Invariant 9. Every duration here is *derived from timestamps the harness recorded*, never
//! measured: [`TraceIr::steps`] subtracts two recorded times and yields [`None`] where either is
//! absent. The same transcript therefore yields the same numbers on any machine at any load,
//! which is what lets a report be committed, diffed and used as evidence.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;
use serde_json::Value;

/// The format string a persisted event IR carries.
pub const IR_FORMAT: &str = "trace-ir/1";

/// A JSON value exactly as the harness recorded it.
///
/// Tool inputs and tool results have a different shape for every tool (design § 2.4), and a type
/// per tool would be a type per harness version. The adapter carries the recorded value through
/// and a matcher names one field of it, which is the whole of the matcher language (design D2).
pub type Recorded = Value;

/// One normalized event.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TraceEvent {
    /// Its position in this IR. What a verdict cites.
    pub index: usize,
    /// The 1-based line of the source transcript it came from.
    ///
    /// Several events may share one line; see the module documentation.
    pub source_line: usize,
    /// The timestamp the harness recorded, verbatim, or [`None`] where it recorded none.
    ///
    /// The first events of an observed run carry none at all — an `init`, a rate-limit event and
    /// the first thinking estimates — which is exactly why this is an `Option` and why a duration
    /// derived across them is `unk` rather than zero.
    pub timestamp: Option<String>,
    /// The recorded timestamp as milliseconds since the Unix epoch, where it parsed.
    ///
    /// Derived by [`parse_timestamp_ms`] from [`Self::timestamp`] and nothing else. A timestamp
    /// this build cannot parse leaves this [`None`], which makes every duration touching it
    /// undecidable rather than wrong.
    pub timestamp_ms: Option<i64>,
    /// What the event says.
    pub kind: EventKind,
}

impl TraceEvent {
    /// Builds one, unindexed. [`TraceIr::new`] assigns [`Self::index`].
    ///
    /// The index is assigned centrally so there is exactly one place that decides what a verdict
    /// cites; an adapter that numbered its own events would be a second such place.
    pub fn new(source_line: usize, timestamp: Option<String>, kind: EventKind) -> Self {
        let timestamp_ms = timestamp.as_deref().and_then(parse_timestamp_ms);
        Self {
            index: 0,
            source_line,
            timestamp,
            timestamp_ms,
            kind,
        }
    }

    /// The tool call this event carries, if it is one.
    pub fn tool_call(&self) -> Option<&ToolCall> {
        match &self.kind {
            EventKind::ToolCall(call) => Some(call),
            _ => None,
        }
    }

    /// The tool result this event carries, if it is one.
    pub fn tool_result(&self) -> Option<&ToolResult> {
        match &self.kind {
            EventKind::ToolResult(result) => Some(result),
            _ => None,
        }
    }
}

/// The event families of `trace-ir/1`.
///
/// Seven recognised families and one opaque one — the census of a real run (design § 2.2), with
/// nothing discarded. Every variant is the *neutral* form: the adapter's job is to get a harness's
/// spelling into one of these or to admit it could not.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum EventKind {
    /// The run's opening record: which model, which permissions, which plugins, which auth.
    ///
    /// A whole observation family of its own (design § 2.3), and the one where a class of eval
    /// defects is visible before the first turn is spent.
    SessionStart(Box<SessionStart>),
    /// One text block the model produced.
    AssistantText {
        /// The text, verbatim.
        text: String,
        /// The API request it arrived on, where the harness records one.
        request_id: Option<String>,
    },
    /// One block of the model's own reasoning.
    ///
    /// A recognised family and not an opaque record, which matters: an opaque event makes every
    /// tool expectation `unk`, and a run with extended thinking on would otherwise be
    /// unjudgeable. It is kept separate from [`AssistantText`](Self::AssistantText) because
    /// `text.matches` reads what the model *said to the operator*, and reasoning is not that.
    AssistantThinking {
        /// The reasoning, verbatim.
        text: String,
    },
    /// One tool call the model made.
    ToolCall(Box<ToolCall>),
    /// One tool result that came back.
    ToolResult(Box<ToolResult>),
    /// Text injected into the conversation by the harness rather than written by a person.
    ///
    /// The observed case is a skill's own content being loaded into context (design § 2.8). It is
    /// recorded and given no expectation kind: a matcher over "a synthetic event containing the
    /// skill's text" would be a wording assertion wearing a structural costume.
    SyntheticInjection {
        /// The injected text, verbatim.
        text: String,
    },
    /// The harness's live estimate of thinking tokens at a point in the stream.
    ///
    /// Never the billed figure. [`RunUsage::thinking_tokens`] is what the API reported, and the
    /// two must not be conflated: one is an estimate emitted mid-stream, the other is an invoice.
    ThinkingEstimate {
        /// The running estimate at this point.
        estimated_tokens: Option<u64>,
        /// How much it moved since the last such event.
        estimated_tokens_delta: Option<i64>,
    },
    /// The account's rate-limit state at the moment the run started.
    RateLimit(Box<RateLimitState>),
    /// The terminal record: how the run ended and what it cost.
    RunOutcome(Box<RunOutcome>),
    /// An event this build does not recognise, kept rather than dropped.
    Opaque(Box<OpaqueEvent>),
}

impl EventKind {
    /// The family's name, as a report and a census print it.
    pub fn family(&self) -> &'static str {
        match self {
            Self::SessionStart(_) => "session_start",
            Self::AssistantText { .. } => "assistant_text",
            Self::AssistantThinking { .. } => "assistant_thinking",
            Self::ToolCall(_) => "tool_call",
            Self::ToolResult(_) => "tool_result",
            Self::SyntheticInjection { .. } => "synthetic_injection",
            Self::ThinkingEstimate { .. } => "thinking_estimate",
            Self::RateLimit(_) => "rate_limit",
            Self::RunOutcome(_) => "run_outcome",
            Self::Opaque(_) => "opaque",
        }
    }
}

/// An MCP server the session was given.
///
/// Recorded with its status and not only its name, because the two answer different questions and
/// the interesting case makes them disagree: a server the session cannot authenticate to still
/// **exists**, is still named in the opening record, and is still a reach outside the sandbox the
/// run was supposed to be. A count of names is what `env.mcp_servers` bounds; the status is what
/// tells the reader why nobody noticed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct McpServer {
    /// Its name, as the harness lists it.
    pub name: String,
    /// What the harness said about connecting to it — `connected`, `needs-auth`, `failed`.
    pub status: Option<String>,
}

/// A plugin the harness loaded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LoadedPlugin {
    /// Its name, which is what `env.exclusive` compares.
    pub name: String,
    /// Its version, where the harness records one.
    pub version: Option<String>,
    /// Where it came from — a marketplace entry, or an inline directory.
    pub source: Option<String>,
    /// The path it was loaded from.
    pub path: Option<String>,
}

/// The run's opening record.
///
/// Every field is optional because every field is a field of a format that is not a stable public
/// schema (design D1): an absent one means *this transcript cannot answer that question*, which
/// is the third verdict's entire job.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct SessionStart {
    /// The **resolved** model — what the alias on the command line turned into.
    pub model: Option<String>,
    /// The permission mode the run was started under.
    pub permission_mode: Option<String>,
    /// Where the credential came from. `none` means the logged-in session paid.
    pub api_key_source: Option<String>,
    /// The harness's own version — the thing you want in the record when the next one turns a
    /// green run red.
    pub harness_version: Option<String>,
    /// The output style in force, which leaks from a user's configuration when isolation breaks.
    pub output_style: Option<String>,
    /// The working directory the run started in.
    pub cwd: Option<String>,
    /// The tools offered, by name.
    pub tools: Option<Vec<String>>,
    /// The slash commands offered, by name.
    pub slash_commands: Option<Vec<String>>,
    /// The skills offered, by name. Available is not invoked.
    pub skills: Option<Vec<String>>,
    /// The agents offered, by name.
    pub agents: Option<Vec<String>>,
    /// The plugins loaded.
    pub plugins: Option<Vec<LoadedPlugin>>,
    /// The MCP servers the session was given.
    ///
    /// [`None`] and `Some(vec![])` are different facts and the distinction is the whole point:
    /// an empty list is a harness that told us it gave the session no server, and an absent field
    /// is a harness that did not say. Only the first is evidence of a hermetic run.
    pub mcp_servers: Option<Vec<McpServer>>,
}

/// One tool call the model made.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ToolCall {
    /// The harness's own id for the call, which is how a result is correlated to it.
    pub call_id: Option<String>,
    /// The tool's name, such as `Bash` or `Skill`.
    pub name: String,
    /// The call's arguments, field by field, exactly as recorded.
    pub input: BTreeMap<String, Recorded>,
    /// How many bytes the arguments took — model *output*, spent at output prices.
    pub input_bytes: usize,
    /// The IR index of the correlated result, where one came back.
    ///
    /// Filled by [`TraceIr::new`] from [`Self::call_id`], so correlation has one owner and is not
    /// a thing each adapter re-derives. [`None`] means no result was correlated — a truncated
    /// transcript, which is not the same as a bad result.
    pub result_event: Option<usize>,
}

impl ToolCall {
    /// The value of one named argument, where the call carries it.
    pub fn argument(&self, field: &str) -> Option<&Recorded> {
        self.input.get(field)
    }

    /// The call's identity for repetition counting: its name and its canonical arguments.
    ///
    /// Byte-identical `(tool, input)` pairs are one group. Two identical `Read`s of one file is a
    /// model that lost track; three identical `Bash` invocations is a retry loop.
    pub fn repetition_key(&self) -> String {
        let arguments = serde_json::to_string(&self.input).unwrap_or_default();
        format!("{} {arguments}", self.name)
    }
}

/// One tool result that came back.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ToolResult {
    /// The call it belongs to, by the harness's id.
    pub call_id: Option<String>,
    /// Whether the harness flagged it as an error, where it said either way.
    pub is_error: Option<bool>,
    /// How many bytes came back — injected into the *next* request, where they cost input tokens
    /// and then sit in the context for the rest of the run.
    pub content_bytes: usize,
    /// The textual content, where the result had one.
    pub content: Option<String>,
    /// The typed, per-tool fields the harness recorded beside the content.
    ///
    /// `Skill` records `commandName` and `success`; `Bash` records `stdout`, `stderr` and
    /// `interrupted`; `Edit` records `filePath` and `userModified`. This is the map a
    /// `tool.result` matcher names a field of, and keeping it open is what lets a tool the
    /// adapter has never heard of still be asserted about.
    pub fields: BTreeMap<String, Recorded>,
}

impl ToolResult {
    /// The value of one named result field, where the result carries it.
    pub fn field(&self, field: &str) -> Option<&Recorded> {
        self.fields.get(field)
    }
}

/// The account's rate-limit state.
///
/// A billing guard, not a performance one: `is_using_overage == false` is the expectation that
/// says *this run must not have been paid for out of overage*, which is a fact about money no
/// other part of the record carries.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RateLimitState {
    /// The status word, such as `allowed` or `allowed_warning`.
    pub status: Option<String>,
    /// Which window, such as `seven_day`.
    pub limit_type: Option<String>,
    /// How much of the window is used, from 0 to 1.
    pub utilization: Option<f64>,
    /// Whether the run is being paid for out of overage.
    pub is_using_overage: Option<bool>,
    /// When the window resets, as the harness recorded it.
    pub resets_at: Option<i64>,
}

/// What one model was used for, from the terminal record's per-model breakdown.
///
/// A token or cost expectation may carry a `model:` scope evaluated against one of these. An
/// expectation scoped to a model the run never used is `unk`, never `ok` — the `infra-spec` rule
/// for a scope that selects nothing, and for the same reason: an expectation must not be able to
/// pass by selecting nothing.
#[derive(Debug, Clone, PartialEq, Default, Serialize)]
pub struct ModelUsage {
    /// Input tokens attributed to this model.
    pub input_tokens: Option<u64>,
    /// Output tokens attributed to this model.
    pub output_tokens: Option<u64>,
    /// Cache reads attributed to this model.
    pub cache_read_input_tokens: Option<u64>,
    /// Cache creation attributed to this model.
    pub cache_creation_input_tokens: Option<u64>,
    /// What this model cost, in US dollars.
    pub cost_usd: Option<f64>,
}

/// The run's aggregate usage, as the terminal record reports it.
#[derive(Debug, Clone, PartialEq, Default, Serialize)]
pub struct RunUsage {
    /// Uncached input tokens.
    pub input_tokens: Option<u64>,
    /// Output tokens.
    pub output_tokens: Option<u64>,
    /// Tokens read from the cache. Excluded from `tokens.total` by definition.
    pub cache_read_input_tokens: Option<u64>,
    /// Tokens written to the cache. Writing the cache is not a miss against it.
    pub cache_creation_input_tokens: Option<u64>,
    /// The **billed** thinking tokens the API reported.
    pub thinking_tokens: Option<u64>,
    /// How many per-iteration usage records the run carried.
    ///
    /// An array's length, not a counter — and nothing like the other three run quantities
    /// (design § 3.5).
    pub iterations: Option<usize>,
    /// The speed tier the account was served at.
    pub speed: Option<String>,
    /// The service tier the account was served at.
    pub service_tier: Option<String>,
}

/// The terminal record: how the run ended, and the source of every resource fact.
#[derive(Debug, Clone, PartialEq, Default, Serialize)]
pub struct RunOutcome {
    /// Whether the harness declared the run an error.
    pub is_error: Option<bool>,
    /// The record's own subtype, such as `success`.
    pub subtype: Option<String>,
    /// Why the model stopped, such as `end_turn`.
    pub stop_reason: Option<String>,
    /// Why the run ended, such as `completed`.
    pub terminal_reason: Option<String>,
    /// The API error status, where there was one. Absent is the healthy case.
    pub api_error_status: Option<String>,
    /// The harness's own notion of a turn — the only one of the four run quantities it names.
    pub num_turns: Option<u64>,
    /// Wall-clock duration of the run.
    pub duration_ms: Option<u64>,
    /// Duration attributed to API calls. Observed *exceeding* `duration_ms` in a real transcript,
    /// which is a good reason not to derive one from the other.
    pub duration_api_ms: Option<u64>,
    /// Recorded time to first token. Read, never derived: the first events of a run carry no
    /// timestamp at all, so a subtraction there would compute zero.
    pub ttft_ms: Option<u64>,
    /// Recorded startup overhead before the first API request — the one latency number that is
    /// about the harness rather than the model.
    pub time_to_request_ms: Option<u64>,
    /// What the run cost, in US dollars.
    pub total_cost_usd: Option<f64>,
    /// How many permission requests were denied.
    pub permission_denials: Option<u64>,
    /// How many subagents were spawned.
    pub subagents_spawned: Option<u64>,
    /// The run's aggregate usage.
    pub usage: Option<RunUsage>,
    /// Per-model usage, keyed by the model's name. [`None`] when the harness records none.
    pub model_usage: Option<BTreeMap<String, ModelUsage>>,
}

/// An event this build does not recognise.
///
/// Kept, digested and never interpreted. An expectation whose truth would depend on one is `unk`,
/// with the reason naming the index and the unrecognised type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OpaqueEvent {
    /// The `type` it declared, where it declared one.
    pub event_type: Option<String>,
    /// The `subtype` it declared, where it declared one.
    pub subtype: Option<String>,
    /// The digest of the raw line, so the report can name it without quoting it.
    pub digest: String,
}

/// One API request the run made, with the usage the harness recorded for it.
///
/// Kept in the IR and given no expectation kind in v0.1 (design § 2.7): assertions over the
/// *series* — the cache-read ramp is monotone, cache creation is front-loaded — need a vocabulary
/// for sequences that a single-field matcher does not have. The data is here so that adding them
/// later costs nothing.
#[derive(Debug, Clone, PartialEq, Default, Serialize)]
pub struct AssistantRequest {
    /// The line of the transcript it came from.
    pub source_line: usize,
    /// The harness's id for the API request, where it records one.
    ///
    /// Several streamed events share one id, which is why `api_requests` and `events.assistant`
    /// are different numbers.
    pub request_id: Option<String>,
    /// The model that answered, where the harness records one.
    pub model: Option<String>,
    /// Uncached input tokens on this request.
    pub input_tokens: Option<u64>,
    /// Output tokens on this request.
    pub output_tokens: Option<u64>,
    /// Cache reads on this request.
    pub cache_read_input_tokens: Option<u64>,
    /// Cache creation on this request.
    pub cache_creation_input_tokens: Option<u64>,
}

/// One tool call's place in the run's wall clock.
///
/// Both durations are **derived from recorded timestamps** and neither is measured (design
/// § 2.6). Where a timestamp is missing the duration is [`None`], and the expectation over it is
/// `unk` — never zero, and never a value obtained by timing something.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Step {
    /// The IR index of the event carrying the call.
    pub call_event: usize,
    /// The tool's name.
    pub tool: String,
    /// The inference interval **ending** at the call — the model thinking and emitting it.
    pub gen_ms: Option<i64>,
    /// From the call being issued to its result coming back — the tool doing the work.
    pub exec_ms: Option<i64>,
}

/// What one tool cost the run.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct ToolTraffic {
    /// How many times it was called.
    pub calls: usize,
    /// How many of those results were flagged as errors.
    pub errors: usize,
    /// How many results were correlated at all.
    pub results: usize,
    /// Bytes of arguments — model output.
    pub input_bytes: usize,
    /// Bytes of results — injected into the next request's input.
    pub result_bytes: usize,
}

/// The census of a run: what `protocol trace inspect` prints.
///
/// The eval's informational metrics block, as a value rather than as sixty-five lines of `jq`.
/// It states quantities and no opinions; the opinions are [`crate::spec`]'s job.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Census {
    /// The digest of the transcript this describes.
    pub transcript_digest: String,
    /// How many IR events.
    pub events: usize,
    /// How many of each family.
    pub events_by_family: BTreeMap<String, usize>,
    /// How many events the adapter could not read.
    pub opaque_events: usize,
    /// How many `assistant` transcript events — an artefact of streaming, not a cost measure.
    pub assistant_events: usize,
    /// How many distinct API requests — the closest thing to "how many times did we call the
    /// model".
    pub api_requests: usize,
    /// Per-tool traffic.
    pub tool_traffic: BTreeMap<String, ToolTraffic>,
    /// How many groups of byte-identical `(tool, input)` calls.
    pub repeated_call_groups: usize,
    /// Per-call timings.
    pub steps: Vec<Step>,
    /// The sum of every step's `gen`, or [`None`] when any step has none.
    pub inference_total_ms: Option<i64>,
    /// The sum of every step's `exec`, or [`None`] when any step has none.
    pub tool_exec_total_ms: Option<i64>,
}

/// One agent run, normalized.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TraceIr {
    /// The format claim, `trace-ir/1`.
    pub format: &'static str,
    /// The digest of the **raw transcript bytes**, so a report can name exactly which run it
    /// judged.
    ///
    /// Over the bytes and not over this model, deliberately (design § 2.9): an adapter upgrade
    /// that starts understanding a field must not silently rename the run.
    pub transcript_digest: String,
    /// Which adapter produced this, and which harness versions it was written against.
    pub adapter: AdapterRef,
    /// Every event, in order.
    pub events: Vec<TraceEvent>,
    /// One record per API-bearing assistant event, in order.
    pub requests: Vec<AssistantRequest>,
}

/// Which adapter read the transcript.
///
/// Versioned because the harness format is not a stable public schema (design D1): an adapter
/// declares the versions it was written against, and a report says which one judged the run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AdapterRef {
    /// The adapter's name, such as `claude-code/stream-json`.
    pub name: &'static str,
    /// The harness versions it was written against.
    pub written_against: &'static [&'static str],
}

impl TraceIr {
    /// Builds an IR: indexes the events and correlates results to their calls.
    ///
    /// Correlation lives here rather than in an adapter because it is harness-neutral — a result
    /// carries the id of the call it answers, whatever the runner spells that id — and because
    /// two adapters correlating separately are two places for the pairing to disagree.
    pub fn new(
        transcript_digest: String,
        adapter: AdapterRef,
        mut events: Vec<TraceEvent>,
        requests: Vec<AssistantRequest>,
    ) -> Self {
        for (position, event) in events.iter_mut().enumerate() {
            event.index = position;
        }

        let mut result_of: BTreeMap<String, usize> = BTreeMap::new();
        for event in &events {
            if let EventKind::ToolResult(result) = &event.kind {
                if let Some(id) = &result.call_id {
                    // First result wins: a harness that repeated one would be describing two
                    // answers to one question, and picking the later silently would hide it.
                    result_of.entry(id.clone()).or_insert(event.index);
                }
            }
        }
        for event in &mut events {
            if let EventKind::ToolCall(call) = &mut event.kind {
                if let Some(id) = &call.call_id {
                    call.result_event = result_of.get(id).copied();
                }
            }
        }

        Self {
            format: IR_FORMAT,
            transcript_digest,
            adapter,
            events,
            requests,
        }
    }

    /// The run's opening record, where the transcript has one.
    pub fn session_start(&self) -> Option<&SessionStart> {
        self.events.iter().find_map(|event| match &event.kind {
            EventKind::SessionStart(start) => Some(&**start),
            _ => None,
        })
    }

    /// The IR index of the opening record.
    pub fn session_start_event(&self) -> Option<usize> {
        self.events
            .iter()
            .find(|event| matches!(event.kind, EventKind::SessionStart(_)))
            .map(|event| event.index)
    }

    /// The terminal record, where the transcript has one.
    ///
    /// A transcript truncated by a crash has none, and that is exactly the case that must not
    /// read as a failed assertion.
    pub fn run_outcome(&self) -> Option<&RunOutcome> {
        self.events
            .iter()
            .rev()
            .find_map(|event| match &event.kind {
                EventKind::RunOutcome(outcome) => Some(&**outcome),
                _ => None,
            })
    }

    /// The IR index of the terminal record.
    pub fn run_outcome_event(&self) -> Option<usize> {
        self.events
            .iter()
            .rev()
            .find(|event| matches!(event.kind, EventKind::RunOutcome(_)))
            .map(|event| event.index)
    }

    /// The rate-limit state, where the transcript records one.
    pub fn rate_limit(&self) -> Option<(usize, &RateLimitState)> {
        self.events
            .iter()
            .rev()
            .find_map(|event| match &event.kind {
                EventKind::RateLimit(state) => Some((event.index, &**state)),
                _ => None,
            })
    }

    /// The last thinking estimate the harness emitted, with the event that carried it.
    pub fn last_thinking_estimate(&self) -> Option<(usize, u64)> {
        self.events
            .iter()
            .rev()
            .find_map(|event| match &event.kind {
                EventKind::ThinkingEstimate {
                    estimated_tokens, ..
                } => estimated_tokens.map(|tokens| (event.index, tokens)),
                _ => None,
            })
    }

    /// The final assistant text, with the event that carried it.
    pub fn final_assistant_text(&self) -> Option<(usize, &str)> {
        self.events
            .iter()
            .rev()
            .find_map(|event| match &event.kind {
                EventKind::AssistantText { text, .. } => Some((event.index, text.as_str())),
                _ => None,
            })
    }

    /// Every tool call, with the index of the event that carried it.
    pub fn tool_calls(&self) -> Vec<(usize, &ToolCall)> {
        self.events
            .iter()
            .filter_map(|event| event.tool_call().map(|call| (event.index, call)))
            .collect()
    }

    /// The result correlated to a call, where one came back.
    pub fn result_of(&self, call: &ToolCall) -> Option<(usize, &ToolResult)> {
        let index = call.result_event?;
        let event = self.events.get(index)?;
        event.tool_result().map(|result| (index, result))
    }

    /// Every event the adapter could not read.
    ///
    /// An expectation whose truth would depend on one of these is `unk`. Kept as a list rather
    /// than a count so the reason can name the index and the type.
    pub fn opaque_events(&self) -> Vec<(usize, &OpaqueEvent)> {
        self.events
            .iter()
            .filter_map(|event| match &event.kind {
                EventKind::Opaque(opaque) => Some((event.index, &**opaque)),
                _ => None,
            })
            .collect()
    }

    /// How many `assistant` transcript events the run produced.
    ///
    /// An artefact of streaming: text and each tool call arrive as separate events sharing one
    /// request id. Bound it to catch a run that fragmented, not to bound cost.
    pub fn assistant_event_count(&self) -> usize {
        self.requests.len()
    }

    /// How many distinct API requests the run made.
    ///
    /// Distinct `request_id` across assistant events. An assistant event without one counts as
    /// its own request rather than being folded into another: pretending two unlabelled events
    /// were one call would understate the number this kind exists to bound.
    pub fn api_request_count(&self) -> usize {
        let mut identified: BTreeSet<&str> = BTreeSet::new();
        let mut anonymous = 0usize;
        for request in &self.requests {
            match &request.request_id {
                Some(id) => {
                    identified.insert(id.as_str());
                }
                None => anonymous += 1,
            }
        }
        identified.len() + anonymous
    }

    /// Per-call timings, derived from recorded timestamps alone.
    ///
    /// `gen` is the interval ending at the event carrying the call, measured against the previous
    /// *timestamped transcript line* — not the previous IR event, because a text block and a tool
    /// call on one line are one moment and a zero between them would be an artefact of how the
    /// IR splits a line. `exec` runs from the call to its correlated result.
    pub fn steps(&self) -> Vec<Step> {
        // The timestamped lines, in order, so a "previous moment" is a previous line rather than
        // a previous event.
        let mut line_times: Vec<(usize, i64)> = Vec::new();
        for event in &self.events {
            if let Some(at) = event.timestamp_ms {
                if line_times.last().map(|(line, _)| *line) != Some(event.source_line) {
                    line_times.push((event.source_line, at));
                }
            }
        }
        let previous_time = |line: usize| -> Option<i64> {
            let position = line_times.iter().position(|(at, _)| *at == line)?;
            position
                .checked_sub(1)
                .and_then(|before| line_times.get(before))
                .map(|(_, at)| *at)
        };

        self.events
            .iter()
            .filter_map(|event| {
                let call = event.tool_call()?;
                let gen_ms = event
                    .timestamp_ms
                    .zip(previous_time(event.source_line))
                    .map(|(now, before)| now - before);
                let exec_ms = call
                    .result_event
                    .and_then(|index| self.events.get(index))
                    .and_then(|result| result.timestamp_ms)
                    .zip(event.timestamp_ms)
                    .map(|(back, out)| back - out);
                Some(Step {
                    call_event: event.index,
                    tool: call.name.clone(),
                    gen_ms,
                    exec_ms,
                })
            })
            .collect()
    }

    /// Per-tool traffic: calls, errors, and the bytes each direction cost.
    pub fn tool_traffic(&self) -> BTreeMap<String, ToolTraffic> {
        let mut traffic: BTreeMap<String, ToolTraffic> = BTreeMap::new();
        for (_, call) in self.tool_calls() {
            let entry = traffic.entry(call.name.clone()).or_default();
            entry.calls += 1;
            entry.input_bytes += call.input_bytes;
            if let Some((_, result)) = self.result_of(call) {
                entry.results += 1;
                entry.result_bytes += result.content_bytes;
                if result.is_error == Some(true) {
                    entry.errors += 1;
                }
            }
        }
        traffic
    }

    /// How many groups of byte-identical `(tool, input)` calls the run made.
    ///
    /// A confusion signal rather than a correctness one, which is why the expectation over it is
    /// a bound and not a prohibition.
    pub fn repeated_call_groups(&self) -> usize {
        let mut seen: BTreeMap<String, usize> = BTreeMap::new();
        for (_, call) in self.tool_calls() {
            *seen.entry(call.repetition_key()).or_default() += 1;
        }
        seen.values().filter(|count| **count > 1).count()
    }

    /// The whole census, for `protocol trace inspect`.
    pub fn census(&self) -> Census {
        let mut events_by_family: BTreeMap<String, usize> = BTreeMap::new();
        for event in &self.events {
            *events_by_family
                .entry(event.kind.family().to_owned())
                .or_default() += 1;
        }
        let steps = self.steps();
        let inference_total_ms = total_of(steps.iter().map(|step| step.gen_ms));
        let tool_exec_total_ms = total_of(steps.iter().map(|step| step.exec_ms));
        Census {
            transcript_digest: self.transcript_digest.clone(),
            events: self.events.len(),
            events_by_family,
            opaque_events: self.opaque_events().len(),
            assistant_events: self.assistant_event_count(),
            api_requests: self.api_request_count(),
            tool_traffic: self.tool_traffic(),
            repeated_call_groups: self.repeated_call_groups(),
            steps,
            inference_total_ms,
            tool_exec_total_ms,
        }
    }
}

/// The sum of a series of derived durations, or [`None`] when any of them is missing.
///
/// Deliberately not "the sum of the ones that are there". A total that silently omitted an
/// unmeasurable step would be a smaller number presented as the same quantity, which is the
/// failure mode invariant 5 exists to prevent — the honest answer is that this transcript cannot
/// state the total.
fn total_of(values: impl Iterator<Item = Option<i64>>) -> Option<i64> {
    let mut total = 0i64;
    let mut any = false;
    for value in values {
        total += value?;
        any = true;
    }
    any.then_some(total)
}

/// Reads an ISO-8601 instant in the one form transcripts record — `2026-08-21T12:04:15.233Z` —
/// into milliseconds since the Unix epoch.
///
/// Hand-written rather than taken from `chrono` or `time`, and the refusal is the point:
/// `AGENTS.md` § *Dependencies* says to prefer no dependency and record why. What is needed here
/// is one fixed, zulu-terminated format with optional fractional seconds — about forty lines,
/// with no timezone database, no parser combinators and no locale. A date library would buy
/// formats no harness writes and bring a transitive tree into a crate whose whole claim is that
/// it reads no clock.
///
/// Anything that is not that exact shape returns [`None`], which becomes an `unk` verdict rather
/// than a wrong duration.
#[must_use]
pub fn parse_timestamp_ms(text: &str) -> Option<i64> {
    let text = text.strip_suffix('Z')?;
    let (date, rest) = text.split_once('T')?;
    let (time, fraction) = match rest.split_once('.') {
        Some((time, fraction)) => (time, Some(fraction)),
        None => (rest, None),
    };

    let mut date_parts = date.split('-');
    let year: i64 = date_parts.next()?.parse().ok()?;
    let month: i64 = date_parts.next()?.parse().ok()?;
    let day: i64 = date_parts.next()?.parse().ok()?;
    if date_parts.next().is_some() || !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }

    let mut time_parts = time.split(':');
    let hour: i64 = time_parts.next()?.parse().ok()?;
    let minute: i64 = time_parts.next()?.parse().ok()?;
    let second: i64 = time_parts.next()?.parse().ok()?;
    if time_parts.next().is_some() || hour > 23 || minute > 59 || second > 60 {
        return None;
    }

    let milliseconds = match fraction {
        None => 0,
        Some(digits) => {
            if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
                return None;
            }
            let mut padded: String = digits.chars().take(3).collect();
            while padded.len() < 3 {
                padded.push('0');
            }
            padded.parse::<i64>().ok()?
        }
    };

    let days = days_from_civil(year, month, day);
    Some(((days * 24 + hour) * 60 + minute) * 60_000 + second * 1_000 + milliseconds)
}

/// Days from `1970-01-01` to a proleptic Gregorian date.
///
/// Howard Hinnant's `days_from_civil`, which is the standard closed form and is exact for every
/// year this will ever meet. Written out rather than pulled in because it is nine lines.
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = year - era * 400;
    let day_of_year = (153 * (if month > 2 { month - 3 } else { month + 9 }) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call(line: usize, at: Option<&str>, id: &str, name: &str, input: &str) -> TraceEvent {
        let parsed: Value = serde_json::from_str(input).expect("the fixture's input is JSON");
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

    fn result(line: usize, at: Option<&str>, id: &str, bytes: usize, error: bool) -> TraceEvent {
        TraceEvent::new(
            line,
            at.map(ToOwned::to_owned),
            EventKind::ToolResult(Box::new(ToolResult {
                call_id: Some(id.to_owned()),
                is_error: Some(error),
                content_bytes: bytes,
                content: None,
                fields: BTreeMap::new(),
            })),
        )
    }

    fn adapter() -> AdapterRef {
        AdapterRef {
            name: "test",
            written_against: &["0"],
        }
    }

    #[test]
    fn a_result_is_correlated_to_its_call_by_the_harnesss_own_id() {
        let ir = TraceIr::new(
            "digest".to_owned(),
            adapter(),
            vec![
                call(
                    1,
                    Some("2026-08-21T12:00:00.000Z"),
                    "a",
                    "Bash",
                    r#"{"command":"ls"}"#,
                ),
                result(2, Some("2026-08-21T12:00:00.100Z"), "a", 12, false),
            ],
            Vec::new(),
        );
        let (index, first) = ir.tool_calls()[0];
        assert_eq!(
            index, 0,
            "indices are assigned by the IR, not by the caller"
        );
        assert_eq!(
            first.result_event,
            Some(1),
            "the call knows which event answered it"
        );
        assert_eq!(ir.result_of(first).expect("a result came back").0, 1);
    }

    #[test]
    fn a_call_whose_result_never_came_back_carries_no_result_rather_than_a_bad_one() {
        // A truncated transcript. The distinction this protects is the one design § 3.3 draws:
        // "no result was correlated" is not "the result was bad".
        let ir = TraceIr::new(
            "digest".to_owned(),
            adapter(),
            vec![call(1, None, "a", "Bash", r#"{"command":"ls"}"#)],
            Vec::new(),
        );
        let (_, only) = ir.tool_calls()[0];
        assert_eq!(only.result_event, None);
        assert!(ir.result_of(only).is_none());
    }

    #[test]
    fn gen_is_measured_against_the_previous_timestamped_line_not_the_previous_event() {
        // Two IR events on one line — a text block and the tool call beside it. Measuring `gen`
        // against the previous *event* would report 0 ms for a call the model spent a second
        // producing, which is the artefact this rule exists to avoid.
        let text = TraceEvent::new(
            5,
            Some("2026-08-21T12:00:01.000Z".to_owned()),
            EventKind::AssistantText {
                text: "thinking".to_owned(),
                request_id: None,
            },
        );
        let earlier = TraceEvent::new(
            4,
            Some("2026-08-21T12:00:00.000Z".to_owned()),
            EventKind::AssistantText {
                text: "hello".to_owned(),
                request_id: None,
            },
        );
        let ir = TraceIr::new(
            "digest".to_owned(),
            adapter(),
            vec![
                earlier,
                text,
                call(
                    5,
                    Some("2026-08-21T12:00:01.000Z"),
                    "a",
                    "Bash",
                    r#"{"command":"ls"}"#,
                ),
                result(6, Some("2026-08-21T12:00:01.250Z"), "a", 4, false),
            ],
            Vec::new(),
        );
        let steps = ir.steps();
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].gen_ms, Some(1_000));
        assert_eq!(steps[0].exec_ms, Some(250));
    }

    #[test]
    fn a_step_with_no_recorded_timestamp_has_no_duration_rather_than_a_zero_one() {
        let ir = TraceIr::new(
            "digest".to_owned(),
            adapter(),
            vec![
                call(1, None, "a", "Bash", r#"{"command":"ls"}"#),
                result(2, None, "a", 4, false),
            ],
            Vec::new(),
        );
        let steps = ir.steps();
        assert_eq!(steps[0].gen_ms, None, "never zero: nothing was recorded");
        assert_eq!(steps[0].exec_ms, None);
        assert_eq!(
            ir.census().inference_total_ms,
            None,
            "a total that omitted an unmeasurable step would be a smaller number wearing the same \
             name"
        );
    }

    #[test]
    fn identical_calls_are_one_repeated_group_and_different_arguments_are_not() {
        let ir = TraceIr::new(
            "digest".to_owned(),
            adapter(),
            vec![
                call(1, None, "a", "Read", r#"{"file_path":"/x"}"#),
                call(2, None, "b", "Read", r#"{"file_path":"/x"}"#),
                call(3, None, "c", "Read", r#"{"file_path":"/y"}"#),
            ],
            Vec::new(),
        );
        assert_eq!(ir.repeated_call_groups(), 1);
    }

    #[test]
    fn tool_traffic_separates_what_the_model_wrote_from_what_came_back() {
        let ir = TraceIr::new(
            "digest".to_owned(),
            adapter(),
            vec![
                call(1, None, "a", "Read", r#"{"file_path":"/x"}"#),
                result(2, None, "a", 4_668, false),
                call(3, None, "b", "Write", r#"{"file_path":"/y","content":"…"}"#),
                result(4, None, "b", 645, true),
            ],
            Vec::new(),
        );
        let traffic = ir.tool_traffic();
        assert_eq!(traffic["Read"].result_bytes, 4_668);
        assert_eq!(traffic["Read"].errors, 0);
        assert_eq!(traffic["Write"].errors, 1);
        assert!(
            traffic["Write"].input_bytes > traffic["Read"].input_bytes,
            "the asymmetry is the point: Write is output-heavy, Read is injection-heavy"
        );
    }

    #[test]
    fn an_assistant_event_without_a_request_id_is_its_own_request_rather_than_folded_into_another()
    {
        let ir = TraceIr::new(
            "digest".to_owned(),
            adapter(),
            Vec::new(),
            vec![
                AssistantRequest {
                    source_line: 1,
                    request_id: Some("req_1".to_owned()),
                    ..AssistantRequest::default()
                },
                AssistantRequest {
                    source_line: 2,
                    request_id: Some("req_1".to_owned()),
                    ..AssistantRequest::default()
                },
                AssistantRequest {
                    source_line: 3,
                    request_id: None,
                    ..AssistantRequest::default()
                },
            ],
        );
        assert_eq!(ir.assistant_event_count(), 3, "three streamed events");
        assert_eq!(ir.api_request_count(), 2, "one shared id, one unlabelled");
    }

    #[test]
    fn a_timestamp_is_read_in_the_one_form_transcripts_record_and_refused_otherwise() {
        assert_eq!(parse_timestamp_ms("1970-01-01T00:00:00.000Z"), Some(0));
        assert_eq!(parse_timestamp_ms("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(
            parse_timestamp_ms("2026-08-21T12:04:15.233Z"),
            Some(1_787_313_855_233),
            "the observed transcript's first timestamp"
        );
        assert_eq!(
            parse_timestamp_ms("2026-08-21T12:04:16.719Z").unwrap()
                - parse_timestamp_ms("2026-08-21T12:04:15.233Z").unwrap(),
            1_486,
            "the design's observed `gen` for step 1"
        );
        for refused in [
            "2026-08-21 12:04:15Z",
            "2026-08-21T12:04:15.233+02:00",
            "2026-08-21T12:04:15.233",
            "2026-13-01T00:00:00Z",
            "not a time",
        ] {
            assert_eq!(
                parse_timestamp_ms(refused),
                None,
                "{refused} is not the one form, and guessing at it would produce a wrong duration"
            );
        }
    }
}
