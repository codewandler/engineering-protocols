//! Does the suite catch anything?
//!
//! This is the test that makes the rest of the crate trustworthy. Every other test here asks whether
//! a correct backend passes; these ask whether an incorrect one *fails*, and which suite notices.
//!
//! Without them, a suite that quietly checked nothing would look exactly like a suite that checked
//! everything and found no problems.

use aep_backend_memory::MemoryBackend;
use aep_conformance::{run, Fault, FaultyBackend, Level, SuiteReport};

/// The suites that fail against a backend carrying `fault`.
fn failing_suites(fault: Fault) -> Vec<String> {
    let backend = FaultyBackend::new(MemoryBackend::new(), fault);
    run(&backend, Level::Full)
        .failing_suites()
        .map(|suite| suite.suite.to_owned())
        .collect()
}

#[test]
fn the_reference_backend_passes_every_level() {
    for level in Level::ALL {
        let report = run(&MemoryBackend::new(), *level);
        assert!(
            report.passed(),
            "the reference backend must pass the level it claims:\n{report}"
        );
        assert!(
            report.checks() > 0,
            "{level} checked nothing, which is a failure of the suites rather than of the backend"
        );
    }
}

#[test]
fn every_suite_checks_something() {
    let report = run(&MemoryBackend::new(), Level::Full);
    for suite in &report.suites {
        assert!(
            !suite.is_empty(),
            "the `{}` suite recorded no checks; a suite that checks nothing passes everything",
            suite.suite
        );
        assert!(
            suite.len() >= 4,
            "the `{}` suite recorded only {} check(s); §78 property families are not that small",
            suite.suite,
            suite.len()
        );
    }
}

#[test]
fn each_fault_is_caught_by_the_suite_that_exists_to_catch_it() {
    let mut uncaught: Vec<String> = Vec::new();

    for fault in Fault::ALL {
        let failing = failing_suites(*fault);
        if !failing.iter().any(|suite| suite == fault.caught_by()) {
            uncaught.push(format!(
                "{fault:?} ({}) went unnoticed by the `{}` suite; suites that did fail: {:?}",
                fault.describe(),
                fault.caught_by(),
                failing
            ));
        }
    }

    assert!(
        uncaught.is_empty(),
        "a suite that does not catch its own fault is not protecting the property it claims:\n  {}",
        uncaught.join("\n  ")
    );
}

#[test]
fn a_fault_does_not_simply_break_everything() {
    // If injecting one fault failed every suite, the suite boundaries would carry no information:
    // a failure would tell an implementer to look everywhere.
    let total = run(&MemoryBackend::new(), Level::Full).suites.len();

    for fault in Fault::ALL {
        let failing = failing_suites(*fault);
        assert!(
            failing.len() < total,
            "{fault:?} failed all {total} suites, so the report says nothing about where to look"
        );
    }
}

#[test]
fn a_faults_collateral_is_explainable() {
    // Some faults legitimately affect more than one suite: dropping every audit record breaks
    // correlation and causation too, because both read audit records. What must not happen is a
    // fault whose blast radius nobody can account for.
    // Each allowance is a claim about why the blast radius is what it is. `DropAffected` is the
    // widest by a distance, and that is a finding rather than a flaw in the suites: a command that
    // does not say what it changed leaves a caller unable to address the thing it just created, so
    // every suite that creates something before asking a question about it fails. `affected` is
    // load-bearing for the whole contract, which is worth knowing.
    let expected_collateral: &[(Fault, usize)] = &[
        (Fault::DropAffected, 8),
        (Fault::DropAudit, 5),
        (Fault::ScrambleCorrelation, 3),
        (Fault::DropCausation, 3),
        (Fault::DropRejectionAudit, 2),
    ];

    for fault in Fault::ALL {
        let failing = failing_suites(*fault);
        let allowance = expected_collateral
            .iter()
            .find(|(known, _)| known == fault)
            .map_or(1, |(_, allowance)| *allowance);
        assert!(
            failing.len() <= allowance,
            "{fault:?} failed {} suites ({failing:?}) but is only accounted for {allowance}; either \
             a suite is over-reaching or the allowance needs updating with a reason",
            failing.len()
        );
    }
}

#[test]
fn a_named_suite_can_be_run_on_its_own() {
    let backend = FaultyBackend::new(MemoryBackend::new(), Fault::ReplayApplies);
    let report: SuiteReport =
        aep_conformance::run_suite(&backend, "idempotency").expect("the suite is registered");
    assert!(!report.passed(), "{report}");

    let unrelated =
        aep_conformance::run_suite(&backend, "type-registry").expect("the suite is registered");
    assert!(
        unrelated.passed(),
        "a fault in replay handling says nothing about type discovery:\n{unrelated}"
    );

    assert!(aep_conformance::run_suite(&backend, "vibes").is_none());
}

#[test]
fn a_core_claim_is_not_checked_against_audited_properties() {
    // A backend claiming `core` should not be failed for something `core` does not require. This is
    // what makes a level a promise rather than a hurdle.
    let backend = FaultyBackend::new(MemoryBackend::new(), Fault::DropAudit);
    let core = run(&backend, Level::Core);
    assert!(
        core.passed(),
        "dropping audit records must not fail a core-level claim:\n{core}"
    );

    let audited = run(&backend, Level::Audited);
    assert!(!audited.passed(), "but it must fail an audited-level one");
}
