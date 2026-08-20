//! What a conformance suite is, proven against the specification it will be generated from.
//!
//! Design §21 defines the scenario IR and §22 says why it exists before any runner does. This file
//! holds the three claims that are worth nothing unasserted:
//!
//! | claim | test |
//! |---|---|
//! | the ten-step vocabulary expresses §10's worked example | `the_step_vocabulary_expresses_the_worked_example_from_section_ten` |
//! | the same suite serialises to the same bytes | `serialising_a_suite_twice_produces_byte_identical_json` |
//! | a suite written in one process resolves in another | `a_suite_serialised_in_one_process_resolves_in_another` |
//!
//! Nothing here executes a scenario. That is a later slice, deliberately: a runner written beside
//! the definition becomes the definition.
//!
//! The scenarios below are **built by hand**. Synthesising them from the model is W4.2, and a test
//! that called a synthesizer would be a test of the synthesizer rather than of the shape it has to
//! produce.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use aep_domain::facts::Number;
use aep_domain::node::Node;
use ess_compiler::ir::{CommandHandle, EssIr, ResolvedCommand};
use ess_compiler::resolve::compile;
use ess_compiler::source::SourceMap;
use ess_conformance::scenario::{
    BindingAspect, BindingRef, CommandRef, ConformanceScenario, ConformanceSuite, DeclaredTypeRef,
    ErrorRef, EssSemanticRef, EventRef, Holds, LeafShape, OutcomeRef, PayloadShape, ScenarioId,
    ScenarioPurpose, ScenarioStep, ScenarioValue, SuiteFormat, SuiteProvenance, TransitionRef,
    ViewExpectation,
};
use ess_domain::binding::BindingName;
use ess_domain::command::OutcomeName;
use ess_domain::name::QualifiedName;
use ess_domain::spec::{RawSpecFile, Specification};
use ess_domain::system::Source;
use ess_domain::types::Primitive;

// ---- the billing example, compiled -----------------------------------------------------------

/// The billing example's directory.
fn example() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/billing")
        .canonicalize()
        .expect("the billing example exists")
}

/// Every `.yaml` file in the example, relative to it, in a stable order.
fn files() -> Vec<String> {
    let base = example();
    let mut found = Vec::new();
    let mut pending = vec![base.clone()];
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(&directory).expect("the example is readable") {
            let path = entry.expect("an entry").path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|it| it == "yaml") {
                found.push(
                    path.strip_prefix(&base)
                        .expect("inside the example")
                        .display()
                        .to_string(),
                );
            }
        }
    }
    assert!(!found.is_empty(), "the billing example holds no files");
    found.sort();
    found
}

/// The billing example, compiled — from the files it lives in, never from a copy inlined here.
fn billing() -> EssIr {
    let mut sources = SourceMap::new();
    let mut parsed = Vec::new();
    for label in files() {
        let text = std::fs::read_to_string(example().join(&label))
            .unwrap_or_else(|error| panic!("{label} is readable: {error}"));
        let raw = RawSpecFile::parse(&text)
            .unwrap_or_else(|error| panic!("{label} is well formed: {error}"));
        sources.insert(label.clone(), text);
        parsed.push((Source::new(label), raw));
    }
    let specification = Specification::assemble(parsed)
        .unwrap_or_else(|errors| panic!("the billing specification validates:\n{errors}"));
    compile(&specification, &sources)
        .unwrap_or_else(|diagnostics| panic!("the billing specification resolves:\n{diagnostics}"))
}

// ---- §10's worked example, by hand ------------------------------------------------------------

/// A handle for a declared name, taken from the domain that owns it.
///
/// Through the domain's roster rather than through `EssIr::commands`, because that is where a
/// generator gets one: a handle has no public constructor, so the only way to hold one is to have
/// been given it by the IR.
fn command_handle<'ir>(ir: &'ir EssIr, name: &str) -> &'ir CommandHandle {
    let owner = QualifiedName::new("billing.invoice").expect("valid");
    ir.domains[&owner]
        .commands
        .iter()
        .find(|handle| handle.name().to_string() == name)
        .unwrap_or_else(|| panic!("`{name}` is a command the invoice domain declares"))
}

