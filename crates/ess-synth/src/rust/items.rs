//! Renderers for the declarations that are types all the way down: named types, command
//! contracts, events, errors, views, and the mechanical conversions.
//!
//! Every renderer appends complete items to a module buffer and nothing else — no file decisions,
//! no plan decisions, both of which were taken before it was called. Doc comments are rendered for
//! every public item because the generated crate holds itself to `deny(missing_docs)`: a public
//! item the emitter cannot say anything about is an emitter defect, and the generated gate is
//! where it fails.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use ess_compiler::ir::{
    EssIr, ResolvedBody, ResolvedCommand, ResolvedConversion, ResolvedError, ResolvedEvent,
    ResolvedField, ResolvedOutcome, ResolvedType, ResolvedTypeRef, ResolvedView, TypeHandle,
};
use ess_domain::entity::Invariant;
use ess_domain::name::QualifiedName;

use super::{name, Emit};
use crate::plan::condition_phrase;

/// A named type: newtype, struct, enum or tagged union.
pub(super) fn named_type(out: &mut String, emit: &Emit<'_>, declared: &ResolvedType) {
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
fn newtype(
    out: &mut String,
    emit: &Emit<'_>,
    declared: &ResolvedType,
    of: &ResolvedTypeRef,
    invariants: &[Invariant],
) {
    let _ = writeln!(
        out,
        "\n/// {} — `{}`: a distinct wrapper around `{of}`.",
        declared.naming.display_or(&declared.name),
        declared.name
    );
    summary_doc(out, declared.naming.summary.as_deref());
    invariant_doc(out, invariants);
    let _ = writeln!(
        out,
        "#[derive(Debug, Clone, PartialEq, Eq)]\npub struct {}(pub {});",
        emit.layout.type_name(&declared.name),
        emit.rust_type(of)
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
    let _ = writeln!(
        out,
        "\n/// {} — `{}`.",
        declared.naming.display_or(&declared.name),
        declared.name
    );
    summary_doc(out, declared.naming.summary.as_deref());
    invariant_doc(out, invariants);
    let _ = writeln!(
        out,
        "#[derive(Debug, Clone, PartialEq, Eq)]\npub struct {} {{",
        emit.layout.type_name(&declared.name)
    );
    for field in fields {
        field_line(out, emit, field);
    }
    out.push_str("}\n");
}

/// A closed set of names. The synthesised state enums arrive here too, and get the doc that says
/// where their variants come from.
fn enumeration(out: &mut String, emit: &Emit<'_>, declared: &ResolvedType, variants: &[String]) {
    if let Some(entity) = state_owner(emit.ir, &declared.name) {
        let _ = writeln!(
            out,
            "\n/// The states of `{entity}`, as runtime values.\n///\n/// Synthesised from the \
             lifecycle, so the two cannot disagree. Which *moves* are legal is not\n/// carried \
             here — it is carried by `{}<S>`, where an undeclared move does not compile.",
            emit.layout.type_name(entity)
        );
    } else {
        let _ = writeln!(
            out,
            "\n/// {} — `{}`: one of a closed set of names.",
            declared.naming.display_or(&declared.name),
            declared.name
        );
        summary_doc(out, declared.naming.summary.as_deref());
    }
    let _ = writeln!(
        out,
        "#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub enum {} {{",
        emit.layout.type_name(&declared.name)
    );
    for variant in variants {
        let _ = writeln!(out, "    /// `{variant}`.\n    {},", name::pascal(variant));
    }
    out.push_str("}\n");
}

/// The entity whose synthesised state enum this is, when it is one.
fn state_owner<'a>(ir: &'a EssIr, type_name: &QualifiedName) -> Option<&'a QualifiedName> {
    ir.entities
        .values()
        .find(|entity| entity.state_type.name() == type_name)
        .map(|entity| &entity.name)
}

