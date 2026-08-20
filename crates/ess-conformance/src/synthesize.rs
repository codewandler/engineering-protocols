//! Turning a resolved specification into the suite it obliges an implementation to pass.
//!
//! Design §10 through §20, and the contract §36 draws around them: this is derivation from an
//! [`EssIr`], like every projection in `ess-gen`, and it is the one derivation that may **refuse**.
//!
//! ```text
//! EssIr ──► ConformanceSuite   what an implementation must satisfy
//!       └─► Vec<Refusal>       what this specification does not yet say enough for
//! ```
//!
//! # A refusal is a result, not a defect
//!
//! [`synthesize`] returns both, always, and the reason is §36's: a suite that quietly holds fewer
//! checks than the specification requires is the "generated tests are green" failure the whole
//! milestone exists to rule out, and unlike a refusal, nothing about it is visible in a passing run.
//! So every construct that gets no scenario appears in [`Synthesis::refusals`] saying which
//! construct, why, and what would have to change — the shape §28 uses for a required semantic a
//! target cannot expose, applied one stage earlier.
//!
//! # Three decisions this module does not take
//!
//! Each was taken once, in the model, and asking again here is the regression the design names by
//! name: *a decision made per projection is a decision made wrong eventually*.
//!
//! | question | answered by | never by |
//! |---|---|---|
//! | can an input reach this branch? | [`ResolvedOutcome::test_strategy`] | re-inspecting the predicate |
//! | `expect` or `eventually`? | [`ResolvedView::assertion_style`] | re-reading the consistency word |
//! | does this candidate satisfy the guard? | [`InputFacts::decide`](crate::InputFacts::decide) | a second evaluator |
//!
//! # `Unknown` refuses
//!
//! §11, and invariant 5 read from the generator's side. A candidate whose guard evaluates to
//! `True` is kept and one that evaluates to `False` is discarded, but `Unknown` ends the search:
//! five of its six causes are properties of the specification that no value can change, so retrying
//! spends the whole budget on a specification defect and then reports it as a flaky test.
//!
//! # What is asserted, and what is deliberately not
//!
//! An outcome scenario asserts the branch, the declared error, every event the branch emits — with
//! the payload **shape** the specification declares for it — and, first class per §10, every event
//! the specification declares that this branch does **not** emit.
//!
//! Where the line falls is worth stating, because it is the model's line and not a budget:
//!
//! | about an event's payload | asserted | why |
//! |---|---|---|
//! | the declared fields are present | yes | the event declares them |
//! | each holds a value of its declared type | yes | the event declares that too |
//! | a field carries a particular *value* | **no** | nothing in the model relates a command's input to a payload field |
//! | no undeclared field is present | **no** | nothing in the model closes an event's payload |
//!
//! `InvoiceCreated.amount == CreateInvoice.amount` reads like a reading and is a match on a shared
//! field name; a specification that called the input `total` would make the same suite wrong. §10's
//! own worked suite asserts `→ InvalidAmount` and no field, and this produces exactly that. When the
//! model gains a way to say where a payload field comes from — the shape a binding's `mapping:`
//! already has for a command input — the values follow from *that* rather than from a name match.
//! The same holds of a declared error's fields, which are asserted by name and never by value.
//!
//! # A lifecycle scenario is a sequence over one instance
//!
//! §19's two classes — the move that must be possible, and the move that must not — are sequences:
//! bring an instance into existence, drive it to the state in question, then act. Every step after
//! the first has to say *which* instance, and until an outcome declared
//! [`instance:`](ess_domain::command::Subject::instance) nothing in the model could. Both classes
//! were refused wholesale; both are synthesised now.
//!
//! What makes it honest rather than convenient is where the identity comes from. Nothing here
//! fabricates one: the arrangement runs the outcome the specification says *creates* the entity, and
//! binds the identity out of the event that outcome declares publishes it — through
//! [`ScenarioStep::CaptureInstance`] — so the value is the target's, and the suite carries a
//! reference to it rather than a guess at it.
//!
//! # A binding is four claims, and each one fails on its own
//!
//! §16 through §18. *When this event occurs, invoke this command, with this mapping, delivered this
//! way, and on failure do this* — so a binding produces four scenarios rather than one, under the
//! four [`BindingAspect`] keys, and each says what it can prove from
//! the declared semantics and nothing more:
//!
//! | aspect | what it requires | why that and not more |
//! |---|---|---|
//! | `flow` | the event happens, and the invoked command's branch publishes what it declares | §16: prove the flow through the resulting event, never by observing the internal command |
//! | `mapping` | each input receives the value the binding names for it | the only clause a document can get *silently* wrong — see [`ScenarioStep::ExpectInvocation`] |
//! | `delivery` | the same event delivered twice still leaves the consequence observable | §17: `at_least_once` permits duplicates, so a count is a test that fails a correct target |
//! | `on-failure` | the declared policy is observable, with the failure forced | §18: force it, and assert what the model says follows |
//!
//! `on_failure: drop` is the one that produces no scenario at all, and that is the model being
//! honest rather than this crate being short: `drop` means the work is lost and nobody is told, so
//! there is nothing to observe. §18 names the response — refuse the check rather than invent one —
//! and [`BindingGap::PolicySilent`] is it.
//!
//! # An invariant is asserted where a view publishes what it reads
//!
//! §20, at the level it asks for: *evaluate invariants after successful state-changing commands,
//! against observable entity or view state, where witnesses are available*. So one scenario per
//! entity per state-changing branch, running that branch and then requiring that every row of a view
//! satisfies the entity's invariants — [`ViewExpectation::Satisfies`], carrying the predicate the
//! specification wrote.
//!
//! Where the witness is missing, it is missing *by construction*: an entity's invariant reads the
//! entity's fields and a view publishes only what it declares, so an invariant over a field no view
//! publishes is unobservable no matter how good the runner is. That is a fact about the
//! specification, and [`RefusalCause::InvariantUnobservable`] says so with the paths in hand.
//!
//! # A wrong-state refusal is asserted where the command declares one
//!
//! §19 asks for two things of an illegal move, and until `wrong_state:` existed the model could
//! express only one. "Must not reach `Cancelled`" was asserted; "the exact rejection mechanism must
//! come from the declared command/error semantics" was not, because a command had no way to say what
//! it answers when its subject is in a state its transitions do not run from. An implementation that
//! refused with the wrong error, or with an untyped infrastructure failure, passed.
//!
//! It can say so now, and it says exactly one thing:
//! [`OutcomeCondition::WrongState`](ess_domain::command::OutcomeCondition::WrongState) names the
//! `error:` and nothing else. The **states** stay where they were already declared — a transition's
//! `from` set — and [`EssIr::wrong_states`] does the subtraction, so no author writes an absence down
//! and no projection re-derives it. Where a command declares the branch, an illegal-move scenario
//! requires it *and* the declared error; where it does not,
//! [`RefusalCause::RefusalUndeclared`] is recorded beside the scenario, which is the same
//! arrangement as before for the specifications that have not adopted the construct.
//!
//! # What the model cannot say yet, and what is therefore not asserted
//!
//! One gap left, reported rather than left to be noticed:
//!
//! | gap | what is asserted instead | what would close it |
//! |---|---|---|
//! | where a payload field's **value** comes from | the field is present and of its declared type | a construct relating an outcome's emitted event to the command's input, in the shape a binding's `mapping:` already has |
//!
//! It is a refusal or a recorded matrix row rather than a silence, because a suite quietly holding a
//! thinner check than the specification requires is the one failure a passing run cannot show.
//!
//! A value object's invariants are the third: `billing.invoice.Money` says `amount >= 0` of every
//! `Money` in the system, which is a claim about a *type* rather than about an instance at rest, and
//! rebasing it onto every entity field that reaches that type is a walk this slice does not do. That
//! one is a gap in this crate rather than in the model, and it is
//! [`RefusalCause::NotSynthesisedYet`].

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;

use aep_domain::facts::{FactPath, FactSource, FactStore, FactValue};
use aep_domain::node::Node;
use aep_domain::predicate::{Predicate, Truth};
use ess_compiler::diagnostic::Code;
use ess_compiler::ir::{
    Driver, EntityHandle, EssIr, ResolvedBinding, ResolvedBody, ResolvedCommand, ResolvedCondition,
    ResolvedEffect, ResolvedFailure, ResolvedInstance, ResolvedMappingValue, ResolvedOutcome,
    ResolvedSubject, ResolvedTypeRef, ResolvedView,
};
use ess_domain::binding::Delivery;
use ess_domain::command::{OutcomeName, TestStrategy};
use ess_domain::entity::{EntitySpec, Invariant, StateName};
use ess_domain::name::QualifiedName;
use ess_domain::types::MAX_TYPE_DEPTH;
use ess_domain::view::AssertionStyle;

use crate::decision::{when, Decision, Unevaluable};
use crate::input::{flatten, resolve_path, ShapeErrors};
use crate::scenario::{
    ActorRef, BindingAspect, BindingRef, CommandRef, ComponentRef, ConformanceScenario,
    ConformanceSuite, DeclaredTypeRef, EntityRef, ErrorRef, EssSemanticRef, EventRef, Holds,
    InstanceName, LeafShape, OutcomeRef, PayloadShape, ScenarioId, ScenarioPurpose, ScenarioStep,
    ScenarioValue, SuiteProvenance, TransitionRef, ViewExpectation, ViewRef,
};
use crate::witness::{candidates, WitnessGap, MAX_CANDIDATES};

/// Everything one specification obliges an implementation to pass, and everything it does not say
/// enough for.
///
/// Both halves, in one value, because they are one answer. A caller that wants only the suite is a
/// caller that has decided refusals do not matter, and making that take a second line of code is
/// the point.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Synthesis {
    /// The scenarios that could be synthesised.
    pub suite: ConformanceSuite,
    /// Every construct that could not, in the order the model declares them.
    pub refusals: Vec<Refusal>,
}

impl Synthesis {
    /// `true` when every construct of the specification produced a scenario.
    pub fn is_complete(&self) -> bool {
        self.refusals.is_empty()
    }

    /// Every refusal carrying this code.
    pub fn refused(&self, code: Code) -> impl Iterator<Item = &Refusal> {
        self.refusals
            .iter()
            .filter(move |refusal| refusal.code() == code)
    }
}

/// One construct that got no scenario, and why.
///
/// The shape §36 asks for — "a stable code, a structured body, and the ESS element that caused it",
/// the same shape `ess-compiler` uses for a bad document, because a coding agent consumes both as
/// repair instructions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    /// The ESS element that has no scenario.
    pub subject: EssSemanticRef,
    /// The scenario that would have existed, where the refusal is about one.
    ///
    /// `None` for a construct whose scenario has no single id — a binding has two aspects, and
    /// neither is synthesised yet.
    pub scenario: Option<ScenarioId>,
    /// Why, as fields rather than as a sentence.
    pub cause: RefusalCause,
}

impl Refusal {
    /// A refusal about the scenario that would have carried `id`.
    fn about(id: &ScenarioId, cause: RefusalCause) -> Self {
        Self {
            subject: subject_of(id),
            scenario: Some(id.clone()),
            cause,
        }
    }

    /// Its stable code.
    pub fn code(&self) -> Code {
        self.cause.code()
    }

    /// What would have to change for this construct to be testable.
    pub fn hint(&self) -> &'static str {
        self.cause.hint()
    }
}

impl fmt::Display for Refusal {
    /// Names the scenario that is missing, not only the construct it is about.
    ///
    /// The id is what a fault matrix and a stored report key on, and it is the only thing that
    /// tells two refusals about one entity apart: `Invoice/state/Paid/refuses/CancelInvoice` and
    /// `Invoice/state/Paid/refuses/IssueInvoice` share a subject and are different checks.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.scenario {
            Some(id) => writeln!(
                f,
                "refusal[{}]: {} has no scenario `{id}`",
                self.code(),
                self.subject
            ),
            None => writeln!(
                f,
                "refusal[{}]: {} has no scenario",
                self.code(),
                self.subject
            ),
        }?;
        for line in self.cause.to_string().lines() {
            writeln!(f, "  {line}")?;
        }
        write!(f, "  help: {}", self.hint())
    }
}

