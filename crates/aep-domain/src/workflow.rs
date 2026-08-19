//! Workflows: the state machine work moves through.
//!
//! A workflow is a graph of states and guarded transitions. The guard is a predicate plus a
//! requirement set, so a harness cannot move from `verify` to `review` by asserting that it is
//! finished — it moves when the evidence says so.
//!
//! ```yaml
//! id: adp/default
//! initial: receive
//! states:
//!   implement:
//!     title: Implement
//!     phases: [implementation]
//!   complete:
//!     title: Complete
//!     terminal: true
//!     phases: [completion]
//! transitions:
//!   - from: verify
//!     to: review
//!     when:
//!       all:
//!         - tests.unit.failed == 0
//!         - static_analysis.errors == 0
//! ```
//!
//! # Phases are the join with principles
//!
//! States declare phases; principles time their obligations against phases. That indirection is
//! what lets `test-driven` apply unchanged to a development workflow, a hotfix workflow and an
//! incident workflow: each declares which of its states is the `implementation` phase.
//!
//! # Validation
//!
//! Construction rejects a workflow that cannot be executed: an initial state that does not
//! exist, a transition to a state that does not exist, a non-terminal state with no way out, a
//! state nothing can reach, and rollback declared on a state whose effects cannot be undone.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;

use crate::capability::CapabilityPolicy;
use crate::error::{ValidationCode, ValidationError, ValidationErrors};
use crate::ids::{PhaseId, StateId, WorkflowId};
use crate::predicate::Predicate;
use crate::principle::FailurePolicy;
use crate::requirement::RequirementSet;
use crate::version::MajorVersion;

/// Whether a state ends the workflow.
#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum StateKind {
    /// Work continues from here.
    #[default]
    Normal,
    /// The workflow is finished when it reaches here.
    Terminal,
}

impl StateKind {
    /// `true` for a terminal state.
    pub fn is_terminal(self) -> bool {
        self == Self::Terminal
    }
}

/// One state of a workflow.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct State {
    /// Its identifier.
    pub id: StateId,
    /// A short human title.
    pub title: String,
    /// What happens here.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    /// Whether the workflow ends here.
    pub kind: StateKind,
    /// Which phases this state belongs to.
    #[serde(skip_serializing_if = "BTreeSet::is_empty")]
    pub phases: BTreeSet<PhaseId>,
    /// What must hold to enter this state.
    #[serde(skip_serializing_if = "RequirementSet::is_empty")]
    pub requires: RequirementSet,
    /// Capabilities this state grants or withdraws on top of the resolved policy.
    #[serde(skip_serializing_if = "CapabilityPolicy::is_empty")]
    pub capabilities: CapabilityPolicy,
    /// Whether work done in this state cannot be undone.
    ///
    /// Marking a state irreversible is what makes "rollback is not a plan here" checkable: a
    /// failure policy that rolls back is rejected at validation time rather than attempted at
    /// three in the morning.
    pub irreversible: bool,
    /// What to do when a requirement here is not met.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on_failure: Option<FailurePolicy>,
}

impl State {
    /// `true` when this state declares `phase`.
    pub fn has_phase(&self, phase: &PhaseId) -> bool {
        self.phases.contains(phase)
    }

    /// `true` when the workflow ends here.
    pub fn is_terminal(&self) -> bool {
        self.kind.is_terminal()
    }
}

/// A guarded move between states.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Transition {
    /// Where it starts.
    pub from: StateId,
    /// Where it goes.
    pub to: StateId,
    /// The condition over facts.
    pub when: Predicate,
    /// Structured requirements that must also hold.
    #[serde(skip_serializing_if = "RequirementSet::is_empty")]
    pub requires: RequirementSet,
    /// What to do when the guard is not met.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on_failure: Option<FailurePolicy>,
    /// What this transition means.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl fmt::Display for Transition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} -> {}", self.from, self.to)
    }
}

/// A state machine work moves through.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Workflow {
    /// Its identifier.
    pub id: WorkflowId,
    /// Its major version.
    pub version: MajorVersion,
    /// A short human title.
    pub title: String,
    /// What it is for.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    /// Where execution starts.
    pub initial: StateId,
    /// Its states.
    pub states: BTreeMap<StateId, State>,
    /// Its transitions.
    pub transitions: Vec<Transition>,
}

impl Workflow {
    /// The state with this id.
    pub fn state(&self, id: &StateId) -> Option<&State> {
        self.states.get(id)
    }

    /// Transitions leaving `state`.
    pub fn outgoing(&self, state: &StateId) -> Vec<&Transition> {
        self.transitions
            .iter()
            .filter(|transition| &transition.from == state)
            .collect()
    }

