//! The web emitter: the third target behind the plan seam, and the only one a person can click.
//!
//! # Why a third target, and why this one
//!
//! Rust proved the plan could be emitted; Go proved the plan was language-neutral. Neither
//! answered the question an implementor actually asks — *what does this system do when I use it* —
//! because the answer to that is a running system with a surface, and until now the only surface
//! was a test suite. This target emits one: a WebAssembly module holding the synthesised system,
//! and a page whose command list, input forms, event log and state panel are built from the model
//! rather than typed into HTML.
//!
//! # It is a front end, not a fourth rendering of the model
//!
//! The Rust target already emits the types, the typestate lifecycles, the ports and the transport.
//! Emitting them again in a third shape would be a third thing to keep in step. So this target
//! emits the **crossing**: JSON encoders and decoders for what enters and leaves, a catalogue of
//! the model for the page to render, and the exports a browser can call. Its manifest names the
//! Rust target's crates by path — the one place a generated tree here is not standalone, stated as
//! a weakening rather than left to be discovered.
//!
//! # It never chooses a realization
//!
//! Every command behaviour is an obligation, and this bridge fills none of them. What it emits is
//! [`install`](self)'s seam: a `Bound` trait with a blanket implementation over the generated
//! `System`, so a host links its own realization and hands the assembled system over. With nothing
//! installed the page runs against the generated stubs and every command answers with the typed
//! refusal naming what is owed — which is the honest empty state, and the one a reader learns the
//! plan from. Gap register D-2 says the machinery does not choose; a page is machinery.

mod bridge;
mod catalog;
mod layout;
mod page;
mod refusal;
mod runtime;
mod wire;

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use ess_compiler::ir::EssIr;
use ess_domain::component::ComponentName;
use ess_domain::name::QualifiedName;
use ess_gen::{Artifact, Provenance};

use crate::plan::{conversion_source, Capability, CapabilityKind, SynthesisPlan, REGENERATE};
use crate::rust::layout::Layout as RustLayout;
use crate::{TargetRefusal, TargetReport, TargetWeakening};

use self::layout::Layout;
use self::refusal::TargetRefusals;

/// The name this target reports itself under.
pub const TARGET: &str = "web";

/// The Rust edition the emitted bridge crate declares.
///
/// Pinned for the reason the Rust target pins its own: the tree must build from exactly the bytes
/// committed, and a directive naming whatever edition generated it dates the tree for no reason.
const EDITION: &str = "2021";

/// The file the page and the bridge crate both read the model from.
pub const CATALOG: &str = "catalog.json";

/// The page a person opens.
pub const PAGE: &str = "index.html";

/// The glue between the page and the module's linear memory.
pub const GLUE: &str = "bridge.js";

/// Every export the emitted module offers, in the order the page uses them.
///
/// A constant rather than a fact spread over the emitter, because two artifacts have to agree
/// about it — the Rust that defines the symbols and the JavaScript that calls them — and
/// `cargo xtask synth` checks the compiled module's export table against this same list. A page
/// calling an export the module does not have is the browser's version of a dangling reference,
/// and nothing in HTML would refuse it.
pub const EXPORTS: &[&str] = &["ess_input_reserve", "ess_dispatch", "ess_output_len"];

/// The optional export a host that links a realization provides.
///
/// Not in [`EXPORTS`] because the emitted module does not have it: the page calls it *if it is
/// there*, which is how a realization reaches a page neither of them was written against. With
/// nothing installed the system runs on the generated stubs and says so.
pub const REALIZE: &str = "ess_realize";

/// What one web emission produced.
pub struct Emission {
    /// Every file, in path order.
    pub artifacts: Vec<Artifact>,
    /// What this target could not carry across from the plan.
    pub report: TargetReport,
}

