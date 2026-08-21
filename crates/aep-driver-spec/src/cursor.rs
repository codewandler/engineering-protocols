//! The driver's own record of a run: where it is, what it has spent, and what it is pinned to.
//!
//! Two documents go to `.engineering/runs/<run-id>/` after every step, because they have two
//! owners. The **snapshot** is the engine's (`Execution::snapshot()`); the **cursor** is this —
//! which step of which state the driver is on, its budgets, and the three things a resume must
//! check. A driver that stored its cursor inside the engine's snapshot would be a driver that had
//! quietly forked the snapshot format.
//!
//! # Why the cursor pins three things a snapshot does not
//!
//! `Snapshot` carries the execution id, the task id, the state, the states entered, the evidence,
//! the events and the actor — and **no workflow id and no version**. `Execution::restore` checks
//! that the snapshot's task matches the plan and that its *state name* still exists. So a workflow
//! that renamed nothing and rewrote every guard restores cleanly today and silently re-governs the
//! run. The cursor closes that: it records the resolved workflow reference, the step map's id and
//! digest, and — per review finding **F20** — the engine version that wrote the snapshot, so an
//! older driver meeting a newer snapshot refuses with a sentence instead of a deserialization
//! error.
//!
//! # A record, not a document
//!
//! Invariant 2 is about authored documents: a `Raw*` deserialises and a validated type does not.
//! Nobody authors a cursor — the driver writes it and the driver reads it back — so it round-trips
//! through serde in both directions and has no schema. The workflow pin is kept here as the
//! **string** `<id>/<major>` for the same reason: what a resume needs is an equality check against
//! what it recorded, and a validated reference that could deserialise would weaken a rule this
//! crate holds elsewhere for no gain.

use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;

use aep_domain::error::ParseError;
use aep_domain::ids::{ExecutionId, StateId, TaskId};

use crate::map::StepMapId;

/// Identifier of one driver run, such as `AUTH-142/3`.
///
/// The driver's own, allocated after the store lock is taken by counting the run directories that
/// exist. It is deliberately **not** the engine's `ExecutionId`: that is
/// `<task>.<ordinal>` where the ordinal comes from a counter held **in each `Engine` value**, so
/// two engines in one process — the shape a test harness builds — mint the same id, and a run
/// directory keyed on it would have one run overwrite the other's snapshot. The execution id is
/// recorded *inside* the cursor so the two can be joined later.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RunId {
    task: String,
    ordinal: u32,
}

impl RunId {
    /// Builds a run id for a task and an ordinal.
    ///
    /// # Errors
    ///
    /// When the ordinal is zero: runs are counted from one, and `<task>/0` would read as a run
    /// that never happened.
    pub fn new(task: &TaskId, ordinal: u32) -> Result<Self, ParseError> {
        if ordinal == 0 {
            return Err(ParseError::identifier(
                "run",
                &format!("{task}/{ordinal}"),
                "runs are numbered from one".to_owned(),
            ));
        }
        Ok(Self {
            task: task.to_string(),
            ordinal,
        })
    }

    /// The task this run is of.
    pub fn task(&self) -> &str {
        &self.task
    }

    /// Which run of that task this is.
    pub fn ordinal(&self) -> u32 {
        self.ordinal
    }

    /// The path segments of this run's directory, below `.engineering/runs/`.
    ///
    /// Two segments rather than one flattened name, so a task's runs sit together and no separator
    /// has to be escaped out of an identifier that may legally contain `/` itself.
    pub fn segments(&self) -> [String; 2] {
        [self.task.clone(), self.ordinal.to_string()]
    }
}

impl fmt::Display for RunId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.task, self.ordinal)
    }
}

impl FromStr for RunId {
    type Err = ParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let reject = |reason: &str| {
            Err(ParseError::identifier(
                "run",
                value,
                format!("{reason}; a run id is written `<task>/<n>`, such as `AUTH-142/3`"),
            ))
        };
        let Some((task, ordinal)) = value.rsplit_once('/') else {
            return reject("no `/` separates the task from the run number");
        };
        let Ok(ordinal) = ordinal.parse::<u32>() else {
            return reject("the part after the last `/` is not a number");
        };
        let task = TaskId::new(task)?;
        Self::new(&task, ordinal)
    }
}

