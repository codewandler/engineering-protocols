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
use ess_conformance::scenario::{ScenarioStep, ScenarioValue, ViewExpectation};
use ess_conformance::synthesize::{synthesize, RefusalCause, Synthesis, Unreachable};
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
            ScenarioStep::CaptureInstance { .. } => "capture",
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

/// The input one scenario sends, in the *last* command it runs — the one it is about.
fn sent(synthesis: &Synthesis, id: &str) -> BTreeMap<String, ScenarioValue> {
    steps(synthesis, id)
        .iter()
        .rev()
        .find_map(|step| match step {
            ScenarioStep::ExecuteCommand { input, .. } => Some(input.clone()),
            _ => None,
        })
        .unwrap_or_else(|| panic!("`{id}` invokes a command"))
}

/// The literal half of that input, which is every field a guard is allowed to read.
fn literals(synthesis: &Synthesis, id: &str) -> BTreeMap<String, Node> {
    sent(synthesis, id)
        .into_iter()
        .filter_map(|(field, value)| value.as_literal().cloned().map(|node| (field, node)))
        .collect()
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
        covered.len(),
        declared.len(),
        "and every one of them is now a scenario rather than a refusal: {:?}",
        refused(&synthesis)
    );
    assert!(
        ids(&synthesis)
            .iter()
            .filter(|id| id.contains("/outcome/"))
            .count()
            == declared.len(),
        "the eight outcomes billing declares are eight scenarios: {:?}",
        ids(&synthesis)
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
                        &literals(&synthesis, "billing.invoice.CreateInvoice/outcome/rejected"),
                    )
                    .expect("the input fits"),
                ),
            ),
        ),
    ] {
        let input = literals(&synthesis, id);
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

// ---- §19: lifecycle, over an instance the specification can name ---------------------------------

#[test]
fn a_scenario_that_moves_an_instance_names_the_one_an_earlier_step_created() {
    // The gate, end to end. `PayInvoice` cannot be exercised against nothing: the invoice has to
    // exist and be `Issued` first, and the scenario has to say *which* invoice it is paying. The
    // identity is never fabricated — it is bound out of the event `CreateInvoice/accepted` declares
    // publishes it, so the value is the target's.
    let synthesis = synthesize(&example("billing"));
    let id = "billing.invoice.Invoice/transition/settle/by/billing.invoice.PayInvoice/settled";

    assert_eq!(
        shape(&synthesis, id),
        vec![
            "execute",
            "outcome",
            "capture",
            "execute",
            "outcome",
            "execute",
            "outcome",
            "event",
            "eventually-view",
            "query",
            "view"
        ],
        "create, bind the new invoice, issue it, then pay it and assert what that promises"
    );

    let bound = steps(&synthesis, id)
        .iter()
        .find_map(|step| match step {
            ScenarioStep::CaptureInstance {
                instance,
                entity,
                event,
                field,
            } => Some((
                instance.to_string(),
                entity.to_string(),
                event.to_string(),
                field.clone(),
            )),
            _ => None,
        })
        .expect("the scenario binds the invoice it created");
    assert_eq!(
        bound,
        (
            "invoice".to_owned(),
            "billing.invoice.Invoice".to_owned(),
            "billing.invoice.InvoiceCreated".to_owned(),
            "invoice_id".to_owned()
        ),
        "the identity is read where the model says it is published, and nowhere else"
    );

    // Every later command names that binding rather than a value, and the fields the guard reads
    // are still literals the synthesizer decided.
    for step in steps(&synthesis, id) {
        let ScenarioStep::ExecuteCommand { command, input, .. } = step else {
            continue;
        };
        if command.to_string() == "billing.invoice.CreateInvoice" {
            continue;
        }
        assert_eq!(
            input.get("invoice_id"),
            Some(&ScenarioValue::instance(
                "invoice".parse().expect("a valid instance name")
            )),
            "`{command}` must act on the invoice step one created, not on an invented id"
        );
    }
    assert!(
        sent(&synthesis, id)
            .get("amount")
            .and_then(ScenarioValue::as_literal)
            .is_some(),
        "and the field its guard reads is still a decided value"
    );
}

#[test]
fn every_declared_transition_has_a_scenario_that_proves_it_can_occur() {
    // §19's first class — *for every declared transition, generate a scenario that proves it can
    // occur under a valid witness* — now in the suite rather than in the refusals. Read from the
    // model rather than from a list, so a transition added to the example has to gain a scenario.
    let ir = example("billing");
    let synthesis = synthesize(&ir);

    let mut expected: BTreeSet<String> = BTreeSet::new();
    for (entity, drivers) in ir.drivers() {
        for driver in &drivers {
            if let Some(transition) = driver.effect.transition() {
                expected.insert(format!(
                    "{}/transition/{}/by/{}/{}",
                    entity.name(),
                    transition.name,
                    driver.command.name,
                    driver.outcome.name
                ));
            }
        }
    }
    assert_eq!(expected.len(), 3, "billing declares three moves");

    let synthesised: BTreeSet<String> = ids(&synthesis)
        .into_iter()
        .filter(|id| id.contains("/transition/"))
        .collect();
    assert_eq!(
        synthesised, expected,
        "one scenario per declared move, naming the outcome that drives it"
    );
    assert!(
        refused(&synthesis)
            .iter()
            .all(|id| !id.contains("/transition/")),
        "and none of them is refused: {:?}",
        refused(&synthesis)
    );
}

#[test]
fn a_move_that_is_illegal_in_a_state_is_attempted_with_the_input_that_would_have_worked() {
    // §19's second class, and the assertion that makes it a check rather than a formality. The
    // input sent is the one that reaches the *moving* branch — send an amount the branch would
    // refuse anyway and the scenario passes whether or not the state rule holds — and what is
    // required is that the event the move publishes did not happen, because the specification
    // declares no outcome for this combination and inventing one would be inventing the rejection
    // mechanism §19 says must come from the declared semantics.
    let synthesis = synthesize(&example("billing"));
    let id = "billing.invoice.Invoice/state/Paid/refuses/billing.invoice.PayInvoice";

    assert_eq!(
        shape(&synthesis, id),
        vec![
            "execute", "outcome", "capture", "execute", "outcome", "execute", "outcome", "execute",
            "no-event"
        ],
        "drive the invoice to `Paid`, then pay it again — and assert no outcome, because none is \
         declared for this"
    );

    let amount = sent(&synthesis, id)
        .get("amount")
        .and_then(ScenarioValue::as_literal)
        .cloned()
        .expect("the second payment carries an amount");
    let ir = example("billing");
    let command = ir
        .commands
        .get(&QualifiedName::new("billing.invoice.PayInvoice").expect("valid"))
        .expect("declared");
    let settled = command
        .outcomes
        .iter()
        .find(|outcome| outcome.name.as_str() == "settled")
        .expect("the branch that moves it");
    // Decided against the guard a second time, from outside the synthesizer. The identity is
    // borrowed from the sibling scenario that sends a literal one: a guard may not read the field
    // that names the instance — `ess-domain` refuses that, because invariant 13 makes an identity
    // opaque — so which id stands in the map cannot change the answer.
    let mut input = literals(&synthesis, "billing.invoice.PayInvoice/outcome/rejected");
    input.insert("amount".to_owned(), amount);
    let facts = flatten(&ir, command, &input).expect("the input fits its declared types");
    assert_eq!(
        facts.decide(when(settled).expect("`settled` is guarded")),
        Decision::Satisfied,
        "the attempt has to be one the branch would otherwise have taken; anything else proves \
         nothing about the state"
    );

    let absent: Vec<String> = steps(&synthesis, id)
        .iter()
        .filter_map(|step| match step {
            ScenarioStep::ExpectNoEvent { event } => Some(event.to_string()),
            _ => None,
        })
        .collect();
    assert_eq!(
        absent,
        vec!["billing.invoice.InvoicePaid"],
        "what must not happen is the fact the move would have published"
    );
}

#[test]
fn a_state_reached_only_through_a_branch_no_input_reaches_is_refused_rather_than_arranged() {
    // The refusal that survives the gate. `Held` is declared, reachable and driven — and the branch
    // that drives it is guarded by `quantity == 0.5`, which no `Integer` satisfies. So the state
    // exists on paper and no scenario can set it up, and everything downstream of it says so with
    // the code and the reason rather than being dropped: a suite quietly holding fewer checks than
    // the specification requires is the failure §36 exists to prevent.
    let synthesis = synthesize(&fixture(UNWITNESSABLE_ROUTE));

    let refusal = synthesis
        .refused(code(4))
        .next()
        .unwrap_or_else(|| panic!("the route refuses: {:?}", refused(&synthesis)));
    let RefusalCause::InstanceRequired { reason, .. } = &refusal.cause else {
        panic!("{refusal}")
    };
    let Unreachable::Unwitnessable { outcome } = reason else {
        panic!("{refusal}")
    };
    assert_eq!(
        outcome.to_string(),
        "stuck.orders.HoldOrder/held",
        "the refusal names the branch that cannot be reached, so a reader goes to its own refusal \
         for the reason"
    );
    assert!(
        synthesis.refused(code(3)).any(|other| other
            .scenario
            .as_ref()
            .is_some_and(|id| id.to_string() == "stuck.orders.HoldOrder/outcome/held")),
        "and that refusal is there, carrying the guard nothing satisfies: {:?}",
        refused(&synthesis)
    );

    // Refusing one branch is not refusing the family: what does not depend on `Held` is synthesised.
    assert!(
        ids(&synthesis)
            .iter()
            .any(|id| id == "stuck.orders.PlaceOrder/outcome/accepted"),
        "{:?}",
        ids(&synthesis)
    );
}

#[test]
fn an_entity_nothing_creates_cannot_be_acted_on_and_says_so() {
    // The other surviving reason, and the one `ess-domain` deliberately does not refuse: an entity
    // may arrive from a migration or from a system outside this document. It is still true that no
    // scenario can act on an instance nothing brings into existence, and the refusal is where that
    // becomes visible rather than a suite that is quietly three checks short.
    let synthesis = synthesize(&fixture(NOTHING_CREATES));

    let refusal = synthesis
        .refused(code(4))
        .next()
        .unwrap_or_else(|| panic!("the entity refuses: {:?}", refused(&synthesis)));
    let RefusalCause::InstanceRequired { reason, .. } = &refusal.cause else {
        panic!("{refusal}")
    };
    assert_eq!(*reason, Unreachable::NothingCreates);
    assert!(
        refusal.hint().contains("creates:"),
        "the hint names the key to write: {}",
        refusal.hint()
    );
}

#[test]
fn an_outcome_that_updates_an_instance_acts_on_one_the_scenario_created() {
    // `updates:` is why `examples/oracle-fixture/` declares `AmendOrder`: billing has no outcome
    // that changes an entity without moving it, so §20's case has no instance there. It names no
    // state, so the cheapest reachable one — where the lifecycle starts — is where it is arranged.
    let synthesis = synthesize(&example("oracle-fixture"));
    let id = "oracle.order.AmendOrder/outcome/amended";

    assert_eq!(
        shape(&synthesis, id),
        vec![
            "execute",
            "outcome",
            "capture",
            "execute",
            "outcome",
            "event",
            "eventually-view",
            "query",
            "view"
        ],
        "place an order, bind it, then amend that one"
    );
    assert_eq!(
        sent(&synthesis, id).get("order_id"),
        Some(&ScenarioValue::instance(
            "order".parse().expect("a valid instance name")
        )),
        "the amendment acts on the order step one placed"
    );
    let (block, open) = expectation(&synthesis, id, "oracle.order.OpenOrders");
    assert_eq!(block, "expect", "`OpenOrders` is `read_your_writes`");
    assert!(
        matches!(open, ViewExpectation::Contains { .. }),
        "an amended order has not moved, so it is still open: {open:?}"
    );
}

#[test]
fn a_move_is_observed_through_the_view_the_state_it_left_is_filtered_on() {
    // What a transition scenario asserts beyond "the command answered": the invoice was `Issued`
    // and is now `Paid`, so the view filtered on `Issued` must no longer hold it. Without this the
    // scenario proves a branch was taken and nothing about the state it left.
    let synthesis = synthesize(&example("billing"));
    let settled = "billing.invoice.Invoice/transition/settle/by/billing.invoice.PayInvoice/settled";
    let issued = "billing.invoice.Invoice/transition/issue/by/billing.invoice.IssueInvoice/issued";

    let (_, after_payment) =
        expectation(&synthesis, settled, "billing.invoice.OutstandingInvoices");
    assert!(
        matches!(after_payment, ViewExpectation::Excludes { .. }),
        "a paid invoice is not outstanding: {after_payment:?}"
    );
    let (_, after_issue) = expectation(&synthesis, issued, "billing.invoice.OutstandingInvoices");
    assert!(
        matches!(after_issue, ViewExpectation::Contains { .. }),
        "and an issued one is, which is the other half of the same rule: {after_issue:?}"
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
        instance: order_id
        emits:
          - hidden.orders.OrderPlaced

  - name: hidden.orders.ShipOrder
    input:
      - name: order_id
        type: hidden.orders.OrderId
    outcomes:
      - name: shipped
        moves: hidden.orders.Order.ship
        instance: order_id
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

/// A state whose only way in is a branch no input reaches.
///
/// `hold` is guarded by `quantity == 0.5` and `quantity` is an `Integer`, so every candidate is
/// decided and none satisfies it. `Held` is therefore declared, reachable on paper — `ess-domain`
/// refuses a state nothing reaches, so it has to be — and impossible to arrange, which is the one
/// shape of unreachability a valid specification can still carry.
const UNWITNESSABLE_ROUTE: &str = r"
format: ess/1
system: stuck
version: v1
domain: stuck.orders

types:
  - name: stuck.orders.OrderId
    kind: newtype
    of: Uuid

errors:
  - name: stuck.orders.Refused
    summary: The hold was refused.

entities:
  - name: stuck.orders.Order
    identity:
      name: order_id
      type: stuck.orders.OrderId
    lifecycle:
      initial: Placed
      states: [Placed, Held, Cancelled]
      terminal: [Cancelled]
      transitions:
        - name: hold
          from: [Placed]
          to: Held
        - name: cancel
          from: [Held]
          to: Cancelled

events:
  - name: stuck.orders.OrderPlaced
    fields:
      - name: order_id
        type: stuck.orders.OrderId

  - name: stuck.orders.OrderHeld
    fields:
      - name: order_id
        type: stuck.orders.OrderId

  - name: stuck.orders.OrderCancelled
    fields:
      - name: order_id
        type: stuck.orders.OrderId

commands:
  - name: stuck.orders.PlaceOrder
    outcomes:
      - name: accepted
        creates: stuck.orders.Order
        instance: order_id
        emits:
          - stuck.orders.OrderPlaced

  - name: stuck.orders.HoldOrder
    input:
      - name: order_id
        type: stuck.orders.OrderId
      - name: quantity
        type: Integer
    outcomes:
      - name: held
        when: quantity == 0.5
        moves: stuck.orders.Order.hold
        instance: order_id
        emits:
          - stuck.orders.OrderHeld
      - name: refused
        error: stuck.orders.Refused

  - name: stuck.orders.CancelOrder
    input:
      - name: order_id
        type: stuck.orders.OrderId
    outcomes:
      - name: cancelled
        moves: stuck.orders.Order.cancel
        instance: order_id
        emits:
          - stuck.orders.OrderCancelled
";

/// An entity that moves and that nothing creates.
///
/// Legal — an order may arrive from a migration, which is why `ess-domain` does not refuse it — and
/// still impossible to write a scenario about, because no step can bring one into existence.
const NOTHING_CREATES: &str = r"
format: ess/1
system: migrated
version: v1
domain: migrated.orders

types:
  - name: migrated.orders.OrderId
    kind: newtype
    of: Uuid

entities:
  - name: migrated.orders.Order
    identity:
      name: order_id
      type: migrated.orders.OrderId
    lifecycle:
      initial: Placed
      states: [Placed, Shipped]
      terminal: [Shipped]
      transitions:
        - name: ship
          from: [Placed]
          to: Shipped

events:
  - name: migrated.orders.OrderShipped
    fields:
      - name: order_id
        type: migrated.orders.OrderId

commands:
  - name: migrated.orders.ShipOrder
    input:
      - name: order_id
        type: migrated.orders.OrderId
    outcomes:
      - name: shipped
        moves: migrated.orders.Order.ship
        instance: order_id
        emits:
          - migrated.orders.OrderShipped
";
