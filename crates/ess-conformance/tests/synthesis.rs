//! What the two examples synthesise to, and what they refuse.
//!
//! `examples/billing/` is the normative example and every claim about the shape of a suite is made
//! against it. `examples/oracle-fixture/` is where the claims billing cannot reach are made — a
//! `read_your_writes` view that holds a row after one command, an `updates:` outcome, and an
//! externally decided outcome in a second system — and its `README.md` records why each construct
//! is there.
//!
//! Nothing here executes a scenario. The assertions are about what the suite *says*, and where a
//! value is claimed to satisfy a guard the test re-decides it through
//! [`InputFacts::decide`](ess_conformance::InputFacts::decide) rather than trusting the synthesizer
//! that produced it.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use aep_domain::node::Node;
use ess_compiler::diagnostic::Code;
use ess_compiler::ir::EssIr;
use ess_compiler::resolve::compile;
use ess_compiler::source::SourceMap;
use ess_conformance::scenario::{ScenarioStep, ViewExpectation};
use ess_conformance::synthesize::{synthesize, RefusalCause, Synthesis};
use ess_conformance::{flatten, when, Decision, ScenarioId};
use ess_domain::name::QualifiedName;
use ess_domain::spec::{RawSpecFile, Specification};
use ess_domain::system::Source;

// ---- the specifications under test -------------------------------------------------------------

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
    assert!(!found.is_empty(), "`{name}` holds no specification files");

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

/// A specification written inline, for a corner neither example carries.
fn fixture(text: &str) -> EssIr {
    let raw = RawSpecFile::parse(text).expect("the fixture is well formed");
    let specification = Specification::assemble([(Source::new("fixture.yaml"), raw)])
        .unwrap_or_else(|errors| panic!("the fixture validates:\n{errors}"));
    compile(&specification, &SourceMap::new())
        .unwrap_or_else(|diagnostics| panic!("the fixture resolves:\n{diagnostics}"))
}

// ---- reading a synthesis -----------------------------------------------------------------------

/// Every scenario id the suite holds, rendered, in file order.
fn ids(synthesis: &Synthesis) -> Vec<String> {
    synthesis
        .suite
        .scenarios
        .keys()
        .map(ToString::to_string)
        .collect()
}

/// Every scenario a refusal names, rendered, in the order they were refused.
fn refused(synthesis: &Synthesis) -> Vec<String> {
    synthesis
        .refusals
        .iter()
        .filter_map(|refusal| refusal.scenario.as_ref())
        .map(ToString::to_string)
        .collect()
}

/// The kind of each step, in order — the shape design §10 writes a scenario as.
fn shape(synthesis: &Synthesis, id: &str) -> Vec<&'static str> {
    steps(synthesis, id)
        .iter()
        .map(|step| match step {
            ScenarioStep::ConfigureExternalOutcome { .. } => "inject",
            ScenarioStep::ExecuteCommand { .. } => "execute",
            ScenarioStep::ExpectOutcome { .. } => "outcome",
            ScenarioStep::ExpectError { .. } => "error",
            ScenarioStep::ExpectEvent { .. } => "event",
            ScenarioStep::ExpectNoEvent { .. } => "no-event",
            ScenarioStep::QueryView { .. } => "query",
            ScenarioStep::ExpectView { .. } => "view",
            ScenarioStep::EventuallyEvent { .. } => "eventually-event",
            ScenarioStep::EventuallyView { .. } => "eventually-view",
        })
        .collect()
}

/// The steps of one scenario.
fn steps<'a>(synthesis: &'a Synthesis, id: &str) -> &'a [ScenarioStep] {
    let id = ScenarioId::parse(id).expect("a scenario id");
    &synthesis
        .suite
        .scenario(&id)
        .unwrap_or_else(|| panic!("`{id}` is in the suite; it holds {:?}", ids(synthesis)))
        .steps
}

