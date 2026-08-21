//! Renderers for the declarations that are types all the way down: named types, command
//! contracts, events, errors, views, and the mechanical conversions.
//!
//! Every renderer appends complete declarations to a package buffer and nothing else — no file
//! decisions, no plan decisions, both taken before it was called. Two encodings do the work Rust's
//! `enum` and tuple struct do there, and both are stated in the [module
//! documentation](super) and in the generated doc comments:
//!
//! * a **sealed interface** for every closed set of alternatives — enums, tagged unions and a
//!   command's outcomes are one construct here, because Go has one way to say "one of these";
//! * a **struct with an unexported field** for every newtype, so a bare `string` cannot become an
//!   `Email` by assignment the way `type Email string` would allow.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use ess_compiler::ir::{
    EssIr, EventHandle, ResolvedBody, ResolvedCommand, ResolvedConversion, ResolvedError,
    ResolvedEvent, ResolvedField, ResolvedOutcome, ResolvedType, ResolvedTypeRef, ResolvedView,
    TypeHandle,
};
use ess_domain::entity::Invariant;
use ess_domain::name::QualifiedName;
use std::collections::BTreeSet;

use crate::plan::{
    condition_phrase, conversion_source, mechanical_conversion, Capability, CapabilityKind,
    SynthesisPlan,
};

use super::refusal::TargetRefusals;
use super::{cover, field_line, name, unique_field, Emit, EXHAUSTIVENESS_NOTE};

/// Every declaration of one bounded context, each behind the plan's gate and this target's.
pub(super) fn declarations(
    out: &mut String,
    emit: &Emit<'_>,
    plan: &SynthesisPlan,
    refusals: &TargetRefusals,
    covered: &mut BTreeSet<Capability>,
) {
    for declared in emit.ir.types.values() {
        if emit.owns(&declared.name)
            && cover(
                plan,
                refusals,
                covered,
                CapabilityKind::DomainType,
                &declared.name.to_string(),
            )
        {
            named_type(out, emit, declared);
        }
    }
    for spec in emit.ir.entities.values() {
        if emit.owns(&spec.name)
            && cover(
                plan,
                refusals,
                covered,
                CapabilityKind::EntityLifecycle,
                &spec.name.to_string(),
            )
        {
            super::entity::lifecycle(out, emit, spec);
        }
    }
    for command in emit.ir.commands.values() {
        if emit.owns(&command.name)
            && cover(
                plan,
                refusals,
                covered,
                CapabilityKind::CommandContract,
                &command.name.to_string(),
            )
        {
            command_contract(out, emit, command);
        }
    }
    for event in emit.ir.events.values() {
        if emit.owns(&event.name)
            && cover(
                plan,
                refusals,
                covered,
                CapabilityKind::EventType,
                &event.name.to_string(),
            )
        {
            self::event(out, emit, event);
        }
    }
    for error in emit.ir.errors.values() {
        if emit.owns(&error.name)
            && cover(
                plan,
                refusals,
                covered,
                CapabilityKind::ErrorType,
                &error.name.to_string(),
            )
        {
            self::error(out, emit, error);
        }
    }
    for view in emit.ir.views.values() {
        if emit.owns(&view.name)
            && cover(
                plan,
                refusals,
                covered,
                CapabilityKind::ViewType,
                &view.name.to_string(),
            )
        {
            self::view(out, emit, view);
        }
    }
}

/// The mechanical conversions whose destination type this package owns.
pub(super) fn conversions(
    out: &mut String,
    emit: &Emit<'_>,
    plan: &SynthesisPlan,
    refusals: &TargetRefusals,
    covered: &mut BTreeSet<Capability>,
) {
    for declared in &emit.ir.conversions {
        let Some((from, to)) = mechanical_conversion(emit.ir, declared) else {
            continue;
        };
        if emit.owns(to.name())
            && cover(
                plan,
                refusals,
                covered,
                CapabilityKind::Conversion,
                &conversion_source(declared),
            )
        {
            conversion(out, emit, declared, from, to);
        }
    }
}

/// A named type: newtype, struct, enum or tagged union.
fn named_type(out: &mut String, emit: &Emit<'_>, declared: &ResolvedType) {
    match &declared.body {
        ResolvedBody::Newtype { of, invariants } => newtype(out, emit, declared, of, invariants),
        ResolvedBody::Struct { fields, invariants } => {
            structure(out, emit, declared, fields, invariants);
        }
        ResolvedBody::Enum { variants } => enumeration(out, emit, declared, variants),
        ResolvedBody::Union { tag, variants } => union(out, emit, declared, tag, variants),
    }
}

