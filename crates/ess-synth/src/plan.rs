//! The synthesis plan: every semantic capability of a specification, with exactly one disposition.
//!
//! # The hinge
//!
//! Code generation here is not `EssIr → files`. It is `EssIr → SynthesisPlan → files`, and the plan
//! is the product that matters: before a line of code exists, an implementor can read what will be
//! generated, what is theirs to write (**obligation** — the contract is declared, the behaviour is
//! not), and what this synthesis refuses to pretend it can represent (**refused**, with the
//! reason). A generator without the plan reports what it could not do as an absence, and an absence
//! is the one thing a reader cannot review.
//!
//! # The plan is language-neutral
//!
//! Rust is the *first* emission target, not the only intended one. So nothing in this module names
//! a language: a capability is a fact about the model (`billing.invoice.CreateInvoice` has a
//! contract and a behaviour), an obligation's contract is phrased in the specification's own
//! spellings, and a refusal is about what this synthesis *stage* covers. Everything
//! language-shaped — file layout, identifier derivation, representation choices — lives behind the
//! emitter seam ([`crate::rust`]), which consumes a finished plan and the IR. A refusal that only
//! one target has to make is marked [`RefusalStage::Target`] so a reader can tell a fact about the
//! model's coverage from a fact about one language; every refusal this planner writes is
//! [`RefusalStage::Planning`].
//!
//! # The scope owns ports and one transport
//!
//! The first slice planned semantic types only, and refused every binding and every component port
//! as "needs the interaction layer". This scope holds that layer: a component's outer surface —
//! command handlers and view queries — is generated, a binding's transformation is generated
//! exactly where every mapped input is determined, and its delivery is generated as the one
//! transport the specification's own words require (`at_least_once`, the only delivery guarantee
//! the model declares). What the interaction layer still cannot determine — how an escalation
//! event's fields are filled, how a value crosses a declared conversion that is not mechanical —
//! becomes an obligation with its contract, never a hole.
//!
//! # No guessing
//!
//! Nothing becomes [`Generated`](SynthesisDisposition::Generated) unless the specification fully
//! determines it. The billing example's `SendEmail` is the worked case: its input, outcomes, error
//! and event types are generated, and the behaviour — whether a provider accepts an address — is an
//! obligation carrying the specification's own words for why (`external: the provider rejects the
//! recipient address`). Generated business logic that *looks* plausible is the failure mode design
//! §5 names, and it is unrepresentable here: there is no disposition for "generated, roughly".
//!
//! # One disposition, exactly
//!
//! [`SynthesisPlan::of`] enumerates every construct of the IR — a command contributes two
//! capabilities, its contract and its behaviour, because those genuinely have different
//! dispositions — and refuses to finish if any capability would carry two. The billing plan's
//! numbers are pinned by tests: a construct that silently gains or loses a disposition is a
//! failing test, not a thinner document.
//!
//! # Determinism
//!
//! Same IR, same bytes. Every sweep below iterates the IR's own ordered collections, nothing reads
//! a clock, and `tests/synthesis.rs` plans twice and compares — beside a scan that keeps unordered
//! maps and clocks out of this crate's sources at all.

use std::collections::BTreeSet;
use std::fmt::Write as _;

use ess_compiler::ir::{
    EssIr, ResolvedBinding, ResolvedBody, ResolvedCommand, ResolvedComponent, ResolvedCondition,
    ResolvedConversion, ResolvedEffect, ResolvedFailure, ResolvedField, ResolvedMappingValue,
    ResolvedTypeRef, ResolvedView, TypeHandle,
};
use ess_domain::component::Reach;
use ess_gen::Provenance;

/// The command that rewrites what this plan describes, named in every rendering of it.
pub const REGENERATE: &str = "protocol ess synthesize";

/// What one specification determines, capability by capability.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct SynthesisPlan {
    /// The specification this plan is a projection of, and the build that resolved it.
    pub provenance: Provenance,
    /// What slice of the specification this synthesis covers, and the build that planned it.
    pub scope: SynthesisScope,
    /// Every semantic capability, each with exactly one disposition, in planning order:
    /// declarations first, behaviour beside its contract, the interaction layer last.
    pub capabilities: Vec<PlannedCapability>,
}

/// The scope a plan was computed for.
///
/// A disposition is a fact about a *pair* — this specification against this scope — not about the
/// specification alone: the same binding that is refused by the semantic-types scope is generated
/// by the scope that owns ports. Naming the scope in the plan is what stops a refusal from reading
/// as a verdict on the specification. Deliberately not a *language*: one plan serves every
/// emission target, and anything a single target cannot do is that target's refusal to report,
/// marked [`RefusalStage::Target`].
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct SynthesisScope {
    /// Which slice of the specification's capabilities this synthesis owns.
    pub profile: String,
    /// The build that planned.
    pub planner_version: String,
}

impl SynthesisScope {
    /// This wave's scope: semantic types, component skeletons, and the one transport the
    /// specification's bindings require — no topology, no runtime beyond in-process delivery.
    fn component_skeletons() -> Self {
        Self {
            profile: "component-skeletons".to_owned(),
            planner_version: env!("CARGO_PKG_VERSION").to_owned(),
        }
    }
}