/// Everything the emitted files need to agree on, carried once.
pub(crate) struct Bridge<'a> {
    /// The resolved model.
    pub ir: &'a EssIr,
    /// The plan, which is the gate on everything below.
    pub plan: &'a SynthesisPlan,
    /// Where everything lands and what it is called.
    pub layout: &'a Layout,
    /// What this target refuses to present.
    pub refusals: &'a TargetRefusals,
    /// The types crate, as a path spells it.
    pub types: String,
    /// The system crate, or `None` where the specification declares no interaction layer.
    pub system: Option<String>,
    /// The one component that accepts each dispatchable command.
    pub acceptors: BTreeMap<QualifiedName, ComponentName>,
    /// The component serving each view.
    pub view_components: BTreeMap<QualifiedName, ComponentName>,
    /// What has been presented so far, so [`workspace`] can hold the emitter to the whole plan.
    ///
    /// Collected while rendering rather than declared up front, exactly as the Go emitter collects
    /// its imports: a hand-maintained list is a list that is wrong the first time a renderer stops
    /// needing something, and here being wrong means `PLAN.md` promising a capability the page
    /// does not show.
    covered: RefCell<BTreeSet<Capability>>,
}

impl<'a> Bridge<'a> {
    /// A bridge over one planned specification.
    pub fn new(
        ir: &'a EssIr,
        plan: &'a SynthesisPlan,
        layout: &'a Layout,
        refusals: &'a TargetRefusals,
    ) -> Self {
        let system = (!ir.components.is_empty() || !ir.bindings.is_empty())
            .then(|| RustLayout::crate_ident(layout.rust().system_package()));
        Self {
            ir,
            plan,
            layout,
            refusals,
            types: RustLayout::crate_ident(layout.rust().package()),
            system,
            acceptors: refusal::acceptors(ir),
            view_components: catalog::view_components(ir),
            covered: RefCell::new(BTreeSet::new()),
        }
    }

    /// Records that this target presents one capability, and answers whether it does.
    ///
    /// The plan is the gate — nothing is presented that it does not mark generated — and this
    /// target's own refusals are the second gate, so a refused capability is never recorded as
    /// covered and never renders a form.
    pub fn present(&self, kind: CapabilityKind, source: &str) -> bool {
        if !self.plan.is_generated(kind, source) || self.refusals.refuses(kind, source) {
            return false;
        }
        self.covered.borrow_mut().insert(Capability {
            kind,
            source: source.to_owned(),
        });
        true
    }

    /// `true` when the page renders values of this declared type.
    pub fn presents_type(&self, declared: &QualifiedName) -> bool {
        self.present(CapabilityKind::DomainType, &declared.to_string())
    }

    /// `true` when the page renders this event on the log.
    pub fn presents_event(&self, declared: &QualifiedName) -> bool {
        self.present(CapabilityKind::EventType, &declared.to_string())
    }

    /// `true` when the page renders this declared error as a refusal reason.
    pub fn presents_error(&self, declared: &QualifiedName) -> bool {
        self.present(CapabilityKind::ErrorType, &declared.to_string())
    }

    /// `true` when the page renders this view's rows.
    pub fn presents_view(&self, declared: &QualifiedName) -> bool {
        self.present(CapabilityKind::ViewType, &declared.to_string())
    }

    /// `true` when the page can send this command.
    pub fn presents_command(&self, declared: &QualifiedName) -> bool {
        self.present(CapabilityKind::CommandContract, &declared.to_string())
    }

    /// A declaration's path from inside the bridge crate.
    pub fn path(&self, declared: &QualifiedName) -> String {
        wire::path(self.layout.rust(), &self.types, declared)
    }

    /// `true` when the system crate carries obligations of its own — a transformation nobody
    /// determined, or an escalation to build.
    ///
    /// The same question the Rust target asks to decide whether `System` takes a third type
    /// parameter, asked here because this bridge has to spell the same type.
    pub fn has_system_obligations(&self) -> bool {
        self.ir.bindings.values().any(|binding| {
            let source = binding.name.to_string();
            if !self
                .plan
                .is_generated(CapabilityKind::BindingDelivery, &source)
            {
                return false;
            }
            !self
                .plan
                .is_generated(CapabilityKind::BindingTransformation, &source)
                || matches!(
                    binding.on_failure(),
                    ess_compiler::ir::ResolvedFailure::Escalate { .. }
                )
        })
    }

