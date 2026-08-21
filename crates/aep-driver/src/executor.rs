//! Adapter point 1: the three things that touch the world, behind three traits.
//!
//! Running a program, calling a model and pausing for a person are the only three things a step can
//! be, and none of them belongs in a crate that claims to be deterministic. They live here as
//! traits so the router can be exercised against a fake — which is not a testing convenience but
//! the acceptance criterion for the neutrality claim itself: a second, fake harness proves the seam
//! is real, and it does it with no model, no network and no credential.
//!
//! The traits are three rather than one so a harness that implements only some of them says so in
//! its types. [`StepExecutors`] is the blanket bundle the loop asks for.
//!
//! # `StepOutcome` is D5, and it is the load-bearing type in this crate
//!
//! The protocol is three-valued and the driver never collapses it:
//!
//! | what happened | variant | what the loop does |
//! |---|---|---|
//! | a verifier produced a verdict — a suite ran, passing or failing | [`StepOutcome::Observed`] | submit it, and let the **engine** route |
//! | the step ran and there is nothing to submit — an `llm` step finished | [`StepOutcome::Nothing`] | advance to the next step |
//! | nothing was observed — crash, timeout, OOM, missing binary, model error | [`StepOutcome::NoVerdict`] | submit **nothing**; retry within the step kind's budget |
//! | a person is owed a question | [`StepOutcome::Paused`] | persist and stop; the snapshot is the queue |
//!
//! **`Unknown` is spelled "submit nothing".** The engine has no `Unknown` value to submit — absence
//! is modelled as the fact simply not being in the store — so a crashed `cargo test` is *not*
//! `tests.unit.failed > 0`. Submitting a failing `TestResult` for a suite that never ran would
//! fabricate an observation, which is invariant 7's failure one layer above the engine, and it would
//! send an agent to fix code nobody ran. A failing suite is [`StepOutcome::Observed`] carrying a
//! `TestResult` with failures, and the back-edge is then the **workflow's** to take.
//!
//! The one exception D5 names is not a fourth variant: `protocol trace check` exit 3 is a *recorded*
//! absence — `trace evidence` writes `status: inconclusive` — so it arrives as
//! [`StepOutcome::Observed`] carrying that record. A recorded absence is strictly better than a
//! silent one, and the requirement stays owed either way.

use std::path::Path;

use aep_domain::ids::StateId;
use aep_driver_spec::map::{CommandStep, LlmStep, OperatorStep};
use aep_driver_spec::tool::ToolConfig;
use aep_engine::EvidenceSubmission;

/// What an executor is told about the step it is being asked to run.
///
/// Everything here is a function of persisted state, which is what makes D4's per-step session
/// granularity checkable: a step's input does not depend on a previous step's hidden context.
#[derive(Debug)]
pub struct StepContext<'a> {
    /// The workflow state the run is in.
    pub state: &'a StateId,
    /// Which step of that state's list this is.
    pub index: usize,
    /// Which attempt at this step this is, counting from `1`.
    ///
    /// Cumulative over the whole run and never reset, so it is unique per execution of the step —
    /// which is what a transcript file name needs to avoid overwriting the attempt that failed.
    pub attempt: u32,
    /// What the model may hold while it runs, decided for **this** state.
    ///
    /// Per state rather than per run: `effective_policy` grants the state's capabilities on top of
    /// the plan's, so the legal tool set genuinely changes at every `Moved`.
    pub tools: &'a ToolConfig,
    /// The run's own directory, where a transcript or a captured output belongs.
    pub run_directory: &'a Path,
    /// One line per requirement in force, from the evaluation, each naming the document that asked.
    ///
    /// Handed over verbatim: the guide's rule is that an explanation is one line per requirement
    /// rather than a summary, and a driver that summarised here would be the only place the
    /// summary existed.
    pub requirements: &'a [String],
}

/// What running a step produced.
#[derive(Debug)]
pub enum StepOutcome {
    /// A verifier produced a verdict: submit this.
    ///
    /// `False` and `True` are both this. Whether a failing verdict means *go round again* is the
    /// workflow's decision, taken by `transition()`, never the driver's.
    Observed(Box<EvidenceSubmission>),
    /// The step ran and there is nothing to submit — an `llm` step that finished.
    ///
    /// An agent's own statement never satisfies an independence requirement, so this is the only
    /// honest outcome an `llm` step has. What it achieved that is *checkable* is observed by a
    /// subsequent `command` step.
    Nothing,
    /// Nothing was observed — a crash, a timeout, a missing binary, a model error.
    ///
    /// D5's `Unknown`, and the reason it is a variant rather than a `Result::Err` is that it is a
    /// routing outcome: the loop retries it within a budget and submits nothing at all.
    NoVerdict {
        /// What went wrong, for the run report.
        reason: String,
    },
    /// An `operator` step: the run pauses here.
    ///
    /// There is no waiting process and no queue. A driver holding a terminal open for a person is a
    /// driver that loses the run when the terminal closes, and the snapshot is already a queue that
    /// survives a reboot.
    Paused {
        /// What the person is owed, for the run report.
        reason: String,
    },
}

impl StepOutcome {
    /// `true` when nothing was observed, so nothing may be submitted.
    pub fn is_no_verdict(&self) -> bool {
        matches!(self, Self::NoVerdict { .. })
    }
}

/// Runs an `llm` step.
///
/// One model session per step (D4): the prompt, the named skills and a tool set derived from the
/// state's capabilities go in, and the process exits when the step does.
pub trait LlmStepExecutor {
    /// Runs `step`, returning what was observed — which for an `llm` step is never evidence.
    fn run_llm(&mut self, step: &LlmStep, context: &StepContext<'_>) -> StepOutcome;
}

/// Runs a `command` step.
///
/// This is how `independent: true` is honestly satisfied: the producer is a verifier because a
/// verifier produced it — the driver ran the program and read its exit status, and nothing about a
/// model's opinion of the run enters the record.
pub trait CommandStepExecutor {
    /// Runs `step`, returning the verdict the program produced, or that it produced none.
    fn run_command(&mut self, step: &CommandStep, context: &StepContext<'_>) -> StepOutcome;
}

/// Hands an `operator` step to a person.
///
/// The implementation shows `CompletionExplanation` verbatim and stops. It never answers on the
/// person's behalf: what comes back is recorded with `Producer::Human` by whoever wrote the
/// document, and never by this crate.
pub trait OperatorStepExecutor {
    /// Presents `step`, returning [`StepOutcome::Paused`] when the run is to stop here.
    fn run_operator(&mut self, step: &OperatorStep, context: &StepContext<'_>) -> StepOutcome;
}

/// A harness that can run all three step kinds.
pub trait StepExecutors: LlmStepExecutor + CommandStepExecutor + OperatorStepExecutor {}

impl<T: LlmStepExecutor + CommandStepExecutor + OperatorStepExecutor> StepExecutors for T {}
