//! Does the suite catch anything?
//!
//! `tests/execution.rs` shows that 29 generated scenarios pass against an implementation written by
//! hand from the same specification. That is the *satisfiability* claim, and it is the weaker half:
//! a suite that asserted nothing would pass exactly as green.
//!
//! This is the other half — design §25 and §26. Every test below runs a **deliberately wrong**
//! implementation and asks which named scenario noticed. Two properties make the matrix worth
//! anything, and both are asserted mechanically rather than read off a table:
//!
//! 1. each fault fails **the one scenario that exists to catch it** — not "the run went red", which
//!    a single panic would also achieve;
//! 2. a fault **does not simply break everything**, held to a per-fault allowance that has to be
//!    changed with a reason rather than relaxed.
//!
//! The matrix prints itself:
//!
//! ```console
//! cargo test -p ess-conformance --test faults -- --nocapture a_faults_blast_radius_is_accounted_for
//! ```
//!
//! # The rows worth reading are the ones nothing catches, and every one recorded has been closed
//!
//! A fault in [`Fault::ALL`] marked [`Caught::Nothing`] is not a hole in this file; it is the
//! finding, and [`a_fault_nothing_catches_is_recorded_rather_than_quietly_dropped`] asserts that a
//! **correct** run still comes back for it — so the day the gap closes, the row fails here and has
//! to be rewritten rather than forgotten. That is what happened to all three that were recorded:
//! `extra-event` and `drop-consistency-token` were closed by teaching synthesis to ask for more,
//! and `wrong-event-payload` — the one that separated what a specification *declares* from what it
//! merely *names* — needed the model itself to change. It publishes `InvoicePaid` with an amount
//! nobody submitted; every field the event declares is present and well-typed, and until an
//! outcome could say `amount: input.amount` there was no check to make that was not a guess. The
//! wave 6.5 `payload:` construct is that declaration, and the row moved. `partial-event-payload`
//! is the same event with a declared field missing, caught all along, because the type was always
//! declared. `crates/ess-conformance/src/faulty.rs` argues both.
//!
//! `wrong-refusal-error` is the row that arrived the other way round. It was uncatchable and is not
//! recorded as such, because the repair landed in the same change: a command can now declare what it
//! answers when its subject is in a state its moves do not start from, so an implementation that
//! refuses with the wrong error is a defect a named scenario reports rather than a difference no
//! assertion could see.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use ess_compiler::ir::EssIr;
use ess_compiler::resolve::compile;
use ess_compiler::source::SourceMap;
use ess_conformance::faulty::{self, Caught, Fault, Injection, System};
use ess_conformance::reference::{Billing, Oracle, Untraced};
use ess_conformance::report::{CheckCode, ConformanceReport, ConformanceStatus, Status};
use ess_conformance::runner::Runner;
use ess_conformance::scenario::ConformanceSuite;
use ess_conformance::synthesize::synthesize;
use ess_conformance::target::{ConformanceTarget, ImplementationIdentity};
use ess_domain::spec::{RawSpecFile, Specification};
use ess_domain::system::Source;

// ---- the specifications under test ---------------------------------------------------------------

/// An example directory, compiled from the files it lives in rather than from a copy inlined here.
fn example(name: &str) -> EssIr {
    let base = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples")
        .join(name)
        .canonicalize()
        .unwrap_or_else(|error| panic!("`{name}` exists: {error}"));

    let mut found: Vec<PathBuf> = Vec::new();
    let mut pending = vec![base.clone()];
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(&directory).expect("the example is readable") {
            let path = entry.expect("an entry").path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|it| it == "yaml") {
                found.push(path);
            }
        }
    }
    found.sort();

    let mut sources = SourceMap::new();
    let mut parsed = Vec::new();
    for path in found {
        let label = path
            .strip_prefix(&base)
            .expect("inside the example")
            .display()
            .to_string();
        let text = std::fs::read_to_string(&path).expect("readable");
        let raw = RawSpecFile::parse(&text)
            .unwrap_or_else(|error| panic!("{label} is well formed: {error}"));
        sources.insert(label.clone(), text);
        parsed.push((Source::new(label), raw));
    }
    let specification = Specification::assemble(parsed)
        .unwrap_or_else(|errors| panic!("`{name}` validates:\n{errors}"));
    compile(&specification, &sources)
        .unwrap_or_else(|diagnostics| panic!("`{name}` resolves:\n{diagnostics}"))
}

/// The suite one of the two systems obliges.
fn suite(system: System) -> ConformanceSuite {
    synthesize(&example(system.directory())).suite
}

