//! Every rejection this compiler implements, each with the specification that provokes it.
//!
//! The specifications are built here rather than by breaking `examples/billing/`, for two reasons:
//! the example is normative and shared, and a defect written as YAML would be refused by
//! [`Specification::assemble`](ess_domain::spec::Specification::assemble) before this pass ever ran —
//! so the test would prove that wave 1 works, which wave 1's own tests already do.
//!
//! Built field by field, a `Specification` has not been validated by anything. That is deliberate:
//! it is exactly the state in which "the domain crate already checked this" is false, which is the
//! state the IR's handles exist to survive.
//!
//! Every test asserts a [`Code`](ess_compiler::diagnostic::Code), never message text.

use std::collections::{BTreeMap, BTreeSet};

use ess_compiler::diagnostic::{Code, Detail, Diagnostics};
use ess_compiler::ir::{EssIr, ResolvedInstance, ResolvedMappingValue, ResolvedPayloadValue};
use ess_compiler::resolve::{codes, compile};
use ess_compiler::source::SourceMap;
use ess_domain::binding::{BindingName, BindingSpec, Delivery, Failure, MappingSource};
use ess_domain::command::{
    CommandSpec, ErrorSpec, EventSpec, Outcome, OutcomeCondition, OutcomeName, PayloadSource,
};
use ess_domain::component::{ComponentName, ComponentSpec};
use ess_domain::domain::DomainSpec;
use ess_domain::entity::{EntitySpec, StateMachine, StateName, Transition};
use ess_domain::name::{Naming, QualifiedName, Version};
use ess_domain::spec::Specification;
use ess_domain::system::{FormatVersion, SystemSpec};
use ess_domain::topology::{Replicas, Topology, Workload};
use ess_domain::types::{
    Conversion, ConversionRegistry, Field, NamedType, Primitive, TypeBody, TypeRef, TypeRegistry,
};

/// The one domain every fixture declares.
const DOMAIN: &str = "shop.orders";

fn name(value: &str) -> QualifiedName {
    QualifiedName::new(value).expect("a valid qualified name")
}

fn newtype(value: &str, of: TypeRef) -> NamedType {
    NamedType {
        name: name(value),
        body: TypeBody::Newtype {
            of,
            invariants: Vec::new(),
        },
        naming: Naming::default(),
    }
}

fn structure(value: &str, fields: Vec<Field>) -> NamedType {
    NamedType {
        name: name(value),
        body: TypeBody::Struct {
            fields,
            invariants: Vec::new(),
        },
        naming: Naming::default(),
    }
}

fn field(field_name: &str, type_ref: &str) -> Field {
    Field::new(field_name, TypeRef::parse(type_ref).expect("a valid type"))
}

fn event(value: &str, fields: Vec<Field>) -> EventSpec {
    EventSpec {
        name: name(value),
        fields,
        naming: Naming::default(),
    }
}

/// One outcome, unconditional, which is all these fixtures need.
fn outcome(label: &str, emits: &[&str], error: Option<&str>) -> Outcome {
    Outcome {
        name: OutcomeName::new(label).expect("a valid outcome name"),
        condition: OutcomeCondition::Otherwise,
        subject: None,
        emits: emits.iter().map(|event| name(event)).collect(),
        payload: BTreeMap::new(),
        error: error.map(name),
        summary: None,
    }
}

fn command(value: &str, input: Vec<Field>, outcomes: Vec<Outcome>) -> CommandSpec {
    CommandSpec {
        name: name(value),
        input,
        outcomes,
        naming: Naming::default(),
    }
}

fn binding(id: &str, trigger: &str, invoked: &str, mapping: &[(&str, &str)]) -> BindingSpec {
    BindingSpec {
        name: BindingName::new(id).expect("a valid binding name"),
        event: name(trigger),
        command: name(invoked),
        mapping: mapping
            .iter()
            .map(|(target, source)| ((*target).to_owned(), MappingSource::parse(source)))
            .collect(),
        delivery: Delivery::AtLeastOnce,
        // `retry`, so that a fixture about some other rule does not also have to declare the event
        // an `escalate` would have to name.
        failure: Failure::Retry,
        escalation: None,
        naming: Naming::default(),
    }
}

/// A specification assembled by hand, so that a rule can be broken one rule at a time.
#[derive(Default)]
struct Fixture {
    types: Vec<NamedType>,
    entities: Vec<EntitySpec>,
    events: Vec<EventSpec>,
    commands: Vec<CommandSpec>,
    errors: Vec<ErrorSpec>,
    bindings: Vec<BindingSpec>,
    components: Vec<ComponentSpec>,
    conversions: Vec<Conversion>,
    workloads: Vec<Workload>,
    /// Members deliberately left out of the domain's roster, so nothing owns them.
    orphans: Vec<QualifiedName>,
}

