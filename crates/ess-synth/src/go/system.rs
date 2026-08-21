//! The system-package emitter: the bindings, and the one transport the specification requires.
//!
//! Semantically identical to the Rust emitter's system crate, and it has to be: the transport is
//! *derived* from the model — `at_least_once` is the only delivery guarantee the specification
//! declares, and the component surfaces say who publishes what and who accepts what — so both
//! targets are writing down the same conclusion. Published events land on an append-only log, a
//! pump delivers each to every binding that reacts to it, the binding's `on_failure:` answers a
//! **declared refusal** (`escalate` publishes the declared event through its obligation, `retry`
//! holds the event for the next pump, `drop` gives up silently), and an unmet obligation
//! propagates instead of being routed into the policy — a workspace being unfinished is not a
//! delivery failing.
//!
//! Two things are shaped by Go rather than by the model, both deliberate:
//!
//! * **One method per generated delivery.** Go has no `let` shadowing inside a `case` block, so
//!   two bindings reacting to one event would redeclare `input` in the same scope. Each delivery
//!   gets its own method instead, which also keeps the pump readable.
//! * **A nil answer from a lift is dropped, not logged.** Lifting a component's outbox entry onto
//!   the system's log is a `switch` Go cannot prove total, so the generated code says what it does
//!   with the case that cannot arise rather than pretending it cannot.

use std::collections::BTreeSet;
use std::fmt::Write as _;

use ess_compiler::ir::{
    EssIr, EventHandle, ResolvedBinding, ResolvedComponent, ResolvedFailure, ResolvedMappingValue,
};
use ess_gen::Artifact;

use crate::plan::{
    accepting_components, determined_input, Capability, CapabilityKind, DeterminedInput,
    SynthesisPlan,
};

use super::layout::{event_variants, Layout};
use super::obligation::zero_of;
use super::refusal::TargetRefusals;
use super::{name, record, Emit, EXHAUSTIVENESS_NOTE};

/// One binding whose delivery the plan generates and this target emits.
struct Delivery<'a> {
    /// The binding.
    binding: &'a ResolvedBinding,
    /// The one component that accepts its command.
    acceptor: &'a ResolvedComponent,
    /// Whether the transformation is generated (a call to the emitted function) or owed (a call
    /// through the transformation obligation's interface).
    transformation_generated: bool,
}

/// The system package: emitted whenever the specification declares an interaction layer at all.
pub(super) fn system_package(
    ir: &EssIr,
    plan: &SynthesisPlan,
    layout: &Layout,
    refusals: &TargetRefusals,
    covered: &mut BTreeSet<Capability>,
    stubbed: &mut BTreeSet<Capability>,
) -> Option<Artifact> {
    if ir.components.is_empty() && ir.bindings.is_empty() {
        return None;
    }
    let package = layout.system();
    let emit = Emit::new(ir, layout, package, None);

    let mut deliveries: Vec<Delivery<'_>> = Vec::new();
    for binding in ir.bindings.values() {
        let source = binding.name.to_string();
        if !super::cover(
            plan,
            refusals,
            covered,
            CapabilityKind::BindingDelivery,
            &source,
        ) {
            continue;
        }
        let acceptors = accepting_components(ir, binding);
        assert_eq!(
            acceptors.len(),
            1,
            "the plan generated delivery for `{source}` without exactly one acceptor; that is a \
             defect in ess-synth"
        );
        deliveries.push(Delivery {
            binding,
            acceptor: acceptors[0],
            transformation_generated: plan
                .is_generated(CapabilityKind::BindingTransformation, &source)
                && !refusals.refuses_kind(CapabilityKind::BindingTransformation, &source),
        });
    }

    let components: Vec<&ResolvedComponent> = ir
        .components
        .values()
        .filter(|component| {
            !refusals.refuses_kind(CapabilityKind::ComponentPort, &component.name.to_string())
        })
        .collect();

    let mut body = String::new();
    system_event(&mut body, &emit);
    lifts(&mut body, &emit, &components);
    binding_invocations(&mut body, &emit, &deliveries);
    transformations(&mut body, &emit, plan, refusals, covered);
    let owed = obligations(&mut body, &emit, plan, refusals, stubbed);
    assembled(&mut body, &emit, &components, &deliveries, &owed);

    let doc = format!(
        "// Package {} is the `{}` system, {}: its components assembled, its bindings wired, and \
         its one\n// transport.\n//\n// The transport is derived from the specification, not \
         chosen: `at_least_once` is the only\n// delivery guarantee the model declares, so \
         published events land on an append-only log and\n// a pump delivers each to every binding \
         that reacts to it. The log is the system's observable\n// record, and so is the record of \
         what each binding invoked. What no specification\n// determines — how an escalation event \
         is filled, behaviour behind the ports — stays an\n// obligation; see the PLAN.md beside \
         this module.\n",
        package.name, ir.system, ir.version
    );
    Some(emit.file(&plan.provenance, &doc, &body))
}