/// Why one construct got no scenario.
///
/// Five outcomes of synthesis, one statement about this build, and three drift alarms. The split
/// matters to whoever reads a refusal: the first five are answered by editing the specification,
/// the sixth by a later slice of this crate, and the last three cannot happen — they are here so
/// that if they ever do, they arrive as a named refusal rather than as a scenario nobody wrote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefusalCause {
    /// No value of the command's declared input type could be constructed at all.
    NoWitness(WitnessGap),
    /// The guard could not be decided against a candidate, and no other candidate would change it.
    ///
    /// §11's `Unknown`, carrying every leaf that could not be decided and
    /// [why](crate::Reason) — which is the difference between "supply the value" and
    /// "fix the specification".
    GuardUnevaluable(Unevaluable),
    /// Every candidate this synthesizer knows how to try was refuted.
    ///
    /// A valid predicate that is not constructively satisfiable by generate-and-check. §11 names
    /// the repair — a constraint solver — as a later extension, so this is a refusal rather than a
    /// longer search.
    GuardUnsatisfiable {
        /// What had to hold, as it reads.
        predicate: String,
        /// How many candidates were decided against it.
        tried: usize,
    },
    /// The scenario needed an instance of an entity, and the specification cannot arrange one.
    ///
    /// The model now says which field names an instance, so "the invoice the previous step created"
    /// is expressible and §19's scenarios are synthesised. What is left here is the case where the
    /// specification cannot *produce* the instance the scenario needs: nothing creates the entity at
    /// all, no sequence of declared moves reaches the state, or a command on the only route has no
    /// witness. Each is a property of the specification, and each is reported rather than papered
    /// over with a fabricated identity — which would be a test that fails a correct implementation.
    InstanceRequired {
        /// Whose instance.
        entity: EntityRef,
        /// What the scenario would have needed of it.
        need: InstanceNeed,
        /// Why the specification cannot produce one.
        reason: Unreachable,
    },
    /// A view's filter could not be decided against the state the scenario reaches.
    ///
    /// A synthesised scenario knows one fact about the entity it created: the state its lifecycle
    /// starts in. A filter reading anything else is undecidable here for the same reason a guard
    /// over an unbound path is, and asserting the view either way would be a guess.
    ViewUndecidable {
        /// Which view.
        view: ViewRef,
        /// Its filter, as it reads.
        filter: String,
        /// The state the entity is in when the assertion would run.
        state: StateName,
        /// The paths the filter reads that nothing binds.
        unbound: Vec<FactPath>,
    },
    /// A construct this build does not synthesise yet.
    ///
    /// Visible rather than silent, deliberately. A reader of a suite cannot tell an unimplemented
    /// slice from a specification with nothing to check, and §36 rules that ambiguity out for
    /// refusals; the same reasoning applies to a gap this crate has not closed.
    NotSynthesisedYet {
        /// What is missing.
        construct: &'static str,
        /// Where it is specified.
        sections: &'static str,
    },
    /// A binding clause with nothing a scenario could observe.
    ///
    /// Not one reason but six, because the repairs differ: one of them is a policy that publishes
    /// nothing *on purpose* (§18), and the others are shapes a specification can be edited out of.
    BindingUnobservable {
        /// Which binding.
        binding: BindingRef,
        /// Why the clause has no witness.
        gap: BindingGap,
    },
    /// An entity invariant nothing observable reads.
    ///
    /// §20 evaluates invariants "against observable entity/view state where possible", and this is
    /// where that qualifier bites. An entity's invariant reads the entity's **fields**; a view
    /// publishes only what it **declares**. `weight_grams >= 0` on an entity whose views publish
    /// `order_id` and `contact` is therefore unobservable by construction — no runner and no witness
    /// generator can close it, and asserting it against something else would be asserting a
    /// different claim.
    InvariantUnobservable {
        /// Whose invariant.
        entity: EntityRef,
        /// The condition, as the author wrote it.
        invariant: String,
        /// The paths it reads that no view of the entity publishes.
        ///
        /// Empty when every path *is* published and the problem is the other one: no view holds an
        /// instance in the state the scenario reaches, so there would be no row to read.
        unpublished: Vec<FactPath>,
        /// The state the entity is in when the assertion would run.
        state: StateName,
    },
    /// §19's rejection mechanism, which this command does not declare.
    ///
    /// A command attempted against an instance in a state none of its transitions run from **is**
    /// refused — that is exactly what makes the combination illegal — and this command declares no
    /// outcome and no error for it. So the scenario that exists asserts what it can, that nothing
    /// the specification declares was published, and cannot assert what §19 asks for: "the exact
    /// rejection mechanism must come from the declared command/error semantics", and "do not
    /// generate vague *operation fails* tests if the domain declares a specific error".
    ///
    /// **The model can express it now**, which is what changed: a `wrong_state:` outcome names the
    /// error the command reports, and the states it answers in stay implied by the transitions it
    /// does not run from. So this is no longer a gap in the model — it is a specification that has
    /// not said what its command does, and the repair is one branch in the document.
    ///
    /// It is a refusal beside a scenario rather than instead of one, exactly as
    /// [`InvariantUnobservable`](Self::InvariantUnobservable) is: the scenario is worth having, and
    /// a reader who cannot see that it asserts less than the section asks for will read a thin check
    /// as a thick one.
    RefusalUndeclared {
        /// Whose instance.
        entity: EntityRef,
        /// The state it is resting in when the command arrives.
        state: StateName,
        /// The command that is not honoured there.
        command: CommandRef,
    },
    /// Two scenarios claimed one id. A drift alarm: `ess-domain` refuses a duplicated declaration.
    DuplicateScenario,
    /// The outcome's strategy and its condition disagree about how a scenario reaches the branch.
    ///
    /// A drift alarm. [`TestStrategy`] is computed from the condition, so the two cannot disagree
    /// unless one of them changes without the other. Two shapes reach it: a
    /// [`ConstructInput`](TestStrategy::ConstructInput) branch that declares no guard, and an
    /// [`ArrangeState`](TestStrategy::ArrangeState) branch asked for the input that reaches it —
    /// nothing reaches that one by choosing an input, and the illegal-move family sends the input
    /// the *moving* branch would have taken.
    StrategyWithoutGuard {
        /// What the strategy said.
        strategy: TestStrategy,
    },
    /// A witness this synthesizer built is not a value of the input's declared type.
    ///
    /// A drift alarm, and the one that matters most: it means the witness walk and the flattener's
    /// walk have come to disagree about what a type accepts, which would otherwise surface as a
    /// guard that mysteriously cannot be decided.
    WitnessRejected(ShapeErrors),
}

impl RefusalCause {
    /// The family every refusal here belongs to.
    const FAMILY: &'static str = "SYNTH";

    /// Its stable code.
    ///
    /// Derived from the variant rather than stored beside it, so a code cannot come to name a body
    /// other than its own.
    pub fn code(&self) -> Code {
        Code::new(
            Self::FAMILY,
            match self {
                Self::NoWitness(_) => 1,
                Self::GuardUnevaluable(_) => 2,
                Self::GuardUnsatisfiable { .. } => 3,
                Self::InstanceRequired { .. } => 4,
                Self::ViewUndecidable { .. } => 5,
                Self::NotSynthesisedYet { .. } => 6,
                Self::DuplicateScenario => 7,
                Self::StrategyWithoutGuard { .. } => 8,
                Self::WitnessRejected(_) => 9,
                Self::BindingUnobservable { .. } => 10,
                Self::InvariantUnobservable { .. } => 11,
                Self::RefusalUndeclared { .. } => 12,
            },
        )
    }

    /// What would have to change for the construct to be testable.
    pub fn hint(&self) -> &'static str {
        match self {
            Self::NoWitness(_) => {
                "give the field a type that has a finite value, or drop it from the command's input"
            }
            Self::GuardUnevaluable(_) => {
                "the guard reads something no input can supply; correct the path or the type it \
                 walks into"
            }
            Self::GuardUnsatisfiable { .. } => {
                "write the branch's condition over values a candidate can carry, or supply a \
                 fixture for it"
            }
            Self::InstanceRequired { reason, .. } => reason.hint(),
            Self::ViewUndecidable { .. } => {
                "filter the view on the entity's state, which is what a generated scenario knows \
                 after the command it ran"
            }
            Self::NotSynthesisedYet { .. } => "a later slice of `ess-conformance` synthesises this",
            Self::BindingUnobservable { gap, .. } => gap.hint(),
            Self::InvariantUnobservable { unpublished, .. } => {
                if unpublished.is_empty() {
                    "declare a view that holds an instance in this state, or the invariant cannot \
                     be read after this branch"
                } else {
                    "publish the fields the invariant reads in a view of this entity, or state the \
                     invariant over what one already publishes"
                }
            }
            Self::RefusalUndeclared { .. } => {
                "give the command a `wrong_state:` outcome naming the error it reports; the states \
                 it answers in are already declared, as the states its transitions do not run from"
            }
            Self::DuplicateScenario => {
                "two declarations produced one scenario id; rename one of them"
            }
            Self::StrategyWithoutGuard { .. } => {
                "`TestStrategy` and `OutcomeCondition` have drifted apart in `ess-domain`"
            }
            Self::WitnessRejected(_) => {
                "the witness walk and the flattener disagree about this type; they read one table"
            }
        }
    }
}

impl fmt::Display for RefusalCause {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoWitness(gap) => write!(f, "no witness: {gap}"),
            Self::GuardUnevaluable(refusal) => write!(f, "{refusal}"),
            Self::GuardUnsatisfiable { predicate, tried } => write!(
                f,
                "no candidate of the {tried} tried satisfies `{predicate}`"
            ),
            Self::InstanceRequired {
                entity,
                need,
                reason,
            } => write!(f, "it needs an instance of `{entity}` {need}, and {reason}"),
            Self::ViewUndecidable {
                view,
                filter,
                state,
                unbound,
            } => {
                write!(
                    f,
                    "`{view}` filters on `{filter}`, which is undecided for an entity in `{state}`"
                )?;
                for path in unbound {
                    write!(f, "\n  - `{path}` is bound by nothing a scenario knows")?;
                }
                Ok(())
            }
            Self::NotSynthesisedYet {
                construct,
                sections,
            } => write!(
                f,
                "{construct} is specified in {sections} and not synthesised yet"
            ),
            Self::RefusalUndeclared {
                entity,
                state,
                command,
            } => write!(
                f,
                "`{command}` on a `{entity}` in `{state}` is refused and the specification does \
                 not say how: no `wrong_state:` outcome and no declared error, so the scenario can \
                 only require that nothing happened"
            ),
            Self::DuplicateScenario => f.write_str("a second scenario claimed this id"),
            Self::StrategyWithoutGuard { strategy } => {
                write!(f, "its strategy is `{strategy}` and it declares no guard")
            }
            Self::WitnessRejected(errors) => {
                write!(
                    f,
                    "the witness is not a value of the input's type:\n{errors}"
                )
            }
            Self::BindingUnobservable { binding, gap } => write!(f, "`{binding}` {gap}"),
            Self::InvariantUnobservable {
                entity,
                invariant,
                unpublished,
                state,
            } => {
                if unpublished.is_empty() {
                    write!(
                        f,
                        "`{invariant}` cannot be read after this branch: no view of `{entity}` \
                         holds an instance in `{state}`"
                    )
                } else {
                    write!(
                        f,
                        "`{invariant}` reads what no view of `{entity}` publishes"
                    )?;
                    for path in unpublished {
                        write!(f, "\n  - `{path}` is published by no view of the entity")?;
                    }
                    Ok(())
                }
            }
        }
    }
}

/// Why one clause of a binding has no scenario.
///
/// Six shapes, and the split that matters is between the first five — a specification an author can
/// edit — and [`PolicySilent`](Self::PolicySilent), which is a decision the author already made and
/// the model deliberately gives nothing to observe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BindingGap {
    /// No command outcome emits the event the binding reacts to.
    ///
    /// Legal: an event may arrive from outside the specification, which is why `ess-domain` does not
    /// refuse it. It still means nothing in a scenario can make the binding fire.
    NothingPublishes {
        /// The event nothing emits.
        event: EventRef,
    },
    /// Which branch the invoked command takes depends on the values the event carries.
    ///
    /// A binding fills the command's input from the event, and no generator knows what the upstream
    /// implementation will publish there — so with two branches an input decides between, a scenario
    /// cannot say which one to require. Deciding it by inspecting the guard would be a claim about a
    /// value nobody has.
    BranchUndecided {
        /// The invoked command.
        command: CommandRef,
        /// The branches a scenario cannot choose between, in declaration order.
        branches: Vec<OutcomeName>,
    },
    /// The branch the binding reaches publishes nothing.
    ///
    /// §16 proves a flow through the event the invoked command publishes. A branch that emits none
    /// leaves the flow with no observable consequence at all.
    NothingPublished {
        /// The branch that publishes nothing.
        outcome: OutcomeRef,
    },
    /// The binding fills no input, so there is no mapping to check.
    NothingMapped {
        /// The invoked command.
        command: CommandRef,
    },
    /// No branch of the invoked command can be made to fail.
    ///
    /// A failure policy is only observable once the failure has been forced, and the only branch a
    /// scenario can force is one the specification declares `external:` (§12). Without one, the
    /// declared policy is a word nothing exercises.
    NoForcibleFailure {
        /// The invoked command.
        command: CommandRef,
    },
    /// The declared policy is `drop`, which publishes nothing on purpose.
    ///
    /// §18's rule, and the one refusal in this family that is not a defect: "give up silently" is
    /// the whole content of the word, and `ess-domain` records why an event here would be wrong —
    /// it would make the policy a notification, which is a different decision that already has a
    /// name. So the check is refused rather than invented, and what a reader learns is that this
    /// binding's failure path is *by declaration* unprovable.
    PolicySilent,
}

impl BindingGap {
    /// What would have to change.
    fn hint(&self) -> &'static str {
        match self {
            Self::NothingPublishes { .. } => {
                "give some command outcome `emits:` for this event; a binding on an event nothing \
                 publishes can never fire"
            }
            Self::BranchUndecided { .. } => {
                "leave the invoked command one branch an input does not choose, or declare the \
                 others `external:` so a scenario can force them"
            }
            Self::NothingPublished { .. } => {
                "emit an event from the branch the binding reaches; a flow with no consequence is a \
                 flow nothing can observe"
            }
            Self::NothingMapped { .. } => {
                "map the command's inputs from the event, or drop the binding: an invocation that \
                 carries nothing carries nothing to check"
            }
            Self::NoForcibleFailure { .. } => {
                "declare the branch that fails `external:`, which is what lets a scenario force it"
            }
            Self::PolicySilent => {
                "`drop` is unobservable by design; write `escalate:` with an event if the failure \
                 has to be provable"
            }
        }
    }
}