/// One capability and the single disposition it received.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct PlannedCapability {
    /// What the specification requires.
    #[serde(flatten)]
    pub capability: Capability,
    /// What this synthesis does about it.
    pub disposition: SynthesisDisposition,
}

/// One semantic capability a specification requires of an implementation.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
pub struct Capability {
    /// Which kind of requirement it is.
    pub kind: CapabilityKind,
    /// The construct that requires it, in the specification's own spelling.
    pub source: String,
}

/// The kinds of capability this planner enumerates.
///
/// A construct can require more than one — a command requires both a contract and a behaviour, a
/// view both a row type and a query — and splitting them is what lets each half carry the honest
/// disposition instead of the average of two.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityKind {
    /// A declared (or lifecycle-synthesised) named type.
    DomainType,
    /// An entity's data, its typed lifecycle, and the boundary between them.
    EntityLifecycle,
    /// A command's input and its set of declared outcomes, as types.
    CommandContract,
    /// The behaviour that decides which outcome a command takes and enacts it.
    CommandBehavior,
    /// An event's payload type.
    EventType,
    /// A declared error's payload type.
    ErrorType,
    /// A view's row type.
    ViewType,
    /// The query that serves a view at its declared consistency.
    ViewQuery,
    /// A declared crossing between two types.
    Conversion,
    /// An actor's grants.
    ActorGrants,
    /// A binding's transformation: the triggering event, read as the invoked command's input.
    BindingTransformation,
    /// A binding's delivery: invoking the command on the component that accepts it, with the
    /// declared guarantee and failure policy.
    BindingDelivery,
    /// A binding's escalation: the declared event that records a delivery being given up on.
    ///
    /// A capability only where the binding's failure policy is `escalate` — `retry` and `drop`
    /// declare nothing to construct.
    BindingEscalation,
    /// A component's port surface.
    ComponentPort,
    /// The transport a component's surface is served over.
    ///
    /// A capability only where the specification says the surface leaves the process — `reached_by:
    /// network`. A component whose callers are deployed with it is reached by calling its port, and
    /// there is nothing to serve.
    ComponentTransport,
    /// A component's runtime requirements.
    Workload,
}

impl CapabilityKind {
    /// The kind as it reads in a sentence or a table cell.
    pub fn describes(self) -> &'static str {
        match self {
            Self::DomainType => "domain type",
            Self::EntityLifecycle => "entity lifecycle",
            Self::CommandContract => "command contract",
            Self::CommandBehavior => "command behaviour",
            Self::EventType => "event type",
            Self::ErrorType => "error type",
            Self::ViewType => "view type",
            Self::ViewQuery => "view query",
            Self::Conversion => "conversion",
            Self::ActorGrants => "actor grants",
            Self::BindingTransformation => "binding transformation",
            Self::BindingDelivery => "binding delivery",
            Self::BindingEscalation => "binding escalation",
            Self::ComponentPort => "component port",
            Self::ComponentTransport => "component transport",
            Self::Workload => "workload",
        }
    }
}

/// What the planner decided for one capability. Three cases, no fourth: generated in full, owed in
/// full, or refused out loud.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "disposition", rename_all = "snake_case")]
pub enum SynthesisDisposition {
    /// The specification fully determines it, and every emitter emits it.
    Generated,
    /// The contract is declared; the behaviour is the implementor's. Never a hole in a generated
    /// file — a typed entry in this document, with the reason it cannot be generated.
    Obligation(ImplementationObligation),
    /// This synthesis does not represent it, and says so rather than approximating.
    Refused(SynthesisRefusal),
}

/// A capability the implementor owes, with the contract they owe it against.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ImplementationObligation {
    /// Why it cannot be generated.
    pub reason: ObligationReason,
    /// What satisfying it looks like, in the specification's own spellings.
    pub contract: String,
}

/// Why an obligation is one.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "class", rename_all = "snake_case")]
pub enum ObligationReason {
    /// An outcome is decided outside the system, and the specification says by what.
    External {
        /// The cause, in the specification author's words.
        cause: String,
    },
    /// The contract is fully declared; the algorithm that satisfies it is not.
    UnspecifiedAlgorithm,
    /// Keeping a projection current at its declared consistency is a storage decision the
    /// specification deliberately does not take.
    ProjectionMaintenance,
}

impl ObligationReason {
    /// The reason as it reads in a sentence.
    pub fn describes(&self) -> String {
        match self {
            Self::External { cause } => format!("decided outside the system: {cause}"),
            Self::UnspecifiedAlgorithm => {
                "the contract is declared; the algorithm is not".to_owned()
            }
            Self::ProjectionMaintenance => {
                "how the projection is kept current is a storage decision".to_owned()
            }
        }
    }
}

