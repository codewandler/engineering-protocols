//! Protocol execution: resolution, evaluation and transitions.
//!
//! The engine is deliberately not an agent harness. It answers questions a harness asks:
//!
//! ```text
//! Harness                                    Engine
//!   │ initialize(task)                    ─▶ ExecutionPlan, first state
//!   │ requirements(execution)             ─▶ what is owed now, and why
//!   │ capabilities(execution)             ─▶ what may be done
//!   │ authorize(execution, action)        ─▶ allowed / needs approval / denied, with the rule
//!   │ submit_evidence(execution, …)       ─▶ recorded, or rejected as undeclared
//!   │ evaluate(execution)                 ─▶ every transition, permitted or not, with unmet lists
//!   │ transition(execution)               ─▶ moved / blocked / complete
//! ```
//!
//! Every answer is deterministic in the validated documents plus the evidence submitted. Given a
//! [`FixedClock`](clock::FixedClock), replaying an execution reproduces its event stream exactly.
//!
//! | module | responsibility |
//! |---|---|
//! | [`registry`] | the documents in force, and the cross-document checks |
//! | [`load`] | reading a document tree from disk |
//! | [`resolve`] | `Task` + registry → [`ExecutionPlan`](aep_domain::ExecutionPlan) |
//! | [`execution`] | live state: facts, evidence, events, artifacts |
//! | [`evaluate`] | what is owed, what is permitted |
//! | [`policy`] | capability decisions, with the rule that produced each one |
//! | [`explain`] | human- and machine-readable explanations |
//! | [`engine`] | the [`ProtocolEngine`](engine::ProtocolEngine) trait and its implementation |
//! | [`clock`] | injected time, so executions replay |

#[cfg(test)]
pub(crate) mod fixtures;

pub mod clock;
pub mod engine;
pub mod error;
pub mod evaluate;
pub mod execution;
pub mod explain;
pub mod load;
pub mod policy;
pub mod registry;
pub mod resolve;

pub use clock::{Clock, FixedClock, SteppingClock, SystemClock};
pub use engine::{Engine, EvidenceSubmission, ProtocolEngine, TransitionResult};
pub use error::ProtocolError;
pub use evaluate::{Evaluation, Requirement, RequirementSource, TransitionEvaluation};
pub use execution::{Execution, Snapshot};
pub use explain::{CompletionExplanation, DecisionExplanation};
pub use load::{load_tree, load_tree_report, LoadErrors, LoadFailure, LoadOutcome};
pub use policy::{Decision, DecisionReason};
pub use registry::Registry;
pub use resolve::resolve;
