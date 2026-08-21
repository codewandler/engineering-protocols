//! The two computations of transcript conformance: read a run, then judge it.
//!
//! ```text
//! result.jsonl  ──adapter──▶  trace-ir/1  ──check(spec)──▶  CheckReport
//!    (harness)                (neutral)                     (verdicts)
//! ```
//!
//! | module | contents |
//! |---|---|
//! | [`adapter`] | the Claude Code `stream-json` adapter: JSONL to [`TraceIr`](trace_domain::ir::TraceIr) |
//! | [`check`] | evaluation: a specification against an IR, three-valued, every verdict citing its events |
//! | [`report`] | [`CheckReport`](report::CheckReport) — the serializable answer, and what a later evidence builder consumes |
//! | [`render`] | the text rendering, and the redacted one |
//! | [`evidence`] | the handoff: a [`CheckReport`](report::CheckReport) becomes the AEP evidence record the protocol decides on |
//!
//! # Two crates, and where the line runs
//!
//! `trace-domain` holds the *models*; this holds the *mechanisms*. An adapter changes when a
//! harness moves, a checker changes when the evaluation moves, and a model changes when the
//! vocabulary moves — three reasons, and the design's D4 asked for the split on exactly that
//! argument.
//!
//! # Determinism
//!
//! Same transcript plus same specification in, byte-identical report out (invariant 9). No clock
//! is read: every duration and every cost comes out of the transcript. `BTreeMap` ordering
//! throughout, and `tests/determinism.rs` checks both twice over.
//!
//! # No model in the checker
//!
//! There is no LLM anywhere here, and this is the single most tempting place in the repository to
//! put one — *"ask a model whether the agent behaved reasonably"* is one function call away and
//! would make every verdict unreproducible and unfalsifiable at once. The eval's adversarial
//! reviewer is a separate artifact beside the report; it cannot move an exit code, and the
//! protocol would classify anything it said as `Producer::Agent` and refuse it as independent
//! evidence.

pub mod adapter;
pub mod check;
pub mod evidence;
pub mod render;
pub mod report;
