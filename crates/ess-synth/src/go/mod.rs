//! The Go emitter: the second target behind the plan seam, and everything Go-shaped in this crate.
//!
//! # Why a second target, and why this one
//!
//! The wave-6 design claims the [`plan`](crate::plan) is language-neutral and Rust is only the
//! first emitter. A claim like that is worth exactly one test, and the test is a second language
//! that cannot cheat. Go has **no sum type**: every tagged union, every enum and every outcome set
//! in the model has to be encoded by hand or refused out loud, and there is no `enum` keyword to
//! hide behind. That is why W7.3 chose it.
//!
//! The result: this module consumes the same [`SynthesisPlan`] through the same public surface the
//! Rust emitter uses — [`SynthesisPlan::is_generated`], [`SynthesisPlan::obligation_of`],
//! [`SynthesisPlan::generated`], [`SynthesisPlan::obligations`] — and the planner did not change
//! by one line to admit it.
//!
//! # The encodings, in one place
//!
//! | model construct | Go |
//! |---|---|
//! | tagged union, enum, command outcome, event log | a **sealed interface**: one unexported marker method, one exported struct per variant. An undeclared variant cannot implement the marker from another package, so the set is closed at the package boundary |
//! | typestate lifecycle | one **distinct type per state**, transitions as methods on exactly the states that declare them. An undeclared move is a method that does not exist — Go refuses it at compile time, as Rust does |
//! | newtype | a struct with an **unexported field**, a constructor and an accessor. `type Email string` was refused: it lets an untyped constant become an `Email` by assignment, which is exactly the distinctness the newtype exists for |
//! | obligation | an **interface** per owed capability and one `Unimplemented` stub returning the typed refusal, bijective with the plan's obligations |
//! | transport | the same in-process at-least-once log, pump, redelivery and failure policy the Rust emitter derives from the same declarations |
//!
//! # What Go cannot carry, said out loud
//!
//! Two answers, never a silent downgrade:
//!
//! * a capability this target cannot represent at all is **refused** at
//!   [`RefusalStage::Target`](crate::plan::RefusalStage::Target), computed before a line is
//!   emitted;
//! * a capability it emits with a weaker guarantee than the first target's carries a
//!   [`TargetWeakening`], stated in the generated doc comment *and* in the
//!   `TARGET.md` beside the module.
//!
//! Both travel in the [`TargetReport`] this module returns, and neither touches the plan: the
//! plan's two renderings are byte-identical in both trees, which is the seam proving itself.

mod entity;
mod http;
mod items;
mod layout;
mod name;
mod obligation;
mod port;
mod refusal;
mod system;

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use ess_compiler::ir::{EssIr, ResolvedTypeRef};
use ess_domain::name::QualifiedName;
use ess_gen::{Artifact, Provenance};

use crate::plan::{Capability, CapabilityKind, SynthesisPlan, REGENERATE};
use crate::{TargetRefusal, TargetReport, TargetWeakening};

use self::layout::{Layout, Package};
use self::refusal::TargetRefusals;

/// The language version every generated Go module declares.
///
/// Pinned rather than read from the toolchain that happens to be installed: the module must build
/// outside this repository, from exactly the bytes committed, and a directive naming whatever
/// version generated it would make yesterday's committed tree unbuildable on an older toolchain
/// for no reason. Nothing emitted here uses anything newer.
const GO_VERSION: &str = "1.21";

/// The name this target reports itself under.
pub const TARGET: &str = "go";

/// What one Go emission produced.
pub struct Emission {
    /// Every file, in path order.
    pub artifacts: Vec<Artifact>,
    /// What this target could not carry across from the plan.
    pub report: TargetReport,
}

/// Everything the generated package renderers need to agree on, carried once.
pub(crate) struct Emit<'a> {
    /// The resolved model.
    pub ir: &'a EssIr,
    /// Where everything lands and what it is called.
    pub layout: &'a Layout,
    /// The package being rendered.
    pub package: &'a Package,
    /// The bounded context whose package it is, where it is one.
    pub domain: Option<&'a QualifiedName>,
    /// The packages referenced so far, which become the file's import block.
    ///
    /// Collected while rendering rather than declared up front, because Go makes an unused import
    /// a compile error: a hand-maintained list is a list that is wrong the first time a renderer
    /// stops needing something.
    imports: RefCell<BTreeSet<String>>,
    /// The subset of them imported for a side effect and never named.
    ///
    /// One package needs it — `embed`, which a `//go:embed` directive requires present and which
    /// no line then mentions, so Go's unused-import rule refuses the ordinary form. Kept beside
    /// the paths rather than as a second block, because `gofmt` sorts one block by path and a
    /// blank import sorts with the rest.
    blank: RefCell<BTreeSet<String>>,
}