impl Fixture {
    fn build(self) -> Specification {
        let mut registry = TypeRegistry::new();
        for declared in self.types {
            registry.insert(declared).expect("one declaration each");
        }
        let mut conversions = ConversionRegistry::new();
        for conversion in self.conversions {
            conversions.insert(conversion).expect("one crossing each");
        }

        let owned = |names: Vec<QualifiedName>| -> Vec<QualifiedName> {
            names
                .into_iter()
                .filter(|value| !self.orphans.contains(value))
                .collect()
        };
        let domain = DomainSpec {
            name: name(DOMAIN),
            types: Vec::new(),
            entities: owned(self.entities.iter().map(|it| it.name.clone()).collect()),
            commands: owned(self.commands.iter().map(|it| it.name.clone()).collect()),
            events: owned(self.events.iter().map(|it| it.name.clone()).collect()),
            views: Vec::new(),
            errors: owned(self.errors.iter().map(|it| it.name.clone()).collect()),
            actors: Vec::new(),
            naming: Naming::default(),
        };

        Specification {
            system: SystemSpec {
                name: name("shop"),
                version: Version::V1,
                format: FormatVersion::V1,
                domains: vec![domain],
                types: registry,
                naming: Naming::default(),
                summary: None,
            },
            entities: self
                .entities
                .into_iter()
                .map(|it| (it.name.clone(), it))
                .collect(),
            commands: self
                .commands
                .into_iter()
                .map(|it| (it.name.clone(), it))
                .collect(),
            events: self
                .events
                .into_iter()
                .map(|it| (it.name.clone(), it))
                .collect(),
            errors: self
                .errors
                .into_iter()
                .map(|it| (it.name.clone(), it))
                .collect(),
            views: BTreeMap::new(),
            actors: BTreeMap::new(),
            components: self
                .components
                .into_iter()
                .map(|it| (it.name.clone(), it))
                .collect(),
            bindings: self
                .bindings
                .into_iter()
                .map(|it| (it.name.clone(), it))
                .collect(),
            topology: Topology {
                workloads: self
                    .workloads
                    .into_iter()
                    .map(|it| (it.component.clone(), it))
                    .collect(),
            },
            conversions,
        }
    }

    /// Compiles, expecting refusal.
    fn refused(self) -> Diagnostics {
        let specification = self.build();
        compile(&specification, &SourceMap::new()).expect_err("this fixture does not resolve")
    }

    /// Compiles, expecting success.
    fn resolved(self) -> EssIr {
        let specification = self.build();
        compile(&specification, &SourceMap::new())
            .unwrap_or_else(|diagnostics| panic!("this fixture resolves:\n{diagnostics}"))
    }
}

/// The two contexts a binding joins: an event carrying an `Email`, a command taking one input.
fn crossing(input: &str) -> Fixture {
    Fixture {
        types: vec![
            newtype("shop.orders.Email", TypeRef::Primitive(Primitive::String)),
            newtype("shop.orders.Address", TypeRef::Primitive(Primitive::String)),
        ],
        events: vec![event(
            "shop.orders.Ordered",
            vec![field("customer_email", "shop.orders.Email")],
        )],
        commands: vec![command(
            "shop.orders.Notify",
            vec![field("recipient", input)],
            vec![outcome("sent", &[], None)],
        )],
        bindings: vec![binding(
            "notify-on-ordered",
            "shop.orders.Ordered",
            "shop.orders.Notify",
            &[("recipient", "event.customer_email")],
        )],
        ..Fixture::default()
    }
}

// ---- types ---------------------------------------------------------------------------------

#[test]
fn an_event_field_typed_as_something_nobody_declares_is_refused() {
    let diagnostics = Fixture {
        events: vec![event(
            "shop.orders.Ordered",
            vec![field("total", "shop.orders.Money")],
        )],
        ..Fixture::default()
    }
    .refused();

    // Filed under the layer that holds the reference, which is where `ess-domain` files it too.
    assert!(
        diagnostics.contains(codes::EVENT_UNDECLARED_REFERENCE),
        "{diagnostics}"
    );
}

#[test]
fn a_type_reference_inside_a_list_is_resolved_too() {
    let diagnostics = Fixture {
        events: vec![event(
            "shop.orders.Ordered",
            vec![field("lines", "List<shop.orders.LineItem>")],
        )],
        ..Fixture::default()
    }
    .refused();

    assert!(
        diagnostics.contains(codes::EVENT_UNDECLARED_REFERENCE),
        "{diagnostics}"
    );
}

