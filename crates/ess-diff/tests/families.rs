//! One mutation at a time, over the families the fixture pair does not move.
//!
//! `examples/revision-pair/` proves the four relations on a specification a person can audit. It
//! deliberately differs by four changes and nothing else, which leaves events, errors, components,
//! the system header and most of a type's insides with no fixture to be right about. This file is
//! those: a witness specification, and one edit per test.
//!
//! It is inline YAML rather than a hand-built [`EssIr`] because a comparator has to be checked
//! against models that came through the same parse-then-validate path an author's files do — an IR
//! assembled field by field would prove this crate works on specifications `ess-domain` refuses. It
//! is a *second* specification rather than corners bolted onto the fixture pair for the reason wave
//! 3.5 decision 9 keeps the normative example readable: the pair's whole value is that its four
//! changes are all of its changes.
//!
//! # What each test is a mutation of
//!
//! The falsifiability list in design §71 asks for four of these by name. Three are here — an
//! event-field type change that must not be ignored, a remove-and-add that must not be merged into a
//! rename, and a grant whose direction must not be flipped — and the fourth, hash-ordered output, is
//! in `canonical.rs` where the bytes are.

use ess_compiler::ir::EssIr;
use ess_compiler::resolve::compile;
use ess_compiler::source::SourceMap;
use ess_diff::change::{
    ActorChange, BindingChange, CommandChange, ComponentChange, EntityChange, ErrorChange,
    EventChange, SemanticChange, SystemChange, TypeChange, ViewChange,
};
use ess_diff::{diff, SemanticRelation};
use ess_domain::spec::{RawSpecFile, Specification};
use ess_domain::system::Source;

/// The header, and the one component: a file that names no domain of its own.
const SYSTEM: &str = r"
format: ess/1
system: witness
version: v1
summary: The system under comparison.
domains:
  - witness.orders

components:
  - component: order-service
    summary: Runs orders.
    owns:
      domains:
        - witness.orders
    accepts:
      commands:
        - witness.orders.CloseOrder
    publishes:
      events:
        - witness.orders.OrderClosed

bindings:
  - id: note-on-close
    summary: Stamp a note on an order the moment it closes.
    when:
      event: witness.orders.OrderClosed
    invoke:
      command: witness.orders.CloseOrder
    mapping:
      order_id: event.order_id
      note: closed-again
    delivery: at_least_once
    on_failure: retry
";

/// One bounded context holding one of every construct the six families reach into.
const DOMAIN: &str = r"
domain: witness.orders

types:
  - name: witness.orders.OrderId
    kind: newtype
    of: Uuid
  - name: witness.orders.Email
    kind: newtype
    of: String
  - name: witness.orders.CompanyRef
    kind: newtype
    of: String
  - name: witness.orders.Channel
    kind: enum
    variants: [Email, Post]
  - name: witness.orders.Payee
    kind: union
    tag: kind
    variants:
      person: witness.orders.Email
      company: witness.orders.CompanyRef
  - name: witness.orders.Money
    kind: struct
    fields:
      - name: amount
        type: Decimal
      - name: currency
        type: String
    invariants:
      - amount >= 0

entities:
  - name: witness.orders.Order
    identity:
      name: order_id
      type: witness.orders.OrderId
    fields:
      - name: total
        type: witness.orders.Money
      - name: payee
        type: witness.orders.Payee
      - name: channel
        type: witness.orders.Channel
    invariants:
      - total.amount >= 0
    lifecycle:
      initial: Open
      states: [Open, Closed]
      terminal: [Closed]
      transitions:
        - name: close
          from: [Open]
          to: Closed

  # A second entity, so a view can change what it projects without inventing a system: its
  # identity deliberately shares the Order's field name and type, which is what lets a
  # source-changed edit be one edit.
  - name: witness.orders.Ticket
    identity:
      name: order_id
      type: witness.orders.OrderId
    lifecycle:
      initial: Open
      states: [Open]
      terminal: [Open]

views:
  - name: witness.orders.OrderById
    source: witness.orders.Order
    consistency: eventual
    filter: state == Open
    fields:
      - name: order_id
        type: witness.orders.OrderId
      - name: total
        type: witness.orders.Money

errors:
  - name: witness.orders.OrderStateConflict
    summary: The order is not in a state this command acts from.
    fields:
      - name: state
        type: witness.orders.Order.State

  - name: witness.orders.OrderLocked
    summary: The order is locked and takes no reopening.

commands:
  - name: witness.orders.CloseOrder
    input:
      - name: order_id
        type: witness.orders.OrderId
      - name: note
        type: String
    outcomes:
      - name: closed
        moves: witness.orders.Order.close
        instance: order_id
        emits:
          - witness.orders.OrderClosed
      - name: wrong-state
        wrong_state: true
        error: witness.orders.OrderStateConflict

  # Declared, and not on any component's surface: the component test adds it to `accepts:` in the
  # later revision, so the edit is one line in one file and one change in the delta.
  - name: witness.orders.ReopenOrder
    input:
      - name: order_id
        type: witness.orders.OrderId
      - name: amount
        type: witness.orders.Money
    outcomes:
      - name: reopened
        when: amount.amount > 0
        emits:
          - witness.orders.OrderReopened

      - name: skipped
        error: witness.orders.OrderLocked
        summary: Nothing was reopened.

events:
  - name: witness.orders.OrderReopened
    fields:
      - name: order_id
        type: witness.orders.OrderId

  - name: witness.orders.OrderClosed
    naming:
      wire: order.closed.v1
    fields:
      - name: order_id
        type: witness.orders.OrderId
      - name: total
        type: witness.orders.Money

actors:
  - name: witness.orders.Clerk
    may:
      - witness.orders.CloseOrder
";

/// Compiles one revision of the witness from however many documents it is written in.
fn compiled(files: &[(&str, &str)]) -> EssIr {
    let parsed = files
        .iter()
        .map(|(label, text)| {
            let raw = RawSpecFile::parse(text)
                .unwrap_or_else(|error| panic!("{label} is well formed: {error}"));
            (Source::new(*label), raw)
        })
        .collect::<Vec<_>>();
    let specification = Specification::assemble(parsed)
        .unwrap_or_else(|errors| panic!("the witness validates:\n{errors}"));
    compile(&specification, &SourceMap::new())
        .unwrap_or_else(|diagnostics| panic!("the witness resolves:\n{diagnostics}"))
}

/// The witness as it is written before any edit.
fn base() -> Vec<(&'static str, &'static str)> {
    vec![("system.yaml", SYSTEM), ("domains/orders.yaml", DOMAIN)]
}

/// Every change between the witness and an edited copy of it.
fn changes(system: &str, domain: &str) -> Vec<SemanticChange> {
    changes_from(&[("system.yaml", system), ("domains/orders.yaml", domain)])
}

/// Every change between the witness and a revision written in whatever documents are given.
fn changes_from(files: &[(&str, &str)]) -> Vec<SemanticChange> {
    let before = compiled(&base());
    let after = compiled(files);
    diff(&before, &after)
        .expect("the witness and its edit are one system")
        .changes()
        .to_vec()
}

