//! The closed loop, end to end: `examples/billing/` → a suite → a run against a known-good target.
//!
//! Design §24 and §49's step 4. Everything before this slice produced a *definition*; this is the
//! first evidence that the definition means anything, and the claim it makes is narrow and checkable:
//! **every one of the 27 scenarios the billing specification obliges passes against an
//! implementation written by hand from that specification.**
//!
//! What that does and does not prove is worth being exact about. A green run here shows the suite is
//! *satisfiable* — that it asks for nothing a correct implementation cannot answer, which is the
//! failure mode a suite full of over-strict assertions has. It does **not** show the suite *bites*;
//! that is the fault matrix (§25, §26), deliberately a separate slice so that the faulty
//! implementations cannot be quietly co-designed with the thing they are meant to falsify.
//!
//! The tests below therefore also hold the three properties a run has to have before its verdict is
//! worth anything: it is reproducible, an unsupported observation fails rather than passing quietly,
//! and a broken expectation produces a diagnostic that names the defect.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use ess_compiler::ir::EssIr;
use ess_compiler::resolve::compile;
use ess_compiler::source::SourceMap;
use ess_conformance::reference::Billing;
use ess_conformance::report::{CheckCode, ConformanceStatus, Status};
use ess_conformance::runner::{AdvancingClock, Ids, Runner, RunnerConfig};
use ess_conformance::scenario::{ConformanceSuite, ScenarioId, ScenarioStep, ScenarioValue};
use ess_conformance::synthesize::synthesize;
use ess_conformance::target::{
    ConformanceTarget, EventObservationRequest, ExternalOutcomeControl, ImplementationIdentity,
    ObservedEvent, RedeliveryRequest, ScenarioContext, SemanticCommandRequest,
    SemanticCommandResult, SemanticViewRequest, SemanticViewResult, TargetError,
};
use ess_domain::spec::{RawSpecFile, Specification};
use ess_domain::system::Source;

// ---- the specification under test ---------------------------------------------------------------

/// `examples/billing/`, compiled from the files it lives in rather than from a copy inlined here.
fn billing() -> EssIr {
    let base = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/billing")
        .canonicalize()
        .expect("the billing example exists");

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
        .unwrap_or_else(|errors| panic!("billing validates:\n{errors}"));
    compile(&specification, &sources)
        .unwrap_or_else(|diagnostics| panic!("billing resolves:\n{diagnostics}"))
}

/// The suite `examples/billing/` obliges.
fn suite() -> ConformanceSuite {
    synthesize(&billing()).suite
}

// ---- the claim ----------------------------------------------------------------------------------

#[test]
fn every_scenario_the_billing_specification_obliges_passes_against_the_reference_implementation() {
    let suite = suite();
    assert_eq!(
        suite.len(),
        27,
        "the fixture is the whole suite, not a subset of it; a run over fewer scenarios would \
         prove less than it looks like it proves"
    );

    let report = Runner::for_suite(&suite).run(&suite, &Billing::new());

    let failures: Vec<String> = report
        .failures()
        .map(|result| format!("{} — {}", result.scenario, result.status))
        .collect();
    assert!(
        failures.is_empty(),
        "the reference implementation is written from this specification, so a scenario it fails \
         is a defect in one of the two:\n{}\n\nfirst diagnostic:\n{}",
        failures.join("\n"),
        report
            .diagnostics()
            .next()
            .map_or_else(|| "none".to_owned(), ToString::to_string)
    );
    assert_eq!(report.scenarios.len(), 27);
    assert_eq!(report.status, ConformanceStatus::Passed);
    assert!(report.is_conformant());
    assert_eq!(
        report.implementation,
        ImplementationIdentity::new("billing-reference", env!("CARGO_PKG_VERSION"))
    );
    assert_eq!(
        report.suite.spec_digest, suite.provenance.spec_digest,
        "a verdict names the specification it is about, or it attests nothing"
    );
}