/// The report a run of `system`'s suite against `target` produces.
fn run<T: ConformanceTarget>(system: System, target: &T) -> ConformanceReport {
    let suite = suite(system);
    Runner::for_suite(&suite).run(&suite, target)
}

/// The report a run against the implementation carrying `fault` produces.
///
/// Which of the two references is wrapped comes from the fault itself, so a row cannot be run
/// against a specification that does not declare what it breaks.
fn injected(fault: Fault) -> ConformanceReport {
    match fault.system() {
        System::Billing => run(fault.system(), &faulty::billing(fault)),
        System::Oracle => run(fault.system(), &faulty::oracle(fault)),
    }
}

/// Every scenario that did not pass, with the status it came to.
fn not_passed(report: &ConformanceReport) -> Vec<(String, Status)> {
    report
        .failures()
        .map(|result| (result.scenario.to_string(), result.status))
        .collect()
}

/// The status of one named scenario, or `None` when the suite holds no such scenario.
fn status_of(report: &ConformanceReport, scenario: &str) -> Option<Status> {
    report
        .scenarios
        .iter()
        .find(|result| result.scenario.to_string() == scenario)
        .map(|result| result.status)
}

// ---- the control group -----------------------------------------------------------------------

#[test]
fn each_specification_is_passed_in_full_by_the_implementation_written_from_it() {
    // Without this, every row below could be read as "the target is broken in some way", and the
    // matrix would be measuring the reference rather than the suite. Both references, because the
    // oracle fixture is where §26's second claim is made and a fixture nothing passes proves less
    // than nothing.
    for (system, scenarios) in [(System::Billing, 29), (System::Oracle, 31)] {
        let report = match system {
            System::Billing => run(system, &Billing::new()),
            System::Oracle => run(system, &Oracle::new()),
        };
        assert_eq!(
            report.scenarios.len(),
            scenarios,
            "`{}` obliges a different number of scenarios than the matrix was measured against",
            system.directory()
        );
        assert_eq!(
            report.status,
            ConformanceStatus::Passed,
            "the reference is written from this specification, so a scenario it fails is a defect \
             in one of the two:\n{report}"
        );
    }
}

// ---- property one: each fault fails the scenario that exists to catch it ------------------------

#[test]
fn each_fault_fails_the_scenario_that_exists_to_catch_it() {
    // §25's important invariant, and the reason it is stated in the negative there: "a generic panic
    // that causes the entire suite to fail proves nothing". So this asserts a *named* scenario, and
    // asserts `failed` specifically — `unsupported` would mean nobody found out, which is the
    // degradation `a_wrong_mapping_is_invisible_to_a_target_that_cannot_show_its_invocations` below
    // shows is real.
    let mut missed: Vec<String> = Vec::new();

    for fault in Fault::ALL {
        let Caught::By(scenario) = fault.caught() else {
            continue;
        };
        let report = injected(*fault);
        match status_of(&report, scenario) {
            None => missed.push(format!(
                "{fault:?} names `{scenario}`, which `{}` does not oblige at all",
                fault.system().directory()
            )),
            Some(Status::Failed) => {}
            Some(status) => missed.push(format!(
                "{fault:?} ({}) left `{scenario}` at `{status}`; scenarios that did not pass: {:?}",
                fault.describe(),
                not_passed(&report)
            )),
        }
    }

    assert!(
        missed.is_empty(),
        "a scenario that does not catch its own fault is not protecting the property it names:\n  \
         {}",
        missed.join("\n  ")
    );
}

#[test]
fn the_diagnostic_of_a_caught_fault_names_the_defect_rather_than_reporting_that_something_broke() {
    // §29: repair feedback, not a red light. The fault is the one whose defect is furthest from the
    // assertion that catches it — a binding reading the wrong field of an event — so the diagnostic
    // has the most work to do.
    let report = injected(Fault::WrongMapping);
    let result = report
        .scenarios
        .iter()
        .find(|result| result.scenario.to_string() == "handoff-on-placed/binding/mapping")
        .expect("the mapping scenario ran");

    let diagnostic = result
        .diagnostics()
        .find(|diagnostic| diagnostic.code == CheckCode::Invocation)
        .expect("the invocation check produced a diagnostic")
        .to_string();
    for required in [
        "ESS-CF-INVOCATION",
        "handoff-on-placed",
        "oracle.dispatch.Handoff",
        "expected:",
        r#"oracle.dispatch.Handoff.recipient = "contact""#,
        r#"recipient = "alternate_contact""#,
    ] {
        assert!(
            diagnostic.contains(required),
            "a wrong mapping is only repairable if the report names the value that was passed and \
             the one that was declared; {required:?} is missing from:\n{diagnostic}"
        );
    }
}