/// The transport's event type: one variant per event the system can carry.
fn system_event(out: &mut String, emit: &Emit<'_>) {
    let interface = emit.layout.system_name("SystemEvent");
    let _ = writeln!(
        out,
        "\n// {interface} is an event on the system's log: everything any component publishes, and \
         everything\n// a binding escalates into.\n//"
    );
    out.push_str(EXHAUSTIVENESS_NOTE);
    let _ = writeln!(
        out,
        "type {interface} interface {{\n\t{}()\n}}",
        name::marker(interface)
    );
    for event in emit.layout.system_events() {
        let variant = emit.layout.system_event(event.name());
        let _ = writeln!(
            out,
            "\n// {variant} is `{event}`.\ntype {variant} struct {{\n\t// Event is what was \
             published.\n\tEvent {}\n}}\n\nfunc ({variant}) {}() {{}}",
            emit.reference(event.name()),
            name::marker(interface)
        );
    }
}

/// One lift per component's outbox type, so collecting an outbox is a conversion rather than a
/// re-statement of which events exist.
fn lifts(out: &mut String, emit: &Emit<'_>, components: &[&ResolvedComponent]) {
    let interface = emit.layout.system_name("SystemEvent");
    for component in components {
        let package = emit.layout.component(&component.name);
        let function = lift(component);
        let published = emit.qualify(package, emit.layout.published(&component.name));
        let events: BTreeSet<&EventHandle> = component.publishes.iter().collect();
        let variants = event_variants(emit.ir, emit.layout, &events);
        let _ = writeln!(
            out,
            "\n// {function} lifts one of `{}`'s published events onto the system's log.\n//\n// \
             `nil` where the value is a variant this module did not declare, which only Go's zero \
             value\n// can produce: the compiler cannot prove the switch below total, and the \
             caller drops what\n// it cannot place rather than logging a nil occurrence.\nfunc \
             {function}(event {published}) {} {{",
            component.name, interface
        );
        if variants.is_empty() {
            out.push_str("\treturn nil\n}\n");
            continue;
        }
        out.push_str("\tswitch value := event.(type) {\n");
        for event in variants.keys() {
            let _ = writeln!(
                out,
                "\tcase {}:\n\t\treturn {}{{Event: value.Event}}",
                emit.qualify(
                    package,
                    emit.layout.published_variant(&component.name, event.name())
                ),
                emit.layout.system_event(event.name())
            );
        }
        out.push_str("\t}\n\treturn nil\n}\n");
    }
}

/// The unexported function that lifts one component's outbox entries onto the log.
///
/// Unexported, so it can never collide with an allocated name: every identifier the name table
/// hands out for this package is exported, and Go's export rule is a spelling rule.
fn lift(component: &ResolvedComponent) -> String {
    format!("liftFrom{}", name::exported(&component.name.to_string()))
}

/// The transport's second record: one variant per generated delivery, holding what was passed.
fn binding_invocations(out: &mut String, emit: &Emit<'_>, deliveries: &[Delivery<'_>]) {
    if deliveries.is_empty() {
        return;
    }
    let interface = emit.layout.system_name("BindingInvocation");
    let _ = writeln!(
        out,
        "\n// {interface} is one command a binding invoked, and the input it passed — the \
         transport's own\n// record.\n//\n// Recorded by the pump at the moment of invocation, so \
         what a binding actually passed is\n// observable from outside — a conformance run holds a \
         mapping to its words with exactly\n// this — without instrumenting the component \
         underneath.\n//"
    );
    out.push_str(EXHAUSTIVENESS_NOTE);
    let _ = writeln!(
        out,
        "type {interface} interface {{\n\t{}()\n}}",
        name::marker(interface)
    );
    for delivery in deliveries {
        let variant = emit.layout.invocation(&delivery.binding.name.to_string());
        let _ = writeln!(
            out,
            "\n// {variant} is `{}` invoking `{}`.\ntype {variant} struct {{\n\t// Input is what \
             the binding passed.\n\tInput {}\n}}\n\nfunc ({variant}) {}() {{}}",
            delivery.binding.name,
            delivery.binding.command,
            emit.reference(delivery.binding.command.name()),
            name::marker(interface)
        );
    }
}

