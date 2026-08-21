//! The component-package emitter: one Go package per component, holding its port.
//!
//! A port is the component's outer surface exactly as the specification declares it: the commands
//! it accepts become methods, the views its domains declare become queries, and the events it
//! declares it publishes become a typed outbox. Nothing behavioural lives here — every handler
//! delegates to the behaviour obligation's interface — so the package is a skeleton in the precise
//! sense: complete surface, owed interior, and the interior's absence is a typed refusal rather
//! than a hole.
//!
//! Separate packages, not one file of a shared package, for the reason the Rust emitter gives one
//! crate per component: a component is the specification's own unit of ownership, so someone
//! deploying only `email-service` takes its package and the types packages, and nothing else.

use std::collections::BTreeSet;
use std::fmt::Write as _;

use ess_compiler::ir::{EssIr, EventHandle, ResolvedComponent};
use ess_gen::Artifact;

use crate::plan::{Capability, CapabilityKind, SynthesisPlan};

use super::items;
use super::layout::{event_variants, Layout};
use super::name;
use super::refusal::TargetRefusals;
use super::{cover, Emit, EXHAUSTIVENESS_NOTE};

/// One component's package: its outbox, the interface bundling what it owes, and the port.
pub(super) fn component_package(
    ir: &EssIr,
    plan: &SynthesisPlan,
    layout: &Layout,
    refusals: &TargetRefusals,
    component: &ResolvedComponent,
    covered: &mut BTreeSet<Capability>,
) -> Option<Artifact> {
    if !cover(
        plan,
        refusals,
        covered,
        CapabilityKind::ComponentPort,
        &component.name.to_string(),
    ) {
        return None;
    }
    let package = layout.component(&component.name);
    let emit = Emit::new(ir, layout, package, None);

    let mut doc = format!(
        "// Package {} is {} — the `{}` component of `{}` {}.\n",
        package.name,
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
        let _ = write!(doc, "//\n// {}\n", summary.trim());
    }
    doc.push_str(
        "//\n// The component's outer surface exactly as the specification declares it: accepted \
         commands as\n// methods, declared views as queries, published events as a typed outbox. \
         The behaviour behind\n// every handler is an implementation obligation — see the PLAN.md \
         beside this module — and\n// until one is satisfied, its stub answers with a typed \
         refusal naming what is owed.\n",
    );

    let mut body = String::new();
    behaviors_interface(&mut body, &emit, component);
    published_events(&mut body, &emit, component);
    port(&mut body, &emit, component);
    Some(emit.file(&plan.provenance, &doc, &body))
}

/// The interface bundling every obligation the port needs, by embedding the domain's own seams.
///
/// Embedding rather than re-declaring, so the bounded context's `Unimplemented` satisfies it
/// without knowing any component exists — and so a seam that changes shape changes in exactly one
/// place.
fn behaviors_interface(out: &mut String, emit: &Emit<'_>, component: &ResolvedComponent) {
    let name = emit.layout.behaviors(&component.name);
    let _ = writeln!(
        out,
        "\n// {name} bundles every behaviour and query this component owes.\n//\n// Constructing \
         the port over each bounded context's `Unimplemented` yields a component\n// that compiles \
         and refuses, in the type system, everything not yet implemented.\ntype {name} interface \
         {{"
    );
    for seam in bound_list(emit, component) {
        let _ = writeln!(out, "\t{seam}");
    }
    out.push_str("}\n");
}

/// The obligation interfaces the port's behaviours must satisfy, in a deterministic order:
/// accepted commands by name, then the views of owned domains by name.
pub(super) fn bound_list(emit: &Emit<'_>, component: &ResolvedComponent) -> Vec<String> {
    let mut bounds = Vec::new();
    for command in &component.accepts {
        let declared = command.name();
        bounds.push(emit.qualify(
            emit.layout.package_of(declared),
            emit.layout.behavior(declared),
        ));
    }
    for domain in &component.owns {
        for view in &emit.ir.domain(domain).views {
            let declared = view.name();
            bounds.push(emit.qualify(
                emit.layout.package_of(declared),
                emit.layout.query(declared),
            ));
        }
    }
    bounds
}

/// The typed outbox: one variant per event the component declares it publishes.
fn published_events(out: &mut String, emit: &Emit<'_>, component: &ResolvedComponent) {
    let published = emit.layout.published(&component.name);
    let _ = writeln!(
        out,
        "\n// {published} is an event this component declares it publishes, on its way to the \
         system's\n// transport.\n//"
    );
    out.push_str(EXHAUSTIVENESS_NOTE);
    let _ = writeln!(
        out,
        "type {published} interface {{\n\t{}()\n}}",
        name::marker(published)
    );

    let events: BTreeSet<&EventHandle> = component.publishes.iter().collect();
    for event in event_variants(emit.ir, emit.layout, &events).keys() {
        let variant = emit.layout.published_variant(&component.name, event.name());
        let _ = writeln!(
            out,
            "\n// {variant} is `{event}`.\ntype {variant} struct {{\n\t// Event is what was \
             published.\n\tEvent {}\n}}\n\nfunc ({variant}) {}() {{}}",
            emit.reference(event.name()),
            name::marker(published)
        );
    }
}