// ---- property two: a fault does not simply break everything -------------------------------------

#[test]
fn a_fault_does_not_simply_break_everything() {
    // If one fault failed every scenario, the scenario boundaries would carry no information at all:
    // a failure would tell an implementer to look everywhere.
    for fault in Fault::ALL {
        let report = injected(*fault);
        let total = report.scenarios.len();
        let broken = not_passed(&report).len();
        assert!(
            broken < total,
            "{fault:?} broke all {total} scenarios, so the report says nothing about where to look"
        );
    }
}

#[test]
fn a_faults_blast_radius_is_accounted_for() {
    // `aep_conformance::tests::faults`'s table, with ESS scenarios where AEP suite names are. Each
    // allowance is a *claim about why* the radius is what it is, so exceeding one means either a
    // scenario is over-reaching or the allowance needs updating with a reason. The default is one:
    // a new fault that touches anything beyond its own scenario has to say so here.
    //
    //   WrongEvent            24  `InvoiceCreated` carries the identity 17 further scenarios are
    //                             arranged from — including the two §20 value-object checks, whose
    //                             rows are put in place by `CreateInvoice` — so renaming it stops
    //                             them being set up at all; see the test below, which pins that
    //                             they come back as `error` rather than as 24 separate verdicts
    //                             about the implementation.
    //   DropBinding            4  the four aspects of the binding that stopped running: its flow,
    //                             its mapping, its delivery and its failure policy. The other two
    //                             bindings' seven scenarios stay green, which is the whole reason
    //                             `examples/oracle-fixture/` exists.
    //   StaleReadYourWrites    4  every scenario whose read-your-writes assertion is *positive* —
    //                             now including the §20 value check at `OutstandingInvoices.total`,
    //                             whose first stale read answers an empty view and "every row, and
    //                             at least one" demands a row. The four that assert a view
    //                             `Excludes` a row pass, because a view answering from before the
    //                             write legitimately excludes it.
    //   AllowIllegalTransition 2  billing declares two states `cancel` must not run from, `Paid`
    //                             and `Cancelled`, so one missing guard is two refusals.
    //   IgnoreExternalOutcome  2  the forced failure is what makes the escalation reachable, so the
    //                             binding's failure policy has nothing to observe either.
    //   DropConsistencyToken   9  every scenario that reads a `read_your_writes` view — the ninth
    //                             is the §20 value check at `OutstandingInvoices.total`. The token
    //                             is missing from *every* command result, so §14's demand can be
    //                             made nowhere; the row designates one of the nine because a matrix
    //                             row names a scenario, not because the other eight are collateral.
    //   ExtraEvent             4  the four scenarios whose asserted command is `CancelInvoice`: its
    //                             own branch, the `cancel` move, and the two states that move may
    //                             not run from — where the stray `InvoicePaid` is now caught too,
    //                             because a command nothing honoured may publish nothing declared.
    //   PartialEventPayload    2  `PayInvoice/settled` is asserted twice, once as §10's branch and
    //                             once as §19's move, and an event missing a declared field fails
    //                             both.
    //   WrongEventPayload      2  the same two scenarios, for the value where the other row is the
    //                             field: `payload: amount: input.amount` is declared on `settled`,
    //                             so both scenarios that run the branch hold the amount to what was
    //                             submitted. The row designates the transition scenario because the
    //                             outcome scenario already names `PartialEventPayload`, and one
    //                             scenario names one fault.
    //   NegativeProjectedTotal 2  the entity's own invariant reads the same words off the same
    //                             view — `Invoice` declares `total.amount >= 0` too, so the §20
    //                             entity check after `issue` fails beside the designated value
    //                             check. The value check at `InvoiceById.total` stays green, which
    //                             is the point of keying the family by position.
    //   WrongRefusalError      3  `issue` runs from `Draft` alone, so `IssueInvoice` answers its
    //                             `wrong_state:` branch in the other three declared states, and one
    //                             wrong error name is wrong in all three. Narrower is not available:
    //                             the injection sees a command and a result, not which invoice.
    let allowance: &[(Fault, usize)] = &[
        (Fault::WrongEvent, 24),
        (Fault::DropConsistencyToken, 9),
        (Fault::DropBinding, 4),
        (Fault::ExtraEvent, 4),
        (Fault::StaleReadYourWrites, 4),
        (Fault::WrongRefusalError, 3),
        (Fault::AllowIllegalTransition, 2),
        (Fault::IgnoreExternalOutcome, 2),
        (Fault::PartialEventPayload, 2),
        (Fault::WrongEventPayload, 2),
        (Fault::NegativeProjectedTotal, 2),
    ];

    for fault in Fault::ALL {
        let report = injected(*fault);
        let broken = not_passed(&report);
        let allowed = allowance
            .iter()
            .find(|(known, _)| known == fault)
            .map_or(1, |(_, allowed)| *allowed);
        // Printed so that `cargo test -p ess-conformance --test faults -- --nocapture
        // a_faults_blast_radius_is_accounted_for` reads as the matrix itself rather than as eleven
        // green ticks. §26 asks for a table; a table nobody can look at is a table on trust.
        println!(
            "{:<24} {:<14} {:<2} caught by {:<62} {} of {} still pass",
            fault.written(),
            fault.system().directory(),
            broken.len(),
            fault.caught().scenario().unwrap_or("— nothing"),
            report.scenarios.len() - broken.len(),
            report.scenarios.len(),
        );
        assert!(
            broken.len() <= allowed,
            "{fault:?} broke {} scenarios ({broken:?}) but is accounted for {allowed}; either a \
             scenario is over-reaching or the allowance needs updating with a reason",
            broken.len()
        );
    }
}

