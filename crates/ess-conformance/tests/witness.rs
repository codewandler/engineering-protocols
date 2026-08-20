//! Where the three-valued answer lands, one specification at a time.
//!
//! The fixture below is a specification, not a corner-case museum bolted onto `examples/billing/`:
//! wave 3.5 decision 9 keeps the normative example readable as a system, so the constructs a guard
//! can be undecidable *over* — a list, a map, a union, an enum, a bare text field — are declared
//! here instead. It is inline YAML rather than a hand-built `Specification` because it must pass the
//! same parse-then-validate path an author's file does; a fixture built field by field would prove
//! this crate works on documents `ess-domain` refuses.
//!
//! The billing example is what proves nothing here. `amount.amount > 0` is a number against a
//! number, so it is decided either way — the collapse this crate exists to prevent leaves it
//! untouched, which is exactly why the guard a runner would meet first is the guard that cannot
//! catch the defect. `the_normative_shape_of_guard_is_decidable_for_both_signs` is that observation
//! written down, and every other test here is the one billing does not do.

use std::collections::BTreeMap;

use aep_domain::facts::{FactPath, FactValue, Number, Scales};
use aep_domain::node::Node;
use aep_domain::predicate::{Predicate, Truth};
use aep_domain::FactSource;
use ess_compiler::ir::{EssIr, ResolvedCommand};
use ess_compiler::resolve::compile;
use ess_compiler::source::SourceMap;
use ess_conformance::{flatten, when, Decision, InputFacts, Reason, ShapeError};
use ess_domain::name::QualifiedName;
use ess_domain::spec::{RawSpecFile, Specification};
use ess_domain::system::Source;

/// One system, declaring one of every construct a fact path can land on.
const WITNESS: &str = r"
format: ess/1
system: witness
version: v1
domain: witness.orders

types:
  - name: witness.orders.Money
    kind: struct
    fields:
      - name: amount
        type: Decimal
      - name: currency
        type: String

  # A newtype over a struct, so the transparency rule has something to be transparent about:
  # `priced.amount` reaches through it without a segment of its own.
  - name: witness.orders.Priced
    kind: newtype
    of: witness.orders.Money

  - name: witness.orders.Channel
    kind: enum
    variants: [Email, Post]

  - name: witness.orders.Email
    kind: newtype
    of: String

  - name: witness.orders.CompanyRef
    kind: newtype
    of: String

  - name: witness.orders.Payee
    kind: union
    tag: kind
    variants:
      person: witness.orders.Email
      company: witness.orders.CompanyRef

  - name: witness.orders.LineItem
    kind: struct
    fields:
      - name: description
        type: String
      - name: quantity
        type: Integer

  - name: witness.orders.OrderId
    kind: newtype
    of: Uuid

entities:
  - name: witness.orders.Order
    identity:
      name: order_id
      type: witness.orders.OrderId
    fields:
      - name: total
        type: witness.orders.Money
    lifecycle:
      initial: Draft
      states: [Draft, Placed]
      terminal: [Placed]
      transitions:
        - name: place
          from: [Draft]
          to: Placed

events:
  - name: witness.orders.OrderPlaced
    fields:
      - name: order_id
        type: witness.orders.OrderId

errors:
  - name: witness.orders.Refused
    summary: The order was refused.