/// The input one scenario sends.
fn sent(synthesis: &Synthesis, id: &str) -> BTreeMap<String, Node> {
    steps(synthesis, id)
        .iter()
        .find_map(|step| match step {
            ScenarioStep::ExecuteCommand { input, .. } => Some(input.clone()),
            _ => None,
        })
        .unwrap_or_else(|| panic!("`{id}` invokes a command"))
}

/// The view expectation one scenario makes of `view`, and the step it makes it in.
fn expectation<'a>(
    synthesis: &'a Synthesis,
    id: &str,
    view: &str,
) -> (&'static str, &'a ViewExpectation) {
    steps(synthesis, id)
        .iter()
        .find_map(|step| match step {
            ScenarioStep::ExpectView {
                view: named,
                expectation,
            } if named.to_string() == view => Some(("expect", expectation)),
            ScenarioStep::EventuallyView {
                view: named,
                expectation,
            } if named.to_string() == view => Some(("eventually", expectation)),
            _ => None,
        })
        .unwrap_or_else(|| panic!("`{id}` asserts `{view}`"))
}

/// A code in this synthesizer's family.
fn code(number: u16) -> Code {
    Code::new("SYNTH", number)
}

// ---- §10: one scenario per reachable declared outcome -------------------------------------------

#[test]
fn every_declared_outcome_is_either_a_scenario_or_a_named_refusal() {
    // §36's rule, made checkable: silently omitting a scenario is the one unacceptable option, so
    // the two lists together must cover every outcome the specification declares. The fixture
    // reaches the state where that rule bites — billing declares eight outcomes and only five are
    // reachable — because a specification whose outcomes are all reachable could not tell a suite
    // that refuses well from one that drops what it cannot do.
    let ir = example("billing");
    let synthesis = synthesize(&ir);

    let declared: BTreeSet<String> = ir
        .commands
        .values()
        .flat_map(|command| {
            command
                .outcomes
                .iter()
                .map(move |outcome| format!("{}/outcome/{}", command.name, outcome.name))
        })
        .collect();
    assert_eq!(declared.len(), 8, "the fixture declares eight outcomes");

    let covered: BTreeSet<String> = ids(&synthesis)
        .into_iter()
        .chain(refused(&synthesis))
        .filter(|id| id.contains("/outcome/"))
        .collect();

    assert_eq!(
        covered, declared,
        "an outcome that is neither in the suite nor in a refusal has disappeared"
    );
    assert_eq!(
        ids(&synthesis),
        vec![
            "billing.email.SendEmail/outcome/failed",
            "billing.email.SendEmail/outcome/sent",
            "billing.invoice.CreateInvoice/outcome/accepted",
            "billing.invoice.CreateInvoice/outcome/rejected",
            "billing.invoice.PayInvoice/outcome/rejected",
        ],
        "the suite billing produces today"
    );
}

#[test]
fn the_refusal_branch_asserts_that_the_success_event_did_not_occur() {
    // Design §10's worked example, both halves. The negative assertion is the half that is easy to
    // leave out and impossible to notice missing: without it the scenario passes against an
    // implementation that refuses the command and emits `InvoiceCreated` anyway.
    let synthesis = synthesize(&example("billing"));

    assert_eq!(
        shape(&synthesis, "billing.invoice.CreateInvoice/outcome/rejected"),
        vec!["execute", "outcome", "error", "no-event"],
        "`→ rejected`, `→ InvalidAmount`, `→ InvoiceCreated must not occur`"
    );
    assert_eq!(
        shape(&synthesis, "billing.invoice.CreateInvoice/outcome/accepted")
            .into_iter()
            .take(3)
            .collect::<Vec<_>>(),
        vec!["execute", "outcome", "event"],
        "and the happy path asserts the event it declares"
    );

    let absent: Vec<String> = steps(&synthesis, "billing.invoice.CreateInvoice/outcome/rejected")
        .iter()
        .filter_map(|step| match step {
            ScenarioStep::ExpectNoEvent { event } => Some(event.to_string()),
            _ => None,
        })
        .collect();
    assert_eq!(
        absent,
        vec!["billing.invoice.InvoiceCreated"],
        "the event the *other* branch emits is the one that must not occur"
    );
}