/// A declared type of the invoice domain, as a name that outlives the IR.
fn declared_type(ir: &EssIr, name: &str) -> DeclaredTypeRef {
    let owner = QualifiedName::new("billing.invoice").expect("valid");
    ir.domains[&owner]
        .types
        .iter()
        .find(|handle| handle.name().to_string() == name)
        .map_or_else(
            || panic!("`{name}` is a type the invoice domain declares"),
            DeclaredTypeRef::from,
        )
}

/// `{amount: <value>, currency: "EUR"}` — a `billing.invoice.Money`.
fn money(amount: i64) -> Node {
    Node::Map(BTreeMap::from([
        ("amount".to_owned(), Node::Number(Number::from(amount))),
        ("currency".to_owned(), Node::Text("EUR".to_owned())),
    ]))
}

/// The command input §10 uses, with the amount that decides the branch.
fn create_invoice_input(amount: i64) -> BTreeMap<String, ScenarioValue> {
    BTreeMap::from([
        (
            "customer_email".to_owned(),
            ScenarioValue::literal(Node::Text("ada@example.com".to_owned())),
        ),
        ("amount".to_owned(), ScenarioValue::literal(money(amount))),
    ])
}

/// The name of one branch of `CreateInvoice`.
fn outcome_ref(command: &ResolvedCommand, handle: &CommandHandle, branch: &str) -> OutcomeRef {
    let declared = command
        .outcomes
        .iter()
        .find(|outcome| outcome.name.as_str() == branch)
        .unwrap_or_else(|| panic!("`{branch}` is a declared outcome"));
    OutcomeRef::new(CommandRef::from(handle), declared.name.clone())
}

/// §10's worked example: `CreateInvoice`, both branches, with the negative assertion.
///
/// Every reference is minted from a handle the IR handed over, which is the one-way door §21
/// describes — and nothing built here holds the handle afterwards.
/// What `billing.invoice.InvoiceCreated` declares it carries, flattened as synthesis flattens it.
///
/// Written out by hand here for the same reason the rest of this example is: it is what §13's claim
/// looks like as data — a newtype is transparent, so `invoice_id` is a `Uuid` and not an
/// `InvoiceId`, and a struct contributes its fields under `amount.amount` and `amount.currency`.
fn invoice_created_shape() -> PayloadShape {
    let mut shape = PayloadShape::new();
    for (path, kind) in [
        ("invoice_id", Primitive::Uuid),
        ("customer_email", Primitive::String),
        ("amount.amount", Primitive::Decimal),
        ("amount.currency", Primitive::String),
    ] {
        shape.insert(path, LeafShape::required(Holds::Primitive { kind }));
    }
    shape
}

