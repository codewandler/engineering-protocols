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
//! # What is not here yet
//!
//! Binding scenarios (§16–§18) and the runner. Both are later slices, and the first appears as a
//! refusal per binding rather than as silence, for the reason above.
//!
//! Invariant scenarios (§20) produce nothing at all, and that is a property of the scenario IR
//! rather than an omission here: [`ScenarioId`] has four shapes — an outcome, a transition, a
//! refusal and a binding — and an entity invariant is none of them. It is observable only through a
//! view, which is what the view steps below assert.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use aep_domain::facts::{FactPath, FactSource, FactStore, FactValue};
use aep_domain::node::Node;
use aep_domain::predicate::{Predicate, Truth};
use ess_compiler::diagnostic::Code;
use ess_compiler::ir::{
    EssIr, ResolvedBody, ResolvedCommand, ResolvedEffect, ResolvedOutcome, ResolvedTypeRef,
    ResolvedView,
};
use ess_domain::command::TestStrategy;
use ess_domain::entity::{EntitySpec, StateName};
use ess_domain::name::QualifiedName;
use ess_domain::view::AssertionStyle;

use crate::decision::{when, Decision, Unevaluable};
use crate::input::{flatten, ShapeErrors};
use crate::scenario::{
    ActorRef, BindingRef, CommandRef, ConformanceScenario, ConformanceSuite, DeclaredTypeRef,
    EntityRef, ErrorRef, EssSemanticRef, EventRef, OutcomeRef, ScenarioId, ScenarioPurpose,
    ScenarioStep, SuiteProvenance, TransitionRef, ViewExpectation, ViewRef,
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
    /// The scenario would have needed an instance of an entity that already exists.
    ///
    /// The gap that stops §19 dead, and it is not the one G14 closed. An outcome now names the
    /// entity it moves and the transition it takes, so *which* command drives a transition is
    /// answered. What is still unanswered is **which input field names the instance**: an entity
    /// declares an identity, a command declares an input, and nothing in the model relates the two.
    /// A scenario therefore cannot say "the invoice the previous step created", and a fabricated id
    /// would be a test that fails against a correct implementation — which §11 rules out in one
    /// line: *a refusal is better than a false test*.
    InstanceRequired {
        /// Whose instance.
        entity: EntityRef,
        /// What the scenario would have needed of it.
        need: InstanceNeed,
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
            Self::InstanceRequired { .. } => {
                "the model must say which command input names the subject's identity before a \
                 scenario can act on an instance an earlier step created"
            }
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
            Self::InstanceRequired { entity, need } => write!(
                f,
                "it needs an instance of `{entity}` {need}, and no step can name one"
            ),
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
            if let Err(claimed) = suite.insert(id, scenario) {
                refusals.push(Refusal::about(&claimed, RefusalCause::DuplicateScenario));
            }
        }
    }
    lifecycle(ir, &mut refusals);
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
    let command_ref = CommandRef::new(command.name.clone());
    let outcome_ref = OutcomeRef::new(command_ref.clone(), outcome.name.clone());
    let id = ScenarioId::Outcome {
        outcome: outcome_ref.clone(),
    };

    if let Some((entity, need)) = instance_needed(outcome) {
        refusals.push(Refusal::about(
            &id,
            RefusalCause::InstanceRequired { entity, need },
        ));
        return None;
    }

    let input = match reach(ir, command, outcome) {
        Ok(input) => input,
        Err(cause) => {
            refusals.push(Refusal::about(&id, cause));
            return None;
        }
    };

    let emitted: Vec<EventRef> = outcome.emits.iter().map(EventRef::from).collect();
    let absent = siblings_emit(command, outcome, &emitted);
    let actor = actors.get(&command.name).cloned();
    let (view_steps, views) = view_expectations(ir, outcome, &id, refusals);

    let mut steps = Vec::new();
    if outcome.test_strategy == TestStrategy::InjectFault {
        // §12: no predicate over a recipient and a template says whether a provider will accept the
        // mail, so the suite injects the answer rather than inventing an input that produces it.
        steps.push(ScenarioStep::ConfigureExternalOutcome {
            force: outcome_ref.clone(),
        });
    }
    steps.push(ScenarioStep::ExecuteCommand {
        command: command_ref.clone(),
        actor: actor.clone(),
        input: input.clone(),
    });
    steps.push(ScenarioStep::ExpectOutcome {
        outcome: outcome_ref.clone(),
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

    let source = dependencies(ir, command, outcome, &absent, actor, &views);
    Some((
        id,
        ConformanceScenario::new(purpose(command, outcome), steps, source),
    ))
}

/// What an outcome needs of an instance that must already exist, when it needs one.
///
/// `Creates` needs none — the model's own words are "a new instance comes into existence" — and a
/// branch with no subject changes nothing at all.
///
/// # A refusal branch of a lifecycle command is still synthesised
///
/// `PayInvoice/rejected` declares no subject, because a refused command changes nothing, and its
/// branch is decided by `amount.amount > 0` and by nothing else. So a scenario for it is what the
/// specification actually says: this input, this outcome, this error. The `invoice_id` it carries
/// names no invoice, and the specification does not make it matter — there is no `not_found`
/// outcome declared.
///
/// If an implementation checks existence first and answers something else, the specification is
/// incomplete and this scenario is how that becomes visible. That is the oracle working, not a
/// false test: the value proves nothing and is claimed to prove nothing (§11), and the branch it
/// asserts was decided by evaluation.
fn instance_needed(outcome: &ResolvedOutcome) -> Option<(EntityRef, InstanceNeed)> {
    let subject = outcome.subject.as_ref()?;
    let need = match &subject.effect {
        ResolvedEffect::Creates => return None,
        ResolvedEffect::Moves { transition } => InstanceNeed::Moves {
            transition: transition.name.clone(),
        },
        ResolvedEffect::Updates => InstanceNeed::Updates,
    };
    Some((EntityRef::from(&subject.entity), need))
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

/// The view assertions an outcome that brings an instance into existence supports (§14, §20).
///
/// Only for `creates:`. A branch that moves or updates an instance is refused before this is
/// reached, and a branch with no subject changes nothing a view could show.
fn view_expectations(
    ir: &EssIr,
    outcome: &ResolvedOutcome,
    id: &ScenarioId,
    refusals: &mut Vec<Refusal>,
) -> (Vec<ScenarioStep>, BTreeSet<ViewRef>) {
    let mut steps = Vec::new();
    let mut named = BTreeSet::new();
    let Some(subject) = &outcome.subject else {
        return (steps, named);
    };
    if !matches!(subject.effect, ResolvedEffect::Creates) {
        return (steps, named);
    }
    let state = ir.entity(&subject.entity).lifecycle.initial.clone();
    let projections = ir.projections();
    let Some(views) = projections.get(&subject.entity) else {
        return (steps, named);
    };

    for view in views {
        let name = ViewRef::new(view.name.clone());
        let expectation = match shows(view, &state) {
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
    let clipped: String = text.chars().take(ScenarioPurpose::MAX_LENGTH).collect();
    ScenarioPurpose::new(clipped)
        .unwrap_or_else(|error| panic!("a synthesised purpose is one line: {error}"))
}

/// One refusal per lifecycle scenario §19 asks for, legal and illegal alike.
///
/// Both classes need an instance that already exists, so both are blocked by the same missing link;
/// see [`RefusalCause::InstanceRequired`]. They are enumerated rather than summarised because each
/// is a scenario a later slice will produce under the same id, and a fault matrix that refers to
/// one has to find it in yesterday's refusals as well as in tomorrow's suite.
fn lifecycle(ir: &EssIr, refusals: &mut Vec<Refusal>) {
    for (handle, drivers) in ir.drivers() {
        let entity = EntityRef::from(handle);
        let lifecycle = &ir.entity(handle).lifecycle;

        for transition in &lifecycle.transitions {
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
                refusals.push(Refusal::about(
                    &id,
                    RefusalCause::InstanceRequired {
                        entity: entity.clone(),
                        need: InstanceNeed::Moves {
                            transition: transition.name.clone(),
                        },
                    },
                ));
            }
        }

        // The absence of a transition is itself semantics (§19). "Relevant" is read here as: a
        // command that moves this entity at all, and a state none of its moves may start from.
        let movers: BTreeSet<&QualifiedName> = drivers
            .iter()
            .filter(|driver| driver.effect.transition().is_some())
            .map(|driver| &driver.command.name)
            .collect();
        for state in &lifecycle.states {
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
                refusals.push(Refusal::about(
                    &id,
                    RefusalCause::InstanceRequired {
                        entity: entity.clone(),
                        need: InstanceNeed::InState {
                            state: state.clone(),
                        },
                    },
                ));
            }
        }
    }
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
