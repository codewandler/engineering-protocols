//! The reference driver's routing core: a loop that asks the engine and does only what the answers
//! permit.
//!
//! ```text
//! restore-or-init  →  evaluate  →  select step  →  execute step
//!                                                       │
//!                         ┌─────────────────────────────┘
//!                         ▼
//!                 submit evidence  →  transition  →  persist  →  (repeat)
//! ```
//!
//! # What this crate may not do
//!
//! **Gates are evaluated only by the engine.** Nothing here reads a predicate, compares a fact path
//! or decides that a transition is legal. It asks, and it does what it is told. A driver that could
//! evaluate a gate would be a second protocol implementation with none of the conformance suites,
//! and the first time the two disagreed the one nobody tested would win.
//!
//! It follows that this crate is **pure**: clock-free, randomness-free, and free of the three
//! things that touch the world — running a program, calling a model, and pausing for a person.
//! Those are [`executor`] traits, implemented in `protocol-cli`. The store lock, the pid-liveness
//! probe and the run-directory allocator are `protocol-cli`'s too, per review finding **F19**: a
//! liveness probe reads ambient OS state and uses neither `SystemTime::now` nor `rand`, so
//! `tests/determinism.rs` would not catch it and **placement** is the only thing keeping the purity
//! claim true. This crate is *handed* a [`lock::LockState`] and probes nothing.
//!
//! | module | what it holds |
//! |---|---|
//! | [`tool`] | adapter point 2 — capabilities → a harness-neutral tool description, by `decide` |
//! | [`executor`] | adapter point 1 — the three step-executor traits, and what a step can return |
//! | [`route`] | the deterministic router: run the next step, transition, or stop on a budget |
//! | [`approval`] | D3(c)'s static pre-flight scan for approvals a run can reach |
//! | [`coverage`] | F-W4.2-4's static pre-flight scan: what the plan demands, against what the map can produce |
//! | [`lock`] | D6's refusal, rendered from a lock state somebody else observed |
//! | [`run`] | the loop, the run directory, and what a run reports when it stops |
//!
//! # Replayability, claimed narrowly
//!
//! What replays is the sequence of **decisions**: the same snapshot and the same evidence yield the
//! same routing. The *work* does not replay — a test run, a model call and a person's answer are
//! not reproducible, and nothing here pretends they are. What is stored is what was decided and on
//! what evidence, not a recording of the world.
//!
//! # Where the loop deviates from § 4.4's sketch, and why
//!
//! § 4.4 draws `execute step → submit evidence → transition` as one cycle. This driver attempts a
//! transition when the **state's steps are done** ([`route::NextStep::Transition`]), not after every
//! step. Transitioning mid-state would leave the remaining steps unrun: § 4.2 says *"order inside a
//! state is the map author's"*, and a driver that walks out of a state with authored work pending
//! has overridden that order silently. It also keeps the audit trail readable — `transition()`
//! emits a `TransitionBlocked` event per candidate every time it is called, so calling it after
//! each retry of a crashing step would bury the run's real history under its own polling.

pub mod approval;
pub mod coverage;
pub mod executor;
pub mod lock;
pub mod route;
pub mod run;
pub mod tool;

pub use approval::{reachable_approvals, ReachableApproval};
pub use coverage::{evidence_coverage, CoverageReport, MissingProducer};
pub use executor::{
    CommandStepExecutor, LlmStepExecutor, OperatorStepExecutor, StepAuthorizer, StepContext,
    StepExecutors, StepOutcome,
};
pub use lock::{Liveness, LockState};
pub use route::{next_step, steps_remaining, NextStep};
pub use run::{drive, resume, DriveError, DriverOptions, RunDirectory, RunReport, ENGINE_VERSION};
pub use tool::{tool_config, TOOL_CANDIDATES};