/// A wrapper distinct from its representation — the reason it exists is that it does not coerce.
///
/// A struct with an unexported field rather than `type Email string`, which is the encoding Go
/// programmers reach for first and the one that gives the guarantee away: a defined string type
/// accepts an untyped constant by assignment, so `var e Email = "anything"` compiles and the
/// wrapper stops being a wrapper.
fn newtype(
    out: &mut String,
    emit: &Emit<'_>,
    declared: &ResolvedType,
    of: &ResolvedTypeRef,
    invariants: &[Invariant],
) {
    let type_name = emit.layout.declared(&declared.name);
    let ctor = emit.layout.ctor(&declared.name);
    let inner = emit.go_type(of);
    let _ = writeln!(
        out,
        "\n// {type_name} is {} — `{}`: a distinct wrapper around `{of}`.",
        declared.naming.display_or(&declared.name),
        declared.name
    );
    summary_doc(out, declared.naming.summary.as_deref());
    invariant_doc(out, invariants);
    let _ = writeln!(
        out,
        "//\n// The field is unexported, so the only way to make one carrying a value is \
         [{ctor}] —\n// a defined type over `{inner}` would have let an untyped constant be \
         assigned straight to\n// {type_name}, which is the distinctness this declaration exists \
         for. Go's zero value still\n// needs no constructor (see TARGET.md).\ntype {type_name} \
         struct {{\n\tvalue {inner}\n}}\n\n// {ctor} wraps a `{of}` as {type_name}.\nfunc \
         {ctor}(value {inner}) {type_name} {{\n\treturn {type_name}{{value: value}}\n}}\n\n// \
         Value is the wrapped `{of}`.\nfunc (v {type_name}) Value() {inner} {{\n\treturn \
         v.value\n}}"
    );
}

/// A struct with named fields.
fn structure(
    out: &mut String,
    emit: &Emit<'_>,
    declared: &ResolvedType,
    fields: &[ResolvedField],
    invariants: &[Invariant],
) {
    let type_name = emit.layout.declared(&declared.name);
    let _ = writeln!(
        out,
        "\n// {type_name} is {} — `{}`.",
        declared.naming.display_or(&declared.name),
        declared.name
    );
    summary_doc(out, declared.naming.summary.as_deref());
    invariant_doc(out, invariants);
    struct_body(out, emit, type_name, fields);
}

/// A closed set of names, as a sealed interface.
///
/// Not `type Channel string` with constants, which is what a Go author writes by hand: that admits
/// `Channel("whatever")`, so the set is not closed and the specification's own guarantee — one of
/// *these* names — is gone. The synthesised state enums arrive here too.
fn enumeration(out: &mut String, emit: &Emit<'_>, declared: &ResolvedType, variants: &[String]) {
    let type_name = emit.layout.declared(&declared.name);
    if let Some(entity) = state_owner(emit.ir, &declared.name) {
        let _ = writeln!(
            out,
            "\n// {type_name} is the states of `{entity}`, as runtime values.\n//\n// Synthesised \
             from the lifecycle, so the two cannot disagree. Which *moves* are legal is\n// not \
             carried here — it is carried by one type per state, where an undeclared move is a\n// \
             method that does not exist."
        );
    } else {
        let _ = writeln!(
            out,
            "\n// {type_name} is {} — `{}`: one of a closed set of names.",
            declared.naming.display_or(&declared.name),
            declared.name
        );
        summary_doc(out, declared.naming.summary.as_deref());
    }
    sealed(out, type_name);
    for variant in variants {
        let variant_name = emit.layout.variant(&declared.name, variant);
        let _ = writeln!(
            out,
            "\n// {variant_name} is `{variant}`.\ntype {variant_name} struct{{}}\n\nfunc \
             ({variant_name}) {}() {{}}",
            name::marker(type_name)
        );
    }
}

/// The entity whose synthesised state enum this is, when it is one.
fn state_owner<'a>(ir: &'a EssIr, type_name: &QualifiedName) -> Option<&'a QualifiedName> {
    ir.entities
        .values()
        .find(|entity| entity.state_type.name() == type_name)
        .map(|entity| &entity.name)
}

/// A tagged union: one variant type per shape, the tag recorded in the docs because the wire needs
/// it and the Go interface does not.
fn union(
    out: &mut String,
    emit: &Emit<'_>,
    declared: &ResolvedType,
    tag: &str,
    variants: &BTreeMap<String, ResolvedTypeRef>,
) {
    let type_name = emit.layout.declared(&declared.name);
    let _ = writeln!(
        out,
        "\n// {type_name} is {} — `{}`: one of a fixed set of shapes, tagged on the wire by \
         `{tag}`.",
        declared.naming.display_or(&declared.name),
        declared.name
    );
    summary_doc(out, declared.naming.summary.as_deref());
    sealed(out, type_name);
    for (tag_value, type_ref) in variants {
        let variant_name = emit.layout.variant(&declared.name, tag_value);
        let _ = writeln!(
            out,
            "\n// {variant_name} is the shape tagged `{tag_value}` — `{type_ref}`.\ntype \
             {variant_name} struct {{\n\t// Value is what this shape carries.\n\tValue {}\n}}\n\n\
             func ({variant_name}) {}() {{}}",
            emit.go_type(type_ref),
            name::marker(type_name)
        );
    }
}

