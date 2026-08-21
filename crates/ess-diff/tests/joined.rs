//! The corners of the four W7.2 families a document cannot reach with one edit.
//!
//! `tests/families.rs` proves every change kind a YAML edit can produce, the way an author would
//! produce it. What is left is the set of edits `ess-domain` refuses to let a *single* document
//! edit make — moving a construct between bounded contexts, changing an identity's type under the
//! commands that supply it, rewriting a lifecycle out from under its causation — so each test here
//! compiles the billing example once and makes the one surgical mutation on the resolved IR
//! instead, exactly as `tests/impact.rs` reaches its fail-closed arms. The comparison's answer is
//! still asserted by name, change for change: the point is that every declared change kind is one
//! some pair of models can actually produce, not a variant waiting for a construct that never
//! comes.

mod support;

use ess_compiler::ir::{EssIr, ResolvedTypeRef};
use ess_diff::change::{BindingChange, CommandChange, EntityChange, SemanticChange, ViewChange};
use ess_diff::diff;
use ess_domain::name::QualifiedName;
use support::compiled;

/// A qualified name, for the lookups.
fn name(value: &str) -> QualifiedName {
    QualifiedName::new(value).expect("a valid qualified name")
}

/// The billing model, and a copy to mutate.
fn pair() -> (EssIr, EssIr) {
    let before = compiled("examples/billing");
    let after = before.clone();
    (before, after)
}

/// The ids of every change a mutation produced.
fn ids(before: &EssIr, after: &EssIr) -> Vec<String> {
    diff(before, after)
        .expect("one system")
        .changes()
        .iter()
        .map(|change| change.id().to_string())
        .collect()
}

#[test]
fn a_construct_removed_is_reported_in_its_own_family_and_never_diffed_against_nothing() {
    let (before, mut after) = pair();
    after.entities.remove(&name("billing.invoice.Invoice"));
    after.commands.remove(&name("billing.email.SendEmail"));
    after.views.remove(&name("billing.invoice.InvoiceById"));
    let binding = after.bindings.keys().next().expect("a binding").clone();
    after.bindings.remove(&binding);

    let found = ids(&before, &after);

    for expected in [
        "entity/billing.invoice.Invoice/removed",
        "command/billing.email.SendEmail/removed",
        "view/billing.invoice.InvoiceById/removed",
        "binding/notify-on-invoice-created/removed",
    ] {
        assert!(found.contains(&expected.to_owned()), "{found:?}");
    }
    assert_eq!(
        found.len(),
        4,
        "each construct is removed whole — nothing inside one is diffed against nothing: {found:?}"
    );
}

#[test]
fn a_construct_moved_to_another_bounded_context_is_a_domain_change() {
    // No single document edit can produce this — a construct's domain is the file that declares
    // it, and moving the declaration moves everything at once — which is exactly why the change
    // kind exists: a merged or split context arrives as *domain-changed* per construct, not as a
    // wall of removals.
    let (before, mut after) = pair();
    let email = after
        .commands
        .get(&name("billing.email.SendEmail"))
        .expect("the fixture declares it")
        .domain
        .clone();
    after
        .entities
        .get_mut(&name("billing.invoice.Invoice"))
        .expect("the fixture declares it")
        .domain = email.clone();
    after
        .commands
        .get_mut(&name("billing.invoice.CreateInvoice"))
        .expect("the fixture declares it")
        .domain = email.clone();
    after
        .views
        .get_mut(&name("billing.invoice.InvoiceById"))
        .expect("the fixture declares it")
        .domain = email;

    let found = ids(&before, &after);

    assert_eq!(
        found,
        vec![
            "entity/billing.invoice.Invoice/domain-changed".to_owned(),
            "command/billing.invoice.CreateInvoice/domain-changed".to_owned(),
            "view/billing.invoice.InvoiceById/domain-changed".to_owned(),
        ]
    );
}