commands:
  - name: witness.orders.PlaceOrder
    input:
      - name: amount
        type: witness.orders.Money
      - name: priced
        type: witness.orders.Priced
      - name: channel
        type: witness.orders.Channel
      - name: currency
        type: String
      - name: payee
        type: witness.orders.Payee
      - name: lines
        type: List<witness.orders.LineItem>
      - name: labels
        type: Map<String, String>
      - name: note
        type: Optional<String>
      - name: express
        type: Boolean
      - name: quantity
        type: Integer
    outcomes:
      - name: accepted
        when: amount.amount > 0
        creates: witness.orders.Order
        emits:
          - witness.orders.OrderPlaced
      - name: refused
        error: witness.orders.Refused

  # `ess-domain` checks a `when` path's **first** segment against the input field names and no
  # deeper one, and says so in its own source. So this guard parses, validates and compiles, and
  # nothing before this crate ever asks what `vat` is.
  - name: witness.orders.TaxOrder
    input:
      - name: amount
        type: witness.orders.Money
    outcomes:
      - name: taxed
        when: amount.vat > 0
        emits:
          - witness.orders.OrderPlaced
      - name: untaxed
        error: witness.orders.Refused

  - name: witness.orders.ConfirmOrder
    input:
      - name: order_id
        type: witness.orders.OrderId
    outcomes:
      - name: placed
        moves: witness.orders.Order.place
        emits:
          - witness.orders.OrderPlaced
";

/// The fixture, parsed, validated and resolved.
fn compiled() -> EssIr {
    let raw = RawSpecFile::parse(WITNESS).expect("the fixture is well formed");
    let specification = Specification::assemble([(Source::new("witness.yaml"), raw)])
        .unwrap_or_else(|errors| panic!("the fixture validates:\n{errors}"));
    compile(&specification, &SourceMap::new())
        .unwrap_or_else(|diagnostics| panic!("the fixture resolves:\n{diagnostics}"))
}

/// One of the fixture's commands.
fn command<'ir>(ir: &'ir EssIr, name: &str) -> &'ir ResolvedCommand {
    ir.commands
        .get(&QualifiedName::new(name).expect("a valid name"))
        .expect("the fixture declares it")
}

/// The command every guard below is read against.
fn place_order(ir: &EssIr) -> &ResolvedCommand {
    command(ir, "witness.orders.PlaceOrder")
}

/// The `when` an outcome of `command` declares.
fn declared_guard<'ir>(command: &'ir ResolvedCommand, outcome: &str) -> &'ir Predicate {
    when(
        command
            .outcomes
            .iter()
            .find(|it| it.name.as_str() == outcome)
            .expect("the fixture declares it"),
    )
    .expect("that outcome is taken by a `when`")
}

fn text(value: &str) -> Node {
    Node::Text(value.to_owned())
}

fn number(value: f64) -> Node {
    Node::Number(Number::new(value).expect("a finite number"))
}

fn map(pairs: &[(&str, Node)]) -> BTreeMap<String, Node> {
    pairs
        .iter()
        .map(|(key, value)| ((*key).to_owned(), value.clone()))
        .collect()
}

fn money(amount: f64, currency: &str) -> Node {
    Node::Map(map(&[
        ("amount", number(amount)),
        ("currency", text(currency)),
    ]))
}

/// A candidate that is a value of `PlaceOrder`'s input, with `amount.amount` chosen by the caller.
///
/// Every field is supplied except `note`, which is `Optional` and is the fixture's one absent value.
fn candidate(amount: f64) -> BTreeMap<String, Node> {
    map(&[
        ("amount", money(amount, "EUR")),
        ("priced", money(7.0, "EUR")),
        ("channel", text("Email")),
        ("currency", text("USD")),
        (
            "payee",
            Node::Map(map(&[
                ("kind", text("person")),
                ("person", text("a@b.test")),
            ])),
        ),
        (
            "lines",
            Node::Seq(vec![Node::Map(map(&[
                ("description", text("a line")),
                ("quantity", number(2.0)),
            ]))]),
        ),
        ("labels", Node::Map(map(&[("region", text("eu"))]))),
        ("express", Node::Bool(true)),
        ("quantity", number(3.0)),
    ])
}

/// The facts a candidate with this `amount.amount` projects.
fn facts(ir: &EssIr, amount: f64) -> InputFacts<'_> {
    flatten(ir, place_order(ir), &candidate(amount))
        .unwrap_or_else(|errors| panic!("the candidate fits the input:\n{errors}"))
}