/// A command's contract: its input type, and one sealed outcome interface holding every declared
/// result — refusals beside successes, because a consumer that cannot see the refusal branch
/// handles only the happy path (design §8).
fn command_contract(out: &mut String, emit: &Emit<'_>, command: &ResolvedCommand) {
    let type_name = emit.layout.declared(&command.name);
    let outcome_name = emit.layout.outcome(&command.name);
    let _ = writeln!(
        out,
        "\n// {type_name} is {} — the input of `{}`.",
        command.naming.display_or(&command.name),
        command.name
    );
    summary_doc(out, command.naming.summary.as_deref());
    let _ = writeln!(
        out,
        "//\n// Everything it can result in is [{outcome_name}]."
    );
    struct_body(out, emit, type_name, &command.input);

    let _ = writeln!(
        out,
        "\n// {outcome_name} is everything `{}` can result in — one variant per declared \
         outcome.\n//\n// An infrastructure failure is deliberately not in here: a refusal is a \
         fact about the\n// domain, a transport fault is a fact about the run, and conflating the \
         two is what the\n// declared outcomes exist to prevent.",
        command.name
    );
    sealed(out, outcome_name);
    for outcome in &command.outcomes {
        outcome_variant(out, emit, command, outcome);
    }
}

/// One emitted event's field on an outcome's variant: the field identifier and the event it
/// carries.
pub(super) struct OutcomeEventField<'a> {
    /// The variant field's identifier.
    pub field: String,
    /// The event the field carries.
    pub event: &'a EventHandle,
}

/// The event fields one outcome's variant carries, in emission order.
///
/// Computed once and shared, exactly as in the Rust emitter: the variant declares these fields and
/// a component's port reads them to publish, and two renderers numbering duplicate events
/// independently is how a reader comes to name a field the variant does not have.
pub(super) fn outcome_event_fields<'a>(
    emit: &Emit<'_>,
    outcome: &'a ResolvedOutcome,
) -> Vec<OutcomeEventField<'a>> {
    let mut used: BTreeMap<String, usize> = BTreeMap::new();
    let mut fields = Vec::new();
    for event in &outcome.emits {
        let mut field = emit.layout.declared(event.name()).to_owned();
        let repeats = used.entry(field.clone()).or_insert(0);
        *repeats += 1;
        if *repeats > 1 {
            // The same event emitted twice on one branch is legal in the model; two fields with
            // one name are not legal here, so the copies are numbered in emission order.
            let _ = write!(field, "{repeats}");
        }
        fields.push(OutcomeEventField { field, event });
    }
    fields
}

/// One variant of a command's outcome: what it publishes and what it reports.
fn outcome_variant(
    out: &mut String,
    emit: &Emit<'_>,
    command: &ResolvedCommand,
    outcome: &ResolvedOutcome,
) {
    let variant_name = emit
        .layout
        .outcome_variant(&command.name, outcome.name.as_str());
    let _ = writeln!(
        out,
        "\n// {variant_name} is `{}` — {}.",
        outcome.name,
        condition_phrase(&outcome.condition)
    );
    if let Some(summary) = &outcome.summary {
        let _ = writeln!(out, "//\n// {}", summary.trim());
    }
    let carried = outcome_event_fields(emit, outcome);
    if carried.is_empty() && outcome.error.is_none() {
        let _ = writeln!(
            out,
            "type {variant_name} struct{{}}\n\nfunc ({variant_name}) {}() {{}}",
            name::marker(emit.layout.outcome(&command.name))
        );
        return;
    }
    let _ = writeln!(out, "type {variant_name} struct {{");
    for field in &carried {
        let _ = writeln!(
            out,
            "\t// {} is the `{}` this outcome publishes.\n\t{} {}",
            field.field,
            field.event,
            field.field,
            emit.reference(field.event.name())
        );
    }
    if let Some(error) = &outcome.error {
        let _ = writeln!(
            out,
            "\t// Error is why it was refused: `{error}`.\n\tError {}",
            emit.reference(error.name())
        );
    }
    let _ = writeln!(
        out,
        "}}\n\nfunc ({variant_name}) {}() {{}}",
        name::marker(emit.layout.outcome(&command.name))
    );
}