impl<'a> Emit<'a> {
    /// A renderer for one package.
    pub fn new(
        ir: &'a EssIr,
        layout: &'a Layout,
        package: &'a Package,
        domain: Option<&'a QualifiedName>,
    ) -> Self {
        Self {
            ir,
            layout,
            package,
            domain,
            imports: RefCell::new(BTreeSet::new()),
            blank: RefCell::new(BTreeSet::new()),
        }
    }

    /// A resolved type reference as Go, from inside this package.
    pub fn go_type(&self, type_ref: &ResolvedTypeRef) -> String {
        self.layout
            .go_type(type_ref, self.package, &mut self.imports.borrow_mut())
    }

    /// A declaration as Go, from inside this package.
    pub fn reference(&self, declared: &QualifiedName) -> String {
        self.layout
            .reference(declared, self.package, &mut self.imports.borrow_mut())
    }

    /// Any name of another package, as spelled from inside this one.
    pub fn qualify(&self, package: &Package, name: &str) -> String {
        layout::qualify(package, name, self.package, &mut self.imports.borrow_mut())
    }

    /// Records an import this file needs that no reference collected on its own — a
    /// standard-library package, always, because every generated package reaches this crate's own
    /// layout instead.
    pub fn import(&self, path: &str) {
        self.imports.borrow_mut().insert(path.to_owned());
    }

    /// Records an import taken for its side effect alone, which Go spells `_ "path"`.
    pub fn import_blank(&self, path: &str) {
        self.imports.borrow_mut().insert(path.to_owned());
        self.blank.borrow_mut().insert(path.to_owned());
    }

    /// One variant type of an enum or a tagged union, spelled from inside this package.
    pub fn reference_variant(&self, declared: &QualifiedName, variant: &str) -> String {
        self.qualify(
            self.layout.package_of(declared),
            self.layout.variant(declared, variant),
        )
    }

    /// A newtype's constructor, spelled from inside this package.
    pub fn reference_ctor(&self, declared: &QualifiedName) -> String {
        self.qualify(self.layout.package_of(declared), self.layout.ctor(declared))
    }

    /// A command's outcome interface, spelled from inside this package.
    pub fn reference_outcome(&self, command: &QualifiedName) -> String {
        self.qualify(
            self.layout.package_of(command),
            self.layout.outcome(command),
        )
    }

    /// One variant of a command's outcome, spelled from inside this package.
    pub fn reference_outcome_variant(&self, command: &QualifiedName, outcome: &str) -> String {
        self.qualify(
            self.layout.package_of(command),
            self.layout.outcome_variant(command, outcome),
        )
    }

    /// One specification primitive's Go type, spelled from inside this package.
    pub fn primitive_type(&self, primitive: ess_domain::types::Primitive) -> String {
        self.go_type(&ResolvedTypeRef::Primitive { name: primitive })
    }

    /// The constructor of a primitive that has no standard-library equivalent.
    ///
    /// # Panics
    ///
    /// For the four primitives that map onto a Go type directly: they have no constructor, and
    /// asking for one is a defect in the caller rather than a fact about a specification.
    pub fn primitive_ctor(&self, primitive: ess_domain::types::Primitive) -> String {
        use ess_domain::types::Primitive;
        let wrapper = match primitive {
            Primitive::Decimal => "NewDecimal",
            Primitive::Timestamp => "NewTimestamp",
            Primitive::Duration => "NewDuration",
            Primitive::Uuid => "NewUuid",
            other => panic!("`{other:?}` maps onto a Go type directly and has no constructor"),
        };
        self.qualify(self.layout.primitives(), wrapper)
    }

    /// The finished file at a chosen path, for a package that holds more than one.
    pub fn file_at(
        &self,
        path: impl Into<String>,
        provenance: &Provenance,
        doc: &str,
        body: &str,
    ) -> Artifact {
        let artifact = self.file(provenance, doc, body);
        Artifact::new(path, artifact.contents)
    }

