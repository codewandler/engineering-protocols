//! The catalogue: the model, as the page reads it.
//!
//! # Why the page is not written against the specification
//!
//! A browser realization whose command list is typed into its HTML is a browser realization that
//! is wrong the first time the specification changes, silently, in the one artifact nobody
//! regenerates. So the page holds no model at all: it renders forms, tables and labels out of this
//! document, which is emitted from the [`EssIr`] beside the code that serves it and drift-checked
//! with it.
//!
//! # It is JSON, and it travels in the module
//!
//! Committed as `catalog.json` at the root of the emitted tree — readable, diffable, and the thing
//! a reviewer opens — and pulled into the bridge crate with `include_str!`, so the page asks the
//! running system for it rather than fetching a second file. A page opened from `file://` can read
//! its own WebAssembly module and cannot always read its neighbours.
//!
//! # What is in it, and what deliberately is not
//!
//! Everything the plan marks generated and this target presents, in the specification's own
//! spellings: commands with their typed inputs and every declared outcome, events, declared
//! errors, views with their consistency and filter, entities with their lifecycles, bindings,
//! conversions, and the wire shape of every named type. Nothing about behaviour — that is an
//! obligation, and a catalogue that guessed at one would be guessing in the most visible place a
//! system has.

use std::collections::BTreeMap;

use ess_compiler::ir::{EssIr, ResolvedFailure, ResolvedField};
use ess_domain::name::QualifiedName;
use serde_json::{json, Map, Value};

use crate::plan::{condition_phrase, CapabilityKind, SynthesisDisposition};

use super::Bridge;

/// The catalogue of one specification, as canonical JSON.
pub(super) fn document(bridge: &Bridge<'_>) -> String {
    let ir = bridge.ir;
    let mut root = Map::new();
    root.insert("provenance".to_owned(), provenance(bridge));
    root.insert("system".to_owned(), json!(ir.system.to_string()));
    root.insert("version".to_owned(), json!(ir.version.to_string()));
    root.insert(
        "display".to_owned(),
        json!(ir.naming.display_or(&ir.system)),
    );
    if let Some(summary) = &ir.summary {
        root.insert("summary".to_owned(), json!(summary.trim()));
    }
    let counts = bridge.plan.counts();
    root.insert(
        "plan".to_owned(),
        json!({
            "generated": counts.generated,
            "obligations": counts.obligations,
            "refused": counts.refused,
        }),
    );
    root.insert("components".to_owned(), components(bridge));
    root.insert("commands".to_owned(), commands(bridge));
    root.insert("events".to_owned(), events(bridge));
    root.insert("errors".to_owned(), errors(bridge));
    root.insert("views".to_owned(), views(bridge));
    root.insert("entities".to_owned(), entities(bridge));
    root.insert("bindings".to_owned(), bindings(bridge));
    root.insert("conversions".to_owned(), conversions(bridge));
    root.insert("types".to_owned(), types(bridge));

    let mut json = serde_json::to_string_pretty(&Value::Object(root))
        .unwrap_or_else(|error| panic!("the catalogue serialises: {error}"));
    json.push('\n');
    json
}

/// The provenance, as the plan's own renderings carry it.
fn provenance(bridge: &Bridge<'_>) -> Value {
    serde_json::to_value(&bridge.plan.provenance)
        .unwrap_or_else(|error| panic!("provenance serialises: {error}"))
}

/// One field, as a form control and a table column can both be built from it.
fn field(declared: &ResolvedField) -> Value {
    let mut entry = Map::new();
    entry.insert("name".to_owned(), json!(declared.name));
    entry.insert(
        "wire".to_owned(),
        json!(ess_gen::schema::wire_field_name(declared)),
    );
    if let Some(display) = &declared.naming.display {
        entry.insert("display".to_owned(), json!(display));
    }
    if let Some(summary) = &declared.naming.summary {
        entry.insert("summary".to_owned(), json!(summary.trim()));
    }
    entry.insert(
        "optional".to_owned(),
        json!(declared.type_ref.is_optional()),
    );
    entry.insert("spelling".to_owned(), json!(declared.type_ref.to_string()));
    entry.insert(
        "type".to_owned(),
        serde_json::to_value(&declared.type_ref)
            .unwrap_or_else(|error| panic!("a type reference serialises: {error}")),
    );
    Value::Object(entry)
}