fn guard(expression: &str) -> Predicate {
    Predicate::parse_expression(expression).expect("a valid predicate")
}

fn path(value: &str) -> FactPath {
    FactPath::new(value).expect("a valid fact path")
}

/// The one refusal reason a decision carries, when it carries exactly one.
fn sole_reason(decision: &Decision) -> Reason {
    let refusal = decision
        .unevaluable()
        .unwrap_or_else(|| panic!("expected a refusal, got {decision}"));
    assert_eq!(refusal.causes.len(), 1, "expected one cause, got {refusal}");
    refusal.causes[0].reason.clone()
}

// ---------------------------------------------------------------------------------------------
// The flattener
// ---------------------------------------------------------------------------------------------

#[test]
fn a_candidate_input_projects_one_fact_per_scalar_leaf() {
    let ir = compiled();
    let facts = facts(&ir, 12.5);

    assert_eq!(
        facts.fact(&path("amount.amount")),
        Some(FactValue::Number(Number::new(12.5).expect("finite"))),
        "a struct-typed input field is reached through its field name"
    );
    assert_eq!(
        facts.fact(&path("amount.currency")),
        Some(FactValue::text("EUR"))
    );
    assert_eq!(
        facts.fact(&path("channel")),
        Some(FactValue::text("Email")),
        "an enum projects as the variant's name"
    );
    assert_eq!(facts.fact(&path("express")), Some(FactValue::Bool(true)));
    assert_eq!(
        facts.fact(&path("quantity")),
        Some(FactValue::Number(Number::from(3_i64)))
    );
}

#[test]
fn a_newtype_is_transparent_so_a_deep_path_reaches_through_it_without_a_segment() {
    let ir = compiled();
    let facts = facts(&ir, 1.0);

    // `priced` is `Priced = newtype of Money`. If a newtype consumed a segment there would be no
    // spelling for the inside of one, and `priced.amount` would name nothing.
    assert_eq!(
        facts.fact(&path("priced.amount")),
        Some(FactValue::Number(Number::new(7.0).expect("finite"))),
    );
    assert_eq!(
        facts.fact(&path("priced")),
        None,
        "the wrapper itself binds nothing; only its scalar leaves do"
    );
}

#[test]
fn a_newtype_is_transparent_when_a_path_is_resolved_as_well_as_when_it_is_projected() {
    let ir = compiled();
    let facts = facts(&ir, 1.0);

    // Projection and classification are two walks over the same rule, and only the first of them is
    // exercised by a path that binds. `priced.vat` binds nothing, so it is the classifier that has
    // to reach through `Priced` — and a newtype that consumed `vat` would report `priced` as a
    // struct instead of reporting `vat` as undeclared.
    assert_eq!(
        sole_reason(&facts.decide(&guard("priced.vat > 0"))),
        Reason::PathNotDeclared {
            path: path("priced.vat"),
            segment: "vat".to_owned(),
        },
    );
    assert_eq!(
        facts.decide(&guard("priced.currency == EUR")),
        Decision::Satisfied,
        "and the projecting walk agrees: the same segment reaches through the same wrapper"
    );
}

#[test]
fn a_list_a_map_and_a_union_bind_no_fact_because_no_fact_path_can_name_one() {
    let ir = compiled();
    let facts = facts(&ir, 1.0);

    for unreachable in ["lines", "labels", "payee", "amount"] {
        assert_eq!(
            facts.fact(&path(unreachable)),
            None,
            "`{unreachable}` is an aggregate and binds nothing"
        );
    }
    // Not even the parts a fact value could have held: an element's field, a map's entry, a union's
    // tag. Each is a decision a later wave takes, and none is taken by accident here.
    for unreachable in ["lines.description", "labels.region", "payee.kind"] {
        assert_eq!(facts.fact(&path(unreachable)), None);
    }
}

