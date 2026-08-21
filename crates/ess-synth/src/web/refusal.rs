//! What a browser cannot reach, decided before a line is emitted.
//!
//! Two rules, and neither is about a language. The first is about the *page*: a command is sent through
//! the port of the component that accepts it, so a command the specification does not land on
//! exactly one component has no port to be sent to. The page still lists it — a surface that
//! silently omits a declared command reads as complete and is not — and offers no form for it,
//! with the refusal beside the entry.
//!
//! It is [`RefusalStage::Target`](crate::plan::RefusalStage::Target) and not a planning refusal
//! because the *contract* is generated: the Rust target emits the input type and the outcome enum
//! whatever the topology says. What no target can do with it and this one cannot is dispatch it.
//!
//! The second is about the *tab*: a component declared `reached_by: network` has a surface that
//! exists on a wire, and this target holds the system in the page rather than across one. A tab
//! cannot bind a socket, and a page that reached the surface over `fetch` would be reaching a
//! server this tree does not contain. So the transport is refused here and served by the other two
//! targets — which is precisely the distinction [`RefusalStage::Target`] exists to make: switching
//! targets dissolves it.

use std::collections::BTreeMap;

use ess_compiler::ir::EssIr;
use ess_domain::component::ComponentName;
use ess_domain::name::QualifiedName;

use crate::plan::{Capability, CapabilityKind, SynthesisPlan};

/// Every capability this target refuses, with the refusal in full.
pub(crate) struct TargetRefusals {
    /// Capability to detail, in capability order.
    refused: BTreeMap<Capability, String>,
}

impl TargetRefusals {
    /// Decides what a browser cannot reach of a planned specification.
    ///
    /// Only capabilities the plan marks generated are considered: one the planner already refused
    /// is not this stage's to refuse again, and two refusals of one capability would break the
    /// plan's promise that a capability gets exactly one disposition.
    pub fn of(
        ir: &EssIr,
        plan: &SynthesisPlan,
        acceptors: &BTreeMap<QualifiedName, ComponentName>,
    ) -> Self {
        let mut refused = BTreeMap::new();
        for command in ir.commands.values() {
            let source = command.name.to_string();
            if !plan.is_generated(CapabilityKind::CommandContract, &source) {
                continue;
            }
            if acceptors.contains_key(&command.name) {
                continue;
            }
            let claimants = ir
                .components
                .values()
                .filter(|component| {
                    component
                        .accepts
                        .iter()
                        .any(|accepted| accepted.name() == &command.name)
                })
                .map(|component| format!("`{}`", component.name))
                .collect::<Vec<_>>();
            let detail = if claimants.is_empty() {
                "a command is sent through the port of the component that accepts it, and no \
                 component of this specification accepts this one — so there is no port for a \
                 page to reach"
                    .to_owned()
            } else {
                format!(
                    "a command is sent through the port of the component that accepts it, and {} \
                     accept this one — a page choosing between them would be choosing an \
                     implementation, which is the selection gap register D-2 forbids the \
                     machinery to make",
                    claimants.join(" and ")
                )
            };
            refused.insert(
                Capability {
                    kind: CapabilityKind::CommandContract,
                    source,
                },
                detail,
            );
        }
        for component in ir.components.values() {
            let source = component.name.to_string();
            if !plan.is_generated(CapabilityKind::ComponentTransport, &source) {
                continue;
            }
            refused.insert(
                Capability {
                    kind: CapabilityKind::ComponentTransport,
                    source,
                },
                "the specification says this component's callers are not deployed with it, so its \
                 surface is served over HTTP — and this target is a page holding the system in one \
                 tab, which binds no socket. A page that fetched the surface instead would be \
                 reaching a server this tree does not contain"
                    .to_owned(),
            );
        }
        Self { refused }
    }

    /// `true` when this target refuses the capability.
    pub fn refuses(&self, kind: CapabilityKind, source: &str) -> bool {
        self.refused.contains_key(&Capability {
            kind,
            source: source.to_owned(),
        })
    }

    /// Why the capability was refused, or an empty string where it was not.
    pub fn detail(&self, kind: CapabilityKind, source: &str) -> String {
        self.refused
            .get(&Capability {
                kind,
                source: source.to_owned(),
            })
            .cloned()
            .unwrap_or_default()
    }

    /// Every refusal, in capability order.
    pub fn iter(&self) -> impl Iterator<Item = (&Capability, &String)> {
        self.refused.iter()
    }
}

/// The one component that accepts each command, where the specification declares exactly one.
///
/// The same question [`accepting_components`](crate::plan::accepting_components) answers for a
/// binding, asked of a command instead — because a page invokes a command directly rather than
/// through a binding, and "which port does this land on" is the same question either way.
pub(crate) fn acceptors(ir: &EssIr) -> BTreeMap<QualifiedName, ComponentName> {
    let mut out: BTreeMap<QualifiedName, Vec<&ComponentName>> = BTreeMap::new();
    for component in ir.components.values() {
        for accepted in &component.accepts {
            out.entry(accepted.name().clone())
                .or_default()
                .push(&component.name);
        }
    }
    out.into_iter()
        .filter(|(_, claimants)| claimants.len() == 1)
        .map(|(command, claimants)| (command, claimants[0].clone()))
        .collect()
}