#[test]
fn a_declared_error_is_asserted_by_name_and_never_by_an_invented_payload() {
    // §11, at the assertion end. Nothing in the model relates `InvalidAmount.submitted` to the
    // command's `amount`, so a payload comparison here would be an inference wearing the clothes of
    // a requirement. §10's own generated suite writes `→ InvalidAmount` and no field.
    let synthesis = synthesize(&example("billing"));

    let errors: Vec<(String, usize)> = synthesis
        .suite
        .scenarios
        .values()
        .flat_map(|scenario| scenario.steps.iter())
        .filter_map(|step| match step {
            ScenarioStep::ExpectError { error, fields } => Some((error.to_string(), fields.len())),
            _ => None,
        })
        .collect();

    assert_eq!(
        errors,
        vec![
            ("billing.email.Undeliverable".to_owned(), 0),
            ("billing.invoice.InvalidAmount".to_owned(), 0),
            ("billing.invoice.InvalidAmount".to_owned(), 0),
        ],
        "every refusal branch names its declared error, and none claims a field value"
    );
}

// ---- §11: a witness is kept because it was decided, not because it looked right -----------------

#[test]
fn the_input_a_scenario_sends_is_re_decided_against_the_guard_it_claims_to_reach() {
    // The claim §11 makes load-bearing — *never generate a value and claim it satisfies a predicate
    // unless the generator can prove or evaluate that it does* — checked by evaluating it a second
    // time, through the same bridge, from outside the synthesizer.
    let ir = example("billing");
    let synthesis = synthesize(&ir);
    let command = ir
        .commands
        .get(&QualifiedName::new("billing.invoice.CreateInvoice").expect("valid"))
        .expect("the example declares it");
    let guard = when(
        command
            .outcomes
            .iter()
            .find(|outcome| outcome.name.as_str() == "accepted")
            .expect("`accepted` is declared"),
    )
    .expect("`accepted` is taken by a `when`");

    for (id, expected) in [
        (
            "billing.invoice.CreateInvoice/outcome/accepted",
            Decision::Satisfied,
        ),
        (
            "billing.invoice.CreateInvoice/outcome/rejected",
            Decision::Refuted(
                guard.outcome(
                    &flatten(
                        &ir,
                        command,
                        &sent(&synthesis, "billing.invoice.CreateInvoice/outcome/rejected"),
                    )
                    .expect("the input fits"),
                ),
            ),
        ),
    ] {
        let input = sent(&synthesis, id);
        let facts = flatten(&ir, command, &input).expect("a synthesised input fits its own type");
        assert_eq!(
            facts.decide(guard),
            expected,
            "`{id}` sends {input:?}, which does not do what the branch it asserts requires"
        );
    }
}

#[test]
fn an_undecidable_guard_refuses_and_does_not_spend_the_candidate_budget() {
    // Invariant 5 from the generator's side. `amount.vat` parses, validates and compiles — the
    // domain checks a `when` path's *first* segment only — so this is a specification defect that
    // reaches synthesis, and treating its `Unknown` as "try another candidate" would report it as a
    // flaky test after burning every candidate.
    let synthesis = synthesize(&fixture(UNDECIDED));

    assert!(
        synthesis.suite.is_empty(),
        "neither branch is reachable: {:?}",
        ids(&synthesis)
    );
    let refusals: Vec<String> = synthesis
        .refusals
        .iter()
        .map(|refusal| format!("{}: {refusal}", refusal.code()))
        .collect();
    assert_eq!(
        synthesis.refused(code(2)).count(),
        2,
        "both the guarded branch and the default branch refuse, because deciding the default \
         branch means deciding every other guard: {refusals:?}"
    );
    assert_eq!(
        synthesis.refused(code(3)).count(),
        0,
        "and neither is reported as an exhausted search, which is what a retry would have \
         produced: {refusals:?}"
    );

    let refusal = synthesis
        .refused(code(2))
        .next()
        .expect("the guarded branch refuses");
    let rendered = refusal.to_string();
    assert!(
        rendered.contains("amount.vat > 0") && rendered.contains("`vat`"),
        "a refusal names the predicate and the segment that could not be resolved: {rendered}"
    );
    assert!(
        matches!(refusal.cause, RefusalCause::GuardUnevaluable(_)),
        "and it is the cause an author repairs, not a search that gave up"
    );
}

