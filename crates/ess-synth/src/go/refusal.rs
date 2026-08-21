//! What this target cannot represent, decided before a line is emitted.
//!
//! The plan marks a capability generated; an emitter either emits it or says, in the plan's own
//! vocabulary, that *this language* cannot. That second answer is
//! [`RefusalStage::Target`](crate::plan::RefusalStage::Target), and it exists so a target-specific
//! refusal can never masquerade as a fact about the model — the wave-6 plan reserved the marking
//! and left it unused, because the first target had nothing to refuse.
//!
//! # The one thing Go cannot spell
//!
//! `Map<Bytes, V>`. A Go map key must be *comparable*, and `[]byte` — the only honest rendering of
//! opaque bytes, since a `string` would silently claim the bytes are text — is not. Rust's
//! `BTreeMap<Vec<u8>, V>` is ordinary. So a specification may legally declare a construct this
//! emitter must refuse, and refusing it is the whole reason a second target was worth building.
//!
//! # Refusal travels the way dependence does
//!
//! A struct holding an unrepresentable field is unrepresentable; a command whose input holds one
//! is refused; a component accepting that command is refused, and so is every binding that lands
//! on it. The alternative — a port emitted with one handler quietly missing — is the failure mode
//! this repository calls an absence, and an absence is the one thing a reader cannot review.

use std::collections::{BTreeMap, BTreeSet};

use ess_compiler::ir::{EssIr, ResolvedBody, ResolvedField, ResolvedTypeRef};
use ess_domain::name::QualifiedName;
use ess_domain::types::Primitive;

use crate::plan::{
    accepting_components, conversion_source, Capability, CapabilityKind, SynthesisDisposition,
    SynthesisPlan,
};

use super::name;

/// A capability this target cannot emit, with the cause and the path that reaches it.
struct Unrepresentable {
    /// Why, in one sentence about the target.
    cause: String,
    /// The hops from the refused construct down to the cause, outermost first.
    path: Vec<String>,
}

impl Unrepresentable {
    /// The refusal as one sentence: the cause, then where it was reached.
    fn detail(&self) -> String {
        format!("{}; reached at {}", self.cause, self.path.join(" → "))
    }

    /// The same refusal, seen from one hop further out.
    fn under(mut self, hop: String) -> Self {
        self.path.insert(0, hop);
        self
    }
}

/// Every capability this target refuses, with the refusal in full.
pub(crate) struct TargetRefusals {
    /// Capability to detail, in capability order.
    refused: BTreeMap<Capability, String>,
}

impl TargetRefusals {
    /// Decides what the Go target cannot represent of a planned specification.
    ///
    /// Only capabilities the plan marks generated or owed are considered: one the *planner*
    /// already refused is not this stage's to refuse again, and two refusals of one capability
    /// would break the plan's promise that a capability gets exactly one disposition.
    pub fn of(ir: &EssIr, plan: &SynthesisPlan) -> Self {
        let mut refused: BTreeMap<Capability, String> = BTreeMap::new();
        refuse_declarations(ir, plan, &mut refused);
        refuse_commands(ir, plan, &mut refused);
        refuse_conversions(ir, plan, &mut refused);
        cascade(ir, plan, &mut refused);
        Self { refused }
    }

    /// `true` when this target refuses the capability.
    pub fn refuses(&self, capability: &Capability) -> bool {
        self.refused.contains_key(capability)
    }

    /// `true` when this target refuses the capability, named by its two parts.
    pub fn refuses_kind(&self, kind: CapabilityKind, source: &str) -> bool {
        self.refuses(&Capability {
            kind,
            source: source.to_owned(),
        })
    }

    /// Every refusal, in capability order.
    pub fn iter(&self) -> impl Iterator<Item = (&Capability, &String)> {
        self.refused.iter()
    }
}