/// A tagged union: one variant per shape, the tag recorded in the docs because the wire needs it
/// and the Rust enum does not.
fn union(
    out: &mut String,
    emit: &Emit<'_>,
    declared: &ResolvedType,
    tag: &str,
    variants: &BTreeMap<String, ResolvedTypeRef>,
) {
    let _ = writeln!(
        out,
        "\n/// {} — `{}`: one of a fixed set of shapes, tagged on the wire by `{tag}`.",
        declared.naming.display_or(&declared.name),
        declared.name
    );
    summary_doc(out, declared.naming.summary.as_deref());
    let _ = writeln!(
        out,
        "#[derive(Debug, Clone, PartialEq, Eq)]\npub enum {} {{",
        emit.layout.type_name(&declared.name)
    );
    for (tag_value, type_ref) in variants {
        let _ = writeln!(
            out,
            "    /// Tagged `{tag_value}` — `{type_ref}`.\n    {}({}),",
            name::pascal(tag_value),
            emit.rust_type(type_ref)
        );
    }
    out.push_str("}\n");
}

/// A command's contract: its input type, and one outcome enum holding every declared result —
/// refusals beside successes, because a consumer that cannot see the refusal branch handles only
/// the happy path (design §8).
pub(super) fn command_contract(out: &mut String, emit: &Emit<'_>, command: &ResolvedCommand) {
    let type_name = emit.layout.type_name(&command.name);
    let _ = writeln!(
        out,
        "\n/// {} — the input of `{}`.",
        command.naming.display_or(&command.name),
        command.name
    );
    summary_doc(out, command.naming.summary.as_deref());
    let _ = writeln!(
        out,
        "///\n/// Everything it can result in is [`{type_name}Outcome`].\n#[derive(Debug, Clone, \
         PartialEq, Eq)]\npub struct {type_name} {{"
    );
    for field in &command.input {
        field_line(out, emit, field);
    }
    out.push_str("}\n");

    let _ = writeln!(
        out,
        "\n/// Everything `{}` can result in — one variant per declared outcome.\n///\n/// An \
         infrastructure failure is deliberately not in here: a refusal is a fact about the \
         domain,\n/// a transport fault is a fact about the run, and conflating the two is what \
         the declared\n/// outcomes exist to prevent.\n#[derive(Debug, Clone, PartialEq, \
         Eq)]\npub enum {type_name}Outcome {{",
        command.name
    );
    for outcome in &command.outcomes {
        outcome_variant(out, emit, outcome);
    }
    out.push_str("}\n");
}

/// One variant of a command's outcome enum: what it publishes and what it reports.
fn outcome_variant(out: &mut String, emit: &Emit<'_>, outcome: &ResolvedOutcome) {
    let _ = writeln!(
        out,
        "    /// `{}` — {}.",
        outcome.name,
        condition_phrase(&outcome.condition)
    );
    if let Some(summary) = &outcome.summary {
        let _ = writeln!(out, "    ///\n    /// {}", summary.trim());
    }
    let variant = name::pascal(outcome.name.as_str());
    if outcome.emits.is_empty() && outcome.error.is_none() {
        let _ = writeln!(out, "    {variant},");
        return;
    }
    let _ = writeln!(out, "    {variant} {{");
    let mut used: BTreeMap<String, usize> = BTreeMap::new();
    for event in &outcome.emits {
        let mut field = name::value_ident(&emit.layout.type_name(event.name()));
        let repeats = used.entry(field.clone()).or_insert(0);
        *repeats += 1;
        if *repeats > 1 {
            // The same event emitted twice on one branch is legal in the model; two fields with
            // one name are not legal here, so the copies are numbered in emission order.
            let _ = write!(field, "_{repeats}");
        }
        let _ = writeln!(
            out,
            "        /// The `{event}` this outcome publishes.\n        {field}: {},",
            emit.reference_name(event.name())
        );
    }
    if let Some(error) = &outcome.error {
        let _ = writeln!(
            out,
            "        /// Why it was refused: `{error}`.\n        error: {},",
            emit.reference_name(error.name())
        );
    }
    out.push_str("    },\n");
}