#[test]
fn a_guard_no_candidate_can_satisfy_is_refused_with_the_number_tried() {
    // The other half of §11: a predicate that is valid and not constructively satisfiable by
    // generate-and-check. The refusal says how many candidates were decided, so a reader can tell
    // it from a guard nothing could decide at all.
    let synthesis = synthesize(&fixture(UNSATISFIABLE));

    let refusal = synthesis
        .refused(code(3))
        .next()
        .unwrap_or_else(|| panic!("the branch refuses: {:?}", refused(&synthesis)));
    let RefusalCause::GuardUnsatisfiable { predicate, tried } = &refusal.cause else {
        panic!("{refusal}")
    };
    assert!(
        predicate.contains("quantity"),
        "the refusal quotes the guard: {predicate}"
    );
    assert!(*tried > 1, "more than the base witness was decided");
}

// ---- §12: an external outcome is injected, never constructed -------------------------------------

#[test]
fn an_outcome_no_input_decides_is_reached_by_injection_and_by_nothing_else() {
    // §12, and the reason `billing.email.SendEmail` declares `external:` rather than `when: false`.
    // The step is a test-adapter control, so the system stays as non-deterministic as it really is.
    for (system, scenario, other) in [
        (
            "billing",
            "billing.email.SendEmail/outcome/failed",
            "billing.email.SendEmail/outcome/sent",
        ),
        (
            "oracle-fixture",
            "oracle.dispatch.Handoff/outcome/refused",
            "oracle.dispatch.Handoff/outcome/accepted",
        ),
    ] {
        let synthesis = synthesize(&example(system));

        assert_eq!(
            shape(&synthesis, scenario),
            vec!["inject", "execute", "outcome", "error", "no-event"],
            "the injection comes first, and the branch still asserts its error and the event that \
             must not have happened"
        );
        assert!(
            !shape(&synthesis, other).contains(&"inject"),
            "the sibling branch is reached by an input, so injecting anything there would be a \
             control the specification does not claim"
        );
    }
}

// ---- §14 and §20: views, in the block their consistency decides ----------------------------------

#[test]
fn a_view_is_asserted_in_the_block_its_own_consistency_decides() {
    // `ResolvedView::assertion_style` holds this decision; re-deriving it from the consistency word
    // is the regression §14 names. The fixture holds one view of each kind over one entity, so a
    // synthesizer that picked one block for both would fail here rather than pass by luck.
    let synthesis = synthesize(&example("billing"));
    let created = "billing.invoice.CreateInvoice/outcome/accepted";

    let (block, _) = expectation(&synthesis, created, "billing.invoice.InvoiceById");
    assert_eq!(
        block, "eventually",
        "`InvoiceById` is `eventual`: asserting it immediately races the projection, and the \
         repair everyone reaches for is a sleep"
    );
    let (block, _) = expectation(&synthesis, created, "billing.invoice.OutstandingInvoices");
    assert_eq!(
        block, "expect",
        "`OutstandingInvoices` is `read_your_writes`, so the assertion is immediate"
    );
    assert_eq!(
        shape(&synthesis, created),
        vec![
            "execute",
            "outcome",
            "event",
            "eventually-view",
            "query",
            "view"
        ],
        "an immediate assertion is a read and a check; a bounded one is a single step, because \
         retrying means re-running the query"
    );
}