#[test]
fn a_type_that_requires_itself_is_not_this_passs_business_because_its_references_resolve() {
    // `ess-domain` refuses this during assembly, as `self_reference` — `SystemSpec::merge` owns the
    // rule — and `tests/billing.rs` shows it arriving at a reader as `ESS-TYPE-008` with a line.
    // Here the point is the boundary: an uninhabitable type is perfectly representable in the IR, so
    // refusing it is not something minting a handle depends on, and this pass does not restate it.
    let specification = Fixture {
        types: vec![
            structure(
                "shop.orders.Left",
                vec![field("right", "shop.orders.Right")],
            ),
            structure("shop.orders.Right", vec![field("left", "shop.orders.Left")]),
        ],
        ..Fixture::default()
    }
    .build();

    assert!(
        compile(&specification, &SourceMap::new()).is_ok(),
        "every reference in it resolves, which is the only question this pass asks"
    );
}

#[test]
fn a_type_that_reaches_itself_through_a_list_is_accepted_because_the_empty_list_is_a_value() {
    let ir = Fixture {
        types: vec![structure(
            "shop.orders.Tree",
            vec![field("children", "List<shop.orders.Tree>")],
        )],
        ..Fixture::default()
    }
    .resolved();

    assert!(ir.types.contains_key(&name("shop.orders.Tree")));
}

#[test]
fn a_union_with_one_terminating_variant_is_accepted_even_though_another_recurses() {
    let ir = Fixture {
        types: vec![
            NamedType {
                name: name("shop.orders.Expr"),
                body: TypeBody::Union {
                    tag: "kind".to_owned(),
                    variants: [
                        ("leaf".to_owned(), TypeRef::Primitive(Primitive::Integer)),
                        ("pair".to_owned(), TypeRef::Named(name("shop.orders.Pair"))),
                    ]
                    .into_iter()
                    .collect(),
                },
                naming: Naming::default(),
            },
            structure(
                "shop.orders.Pair",
                vec![
                    field("left", "shop.orders.Expr"),
                    field("right", "shop.orders.Expr"),
                ],
            ),
        ],
        ..Fixture::default()
    }
    .resolved();

    assert_eq!(
        ir.types.len(),
        2,
        "a cycle is not the defect; uninhabitability is"
    );
}

// ---- commands and ownership ---------------------------------------------------------------

#[test]
fn an_outcome_that_emits_an_event_nobody_declares_is_refused() {
    let diagnostics = Fixture {
        commands: vec![command(
            "shop.orders.Place",
            Vec::new(),
            vec![outcome("accepted", &["shop.orders.Ordered"], None)],
        )],
        ..Fixture::default()
    }
    .refused();

    assert!(
        diagnostics.contains(codes::COMMAND_UNDECLARED_REFERENCE),
        "{diagnostics}"
    );
}

/// An entity with a two-state lifecycle and one move, so an outcome has something to take.
fn entity(value: &str, transition: &str) -> EntitySpec {
    let state = |label: &str| StateName::new(label).expect("a valid state name");
    EntitySpec {
        name: name(value),
        identity: field("order_id", "String"),
        fields: Vec::new(),
        states: StateMachine {
            states: [state("Open"), state("Closed")].into(),
            initial: state("Open"),
            terminal: [state("Closed")].into(),
            transitions: vec![
                Transition::new(transition, [state("Open")], state("Closed")).expect("a move"),
            ],
        },
        invariants: Vec::new(),
        naming: Naming::default(),
    }
}

/// One outcome that acts on an entity, so the subject has to resolve.
fn acting(label: &str, subject: ess_domain::command::Subject) -> Outcome {
    Outcome {
        name: OutcomeName::new(label).expect("a valid outcome name"),
        condition: OutcomeCondition::Otherwise,
        subject: Some(subject),
        emits: Vec::new(),
        payload: BTreeMap::new(),
        error: None,
        summary: None,
    }
}

#[test]
fn an_outcome_that_acts_on_an_entity_nobody_declares_is_refused() {
    // There is no `EntityHandle` to put in the outcome, so this pass cannot build the IR and says
    // so — the same reason a view's source is resolved here rather than trusted.
    let diagnostics = Fixture {
        entities: vec![entity("shop.orders.Order", "close")],
        commands: vec![command(
            "shop.orders.Close",
            vec![field("order_id", "String")],
            vec![acting(
                "closed",
                ess_domain::command::Subject::moves(
                    name("shop.orders.Receipt"),
                    "close",
                    "order_id",
                ),
            )],
        )],
        ..Fixture::default()
    }
    .refused();

    assert!(
        diagnostics.contains(codes::COMMAND_UNDECLARED_REFERENCE),
        "{diagnostics}"
    );
}

