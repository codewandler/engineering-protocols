//! The obligation surface: typed seams for what the plan owes, and the stub that refuses them.
//!
//! Wave 6.2's rule is that an obligation is *visible twice*: once in the plan document, once in
//! the generated code as a named stub. The stub never panics — it returns a value, the
//! `UnmetObligation` this module emits, naming the plan entry it stands in for — so a module built
//! entirely on stubs compiles, runs, and reports exactly what it cannot yet do. [`workspace`]
//! holds the two lists to a bijection, minus whatever this target refused.
//!
//! # Where they live, and why that is not where Rust puts them
//!
//! Rust files an owed conversion beside the refusal type in one `obligation` module, because Rust
//! modules may reference each other freely. Go forbids an import cycle, and every bounded
//! context's package must import the refusal type — so the refusal type gets a package that
//! imports **nothing** ([`refusal_package`]), and the owed conversions, which name both ends, get
//! a package of their own ([`conversion_package`]). One file moved by the target's rules; no plan
//! entry moved at all.
//!
//! [`workspace`]: super::workspace

use std::fmt::Write as _;

use ess_compiler::ir::EssIr;
use ess_gen::{Artifact, Provenance};

use crate::plan::{
    conversion_source, mechanical_conversion, Capability, CapabilityKind, ImplementationObligation,
    SynthesisPlan, REGENERATE,
};

use super::layout::Layout;
use super::name;
use super::refusal::TargetRefusals;
use super::{record, Emit};

/// The package holding the typed refusal of an unmet obligation, and nothing else.
pub(super) fn refusal_package(layout: &Layout, provenance: &Provenance) -> Artifact {
    let package = layout.obligation();
    let mut out = provenance.commented_for("//", REGENERATE);
    let _ = write!(
        out,
        "\n// Package {} carries the typed refusal of an unmet obligation.\n//\n// An obligation \
         is a capability the synthesis plan owes the implementor — the contract is\n// declared, \
         the behaviour is not. Until an implementation satisfies one, its stub returns\n// \
         [UnmetObligation]: a value naming the plan entry, never a panic and never a guess, so \
         a\n// module built on stubs compiles and reports its own gaps.\n//\n// Its own package, \
         and one that imports nothing from this module: Go refuses an import\n// cycle where Rust \
         allows a module cycle, and every bounded context's package has to name\n// this \
         type.\npackage {}\n\nimport (\n\t\"fmt\"\n)\n\n// UnmetObligation is a capability the \
         synthesis plan owes and nothing has satisfied yet.\n//\n// The two fields spell the plan \
         entry: look the pair up in PLAN.md for the contract being\n// refused. A satisfying \
         implementation never constructs one.\ntype UnmetObligation struct {{\n\t// Capability is \
         the capability kind, as the plan spells it.\n\tCapability string\n\t// Source is the \
         construct that requires it, in the specification's own spelling.\n\tSource \
         string\n}}\n\n// Error names the plan entry being refused.\nfunc (u *UnmetObligation) \
         Error() string {{\n\treturn fmt.Sprintf(\"unmet obligation: %s `%s` — see PLAN.md\", \
         u.Capability, u.Source)\n}}\n",
        package.name, package.name
    );
    Artifact::new(package.file(), out)
}

