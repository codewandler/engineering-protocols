//! What a conformance suite is, whether a candidate command input reaches a guard in it, and how a
//! specification becomes one.
//!
//! Three things, in the order wave 4 needs them.
//!
//! [`scenario`] is the **canonical scenario IR** (design §21): the serialisable definition of every
//! check a specification obliges an implementation to pass. It executes nothing. That separation is
//! the point — generate a runner's tests straight from an
//! [`EssIr`](ess_compiler::EssIr) and the first runner becomes the semantic definition by accident,
//! which is the failure §22 exists to prevent.
//!
//! [`decision`] and [`input`] are the other half: given a candidate input, does a declared guard
//! hold? A suite cannot claim an input reaches an outcome without one.
//!
//! # The rest of the crate's job
//!
//! An [`EssIr`](ess_compiler::EssIr) says a command has an outcome taken `when: amount.amount > 0`.
//! A conformance runner wanting to exercise that branch has to answer one question first: **given
//! this candidate input, does that predicate hold?** Nothing in the workspace could answer it.
//! [`Predicate::evaluate`](aep_domain::predicate::Predicate::evaluate) has always been there, but it
//! reads a [`FactSource`](aep_domain::facts::FactSource), and the only implementor was
//! [`FactStore`](aep_domain::facts::FactStore) — nothing projected a *command input* into one. This
//! crate is that projection, and the decision procedure built on top of it.
//!
//! # Why this is not part of `ess-gen`
//!
//! `ess-gen`'s `Generator` trait is infallible for a valid IR by contract: a construct it cannot
//! render is a defect in the crate, not an outcome, and the crate documents that it holds no clock
//! and no RNG. Both claims are load-bearing — they are what make
//! `cargo xtask generate --check` a drift check rather than a re-run.
//!
//! A runner is the opposite on both counts. Refusing is a legitimate, expected result whenever a
//! specification cannot be witnessed, and running against a real target takes a clock. Putting this
//! in `ess-gen` would have falsified two sentences that crate relies on, so wave 3.5 decision 3 put
//! the runner in a crate of its own. This is the first thing in it.
//!
//! # Three values in, two values out
//!
//! Evaluation is Kleene three-valued and a runner's question is not. `True` means *use this
//! candidate*; `False` means *try another one*; and `Unknown` means neither — it means the question
//! was not answerable, which is a statement about the specification and the candidate's shape, not
//! about the candidate's values.
//!
//! Collapsing `Unknown` into `False` is the single mistake this crate exists to prevent. A generator
//! that treats "unknown" as "try another candidate" spends its whole budget re-rolling values
//! against a predicate that can never be decided, and then reports a *specification* defect as a
//! flaky test. That is invariant 5 — `Unknown` is not `False` — read from the generator's side.
//!
//! So [`Decision`] has three cases and only one of them means "try again":
//!
//! | evaluation | [`Decision`] | what a runner does |
//! |---|---|---|
//! | `True` | [`Satisfied`](Decision::Satisfied) | use this candidate |
//! | `False` | [`Refuted`](Decision::Refuted) | try another candidate |
//! | `Unknown` | [`Unevaluable`](Decision::Unevaluable) | **stop**, and report the predicate and the reason |
//!
//! [`Unevaluable`] carries at least one [`UnknownCause`], each naming the leaf that could not be
//! decided and [why](Reason). Five of the six reasons are properties of the specification that no
//! candidate value can change; [`Reason::fixable_by_another_candidate`] is the one that says which.
//!
//! # Synthesis: from a specification to the suite it obliges
//!
//! [`synthesize()`] is the third part, and it is where the first two meet. It walks an
//! [`EssIr`](ess_compiler::EssIr), asks [`witness`] for candidate inputs, asks [`decision`] whether
//! one reaches a branch, and writes a [`scenario`] for every declared outcome it can reach.
//!
//! It returns a suite **and** typed refusals, never a silent omission — design §36. A construct
//! that cannot be witnessed appears in the output saying which construct, why, and what would have
//! to change, because a suite quietly holding fewer checks than the specification requires is the
//! one failure a passing run cannot show.
//!
//! # What is deliberately not here
//!
//! * **A constraint solver.** §11 names one as a later extension and not a requirement of the first
//!   closed loop. [`witness`] tries the literals the guard itself writes, and refuses.
//! * **Branch selection by inspection.** [`when`] returns the predicate an outcome declares, and
//!   which branch a candidate reaches is read off `ResolvedOutcome::test_strategy`, never derived a
//!   second time here.
//! * **A runner.** [`scenario`] defines what a suite *is*; executing one against a target is a
//!   later slice, and mixing the two is how a runner becomes the definition.
//! * **A clock.** The crate that will hold one is this one; the part of it that needs one does not
//!   exist yet.

pub mod decision;
pub mod input;
pub mod scenario;
pub mod synthesize;
pub mod witness;

pub use decision::{when, Decision, Reason, Unevaluable, UnknownCause};
pub use input::{flatten, InputFacts, ShapeError, ShapeErrors};
pub use scenario::{
    ConformanceScenario, ConformanceSuite, EssSemanticRef, ScenarioId, ScenarioPurpose,
    ScenarioStep, SuiteProvenance,
};
pub use synthesize::{synthesize, InstanceNeed, Refusal, RefusalCause, Synthesis};
pub use witness::WitnessGap;