fn worked_example(ir: &EssIr) -> ConformanceSuite {
    let mut suite = ConformanceSuite::new(SuiteProvenance::of(ir));
    let handle = command_handle(ir, "billing.invoice.CreateInvoice");
    let command = ir.command(handle);
    let accepted = outcome_ref(command, handle, "accepted");
    let rejected = outcome_ref(command, handle, "rejected");

    let created = command
        .outcomes
        .iter()
        .find(|outcome| outcome.name.as_str() == "accepted")
        .and_then(|outcome| outcome.emits.first())
        .map(EventRef::from)
        .expect("`accepted` emits an event");
    let invalid_amount = command
        .outcomes
        .iter()
        .find(|outcome| outcome.name.as_str() == "rejected")
        .and_then(|outcome| outcome.error.as_ref())
        .map(ErrorRef::from)
        .expect("`rejected` reports a declared error");
    let invoice = command
        .outcomes
        .iter()
        .find(|outcome| outcome.name.as_str() == "accepted")
        .and_then(|outcome| outcome.subject.as_ref())
        .map(|subject| ess_conformance::scenario::EntityRef::from(&subject.entity))
        .expect("`accepted` brings an invoice into existence");
    let money_type = declared_type(ir, "billing.invoice.Money");
    let email_type = declared_type(ir, "billing.invoice.Email");

    suite
        .insert(
            ScenarioId::Outcome {
                outcome: accepted.clone(),
            },
            ConformanceScenario::new(
                ScenarioPurpose::new("a positive amount is accepted, and says so by emitting")
                    .expect("one line"),
                [
                    ScenarioStep::ExecuteCommand {
                        command: CommandRef::from(handle),
                        actor: None,
                        input: create_invoice_input(120),
                    },
                    ScenarioStep::ExpectOutcome {
                        outcome: accepted.clone(),
                    },
                    ScenarioStep::ExpectEvent {
                        event: created.clone(),
                        payload: BTreeMap::from([("amount".to_owned(), money(120))]),
                        shape: invoice_created_shape(),
                    },
                ],
                [
                    EssSemanticRef::from(CommandRef::from(handle)),
                    EssSemanticRef::from(accepted),
                    EssSemanticRef::from(created.clone()),
                    EssSemanticRef::from(invoice),
                    EssSemanticRef::from(money_type.clone()),
                    EssSemanticRef::from(email_type.clone()),
                ],
            ),
        )
        .expect("the first scenario");

    suite
        .insert(
            ScenarioId::Outcome {
                outcome: rejected.clone(),
            },
            ConformanceScenario::new(
                ScenarioPurpose::new("a zero amount is refused, and nothing is created")
                    .expect("one line"),
                [
                    ScenarioStep::ExecuteCommand {
                        command: CommandRef::from(handle),
                        actor: None,
                        input: create_invoice_input(0),
                    },
                    ScenarioStep::ExpectOutcome {
                        outcome: rejected.clone(),
                    },
                    ScenarioStep::ExpectError {
                        error: invalid_amount.clone(),
                        fields: BTreeMap::from([("submitted".to_owned(), money(0))]),
                    },
                    // The assertion §10 makes first class. Without it the scenario passes against an
                    // implementation that refuses the command and emits the event anyway.
                    ScenarioStep::ExpectNoEvent {
                        event: created.clone(),
                    },
                ],
                [
                    EssSemanticRef::from(CommandRef::from(handle)),
                    EssSemanticRef::from(rejected),
                    EssSemanticRef::from(invalid_amount),
                    EssSemanticRef::from(created),
                    EssSemanticRef::from(money_type),
                    EssSemanticRef::from(email_type),
                ],
            ),
        )
        .expect("the second scenario");

    suite
}

// ---- the claims --------------------------------------------------------------------------------

#[test]
fn the_step_vocabulary_expresses_the_worked_example_from_section_ten() {
    let ir = billing();
    let suite = worked_example(&ir);

    assert_eq!(suite.len(), 2, "one scenario per declared outcome");
    let rejected = suite
        .scenario(&ScenarioId::parse("billing.invoice.CreateInvoice/outcome/rejected").expect("id"))
        .expect("the refusal branch is in the suite");

    // §10 spells the refusal case as four requirements. All four are steps, not commentary.
    let steps: Vec<&str> = rejected
        .steps
        .iter()
        .map(|step| match step {
            ScenarioStep::ExecuteCommand { .. } => "execute",
            ScenarioStep::ExpectOutcome { .. } => "outcome",
            ScenarioStep::ExpectError { .. } => "error",
            ScenarioStep::ExpectNoEvent { .. } => "no-event",
            _ => "other",
        })
        .collect();
    assert_eq!(
        steps,
        vec!["execute", "outcome", "error", "no-event"],
        "`→ rejected`, `→ InvalidAmount`, `→ InvoiceCreated must not occur`"
    );
}

#[test]
fn the_dependency_set_names_a_type_no_derived_from_would_have_mentioned() {
    // Design §37's worked argument. A `derived_from` lists what *caused* the scenario — the command,
    // the outcome, the error. This scenario's result also depends on `Money`, because the input it
    // sends and the payload it asserts are both of that type: if `Money` gains a field, the stored
    // result is stale and a `derived_from` naming only the outcome does not say so.
    let ir = billing();
    let suite = worked_example(&ir);
    let rejected = suite
        .scenario(&ScenarioId::parse("billing.invoice.CreateInvoice/outcome/rejected").expect("id"))
        .expect("the refusal branch");

    let caused_it = [
        "command billing.invoice.CreateInvoice",
        "outcome billing.invoice.CreateInvoice/rejected",
        "error billing.invoice.InvalidAmount",
    ];
    let depends_on: Vec<String> = rejected.source.iter().map(ToString::to_string).collect();
    for cause in caused_it {
        assert!(
            depends_on.iter().any(|held| held == cause),
            "the dependency set is a superset of what caused the scenario: {depends_on:?}"
        );
    }
    assert!(
        depends_on
            .iter()
            .any(|held| held == "type billing.invoice.Money"),
        "and it names the type the input and the payload are made of: {depends_on:?}"
    );
    assert!(
        rejected.source.len() > caused_it.len(),
        "a dependency set that is exactly the cause list is a `derived_from` wearing another name"
    );
}