    /// The system crate, which every dispatching artifact needs.
    ///
    /// # Panics
    ///
    /// If the specification declares no interaction layer — which every caller checks first, so
    /// reaching it is a defect in this module rather than a fact about a specification.
    pub fn system(&self) -> &str {
        self.system.as_deref().expect(
            "the emitter asked for the system crate of a specification that declares no component \
             and no binding; that is a defect in ess-synth",
        )
    }
}

/// Emits the browser realization a plan determines, and reports what this target could not carry.
///
/// # Panics
///
/// If what was presented is not exactly what the plan marks generated *minus* what this target
/// refused — a defect in this crate, and the one lie neither the plan nor the target report may be
/// allowed to tell.
pub fn workspace(ir: &EssIr, plan: &SynthesisPlan) -> Emission {
    let layout = Layout::of(ir);
    let acceptors = refusal::acceptors(ir);
    let refusals = TargetRefusals::of(ir, plan, &acceptors);
    let bridge = Bridge::new(ir, plan, &layout, &refusals);
    let provenance = &plan.provenance;

    // Presented here rather than inside a renderer, because these three are surfaces of the model
    // that the page shows as tables rather than as code: a binding row, a component's grouping,
    // and the crossing a transformation reads a field through.
    for component in ir.components.keys() {
        bridge.present(CapabilityKind::ComponentPort, &component.to_string());
    }
    for binding in ir.bindings.keys() {
        bridge.present(CapabilityKind::BindingTransformation, &binding.to_string());
        bridge.present(CapabilityKind::BindingDelivery, &binding.to_string());
    }
    for conversion in &ir.conversions {
        bridge.present(CapabilityKind::Conversion, &conversion_source(conversion));
    }
    for entity in ir.entities.keys() {
        bridge.present(CapabilityKind::EntityLifecycle, &entity.to_string());
    }

    let catalog = catalog::document(&bridge);
    let wire = wire::module(&bridge);
    let library = bridge::module(&bridge);

    let artifacts = vec![
        workspace_manifest(&layout, provenance),
        crate_manifest(ir, &layout, provenance),
        Artifact::new(layout.source("lib"), library),
        Artifact::new(layout.source("json"), json_module(provenance)),
        Artifact::new(
            layout.source("wire"),
            format!("{}\n{wire}", provenance.commented_for("//", &regenerate())),
        ),
        Artifact::new(layout.source("catalog"), catalog_module(provenance)),
        Artifact::new(CATALOG, catalog),
        Artifact::new(PAGE, page::html(&bridge)),
        Artifact::new(GLUE, page::glue(&bridge)),
        Artifact::new("README.md", readme(&bridge)),
    ];

    let refused: BTreeSet<Capability> = refusals
        .iter()
        .map(|(capability, _)| capability.clone())
        .collect();
    let planned: BTreeSet<Capability> = plan
        .generated()
        .filter(|capability| !refused.contains(capability))
        .cloned()
        .collect();
    assert_eq!(
        bridge.covered.borrow().clone(),
        planned,
        "the web emitter presented a different set of capabilities than the plan marks generated \
         and this target did not refuse; that is a defect in ess-synth, and shipping it would \
         make PLAN.md a lie about the page"
    );

    Emission {
        artifacts,
        report: TargetReport {
            provenance: provenance.clone(),
            target: TARGET,
            weakenings: weakenings(),
            refusals: refusals
                .iter()
                .map(|(capability, detail)| TargetRefusal {
                    capability: capability.clone(),
                    detail: detail.clone(),
                })
                .collect(),
        },
    }
}