#[test]
fn every_scenario_checked_something_and_no_family_of_them_was_silently_empty() {
    // A run whose scenarios assert nothing passes just as green as one that asserts everything, and
    // §36's whole argument is that the silent omission is the failure a passing run cannot show.
    let suite = suite();
    let report = Runner::for_suite(&suite).run(&suite, &Billing::new());

    for result in &report.scenarios {
        assert!(
            !result.checks.is_empty(),
            "{} checked nothing, so its `passed` means nothing",
            result.scenario
        );
    }
    let checked: std::collections::BTreeSet<CheckCode> = report
        .scenarios
        .iter()
        .flat_map(|result| result.checks.iter().map(|check| check.code))
        .collect();
    for expected in [
        CheckCode::Outcome,
        CheckCode::Error,
        CheckCode::Event,
        CheckCode::NoEvent,
        CheckCode::EventualEvent,
        CheckCode::View,
        CheckCode::EventualView,
        CheckCode::Invocation,
    ] {
        assert!(
            checked.contains(&expected),
            "no scenario in the billing suite exercised {expected}"
        );
    }
    assert!(
        !checked.contains(&CheckCode::Suite) && !checked.contains(&CheckCode::Target),
        "a run against a correct target records no suite defect and no adapter failure: {checked:?}"
    );
}

#[test]
fn two_runs_of_one_suite_against_one_target_produce_byte_identical_reports() {
    // §37's execution-determinism claim, and the reason the runner is constructed with its clock and
    // its id source rather than reaching for one: evidence from a run nobody can repeat is a claim,
    // not evidence. Two *separate* runners and two *separate* targets, because a single one that
    // agreed with itself would only be showing that nothing was reset.
    let suite = suite();

    let first = Runner::for_suite(&suite).run(&suite, &Billing::new());
    let second = Runner::for_suite(&suite).run(&suite, &Billing::new());

    assert_eq!(
        first.to_canonical_json(),
        second.to_canonical_json(),
        "a status, a correlation id, a timestamp or a duration differed between two runs of one \
         suite; §37 makes that a defect rather than noise"
    );
    assert_eq!(first.started_at, second.started_at);
    assert_eq!(first.completed_at, second.completed_at);
}

// ---- what a failure says ------------------------------------------------------------------------

#[test]
fn a_scenario_whose_input_no_longer_reaches_its_branch_fails_with_a_diagnostic_naming_the_defect() {
    // Verifying the guard by breaking it, the way this repository asks. The mutation is on the
    // *suite* side rather than in the implementation — the faulty implementations are the next
    // slice, kept separate so they cannot be co-designed with the thing they falsify — and it is the
    // smallest one that matters: `CreateInvoice/outcome/rejected` executes an amount that satisfies
    // `accepted`'s guard while still requiring `rejected`.
    //
    // Three checks must fail, not one. A suite that reported only the outcome would let an
    // implementation that refuses correctly but still publishes `InvoiceCreated` look half-right.
    let mut suite = suite();
    let id = ScenarioId::parse("billing.invoice.CreateInvoice/outcome/rejected").expect("an id");
    let scenario = suite
        .scenarios
        .get_mut(&id)
        .expect("the rejection scenario is in the suite");
    for step in &mut scenario.steps {
        if let ScenarioStep::ExecuteCommand { input, .. } = step {
            input.insert("amount".to_owned(), ScenarioValue::literal(money(1.0)));
        }
    }

    let report = Runner::for_suite(&suite).run(&suite, &Billing::new());

    let result = report
        .scenarios
        .iter()
        .find(|result| result.scenario == id)
        .expect("the mutated scenario ran");
    assert_eq!(
        result.status,
        Status::Failed,
        "the implementation contradicted what the suite required, which is `failed` and not `error`"
    );
    assert_eq!(report.status, ConformanceStatus::Failed);
    assert_eq!(
        result
            .checks
            .iter()
            .filter(|check| check.status == Status::Failed)
            .count(),
        3,
        "the outcome, the declared error and the negative event assertion each fail on their own"
    );

    let diagnostic = result
        .diagnostics()
        .find(|diagnostic| diagnostic.code == CheckCode::Outcome)
        .expect("the outcome check produced a diagnostic")
        .to_string();
    for required in [
        "ESS-CF-OUTCOME",
        "billing.invoice.CreateInvoice/outcome/rejected",
        "a command takes the declared branch its guards select",
        "outcome billing.invoice.CreateInvoice/rejected",
        r#"billing.invoice.CreateInvoice(amount = {"amount":1.0"#,
        "expected:\n  outcome = rejected",
        "observed:\n  outcome = accepted",
    ] {
        assert!(
            diagnostic.contains(required),
            "§29 requires a failure to say what rule was checked, which element declared it, what \
             was executed, what was expected and what was observed; {required:?} is missing \
             from:\n{diagnostic}"
        );
    }

    let negative = result
        .diagnostics()
        .find(|diagnostic| diagnostic.code == CheckCode::NoEvent)
        .expect("the negative assertion produced a diagnostic")
        .to_string();
    assert!(
        negative.contains("billing.invoice.InvoiceCreated published"),
        "a negative assertion says which event turned up: {negative}"
    );
}

