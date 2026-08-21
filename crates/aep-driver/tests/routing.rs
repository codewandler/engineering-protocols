//! The router and the lock refusal: the two pieces of the driver that decide without touching
//! anything.
//!
//! Both are pure functions over values somebody else observed, which is what makes them testable
//! without a store, a process or a second machine. That is the point of the placement, not a side
//! effect of it (review finding **F19**).

use std::collections::BTreeMap;

use aep_domain::ids::{ExecutionId, StateId, TaskId};
use aep_driver::lock::{Liveness, LockState};
use aep_driver::route::{next_step, steps_remaining, NextStep};
use aep_driver_spec::cursor::{DriverCursor, RunId, RunStatus};
use aep_driver_spec::map::{StepMapId, DEFAULT_VISIT_BUDGET};

/// A map with two steps in `implement`, a visit budget of two, and nothing said about `verify`.
const MAP: &str = r"
format: aep.driver-steps/1
id: test/routing
workflow: test/linear/1
states:
  implement:
    visit_budget: 2
    steps:
      - kind: llm
        prompt: write the code
      - kind: command
        run: [cargo, test]
        evidence:
          kind: test_result
          verifier: test-runner
          suite: unit
";

fn map() -> aep_driver_spec::map::StepMap {
    aep_schema::parse::step_map(MAP, Some("test/routing.yaml")).expect("the fixture map validates")
}

fn cursor(state: &str) -> DriverCursor {
    let task = TaskId::new("T-1").expect("a task id");
    DriverCursor {
        run: RunId::new(&task, 1).expect("a run id"),
        task: task.clone(),
        execution: ExecutionId::new("T-1.1").expect("an execution id"),
        workflow: "test/linear/1".to_owned(),
        map: StepMapId::new("test/routing").expect("a map id"),
        map_digest: "digest".to_owned(),
        engine_version: aep_driver::ENGINE_VERSION.to_owned(),
        state: StateId::new(state).expect("a state id"),
        step: 0,
        visits: BTreeMap::new(),
        attempts: BTreeMap::new(),
        iterations: 0,
        status: RunStatus::Running,
        reasons: Vec::new(),
        took_lock_from: None,
    }
}

#[test]
fn the_router_walks_a_states_steps_in_order_and_then_asks_the_engine_to_move() {
    let map = map();
    let mut cursor = cursor("implement");
    cursor.record_visit(&cursor.state.clone());

    assert_eq!(next_step(&map, &cursor), NextStep::Run { index: 0 });
    assert_eq!(steps_remaining(&map, &cursor), 2);

    cursor.step = 1;
    assert_eq!(next_step(&map, &cursor), NextStep::Run { index: 1 });
    assert_eq!(steps_remaining(&map, &cursor), 1);

    cursor.step = 2;
    assert_eq!(
        next_step(&map, &cursor),
        NextStep::Transition,
        "the state's steps are done, so the next move is the workflow's"
    );
    assert_eq!(steps_remaining(&map, &cursor), 0);
}

#[test]
fn a_state_the_map_says_nothing_about_transitions_immediately() {
    let map = map();
    let mut cursor = cursor("verify");
    cursor.record_visit(&cursor.state.clone());

    assert_eq!(next_step(&map, &cursor), NextStep::Transition);
    assert_eq!(steps_remaining(&map, &cursor), 0);
    assert_eq!(
        map.visit_budget(&StateId::new("verify").expect("a state id")),
        DEFAULT_VISIT_BUDGET,
        "a state the map is silent about still has a budget, or a back-edge into it is unbounded"
    );
}

#[test]
fn a_state_entered_past_its_visit_budget_stops_the_run_rather_than_running_its_steps_again() {
    let map = map();
    let mut cursor = cursor("implement");
    let state = cursor.state.clone();

    for _ in 0..2 {
        cursor.record_visit(&state);
        assert_eq!(
            next_step(&map, &cursor),
            NextStep::Run { index: 0 },
            "a budget of two permits two entries"
        );
    }

    cursor.record_visit(&state);
    assert_eq!(
        next_step(&map, &cursor),
        NextStep::VisitBudgetExhausted {
            state: state.clone(),
            budget: 2
        },
        "the third entry exceeds the budget, and the run stops with the state named rather than \
         burning a token budget in silence"
    );
    assert_eq!(
        cursor.step, 0,
        "the budget is checked before the step list, so a spent state has no next step however \
         many steps it still holds"
    );
}

#[test]
fn a_cursor_pointing_past_the_end_of_a_shortened_list_reports_no_steps_left() {
    let map = map();
    let mut cursor = cursor("implement");
    cursor.record_visit(&cursor.state.clone());
    cursor.step = 9;

    assert_eq!(steps_remaining(&map, &cursor), 0);
    assert_eq!(next_step(&map, &cursor), NextStep::Transition);
}

#[test]
fn a_live_holder_is_refused_and_take_lock_is_refused_with_it() {
    let held = LockState {
        run: "AUTH-142/2".to_owned(),
        pid: 4711,
        host: "workbench".to_owned(),
        liveness: Liveness::Alive,
    };
    assert!(!held.is_stale());

    for taking in [false, true] {
        let refusal = held.refusal(taking);
        assert!(refusal.contains("AUTH-142/2"), "{refusal}");
        assert!(refusal.contains("4711"), "{refusal}");
        assert!(refusal.contains("workbench"), "{refusal}");
        assert!(
            refusal.contains("--resume"),
            "a refusal that does not name the way out is a puzzle: {refusal}"
        );
    }
    assert!(
        held.refusal(true)
            .contains("refused while the holder is alive"),
        "`--take-lock` is not a way past a running process"
    );
}

#[test]
fn a_dead_holder_is_stale_and_still_refused_until_a_person_says_take_it() {
    let stale = LockState {
        run: "AUTH-142/2".to_owned(),
        pid: 4711,
        host: "workbench".to_owned(),
        liveness: Liveness::Dead,
    };
    assert!(stale.is_stale());

    let refusal = stale.refusal(false);
    assert!(
        refusal.contains("--take-lock"),
        "a stale lock is refused *and* the route out is named: {refusal}"
    );
    assert!(
        stale.refusal(true).contains("supersedes"),
        "taking a lock supersedes rather than erases; the stolen lock goes into the new cursor"
    );
}

#[test]
fn a_lock_held_on_another_host_is_never_stale_whatever_the_local_pid_table_says() {
    let elsewhere = LockState {
        run: "AUTH-142/2".to_owned(),
        pid: 4711,
        host: "ci-runner-3".to_owned(),
        liveness: Liveness::OtherHost,
    };
    assert!(
        !elsewhere.is_stale(),
        "a pid on another machine is a number about a process this one cannot see"
    );
    let refusal = elsewhere.refusal(true);
    assert!(refusal.contains("ci-runner-3"), "{refusal}");
    assert!(
        refusal.contains("never stale"),
        "the reason has to travel with the refusal: {refusal}"
    );
}