#[test]
fn serialising_a_suite_twice_produces_byte_identical_json() {
    // Two independent compilations and two independent suites. Nothing is shared between them, so an
    // unordered map, a clock or an address-dependent iteration order anywhere in the path would show
    // up here as a diff rather than as a rumour.
    let first = worked_example(&billing()).to_canonical_json();
    let second = worked_example(&billing()).to_canonical_json();

    assert_eq!(
        first.as_bytes(),
        second.as_bytes(),
        "the same model must produce the same suite, byte for byte"
    );
    assert!(
        first.ends_with('\n'),
        "a file without a trailing newline shows up modified"
    );
    assert!(!first.ends_with("\n\n"), "one newline, not two");
    assert!(
        first.len() > 500,
        "the suite is not empty: {} bytes",
        first.len()
    );
}

#[test]
fn a_suite_serialised_in_one_process_resolves_in_another() {
    // Design §49's step-1 acceptance, and the reason every reference in a suite is a name: a handle
    // has no public constructor, so nothing that survives this round trip can be one.
    let suite = worked_example(&billing());
    let written = suite.to_canonical_json();

    let read = ConformanceSuite::from_json(&written).expect("a written suite parses");

    assert_eq!(read, suite, "what came back is what went in");
    assert_eq!(
        read.to_canonical_json().as_bytes(),
        written.as_bytes(),
        "and writing it again produces the same file"
    );
}

#[test]
fn a_suite_parses_from_text_alone_without_an_ir() {
    // No `EssIr` in this test at all — which is the whole claim of §21: a committed suite is read
    // back on a later checkout, by a later build, possibly in another language, and none of those
    // has the IR that produced it.
    let written = r#"{
  "provenance": {
    "suite_version": "ess-conformance/1",
    "system": "billing",
    "specification_version": "v3",
    "spec_digest": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
    "compiler_version": "0.1.0",
    "generator_version": "0.1.0",
    "synthesizer_version": "0.1.0"
  },
  "scenarios": {
    "billing.invoice.CreateInvoice/outcome/rejected": {
      "purpose": "a zero amount is refused",
      "steps": [
        {
          "step": "expect_outcome",
          "outcome": {
            "command": "billing.invoice.CreateInvoice",
            "outcome": "rejected"
          }
        },
        {
          "step": "expect_no_event",
          "event": "billing.invoice.InvoiceCreated"
        }
      ],
      "source": [
        { "kind": "command", "name": "billing.invoice.CreateInvoice" },
        { "kind": "type", "name": "billing.invoice.Money" }
      ]
    }
  }
}
"#;

    let suite = ConformanceSuite::from_json(written).expect("text alone is enough");

    assert_eq!(suite.provenance.suite_version, SuiteFormat::CURRENT);
    assert_eq!(suite.len(), 1);
    let id = ScenarioId::parse("billing.invoice.CreateInvoice/outcome/rejected").expect("id");
    assert_eq!(
        suite.scenario(&id).expect("the scenario").steps.len(),
        2,
        "the steps came back too, not just the key"
    );
    assert_eq!(
        suite.dependencies().len(),
        2,
        "and so did what the scenario depends on"
    );
}