/// The generated transformations: one function per binding whose mapping the specification fully
/// determines.
fn transformations(
    out: &mut String,
    emit: &Emit<'_>,
    plan: &SynthesisPlan,
    refusals: &TargetRefusals,
    covered: &mut BTreeSet<Capability>,
) {
    for binding in emit.ir.bindings.values() {
        let source = binding.name.to_string();
        if !super::cover(
            plan,
            refusals,
            covered,
            CapabilityKind::BindingTransformation,
            &source,
        ) {
            continue;
        }
        let function = emit.layout.transform(&source);
        let _ = writeln!(
            out,
            "\n// {function} reads a `{}` as `{}` input — the binding `{source}`.\n//\n// Fully \
             determined by the specification: every input is filled from an event field —\n// \
             through the declared crossing where one is named — from a literal the target admits, \
             or\n// left absent where the input is optional and the binding says nothing.\nfunc \
             {function}(event {}) {} {{",
            binding.event,
            binding.command,
            emit.reference(binding.event.name()),
            emit.reference(binding.command.name()),
        );
        if emit.ir.command(&binding.command).input.is_empty() {
            let _ = writeln!(
                out,
                "\treturn {}{{}}\n}}",
                emit.reference(binding.command.name())
            );
            continue;
        }
        let _ = writeln!(out, "\treturn {}{{", emit.reference(binding.command.name()));
        let mut taken = std::collections::BTreeMap::new();
        for field in &emit.ir.command(&binding.command).input {
            let determined = determined_input(emit.ir, binding, field).unwrap_or_else(|| {
                panic!(
                    "the plan generated the transformation of `{source}` with an undetermined \
                         mapping for `{}`; that is a defect in ess-synth",
                    field.name
                )
            });
            let ident = super::items::field_ident(&mut taken, &field.name);
            let (note, expression) = match determined {
                DeterminedInput::Copy { field } => (
                    format!("copied from the event's `{field}`"),
                    format!("event.{}", name::exported(field)),
                ),
                DeterminedInput::Convert { field, to } => (
                    format!("read from the event's `{field}` through the declared crossing"),
                    format!(
                        "{}(event.{})",
                        emit.qualify(
                            emit.layout.package_of(to.name()),
                            emit.layout.convert(&crossing(binding, field))
                        ),
                        name::exported(field)
                    ),
                ),
                DeterminedInput::Literal { value, wraps } => {
                    let mut expression = format!("{value:?}");
                    for wrap in wraps.iter().rev() {
                        expression = format!(
                            "{}({expression})",
                            emit.qualify(
                                emit.layout.package_of(wrap.name()),
                                emit.layout.ctor(wrap.name())
                            )
                        );
                    }
                    (
                        format!("the literal `{value}` the binding wrote"),
                        expression,
                    )
                }
                DeterminedInput::Variant { of, value } => (
                    format!("the declared variant `{value}`"),
                    format!(
                        "{}{{}}",
                        emit.qualify(
                            emit.layout.package_of(of.name()),
                            emit.layout.variant(of.name(), value)
                        )
                    ),
                ),
                DeterminedInput::Omitted => (
                    "left absent: the input is optional and the binding says nothing".to_owned(),
                    "nil".to_owned(),
                ),
            };
            let _ = writeln!(out, "\t\t// {ident} is {note}.\n\t\t{ident}: {expression},");
        }
        out.push_str("\t}\n}\n");
    }
}

/// The plan's key for the crossing one mapping goes through.
///
/// The plan already decided the crossing is mechanical; this recovers *which* declared conversion
/// it was, because the emitted call names the function that conversion was generated as, and the
/// plan files a conversion under both its ends.
fn crossing(binding: &ResolvedBinding, field: &str) -> String {
    let mapping = binding
        .mapping
        .iter()
        .find(|mapping| match &mapping.value {
            ResolvedMappingValue::EventField { field: from, .. } => from == field,
            ResolvedMappingValue::Literal { .. } => false,
        })
        .expect("a converted mapping came from an event field");
    let ResolvedMappingValue::EventField { type_ref, .. } = &mapping.value else {
        unreachable!("the search above kept only event-field mappings")
    };
    format!("{type_ref} -> {}", mapping.target_type)
}