/// An event: a semantic value that knows nothing about any transport (design §9).
pub(super) fn event(out: &mut String, emit: &Emit<'_>, event: &ResolvedEvent) {
    let _ = writeln!(
        out,
        "\n/// {} — the event `{}`.",
        event.naming.display_or(&event.name),
        event.name
    );
    summary_doc(out, event.naming.summary.as_deref());
    out.push_str("#[derive(Debug, Clone, PartialEq, Eq)]\n");
    let type_name = emit.layout.type_name(&event.name);
    if event.fields.is_empty() {
        let _ = writeln!(out, "pub struct {type_name};");
        return;
    }
    let _ = writeln!(out, "pub struct {type_name} {{");
    for field in &event.fields {
        field_line(out, emit, field);
    }
    out.push_str("}\n");
}

/// A declared error: what a refusal carries beyond its name, so a caller can react rather than
/// guess.
pub(super) fn error(out: &mut String, emit: &Emit<'_>, error: &ResolvedError) {
    let _ = writeln!(out, "\n/// The declared error `{}`.", error.name);
    summary_doc(out, error.summary.as_deref());
    out.push_str("#[derive(Debug, Clone, PartialEq, Eq)]\n");
    let type_name = emit.layout.type_name(&error.name);
    if error.fields.is_empty() {
        let _ = writeln!(out, "pub struct {type_name};");
        return;
    }
    let _ = writeln!(out, "pub struct {type_name} {{");
    for field in &error.fields {
        field_line(out, emit, field);
    }
    out.push_str("}\n");
}

/// A view's row type. The *query* that serves it is an obligation in the plan, not code in here.
pub(super) fn view(out: &mut String, emit: &Emit<'_>, view: &ResolvedView) {
    let _ = write!(
        out,
        "\n/// {} — one row of the view `{}`.\n///\n/// Projects `{}` at `{}` consistency",
        view.naming.display_or(&view.name),
        view.name,
        view.source,
        view.consistency.as_str()
    );
    if let Some(filter) = &view.filter {
        let _ = write!(out, ", containing instances where `{filter}`");
    }
    out.push_str(
        ".\n/// Serving it is an implementation obligation — see the plan — because how a \
         projection is kept\n/// current is a storage decision the specification does not take.\n",
    );
    let _ = writeln!(
        out,
        "#[derive(Debug, Clone, PartialEq, Eq)]\npub struct {} {{",
        emit.layout.type_name(&view.name)
    );
    for field in &view.fields {
        field_line(out, emit, field);
    }
    out.push_str("}\n");
}

/// A mechanical conversion: the declared crossing between two newtypes over one representation,
/// written by re-wrapping — the one kind of conversion the specification fully determines.
pub(super) fn conversion(
    out: &mut String,
    emit: &Emit<'_>,
    declared: &ResolvedConversion,
    from: &TypeHandle,
    to: &TypeHandle,
) {
    let from_type = emit.reference(from);
    let _ = writeln!(
        out,
        "\n/// The declared crossing `{}` → `{}`.\n///\n/// Permitted by the specification \
         because: {}\nimpl From<{from_type}> for {} {{\n    fn from(value: {from_type}) -> Self \
         {{\n        Self(value.0)\n    }}\n}}",
        declared.from,
        declared.to,
        declared.because.trim(),
        emit.reference(to)
    );
}

/// One struct field, with the specification's own spelling of its type in the docs.
fn field_line(out: &mut String, emit: &Emit<'_>, field: &ResolvedField) {
    let _ = writeln!(
        out,
        "    /// `{}` — `{}`.\n    pub {}: {},",
        field.name,
        field.type_ref,
        name::value_ident(&field.name),
        emit.rust_type(&field.type_ref)
    );
}

/// The optional one-line summary, as its own doc paragraph.
pub(super) fn summary_doc(out: &mut String, summary: Option<&str>) {
    if let Some(summary) = summary {
        let _ = writeln!(out, "///\n/// {}", summary.trim());
    }
}

/// The declared invariants, documented rather than silently dropped — and documented rather than
/// enforced, because checking is behaviour, and behaviour is an obligation in this scope.
pub(super) fn invariant_doc(out: &mut String, invariants: &[Invariant]) {
    if invariants.is_empty() {
        return;
    }
    out.push_str("///\n");
    for invariant in invariants {
        let _ = writeln!(
            out,
            "/// Every value satisfies `{}` — declared here, enforced by whatever behaviour \
             constructs one.",
            invariant.statement
        );
    }
}