impl serde::Serialize for RunId {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> serde::Deserialize<'de> for RunId {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        raw.parse().map_err(serde::de::Error::custom)
    }
}

/// Where a run got to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    /// Still going, or interrupted without recording anything else.
    Running,
    /// The workflow reached its terminal state.
    Completed,
    /// Nothing can move, and the engine said why.
    Blocked,
    /// An `operator` step is owed an answer from a person.
    AwaitingOperator,
    /// A visit or retry budget ran out.
    BudgetExhausted,
    /// The planning store stopped parsing, so no evaluation could be trusted.
    ///
    /// Deliberately not [`RunStatus::Blocked`]: `Blocked` is the engine's word for *the protocol
    /// says no*, and a store with a typo in it is not that. The run directory stays resumable, so
    /// fixing the file and resuming is one word rather than a new run.
    StoreBroken,
}

impl RunStatus {
    /// The status as written in a report.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Blocked => "blocked",
            Self::AwaitingOperator => "awaiting-operator",
            Self::BudgetExhausted => "budget-exhausted",
            Self::StoreBroken => "store-broken",
        }
    }

    /// `true` when the run can be picked up again where it stopped.
    pub fn is_resumable(self) -> bool {
        matches!(
            self,
            Self::Running
                | Self::Blocked
                | Self::AwaitingOperator
                | Self::BudgetExhausted
                | Self::StoreBroken
        )
    }
}

impl fmt::Display for RunStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What a run took a lock from, when it took one that was not its own.
///
/// `--take-lock` supersedes rather than erases: the stolen lock's contents go into the new run's
/// cursor, so *"this run took the lock from pid 4711 of run `AUTH-142/2`"* is in the record.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct StolenLock {
    /// The run that held it.
    pub run: String,
    /// The process that held it.
    pub pid: u32,
    /// The host it claimed to be on.
    pub host: String,
}

/// What the driver knows about a run that the engine's snapshot does not hold.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DriverCursor {
    /// Which run this is.
    pub run: RunId,
    /// The task being driven.
    pub task: TaskId,
    /// The engine's own identifier for the execution inside this run.
    pub execution: ExecutionId,
    /// The resolved workflow, as `<id>/<major>`.
    pub workflow: String,
    /// Which step map is driving.
    pub map: StepMapId,
    /// The digest of that map, validated.
    pub map_digest: String,
    /// The version of the engine that wrote the snapshot beside this cursor.
    pub engine_version: String,
    /// Which state the run is in.
    pub state: StateId,
    /// Which step of that state's list runs next.
    pub step: usize,
    /// How many times each state has been entered.
    pub visits: BTreeMap<StateId, u32>,
    /// How many attempts each step has had, keyed `<state>#<index>`.
    ///
    /// Spent and not reset. A retried step that succeeds does not erase the first attempt: there
    /// is no evidence to erase, because a step that produced no verdict submitted nothing, but the
    /// count stays so *"green on the third try"* is in the record.
    pub attempts: BTreeMap<String, u32>,
    /// How many loop iterations the run has taken.
    pub iterations: u32,
    /// Where the run got to.
    pub status: RunStatus,
    /// What the run says about itself, in the engine's words where there are any.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reasons: Vec<String>,
    /// The lock this run took from somebody else, if it took one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub took_lock_from: Option<StolenLock>,
}

impl DriverCursor {
    /// The key an attempt count is held under.
    pub fn attempt_key(state: &StateId, step: usize) -> String {
        format!("{state}#{step}")
    }

    /// How many attempts the step at `state`/`step` has had.
    pub fn attempts_at(&self, state: &StateId, step: usize) -> u32 {
        self.attempts
            .get(&Self::attempt_key(state, step))
            .copied()
            .unwrap_or(0)
    }

    /// Records one more attempt at `state`/`step`, returning the new count.
    pub fn record_attempt(&mut self, state: &StateId, step: usize) -> u32 {
        let counter = self
            .attempts
            .entry(Self::attempt_key(state, step))
            .or_insert(0);
        *counter += 1;
        *counter
    }

    /// How many times `state` has been entered.
    pub fn visits_of(&self, state: &StateId) -> u32 {
        self.visits.get(state).copied().unwrap_or(0)
    }