#[test]
fn an_outcome_that_takes_a_move_the_entity_does_not_declare_is_refused() {
    // The transition is resolved for the same reason the entity is: the IR carries the move itself
    // rather than its name, so there is nothing to carry for a move that does not exist.
    let diagnostics = Fixture {
        entities: vec![entity("shop.orders.Order", "close")],
        commands: vec![command(
            "shop.orders.Close",
            vec![field("order_id", "String")],
            vec![acting(
                "closed",
                ess_domain::command::Subject::moves(
                    name("shop.orders.Order"),
                    "settle",
                    "order_id",
                ),
            )],
        )],
        ..Fixture::default()
    }
    .refused();

    assert!(
        diagnostics.contains(codes::COMMAND_UNDECLARED_REFERENCE),
        "{diagnostics}"
    );
    assert!(
        diagnostics.as_slice().iter().any(|diagnostic| {
            diagnostic.details.iter().any(|detail| {
                matches!(
                    detail,
                    Detail::Undeclared { expected, available, .. }
                        if *expected == "transition" && available.iter().any(|it| it == "close")
                )
            })
        }),
        "the moves the entity does declare are offered: {diagnostics}"
    );
}

#[test]
fn an_outcome_whose_instance_is_not_typed_as_the_identity_is_refused_as_a_type_mismatch() {
    // The link between a command and the instance it acts on, resolved here for the reason the
    // entity and the transition are: there is no `ResolvedField` to carry for a field that is not
    // there, and no honest one to carry for a field of the wrong type. `order_id` here is an
    // `Integer` and an `Order` is identified by a `String`, so the name resolves and the link does
    // not — which is exactly the case a name check alone would let through.
    let diagnostics = Fixture {
        entities: vec![entity("shop.orders.Order", "close")],
        commands: vec![command(
            "shop.orders.Close",
            vec![field("order_id", "Integer")],
            vec![acting(
                "closed",
                ess_domain::command::Subject::moves(name("shop.orders.Order"), "close", "order_id"),
            )],
        )],
        ..Fixture::default()
    }
    .refused();

    assert!(
        diagnostics.contains(codes::COMMAND_TYPE_MISMATCH),
        "{diagnostics}"
    );
    assert!(
        diagnostics.as_slice().iter().any(|diagnostic| {
            diagnostic.details.iter().any(|detail| {
                matches!(
                    detail,
                    Detail::Typed { subject, type_ref, requires }
                        if subject == "shop.orders.Close.order_id" && type_ref == "String" && *requires
                )
            })
        }),
        "the type the position requires is a field, not a sentence: {diagnostics}"
    );
}

#[test]
fn an_outcome_whose_instance_names_no_input_field_is_refused() {
    // The other half. `reference` is well formed and names nothing the command takes, so it gets the
    // code every other well-formed name pointing at nothing gets.
    let diagnostics = Fixture {
        entities: vec![entity("shop.orders.Order", "close")],
        commands: vec![command(
            "shop.orders.Close",
            vec![field("order_id", "String")],
            vec![acting(
                "closed",
                ess_domain::command::Subject::moves(
                    name("shop.orders.Order"),
                    "close",
                    "reference",
                ),
            )],
        )],
        ..Fixture::default()
    }
    .refused();

    assert!(
        diagnostics.contains(codes::COMMAND_UNDECLARED_REFERENCE),
        "{diagnostics}"
    );
    assert!(
        diagnostics.to_string().contains("order_id"),
        "the fields it does take are offered: {diagnostics}"
    );
}

#[test]
fn a_resolved_outcome_carries_the_move_itself_rather_than_its_name() {
    // The one place this IR resolves a reference to a *value* instead of to a handle: a transition
    // is declared inside a lifecycle and has no map on `EssIr` to be keyed in, so a projection gets
    // `from` and `to` in hand rather than an `Option` from a lookup.
    let ir = Fixture {
        entities: vec![entity("shop.orders.Order", "close")],
        commands: vec![command(
            "shop.orders.Close",
            vec![field("order_id", "String")],
            vec![acting(
                "closed",
                ess_domain::command::Subject::moves(name("shop.orders.Order"), "close", "order_id"),
            )],
        )],
        ..Fixture::default()
    }
    .resolved();

    let closed = &ir
        .commands
        .get(&name("shop.orders.Close"))
        .expect("the command resolves")
        .outcomes[0];
    let subject = closed.subject.as_ref().expect("a subject");
    let transition = subject
        .effect
        .transition()
        .expect("the effect is a declared move");
    assert_eq!(transition.to.as_str(), "Closed");
    assert_eq!(
        subject.entity.name(),
        &name("shop.orders.Order"),
        "the subject is a handle, so `EssIr::entity` answers whose invariants a scenario checks"
    );
    // The third part, resolved the same way and for the same reason: a projection asking "which
    // instance" gets the field, with its type and its wire name, rather than a name to look up.
    let ResolvedInstance::Supplied { field } = &subject.instance else {
        panic!(
            "a `moves:` is supplied by the caller: {:?}",
            subject.instance
        )
    };
    assert_eq!(field.name, "order_id");
    assert_eq!(
        field.type_ref.to_string(),
        "String",
        "and it is the entity's identity type, which is what makes the link mean anything"
    );

    // And the same relation, read the way a lifecycle diagram asks it.
    let drivers = ir.drivers();
    let taken = drivers
        .get(&subject.entity)
        .expect("the entity has a driver");
    assert_eq!(taken.len(), 1);
    assert!(taken[0].takes("close"), "{:?}", taken[0].effect);
}