#[test]
fn a_view_the_entity_has_not_reached_yet_is_asserted_to_exclude_it() {
    // The negative view assertion, decided rather than assumed: `OutstandingInvoices` filters on
    // `state == Issued` and `CreateInvoice` leaves the invoice in `Draft`, so the filter evaluates
    // to `False` against the one fact the scenario knows.
    let synthesis = synthesize(&example("billing"));
    let (_, expectation) = expectation(
        &synthesis,
        "billing.invoice.CreateInvoice/outcome/accepted",
        "billing.invoice.OutstandingInvoices",
    );

    assert!(
        matches!(expectation, ViewExpectation::Excludes { fields } if fields.is_empty()),
        "an invoice in `Draft` must not be outstanding: {expectation:?}"
    );
}

#[test]
fn a_read_your_writes_view_filled_by_the_command_that_ran_is_asserted_to_hold_a_row() {
    // What `examples/oracle-fixture/` exists for. Billing's read-your-writes view filters on a
    // state two commands away, so nothing there can make the positive assertion; `OpenOrders`
    // filters on the *initial* state, which one command reaches.
    let synthesis = synthesize(&example("oracle-fixture"));
    let placed = "oracle.order.PlaceOrder/outcome/accepted";

    let (block, open) = expectation(&synthesis, placed, "oracle.order.OpenOrders");
    assert_eq!(block, "expect", "`OpenOrders` is `read_your_writes`");
    assert!(
        matches!(open, ViewExpectation::Contains { .. }),
        "an order in `Placed` is open: {open:?}"
    );

    let (block, held) = expectation(&synthesis, placed, "oracle.order.HeldOrders");
    assert_eq!(block, "eventually", "`HeldOrders` is `eventual`");
    assert!(
        matches!(held, ViewExpectation::Excludes { .. }),
        "and it is not held, which the positive assertion cannot see: {held:?}"
    );
}

#[test]
fn a_filter_reading_something_no_scenario_knows_refuses_rather_than_guessing() {
    // A synthesised scenario knows one fact about the entity it created: where its lifecycle
    // starts. A filter over a field is undecidable against that, and asserting the view either way
    // would be the invention §11 refuses.
    let synthesis = synthesize(&fixture(UNKNOWABLE_VIEW));

    let refusal = synthesis
        .refused(code(5))
        .next()
        .unwrap_or_else(|| panic!("the view refuses: {:?}", refused(&synthesis)));
    let rendered = refusal.to_string();
    assert!(
        rendered.contains("hidden.orders.HeavyOrders") && rendered.contains("weight_grams"),
        "the refusal names the view and the path nothing binds: {rendered}"
    );
    assert!(
        !steps(&synthesis, "hidden.orders.PlaceOrder/outcome/accepted")
            .iter()
            .any(|step| matches!(step, ScenarioStep::ExpectView { .. })),
        "and the scenario is still produced, without the assertion it could not make"
    );
}

// ---- §19: lifecycle, and the link that is still missing -----------------------------------------