#[test]
fn a_target_that_cannot_expose_an_observation_fails_the_run_rather_than_skipping_it() {
    // §28, and the one place a suite could quietly hold fewer checks than the specification demands.
    // §16 refuses to require command tracing of every implementation, so a target may legitimately
    // be unable to answer `binding/mapping` — and the run must still not pass.
    let suite = suite();
    let report = Runner::for_suite(&suite).run(&suite, &Untraced(Billing::new()));

    let mapping = ScenarioId::parse("notify-on-invoice-created/binding/mapping").expect("an id");
    let result = report
        .scenarios
        .iter()
        .find(|result| result.scenario == mapping)
        .expect("the mapping scenario ran");

    assert_eq!(result.status, Status::Unsupported);
    assert_eq!(
        report.status,
        ConformanceStatus::Failed,
        "an unsupported required scenario makes conformance fail; a skip that looked like a pass \
         is exactly what §28 forbids"
    );
    assert_eq!(
        report
            .scenarios
            .iter()
            .filter(|result| result.status == Status::Passed)
            .count(),
        26,
        "everything the target *can* answer still answered; only the one observation is missing"
    );
    let diagnostic = result
        .diagnostics()
        .next()
        .expect("an unsupported check says what could not be exposed")
        .to_string();
    assert!(
        diagnostic.contains("does not expose the commands its bindings invoke"),
        "the report says what the target could not do, in the target's own words: {diagnostic}"
    );
}

// ---- waiting -------------------------------------------------------------------------------------

#[test]
fn an_eventual_assertion_asks_again_within_a_deadline_and_never_sleeps() {
    // §40. The lag is far beyond the budget, so the consequence never becomes observable — and the
    // run has to *end*, with a failure that says how many times it asked. A runner that slept would
    // take the wall-clock budget to reach this line; this one takes none, because the deadline is
    // measured in the runner's own clock.
    let suite = suite();
    let flow = ScenarioId::parse("notify-on-invoice-created/binding/flow").expect("an id");
    let mut narrowed = ConformanceSuite::new(suite.provenance.clone());
    narrowed
        .insert(
            flow.clone(),
            suite.scenario(&flow).expect("the flow scenario").clone(),
        )
        .expect("the first insertion");

    let runner = Runner::new(
        RunnerConfig::new(1_000),
        AdvancingClock::new(0, 100),
        Ids::for_suite(&narrowed),
    );
    let report = runner.run(&narrowed, &Billing::with_lag(1_000));

    let result = &report.scenarios[0];
    assert_eq!(result.status, Status::Failed);
    let diagnostic = result
        .diagnostics()
        .next()
        .expect("a bounded assertion that gave up says so")
        .to_string();
    assert!(
        diagnostic.contains("ESS-CF-EVENTUAL-EVENT"),
        "the code names the rule: {diagnostic}"
    );
    assert!(
        diagnostic.contains("not observed after 10 observations"),
        "a budget of 1000 over a clock stepping by 100 is ten asks, and the count is part of the \
         repair feedback: {diagnostic}"
    );

    // The same specification, the same suite, a projection that keeps up: it passes. Which is what
    // makes the failure above a statement about the target rather than about the assertion.
    let quick = Runner::new(
        RunnerConfig::new(1_000),
        AdvancingClock::new(0, 100),
        Ids::for_suite(&narrowed),
    );
    assert_eq!(
        quick.run(&narrowed, &Billing::new()).status,
        ConformanceStatus::Passed
    );
}

