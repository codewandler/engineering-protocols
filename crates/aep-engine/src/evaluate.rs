//! What is owed, and what is permitted.
//!
//! Evaluation answers three questions at once, from the same data, so a harness and a human cannot
//! be told different things:
//!
//! * what must hold **now**, in the current state;
//! * for each outgoing transition, whether it is permitted and, if not, exactly what is missing;
//! * whether the task is **complete**.
//!
//! Nothing here decides anything new. It reads the plan and the execution's facts, evidence and
//! artifact graph, and reports.
//!
//! [`demanded_evidence`] answers a fourth question, and it is the only one here that needs no
//! execution: *what evidence could this plan ever ask for?* A launch-time check has to answer it
//! before there is anything to evaluate — see its own documentation for why the two walks are kept
//! in one file.

use aep_domain::ids::{ClaimId, ObligationId, PrincipleId, StateId};
use aep_domain::plan::ExecutionPlan;
use aep_domain::predicate::{PredicateOutcome, Truth};
use aep_domain::principle::{FailurePolicy, ObligationTiming, Principle};
use aep_domain::requirement::{
    EvidenceRequirement, RequirementFlavour, RequirementOutcome, RequirementSet,
};
use aep_domain::verification::Verifier;
use aep_domain::workflow::State;

use crate::execution::Execution;

/// Which document asked for a requirement.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RequirementSource {
    /// A principle's obligation.
    Principle {
        /// Which principle.
        principle: PrincipleId,
        /// Which obligation of it.
        obligation: ObligationId,
    },
    /// A state's entry requirements.
    State {
        /// Which state.
        state: StateId,
    },
    /// A transition's own requirements.
    Transition {
        /// Where from.
        from: StateId,
        /// Where to.
        to: StateId,
    },
    /// The profile's completion condition.
    Completion,
}

impl RequirementSource {
    /// A one-line attribution, for reports.
    pub fn label(&self) -> String {
        match self {
            Self::Principle { principle, .. } => format!("principle {principle}"),
            Self::State { state } => format!("state {state}"),
            Self::Transition { from, to } => format!("transition {from} -> {to}"),
            Self::Completion => "completion".to_owned(),
        }
    }
}

/// One requirement, with where it came from and whether it holds.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Requirement {
    /// Which document asked for it.
    pub source: RequirementSource,
    /// When it must hold, for a principle's obligation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timing: Option<ObligationTiming>,
    /// The requirement and its outcome.
    #[serde(flatten)]
    pub outcome: RequirementOutcome,
}

impl Requirement {
    /// `true` when it holds.
    pub fn is_satisfied(&self) -> bool {
        self.outcome.is_satisfied()
    }

    /// A one-line rendering, `✓`/`✗`/`?` then the requirement.
    pub fn line(&self) -> String {
        format!("{} [{}]", self.outcome, self.source.label())
    }
}

/// One outgoing transition, and whether it may be taken.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct TransitionEvaluation {
    /// Where it goes.
    pub to: StateId,
    /// Whether it may be taken now.
    pub permitted: bool,
    /// The guard predicate's outcome.
    pub guard: PredicateOutcome,
    /// Everything else that must hold, satisfied or not.
    pub requirements: Vec<Requirement>,
    /// What to do while it cannot be taken.
    pub on_failure: FailurePolicy,
}

impl TransitionEvaluation {
    /// One line per unmet requirement, including the guard.
    pub fn unmet(&self) -> Vec<String> {
        let mut unmet = Vec::new();
        if !self.guard.is_satisfied() {
            for cause in &self.guard.causes {
                unmet.push(format!("guard: {}", cause.expression));
            }
            if self.guard.causes.is_empty() {
                unmet.push(format!("guard: {}", self.guard.expression));
            }
        }
        for requirement in &self.requirements {
            if !requirement.is_satisfied() {
                unmet.push(requirement.line());
            }
        }
        unmet
    }
}

/// The whole picture at one point in an execution.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Evaluation {
    /// Where the execution is.
    pub state: StateId,
    /// The state's human title.
    pub state_title: String,
    /// Whether this state ends the workflow.
    pub terminal: bool,
    /// What must hold while in this state.
    pub requirements: Vec<Requirement>,
    /// Every outgoing transition.
    pub transitions: Vec<TransitionEvaluation>,
    /// What completion needs, satisfied or not.
    pub completion: Vec<Requirement>,
    /// Whether the task is finished.
    pub is_complete: bool,
    /// Whether nothing can move and the task is not finished.
    pub blocked: bool,
}

