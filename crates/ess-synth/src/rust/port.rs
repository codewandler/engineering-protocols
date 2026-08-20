//! The component-crate emitter: one crate per component, holding its port.
//!
//! A port is the component's outer surface exactly as the specification declares it: the commands
//! it accepts become handler methods, the views its domains declare become query methods, and the
//! events it declares it publishes become a typed outbox. Nothing behavioural lives here — every
//! handler delegates to the behaviour obligation's trait, and every query to the query
//! obligation's trait, so the crate is a skeleton in the precise sense: complete surface, owed
//! interior, and the interior's absence is a typed refusal rather than a hole.
//!
//! Separate crates, not modules of the types crate, because a component is the specification's
//! own unit of ownership: someone deploying only `email-service` takes its crate and the types
//! crate, and nothing else.

use std::collections::BTreeSet;
use std::fmt::Write as _;

use ess_compiler::ir::{EssIr, EventHandle, ResolvedComponent};
use ess_gen::{Artifact, Provenance};

use crate::plan::{Capability, CapabilityKind, SynthesisPlan, REGENERATE};

use super::layout::Layout;
use super::{event_variants, items, name, Emit, EDITION};

/// One component's crate: its manifest and its port module, when the plan marks the port
/// generated.
pub(super) fn component_crate(
    ir: &EssIr,
    plan: &SynthesisPlan,
    layout: &Layout,
    component: &ResolvedComponent,
    covered: &mut BTreeSet<Capability>,
) -> Vec<Artifact> {
    if !plan.is_generated(CapabilityKind::ComponentPort, &component.name.to_string()) {
        return Vec::new();
    }
    covered.insert(Capability {
        kind: CapabilityKind::ComponentPort,
        source: component.name.to_string(),
    });
    vec![
        manifest(ir, layout, component, &plan.provenance),
        lib_module(ir, layout, component, &plan.provenance),
    ]
}

/// The component crate's manifest: one dependency, the types crate, by path — the workspace is
/// self-contained, and stays zero third-party dependencies.
fn manifest(
    ir: &EssIr,
    layout: &Layout,
    component: &ResolvedComponent,
    provenance: &Provenance,
) -> Artifact {
    let package = layout.component_package(&component.name);
    let mut out = provenance.commented_for("#", REGENERATE);
    let _ = write!(
        out,
        "\n[package]\nname = \"{package}\"\ndescription = \"The `{}` component of the `{}` \
         specification, {}: its port, generated.\"\nversion = \"{}.0.0\"\nedition = \
         \"{EDITION}\"\n\n[dependencies]\n{} = {{ path = \"../{}\" }}\n",
        component.name,
        ir.system,
        ir.version,
        ir.version.get(),
        layout.package(),
        layout.package(),
    );
    Artifact::new(format!("crates/{package}/Cargo.toml"), out)
}

/// The component crate's one module: the published-event enum and the port.
fn lib_module(
    ir: &EssIr,
    layout: &Layout,
    component: &ResolvedComponent,
    provenance: &Provenance,
) -> Artifact {
    let package = layout.component_package(&component.name);
    let types = Layout::crate_ident(layout.package());
    let port = name::pascal(&component.name.to_string());

    let mut out = provenance.commented_for("//", REGENERATE);
    out.push('\n');
    let _ = writeln!(
        out,
        "//! {} — the `{}` component of `{}` {}.",
        component
            .naming
            .display
            .as_deref()
            .unwrap_or(&component.name.to_string()),
        component.name,
        ir.system,
        ir.version
    );
    if let Some(summary) = &component.naming.summary {
        out.push_str("//!\n");
        let _ = writeln!(out, "//! {}", summary.trim());
    }
    out.push_str(
        "//!\n//! The component's outer surface exactly as the specification declares it: \
         accepted commands as\n//! handlers, declared views as queries, published events as a \
         typed outbox. The behaviour behind\n//! every handler is an implementation obligation — \
         see the `PLAN.md` beside this workspace — and\n//! until one is satisfied, its stub \
         answers with a typed refusal naming what is \
         owed.\n\n#![forbid(unsafe_code)]\n#![deny(missing_docs)]\n",
    );

    published_event_enum(&mut out, ir, layout, component, &types);
    port_struct(&mut out, ir, layout, component, &types, &port);
    Artifact::new(format!("crates/{package}/src/lib.rs"), out)
}

/// The typed outbox entry: one variant per event the component declares it publishes.
fn published_event_enum(
    out: &mut String,
    ir: &EssIr,
    layout: &Layout,
    component: &ResolvedComponent,
    types: &str,
) {
    let events: BTreeSet<&EventHandle> = component.publishes.iter().collect();
    let variants = event_variants(ir, layout, &events);
    out.push_str(
        "\n/// An event this component declares it publishes, on its way to the system's \
         transport.\n#[derive(Debug, Clone, PartialEq, Eq)]\npub enum PublishedEvent {\n",
    );
    for (event, variant) in &variants {
        let _ = writeln!(
            out,
            "    /// `{event}`.\n    {variant}({}),",
            types_path(layout, types, event.name())
        );
    }
    out.push_str("}\n");
}