#[test]
fn the_steps_a_binding_and_an_invariant_need_survive_being_read_back_from_text() {
    // The three steps §21 does not list, and the expectation that carries a predicate rather than a
    // value. Each is only worth having if a runner that never saw the `EssIr` can read it — a
    // `Predicate` in particular travels as the expression an author wrote and is parsed on the way
    // in, so a suite naming a condition nothing can parse is refused here rather than at the step
    // that would have evaluated it.
    let written = r#"{
  "provenance": {
    "suite_version": "ess-conformance/1",
    "system": "billing",
    "specification_version": "v3",
    "spec_digest": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
    "compiler_version": "0.1.0",
    "generator_version": "0.1.0",
    "synthesizer_version": "0.1.0"
  },
  "scenarios": {
    "billing.invoice.Invoice/invariant/after/billing.invoice.CreateInvoice/accepted": {
      "purpose": "an invoice still satisfies what it declares",
      "steps": [
        {
          "step": "expect_view",
          "view": "billing.invoice.InvoiceById",
          "expectation": { "expect": "satisfies", "predicate": "total.amount >= 0" }
        }
      ],
      "source": [{ "kind": "entity", "name": "billing.invoice.Invoice" }]
    },
    "notify-on-invoice-created/binding/delivery": {
      "purpose": "the same event twice",
      "steps": [
        { "step": "redeliver_event", "event": "billing.invoice.InvoiceCreated" },
        { "step": "eventually_event", "event": "billing.email.EmailSent" }
      ],
      "source": [{ "kind": "binding", "name": "notify-on-invoice-created" }]
    },
    "notify-on-invoice-created/binding/mapping": {
      "purpose": "the recipient is the address the event carried",
      "steps": [
        {
          "step": "expect_invocation",
          "binding": "notify-on-invoice-created",
          "command": "billing.email.SendEmail",
          "input": {
            "recipient": {
              "kind": "observed",
              "event": "billing.invoice.InvoiceCreated",
              "field": "customer_email"
            },
            "template": { "kind": "literal", "value": "invoice-created" }
          }
        }
      ],
      "source": [{ "kind": "binding", "name": "notify-on-invoice-created" }]
    }
  }
}
"#;

    let suite = ConformanceSuite::from_json(written).expect("text alone is enough");

    assert_eq!(suite.len(), 3);
    let invariant = ScenarioId::parse(
        "billing.invoice.Invoice/invariant/after/billing.invoice.CreateInvoice/accepted",
    )
    .expect("an id");
    let ScenarioStep::ExpectView { expectation, .. } = &suite
        .scenario(&invariant)
        .expect("the invariant scenario")
        .steps[0]
    else {
        panic!("the step came back as something else")
    };
    let ViewExpectation::Satisfies { predicate } = expectation else {
        panic!("a predicate is not a set of field values: {expectation:?}")
    };
    assert_eq!(
        predicate.to_string(),
        "total.amount >= 0",
        "the condition reads the same after a round trip as the specification wrote it"
    );

    let mapping = ScenarioId::parse("notify-on-invoice-created/binding/mapping").expect("an id");
    let ScenarioStep::ExpectInvocation { input, .. } =
        &suite.scenario(&mapping).expect("the mapping").steps[0]
    else {
        panic!("the step came back as something else")
    };
    assert_eq!(
        input.get("recipient"),
        Some(&ScenarioValue::observed(
            "billing.invoice.InvoiceCreated"
                .parse()
                .expect("an event name"),
            "customer_email"
        )),
        "a value nobody could have written down came back as the reference it is"
    );

    assert!(
        ConformanceSuite::from_json(&written.replace("total.amount >= 0", "total amount >= 0"))
            .is_err(),
        "and a condition nothing can parse is refused while the suite is read"
    );
}

#[test]
fn a_suite_naming_something_that_is_not_an_ess_name_is_refused_while_it_is_read() {
    let malformed = r#"{
  "provenance": {
    "suite_version": "ess-conformance/1",
    "system": "billing",
    "specification_version": "v3",
    "spec_digest": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
    "compiler_version": "0.1.0",
    "generator_version": "0.1.0",
    "synthesizer_version": "0.1.0"
  },
  "scenarios": {
    "billing.invoice.CreateInvoice/outcome/rejected": {
      "purpose": "a zero amount is refused",
      "steps": [{ "step": "expect_no_event", "event": "not a name" }],
      "source": []
    }
  }
}
"#;

    let error = ConformanceSuite::from_json(malformed).expect_err("refused on the way in");

    assert!(
        error.to_string().contains("not a name"),
        "the refusal quotes what it refused: {error}"
    );
}