impl Evaluation {
    /// Transitions that may be taken now, in document order.
    pub fn permitted_transitions(&self) -> Vec<&TransitionEvaluation> {
        self.transitions
            .iter()
            .filter(|transition| transition.permitted)
            .collect()
    }

    /// Completion requirements that do not hold.
    pub fn missing_for_completion(&self) -> Vec<&Requirement> {
        self.completion
            .iter()
            .filter(|requirement| !requirement.is_satisfied())
            .collect()
    }

    /// One line per reason nothing can move.
    pub fn blocking_reasons(&self) -> Vec<String> {
        if self.transitions.is_empty() {
            return self
                .missing_for_completion()
                .iter()
                .map(|requirement| requirement.line())
                .collect();
        }
        self.transitions
            .iter()
            .flat_map(|transition| {
                transition
                    .unmet()
                    .into_iter()
                    .map(move |reason| format!("{} -> {}: {reason}", self.state, transition.to))
            })
            .collect()
    }
}

/// Evaluates `execution` in its current state.
pub fn evaluate(execution: &Execution) -> Evaluation {
    let plan = execution.plan();
    let workflow = &plan.workflow;
    let Some(current) = workflow.state(execution.state_id()) else {
        // A state outside the workflow can only come from a snapshot taken against different
        // documents; report it as blocked rather than panicking.
        return Evaluation {
            state: execution.state_id().clone(),
            state_title: "unknown state".to_owned(),
            terminal: false,
            requirements: Vec::new(),
            transitions: Vec::new(),
            completion: Vec::new(),
            is_complete: false,
            blocked: true,
        };
    };

    let mut requirements = obligations_now(execution, current);
    requirements.extend(evaluate_set(
        execution,
        &current.requires,
        &RequirementSource::State {
            state: current.id.clone(),
        },
        None,
    ));

    let mut transitions = Vec::new();
    for transition in workflow.outgoing(&current.id) {
        let guard = transition.when.outcome(execution.fact_store());
        let mut items = evaluate_set(
            execution,
            &transition.requires,
            &RequirementSource::Transition {
                from: transition.from.clone(),
                to: transition.to.clone(),
            },
            None,
        );

        if let Some(target) = workflow.state(&transition.to) {
            items.extend(evaluate_set(
                execution,
                &target.requires,
                &RequirementSource::State {
                    state: target.id.clone(),
                },
                None,
            ));
            items.extend(obligations_to_enter(execution, target));
        }
        items.extend(obligations_now(execution, current));

        let permitted = guard.is_satisfied() && items.iter().all(Requirement::is_satisfied);
        let on_failure = transition
            .on_failure
            .clone()
            .or_else(|| current.on_failure.clone())
            .unwrap_or_else(|| plan.protocol.default_failure_policy.clone());

        transitions.push(TransitionEvaluation {
            to: transition.to.clone(),
            permitted,
            guard,
            requirements: items,
            on_failure,
        });
    }

    let completion = completion_requirements(execution);
    let completion_met = completion.iter().all(Requirement::is_satisfied);
    let is_complete = current.is_terminal() && completion_met;
    let blocked = !is_complete && !transitions.iter().any(|transition| transition.permitted);

    Evaluation {
        state: current.id.clone(),
        state_title: current.title.clone(),
        terminal: current.is_terminal(),
        requirements,
        transitions,
        completion,
        is_complete,
        blocked,
    }
}

/// Obligations owed while in `state`: the always-on ones and those scoped to it.
fn obligations_now(execution: &Execution, state: &State) -> Vec<Requirement> {
    execution
        .plan()
        .obligations
        .iter()
        .filter(|obligation| obligation.is_always() || obligation.applies_within(state))
        .flat_map(|obligation| {
            evaluate_set(
                execution,
                &obligation.requires,
                &RequirementSource::Principle {
                    principle: obligation.principle.clone(),
                    obligation: obligation.id.clone(),
                },
                Some(&obligation.timing),
            )
        })
        .collect()
}

/// Obligations that must hold before entering `state`.
fn obligations_to_enter(execution: &Execution, state: &State) -> Vec<Requirement> {
    execution
        .plan()
        .obligations
        .iter()
        .filter(|obligation| obligation.blocks_entry_to(state))
        .flat_map(|obligation| {
            evaluate_set(
                execution,
                &obligation.requires,
                &RequirementSource::Principle {
                    principle: obligation.principle.clone(),
                    obligation: obligation.id.clone(),
                },
                Some(&obligation.timing),
            )
        })
        .collect()
}