#[test]
fn an_outcome_that_names_an_error_nobody_declares_is_refused() {
    let diagnostics = Fixture {
        commands: vec![command(
            "shop.orders.Place",
            Vec::new(),
            vec![outcome("rejected", &[], Some("shop.orders.Unpayable"))],
        )],
        ..Fixture::default()
    }
    .refused();

    assert!(
        diagnostics.contains(codes::COMMAND_UNDECLARED_REFERENCE),
        "{diagnostics}"
    );
}

#[test]
fn a_command_inside_no_declared_domain_is_refused_because_the_ir_records_who_owns_it() {
    let orphan = name("shop.orders.Place");
    let diagnostics = Fixture {
        commands: vec![command(
            "shop.orders.Place",
            Vec::new(),
            vec![outcome("accepted", &[], None)],
        )],
        orphans: vec![orphan],
        ..Fixture::default()
    }
    .refused();

    assert!(
        diagnostics.contains(codes::COMMAND_UNDECLARED_REFERENCE),
        "{diagnostics}"
    );
}

// ---- bindings ------------------------------------------------------------------------------

#[test]
fn a_binding_that_reacts_to_an_event_nobody_declares_is_refused() {
    let mut fixture = crossing("shop.orders.Email");
    fixture.bindings = vec![binding(
        "notify-on-ordered",
        "shop.orders.Cancelled",
        "shop.orders.Notify",
        &[("recipient", "event.customer_email")],
    )];

    let diagnostics = fixture.refused();
    assert!(
        diagnostics.contains(codes::BINDING_UNDECLARED_REFERENCE),
        "{diagnostics}"
    );
}

#[test]
fn a_binding_that_invokes_a_command_nobody_declares_is_refused() {
    let mut fixture = crossing("shop.orders.Email");
    fixture.bindings = vec![binding(
        "notify-on-ordered",
        "shop.orders.Ordered",
        "shop.orders.Shout",
        &[("recipient", "event.customer_email")],
    )];

    let diagnostics = fixture.refused();
    assert!(
        diagnostics.contains(codes::BINDING_UNDECLARED_REFERENCE),
        "{diagnostics}"
    );
}

#[test]
fn a_binding_that_escalates_into_an_event_nobody_declares_is_refused() {
    // The guarantee every other reference in the IR has, read on the failure path: a binding cannot
    // escalate into an event nobody declares, so `ResolvedBinding::escalation` is always a handle
    // the IR can look up.
    let mut fixture = crossing("shop.orders.Email");
    let mut escalating = binding(
        "notify-on-ordered",
        "shop.orders.Ordered",
        "shop.orders.Notify",
        &[("recipient", "event.customer_email")],
    );
    escalating.failure = Failure::Escalate;
    escalating.escalation = Some(name("shop.orders.NotifyGaveUp"));
    fixture.bindings = vec![escalating];

    let diagnostics = fixture.refused();
    assert!(
        diagnostics.contains(codes::BINDING_UNDECLARED_REFERENCE),
        "{diagnostics}"
    );
    let diagnostic = diagnostics
        .as_slice()
        .iter()
        .find(|diagnostic| diagnostic.code == codes::BINDING_UNDECLARED_REFERENCE)
        .expect("the refusal");
    assert!(
        diagnostic.message.contains("escalates"),
        "the message tells the two events a binding names apart: {diagnostic:?}"
    );
}

#[test]
fn a_mapping_between_two_distinct_types_with_no_conversion_is_refused() {
    let diagnostics = crossing("shop.orders.Address").refused();

    assert!(
        diagnostics.contains(codes::MAPPING_TYPE_MISMATCH),
        "{diagnostics}"
    );

    // Design §29: the two types and the two paths arrive as fields, so an agent repairing this does
    // not have to parse them back out of the sentence.
    let diagnostic = diagnostics
        .as_slice()
        .iter()
        .find(|diagnostic| diagnostic.code == codes::MAPPING_TYPE_MISMATCH)
        .expect("the mismatch");
    let typed: Vec<(&str, &str, bool)> = diagnostic
        .details
        .iter()
        .filter_map(|detail| match detail {
            Detail::Typed {
                subject,
                type_ref,
                requires,
            } => Some((subject.as_str(), type_ref.as_str(), *requires)),
            _ => None,
        })
        .collect();
    assert_eq!(
        typed,
        vec![
            (
                "shop.orders.Ordered.customer_email",
                "shop.orders.Email",
                false
            ),
            ("shop.orders.Notify.recipient", "shop.orders.Address", true),
        ]
    );
}

