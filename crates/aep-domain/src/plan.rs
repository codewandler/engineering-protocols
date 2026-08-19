//! Resolved execution plans.
//!
//! An [`ExecutionPlan`] is what resolution produces: a task, the protocol vocabulary it runs
//! under, the profile it selected, the principles actually in force, the workflow, the capability
//! policy with the reason for every entry, the obligations sorted by when they bite, and what
//! completion means.
//!
//! Everything in a plan is already validated and cross-checked, so the engine can execute
//! without re-deciding anything, and a person can read one document to see exactly what a task
//! is being held to.

use std::collections::BTreeMap;

use crate::capability::{Capability, CapabilityDecision, CapabilityPolicy, PolicySource};
use crate::facts::FactStore;
use crate::ids::{ObligationId, PhaseId, PrincipleId, StateId};
use crate::principle::{Obligation, ObligationTiming, PhaseRef, Principle};
use crate::profile::Profile;
use crate::protocol::Protocol;
use crate::requirement::RequirementSet;
use crate::task::Task;
use crate::workflow::{State, Workflow};

/// Why one capability ended up allowed, denied or behind approval.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, schemars::JsonSchema)]
pub struct CapabilityGrant {
    /// The capability.
    pub capability: Capability,
    /// What the resolved policy says about it.
    pub effect: CapabilityDecision,
    /// Which document said so.
    pub source: PolicySource,
}

/// One obligation, with the principle it came from.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, schemars::JsonSchema)]
pub struct ResolvedObligation {
    /// Which principle obliges this.
    pub principle: PrincipleId,
    /// The obligation's identifier.
    pub id: ObligationId,
    /// What it is for.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// When it must hold.
    pub timing: ObligationTiming,
    /// What must hold.
    pub requires: RequirementSet,
}

impl ResolvedObligation {
    /// Builds a resolved obligation from a principle's obligation.
    pub fn new(principle: &PrincipleId, obligation: &Obligation) -> Self {
        Self {
            principle: principle.clone(),
            id: obligation.id.clone(),
            description: obligation.description.clone(),
            timing: obligation.timing.clone(),
            requires: obligation.requires.clone(),
        }
    }

    /// `true` when this obligation must hold before entering `state`.
    pub fn blocks_entry_to(&self, state: &State) -> bool {
        matches!(&self.timing, ObligationTiming::Before { target } if targets(target, state))
    }

    /// `true` when this obligation must hold while in `state`, and is therefore checked on the
    /// way out.
    pub fn applies_within(&self, state: &State) -> bool {
        matches!(&self.timing, ObligationTiming::During { target } if targets(target, state))
    }

    /// `true` when this obligation is checked at every transition.
    pub fn is_always(&self) -> bool {
        self.timing == ObligationTiming::Always
    }
}

/// `true` when `target` names `state`, by phase or by identifier.
fn targets(target: &PhaseRef, state: &State) -> bool {
    match target {
        PhaseRef::Phase(phase) => state.has_phase(phase),
        PhaseRef::State(id) => &state.id == id,
    }
}

/// A task with everything resolved: what it must do, may do and has to prove.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ExecutionPlan {
    /// The task.
    pub task: Task,
    /// The protocol vocabulary, with anything it extends already merged in.
    pub protocol: Protocol,
    /// The profile, with anything it extends already merged in.
    pub profile: Profile,
    /// The principles actually in force, after profile and task overrides.
    pub principles: Vec<Principle>,
    /// The workflow to execute.
    pub workflow: Workflow,
    /// What may be done.
    pub capability_policy: CapabilityPolicy,
    /// Why each capability entry is what it is.
    pub capability_rationale: Vec<CapabilityGrant>,
    /// Every obligation in force, from every applicable principle.
    pub obligations: Vec<ResolvedObligation>,
    /// What being finished means.
    pub completion: RequirementSet,
    /// Facts known before anything is observed: the task's and the profile's.
    pub facts: FactStore,
    /// Principles the task or profile dropped, recorded so the decision stays visible.
    pub dropped_principles: Vec<PrincipleId>,
}

impl ExecutionPlan {
    /// The principle with this id, if it is in force.
    pub fn principle(&self, id: &PrincipleId) -> Option<&Principle> {
        self.principles.iter().find(|principle| &principle.id == id)
    }

    /// Obligations that must hold before entering `state`, plus the always-on ones.
    pub fn obligations_to_enter(&self, state: &State) -> Vec<&ResolvedObligation> {
        self.obligations
            .iter()
            .filter(|obligation| obligation.is_always() || obligation.blocks_entry_to(state))
            .collect()
    }

    /// Obligations that must hold while in `state`.
    pub fn obligations_within(&self, state: &State) -> Vec<&ResolvedObligation> {
        self.obligations
            .iter()
            .filter(|obligation| obligation.applies_within(state))
            .collect()
    }

    /// Everything that must hold to move from `from` to `to`.
    ///
    /// The union of: obligations in force at every transition, obligations owed while in `from`,
    /// obligations owed before entering `to`, and `to`'s own entry requirements. The transition's
    /// own guard is evaluated separately, because a failed guard and an unmet obligation want
    /// different explanations.
    pub fn transition_requirements(&self, from: &State, to: &State) -> Vec<&ResolvedObligation> {
        let mut obligations: Vec<&ResolvedObligation> = self
            .obligations
            .iter()
            .filter(|obligation| {
                obligation.is_always()
                    || obligation.applies_within(from)
                    || obligation.blocks_entry_to(to)
            })
            .collect();
        obligations.sort_by(|left, right| {
            (left.principle.as_str(), left.id.as_str())
                .cmp(&(right.principle.as_str(), right.id.as_str()))
        });
        obligations
    }

    /// Phases referenced by obligations that no state in the workflow declares.
    ///
    /// A non-empty result means a principle's obligation can never be checked, which is worth
    /// refusing to run rather than discovering by its absence.
    pub fn unmatched_phases(&self) -> Vec<&PhaseId> {
        let declared = self.workflow.phases();
        let mut unmatched = Vec::new();
        for obligation in &self.obligations {
            if let Some(PhaseRef::Phase(phase)) = obligation.timing.target() {
                if !declared.contains(&phase) && !unmatched.contains(&phase) {
                    unmatched.push(phase);
                }
            }
        }
        unmatched
    }

    /// States referenced by obligations that the workflow does not declare.
    pub fn unmatched_states(&self) -> Vec<&StateId> {
        let mut unmatched = Vec::new();
        for obligation in &self.obligations {
            if let Some(PhaseRef::State(state)) = obligation.timing.target() {
                if !self.workflow.states.contains_key(state) && !unmatched.contains(&state) {
                    unmatched.push(state);
                }
            }
        }
        unmatched
    }

    /// A one-line-per-entry summary of the capability policy, for reports.
    pub fn capability_summary(&self) -> BTreeMap<String, CapabilityDecision> {
        self.capability_policy
            .mentioned()
            .into_iter()
            .map(|capability| {
                (
                    capability.to_string(),
                    self.capability_policy.decide(capability),
                )
            })
            .collect()
    }
}