/// The one change an edit produced, or a failure naming everything it produced instead.
fn one(system: &str, domain: &str) -> SemanticChange {
    let found = changes(system, domain);
    assert_eq!(
        found.len(),
        1,
        "one edit should be one change, and this produced {}:\n  {}",
        found.len(),
        found
            .iter()
            .map(|change| change.id().to_string())
            .collect::<Vec<_>>()
            .join("\n  ")
    );
    found.into_iter().next().expect("one change")
}

/// The witness domain with one substring replaced, refusing an edit that matched nothing.
///
/// The refusal matters more than it looks: a `replace` that matched nothing produces an *identical*
/// revision, an empty delta, and a test that passes because it compared a document with itself.
fn edited(from: &str, to: &str) -> String {
    assert!(
        DOMAIN.contains(from),
        "the witness does not contain `{from}`, so this edit would compare a document with itself"
    );
    DOMAIN.replace(from, to)
}

// ---- events ----------------------------------------------------------------------------------

#[test]
fn an_event_field_that_changed_type_is_reported() {
    // Design §71's first mutation, from the other side: an engine that ignored an event-field type
    // change would report nothing here, and a payload that silently stopped carrying money is the
    // change a consumer finds in production.
    let change = one(
        SYSTEM,
        &edited(
            "      - name: total\n        type: witness.orders.Money\n\nactors:",
            "      - name: total\n        type: Decimal\n\nactors:",
        ),
    );

    let SemanticChange::Event { subject, changed } = &change else {
        panic!("an event payload change is an event change, and this is {change:?}");
    };
    assert_eq!(subject.to_string(), "witness.orders.OrderClosed");
    assert_eq!(
        changed,
        &EventChange::FieldTypeChanged {
            field: "total".to_owned(),
            before: "witness.orders.Money".to_owned(),
            after: "Decimal".to_owned(),
        }
    );
    assert_eq!(change.relation(), SemanticRelation::Changed);
}

#[test]
fn an_events_wire_name_moving_is_not_the_event_moving() {
    // Design §41's worked example. The logical identity is untouched, so nothing inside the
    // specification moves; every deployed consumer breaks. Those are different sentences and the
    // model separates them precisely so a delta can say which one happened.
    let change = one(
        SYSTEM,
        &edited("wire: order.closed.v1", "wire: order.closed.v2"),
    );

    let SemanticChange::Event { subject, changed } = &change else {
        panic!("this is {change:?}");
    };
    assert_eq!(
        subject.to_string(),
        "witness.orders.OrderClosed",
        "the event is the same event: its qualified name did not move"
    );
    assert_eq!(
        changed,
        &EventChange::WireNameChanged {
            before: "order.closed.v1".to_owned(),
            after: "order.closed.v2".to_owned(),
        }
    );
}

#[test]
fn an_event_renamed_is_reported_as_removed_and_added_and_never_as_a_rename() {
    // Design §71's third mutation, and design §6's rule. The two names differ by one word and the
    // payloads are identical, which is the most tempting shape there is for a similarity heuristic —
    // so it is the shape the test uses.
    let found = changes(
        &SYSTEM.replace("witness.orders.OrderClosed", "witness.orders.OrderSettled"),
        &edited("witness.orders.OrderClosed", "witness.orders.OrderSettled"),
    );

    let ids: Vec<String> = found.iter().map(|change| change.id().to_string()).collect();
    assert!(
        ids.contains(&"event/witness.orders.OrderClosed/removed".to_owned()),
        "the old identity is gone: {ids:?}"
    );
    assert!(
        ids.contains(&"event/witness.orders.OrderSettled/added".to_owned()),
        "the new identity is new: {ids:?}"
    );
    assert!(
        !ids.iter().any(|id| id.contains("renam")),
        "no rename is inferred, however similar the names look: {ids:?}"
    );
    // The payloads are byte-identical, which is the last thing a similarity heuristic would have
    // needed, and it is still two changes rather than one.
    assert!(
        !ids.iter()
            .any(|id| id.starts_with("event/witness.orders.OrderSettled/field")),
        "the new event is added whole, not diffed against the old one: {ids:?}"
    );
    // The component published it, so its surface moved too — the honest answer, and the reason this
    // edit is not a `one()`.
    assert!(
        ids.contains(
            &"component/order-service/publishes-removed/witness.orders.OrderClosed".to_owned()
        ) && ids.contains(
            &"component/order-service/publishes-added/witness.orders.OrderSettled".to_owned()
        ),
        "the component that published it publishes the new one instead: {ids:?}"
    );
}

#[test]
fn reordering_an_event_payload_is_reported_once_and_not_as_a_field_change() {
    let change = one(
        SYSTEM,
        &edited(
            "      - name: order_id\n        type: witness.orders.OrderId\n      - name: total\n        type: witness.orders.Money\n\nactors:",
            "      - name: total\n        type: witness.orders.Money\n      - name: order_id\n        type: witness.orders.OrderId\n\nactors:",
        ),
    );

    let SemanticChange::Event { changed, .. } = &change else {
        panic!("this is {change:?}");
    };
    assert_eq!(
        changed,
        &EventChange::FieldOrderChanged {
            before: vec!["order_id".to_owned(), "total".to_owned()],
            after: vec!["total".to_owned(), "order_id".to_owned()],
        }
    );
}

// ---- errors ----------------------------------------------------------------------------------

#[test]
fn what_an_error_tells_the_caller_is_compared() {
    let change = one(
        SYSTEM,
        &edited(
            "summary: The order is not in a state this command acts from.",
            "summary: The order is already closed.",
        ),
    );

    let SemanticChange::Error { subject, changed } = &change else {
        panic!("this is {change:?}");
    };
    assert_eq!(subject.to_string(), "witness.orders.OrderStateConflict");
    assert_eq!(
        changed,
        &ErrorChange::SummaryChanged {
            before: Some("The order is not in a state this command acts from.".to_owned()),
            after: Some("The order is already closed.".to_owned()),
        }
    );
}

#[test]
fn an_error_that_gained_a_field_is_reported_with_the_type_it_carries() {
    let change = one(
        SYSTEM,
        &edited(
            "      - name: state\n        type: witness.orders.Order.State",
            "      - name: state\n        type: witness.orders.Order.State\n      - name: attempted\n        type: String",
        ),
    );

    let SemanticChange::Error { changed, .. } = &change else {
        panic!("this is {change:?}");
    };
    assert_eq!(
        changed,
        &ErrorChange::FieldAdded {
            field: "attempted".to_owned(),
            type_ref: "String".to_owned(),
        }
    );
}

// ---- types, down to every arm of a body ---------------------------------------------------------

#[test]
fn a_union_gaining_a_variant_widens_it_just_as_an_enum_does() {
    // The relation is about what a type accepts, not about which keyword declared it — so the rule
    // has to hold for a union as well, and this is the only fixture in the repository that checks
    // it.
    let change = one(
        SYSTEM,
        &edited(
            "      company: witness.orders.CompanyRef",
            "      company: witness.orders.CompanyRef\n      agent: witness.orders.Email",
        ),
    );

    let SemanticChange::Type { subject, changed } = &change else {
        panic!("this is {change:?}");
    };
    assert_eq!(subject.to_string(), "witness.orders.Payee");
    assert_eq!(
        changed,
        &TypeChange::VariantAdded {
            variant: "agent".to_owned()
        }
    );
    assert_eq!(change.relation(), SemanticRelation::Expanded);
}