/// The port itself: construction, one handler per accepted command, one query per declared view,
/// and the outbox drain the system's transport collects from.
fn port_struct(
    out: &mut String,
    ir: &EssIr,
    layout: &Layout,
    component: &ResolvedComponent,
    types: &str,
    port: &str,
) {
    let _ = writeln!(
        out,
        "\n/// {} — the port over the component's obligations.\n///\n/// `B` bundles every \
         behaviour and query this component owes; \
         constructing it over the domain's\n/// `obligations::Unimplemented` yields a component \
         that compiles and refuses, in the type system,\n/// everything not yet \
         implemented.\npub struct {port}<B> {{\n    behaviors: B,\n    outbox: \
         Vec<PublishedEvent>,\n}}",
        component
            .naming
            .display
            .as_deref()
            .unwrap_or(&component.name.to_string()),
    );

    let bounds = bound_list(ir, layout, component, types);
    let _ = writeln!(
        out,
        "\nimpl<B> {port}<B> {{\n    /// A new port over the given obligation \
         implementations.\n    pub fn new(behaviors: B) -> Self {{\n        Self {{\n            \
         behaviors,\n            outbox: Vec::new(),\n        }}\n    }}\n\n    /// Hands over \
         everything published since the last drain, in publication order.\n    ///\n    /// The \
         system's transport calls this; anything else reading it is taking events the \
         transport\n    /// will then never deliver.\n    pub fn drain_outbox(&mut self) -> \
         Vec<PublishedEvent> {{\n        core::mem::take(&mut self.outbox)\n    }}\n}}"
    );

    if bounds.is_empty() {
        return;
    }
    let _ = writeln!(
        out,
        "\nimpl<B> {port}<B>\nwhere\n    B: {},\n{{",
        bounds.join(" + ")
    );
    let mut first = true;
    for command in ir.commands.values() {
        let handle = component
            .accepts
            .iter()
            .find(|accepted| *accepted.name() == command.name);
        if handle.is_none() {
            continue;
        }
        if !first {
            out.push('\n');
        }
        first = false;
        handler(out, ir, layout, component, types, command);
    }
    for domain in &component.owns {
        for view in &ir.domain(domain).views {
            let view = ir.view(view);
            if !first {
                out.push('\n');
            }
            first = false;
            query(out, layout, types, view);
        }
    }
    out.push_str("}\n");
}

/// The obligation traits the port's `B` must satisfy, in a deterministic order: accepted
/// commands by name, then the views of owned domains by name.
pub(super) fn bound_list(
    ir: &EssIr,
    layout: &Layout,
    component: &ResolvedComponent,
    types: &str,
) -> Vec<String> {
    let mut bounds = Vec::new();
    for command in &component.accepts {
        let declared = command.name();
        bounds.push(format!(
            "{types}::{}::obligations::{}Behavior",
            layout.module(layout.owner(declared)),
            layout.type_name(declared)
        ));
    }
    for domain in &component.owns {
        for view in &ir.domain(domain).views {
            let declared = view.name();
            bounds.push(format!(
                "{types}::{}::obligations::{}Query",
                layout.module(layout.owner(declared)),
                layout.type_name(declared)
            ));
        }
    }
    bounds
}

/// One accepted command as a handler: run the owed behaviour, publish what the outcome declares.
fn handler(
    out: &mut String,
    ir: &EssIr,
    layout: &Layout,
    component: &ResolvedComponent,
    types: &str,
    command: &ess_compiler::ir::ResolvedCommand,
) {
    let input = types_path(layout, types, &command.name);
    let outcome_type = format!("{input}Outcome");
    let method = name::value_ident(&layout.type_name(&command.name));
    let _ = writeln!(
        out,
        "    /// Accepts `{}`: runs the behaviour obligation, then publishes the declared \
         events\n    /// the outcome carries.\n    ///\n    /// `Err` is the typed refusal of an \
         unmet obligation — never a domain outcome, which always\n    /// arrives as a variant of \
         the outcome type, refusals included.\n    pub fn {method}(&mut self, input: {input}) -> \
         Result<{outcome_type}, {}::obligation::UnmetObligation> {{\n        let outcome = \
         self.behaviors.{method}(input)?;\n        match &outcome {{",
        command.name, types
    );

    let domain = ir.domain(&command.domain).name.clone();
    let emit = Emit {
        ir,
        layout,
        domain: &domain,
    };
    let events: BTreeSet<&EventHandle> = component.publishes.iter().collect();
    let variants = event_variants(ir, layout, &events);
    for outcome in &command.outcomes {
        let carried: Vec<_> = items::outcome_event_fields(&emit, outcome)
            .into_iter()
            .filter(|field| variants.contains_key(field.event))
            .collect();
        let variant = name::pascal(outcome.name.as_str());
        if carried.is_empty() {
            let _ = writeln!(
                out,
                "            {outcome_type}::{variant} {{ .. }} => {{}}"
            );
            continue;
        }
        let fields = carried
            .iter()
            .map(|field| field.field.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        let _ = writeln!(
            out,
            "            {outcome_type}::{variant} {{ {fields}, .. }} => {{"
        );
        for field in &carried {
            let _ = writeln!(
                out,
                "                self.outbox.push(PublishedEvent::{}({}.clone()));",
                variants[field.event], field.field
            );
        }
        out.push_str("            }\n");
    }
    out.push_str("        }\n        Ok(outcome)\n    }\n");
}

/// One declared view as a query, delegating to the owed projection.
fn query(out: &mut String, layout: &Layout, types: &str, view: &ess_compiler::ir::ResolvedView) {
    let row = types_path(layout, types, &view.name);
    let method = name::value_ident(&layout.type_name(&view.name));
    let _ = writeln!(
        out,
        "    /// Serves `{}` at `{}` consistency, from the owed projection.\n    pub fn \
         {method}(&self) -> Result<Vec<{row}>, {types}::obligation::UnmetObligation> {{\n        \
         self.behaviors.{method}()\n    }}",
        view.name,
        view.consistency.as_str(),
    );
}

/// A declaration's absolute path from inside a component or system crate.
pub(super) fn types_path(
    layout: &Layout,
    types: &str,
    declared: &ess_domain::name::QualifiedName,
) -> String {
    format!(
        "{types}::{}::{}",
        layout.module(layout.owner(declared)),
        layout.type_name(declared)
    )
}