#[test]
fn the_widest_blast_radius_is_scenarios_that_could_not_be_arranged_rather_than_extra_verdicts() {
    // Why `WrongEvent`'s allowance is 24 and why that is a finding rather than a flaw, the way
    // `aep_conformance` records `DropAffected`'s 8. `billing.invoice.InvoiceCreated` is where a new
    // invoice's identity is published, and `CaptureInstance` reads it in seventeen further
    // scenarios; rename it and those scenarios cannot be set up at all.
    //
    // §28's distinction is what keeps that readable: they come back as `error` — nobody found out —
    // and not as `failed`, which would claim the implementation contradicted the specification
    // nineteen more times.
    let report = injected(Fault::WrongEvent);
    let broken = not_passed(&report);

    let failed = broken
        .iter()
        .filter(|(_, status)| *status == Status::Failed)
        .count();
    let errored = broken
        .iter()
        .filter(|(_, status)| *status == Status::Error)
        .count();
    assert_eq!(
        (failed, errored),
        (5, 19),
        "the split between a verdict about the implementation and a scenario nobody could arrange \
         is the whole reason §28 has four words rather than two: {broken:?}"
    );

    let arrangement = report
        .diagnostics()
        .find(|diagnostic| diagnostic.code == CheckCode::Instance)
        .expect("a scenario that could not be arranged says so")
        .to_string();
    assert!(
        arrangement.contains("carries the new identity"),
        "an `error` has to name what could not be established, or it reads as noise:\n{arrangement}"
    );
}

#[test]
fn dropping_one_binding_leaves_the_other_two_green() {
    // §26 in as many words — "unrelated core scenarios still pass" — and the claim
    // `examples/billing/` cannot make, because it declares one binding and dropping it fails every
    // binding scenario there is. The oracle fixture declares three, on three events, and its
    // `README.md` records that this is what the extra two are for.
    let report = injected(Fault::DropBinding);

    let mut green = 0;
    for result in &report.scenarios {
        let id = result.scenario.to_string();
        if !id.contains("/binding/") {
            continue;
        }
        if id.starts_with(Oracle::HANDOFF_ON_PLACED) {
            assert_eq!(
                result.status,
                Status::Failed,
                "every aspect of the binding that stopped running is unobservable: {id}"
            );
        } else {
            assert_eq!(
                result.status,
                Status::Passed,
                "a binding that still runs must still be provable: {id}"
            );
            green += 1;
        }
    }
    assert_eq!(
        green, 7,
        "the two surviving bindings oblige seven scenarios between them, and all seven are the \
         evidence that a binding failure is attributable rather than systemic"
    );
}

// ---- the rows nothing catches --------------------------------------------------------------------

#[test]
fn a_fault_nothing_catches_is_recorded_rather_than_quietly_dropped() {
    // The most valuable rows in the matrix, and the ones this slice went looking for. Each of these
    // is a wrong implementation that the generated suite passes in full — which is a statement about
    // what the model can express or what synthesis asks for, not about this file.
    //
    // Asserting that they *still* pass is deliberate. The day a later slice teaches synthesis to
    // compare an event's payload against the input that caused it, this test fails, and the row has
    // to be moved to `Caught::By` with the scenario that now catches it. A gap nobody re-checks is a
    // gap that gets forgotten.
    let mut closed: Vec<String> = Vec::new();

    for fault in Fault::ALL {
        let Caught::Nothing(why) = fault.caught() else {
            continue;
        };
        let report = injected(*fault);
        if report.status != ConformanceStatus::Passed {
            closed.push(format!(
                "{fault:?} ({}) is recorded as uncaught because {why}, but {:?} failed — the gap \
                 has been closed and the row belongs on the other side of the matrix",
                fault.describe(),
                not_passed(&report)
            ));
        }
    }

    assert!(closed.is_empty(), "{}", closed.join("\n  "));
}