/// Everything completion needs: the profile's condition, obligations owed before completion, and
/// each principle's evidence and verification requirements.
fn completion_requirements(execution: &Execution) -> Vec<Requirement> {
    let plan = execution.plan();
    let mut requirements = evaluate_set(
        execution,
        &plan.completion,
        &RequirementSource::Completion,
        None,
    );

    let completion_states: Vec<&State> = plan
        .workflow
        .states
        .values()
        .filter(|state| state.is_terminal() || state.has_phase(&completion_phase()))
        .collect();

    for obligation in &plan.obligations {
        let owed = obligation.is_always()
            || completion_states
                .iter()
                .any(|state| obligation.blocks_entry_to(state));
        if !owed {
            continue;
        }
        requirements.extend(evaluate_set(
            execution,
            &obligation.requires,
            &RequirementSource::Principle {
                principle: obligation.principle.clone(),
                obligation: obligation.id.clone(),
            },
            Some(&obligation.timing),
        ));
    }

    for principle in &plan.principles {
        requirements.extend(principle_evidence(execution, principle));
        requirements.extend(principle_verification(execution, principle));
    }

    requirements
}

/// The `completion` phase, which terminal states are expected to declare.
fn completion_phase() -> aep_domain::ids::PhaseId {
    aep_domain::ids::PhaseId::new(aep_domain::principle::COMPLETION_PHASE)
        .expect("the completion phase name is valid")
}

/// Evaluates a principle's top-level evidence list.
fn principle_evidence(execution: &Execution, principle: &Principle) -> Vec<Requirement> {
    if principle.evidence.is_empty() {
        return Vec::new();
    }
    let set = RequirementSet {
        evidence: principle.evidence.clone(),
        ..RequirementSet::empty()
    };
    evaluate_set(
        execution,
        &set,
        &RequirementSource::Principle {
            principle: principle.id.clone(),
            obligation: obligation_id(&principle.id, "evidence"),
        },
        None,
    )
}

/// Checks that each verifier a principle requires has actually produced something **recently
/// enough**.
///
/// A verifier requirement is not about a fact being true; it is about *who established it*. A green
/// suite that only an agent ever reported does not satisfy `verifier: test-runner`.
///
/// # And a verifier who spoke three weeks ago has not spoken about today
///
/// Without the liveness filter this check would answer *has anyone ever?* rather than *does anyone
/// still say so?*, and it would be the one surface where a lapsed record still reads `True` — a
/// hole under the requirement beside it that reads `?`. It uses
/// [`Execution::has_lapsed`], which is the same rule the fact store applies, so a
/// verification requirement and an evidence requirement cannot disagree about one record.
fn principle_verification(execution: &Execution, principle: &Principle) -> Vec<Requirement> {
    principle
        .verification
        .iter()
        .map(|requirement| {
            let spoken = execution.recorded_evidence().iter().any(|recorded| {
                !execution.has_lapsed(&recorded.record)
                    && produced_by(&recorded.record, &requirement.verifier)
                    && requirement
                        .claim
                        .as_ref()
                        .is_none_or(|claim| establishes_claim(&recorded.record.value, claim))
            });
            let outcome = RequirementOutcome {
                flavour: RequirementFlavour::Evidence,
                requirement: requirement.to_string(),
                truth: if spoken { Truth::True } else { Truth::Unknown },
                detail: if spoken {
                    None
                } else {
                    Some(format!(
                        "no evidence from {} has been recorded",
                        requirement.verifier
                    ))
                },
            };
            Requirement {
                source: RequirementSource::Principle {
                    principle: principle.id.clone(),
                    obligation: obligation_id(&principle.id, "verification"),
                },
                timing: Some(requirement.timing.clone()),
                outcome,
            }
        })
        .collect()
}

/// `true` when this evidence is about `claim`.
fn establishes_claim(evidence: &aep_domain::evidence::Evidence, claim: &ClaimId) -> bool {
    match evidence {
        aep_domain::evidence::Evidence::Verification(record) => &record.claim == claim,
        aep_domain::evidence::Evidence::PropertyTestResult(result) => &result.property == claim,
        _ => false,
    }
}