/// The declarations that are types all the way down: named types, entities, events, errors, views.
fn refuse_declarations(
    ir: &EssIr,
    plan: &SynthesisPlan,
    refused: &mut BTreeMap<Capability, String>,
) {
    for declared in ir.types.values() {
        if let Some(refusal) = body_refusal(ir, &declared.body) {
            let refusal = refusal.under(format!("`{}`", declared.name));
            insert(
                plan,
                refused,
                CapabilityKind::DomainType,
                &declared.name.to_string(),
                &refusal,
            );
        }
    }
    for entity in ir.entities.values() {
        let fields = std::iter::once(&entity.identity).chain(entity.fields.iter());
        if let Some(refusal) = fields_refusal(ir, fields) {
            let refusal = refusal.under(format!("`{}`", entity.name));
            insert(
                plan,
                refused,
                CapabilityKind::EntityLifecycle,
                &entity.name.to_string(),
                &refusal,
            );
        }
    }
    for event in ir.events.values() {
        if let Some(refusal) = fields_refusal(ir, event.fields.iter()) {
            let refusal = refusal.under(format!("`{}`", event.name));
            insert(
                plan,
                refused,
                CapabilityKind::EventType,
                &event.name.to_string(),
                &refusal,
            );
        }
    }
    for error in ir.errors.values() {
        if let Some(refusal) = fields_refusal(ir, error.fields.iter()) {
            let refusal = refusal.under(format!("`{}`", error.name));
            insert(
                plan,
                refused,
                CapabilityKind::ErrorType,
                &error.name.to_string(),
                &refusal,
            );
        }
    }
    for view in ir.views.values() {
        if let Some(refusal) = fields_refusal(ir, view.fields.iter()) {
            let refusal = refusal.under(format!("`{}`", view.name));
            let source = view.name.to_string();
            insert(plan, refused, CapabilityKind::ViewType, &source, &refusal);
            insert(plan, refused, CapabilityKind::ViewQuery, &source, &refusal);
        }
    }
}

/// A command, and everything its outcomes name.
fn refuse_commands(ir: &EssIr, plan: &SynthesisPlan, refused: &mut BTreeMap<Capability, String>) {
    for command in ir.commands.values() {
        let mut refusal = fields_refusal(ir, command.input.iter());
        for outcome in &command.outcomes {
            for event in &outcome.emits {
                refusal = refusal.or_else(|| {
                    fields_refusal(ir, ir.event(event).fields.iter())
                        .map(|found| found.under(format!("`{event}`")))
                });
            }
            if let Some(error) = &outcome.error {
                refusal = refusal.or_else(|| {
                    fields_refusal(ir, ir.error(error).fields.iter())
                        .map(|found| found.under(format!("`{error}`")))
                });
            }
        }
        if let Some(refusal) = refusal {
            let refusal = refusal.under(format!("`{}`", command.name));
            let source = command.name.to_string();
            insert(
                plan,
                refused,
                CapabilityKind::CommandContract,
                &source,
                &refusal,
            );
            insert(
                plan,
                refused,
                CapabilityKind::CommandBehavior,
                &source,
                &refusal,
            );
        }
    }
}

/// A declared crossing, from either end.
fn refuse_conversions(
    ir: &EssIr,
    plan: &SynthesisPlan,
    refused: &mut BTreeMap<Capability, String>,
) {
    for conversion in &ir.conversions {
        let refusal = type_refusal(ir, &conversion.from)
            .map(|found| found.under(format!("`{}`", conversion.from)))
            .or_else(|| {
                type_refusal(ir, &conversion.to)
                    .map(|found| found.under(format!("`{}`", conversion.to)))
            });
        if let Some(refusal) = refusal {
            insert(
                plan,
                refused,
                CapabilityKind::Conversion,
                &conversion_source(conversion),
                &refusal,
            );
        }
    }
}

/// Records one refusal, but only where the plan left the capability to this stage to answer.
fn insert(
    plan: &SynthesisPlan,
    refused: &mut BTreeMap<Capability, String>,
    kind: CapabilityKind,
    source: &str,
    refusal: &Unrepresentable,
) {
    if matches!(
        plan.disposition_of(kind, source),
        Some(SynthesisDisposition::Generated | SynthesisDisposition::Obligation(_))
    ) {
        refused.insert(
            Capability {
                kind,
                source: source.to_owned(),
            },
            refusal.detail(),
        );
    }
}