#[test]
fn a_union_variant_that_carries_something_else_is_not_a_variant_removed_and_added() {
    // A union's variant is identified by its tag value, so a payload that moved is a payload that
    // moved. Reporting it as one variant gone and another arrived would tell a reader the tag
    // vocabulary changed, which it did not.
    let change = one(
        SYSTEM,
        &edited(
            "      person: witness.orders.Email",
            "      person: witness.orders.CompanyRef",
        ),
    );

    let SemanticChange::Type { changed, .. } = &change else {
        panic!("this is {change:?}");
    };
    assert_eq!(
        changed,
        &TypeChange::VariantTypeChanged {
            variant: "person".to_owned(),
            before: "witness.orders.Email".to_owned(),
            after: "witness.orders.CompanyRef".to_owned(),
        }
    );
    assert_eq!(change.relation(), SemanticRelation::Changed);
}

#[test]
fn a_union_that_is_tagged_by_another_field_is_reported() {
    let change = one(SYSTEM, &edited("    tag: kind", "    tag: sort"));

    let SemanticChange::Type { changed, .. } = &change else {
        panic!("this is {change:?}");
    };
    assert_eq!(
        changed,
        &TypeChange::UnionTagChanged {
            before: "kind".to_owned(),
            after: "sort".to_owned(),
        }
    );
}

#[test]
fn reordering_an_enums_variants_is_reported_without_claiming_a_direction() {
    let change = one(
        SYSTEM,
        &edited("variants: [Email, Post]", "variants: [Post, Email]"),
    );

    let SemanticChange::Type { subject, changed } = &change else {
        panic!("this is {change:?}");
    };
    assert_eq!(subject.to_string(), "witness.orders.Channel");
    assert_eq!(
        changed,
        &TypeChange::VariantOrderChanged {
            before: vec!["Email".to_owned(), "Post".to_owned()],
            after: vec!["Post".to_owned(), "Email".to_owned()],
        }
    );
    assert_eq!(
        change.relation(),
        SemanticRelation::Changed,
        "reordering accepts the same set of values, so it is neither a widening nor a narrowing"
    );
}

#[test]
fn a_types_own_invariants_are_reported_as_different_and_never_as_stronger() {
    // The boundary of the slice, met inside one of the six families it does compare. An invariant is
    // a predicate; `amount > 0` is strictly stronger than `amount >= 0` and saying so is a proof,
    // not a comparison. So the delta says they differ, quotes both, and stops — which is a real
    // answer, and the alternative is a classification nobody checked.
    let change = one(SYSTEM, &edited("      - amount >= 0", "      - amount > 0"));

    let SemanticChange::Type { subject, changed } = &change else {
        panic!("this is {change:?}");
    };
    assert_eq!(subject.to_string(), "witness.orders.Money");
    assert_eq!(
        changed,
        &TypeChange::InvariantsChanged {
            before: vec!["amount >= 0".to_owned()],
            after: vec!["amount > 0".to_owned()],
        }
    );
    assert_eq!(
        change.relation(),
        SemanticRelation::Changed,
        "a strictly stronger predicate is still only `changed` here: proving it is a later wave"
    );
}

#[test]
fn a_newtype_that_wraps_something_else_is_reported() {
    let change = one(
        SYSTEM,
        &edited(
            "  - name: witness.orders.CompanyRef\n    kind: newtype\n    of: String",
            "  - name: witness.orders.CompanyRef\n    kind: newtype\n    of: Uuid",
        ),
    );

    let SemanticChange::Type { changed, .. } = &change else {
        panic!("this is {change:?}");
    };
    assert_eq!(
        changed,
        &TypeChange::RepresentationChanged {
            before: "String".to_owned(),
            after: "Uuid".to_owned(),
        }
    );
}

#[test]
fn a_type_that_became_a_different_kind_of_thing_is_reported_as_that_and_nothing_else() {
    // A newtype that became an enum shares no field, no variant and no representation with what it
    // was, so reporting the pieces would be a list of differences nobody can read. One change says
    // it.
    let change = one(
        SYSTEM,
        &edited(
            "  - name: witness.orders.CompanyRef\n    kind: newtype\n    of: String",
            "  - name: witness.orders.CompanyRef\n    kind: enum\n    variants: [Ltd, Gmbh]",
        ),
    );

    let SemanticChange::Type { changed, .. } = &change else {
        panic!("this is {change:?}");
    };
    assert_eq!(
        changed,
        &TypeChange::KindChanged {
            before: "newtype".to_owned(),
            after: "enum".to_owned(),
        }
    );
}

#[test]
fn a_struct_field_that_changed_type_is_reported() {
    let change = one(
        SYSTEM,
        &edited(
            "      - name: currency\n        type: String",
            "      - name: currency\n        type: witness.orders.Channel",
        ),
    );

    let SemanticChange::Type { subject, changed } = &change else {
        panic!("this is {change:?}");
    };
    assert_eq!(subject.to_string(), "witness.orders.Money");
    assert_eq!(
        changed,
        &TypeChange::FieldTypeChanged {
            field: "currency".to_owned(),
            before: "String".to_owned(),
            after: "witness.orders.Channel".to_owned(),
        }
    );
}

// ---- actors ------------------------------------------------------------------------------------

#[test]
fn an_actor_declared_with_no_grants_at_all_is_still_a_change_to_report() {
    let found = changes(
        SYSTEM,
        &edited("actors:\n", "actors:\n  - name: witness.orders.Auditor\n"),
    );

    assert_eq!(found.len(), 1, "{found:?}");
    let SemanticChange::Actor { subject, changed } = &found[0] else {
        panic!("this is {:?}", found[0]);
    };
    assert_eq!(subject.to_string(), "witness.orders.Auditor");
    assert_eq!(changed, &ActorChange::Added);
    assert_eq!(
        found[0].relation(),
        SemanticRelation::Changed,
        "an actor arriving grants nothing by itself; the grants are the widening"
    );
}

// ---- components --------------------------------------------------------------------------------

#[test]
fn a_component_accepting_a_new_command_is_changed_and_not_widened() {
    // The nearest miss to a grant in the whole model, and the reason the rule is written down rather
    // than inferred from the shape: both are a name arriving in a `BTreeSet`. A component's
    // `accepts` says which process serves a command; an actor's `may` says who is allowed to send
    // one. Putting an ownership refactor in the same bucket as a permission grant would spend the
    // bucket a reviewer reads first.
    let system = SYSTEM.replace(
        "        - witness.orders.CloseOrder\n",
        "        - witness.orders.CloseOrder\n        - witness.orders.ReopenOrder\n",
    );
    let found = changes(&system, DOMAIN);
    assert_eq!(found.len(), 1, "one line, one change: {found:?}");
    let accepted = &found[0];

    let SemanticChange::Component { subject, changed } = accepted else {
        panic!("this is {accepted:?}")
    };
    assert_eq!(subject.to_string(), "order-service");
    let ComponentChange::AcceptsAdded { command } = changed else {
        panic!("this is {changed:?}")
    };
    assert_eq!(command.to_string(), "witness.orders.ReopenOrder");
    assert_eq!(
        accepted.relation(),
        SemanticRelation::Changed,
        "a component's surface is an assignment of work, not an authority: it must not read as a \
         widening"
    );
    assert_eq!(
        found
            .iter()
            .filter(|change| change.relation() != SemanticRelation::Changed)
            .count(),
        0,
        "nothing about adding a command widens or narrows what the system permits until an actor \
         may invoke it: {found:?}"
    );
}