/// A capability this synthesis refuses to represent, and why.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct SynthesisRefusal {
    /// The reason class.
    pub reason: RefusalReason,
    /// Which stage refused: the plan, or one emission target.
    pub stage: RefusalStage,
    /// The refusal in full, naming the construct's own facts.
    pub detail: String,
}

/// Which stage a refusal came from.
///
/// The distinction a reader needs before acting on one: a [`Planning`](Self::Planning) refusal
/// holds for every emission target and changes only when the scope grows; a
/// [`Target`](Self::Target) refusal is one language's limitation, and switching targets can
/// dissolve it. Every refusal the planner writes today is `Planning` — the Rust emitter has none —
/// but the marking exists so the first target-stage refusal cannot masquerade as a fact about the
/// model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RefusalStage {
    /// Refused by the planner: outside this synthesis scope, for every target.
    Planning,
    /// Refused by one emission target: the capability is plannable, but that target cannot
    /// represent it.
    Target,
}

/// Why a refusal is one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RefusalReason {
    /// The capability is about who may call, and types carry no caller identity. Deliberately not
    /// an obligation: deriving anything grant-shaped from a plan is the second grant path
    /// invariant 6 exists to forbid.
    NeedsCallerIdentity,
    /// Delivery lands on the component that accepts the command, and the specification does not
    /// declare exactly one. Deliberately not an obligation and never a choice: picking an acceptor
    /// among zero or several is the selection the D-2 rule forbids the machinery to make.
    AcceptorUndetermined,
    /// Topology synthesis is deferred with its design (§35).
    TopologyDeferred,
}

impl RefusalReason {
    /// The reason as it reads in a sentence.
    pub fn describes(self) -> &'static str {
        match self {
            Self::NeedsCallerIdentity => {
                "a grant is checked against a caller identity, which types do not carry"
            }
            Self::AcceptorUndetermined => {
                "delivery lands on the component that accepts the command, and the specification \
                 does not declare exactly one"
            }
            Self::TopologyDeferred => "topology synthesis is deferred with its design",
        }
    }
}

/// How many capabilities took each disposition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct DispositionCounts {
    /// Fully determined and emitted.
    pub generated: usize,
    /// Owed by the implementor, each with a contract.
    pub obligations: usize,
    /// Not represented by this synthesis, each with a reason.
    pub refused: usize,
}

impl SynthesisPlan {
    /// Plans the synthesis of a resolved specification for the semantic-types scope.
    ///
    /// # Panics
    ///
    /// If the planner would give one capability two dispositions — which is a defect in this crate,
    /// not in any specification, and papering over it would make the plan's central promise false.
    pub fn of(ir: &EssIr) -> Self {
        let mut capabilities = Vec::new();
        plan_types(ir, &mut capabilities);
        plan_entities(ir, &mut capabilities);
        plan_commands(ir, &mut capabilities);
        plan_events(ir, &mut capabilities);
        plan_errors(ir, &mut capabilities);
        plan_views(ir, &mut capabilities);
        plan_conversions(ir, &mut capabilities);
        plan_actors(ir, &mut capabilities);
        plan_bindings(ir, &mut capabilities);
        plan_components(ir, &mut capabilities);
        plan_workloads(ir, &mut capabilities);

        let mut seen = BTreeSet::new();
        for planned in &capabilities {
            assert!(
                seen.insert(planned.capability.clone()),
                "the planner gave `{}` ({}) two dispositions; a capability gets exactly one",
                planned.capability.source,
                planned.capability.kind.describes()
            );
        }

        Self {
            provenance: Provenance::of(ir),
            scope: SynthesisScope::component_skeletons(),
            capabilities,
        }
    }

    /// How many capabilities took each disposition.
    pub fn counts(&self) -> DispositionCounts {
        let mut counts = DispositionCounts {
            generated: 0,
            obligations: 0,
            refused: 0,
        };
        for planned in &self.capabilities {
            match &planned.disposition {
                SynthesisDisposition::Generated => counts.generated += 1,
                SynthesisDisposition::Obligation(_) => counts.obligations += 1,
                SynthesisDisposition::Refused(_) => counts.refused += 1,
            }
        }
        counts
    }

    /// The disposition one capability received.
    pub fn disposition_of(
        &self,
        kind: CapabilityKind,
        source: &str,
    ) -> Option<&SynthesisDisposition> {
        self.capabilities
            .iter()
            .find(|planned| planned.capability.kind == kind && planned.capability.source == source)
            .map(|planned| &planned.disposition)
    }

    /// `true` when the plan marks this capability generated — the question every emitter asks
    /// before writing a line, because emitting something the plan does not claim is how a
    /// generator's output and its document come to disagree.
    pub fn is_generated(&self, kind: CapabilityKind, source: &str) -> bool {
        matches!(
            self.disposition_of(kind, source),
            Some(SynthesisDisposition::Generated)
        )
    }

    /// Every capability the plan marks generated, in planning order.
    pub fn generated(&self) -> impl Iterator<Item = &Capability> {
        self.capabilities.iter().filter_map(|planned| {
            matches!(planned.disposition, SynthesisDisposition::Generated)
                .then_some(&planned.capability)
        })
    }