impl fmt::Display for BindingGap {
    /// Reads as the tail of "`<binding>` …".
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NothingPublishes { event } => {
                write!(f, "reacts to `{event}`, which no outcome emits")
            }
            Self::BranchUndecided { command, branches } => write!(
                f,
                "invokes `{command}`, whose branch is decided by an input the event fills: {}",
                branches
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Self::NothingPublished { outcome } => {
                write!(f, "invokes `{outcome}`, which publishes nothing")
            }
            Self::NothingMapped { command } => {
                write!(f, "fills none of `{command}`'s input")
            }
            Self::NoForcibleFailure { command } => write!(
                f,
                "declares a failure policy, and no branch of `{command}` can be forced to fail"
            ),
            Self::PolicySilent => {
                f.write_str("gives up silently, which the model publishes nothing for")
            }
        }
    }
}

/// Why a scenario could not get the instance it needed.
///
/// Three shapes, and the difference is which line of the specification an author goes and edits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unreachable {
    /// No outcome brings an instance of the entity into existence.
    ///
    /// Legal: an entity may arrive from a migration or from a system outside this document, which is
    /// why `ess-domain` does not refuse it. It still means no scenario can act on one.
    NothingCreates,
    /// No sequence of declared, driven transitions reaches the state from where the lifecycle starts.
    ///
    /// A drift alarm. `ess-domain` refuses a state nothing reaches (`unreachable_state`) and a
    /// transition nothing drives (`missing_causation`), so the graph this walk searches and the
    /// graph that check searches are the same one — and a valid specification cannot produce this.
    /// It exists so that if the two ever come apart, the result is a named refusal rather than a
    /// scenario nobody wrote. The reachable shape of "cannot get there" is
    /// [`Unwitnessable`](Self::Unwitnessable).
    NoPath {
        /// Where a new instance begins.
        from: StateName,
    },
    /// A command on the route to the state has no input that reaches the branch that moves it.
    ///
    /// The full reason is on that outcome's own refusal; naming it here keeps one cause in one
    /// place rather than restating it under a second id.
    Unwitnessable {
        /// The branch that could not be reached.
        outcome: OutcomeRef,
    },
}

impl Unreachable {
    /// What would have to change.
    fn hint(&self) -> &'static str {
        match self {
            Self::NothingCreates => {
                "give some command outcome `creates:` for this entity; nothing can act on an \
                 instance nothing brings into existence"
            }
            Self::NoPath { .. } => {
                "declare a transition that reaches this state, and an outcome that takes it"
            }
            Self::Unwitnessable { .. } => {
                "the route to this state runs through a branch no input reaches; see that \
                 outcome's own refusal"
            }
        }
    }
}

impl fmt::Display for Unreachable {
    /// Reads as the tail of "it needs an instance of `X` …, and …".
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NothingCreates => f.write_str("no outcome creates one"),
            Self::NoPath { from } => {
                write!(f, "no declared move reaches it from `{from}`")
            }
            Self::Unwitnessable { outcome } => {
                write!(
                    f,
                    "the route runs through `{outcome}`, which no input reaches"
                )
            }
        }
    }
}

/// What a scenario would have needed of an instance that already exists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstanceNeed {
    /// The outcome moves one along a declared transition.
    Moves {
        /// Which move.
        transition: String,
    },
    /// The outcome changes one without moving it.
    Updates,
    /// The scenario needs one resting in a particular state before it acts.
    InState {
        /// Which state.
        state: StateName,
    },
}

impl fmt::Display for InstanceNeed {
    /// Reads as the tail of "it needs an instance of `X` …".
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Moves { transition } => write!(f, "to move along `{transition}`"),
            Self::Updates => f.write_str("to change without moving"),
            Self::InState { state } => write!(f, "resting in `{state}`"),
        }
    }
}

/// Every check one specification obliges an implementation to pass, and every one it cannot.
///
/// Deterministic (§37): the walk is over [`BTreeMap`]s in name order and declaration order, every
/// value it chooses is a function of the model, and nothing here reads a clock or a random device.
/// `tests/synthesis.rs` synthesises the billing example twice and compares bytes.
pub fn synthesize(ir: &EssIr) -> Synthesis {
    let mut suite = ConformanceSuite::new(SuiteProvenance::of(ir));
    let mut refusals = Vec::new();
    let actors = granted_actors(ir);

    for command in ir.commands.values() {
        for outcome in &command.outcomes {
            // A wrong-state branch gets no scenario of its own, and this is the one place in the
            // file where "no scenario" is neither a refusal nor a defect. §10 asks for one scenario
            // per *reachable* outcome, and the states this branch is reachable in are exactly the
            // ones the illegal-move family below already enumerates — one scenario each, against an
            // instance the arrangement really drove there. A ninth `/outcome/` scenario would have
            // had to pick one of those states arbitrarily and would assert a strict subset of what
            // the eight already assert. `wrong_state_is_covered_by_the_illegal_move_family` in
            // `tests/synthesis.rs` is what keeps that a claim rather than a hope.
            if outcome.condition == ResolvedCondition::WrongState {
                continue;
            }
            let Some((id, scenario)) =
                outcome_scenario(ir, command, outcome, &actors, &mut refusals)
            else {
                continue;
            };
            insert(&mut suite, id, scenario, &mut refusals);
        }
    }
    lifecycle(ir, &actors, &mut suite, &mut refusals);
    invariants(ir, &actors, &mut suite, &mut refusals);
    bindings(ir, &actors, &mut suite, &mut refusals);

    Synthesis { suite, refusals }
}

/// One scenario per declared outcome (§10), or the refusal that says why there is none.
fn outcome_scenario(
    ir: &EssIr,
    command: &ResolvedCommand,
    outcome: &ResolvedOutcome,
    actors: &BTreeMap<QualifiedName, ActorRef>,
    refusals: &mut Vec<Refusal>,
) -> Option<(ScenarioId, ConformanceScenario)> {
    let id = ScenarioId::Outcome {
        outcome: OutcomeRef::new(CommandRef::new(command.name.clone()), outcome.name.clone()),
    };
    let (steps, source) = exercise(ir, command, outcome, actors, &id, refusals)?;
    Some((
        id,
        ConformanceScenario::new(purpose(command, outcome), steps, source),
    ))
}

/// Arrange the instance the branch acts on, run the branch, and assert everything it promises.
///
/// The body §10 and §19 share. An outcome scenario and a transition scenario differ in the id they
/// are filed under and in the sentence they print, and not in what they do — §10's unit is the
/// branch and §19's is the move, and a suite that dropped one because it resembled the other would
/// lose the id a fault matrix refers to.
fn exercise(
    ir: &EssIr,
    command: &ResolvedCommand,
    outcome: &ResolvedOutcome,
    actors: &BTreeMap<QualifiedName, ActorRef>,
    id: &ScenarioId,
    refusals: &mut Vec<Refusal>,
) -> Option<(Vec<ScenarioStep>, BTreeSet<EssSemanticRef>)> {
    let run = match run(ir, command, outcome, actors) {
        Ok(run) => run,
        Err(cause) => {
            refusals.push(Refusal::about(id, cause));
            return None;
        }
    };

    let emitted: Vec<EventRef> = outcome.emits.iter().map(EventRef::from).collect();
    let absent = not_emitted(ir, &emitted);
    let actor = run.actor.clone();
    let (view_steps, views) = view_expectations(
        ir,
        outcome,
        run.after.as_ref(),
        run.instance.as_ref(),
        id,
        refusals,
    );

    let mut steps = run.steps();
    if let Some(error) = &outcome.error {
        steps.push(ScenarioStep::ExpectError {
            error: ErrorRef::from(error),
            fields: BTreeMap::new(),
        });
    }
    for event in &emitted {
        steps.push(ScenarioStep::ExpectEvent {
            event: event.clone(),
            payload: BTreeMap::new(),
            shape: payload_shape(ir, event),
        });
    }
    // §10's first-class negative assertion: without it the refusal case passes against an
    // implementation that refuses the command and emits the success event anyway.
    for event in &absent {
        steps.push(ScenarioStep::ExpectNoEvent {
            event: event.clone(),
        });
    }
    steps.extend(view_steps);

    let mut source = dependencies(ir, command, outcome, &absent, actor, &views);
    source.extend(run.source);
    Some((steps, source))
}

/// One branch, arranged and run: everything before the assertions that are particular to a family.
///
/// The three families that execute a command all need the same four things — an instance in the
/// state the branch may be taken from, an input that reaches the branch, the invocation, and the
/// requirement that the declared branch was the one taken — and they differ only in what they assert
/// afterwards. Sharing it is not only economy: an arrangement built two ways is two answers to
/// "which invoice is this scenario about", and the second one is wrong eventually.
///
/// [`Run::steps`] is kept separate from the arrangement so a caller can inject something *between*
/// them — which §18's failure scenario needs, because the control it arms must be armed after the
/// arrangement's own commands have run and before the one that triggers the binding.
struct Run {
    /// The steps that bring the instance into the state the branch needs.
    setup: Vec<ScenarioStep>,
    /// Forcing the branch where it is externally decided, invoking it, and requiring it.
    invoke: Vec<ScenarioStep>,
    /// The state the subject is in afterwards, where there is a subject.
    after: Option<StateName>,
    /// What the arrangement bound the instance as, where it arranged one.
    ///
    /// Carried out of the arrangement rather than dropped there, because a view assertion has to
    /// name the row it is about: "the view holds a row" is only the same claim as "the view holds
    /// *this* invoice" while nothing else is using the target.
    instance: Option<InstanceName>,
    /// The actor the command is invoked as, where the specification grants one.
    actor: Option<ActorRef>,
    /// What the arrangement depends on.
    source: BTreeSet<EssSemanticRef>,
}

impl Run {
    /// The arrangement and the invocation, in order.
    fn steps(&self) -> Vec<ScenarioStep> {
        let mut steps = self.setup.clone();
        steps.extend(self.invoke.iter().cloned());
        steps
    }
}

/// Arranges the instance a branch acts on and invokes it, or says why neither is possible.
fn run(
    ir: &EssIr,
    command: &ResolvedCommand,
    outcome: &ResolvedOutcome,
    actors: &BTreeMap<QualifiedName, ActorRef>,
) -> Result<Run, RefusalCause> {
    let setup = prepare(ir, outcome, actors)?;
    let input = reach(ir, command, outcome)?;

    let command_ref = CommandRef::new(command.name.clone());
    let outcome_ref = OutcomeRef::new(command_ref.clone(), outcome.name.clone());
    let actor = actors.get(&command.name).cloned();

    let mut invoke = Vec::new();
    if outcome.test_strategy == TestStrategy::InjectFault {
        // §12: no predicate over a recipient and a template says whether a provider will accept the
        // mail, so the suite injects the answer rather than inventing an input that produces it.
        invoke.push(ScenarioStep::ConfigureExternalOutcome {
            force: outcome_ref.clone(),
        });
    }
    invoke.push(ScenarioStep::ExecuteCommand {
        command: command_ref,
        actor: actor.clone(),
        input: supply(&input, outcome.subject.as_ref(), setup.instance.as_ref()),
    });
    invoke.push(ScenarioStep::ExpectOutcome {
        outcome: outcome_ref,
    });

    Ok(Run {
        setup: setup.steps,
        invoke,
        after: setup.after,
        instance: setup.instance,
        actor,
        source: setup.source,
    })
}

/// What has to be true before a branch can be run, and what is true of its subject afterwards.
///
/// Empty for a branch that changes no entity, and for one that *creates* its own subject: a created
/// instance is the scenario's own doing, so there is nothing to arrange first.
struct Setup {
    /// The steps that bring the instance into the state the branch needs.
    steps: Vec<ScenarioStep>,
    /// What those steps bound the instance as, where they bound one.
    instance: Option<InstanceName>,
    /// The constructs the arrangement depends on, so a change to one makes a stored result stale.
    source: BTreeSet<EssSemanticRef>,
    /// The state the subject is in once the branch has been taken, where there is a subject.
    after: Option<StateName>,
}

impl Setup {
    /// Nothing to arrange, and no entity to assert about afterwards.
    fn none() -> Self {
        Self {
            steps: Vec::new(),
            instance: None,
            source: BTreeSet::new(),
            after: None,
        }
    }
}

/// The instance one branch needs, brought to the state that branch may be taken from.
///
/// `creates:` needs nothing arranged and lands in the lifecycle's `initial`. `moves:` needs an
/// instance resting in one of the transition's `from` states, and takes the first that the
/// specification can actually reach — trying them in name order rather than picking one, so a
/// transition whose only reachable source is the second one still gets a scenario. `updates:` names
/// no state at all, so the cheapest reachable one is where the lifecycle starts.
fn prepare(
    ir: &EssIr,
    outcome: &ResolvedOutcome,
    actors: &BTreeMap<QualifiedName, ActorRef>,
) -> Result<Setup, RefusalCause> {
    let Some(subject) = &outcome.subject else {
        return Ok(Setup::none());
    };
    let lifecycle = &ir.entity(&subject.entity).lifecycle;
    let (targets, need): (Vec<StateName>, InstanceNeed) = match &subject.effect {
        ResolvedEffect::Creates => {
            return Ok(Setup {
                after: Some(lifecycle.initial.clone()),
                ..Setup::none()
            })
        }
        ResolvedEffect::Moves { transition } => (
            transition.from.iter().cloned().collect(),
            InstanceNeed::Moves {
                transition: transition.name.clone(),
            },
        ),
        ResolvedEffect::Updates => (vec![lifecycle.initial.clone()], InstanceNeed::Updates),
    };
    let after = match &subject.effect {
        ResolvedEffect::Moves { transition } => transition.to.clone(),
        ResolvedEffect::Creates | ResolvedEffect::Updates => lifecycle.initial.clone(),
    };

    let arrangement = arrange_first(ir, &subject.entity, &targets, actors).map_err(|reason| {
        RefusalCause::InstanceRequired {
            entity: EntityRef::from(&subject.entity),
            need,
            reason,
        }
    })?;
    Ok(Setup {
        steps: arrangement.steps,
        instance: Some(arrangement.instance),
        source: arrangement.source,
        after: Some(after),
    })
}