#[test]
fn an_absent_optional_binds_nothing_rather_than_binding_a_default() {
    let ir = compiled();
    assert_eq!(
        facts(&ir, 1.0).fact(&path("note")),
        None,
        "an omitted `Optional<String>` is unobserved, and unobserved is not empty"
    );
}

#[test]
fn a_candidate_missing_a_required_field_is_refused_before_any_guard_is_read() {
    let ir = compiled();
    let mut short = candidate(1.0);
    short.remove("currency");
    short.remove("express");

    let errors = flatten(&ir, place_order(&ir), &short).expect_err("two fields are missing");
    assert_eq!(
        errors.len(),
        2,
        "both are reported, not just the first: {errors}"
    );
    assert!(errors.iter().any(|error| matches!(
        error,
        ShapeError::MissingField { field, .. } if field == "currency"
    )));
    assert!(errors.iter().any(|error| matches!(
        error,
        ShapeError::MissingField { field, .. } if field == "express"
    )));
}

#[test]
fn a_candidate_carrying_a_field_no_type_declares_is_refused() {
    let ir = compiled();
    let mut extra = candidate(1.0);
    extra.insert("discount".to_owned(), number(1.0));
    let Node::Map(inner) = extra
        .get_mut("amount")
        .expect("the candidate carries an amount")
    else {
        panic!("the amount is a mapping");
    };
    inner.insert("vat".to_owned(), number(19.0));

    let errors = flatten(&ir, place_order(&ir), &extra).expect_err("two fields are undeclared");
    assert!(errors.iter().any(|error| matches!(
        error,
        ShapeError::UndeclaredField { at, field } if at.is_empty() && field == "discount"
    )));
    assert!(errors.iter().any(|error| matches!(
        error,
        ShapeError::UndeclaredField { at, field } if at == "amount" && field == "vat"
    )));
}

#[test]
fn a_scalar_of_the_wrong_shape_is_refused_rather_than_coerced() {
    let ir = compiled();
    let mut wrong = candidate(1.0);
    wrong.insert("express".to_owned(), text("true"));
    wrong.insert("quantity".to_owned(), number(1.5));
    wrong.insert("channel".to_owned(), text("Carrier pigeon"));

    let errors = flatten(&ir, place_order(&ir), &wrong).expect_err("three values are wrong");
    let shapes: Vec<_> = errors
        .iter()
        .filter(|error| matches!(error, ShapeError::WrongShape { .. }))
        .collect();
    assert_eq!(shapes.len(), 3, "each is reported: {errors}");
    assert!(
        errors.iter().any(|error| matches!(
            error,
            ShapeError::WrongShape { at, .. } if at == "quantity"
        )),
        "1.5 is not an Integer, and rounding it would decide `quantity == 1` differently from the \
         system under test"
    );
    assert!(errors.iter().any(|error| matches!(
        error,
        ShapeError::WrongShape { at, .. } if at == "channel"
    )));
}

// ---------------------------------------------------------------------------------------------
// The two decidable answers
// ---------------------------------------------------------------------------------------------

#[test]
fn the_normative_shape_of_guard_is_decidable_for_both_signs() {
    let ir = compiled();
    let guard = declared_guard(place_order(&ir), "accepted");

    assert_eq!(facts(&ir, 12.5).decide(guard), Decision::Satisfied);
    assert!(
        matches!(facts(&ir, -1.0).decide(guard), Decision::Refuted(_)),
        "a number against a number is decided either way, which is why this guard cannot catch a \
         runner that reads `Unknown` as `False`"
    );
}