/// The units whose whole surface a refused declaration takes with it: component ports, and the
/// bindings that land on them.
///
/// Second pass rather than folded into the first, because it reads the answers the first pass
/// produced — and because a cascade computed while the set it reads is still growing is a cascade
/// whose result depends on iteration order.
fn cascade(ir: &EssIr, plan: &SynthesisPlan, refused: &mut BTreeMap<Capability, String>) {
    let reason = |refused: &BTreeMap<Capability, String>, kind: CapabilityKind, source: String| {
        refused.get(&Capability { kind, source }).cloned()
    };

    let mut ports: BTreeMap<Capability, String> = BTreeMap::new();
    for component in ir.components.values() {
        let mut because = None;
        for command in &component.accepts {
            because = because.or_else(|| {
                reason(
                    refused,
                    CapabilityKind::CommandContract,
                    command.name().to_string(),
                )
            });
        }
        for event in &component.publishes {
            because = because
                .or_else(|| reason(refused, CapabilityKind::EventType, event.name().to_string()));
        }
        for domain in &component.owns {
            for view in &ir.domain(domain).views {
                because = because
                    .or_else(|| reason(refused, CapabilityKind::ViewType, view.name().to_string()));
            }
        }
        because = because.or_else(|| method_collision(ir, component));
        if let Some(because) = because {
            ports.insert(
                Capability {
                    kind: CapabilityKind::ComponentPort,
                    source: component.name.to_string(),
                },
                because,
            );
        }
    }

    let mut bindings: BTreeMap<Capability, String> = BTreeMap::new();
    for binding in ir.bindings.values() {
        let mut because = reason(
            refused,
            CapabilityKind::EventType,
            binding.event.name().to_string(),
        )
        .or_else(|| {
            reason(
                refused,
                CapabilityKind::CommandContract,
                binding.command.name().to_string(),
            )
        })
        .or_else(|| {
            binding.escalation.as_ref().and_then(|escalation| {
                reason(
                    refused,
                    CapabilityKind::EventType,
                    escalation.name().to_string(),
                )
            })
        });
        for acceptor in accepting_components(ir, binding) {
            because = because.or_else(|| {
                ports
                    .get(&Capability {
                        kind: CapabilityKind::ComponentPort,
                        source: acceptor.name.to_string(),
                    })
                    .cloned()
            });
        }
        if let Some(because) = because {
            for kind in [
                CapabilityKind::BindingTransformation,
                CapabilityKind::BindingDelivery,
                CapabilityKind::BindingEscalation,
            ] {
                bindings.insert(
                    Capability {
                        kind,
                        source: binding.name.to_string(),
                    },
                    because.clone(),
                );
            }
        }
    }

    for (capability, detail) in ports.into_iter().chain(bindings) {
        if matches!(
            plan.disposition_of(capability.kind, &capability.source),
            Some(SynthesisDisposition::Generated | SynthesisDisposition::Obligation(_))
        ) {
            refused.insert(capability, detail);
        }
    }
}

/// Why a declared type's body cannot be represented in Go, or `None`.
fn body_refusal(ir: &EssIr, body: &ResolvedBody) -> Option<Unrepresentable> {
    match body {
        ResolvedBody::Newtype { of, .. } => type_refusal(ir, of),
        ResolvedBody::Struct { fields, .. } => fields_refusal(ir, fields.iter()),
        ResolvedBody::Enum { .. } => None,
        ResolvedBody::Union { variants, .. } => variants.iter().find_map(|(tag, variant)| {
            type_refusal(ir, variant).map(|found| found.under(format!("variant `{tag}`")))
        }),
    }
}