    /// What one capability owes, when the plan marks it an obligation — the question an emitter
    /// asks before writing a stub, because a stub the plan does not owe is a hole wearing a
    /// refusal's clothes.
    pub fn obligation_of(
        &self,
        kind: CapabilityKind,
        source: &str,
    ) -> Option<&ImplementationObligation> {
        match self.disposition_of(kind, source) {
            Some(SynthesisDisposition::Obligation(obligation)) => Some(obligation),
            _ => None,
        }
    }

    /// Every capability the plan marks an obligation, with what is owed, in planning order.
    pub fn obligations(&self) -> impl Iterator<Item = (&Capability, &ImplementationObligation)> {
        self.capabilities
            .iter()
            .filter_map(|planned| match &planned.disposition {
                SynthesisDisposition::Obligation(obligation) => {
                    Some((&planned.capability, obligation))
                }
                _ => None,
            })
    }

    /// The plan as canonical JSON: stable key order, two-space indentation, trailing newline — the
    /// same convention as `EssIr::to_canonical_json`, for the same reason: the committed copy is
    /// compared byte for byte.
    pub fn to_canonical_json(&self) -> String {
        let mut json = serde_json::to_string_pretty(self)
            .unwrap_or_else(|error| panic!("the plan serialises: {error}"));
        json.push('\n');
        json
    }

    /// The plan as Markdown, for the person deciding whether to trust the generated half.
    ///
    /// Language-neutral, like everything else in the plan: which files a target writes and how it
    /// spells an identifier are that target's facts, recorded in that target's output.
    pub fn to_markdown(&self) -> String {
        let mut out = self.provenance.html_comment_for(REGENERATE);
        let counts = self.counts();
        let _ = write!(
            out,
            "# Synthesis plan — {} {}\n\nScope: `{}`, planner `ess-synth {}`. Regenerate with \
             `{REGENERATE}`.\n\n{} capabilities: **{} generated**, **{} obligations**, **{} \
             refused**. An obligation is yours to implement against its contract; a refusal is a \
             fact about this synthesis scope, not about the specification.\n",
            self.provenance.system,
            self.provenance.specification_version,
            self.scope.profile,
            self.scope.planner_version,
            self.capabilities.len(),
            counts.generated,
            counts.obligations,
            counts.refused,
        );
        self.markdown_generated(&mut out);
        self.markdown_obligations(&mut out);
        self.markdown_refusals(&mut out);
        out
    }

    /// The generated table: what every emitter of this plan emits.
    fn markdown_generated(&self, out: &mut String) {
        out.push_str("\n## Generated\n\n| capability | source |\n| --- | --- |\n");
        for planned in &self.capabilities {
            if planned.disposition == SynthesisDisposition::Generated {
                let _ = writeln!(
                    out,
                    "| {} | `{}` |",
                    planned.capability.kind.describes(),
                    planned.capability.source,
                );
            }
        }
    }

    /// The obligations table: the typed list of exactly what remains.
    fn markdown_obligations(&self, out: &mut String) {
        out.push_str(
            "\n## Obligations — yours to implement\n\n| capability | source | why not generated | \
             contract |\n| --- | --- | --- | --- |\n",
        );
        for planned in &self.capabilities {
            if let SynthesisDisposition::Obligation(obligation) = &planned.disposition {
                let _ = writeln!(
                    out,
                    "| {} | `{}` | {} | {} |",
                    planned.capability.kind.describes(),
                    planned.capability.source,
                    obligation.reason.describes(),
                    obligation.contract
                );
            }
        }
    }

    /// The refusals table: what this synthesis does not represent, said out loud.
    fn markdown_refusals(&self, out: &mut String) {
        out.push_str(
            "\n## Refused — not represented by this synthesis\n\n| capability | source | stage | \
             why |\n| --- | --- | --- | --- |\n",
        );
        for planned in &self.capabilities {
            if let SynthesisDisposition::Refused(refusal) = &planned.disposition {
                let _ = writeln!(
                    out,
                    "| {} | `{}` | {} | {} |",
                    planned.capability.kind.describes(),
                    planned.capability.source,
                    match refusal.stage {
                        RefusalStage::Planning => "planning",
                        RefusalStage::Target => "target",
                    },
                    refusal.detail
                );
            }
        }
    }
}

/// Every declared type is fully determined, the lifecycle-synthesised state enums included.
fn plan_types(ir: &EssIr, capabilities: &mut Vec<PlannedCapability>) {
    for declared in ir.types.values() {
        capabilities.push(PlannedCapability {
            capability: Capability {
                kind: CapabilityKind::DomainType,
                source: declared.name.to_string(),
            },
            disposition: SynthesisDisposition::Generated,
        });
    }
}

/// An entity's data and lifecycle are fully determined — including that its illegal transitions
/// are unexpressable in the generated state surface, which is this synthesis's rendering of "a
/// move nobody declared is a move nobody may make".
fn plan_entities(ir: &EssIr, capabilities: &mut Vec<PlannedCapability>) {
    for entity in ir.entities.values() {
        capabilities.push(PlannedCapability {
            capability: Capability {
                kind: CapabilityKind::EntityLifecycle,
                source: entity.name.to_string(),
            },
            disposition: SynthesisDisposition::Generated,
        });
    }
}