#[test]
fn a_refuted_guard_carries_the_leaf_and_the_value_that_refuted_it() {
    let ir = compiled();
    let decision = facts(&ir, -1.0).decide(&guard("amount.amount > 0"));

    let refutation = decision
        .refutation()
        .unwrap_or_else(|| panic!("expected a refutation, got {decision}"));
    assert_eq!(refutation.truth, Truth::False);
    assert_eq!(refutation.causes.len(), 1);
    assert_eq!(
        refutation.causes[0].observed,
        vec![(
            path("amount.amount"),
            FactValue::Number(Number::new(-1.0).expect("finite"))
        )],
        "a refutation says which value refuted it, so a shrunk counterexample is already in hand"
    );
    assert!(!decision.is_satisfied());
}

#[test]
fn equality_over_two_texts_is_decided_even_though_ordering_them_is_not() {
    let ir = compiled();
    let facts = facts(&ir, 1.0);

    assert_eq!(facts.decide(&guard("currency == USD")), Decision::Satisfied);
    assert!(matches!(
        facts.decide(&guard("currency == EUR")),
        Decision::Refuted(_)
    ));
}

// ---------------------------------------------------------------------------------------------
// The five sources of `Unknown` that no candidate value can fix
// ---------------------------------------------------------------------------------------------

#[test]
fn ordering_two_texts_is_unevaluable_because_an_ess_specification_declares_no_scale() {
    let ir = compiled();
    let decision = facts(&ir, 1.0).decide(&guard("currency > EUR"));

    assert_eq!(
        sole_reason(&decision),
        Reason::TextNotOrdered {
            left: "USD".to_owned(),
            right: "EUR".to_owned(),
        },
        "`USD > EUR` is not false; it is a comparison nothing in the model gives a meaning"
    );
    assert!(
        decision.refutation().is_none(),
        "reading this as a refutation is the collapse invariant 5 forbids, seen from a generator"
    );
}

#[test]
fn the_same_text_ordering_is_decidable_once_a_scale_contains_both_values() {
    let ir = compiled();
    let mut scales = Scales::default();
    scales.insert(
        "currency".to_owned(),
        vec!["EUR".to_owned(), "USD".to_owned()],
    );
    let facts = facts(&ir, 1.0).with_scales(scales);

    assert_eq!(
        facts.decide(&guard("currency > EUR")),
        Decision::Satisfied,
        "the refusal above is about the empty scale vocabulary, not about text being unorderable"
    );
}

#[test]
fn ordering_across_two_types_is_unevaluable_not_false() {
    let ir = compiled();
    let facts = facts(&ir, 1.0);

    assert_eq!(
        sole_reason(&facts.decide(&guard("currency > 0"))),
        Reason::TypesNotOrdered {
            left: "text",
            right: "number",
        }
    );
    assert_eq!(
        sole_reason(&facts.decide(&guard("express > false"))),
        Reason::TypesNotOrdered {
            left: "boolean",
            right: "boolean",
        },
        "same type, still no ordering: two booleans have no `>` either"
    );
}

#[test]
fn a_deep_path_no_type_declares_is_unevaluable_and_ess_domain_does_not_refuse_it() {
    let ir = compiled();

    // Not a predicate this test made up: `TaxOrder`'s `taxed` branch is declared with this guard,
    // and the fixture compiled — which is the gap. `ess-domain` resolves a `when` path's first
    // segment only, so `amount` is checked and `vat` is not, by anything, until here.
    let declared = declared_guard(command(&ir, "witness.orders.TaxOrder"), "taxed");
    let over_tax_order = flatten(
        &ir,
        command(&ir, "witness.orders.TaxOrder"),
        &map(&[("amount", money(1.0, "EUR"))]),
    )
    .expect("the candidate fits the input");
    assert_eq!(
        sole_reason(&over_tax_order.decide(declared)),
        Reason::PathNotDeclared {
            path: path("amount.vat"),
            segment: "vat".to_owned(),
        },
        "a branch the compiler accepted, refused by the first thing that resolves its path"
    );

    assert_eq!(
        sole_reason(&facts(&ir, 1.0).decide(&guard("amount.vat > 0"))),
        Reason::PathNotDeclared {
            path: path("amount.vat"),
            segment: "vat".to_owned(),
        }
    );
    assert_eq!(
        sole_reason(&facts(&ir, 1.0).decide(&guard("currency.length > 0"))),
        Reason::PathNotDeclared {
            path: path("currency.length"),
            segment: "length".to_owned(),
        },
        "a primitive has no members, so a segment after one names nothing either"
    );
}