/// One owed binding capability, as its interface and stub need it.
struct Owed {
    /// The plan capability the stub stands in for.
    kind: CapabilityKind,
    /// The binding.
    source: String,
    /// The interface's name, which the system's `Obligations` embeds.
    interface: String,
    /// The method's name.
    method: String,
    /// The method's one-line doc.
    method_doc: String,
    /// The parameter's identifier and type.
    parameter: (String, String),
    /// The first result type.
    answer: String,
    /// Its zero value, which the refusing stub returns.
    zero: String,
    /// The plan's reason.
    reason: String,
    /// The plan's contract.
    contract: String,
    /// The interface's one-line heading.
    heading: String,
}

/// The system's own obligations: an interface per owed binding capability, and the stub refusing
/// them all.
fn obligations(
    out: &mut String,
    emit: &Emit<'_>,
    plan: &SynthesisPlan,
    refusals: &TargetRefusals,
    stubbed: &mut BTreeSet<Capability>,
) -> Vec<Owed> {
    let owed = owed_by_system(emit, plan, refusals);
    if owed.is_empty() {
        return owed;
    }
    render_system_obligations(out, emit, &owed, stubbed);
    owed
}

/// What the system owes, in binding order: owed transformations, then escalations.
fn owed_by_system(emit: &Emit<'_>, plan: &SynthesisPlan, refusals: &TargetRefusals) -> Vec<Owed> {
    let mut owed = Vec::new();
    for binding in emit.ir.bindings.values() {
        let source = binding.name.to_string();
        let pascal = name::exported(&source);
        if let Some(obligation) = plan.obligation_of(CapabilityKind::BindingTransformation, &source)
        {
            if !refusals.refuses_kind(CapabilityKind::BindingTransformation, &source) {
                let answer = emit.reference(binding.command.name());
                owed.push(Owed {
                    kind: CapabilityKind::BindingTransformation,
                    source: source.clone(),
                    interface: emit.layout.transformation(&source).to_owned(),
                    method: format!("{pascal}Input"),
                    method_doc: format!(
                        "reads a `{}` as `{}` input, where the specification does not say how.",
                        binding.event, binding.command
                    ),
                    parameter: ("event".to_owned(), emit.reference(binding.event.name())),
                    zero: zero_of(&answer),
                    answer,
                    reason: obligation.reason.describes(),
                    contract: obligation.contract.clone(),
                    heading: format!(
                        "the transformation of `{source}` — an implementation obligation."
                    ),
                });
            }
        }
        if let Some(obligation) = plan.obligation_of(CapabilityKind::BindingEscalation, &source) {
            if refusals.refuses_kind(CapabilityKind::BindingEscalation, &source) {
                continue;
            }
            let ResolvedFailure::Escalate { emits } = binding.on_failure() else {
                panic!(
                    "the plan owes an escalation for `{source}`, whose failure policy does not \
                     escalate; that is a defect in ess-synth"
                );
            };
            let answer = emit.reference(emits.name());
            owed.push(Owed {
                kind: CapabilityKind::BindingEscalation,
                source: source.clone(),
                interface: emit.layout.escalation(&source).to_owned(),
                method: format!("{pascal}Escalation"),
                method_doc: format!(
                    "builds the declared `{emits}` from the invocation that was given up on."
                ),
                parameter: ("failed".to_owned(), emit.reference(binding.command.name())),
                zero: zero_of(&answer),
                answer,
                reason: obligation.reason.describes(),
                contract: obligation.contract.clone(),
                heading: format!("the escalation of `{source}` — an implementation obligation."),
            });
        }
    }
    owed
}

