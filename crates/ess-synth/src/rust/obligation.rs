//! The obligation surface: typed seams for what the plan owes, and the stub that refuses them.
//!
//! Wave 6.2's rule is that an obligation is *visible twice*: once in the plan document, once in
//! the generated workspace as a named stub. The stub is never `todo!()` and never a panic — it is
//! a value, [the `UnmetObligation` this module emits](obligation_module), naming the plan entry it
//! stands in for — so a workspace built entirely on stubs compiles, runs, and reports exactly
//! what it cannot yet do. `workspace` holds the two lists to a bijection: every plan obligation
//! has exactly one stub, and no stub exists that the plan does not owe.
//!
//! Three homes, decided by what the obligation is about: a command behaviour or a view query is a
//! bounded context's, so its trait and stub live in that domain's `obligations` module; a
//! non-mechanical conversion is between contexts, so it lives in the root `obligation` module
//! beside the refusal type; a binding's obligations are the system's, and `system.rs` emits them.

use std::fmt::Write as _;

use ess_compiler::ir::EssIr;
use ess_gen::{Artifact, Provenance};

use super::{name, Emit};
use crate::plan::{
    conversion_source, mechanical_conversion, CapabilityKind, ImplementationObligation,
    SynthesisPlan, REGENERATE,
};

/// The path every generated reference to the refusal type spells, from inside the types crate.
const UNMET: &str = "crate::obligation::UnmetObligation";

/// The root `obligation` module of the types crate: the refusal type, and the stubs whose
/// obligation belongs to no single bounded context.
///
/// `None` when the specification gives it nothing to say — no obligations, no components, no
/// bindings — so a pure-types workspace does not carry an empty mechanism.
pub(super) fn obligation_module(
    ir: &EssIr,
    plan: &SynthesisPlan,
    layout: &super::layout::Layout,
    provenance: &Provenance,
    stubbed: &mut std::collections::BTreeSet<crate::plan::Capability>,
) -> Option<Artifact> {
    let needed =
        plan.obligations().next().is_some() || !ir.components.is_empty() || !ir.bindings.is_empty();
    if !needed {
        return None;
    }

    let mut out = provenance.commented_for("//", REGENERATE);
    out.push_str(
        "\n//! The typed refusal of an unmet obligation, and the conversion seams owed between \
         contexts.\n//!\n//! An obligation is a capability the synthesis plan owes the \
         implementor — the contract is declared,\n//! the behaviour is not. Until an \
         implementation satisfies one, its stub returns [`UnmetObligation`]:\n//! a value naming \
         the plan entry, never a panic and never a guess, so a workspace built on stubs\n//! \
         compiles and reports its own gaps.\n\n/// A capability the synthesis plan owes and \
         nothing has satisfied yet.\n///\n/// The two fields spell the plan entry: look the pair \
         up in `PLAN.md` for the contract being\n/// refused. A satisfying implementation never \
         constructs one.\n#[derive(Debug, Clone, PartialEq, Eq)]\npub struct UnmetObligation \
         {\n    /// The capability kind, as the plan spells it.\n    pub capability: &'static \
         str,\n    /// The construct that requires it, in the specification's own spelling.\n    \
         pub source: &'static str,\n}\n\nimpl core::fmt::Display for UnmetObligation {\n    fn \
         fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {\n        write!(f, \
         \"unmet obligation: {} `{}` — see PLAN.md\", self.capability, self.source)\n    \
         }\n}\n",
    );

    conversion_traits(&mut out, ir, plan, layout, stubbed);
    Some(Artifact::new(
        format!("crates/{}/src/obligation.rs", layout.package()),
        out,
    ))
}