/// Why a set of fields cannot be represented in Go, or `None`.
fn fields_refusal<'a>(
    ir: &EssIr,
    fields: impl Iterator<Item = &'a ResolvedField>,
) -> Option<Unrepresentable> {
    fields.into_iter().find_map(|field| {
        type_refusal(ir, &field.type_ref)
            .map(|found| found.under(format!("field `{}`", field.name)))
    })
}

/// Why a resolved type reference cannot be represented in Go, or `None`.
fn type_refusal(ir: &EssIr, type_ref: &ResolvedTypeRef) -> Option<Unrepresentable> {
    let mut seen = BTreeSet::new();
    unrepresentable(ir, type_ref, &mut seen)
}

/// The recursive half, carrying the declarations already entered so a self-referential type
/// terminates.
fn unrepresentable(
    ir: &EssIr,
    type_ref: &ResolvedTypeRef,
    seen: &mut BTreeSet<QualifiedName>,
) -> Option<Unrepresentable> {
    match type_ref {
        ResolvedTypeRef::Primitive { .. } => None,
        ResolvedTypeRef::Declared { name } => {
            if !seen.insert(name.name().clone()) {
                return None;
            }
            let declared = ir.named_type(name);
            let found = match &declared.body {
                ResolvedBody::Newtype { of, .. } => unrepresentable(ir, of, seen),
                ResolvedBody::Struct { fields, .. } => fields.iter().find_map(|field| {
                    unrepresentable(ir, &field.type_ref, seen)
                        .map(|found| found.under(format!("field `{}`", field.name)))
                }),
                ResolvedBody::Enum { .. } => None,
                ResolvedBody::Union { variants, .. } => {
                    variants.iter().find_map(|(tag, variant)| {
                        unrepresentable(ir, variant, seen)
                            .map(|found| found.under(format!("variant `{tag}`")))
                    })
                }
            };
            found.map(|found| found.under(format!("`{name}`")))
        }
        ResolvedTypeRef::Optional { of } | ResolvedTypeRef::List { of } => {
            unrepresentable(ir, of, seen)
        }
        ResolvedTypeRef::Map { key, value } => {
            if *key == Primitive::Bytes {
                return Some(Unrepresentable {
                    cause: "a Go map key must be comparable, and `Bytes` is `[]byte`, which is \
                            not — rendering it as text instead would claim the bytes are text"
                        .to_owned(),
                    path: vec![format!("`{type_ref}`")],
                });
            }
            unrepresentable(ir, value, seen)
        }
    }
}

/// The second thing Go cannot spell: two obligation seams of one component whose methods collide.
///
/// A component's behaviours are bundled by embedding each bounded context's own interface, and Go
/// gives a type **one method set** — so two accepted commands (or a command and a view) that
/// derive the same method name cannot both be embedded. Rust never meets this: a trait method is
/// disambiguated by its trait. The candidate name is used rather than the layout's allocated one
/// because the layout is derived *from* this answer, and because a repair inside one package
/// cannot separate two names that come from different packages anyway.
fn method_collision(ir: &EssIr, component: &ess_compiler::ir::ResolvedComponent) -> Option<String> {
    let mut methods: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut record = |declared: &QualifiedName, domain: &QualifiedName| {
        methods
            .entry(name::type_name(declared, domain.segments().len()))
            .or_default()
            .push(declared.to_string());
    };
    for command in &component.accepts {
        let resolved = ir.command(command);
        record(&resolved.name, &ir.domain(&resolved.domain).name);
    }
    for domain in &component.owns {
        for view in &ir.domain(domain).views {
            let resolved = ir.view(view);
            record(&resolved.name, &ir.domain(&resolved.domain).name);
        }
    }
    methods.into_iter().find_map(|(method, sources)| {
        (sources.len() > 1).then(|| {
            format!(
                "Go gives a type one method set, so the interface bundling a component's \
                 obligations cannot embed two seams that both declare `{method}` — `{}` do; \
                 reached at `{}`",
                sources.join("` and `"),
                component.name
            )
        })
    })
}