/// Every field of a list, in declaration order.
fn fields(declared: &[ResolvedField]) -> Value {
    Value::Array(declared.iter().map(field).collect())
}

/// The components, and what each one's port accepts and publishes.
fn components(bridge: &Bridge<'_>) -> Value {
    let mut out = Vec::new();
    for component in bridge.ir.components.values() {
        let name = QualifiedName::new(component.name.as_str()).ok();
        out.push(json!({
            "name": component.name.to_string(),
            "display": name.as_ref().map_or_else(
                || component.name.to_string(),
                |name| component.naming.display_or(name).to_owned(),
            ),
            "summary": component.naming.summary.as_deref().map(str::trim),
            "accepts": component.accepts.iter().map(ToString::to_string).collect::<Vec<_>>(),
            "publishes": component.publishes.iter().map(ToString::to_string).collect::<Vec<_>>(),
        }));
    }
    Value::Array(out)
}

/// Every command the plan marks contracted, with what it takes and everything it can result in.
///
/// Including the ones this target refuses to dispatch: a page that silently omitted a declared
/// command would be a page that reads as a complete surface and is not one. The refusal travels
/// with the entry, and the form is not offered.
fn commands(bridge: &Bridge<'_>) -> Value {
    let mut out = Vec::new();
    for command in bridge.ir.commands.values() {
        let source = command.name.to_string();
        if !bridge
            .plan
            .is_generated(CapabilityKind::CommandContract, &source)
        {
            continue;
        }
        let mut entry = Map::new();
        entry.insert("name".to_owned(), json!(source));
        entry.insert(
            "display".to_owned(),
            json!(command.naming.display_or(&command.name)),
        );
        if let Some(summary) = &command.naming.summary {
            entry.insert("summary".to_owned(), json!(summary.trim()));
        }
        entry.insert(
            "domain".to_owned(),
            json!(bridge.ir.domain(&command.domain).name.to_string()),
        );
        entry.insert("input".to_owned(), fields(&command.input));
        entry.insert("outcomes".to_owned(), outcomes(bridge, command));
        entry.insert(
            "behavior".to_owned(),
            json!(disposition(
                bridge,
                CapabilityKind::CommandBehavior,
                &source
            )),
        );
        if let Some(component) = bridge.acceptors.get(&command.name) {
            entry.insert("component".to_owned(), json!(component.to_string()));
            entry.insert("dispatchable".to_owned(), json!(true));
        } else {
            entry.insert("component".to_owned(), Value::Null);
            entry.insert("dispatchable".to_owned(), json!(false));
            entry.insert(
                "refusal".to_owned(),
                json!(bridge
                    .refusals
                    .detail(CapabilityKind::CommandContract, &source)),
            );
        }
        out.push(Value::Object(entry));
    }
    Value::Array(out)
}

/// Every declared outcome of one command, refusals beside successes.
fn outcomes(bridge: &Bridge<'_>, command: &ess_compiler::ir::ResolvedCommand) -> Value {
    let mut out = Vec::new();
    for outcome in &command.outcomes {
        out.push(json!({
            "name": outcome.name.as_str(),
            "condition": condition_phrase(&outcome.condition),
            "summary": outcome.summary.as_deref().map(str::trim),
            "publishes": outcome.emits.iter().map(ToString::to_string).collect::<Vec<_>>(),
            "error": outcome.error.as_ref().map(ToString::to_string),
            "moves": outcome.subject.as_ref().map(|subject| json!({
                "entity": subject.entity.to_string(),
                "effect": effect(&subject.effect),
            })),
        }));
        let _ = bridge;
    }
    Value::Array(out)
}

/// What an outcome does to the entity it acts on, in the specification's own words.
fn effect(effect: &ess_compiler::ir::ResolvedEffect) -> Value {
    match effect {
        ess_compiler::ir::ResolvedEffect::Creates => json!({ "kind": "creates" }),
        ess_compiler::ir::ResolvedEffect::Updates => json!({ "kind": "updates" }),
        ess_compiler::ir::ResolvedEffect::Moves { transition } => json!({
            "kind": "moves",
            "transition": transition.name,
            "from": transition.from.iter().map(ToString::to_string).collect::<Vec<_>>(),
            "to": transition.to.to_string(),
        }),
    }
}