/// One instance of an entity, resting in a state, and the name the scenario calls it by.
struct Arrangement {
    /// What it is called for the rest of the scenario.
    instance: InstanceName,
    /// The steps that produce it.
    steps: Vec<ScenarioStep>,
    /// What those steps depend on.
    source: BTreeSet<EssSemanticRef>,
}

/// The cheapest of several candidate states the specification can actually reach.
///
/// `cancel` may start in `Placed` or in `Held`, and both are legal sources for the same scenario —
/// so the one taken is the one that needs the fewest commands to set up. Every extra command in an
/// arrangement is another way for a scenario to fail for a reason that has nothing to do with what
/// it is testing. Ties go to the lower-named state, and the states arrive in name order, so the
/// choice is a function of the model (§37).
///
/// The error kept is the first one, in name order, so a refusal does not move when an unrelated
/// state is added to the transition's `from` set.
fn arrange_first(
    ir: &EssIr,
    entity: &EntityHandle,
    targets: &[StateName],
    actors: &BTreeMap<QualifiedName, ActorRef>,
) -> Result<Arrangement, Unreachable> {
    let mut cheapest: Option<Arrangement> = None;
    let mut first: Option<Unreachable> = None;
    for target in targets {
        match arrange(ir, entity, target, actors) {
            Ok(arrangement) => {
                if cheapest
                    .as_ref()
                    .is_none_or(|held| arrangement.steps.len() < held.steps.len())
                {
                    cheapest = Some(arrangement);
                }
            }
            Err(reason) => {
                first.get_or_insert(reason);
            }
        }
    }
    cheapest.ok_or(first.unwrap_or(Unreachable::NothingCreates))
}

/// Bring one instance of `entity` into existence and drive it to `target`.
///
/// Two questions the model can now answer and could not before G21: which outcome brings an instance
/// into existence, and — the one this gate closed — **which field carries its identity**, so the
/// steps that follow can name the instance the first step created rather than inventing one.
///
/// The route is the shortest sequence of declared, driven transitions from the lifecycle's `initial`
/// to `target`. Shortest because a scenario is a fixture, not a tour: every extra command is another
/// way for the arrangement to fail for a reason that has nothing to do with what is being tested.
fn arrange(
    ir: &EssIr,
    entity: &EntityHandle,
    target: &StateName,
    actors: &BTreeMap<QualifiedName, ActorRef>,
) -> Result<Arrangement, Unreachable> {
    let all = ir.drivers();
    let drivers: &[Driver<'_>] = all.get(entity).map_or(&[], Vec::as_slice);
    let creator = drivers
        .iter()
        .find(|driver| matches!(driver.effect, ResolvedEffect::Creates))
        .ok_or(Unreachable::NothingCreates)?;
    let route = route(ir, entity, drivers, target).ok_or_else(|| Unreachable::NoPath {
        from: ir.entity(entity).lifecycle.initial.clone(),
    })?;

    let instance = instance_name(&ir.entity(entity).name);
    let mut steps = Vec::new();
    let mut source = BTreeSet::new();

    let (created, used) = invoke(ir, creator, None, actors)?;
    steps.extend(created);
    source.extend(used);
    // Where the identity becomes knowable. `creates:` names a field of an event the branch emits,
    // because the caller could not have named an instance that did not exist when it called — and
    // because §9's command result already carries the events a command emitted, so binding it here
    // asks a target for nothing it was not already going to report.
    let ResolvedInstance::Observed { event, field } = &subject(creator).instance else {
        // `Subject::surface` makes this a function of the verb, and the verb here is `creates`.
        unreachable!("a `creates:` link is observed in an event")
    };
    steps.push(ScenarioStep::CaptureInstance {
        instance: instance.clone(),
        entity: EntityRef::from(entity),
        event: EventRef::from(event),
        field: field.name.clone(),
    });
    source.insert(EventRef::from(event).into());

    for driver in route {
        let (moved, used) = invoke(ir, &driver, Some(&instance), actors)?;
        steps.extend(moved);
        source.extend(used);
    }
    Ok(Arrangement {
        instance,
        steps,
        source,
    })
}

/// One command run as part of an arrangement: reach its branch, and require that it was taken.
///
/// The outcome is asserted rather than assumed, because an arrangement that quietly failed produces
/// a scenario that proves nothing and says it passed — which is the shape of green this whole
/// milestone exists to rule out.
fn invoke(
    ir: &EssIr,
    driver: &Driver<'_>,
    instance: Option<&InstanceName>,
    actors: &BTreeMap<QualifiedName, ActorRef>,
) -> Result<(Vec<ScenarioStep>, BTreeSet<EssSemanticRef>), Unreachable> {
    let command_ref = CommandRef::new(driver.command.name.clone());
    let outcome_ref = OutcomeRef::new(command_ref.clone(), driver.outcome.name.clone());
    // The cause is not carried up. That branch has a refusal of its own, under its own id, saying
    // exactly why no input reaches it; repeating it here would be one defect reported twice with two
    // repairs to weigh.
    let input =
        reach(ir, driver.command, driver.outcome).map_err(|_| Unreachable::Unwitnessable {
            outcome: outcome_ref.clone(),
        })?;

    let mut steps = Vec::new();
    if driver.outcome.test_strategy == TestStrategy::InjectFault {
        steps.push(ScenarioStep::ConfigureExternalOutcome {
            force: outcome_ref.clone(),
        });
    }
    steps.push(ScenarioStep::ExecuteCommand {
        command: command_ref.clone(),
        actor: actors.get(&driver.command.name).cloned(),
        input: supply(&input, driver.outcome.subject.as_ref(), instance),
    });
    steps.push(ScenarioStep::ExpectOutcome {
        outcome: outcome_ref.clone(),
    });

    let source: BTreeSet<EssSemanticRef> = [command_ref.into(), outcome_ref.into()]
        .into_iter()
        .collect();
    Ok((steps, source))
}

/// The shortest sequence of driven transitions from where the lifecycle starts to `target`.
///
/// Breadth-first over the states, with the edges out of each state visited in a fixed order —
/// transition name, then command, then branch — so the route is a function of the model and not of
/// how a map happened to iterate (§37).
fn route<'a>(
    ir: &EssIr,
    entity: &EntityHandle,
    drivers: &[Driver<'a>],
    target: &StateName,
) -> Option<Vec<Driver<'a>>> {
    let lifecycle = &ir.entity(entity).lifecycle;
    if &lifecycle.initial == target {
        return Some(Vec::new());
    }

    let mut edges: BTreeMap<StateName, Vec<(StateName, Driver<'a>)>> = BTreeMap::new();
    for driver in drivers {
        let Some(transition) = driver.effect.transition() else {
            continue;
        };
        for from in &transition.from {
            edges
                .entry(from.clone())
                .or_default()
                .push((transition.to.clone(), *driver));
        }
    }
    for outgoing in edges.values_mut() {
        outgoing.sort_by_key(|(to, driver)| {
            (
                driver
                    .effect
                    .transition()
                    .map(|transition| transition.name.clone())
                    .unwrap_or_default(),
                driver.command.name.to_string(),
                driver.outcome.name.to_string(),
                to.to_string(),
            )
        });
    }

    let mut came: BTreeMap<StateName, (StateName, Driver<'a>)> = BTreeMap::new();
    let mut seen: BTreeSet<StateName> = [lifecycle.initial.clone()].into();
    let mut queue: VecDeque<StateName> = [lifecycle.initial.clone()].into();
    while let Some(state) = queue.pop_front() {
        for (to, driver) in edges.get(&state).map(Vec::as_slice).unwrap_or_default() {
            if !seen.insert(to.clone()) {
                continue;
            }
            came.insert(to.clone(), (state.clone(), *driver));
            if to == target {
                let mut route = Vec::new();
                let mut at = target.clone();
                while let Some((previous, driver)) = came.get(&at) {
                    route.push(*driver);
                    at = previous.clone();
                }
                route.reverse();
                return Some(route);
            }
            queue.push_back(to.clone());
        }
    }
    None
}

/// The subject of a driver's outcome, which a driver always has.
fn subject<'a>(driver: &Driver<'a>) -> &'a ResolvedSubject {
    driver.outcome.subject.as_ref().unwrap_or_else(|| {
        panic!(
            "a driver is an outcome with a subject: {}",
            driver.outcome.name
        )
    })
}

/// What a scenario calls one instance of this entity: its local name, in lower-kebab.
///
/// Derived from the model rather than counted, so two scenarios about one entity use one word and a
/// reader of a suite recognises it.
fn instance_name(entity: &QualifiedName) -> InstanceName {
    let local = entity.local();
    let mut out = String::with_capacity(local.len() + 4);
    for (index, character) in local.char_indices() {
        if character == '_' || character == '-' {
            out.push('-');
        } else if character.is_ascii_uppercase() {
            if index > 0 && !out.ends_with('-') {
                out.push('-');
            }
            out.push(character.to_ascii_lowercase());
        } else {
            out.push(character);
        }
    }
    InstanceName::new(out)
        .unwrap_or_else(|_| InstanceName::new("subject").expect("`subject` is lower-kebab"))
}

/// A witness input, with the field that names the instance replaced by the instance itself.
///
/// The one place a scenario carries a reference rather than a value. Every other field holds what
/// synthesis decided against the branch's guard; this one holds "the instance step one created",
/// because no generator can know an identity a target has not assigned yet.
///
/// A guard may not read this field — `ess-domain` refuses that under `unobservable_fact`, since
/// invariant 13 makes an identity opaque — so replacing it cannot invalidate the decision that chose
/// the rest of the input.
fn supply(
    input: &BTreeMap<String, Node>,
    subject: Option<&ResolvedSubject>,
    instance: Option<&InstanceName>,
) -> BTreeMap<String, ScenarioValue> {
    let named = subject.and_then(|subject| match &subject.instance {
        ResolvedInstance::Supplied { field } => Some(field.name.as_str()),
        ResolvedInstance::Observed { .. } => None,
    });
    input
        .iter()
        .map(|(field, value)| {
            let supplied = match (named, instance) {
                (Some(named), Some(bound)) if named == field => {
                    ScenarioValue::instance(bound.clone())
                }
                _ => ScenarioValue::literal(value.clone()),
            };
            (field.clone(), supplied)
        })
        .collect()
}

/// The input that reaches this branch, decided rather than assumed.
///
/// Branches on [`ResolvedOutcome::test_strategy`] and never on the predicate: the compiler decided
/// reachability once, and asking again here is the divergence the field exists to prevent.
fn reach(
    ir: &EssIr,
    command: &ResolvedCommand,
    outcome: &ResolvedOutcome,
) -> Result<BTreeMap<String, Node>, RefusalCause> {
    let strategy = outcome.test_strategy;
    let guards: Vec<&Predicate> = match strategy {
        TestStrategy::ConstructInput => match when(outcome) {
            Some(guard) => vec![guard],
            None => return Err(RefusalCause::StrategyWithoutGuard { strategy }),
        },
        // The default branch is defined relative to every *other* branch, so what a candidate has
        // to do is refute all of them rather than satisfy anything.
        TestStrategy::DefaultBranch => command
            .outcomes
            .iter()
            .filter(|other| other.name != outcome.name)
            .filter_map(when)
            .collect(),
        TestStrategy::InjectFault => Vec::new(),
        // A wrong-state branch is decided by the subject, not by the input, so nothing asks this
        // function for the input that reaches it: `refused_here` sends the input that reaches the
        // *moving* branch and arranges the subject instead. Answering with "no guards" would hand
        // back an arbitrary candidate presented as the one that reaches the branch, which is the
        // invention this crate refuses everywhere else — so it is a drift alarm.
        TestStrategy::ArrangeState => return Err(RefusalCause::StrategyWithoutGuard { strategy }),
    };
    let satisfy = strategy == TestStrategy::ConstructInput;

    let inputs = candidates(ir, command, &guards).map_err(RefusalCause::NoWitness)?;
    for input in &inputs {
        let facts = flatten(ir, command, input).map_err(RefusalCause::WitnessRejected)?;
        if decides(&facts, &guards, satisfy)? {
            return Ok(input.clone());
        }
    }
    Err(RefusalCause::GuardUnsatisfiable {
        predicate: rendered(&guards, satisfy),
        tried: inputs.len().min(MAX_CANDIDATES),
    })
}