    /// Transitions arriving at `state`.
    pub fn incoming(&self, state: &StateId) -> Vec<&Transition> {
        self.transitions
            .iter()
            .filter(|transition| &transition.to == state)
            .collect()
    }

    /// Every phase any state declares.
    pub fn phases(&self) -> BTreeSet<&PhaseId> {
        self.states
            .values()
            .flat_map(|state| state.phases.iter())
            .collect()
    }

    /// States declaring `phase`.
    pub fn states_with_phase(&self, phase: &PhaseId) -> Vec<&State> {
        self.states
            .values()
            .filter(|state| state.has_phase(phase))
            .collect()
    }

    /// Terminal states.
    pub fn terminal_states(&self) -> Vec<&State> {
        self.states
            .values()
            .filter(|state| state.is_terminal())
            .collect()
    }

    /// States reachable from the initial state.
    fn reachable(&self) -> BTreeSet<&StateId> {
        let mut seen: BTreeSet<&StateId> = BTreeSet::new();
        let Some(initial) = self.states.get_key_value(&self.initial).map(|(id, _)| id) else {
            return seen;
        };
        let mut queue: VecDeque<&StateId> = [initial].into();
        seen.insert(initial);
        while let Some(current) = queue.pop_front() {
            for transition in self.outgoing(current) {
                if let Some((id, _)) = self.states.get_key_value(&transition.to) {
                    if seen.insert(id) {
                        queue.push_back(id);
                    }
                }
            }
        }
        seen
    }
}

/// A workflow document, as parsed.
#[derive(Debug, Clone, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RawWorkflow {
    /// Its identifier.
    pub id: WorkflowId,
    /// Its major version.
    #[serde(default = "default_version")]
    pub version: MajorVersion,
    /// A short human title.
    pub title: String,
    /// What it is for.
    #[serde(default)]
    pub summary: Option<String>,
    /// Where execution starts.
    pub initial: StateId,
    /// Its states, keyed by identifier.
    pub states: BTreeMap<StateId, RawState>,
    /// Its transitions.
    #[serde(default)]
    pub transitions: Vec<RawTransition>,
    /// Whether states unreachable from the initial state are permitted.
    ///
    /// Off by default: an unreachable state is usually a typo in a transition, and finding that
    /// at validation time costs nothing.
    #[serde(default)]
    pub allow_unreachable_states: bool,
}

/// Serde default for a document's major version.
fn default_version() -> MajorVersion {
    MajorVersion::V1
}

/// A state, as parsed.
#[derive(Debug, Clone, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RawState {
    /// A short human title.
    pub title: String,
    /// What happens here.
    #[serde(default)]
    pub summary: Option<String>,
    /// Whether the workflow ends here.
    #[serde(default)]
    pub terminal: bool,
    /// Which phases this state belongs to.
    #[serde(default)]
    pub phases: BTreeSet<PhaseId>,
    /// What must hold to enter this state.
    #[serde(default, alias = "require")]
    pub requires: RequirementSet,
    /// Capability adjustments while here.
    #[serde(default)]
    pub capabilities: CapabilityPolicy,
    /// Whether work done here cannot be undone.
    #[serde(default)]
    pub irreversible: bool,
    /// What to do when a requirement here is not met.
    #[serde(default, alias = "failure_policy")]
    pub on_failure: Option<FailurePolicy>,
}

/// A transition, as parsed.
#[derive(Debug, Clone, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RawTransition {
    /// Where it starts.
    pub from: StateId,
    /// Where it goes.
    pub to: StateId,
    /// The condition over facts; omitted means unconditional.
    #[serde(default)]
    pub when: Option<Predicate>,
    /// Structured requirements that must also hold.
    #[serde(default, alias = "require")]
    pub requires: RequirementSet,
    /// What to do when the guard is not met.
    #[serde(default, alias = "failure_policy")]
    pub on_failure: Option<FailurePolicy>,
    /// What this transition means.
    #[serde(default)]
    pub description: Option<String>,
}

impl TryFrom<RawWorkflow> for Workflow {
    type Error = ValidationErrors;