#[test]
fn a_path_landing_on_an_aggregate_is_unevaluable_by_construction() {
    let ir = compiled();
    let facts = facts(&ir, 1.0);

    // A fact value is `Bool | Number | Text`. None of these four has a scalar spelling, and no
    // candidate value changes that.
    for (expression, target, holds) in [
        ("lines > 0", "lines", "a list"),
        ("labels > 0", "labels", "a map"),
        ("payee == person", "payee", "a union"),
        ("amount > 0", "amount", "a struct"),
    ] {
        assert_eq!(
            sole_reason(&facts.decide(&guard(expression))),
            Reason::PathNotScalar {
                path: path(target),
                holds,
            },
            "`{expression}` reads {holds}"
        );
    }
}

#[test]
fn a_path_into_a_list_or_a_union_names_the_aggregate_rather_than_the_missing_element() {
    let ir = compiled();
    let facts = facts(&ir, 1.0);

    assert_eq!(
        sole_reason(&facts.decide(&guard("lines.quantity > 0"))),
        Reason::PathNotScalar {
            path: path("lines.quantity"),
            holds: "a list",
        },
        "the element exists in the type; what does not exist is a path that selects one"
    );
    assert_eq!(
        sole_reason(&facts.decide(&guard("payee.kind == person"))),
        Reason::PathNotScalar {
            path: path("payee.kind"),
            holds: "a union",
        },
        "a union's tag is a text a fact could hold; binding it is a decision, not an oversight"
    );
}

// ---------------------------------------------------------------------------------------------
// The one source of `Unknown` a different candidate can fix
// ---------------------------------------------------------------------------------------------

#[test]
fn an_absent_optional_is_unevaluable_but_says_a_candidate_could_repair_it() {
    let ir = compiled();
    let decision = facts(&ir, 1.0).decide(&guard("note == urgent"));

    let refusal = decision
        .unevaluable()
        .unwrap_or_else(|| panic!("expected a refusal, got {decision}"));
    assert_eq!(
        refusal.causes[0].reason,
        Reason::ValueAbsent { path: path("note") }
    );
    assert!(
        refusal.fixable_by_another_candidate(),
        "supplying `note` decides this guard; fixing the specification is not what is needed"
    );
    assert!(
        !decision.is_satisfied(),
        "still a refusal: `Unknown` never permits a transition, whoever could repair it"
    );
}