    /// Records one more entry into `state`, returning the new count.
    pub fn record_visit(&mut self, state: &StateId) -> u32 {
        let counter = self.visits.entry(state.clone()).or_insert(0);
        *counter += 1;
        *counter
    }

    /// Why a resume would be refused, or nothing when the three pins still hold.
    ///
    /// Fail closed, and name both values: the routes out are `--restart`, which allocates a new run
    /// id and re-observes the evidence, or reverting the document that moved.
    pub fn resume_refusal(
        &self,
        workflow: &str,
        map: &StepMapId,
        map_digest: &str,
        engine_version: &str,
    ) -> Option<String> {
        if self.workflow != workflow {
            return Some(format!(
                "this run is pinned to workflow `{}` and the task now resolves to `{workflow}`",
                self.workflow
            ));
        }
        if self.map != *map {
            return Some(format!(
                "this run was driven by step map `{}` and `{map}` was given",
                self.map
            ));
        }
        if self.map_digest != map_digest {
            return Some(format!(
                "step map `{map}` has changed since this run started (recorded {}, now {map_digest})",
                self.map_digest
            ));
        }
        if self.engine_version != engine_version {
            return Some(format!(
                "the snapshot was written by engine {} and this driver links engine {engine_version}",
                self.engine_version
            ));
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cursor() -> DriverCursor {
        DriverCursor {
            run: RunId::new(&TaskId::new("AUTH-142").unwrap(), 3).unwrap(),
            task: TaskId::new("AUTH-142").unwrap(),
            execution: ExecutionId::new("AUTH-142.1").unwrap(),
            workflow: "adp/default/1".to_owned(),
            map: StepMapId::new("development/default").unwrap(),
            map_digest: "abc".to_owned(),
            engine_version: "0.1.0".to_owned(),
            state: StateId::new("implement").unwrap(),
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
    fn a_run_id_round_trips_through_its_written_form() {
        let id: RunId = "AUTH-142/3".parse().expect("parses");
        assert_eq!(id.to_string(), "AUTH-142/3");
        assert_eq!(id.ordinal(), 3);
        assert_eq!(id.segments(), ["AUTH-142".to_owned(), "3".to_owned()]);
        assert!("AUTH-142/0".parse::<RunId>().is_err());
        assert!("AUTH-142".parse::<RunId>().is_err());
    }

    #[test]
    fn a_cursor_round_trips_through_json() {
        let mut cursor = cursor();
        cursor.record_visit(&StateId::new("implement").unwrap());
        cursor.record_attempt(&StateId::new("implement").unwrap(), 0);
        let text = serde_json::to_string(&cursor).expect("serialises");
        let read: DriverCursor = serde_json::from_str(&text).expect("deserialises");
        assert_eq!(read, cursor);
        assert_eq!(read.visits_of(&StateId::new("implement").unwrap()), 1);
        assert_eq!(read.attempts_at(&StateId::new("implement").unwrap(), 0), 1);
    }

    #[test]
    fn a_moved_workflow_map_digest_or_engine_refuses_a_resume() {
        let cursor = cursor();
        let map = StepMapId::new("development/default").unwrap();
        assert!(cursor
            .resume_refusal("adp/default/1", &map, "abc", "0.1.0")
            .is_none());
        for (workflow, digest, engine) in [
            ("adp/default/2", "abc", "0.1.0"),
            ("adp/default/1", "def", "0.1.0"),
            ("adp/default/1", "abc", "0.2.0"),
        ] {
            let refusal = cursor
                .resume_refusal(workflow, &map, digest, engine)
                .expect("refused");
            assert!(
                refusal.contains("adp/default/1")
                    || refusal.contains("abc")
                    || refusal.contains("0.1.0"),
                "a refusal names both values: {refusal}"
            );
        }
    }

    #[test]
    fn a_budget_exhausted_run_is_resumable_and_a_completed_one_is_not() {
        assert!(RunStatus::BudgetExhausted.is_resumable());
        assert!(RunStatus::AwaitingOperator.is_resumable());
        assert!(RunStatus::StoreBroken.is_resumable());
        assert!(!RunStatus::Completed.is_resumable());
    }
}