/// A command's contract is generated; its behaviour is owed.
fn plan_commands(ir: &EssIr, capabilities: &mut Vec<PlannedCapability>) {
    for command in ir.commands.values() {
        capabilities.push(PlannedCapability {
            capability: Capability {
                kind: CapabilityKind::CommandContract,
                source: command.name.to_string(),
            },
            disposition: SynthesisDisposition::Generated,
        });
        capabilities.push(PlannedCapability {
            capability: Capability {
                kind: CapabilityKind::CommandBehavior,
                source: command.name.to_string(),
            },
            disposition: SynthesisDisposition::Obligation(ImplementationObligation {
                reason: behavior_reason(command),
                contract: behavior_contract(command),
            }),
        });
    }
}

/// External when any outcome is: the specification itself says the input cannot decide it.
fn behavior_reason(command: &ResolvedCommand) -> ObligationReason {
    for outcome in &command.outcomes {
        if let ResolvedCondition::External { cause } = &outcome.condition {
            return ObligationReason::External {
                cause: cause.clone(),
            };
        }
    }
    ObligationReason::UnspecifiedAlgorithm
}

/// The behaviour's contract, phrased against the model: the input, and every declared outcome with
/// what taking it entails.
fn behavior_contract(command: &ResolvedCommand) -> String {
    let mut branches = Vec::new();
    for outcome in &command.outcomes {
        let mut branch = format!(
            "`{}` {}",
            outcome.name,
            condition_phrase(&outcome.condition)
        );
        if let Some(subject) = &outcome.subject {
            let _ = write!(
                branch,
                ", {} `{}`",
                effect_phrase(&subject.effect),
                subject.entity
            );
        }
        for event in &outcome.emits {
            let _ = write!(branch, ", emits `{event}`");
        }
        if let Some(error) = &outcome.error {
            let _ = write!(branch, ", error `{error}`");
        }
        branches.push(branch);
    }
    format!(
        "given `{}` input, decide and enact exactly one outcome — {}",
        command.name,
        branches.join("; ")
    )
}

/// A condition as a contract phrase.
///
/// `pub(crate)` because the Rust emitter's doc comments quote the same phrasing: one sentence per
/// condition, however many documents carry it, or the plan and the code drift apart in wording
/// over the one fact they must agree on.
pub(crate) fn condition_phrase(condition: &ResolvedCondition) -> String {
    match condition {
        ResolvedCondition::When { predicate } => format!("when `{predicate}`"),
        ResolvedCondition::Otherwise => "otherwise".to_owned(),
        ResolvedCondition::External { cause } => format!("externally decided ({cause})"),
        ResolvedCondition::WrongState => "from a state no declared move starts in".to_owned(),
    }
}

/// An effect as a contract verb.
fn effect_phrase(effect: &ResolvedEffect) -> String {
    match effect {
        ResolvedEffect::Creates => "creates".to_owned(),
        ResolvedEffect::Moves { transition } => format!("takes `{}` of", transition.name),
        ResolvedEffect::Updates => "updates".to_owned(),
    }
}

/// An event's payload is fully determined.
fn plan_events(ir: &EssIr, capabilities: &mut Vec<PlannedCapability>) {
    for event in ir.events.values() {
        capabilities.push(PlannedCapability {
            capability: Capability {
                kind: CapabilityKind::EventType,
                source: event.name.to_string(),
            },
            disposition: SynthesisDisposition::Generated,
        });
    }
}

/// An error's payload is fully determined.
fn plan_errors(ir: &EssIr, capabilities: &mut Vec<PlannedCapability>) {
    for error in ir.errors.values() {
        capabilities.push(PlannedCapability {
            capability: Capability {
                kind: CapabilityKind::ErrorType,
                source: error.name.to_string(),
            },
            disposition: SynthesisDisposition::Generated,
        });
    }
}

/// A view's row type is generated; serving it is owed.
fn plan_views(ir: &EssIr, capabilities: &mut Vec<PlannedCapability>) {
    for view in ir.views.values() {
        capabilities.push(PlannedCapability {
            capability: Capability {
                kind: CapabilityKind::ViewType,
                source: view.name.to_string(),
            },
            disposition: SynthesisDisposition::Generated,
        });
        capabilities.push(PlannedCapability {
            capability: Capability {
                kind: CapabilityKind::ViewQuery,
                source: view.name.to_string(),
            },
            disposition: SynthesisDisposition::Obligation(ImplementationObligation {
                reason: ObligationReason::ProjectionMaintenance,
                contract: view_contract(view),
            }),
        });
    }
}

/// The query's contract: what rows, of what, how fresh.
fn view_contract(view: &ResolvedView) -> String {
    let mut contract = format!(
        "a query answering `{}` with rows projected from `{}` at `{}` consistency",
        view.name,
        view.source,
        view.consistency.as_str()
    );
    if let Some(filter) = &view.filter {
        let _ = write!(contract, ", containing instances where `{filter}`");
    }
    contract
}