#[test]
fn a_declared_conversion_makes_the_same_mapping_legal_and_the_ir_records_why() {
    let mut fixture = crossing("shop.orders.Address");
    fixture.conversions = vec![Conversion {
        from: TypeRef::Named(name("shop.orders.Email")),
        to: TypeRef::Named(name("shop.orders.Address")),
        because: "an order's email is a deliverable address".to_owned(),
    }];

    let ir = fixture.resolved();
    let binding = &ir.bindings[&BindingName::new("notify-on-ordered").expect("a name")];
    assert_eq!(
        binding.mapping[0].conversion.as_deref(),
        Some("an order's email is a deliverable address"),
        "the reason someone wrote down is what a generator has to emit the conversion for"
    );
    assert_eq!(ir.conversions.len(), 1);
}

#[test]
fn a_mapping_that_reads_a_field_the_event_does_not_carry_is_refused() {
    let mut fixture = crossing("shop.orders.Email");
    fixture.bindings = vec![binding(
        "notify-on-ordered",
        "shop.orders.Ordered",
        "shop.orders.Notify",
        &[("recipient", "event.customer_mail")],
    )];

    let diagnostics = fixture.refused();
    assert!(
        diagnostics.contains(codes::MAPPING_READS_UNDECLARED_FIELD),
        "{diagnostics}"
    );
}

#[test]
fn a_mapping_that_fills_an_input_the_command_does_not_take_is_refused() {
    let mut fixture = crossing("shop.orders.Email");
    fixture.bindings = vec![binding(
        "notify-on-ordered",
        "shop.orders.Ordered",
        "shop.orders.Notify",
        &[("recipient", "event.customer_email"), ("subject", "hello")],
    )];

    let diagnostics = fixture.refused();
    assert!(
        diagnostics.contains(codes::BINDING_UNDECLARED_REFERENCE),
        "{diagnostics}"
    );
}

#[test]
fn a_required_command_input_left_unmapped_is_refused() {
    let mut fixture = crossing("shop.orders.Email");
    fixture.bindings = vec![binding(
        "notify-on-ordered",
        "shop.orders.Ordered",
        "shop.orders.Notify",
        &[],
    )];

    let diagnostics = fixture.refused();
    assert!(
        diagnostics.contains(codes::UNMAPPED_COMMAND_INPUT),
        "{diagnostics}"
    );
}

#[test]
fn an_optional_command_input_may_be_left_unmapped_because_the_command_said_it_may_be_absent() {
    let mut fixture = crossing("Optional<shop.orders.Email>");
    fixture.bindings = vec![binding(
        "notify-on-ordered",
        "shop.orders.Ordered",
        "shop.orders.Notify",
        &[],
    )];

    let ir = fixture.resolved();
    let binding = &ir.bindings[&BindingName::new("notify-on-ordered").expect("a name")];
    assert!(binding.mapping.is_empty());
}

#[test]
fn a_literal_fills_an_input_and_is_recorded_as_unverified() {
    let mut fixture = crossing("shop.orders.Email");
    fixture.commands = vec![command(
        "shop.orders.Notify",
        vec![
            field("recipient", "shop.orders.Email"),
            field("template", "String"),
        ],
        vec![outcome("sent", &[], None)],
    )];
    fixture.bindings = vec![binding(
        "notify-on-ordered",
        "shop.orders.Ordered",
        "shop.orders.Notify",
        &[
            ("recipient", "event.customer_email"),
            ("template", "ordered"),
        ],
    )];

    let ir = fixture.resolved();
    let binding = &ir.bindings[&BindingName::new("notify-on-ordered").expect("a name")];
    let targets: Vec<&str> = binding
        .mapping
        .iter()
        .map(|entry| entry.target.as_str())
        .collect();
    assert_eq!(
        targets,
        vec!["recipient", "template"],
        "the command's input order, not the document's"
    );
    assert!(matches!(
        binding.mapping[1].value,
        ResolvedMappingValue::Literal { .. }
    ));
}

// ---- an outcome's payload ------------------------------------------------------------------

/// [`crossing`] turned inward: the command emits the event and declares where its payload comes
/// from, and the binding is dropped so the one construct under test is the one that can refuse.
fn determining(input: &str, payload: &[(&str, &str)]) -> Fixture {
    let mut fixture = crossing(input);
    fixture.bindings = Vec::new();
    let mut sent = outcome("sent", &["shop.orders.Ordered"], None);
    sent.payload.insert(
        name("shop.orders.Ordered"),
        payload
            .iter()
            .map(|(target, source)| ((*target).to_owned(), PayloadSource::parse(source)))
            .collect(),
    );
    fixture.commands = vec![command(
        "shop.orders.Notify",
        vec![field("recipient", input)],
        vec![sent],
    )];
    fixture
}