#[test]
fn only_an_absent_value_says_another_candidate_would_help() {
    let ir = compiled();
    let facts = facts(&ir, 1.0);

    for undecidable in [
        "currency > EUR",
        "currency > 0",
        "amount.vat > 0",
        "lines > 0",
        "payee == person",
    ] {
        let decision = facts.decide(&guard(undecidable));
        let refusal = decision
            .unevaluable()
            .unwrap_or_else(|| panic!("expected a refusal for `{undecidable}`"));
        assert!(
            !refusal.fixable_by_another_candidate(),
            "`{undecidable}` is a property of the specification; retrying values on it is a loop \
             with no exit"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// The refusal itself
// ---------------------------------------------------------------------------------------------

#[test]
fn a_conjunction_of_two_undecidable_leaves_reports_both() {
    let ir = compiled();
    let conjunction = Predicate::all(vec![
        guard("amount.amount > 0"),
        guard("amount.vat > 0"),
        guard("lines > 0"),
    ]);
    let decision = facts(&ir, 1.0).decide(&conjunction);

    let refusal = decision
        .unevaluable()
        .unwrap_or_else(|| panic!("expected a refusal, got {decision}"));
    assert_eq!(
        refusal.causes.len(),
        2,
        "the decidable leaf is not a cause and the two undecidable ones both are: {refusal}"
    );
    assert!(refusal
        .causes
        .iter()
        .any(|cause| cause.expression == "amount.vat > 0"));
    assert!(refusal
        .causes
        .iter()
        .any(|cause| cause.expression == "lines > 0"));
}

#[test]
fn a_refusal_names_the_predicate_the_command_and_the_path() {
    let ir = compiled();
    let decision = facts(&ir, 1.0).decide(&guard("amount.vat > 0"));
    let rendered = decision.to_string();

    for expected in [
        "amount.vat > 0",
        "witness.orders.PlaceOrder",
        "vat",
        "do not declare",
    ] {
        assert!(
            rendered.contains(expected),
            "a refusal a person acts on names {expected:?}; it read:\n{rendered}"
        );
    }
}

#[test]
fn a_disjunction_one_of_whose_branches_holds_is_satisfied_despite_an_undecidable_branch() {
    let ir = compiled();
    // Kleene: `True or Unknown` is `True`. An undecidable leaf under a satisfied disjunction is not
    // a refusal, and reporting it as one would refuse specifications that are perfectly witnessable.
    let disjunction = Predicate::any(vec![guard("amount.amount > 0"), guard("amount.vat > 0")]);
    assert_eq!(facts(&ir, 12.5).decide(&disjunction), Decision::Satisfied);
    // And the dual: `False and Unknown` is `False`, so the guard is refuted rather than refused.
    let conjunction = Predicate::all(vec![guard("amount.amount > 0"), guard("amount.vat > 0")]);
    assert!(matches!(
        facts(&ir, -1.0).decide(&conjunction),
        Decision::Refuted(_)
    ));
}

#[test]
fn unclassified_is_a_drift_alarm_and_no_enumerated_source_trips_it() {
    let ir = compiled();
    let facts = facts(&ir, 1.0);

    // Every guard this crate claims to explain, decided and classified. A branch added to
    // `Predicate::evaluate` without a `Reason` added beside it lands on `Unclassified`, and this is
    // what notices.
    for expression in [
        "amount.amount > 0",
        "currency == USD",
        "currency > EUR",
        "currency > 0",
        "express > false",
        "amount.vat > 0",
        "currency.length > 0",
        "lines > 0",
        "labels > 0",
        "payee == person",
        "amount > 0",
        "lines.quantity > 0",
        "payee.kind == person",
        "note == urgent",
        "note",
        "defined(note)",
        "not note",
        "channel == Email",
    ] {
        let predicate = guard(expression);
        let decision = facts.decide(&predicate);
        match (predicate.evaluate(&facts), &decision) {
            (Truth::True, Decision::Satisfied) | (Truth::False, Decision::Refuted(_)) => {}
            (Truth::Unknown, Decision::Unevaluable(refusal)) => {
                assert!(
                    !refusal.causes.is_empty(),
                    "`{expression}` refused with no reason at all"
                );
                assert!(
                    refusal
                        .causes
                        .iter()
                        .all(|cause| cause.reason != Reason::Unclassified),
                    "`{expression}` is a source of `Unknown` this crate does not enumerate: \
                     {refusal}"
                );
            }
            (truth, decision) => panic!("`{expression}` evaluated {truth} and decided {decision}"),
        }
    }
}

#[test]
fn otherwise_and_external_are_not_guards_over_the_input() {
    let ir = compiled();
    let refused = place_order(&ir)
        .outcomes
        .iter()
        .find(|outcome| outcome.name.as_str() == "refused")
        .expect("the fixture declares it");

    assert!(
        when(refused).is_none(),
        "`otherwise` is decided relative to every other branch, so one predicate cannot answer it; \
         choosing a branch is wave 4's job and not this gate's"
    );
}