/// Every event the plan marks generated.
fn events(bridge: &Bridge<'_>) -> Value {
    let mut out = Vec::new();
    for event in bridge.ir.events.values() {
        if !bridge.presents_event(&event.name) {
            continue;
        }
        out.push(json!({
            "name": event.name.to_string(),
            "display": event.naming.display_or(&event.name),
            "summary": event.naming.summary.as_deref().map(str::trim),
            "fields": fields(&event.fields),
        }));
    }
    Value::Array(out)
}

/// Every declared error the plan marks generated.
fn errors(bridge: &Bridge<'_>) -> Value {
    let mut out = Vec::new();
    for error in bridge.ir.errors.values() {
        if !bridge.presents_error(&error.name) {
            continue;
        }
        out.push(json!({
            "name": error.name.to_string(),
            "summary": error.summary.as_deref().map(str::trim),
            "fields": fields(&error.fields),
        }));
    }
    Value::Array(out)
}

/// Every view, with the consistency and filter that decide what its rows mean.
fn views(bridge: &Bridge<'_>) -> Value {
    let mut out = Vec::new();
    for view in bridge.ir.views.values() {
        if !bridge.presents_view(&view.name) {
            continue;
        }
        let source = view.name.to_string();
        out.push(json!({
            "name": source,
            "display": view.naming.display_or(&view.name),
            "summary": view.naming.summary.as_deref().map(str::trim),
            "entity": view.source.to_string(),
            "consistency": view.consistency.as_str(),
            "filter": view.filter.as_ref().map(ToString::to_string),
            "fields": fields(&view.fields),
            "served": disposition(bridge, CapabilityKind::ViewQuery, &source),
            "component": bridge.view_components.get(&view.name).map(ToString::to_string),
        }));
    }
    Value::Array(out)
}

/// Every entity, with the lifecycle the typed states are synthesised from.
fn entities(bridge: &Bridge<'_>) -> Value {
    let mut out = Vec::new();
    let projections = bridge.ir.projections();
    for entity in bridge.ir.entities.values() {
        if !bridge
            .plan
            .is_generated(CapabilityKind::EntityLifecycle, &entity.name.to_string())
        {
            continue;
        }
        let handle = bridge
            .ir
            .entities
            .keys()
            .find(|name| **name == entity.name)
            .map(ToString::to_string);
        let observed: Vec<String> = projections
            .iter()
            .filter(|(source, _)| source.name() == &entity.name)
            .flat_map(|(_, views)| views.iter().map(|view| view.name.to_string()))
            .collect();
        out.push(json!({
            "name": handle.unwrap_or_else(|| entity.name.to_string()),
            "display": entity.naming.display_or(&entity.name),
            "summary": entity.naming.summary.as_deref().map(str::trim),
            "identity": field(&entity.identity),
            "fields": fields(&entity.fields),
            "states": entity.lifecycle.states.iter().map(ToString::to_string).collect::<Vec<_>>(),
            "initial": entity.lifecycle.initial.to_string(),
            "terminal": entity.lifecycle.terminal.iter().map(ToString::to_string).collect::<Vec<_>>(),
            "transitions": entity.lifecycle.transitions.iter().map(|transition| json!({
                "name": transition.name,
                "from": transition.from.iter().map(ToString::to_string).collect::<Vec<_>>(),
                "to": transition.to.to_string(),
            })).collect::<Vec<_>>(),
            "invariants": entity.invariants.iter().map(|invariant| invariant.statement.clone()).collect::<Vec<_>>(),
            "views": observed,
        }));
    }
    Value::Array(out)
}

/// Every binding, with the delivery guarantee and failure policy the transport is derived from.
fn bindings(bridge: &Bridge<'_>) -> Value {
    let mut out = Vec::new();
    for binding in bridge.ir.bindings.values() {
        let source = binding.name.to_string();
        let (failure, escalation) = match binding.on_failure() {
            ResolvedFailure::Retry => ("retry", None),
            ResolvedFailure::Drop => ("drop", None),
            ResolvedFailure::Escalate { emits } => ("escalate", Some(emits.to_string())),
        };
        out.push(json!({
            "name": source,
            "event": binding.event.to_string(),
            "command": binding.command.to_string(),
            "delivery": serde_json::to_value(binding.delivery)
                .unwrap_or_else(|error| panic!("a delivery guarantee serialises: {error}")),
            "on_failure": failure,
            "escalation": escalation,
            "transformation": disposition(bridge, CapabilityKind::BindingTransformation, &source),
            "delivered": disposition(bridge, CapabilityKind::BindingDelivery, &source),
        }));
    }
    Value::Array(out)
}

