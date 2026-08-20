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
//! An outcome scenario asserts the branch, the declared error, every event the branch emits, and —
//! first class, per §10 — every event a *sibling* branch emits and this one does not. What it does
//! **not** assert is any payload *value*: nothing in the model relates a command's input to an
//! event's or an error's payload, so `InvalidAmount.submitted == amount` is an inference, not a
//! reading. §10's own worked suite asserts `→ InvalidAmount` and no field, and this produces exactly
//! that. When the model gains a mapping, the fields follow from it rather than from a name match.
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
//! # What is not here yet
//!
//! Binding scenarios (§16–§18) and the runner. Both are later slices, and the first appears as a
//! refusal per binding rather than as silence, for the reason above.
//!
//! Invariant scenarios (§20) produce nothing at all, and the two things that stop them are not this
//! gate. [`ScenarioId`] has four shapes — an outcome, a transition, a refusal and a binding — and an
//! entity invariant is none of them; and [`ViewExpectation`] matches rows by field *values*, so
//! there is no step that evaluates `total.amount >= 0` against what a view shows. Both are decisions
//! about what a suite can say, in the sense §21 means: a new shape there is a change to what an ESS
//! *means*, not a convenience for one generator.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;

use aep_domain::facts::{FactPath, FactSource, FactStore, FactValue};
use aep_domain::node::Node;
use aep_domain::predicate::{Predicate, Truth};
use ess_compiler::diagnostic::Code;
use ess_compiler::ir::{
    Driver, EntityHandle, EssIr, ResolvedBody, ResolvedCommand, ResolvedEffect, ResolvedInstance,
    ResolvedOutcome, ResolvedSubject, ResolvedTypeRef, ResolvedView,
};
use ess_domain::command::TestStrategy;
use ess_domain::entity::{EntitySpec, StateName};
use ess_domain::name::QualifiedName;
use ess_domain::view::AssertionStyle;

use crate::decision::{when, Decision, Unevaluable};
use crate::input::{flatten, ShapeErrors};
use crate::scenario::{
    ActorRef, BindingRef, CommandRef, ConformanceScenario, ConformanceSuite, DeclaredTypeRef,
    EntityRef, ErrorRef, EssSemanticRef, EventRef, InstanceName, OutcomeRef, ScenarioId,
    ScenarioPurpose, ScenarioStep, ScenarioValue, SuiteProvenance, TransitionRef, ViewExpectation,
    ViewRef,
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
    /// Two scenarios claimed one id. A drift alarm: `ess-domain` refuses a duplicated declaration.
    DuplicateScenario,
    /// The outcome's strategy says an input reaches it and its condition declares no guard.
    ///
    /// A drift alarm. [`TestStrategy`] is computed from the condition, so the two cannot disagree
    /// unless one of them changes without the other.
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
            let Some((id, scenario)) =
                outcome_scenario(ir, command, outcome, &actors, &mut refusals)
            else {
                continue;
            };
            insert(&mut suite, id, scenario, &mut refusals);
        }
    }
    lifecycle(ir, &actors, &mut suite, &mut refusals);
    bindings(ir, &mut refusals);

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
    let setup = match prepare(ir, outcome, actors) {
        Ok(setup) => setup,
        Err(cause) => {
            refusals.push(Refusal::about(id, cause));
            return None;
        }
    };
    let input = match reach(ir, command, outcome) {
        Ok(input) => input,
        Err(cause) => {
            refusals.push(Refusal::about(id, cause));
            return None;
        }
    };

    let command_ref = CommandRef::new(command.name.clone());
    let outcome_ref = OutcomeRef::new(command_ref.clone(), outcome.name.clone());
    let emitted: Vec<EventRef> = outcome.emits.iter().map(EventRef::from).collect();
    let absent = siblings_emit(command, outcome, &emitted);
    let actor = actors.get(&command.name).cloned();
    let (view_steps, views) = view_expectations(ir, outcome, setup.after.as_ref(), id, refusals);

    let mut steps = setup.steps.clone();
    if outcome.test_strategy == TestStrategy::InjectFault {
        // §12: no predicate over a recipient and a template says whether a provider will accept the
        // mail, so the suite injects the answer rather than inventing an input that produces it.
        steps.push(ScenarioStep::ConfigureExternalOutcome {
            force: outcome_ref.clone(),
        });
    }
    steps.push(ScenarioStep::ExecuteCommand {
        command: command_ref,
        actor: actor.clone(),
        input: supply(&input, outcome.subject.as_ref(), setup.instance.as_ref()),
    });
    steps.push(ScenarioStep::ExpectOutcome {
        outcome: outcome_ref,
    });
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
    source.extend(setup.source);
    Some((steps, source))
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