#[test]
fn an_outcome_that_moves_an_existing_instance_refuses_rather_than_inventing_an_identity() {
    // G14 closed *which command drives a transition*. What is still unanswered is which input names
    // the instance: nothing relates a command's input to the subject's identity, so a scenario
    // cannot say "the invoice the previous step created" and a fabricated id would be a test that
    // fails against a correct implementation.
    let synthesis = synthesize(&example("billing"));

    let refused_here: Vec<String> = synthesis
        .refused(code(4))
        .filter_map(|refusal| refusal.scenario.as_ref())
        .map(ToString::to_string)
        .collect();

    for expected in [
        "billing.invoice.IssueInvoice/outcome/issued",
        "billing.invoice.PayInvoice/outcome/settled",
        "billing.invoice.CancelInvoice/outcome/cancelled",
        "billing.invoice.Invoice/transition/settle/by/billing.invoice.PayInvoice/settled",
        "billing.invoice.Invoice/state/Paid/refuses/billing.invoice.CancelInvoice",
    ] {
        assert!(
            refused_here.iter().any(|id| id == expected),
            "`{expected}` must appear as a refusal rather than as an absence: {refused_here:?}"
        );
        assert!(
            synthesis
                .suite
                .scenario(&ScenarioId::parse(expected).expect("an id"))
                .is_none(),
            "`{expected}` must not be in the suite"
        );
    }
    assert_eq!(
        synthesis.refused(code(4)).count(),
        14,
        "three outcomes, three transitions and eight state/command pairs the lifecycle forbids"
    );
}

#[test]
fn every_declared_transition_is_named_by_a_scenario_id_that_does_not_exist_yet() {
    // §19's first class — *for every declared transition, generate a scenario that proves it can
    // occur* — enumerated rather than summarised, so the id a fault matrix will refer to is already
    // the id the refusal carries.
    let ir = example("billing");
    let synthesis = synthesize(&ir);

    let transitions: BTreeSet<String> = refused(&synthesis)
        .into_iter()
        .filter(|id| id.contains("/transition/"))
        .collect();
    assert_eq!(
        transitions,
        BTreeSet::from([
            "billing.invoice.Invoice/transition/cancel/by/billing.invoice.CancelInvoice/cancelled"
                .to_owned(),
            "billing.invoice.Invoice/transition/issue/by/billing.invoice.IssueInvoice/issued"
                .to_owned(),
            "billing.invoice.Invoice/transition/settle/by/billing.invoice.PayInvoice/settled"
                .to_owned(),
        ]),
        "one per declared transition, naming the outcome that drives it"
    );
}

#[test]
fn an_outcome_that_updates_an_instance_refuses_for_the_same_reason_one_that_moves_it_does() {
    // `updates:` is why `examples/oracle-fixture/` declares `AmendOrder`: billing has no outcome
    // that changes an entity without moving it, so §20's case has no instance there.
    let synthesis = synthesize(&example("oracle-fixture"));

    let refusal = synthesis
        .refusals
        .iter()
        .find(|refusal| {
            refusal
                .scenario
                .as_ref()
                .is_some_and(|id| id.to_string() == "oracle.order.AmendOrder/outcome/amended")
        })
        .unwrap_or_else(|| panic!("`amended` refuses: {:?}", refused(&synthesis)));

    assert_eq!(refusal.code(), code(4));
    assert!(
        refusal.to_string().contains("to change without moving"),
        "the refusal says which of the three verbs it could not reach: {refusal}"
    );
    assert!(
        synthesis
            .suite
            .scenario(&ScenarioId::parse("oracle.order.AmendOrder/outcome/rejected").expect("id"))
            .is_some(),
        "while the branch that changes nothing is still synthesised: a refused command has no \
         subject, so it needs no instance"
    );
}

// ---- §16–§18: what this build does not do yet ----------------------------------------------------

#[test]
fn a_binding_this_build_does_not_synthesise_is_visible_rather_than_missing() {
    // A reader of a suite cannot tell an unimplemented slice from a specification with nothing to
    // check. §36 rules that ambiguity out for refusals, and the same reasoning applies to a gap
    // this crate has not closed yet.
    for (system, bindings) in [("billing", 1), ("oracle-fixture", 3)] {
        let synthesis = synthesize(&example(system));
        let named: Vec<String> = synthesis
            .refused(code(6))
            .map(|refusal| refusal.subject.to_string())
            .collect();

        assert_eq!(named.len(), bindings, "one refusal per binding: {named:?}");
        assert!(
            named.iter().all(|subject| subject.starts_with("binding ")),
            "each names the binding it is about: {named:?}"
        );
    }
}