/// Every crossing the specification permits, and the reason its author gave.
fn conversions(bridge: &Bridge<'_>) -> Value {
    let mut out = Vec::new();
    for conversion in &bridge.ir.conversions {
        out.push(json!({
            "from": conversion.from.to_string(),
            "to": conversion.to.to_string(),
            "because": conversion.because,
        }));
    }
    Value::Array(out)
}

/// The wire shape of every named type the page has to render or build.
fn types(bridge: &Bridge<'_>) -> Value {
    let mut out = Map::new();
    for declared in bridge.ir.types.values() {
        if !bridge.presents_type(&declared.name) {
            continue;
        }
        let mut entry = Map::new();
        entry.insert(
            "display".to_owned(),
            json!(declared.naming.display_or(&declared.name)),
        );
        if let Some(summary) = &declared.naming.summary {
            entry.insert("summary".to_owned(), json!(summary.trim()));
        }
        match &declared.body {
            ess_compiler::ir::ResolvedBody::Newtype { of, invariants } => {
                entry.insert("kind".to_owned(), json!("newtype"));
                entry.insert(
                    "of".to_owned(),
                    serde_json::to_value(of)
                        .unwrap_or_else(|error| panic!("a type reference serialises: {error}")),
                );
                entry.insert("spelling".to_owned(), json!(of.to_string()));
                entry.insert("invariants".to_owned(), statements(invariants));
            }
            ess_compiler::ir::ResolvedBody::Struct {
                fields: declared_fields,
                invariants,
            } => {
                entry.insert("kind".to_owned(), json!("struct"));
                entry.insert("fields".to_owned(), fields(declared_fields));
                entry.insert("invariants".to_owned(), statements(invariants));
            }
            ess_compiler::ir::ResolvedBody::Enum { variants } => {
                entry.insert("kind".to_owned(), json!("enum"));
                entry.insert("variants".to_owned(), json!(variants));
            }
            ess_compiler::ir::ResolvedBody::Union { tag, variants } => {
                entry.insert("kind".to_owned(), json!("union"));
                entry.insert("tag".to_owned(), json!(tag));
                entry.insert(
                    "content".to_owned(),
                    json!(ess_gen::schema::union_content_key(tag)),
                );
                let mut branches = Map::new();
                for (label, payload) in variants {
                    branches.insert(
                        label.clone(),
                        json!({
                            "spelling": payload.to_string(),
                            "type": serde_json::to_value(payload)
                                .unwrap_or_else(|error| panic!("a type reference serialises: {error}")),
                        }),
                    );
                }
                entry.insert("variants".to_owned(), Value::Object(branches));
            }
        }
        out.insert(declared.name.to_string(), Value::Object(entry));
    }
    Value::Object(out)
}

/// Invariant statements, as their authors wrote them.
fn statements(invariants: &[ess_domain::entity::Invariant]) -> Value {
    Value::Array(
        invariants
            .iter()
            .map(|invariant| json!(invariant.statement))
            .collect(),
    )
}

/// One capability's disposition, in the plan's own vocabulary.
///
/// On the page because it is the answer to the question a person asks first when a command comes
/// back refusing: *is this system saying no, or has nobody written this yet?*
fn disposition(bridge: &Bridge<'_>, kind: CapabilityKind, source: &str) -> Value {
    match bridge.plan.disposition_of(kind, source) {
        Some(SynthesisDisposition::Generated) => json!({ "disposition": "generated" }),
        Some(SynthesisDisposition::Obligation(obligation)) => json!({
            "disposition": "obligation",
            "why": obligation.reason.describes(),
            "contract": obligation.contract,
        }),
        Some(SynthesisDisposition::Refused(refusal)) => json!({
            "disposition": "refused",
            "why": refusal.detail,
        }),
        None => Value::Null,
    }
}

/// The component each view is served by, for the catalogue's `component` field.
pub(super) fn view_components(
    ir: &EssIr,
) -> BTreeMap<QualifiedName, ess_domain::component::ComponentName> {
    let mut out = BTreeMap::new();
    for component in ir.components.values() {
        for domain in &component.owns {
            for view in &ir.domain(domain).views {
                out.insert(view.name().clone(), component.name.clone());
            }
        }
    }
    out
}