/// A crossing between two newtypes over one representation is mechanical and generated; any other
/// declared crossing has a declared *permission* but no declared *computation*, so it is owed.
fn plan_conversions(ir: &EssIr, capabilities: &mut Vec<PlannedCapability>) {
    for conversion in &ir.conversions {
        let disposition = if mechanical_conversion(ir, conversion).is_some() {
            SynthesisDisposition::Generated
        } else {
            SynthesisDisposition::Obligation(ImplementationObligation {
                reason: ObligationReason::UnspecifiedAlgorithm,
                contract: format!(
                    "a function from `{}` to `{}` — the crossing is permitted ({}), the \
                     computation is not declared",
                    conversion.from, conversion.to, conversion.because
                ),
            })
        };
        capabilities.push(PlannedCapability {
            capability: Capability {
                kind: CapabilityKind::Conversion,
                source: conversion_source(conversion),
            },
            disposition,
        });
    }
}

/// The spelling a conversion capability is filed under: both ends, in the specification's own
/// names.
pub fn conversion_source(conversion: &ResolvedConversion) -> String {
    format!("{} -> {}", conversion.from, conversion.to)
}

/// The two handles of a conversion any emitter can write by re-wrapping: both ends are declared
/// newtypes and their representations are the same type. A model-level fact, which is why it is
/// decided here and reused by emitters, rather than decided per target and eventually decided
/// twice.
pub(crate) fn mechanical_conversion<'a>(
    ir: &'a EssIr,
    conversion: &'a ResolvedConversion,
) -> Option<(&'a TypeHandle, &'a TypeHandle)> {
    let (ResolvedTypeRef::Declared { name: from }, ResolvedTypeRef::Declared { name: to }) =
        (&conversion.from, &conversion.to)
    else {
        return None;
    };
    let (ResolvedBody::Newtype { of: from_inner, .. }, ResolvedBody::Newtype { of: to_inner, .. }) =
        (&ir.named_type(from).body, &ir.named_type(to).body)
    else {
        return None;
    };
    (from_inner == to_inner).then_some((from, to))
}

/// Grants are refused, not owed: deriving anything grant-shaped from this plan would be a second
/// grant path (review H8, and the wave's own decision on design §28).
fn plan_actors(ir: &EssIr, capabilities: &mut Vec<PlannedCapability>) {
    for actor in ir.actors.values() {
        let grants = if actor.may.is_empty() {
            "observes only; it may invoke no command".to_owned()
        } else {
            format!(
                "may invoke {}",
                actor
                    .may
                    .iter()
                    .map(|command| format!("`{command}`"))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        };
        capabilities.push(PlannedCapability {
            capability: Capability {
                kind: CapabilityKind::ActorGrants,
                source: actor.name.to_string(),
            },
            disposition: SynthesisDisposition::Refused(SynthesisRefusal {
                reason: RefusalReason::NeedsCallerIdentity,
                stage: RefusalStage::Planning,
                detail: format!(
                    "{grants}; {}, and enforcement belongs to the layer that knows who is calling",
                    RefusalReason::NeedsCallerIdentity.describes()
                ),
            }),
        });
    }
}

/// A binding is three capabilities with three honest dispositions: the transformation is
/// generated exactly where every mapped input is determined, the delivery is generated onto the
/// one component that accepts the command, and an escalation — the one failure policy that
/// declares an event — is owed, because nothing declares how that event's fields are filled.
fn plan_bindings(ir: &EssIr, capabilities: &mut Vec<PlannedCapability>) {
    for binding in ir.bindings.values() {
        capabilities.push(PlannedCapability {
            capability: Capability {
                kind: CapabilityKind::BindingTransformation,
                source: binding.name.to_string(),
            },
            disposition: transformation_disposition(ir, binding),
        });
        capabilities.push(PlannedCapability {
            capability: Capability {
                kind: CapabilityKind::BindingDelivery,
                source: binding.name.to_string(),
            },
            disposition: delivery_disposition(ir, binding),
        });
        if let ResolvedFailure::Escalate { emits } = binding.on_failure() {
            capabilities.push(PlannedCapability {
                capability: Capability {
                    kind: CapabilityKind::BindingEscalation,
                    source: binding.name.to_string(),
                },
                disposition: SynthesisDisposition::Obligation(ImplementationObligation {
                    reason: ObligationReason::UnspecifiedAlgorithm,
                    contract: format!(
                        "the declared `{emits}`, recording that delivering `{}` for `{}` was \
                         given up on — the event is declared; how its fields are filled from the \
                         failed invocation is not",
                        binding.command, binding.name
                    ),
                }),
            });
        }
    }
}

/// The transformation: generated when every input of the invoked command is filled by a
/// determined mapping, owed otherwise — with the undetermined entries named, because "write the
/// transformation" without them sends the implementor back to diff the plan against the source.
fn transformation_disposition(ir: &EssIr, binding: &ResolvedBinding) -> SynthesisDisposition {
    let undetermined = undetermined_mappings(ir, binding);
    if undetermined.is_empty() {
        SynthesisDisposition::Generated
    } else {
        SynthesisDisposition::Obligation(ImplementationObligation {
            reason: ObligationReason::UnspecifiedAlgorithm,
            contract: format!(
                "a transformation from `{}` to `{}` input — {}",
                binding.event,
                binding.command,
                undetermined.join("; ")
            ),
        })
    }
}

/// The delivery: the one transport this scope holds, generated onto the one declared acceptor.
///
/// The transport is derived, not chosen: `at_least_once` is the only delivery guarantee the model
/// declares, and the component surfaces say who invokes whom — so what is generated is an
/// in-process, at-least-once dispatch from the publisher's events to the acceptor's port, and
/// nothing else, because no other transport is declared anywhere to derive from.
fn delivery_disposition(ir: &EssIr, binding: &ResolvedBinding) -> SynthesisDisposition {
    let acceptors = accepting_components(ir, binding);
    if acceptors.len() == 1 {
        return SynthesisDisposition::Generated;
    }
    let detail = if acceptors.is_empty() {
        format!(
            "reacts to `{}` by invoking `{}` ({}, on failure {}); no declared component accepts \
             `{}`, so there is no surface to deliver to",
            binding.event,
            binding.command,
            ess_gen::graph::delivery_word(binding.delivery),
            binding.failure.as_str(),
            binding.command,
        )
    } else {
        format!(
            "reacts to `{}` by invoking `{}` ({}, on failure {}); {} components accept `{}` ({}), \
             and choosing among them is not this synthesis's decision",
            binding.event,
            binding.command,
            ess_gen::graph::delivery_word(binding.delivery),
            binding.failure.as_str(),
            acceptors.len(),
            binding.command,
            acceptors
                .iter()
                .map(|component| format!("`{}`", component.name))
                .collect::<Vec<_>>()
                .join(", "),
        )
    };
    SynthesisDisposition::Refused(SynthesisRefusal {
        reason: RefusalReason::AcceptorUndetermined,
        stage: RefusalStage::Planning,
        detail,
    })
}

/// The components that accept a binding's command, in name order.
///
/// A model-level fact decided here and reused by emitters — the delivery's target is the same
/// answer whether the plan is judging it or a transport is being written from it.
pub(crate) fn accepting_components<'a>(
    ir: &'a EssIr,
    binding: &ResolvedBinding,
) -> Vec<&'a ResolvedComponent> {
    ir.components
        .values()
        .filter(|component| component.accepts.contains(&binding.command))
        .collect()
}