/// The interface half and the stub half, in that order: a reader meets the contract before the
/// refusal that stands in for it.
fn render_system_obligations(
    out: &mut String,
    emit: &Emit<'_>,
    owed: &[Owed],
    stubbed: &mut BTreeSet<Capability>,
) {
    for spec in owed {
        let _ = writeln!(
            out,
            "\n// {} is {}\n//\n// Why it is not generated: {}.\n//\n// Contract: {}.\ntype {} \
             interface {{\n\t// {} {}\n\t//\n\t// The second result is the typed refusal of an \
             obligation nothing has satisfied; a\n\t// satisfying implementation never returns \
             one.\n\t{}({} {}) ({}, {})\n}}",
            spec.interface,
            spec.heading,
            spec.reason,
            spec.contract,
            spec.interface,
            spec.method,
            spec.method_doc,
            spec.method,
            spec.parameter.0,
            spec.parameter.1,
            spec.answer,
            emit.unmet(),
        );
    }

    let unimplemented = emit.layout.unimplemented(emit.layout.system());
    let _ = writeln!(
        out,
        "\n// {unimplemented} satisfies every obligation of the system by refusing in the type \
         system.\n//\n// Each method returns the typed refusal naming what is owed — never a \
         panic, never a guessed\n// value — so a system built on this stub compiles and reports \
         its own gaps.\ntype {unimplemented} struct{{}}"
    );
    for spec in owed {
        record(stubbed, spec.kind, &spec.source);
        let _ = writeln!(
            out,
            "\n// {} refuses: {}\nfunc ({unimplemented}) {}({} {}) ({}, {}) {{\n\treturn {}, \
             {}\n}}",
            spec.method,
            spec.heading,
            spec.method,
            spec.parameter.0,
            spec.parameter.1,
            spec.answer,
            emit.unmet(),
            spec.zero,
            emit.unmet_literal(spec.kind, &spec.source),
        );
    }
}

