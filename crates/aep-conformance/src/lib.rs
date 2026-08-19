//! Black-box conformance suites for AEP backends.
//!
//! A contract with one implementation is a description of that implementation. These suites are how
//! a second one proves it belongs: they drive a backend through the
//! [`aep-contract`](aep_contract) traits and check *properties*, never internals.
//!
//! ```no_run
//! # fn main() {
//! use aep_backend_memory::MemoryBackend;
//! use aep_conformance::{run, Level};
//!
//! let report = run(&MemoryBackend::new(), Level::Full);
//! assert!(report.passed(), "{report}");
//! # }
//! ```
//!
//! # What makes this a suite rather than a smoke test
//!
//! Every suite is checked against a backend that is deliberately broken in exactly the way that
//! suite exists to catch — see [`faulty`]. A suite that passes everything is not a suite, and the
//! only way to know is to hand it something wrong and watch it complain.
//!
//! # No sleeping
//!
//! Ordering is established with consistency tokens ([`aep_contract::consistency`]), never with
//! delays. A suite that sleeps tests the machine it runs on: the first slow CI box turns a correct
//! backend red, and the fix everyone reaches for is a longer sleep.

pub mod faulty;
pub mod harness;
pub mod report;
pub mod suites;

pub use faulty::{Fault, FaultyBackend};
pub use harness::{Backend, Harness};
pub use report::{Check, ConformanceReport, Level, SuiteReport};

/// Runs every suite required at `level`.
pub fn run<B: Backend>(backend: &B, level: Level) -> ConformanceReport {
    let suites = suites::for_level(level)
        .iter()
        .map(|suite| (suite.run)(backend))
        .collect();
    ConformanceReport { level, suites }
}

/// Runs one suite by name, for a targeted re-check.
pub fn run_suite<B: Backend>(backend: &B, name: &str) -> Option<SuiteReport> {
    suites::all()
        .iter()
        .find(|suite| suite.name == name)
        .map(|suite| (suite.run)(backend))
}