#[test]
fn inserting_one_outcome_re_keys_nothing_around_it() {
    // Why a scenario id is a name and never a counter (§37). Under a counter, adding a scenario that
    // sorts before the others renumbers every one after it: the committed suite re-keys wholesale,
    // the fault matrix's references rot, and no diff can line yesterday's result up with today's
    // scenario.
    let ir = billing();
    let before = worked_example(&ir);
    let mut after = worked_example(&ir);
    let inserted =
        ScenarioId::parse("billing.invoice.CancelInvoice/outcome/cancelled").expect("id");
    after
        .insert(
            inserted.clone(),
            ConformanceScenario::new(
                ScenarioPurpose::new("an invoice can be cancelled").expect("one line"),
                [ScenarioStep::ExpectOutcome {
                    outcome: OutcomeRef::new(
                        CommandRef::new(
                            QualifiedName::new("billing.invoice.CancelInvoice").expect("valid"),
                        ),
                        OutcomeName::new("cancelled").expect("valid"),
                    ),
                }],
                [],
            ),
        )
        .expect("a new scenario");

    assert!(
        after
            .scenarios
            .keys()
            .next()
            .is_some_and(|first| first == &inserted),
        "the fixture inserts a scenario that sorts *before* both existing ones, which is the case a \
         counter cannot survive"
    );
    for (id, scenario) in &before.scenarios {
        assert_eq!(
            after.scenario(id),
            Some(scenario),
            "`{id}` must mean the same thing after an unrelated scenario appeared"
        );
    }
}

#[test]
fn the_scenario_ids_appear_in_the_file_in_the_order_a_sorted_key_list_would_be() {
    // So a tool that re-sorts the keys of the JSON object reproduces the committed file rather than
    // producing a diff a drift check would fail on.
    let ir = billing();
    let suite = worked_example(&ir);
    let json = suite.to_canonical_json();

    let mut names: Vec<String> = suite.scenarios.keys().map(ToString::to_string).collect();
    let offsets: Vec<usize> = names
        .iter()
        .map(|name| json.find(name).expect("every id appears in the file"))
        .collect();
    let mut sorted = names.clone();
    sorted.sort();

    assert_eq!(names, sorted, "the keys come out sorted");
    assert!(
        offsets.windows(2).all(|pair| pair[0] < pair[1]),
        "and they appear in the file in that order"
    );
    names.dedup();
    assert_eq!(names.len(), suite.len(), "no id is written twice");
}

#[test]
fn the_suite_records_the_same_model_digest_the_projections_do() {
    // The reuse §23 asks for, made load-bearing. `SuiteProvenance` derives every shared fact from
    // `ess_gen::Provenance`; if anyone reimplements the digest here, these stop agreeing and the
    // protocol engine can no longer tell that a conformance record and a generated artifact describe
    // one model.
    let ir = billing();
    let projection = ess_gen::Provenance::of(&ir);
    let suite = SuiteProvenance::of(&ir);

    assert_eq!(suite.spec_digest.as_str(), projection.source_digest);
    assert_eq!(suite.system, projection.system);
    assert_eq!(
        suite.specification_version,
        projection.specification_version
    );
    assert_eq!(suite.compiler_version, projection.compiler_version);
    assert_eq!(suite.generator_version, projection.generator_version);
    assert_eq!(
        suite.synthesizer_version,
        SuiteProvenance::SYNTHESIZER_VERSION,
        "and the synthesizer is named separately, because it is a different thing (D4)"
    );
}

#[test]
fn every_scenario_id_the_billing_model_can_produce_reads_back() {
    // The four id shapes against the real model rather than against a literal: a transition and its
    // driver, a refusal, and a binding — the constructs a later slice keys its scenarios on.
    let ir = billing();
    let invoice = QualifiedName::new("billing.invoice.Invoice").expect("valid");
    let entity_handle = ir.domains[&QualifiedName::new("billing.invoice").expect("valid")]
        .entities
        .iter()
        .find(|handle| handle.name() == &invoice)
        .expect("the invoice entity");
    let entity = ess_conformance::scenario::EntityRef::from(entity_handle);
    let transition = ir
        .entity(entity_handle)
        .lifecycle
        .transitions
        .iter()
        .find(|transition| transition.name == "settle")
        .expect("`settle` is declared");
    let binding = ir
        .bindings
        .values()
        .next()
        .expect("the example declares a binding");

    let ids = [
        ScenarioId::Invariant {
            entity: ess_conformance::scenario::EntityRef::from(entity_handle),
            after: OutcomeRef::new(
                CommandRef::from(command_handle(&ir, "billing.invoice.CreateInvoice")),
                OutcomeName::new("accepted").expect("valid"),
            ),
        },
        ScenarioId::Transition {
            transition: TransitionRef::new(entity.clone(), &transition.name).expect("valid"),
            by: OutcomeRef::new(
                CommandRef::from(command_handle(&ir, "billing.invoice.PayInvoice")),
                OutcomeName::new("settled").expect("valid"),
            ),
        },
        ScenarioId::Refusal {
            entity,
            state: transition.to.clone(),
            command: CommandRef::from(command_handle(&ir, "billing.invoice.CancelInvoice")),
        },
        ScenarioId::Binding {
            binding: BindingRef::new(BindingName::new(binding.name.as_str()).expect("valid")),
            aspect: BindingAspect::OnFailure,
        },
    ];

    for id in ids {
        let rendered = id.to_string();
        assert_eq!(
            ScenarioId::parse(&rendered).expect("the rendered form parses"),
            id,
            "`{rendered}` did not survive its own rendering"
        );
    }
}