/// How one command input is filled, where the specification fully determines it.
///
/// Model-level, like [`mechanical_conversion`]: the plan uses it to judge a transformation and an
/// emitter uses it to write one, and a decision made twice is a decision made two ways.
pub(crate) enum DeterminedInput<'a> {
    /// The event field's value already has the target type.
    Copy {
        /// The event field.
        field: &'a str,
    },
    /// The event field's value crosses by the declared mechanical conversion.
    Convert {
        /// The event field.
        field: &'a str,
        /// The declared type it is wrapped into.
        to: &'a TypeHandle,
    },
    /// The literal, wrapped outside-in by this chain of newtypes over text.
    Literal {
        /// The value, as the binding wrote it.
        value: &'a str,
        /// The newtypes around it, outermost first; empty when the target is plain text.
        wraps: Vec<&'a TypeHandle>,
    },
    /// The literal names a declared variant of the target enum.
    Variant {
        /// The target enum.
        of: &'a TypeHandle,
        /// The variant, as the binding wrote it.
        value: &'a str,
    },
    /// The input is optional and the binding deliberately leaves it absent — "a decision, not an
    /// omission", in the compiler's own words.
    Omitted,
}

/// What determines one command input under a binding, or `None` where nothing does.
///
/// The determined shapes are the ones the model itself closes. `ess-domain` refuses an unmapped
/// *required* input and a literal into anything but text or an enum's variants, and the compiler
/// verifies event fields and requires a declared crossing where types differ — so what is left
/// undetermined here is exactly one reachable case: a declared crossing that is not mechanical,
/// whose computation is the conversion obligation's to satisfy. The other `None` branches are
/// defensive: unrepresentable after validation, but this function does not get to assume that.
pub(crate) fn determined_input<'a>(
    ir: &'a EssIr,
    binding: &'a ResolvedBinding,
    input: &'a ResolvedField,
) -> Option<DeterminedInput<'a>> {
    let Some(mapping) = binding
        .mapping
        .iter()
        .find(|mapping| mapping.target == input.name)
    else {
        return matches!(input.type_ref, ResolvedTypeRef::Optional { .. })
            .then_some(DeterminedInput::Omitted);
    };
    match &mapping.value {
        ResolvedMappingValue::EventField { field, type_ref } => {
            if mapping.conversion.is_none() {
                return Some(DeterminedInput::Copy { field });
            }
            let (ResolvedTypeRef::Declared { name: from }, ResolvedTypeRef::Declared { name: to }) =
                (type_ref, &mapping.target_type)
            else {
                return None;
            };
            let (
                ResolvedBody::Newtype { of: from_inner, .. },
                ResolvedBody::Newtype { of: to_inner, .. },
            ) = (&ir.named_type(from).body, &ir.named_type(to).body)
            else {
                return None;
            };
            (from_inner == to_inner).then_some(DeterminedInput::Convert { field, to })
        }
        ResolvedMappingValue::Literal { value } => {
            if let ResolvedTypeRef::Declared { name } = &mapping.target_type {
                if matches!(&ir.named_type(name).body, ResolvedBody::Enum { .. }) {
                    return Some(DeterminedInput::Variant { of: name, value });
                }
            }
            let mut wraps = Vec::new();
            literal_reaches_text(ir, &mapping.target_type, &mut wraps)
                .then_some(DeterminedInput::Literal { value, wraps })
        }
    }
}