/// `true` when `record` was produced by `verifier`, directly or by its tool.
fn produced_by(record: &aep_domain::evidence::EvidenceRecord, verifier: &Verifier) -> bool {
    if let aep_domain::evidence::Producer::Verifier { verifier: actual } = &record.producer {
        if actual == verifier {
            return true;
        }
    }
    if let (Verifier::ExternalTool(expected), Some(actual)) = (verifier, &record.provenance.tool) {
        return expected == actual;
    }
    false
}

/// Builds a synthetic obligation id for a principle's list-level requirements.
fn obligation_id(principle: &PrincipleId, suffix: &str) -> ObligationId {
    ObligationId::new(format!("{principle}/{suffix}"))
        .unwrap_or_else(|error| panic!("generated obligation id is invalid: {error}"))
}

/// Evaluates a requirement set and attributes every item.
fn evaluate_set(
    execution: &Execution,
    requirements: &RequirementSet,
    source: &RequirementSource,
    timing: Option<&ObligationTiming>,
) -> Vec<Requirement> {
    if requirements.is_empty() {
        return Vec::new();
    }
    requirements
        .evaluate(execution)
        .items
        .into_iter()
        .map(|outcome| Requirement {
            source: source.clone(),
            timing: timing.cloned(),
            outcome,
        })
        .collect()
}

/// What an unmet demand stops, in the workflow's own vocabulary.
///
/// Typed rather than a sentence, because the engine decides *what* is blocked and a caller decides
/// how to say it. [`Self::label`] is the house rendering.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Blocked {
    /// The task cannot be declared finished.
    Completion,
    /// This state cannot be entered.
    Entering {
        /// Which state.
        state: StateId,
    },
    /// This state cannot be left.
    Leaving {
        /// Which state.
        state: StateId,
    },
    /// This one move cannot be made.
    Move {
        /// Where from.
        from: StateId,
        /// Where to.
        to: StateId,
    },
    /// Every transition, for an obligation owed at all times.
    EveryTransition,
}

impl Blocked {
    /// A one-line rendering, for a report.
    pub fn label(&self) -> String {
        match self {
            Self::Completion => "completion".to_owned(),
            Self::Entering { state } => format!("entering {state}"),
            Self::Leaving { state } => format!("leaving {state}"),
            Self::Move { from, to } => format!("{from} -> {to}"),
            Self::EveryTransition => "every transition".to_owned(),
        }
    }
}

/// One evidence requirement a resolved plan can demand, and the document that asked for it.
///
/// Produced by [`demanded_evidence`].
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct DemandedEvidence {
    /// Which document asked.
    pub source: RequirementSource,
    /// When it must hold, for a principle's obligation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timing: Option<ObligationTiming>,
    /// The requirement exactly as the document wrote it.
    pub requirement: EvidenceRequirement,
    /// The conditions it sits under, outermost first; empty when it is unconditional.
    pub guards: Vec<String>,
    /// Whether an unmet instance of this raises `evidence.missing`.
    ///
    /// The distinction matters to a caller reporting *what a gap blocks*: a demand that feeds
    /// `evidence.missing` blocks every guard reading that count, wherever in the workflow it sits,
    /// while a state's or a transition's own requirement blocks only that one move.
    pub counted_in_missing: bool,
    /// What stays shut while this demand is unmet, deduplicated and in a stable order.
    ///
    /// Empty is a real answer and is not the same as *nothing is wrong*: it means the demand's
    /// timing points at a state this workflow cannot reach and no guard reads the count it feeds,
    /// so nothing this plan describes waits on it.
    pub blocks: Vec<Blocked>,
}

