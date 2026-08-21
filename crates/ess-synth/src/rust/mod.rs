//! The Rust emitter: the first target behind the plan seam, and everything Rust-shaped in this
//! crate.
//!
//! # The seam
//!
//! The planner knows no language; this module knows exactly one. [`workspace`] consumes a finished
//! [`SynthesisPlan`] and the [`EssIr`] — the same two inputs any later target's emitter would take —
//! and everything downstream of that signature is Rust: file layout (`layout`), identifier
//! derivation (`name`), representation choices, and the rendered source itself. There is no
//! target registry and no abstraction over languages, deliberately: one seam, one target behind
//! it, and a second target is a second sibling module when it exists rather than a framework
//! before it does.
//!
//! # The plan is the gate
//!
//! Nothing is emitted unless the plan marks it generated, and everything the plan marks generated
//! must be emitted: [`workspace`] tracks coverage and refuses to finish on a mismatch. That keeps
//! the plan and the code the same claim in two renderings — the failure it prevents is a plan that
//! promises a type the workspace does not contain, which a reader has no way to notice.
//!
//! # What the generated workspace is
//!
//! A standalone Cargo workspace with its own `[workspace]` root and zero third-party
//! dependencies: one types crate with a module per bounded context, one crate per component
//! holding its port, and one system crate holding the bindings and the transport — the transport
//! itself standard-library only, because the one delivery guarantee the model declares
//! (`at_least_once`, in process) does not need a crate. Zero dependencies is a property of the
//! *gate*, not a style preference: `cargo check` inside the generated tree is a step of
//! `task check`, and a step that resolves crates is a step that reaches the network
//! (AGENTS.md § Dependencies).

mod entity;
pub(crate) mod http;
pub(crate) mod items;
pub(crate) mod json;
pub(crate) mod layout;
pub(crate) mod name;
mod obligation;
pub(crate) mod port;
pub(crate) mod system;
pub(crate) mod wire;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use ess_compiler::ir::{EssIr, EventHandle};
use ess_domain::name::QualifiedName;
use ess_gen::{Artifact, Provenance};

use crate::plan::{Capability, CapabilityKind, SynthesisPlan, REGENERATE};

use self::layout::Layout;

/// The Rust edition every generated crate declares.
///
/// Pinned here rather than inherited from anything, because the generated workspace inherits
/// nothing: it must build outside this repository, from exactly the bytes committed.
const EDITION: &str = "2021";

/// Everything the generated module renderers need to agree on, carried once.
pub(crate) struct Emit<'a> {
    /// The resolved model.
    pub ir: &'a EssIr,
    /// Where everything lands and what it is called.
    pub layout: &'a Layout,
    /// The bounded context whose module is being rendered.
    pub domain: &'a QualifiedName,
}

impl Emit<'_> {
    /// A resolved type reference as Rust, from inside this module.
    pub fn rust_type(&self, type_ref: &ess_compiler::ir::ResolvedTypeRef) -> String {
        self.layout.rust_type(type_ref, self.domain)
    }

    /// A declared type as Rust, from inside this module.
    pub fn reference(&self, declared: &ess_compiler::ir::TypeHandle) -> String {
        self.reference_name(declared.name())
    }

    /// Any declaration as Rust, from inside this module — events and errors reach here, whose
    /// handles are not `TypeHandle`s.
    pub fn reference_name(&self, declared: &QualifiedName) -> String {
        self.layout.reference(declared, self.domain)
    }
}