#[test]
fn a_component_that_no_longer_publishes_an_event_is_reported() {
    let system = SYSTEM.replace(
        "      events:\n        - witness.orders.OrderClosed\n",
        "      events: []\n",
    );
    let change = {
        let found = changes(&system, DOMAIN);
        assert_eq!(found.len(), 1, "{found:?}");
        found.into_iter().next().expect("one")
    };

    let SemanticChange::Component { changed, .. } = &change else {
        panic!("this is {change:?}");
    };
    let ComponentChange::PublishesRemoved { event } = changed else {
        panic!("this is {changed:?}");
    };
    assert_eq!(event.to_string(), "witness.orders.OrderClosed");
}

// ---- the system header -------------------------------------------------------------------------

#[test]
fn the_specifications_version_moving_is_reported_and_is_not_the_identity() {
    let change = one(&SYSTEM.replace("version: v1", "version: v2"), DOMAIN);

    let SemanticChange::System { subject, changed } = &change else {
        panic!("this is {change:?}");
    };
    assert_eq!(subject.to_string(), "witness");
    assert_eq!(
        changed,
        &SystemChange::VersionChanged {
            before: ess_domain::name::Version::V1,
            after: ess_domain::name::Version::new(2).expect("v2"),
        }
    );
    assert_eq!(
        change.subject(),
        None,
        "the system is not a construct declared inside the specification, so it has no semantic ref"
    );
}

#[test]
fn the_paragraph_saying_what_the_system_is_is_compared() {
    let change = one(
        &SYSTEM.replace(
            "summary: The system under comparison.",
            "summary: Orders, and the one command that closes them.",
        ),
        DOMAIN,
    );

    let SemanticChange::System { changed, .. } = &change else {
        panic!("this is {change:?}");
    };
    assert_eq!(
        changed,
        &SystemChange::SummaryChanged {
            before: Some("The system under comparison.".to_owned()),
            after: Some("Orders, and the one command that closes them.".to_owned()),
        }
    );
}

// ---- what is deliberately not a change -----------------------------------------------------------

#[test]
fn writing_out_a_naming_default_is_not_a_change() {
    // `wire:` left out and `wire:` written out as the declaration's own last segment are the same
    // wire name. A delta that fired here would report a change every time an author made something
    // explicit, which is the fastest way to make a report nobody reads.
    let found = changes(
        SYSTEM,
        &edited(
            "  - name: witness.orders.Channel\n    kind: enum",
            "  - name: witness.orders.Channel\n    naming:\n      wire: Channel\n      display: Channel\n    kind: enum",
        ),
    );

    assert!(
        found.is_empty(),
        "writing out the model's own fallback is not a change: {found:?}"
    );
}

#[test]
fn a_construct_moving_between_files_is_not_a_change() {
    // Design §7's claim, at the granularity the fixture pair cannot show: the witness is two files,
    // so an entire domain's actors can move from one to the other. Everything an author writes is
    // indexed by the name it declares, so where it is written is not a fact about the system.
    let actors =
        "\nactors:\n  - name: witness.orders.Clerk\n    may:\n      - witness.orders.CloseOrder\n";
    assert!(DOMAIN.ends_with(actors), "the witness ends with its actors");
    let domain = DOMAIN.strip_suffix(actors).expect("checked above");
    let moved = format!("domain: witness.orders\n{actors}");

    let found = changes_from(&[
        ("system.yaml", SYSTEM),
        ("domains/orders.yaml", domain),
        ("domains/orders-actors.yaml", &moved),
    ]);

    assert!(
        found.is_empty(),
        "moving a declaration to another file changed the system: {found:?}"
    );
}

// ---- the four families W7.2 joined -------------------------------------------------------------
//
// The same discipline as above — one edit, and an assertion on exactly what the delta says about
// it — over the entity, command, view and binding families. Where one document edit necessarily
// moves two constructs (a transition needs an outcome to take it; a view's fields have to agree
// with its source), the test asserts membership of each expected row rather than a count of one.

/// The ids of every change an edit produced, for the tests that assert membership.
fn ids_of(found: &[SemanticChange]) -> Vec<String> {
    found.iter().map(|change| change.id().to_string()).collect()
}

// ---- entities ----------------------------------------------------------------------------------

#[test]
fn an_entity_added_arrives_with_its_synthesised_state_enum_and_nothing_is_diffed_inside() {
    let found = changes(
        SYSTEM,
        &edited(
            "views:\n",
            "  - name: witness.orders.Memo\n    identity:\n      name: order_id\n        \
             type: witness.orders.OrderId\n    lifecycle:\n      initial: Open\n      \
             states: [Open]\n      terminal: [Open]\n\nviews:\n"
                .replace("order_id\n        type", "order_id\n      type")
                .as_str(),
        ),
    );

    let ids = ids_of(&found);
    assert!(
        ids.contains(&"entity/witness.orders.Memo/added".to_owned()),
        "{ids:?}"
    );
    assert!(
        ids.contains(&"type/witness.orders.Memo.State/added".to_owned()),
        "the lifecycle's state set arrives as the synthesised enum, in the type family: {ids:?}"
    );
    assert_eq!(
        found.len(),
        2,
        "added whole, never diffed against nothing: {ids:?}"
    );
}

#[test]
fn renaming_an_entitys_identity_is_the_one_rename_this_crate_reports() {
    // An entity has exactly one identity, so "which declaration is this" has no room for the
    // ambiguity the no-rename rule refuses — and the wire name moves with it, because the model
    // falls back to the field's own name. The view that projects the identity has to follow
    // (`ess-domain` refuses a projection of a field the source does not have), which is why this
    // edit is a membership check and not a `one()`.
    let domain = edited(
        "    identity:\n      name: order_id\n      type: witness.orders.OrderId\n    fields:",
        "    identity:\n      name: order_ref\n      type: witness.orders.OrderId\n    fields:",
    )
    .replace(
        "    filter: state == Open\n    fields:\n      - name: order_id",
        "    filter: state == Open\n    fields:\n      - name: order_ref",
    );
    let found = changes(SYSTEM, &domain);

    let ids = ids_of(&found);
    assert!(
        ids.contains(&"entity/witness.orders.Order/identity-renamed".to_owned()),
        "{ids:?}"
    );
    assert!(
        ids.contains(&"entity/witness.orders.Order/identity-wire-name-changed".to_owned()),
        "the default wire name follows the field's own name: {ids:?}"
    );
    assert!(
        !ids.iter()
            .any(|id| id == "entity/witness.orders.Order/removed"),
        "the entity is the same entity: {ids:?}"
    );
}