/// The command that rewrites this target's tree.
fn regenerate() -> String {
    format!("{REGENERATE} --target {TARGET}")
}

/// What this target emits with a weaker guarantee than the first target's, stated once per rule.
///
/// Six rules, and none of them is about a language: this target's limits are the browser's — a
/// boundary that carries JSON and no types, a page that can only observe what the model publishes,
/// a number format narrower than the model's, and an export mechanism the compiler classes as
/// unsafe.
fn weakenings() -> Vec<TargetWeakening> {
    vec![
        TargetWeakening {
            guarantee: "the generated crate forbids `unsafe`, so the compiler closes the question \
                        rather than a reader checking it"
                .to_owned(),
            instead: "a WebAssembly export is a `#[no_mangle]` item, and rustc's own \
                      `unsafe_code` lint flags one — so the bridge crate cannot declare \
                      `#![forbid(unsafe_code)]` and declares `#![deny(missing_docs)]` alone. It \
                      contains no `unsafe` block, no `unsafe fn` and no raw-pointer dereference; \
                      what is lost is the compiler closing the question, not the property"
                .to_owned(),
            affects: vec![
                CapabilityKind::ComponentPort,
                CapabilityKind::CommandContract,
            ],
        },
        TargetWeakening {
            guarantee: "a move the lifecycle does not declare does not compile".to_owned(),
            instead: "the page speaks JSON, and JSON carries no type parameter: any declared \
                      command can be sent from any state, and an illegal move comes back as the \
                      declared refusal the behaviour answers with — at run time, from the system, \
                      rather than as a build that failed. The typed lifecycle still holds inside \
                      the system this bridge drives; it simply does not reach across the boundary"
                .to_owned(),
            affects: vec![
                CapabilityKind::EntityLifecycle,
                CapabilityKind::CommandContract,
            ],
        },
        TargetWeakening {
            guarantee: "the current state of every instance is observable".to_owned(),
            instead: "the synthesised system holds no entity store — where instances live is an \
                      obligation — so the page shows each declared view's rows beside the \
                      entity's declared lifecycle, and shows a per-instance state only where a \
                      view projects one. Deriving a state from the event log would be behaviour, \
                      and behaviour is not synthesised"
                .to_owned(),
            affects: vec![CapabilityKind::EntityLifecycle, CapabilityKind::ViewType],
        },
        TargetWeakening {
            guarantee: "an `Integer` is sixty-four bits wide, end to end".to_owned(),
            instead: "the bridge writes it as a JSON number, which is what the published wire \
                      contract fixes, and a browser reads every JSON number as a double — so a \
                      magnitude past 2^53 is rounded by the page. The bridge itself never \
                      truncates: a fraction, an exponent or an out-of-range magnitude arriving \
                      from the page is refused with the path it was found at"
                .to_owned(),
            affects: vec![
                CapabilityKind::DomainType,
                CapabilityKind::CommandContract,
                CapabilityKind::EventType,
                CapabilityKind::ErrorType,
                CapabilityKind::ViewType,
            ],
        },
        TargetWeakening {
            guarantee: "a generated tree builds from exactly its committed bytes, outside this \
                        repository"
                .to_owned(),
            instead: "this tree is a front end over the Rust target's crates, so its manifest \
                      names them by relative path — `../../rust/<system>/` from this tree's root, \
                      which is the layout `cargo xtask synth` commits. Copy both trees or \
                      neither; a browser realization on its own has no system to drive"
                .to_owned(),
            affects: vec![
                CapabilityKind::DomainType,
                CapabilityKind::EntityLifecycle,
                CapabilityKind::CommandContract,
                CapabilityKind::EventType,
                CapabilityKind::ErrorType,
                CapabilityKind::ViewType,
                CapabilityKind::Conversion,
                CapabilityKind::BindingTransformation,
                CapabilityKind::BindingDelivery,
                CapabilityKind::ComponentPort,
            ],
        },
        TargetWeakening {
            guarantee: "a binding whose failure policy is `retry` is redelivered on the schedule \
                        the transport provides"
                .to_owned(),
            instead: "the page is the transport's caller, and nothing here advances a clock: \
                      redelivery is a request a person makes, one occurrence at a time, and the \
                      duplicate `at_least_once` permits is something to watch rather than \
                      something to wait for. When to try again is a deployment decision the \
                      specification does not take, and this target does not take it either"
                .to_owned(),
            affects: vec![CapabilityKind::BindingDelivery],
        },
    ]
}

