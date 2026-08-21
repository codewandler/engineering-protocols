//! D6's refusal, rendered from a lock somebody else observed.
//!
//! **This crate is handed a [`LockState`] and probes nothing.** It never reads a pid table, never
//! reads a hostname and never reads a clock. That placement is not tidiness: a liveness probe reads
//! ambient OS state and uses neither `SystemTime::now` nor `rand`, so `tests/determinism.rs` would
//! not catch it, and placement is the only thing keeping this crate's purity claim true (review
//! finding **F19**). It also makes the refusal testable without spawning a second process.
//!
//! The lock itself is one fixed path per store — `.engineering/runs/lock.json`, created with
//! `create_new` **before** any run id is allocated — and it belongs to `protocol-cli`, along with
//! the run directory it grants. The first draft of D6 put the lock *inside* the directory the lock
//! was allocating, which has no order in which it can execute: two invocations count the existing
//! directories, get `3` and `4`, and **both** `create_new` succeed. That is D6's own rejected
//! option, *"no lock, last writer wins"*, reached by accident (review finding **F2**).
//!
//! # Staleness is liveness, never age
//!
//! Any age threshold has to exceed the longest legitimate step, and the longest legitimate step is
//! *an operator step waiting for a person*, which has no bound. A driver that broke a lock after two
//! hours would break exactly the runs that paused correctly. Requiring `--take-lock` makes stealing
//! a lock something a person did.
//!
//! | condition | verdict |
//! |---|---|
//! | pid alive | held — refuse |
//! | same host, pid not alive | **stale, and still refused** without `--take-lock` |
//! | different host | never stale, whatever the local pid table says |

use std::fmt;

/// What the caller found out about the process named in a lock.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Liveness {
    /// The process is running on this host.
    Alive,
    /// The process is not running, and this host is the one that would know.
    Dead,
    /// The lock was taken on another host, so the local pid table answers nothing.
    ///
    /// Not a third shade of alive: a pid on another machine is a number about a process this one
    /// cannot see, and treating an unanswerable question as *dead* is how two runs end up sharing a
    /// store.
    OtherHost,
}

impl Liveness {
    /// The finding as written in a refusal.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Alive => "alive",
            Self::Dead => "not running",
            Self::OtherHost => "on another host",
        }
    }
}

impl fmt::Display for Liveness {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A store lock as its holder wrote it, plus what the caller observed about the holder.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LockState {
    /// The run that holds it.
    pub run: String,
    /// The process that holds it.
    pub pid: u32,
    /// The host it claims to be on.
    pub host: String,
    /// What the caller found out about that process.
    pub liveness: Liveness,
}

impl LockState {
    /// `true` when the holder is provably dead **on this host**, so `--take-lock` may supersede it.
    ///
    /// Stale is not free: a stale lock is still refused without `--take-lock`. Being stale only
    /// means a route out exists.
    pub fn is_stale(&self) -> bool {
        matches!(self.liveness, Liveness::Dead)
    }

    /// The line to print when this lock stands in the way, naming both routes out.
    ///
    /// `taking` says whether the caller offered `--take-lock`. Refusing while naming what to do
    /// instead is the same choice `protocol artifact move` makes for an illegal transition: the
    /// refusal **is** the answer, and a refusal that does not name the answer is a puzzle.
    ///
    /// The one combination in which this lock does not refuse — stale, with `--take-lock` given —
    /// returns the supersession line rather than a refusal, so a caller that prints it
    /// unconditionally still prints something true. The record of the theft itself belongs in the
    /// new run's cursor (`StolenLock`), because `--take-lock` supersedes rather than erases.
    pub fn refusal(&self, taking: bool) -> String {
        let holder = format!(
            "run `{}` holds the store lock (pid {} on {}, {})",
            self.run, self.pid, self.host, self.liveness
        );
        match (self.liveness, taking) {
            (Liveness::Alive, true) => format!(
                "{holder}. `--take-lock` is refused while the holder is alive: resume that run \
                 with `--resume`, or wait for it to finish"
            ),
            (Liveness::Alive, false) => format!(
                "{holder}. Resume that run with `--resume`, or wait for it; `--take-lock` \
                 supersedes a lock only when its holder is provably dead on this host"
            ),
            (Liveness::Dead, true) => format!(
                "{holder}, so `--take-lock` supersedes it; pid {} is recorded in this run's cursor \
                 as the lock it took",
                self.pid
            ),
            (Liveness::Dead, false) => format!(
                "{holder}. A stale lock is still refused: pass `--take-lock` to supersede it, or \
                 `--resume` run `{}`",
                self.run
            ),
            (Liveness::OtherHost, _) => format!(
                "{holder}. A lock held on another host is never stale, whatever this machine's pid \
                 table says, so `--take-lock` is refused: resume run `{}` on {}, or clear the lock \
                 there",
                self.run, self.host
            ),
        }
    }
}