/// Emits the generated workspace a plan determines: manifests, the primitives module, and one
/// module per bounded context.
///
/// # Panics
///
/// If what was emitted is not exactly what the plan marks generated — a defect in this crate, and
/// the one lie the plan document must never be allowed to tell.
pub fn workspace(ir: &EssIr, plan: &SynthesisPlan) -> Vec<Artifact> {
    let layout = Layout::of(ir);
    let provenance = &plan.provenance;

    let mut covered: BTreeSet<Capability> = BTreeSet::new();
    let mut stubbed: BTreeSet<Capability> = BTreeSet::new();
    let obligation_module =
        obligation::obligation_module(ir, plan, &layout, provenance, &mut stubbed);
    let mut artifacts = vec![
        workspace_manifest(ir, &layout, provenance),
        crate_manifest(ir, &layout, provenance),
        lib_module(ir, &layout, provenance, obligation_module.is_some()),
        primitives_module(&layout, provenance),
    ];
    artifacts.extend(obligation_module);
    let domains: Vec<QualifiedName> = layout.modules().map(|(domain, _)| domain.clone()).collect();
    for domain in &domains {
        artifacts.push(domain_module(
            ir,
            plan,
            &layout,
            domain,
            &mut covered,
            &mut stubbed,
        ));
    }
    for component in ir.components.values() {
        artifacts.extend(port::component_crate(
            ir,
            plan,
            &layout,
            component,
            &mut covered,
        ));
    }
    artifacts.extend(system::system_crate(
        ir,
        plan,
        &layout,
        &mut covered,
        &mut stubbed,
    ));
    artifacts.extend(http::server_crate(ir, plan, &layout, &mut covered));

    let planned: BTreeSet<Capability> = plan.generated().cloned().collect();
    assert_eq!(
        covered, planned,
        "the Rust emitter emitted a different set of capabilities than the plan marks generated; \
         that is a defect in ess-synth, and shipping it would make PLAN.md a lie about the workspace"
    );
    let owed: BTreeSet<Capability> = plan
        .obligations()
        .map(|(capability, _)| capability.clone())
        .collect();
    assert_eq!(
        stubbed, owed,
        "the Rust emitter's stubs are not exactly the plan's obligations; that is a defect in \
         ess-synth, and shipping it would break the promise that every owed capability is visible \
         twice — in the plan, and as a typed refusal in the workspace"
    );
    artifacts
}

/// One enum variant name per event of a set, collision-free by rule rather than by luck.
///
/// The candidate is the event's own type name, which is domain-relative. When two domains of one
/// set declare same-named events, **every** variant switches to the event's full name (minus the
/// system prefix every event carries), pascal-joined — all of them, not just the colliding pair,
/// on the same reasoning as the module-identifier rule: adding one event must not silently rename
/// an unrelated variant another crate matches on.
pub(crate) fn event_variants<'a>(
    ir: &EssIr,
    layout: &Layout,
    events: &BTreeSet<&'a EventHandle>,
) -> BTreeMap<&'a EventHandle, String> {
    let mut candidates: BTreeMap<&EventHandle, String> = events
        .iter()
        .map(|event| (*event, layout.type_name(event.name())))
        .collect();
    let distinct: BTreeSet<&String> = candidates.values().collect();
    if distinct.len() != candidates.len() {
        let prefix = ir.system.segments().len();
        candidates = events
            .iter()
            .map(|event| {
                let segments = event.name().segments();
                let full: String = segments
                    .get(prefix..)
                    .unwrap_or(segments)
                    .iter()
                    .map(|segment| name::pascal(segment))
                    .collect();
                (*event, full)
            })
            .collect();
    }
    candidates
}

/// The `[workspace]` manifest at the generated root.
///
/// Its own workspace root, always: it is what stops `cargo` inside the generated tree from walking
/// up into this repository's workspace, and it is what a consumer who vendors the directory gets.
fn workspace_manifest(ir: &EssIr, layout: &Layout, provenance: &Provenance) -> Artifact {
    let mut members = vec![layout.package().to_owned()];
    for component in ir.components.keys() {
        members.push(layout.component_package(component).to_owned());
    }
    if !ir.components.is_empty() || !ir.bindings.is_empty() {
        members.push(layout.system_package().to_owned());
    }
    if !http::served(ir).is_empty() {
        members.push(layout.server_package().to_owned());
    }
    members.sort();
    let mut out = provenance.commented_for("#", REGENERATE);
    let _ = write!(
        out,
        "\n[workspace]\nresolver = \"2\"\nmembers = [{}]\n",
        members
            .iter()
            .map(|member| format!("\"crates/{member}\""))
            .collect::<Vec<_>>()
            .join(", ")
    );
    Artifact::new("Cargo.toml", out)
}

/// The types crate's manifest. `[dependencies]` is present and empty on purpose: zero dependencies
/// is a stated property here, not an accident a later edit can quietly end without a diff line.
fn crate_manifest(ir: &EssIr, layout: &Layout, provenance: &Provenance) -> Artifact {
    let mut out = provenance.commented_for("#", REGENERATE);
    let _ = write!(
        out,
        "\n[package]\nname = \"{}\"\ndescription = \"Semantic types synthesised from the `{}` \
         specification, {}.\"\nversion = \"{}.0.0\"\nedition = \"{EDITION}\"\n\n[dependencies]\n",
        layout.package(),
        ir.system,
        ir.version,
        ir.version.get()
    );
    Artifact::new(format!("crates/{}/Cargo.toml", layout.package()), out)
}