/// `true` when this candidate does what the strategy asks of every guard.
///
/// `Unknown` leaves through the `Err`, and it leaves immediately: five of its six causes are
/// properties of the specification, so the next candidate meets the same wall.
fn decides(
    facts: &crate::InputFacts<'_>,
    guards: &[&Predicate],
    satisfy: bool,
) -> Result<bool, RefusalCause> {
    for guard in guards {
        match facts.decide(guard) {
            Decision::Satisfied => {
                if !satisfy {
                    return Ok(false);
                }
            }
            Decision::Refuted(_) => {
                if satisfy {
                    return Ok(false);
                }
            }
            Decision::Unevaluable(refusal) => return Err(RefusalCause::GuardUnevaluable(refusal)),
        }
    }
    Ok(true)
}

/// What a refused search was looking for, for the diagnostic.
fn rendered(guards: &[&Predicate], satisfy: bool) -> String {
    let written: Vec<String> = guards.iter().map(ToString::to_string).collect();
    if satisfy {
        written.join(" and ")
    } else {
        format!("none of: {}", written.join(", "))
    }
}

/// Every event this branch must not publish: everything the specification declares, minus its own.
///
/// # Why the whole specification and not just the sibling branches
///
/// `ESS-CF-NO-EVENT` names the rule "a branch publishes no event it does not declare it emits", and
/// the sibling set is a narrower claim wearing that sentence: it catches a refusal that announces
/// the success it refused, and lets a branch announce anything declared elsewhere. An invoice
/// created and simultaneously announced as cancelled is a defect no downstream consumer survives,
/// and no sibling of `accepted` emits `InvoiceCancelled`.
///
/// # Why it is narrower than "no other event at all", twice over
///
/// **It is about one invocation, not about the scenario.** The check reads
/// `SemanticCommandResult::direct_events` — what §9 says *this* command published — and nothing
/// else. A scenario legitimately causes other events while it runs: an arrangement creates an
/// invoice before the branch under test, and a binding invokes a downstream command whose own
/// events arrive later. Asserting over everything a scenario observed would fail a correct
/// implementation for doing what the specification told it to.
///
/// **It names only declared events.** An implementation may publish occurrences this specification
/// says nothing about — an audit record, a technical heartbeat — and a suite refusing those would
/// be enforcing a rule no document wrote. The model closes a *branch's* emissions, not the
/// system's output.
fn not_emitted(ir: &EssIr, emitted: &[EventRef]) -> Vec<EventRef> {
    ir.events
        .values()
        .map(|event| EventRef::new(event.name.clone()))
        .filter(|event| !emitted.contains(event))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

/// What the specification declares an event carries, flattened to leaves a runner can check (§13).
///
/// The one payload claim that needs no model change. [`PayloadShape`] argues why the *values* are
/// not here and what the model would have to gain before they could be.
fn payload_shape(ir: &EssIr, event: &EventRef) -> PayloadShape {
    let mut shape = PayloadShape::new();
    // Every `EventRef` a suite carries was minted from this IR's own events, so the lookup finds
    // one; a shape that described nothing would be a silently weaker assertion, which is the one
    // failure mode §36 rules out, so the absence is not quietly tolerated.
    let declared = ir
        .events
        .get(event.name())
        .unwrap_or_else(|| panic!("`{event}` is an event this specification declares"));
    for field in &declared.fields {
        describe(ir, &field.type_ref, &field.name, false, 0, &mut shape);
    }
    shape
}

/// One leaf per scalar the declared type reaches, under the dotted path that names it.
///
/// The same walk [`crate::input`] documents as a table, so a payload and a command input are held to
/// one reading of what `Optional<Money>` exposes. A type that refers to itself contributes nothing
/// past [`MAX_TYPE_DEPTH`]: refusing to describe a leaf is a weaker assertion, never a wrong one.
fn describe(
    ir: &EssIr,
    type_ref: &ResolvedTypeRef,
    path: &str,
    optional: bool,
    depth: usize,
    shape: &mut PayloadShape,
) {
    if depth > MAX_TYPE_DEPTH {
        return;
    }
    let mut leaf = |holds: Holds| {
        let described = LeafShape::required(holds);
        shape.insert(
            path,
            if optional {
                described.optional()
            } else {
                described
            },
        );
    };
    match type_ref {
        // The wrapper marks every leaf underneath absentable, including a struct's fields: a value
        // that is not there does not have fields that are.
        ResolvedTypeRef::Optional { of } => describe(ir, of, path, true, depth + 1, shape),
        ResolvedTypeRef::Primitive { name } => leaf(Holds::Primitive { kind: *name }),
        ResolvedTypeRef::List { .. } => leaf(Holds::List),
        ResolvedTypeRef::Map { .. } => leaf(Holds::Map),
        ResolvedTypeRef::Declared { name } => match &ir.named_type(name).body {
            // Transparent, exactly as it is to a fact path: a newtype names no member, so the path
            // does not grow.
            ResolvedBody::Newtype { of, .. } => describe(ir, of, path, optional, depth + 1, shape),
            ResolvedBody::Enum { variants } => leaf(Holds::Enum {
                variants: variants.clone(),
            }),
            ResolvedBody::Union { .. } => leaf(Holds::Union),
            ResolvedBody::Struct { fields, .. } => {
                for field in fields {
                    describe(
                        ir,
                        &field.type_ref,
                        &format!("{path}.{}", field.name),
                        optional,
                        depth + 1,
                        shape,
                    );
                }
            }
        },
    }
}

/// The view assertions a branch that changed an entity supports (§14, §20).
///
/// `after` is the state the subject is in once the branch has been taken — the lifecycle's `initial`
/// for a `creates:`, the transition's `to` for a `moves:`, and the state it was arranged in for an
/// `updates:`. It is passed in rather than re-derived, because deciding a filter against the wrong
/// state is a wrong assertion rather than a missing one.
///
/// A branch with no subject changes nothing a view could show, and produces no steps.
///
/// `instance` is what the arrangement bound the subject as, and it is what turns "the view holds a
/// row" into "the view holds *this* one" — see [`identifying`].
fn view_expectations(
    ir: &EssIr,
    outcome: &ResolvedOutcome,
    after: Option<&StateName>,
    instance: Option<&InstanceName>,
    id: &ScenarioId,
    refusals: &mut Vec<Refusal>,
) -> (Vec<ScenarioStep>, BTreeSet<ViewRef>) {
    let mut steps = Vec::new();
    let mut named = BTreeSet::new();
    let (Some(subject), Some(state)) = (&outcome.subject, after) else {
        return (steps, named);
    };
    let projections = ir.projections();
    let Some(views) = projections.get(&subject.entity) else {
        return (steps, named);
    };

    for view in views {
        let name = ViewRef::new(view.name.clone());
        let fields = identifying(ir, subject, instance, view);
        let expectation = match shows(view, state) {
            Ok(true) => ViewExpectation::Contains {
                fields: fields.clone(),
            },
            // A cancelled invoice that stays in `OutstandingInvoices` is the defect the positive
            // assertion cannot see, and an entity that has not reached the filtered state yet is
            // exactly that case at the other end.
            Ok(false) => ViewExpectation::Excludes {
                fields: fields.clone(),
            },
            Err(unbound) => {
                refusals.push(Refusal::about(
                    id,
                    RefusalCause::ViewUndecidable {
                        view: name,
                        filter: view
                            .filter
                            .as_ref()
                            .map_or_else(String::new, ToString::to_string),
                        state: state.clone(),
                        unbound,
                    },
                ));
                continue;
            }
        };
        // The style is read, never re-derived: asserting an `eventual` view with `expect` races the
        // projection, and the repair everyone reaches for is a sleep.
        match view.assertion_style {
            AssertionStyle::Expect => {
                steps.push(ScenarioStep::QueryView { view: name.clone() });
                steps.push(ScenarioStep::ExpectView {
                    view: name.clone(),
                    expectation,
                });
            }
            AssertionStyle::Eventually => steps.push(ScenarioStep::EventuallyView {
                view: name.clone(),
                expectation,
            }),
        }
        named.insert(name);
    }
    (steps, named)
}

/// The field match that names the instance this scenario is about, where the model publishes one.
///
/// # Why this is a reading and not a name match
///
/// Three declarations meet, and no two of them are being guessed at. The outcome's `instance:` says
/// where the identity of the instance it acts on is — an input field for `moves:` and `updates:`, a
/// field of an emitted event for `creates:` — which is the same declaration
/// [`ScenarioStep::CaptureInstance`] is written from. The entity's `identity:` says what that field
/// is *called*. And `ess-domain` validates every view field against the entity's observable fields
/// by that name, so a view projecting `invoice_id` is projecting the identity rather than something
/// spelled like it.
///
/// # What it is worth
///
/// Without it, `Contains {}` says "the view holds some row" and `Excludes {}` says "the view holds
/// none" — claims that are only equivalent to the intended ones because §8 isolates each scenario.
/// Against a target that shares state with anything else, the first passes on somebody else's row
/// and the second fails on it.
///
/// Empty where the view does not project the identity at all, or where nothing bound one. That is a
/// weaker assertion rather than a wrong one, and not a refusal: the specification did not ask for a
/// check this view cannot carry.
fn identifying(
    ir: &EssIr,
    subject: &ResolvedSubject,
    instance: Option<&InstanceName>,
    view: &ResolvedView,
) -> BTreeMap<String, ScenarioValue> {
    let named = ir.entity(&subject.entity).identity.name.clone();
    if view.field(&named).is_none() {
        return BTreeMap::new();
    }
    let value = match &subject.instance {
        // The caller supplied it, so the scenario knows it as whatever the arrangement bound.
        ResolvedInstance::Supplied { .. } => {
            let Some(bound) = instance else {
                return BTreeMap::new();
            };
            ScenarioValue::instance(bound.clone())
        }
        // The branch published it, and this scenario is the one that ran the branch.
        ResolvedInstance::Observed { event, field } => {
            ScenarioValue::observed(EventRef::from(event), field.name.clone())
        }
    };
    [(named, value)].into_iter().collect()
}

/// Whether a view holds a row for an entity in `state`, or the paths that stop the question being
/// answered.
///
/// The one fact a synthesised scenario knows about the entity it just created is where its
/// lifecycle starts, so that is the one fact bound. Three-valued, and the third value refuses:
/// `Unknown` here means the filter reads something no scenario can know, and asserting either way
/// would be the invention §11 rules out.
fn shows(view: &ResolvedView, state: &StateName) -> Result<bool, Vec<FactPath>> {
    let Some(filter) = &view.filter else {
        return Ok(true);
    };
    let mut facts = FactStore::new();
    let path = FactPath::new(EntitySpec::STATE)
        .unwrap_or_else(|error| panic!("`{}` is a fact path: {error}", EntitySpec::STATE));
    facts.set(path, FactValue::text(state.as_str()));

    match filter.evaluate(&facts) {
        Truth::True => Ok(true),
        Truth::False => Ok(false),
        Truth::Unknown => Err(filter
            .fact_paths()
            .into_iter()
            .filter(|path| facts.fact(path).is_none())
            .cloned()
            .collect()),
    }
}

/// Every construct this scenario's result depends on (§37).
///
/// Not what caused it to exist: the types its input mentions and the payloads it asserts are in
/// here too, because a change to one of those makes a stored result stale while a list of causes
/// says nothing.
fn dependencies(
    ir: &EssIr,
    command: &ResolvedCommand,
    outcome: &ResolvedOutcome,
    absent: &[EventRef],
    actor: Option<ActorRef>,
    views: &BTreeSet<ViewRef>,
) -> BTreeSet<EssSemanticRef> {
    let mut source = BTreeSet::new();
    let command_ref = CommandRef::new(command.name.clone());
    source.insert(command_ref.clone().into());
    source.insert(OutcomeRef::new(command_ref, outcome.name.clone()).into());

    let mut types = BTreeSet::new();
    for field in &command.input {
        reachable_types(ir, &field.type_ref, &mut types);
    }
    for handle in &outcome.emits {
        source.insert(EventRef::from(handle).into());
        for field in &ir.event(handle).fields {
            reachable_types(ir, &field.type_ref, &mut types);
        }
    }
    if let Some(handle) = &outcome.error {
        source.insert(ErrorRef::from(handle).into());
        for field in &ir.error(handle).fields {
            reachable_types(ir, &field.type_ref, &mut types);
        }
    }
    // An event asserted *absent* contributes its name and not its shape: the check is that nothing
    // arrived, which no change to the payload can affect.
    for event in absent {
        source.insert(event.clone().into());
    }
    if let Some(subject) = &outcome.subject {
        source.insert(EntityRef::from(&subject.entity).into());
    }
    if let Some(actor) = actor {
        source.insert(actor.into());
    }
    for view in views {
        source.insert(view.clone().into());
    }
    source.extend(types.into_iter().map(EssSemanticRef::from));
    source
}

/// Every declared type a reference reaches, through newtypes, structs and unions.
///
/// The set is the visited guard, so a type that refers to itself terminates rather than recursing.
fn reachable_types(ir: &EssIr, type_ref: &ResolvedTypeRef, found: &mut BTreeSet<DeclaredTypeRef>) {
    for handle in type_ref.named_leaves() {
        if !found.insert(DeclaredTypeRef::from(handle)) {
            continue;
        }
        match &ir.named_type(handle).body {
            ResolvedBody::Newtype { of, .. } => reachable_types(ir, of, found),
            ResolvedBody::Struct { fields, .. } => {
                for field in fields {
                    reachable_types(ir, &field.type_ref, found);
                }
            }
            ResolvedBody::Union { variants, .. } => {
                for variant in variants.values() {
                    reachable_types(ir, variant, found);
                }
            }
            ResolvedBody::Enum { .. } => {}
        }
    }
}

/// One line saying what the scenario proves, for the person reading a report.
fn purpose(command: &ResolvedCommand, outcome: &ResolvedOutcome) -> ScenarioPurpose {
    let reached = match outcome.test_strategy {
        TestStrategy::ConstructInput => "an input that satisfies that branch's guard",
        TestStrategy::DefaultBranch => "an input no other branch's guard claims",
        TestStrategy::InjectFault => "the cause it declares as external, injected",
        TestStrategy::ArrangeState => "a subject in a state its moves do not start from",
    };
    let text = format!(
        "`{}` answers `{}` for {reached}",
        command.name, outcome.name
    );
    clipped(&text)
}

/// The two classes of lifecycle scenario §19 asks for, legal and illegal alike.
///
/// Both are sequences over one instance: bring one into existence, drive it to the state in
/// question, then either take the move and require it happened, or issue the command that must not
/// be honoured there and require that it did not. Neither could be written before an outcome said
/// which field names the instance it acts on.
fn lifecycle(
    ir: &EssIr,
    actors: &BTreeMap<QualifiedName, ActorRef>,
    suite: &mut ConformanceSuite,
    refusals: &mut Vec<Refusal>,
) {
    for (handle, drivers) in ir.drivers() {
        let entity = EntityRef::from(handle);
        let states = &ir.entity(handle).lifecycle;

        for transition in &states.transitions {
            for driver in drivers
                .iter()
                .filter(|driver| driver.takes(&transition.name))
            {
                let id = ScenarioId::Transition {
                    transition: TransitionRef::new(entity.clone(), &transition.name)
                        .unwrap_or_else(|error| {
                            panic!("a declared transition is a single segment: {error}")
                        }),
                    by: OutcomeRef::new(
                        CommandRef::new(driver.command.name.clone()),
                        driver.outcome.name.clone(),
                    ),
                };
                let Some((steps, source)) =
                    exercise(ir, driver.command, driver.outcome, actors, &id, refusals)
                else {
                    continue;
                };
                let purpose = moving(&entity, transition, driver);
                insert(
                    suite,
                    id,
                    ConformanceScenario::new(purpose, steps, source),
                    refusals,
                );
            }
        }

        // The absence of a transition is itself semantics (§19). Which states those are is not
        // computed here and not computed twice: `EssIr::wrong_states` subtracts the `from` sets of
        // the moves a command declares from the states its entity declares, and the documentation
        // projection reads the same answer to print it on the page.
        let movers: BTreeMap<&QualifiedName, BTreeSet<&StateName>> = drivers
            .iter()
            .filter(|driver| driver.effect.transition().is_some())
            .map(|driver| (&driver.command.name, driver.command))
            .collect::<BTreeMap<_, _>>()
            .into_iter()
            .filter_map(|(name, command)| {
                ir.wrong_states(command)
                    .remove(handle)
                    .map(|states| (name, states))
            })
            .collect();
        for state in &states.states {
            for (command, wrong) in &movers {
                if !wrong.contains(state) {
                    continue;
                }
                let id = ScenarioId::Refusal {
                    entity: entity.clone(),
                    state: state.clone(),
                    command: CommandRef::new((*command).clone()),
                };
                let Some(scenario) =
                    refused_here(ir, handle, &drivers, command, state, actors, &id, refusals)
                else {
                    continue;
                };
                insert(suite, id, scenario, refusals);
            }
        }
    }
}

/// Adds a scenario, or records the collision as a refusal rather than losing one of the two.
fn insert(
    suite: &mut ConformanceSuite,
    id: ScenarioId,
    scenario: ConformanceScenario,
    refusals: &mut Vec<Refusal>,
) {
    if let Err(claimed) = suite.insert(id, scenario) {
        refusals.push(Refusal::about(&claimed, RefusalCause::DuplicateScenario));
    }
}

/// The scenario that proves a command is not honoured in a state its moves cannot start from.
///
/// Three decisions are worth stating, because each is the difference between a check and a
/// formality:
///
/// * The input is the one that **would have reached the moving branch**. Sending a value the branch
///   would refuse anyway produces a scenario that passes whether or not the state rule holds.
/// * Where the command declares a [`WrongState`](ResolvedCondition::WrongState) branch, that branch
///   and its `error:` are both required. This is §19's "the exact rejection mechanism must come from
///   the declared command/error semantics", and it is the whole reason the construct exists: without
///   it the only honest assertion was a negative one, so an implementation that refused with the
///   wrong error — or with an untyped infrastructure failure — passed. Where the command declares no
///   such branch the scenario is still produced and [`RefusalCause::RefusalUndeclared`] is recorded
///   beside it, because a thin check that looks like a thick one is the silence §36 rules out.
/// * What is asserted is that **no** event the specification declares was published — not merely
///   that the move's own events were not. No branch that emits was taken here, and every declared
///   event belongs to a branch, so an invocation that publishes one has published something no
///   branch of it licensed. The narrower claim let an unhonoured `CancelInvoice` announce that the
///   invoice was paid, which is the same hole `not_emitted` closes on the branches that *are*
///   declared.
#[allow(clippy::too_many_arguments)]
fn refused_here(
    ir: &EssIr,
    handle: &EntityHandle,
    drivers: &[Driver<'_>],
    command: &QualifiedName,
    state: &StateName,
    actors: &BTreeMap<QualifiedName, ActorRef>,
    id: &ScenarioId,
    refusals: &mut Vec<Refusal>,
) -> Option<ConformanceScenario> {
    let entity = EntityRef::from(handle);
    let movers: Vec<&Driver<'_>> = drivers
        .iter()
        .filter(|driver| &driver.command.name == command && driver.effect.transition().is_some())
        .collect();
    let attempt = movers.first().copied()?;

    let arrangement = match arrange(ir, handle, state, actors) {
        Ok(arrangement) => arrangement,
        Err(reason) => {
            refusals.push(Refusal::about(
                id,
                RefusalCause::InstanceRequired {
                    entity,
                    need: InstanceNeed::InState {
                        state: state.clone(),
                    },
                    reason,
                },
            ));
            return None;
        }
    };
    let input = match reach(ir, attempt.command, attempt.outcome) {
        Ok(input) => input,
        Err(cause) => {
            refusals.push(Refusal::about(id, cause));
            return None;
        }
    };

    let command_ref = CommandRef::new(command.clone());
    let mut steps = arrangement.steps;
    steps.push(ScenarioStep::ExecuteCommand {
        command: command_ref.clone(),
        actor: actors.get(command).cloned(),
        input: supply(
            &input,
            attempt.outcome.subject.as_ref(),
            Some(&arrangement.instance),
        ),
    });

    let mut source = arrangement.source;
    source.insert(command_ref.clone().into());
    source.insert(EntityRef::from(handle).into());
    for driver in &movers {
        source.insert(OutcomeRef::new(command_ref.clone(), driver.outcome.name.clone()).into());
    }
    if let Some(actor) = actors.get(command) {
        source.insert(actor.clone().into());
    }

    let declared = attempt
        .command
        .outcomes
        .iter()
        .find(|outcome| outcome.condition == ResolvedCondition::WrongState);
    let reported = if let Some(refusal) = declared {
        let branch = OutcomeRef::new(command_ref.clone(), refusal.name.clone());
        steps.push(ScenarioStep::ExpectOutcome {
            outcome: branch.clone(),
        });
        source.insert(branch.into());
        // The declared error, by name and with no invented payload — the same line `exercise` draws
        // for every other refusal, and the reason this family stopped being a "something went
        // wrong" check.
        refusal.error.as_ref().map(|error| {
            let named = ErrorRef::from(error);
            steps.push(ScenarioStep::ExpectError {
                error: named.clone(),
                fields: BTreeMap::new(),
            });
            source.insert(named.clone().into());
            named
        })
    } else {
        // Beside the scenario, never instead of it: what the scenario asserts is real, and it is
        // less than §19 asks for, and only one of those two facts is visible in a passing run.
        refusals.push(Refusal::about(
            id,
            RefusalCause::RefusalUndeclared {
                entity: entity.clone(),
                state: state.clone(),
                command: command_ref,
            },
        ));
        None
    };

    let forbidden = not_emitted(ir, &[]);
    for event in &forbidden {
        steps.push(ScenarioStep::ExpectNoEvent {
            event: event.clone(),
        });
    }
    source.extend(forbidden.into_iter().map(EssSemanticRef::from));

    let text = match &reported {
        Some(error) => format!(
            "`{command}` does not move a `{entity}` that is in `{state}`, and reports `{error}`"
        ),
        None => format!("`{command}` does not move a `{entity}` that is in `{state}`"),
    };
    Some(ConformanceScenario::new(clipped(&text), steps, source))
}

/// One line saying which move a transition scenario proves, and by which verb.
fn moving(
    entity: &EntityRef,
    transition: &ess_domain::entity::Transition,
    driver: &Driver<'_>,
) -> ScenarioPurpose {
    clipped(&format!(
        "`{}` on `{}` moves a `{entity}` to `{}` along `{}`",
        driver.command.name, driver.outcome.name, transition.to, transition.name
    ))
}

/// A purpose cut to one line, which is what [`ScenarioPurpose`] accepts.
fn clipped(text: &str) -> ScenarioPurpose {
    let clipped: String = text.chars().take(ScenarioPurpose::MAX_LENGTH).collect();
    ScenarioPurpose::new(clipped)
        .unwrap_or_else(|error| panic!("a synthesised purpose is one line: {error}"))
}

/// What must still hold of an entity once a branch has changed one (§20).
///
/// One scenario per entity per state-changing branch: run the branch, then require that a view of
/// the entity satisfies every invariant it declares. Not one per invariant — see
/// [`ScenarioId::Invariant`] for why an invariant has no name to be keyed by, and what carrying them
/// together costs.
///
/// A branch with no subject changes no entity, and an entity that declares no invariant has nothing
/// to check; neither is a refusal, because neither is a check the specification asked for and did
/// not get.
fn invariants(
    ir: &EssIr,
    actors: &BTreeMap<QualifiedName, ActorRef>,
    suite: &mut ConformanceSuite,
    refusals: &mut Vec<Refusal>,
) {
    let projections = ir.projections();
    for command in ir.commands.values() {
        for outcome in &command.outcomes {
            let Some(subject) = &outcome.subject else {
                continue;
            };
            if ir.entity(&subject.entity).invariants.is_empty() {
                continue;
            }
            let id = ScenarioId::Invariant {
                entity: EntityRef::from(&subject.entity),
                after: OutcomeRef::new(CommandRef::new(command.name.clone()), outcome.name.clone()),
            };
            let Some(scenario) =
                holds_after(ir, command, outcome, &projections, actors, &id, refusals)
            else {
                continue;
            };
            insert(suite, id, scenario, refusals);
        }
    }
    value_object_invariants(ir, refusals);
}

/// The scenario that runs one branch and then reads the entity's invariants off a view.
///
/// `None` where the branch cannot be run at all, and where **every** invariant was refused: a
/// scenario that executes a command and asserts nothing about what it was written to check is the
/// shape of green this milestone exists to rule out. A scenario that could assert some of them is
/// still worth having, and the ones it could not appear as refusals beside it.
#[allow(clippy::too_many_arguments)]
fn holds_after(
    ir: &EssIr,
    command: &ResolvedCommand,
    outcome: &ResolvedOutcome,
    projections: &BTreeMap<&EntityHandle, Vec<&ResolvedView>>,
    actors: &BTreeMap<QualifiedName, ActorRef>,
    id: &ScenarioId,
    refusals: &mut Vec<Refusal>,
) -> Option<ConformanceScenario> {
    let subject = outcome.subject.as_ref()?;
    let entity = ir.entity(&subject.entity);
    let entity_ref = EntityRef::from(&subject.entity);
    let run = match run(ir, command, outcome, actors) {
        Ok(run) => run,
        Err(cause) => {
            refusals.push(Refusal::about(id, cause));
            return None;
        }
    };
    // The state the branch leaves the instance in, which is what decides whether a filtered view
    // holds a row to read the invariant off. A branch with a subject always has one.
    let state = run.after.clone()?;
    let views = projections
        .get(&subject.entity)
        .map(Vec::as_slice)
        .unwrap_or_default();

    let mut steps = run.steps();
    let mut named: BTreeSet<ViewRef> = BTreeSet::new();
    for invariant in &entity.invariants {
        let witnesses = witnesses_for(ir, invariant, views, &state);
        if witnesses.is_empty() {
            refusals.push(Refusal::about(
                id,
                RefusalCause::InvariantUnobservable {
                    entity: entity_ref.clone(),
                    invariant: invariant.statement.clone(),
                    unpublished: unpublished(ir, invariant, views),
                    state: state.clone(),
                },
            ));
            continue;
        }
        for view in witnesses {
            steps.extend(assert_invariant(view, invariant));
            named.insert(ViewRef::new(view.name.clone()));
        }
    }
    if named.is_empty() {
        return None;
    }

    let mut source: BTreeSet<EssSemanticRef> = run.source.clone();
    let command_ref = CommandRef::new(command.name.clone());
    source.insert(command_ref.clone().into());
    source.insert(OutcomeRef::new(command_ref, outcome.name.clone()).into());
    source.insert(entity_ref.clone().into());
    if let Some(actor) = run.actor.clone() {
        source.insert(actor.into());
    }
    source.extend(named.into_iter().map(EssSemanticRef::from));
    source.extend(
        input_types(ir, command)
            .into_iter()
            .map(EssSemanticRef::from),
    );

    let text = format!(
        "a `{entity_ref}` still satisfies what it declares after `{}` on `{}`",
        command.name, outcome.name
    );
    Some(ConformanceScenario::new(clipped(&text), steps, source))
}

/// Requiring one invariant of one view, in the block that view's consistency decides.
///
/// The style is read off [`ResolvedView::assertion_style`], exactly as §14 requires everywhere else:
/// an invariant asserted immediately against an `eventual` projection races it, and the repair
/// everyone reaches for is a sleep.
fn assert_invariant(view: &ResolvedView, invariant: &Invariant) -> Vec<ScenarioStep> {
    let name = ViewRef::new(view.name.clone());
    let expectation = ViewExpectation::Satisfies {
        predicate: invariant.predicate.clone(),
    };
    match view.assertion_style {
        AssertionStyle::Expect => vec![
            ScenarioStep::QueryView { view: name.clone() },
            ScenarioStep::ExpectView {
                view: name,
                expectation,
            },
        ],
        AssertionStyle::Eventually => vec![ScenarioStep::EventuallyView {
            view: name,
            expectation,
        }],
    }
}

/// One refusal per value object that declares an invariant this build does not evaluate.
///
/// A value object's invariant is a claim about a *type* — every `Money` in the system — rather than
/// about one instance at rest, so it is not what §20's "after a state-changing command" evaluates.
/// Reading one off an entity means rebasing `amount >= 0` onto every path that reaches a `Money`,
/// which this slice does not do. §36's rule is that the gap is visible rather than silent, so it is
/// a refusal per type and not an omission a reader has to notice.
fn value_object_invariants(ir: &EssIr, refusals: &mut Vec<Refusal>) {
    for declared in ir.types.values() {
        let invariants = match &declared.body {
            ResolvedBody::Newtype { invariants, .. } | ResolvedBody::Struct { invariants, .. } => {
                invariants
            }
            ResolvedBody::Enum { .. } | ResolvedBody::Union { .. } => continue,
        };
        if invariants.is_empty() {
            continue;
        }
        refusals.push(Refusal {
            subject: DeclaredTypeRef::new(declared.name.clone()).into(),
            scenario: None,
            cause: RefusalCause::NotSynthesisedYet {
                construct: "reading a value object's own invariants off the fields that hold one",
                sections: "design §20",
            },
        });
    }
}

/// Every view that can answer this invariant about an instance resting in `state`.
///
/// Two conditions, and both are readings of the model rather than guesses about a runner. The view
/// has to **publish what the invariant reads** — every path resolved against the view's own declared
/// fields, by the same walk a guard's path takes through a command's input — and its filter has to
/// **hold an instance in this state**, because an assertion about every row of an empty view is an
/// assertion nothing can fail.
///
/// A view whose filter cannot be decided against the one fact a scenario knows is not a witness
/// either. That view already has its own refusal under the outcome scenario, saying so with the
/// paths; repeating it here would be one defect reported twice.
fn witnesses_for<'a>(
    ir: &EssIr,
    invariant: &Invariant,
    views: &[&'a ResolvedView],
    state: &StateName,
) -> Vec<&'a ResolvedView> {
    views
        .iter()
        .filter(|view| {
            invariant
                .predicate
                .fact_paths()
                .into_iter()
                .all(|path| resolve_path(ir, &view.fields, path).is_scalar())
                && matches!(shows(view, state), Ok(true))
        })
        .copied()
        .collect()
}

/// Every path an invariant reads that no view of the entity publishes.
///
/// The difference between "no view holds a row here" and "no view could ever answer this", which is
/// the difference between a filter an author might widen and a field an author has to publish.
fn unpublished(ir: &EssIr, invariant: &Invariant, views: &[&ResolvedView]) -> Vec<FactPath> {
    invariant
        .predicate
        .fact_paths()
        .into_iter()
        .filter(|path| {
            !views
                .iter()
                .any(|view| resolve_path(ir, &view.fields, path).is_scalar())
        })
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

/// Every declared type a command's input reaches.
fn input_types(ir: &EssIr, command: &ResolvedCommand) -> BTreeSet<DeclaredTypeRef> {
    let mut types = BTreeSet::new();
    for field in &command.input {
        reachable_types(ir, &field.type_ref, &mut types);
    }
    types
}

/// The four claims a binding makes, one scenario each (§16, §17, §18).
///
/// Every one of them starts the same way — make the event happen — and that alone is more than a
/// preamble: the trigger is a whole outcome scenario's worth of arrangement, because the event a
/// binding reacts to is published by a command that may itself need an instance driven into a state.
///
/// Where the trigger cannot be produced at all, the refusal is about the **binding** rather than
/// about one of its four scenarios: none of the four exists, and four copies of one reason is a
/// diagnostic a reader has to deduplicate by hand.
fn bindings(
    ir: &EssIr,
    actors: &BTreeMap<QualifiedName, ActorRef>,
    suite: &mut ConformanceSuite,
    refusals: &mut Vec<Refusal>,
) {
    for binding in ir.bindings.values() {
        let subject = BindingRef::new(binding.name.clone());
        let event = EventRef::from(&binding.event);
        let Some((publisher, published_by)) = publisher(ir, &binding.event) else {
            refusals.push(Refusal {
                subject: subject.clone().into(),
                scenario: None,
                cause: RefusalCause::BindingUnobservable {
                    binding: subject,
                    gap: BindingGap::NothingPublishes { event },
                },
            });
            continue;
        };
        let trigger = match run(ir, publisher, published_by, actors) {
            Ok(run) => run,
            Err(cause) => {
                refusals.push(Refusal {
                    subject: subject.clone().into(),
                    scenario: None,
                    cause,
                });
                continue;
            }
        };

        let invoked = ir.command(&binding.command);
        let source = binding_source(ir, binding, publisher, published_by, &trigger);
        for aspect in BindingAspect::ALL.map(|(aspect, _)| aspect) {
            let id = ScenarioId::Binding {
                binding: subject.clone(),
                aspect,
            };
            let built = match aspect {
                BindingAspect::Flow => flow(ir, invoked, &trigger, &event),
                BindingAspect::Mapping => mapping(ir, binding, invoked, &trigger, &event),
                BindingAspect::Delivery => delivery(ir, binding, invoked, &trigger, &event),
                BindingAspect::OnFailure => on_failure(ir, binding, invoked, &trigger, &event),
            };
            let (steps, purpose, extra) = match built {
                Ok(built) => built,
                Err(gap) => {
                    refusals.push(Refusal::about(
                        &id,
                        RefusalCause::BindingUnobservable {
                            binding: subject.clone(),
                            gap,
                        },
                    ));
                    continue;
                }
            };
            let mut depends = source.clone();
            depends.extend(extra);
            insert(
                suite,
                id,
                ConformanceScenario::new(purpose, steps, depends),
                refusals,
            );
        }
    }
}

/// What one binding aspect produces: its steps, its one-line purpose, and what else it depends on.
type Built = Result<(Vec<ScenarioStep>, ScenarioPurpose, BTreeSet<EssSemanticRef>), BindingGap>;

/// §16: the event happens, and the invoked command publishes what its branch declares.
///
/// Proved through the downstream **event**, never by observing the invocation — that is §16's rule,
/// and it is why a target with no command tracing can still be held to a binding's flow.
///
/// The observation is bounded rather than immediate ([`ScenarioStep::EventuallyEvent`]) because a
/// binding crosses a component boundary: nothing in the model says the consequence has happened by
/// the time the triggering command returns, and requiring that would be a transport assumption §41
/// refuses.
///
/// # What a flow scenario cannot tell apart, and what covers it
///
/// Where two bindings invoke one command, the event this waits for is one either of them could have
/// produced — and a binding whose trigger needs an arrangement may set the other one off while being
/// arranged. So a dropped binding is provable here only when its consequence is its own.
/// [`BindingAspect::Mapping`] is what closes that: [`ScenarioStep::ExpectInvocation`] names the
/// binding, which no event does.
fn flow(ir: &EssIr, invoked: &ResolvedCommand, trigger: &Run, event: &EventRef) -> Built {
    let reached = reachable_branch(invoked)?;
    let published = publishes(invoked, reached)?;

    let mut steps = trigger.steps();
    steps.push(ScenarioStep::ExpectEvent {
        event: event.clone(),
        payload: BTreeMap::new(),
        shape: payload_shape(ir, event),
    });
    for event in &published {
        steps.push(ScenarioStep::EventuallyEvent {
            event: event.clone(),
            payload: BTreeMap::new(),
            shape: payload_shape(ir, event),
        });
    }

    let text = format!(
        "`{event}` invokes `{}`, which publishes {}",
        invoked.name,
        listed(&published)
    );
    Ok((steps, clipped(&text), downstream(ir, invoked, reached)))
}

/// §16: each input receives the value the binding's mapping names for it.
///
/// The one clause a document can get *silently* wrong, and the reason
/// [`ScenarioStep::ExpectInvocation`] exists — a mapping's target is a command input, and the model
/// relates a command's input to no observable fact afterwards. Every value here is read straight off
/// the resolved mapping: a field of the triggering event becomes
/// [`ScenarioValue::Observed`], because no generator knows what the upstream implementation
/// published there, and a literal becomes the text the binding wrote.
fn mapping(
    ir: &EssIr,
    binding: &ResolvedBinding,
    invoked: &ResolvedCommand,
    trigger: &Run,
    event: &EventRef,
) -> Built {
    let command = CommandRef::new(invoked.name.clone());
    if binding.mapping.is_empty() {
        return Err(BindingGap::NothingMapped { command });
    }
    let input: BTreeMap<String, ScenarioValue> = binding
        .mapping
        .iter()
        .map(|mapped| {
            let value = match &mapped.value {
                ResolvedMappingValue::EventField { field, .. } => {
                    ScenarioValue::observed(event.clone(), field.clone())
                }
                // A literal reaches the model as text and fills a target that is a `String` or an
                // enum underneath — `ess-domain` refuses any other target — so the text is the value
                // and no conversion is being invented here.
                ResolvedMappingValue::Literal { value } => {
                    ScenarioValue::literal(Node::Text(value.clone()))
                }
            };
            (mapped.target.clone(), value)
        })
        .collect();

    let mut steps = trigger.steps();
    steps.push(ScenarioStep::ExpectEvent {
        event: event.clone(),
        payload: BTreeMap::new(),
        shape: payload_shape(ir, event),
    });
    steps.push(ScenarioStep::ExpectInvocation {
        binding: BindingRef::new(binding.name.clone()),
        command: command.clone(),
        input,
    });

    let text = format!(
        "`{}` fills `{}` from `{event}` as it is mapped",
        binding.name, invoked.name
    );
    // The invoked command's input types, because that is what the mapping fills: widen
    // `Recipient` and this scenario's stored result is stale even though nothing it names moved.
    let mut source: BTreeSet<EssSemanticRef> = [command.into()].into_iter().collect();
    source.extend(
        input_types(ir, invoked)
            .into_iter()
            .map(EssSemanticRef::from),
    );
    Ok((steps, clipped(&text), source))
}

/// §17: the same event delivered twice still leaves the declared consequence observable.
///
/// What `delivery: at_least_once` actually says, and the only thing it says. A conformant
/// implementation **may** produce duplicates, so the assertion is deliberately not a count: §17
/// writes "exactly one `EmailSent` exists" out as the bad test, because it fails a target that is
/// doing exactly what the specification permits.
///
/// What is left is survivability, and it is worth a scenario of its own: an implementation that
/// treats a redelivery as an error, or stops delivering after one, breaks here and nowhere else.
fn delivery(
    ir: &EssIr,
    binding: &ResolvedBinding,
    invoked: &ResolvedCommand,
    trigger: &Run,
    event: &EventRef,
) -> Built {
    let reached = reachable_branch(invoked)?;
    let published = publishes(invoked, reached)?;

    let mut steps = trigger.steps();
    steps.push(ScenarioStep::ExpectEvent {
        event: event.clone(),
        payload: BTreeMap::new(),
        shape: payload_shape(ir, event),
    });
    // The second delivery, which is the whole scenario. A total match rather than a wildcard: a
    // second delivery guarantee would mean something different here and must not inherit this.
    match binding.delivery {
        Delivery::AtLeastOnce => steps.push(ScenarioStep::RedeliverEvent {
            event: event.clone(),
        }),
    }
    for event in &published {
        steps.push(ScenarioStep::EventuallyEvent {
            event: event.clone(),
            payload: BTreeMap::new(),
            shape: payload_shape(ir, event),
        });
    }

    let text = format!(
        "`{event}` delivered twice still leaves {} observable, and no count is required",
        listed(&published)
    );
    Ok((steps, clipped(&text), downstream(ir, invoked, reached)))
}

/// §18: the declared failure policy, with the failure forced.
///
/// Forced by injection and by nothing else. A binding fills the command's input from the event, so a
/// scenario cannot choose an input that fails; the only failure it can cause is one the
/// specification declares `external:` (§12), which is why a command with no such branch refuses here
/// rather than getting a scenario that hopes.
///
/// The control is armed **after** the arrangement and before the triggering command, because
/// `ConfigureExternalOutcome` says what the adapter must produce *next*: arming it first would spend
/// the injection on whatever the arrangement's own commands set off — in a system with several
/// bindings, that is not hypothetical.
///
/// | policy | what the scenario requires | why |
/// |---|---|---|
/// | `retry` | the consequence happens anyway | one injection forces one failure; a handler that retries reaches the branch that publishes |
/// | `escalate` | the declared escalation event | since gate G2 the model names it, so this is a reading rather than a hope |
/// | `drop` | nothing — refused | "give up silently" is the whole content of the word |
fn on_failure(
    ir: &EssIr,
    binding: &ResolvedBinding,
    invoked: &ResolvedCommand,
    trigger: &Run,
    event: &EventRef,
) -> Built {
    // Read before the failure is forced, so a `drop` refuses for what it is rather than for a
    // missing external branch it would not have used.
    let policy = binding.on_failure();
    if matches!(policy, ResolvedFailure::Drop) {
        return Err(BindingGap::PolicySilent);
    }
    let forced = invoked
        .outcomes
        .iter()
        .find(|outcome| outcome.test_strategy == TestStrategy::InjectFault)
        .ok_or_else(|| BindingGap::NoForcibleFailure {
            command: CommandRef::new(invoked.name.clone()),
        })?;
    let forced_ref = OutcomeRef::new(CommandRef::new(invoked.name.clone()), forced.name.clone());

    let mut steps = trigger.setup.clone();
    steps.push(ScenarioStep::ConfigureExternalOutcome {
        force: forced_ref.clone(),
    });
    steps.extend(trigger.invoke.iter().cloned());
    steps.push(ScenarioStep::ExpectEvent {
        event: event.clone(),
        payload: BTreeMap::new(),
        shape: payload_shape(ir, event),
    });

    let mut source: BTreeSet<EssSemanticRef> = [
        CommandRef::new(invoked.name.clone()).into(),
        forced_ref.clone().into(),
    ]
    .into_iter()
    .collect();
    if let Some(component) = accepting_component(ir, &invoked.name) {
        source.insert(component.into());
    }

    let text = match policy {
        ResolvedFailure::Retry => {
            let reached = reachable_branch(invoked)?;
            let published = publishes(invoked, reached)?;
            for event in &published {
                steps.push(ScenarioStep::EventuallyEvent {
                    event: event.clone(),
                    payload: BTreeMap::new(),
                    shape: payload_shape(ir, event),
                });
            }
            source.extend(downstream(ir, invoked, reached));
            format!(
                "`{}` retries a failed `{}` until {} is published",
                binding.name,
                invoked.name,
                listed(&published)
            )
        }
        ResolvedFailure::Escalate { emits } => {
            let escalation = EventRef::from(emits);
            steps.push(ScenarioStep::EventuallyEvent {
                event: escalation.clone(),
                payload: BTreeMap::new(),
                shape: payload_shape(ir, &escalation),
            });
            source.insert(escalation.clone().into());
            format!(
                "a failed `{}` makes `{}` escalate into `{escalation}`",
                invoked.name, binding.name
            )
        }
        // Refused above, before anything was forced.
        ResolvedFailure::Drop => unreachable!("`drop` is refused before the failure is forced"),
    };
    Ok((steps, clipped(&text), source))
}

/// The branch a binding's invocation reaches, or why a scenario cannot say which one.
///
/// An input decides which branch a command takes, and a binding's input comes from the event — whose
/// values the upstream implementation chose. So the branch is knowable only when the input does not
/// choose it: exactly one branch that is not externally decided. Two of those and the answer depends
/// on a value nobody has, which is a refusal rather than a guess (§11).
fn reachable_branch(invoked: &ResolvedCommand) -> Result<&ResolvedOutcome, BindingGap> {
    let reachable: Vec<&ResolvedOutcome> = invoked
        .outcomes
        .iter()
        .filter(|outcome| outcome.test_strategy != TestStrategy::InjectFault)
        .collect();
    match reachable.as_slice() {
        // One branch, and no guard on it — so no value the event carries can send the invocation
        // anywhere else. A single *guarded* branch is not the same thing: it is a branch a
        // specification says may not be taken, and a scenario that required it anyway would be
        // claiming something about values only the upstream implementation knows.
        [only] if only.test_strategy == TestStrategy::DefaultBranch => Ok(only),
        branches => Err(BindingGap::BranchUndecided {
            command: CommandRef::new(invoked.name.clone()),
            branches: branches
                .iter()
                .map(|outcome| outcome.name.clone())
                .collect(),
        }),
    }
}

/// What a branch publishes, which is what a flow is proved through.
fn publishes(
    invoked: &ResolvedCommand,
    reached: &ResolvedOutcome,
) -> Result<Vec<EventRef>, BindingGap> {
    let published: Vec<EventRef> = reached.emits.iter().map(EventRef::from).collect();
    if published.is_empty() {
        return Err(BindingGap::NothingPublished {
            outcome: OutcomeRef::new(CommandRef::new(invoked.name.clone()), reached.name.clone()),
        });
    }
    Ok(published)
}

/// The command outcome that publishes an event, in name order, or nothing where none does.
///
/// The first in the model's own order, so the choice is a function of the specification and moves
/// only when the specification does (§37). Where two branches publish one event, either would make
/// the binding fire; taking the lower-named one keeps the suite stable when a third is added.
fn publisher<'ir>(
    ir: &'ir EssIr,
    event: &ess_compiler::ir::EventHandle,
) -> Option<(&'ir ResolvedCommand, &'ir ResolvedOutcome)> {
    ir.commands.values().find_map(|command| {
        command
            .outcomes
            .iter()
            .find(|outcome| outcome.emits.contains(event))
            .map(|outcome| (command, outcome))
    })
}

/// Everything a binding scenario's result rests on before its own aspect adds to it.
///
/// Including the two **components** it crosses, which is what a binding is: an event one component
/// publishes invoking a command another accepts. A semantic diff asking "did this change touch
/// anything this scenario rests on" has to be able to answer yes when a command moves between
/// components, and nothing else in a scenario names one.
fn binding_source(
    ir: &EssIr,
    binding: &ResolvedBinding,
    publisher: &ResolvedCommand,
    published_by: &ResolvedOutcome,
    trigger: &Run,
) -> BTreeSet<EssSemanticRef> {
    let mut source = trigger.source.clone();
    source.insert(BindingRef::new(binding.name.clone()).into());
    source.insert(EventRef::from(&binding.event).into());
    let command_ref = CommandRef::new(publisher.name.clone());
    source.insert(command_ref.clone().into());
    source.insert(OutcomeRef::new(command_ref, published_by.name.clone()).into());
    if let Some(subject) = &published_by.subject {
        source.insert(EntityRef::from(&subject.entity).into());
    }
    if let Some(actor) = trigger.actor.clone() {
        source.insert(actor.into());
    }
    if let Some(component) = accepting_component(ir, &publisher.name) {
        source.insert(component.into());
    }
    source.extend(
        input_types(ir, publisher)
            .into_iter()
            .map(EssSemanticRef::from),
    );
    source
}

/// What the branch a binding reaches depends on: the command, the branch, and what it publishes.
fn downstream(
    ir: &EssIr,
    invoked: &ResolvedCommand,
    reached: &ResolvedOutcome,
) -> BTreeSet<EssSemanticRef> {
    let command_ref = CommandRef::new(invoked.name.clone());
    let mut source: BTreeSet<EssSemanticRef> = [
        command_ref.clone().into(),
        OutcomeRef::new(command_ref, reached.name.clone()).into(),
    ]
    .into_iter()
    .collect();
    for event in &reached.emits {
        source.insert(EventRef::from(event).into());
        for field in &ir.event(event).fields {
            let mut types = BTreeSet::new();
            reachable_types(ir, &field.type_ref, &mut types);
            source.extend(types.into_iter().map(EssSemanticRef::from));
        }
    }
    if let Some(component) = accepting_component(ir, &invoked.name) {
        source.insert(component.into());
    }
    source
}

/// The component that accepts a command, where one does.
///
/// By name rather than by handle, because a command reached from a binding's own side is a
/// `ResolvedCommand`. Deterministic: the components are a [`BTreeMap`], and `ess-domain` refuses one
/// command accepted by two components.
fn accepting_component(ir: &EssIr, command: &QualifiedName) -> Option<ComponentRef> {
    ir.components
        .values()
        .find(|component| {
            component
                .accepts
                .iter()
                .any(|accepted| accepted.name() == command)
        })
        .map(|component| ComponentRef::new(component.name.clone()))
}

/// `a`, `a and b`, `a, b and c` — for the one line a report prints beside a verdict.
fn listed(events: &[EventRef]) -> String {
    let written: Vec<String> = events.iter().map(|event| format!("`{event}`")).collect();
    match written.split_last() {
        None => "nothing".to_owned(),
        Some((last, [])) => last.clone(),
        Some((last, rest)) => format!("{} and {last}", rest.join(", ")),
    }
}

/// The actor a scenario acts as, per command, where the specification grants one.
///
/// Read off [`EssIr::grants`] rather than by walking `may` again, and the lowest-named actor where a
/// command is granted to several: any of them is authorised, and choosing by name is the choice
/// that does not move when an unrelated actor is declared.
fn granted_actors(ir: &EssIr) -> BTreeMap<QualifiedName, ActorRef> {
    ir.grants()
        .iter()
        .filter_map(|(command, actors)| {
            actors
                .first()
                .map(|actor| (command.name().clone(), ActorRef::new(actor.name.clone())))
        })
        .collect()
}

/// The construct a scenario id is about.
fn subject_of(id: &ScenarioId) -> EssSemanticRef {
    match id {
        ScenarioId::Outcome { outcome } => outcome.clone().into(),
        ScenarioId::Transition { transition, .. } => transition.clone().into(),
        ScenarioId::Refusal { entity, .. } | ScenarioId::Invariant { entity, .. } => {
            entity.clone().into()
        }
        ScenarioId::Binding { binding, .. } => binding.clone().into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_refusal_carries_a_distinct_code_in_one_family() {
        // A code is what a harness matches on, so two causes sharing one is two defects a reader
        // cannot tell apart. Built here rather than derived, so adding a variant without deciding
        // its number fails to compile.
        let causes = [
            RefusalCause::NoWitness(WitnessGap {
                path: "amount".to_owned(),
                type_ref: "A".to_owned(),
                reason: "refers to itself",
            }),
            RefusalCause::GuardUnevaluable(Unevaluable {
                predicate: "amount.vat > 0".to_owned(),
                command: "witness.orders.TaxOrder".to_owned(),
                causes: Vec::new(),
            }),
            RefusalCause::GuardUnsatisfiable {
                predicate: "never".to_owned(),
                tried: 4,
            },
            RefusalCause::InstanceRequired {
                entity: EntityRef::new(
                    QualifiedName::new("billing.invoice.Invoice").expect("valid"),
                ),
                need: InstanceNeed::Updates,
                reason: Unreachable::NothingCreates,
            },
            RefusalCause::ViewUndecidable {
                view: ViewRef::new(QualifiedName::new("billing.invoice.ById").expect("valid")),
                filter: "total.amount > 0".to_owned(),
                state: StateName::new("Draft").expect("valid"),
                unbound: Vec::new(),
            },
            RefusalCause::NotSynthesisedYet {
                construct: "a binding",
                sections: "§16",
            },
            RefusalCause::DuplicateScenario,
            RefusalCause::StrategyWithoutGuard {
                strategy: TestStrategy::ConstructInput,
            },
            RefusalCause::BindingUnobservable {
                binding: BindingRef::new(
                    ess_domain::binding::BindingName::new("notify-on-invoice-created")
                        .expect("valid"),
                ),
                gap: BindingGap::PolicySilent,
            },
            RefusalCause::InvariantUnobservable {
                entity: EntityRef::new(QualifiedName::new("oracle.order.Order").expect("valid")),
                invariant: "weight_grams >= 0".to_owned(),
                unpublished: vec![FactPath::new("weight_grams").expect("a fact path")],
                state: StateName::new("Placed").expect("valid"),
            },
        ];

        let codes: BTreeSet<String> = causes
            .iter()
            .map(|cause| cause.code().to_string())
            .collect();
        assert_eq!(
            codes.len(),
            causes.len(),
            "two causes share a code: {codes:?}"
        );
        for code in &codes {
            assert!(
                code.starts_with("ESS-SYNTH-"),
                "{code} is not in this module's family"
            );
        }
        for cause in &causes {
            assert!(
                !cause.hint().is_empty(),
                "`{cause}` says nothing about what would have to change"
            );
        }
    }

    #[test]
    fn a_refusal_names_the_construct_the_code_and_the_repair() {
        let refusal = Refusal {
            subject: EssSemanticRef::Entity {
                name: EntityRef::new(QualifiedName::new("billing.invoice.Invoice").expect("valid")),
            },
            scenario: None,
            cause: RefusalCause::InstanceRequired {
                entity: EntityRef::new(
                    QualifiedName::new("billing.invoice.Invoice").expect("valid"),
                ),
                need: InstanceNeed::InState {
                    state: StateName::new("Paid").expect("valid"),
                },
                reason: Unreachable::NoPath {
                    from: StateName::new("Draft").expect("valid"),
                },
            },
        };
        let rendered = refusal.to_string();

        assert!(rendered.contains("ESS-SYNTH-004"), "{rendered}");
        assert!(rendered.contains("billing.invoice.Invoice"), "{rendered}");
        assert!(rendered.contains("Paid"), "{rendered}");
        assert!(
            rendered.contains("help:"),
            "a refusal that does not say what to change is a refusal nobody can act on: {rendered}"
        );
    }
}