/// The port itself: construction, one method per accepted command, one query per declared view,
/// and the outbox drain the system's transport collects from.
fn port(out: &mut String, emit: &Emit<'_>, component: &ResolvedComponent) {
    let port = emit.layout.port(&component.name);
    let new = emit.layout.port_new(&component.name);
    let behaviors = emit.layout.behaviors(&component.name);
    let published = emit.layout.published(&component.name);
    let _ = writeln!(
        out,
        "\n// {port} is {} — the port over the component's obligations.\n//\n// The behaviours \
         and the outbox are unexported: commands enter through the methods below,\n// and the \
         system's transport is the only thing that drains what they published.\ntype {port} struct \
         {{\n\t// behaviors is everything this component owes.\n\tbehaviors {behaviors}\n\t// \
         outbox holds what has been published since the last drain.\n\toutbox \
         []{published}\n}}\n\n// {new} builds a port over the given obligation \
         implementations.\nfunc {new}(behaviors {behaviors}) *{port} {{\n\treturn \
         &{port}{{behaviors: behaviors}}\n}}\n\n// DrainOutbox hands over everything published \
         since the last drain, in publication order.\n//\n// The system's transport calls this; \
         anything else reading it is taking events the\n// transport will then never \
         deliver.\nfunc (c *{port}) DrainOutbox() []{published} {{\n\tdrained := \
         c.outbox\n\tc.outbox = nil\n\treturn drained\n}}",
        component
            .naming
            .display
            .as_deref()
            .unwrap_or(&component.name.to_string()),
    );

    for command in emit.ir.commands.values() {
        if !component
            .accepts
            .iter()
            .any(|accepted| *accepted.name() == command.name)
        {
            continue;
        }
        handler(out, emit, component, command);
    }
    for domain in &component.owns {
        for view in &emit.ir.domain(domain).views {
            query(out, emit, component, emit.ir.view(view));
        }
    }
}

/// One accepted command as a method: run the owed behaviour, publish what the outcome declares.
fn handler(
    out: &mut String,
    emit: &Emit<'_>,
    component: &ResolvedComponent,
    command: &ess_compiler::ir::ResolvedCommand,
) {
    let port = emit.layout.port(&component.name);
    let method = emit.layout.declared(&command.name);
    let input = emit.reference(&command.name);
    let outcome = emit.qualify(
        emit.layout.package_of(&command.name),
        emit.layout.outcome(&command.name),
    );
    let unmet = emit.unmet();

    let events: BTreeSet<&EventHandle> = component.publishes.iter().collect();
    let variants = event_variants(emit.ir, emit.layout, &events);
    let mut arms = String::new();
    let mut binds = false;
    let mut publishes = false;
    for declared in &command.outcomes {
        let variant = emit.qualify(
            emit.layout.package_of(&command.name),
            emit.layout
                .outcome_variant(&command.name, declared.name.as_str()),
        );
        let carried: Vec<_> = items::outcome_event_fields(emit, declared)
            .into_iter()
            .filter(|field| variants.contains_key(field.event))
            .collect();
        let _ = writeln!(&mut arms, "\tcase {variant}:");
        for field in &carried {
            binds = true;
            publishes = true;
            let _ = writeln!(
                &mut arms,
                "\t\tc.outbox = append(c.outbox, {}{{Event: value.{}}})",
                emit.layout
                    .published_variant(&component.name, field.event.name()),
                field.field
            );
        }
    }

    let _ = writeln!(
        out,
        "\n// {method} accepts `{}`: runs the behaviour obligation, then publishes the declared \
         events\n// the outcome carries.\n//\n// The second result is the typed refusal of an \
         unmet obligation — never a domain outcome,\n// which always arrives as a variant of the \
         outcome interface, refusals included.\nfunc (c *{port}) {method}(input {input}) \
         ({outcome}, {unmet}) {{\n\toutcome, unmet := c.behaviors.{method}(input)\n\tif unmet != \
         nil {{\n\t\treturn nil, unmet\n\t}}",
        command.name
    );
    if publishes {
        let subject = if binds {
            "switch value := outcome.(type) {"
        } else {
            "switch outcome.(type) {"
        };
        let _ = writeln!(out, "\t{subject}\n{arms}\t}}");
    }
    out.push_str("\treturn outcome, nil\n}\n");
}

/// One declared view as a query, delegating to the owed projection.
fn query(
    out: &mut String,
    emit: &Emit<'_>,
    component: &ResolvedComponent,
    view: &ess_compiler::ir::ResolvedView,
) {
    let port = emit.layout.port(&component.name);
    let method = emit.layout.declared(&view.name);
    let row = emit.reference(&view.name);
    let _ = writeln!(
        out,
        "\n// {method} serves `{}` at `{}` consistency, from the owed projection.\nfunc (c \
         *{port}) {method}() ([]{row}, {}) {{\n\treturn c.behaviors.{method}()\n}}",
        view.name,
        view.consistency.as_str(),
        emit.unmet(),
    );
}