/// The crate root: docs, the two lints the generated code holds itself to, and the module list.
fn lib_module(
    ir: &EssIr,
    layout: &Layout,
    provenance: &Provenance,
    with_obligation_module: bool,
) -> Artifact {
    let mut out = provenance.commented_for("//", REGENERATE);
    out.push('\n');
    let _ = writeln!(
        out,
        "//! Semantic types synthesised from the `{}` specification, {}.",
        ir.system, ir.version
    );
    if let Some(summary) = &ir.summary {
        out.push_str("//!\n");
        let _ = writeln!(out, "//! {}", summary.trim());
    }
    out.push_str(
        "//!\n//! Generated, not written: the specification is the source of truth, and the door \
         to changing\n//! anything here is `",
    );
    out.push_str(REGENERATE);
    out.push_str(
        "`. What is deliberately absent — behaviour, queries,\n//! escalations — is listed with \
         reasons in the `PLAN.md` beside this workspace, and every entry\n//! there is owed \
         through a typed seam in an `obligations` module here.\n\n// `deny`, not the source \
         workspace's lint \
         set: this crate must hold on its own, and an undocumented\n// public item here is an \
         emitter defect worth failing the gate over.\n#![forbid(unsafe_code)]\n#![deny(missing_docs)]\n\n",
    );
    let mut modules: Vec<String> = layout
        .modules()
        .map(|(_, module)| module.to_owned())
        .collect();
    modules.push("primitives".to_owned());
    if with_obligation_module {
        modules.push("obligation".to_owned());
    }
    modules.sort();
    for module in modules {
        let _ = writeln!(out, "pub mod {module};");
    }
    Artifact::new(format!("crates/{}/src/lib.rs", layout.package()), out)
}

/// The `primitives` module: the representation each specification primitive gets in this target.
///
/// The content is fixed per emitter version rather than derived from the specification, because it
/// is a fact about the *target*: the same eight primitives get the same eight spellings whatever
/// the system. The wrappers carry their value in the rendering the published wire contracts fix —
/// the JSON Schema projection writes `Decimal` as a decimal string, `Timestamp` as `date-time`,
/// `Duration` as an ISO 8601 duration and `Uuid` as a UUID string — so two projections of one
/// model cannot disagree about what a value looks like.
fn primitives_module(layout: &Layout, provenance: &Provenance) -> Artifact {
    let mut out = provenance.commented_for("//", REGENERATE);
    out.push_str(PRIMITIVES);
    Artifact::new(
        format!("crates/{}/src/primitives.rs", layout.package()),
        out,
    )
}

/// The body of the generated `primitives` module.
const PRIMITIVES: &str = r"
//! How the specification's primitives are spelled in this workspace.
//!
//! Four map onto types that already mean exactly the same thing: `String` stays `String`,
//! `Boolean` is `bool`, `Integer` is `i64`, `Bytes` is `Vec<u8>`. The four below have no `std`
//! equivalent, and no dependency is taken for them — this workspace builds from exactly its
//! committed bytes. Each is a transparent wrapper over its wire rendering, distinct from `String`
//! and from each other for the same reason the specification's own newtypes are distinct from
//! their representations: a value's meaning is not its shape.

/// An exact decimal, carried as its wire rendering — a decimal string such as `10.50`.
///
/// Never a float: money does not round the way a float does. Equality and order are over the
/// rendering, so `1.5` and `1.50` are different values here; arithmetic is deliberately absent,
/// because what a decimal *does* is behaviour, and behaviour is not synthesised.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Decimal(pub String);

/// An instant, carried as its wire rendering — RFC 3339, such as `2026-01-01T00:00:00Z`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Timestamp(pub String);

/// A length of time, carried as its wire rendering — an ISO 8601 duration such as `P30D`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Duration(pub String);

/// A UUID, carried as its canonical textual rendering.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Uuid(pub String);
";