// ---- §23 and §37: provenance and determinism -----------------------------------------------------

#[test]
fn synthesising_the_same_specification_twice_produces_byte_identical_output() {
    // Two independent compilations and two independent syntheses. Nothing is shared between them,
    // so an unordered map, a clock or an address-dependent iteration order anywhere in the path
    // shows up here as a diff rather than as a rumour — including in the refusals, which a report
    // prints and a reviewer diffs.
    let first = synthesize(&example("billing"));
    let second = synthesize(&example("billing"));

    assert_eq!(
        first.suite.to_canonical_json().as_bytes(),
        second.suite.to_canonical_json().as_bytes(),
        "the same model must produce the same suite, byte for byte"
    );
    let rendered = |synthesis: &Synthesis| {
        synthesis
            .refusals
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
    };
    assert_eq!(
        rendered(&first),
        rendered(&second),
        "and the same refusals, in the same order"
    );
    assert!(
        first.suite.to_canonical_json().ends_with('\n'),
        "a file without a trailing newline shows up modified"
    );
}

#[test]
fn a_synthesised_suite_survives_being_written_and_read_back() {
    // §49's step-1 acceptance, now against a suite nobody wrote by hand: every reference in it is a
    // name, so a suite generated here resolves in a process that never saw the `EssIr`.
    let synthesis = synthesize(&example("billing"));
    let written = synthesis.suite.to_canonical_json();

    let read =
        ess_conformance::ConformanceSuite::from_json(&written).expect("a written suite parses");

    assert_eq!(read, synthesis.suite, "what came back is what went in");
    assert_eq!(read.provenance.system, "billing");
    assert_eq!(read.provenance.specification_version, "v3");
}

#[test]
fn the_dependency_set_names_the_types_the_scenario_is_made_of() {
    // §37's second correction. A `derived_from` would list the command, the outcome and the error;
    // what a later consumer needs is `Money`, the view it asserts and the actor it acts as — the
    // things whose change makes a stored result stale.
    let synthesis = synthesize(&example("billing"));
    let scenario = synthesis
        .suite
        .scenario(
            &ScenarioId::parse("billing.invoice.CreateInvoice/outcome/accepted").expect("an id"),
        )
        .expect("the happy path");

    let depends: BTreeSet<String> = scenario.source.iter().map(ToString::to_string).collect();
    for expected in [
        "command billing.invoice.CreateInvoice",
        "outcome billing.invoice.CreateInvoice/accepted",
        "event billing.invoice.InvoiceCreated",
        "entity billing.invoice.Invoice",
        "type billing.invoice.Money",
        "type billing.invoice.Email",
        "type billing.invoice.InvoiceId",
        "view billing.invoice.InvoiceById",
        "view billing.invoice.OutstandingInvoices",
        "actor billing.invoice.Customer",
    ] {
        assert!(
            depends.contains(expected),
            "the scenario depends on `{expected}`: {depends:?}"
        );
    }
}

#[test]
fn an_actor_is_named_only_where_the_specification_grants_the_command() {
    // `ExecuteCommand::actor` is `Option`, and filling it where no grant exists would be an
    // authorization claim the specification does not make.
    let synthesis = synthesize(&example("billing"));
    let actor = |id: &str| {
        steps(&synthesis, id).iter().find_map(|step| match step {
            ScenarioStep::ExecuteCommand { actor, .. } => {
                Some(actor.as_ref().map(ToString::to_string))
            }
            _ => None,
        })
    };

    assert_eq!(
        actor("billing.invoice.CreateInvoice/outcome/accepted"),
        Some(Some("billing.invoice.Customer".to_owned())),
        "`Customer` may invoke `CreateInvoice`"
    );
    assert_eq!(
        actor("billing.email.SendEmail/outcome/sent"),
        Some(None),
        "no actor is granted `SendEmail`; a binding invokes it"
    );
}