/// Every evidence requirement this plan can demand, without running anything.
///
/// # Why this exists, and what it cost not to have it
///
/// [`evaluate`] answers *what is owed here, now*, and it needs an [`Execution`] — which exists only
/// after a run has started. So the question *can this run ever produce what it will be asked for?*
/// had no answer until the run had already spent its budget finding out. Run `W4-2/1` is the
/// measurement: six states, ten model sessions, 76 minutes and $31.46 to arrive at
/// `evidence.missing = 2` at the `adversarial_verify -> review` guard, because no step of its step
/// map could mint a `specification` or a `verification` record and nothing had ever compared the
/// two documents. This is that comparison's left-hand side; it reads documents and nothing else.
///
/// # What is walked, and why each part is here
///
/// * **`plan.completion`**, **every obligation**, and **every in-force principle's own `evidence:`
///   list** — the three sources `Execution::count_missing_evidence` folds into `evidence.missing`.
///   Obligations are taken whatever their timing, because that function applies no timing filter
///   either: an obligation owed before implementation still raises the count that guards a much
///   later transition.
/// * **Each *reachable* state's `requires`** and **each transition leaving a reachable state**.
///   These are *not* in `evidence.missing`; [`evaluate`] reads them directly when it decides a
///   move. Reachability is applied here and nowhere else, because a requirement on a state nothing
///   can walk into blocks nothing — counting one would make a launch check refuse a run for a rule
///   that will never fire.
///
/// # Applicability and the Kleene posture
///
/// A principle scoped away by a declared fact is already absent: `resolve` filtered it out through
/// `Principle::applies`, so nothing it asks for appears here. An **undeclared** fact leaves the
/// principle in force and its demands are returned in full — silence is not an exemption
/// (invariant 5).
///
/// Inside a set, a conditional whose `when` evaluates **`False`** against the plan's pre-run facts
/// is pruned along with everything below it, and that is the only pruning. `Unknown` is kept: it
/// means nobody has observed whether the branch applies, and a check that read *unobserved* as
/// *does not apply* would wave through the run it exists to stop. This is the same rule
/// `count_missing_evidence` and `reachable_approvals` apply, deliberately.
///
/// # What it does not answer, stated rather than implied
///
/// * **Predicates.** A completion condition such as `contracts.failed == 0` demands a fact that
///   only a `ContractResult` projects, and nothing in this workspace maps a fact path back to the
///   kinds that project it. Deriving one here would be a second copy of `Evidence::facts`, which is
///   how a fact and a requirement come to disagree. Such an obligation is invisible to this walk —
///   `profiles/development-standard.yaml:38` is the live example.
/// * **`reviews:`, `approvals:` and a principle's `verification:` list.** The first two are
///   person-shaped and already have a pre-flight of their own
///   (`aep_driver::approval::reachable_approvals`); the third pins a *verifier* and no kind at all,
///   so it cannot be expressed as a gap in a set of kinds.
pub fn demanded_evidence(plan: &ExecutionPlan) -> Vec<DemandedEvidence> {
    let reachable = plan.workflow.reachable();
    let counted_blocks = blocked_by_the_count(plan, &reachable);
    let mut found = Vec::new();

    collect_demands(
        &plan.completion,
        &RequirementSource::Completion,
        None,
        true,
        &[Blocked::Completion],
        &counted_blocks,
        plan,
        &[],
        &mut found,
    );

    for obligation in &plan.obligations {
        let owned = blocked_by_timing(plan, obligation, &reachable);
        collect_demands(
            &obligation.requires,
            &RequirementSource::Principle {
                principle: obligation.principle.clone(),
                obligation: obligation.id.clone(),
            },
            Some(&obligation.timing),
            true,
            &owned,
            &counted_blocks,
            plan,
            &[],
            &mut found,
        );
    }

    for principle in &plan.principles {
        for requirement in &principle.evidence {
            // A principle's own `evidence:` list carries no timing. `completion_requirements`
            // folds it in unconditionally, so completion is what it holds shut.
            found.push(demand(
                RequirementSource::Principle {
                    principle: principle.id.clone(),
                    obligation: obligation_id(&principle.id, "evidence"),
                },
                None,
                requirement,
                &[],
                true,
                &[Blocked::Completion],
                &counted_blocks,
            ));
        }
    }

    for (id, state) in &plan.workflow.states {
        if !reachable.contains(id) {
            continue;
        }
        collect_demands(
            &state.requires,
            &RequirementSource::State { state: id.clone() },
            None,
            false,
            &[Blocked::Entering { state: id.clone() }],
            &counted_blocks,
            plan,
            &[],
            &mut found,
        );
    }
    for transition in &plan.workflow.transitions {
        if !reachable.contains(&transition.from) {
            continue;
        }
        collect_demands(
            &transition.requires,
            &RequirementSource::Transition {
                from: transition.from.clone(),
                to: transition.to.clone(),
            },
            None,
            false,
            &[Blocked::Move {
                from: transition.from.clone(),
                to: transition.to.clone(),
            }],
            &counted_blocks,
            plan,
            &[],
            &mut found,
        );
    }

    found
}

