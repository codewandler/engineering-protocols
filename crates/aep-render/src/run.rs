//! What a run looks like from outside, as this crate needs it.
//!
//! # Why this is a plain struct and not the engine's `Snapshot`
//!
//! Decision 1 of the renderer plan, and it is a dependency decision before it is a modelling one.
//! `Snapshot` lives in `aep-engine`, the driver's cursor in `aep-driver-spec`, and a renderer that
//! took either as its input would drag that crate — and everything behind it — into a tree whose
//! whole job is to write text. [`RunView`] is the seam instead: the caller owns the conversion, and
//! this crate depends on `aep-domain` alone.
//!
//! The practical gain is that a run is not the only thing that can be drawn. A test builds a
//! `RunView` by hand in four lines; `protocol workflow render --state` builds one from a snapshot
//! with no driver anywhere; `--run` builds one from a snapshot *and* a cursor, which is the only
//! combination that knows why a run stopped. None of the three needs a different renderer.
//!
//! # The path, not a set of visited states
//!
//! [`RunView::path`] is the states a run **entered, in order**, because a set cannot answer the
//! question the overlay exists to answer: *did this run go back?* Consecutive pairs of the path are
//! the transitions the run actually took, so `verify → implement` appearing in it is what turns
//! that edge amber. A `BTreeSet<StateId>` would have drawn the same nine boxes and lost the retreat.

use std::collections::{BTreeMap, BTreeSet};

use aep_domain::ids::StateId;

/// How a run stands.
///
/// Deliberately this crate's own enum rather than the driver's `RunStatus`, for the reason the whole
/// module exists: a renderer that named the driver's type would depend on the driver. The variants
/// are not a copy either — [`RunStatus::Unknown`] has no counterpart there, and it is the honest
/// answer when the caller has a snapshot and no cursor, which is exactly what `--state` gives it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum RunStatus {
    /// Nobody said. A snapshot on its own cannot know why a run stopped, and guessing `Running`
    /// would put a moving-looking overlay on a run that died three days ago.
    #[default]
    Unknown,
    /// Still moving.
    Running,
    /// The workflow reached its terminal state.
    Completed,
    /// Nothing can move, and the engine said why. The reasons are in [`RunView::reasons`].
    Blocked,
    /// A person owes the run an answer.
    Waiting,
    /// A visit or retry budget ran out.
    Exhausted,
    /// The documents the run reads stopped parsing.
    ///
    /// Distinct from [`RunStatus::Blocked`] on purpose, and the distinction is the driver's:
    /// *blocked* is the protocol saying no, and a typo in a file is not the protocol saying
    /// anything.
    Broken,
}

impl RunStatus {
    /// The word a report and a footer use for this status.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Blocked => "blocked",
            Self::Waiting => "awaiting-operator",
            Self::Exhausted => "budget-exhausted",
            Self::Broken => "store-broken",
        }
    }

    /// `true` when this status is the run saying *nothing further will happen without help*.
    ///
    /// What it is used for: the current state's box is drawn in red rather than in the accent, and
    /// the reasons are printed under the diagram. [`RunStatus::Completed`] is not stopped in that
    /// sense — it is finished, which is the opposite claim.
    pub fn is_stopped(self) -> bool {
        matches!(self, Self::Blocked | Self::Exhausted | Self::Broken)
    }
}