    /// The typed refusal an owed seam answers with, spelled from inside this package.
    pub fn unmet(&self) -> String {
        format!(
            "*{}",
            self.qualify(self.layout.obligation(), "UnmetObligation")
        )
    }

    /// The refusal type's constructor, spelled from inside this package.
    pub fn unmet_literal(&self, kind: CapabilityKind, source: &str) -> String {
        format!(
            "&{}{{Capability: {:?}, Source: {:?}}}",
            self.qualify(self.layout.obligation(), "UnmetObligation"),
            kind.describes(),
            source
        )
    }

    /// `true` when this package's bounded context owns the declaration.
    pub fn owns(&self, declared: &QualifiedName) -> bool {
        self.domain
            .is_some_and(|domain| self.layout.owner(declared) == domain)
    }

    /// The finished file: provenance, package documentation, the imports collected while
    /// rendering, and the body.
    pub fn file(&self, provenance: &Provenance, doc: &str, body: &str) -> Artifact {
        let mut out = provenance.commented_for("//", REGENERATE);
        out.push('\n');
        out.push_str(doc);
        let _ = writeln!(out, "package {}", self.package.name);
        let imports = self.imports.borrow();
        if !imports.is_empty() {
            let blank = self.blank.borrow();
            out.push_str("\nimport (\n");
            for import in imports.iter() {
                if blank.contains(import) {
                    let _ = writeln!(out, "\t_ {import:?}");
                } else {
                    let _ = writeln!(out, "\t{import:?}");
                }
            }
            out.push_str(")\n");
        }
        out.push_str(body);
        Artifact::new(self.package.file(), out)
    }
}

