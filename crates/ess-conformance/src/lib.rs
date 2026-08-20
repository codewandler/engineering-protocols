//! Deciding whether a candidate command input reaches a specification's guard.
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
//! # What is deliberately not here
//!
//! * **Candidate generation.** This crate decides a candidate it is given. Producing one is wave 4's
//!   job, and generate-and-filter versus a constraint solver is a decision nobody has taken.
//! * **Branch selection.** [`when`] returns the predicate an outcome declares, and nothing here says
//!   *which* outcome a candidate reaches: `otherwise` is defined relative to every other branch of
//!   the same command, and `external` is by construction not decidable from an input at all.
//! * **A clock.** The crate that will hold one is this one; the part of it that needs one does not
//!   exist yet.

pub mod decision;
pub mod input;

pub use decision::{when, Decision, Reason, Unevaluable, UnknownCause};
pub use input::{flatten, InputFacts, ShapeError, ShapeErrors};
