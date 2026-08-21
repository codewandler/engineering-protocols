//! What moved between two revisions of one executable system specification.
//!
//! Two compiled [`EssIr`](ess_compiler::ir::EssIr)s go in and an [`EssDelta`] comes out: a typed,
//! canonically ordered list of the semantic differences between them, or a [`DiffRefusal`] saying
//! why the pair cannot be compared at all.
//!
//! ```text
//! ESS source, revision A          ESS source, revision B
//!         |                                |
//!    validate + compile              validate + compile
//!         |                                |
//!      EssIr A                          EssIr B
//!            \                          /
//!             +---------- diff --------+
//!                          |
//!                       EssDelta
//! ```
//!
//! # Why the IR, and not the source or the projections
//!
//! Design §4. A text diff of two specification directories answers "which bytes moved", which is a
//! question about an editor. Moving one domain's events into a second file and reflowing every
//! comment is two hundred lines of `git diff` and **no change to the system** — and the pair under
//! `examples/revision-pair/` is exactly that claim made checkable, because the `after` revision is
//! reordered and re-commented throughout and the delta still holds four changes.
//!
//! Diffing the *projections* is the other wrong answer: an `OpenAPI` document is one rendering of the
//! model, so a change to the renderer looks like a change to the system, and a semantic change that
//! this renderer happens not to project looks like nothing at all.
//!
//! # Ten construct families, and what predicate comparison is allowed to mean
//!
//! Wave 5's first slice compared **system, types, events, errors, actors and components** — the six
//! whose comparison is a walk over values that are equal or are not — and excluded entities,
//! commands, views and bindings because their invariants, guards and filters are
//! [`Predicate`](aep_domain::predicate::Predicate)s. W7.2 brought the four in by executing gap
//! register D-1: the wave-5 record conflated two questions, and only one of them is hard.
//!
//! Predicate **implication** — does the new `when:` accept everything the old one did — is
//! undecidable in general and stays refused: "is `amount > 0` weaker than `amount >= 1`" is a proof
//! obligation, not a field comparison, and it is why there is no `Unknown` in [`SemanticRelation`].
//! Predicate **equality after canonicalisation** is decidable and cheap, and it is all a delta
//! needs. The canonical form is the parsed `Predicate` itself, exactly as the compiler resolves it:
//! the parser's own simplifications applied (`not not p` is `p`, an empty `all` is *always*, a
//! singleton `all`/`any` is its child), and nothing else — no reordering of `all`/`any` children,
//! no algebraic rewriting. Canonically equal is silence; canonically different is
//! [`Changed`](SemanticRelation::Changed) with no direction, and the closure invalidates through it
//! like through any other change. A rewritten-but-equivalent predicate this cannot recognise
//! reports as *changed* and costs a re-run, which is the cheap error; the expensive one —
//! recognising too much — has no code path, because nothing reads a predicate beyond `==`.
//!
//! Where the model keeps the author's own statement beside the parsed predicate (an entity's or a
//! type's invariants), the statement is part of the canonical form too: a documentation projection
//! quotes it, so a reworded statement over an unchanged predicate is a model that moved.
//!
//! What still has no family — conversions, workloads, and each domain's naming — stays behind the
//! fail-closed equality check in [`mod@crate::impact`] (mechanism 6), owed whole when it moves.
//!
//! # Four relations, and everything else is `Changed`
//!
//! | what happened | relation | decided in |
//! |---|---|---|
//! | an actor gains a command it may invoke | [`Expanded`](SemanticRelation::Expanded) | [`ActorChange::relation`] |
//! | an actor loses one | [`Narrowed`](SemanticRelation::Narrowed) | [`ActorChange::relation`] |
//! | an enum or union gains a variant | [`Expanded`](SemanticRelation::Expanded) | [`TypeChange::relation`] |
//! | one loses a variant | [`Narrowed`](SemanticRelation::Narrowed) | [`TypeChange::relation`] |
//! | anything else | [`Changed`](SemanticRelation::Changed) | the family's `relation` |
//!
//! Design §21 lists seven relations. Four of them — `Equivalent`, `Strengthened`, `Weakened`,
//! `Unknown` — are not declared here, because a variant nothing can produce is a refusal that cannot
//! fire, and this repository has spent a day on that defect class already
//! (`docs/reviews/2026-08-20-guard-efficacy-review.md`). They arrive with the constructs that need
//! them.
//!
//! # One refusal
//!
//! [`DiffRefusal::DifferentSystem`]. Comparing two revisions of one system is what this answers;
//! comparing `billing` with `ordering` is a different feature and is refused rather than smuggled
//! in. Design §5's other three preconditions are not refusals here: two of them are guaranteed by
//! the types (an `EssIr` exists only because it compiled), and the third names an IR format version
//! the IR does not have.
//!
//! # Handles never cross
//!
//! Every [`EssIr`](ess_compiler::ir::EssIr) handle is valid only inside the compilation that minted
//! it, and using one against another **panics by design** — "a handle belongs to the IR that minted
//! it". For every consumer so far that has been an edge case. For a diff engine it is the normal
//! case, because holding two IRs is the entire job.
//!
//! So no source file in this crate ever calls an `EssIr` handle accessor. Every comparison walks the
//! name-keyed maps directly and resolves a handle through
//! [`name()`](ess_compiler::ir::TypeHandle::name) into an
//! [`EssSemanticRef`](ess_conformance::scenario::EssSemanticRef). That is a discipline, so it is
//! read for rather than trusted: `tests/canonical.rs` scans this crate's sources for the accessor
//! names and fails if one appears.
//!
//! # And then what it invalidates
//!
//! A delta says what moved; [`impact()`] says what stands on what moved. It builds a
//! [`SemanticDependencyGraph`] over both revisions' IRs, runs a closure backwards from each change,
//! and intersects the result with what a wave-4 [`ConformanceSuite`](ess_conformance::ConformanceSuite)
//! records each of its scenarios as depending on. Every impact carries the path that explains it —
//! design §24's requirement, because an impact nobody can explain is an impact nobody will act on.
//!
//! **It fails closed.** A closure may narrow what has to be re-established and may never say a
//! scenario survived: [`Invalidation`] has no vocabulary for a survival, its only combinator is a
//! join whose top is "the whole suite", and a change the graph cannot seed a closure at owes
//! everything. [`mod@crate::impact`] lists the six mechanisms and what each one forecloses.
//!
//! # Determinism
//!
//! Design §59. Same pair in, byte-identical bytes out: [`BTreeMap`](std::collections::BTreeMap) and
//! [`BTreeSet`](std::collections::BTreeSet) only, no clock, no RNG, canonical serialisation,
//! trailing newline, and a change ordering that is a format contract rather than an accident of
//! iteration. Sentences like that are worth nothing unasserted, so `tests/canonical.rs` diffs the
//! same pair twice and compares bytes, and reads this crate's own sources for the tokens that would
//! break the claim.

pub mod change;
pub mod delta;
pub mod diff;
pub mod graph;
pub mod impact;
pub mod raw;
pub mod render;

pub use change::{
    ActorChange, BindingChange, ChangeCategory, ChangeId, CommandChange, ComponentChange,
    EntityChange, ErrorChange, EventChange, SemanticChange, SemanticRelation, SystemChange,
    TypeChange, ViewChange,
};
pub use delta::{DeltaFormat, EssDelta, EssRevisionRef, SUPPORTED_DELTA_FORMATS};
pub use diff::{diff, DiffRefusal};
pub use graph::{DependencyEdge, DependencyRelation, ImpactClass, Reach, SemanticDependencyGraph};
pub use impact::{
    impact, ArtifactAnswer, ArtifactId, ArtifactObligation, Churn, EssImpact, GeneratedTree,
    ImpactPath, ImpactRefusal, Invalidation, ScenarioImpact, WholeAnswer, IMPACT_FORMAT,
};
pub use raw::RawEssDelta;