/// The package holding the owed crossings between bounded contexts.
///
/// `None` when the specification owes none, so a module with only mechanical conversions does not
/// carry an empty mechanism.
pub(super) fn conversion_package(
    ir: &EssIr,
    plan: &SynthesisPlan,
    layout: &Layout,
    refusals: &TargetRefusals,
    provenance: &Provenance,
    stubbed: &mut std::collections::BTreeSet<Capability>,
) -> Option<Artifact> {
    let package = layout.conversion();
    let emit = Emit::new(ir, layout, package, None);

    let mut owed = Vec::new();
    for conversion in &ir.conversions {
        let source = conversion_source(conversion);
        if mechanical_conversion(ir, conversion).is_some()
            || refusals.refuses_kind(CapabilityKind::Conversion, &source)
        {
            continue;
        }
        let Some(obligation) = plan.obligation_of(CapabilityKind::Conversion, &source) else {
            continue;
        };
        owed.push((conversion, source, obligation));
    }
    if owed.is_empty() {
        return None;
    }

    let mut body = String::new();
    for (conversion, source, obligation) in &owed {
        let interface = layout.owed(source);
        let method = convert_method(conversion);
        let _ = writeln!(
            &mut body,
            "\n// {interface} is the declared crossing `{}` → `{}` — an implementation \
             obligation.\n//\n// Why it is not generated: {}.\n//\n// Contract: {}.\ntype \
             {interface} interface {{\n\t// {method} computes the crossing the specification \
             permits but does not declare.\n\t//\n\t// The second result is the typed refusal of \
             an obligation nothing has satisfied; a\n\t// satisfying implementation never returns \
             one. The crossing is in the method's name\n\t// because Go gives a type one method \
             set, and one shared stub cannot answer two seams\n\t// that both call their method \
             `Convert`.\n\t{method}(value {}) ({}, {})\n}}",
            conversion.from,
            conversion.to,
            obligation.reason.describes(),
            obligation.contract,
            emit.go_type(&conversion.from),
            emit.go_type(&conversion.to),
            emit.unmet(),
        );
    }

    let unimplemented = layout.unimplemented(package);
    let _ = writeln!(
        &mut body,
        "\n// {unimplemented} satisfies every owed crossing by refusing in the type \
         system.\n//\n// Each method returns the typed refusal naming what is owed — never a \
         panic, never a guessed\n// value — so a module built on this stub compiles and reports \
         its own gaps.\ntype {unimplemented} struct{{}}"
    );
    for (conversion, source, _) in &owed {
        record(stubbed, CapabilityKind::Conversion, source);
        let method = convert_method(conversion);
        let _ = writeln!(
            &mut body,
            "\n// {method} refuses: the crossing `{}` → `{}` is owed.\nfunc ({unimplemented}) \
             {method}(value {}) ({}, {}) {{\n\treturn {}, {}\n}}",
            conversion.from,
            conversion.to,
            emit.go_type(&conversion.from),
            emit.go_type(&conversion.to),
            emit.unmet(),
            zero_of(&emit.go_type(&conversion.to)),
            emit.unmet_literal(CapabilityKind::Conversion, source),
        );
    }

    let doc = format!(
        "// Package {} carries the crossings between bounded contexts the specification permits \
         but\n// does not compute.\n//\n// Its own package because Go refuses an import cycle: an \
         owed crossing names both ends, so\n// it cannot live where either end lives, nor beside \
         the refusal type every end \
         imports.\n",
        package.name
    );
    Some(emit.file(provenance, &doc, &body))
}

/// The method one owed crossing is answered by.
///
/// It carries both ends because Go gives a type one method set: a single `Unimplemented` cannot
/// implement two interfaces that both declare `Convert`, which Rust's traits make free.
fn convert_method(conversion: &ess_compiler::ir::ResolvedConversion) -> String {
    format!(
        "Convert{}To{}",
        name::type_fragment(&conversion.from.to_string()),
        name::type_fragment(&conversion.to.to_string())
    )
}

/// One bounded context's obligations: an interface per owed behaviour and query, and the shared
/// stub refusing them all.
pub(super) fn domain_obligations(
    out: &mut String,
    emit: &Emit<'_>,
    plan: &SynthesisPlan,
    refusals: &TargetRefusals,
    stubbed: &mut std::collections::BTreeSet<Capability>,
) {
    let seams = owed_by_domain(emit, plan, refusals);
    if seams.is_empty() {
        return;
    }
    for seam in &seams {
        let parameter = seam
            .parameter
            .as_ref()
            .map(|(ident, of)| format!("{ident} {of}"))
            .unwrap_or_default();
        let _ = writeln!(
            out,
            "\n// {} is {}\n//\n// Why it is not generated: {}.\n//\n// Contract: {}.\ntype {} \
             interface {{\n\t// {} {}\n\t//\n\t// The second result is the typed refusal of an \
             obligation nothing has satisfied; a\n\t// satisfying implementation never returns \
             one.\n\t{}({parameter}) ({}, {})\n}}",
            seam.interface,
            seam.heading,
            seam.obligation.reason.describes(),
            seam.obligation.contract,
            seam.interface,
            seam.method,
            seam.method_doc,
            seam.method,
            seam.answer,
            emit.unmet(),
        );
    }

    let unimplemented = emit.layout.unimplemented(emit.package);
    let _ = writeln!(
        out,
        "\n// {unimplemented} satisfies every obligation of this bounded context by refusing in \
         the type\n// system.\n//\n// Each method returns the typed refusal naming what is owed — \
         never a panic, never a\n// guessed value — so a module built on this stub compiles and \
         reports its own gaps.\ntype {unimplemented} struct{{}}"
    );
    for seam in &seams {
        record(stubbed, seam.kind, &seam.source);
        let parameter = seam
            .parameter
            .as_ref()
            .map(|(ident, of)| format!("{ident} {of}"))
            .unwrap_or_default();
        let _ = writeln!(
            out,
            "\n// {} refuses: {}\nfunc ({unimplemented}) {}({parameter}) ({}, {}) {{\n\treturn \
             {}, {}\n}}",
            seam.method,
            seam.heading,
            seam.method,
            seam.answer,
            emit.unmet(),
            seam.zero,
            emit.unmet_literal(seam.kind, &seam.source),
        );
    }
}