// ---- fixtures for corners neither example carries ------------------------------------------------

/// A guard that parses, validates, compiles — and is decidable by nothing.
///
/// `ess-domain` checks a `when` path's **first** segment against the input's field names and says
/// that resolving a deeper one belongs with the IR. So `amount.vat` on a two-field `Money` is a
/// specification defect that reaches synthesis, which is exactly what makes it a fixture worth
/// having: it is the input on which "retry on `Unknown`" and "refuse on `Unknown`" differ.
const UNDECIDED: &str = r"
format: ess/1
system: undecided
version: v1
domain: undecided.orders

types:
  - name: undecided.orders.Money
    kind: struct
    fields:
      - name: amount
        type: Decimal
      - name: currency
        type: String

errors:
  - name: undecided.orders.Refused
    summary: The order was refused.

events:
  - name: undecided.orders.Taxed
    fields:
      - name: currency
        type: String

commands:
  - name: undecided.orders.TaxOrder
    input:
      - name: amount
        type: undecided.orders.Money
    outcomes:
      - name: taxed
        when: amount.vat > 0
        emits:
          - undecided.orders.Taxed
      - name: untaxed
        error: undecided.orders.Refused
";

/// A guard that is decided every time, and satisfied by no value the field can hold.
///
/// `quantity` is an `Integer` and the guard compares it to `0.5`. Every candidate is decided —
/// there is no `Unknown` here — and none of them satisfies it, because none of them can: the
/// flattener refuses a fractional value for an `Integer` rather than rounding it, which is what
/// stops a candidate deciding `quantity == 1` differently from the system it is testing.
const UNSATISFIABLE: &str = r"
format: ess/1
system: unsatisfiable
version: v1
domain: unsatisfiable.orders

errors:
  - name: unsatisfiable.orders.Refused
    summary: The order was refused.

events:
  - name: unsatisfiable.orders.Counted
    fields:
      - name: quantity
        type: Integer

commands:
  - name: unsatisfiable.orders.CountOrder
    input:
      - name: quantity
        type: Integer
    outcomes:
      - name: counted
        when: quantity == 0.5
        emits:
          - unsatisfiable.orders.Counted
      - name: refused
        error: unsatisfiable.orders.Refused
";

/// A view whose filter reads a field rather than the state.
const UNKNOWABLE_VIEW: &str = r"
format: ess/1
system: hidden
version: v1
domain: hidden.orders

types:
  - name: hidden.orders.OrderId
    kind: newtype
    of: Uuid

entities:
  - name: hidden.orders.Order
    identity:
      name: order_id
      type: hidden.orders.OrderId
    fields:
      - name: weight_grams
        type: Integer
    lifecycle:
      initial: Placed
      states: [Placed, Shipped]
      terminal: [Shipped]
      transitions:
        - name: ship
          from: [Placed]
          to: Shipped

events:
  - name: hidden.orders.OrderPlaced
    fields:
      - name: order_id
        type: hidden.orders.OrderId

  - name: hidden.orders.OrderShipped
    fields:
      - name: order_id
        type: hidden.orders.OrderId

commands:
  - name: hidden.orders.PlaceOrder
    input:
      - name: weight_grams
        type: Integer
    outcomes:
      - name: accepted
        creates: hidden.orders.Order
        emits:
          - hidden.orders.OrderPlaced

  - name: hidden.orders.ShipOrder
    input:
      - name: order_id
        type: hidden.orders.OrderId
    outcomes:
      - name: shipped
        moves: hidden.orders.Order.ship
        emits:
          - hidden.orders.OrderShipped

views:
  - name: hidden.orders.HeavyOrders
    source: hidden.orders.Order
    consistency: read_your_writes
    filter: weight_grams > 100
    fields:
      - name: order_id
        type: hidden.orders.OrderId
";