#[test]
fn a_payload_filling_a_field_the_event_does_not_carry_is_refused_by_this_pass_too() {
    // `ess-domain` refuses the same document; built field by field, nothing has checked it yet,
    // which is the state the backstop exists for.
    let diagnostics =
        determining("shop.orders.Email", &[("customer_mail", "input.recipient")]).refused();
    assert!(
        diagnostics.contains(codes::PAYLOAD_FILLS_UNDECLARED_FIELD),
        "{diagnostics}"
    );
}

#[test]
fn a_payload_between_two_distinct_types_with_no_conversion_is_refused() {
    let diagnostics = determining(
        "shop.orders.Address",
        &[("customer_email", "input.recipient")],
    )
    .refused();
    assert!(
        diagnostics.contains(codes::COMMAND_TYPE_MISMATCH),
        "{diagnostics}"
    );

    // Design §29's shape, exactly as the binding mapping's mismatch carries it: the two paths and
    // the two types arrive as fields, so an agent repairing this does not parse a sentence.
    let diagnostic = diagnostics
        .as_slice()
        .iter()
        .find(|diagnostic| diagnostic.code == codes::COMMAND_TYPE_MISMATCH)
        .expect("the mismatch");
    let typed: Vec<(&str, &str, bool)> = diagnostic
        .details
        .iter()
        .filter_map(|detail| match detail {
            Detail::Typed {
                subject,
                type_ref,
                requires,
            } => Some((subject.as_str(), type_ref.as_str(), *requires)),
            _ => None,
        })
        .collect();
    assert_eq!(
        typed,
        vec![
            ("shop.orders.Notify.recipient", "shop.orders.Address", false),
            (
                "shop.orders.Ordered.customer_email",
                "shop.orders.Email",
                true
            ),
        ]
    );
}

#[test]
fn a_declared_conversion_makes_the_same_payload_legal_and_the_ir_records_why() {
    let mut fixture = determining(
        "shop.orders.Address",
        &[("customer_email", "input.recipient")],
    );
    fixture.conversions = vec![Conversion {
        from: TypeRef::Named(name("shop.orders.Address")),
        to: TypeRef::Named(name("shop.orders.Email")),
        because: "a deliverable address is written to the order as its email".to_owned(),
    }];

    let ir = fixture.resolved();
    let sent = &ir.commands[&name("shop.orders.Notify")].outcomes[0];
    assert_eq!(sent.payload.len(), 1);
    assert_eq!(
        sent.payload[0].fields[0].conversion.as_deref(),
        Some("a deliverable address is written to the order as its email"),
        "the reason someone wrote down is what a generator has to emit the conversion for"
    );
}

#[test]
fn a_payload_reading_an_input_the_command_does_not_take_is_refused() {
    let diagnostics =
        determining("shop.orders.Email", &[("customer_email", "input.recipint")]).refused();
    assert!(
        diagnostics.contains(codes::COMMAND_UNDECLARED_REFERENCE),
        "{diagnostics}"
    );
}

#[test]
fn a_payload_for_an_event_the_branch_does_not_emit_is_refused_here_too() {
    let mut fixture = determining("shop.orders.Email", &[]);
    let mut sent = outcome("sent", &[], None);
    sent.payload.insert(
        name("shop.orders.Ordered"),
        [(
            "customer_email".to_owned(),
            PayloadSource::parse("input.recipient"),
        )]
        .into(),
    );
    fixture.commands = vec![command(
        "shop.orders.Notify",
        vec![field("recipient", "shop.orders.Email")],
        vec![sent],
    )];

    let diagnostics = fixture.refused();
    assert!(
        diagnostics.contains(codes::COMMAND_UNDECLARED_REFERENCE),
        "{diagnostics}"
    );
}

#[test]
fn a_resolved_payload_is_in_the_events_declaration_order_and_a_literal_is_recorded_as_unverified() {
    let mut fixture = determining(
        "shop.orders.Email",
        &[
            ("note", "as ordered"),
            ("customer_email", "input.recipient"),
        ],
    );
    fixture.events = vec![event(
        "shop.orders.Ordered",
        vec![
            field("customer_email", "shop.orders.Email"),
            field("note", "String"),
        ],
    )];

    let ir = fixture.resolved();
    let sent = &ir.commands[&name("shop.orders.Notify")].outcomes[0];
    let targets: Vec<&str> = sent.payload[0]
        .fields
        .iter()
        .map(|entry| entry.target.as_str())
        .collect();
    assert_eq!(
        targets,
        vec!["customer_email", "note"],
        "the event's declaration order, not the document's"
    );
    assert!(matches!(
        sent.payload[0].fields[1].value,
        ResolvedPayloadValue::Literal { .. }
    ));
    assert!(matches!(
        &sent.payload[0].fields[0].value,
        ResolvedPayloadValue::InputField { field, .. } if field == "recipient"
    ));
}

// ---- layers `ess-domain` validates ---------------------------------------------------------