#[test]
fn an_identitys_display_name_and_summary_are_compared() {
    let found = changes(
        SYSTEM,
        &edited(
            "    identity:\n      name: order_id\n      type: witness.orders.OrderId\n    fields:",
            "    identity:\n      name: order_id\n      type: witness.orders.OrderId\n      \
             display: Order number\n      summary: Which order this is.\n    fields:",
        ),
    );

    let ids = ids_of(&found);
    assert_eq!(
        ids,
        vec![
            "entity/witness.orders.Order/identity-display-name-changed".to_owned(),
            "entity/witness.orders.Order/identity-summary-changed".to_owned(),
        ]
    );
}

#[test]
fn an_entity_field_that_changed_type_is_reported() {
    let change = one(
        SYSTEM,
        &edited(
            "      - name: channel\n        type: witness.orders.Channel\n    invariants:",
            "      - name: channel\n        type: String\n    invariants:",
        ),
    );

    let SemanticChange::Entity { subject, changed } = &change else {
        panic!("this is {change:?}");
    };
    assert_eq!(subject.to_string(), "witness.orders.Order");
    assert_eq!(
        changed,
        &EntityChange::FieldTypeChanged {
            field: "channel".to_owned(),
            before: "witness.orders.Channel".to_owned(),
            after: "String".to_owned(),
        }
    );
    assert_eq!(change.relation(), SemanticRelation::Changed);
}

#[test]
fn an_entity_field_replaced_is_removed_and_added_and_never_a_rename() {
    let found = changes(
        SYSTEM,
        &edited(
            "      - name: payee\n        type: witness.orders.Payee\n",
            "      - name: flag\n        type: witness.orders.Channel\n",
        ),
    );

    let ids = ids_of(&found);
    assert_eq!(
        ids,
        vec![
            "entity/witness.orders.Order/field-added/flag".to_owned(),
            "entity/witness.orders.Order/field-removed/payee".to_owned(),
        ]
    );
}

#[test]
fn reordering_an_entitys_fields_is_reported_once() {
    let change = one(
        SYSTEM,
        &edited(
            "      - name: payee\n        type: witness.orders.Payee\n      - name: channel\n        type: witness.orders.Channel\n",
            "      - name: channel\n        type: witness.orders.Channel\n      - name: payee\n        type: witness.orders.Payee\n",
        ),
    );

    let SemanticChange::Entity { changed, .. } = &change else {
        panic!("this is {change:?}");
    };
    assert_eq!(
        changed,
        &EntityChange::FieldOrderChanged {
            before: vec!["total".to_owned(), "payee".to_owned(), "channel".to_owned()],
            after: vec!["total".to_owned(), "channel".to_owned(), "payee".to_owned()],
        }
    );
}

#[test]
fn an_entity_fields_naming_is_compared_key_by_key() {
    let found = changes(
        SYSTEM,
        &edited(
            "      - name: channel\n        type: witness.orders.Channel\n    invariants:",
            "      - name: channel\n        type: witness.orders.Channel\n        wire: chan\n        \
             display: Channel of record\n        summary: How it ships.\n    invariants:",
        ),
    );

    let ids = ids_of(&found);
    assert_eq!(
        ids,
        vec![
            "entity/witness.orders.Order/field-display-name-changed/channel".to_owned(),
            "entity/witness.orders.Order/field-summary-changed/channel".to_owned(),
            "entity/witness.orders.Order/field-wire-name-changed/channel".to_owned(),
        ]
    );
}

#[test]
fn a_new_transition_arrives_with_the_outcome_that_takes_it() {
    // A transition nothing takes is refused by `ess-domain` (`missing_causation`), so a lifecycle
    // edit is never alone in a valid pair: the move arrives in the entity family and the branch
    // that takes it moves in the command family. Two families, one edit, both named.
    let domain = edited(
        "        - name: close\n          from: [Open]\n          to: Closed\n",
        "        - name: close\n          from: [Open]\n          to: Closed\n        - name: hold\n          from: [Open]\n          to: Open\n",
    )
    .replace(
        "      - name: reopened\n        when: amount.amount > 0\n        emits:",
        "      - name: reopened\n        when: amount.amount > 0\n        moves: witness.orders.Order.hold\n        instance: order_id\n        emits:",
    );
    let found = changes(SYSTEM, &domain);

    let ids = ids_of(&found);
    assert_eq!(
        ids,
        vec![
            "entity/witness.orders.Order/transition-added/hold".to_owned(),
            "command/witness.orders.ReopenOrder/outcome-subject-changed/reopened".to_owned(),
        ]
    );
    let subject_change = &found[1];
    let SemanticChange::Command { changed, .. } = subject_change else {
        panic!("this is {subject_change:?}");
    };
    let CommandChange::OutcomeSubjectChanged { before, after, .. } = changed else {
        panic!("this is {changed:?}");
    };
    assert_eq!(
        before.as_deref(),
        None,
        "the branch changed no entity before"
    );
    assert_eq!(
        after.as_deref(),
        Some("moves witness.orders.Order via hold (Open -> Open), instance from input `order_id`"),
        "the rendering carries every part that decides equality"
    );
}

#[test]
fn an_invariant_statement_reworded_without_moving_the_predicate_is_still_a_change() {
    // The conservative half of the canonical form, and the asymmetry is deliberate: an entity
    // invariant keeps the author's own statement beside the parsed predicate, a documentation
    // projection quotes the statement, so a respaced statement is a model that moved — reported,
    // at the cost of a re-run nobody strictly needed. Compare
    // `a_guard_respaced_is_the_same_predicate_and_no_change`, where the model keeps only the
    // predicate and the same respacing is silence.
    let change = one(
        SYSTEM,
        &edited("      - total.amount >= 0", "      - total.amount  >=  0"),
    );

    let SemanticChange::Entity { changed, .. } = &change else {
        panic!("this is {change:?}");
    };
    assert_eq!(
        changed,
        &EntityChange::InvariantsChanged {
            before: vec!["total.amount >= 0".to_owned()],
            after: vec!["total.amount  >=  0".to_owned()],
        }
    );
    assert_eq!(change.relation(), SemanticRelation::Changed);
}

#[test]
fn an_entitys_naming_is_compared_key_by_key() {
    let found = changes(
        SYSTEM,
        &edited(
            "  - name: witness.orders.Order\n    identity:",
            "  - name: witness.orders.Order\n    naming:\n      wire: order.v2\n      display: An order\n      summary: What a customer asked for.\n    identity:",
        ),
    );

    let ids = ids_of(&found);
    assert_eq!(
        ids,
        vec![
            "entity/witness.orders.Order/display-name-changed".to_owned(),
            "entity/witness.orders.Order/summary-changed".to_owned(),
            "entity/witness.orders.Order/wire-name-changed".to_owned(),
        ]
    );
}

// ---- commands ----------------------------------------------------------------------------------

#[test]
fn a_guard_respaced_is_the_same_predicate_and_no_change() {
    // The decidable half of gap register D-1, on the construct that motivated it. The IR keeps no
    // author spelling for a `when:` — only the parsed predicate — so two spellings the parser
    // normalises to one AST are one guard, and the delta says nothing. This is the exact edit that
    // would have owed every scenario and every artifact under wave 5's fail-closed catch-all.
    let found = changes(
        SYSTEM,
        &edited("when: amount.amount > 0", "when: amount.amount  >  0"),
    );

    assert!(
        found.is_empty(),
        "a respaced guard is the same canonical predicate, and reporting it would make the delta \
         fire on formatting: {found:?}"
    );
}