    // The checks belong together: each one reports against the same accumulating error set, and
    // splitting them would mean threading that set through half a dozen private functions.
    #[allow(clippy::too_many_lines)]
    fn try_from(raw: RawWorkflow) -> Result<Self, Self::Error> {
        let mut errors = ValidationErrors::new();

        if raw.states.is_empty() {
            errors.push(ValidationError::new(
                ValidationCode::EmptyWorkflow,
                format!("workflow {}.states", raw.id),
                "a workflow must declare at least one state",
            ));
        }

        let states: BTreeMap<StateId, State> = raw
            .states
            .into_iter()
            .map(|(id, state)| {
                let built = State {
                    id: id.clone(),
                    title: state.title,
                    summary: state.summary,
                    kind: if state.terminal {
                        StateKind::Terminal
                    } else {
                        StateKind::Normal
                    },
                    phases: state.phases,
                    requires: state.requires,
                    capabilities: state.capabilities,
                    irreversible: state.irreversible,
                    on_failure: state.on_failure,
                };
                (id, built)
            })
            .collect();

        if !states.is_empty() && !states.contains_key(&raw.initial) {
            errors.push(
                ValidationError::new(
                    ValidationCode::UnknownInitialState,
                    format!("workflow {}.initial", raw.id),
                    format!("initial state `{}` is not declared", raw.initial),
                )
                .with_hint(format!(
                    "declared states: {}",
                    states
                        .keys()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(", ")
                )),
            );
        }

        let mut transitions = Vec::new();
        let mut seen_pairs: BTreeSet<(StateId, StateId)> = BTreeSet::new();

        for (index, transition) in raw.transitions.into_iter().enumerate() {
            let location = format!("workflow {}.transitions[{index}]", raw.id);
            for (label, state) in [("from", &transition.from), ("to", &transition.to)] {
                if !states.contains_key(state) {
                    errors.push(ValidationError::new(
                        ValidationCode::UnknownState,
                        format!("{location}.{label}"),
                        format!("`{state}` is not a declared state"),
                    ));
                }
            }
            if !seen_pairs.insert((transition.from.clone(), transition.to.clone())) {
                errors.push(
                    ValidationError::new(
                        ValidationCode::DuplicateTransition,
                        location.clone(),
                        format!(
                            "a second transition from `{}` to `{}` is declared",
                            transition.from, transition.to
                        ),
                    )
                    .with_hint(
                        "combine the guards with `any`, so it is clear which condition permits the \
                         move",
                    ),
                );
            }
            if let Some(policy) = &transition.on_failure {
                check_rollback_policy(&mut errors, &location, policy, states.get(&transition.from));
            }
            transitions.push(Transition {
                from: transition.from,
                to: transition.to,
                when: transition.when.unwrap_or(Predicate::Always),
                requires: transition.requires,
                on_failure: transition.on_failure,
                description: transition.description,
            });
        }

        let workflow = Self {
            id: raw.id,
            version: raw.version,
            title: raw.title,
            summary: raw.summary,
            initial: raw.initial,
            states,
            transitions,
        };

        for state in workflow.states.values() {
            let location = format!("workflow {}.states.{}", workflow.id, state.id);
            if !state.is_terminal() && workflow.outgoing(&state.id).is_empty() {
                errors.push(
                    ValidationError::new(
                        ValidationCode::DeadEndState,
                        location.clone(),
                        format!(
                            "`{}` is not terminal but has no outgoing transition, so execution \
                             would wedge here",
                            state.id
                        ),
                    )
                    .with_hint("add a transition, or mark the state `terminal: true`"),
                );
            }
            if let Some(policy) = &state.on_failure {
                check_rollback_policy(&mut errors, &location, policy, Some(state));
            }
        }

        if !raw.allow_unreachable_states && !workflow.states.is_empty() {
            let reachable = workflow.reachable();
            for id in workflow.states.keys() {
                if !reachable.contains(id) {
                    errors.push(
                        ValidationError::new(
                            ValidationCode::UnreachableState,
                            format!("workflow {}.states.{id}", workflow.id),
                            format!("`{id}` cannot be reached from `{}`", workflow.initial),
                        )
                        .with_hint(
                            "add a transition into it, or set `allow_unreachable_states: true` if \
                             it is entered out of band",
                        ),
                    );
                }
            }
        }

        errors.into_result(workflow)
    }
}

