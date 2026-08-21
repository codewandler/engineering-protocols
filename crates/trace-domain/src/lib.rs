//! The two models of transcript conformance: what an agent run *was*, and what it was supposed to
//! be.
//!
//! The third instance of a pattern this repository has now built twice. `ess-domain` models a
//! system somebody authored; `infra-domain` models a cluster somebody scanned; this models an
//! **agent run somebody recorded** — and the expectations somebody wrote about it.
//!
//! | | ESS | Infra | **Trace** |
//! |---|---|---|---|
//! | observation | — (the model is authored) | a cluster scan, out of process | **an agent-run transcript** |
//! | normalized IR | `EssIr` | `infra-ir/1`, content-addressed | [`trace-ir/1`](ir), content-addressed |
//! | authored expectations | the specification itself | `infra-spec/1`, twelve kinds | [`trace-spec/1`](spec), fifty-one kinds |
//! | verdicts | pass / fail / unsupported | `ok` / `gap` / `unk` | `ok` / `gap` / `unk` |
//! | the third value means | the scenario could not be executed | the snapshot cannot decide | **the adapter did not understand the event** |
//!
//! The pattern is not copied for tidiness. It is copied because **the third value is the whole
//! point in each case**, and getting it wrong in a new domain is how a checker starts lying. An
//! event kind the adapter does not understand must yield `unk` — never a pass, and never a fail.
//! A transcript from a harness version that renamed a field must produce *"this run could not be
//! judged on that expectation"*, not *"the agent did not load the skill"*.
//!
//! | module | contents |
//! |---|---|
//! | [`ir`] | the harness-neutral event IR: seven recognised event families, one opaque one, and the derived census |
//! | [`spec`] | the expectation vocabulary: fifty-one kinds, severity, and the `unk` policy |
//! | [`matcher`] | bounds, field matchers and call selectors — the whole of the language, and no more of it |
//! | [`raw`] | the permissive half, and the `TryFrom` that is the only way into a [`spec::TraceSpec`] |
//! | [`code`] | the `TRACE-` refusal registry and the accumulator |
//! | [`digest`] | the one hash construction, over the transcript's bytes and over the specification's content |
//!
//! ```text
//! result.jsonl  ──adapter──▶  trace-ir/1  ──check(spec)──▶  report  ──▶  evidence
//!    (harness)                (neutral)                    (verdicts)     (AEP)
//! ```
//!
//! The adapter and the checker are the next crate, `trace-spec`. This one holds only the two
//! models, and it holds them together on purpose: they are the pair that a schema is published
//! from and a report is written against, and they change for one reason — the vocabulary moved.
//! An adapter changes when a *harness* moves, and a checker changes when the *evaluation* moves,
//! which are two other reasons and two other files.
//!
//! # What is not here
//!
//! * **No I/O.** Nothing in this crate opens a file, and nothing reaches a network.
//! * **No clock and no randomness** (invariants 8 and 9). Every duration is derived from a
//!   timestamp the harness recorded; `tests/determinism.rs` scans for the tokens that would break
//!   this and builds the same values twice to compare bytes.
//! * **No model, anywhere.** A transcript checker is the single most tempting place in this
//!   repository to ask a model whether an agent behaved reasonably, and that would make every
//!   verdict unreproducible and unfalsifiable at once.

pub mod code;
pub mod digest;
pub mod ir;
pub mod matcher;
pub mod raw;
pub mod spec;

pub use code::{TraceCode, ValidationError, ValidationErrors};
pub use digest::{digest_of_bytes, digest_of_canonical};
pub use ir::{
    parse_timestamp_ms, AdapterRef, AssistantRequest, Census, EventKind, LoadedPlugin, McpServer,
    ModelUsage, OpaqueEvent, RateLimitState, Recorded, RunOutcome, RunUsage, SessionStart, Step,
    ToolCall, ToolResult, ToolTraffic, TraceEvent, TraceIr, IR_FORMAT,
};
pub use matcher::{
    glob_matches, text_of, CallSelector, CountBound, FieldMatcher, RangeBound, ResultMatcher,
    ScalarValue,
};
pub use spec::{
    Aggregate, ApiErrorStatus, Expectation, ExpectationKind, OnUnknown, Severity, ToolAvailability,
    TraceSpec, SPEC_FORMAT,
};