/// The owed conversions: a declared crossing whose computation is not mechanical gets a trait
/// here, and the shared [`Unimplemented`] stub refuses each one.
fn conversion_traits(
    out: &mut String,
    ir: &EssIr,
    plan: &SynthesisPlan,
    layout: &super::layout::Layout,
    stubbed: &mut std::collections::BTreeSet<crate::plan::Capability>,
) {
    let mut owed = Vec::new();
    for conversion in &ir.conversions {
        let source = conversion_source(conversion);
        if mechanical_conversion(ir, conversion).is_some() {
            continue;
        }
        let Some(obligation) = plan.obligation_of(CapabilityKind::Conversion, &source) else {
            continue;
        };
        owed.push((conversion, source, obligation));
    }
    if owed.is_empty() {
        return;
    }

    for (conversion, _, obligation) in &owed {
        let trait_name = format!(
            "{}To{}Conversion",
            name::type_fragment(&conversion.from.to_string()),
            name::type_fragment(&conversion.to.to_string())
        );
        let _ = writeln!(
            out,
            "\n/// The declared crossing `{}` → `{}` — an implementation obligation.\n///\n/// \
             Why it is not generated: {}.\n///\n/// Contract: {}.\npub trait {trait_name} {{\n    \
             /// Computes the crossing the specification permits but does not declare.\n    ///\n    \
             /// `Err` is the typed refusal of an obligation nothing has satisfied; a satisfying\n    \
             /// implementation never returns it.\n    fn convert(&self, value: {}) -> Result<{}, \
             UnmetObligation>;\n}}",
            conversion.from,
            conversion.to,
            obligation.reason.describes(),
            obligation.contract,
            layout.absolute_type(&conversion.from),
            layout.absolute_type(&conversion.to),
        );
    }

    out.push_str(
        "\n/// Every owed conversion, refused in the type system.\n///\n/// Each method returns \
         the typed refusal naming what is owed — never a panic, never a guessed\n/// value — so a \
         workspace built on this stub compiles and reports its own gaps.\npub struct \
         Unimplemented;\n",
    );
    for (conversion, source, _) in &owed {
        let trait_name = format!(
            "{}To{}Conversion",
            name::type_fragment(&conversion.from.to_string()),
            name::type_fragment(&conversion.to.to_string())
        );
        record(stubbed, CapabilityKind::Conversion, source);
        let _ = writeln!(
            out,
            "\nimpl {trait_name} for Unimplemented {{\n    fn convert(&self, _value: {}) -> \
             Result<{}, UnmetObligation> {{\n        Err(UnmetObligation {{ capability: \
             \"{}\", source: \"{source}\" }})\n    }}\n}}",
            layout.absolute_type(&conversion.from),
            layout.absolute_type(&conversion.to),
            CapabilityKind::Conversion.describes(),
        );
    }
}

/// One bounded context's `obligations` module: a trait per owed behaviour and query, and the
/// `Unimplemented` stub refusing them all.
///
/// Appended to the domain's module, so `super::` names the domain's own types the way every
/// other item in the file does.
pub(super) fn domain_obligations(
    out: &mut String,
    emit: &Emit<'_>,
    plan: &SynthesisPlan,
    stubbed: &mut std::collections::BTreeSet<crate::plan::Capability>,
) {
    let traits = owed_by_domain(emit, plan);
    if traits.is_empty() {
        return;
    }

    out.push_str(
        "\n/// What this bounded context owes its implementor, as typed seams.\n///\n/// One \
         trait per obligation in the synthesis plan, each carrying the plan's own contract.\n/// \
         [`Unimplemented`](obligations::Unimplemented) satisfies every trait by refusing in the \
         type system, so the workspace builds —\n/// and says exactly what it cannot yet do — \
         before a line is hand-written.\npub mod obligations {\n",
    );
    render_traits(out, &traits);
    render_stubs(out, &traits, stubbed);
    out.push_str("}\n");
}

/// The behaviours and queries one bounded context owes, in declaration order.
fn owed_by_domain(emit: &Emit<'_>, plan: &SynthesisPlan) -> Vec<TraitStub> {
    let mut traits: Vec<TraitStub> = Vec::new();
    for command in emit.ir.commands.values() {
        if !super::owned(emit, &command.name) {
            continue;
        }
        let source = command.name.to_string();
        let Some(obligation) = plan.obligation_of(CapabilityKind::CommandBehavior, &source) else {
            continue;
        };
        let type_name = emit.layout.type_name(&command.name);
        traits.push(TraitStub {
            kind: CapabilityKind::CommandBehavior,
            source: source.clone(),
            heading: format!("The behaviour `{source}` — an implementation obligation."),
            obligation: obligation.clone(),
            trait_name: format!("{type_name}Behavior"),
            method_doc: format!("Decides and enacts exactly one declared outcome of `{source}`."),
            method: name::value_ident(&type_name),
            receiver: "&mut self",
            argument: Some((
                "input".to_owned(),
                scoped(&emit.reference_name(&command.name)),
            )),
            answer: scoped(&format!("{type_name}Outcome")),
        });
    }
    for view in emit.ir.views.values() {
        if !super::owned(emit, &view.name) {
            continue;
        }
        let source = view.name.to_string();
        let Some(obligation) = plan.obligation_of(CapabilityKind::ViewQuery, &source) else {
            continue;
        };
        let type_name = emit.layout.type_name(&view.name);
        traits.push(TraitStub {
            kind: CapabilityKind::ViewQuery,
            source: source.clone(),
            heading: format!("The query `{source}` — an implementation obligation."),
            obligation: obligation.clone(),
            trait_name: format!("{type_name}Query"),
            method_doc: format!("Serves `{source}` rows at the view's declared consistency."),
            method: name::value_ident(&type_name),
            receiver: "&self",
            argument: None,
            answer: format!("Vec<{}>", scoped(&emit.reference_name(&view.name))),
        });
    }
    traits
}