/// Emits the Go module a plan determines, and reports what this target could not carry.
///
/// # Panics
///
/// If what was emitted is not exactly what the plan marks generated *minus* what this target
/// refused — a defect in this crate, and the one lie neither the plan nor the target report may be
/// allowed to tell.
pub fn workspace(ir: &EssIr, plan: &SynthesisPlan) -> Emission {
    let refusals = TargetRefusals::of(ir, plan);
    let layout = Layout::of(ir, plan, &refusals);
    let provenance = &plan.provenance;

    let mut covered: BTreeSet<Capability> = BTreeSet::new();
    let mut stubbed: BTreeSet<Capability> = BTreeSet::new();

    let mut artifacts = vec![
        module_file(&layout, provenance),
        primitives_package(&layout, provenance),
    ];
    if wants_obligations(ir, plan) {
        artifacts.push(obligation::refusal_package(&layout, provenance));
    }
    artifacts.extend(obligation::conversion_package(
        ir,
        plan,
        &layout,
        &refusals,
        provenance,
        &mut stubbed,
    ));
    let domains: Vec<QualifiedName> = layout
        .packages()
        .map(|(domain, _)| domain.clone())
        .collect();
    for domain in &domains {
        artifacts.push(domain_package(
            ir,
            plan,
            &layout,
            &refusals,
            domain,
            &mut covered,
            &mut stubbed,
        ));
    }
    for component in ir.components.values() {
        artifacts.extend(port::component_package(
            ir,
            plan,
            &layout,
            &refusals,
            component,
            &mut covered,
        ));
    }
    artifacts.extend(system::system_package(
        ir,
        plan,
        &layout,
        &refusals,
        &mut covered,
        &mut stubbed,
    ));
    artifacts.extend(http::server_package(
        ir,
        plan,
        &layout,
        &refusals,
        &mut covered,
    ));

    assert_bijection(plan, &refusals, &covered, &stubbed);

    Emission {
        artifacts,
        report: TargetReport {
            provenance: provenance.clone(),
            target: TARGET,
            weakenings: weakenings(ir, &refusals),
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

/// Holds the emitter to the plan: emitted is exactly generated minus target-refused, and stubbed
/// is exactly owed minus target-refused.
///
/// Its own function rather than a tail of [`workspace`], because the two assertions are one claim —
/// the plan and the module are the same statement in two renderings — and a reader checking that
/// claim should not have to read an artifact list first.
fn assert_bijection(
    plan: &SynthesisPlan,
    refusals: &TargetRefusals,
    covered: &BTreeSet<Capability>,
    stubbed: &BTreeSet<Capability>,
) {
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
        covered, &planned,
        "the Go emitter emitted a different set of capabilities than the plan marks generated \
         and this target did not refuse; that is a defect in ess-synth, and shipping it would \
         make PLAN.md a lie about the module"
    );
    let owed: BTreeSet<Capability> = plan
        .obligations()
        .map(|(capability, _)| capability.clone())
        .filter(|capability| !refused.contains(capability))
        .collect();
    assert_eq!(
        stubbed, &owed,
        "the Go emitter's stubs are not exactly the plan's obligations minus what this target \
         refused; that is a defect in ess-synth, and shipping it would break the promise that \
         every owed capability is visible twice — in the plan, and as a typed refusal in the code"
    );
}

/// `true` when the module needs the refusal type at all: something is owed, or there is an
/// interaction layer whose ports return one.
fn wants_obligations(ir: &EssIr, plan: &SynthesisPlan) -> bool {
    plan.obligations().next().is_some() || !ir.components.is_empty() || !ir.bindings.is_empty()
}

/// What this target emits with a weaker guarantee than the first target's, stated once per rule.
///
/// Per rule rather than per capability, deliberately: "Go has no exhaustiveness check" is one
/// fact about the language, and repeating it against forty capabilities would bury the two rows a
/// reader has to act on. Each rule names the capability kinds it touches, so the parity question —
/// *what is different about my command contracts* — is still answerable from the table.
///
/// Two rows are conditional on the specification declaring a served surface at all, because a
/// weakening naming a capability kind this module has no instance of is a row a reader has to check
/// and then discard. Everything else here is a fact about Go and holds whatever the specification
/// says.
fn weakenings(ir: &EssIr, refusals: &TargetRefusals) -> Vec<TargetWeakening> {
    let serves = !http::served(ir, refusals).is_empty();
    let mut exhaustive_affects = vec![
        CapabilityKind::DomainType,
        CapabilityKind::EntityLifecycle,
        CapabilityKind::CommandContract,
        CapabilityKind::ComponentPort,
        CapabilityKind::BindingDelivery,
    ];
    if serves {
        exhaustive_affects.push(CapabilityKind::ComponentTransport);
        exhaustive_affects.sort();
    }
    let mut out = vec![
        TargetWeakening {
            guarantee: "handling a closed set of variants is exhaustive: a `match` that forgets \
                        one does not compile"
                .to_owned(),
            instead: "the set stays closed — an undeclared variant cannot implement the sealed \
                      interface's unexported marker from another package — but a `switch` over it \
                      is not checked, so a consumer that forgets a variant compiles and falls \
                      through. Go has no exhaustiveness check and none can be emitted; every \
                      generated sealed interface says so in its own doc comment"
                .to_owned(),
            affects: exhaustive_affects,
        },
        TargetWeakening {
            guarantee: "a value of a generated type exists only where a generated constructor or \
                        transition produced one"
                .to_owned(),
            instead: "Go gives every type a zero value that no constructor has to produce, so \
                      `Email{}`, an invoice resting in a state nothing moved it to, and a nil \
                      variant of a sealed interface are all spellable from any package. The \
                      unexported field stops a *populated* value being forged; nothing in the \
                      language stops the empty one existing"
                .to_owned(),
            affects: vec![
                CapabilityKind::DomainType,
                CapabilityKind::EntityLifecycle,
                CapabilityKind::CommandContract,
                CapabilityKind::EventType,
                CapabilityKind::ErrorType,
                CapabilityKind::ViewType,
            ],
        },
        TargetWeakening {
            guarantee:
                "refining a runtime state into the typed lifecycle is total: every declared \
                        state has an arm and no other state can reach it"
                    .to_owned(),
            instead: "the snapshot's state field is a sealed interface, whose zero value is nil \
                      and names no declared state — the previous row's weakening, reaching this \
                      one. Refinement therefore answers `(value, ok)`, and a caller that ignores \
                      the second result gets the interface's own zero value"
                .to_owned(),
            affects: vec![CapabilityKind::EntityLifecycle],
        },
        TargetWeakening {
            guarantee: "every generated type compares by value".to_owned(),
            instead: "Go defines `==` only for comparable types, so a generated type carrying a \
                      list, a map or bytes cannot be compared at all — and no deep comparison is \
                      emitted in its place, because a hand-written equality is behaviour, and \
                      behaviour is not synthesised"
                .to_owned(),
            affects: vec![
                CapabilityKind::DomainType,
                CapabilityKind::EntityLifecycle,
                CapabilityKind::CommandContract,
                CapabilityKind::EventType,
                CapabilityKind::ErrorType,
                CapabilityKind::ViewType,
            ],
        },
    ];
    if serves {
        out.push(TargetWeakening {
            guarantee: "a JSON object leaves this system with its members in the order the \
                        specification declares them"
                .to_owned(),
            instead: "the served bodies are built as `map[string]any` and written by \
                      `encoding/json`, which sorts a map's keys — so a body's members come out \
                      alphabetical here and in declaration order in the first target. The two are \
                      the same *value*, no published contract states an order, and every consumer \
                      that parses rather than greps is unaffected; what is lost is the ability to \
                      compare two applications' bodies byte for byte, which is why the gate \
                      compares them as values. Emitting a writer that kept the order would mean \
                      emitting a second JSON writer beside the standard library's"
                .to_owned(),
            affects: vec![CapabilityKind::ComponentTransport],
        });
    }
    out
}

/// The module file at the generated root.
fn module_file(layout: &Layout, provenance: &Provenance) -> Artifact {
    let mut out = provenance.commented_for("//", REGENERATE);
    let _ = write!(out, "\nmodule {}\n\ngo {GO_VERSION}\n", layout.module());
    Artifact::new("go.mod", out)
}

/// The `primitives` package: the representation each specification primitive gets in this target.
///
/// Fixed per emitter version rather than derived from the specification, exactly as the Rust
/// emitter's is: the same eight primitives get the same eight spellings whatever the system.
fn primitives_package(layout: &Layout, provenance: &Provenance) -> Artifact {
    let package = layout.primitives();
    let mut out = provenance.commented_for("//", REGENERATE);
    out.push('\n');
    out.push_str(PRIMITIVES_DOC);
    let _ = writeln!(out, "package {}", package.name);
    for (type_name, what, rendering) in PRIMITIVES {
        let _ = write!(
            out,
            "\n// {type_name} is {what}\n//\n// A wrapper over its wire rendering, distinct from \
             `string` and from every other wrapper here for\n// the reason the specification's own \
             newtypes are distinct from their representations: a value's\n// meaning is not its \
             shape. The field is unexported, so the only way to make one is\n// [New{type_name}] — \
             but Go's zero value needs no constructor, so `{type_name}{{}}` is still\n// spellable \
             (see TARGET.md).\ntype {type_name} struct {{\n\tvalue string\n}}\n\n// New{type_name} \
             wraps {rendering} as a {type_name}.\nfunc New{type_name}(value string) {type_name} \
             {{\n\treturn {type_name}{{value: value}}\n}}\n\n// Value is the wrapped \
             rendering.\nfunc (v {type_name}) Value() string {{\n\treturn v.value\n}}\n"
        );
    }
    Artifact::new(package.file(), out)
}

/// The `primitives` package's own documentation.
const PRIMITIVES_DOC: &str =
    "// Package primitives spells the specification's primitives for this target.\n\
    //\n\
    // Four map onto types that already mean exactly the same thing: `String` stays `string`,\n\
    // `Boolean` is `bool`, `Integer` is `int64`, `Bytes` is `[]byte`. The four below have no\n\
    // standard-library equivalent, and no dependency is taken for them — this module builds from\n\
    // exactly its committed bytes, with nothing to download.\n";

/// The four primitives with no standard-library equivalent: type name, what it is, what it wraps.
const PRIMITIVES: &[(&str, &str, &str)] = &[
    (
        "Decimal",
        "an exact decimal, carried as its wire rendering — a decimal string such as `10.50`.\n// \
         Never a float: money does not round the way a float does, and arithmetic is deliberately\n// \
         absent, because what a decimal *does* is behaviour.",
        "a decimal string",
    ),
    (
        "Duration",
        "a length of time, carried as its wire rendering — an ISO 8601 duration such as `P30D`.",
        "an ISO 8601 duration",
    ),
    (
        "Timestamp",
        "an instant, carried as its wire rendering — RFC 3339, such as `2026-01-01T00:00:00Z`.",
        "an RFC 3339 instant",
    ),
    (
        "Uuid",
        "a UUID, carried as its canonical textual rendering.",
        "a canonical UUID rendering",
    ),
];

/// One bounded context's package: every declaration the plan marks generated and this target does
/// not refuse, in a fixed order.
fn domain_package(
    ir: &EssIr,
    plan: &SynthesisPlan,
    layout: &Layout,
    refusals: &TargetRefusals,
    domain: &QualifiedName,
    covered: &mut BTreeSet<Capability>,
    stubbed: &mut BTreeSet<Capability>,
) -> Artifact {
    let package = layout.package(domain);
    let emit = Emit::new(ir, layout, package, Some(domain));
    let resolved = ir
        .domains
        .get(domain)
        .expect("the layout only knows domains the IR declares");

    let mut doc = format!(
        "// Package {} is {} — `{domain}`.\n",
        package.name,
        resolved.naming.display_or(domain)
    );
    if let Some(summary) = &resolved.naming.summary {
        let _ = write!(doc, "//\n// {}\n", summary.trim());
    }
    doc.push_str(
        "//\n// Everything this bounded context declares that the synthesis plan marks generated \
         and this\n// target can represent. What it cannot is in the TARGET.md beside this \
         module, never absent.\n",
    );

    let mut body = String::new();
    items::declarations(&mut body, &emit, plan, refusals, covered);
    items::conversions(&mut body, &emit, plan, refusals, covered);
    obligation::domain_obligations(&mut body, &emit, plan, refusals, stubbed);
    emit.file(&plan.provenance, &doc, &body)
}

/// The plan's gate: emit only what it marks generated *and* this target can represent, and record
/// what was emitted so [`workspace`] can hold the emitter to the whole list.
pub(crate) fn cover(
    plan: &SynthesisPlan,
    refusals: &TargetRefusals,
    covered: &mut BTreeSet<Capability>,
    kind: CapabilityKind,
    source: &str,
) -> bool {
    if !plan.is_generated(kind, source) || refusals.refuses_kind(kind, source) {
        return false;
    }
    covered.insert(Capability {
        kind,
        source: source.to_owned(),
    });
    true
}

/// Records one stub against the bijection [`workspace`] asserts.
pub(crate) fn record(stubbed: &mut BTreeSet<Capability>, kind: CapabilityKind, source: &str) {
    assert!(
        stubbed.insert(Capability {
            kind,
            source: source.to_owned(),
        }),
        "two stubs claimed the obligation `{source}` ({}); that is a defect in ess-synth",
        kind.describes()
    );
}

/// The doc paragraph every generated sealed interface carries, naming the weakening it rests on.
///
/// One sentence, in the generated source, because a weakening recorded only in a report beside the
/// code is a weakening the next reader of the code does not meet. `switch` is the word they will
/// have typed.
pub(crate) const EXHAUSTIVENESS_NOTE: &str =
    "// A closed set: the marker method below is unexported, so no type outside this package can\n\
     // join it. Go cannot check that a `switch` over it handles every case — that is a \
     target-stage\n// weakening of what the specification declares, recorded in TARGET.md, not a \
     gap in the model.\n";

/// One field of a generated struct, with the specification's own spelling of its type in the docs.
pub(crate) fn field_line(
    out: &mut String,
    emit: &Emit<'_>,
    taken: &mut BTreeMap<String, usize>,
    field: &ess_compiler::ir::ResolvedField,
) {
    let ident = unique_field(taken, name::exported(&field.name));
    let _ = writeln!(
        out,
        "\t// {ident} is `{}` — `{}`.\n\t{ident} {}",
        field.name,
        field.type_ref,
        emit.go_type(&field.type_ref)
    );
}

/// A struct field name, moved out of the way of one already used in the same struct.
///
/// Two specification fields can pascal-case to one Go identifier (`invoice_id` and `invoiceId`),
/// which Rust never has to repair because it keeps the specification's own snake case. The repair
/// appends `_`, and repeats.
pub(crate) fn unique_field(taken: &mut BTreeMap<String, usize>, mut candidate: String) -> String {
    while taken.contains_key(&candidate) {
        candidate.push('_');
    }
    taken.insert(candidate.clone(), 0);
    candidate
}