/// The workspace manifest at the root of the emitted tree.
fn workspace_manifest(layout: &Layout, provenance: &Provenance) -> Artifact {
    let mut out = provenance.commented_for("#", &regenerate());
    let _ = write!(
        out,
        "\n[workspace]\nresolver = \"2\"\nmembers = [\"crates/{}\"]\n\n# The module is downloaded \
         by a browser, so its size is a property of the product rather than of the build. Nothing \
         else is set: a profile is where a generated tree quietly stops being reproducible.\n\
         [profile.release]\nopt-level = \"s\"\nlto = true\n",
        layout.package()
    );
    Artifact::new("Cargo.toml", out)
}

/// The bridge crate's manifest: a `cdylib` for the browser, an `rlib` for a host that links a
/// realization into it.
fn crate_manifest(ir: &EssIr, layout: &Layout, provenance: &Provenance) -> Artifact {
    let system = ir.system.segments().join("-");
    let mut out = provenance.commented_for("#", &regenerate());
    let _ = write!(
        out,
        "\n[package]\nname = \"{}\"\ndescription = \"The `{}` system, {}, behind a WebAssembly \
         boundary: JSON in, JSON out, and the catalogue a page renders itself \
         from.\"\nversion = \"{}.0.0\"\nedition = \"{EDITION}\"\n\n# Both, and both are used. \
         `cdylib` is what a browser instantiates. `rlib` is what a host crate links against to \
         install its own realization — the exports below travel into that host's own `cdylib`, so \
         a realized module and this one answer the same page.\n[lib]\ncrate-type = [\"cdylib\", \
         \"rlib\"]\n\n# Path dependencies into the Rust target's committed tree, and no third \
         party at all: `cargo build` inside this tree is a gate step, and a step that resolves a \
         crate is a step that reaches the network.\n[dependencies]\n",
        layout.package(),
        ir.system,
        ir.version,
        ir.version.get(),
    );
    let mut packages = vec![layout.rust().package().to_owned()];
    if !ir.components.is_empty() || !ir.bindings.is_empty() {
        packages.push(layout.rust().system_package().to_owned());
    }
    for component in ir.components.keys() {
        packages.push(layout.rust().component_package(component).to_owned());
    }
    packages.sort();
    for package in packages {
        let _ = writeln!(
            out,
            "{package} = {{ path = \"{}\" }}",
            Layout::rust_crate_path(&package, &system)
        );
    }
    Artifact::new(layout.manifest(), out)
}

/// The fixed JSON module, stamped with the provenance of the tree it is committed in.
fn json_module(provenance: &Provenance) -> String {
    format!(
        "{}{}",
        provenance.commented_for("//", &regenerate()),
        runtime::JSON
    )
}

/// The catalogue module: one `include_str!` of the document beside the tree root.
fn catalog_module(provenance: &Provenance) -> String {
    format!(
        "{}\n//! The model this page renders itself from.\n//!\n//! Pulled in from `{CATALOG}` \
         beside the tree root rather than written here, so a reviewer reads the\n//! catalogue as \
         JSON and the module carries it without a second copy. The page asks the running\n//! \
         system for it — a page opened from `file://` can read its own WebAssembly module and \
         cannot\n//! always read its neighbours.\n\n/// The model, as canonical \
         JSON.\npub const CATALOG: &str = include_str!(\"../../../{CATALOG}\");\n",
        provenance.commented_for("//", &regenerate())
    )
}