/// Every event a sibling branch emits and this one does not.
fn siblings_emit(
    command: &ResolvedCommand,
    outcome: &ResolvedOutcome,
    emitted: &[EventRef],
) -> Vec<EventRef> {
    command
        .outcomes
        .iter()
        .filter(|other| other.name != outcome.name)
        .flat_map(|other| other.emits.iter().map(EventRef::from))
        .filter(|event| !emitted.contains(event))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

/// The view assertions a branch that changed an entity supports (§14, §20).
///
/// `after` is the state the subject is in once the branch has been taken — the lifecycle's `initial`
/// for a `creates:`, the transition's `to` for a `moves:`, and the state it was arranged in for an
/// `updates:`. It is passed in rather than re-derived, because deciding a filter against the wrong
/// state is a wrong assertion rather than a missing one.
///
/// A branch with no subject changes nothing a view could show, and produces no steps.
fn view_expectations(
    ir: &EssIr,
    outcome: &ResolvedOutcome,
    after: Option<&StateName>,
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
        let expectation = match shows(view, state) {
            Ok(true) => ViewExpectation::Contains {
                fields: BTreeMap::new(),
            },
            // A cancelled invoice that stays in `OutstandingInvoices` is the defect the positive
            // assertion cannot see, and an entity that has not reached the filtered state yet is
            // exactly that case at the other end.
            Ok(false) => ViewExpectation::Excludes {
                fields: BTreeMap::new(),
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

        // The absence of a transition is itself semantics (§19). "Relevant" is read here as: a
        // command that moves this entity at all, and a state none of its moves may start from.
        let movers: BTreeSet<&QualifiedName> = drivers
            .iter()
            .filter(|driver| driver.effect.transition().is_some())
            .map(|driver| &driver.command.name)
            .collect();
        for state in &states.states {
            for command in &movers {
                let legal = drivers.iter().any(|driver| {
                    &driver.command.name == *command
                        && driver
                            .effect
                            .transition()
                            .is_some_and(|transition| transition.from.contains(state))
                });
                if legal {
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
/// * There is no `ExpectOutcome`. The specification declares no branch for this case — that is
///   precisely what makes the combination illegal — so asserting one would be inventing the
///   rejection mechanism §19 says must come from the declared semantics.
/// * What is asserted is that the events the move would have published did **not** happen. An
///   outcome with a subject can neither report an error (invariant 15) nor be silent
///   (`empty_change`), so a mover always emits something, and there is always something to require
///   the absence of.
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
    let forbidden: BTreeSet<EventRef> = movers
        .iter()
        .flat_map(|driver| driver.outcome.emits.iter().map(EventRef::from))
        .collect();
    for event in &forbidden {
        steps.push(ScenarioStep::ExpectNoEvent {
            event: event.clone(),
        });
    }

    let mut source = arrangement.source;
    source.insert(command_ref.clone().into());
    source.insert(EntityRef::from(handle).into());
    for driver in &movers {
        source.insert(OutcomeRef::new(command_ref.clone(), driver.outcome.name.clone()).into());
    }
    source.extend(forbidden.into_iter().map(EssSemanticRef::from));
    if let Some(actor) = actors.get(command) {
        source.insert(actor.clone().into());
    }

    let text = format!("`{command}` does not move a `{entity}` that is in `{state}`");
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

/// One refusal per binding, until the slice that synthesises §16–§18 lands.
fn bindings(ir: &EssIr, refusals: &mut Vec<Refusal>) {
    for binding in ir.bindings.values() {
        refusals.push(Refusal {
            subject: BindingRef::new(binding.name.clone()).into(),
            scenario: None,
            cause: RefusalCause::NotSynthesisedYet {
                construct: "a binding's flow and its failure policy",
                sections: "design §16, §17 and §18",
            },
        });
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
        ScenarioId::Refusal { entity, .. } => entity.clone().into(),
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