#[test]
fn a_command_added_is_one_change() {
    let change = one(
        SYSTEM,
        &edited(
            "events:\n",
            "  - name: witness.orders.TagOrder\n    input:\n      - name: order_id\n        \
             type: witness.orders.OrderId\n    outcomes:\n      - name: tagged\n        emits:\n          \
             - witness.orders.OrderReopened\n\nevents:\n",
        ),
    );

    let SemanticChange::Command { subject, changed } = &change else {
        panic!("this is {change:?}");
    };
    assert_eq!(subject.to_string(), "witness.orders.TagOrder");
    assert_eq!(changed, &CommandChange::Added);
    assert_eq!(
        change.relation(),
        SemanticRelation::Changed,
        "a command arriving grants nothing by itself; whether anyone may invoke it is the actors'"
    );
}

#[test]
fn an_input_added_is_reported_with_the_type_it_carries() {
    // `Optional`, deliberately: the binding that invokes this command maps every required input,
    // and an optional input is the one kind a binding may leave unmapped — so the edit is one
    // change rather than an input plus the mapping that has to follow.
    let change = one(
        SYSTEM,
        &edited(
            "      - name: note\n        type: String\n    outcomes:",
            "      - name: note\n        type: String\n      - name: flag\n        type: Optional<String>\n    outcomes:",
        ),
    );

    let SemanticChange::Command { subject, changed } = &change else {
        panic!("this is {change:?}");
    };
    assert_eq!(subject.to_string(), "witness.orders.CloseOrder");
    assert_eq!(
        changed,
        &CommandChange::InputAdded {
            field: "flag".to_owned(),
            type_ref: "Optional<String>".to_owned(),
        }
    );
}

#[test]
fn an_input_that_changed_type_is_reported() {
    let change = one(
        SYSTEM,
        &edited(
            "    input:\n      - name: order_id\n        type: witness.orders.OrderId\n      - name: amount",
            "    input:\n      - name: order_id\n        type: String\n      - name: amount",
        ),
    );

    let SemanticChange::Command { subject, changed } = &change else {
        panic!("this is {change:?}");
    };
    assert_eq!(subject.to_string(), "witness.orders.ReopenOrder");
    assert_eq!(
        changed,
        &CommandChange::InputTypeChanged {
            field: "order_id".to_owned(),
            before: "witness.orders.OrderId".to_owned(),
            after: "String".to_owned(),
        }
    );
}

#[test]
fn reordering_a_commands_input_is_reported_once() {
    let change = one(
        SYSTEM,
        &edited(
            "    input:\n      - name: order_id\n        type: witness.orders.OrderId\n      - name: note\n        type: String\n",
            "    input:\n      - name: note\n        type: String\n      - name: order_id\n        type: witness.orders.OrderId\n",
        ),
    );

    let SemanticChange::Command { subject, changed } = &change else {
        panic!("this is {change:?}");
    };
    assert_eq!(subject.to_string(), "witness.orders.CloseOrder");
    assert_eq!(
        changed,
        &CommandChange::InputOrderChanged {
            before: vec!["order_id".to_owned(), "note".to_owned()],
            after: vec!["note".to_owned(), "order_id".to_owned()],
        }
    );
}

#[test]
fn an_input_fields_naming_is_compared_key_by_key() {
    let found = changes(
        SYSTEM,
        &edited(
            "      - name: amount\n        type: witness.orders.Money\n    outcomes:",
            "      - name: amount\n        type: witness.orders.Money\n        wire: amt\n        \
             display: Amount\n        summary: What it costs.\n    outcomes:",
        ),
    );

    let ids = ids_of(&found);
    assert_eq!(
        ids,
        vec![
            "command/witness.orders.ReopenOrder/input-display-name-changed/amount".to_owned(),
            "command/witness.orders.ReopenOrder/input-summary-changed/amount".to_owned(),
            "command/witness.orders.ReopenOrder/input-wire-name-changed/amount".to_owned(),
        ]
    );
}

#[test]
fn an_outcome_added_is_one_change_and_claims_no_direction() {
    let change = one(
        SYSTEM,
        &edited(
            "      - name: skipped\n        error: witness.orders.OrderLocked\n",
            "      - name: boosted\n        when: amount.amount > 100\n        emits:\n          \
             - witness.orders.OrderReopened\n\n      - name: skipped\n        error: witness.orders.OrderLocked\n",
        ),
    );

    let SemanticChange::Command { changed, .. } = &change else {
        panic!("this is {change:?}");
    };
    assert_eq!(
        changed,
        &CommandChange::OutcomeAdded {
            outcome: "boosted".to_owned(),
        }
    );
    assert_eq!(
        change.relation(),
        SemanticRelation::Changed,
        "whether callers can reach a new branch depends on every other guard, which is the proof \
         this slice refuses"
    );
}

#[test]
fn what_an_outcome_emits_is_compared_in_order() {
    let change = one(
        SYSTEM,
        &edited(
            "        emits:\n          - witness.orders.OrderReopened\n\n      - name: skipped",
            "        emits:\n          - witness.orders.OrderReopened\n          - witness.orders.OrderClosed\n\n      - name: skipped",
        ),
    );

    let SemanticChange::Command { changed, .. } = &change else {
        panic!("this is {change:?}");
    };
    assert_eq!(
        changed,
        &CommandChange::OutcomeEmitsChanged {
            outcome: "reopened".to_owned(),
            before: vec!["witness.orders.OrderReopened".to_owned()],
            after: vec![
                "witness.orders.OrderReopened".to_owned(),
                "witness.orders.OrderClosed".to_owned()
            ],
        }
    );
}

#[test]
fn a_payload_declaration_arriving_is_a_payload_change() {
    let change = one(
        SYSTEM,
        &edited(
            "        emits:\n          - witness.orders.OrderReopened\n\n      - name: skipped",
            "        emits:\n          - witness.orders.OrderReopened\n        payload:\n          \
             witness.orders.OrderReopened:\n            order_id: input.order_id\n\n      - name: skipped",
        ),
    );

    let SemanticChange::Command { changed, .. } = &change else {
        panic!("this is {change:?}");
    };
    assert_eq!(
        changed,
        &CommandChange::OutcomePayloadChanged {
            outcome: "reopened".to_owned(),
            before: vec![],
            after: vec!["witness.orders.OrderReopened.order_id <- input.order_id".to_owned()],
        }
    );
}

#[test]
fn the_error_a_branch_reports_is_compared() {
    let change = one(
        SYSTEM,
        &edited(
            "      - name: skipped\n        error: witness.orders.OrderLocked",
            "      - name: skipped\n        error: witness.orders.OrderStateConflict",
        ),
    );

    let SemanticChange::Command { changed, .. } = &change else {
        panic!("this is {change:?}");
    };
    assert_eq!(
        changed,
        &CommandChange::OutcomeErrorChanged {
            outcome: "skipped".to_owned(),
            before: Some("witness.orders.OrderLocked".to_owned()),
            after: Some("witness.orders.OrderStateConflict".to_owned()),
        }
    );
}