/// One bounded context's module: every declaration the plan marks generated, in a fixed order —
/// types, entities, commands, events, errors, views, then the conversions that land here.
fn domain_module(
    ir: &EssIr,
    plan: &SynthesisPlan,
    layout: &Layout,
    domain: &QualifiedName,
    covered: &mut BTreeSet<Capability>,
    stubbed: &mut BTreeSet<Capability>,
) -> Artifact {
    let emit = Emit { ir, layout, domain };
    let resolved = ir
        .domains
        .get(domain)
        .expect("the layout only knows domains the IR declares");

    let mut out = plan.provenance.commented_for("//", REGENERATE);
    out.push('\n');
    let _ = writeln!(
        out,
        "//! {} — `{domain}`.",
        resolved.naming.display_or(domain)
    );
    if let Some(summary) = &resolved.naming.summary {
        out.push_str("//!\n");
        let _ = writeln!(out, "//! {}", summary.trim());
    }
    out.push_str(
        "//!\n//! Everything this bounded context declares that the synthesis plan marks \
         generated.\n",
    );

    render_declarations(&mut out, &emit, plan, covered);
    render_conversions(&mut out, &emit, plan, covered);
    obligation::domain_obligations(&mut out, &emit, plan, stubbed);

    Artifact::new(layout.module_path(domain), out)
}

/// The declarations of one domain, each behind the plan's gate.
fn render_declarations(
    out: &mut String,
    emit: &Emit<'_>,
    plan: &SynthesisPlan,
    covered: &mut BTreeSet<Capability>,
) {
    for declared in emit.ir.types.values() {
        if owned(emit, &declared.name)
            && cover(
                plan,
                covered,
                CapabilityKind::DomainType,
                &declared.name.to_string(),
            )
        {
            items::named_type(out, emit, declared);
        }
    }
    for spec in emit.ir.entities.values() {
        if owned(emit, &spec.name)
            && cover(
                plan,
                covered,
                CapabilityKind::EntityLifecycle,
                &spec.name.to_string(),
            )
        {
            entity::lifecycle(out, emit, spec);
        }
    }
    for command in emit.ir.commands.values() {
        if owned(emit, &command.name)
            && cover(
                plan,
                covered,
                CapabilityKind::CommandContract,
                &command.name.to_string(),
            )
        {
            items::command_contract(out, emit, command);
        }
    }
    for event in emit.ir.events.values() {
        if owned(emit, &event.name)
            && cover(
                plan,
                covered,
                CapabilityKind::EventType,
                &event.name.to_string(),
            )
        {
            items::event(out, emit, event);
        }
    }
    for error in emit.ir.errors.values() {
        if owned(emit, &error.name)
            && cover(
                plan,
                covered,
                CapabilityKind::ErrorType,
                &error.name.to_string(),
            )
        {
            items::error(out, emit, error);
        }
    }
    for view in emit.ir.views.values() {
        if owned(emit, &view.name)
            && cover(
                plan,
                covered,
                CapabilityKind::ViewType,
                &view.name.to_string(),
            )
        {
            items::view(out, emit, view);
        }
    }
}

/// The mechanical conversions whose destination type this domain owns.
///
/// Filed with the destination because that is whose meaning is being produced — and because an
/// `impl From` has to live beside one of its two types, which makes "where" a decision to take
/// once, here, rather than per conversion.
fn render_conversions(
    out: &mut String,
    emit: &Emit<'_>,
    plan: &SynthesisPlan,
    covered: &mut BTreeSet<Capability>,
) {
    for conversion in &emit.ir.conversions {
        let Some((from, to)) = crate::plan::mechanical_conversion(emit.ir, conversion) else {
            continue;
        };
        if owned(emit, to.name())
            && cover(
                plan,
                covered,
                CapabilityKind::Conversion,
                &crate::plan::conversion_source(conversion),
            )
        {
            items::conversion(out, emit, conversion, from, to);
        }
    }
}

/// `true` when this module's domain owns the declaration.
fn owned(emit: &Emit<'_>, declared: &QualifiedName) -> bool {
    emit.layout.owner(declared) == emit.domain
}

/// The plan's gate: emit only what it marks generated, and record what was emitted so
/// [`workspace`] can hold the emitter to the whole list.
fn cover(
    plan: &SynthesisPlan,
    covered: &mut BTreeSet<Capability>,
    kind: CapabilityKind,
    source: &str,
) -> bool {
    if plan.is_generated(kind, source) {
        covered.insert(Capability {
            kind,
            source: source.to_owned(),
        });
        true
    } else {
        false
    }
}