#[test]
fn a_component_accepting_a_command_nobody_declares_is_reported_once_as_never_validated() {
    let diagnostics = Fixture {
        components: vec![ComponentSpec {
            name: ComponentName::new("order-service").expect("a valid component name"),
            owns: [name(DOMAIN)].into_iter().collect(),
            accepts: [name("shop.orders.Place")].into_iter().collect(),
            publishes: BTreeSet::new(),
            naming: Naming::default(),
        }],
        ..Fixture::default()
    }
    .refused();

    // The rule belongs to `ess-domain`'s `validate_components`, so it is not restated here under a
    // second code. What this pass reports is its own precondition: it was handed a specification
    // that never validated, and it will not return an IR quietly missing a component.
    assert!(
        diagnostics.contains(Code::new(
            codes::family::COMPONENT,
            codes::class::UNDECLARED
        )),
        "{diagnostics}"
    );
    assert_eq!(diagnostics.len(), 1, "{diagnostics}");
}

#[test]
fn a_workload_for_a_component_nobody_declares_is_reported_once_as_never_validated() {
    let diagnostics = Fixture {
        workloads: vec![Workload {
            component: ComponentName::new("order-service").expect("a valid component name"),
            replicas: Replicas::default(),
            stateless: true,
            requires: Vec::new(),
        }],
        ..Fixture::default()
    }
    .refused();

    assert!(
        diagnostics.contains(Code::new(codes::family::TOPOLOGY, codes::class::UNDECLARED)),
        "{diagnostics}"
    );
    assert_eq!(diagnostics.len(), 1, "{diagnostics}");
}

// ---- accumulation --------------------------------------------------------------------------

#[test]
fn every_problem_is_reported_in_one_pass() {
    let diagnostics = Fixture {
        events: vec![event(
            "shop.orders.Ordered",
            vec![field("total", "shop.orders.Money")],
        )],
        commands: vec![command(
            "shop.orders.Place",
            Vec::new(),
            vec![
                outcome("accepted", &["shop.orders.Shipped"], None),
                outcome("rejected", &[], Some("shop.orders.Unpayable")),
            ],
        )],
        ..Fixture::default()
    }
    .refused();

    for code in [
        codes::EVENT_UNDECLARED_REFERENCE,
        codes::COMMAND_UNDECLARED_REFERENCE,
    ] {
        assert!(diagnostics.contains(code), "{code} missing:\n{diagnostics}");
    }
    assert_eq!(
        diagnostics.len(),
        3,
        "three defects, three diagnostics, one pass:\n{diagnostics}"
    );
}

#[test]
fn a_defect_this_pass_reports_is_not_reported_again_by_the_bridge() {
    // Two defects in two layers: an event field with no type, which this pass refuses because it
    // cannot mint the handle, and a component accepting an undeclared command, which `ess-domain`
    // owns. Two diagnostics, not three: bridging the domain crate's whole verdict would have
    // reported the first one twice.
    let diagnostics = Fixture {
        events: vec![event(
            "shop.orders.Ordered",
            vec![field("total", "shop.orders.Money")],
        )],
        components: vec![ComponentSpec {
            name: ComponentName::new("order-service").expect("a valid component name"),
            owns: [name(DOMAIN)].into_iter().collect(),
            accepts: [name("shop.orders.Place")].into_iter().collect(),
            publishes: BTreeSet::new(),
            naming: Naming::default(),
        }],
        ..Fixture::default()
    }
    .refused();

    assert!(
        diagnostics.contains(codes::EVENT_UNDECLARED_REFERENCE),
        "{diagnostics}"
    );
    assert!(
        diagnostics.contains(Code::new(
            codes::family::COMPONENT,
            codes::class::UNDECLARED
        )),
        "{diagnostics}"
    );
    assert_eq!(diagnostics.len(), 2, "{diagnostics}");
}

#[test]
fn a_reference_to_a_declaration_that_itself_failed_to_resolve_is_reported_once() {
    let diagnostics = Fixture {
        commands: vec![command(
            "shop.orders.Place",
            vec![field("total", "shop.orders.Money")],
            vec![outcome("accepted", &[], None)],
        )],
        components: vec![ComponentSpec {
            name: ComponentName::new("order-service").expect("a valid component name"),
            owns: [name(DOMAIN)].into_iter().collect(),
            accepts: [name("shop.orders.Place")].into_iter().collect(),
            publishes: BTreeSet::new(),
            naming: Naming::default(),
        }],
        ..Fixture::default()
    }
    .refused();

    // `Place` is declared; it did not resolve because its input names a type nobody declares. The
    // component that accepts it must not be reported as accepting something undeclared, which would
    // send the reader to the wrong file.
    assert_eq!(diagnostics.len(), 1, "{diagnostics}");
    assert!(
        diagnostics.contains(codes::COMMAND_UNDECLARED_REFERENCE),
        "{diagnostics}"
    );
}