#[test]
fn an_outcomes_summary_is_compared() {
    let change = one(
        SYSTEM,
        &edited(
            "        summary: Nothing was reopened.",
            "        summary: The order stays as it is.",
        ),
    );

    let SemanticChange::Command { changed, .. } = &change else {
        panic!("this is {change:?}");
    };
    assert_eq!(
        changed,
        &CommandChange::OutcomeSummaryChanged {
            outcome: "skipped".to_owned(),
            before: Some("Nothing was reopened.".to_owned()),
            after: Some("The order stays as it is.".to_owned()),
        }
    );
}

#[test]
fn reordering_a_commands_outcomes_is_a_real_change() {
    // Conditional outcomes are decided in declaration order, so two branches swapped can take
    // different inputs to different branches — reported once, over the branches both sides
    // declare, and with no direction.
    let change = one(
        SYSTEM,
        &edited(
            "      - name: reopened\n        when: amount.amount > 0\n        emits:\n          - witness.orders.OrderReopened\n\n      - name: skipped\n        error: witness.orders.OrderLocked\n        summary: Nothing was reopened.\n",
            "      - name: skipped\n        error: witness.orders.OrderLocked\n        summary: Nothing was reopened.\n\n      - name: reopened\n        when: amount.amount > 0\n        emits:\n          - witness.orders.OrderReopened\n",
        ),
    );

    let SemanticChange::Command { changed, .. } = &change else {
        panic!("this is {change:?}");
    };
    assert_eq!(
        changed,
        &CommandChange::OutcomeOrderChanged {
            before: vec!["reopened".to_owned(), "skipped".to_owned()],
            after: vec!["skipped".to_owned(), "reopened".to_owned()],
        }
    );
}

#[test]
fn a_commands_naming_is_compared_key_by_key() {
    let found = changes(
        SYSTEM,
        &edited(
            "  - name: witness.orders.ReopenOrder\n    input:",
            "  - name: witness.orders.ReopenOrder\n    naming:\n      wire: reopen-order\n      \
             display: Reopen order\n      summary: Puts a closed order back to work.\n    input:",
        ),
    );

    let ids = ids_of(&found);
    assert_eq!(
        ids,
        vec![
            "command/witness.orders.ReopenOrder/display-name-changed".to_owned(),
            "command/witness.orders.ReopenOrder/summary-changed".to_owned(),
            "command/witness.orders.ReopenOrder/wire-name-changed".to_owned(),
        ]
    );
}

// ---- views -------------------------------------------------------------------------------------

#[test]
fn a_view_added_is_one_change() {
    let change = one(
        SYSTEM,
        &edited(
            "errors:\n",
            "  - name: witness.orders.TicketById\n    source: witness.orders.Ticket\n    \
             consistency: eventual\n    fields:\n      - name: order_id\n        \
             type: witness.orders.OrderId\n\nerrors:\n",
        ),
    );

    let SemanticChange::View { subject, changed } = &change else {
        panic!("this is {change:?}");
    };
    assert_eq!(subject.to_string(), "witness.orders.TicketById");
    assert_eq!(changed, &ViewChange::Added);
}

#[test]
fn a_view_projecting_a_different_entity_is_a_source_change() {
    // The view's fields have to agree with what its source observes, so pointing it at the ticket
    // drops the field the ticket does not have — two rows, both named, neither a rename.
    let found = changes(
        SYSTEM,
        &edited(
            "    source: witness.orders.Order\n    consistency: eventual\n    filter: state == Open\n    fields:\n      - name: order_id\n        type: witness.orders.OrderId\n      - name: total\n        type: witness.orders.Money\n",
            "    source: witness.orders.Ticket\n    consistency: eventual\n    filter: state == Open\n    fields:\n      - name: order_id\n        type: witness.orders.OrderId\n",
        ),
    );

    let ids = ids_of(&found);
    assert_eq!(
        ids,
        vec![
            "view/witness.orders.OrderById/field-removed/total".to_owned(),
            "view/witness.orders.OrderById/source-changed".to_owned(),
        ]
    );
}

#[test]
fn a_view_exposing_a_new_field_is_reported_with_the_type_it_carries() {
    let change = one(
        SYSTEM,
        &edited(
            "      - name: total\n        type: witness.orders.Money\n\nerrors:",
            "      - name: total\n        type: witness.orders.Money\n      - name: channel\n        type: witness.orders.Channel\n\nerrors:",
        ),
    );

    let SemanticChange::View { subject, changed } = &change else {
        panic!("this is {change:?}");
    };
    assert_eq!(subject.to_string(), "witness.orders.OrderById");
    assert_eq!(
        changed,
        &ViewChange::FieldAdded {
            field: "channel".to_owned(),
            type_ref: "witness.orders.Channel".to_owned(),
        }
    );
}

#[test]
fn a_filter_respaced_is_the_same_predicate_and_no_change() {
    // D-1's decidable half again, on the third predicate-bearing construct: the model keeps a
    // filter as a parsed predicate, so a respaced filter is the same canonical form.
    let found = changes(
        SYSTEM,
        &edited("filter: state == Open", "filter: state  ==  Open"),
    );

    assert!(
        found.is_empty(),
        "a respaced filter is the same canonical predicate: {found:?}"
    );
}

#[test]
fn a_filter_that_contains_different_instances_is_changed_with_no_direction() {
    let change = one(
        SYSTEM,
        &edited("filter: state == Open", "filter: state == Closed"),
    );

    let SemanticChange::View { subject, changed } = &change else {
        panic!("this is {change:?}");
    };
    assert_eq!(subject.to_string(), "witness.orders.OrderById");
    assert_eq!(
        changed,
        &ViewChange::FilterChanged {
            before: Some("state == Open".to_owned()),
            after: Some("state == Closed".to_owned()),
        }
    );
    assert_eq!(
        change.relation(),
        SemanticRelation::Changed,
        "whether the new filter admits more instances or fewer is implication, which is refused"
    );
}

#[test]
fn a_filter_removed_reads_as_containing_every_instance() {
    let change = one(SYSTEM, &edited("    filter: state == Open\n", ""));

    let SemanticChange::View { changed, .. } = &change else {
        panic!("this is {change:?}");
    };
    assert_eq!(
        changed,
        &ViewChange::FilterChanged {
            before: Some("state == Open".to_owned()),
            after: None,
        }
    );
    assert!(
        change.describe().contains("every instance"),
        "`None` renders as what it means: {}",
        change.describe()
    );
}

#[test]
fn a_views_consistency_promise_is_compared_and_not_classified() {
    let change = one(
        SYSTEM,
        &edited(
            "    source: witness.orders.Order\n    consistency: eventual",
            "    source: witness.orders.Order\n    consistency: read_your_writes",
        ),
    );

    let SemanticChange::View { changed, .. } = &change else {
        panic!("this is {change:?}");
    };
    assert_eq!(
        changed,
        &ViewChange::ConsistencyChanged {
            before: "eventual".to_owned(),
            after: "read_your_writes".to_owned(),
        }
    );
    assert_eq!(
        change.relation(),
        SemanticRelation::Changed,
        "read-your-writes is strictly the stronger promise, and it is still only `changed`: the \
         directional relations are about what the system permits, not about when a read holds"
    );
}