/// `true` when a target type is text under however many newtype wrappers, collecting the
/// wrappers outermost-first on the way down.
fn literal_reaches_text<'a>(
    ir: &'a EssIr,
    target: &'a ResolvedTypeRef,
    wraps: &mut Vec<&'a TypeHandle>,
) -> bool {
    match target {
        ResolvedTypeRef::Primitive { name } => *name == ess_domain::types::Primitive::String,
        ResolvedTypeRef::Declared { name } => match &ir.named_type(name).body {
            ResolvedBody::Newtype { of, .. } => {
                wraps.push(name);
                literal_reaches_text(ir, of, wraps)
            }
            ResolvedBody::Struct { .. }
            | ResolvedBody::Enum { .. }
            | ResolvedBody::Union { .. } => false,
        },
        ResolvedTypeRef::Optional { .. }
        | ResolvedTypeRef::List { .. }
        | ResolvedTypeRef::Map { .. } => false,
    }
}

/// The undetermined entries of a binding's mapping, each as a phrase naming its own facts.
fn undetermined_mappings(ir: &EssIr, binding: &ResolvedBinding) -> Vec<String> {
    let command = ir.command(&binding.command);
    let mut undetermined = Vec::new();
    for input in &command.input {
        if determined_input(ir, binding, input).is_some() {
            continue;
        }
        let Some(mapping) = binding
            .mapping
            .iter()
            .find(|mapping| mapping.target == input.name)
        else {
            undetermined.push(format!("`{}` has no mapping", input.name));
            continue;
        };
        undetermined.push(match &mapping.value {
            ResolvedMappingValue::EventField { field, .. } => format!(
                "`{}` is filled from event field `{field}` through the declared crossing to `{}`, \
                 whose computation is owed",
                mapping.target, mapping.target_type
            ),
            ResolvedMappingValue::Literal { value } => format!(
                "`{}` is filled from the literal `{value}`, and no reading of it as `{}` is \
                 declared",
                mapping.target, mapping.target_type
            ),
        });
    }
    undetermined
}

/// A component's port surface is fully determined: which commands it accepts, which events it
/// publishes and which views its domains declare are all validated declarations, and the handlers
/// behind the surface are the behaviour obligations the plan already carries.
fn plan_components(ir: &EssIr, capabilities: &mut Vec<PlannedCapability>) {
    for component in ir.components.values() {
        capabilities.push(PlannedCapability {
            capability: Capability {
                kind: CapabilityKind::ComponentPort,
                source: component.name.to_string(),
            },
            disposition: SynthesisDisposition::Generated,
        });
        // The second transport this scope holds, and the second one derived rather than chosen. A
        // binding's `at_least_once` determines an in-process log; a component's `reached_by:
        // network` determines that the surface exists on a wire, and the only wire contract this
        // repository projects for a command surface is the `OpenAPI` document — so the transport is
        // HTTP, serving exactly the routes that document declares. A specification that says
        // nothing about reach contributes no capability here at all, which is why the normative
        // example's plan is the same document it was before this word existed.
        if component.reached_by != Reach::Network {
            continue;
        }
        capabilities.push(PlannedCapability {
            capability: Capability {
                kind: CapabilityKind::ComponentTransport,
                source: component.name.to_string(),
            },
            disposition: SynthesisDisposition::Generated,
        });
    }
}

/// A workload is a runtime requirement, and this synthesis emits no runtime.
fn plan_workloads(ir: &EssIr, capabilities: &mut Vec<PlannedCapability>) {
    for workload in ir.workloads.values() {
        capabilities.push(PlannedCapability {
            capability: Capability {
                kind: CapabilityKind::Workload,
                source: workload.component.to_string(),
            },
            disposition: SynthesisDisposition::Refused(SynthesisRefusal {
                reason: RefusalReason::TopologyDeferred,
                stage: RefusalStage::Planning,
                detail: format!(
                    "requires at least {} replica(s); {}",
                    workload.replicas.min,
                    RefusalReason::TopologyDeferred.describes()
                ),
            }),
        });
    }
}