#[test]
fn no_source_file_in_this_crate_reads_a_clock_or_an_unordered_map() {
    // Invariant 9's scan, which covers `ess-compiler` only, applied to the crate that holds both a
    // generated artifact's definition and the runner that executes one. Two claims rest on it and
    // neither is visible to a test that runs twice in one process: a `HashMap` iterates the same way
    // twice in a row, and a wall-clock read only differs across runs. So they are read for, not
    // trusted.
    //
    // `thread::sleep` is in the list for the runner's sake — §40 makes a fixed delay a test of the
    // machine it runs on rather than of the system's semantics, and the repair everyone reaches for
    // when an eventual assertion races a projection is exactly that.
    //
    // Comment lines are stripped first, because this crate's documentation names the tokens it
    // refuses; a scan that could not tell an explanation from a call would have to be weakened until
    // it caught nothing. What that costs is a banned token inside a block comment or a string
    // literal, which is a price worth paying for a scan that stays on.
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut checked = 0;
    for entry in std::fs::read_dir(&source).expect("the crate has sources") {
        let path = entry.expect("an entry").path();
        if path.extension().is_some_and(|it| it == "rs") {
            let text = std::fs::read_to_string(&path).expect("readable");
            let code: String = text
                .lines()
                .filter(|line| !line.trim_start().starts_with("//"))
                .collect::<Vec<_>>()
                .join("\n");
            for banned in [
                "HashMap",
                "HashSet",
                "SystemTime",
                "Instant",
                "rand::",
                "thread::sleep",
            ] {
                assert!(
                    !contains_token(&code, banned),
                    "{} uses {banned}, which makes a generated suite depend on when and where it \
                     was built, and a run depend on when and where it happened",
                    path.display()
                );
            }
            checked += 1;
        }
    }
    assert!(checked >= 8, "only {checked} source files were read");
}

#[test]
fn the_scan_for_a_clock_finds_one_and_does_not_find_a_word_that_merely_ends_in_a_banned_token() {
    // The scan asserts the inverse too, for the reason `aep-domain`'s invariant scan does: a scan
    // that has silently stopped working passes every file, and this one has two ways to stop —
    // matching a substring of a longer identifier until someone deletes it as a false alarm, or
    // matching nothing at all.
    assert!(contains_token(
        "let now = std::time::SystemTime::now();",
        "SystemTime"
    ));
    assert!(contains_token("use rand::rngs::OsRng;", "rand::"));
    assert!(contains_token(
        "std::thread::sleep(delay);",
        "thread::sleep"
    ));
    assert!(!contains_token(
        "if let Operand::Fact(path) = operand {",
        "rand::"
    ));
    assert!(!contains_token("let ranked = rank(status);", "rand::"));
}

/// `true` when `text` uses `token` as a token rather than as the tail of a longer identifier.
///
/// A plain `contains` is what `ess-compiler`'s scan uses and it cannot be reused here: `Operand::`
/// ends in `rand::`, so this crate's predicate walk would fail a scan for randomness that has found
/// nothing. A scan that reports a defect that is not there gets deleted by the next reader, which
/// costs the real check too.
fn contains_token(text: &str, token: &str) -> bool {
    text.match_indices(token).any(|(at, _)| {
        at == 0
            || !text[..at]
                .chars()
                .next_back()
                .is_some_and(|character| character.is_alphanumeric() || character == '_')
    })
}