#[test]
fn an_eventual_view_is_read_again_and_a_read_your_writes_view_is_not() {
    // The distinction §14 requires the runner to preserve, observed from the outside. The reference
    // publishes to `billing.invoice.InvoiceById` only after further reads, so the assertion holds
    // only because the runner asked again — and the fixture proves that rather than asserting it:
    // the same scenario against the same target, with a budget of one ask, fails on the eventual
    // view and passes on the read-your-writes one beside it.
    let suite = suite();
    let created =
        ScenarioId::parse("billing.invoice.CreateInvoice/outcome/accepted").expect("an id");
    let mut narrowed = ConformanceSuite::new(suite.provenance.clone());
    narrowed
        .insert(
            created.clone(),
            suite.scenario(&created).expect("the scenario").clone(),
        )
        .expect("the first insertion");

    let one_ask = Runner::new(
        RunnerConfig::new(100),
        AdvancingClock::new(0, 100),
        Ids::for_suite(&narrowed),
    )
    .run(&narrowed, &Billing::new());
    let starved = &one_ask.scenarios[0];
    assert_eq!(
        starved.status,
        Status::Failed,
        "with one ask the projection has not caught up, so the eventual assertion must fail rather \
         than pass by luck"
    );
    assert_eq!(
        starved
            .checks
            .iter()
            .filter(|check| check.status == Status::Failed)
            .map(|check| check.code)
            .collect::<Vec<_>>(),
        vec![CheckCode::EventualView],
        "only the eventual view is starved; the read-your-writes view beside it never waits"
    );

    let patient = Runner::for_suite(&narrowed).run(&narrowed, &Billing::new());
    let result = &patient.scenarios[0];
    assert_eq!(result.status, Status::Passed);
    let codes: Vec<CheckCode> = result.checks.iter().map(|check| check.code).collect();
    assert!(
        codes.contains(&CheckCode::EventualView) && codes.contains(&CheckCode::View),
        "the scenario asserts one view of each consistency, or it is not exercising the \
         distinction: {codes:?}"
    );
}

// ---- fixtures -----------------------------------------------------------------------------------

/// A `billing.invoice.Money`.
fn money(amount: f64) -> aep_domain::node::Node {
    let mut fields = BTreeMap::new();
    fields.insert(
        "amount".to_owned(),
        aep_domain::node::Node::Number(
            aep_domain::facts::Number::new(amount).expect("a finite amount"),
        ),
    );
    fields.insert(
        "currency".to_owned(),
        aep_domain::node::Node::Text("amount.currency".to_owned()),
    );
    aep_domain::node::Node::Map(fields)
}

/// The reference implementation, minus the one observation §16 refuses to require of anybody.
///
/// Not a faulty implementation: it answers every semantic question correctly and cannot answer one
/// question about its own internals, which is exactly the target §16 has in mind when it says
/// command tracing "should not become a requirement for every implementation".
struct Untraced(Billing);

impl ConformanceTarget for Untraced {
    fn identity(&self) -> Result<ImplementationIdentity, TargetError> {
        self.0.identity()
    }

    fn begin_scenario(&self, scenario: &ScenarioContext) -> Result<(), TargetError> {
        self.0.begin_scenario(scenario)
    }

    fn execute_command(
        &self,
        request: SemanticCommandRequest,
    ) -> Result<SemanticCommandResult, TargetError> {
        self.0.execute_command(request)
    }

    fn query_view(&self, request: SemanticViewRequest) -> Result<SemanticViewResult, TargetError> {
        self.0.query_view(request)
    }

    fn observe_events(
        &self,
        request: EventObservationRequest,
    ) -> Result<Vec<ObservedEvent>, TargetError> {
        self.0.observe_events(request)
    }

    fn configure_external_outcome(
        &self,
        request: ExternalOutcomeControl,
    ) -> Result<(), TargetError> {
        self.0.configure_external_outcome(request)
    }

    fn redeliver_event(&self, request: RedeliveryRequest) -> Result<(), TargetError> {
        self.0.redeliver_event(request)
    }

    fn end_scenario(&self, scenario: &ScenarioContext) -> Result<(), TargetError> {
        self.0.end_scenario(scenario)
    }
}
