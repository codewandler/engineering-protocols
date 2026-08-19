//! The suites.
//!
//! Each suite is one property family from the design specification's §78, and each is a plain
//! function of a backend. They are listed here with the level that requires them, so a backend's
//! claim — "we are audited-level" — is something a runner can check rather than something a README
//! asserts.

use crate::harness::Backend;
use crate::report::{Level, SuiteReport};

pub mod audit;
pub mod causation;
pub mod command_execution;
pub mod concurrency;
pub mod consistency;
pub mod correlation;
pub mod events;
pub mod history;
pub mod idempotency;
pub mod identity;
pub mod immutability;
pub mod provenance;
pub mod query;
pub mod rejected_audit;
pub mod relations;
pub mod type_registry;

/// One registered suite.
pub struct Suite {
    /// Its name, as used on a command line and in a report.
    pub name: &'static str,
    /// What it establishes, in one line.
    pub summary: &'static str,
    /// The weakest level that requires it.
    pub level: Level,
    /// How to run it.
    pub run: fn(&dyn ErasedBackend) -> SuiteReport,
}

/// A backend behind a trait object, so suites can be held in a table.
///
/// The contract's traits use `async fn`, which is not dyn-compatible, so this narrows the surface to
/// the blocking shape the suites actually use. It is an implementation detail of the registry, not
/// part of the contract.
pub trait ErasedBackend {
    /// Runs a suite's body against the concrete backend.
    fn identity(&self) -> SuiteReport;
    /// Runs the command-execution suite.
    fn command_execution(&self) -> SuiteReport;
    /// Runs the idempotency suite.
    fn idempotency(&self) -> SuiteReport;
    /// Runs the optimistic-concurrency suite.
    fn concurrency(&self) -> SuiteReport;
    /// Runs the query suite.
    fn query(&self) -> SuiteReport;
    /// Runs the consistency suite.
    fn consistency(&self) -> SuiteReport;
    /// Runs the relations suite.
    fn relations(&self) -> SuiteReport;
    /// Runs the history suite.
    fn history(&self) -> SuiteReport;
    /// Runs the immutability suite.
    fn immutability(&self) -> SuiteReport;
    /// Runs the audit suite.
    fn audit(&self) -> SuiteReport;
    /// Runs the rejected-action audit suite.
    fn rejected_audit(&self) -> SuiteReport;
    /// Runs the correlation suite.
    fn correlation(&self) -> SuiteReport;
    /// Runs the causation suite.
    fn causation(&self) -> SuiteReport;
    /// Runs the provenance suite.
    fn provenance(&self) -> SuiteReport;
    /// Runs the events suite.
    fn events(&self) -> SuiteReport;
    /// Runs the type-registry suite.
    fn type_registry(&self) -> SuiteReport;
}

impl<B: Backend> ErasedBackend for B {
    fn identity(&self) -> SuiteReport {
        identity::run(self)
    }
    fn command_execution(&self) -> SuiteReport {
        command_execution::run(self)
    }
    fn idempotency(&self) -> SuiteReport {
        idempotency::run(self)
    }
    fn concurrency(&self) -> SuiteReport {
        concurrency::run(self)
    }
    fn query(&self) -> SuiteReport {
        query::run(self)
    }
    fn consistency(&self) -> SuiteReport {
        consistency::run(self)
    }
    fn relations(&self) -> SuiteReport {
        relations::run(self)
    }
    fn history(&self) -> SuiteReport {
        history::run(self)
    }
    fn immutability(&self) -> SuiteReport {
        immutability::run(self)
    }
    fn audit(&self) -> SuiteReport {
        audit::run(self)
    }
    fn rejected_audit(&self) -> SuiteReport {
        rejected_audit::run(self)
    }
    fn correlation(&self) -> SuiteReport {
        correlation::run(self)
    }
    fn causation(&self) -> SuiteReport {
        causation::run(self)
    }
    fn provenance(&self) -> SuiteReport {
        provenance::run(self)
    }
    fn events(&self) -> SuiteReport {
        events::run(self)
    }
    fn type_registry(&self) -> SuiteReport {
        type_registry::run(self)
    }
}