/// The tree's own README: what it is, how to build it, and how to open it.
fn readme(bridge: &Bridge<'_>) -> String {
    let regenerate = regenerate();
    let package = bridge.layout.package();
    let module = RustLayout::crate_ident(package);
    let system = bridge.ir.system.segments().join("-");
    let mut out = bridge.plan.provenance.html_comment_for(&regenerate);
    let _ = write!(
        out,
        "# {} in a browser\n\n**Do not edit these files.** They are synthesised from \
         `examples/{system}/` by `cargo xtask synth`, and CI fails if they differ from what the \
         specification determines, if the module stops building for \
         `wasm32-unknown-unknown`, or if `{PAGE}` calls an export the module does not have. \
         Regenerate with `{regenerate}`.\n\nWhat is here:\n\n| file | what it is |\n| --- | --- \
         |\n| `{PAGE}` | the page. Every command, field, event, view and state on it is built \
         from `{CATALOG}`; none of it is typed into the HTML |\n| `{GLUE}` | the glue: reserve a \
         buffer, write the request, call the module, read the response |\n| `{CATALOG}` | the \
         model, as the page reads it — and the same bytes the module carries |\n| `crates/{package}/` \
         | the bridge crate: JSON in, JSON out, and the exports a browser calls |\n| `PLAN.md`, \
         `plan.json` | the plan, byte-identical to every other target's |\n| `TARGET.md`, \
         `target.json` | what a browser could not carry across the plan |\n\n## Building it\n\n\
         ```console\n$ rustup target add wasm32-unknown-unknown\n$ cargo build --release \
         --target wasm32-unknown-unknown\n```\n\nThat produces \
         `target/wasm32-unknown-unknown/release/{module}.wasm`. Every command will answer with \
         the typed refusal naming an unmet obligation, because this tree implements none of \
         them — that is the honest empty state, and the page shows the plan's own contract beside \
         each one.\n\n## Running it against a realization\n\nA host crate that depends on this \
         one as an `rlib`, links an implementation of every obligation, and exports \
         `{REALIZE}` hands the assembled system to `install`. The exports below travel into \
         that host's `cdylib`, so the same page drives it, and the page calls `{REALIZE}` if it \
         is there. `examples/{system}-web/` in this repository is that host.\n\nThe page looks \
         for its module in three places, in order: the release build beside this file, the debug \
         build, and `{module}.wasm` in this directory. That last one is how a *realized* module \
         is opened — copy it in under that name:\n\n```console\n$ (cd \
         ../../../examples/{system}-web && cargo build --release --target \
         wasm32-unknown-unknown)\n$ cp \
         ../../../examples/{system}-web/target/wasm32-unknown-unknown/release/*.wasm \
         ./{module}.wasm\n```\n\nThen serve this directory and open `{PAGE}` — a browser will \
         not instantiate WebAssembly from a `file://` URL:\n\n```console\n$ python3 -m \
         http.server\n$ open http://localhost:8000/{PAGE}\n```\n\n## The exports\n\n| export \
         | what it does |\n| --- | --- |\n",
        bridge.ir.naming.display_or(&bridge.ir.system),
    );
    for export in EXPORTS {
        let _ = writeln!(out, "| `{export}` | {} |", export_describes(export));
    }
    let _ = writeln!(
        out,
        "| `{REALIZE}` | optional, and never in this module: a host that links a realization \
         exports it, and the page calls it if it is there |"
    );
    out
}

/// One line per export, for the README's table.
fn export_describes(export: &str) -> &'static str {
    match export {
        "ess_input_reserve" => {
            "reserves a buffer of the given length for the next request and answers its address in \
             linear memory"
        }
        "ess_dispatch" => {
            "serves the request in that buffer and answers the address of the JSON response"
        }
        "ess_output_len" => "the length in bytes of the response just produced",
        other => panic!("`{other}` is exported and this table does not describe it"),
    }
}