#[test]
fn reordering_a_views_fields_is_reported_once() {
    let change = one(
        SYSTEM,
        &edited(
            "    filter: state == Open\n    fields:\n      - name: order_id\n        type: witness.orders.OrderId\n      - name: total\n        type: witness.orders.Money\n",
            "    filter: state == Open\n    fields:\n      - name: total\n        type: witness.orders.Money\n      - name: order_id\n        type: witness.orders.OrderId\n",
        ),
    );

    let SemanticChange::View { changed, .. } = &change else {
        panic!("this is {change:?}");
    };
    assert_eq!(
        changed,
        &ViewChange::FieldOrderChanged {
            before: vec!["order_id".to_owned(), "total".to_owned()],
            after: vec!["total".to_owned(), "order_id".to_owned()],
        }
    );
}

#[test]
fn a_view_fields_naming_is_compared_key_by_key() {
    let found = changes(
        SYSTEM,
        &edited(
            "      - name: total\n        type: witness.orders.Money\n\nerrors:",
            "      - name: total\n        type: witness.orders.Money\n        wire: amount_due\n        \
             display: Total due\n        summary: What is owed.\n\nerrors:",
        ),
    );

    let ids = ids_of(&found);
    assert_eq!(
        ids,
        vec![
            "view/witness.orders.OrderById/field-display-name-changed/total".to_owned(),
            "view/witness.orders.OrderById/field-summary-changed/total".to_owned(),
            "view/witness.orders.OrderById/field-wire-name-changed/total".to_owned(),
        ]
    );
}

#[test]
fn a_views_naming_is_compared_key_by_key() {
    let found = changes(
        SYSTEM,
        &edited(
            "  - name: witness.orders.OrderById\n    source:",
            "  - name: witness.orders.OrderById\n    naming:\n      wire: order-by-id\n      \
             display: Order by id\n      summary: One order, looked up.\n    source:",
        ),
    );

    let ids = ids_of(&found);
    assert_eq!(
        ids,
        vec![
            "view/witness.orders.OrderById/display-name-changed".to_owned(),
            "view/witness.orders.OrderById/summary-changed".to_owned(),
            "view/witness.orders.OrderById/wire-name-changed".to_owned(),
        ]
    );
}

// ---- bindings ----------------------------------------------------------------------------------

/// The witness system with one substring replaced, refusing an edit that matched nothing.
fn edited_system(from: &str, to: &str) -> String {
    assert!(
        SYSTEM.contains(from),
        "the witness system does not contain `{from}`, so this edit would compare a document with \
         itself"
    );
    SYSTEM.replace(from, to)
}

#[test]
fn a_binding_added_is_one_change() {
    let change = {
        let found = changes(
            &edited_system(
                "bindings:\n",
                "bindings:\n  - id: note-on-reopen\n    when:\n      event: witness.orders.OrderReopened\n    \
                 invoke:\n      command: witness.orders.CloseOrder\n    mapping:\n      \
                 order_id: event.order_id\n      note: reopened-note\n    delivery: at_least_once\n    \
                 on_failure: retry\n\n",
            ),
            DOMAIN,
        );
        assert_eq!(found.len(), 1, "{found:?}");
        found.into_iter().next().expect("one")
    };

    let SemanticChange::Binding { subject, changed } = &change else {
        panic!("this is {change:?}");
    };
    assert_eq!(subject.to_string(), "note-on-reopen");
    assert_eq!(changed, &BindingChange::Added);
}

#[test]
fn a_binding_reacting_to_a_different_event_is_reported() {
    let change = {
        let found = changes(
            &edited_system(
                "      event: witness.orders.OrderClosed",
                "      event: witness.orders.OrderReopened",
            ),
            DOMAIN,
        );
        assert_eq!(found.len(), 1, "{found:?}");
        found.into_iter().next().expect("one")
    };

    let SemanticChange::Binding { subject, changed } = &change else {
        panic!("this is {change:?}");
    };
    assert_eq!(subject.to_string(), "note-on-close");
    assert_eq!(
        changed,
        &BindingChange::EventChanged {
            before: "witness.orders.OrderClosed".parse().expect("an event name"),
            after: "witness.orders.OrderReopened"
                .parse()
                .expect("an event name"),
        }
    );
}

#[test]
fn a_binding_invoking_a_different_command_moves_its_mapping_with_it() {
    // The invoked command decides which inputs exist to fill, so the command cannot move alone:
    // the mapping follows, and every part of the edit is named.
    let found = changes(
        &edited_system(
            "    invoke:\n      command: witness.orders.CloseOrder\n    mapping:\n      order_id: event.order_id\n      note: closed-again\n",
            "    invoke:\n      command: witness.orders.ReopenOrder\n    mapping:\n      order_id: event.order_id\n      amount: event.total\n",
        ),
        DOMAIN,
    );

    let ids = ids_of(&found);
    assert_eq!(
        ids,
        vec![
            "binding/note-on-close/command-changed".to_owned(),
            "binding/note-on-close/mapping-added/amount".to_owned(),
            "binding/note-on-close/mapping-removed/note".to_owned(),
        ]
    );
}

#[test]
fn a_mapping_filled_from_somewhere_else_is_reported_with_both_sources() {
    let change = {
        let found = changes(
            &edited_system("      note: closed-again", "      note: closed-later"),
            DOMAIN,
        );
        assert_eq!(found.len(), 1, "{found:?}");
        found.into_iter().next().expect("one")
    };

    let SemanticChange::Binding { changed, .. } = &change else {
        panic!("this is {change:?}");
    };
    assert_eq!(
        changed,
        &BindingChange::MappingValueChanged {
            target: "note".to_owned(),
            before: "literal `closed-again`".to_owned(),
            after: "literal `closed-later`".to_owned(),
        }
    );
}

#[test]
fn a_bindings_failure_policy_is_compared() {
    let change = {
        let found = changes(
            &edited_system("    on_failure: retry", "    on_failure: drop"),
            DOMAIN,
        );
        assert_eq!(found.len(), 1, "{found:?}");
        found.into_iter().next().expect("one")
    };

    let SemanticChange::Binding { changed, .. } = &change else {
        panic!("this is {change:?}");
    };
    assert_eq!(
        changed,
        &BindingChange::FailureChanged {
            before: "retry".to_owned(),
            after: "drop".to_owned(),
        }
    );
}

#[test]
fn a_bindings_naming_is_compared_key_by_key() {
    let found = changes(
        &edited_system(
            "  - id: note-on-close\n    summary: Stamp a note on an order the moment it closes.\n",
            "  - id: note-on-close\n    naming:\n      wire: note-on-close-v2\n      display: Note on close\n    \
             summary: Stamps a note the moment an order closes.\n",
        ),
        DOMAIN,
    );

    let ids = ids_of(&found);
    assert_eq!(
        ids,
        vec![
            "binding/note-on-close/display-name-changed".to_owned(),
            "binding/note-on-close/summary-changed".to_owned(),
            "binding/note-on-close/wire-name-changed".to_owned(),
        ]
    );
}