#[test]
fn an_identity_that_changed_type_is_reported() {
    let (before, mut after) = pair();
    after
        .entities
        .get_mut(&name("billing.invoice.Invoice"))
        .expect("the fixture declares it")
        .identity
        .type_ref = ResolvedTypeRef::Primitive {
        name: ess_domain::types::Primitive::String,
    };

    let delta = diff(&before, &after).expect("one system");
    assert_eq!(delta.len(), 1, "{:?}", ids(&before, &after));
    let SemanticChange::Entity { changed, .. } = &delta.changes()[0] else {
        panic!("this is {:?}", delta.changes()[0]);
    };
    assert_eq!(
        changed,
        &EntityChange::IdentityTypeChanged {
            before: "billing.invoice.InvoiceId".to_owned(),
            after: "String".to_owned(),
        }
    );
}

#[test]
fn a_lifecycle_that_starts_somewhere_else_is_reported() {
    let (before, mut after) = pair();
    after
        .entities
        .get_mut(&name("billing.invoice.Invoice"))
        .expect("the fixture declares it")
        .lifecycle
        .initial = ess_domain::entity::StateName::new("Issued").expect("a state name");

    let delta = diff(&before, &after).expect("one system");
    let SemanticChange::Entity { changed, .. } = &delta.changes()[0] else {
        panic!("this is {:?}", delta.changes()[0]);
    };
    assert_eq!(
        changed,
        &EntityChange::InitialStateChanged {
            before: "Draft".to_owned(),
            after: "Issued".to_owned(),
        }
    );
}

#[test]
fn where_an_entity_may_rest_forever_is_compared_as_a_set() {
    let (before, mut after) = pair();
    let lifecycle = &mut after
        .entities
        .get_mut(&name("billing.invoice.Invoice"))
        .expect("the fixture declares it")
        .lifecycle;
    let cancelled = ess_domain::entity::StateName::new("Cancelled").expect("a state name");
    let issued = ess_domain::entity::StateName::new("Issued").expect("a state name");
    assert!(
        lifecycle.terminal.remove(&cancelled),
        "the fixture rests there"
    );
    assert!(lifecycle.terminal.insert(issued), "the fixture did not");

    let found = ids(&before, &after);

    assert_eq!(
        found,
        vec![
            "entity/billing.invoice.Invoice/terminal-added/Issued".to_owned(),
            "entity/billing.invoice.Invoice/terminal-removed/Cancelled".to_owned(),
        ]
    );
}

#[test]
fn a_transition_that_starts_somewhere_else_is_a_route_change_and_not_a_replacement() {
    // A transition is identified by its own name inside the lifecycle, so `cancel` losing one of
    // its two starting states is `cancel` having moved — not one move removed and another added,
    // which would tell a reader the vocabulary changed.
    let (before, mut after) = pair();
    let lifecycle = &mut after
        .entities
        .get_mut(&name("billing.invoice.Invoice"))
        .expect("the fixture declares it")
        .lifecycle;
    let cancel = lifecycle
        .transitions
        .iter_mut()
        .find(|transition| transition.name == "cancel")
        .expect("the fixture declares it");
    let issued = ess_domain::entity::StateName::new("Issued").expect("a state name");
    assert!(cancel.from.remove(&issued), "the fixture starts there");

    let delta = diff(&before, &after).expect("one system");
    let SemanticChange::Entity { changed, .. } = &delta.changes()[0] else {
        panic!("this is {:?}", delta.changes()[0]);
    };
    assert_eq!(
        changed,
        &EntityChange::TransitionRouteChanged {
            transition: "cancel".to_owned(),
            before: "Draft, Issued -> Cancelled".to_owned(),
            after: "Draft -> Cancelled".to_owned(),
        }
    );
}

#[test]
fn a_transition_removed_is_reported_by_its_own_name() {
    let (before, mut after) = pair();
    after
        .entities
        .get_mut(&name("billing.invoice.Invoice"))
        .expect("the fixture declares it")
        .lifecycle
        .transitions
        .retain(|transition| transition.name != "cancel");

    let found = ids(&before, &after);

    assert_eq!(
        found,
        vec!["entity/billing.invoice.Invoice/transition-removed/cancel".to_owned()]
    );
}