#[test]
fn a_wrong_mapping_is_invisible_to_a_target_that_cannot_show_its_invocations() {
    // What the matrix does with a fault whose scenario degrades to `unsupported`, and the finding
    // behind the question. §16 refuses to require command tracing of every implementation, and
    // `handoff-on-placed/binding/mapping` is the *only* scenario that catches a wrong mapping — so
    // against a target that legitimately cannot answer it, the whole defect is invisible.
    //
    // Conformance still fails, which is §28 doing its job. What is lost is the diagnostic: the
    // report says "I cannot show you my invocations", never "you mailed the wrong address".
    let mis_mapped = Untraced(faulty::oracle(Fault::WrongMapping));
    let report = run(System::Oracle, &mis_mapped);

    assert_eq!(
        status_of(&report, "handoff-on-placed/binding/mapping"),
        Some(Status::Unsupported),
        "the scenario that catches this fault is the one §16 lets a target refuse"
    );
    assert_eq!(
        report.status,
        ConformanceStatus::Failed,
        "an unsupported required scenario still fails the run (§28)"
    );

    let broken = not_passed(&report);
    assert_eq!(
        broken.len(),
        3,
        "only the three mapping scenarios are unanswerable; the wrong address is not among the \
         findings at all: {broken:?}"
    );
    assert!(
        report
            .diagnostics()
            .all(|diagnostic| !diagnostic.to_string().contains("alternate_contact")),
        "nothing in the report names the value that was actually mapped, which is the cost of \
         having exactly one scenario able to see it"
    );
}

// ---- the matrix has to be repeatable, or it is not a matrix ---------------------------------------

#[test]
fn two_runs_against_one_faulty_target_produce_byte_identical_reports() {
    // §37, applied to the faults rather than to the reference: a faulty target that introduced a new
    // source of variation would make every row above flaky, and a flaky matrix is worth less than no
    // matrix. `StaleReadYourWrites` is the one that carries state of its own — the previous answer
    // to a view — so it is the one worth pinning.
    for fault in [
        Fault::StaleReadYourWrites,
        Fault::WrongEvent,
        Fault::DropBinding,
    ] {
        let first = injected(fault);
        let second = injected(fault);
        assert_eq!(
            first.to_canonical_json(),
            second.to_canonical_json(),
            "{fault:?} did not reproduce, so nothing it reports can be evidence"
        );
        assert_eq!(
            first.implementation,
            ImplementationIdentity::new(
                match fault.system() {
                    System::Billing => format!("billing-reference-{}", fault.written()),
                    System::Oracle => format!("oracle-reference-{}", fault.written()),
                },
                env!("CARGO_PKG_VERSION")
            ),
            "a report names the build that answered, and a deliberately wrong build says so (§30)"
        );
    }
}

#[test]
fn every_fault_that_could_be_a_boundary_perturbation_is_one() {
    // The judgement §25 leaves open, pinned rather than assumed: a wrapper that perturbs what goes
    // in and what comes out is in the same position as a real client, and an implementation-side
    // defect is not. Only the two the interface cannot express are allowed the second mechanism, and
    // `crates/ess-conformance/src/faulty.rs` argues why.
    let implementation: Vec<Fault> = Fault::ALL
        .iter()
        .copied()
        .filter(|fault| fault.injection() == Injection::Implementation)
        .collect();
    assert_eq!(
        implementation,
        vec![Fault::DropBinding, Fault::WrongMapping]
    );

    // And the argument itself is checkable: the events a dropped binding would have caused are
    // indistinguishable at the boundary from the ones the other two bindings cause, because an
    // observation carries no attribution to the binding behind it.
    let placed: BTreeMap<String, Status> = injected(Fault::DropBinding)
        .scenarios
        .iter()
        .map(|result| (result.scenario.to_string(), result.status))
        .collect();
    assert_eq!(
        placed.get("handoff-on-held/binding/flow"),
        Some(&Status::Passed),
        "`oracle.dispatch.HandedOff` is one event name for three bindings, so a wrapper filtering \
         it out of `observe_events` would have silenced this one too"
    );
}