/// Everything an unmet demand shuts *by raising `evidence.missing`*, computed once for the plan.
///
/// A demand folded into that count blocks all of these at the same time, wherever in the workflow
/// the demand itself was written — which is precisely what makes the count expensive to be wrong
/// about, and why `W4-2/1` walked six states before the number mattered.
fn blocked_by_the_count(
    plan: &ExecutionPlan,
    reachable: &std::collections::BTreeSet<&StateId>,
) -> Vec<Blocked> {
    plan.workflow
        .transitions
        .iter()
        .filter(|transition| reachable.contains(&transition.from))
        .filter(|transition| reads_missing_count(&transition.when))
        .map(|transition| Blocked::Move {
            from: transition.from.clone(),
            to: transition.to.clone(),
        })
        .chain(
            plan.completion
                .predicates
                .iter()
                .any(reads_missing_count)
                .then_some(Blocked::Completion),
        )
        .collect()
}

/// What one obligation's own timing holds shut, over reachable states only.
///
/// `before {phase: completion}` blocks arriving at `complete`, `during {state: verify}` blocks
/// leaving `verify`, and `always` blocks every move there is.
fn blocked_by_timing(
    plan: &ExecutionPlan,
    obligation: &aep_domain::plan::ResolvedObligation,
    reachable: &std::collections::BTreeSet<&StateId>,
) -> Vec<Blocked> {
    match &obligation.timing {
        ObligationTiming::Always => vec![Blocked::EveryTransition],
        ObligationTiming::Before { .. } => plan
            .workflow
            .states
            .values()
            .filter(|state| reachable.contains(&state.id) && obligation.blocks_entry_to(state))
            .map(|state| Blocked::Entering {
                state: state.id.clone(),
            })
            .collect(),
        ObligationTiming::During { .. } => plan
            .workflow
            .states
            .values()
            .filter(|state| reachable.contains(&state.id) && obligation.applies_within(state))
            .map(|state| Blocked::Leaving {
                state: state.id.clone(),
            })
            .collect(),
    }
}

/// `true` when this predicate reads the count of unmet evidence requirements.
///
/// Both spellings, because the engine derives both: `evidence.missing` is canonical and
/// `required_evidence.missing` is the alias a document may have been written against.
fn reads_missing_count(predicate: &aep_domain::predicate::Predicate) -> bool {
    predicate.fact_paths().iter().any(|path| {
        let written = path.to_string();
        written == "evidence.missing" || written == "required_evidence.missing"
    })
}

/// Walks one requirement set for evidence demands, recursing through conditionals not ruled out.
///
/// `folds_into_missing` says whether this set is one of the three `evidence.missing` is counted
/// over. The reported `counted_in_missing` narrows it by depth, because
/// `Execution::count_missing_evidence` descends exactly **one** conditional level: a demand nested
/// deeper is still owed — `RequirementSet::evaluate` recurses all the way, so it blocks the
/// transition or the completion it sits under — it simply does not show up in that count, and
/// saying otherwise would attribute a gap to a guard that never reads it.
#[allow(clippy::too_many_arguments)]
fn collect_demands(
    set: &RequirementSet,
    source: &RequirementSource,
    timing: Option<&ObligationTiming>,
    folds_into_missing: bool,
    owned_blocks: &[Blocked],
    counted_blocks: &[Blocked],
    plan: &ExecutionPlan,
    guards: &[String],
    found: &mut Vec<DemandedEvidence>,
) {
    for requirement in &set.evidence {
        found.push(demand(
            source.clone(),
            timing,
            requirement,
            guards,
            folds_into_missing && guards.len() <= 1,
            owned_blocks,
            counted_blocks,
        ));
    }

    for conditional in &set.conditional {
        if conditional.when.evaluate(&plan.facts) == Truth::False {
            continue;
        }
        let mut nested = guards.to_vec();
        nested.push(format!("if {}", conditional.when));
        collect_demands(
            &conditional.require,
            source,
            timing,
            folds_into_missing,
            owned_blocks,
            counted_blocks,
            plan,
            &nested,
            found,
        );
    }
}

/// Assembles one demand, merging what its own position blocks with what the count blocks.
fn demand(
    source: RequirementSource,
    timing: Option<&ObligationTiming>,
    requirement: &EvidenceRequirement,
    guards: &[String],
    counted_in_missing: bool,
    owned_blocks: &[Blocked],
    counted_blocks: &[Blocked],
) -> DemandedEvidence {
    let mut blocks: Vec<Blocked> = owned_blocks.to_vec();
    if counted_in_missing {
        blocks.extend(counted_blocks.iter().cloned());
    }
    blocks.sort();
    blocks.dedup();
    DemandedEvidence {
        source,
        timing: timing.cloned(),
        requirement: requirement.clone(),
        guards: guards.to_vec(),
        counted_in_missing,
        blocks,
    }
}
