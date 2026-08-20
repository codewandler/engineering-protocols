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
use ess_compiler::ir::{EssIr, ResolvedCondition};
use ess_compiler::resolve::compile;
use ess_compiler::source::SourceMap;
use ess_conformance::scenario::{BindingAspect, ScenarioStep, ScenarioValue, ViewExpectation};
use ess_conformance::synthesize::{synthesize, BindingGap, RefusalCause, Synthesis, Unreachable};
use ess_conformance::{flatten, when, Decision, ScenarioId};
use ess_domain::binding::{Delivery, Failure};
use ess_domain::entity::StateName;
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
            ScenarioStep::RedeliverEvent { .. } => "redeliver",
            ScenarioStep::ExpectInvocation { .. } => "invocation",
            ScenarioStep::QueryView { .. } => "query",
            ScenarioStep::ExpectView { .. } => "view",
            ScenarioStep::EventuallyEvent { .. } => "eventually-event",
            ScenarioStep::EventuallyView { .. } => "eventually-view",
        })
        .collect()
}

/// The shape of one scenario with its negative event assertions left out.
///
/// There is one of those per event the *specification* declares, so pinning them inside every shape
/// assertion would make an unrelated new event fail half this file with nothing readable in any of
/// the messages. `the_refusal_branch_asserts_that_no_event_the_specification_declares_occurred` is
/// where the count and the names are held.
fn asserted(synthesis: &Synthesis, id: &str) -> Vec<&'static str> {
    shape(synthesis, id)
        .into_iter()
        .filter(|step| *step != "no-event")
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
fn every_declared_outcome_is_either_a_scenario_or_a_named_refusal_or_asserted_by_the_state_family()
{
    // §36's rule, made checkable: silently omitting a scenario is the one unacceptable option, so
    // the lists together must cover every outcome the specification declares. The fixture reaches
    // the state where that rule bites — billing declares eleven outcomes and only eight of them get
    // a scenario under their own name — because a specification whose outcomes are all covered one
    // way could not tell a suite that refuses well from one that drops what it cannot do.
    //
    // The third bucket is where a `wrong_state:` branch goes, and it is checked by *finding the
    // assertion*, not by allowing an exception: the branch is required by name in at least one
    // illegal-move scenario, once per state it answers in. Anything less would have made "covered"
    // a word this test says rather than a thing it observes.
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
    assert_eq!(declared.len(), 11, "the fixture declares eleven outcomes");

    let wrong_state: BTreeSet<String> = ir
        .commands
        .values()
        .flat_map(|command| {
            command
                .outcomes
                .iter()
                .filter(|outcome| outcome.condition == ResolvedCondition::WrongState)
                .map(move |outcome| format!("{}/outcome/{}", command.name, outcome.name))
        })
        .collect();
    assert_eq!(
        wrong_state.len(),
        3,
        "the three lifecycle commands each declare what they answer in a state they do not act from"
    );

    // Every branch required by an `ExpectOutcome` step anywhere in the suite, written as the same
    // `<command>/outcome/<branch>` string the ids use.
    let asserted: BTreeSet<String> = synthesis
        .suite
        .scenarios
        .values()
        .flat_map(|scenario| scenario.steps.iter())
        .filter_map(|step| match step {
            ScenarioStep::ExpectOutcome { outcome } => {
                Some(format!("{}/outcome/{}", outcome.command, outcome.outcome))
            }
            _ => None,
        })
        .collect();
    assert!(
        wrong_state.is_subset(&asserted),
        "a wrong-state branch with no scenario of its own must be required by one that exists: \
         {wrong_state:?} against {asserted:?}"
    );

    let covered: BTreeSet<String> = ids(&synthesis)
        .into_iter()
        .chain(refused(&synthesis))
        .filter(|id| id.contains("/outcome/"))
        .chain(wrong_state.iter().cloned())
        .collect();

    assert_eq!(
        covered, declared,
        "an outcome that is neither in the suite, nor in a refusal, nor asserted by the \
         illegal-move family has disappeared"
    );
    assert_eq!(
        ids(&synthesis)
            .iter()
            .filter(|id| id.contains("/outcome/"))
            .count(),
        declared.len() - wrong_state.len(),
        "and every outcome an input can reach is a scenario rather than a refusal: {:?}",
        refused(&synthesis)
    );
}