/// Every suite, in the order a runner should run them.
///
/// The order is not arbitrary: identity and command execution come first because everything else
/// assumes they work, and a failure there explains failures further down.
pub fn all() -> Vec<Suite> {
    vec![
        Suite {
            name: "identity",
            summary: "identity is opaque, stable and never reused",
            level: Level::Core,
            run: |backend| backend.identity(),
        },
        Suite {
            name: "command-execution",
            summary: "a mutation produces a revision, and is readable afterwards",
            level: Level::Core,
            run: |backend| backend.command_execution(),
        },
        Suite {
            name: "idempotency",
            summary: "a replayed command applies once and returns its original result",
            level: Level::Core,
            run: |backend| backend.idempotency(),
        },
        Suite {
            name: "concurrency",
            summary: "a stale write is refused rather than merged",
            level: Level::Core,
            run: |backend| backend.concurrency(),
        },
        Suite {
            name: "query",
            summary: "filters are applied rather than ignored",
            level: Level::Core,
            run: |backend| backend.query(),
        },
        Suite {
            name: "consistency",
            summary: "a read can demand a view no older than a given write",
            level: Level::Core,
            run: |backend| backend.consistency(),
        },
        Suite {
            name: "relations",
            summary: "edges resolve, and are answerable in both directions",
            level: Level::Core,
            run: |backend| backend.relations(),
        },
        Suite {
            name: "history",
            summary: "every revision is recorded, in order",
            level: Level::Audited,
            run: |backend| backend.history(),
        },
        Suite {
            name: "immutability",
            summary: "nothing is deleted, and immutable types are not edited",
            level: Level::Audited,
            run: |backend| backend.immutability(),
        },
        Suite {
            name: "audit",
            summary: "every mutation leaves a record naming who and what",
            level: Level::Audited,
            run: |backend| backend.audit(),
        },
        Suite {
            name: "rejected-audit",
            summary: "a refused command leaves a record and changes nothing",
            level: Level::Audited,
            run: |backend| backend.rejected_audit(),
        },
        Suite {
            name: "correlation",
            summary: "one activity is reassembled from one identifier",
            level: Level::Audited,
            run: |backend| backend.correlation(),
        },
        Suite {
            name: "causation",
            summary: "the immediate cause of each step is recoverable",
            level: Level::Audited,
            run: |backend| backend.causation(),
        },
        Suite {
            name: "provenance",
            summary: "every entity says who created it and who changed it last",
            level: Level::Audited,
            run: |backend| backend.provenance(),
        },
        Suite {
            name: "events",
            summary: "a command reports the events it emitted",
            level: Level::Full,
            run: |backend| backend.events(),
        },
        Suite {
            name: "type-registry",
            summary: "a type can be described without hard-coding it",
            level: Level::Full,
            run: |backend| backend.type_registry(),
        },
    ]
}

/// The suites required at `level`.
pub fn for_level(level: Level) -> Vec<Suite> {
    all()
        .into_iter()
        .filter(|suite| level.includes(suite.level))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_suite_is_registered_exactly_once() {
        let suites = all();
        let mut names: Vec<&str> = suites.iter().map(|suite| suite.name).collect();
        names.sort_unstable();
        let unique = {
            let mut unique = names.clone();
            unique.dedup();
            unique
        };
        assert_eq!(
            names, unique,
            "a suite registered twice runs twice and reports twice"
        );
        assert_eq!(suites.len(), 16, "§78 names sixteen property families");
    }

    #[test]
    fn levels_select_a_growing_set_of_suites() {
        let core = for_level(Level::Core).len();
        let audited = for_level(Level::Audited).len();
        let full = for_level(Level::Full).len();

        assert!(
            core < audited && audited < full,
            "{core} < {audited} < {full}"
        );
        assert_eq!(full, all().len(), "full means everything");
        assert!(
            for_level(Level::Core)
                .iter()
                .all(|suite| suite.level == Level::Core),
            "a core claim must not be checked against audited-level properties"
        );
    }
}
