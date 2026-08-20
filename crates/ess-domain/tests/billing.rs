//! The normative example, parsed from the files it actually lives in.
//!
//! Design §38's first success criterion is "parse the billing ESS". This is that criterion, and it is
//! deliberately a test over `examples/billing/` rather than over a copy inlined here: the design
//! document's snippets drifted from each other three ways before anyone noticed (review F7), and a
//! copy would drift the same way.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use ess_domain::name::QualifiedName;
use ess_domain::spec::{RawSpecFile, Specification};
use ess_domain::system::Source;
use ess_domain::view::{AssertionStyle, Consistency};

/// The example directory.
fn example() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/billing")
        .canonicalize()
        .expect("the billing example exists")
}

/// Reads one specification file.
fn read(relative: &str) -> (Source, RawSpecFile) {
    let path = example().join(relative);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{} is readable: {error}", path.display()));
    let parsed: RawSpecFile = serde_yaml::from_str(&text)
        .unwrap_or_else(|error| panic!("{} is well formed: {error}", path.display()));
    (Source::new(relative), parsed)
}

/// Every `.yaml` file in the example, relative to it, in a stable order.
///
/// Discovered rather than listed: a fourth file added to the example would be read by the CLI and
/// silently ignored by the test that is supposed to keep the example honest, which is the failure
/// mode this whole file exists to prevent.
fn files() -> Vec<String> {
    let base = example();
    let mut found = Vec::new();
    let mut pending = vec![base.clone()];
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(&directory)
            .unwrap_or_else(|error| panic!("{} is readable: {error}", directory.display()))
        {
            let path = entry.expect("an entry").path();
            if path.is_dir() {
                pending.push(path);
            } else if path
                .extension()
                .is_some_and(|extension| extension == "yaml")
            {
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

/// The whole billing specification.
fn billing() -> Specification {
    Specification::assemble(files().iter().map(|file| read(file)))
        .unwrap_or_else(|errors| panic!("the billing specification is valid:\n{errors}"))
}

#[test]
fn the_billing_specification_parses_and_validates() {
    let specification = billing();

    assert_eq!(specification.system.name.to_string(), "billing");
    assert_eq!(specification.system.version.to_string(), "v3");
    assert_eq!(specification.system.format.to_string(), "ess/1");
    assert_eq!(specification.system.domains.len(), 2);

    // Two contexts, and the members are owned by the right ones.
    let invoice: QualifiedName = "billing.invoice.CreateInvoice".parse().expect("a name");
    assert_eq!(
        specification
            .system
            .owner_of(&invoice)
            .map(|domain| domain.name.to_string()),
        Some("billing.invoice".to_owned())
    );
    let email: QualifiedName = "billing.email.SendEmail".parse().expect("a name");
    assert_eq!(
        specification
            .system
            .owner_of(&email)
            .map(|domain| domain.name.to_string()),
        Some("billing.email".to_owned())
    );
}

#[test]
fn a_command_that_can_be_refused_says_so() {
    // The review's F1: a command with a precondition has more than one outcome, and a specification
    // that recorded only the happy one would generate a suite that never checks the other.
    let specification = billing();
    let create = specification
        .commands
        .get(&"billing.invoice.CreateInvoice".parse().expect("a name"))
        .expect("the example declares CreateInvoice");

    assert_eq!(create.outcomes.len(), 2);
    let accepted = create
        .outcomes
        .iter()
        .find(|outcome| outcome.name.as_str() == "accepted")
        .expect("an accepted outcome");
    assert_eq!(accepted.emits.len(), 1);
    assert!(accepted.error.is_none());

    let rejected = create
        .outcomes
        .iter()
        .find(|outcome| outcome.name.as_str() == "rejected")
        .expect("a rejected outcome");
    assert!(
        rejected.emits.is_empty() && rejected.error.is_some(),
        "a refused command changes nothing and names why"
    );
}

#[test]
fn an_outcome_the_input_cannot_decide_says_that_too() {
    // Whether a mail provider accepts an address is not a function of the input. Writing `when:
    // false` would have claimed the branch is unreachable, which is a different and false statement.
    let specification = billing();
    let send = specification
        .commands
        .get(&"billing.email.SendEmail".parse().expect("a name"))
        .expect("the example declares SendEmail");

    let failed = send
        .outcomes
        .iter()
        .find(|outcome| outcome.name.as_str() == "failed")
        .expect("a failure outcome");
    assert!(
        !failed.is_testable_from_input(),
        "a generated test must inject this fault, not construct an input for it"
    );
    assert_eq!(
        failed.test_strategy(),
        ess_domain::command::TestStrategy::InjectFault
    );
}

#[test]
fn a_projection_declares_how_it_must_be_asserted() {
    // The review's F2: asserting a projection immediately after the command that caused it races,
    // and the usual fix — a sleep — makes the suite test the machine it runs on.
    let specification = billing();
    let view = specification
        .views
        .get(&"billing.invoice.InvoiceById".parse().expect("a name"))
        .expect("the example declares InvoiceById");

    assert_eq!(view.consistency, Consistency::Eventual);
    assert_eq!(view.assertion_style(), AssertionStyle::Eventually);
}

#[test]
fn an_illegal_move_is_illegal_because_nobody_wrote_it() {
    let specification = billing();
    let invoice = specification
        .entities
        .get(&"billing.invoice.Invoice".parse().expect("a name"))
        .expect("the example declares Invoice");

    let state = |name: &str| name.parse().expect("a state name");
    assert!(invoice.states.can_move(&state("Draft"), &state("Issued")));
    assert!(invoice.states.can_move(&state("Issued"), &state("Paid")));
    assert!(invoice
        .states
        .can_move(&state("Draft"), &state("Cancelled")));
    assert!(
        !invoice.states.can_move(&state("Paid"), &state("Cancelled")),
        "no transition says so, and no rule needs to: a second statement of the same truth is a \
         second thing that can disagree"
    );
}

#[test]
fn a_view_may_project_the_identity_and_the_state_as_well_as_the_fields() {
    let specification = billing();
    let invoice = specification
        .entities
        .get(&"billing.invoice.Invoice".parse().expect("a name"))
        .expect("the example declares Invoice");

    let observable: Vec<String> = invoice
        .observable_fields()
        .iter()
        .map(|field| field.name.clone())
        .collect();
    assert!(
        observable.contains(&"invoice_id".to_owned()),
        "{observable:?}"
    );
    assert!(observable.contains(&"total".to_owned()), "{observable:?}");
    assert!(
        observable.contains(&"state".to_owned()),
        "the two things most often observed are not declared fields: {observable:?}"
    );
}

#[test]
fn a_reference_to_something_nobody_declared_is_refused_with_what_was_available() {
    let (source, mut invoice) = read("domains/invoice.yaml");
    // Emit an event nobody declares.
    let text = serde_yaml::to_string(&invoice.commands).expect("re-serialises");
    let broken = text.replace(
        "billing.invoice.InvoiceCreated",
        "billing.invoice.InvoiceRaised",
    );
    invoice.commands = serde_yaml::from_str(&broken).expect("still well formed");

    let errors = Specification::assemble([
        read("system.yaml"),
        (source, invoice),
        read("domains/email.yaml"),
    ])
    .expect_err("the event does not exist");
    let rendered = errors.to_string();
    assert!(rendered.contains("InvoiceRaised"), "{rendered}");
}

/// Every primitive and composite reachable from the example, by name.
///
/// Walks bodies as well as fields: `Uuid` appears in the example only inside a newtype, and a check
/// that only looked at fields would call it uncovered.
fn types_used() -> (BTreeSet<String>, BTreeSet<&'static str>) {
    use ess_domain::types::{TypeBody, TypeRef};

    let specification = billing();
    let mut primitives = BTreeSet::new();
    let mut composites = BTreeSet::new();

    let mut collect = |type_ref: &TypeRef| {
        let mut pending = vec![type_ref.clone()];
        while let Some(current) = pending.pop() {
            match current {
                TypeRef::Primitive(primitive) => {
                    primitives.insert(format!("{primitive:?}"));
                }
                TypeRef::Optional(inner) => {
                    composites.insert("Optional");
                    pending.push(*inner);
                }
                TypeRef::List(inner) => {
                    composites.insert("List");
                    pending.push(*inner);
                }
                TypeRef::Map(key, value) => {
                    composites.insert("Map");
                    primitives.insert(format!("{key:?}"));
                    pending.push(*value);
                }
                TypeRef::Named(_) => {}
            }
        }
    };

    for entity in specification.entities.values() {
        for field in entity.observable_fields() {
            collect(&field.type_ref);
        }
    }
    for declared in specification.system.types.iter() {
        match &declared.body {
            TypeBody::Struct { fields, .. } => {
                for field in fields {
                    collect(&field.type_ref);
                }
            }
            TypeBody::Newtype { of, .. } => collect(of),
            TypeBody::Union { variants, .. } => {
                for variant in variants.values() {
                    collect(variant);
                }
            }
            TypeBody::Enum { .. } => {}
        }
    }

    (primitives, composites)
}

// The example is normative, so what it leaves out is what nothing checks. The four tests below are
// the inventory: a construct added to the model without reaching the example is a construct whose
// only test is the one its author remembered to write.

#[test]
fn the_example_declares_a_type_of_every_kind() {
    use ess_domain::types::TypeBody;

    let specification = billing();
    let bodies: Vec<&TypeBody> = specification
        .system
        .types
        .iter()
        .map(|declared| &declared.body)
        .collect();

    for (label, present) in [
        (
            "newtype",
            bodies
                .iter()
                .any(|body| matches!(body, TypeBody::Newtype { .. })),
        ),
        (
            "struct",
            bodies
                .iter()
                .any(|body| matches!(body, TypeBody::Struct { .. })),
        ),
        (
            "enum",
            bodies
                .iter()
                .any(|body| matches!(body, TypeBody::Enum { .. })),
        ),
        (
            "union",
            bodies
                .iter()
                .any(|body| matches!(body, TypeBody::Union { .. })),
        ),
    ] {
        assert!(present, "the example declares no {label}");
    }
}

#[test]
fn the_example_uses_every_primitive_and_every_composite() {
    let (primitives, composites) = types_used();

    for composite in ["Optional", "List", "Map"] {
        assert!(
            composites.contains(composite),
            "the example uses no {composite}<>: {composites:?}"
        );
    }
    for primitive in ess_domain::types::Primitive::ALL {
        assert!(
            primitives.contains(&format!("{primitive:?}")),
            "the example uses no {primitive:?}: {primitives:?}"
        );
    }
}

#[test]
fn the_example_shows_both_kinds_of_actor_and_both_consistency_levels() {
    let specification = billing();

    assert!(
        specification
            .actors
            .values()
            .any(|actor| !actor.may.is_empty()),
        "no actor in the example may invoke anything, so `may` is unexercised"
    );
    assert!(
        specification
            .actors
            .values()
            .any(|actor| actor.may.is_empty()),
        "an actor that only observes is legal and should be shown to be"
    );
    assert!(
        specification
            .views
            .values()
            .any(|view| view.consistency == Consistency::ReadYourWrites),
        "every view in the example is eventual, so `read_your_writes` is unexercised"
    );
    assert!(
        specification
            .views
            .values()
            .any(|view| view.consistency == Consistency::Eventual),
        "no view in the example is a projection"
    );
}

#[test]
fn the_example_shows_a_filter_a_payload_and_an_overridden_wire_name() {
    let specification = billing();

    assert!(
        specification
            .views
            .values()
            .any(|view| view.filter.is_some()),
        "no view in the example filters"
    );
    assert!(
        specification
            .errors
            .values()
            .any(|error| !error.fields.is_empty()),
        "no error in the example carries a payload"
    );
    assert!(
        specification
            .commands
            .values()
            .any(|command| !command.naming.is_empty()),
        "no command in the example overrides its wire or display name"
    );
}
