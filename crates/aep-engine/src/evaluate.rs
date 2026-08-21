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

use aep_domain::ids::{ClaimId, ObligationId, PrincipleId, StateId};
use aep_domain::predicate::{PredicateOutcome, Truth};
use aep_domain::principle::{FailurePolicy, ObligationTiming, Principle};
use aep_domain::requirement::{RequirementFlavour, RequirementOutcome, RequirementSet};
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