/// The behaviours and queries one bounded context owes, in declaration order.
fn owed_by_domain(emit: &Emit<'_>, plan: &SynthesisPlan, refusals: &TargetRefusals) -> Vec<Seam> {
    let mut seams: Vec<Seam> = Vec::new();
    for command in emit.ir.commands.values() {
        let source = command.name.to_string();
        if !emit.owns(&command.name)
            || refusals.refuses_kind(CapabilityKind::CommandBehavior, &source)
        {
            continue;
        }
        let Some(obligation) = plan.obligation_of(CapabilityKind::CommandBehavior, &source) else {
            continue;
        };
        seams.push(Seam {
            kind: CapabilityKind::CommandBehavior,
            source: source.clone(),
            heading: format!("the behaviour `{source}` — an implementation obligation."),
            obligation: obligation.clone(),
            interface: emit.layout.behavior(&command.name).to_owned(),
            method: emit.layout.declared(&command.name).to_owned(),
            method_doc: format!("decides and enacts exactly one declared outcome of `{source}`."),
            parameter: Some((
                "input".to_owned(),
                emit.layout.declared(&command.name).to_owned(),
            )),
            answer: emit.layout.outcome(&command.name).to_owned(),
            zero: "nil".to_owned(),
        });
    }
    for view in emit.ir.views.values() {
        let source = view.name.to_string();
        if !emit.owns(&view.name) || refusals.refuses_kind(CapabilityKind::ViewQuery, &source) {
            continue;
        }
        let Some(obligation) = plan.obligation_of(CapabilityKind::ViewQuery, &source) else {
            continue;
        };
        seams.push(Seam {
            kind: CapabilityKind::ViewQuery,
            source: source.clone(),
            heading: format!("the query `{source}` — an implementation obligation."),
            obligation: obligation.clone(),
            interface: emit.layout.query(&view.name).to_owned(),
            method: emit.layout.declared(&view.name).to_owned(),
            method_doc: format!("serves `{source}` rows at the view's declared consistency."),
            parameter: None,
            answer: format!("[]{}", emit.layout.declared(&view.name)),
            zero: "nil".to_owned(),
        });
    }
    seams
}

/// Everything one owed seam and its stub need to agree on, carried once.
struct Seam {
    /// The plan capability the stub stands in for.
    kind: CapabilityKind,
    /// Its source, in the specification's spelling.
    source: String,
    /// The seam's one-line heading, reused by the interface and by its refusal.
    heading: String,
    /// The plan's own entry, quoted on the interface.
    obligation: ImplementationObligation,
    /// The interface's name.
    interface: String,
    /// The method's name.
    method: String,
    /// The method's one-line doc.
    method_doc: String,
    /// The parameter beyond the receiver, if the seam takes one.
    parameter: Option<(String, String)>,
    /// The first result type.
    answer: String,
    /// The first result's zero value, which the refusing stub returns beside the refusal.
    zero: String,
}

/// The zero value of a generated type, as a stub returns it beside its refusal.
///
/// A sealed interface, a slice and a pointer are all `nil`; a struct's zero value is written out,
/// because Go has no `Default` to lean on and a stub must return *something* in the first result.
pub(super) fn zero_of(go_type: &str) -> String {
    if go_type.starts_with("[]") || go_type.starts_with('*') || go_type.starts_with("map[") {
        return "nil".to_owned();
    }
    format!("{go_type}{{}}")
}