impl std::fmt::Display for RunStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One run, as much of it as a picture can show.
///
/// Built by the caller. `protocol workflow render` fills it from the engine's snapshot and the
/// driver's cursor; a test fills it by hand.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RunView {
    /// Which run this is, as the driver names one — `AUTH-142/3`. Absent when the caller only had a
    /// snapshot, which carries no run id.
    pub run: Option<String>,
    /// The task being worked. Absent for the same reason a run id can be.
    pub task: Option<String>,
    /// Where the run stands.
    pub status: RunStatus,
    /// Which state the run is in.
    ///
    /// `None` renders as the bare workflow: every box unmarked, which is what `render` without
    /// `--run` or `--state` draws.
    pub current: Option<StateId>,
    /// The states entered, in order, oldest first.
    ///
    /// Consecutive pairs are the transitions the run took. See the module documentation for why
    /// this is a sequence and not a set.
    pub path: Vec<StateId>,
    /// How many times each state was entered, when the caller knows. A state visited three times
    /// says so on its box; a state absent from this map says nothing rather than *once*.
    pub visits: BTreeMap<StateId, u32>,
    /// How many records of each evidence kind the run has produced.
    pub evidence: BTreeMap<String, u32>,
    /// Why the run stopped, **verbatim**.
    ///
    /// Never paraphrased, never truncated and never re-ordered by any emitter. These are the
    /// engine's own sentences about what is outstanding, and a renderer that summarised them would
    /// be answering a question it did not evaluate.
    pub reasons: Vec<String>,
    /// How many loop iterations the driver took, when a cursor said.
    pub iterations: Option<u32>,
}

impl RunView {
    /// A view of a run sitting in `current`, with nothing else known.
    ///
    /// The path is seeded with `current`, because a run is in a state it has by definition entered.
    pub fn at(current: StateId) -> Self {
        Self {
            current: Some(current.clone()),
            path: vec![current],
            ..Self::default()
        }
    }

    /// Every state this run has been in.
    pub fn visited(&self) -> BTreeSet<&StateId> {
        self.path.iter().collect()
    }

    /// `true` when the run has been in `state`.
    pub fn has_visited(&self, state: &StateId) -> bool {
        self.path.iter().any(|entry| entry == state)
    }

    /// The transitions this run took, as `(from, to)` pairs in the order they were taken.
    ///
    /// A pair may repeat: a run that went round the `verify → implement` loop twice took that
    /// transition twice, and collapsing the repeats here would make [`RunView::times_taken`] a
    /// boolean with a number's name.
    pub fn taken(&self) -> impl Iterator<Item = (&StateId, &StateId)> {
        self.path.windows(2).map(|pair| (&pair[0], &pair[1]))
    }

    /// How many times the run moved from `from` to `to`.
    pub fn times_taken(&self, from: &StateId, to: &StateId) -> usize {
        self.taken()
            .filter(|(left, right)| *left == from && *right == to)
            .count()
    }

    /// How many times `state` was entered, or `None` when the caller did not say.
    pub fn visits_of(&self, state: &StateId) -> Option<u32> {
        self.visits.get(state).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(id: &str) -> StateId {
        StateId::new(id).expect("a legal state id")
    }

    #[test]
    fn a_run_that_went_back_reports_the_back_edge_twice_not_once() {
        let view = RunView {
            path: vec![
                state("implement"),
                state("verify"),
                state("implement"),
                state("verify"),
                state("implement"),
            ],
            ..RunView::default()
        };
        // The set has lost it entirely; the sequence has kept the count.
        assert_eq!(
            view.visited().len(),
            2,
            "the fixture reaches only two states"
        );
        assert_eq!(
            view.times_taken(&state("verify"), &state("implement")),
            2,
            "verification failed twice, and the overlay has to be able to say so"
        );
        assert_eq!(view.taken().count(), 4, "four moves produced two states");
    }

    #[test]
    fn a_snapshot_without_a_cursor_is_unknown_and_not_running() {
        let view = RunView::at(state("implement"));
        assert_eq!(
            view.status,
            RunStatus::Unknown,
            "the default must not claim a run is moving; only a cursor knows that"
        );
        assert!(!view.status.is_stopped());
        assert!(view.has_visited(&state("implement")));
        assert_eq!(
            view.visits_of(&state("implement")),
            None,
            "unknown is not one"
        );
    }

    #[test]
    fn blocked_and_exhausted_are_stopped_and_completed_is_not() {
        assert!(RunStatus::Blocked.is_stopped());
        assert!(RunStatus::Exhausted.is_stopped());
        assert!(RunStatus::Broken.is_stopped());
        assert!(!RunStatus::Completed.is_stopped());
        assert_eq!(RunStatus::Waiting.to_string(), "awaiting-operator");
    }
}