#[test]
fn the_refusal_branch_asserts_that_no_event_the_specification_declares_occurred() {
    // Design §10's worked example, both halves. The negative assertion is the half that is easy to
    // leave out and impossible to notice missing: without it the scenario passes against an
    // implementation that refuses the command and emits `InvoiceCreated` anyway.
    //
    // And it is every declared event, not only the sibling branch's. `ESS-CF-NO-EVENT` names the
    // rule "a branch publishes no event it does not declare it emits"; asking it only of a sibling
    // let a refused `CreateInvoice` announce a cancellation, which is the wider hole the same
    // sentence always covered.
    let synthesis = synthesize(&example("billing"));

    assert_eq!(
        shape(&synthesis, "billing.invoice.CreateInvoice/outcome/rejected"),
        vec![
            "execute", "outcome", "error", "no-event", "no-event", "no-event", "no-event",
            "no-event", "no-event"
        ],
        "`→ rejected`, `→ InvalidAmount`, and none of the six events billing declares"
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
        vec![
            "billing.email.DeliveryEscalated",
            "billing.email.EmailSent",
            "billing.invoice.InvoiceCancelled",
            "billing.invoice.InvoiceCreated",
            "billing.invoice.InvoiceIssued",
            "billing.invoice.InvoicePaid",
        ],
        "a branch that emits nothing must publish none of them"
    );

    // The other side of the same rule: the branch that *does* emit one is required to publish that
    // one and no other, which is what stops this from being a check only refusals get.
    let accepted: Vec<String> = steps(&synthesis, "billing.invoice.CreateInvoice/outcome/accepted")
        .iter()
        .filter_map(|step| match step {
            ScenarioStep::ExpectNoEvent { event } => Some(event.to_string()),
            _ => None,
        })
        .collect();
    assert!(
        !accepted.contains(&"billing.invoice.InvoiceCreated".to_owned())
            && accepted.contains(&"billing.invoice.InvoiceCancelled".to_owned()),
        "the event it emits is required present and every other declared one absent: {accepted:?}"
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

    let named: Vec<&str> = errors.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(
        named,
        vec![
            "billing.email.Undeliverable",
            "billing.invoice.InvalidAmount",
            // Eight of these, one per illegal-move scenario: `InvoiceStateConflict` is what the
            // three lifecycle commands declare they answer with when the invoice is somewhere they
            // do not act from, and asserting it is the whole point of `wrong_state:`.
            "billing.invoice.InvoiceStateConflict",
            "billing.invoice.InvoiceStateConflict",
            "billing.invoice.InvoiceStateConflict",
            "billing.invoice.InvoiceStateConflict",
            "billing.invoice.InvoiceStateConflict",
            "billing.invoice.InvoiceStateConflict",
            "billing.invoice.InvoiceStateConflict",
            "billing.invoice.InvoiceStateConflict",
            "billing.invoice.InvalidAmount",
        ],
        "every refusal branch names its declared error"
    );
    assert!(
        errors.iter().all(|(_, fields)| *fields == 0),
        "and none claims a field value — including the wrong-state ones, whose declared `state` \
         field the model does not say where to fill from: {errors:?}"
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

        let steps = shape(&synthesis, scenario);
        assert_eq!(
            steps.iter().take(4).copied().collect::<Vec<_>>(),
            vec!["inject", "execute", "outcome", "error"],
            "the injection comes first, and the branch still asserts its error"
        );
        assert!(
            steps.len() > 4 && steps[4..].iter().all(|step| *step == "no-event"),
            "and then that no event this specification declares was published: {steps:?}"
        );
        assert!(
            !shape(&synthesis, other).contains(&"inject"),
            "the sibling branch is reached by an input, so injecting anything there would be a \
             control the specification does not claim"
        );
    }
}

#[test]
fn an_event_assertion_carries_the_shape_the_specification_declares_and_no_value_at_all() {
    // §13, and the line the model draws through the middle of an event's payload. What the
    // specification *declares* is the field set and the types, and that is what the assertion
    // carries: a newtype is transparent, so `invoice_id` is a `Uuid`, and a struct contributes one
    // leaf per field under a dotted path.
    //
    // What it does not carry is a **value**. Nothing in the model relates a command's input to a
    // payload field, so `amount = 120` here would be a match on a shared field name — and a suite
    // that guessed would fail an implementation doing nothing wrong. `wrong-event-payload` in the
    // fault matrix is that gap, kept visible rather than closed by inference.
    let synthesis = synthesize(&example("billing"));
    let created = steps(&synthesis, "billing.invoice.CreateInvoice/outcome/accepted")
        .iter()
        .find_map(|step| match step {
            ScenarioStep::ExpectEvent {
                event,
                payload,
                shape,
            } if event.to_string() == "billing.invoice.InvoiceCreated" => {
                Some((payload.clone(), shape.clone()))
            }
            _ => None,
        })
        .expect("`accepted` asserts the event it emits");

    let (payload, shape) = created;
    assert!(
        payload.is_empty(),
        "a value is asserted only where the model says where it comes from, and it does not: \
         {payload:?}"
    );

    let leaves: Vec<(&str, String)> = shape
        .leaves()
        .iter()
        .map(|(path, leaf)| (path.as_str(), leaf.holds.to_string()))
        .collect();
    assert_eq!(
        leaves,
        vec![
            ("amount.amount", "a Decimal".to_owned()),
            ("amount.currency", "a String".to_owned()),
            ("customer_email", "a String".to_owned()),
            ("invoice_id", "a Uuid".to_owned()),
        ],
        "every declared field, flattened by the same rule `flatten` applies to a command input"
    );
    assert!(
        shape.leaves().values().all(|leaf| !leaf.optional),
        "none of `InvoiceCreated`'s fields is `Optional`, so none of them may be missing"
    );
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
        asserted(&synthesis, created),
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
fn a_view_the_entity_has_not_reached_yet_is_asserted_to_exclude_the_instance_by_name() {
    // The negative view assertion, decided rather than assumed: `OutstandingInvoices` filters on
    // `state == Issued` and `CreateInvoice` leaves the invoice in `Draft`, so the filter evaluates
    // to `False` against the one fact the scenario knows.
    //
    // And it excludes *this* invoice rather than every row. With an empty field set the assertion
    // reads "the view is empty", which is the same claim only while §8's isolation holds; against a
    // target shared with anything else it fails on somebody else's row. The identity comes from the
    // branch's own `instance:` — `creates:` publishes it in an event — so nothing is guessed.
    let synthesis = synthesize(&example("billing"));
    let (_, expectation) = expectation(
        &synthesis,
        "billing.invoice.CreateInvoice/outcome/accepted",
        "billing.invoice.OutstandingInvoices",
    );

    let ViewExpectation::Excludes { fields } = expectation else {
        panic!("an invoice in `Draft` must not be outstanding: {expectation:?}")
    };
    assert_eq!(
        fields.get("invoice_id"),
        Some(&ScenarioValue::observed(
            "billing.invoice.InvoiceCreated".parse().expect("an event"),
            "invoice_id"
        )),
        "the row is named by the identity the creating branch declares it publishes: {fields:?}"
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

    // The negative assertions are filtered out here and asserted in
    // `the_refusal_branch_asserts_that_no_event_the_specification_declares_occurred`: they are one
    // per declared event, and repeating that count in every shape assertion would make an unrelated
    // new event fail six tests with nothing to read in any of them.
    assert_eq!(
        asserted(&synthesis, id),
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
    // required is the branch the specification declares for this combination, the error it names,
    // and that the event the move publishes did not happen.
    let synthesis = synthesize(&example("billing"));
    let id = "billing.invoice.Invoice/state/Paid/refuses/billing.invoice.PayInvoice";

    assert_eq!(
        asserted(&synthesis, id),
        vec![
            "execute", "outcome", "capture", "execute", "outcome", "execute", "outcome", "execute",
            "outcome", "error"
        ],
        "drive the invoice to `Paid`, then pay it again — and require the declared `wrong-state` \
         branch and its error, not merely that nothing happened"
    );
    let forbidden = shape(&synthesis, id)
        .into_iter()
        .filter(|step| *step == "no-event")
        .count();
    assert_eq!(
        forbidden, 6,
        "and none of the six events billing declares: the branch that was taken emits nothing, and \
         every declared event belongs to a branch this invocation did not run"
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
    assert!(
        absent.contains(&"billing.invoice.InvoicePaid".to_owned()),
        "what must not happen is first of all the fact the move would have published: {absent:?}"
    );
    assert_eq!(
        absent.len(),
        6,
        "and every other declared event too — a command nothing honoured took no branch, and every \
         declared event belongs to a branch, so publishing any of them is publishing one this \
         invocation never ran: {absent:?}"
    );
}

#[test]
fn an_illegal_move_requires_the_branch_and_the_declared_error_rather_than_merely_failing() {
    // §19 asks for two things of this family, and until `wrong_state:` existed the model could
    // express only one. "Must not reach `Cancelled`" was asserted; "the exact rejection mechanism
    // must come from the declared command/error semantics" was not, so an implementation that
    // refused with the wrong error — or with an untyped infrastructure failure — passed.
    //
    // The fixture is driven to the state where that rule is load-bearing before anything is
    // asserted about it: `Paid` really is a state `cancel` does not run from, which is what makes
    // the combination illegal in the first place.
    let ir = example("billing");
    let synthesis = synthesize(&ir);
    let id = "billing.invoice.Invoice/state/Paid/refuses/billing.invoice.CancelInvoice";

    let cancel = ir
        .commands
        .get(&QualifiedName::new("billing.invoice.CancelInvoice").expect("valid"))
        .expect("declared");
    let paid = StateName::new("Paid").expect("a state name");
    let wrong = ir.wrong_states(cancel);
    assert!(
        wrong.values().any(|states| states.contains(&paid)),
        "the fixture only tests the rule if `Paid` is a state `cancel` cannot start from: {wrong:?}"
    );

    let branch = steps(&synthesis, id)
        .iter()
        .rev()
        .find_map(|step| match step {
            ScenarioStep::ExpectOutcome { outcome } => Some(outcome.to_string()),
            _ => None,
        })
        .expect("the scenario requires a branch");
    assert_eq!(
        branch, "billing.invoice.CancelInvoice/wrong-state",
        "the branch required is the one the command declares for a subject it will not act on"
    );

    let reported: Vec<String> = steps(&synthesis, id)
        .iter()
        .filter_map(|step| match step {
            ScenarioStep::ExpectError { error, .. } => Some(error.to_string()),
            _ => None,
        })
        .collect();
    assert_eq!(
        reported,
        vec!["billing.invoice.InvoiceStateConflict".to_owned()],
        "and the error is the declared one, which is what tells a wrong refusal from a right one"
    );

    assert_eq!(
        synthesis.refused(code(12)).count(),
        0,
        "nothing about this family is refused any more: {:?}",
        refused(&synthesis)
    );
}

#[test]
fn a_command_that_declares_no_wrong_state_answer_is_refused_by_name_beside_its_scenario() {
    // The other half, and the reason the refusal stays in the code: a specification that has not
    // adopted the construct still gets the scenario, and still gets told what its suite is not
    // checking. §36's rule is about silence, not about absence — a reader of a passing run cannot
    // otherwise tell a scenario that asserts everything the section asks for from one that asserts
    // what was left.
    let synthesis = synthesize(&fixture(NO_DECLARED_REFUSAL));
    let id = "quiet.orders.Order/state/Shipped/refuses/quiet.orders.ShipOrder";

    assert!(
        ids(&synthesis).iter().any(|known| known == id),
        "the scenario is still produced; what it cannot assert is not a reason to drop it: {:?}",
        ids(&synthesis)
    );
    assert!(
        !shape(&synthesis, id).contains(&"error"),
        "and it asserts no error, because the command names none for this"
    );

    let refusal = synthesis
        .refused(code(12))
        .find(|refusal| {
            refusal
                .scenario
                .as_ref()
                .is_some_and(|scenario| scenario.to_string() == id)
        })
        .unwrap_or_else(|| panic!("the mechanism refuses: {:?}", refused(&synthesis)));

    let rendered = refusal.to_string();
    for required in [
        "quiet.orders.ShipOrder",
        "quiet.orders.Order",
        "Shipped",
        "no `wrong_state:` outcome and no declared error",
    ] {
        assert!(
            rendered.contains(required),
            "the refusal names the combination the document is silent about; {required:?} is \
             missing from: {rendered}"
        );
    }
    assert!(
        refusal.hint().contains("wrong_state:"),
        "and says the repair is one branch in the document, not a change to this crate: {}",
        refusal.hint()
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
        asserted(&synthesis, id),
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

// ---- §16, §17 and §18: a binding is four claims ------------------------------------------------

#[test]
fn every_clause_of_every_binding_is_either_a_scenario_or_a_named_refusal() {
    // §36's rule at the binding level, driven by `BindingAspect::ALL` rather than by a list written
    // here: an aspect added without being synthesised or refused fails this rather than quietly
    // producing a suite one check short. The fixture reaches the state where that bites — the
    // oracle's third binding drops its failures, so one of the twelve is legitimately a refusal.
    for (system, bindings) in [("billing", 1), ("oracle-fixture", 3)] {
        let synthesis = synthesize(&example(system));
        let ir = example(system);
        assert_eq!(
            ir.bindings.len(),
            bindings,
            "the fixture declares {bindings}"
        );

        let mut expected: BTreeSet<String> = BTreeSet::new();
        for name in ir.bindings.keys() {
            for (_, aspect) in BindingAspect::ALL {
                expected.insert(format!("{name}/binding/{aspect}"));
            }
        }
        let covered: BTreeSet<String> = ids(&synthesis)
            .into_iter()
            .chain(refused(&synthesis))
            .filter(|id| id.contains("/binding/"))
            .collect();

        assert_eq!(
            covered, expected,
            "a binding clause that is neither in the suite nor in a refusal has disappeared"
        );
        assert_eq!(
            expected.len(),
            bindings * BindingAspect::ALL.len(),
            "four claims per binding"
        );
    }
}

#[test]
fn a_binding_flow_is_proved_through_the_event_the_invoked_command_publishes() {
    // §16, both halves. The flow is proved *through the resulting event* — "do not require the
    // runner to observe the internal `SendEmail` command" — so the negative half matters as much as
    // the positive one: a flow scenario that reached for the invocation would make command tracing
    // a requirement of every implementation, which is exactly what that section refuses.
    let synthesis = synthesize(&example("billing"));
    let id = "notify-on-invoice-created/binding/flow";

    assert_eq!(
        shape(&synthesis, id),
        vec!["execute", "outcome", "event", "eventually-event"],
        "create an invoice, observe `InvoiceCreated`, and wait for what `SendEmail` publishes"
    );
    let observed: Vec<String> = steps(&synthesis, id)
        .iter()
        .filter_map(|step| match step {
            ScenarioStep::EventuallyEvent { event, .. } => Some(event.to_string()),
            _ => None,
        })
        .collect();
    assert_eq!(
        observed,
        vec!["billing.email.EmailSent"],
        "the downstream consequence is the event the branch declares, not the command"
    );
    assert!(
        !shape(&synthesis, id).contains(&"invocation"),
        "a flow proved by observing the invocation would require tracing of every target"
    );
    assert!(
        !shape(&synthesis, id).contains(&"event-then-eventually"),
        "and the downstream event is bounded rather than immediate: nothing in the model says it \
         has happened by the time the upstream command returns"
    );
}

#[test]
fn a_binding_mapping_names_the_source_the_document_wrote_and_not_its_same_typed_sibling() {
    // What `examples/oracle-fixture/` exists for. `OrderPlaced` carries `contact` *and*
    // `alternate_contact`, both `oracle.order.Email`, so `recipient: event.alternate_contact` is a
    // document that still compiles and a system that mails the wrong person. The fixture is
    // asserted to hold that pair before the mapping is checked, because against an event with one
    // address this assertion would pass whether or not the synthesizer read the mapping at all.
    let ir = example("oracle-fixture");
    let placed = ir
        .events
        .get(&QualifiedName::new("oracle.order.OrderPlaced").expect("valid"))
        .expect("declared");
    let same_typed: Vec<&str> = placed
        .fields
        .iter()
        .filter(|field| field.type_ref.to_string() == "oracle.order.Email")
        .map(|field| field.name.as_str())
        .collect();
    assert_eq!(
        same_typed,
        vec!["contact", "alternate_contact"],
        "the fixture holds the pair a swapped mapping would hide behind"
    );

    let synthesis = synthesize(&ir);
    let id = "handoff-on-placed/binding/mapping";
    let invocation = steps(&synthesis, id)
        .iter()
        .find_map(|step| match step {
            ScenarioStep::ExpectInvocation {
                binding,
                command,
                input,
            } => Some((binding.to_string(), command.to_string(), input.clone())),
            _ => None,
        })
        .expect("the mapping scenario requires the invocation");

    assert_eq!(
        invocation.0, "handoff-on-placed",
        "the assertion names the binding, so a system with three bindings on one command can say \
         which one filled the input wrongly"
    );
    assert_eq!(invocation.1, "oracle.dispatch.Handoff");
    assert_eq!(
        invocation.2.get("recipient"),
        Some(&ScenarioValue::observed(
            "oracle.order.OrderPlaced".parse().expect("an event name"),
            "contact"
        )),
        "`recipient: event.contact` — the sibling is what a wrong document would have named"
    );
    assert_eq!(
        invocation.2.get("label"),
        Some(&ScenarioValue::literal(Node::Text("placed".to_owned()))),
        "and a literal mapping carries the text the binding wrote"
    );
    assert!(
        !format!("{:?}", invocation.2).contains("alternate_contact"),
        "the value asserted must be the one the document names: {:?}",
        invocation.2
    );
}

#[test]
fn an_at_least_once_binding_delivers_the_event_twice_and_requires_no_count() {
    // §17, and its worked bad test. `at_least_once` permits duplicates, so "exactly one `EmailSent`
    // exists" fails a target doing exactly what the specification allows — what the scenario may
    // require is that the consequence is still observable after the same event arrives again.
    let ir = example("billing");
    let binding = ir.bindings.values().next().expect("one binding");
    assert_eq!(
        binding.delivery,
        Delivery::AtLeastOnce,
        "the fixture declares the guarantee this scenario is about"
    );

    let synthesis = synthesize(&ir);
    let id = "notify-on-invoice-created/binding/delivery";
    assert_eq!(
        shape(&synthesis, id),
        vec![
            "execute",
            "outcome",
            "event",
            "redeliver",
            "eventually-event"
        ],
        "publish the event, deliver it a second time, and require the consequence anyway"
    );

    let redelivered = steps(&synthesis, id)
        .iter()
        .find_map(|step| match step {
            ScenarioStep::RedeliverEvent { event } => Some(event.to_string()),
            _ => None,
        })
        .expect("the scenario delivers the event again");
    assert_eq!(
        redelivered, "billing.invoice.InvoiceCreated",
        "the event delivered twice is the one the binding reacts to"
    );
    assert!(
        !shape(&synthesis, id).contains(&"no-event"),
        "nothing here may assert that a duplicate did *not* happen: {:?}",
        shape(&synthesis, id)
    );
}

#[test]
fn a_binding_that_escalates_requires_the_event_the_escalation_declares() {
    // §18, and what gate G2 made possible: `escalate` names the event it emits, so "the failure was
    // escalated" is an assertion rather than a hope. Both examples escalate, and both are checked —
    // billing because it is normative, the oracle because its escalation is one of three policies
    // over one command, which is where naming the wrong event would hide.
    for (system, id, escalation) in [
        (
            "billing",
            "notify-on-invoice-created/binding/on-failure",
            "billing.email.DeliveryEscalated",
        ),
        (
            "oracle-fixture",
            "handoff-on-held/binding/on-failure",
            "oracle.dispatch.HandoffEscalated",
        ),
    ] {
        let ir = example(system);
        let synthesis = synthesize(&ir);
        let declared = ir
            .bindings
            .values()
            .find(|binding| id.starts_with(binding.name.as_str()))
            .and_then(|binding| binding.escalation.clone())
            .expect("the binding escalates, and the model says into what");
        assert_eq!(
            declared.name().to_string(),
            escalation,
            "the fixture escalates into the event this scenario asserts"
        );

        let observed: Vec<String> = steps(&synthesis, id)
            .iter()
            .filter_map(|step| match step {
                ScenarioStep::EventuallyEvent { event, .. } => Some(event.to_string()),
                _ => None,
            })
            .collect();
        assert_eq!(
            observed,
            vec![escalation.to_owned()],
            "what is required is the escalation the model declares, and nothing else"
        );
        assert!(
            shape(&synthesis, id).contains(&"inject"),
            "and the failure is forced rather than waited for: {:?}",
            shape(&synthesis, id)
        );
    }
}

#[test]
fn a_binding_that_retries_forces_one_failure_and_still_requires_the_consequence() {
    // The other observable policy. A retry publishes nothing of its own — `ess-domain` says so, and
    // says why: "a retry is another invocation of the command", which `at_least_once` already
    // obliges the handler to survive. So it is observable exactly once the failure is forced *once*:
    // the injected outcome is what the adapter must produce next, so the second invocation reaches
    // the branch that publishes, and a target that gave up instead never produces the event.
    let synthesis = synthesize(&example("oracle-fixture"));
    let id = "handoff-on-placed/binding/on-failure";

    assert_eq!(
        shape(&synthesis, id),
        vec!["inject", "execute", "outcome", "event", "eventually-event"],
        "arm the refusal, place the order, and require the handoff to arrive anyway"
    );
    let observed: Vec<String> = steps(&synthesis, id)
        .iter()
        .filter_map(|step| match step {
            ScenarioStep::EventuallyEvent { event, .. } => Some(event.to_string()),
            _ => None,
        })
        .collect();
    assert_eq!(observed, vec!["oracle.dispatch.HandedOff".to_owned()]);
}

#[test]
fn the_failure_control_is_armed_after_the_arrangement_and_before_the_command_that_triggers_it() {
    // A one-shot control and a system with three bindings on one command: arming the refusal before
    // the arrangement would spend it on `handoff-on-placed`, which the arrangement's own
    // `PlaceOrder` sets off, and the scenario would then assert an escalation for a handoff that
    // never failed. The fixture reaches that state — `handoff-on-held` needs an order placed before
    // it can be held — which is why the order of these two steps is a check rather than a detail.
    let synthesis = synthesize(&example("oracle-fixture"));
    let id = "handoff-on-held/binding/on-failure";
    let shape = shape(&synthesis, id);

    let armed = shape
        .iter()
        .position(|step| *step == "inject")
        .expect("the scenario forces the failure");
    let arrangement = shape
        .iter()
        .position(|step| *step == "capture")
        .expect("the arrangement places an order first");
    let triggering = shape
        .iter()
        .rposition(|step| *step == "execute")
        .expect("the scenario runs the command that publishes the event");

    assert!(
        arrangement < armed && armed < triggering,
        "the control must be armed between the arrangement and the trigger: {shape:?}"
    );
}

#[test]
fn a_binding_that_drops_its_failures_refuses_that_check_and_names_the_reason() {
    // §18's rule, on the one input in either example that reaches it: "if the ESS does not yet
    // define an observable representation, scenario synthesis must refuse that check rather than
    // invent one". `drop` is not an omission — it is the decision that the work is lost and nobody
    // is told — so the refusal says so, and says which word to write instead.
    let ir = example("oracle-fixture");
    let dropping = ir
        .bindings
        .values()
        .find(|binding| binding.failure == Failure::Drop)
        .expect("the fixture declares a binding that drops");
    assert_eq!(dropping.name.as_str(), "handoff-on-shipped");

    let synthesis = synthesize(&ir);
    let refusal = synthesis
        .refused(code(10))
        .next()
        .unwrap_or_else(|| panic!("the failure check refuses: {:?}", refused(&synthesis)));

    assert_eq!(
        refusal
            .scenario
            .as_ref()
            .map(ToString::to_string)
            .unwrap_or_default(),
        "handoff-on-shipped/binding/on-failure",
        "the refusal names the one clause that has no witness"
    );
    assert!(
        matches!(
            refusal.cause,
            RefusalCause::BindingUnobservable {
                gap: BindingGap::PolicySilent,
                ..
            }
        ),
        "{refusal}"
    );
    assert!(
        refusal.hint().contains("escalate:"),
        "the hint names what to write if the failure has to be provable: {}",
        refusal.hint()
    );

    // Refusing one clause is not refusing the binding: the other three are still checks.
    let others: Vec<String> = ids(&synthesis)
        .into_iter()
        .filter(|id| id.starts_with("handoff-on-shipped/"))
        .collect();
    assert_eq!(
        others,
        vec![
            "handoff-on-shipped/binding/delivery".to_owned(),
            "handoff-on-shipped/binding/flow".to_owned(),
            "handoff-on-shipped/binding/mapping".to_owned(),
        ],
        "the flow, the mapping and the delivery of a dropping binding are all still provable"
    );
}

#[test]
fn a_binding_whose_branch_the_event_decides_refuses_the_flow_and_still_checks_the_mapping() {
    // Neither example reaches this, and it is the shape §11 is about. A binding fills the command's
    // input from the event, so when two branches are decided by that input, which one the
    // invocation reaches depends on a value only the upstream implementation knows — and requiring
    // either would be a claim about a value nobody has. The mapping is unaffected: what the
    // invocation *carries* is a reading of the document whatever branch it then takes.
    let synthesis = synthesize(&fixture(UNDECIDED_BRANCH));
    let gaps: BTreeMap<String, String> = synthesis
        .refusals
        .iter()
        .filter_map(|refusal| match &refusal.cause {
            RefusalCause::BindingUnobservable { gap, .. } => Some((
                refusal
                    .scenario
                    .as_ref()
                    .map(ToString::to_string)
                    .unwrap_or_default(),
                format!("{gap:?}"),
            )),
            _ => None,
        })
        .collect();

    assert_eq!(
        gaps.keys().cloned().collect::<Vec<_>>(),
        vec![
            "notify-on-placed/binding/delivery".to_owned(),
            "notify-on-placed/binding/flow".to_owned(),
            "notify-on-placed/binding/on-failure".to_owned(),
        ],
        "the three clauses that need to know what the invocation does: {gaps:?}"
    );
    assert!(
        gaps["notify-on-placed/binding/flow"].starts_with("BranchUndecided"),
        "the flow cannot say which branch it reaches: {gaps:?}"
    );
    assert!(
        gaps["notify-on-placed/binding/on-failure"].starts_with("NoForcibleFailure"),
        "and nothing about this command can be made to fail, which is the earlier obstacle: {gaps:?}"
    );
    assert!(
        ids(&synthesis)
            .iter()
            .any(|id| id == "notify-on-placed/binding/mapping"),
        "the mapping is still a check: {:?}",
        ids(&synthesis)
    );
}

// ---- §20: what must still hold once a command has changed something ----------------------------

#[test]
fn an_invariant_is_asserted_against_every_view_that_publishes_what_it_reads() {
    // §20 at the level it asks for: after a successful state-changing command, against observable
    // view state. `InvoiceById` and `OutstandingInvoices` both publish `total`, so both can answer
    // `total.amount >= 0` — and each is asserted in the block its own consistency decides, which is
    // §14's rule reaching a second family rather than being decided again here.
    let synthesis = synthesize(&example("billing"));
    let id = "billing.invoice.Invoice/invariant/after/billing.invoice.IssueInvoice/issued";

    assert_eq!(
        shape(&synthesis, id),
        vec![
            "execute",
            "outcome",
            "capture",
            "execute",
            "outcome",
            "eventually-view",
            "query",
            "view"
        ],
        "create an invoice, issue it, then read what must still hold of it"
    );
    for (view, block) in [
        ("billing.invoice.InvoiceById", "eventually"),
        ("billing.invoice.OutstandingInvoices", "expect"),
    ] {
        let (found, expectation) = expectation(&synthesis, id, view);
        assert_eq!(
            found, block,
            "`{view}` is asserted where its consistency says"
        );
        let ViewExpectation::Satisfies { predicate } = expectation else {
            panic!(
                "an invariant is a range, which no set of field values expresses: {expectation:?}"
            )
        };
        assert_eq!(
            predicate.to_string(),
            "total.amount >= 0",
            "the predicate carried is the one the specification wrote"
        );
    }
}

#[test]
fn a_view_that_does_not_hold_the_instance_yet_is_not_asked_about_its_invariants() {
    // The negative half, and the reason `Satisfies` demands a row: every row of an empty view
    // satisfies everything. `OutstandingInvoices` filters on `state == Issued` and a created
    // invoice is in `Draft`, so asserting the invariant there would be a check nothing can fail.
    let synthesis = synthesize(&example("billing"));
    let id = "billing.invoice.Invoice/invariant/after/billing.invoice.CreateInvoice/accepted";
    let asserted: Vec<String> = steps(&synthesis, id)
        .iter()
        .filter_map(|step| match step {
            ScenarioStep::ExpectView { view, .. } | ScenarioStep::EventuallyView { view, .. } => {
                Some(view.to_string())
            }
            _ => None,
        })
        .collect();

    assert_eq!(
        asserted,
        vec!["billing.invoice.InvoiceById".to_owned()],
        "only the view that holds a `Draft` invoice is asked what must be true of one"
    );
}

#[test]
fn an_invariant_over_a_field_no_view_publishes_refuses_rather_than_being_dropped() {
    // The specification fact this family runs into, and the one `examples/oracle-fixture/` reaches:
    // an entity's invariant reads the entity's *fields*, and a view publishes only what it
    // *declares*. `weight_grams >= 0` is therefore unobservable by construction — no runner and no
    // witness generator closes it — so the suite says which path and which entity rather than
    // holding one check fewer than the specification requires.
    let ir = example("oracle-fixture");
    let order = ir
        .entities
        .get(&QualifiedName::new("oracle.order.Order").expect("valid"))
        .expect("declared");
    assert_eq!(
        order
            .invariants
            .iter()
            .map(|invariant| invariant.statement.clone())
            .collect::<Vec<_>>(),
        vec!["weight_grams >= 0".to_owned()],
        "the fixture declares the invariant this refusal is about"
    );
    assert!(
        ir.views
            .values()
            .filter(|view| view.source.name() == &order.name)
            .all(|view| view.field("weight_grams").is_none()),
        "and no view of it publishes what that invariant reads, which is what makes it unobservable"
    );

    let synthesis = synthesize(&ir);
    let refusal = synthesis
        .refused(code(11))
        .next()
        .unwrap_or_else(|| panic!("the invariant refuses: {:?}", refused(&synthesis)));
    let rendered = refusal.to_string();

    assert!(
        rendered.contains("weight_grams >= 0") && rendered.contains("`weight_grams`"),
        "the refusal names the invariant and the path no view publishes: {rendered}"
    );
    assert!(
        rendered.contains("oracle.order.Order/invariant/after/"),
        "and the scenario that is missing: {rendered}"
    );
    assert!(
        !ids(&synthesis).iter().any(|id| id.contains("/invariant/")),
        "nothing is asserted about an invariant nothing can read: {:?}",
        ids(&synthesis)
    );
    assert_eq!(
        synthesis.refused(code(11)).count(),
        5,
        "one per state-changing branch, because each is a different check that is missing"
    );
}

#[test]
fn a_value_objects_own_invariants_are_refused_rather_than_silently_skipped() {
    // `billing.invoice.Money` says `amount >= 0` of every `Money` in the system, and this build
    // evaluates entity invariants only. A reader of the suite cannot tell that from a specification
    // with nothing to check, so §36's rule applies to a gap in this crate exactly as it does to a
    // gap in the model.
    let synthesis = synthesize(&example("billing"));
    let named: Vec<String> = synthesis
        .refused(code(6))
        .map(|refusal| refusal.subject.to_string())
        .collect();

    assert_eq!(
        named,
        vec!["type billing.invoice.Money".to_owned()],
        "the one value object in the example that constrains its own values"
    );
}

// ---- §23 and §37: provenance and determinism -----------------------------------------------------

#[test]
fn synthesising_the_same_specification_twice_produces_byte_identical_output() {
    // Two independent compilations and two independent syntheses. Nothing is shared between them,
    // so an unordered map, a clock or an address-dependent iteration order anywhere in the path
    // shows up here as a diff rather than as a rumour — including in the refusals, which a report
    // prints and a reviewer diffs.
    // Both examples, because the oracle is where the families that walk a second index live: the
    // bindings, and the views an invariant is read off.
    for system in ["billing", "oracle-fixture"] {
        let first = synthesize(&example(system));
        let second = synthesize(&example(system));

        assert_eq!(
            first.suite.to_canonical_json().as_bytes(),
            second.suite.to_canonical_json().as_bytes(),
            "`{system}` must produce the same suite, byte for byte"
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
}

#[test]
fn each_example_synthesises_the_families_its_specification_declares() {
    // The count, by family, for both examples — the one assertion that fails when a family stops
    // being produced at all, which no test of one scenario's shape can see. Read as a table because
    // that is what it is: what each specification declares, and what it therefore obliges.
    //
    // The refusal count no longer includes the illegal-move family. Both examples declare a
    // `wrong_state:` branch on every command that moves an entity, so all sixteen `ESS-SYNTH-012`
    // refusals became assertions of a declared error. What is left is what the model still cannot
    // say: one for billing — the value object whose own invariants are not read off the fields that
    // hold one — and six for the oracle fixture, its five unobservable invariants and its one
    // `on_failure: drop` binding.
    for (system, expected, refusals) in [
        (
            "billing",
            [
                ("/outcome/", 8),
                ("/transition/", 3),
                ("/state/", 8),
                ("/invariant/", 4),
                ("/binding/", 4),
            ],
            1,
        ),
        (
            "oracle-fixture",
            [
                ("/outcome/", 9),
                ("/transition/", 3),
                ("/state/", 8),
                ("/invariant/", 0),
                ("/binding/", 11),
            ],
            6,
        ),
    ] {
        let synthesis = synthesize(&example(system));
        let counted: Vec<(&str, usize)> = expected
            .iter()
            .map(|(family, _)| {
                (
                    *family,
                    ids(&synthesis)
                        .iter()
                        .filter(|id| id.contains(family))
                        .count(),
                )
            })
            .collect();

        assert_eq!(
            counted,
            expected.to_vec(),
            "`{system}` synthesises a different suite than its specification declares"
        );
        assert_eq!(
            synthesis.suite.len(),
            expected.iter().map(|(_, count)| count).sum::<usize>(),
            "and every scenario in it belongs to one of the five families"
        );
        assert_eq!(
            synthesis.refusals.len(),
            refusals,
            "`{system}` refuses: {:?}",
            synthesis
                .refusals
                .iter()
                .map(|refusal| format!("{}", refusal.code()))
                .collect::<Vec<_>>()
        );
    }
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

/// A binding whose invoked command has two branches an input decides between.
///
/// The values come from the event, so no scenario can say which branch the invocation reaches — and
/// with no `external:` branch, nothing about the command can be forced to fail either. Neither
/// example carries this: both invoke a command with one unconditional branch and one external one.
///
/// `skipped` is unconditional because `ess-domain` refuses a command whose every branch is
/// conditional, which is what makes two *reachable* branches the smallest shape this can have.
const UNDECIDED_BRANCH: &str = r"
format: ess/1
system: flow
version: v1
domain: flow.orders

types:
  - name: flow.orders.Recipient
    kind: newtype
    of: String

events:
  - name: flow.orders.OrderPlaced
    fields:
      - name: contact
        type: flow.orders.Recipient

  - name: flow.orders.Notified
    fields:
      - name: recipient
        type: flow.orders.Recipient

  - name: flow.orders.Skipped
    fields:
      - name: recipient
        type: flow.orders.Recipient

commands:
  - name: flow.orders.PlaceOrder
    input:
      - name: contact
        type: flow.orders.Recipient
    outcomes:
      - name: placed
        emits:
          - flow.orders.OrderPlaced

  - name: flow.orders.Notify
    input:
      - name: recipient
        type: flow.orders.Recipient
    outcomes:
      - name: sent
        when: recipient == urgent
        emits:
          - flow.orders.Notified
      - name: skipped
        emits:
          - flow.orders.Skipped

bindings:
  - id: notify-on-placed
    when:
      event: flow.orders.OrderPlaced
    invoke:
      command: flow.orders.Notify
    mapping:
      recipient: event.contact
    delivery: at_least_once
    on_failure: retry
";

/// A specification whose only lifecycle command says nothing about being asked at the wrong time.
///
/// `ship` runs from `Placed` alone, so `Shipped` is a state `ShipOrder` answers in and the document
/// does not say how — which is every specification written before `wrong_state:` existed, and the
/// shape `ESS-SYNTH-012` has to keep reporting.
const NO_DECLARED_REFUSAL: &str = r"
format: ess/1
system: quiet
version: v1
domain: quiet.orders

types:
  - name: quiet.orders.OrderId
    kind: newtype
    of: Uuid

entities:
  - name: quiet.orders.Order
    identity:
      name: order_id
      type: quiet.orders.OrderId
    lifecycle:
      initial: Placed
      states: [Placed, Shipped]
      terminal: [Shipped]
      transitions:
        - name: ship
          from: [Placed]
          to: Shipped

events:
  - name: quiet.orders.OrderPlaced
    fields:
      - name: order_id
        type: quiet.orders.OrderId

  - name: quiet.orders.OrderShipped
    fields:
      - name: order_id
        type: quiet.orders.OrderId

commands:
  - name: quiet.orders.PlaceOrder
    outcomes:
      - name: accepted
        creates: quiet.orders.Order
        instance: order_id
        emits:
          - quiet.orders.OrderPlaced

  - name: quiet.orders.ShipOrder
    input:
      - name: order_id
        type: quiet.orders.OrderId
    outcomes:
      - name: shipped
        moves: quiet.orders.Order.ship
        instance: order_id
        emits:
          - quiet.orders.OrderShipped
";
