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
    // `RawSpecFile::parse`, not `serde_yaml::from_str`: the latter keeps the last of two identical
    // mapping keys without saying so, and it is not what any real caller uses — the CLI reads the
    // example through `RawSpecFile::parse` (`protocol-cli/src/main.rs:734`). A test of the
    // normative example that takes a path no caller takes is not testing the example.
    let parsed = RawSpecFile::parse(&text)
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
fn every_move_the_invoice_can_make_names_the_command_that_makes_it() {
    // Gate G14 on the normative example. Before it, the invoice declared three moves and no command
    // took any of them: design §19 could see that `Issued -> Paid` is legal and had no verb to
    // reach it with, so the scenario it wants to generate was unwritable. Read here from the
    // specification rather than from a name that looks like a transition's — inferring a command
    // from a spelling is the invention §19 refuses.
    let specification = billing();

    let taker = |transition: &str| -> Vec<String> {
        specification
            .commands
            .values()
            .flat_map(|command| {
                command.outcomes.iter().filter_map(move |outcome| {
                    let subject = outcome.subject.as_ref()?;
                    (subject.entity.to_string() == "billing.invoice.Invoice"
                        && subject.effect.transition() == Some(transition))
                    .then(|| format!("{}.{}", command.name, outcome.name))
                })
            })
            .collect()
    };

    assert_eq!(
        taker("settle"),
        vec!["billing.invoice.PayInvoice.settled".to_owned()],
        "`Issued -> Paid` is design §19's worked example, and it is the one that has to have a verb"
    );
    assert_eq!(
        taker("issue"),
        vec!["billing.invoice.IssueInvoice.issued".to_owned()]
    );
    assert_eq!(
        taker("cancel"),
        vec!["billing.invoice.CancelInvoice.cancelled".to_owned()]
    );

    // Every declared move, not just the three checked by name: the rule is total, and a fourth
    // transition added to the example without a command would fail here as well as in validation.
    let invoice = specification
        .entities
        .get(&"billing.invoice.Invoice".parse().expect("a name"))
        .expect("the example declares Invoice");
    for transition in &invoice.states.transitions {
        assert!(
            !taker(&transition.name).is_empty(),
            "`{}` is a state change nothing can trigger",
            transition.name
        );
    }

    // And creation, which is not a transition: an instance has to come from somewhere, and §20 asks
    // whose invariants to evaluate after `CreateInvoice`.
    let created = specification
        .commands
        .get(&"billing.invoice.CreateInvoice".parse().expect("a name"))
        .expect("the example declares CreateInvoice")
        .outcome(&"accepted".parse().expect("an outcome name"))
        .expect("the accepted branch")
        .subject
        .clone()
        .expect("acceptance creates an invoice");
    assert_eq!(created.entity.to_string(), "billing.invoice.Invoice");
    assert_eq!(created.effect, ess_domain::command::Effect::Creates);
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

#[test]
fn the_example_decomposes_into_components_that_own_the_domains() {
    let specification = billing();

    assert_eq!(specification.components.len(), 2);
    let owned: BTreeSet<String> = specification
        .components
        .values()
        .flat_map(|component| component.owns.iter().map(ToString::to_string))
        .collect();
    for domain in &specification.system.domains {
        assert!(
            owned.contains(&domain.name.to_string()),
            "`{}` is owned by no component: {owned:?}",
            domain.name
        );
    }
}

#[test]
fn the_example_binds_one_context_to_the_other_and_says_what_happens_when_it_fails() {
    // Review F3: a binding that can fail silently is the difference between specifying a system and
    // specifying a demo, and the way that difference disappears is a default nobody read. So the
    // example states both, and this asserts it states them rather than inherits them.
    use ess_domain::binding::{Delivery, Failure};

    let specification = billing();
    let binding = specification
        .bindings
        .values()
        .find(|binding| binding.name.as_str() == "notify-on-invoice-created")
        .expect("the example binds the two contexts");

    assert_eq!(binding.event.to_string(), "billing.invoice.InvoiceCreated");
    assert_eq!(binding.command.to_string(), "billing.email.SendEmail");
    assert_eq!(binding.delivery, Delivery::AtLeastOnce);
    assert_eq!(binding.failure, Failure::Escalate);
    assert!(
        binding.mapping.len() >= 2,
        "the binding fills both of SendEmail's inputs: {:?}",
        binding.mapping
    );
}

#[test]
fn the_examples_escalation_names_the_event_that_proves_it_happened() {
    // G2. `escalate` names a consequence outside the system — a person is told — so without an
    // event, nothing inside the system records that it happened and the normative example carried a
    // requirement no oracle could check. The scenario this enables: force `SendEmail` to fail,
    // expect `billing.email.DeliveryEscalated`.
    use ess_domain::binding::Failure;

    let specification = billing();
    let binding = specification
        .bindings
        .values()
        .find(|binding| binding.name.as_str() == "notify-on-invoice-created")
        .expect("the example binds the two contexts");

    assert_eq!(binding.failure, Failure::Escalate);
    let emitted = binding
        .escalation
        .as_ref()
        .expect("an escalation nobody can observe is the silent failure `on_failure` prevents");
    assert_eq!(emitted.to_string(), "billing.email.DeliveryEscalated");

    // Declared, so it has fields a scenario can assert on rather than being a bare name.
    let declared = specification
        .events
        .get(emitted)
        .expect("the escalation event is declared");
    let carried: Vec<&str> = declared
        .fields
        .iter()
        .map(|field| field.name.as_str())
        .collect();
    assert_eq!(carried, ["recipient", "template"]);
}

#[test]
fn the_example_shows_a_mapping_that_reads_the_event_and_one_that_does_not() {
    // The two kinds are kept apart in the model so a reader can tell which mappings the compiler
    // verified: a field's type is checked against the target, a literal's cannot be.
    use ess_domain::binding::MappingSource;

    let specification = billing();
    let binding = specification
        .bindings
        .values()
        .next()
        .expect("the example declares a binding");

    assert!(
        binding
            .mapping
            .values()
            .any(|source| matches!(source, MappingSource::EventField { .. })),
        "no mapping reads the event: {:?}",
        binding.mapping
    );
    assert!(
        binding
            .mapping
            .values()
            .any(|source| matches!(source, MappingSource::Literal { .. })),
        "no mapping is a literal, so the unchecked case is unexercised: {:?}",
        binding.mapping
    );
}

#[test]
fn the_example_declares_the_one_type_crossing_it_needs_and_says_why() {
    // A conversion with no reason is what this declaration exists to prevent: a silent widening
    // someone added to make a build pass.
    let specification = billing();

    assert_eq!(specification.conversions.len(), 1);
    let conversion = specification
        .conversions
        .iter()
        .next()
        .expect("the example declares one crossing");
    assert!(!conversion.because.trim().is_empty());

    let from = &conversion.from;
    let to = &conversion.to;
    assert!(
        specification.conversions.permits(from, to),
        "the declared crossing is not permitted"
    );
    assert!(
        !specification.conversions.permits(to, from),
        "a crossing is one-directional; the reverse is usually the unsafe one"
    );
}

#[test]
fn the_example_states_a_runtime_shape_without_deploying_anything() {
    let specification = billing();

    assert_eq!(specification.topology.workloads.len(), 2);
    for workload in specification.topology.workloads.values() {
        assert!(
            workload.replicas.min >= 2,
            "`{}` runs a binding's listener; one instance leaves a window with nothing listening",
            workload.component
        );
        assert!(
            !workload.requires.is_empty(),
            "`{}` needs nothing, which is not a statement anyone made",
            workload.component
        );
    }
}