#[test]
fn an_outcome_removed_is_reported_by_the_branch_it_was() {
    let (before, mut after) = pair();
    after
        .commands
        .get_mut(&name("billing.invoice.CreateInvoice"))
        .expect("the fixture declares it")
        .outcomes
        .retain(|outcome| outcome.name.as_str() != "rejected");

    let found = ids(&before, &after);

    assert_eq!(
        found,
        vec!["command/billing.invoice.CreateInvoice/outcome-removed/rejected".to_owned()]
    );
}

#[test]
fn an_input_removed_is_reported() {
    let (before, mut after) = pair();
    after
        .commands
        .get_mut(&name("billing.email.SendEmail"))
        .expect("the fixture declares it")
        .input
        .retain(|field| field.name != "template");

    let delta = diff(&before, &after).expect("one system");
    let SemanticChange::Command { changed, .. } = &delta.changes()[0] else {
        panic!("this is {:?}", delta.changes()[0]);
    };
    assert_eq!(
        changed,
        &CommandChange::InputRemoved {
            field: "template".to_owned(),
        }
    );
}

#[test]
fn a_view_field_that_changed_type_is_reported() {
    // Unreachable by one document edit — `ess-domain` refuses a view field whose type disagrees
    // with the entity's, so the entity has to move with it — and still a state two real revisions
    // can be in, which is why the kind exists.
    let (before, mut after) = pair();
    after
        .views
        .get_mut(&name("billing.invoice.InvoiceById"))
        .expect("the fixture declares it")
        .fields
        .iter_mut()
        .find(|field| field.name == "total")
        .expect("the view projects it")
        .type_ref = ResolvedTypeRef::Primitive {
        name: ess_domain::types::Primitive::Decimal,
    };

    let delta = diff(&before, &after).expect("one system");
    let SemanticChange::View { changed, .. } = &delta.changes()[0] else {
        panic!("this is {:?}", delta.changes()[0]);
    };
    assert_eq!(
        changed,
        &ViewChange::FieldTypeChanged {
            field: "total".to_owned(),
            before: "billing.invoice.Money".to_owned(),
            after: "Decimal".to_owned(),
        }
    );
}

#[test]
fn an_escalation_that_publishes_a_different_event_is_a_failure_change_that_names_it() {
    // The escalation travels inside the failure answer — `ess-domain` refuses either half without
    // the other — so pointing it at another event is the failure policy having moved, and the
    // rendering names both events rather than leaving a reader to look them up.
    let (before, mut after) = pair();
    let binding = after
        .bindings
        .values_mut()
        .next()
        .expect("billing declares bindings");
    assert_eq!(
        binding.failure,
        ess_domain::binding::Failure::Escalate,
        "the fixture escalates, or this test mutates nothing that matters"
    );
    let other = before
        .events
        .keys()
        .find(|event| {
            binding
                .escalation
                .as_ref()
                .is_some_and(|current| current.name() != *event)
        })
        .expect("billing declares a second event");
    // A handle cannot be minted by a test; borrowing one from the model under mutation keeps the
    // one-way door intact.
    let handle = after
        .commands
        .values()
        .flat_map(|command| command.outcomes.iter())
        .flat_map(|outcome| outcome.emits.iter())
        .find(|handle| handle.name() == other)
        .expect("some outcome emits it")
        .clone();
    after
        .bindings
        .values_mut()
        .next()
        .expect("billing declares bindings")
        .escalation = Some(handle);

    let delta = diff(&before, &after).expect("one system");
    let SemanticChange::Binding { changed, .. } = &delta.changes()[0] else {
        panic!("this is {:?}", delta.changes()[0]);
    };
    let BindingChange::FailureChanged { before, after } = changed else {
        panic!("this is {changed:?}");
    };
    assert!(
        before.starts_with("escalate, publishing `billing."),
        "{before}"
    );
    assert!(
        after.starts_with("escalate, publishing `billing.") && after != before,
        "{after}"
    );
}