/// The trait half of the `obligations` module: one seam per owed capability.
fn render_traits(out: &mut String, traits: &[TraitStub]) {
    for spec in traits {
        let argument = spec
            .argument
            .as_ref()
            .map(|(ident, of)| format!(", {ident}: {of}"))
            .unwrap_or_default();
        let _ = writeln!(
            out,
            "    /// {}\n    ///\n    /// Why it is not generated: {}.\n    ///\n    /// \
             Contract: {}.\n    pub trait {} {{\n        /// {}\n        ///\n        /// `Err` \
             is the typed refusal of an obligation nothing has satisfied; a satisfying\n        \
             /// implementation never returns it.\n        fn {}({}{argument}) -> Result<{}, \
             {UNMET}>;\n    }}\n",
            spec.heading,
            spec.obligation.reason.describes(),
            spec.obligation.contract,
            spec.trait_name,
            spec.method_doc,
            spec.method,
            spec.receiver,
            spec.answer,
        );
    }
}

/// The stub half of the `obligations` module: `Unimplemented`, refusing each seam by value.
fn render_stubs(
    out: &mut String,
    traits: &[TraitStub],
    stubbed: &mut std::collections::BTreeSet<crate::plan::Capability>,
) {
    out.push_str(
        "    /// Every obligation of this bounded context, refused in the type system.\n    \
         ///\n    /// Each method returns the typed refusal naming what is owed — never a panic, \
         never a guessed\n    /// value — so a workspace built on this stub compiles and reports \
         its own gaps.\n    pub struct Unimplemented;\n",
    );
    for spec in traits {
        record(stubbed, spec.kind, &spec.source);
        let argument = spec
            .argument
            .as_ref()
            .map(|(ident, of)| format!(", _{ident}: {of}"))
            .unwrap_or_default();
        let _ = writeln!(
            out,
            "\n    impl {} for Unimplemented {{\n        fn {}({}{argument}) -> Result<{}, \
             {UNMET}> {{\n            Err({UNMET} {{ capability: \"{}\", source: \"{}\" }})\n        \
             }}\n    }}",
            spec.trait_name,
            spec.method,
            spec.receiver,
            spec.answer,
            spec.kind.describes(),
            spec.source,
        );
    }
}

/// Everything one owed trait and its stub need to agree on, carried once.
struct TraitStub {
    /// The plan capability the stub stands in for.
    kind: CapabilityKind,
    /// Its source, in the specification's spelling.
    source: String,
    /// The trait's one-line heading.
    heading: String,
    /// The plan's own entry, quoted on the trait.
    obligation: ImplementationObligation,
    /// The trait's name.
    trait_name: String,
    /// The method's one-line doc.
    method_doc: String,
    /// The method's name.
    method: String,
    /// The receiver — `&mut self` for a behaviour, `&self` for a query.
    receiver: &'static str,
    /// The argument beyond the receiver, if the seam takes one.
    argument: Option<(String, String)>,
    /// The `Ok` type.
    answer: String,
}

/// A reference as spelled from inside the nested `obligations` module: one level deeper than the
/// domain module, so a bare name gains `super::` and an absolute path stays itself.
fn scoped(reference: &str) -> String {
    if reference.contains("::") {
        reference.to_owned()
    } else {
        format!("super::{reference}")
    }
}

/// Records one stub against the bijection `workspace` asserts.
fn record(
    stubbed: &mut std::collections::BTreeSet<crate::plan::Capability>,
    kind: CapabilityKind,
    source: &str,
) {
    assert!(
        stubbed.insert(crate::plan::Capability {
            kind,
            source: source.to_owned(),
        }),
        "two stubs claimed the obligation `{source}` ({}); that is a defect in ess-synth",
        kind.describes()
    );
}