/// Checks a rollback policy against the state it applies to.
fn check_rollback_policy(
    errors: &mut ValidationErrors,
    location: &str,
    policy: &FailurePolicy,
    state: Option<&State>,
) {
    if !policy.involves_rollback() {
        return;
    }
    if let Some(state) = state {
        if state.irreversible {
            errors.push(
                ValidationError::new(
                    ValidationCode::RollbackOnIrreversibleState,
                    format!("{location}.on_failure"),
                    format!(
                        "rollback is declared for `{}`, which is marked irreversible",
                        state.id
                    ),
                )
                .with_hint(
                    "an irreversible step needs a forward recovery plan; use `escalate` or \
                     `block` instead of pretending the change can be undone",
                ),
            );
        }
    }
    if policy
        .rollback_requirement()
        .is_some_and(Predicate::is_trivially_true)
    {
        errors.push(
            ValidationError::new(
                ValidationCode::IncompleteRollbackPolicy,
                format!("{location}.on_failure"),
                "a rollback policy must state what makes rollback possible".to_owned(),
            )
            .with_hint("add `rollback.require`, for example `deployment.previous_revision.exists`"),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workflow(yaml: &str) -> Result<Workflow, ValidationErrors> {
        let raw: RawWorkflow = serde_yaml::from_str(yaml).expect("document parses");
        Workflow::try_from(raw)
    }

    const LINEAR: &str = r"
id: adp/default
title: Development
initial: implement
states:
  implement:
    title: Implement
    phases: [implementation]
  verify:
    title: Verify
    phases: [verification]
  complete:
    title: Complete
    terminal: true
    phases: [completion]
transitions:
  - from: implement
    to: verify
    when: diff.exists
  - from: verify
    to: complete
    when:
      all:
        - tests.unit.failed == 0
        - static_analysis.errors == 0
";

    #[test]
    fn accepts_a_well_formed_workflow_and_indexes_it() {
        let parsed = workflow(LINEAR).expect("validates");
        assert_eq!(parsed.states.len(), 3);
        assert_eq!(
            parsed
                .outgoing(&StateId::new("implement").expect("state"))
                .len(),
            1
        );
        assert_eq!(parsed.terminal_states().len(), 1);
        assert_eq!(
            parsed
                .phases()
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>(),
            vec!["completion", "implementation", "verification"]
        );
        assert_eq!(
            parsed
                .states_with_phase(&PhaseId::new("implementation").expect("phase"))
                .len(),
            1
        );
    }

    #[test]
    fn rejects_a_transition_to_a_state_that_does_not_exist() {
        let errors = workflow(
            r"
id: broken
title: Broken
initial: a
states:
  a:
    title: A
transitions:
  - from: a
    to: b
",
        )
        .expect_err("unknown state");
        assert!(errors.contains(ValidationCode::UnknownState), "{errors}");
    }

    #[test]
    fn rejects_a_non_terminal_state_with_no_way_out() {
        let errors = workflow(
            r"
id: wedged
title: Wedged
initial: a
states:
  a:
    title: A
  b:
    title: B
transitions:
  - from: a
    to: b
",
        )
        .expect_err("dead end");
        assert!(errors.contains(ValidationCode::DeadEndState), "{errors}");
    }

    #[test]
    fn rejects_an_unreachable_state_unless_it_is_declared_intentional() {
        let yaml = r"
id: orphan
title: Orphan
initial: a
states:
  a:
    title: A
    terminal: true
  b:
    title: B
    terminal: true
";
        let errors = workflow(yaml).expect_err("unreachable");
        assert!(
            errors.contains(ValidationCode::UnreachableState),
            "{errors}"
        );

        let permitted = format!("{yaml}allow_unreachable_states: true\n");
        assert!(workflow(&permitted).is_ok());
    }

    #[test]
    fn rejects_duplicate_transitions_between_the_same_states() {
        let errors = workflow(
            r"
id: doubled
title: Doubled
initial: a
states:
  a:
    title: A
  b:
    title: B
    terminal: true
transitions:
  - from: a
    to: b
    when: x.ready
  - from: a
    to: b
    when: y.ready
",
        )
        .expect_err("duplicate");
        assert!(
            errors.contains(ValidationCode::DuplicateTransition),
            "{errors}"
        );
    }

    #[test]
    fn rejects_rollback_on_an_irreversible_state() {
        let errors = workflow(
            r"
id: migration/forward-only
title: Forward-only migration
initial: migrate
states:
  migrate:
    title: Migrate
    irreversible: true
    on_failure:
      action: rollback
      rollback:
        require:
          - backup.exists
  done:
    title: Done
    terminal: true
transitions:
  - from: migrate
    to: done
",
        )
        .expect_err("rollback on irreversible state");
        assert!(
            errors.contains(ValidationCode::RollbackOnIrreversibleState),
            "{errors}"
        );
    }

    #[test]
    fn rejects_a_rollback_policy_that_does_not_say_what_it_needs() {
        let errors = workflow(
            r"
id: release/progressive
title: Progressive release
initial: canary
states:
  canary:
    title: Canary
    on_failure: rollback
  done:
    title: Done
    terminal: true
transitions:
  - from: canary
    to: done
",
        )
        .expect_err("incomplete rollback policy");
        assert!(
            errors.contains(ValidationCode::IncompleteRollbackPolicy),
            "{errors}"
        );
    }

    #[test]
    fn reports_every_problem_in_one_pass() {
        let errors = workflow(
            r"
id: messy
title: Messy
initial: nowhere
states:
  a:
    title: A
transitions:
  - from: a
    to: ghost
",
        )
        .expect_err("several problems");
        assert!(errors.len() >= 3, "expected several errors, got: {errors}");
        assert!(errors.contains(ValidationCode::UnknownInitialState));
        assert!(errors.contains(ValidationCode::UnknownState));
    }
}
