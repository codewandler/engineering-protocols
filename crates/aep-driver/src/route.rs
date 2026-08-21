//! The deterministic router: given a map and a cursor, what happens next.
//!
//! Two inputs, no clock, no store, no engine call — so the same cursor over the same map returns
//! the same answer forever, which is the whole of the replay claim § 4.1 makes. The engine's
//! `Evaluation` is deliberately **not** an input here: routing between states is the workflow's,
//! and the only thing this function decides is whether the state's own step list is finished.
//!
//! # Two budgets, and collapsing them would hide the difference that matters
//!
//! The **visit** budget bounds *this loop keeps going round*; the **retry** budget
//! (`Step::retry_budget`, spent by [`crate::run::drive`]) bounds *this step keeps crashing*. A
//! workflow with a `verify → implement` back-edge is going round on purpose — the shipped workflow
//! says so in its own comment, *"a workflow that can only go forwards is a lie about how
//! engineering works"* — so a driver must be able to go round again, and must not go round forever.
//! One counter for both would make a legitimate cycle indistinguishable from a wedged command.

use aep_domain::ids::StateId;
use aep_driver_spec::cursor::DriverCursor;
use aep_driver_spec::map::StepMap;

/// What the driver does next.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "next", rename_all = "snake_case")]
pub enum NextStep {
    /// Run the step at this index of the current state's list.
    Run {
        /// Which step of the current state.
        index: usize,
    },
    /// The state's steps are done: ask the engine to move.
    ///
    /// Also the answer for a state the map says nothing about. A workflow state whose transition is
    /// unguarded needs no work done in it, and a map that had to say so for every such state would
    /// be noise.
    Transition,
    /// The state has been entered more often than its budget allows, so the run stops.
    VisitBudgetExhausted {
        /// The state that was being cycled in.
        state: StateId,
        /// The budget it exceeded.
        budget: u32,
    },
}

/// What to do next, from the map and the cursor alone.
///
/// The visit budget is checked **first**: a state whose budget is spent has no next step, however
/// many steps its list still holds. Visits are counted on *entry* by the loop, so a budget of `3`
/// permits three entries and the fourth is refused.
pub fn next_step(map: &StepMap, cursor: &DriverCursor) -> NextStep {
    let budget = map.visit_budget(&cursor.state);
    if cursor.visits_of(&cursor.state) > budget {
        return NextStep::VisitBudgetExhausted {
            state: cursor.state.clone(),
            budget,
        };
    }
    if cursor.step < map.steps_for(&cursor.state).len() {
        return NextStep::Run { index: cursor.step };
    }
    NextStep::Transition
}

/// How many steps of the current state have not run yet.
///
/// Saturating rather than wrapping: a cursor pointing past the end of a list is what a resume after
/// a step map lost a step looks like, and the honest answer there is *none left*, not a number the
/// size of the address space. The map digest in the cursor is what actually refuses that resume.
pub fn steps_remaining(map: &StepMap, cursor: &DriverCursor) -> usize {
    map.steps_for(&cursor.state)
        .len()
        .saturating_sub(cursor.step)
}