/// The assembled system: its components, the log, the pump, and one method per generated delivery.
fn assembled(
    out: &mut String,
    emit: &Emit<'_>,
    components: &[&ResolvedComponent],
    deliveries: &[Delivery<'_>],
    owed: &[Owed],
) {
    // Which of the system's obligations the pump actually calls: those of the bindings it
    // delivers.
    let mut used: Vec<&Owed> = Vec::new();
    let mut retries = false;
    for delivery in deliveries {
        let source = delivery.binding.name.to_string();
        for spec in owed {
            if spec.source == source {
                let needed = match spec.kind {
                    CapabilityKind::BindingTransformation => !delivery.transformation_generated,
                    _ => true,
                };
                if needed {
                    used.push(spec);
                }
            }
        }
        if matches!(delivery.binding.on_failure(), ResolvedFailure::Retry) {
            retries = true;
        }
    }
    let with_obligations = !used.is_empty();

    obligations_interface(out, emit, &used, with_obligations);
    system_struct(out, emit, components, deliveries, with_obligations, retries);
    constructor(out, emit, components, with_obligations);
    observers(out, emit, deliveries);
    pump(out, emit, components, deliveries, retries);
    for delivery in deliveries {
        delivery_method(out, emit, delivery);
    }
}

/// The bundle of seams the pump calls, where it calls any.
fn obligations_interface(
    out: &mut String,
    emit: &Emit<'_>,
    used: &[&Owed],
    with_obligations: bool,
) {
    let obligations = emit.layout.system_name("Obligations");
    if with_obligations {
        let _ = writeln!(
            out,
            "\n// {obligations} is what the system itself owes its implementor: exactly the seams \
             the pump\n// calls, bundled.\ntype {obligations} interface {{"
        );
        for spec in used {
            let _ = writeln!(out, "\t{}", spec.interface);
        }
        out.push_str("}\n");
    }
}

/// The assembled system's fields: the ports, what it owes, and the transport's own records.
fn system_struct(
    out: &mut String,
    emit: &Emit<'_>,
    components: &[&ResolvedComponent],
    deliveries: &[Delivery<'_>],
    with_obligations: bool,
    retries: bool,
) {
    let system = emit.layout.system_name("System");
    let obligations = emit.layout.system_name("Obligations");
    let invocation = emit.layout.system_name("BindingInvocation");
    let event = emit.layout.system_name("SystemEvent");
    let _ = writeln!(
        out,
        "\n// {system} is the `{}` system: every component behind its port, and the transport \
         between them.\n//\n// The component fields are exported because commands enter the system \
         through a component's\n// own port; the log and its delivery cursor are not, because \
         publishing happens by pumping,\n// not by writing history directly.\ntype {system} struct \
         {{",
        emit.ir.system
    );
    for component in components {
        let _ = writeln!(
            out,
            "\t// {} is the `{}` component.\n\t{} *{}",
            name::exported(&component.name.to_string()),
            component.name,
            name::exported(&component.name.to_string()),
            emit.qualify(
                emit.layout.component(&component.name),
                emit.layout.port(&component.name)
            )
        );
    }
    if with_obligations {
        let _ = writeln!(
            out,
            "\t// obligations is what nothing in this module can determine.\n\tobligations \
             {obligations}"
        );
    }
    if !deliveries.is_empty() {
        let _ = writeln!(
            out,
            "\t// invocations records every command a binding invoked, with what it \
             passed.\n\tinvocations []{invocation}"
        );
    }
    let _ = writeln!(
        out,
        "\t// published is the log, in publication order.\n\tpublished []{event}\n\t// cursor is \
         how far the pump has delivered.\n\tcursor int"
    );
    if retries {
        let _ = writeln!(
            out,
            "\t// retries holds what a declared refusal asked to be delivered again.\n\tretries \
             []{event}"
        );
    }
    out.push_str("}\n");
}

/// Construction, possible whatever the obligations are.
fn constructor(
    out: &mut String,
    emit: &Emit<'_>,
    components: &[&ResolvedComponent],
    with_obligations: bool,
) {
    let system = emit.layout.system_name("System");
    let new = emit.layout.system_name("NewSystem");
    let obligations = emit.layout.system_name("Obligations");
    let mut parameters: Vec<String> = components
        .iter()
        .map(|component| {
            format!(
                "{} *{}",
                lower(&name::exported(&component.name.to_string())),
                emit.qualify(
                    emit.layout.component(&component.name),
                    emit.layout.port(&component.name)
                )
            )
        })
        .collect();
    if with_obligations {
        parameters.push(format!("obligations {obligations}"));
    }
    let _ = writeln!(
        out,
        "\n// {new} assembles the system from its components{}.\nfunc {new}({}) *{system} {{",
        if with_obligations {
            " and the owed obligations"
        } else {
            ""
        },
        parameters.join(", ")
    );
    // A system with no components and nothing owed sets no field, and `&System{}` is what gofmt
    // writes for that — an open-and-close literal with a newline inside it is not.
    if components.is_empty() && !with_obligations {
        let _ = writeln!(out, "\treturn &{system}{{}}\n}}");
    } else {
        let _ = writeln!(out, "\treturn &{system}{{");
        for component in components {
            let field = name::exported(&component.name.to_string());
            let _ = writeln!(
                out,
                "\t\t// {field} is the `{}` component's port.\n\t\t{field}: {},",
                component.name,
                lower(&field)
            );
        }
        if with_obligations {
            let _ = writeln!(
                out,
                "\t\t// obligations is what the pump calls where the specification determines \
                 nothing.\n\t\tobligations: obligations,"
            );
        }
        out.push_str("\t}\n}\n");
    }
}

/// The two observable records: everything published, and what each binding invoked.
fn observers(out: &mut String, emit: &Emit<'_>, deliveries: &[Delivery<'_>]) {
    let system = emit.layout.system_name("System");
    let invocation = emit.layout.system_name("BindingInvocation");
    let event = emit.layout.system_name("SystemEvent");
    let _ = writeln!(
        out,
        "\n// Published is everything published so far, in publication order — the system's \
         observable\n// record.\nfunc (s *{system}) Published() []{event} {{\n\treturn \
         s.published\n}}"
    );
    if !deliveries.is_empty() {
        let _ = writeln!(
            out,
            "\n// Invocations is every command a binding invoked so far, in invocation order, \
             with what it\n// passed.\nfunc (s *{system}) Invocations() []{invocation} \
             {{\n\treturn s.invocations\n}}"
        );
    }
}

/// The pump: collection, delivery, redelivery, and the held-back attempts a retry policy asks for.
fn pump(
    out: &mut String,
    emit: &Emit<'_>,
    components: &[&ResolvedComponent],
    deliveries: &[Delivery<'_>],
    retries: bool,
) {
    let system = emit.layout.system_name("System");
    let event = emit.layout.system_name("SystemEvent");
    let unmet = emit.unmet();

    let _ = writeln!(
        out,
        "\n// Pump delivers until quiescent: collects every component's outbox onto the log, then \
         delivers\n// each logged event to every binding that reacts to it — at least once each, \
         which is the\n// guarantee the specification declares.\n//\n// The result carries the \
         first unmet obligation that delivery could not route around; the log\n// keeps everything \
         already published. A specification whose bindings feed each other without\n// end will not \
         quiesce, and this pump will not pretend otherwise.\nfunc (s *{system}) Pump() {unmet} {{"
    );
    if retries {
        out.push_str(
            "\t// Held-back deliveries first: one more attempt per pump is the redelivery\n\t// \
             schedule this transport provides.\n\tretrying := s.retries\n\ts.retries = nil\n\tfor \
             _, held := range retrying {\n\t\tif unmet := s.deliver(held); unmet != nil \
             {\n\t\t\treturn unmet\n\t\t}\n\t}\n",
        );
    }
    out.push_str("\tfor {\n\t\ts.collect()\n\t\tif s.cursor == len(s.published) {\n\t\t\treturn nil\n\t\t}\n");
    if deliveries.is_empty() {
        out.push_str("\t\ts.cursor++\n\t}\n}\n");
    } else {
        out.push_str(
            "\t\tevent := s.published[s.cursor]\n\t\ts.cursor++\n\t\tif unmet := \
             s.deliver(event); unmet != nil {\n\t\t\treturn unmet\n\t\t}\n\t}\n}\n",
        );
        let _ = writeln!(
            out,
            "\n// Redeliver delivers one already-published occurrence to every binding that reacts \
             to it,\n// again, then pumps until quiescent.\n//\n// The duplicate a delivery \
             guarantee of at least once explicitly permits: the occurrence is\n// not published a \
             second time — a second occurrence would be a different claim — but every\n// reacting \
             binding runs again, and what that causes lands on the log as usual.\nfunc (s \
             *{system}) Redeliver(event {event}) {unmet} {{\n\tif unmet := s.deliver(event); unmet \
             != nil {{\n\t\treturn unmet\n\t}}\n\treturn s.Pump()\n}}"
        );
    }

    let _ = writeln!(
        out,
        "\n// collect moves every component's outbox onto the log, in component order.\nfunc (s \
         *{system}) collect() {{"
    );
    for component in components {
        let field = name::exported(&component.name.to_string());
        let _ = writeln!(
            out,
            "\tfor _, published := range s.{field}.DrainOutbox() {{\n\t\tif lifted := \
             {}(published); lifted != nil {{\n\t\t\ts.published = append(s.published, \
             lifted)\n\t\t}}\n\t}}",
            lift(component)
        );
    }
    out.push_str("}\n");

    if deliveries.is_empty() {
        return;
    }
    let _ = writeln!(
        out,
        "\n// deliver delivers one logged event to every binding that reacts to it.\nfunc (s \
         *{system}) deliver(event {event}) {unmet} {{\n\tswitch value := event.(type) {{"
    );
    for logged in emit.layout.system_events() {
        let reacting: Vec<&Delivery<'_>> = deliveries
            .iter()
            .filter(|delivery| delivery.binding.event.name() == logged.name())
            .collect();
        let _ = writeln!(out, "\tcase {}:", emit.layout.system_event(logged.name()));
        for delivery in reacting {
            let _ = writeln!(
                out,
                "\t\tif unmet := s.{}(value.Event); unmet != nil {{\n\t\t\treturn \
                 unmet\n\t\t}}",
                delivery_name(delivery)
            );
        }
    }
    out.push_str("\t}\n\treturn nil\n}\n");
}

/// One binding's delivery, as its own method.
///
/// A method rather than an inline arm because Go declares a variable once per block: two bindings
/// reacting to one event would redeclare `input` in the same `case`, which Rust's `let` shadowing
/// makes a non-question.
fn delivery_method(out: &mut String, emit: &Emit<'_>, delivery: &Delivery<'_>) {
    let binding = delivery.binding;
    let source = binding.name.to_string();
    let system = emit.layout.system_name("System");
    let unmet = emit.unmet();
    let method = delivery_name(delivery);
    let acceptor = name::exported(&delivery.acceptor.name.to_string());
    let handler = emit.layout.declared(binding.command.name());
    let invocation = emit.layout.invocation(&source);
    let command = emit.ir.command(&binding.command);
    let refusals: Vec<String> = command
        .outcomes
        .iter()
        .filter(|outcome| outcome.error.is_some())
        .map(|outcome| {
            emit.qualify(
                emit.layout.package_of(&command.name),
                emit.layout
                    .outcome_variant(&command.name, outcome.name.as_str()),
            )
        })
        .collect();
    let successes: Vec<String> = command
        .outcomes
        .iter()
        .filter(|outcome| outcome.error.is_none())
        .map(|outcome| {
            emit.qualify(
                emit.layout.package_of(&command.name),
                emit.layout
                    .outcome_variant(&command.name, outcome.name.as_str()),
            )
        })
        .collect();

    let _ = writeln!(
        out,
        "\n// {method} delivers one `{}` to `{source}`: transform, record the invocation, invoke \
         the\n// acceptor's port, and answer a declared refusal with the declared policy \
         ({}, on failure\n// {}).\n//\n// An unmet obligation is deliberately not routed into the \
         policy: a port refusing because\n// its behaviour is owed is a fact about the module being \
         unfinished, not about a delivery.\nfunc (s *{system}) {method}(event {}) {unmet} {{",
        binding.event,
        ess_gen::graph::delivery_word(binding.delivery),
        binding.failure.as_str(),
        emit.reference(binding.event.name()),
    );
    if delivery.transformation_generated {
        let _ = writeln!(out, "\tinput := {}(event)", emit.layout.transform(&source));
    } else {
        let _ = writeln!(
            out,
            "\tinput, unmet := s.obligations.{}Input(event)\n\tif unmet != nil {{\n\t\treturn \
             unmet\n\t}}",
            name::exported(&source)
        );
    }
    let reads_outcome =
        !refusals.is_empty() && !matches!(binding.on_failure(), ResolvedFailure::Drop);
    let _ = writeln!(
        out,
        "\ts.invocations = append(s.invocations, {invocation}{{Input: input}})\n\t{}, refused \
         := s.{acceptor}.{handler}(input)\n\tif refused != nil {{\n\t\treturn \
         refused\n\t}}",
        if reads_outcome { "outcome" } else { "_" }
    );

    if refusals.is_empty() {
        out.push_str(
            "\t// No declared refusal exists, so this invocation cannot fail; what the outcome \
             carries\n\t// was already published by the port that produced it.\n\treturn nil\n}\n",
        );
        return;
    }
    failure_policy(out, emit, binding, &successes, &refusals);
}

/// The declared policy, on exactly the outcomes that carry an error.
///
/// A `switch` over the outcome interface rather than an `if`: the success arms are listed so the
/// generated code reads as the whole outcome set, which is the only compensation Go allows for
/// having no exhaustiveness check.
fn failure_policy(
    out: &mut String,
    emit: &Emit<'_>,
    binding: &ResolvedBinding,
    successes: &[String],
    refusals: &[String],
) {
    let source = binding.name.to_string();
    match binding.on_failure() {
        ResolvedFailure::Escalate { emits } => {
            out.push_str("\tswitch outcome.(type) {\n");
            for success in successes {
                let _ = writeln!(out, "\tcase {success}:");
            }
            let _ = writeln!(out, "\tcase {}:", refusals.join(", "));
            let _ = writeln!(
                out,
                "\t\t// The declared refusal is the failure the policy names: \
                 escalate.\n\t\tescalation, owed := s.obligations.{}Escalation(input)\n\t\tif owed \
                 != nil {{\n\t\t\treturn owed\n\t\t}}\n\t\ts.published = append(s.published, \
                 {}{{Event: escalation}})",
                name::exported(&source),
                emit.layout.system_event(emits.name())
            );
            out.push_str("\t}\n\treturn nil\n}\n");
        }
        ResolvedFailure::Retry => {
            out.push_str("\tswitch outcome.(type) {\n");
            for success in successes {
                let _ = writeln!(out, "\tcase {success}:");
            }
            let _ = writeln!(out, "\tcase {}:", refusals.join(", "));
            let _ = writeln!(
                out,
                "\t\t// The declared refusal is the failure the policy names: hold the event for \
                 the\n\t\t// next pump, which is one more at-least-once \
                 attempt.\n\t\ts.retries = append(s.retries, {}{{Event: event}})",
                emit.layout.system_event(binding.event.name())
            );
            out.push_str("\t}\n\treturn nil\n}\n");
        }
        ResolvedFailure::Drop => {
            out.push_str(
                "\t// `drop`: a declared refusal is given up silently, because that is what the \
                 author\n\t// wrote; an unmet obligation still propagated \
                 above.\n\treturn nil\n}\n",
            );
        }
    }
}

/// The unexported method one binding's delivery is emitted as.
fn delivery_name(delivery: &Delivery<'_>) -> String {
    format!(
        "deliver{}",
        name::exported(&delivery.binding.name.to_string())
    )
}

/// A Go identifier with its first letter lower-cased, for a parameter shadowing nothing.
fn lower(ident: &str) -> String {
    let mut characters = ident.chars();
    match characters.next() {
        Some(first) => first.to_lowercase().chain(characters).collect(),
        None => String::new(),
    }
}
