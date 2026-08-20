//! One mutation at a time, over the four families the fixture pair does not move.
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
    ActorChange, ComponentChange, ErrorChange, EventChange, SemanticChange, SystemChange,
    TypeChange,
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
    lifecycle:
      initial: Open
      states: [Open, Closed]
      terminal: [Closed]
      transitions:
        - name: close
          from: [Open]
          to: Closed

errors:
  - name: witness.orders.OrderStateConflict
    summary: The order is not in a state this command acts from.
    fields:
      - name: state
        type: witness.orders.Order.State

commands:
  - name: witness.orders.CloseOrder
    input:
      - name: order_id
        type: witness.orders.OrderId
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
    outcomes:
      - name: reopened
        emits:
          - witness.orders.OrderReopened

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