/// An event: a semantic value that knows nothing about any transport (design §9).
fn event(out: &mut String, emit: &Emit<'_>, event: &ResolvedEvent) {
    let type_name = emit.layout.declared(&event.name);
    let _ = writeln!(
        out,
        "\n// {type_name} is {} — the event `{}`.",
        event.naming.display_or(&event.name),
        event.name
    );
    summary_doc(out, event.naming.summary.as_deref());
    struct_body(out, emit, type_name, &event.fields);
}

/// A declared error: what a refusal carries beyond its name, so a caller can react rather than
/// guess.
fn error(out: &mut String, emit: &Emit<'_>, error: &ResolvedError) {
    let type_name = emit.layout.declared(&error.name);
    let _ = writeln!(
        out,
        "\n// {type_name} is the declared error `{}`.",
        error.name
    );
    summary_doc(out, error.summary.as_deref());
    struct_body(out, emit, type_name, &error.fields);
}

/// A view's row type. The *query* that serves it is an obligation in the plan, not code in here.
fn view(out: &mut String, emit: &Emit<'_>, view: &ResolvedView) {
    let type_name = emit.layout.declared(&view.name);
    let _ = write!(
        out,
        "\n// {type_name} is {} — one row of the view `{}`.\n//\n// Projects `{}` at `{}` \
         consistency",
        view.naming.display_or(&view.name),
        view.name,
        view.source,
        view.consistency.as_str()
    );
    if let Some(filter) = &view.filter {
        let _ = write!(out, ", containing instances where `{filter}`");
    }
    out.push_str(
        ".\n// Serving it is an implementation obligation — see the plan — because how a \
         projection is\n// kept current is a storage decision the specification does not take.\n",
    );
    struct_body(out, emit, type_name, &view.fields);
}

/// A mechanical conversion: the declared crossing between two newtypes over one representation,
/// written by re-wrapping — the one kind of conversion the specification fully determines.
///
/// A function rather than a method, because Go has no `From` to implement and a method on the
/// *source* type would have to live in the source's package, which is the package that must not
/// know the destination exists.
fn conversion(
    out: &mut String,
    emit: &Emit<'_>,
    declared: &ResolvedConversion,
    from: &TypeHandle,
    to: &TypeHandle,
) {
    let function = emit.layout.convert(&conversion_source(declared));
    let from_type = emit.reference(from.name());
    let to_ctor = emit.layout.ctor(to.name());
    let _ = writeln!(
        out,
        "\n// {function} is the declared crossing `{}` → `{}`.\n//\n// Permitted by the \
         specification because: {}\nfunc {function}(value {from_type}) {} {{\n\treturn \
         {to_ctor}(value.Value())\n}}",
        declared.from,
        declared.to,
        declared.because.trim(),
        emit.reference(to.name()),
    );
}

/// One struct declaration and its fields, or the empty struct where there are none.
fn struct_body(out: &mut String, emit: &Emit<'_>, type_name: &str, fields: &[ResolvedField]) {
    if fields.is_empty() {
        let _ = writeln!(out, "type {type_name} struct{{}}");
        return;
    }
    let _ = writeln!(out, "type {type_name} struct {{");
    let mut taken = BTreeMap::new();
    for field in fields {
        field_line(out, emit, &mut taken, field);
    }
    out.push_str("}\n");
}

/// The sealed interface half of a closed set: the marker method, and the note that says what Go
/// can and cannot hold about it.
pub(super) fn sealed(out: &mut String, type_name: &str) {
    out.push_str("//\n");
    out.push_str(EXHAUSTIVENESS_NOTE);
    let _ = writeln!(
        out,
        "type {type_name} interface {{\n\t{}()\n}}",
        name::marker(type_name)
    );
}

/// The optional one-line summary, as its own doc paragraph.
pub(super) fn summary_doc(out: &mut String, summary: Option<&str>) {
    if let Some(summary) = summary {
        let _ = writeln!(out, "//\n// {}", summary.trim());
    }
}

/// The declared invariants, documented rather than silently dropped — and documented rather than
/// enforced, because checking is behaviour, and behaviour is an obligation in this scope.
pub(super) fn invariant_doc(out: &mut String, invariants: &[Invariant]) {
    if invariants.is_empty() {
        return;
    }
    out.push_str("//\n");
    for invariant in invariants {
        let _ = writeln!(
            out,
            "// Every value satisfies `{}` — declared here, enforced by whatever behaviour \
             constructs one.",
            invariant.statement
        );
    }
}

/// One struct field, taking the same repair path every generated struct does.
pub(super) fn field_ident(taken: &mut BTreeMap<String, usize>, field: &str) -> String {
    unique_field(taken, name::exported(field))
}
