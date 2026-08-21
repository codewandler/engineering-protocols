//! Where every declaration lands in the generated Go module, and what it is called there.
//!
//! The Rust emitter's `layout` decides the same two questions for its target, and the reasoning
//! carries over: the plan names an item, a renderer declares it, another package imports it, so
//! the decision is a value computed once from the IR rather than a convention each renderer
//! re-implements.
//!
//! What does *not* carry over is why the name table is exhaustive here. Rust has several
//! namespaces — a type, a function, a module and an enum's variants can all be spelled `Invoice`
//! without colliding — and Go has exactly one per package. Every identifier this emitter derives
//! (`NewEmail`, `InvoiceData`, `PayeeCompany`, `SendEmailOutcomeFailed`) therefore competes with
//! every declared type name in the same package, so the names are **allocated**, in one fixed
//! sweep, with a deterministic repair. That is a target-stage fact about Go, not a fact about the
//! model, and it is the reason this module is larger than its Rust sibling.

use std::collections::{BTreeMap, BTreeSet};

use ess_compiler::ir::{EssIr, EventHandle, ResolvedBody, ResolvedTypeRef};
use ess_domain::component::ComponentName;
use ess_domain::name::QualifiedName;
use ess_domain::types::Primitive;

use crate::plan::{conversion_source, mechanical_conversion, Capability, SynthesisPlan};

use super::name;
use super::refusal::TargetRefusals;

/// The module path every generated import is rooted at.
///
/// `.invalid` is the reserved top-level domain of RFC 2606: it can never resolve, so a module
/// path under it can never be mistaken for something publishable and `go get` on it can never
/// reach a network. A synthesised workspace is a starting point someone re-homes under their own
/// path, and a plausible-looking path would be the one thing they forget to change.
pub const MODULE_HOST: &str = "example.invalid";

/// One Go package of the generated module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Package {
    /// Its package clause and the qualifier every reference to it is spelled with.
    pub name: String,
    /// Its directory under the module root, `/`-separated.
    pub dir: String,
    /// Its full import path.
    pub import: String,
}

impl Package {
    /// The file this package's declarations are generated into.
    pub fn file(&self) -> String {
        format!("{}/{}.go", self.dir, self.name)
    }
}

/// The shape of the generated Go module: one package per bounded context, one per component, one
/// for the system, and three the target itself needs.
pub(crate) struct Layout {
    /// The module path — `example.invalid/billing`.
    module: String,
    /// Package per bounded context.
    domains: BTreeMap<QualifiedName, Package>,
    /// Package per component.
    components: BTreeMap<ComponentName, Package>,
    /// The primitive representations every other package spells values in.
    primitives: Package,
    /// The typed refusal of an unmet obligation, in a package that imports nothing.
    ///
    /// Its own package, and a leaf one, because Go forbids an import cycle where Rust allows a
    /// module cycle: a bounded context's package must import the refusal type, so the refusal
    /// type's package can never import a bounded context. The owed *conversions*, which do
    /// reference both ends, live in [`Self::conversion`] for exactly that reason — one place
    /// where the target's rules move a file, and the plan does not notice.
    obligation: Package,
    /// The owed crossings between bounded contexts.
    conversion: Package,
    /// The bindings and the one transport.
    system: Package,
    /// The HTTP surface of every component reached over a network.
    server: Package,
    /// The bounded context that owns each declaration.
    owners: BTreeMap<QualifiedName, QualifiedName>,
    /// Every identifier this emitter declares, allocated once, keyed by [`Key`].
    names: BTreeMap<String, String>,
    /// Every event the system's log can carry, in name order.
    system_events: BTreeSet<EventHandle>,
}

/// The kinds of identifier the emitter derives, as the first half of a name-table key.
///
/// A key rather than a typed enum because the second half is one, two or three specification
/// spellings and the table only ever needs equality — the cost of the typed version is a variant
/// per kind and a `match` in every accessor, for no property this does not already have.
mod key {
    /// A declared type, entity, command, event, error or view.
    pub const DECLARED: &str = "declared";
    /// A newtype's or entity's constructor function.
    pub const CTOR: &str = "ctor";
    /// One variant type of an enum or a tagged union.
    pub const VARIANT: &str = "variant";
    /// An entity's data struct.
    pub const DATA: &str = "data";
    /// An entity resting in one state, as a type.
    pub const STATE: &str = "state";
    /// An entity's boundary shape.
    pub const SNAPSHOT: &str = "snapshot";
    /// An entity in whichever state it was found.
    pub const ANY: &str = "any";
    /// A command's outcome interface.
    pub const OUTCOME: &str = "outcome";
    /// One variant type of a command's outcome.
    pub const OUTCOME_VARIANT: &str = "outcomevariant";
    /// A command's behaviour obligation.
    pub const BEHAVIOR: &str = "behavior";
    /// A view's query obligation.
    pub const QUERY: &str = "query";
    /// A generated mechanical conversion function.
    pub const CONVERT: &str = "convert";
    /// An owed conversion's interface.
    pub const OWED: &str = "owed";
    /// The refusing stub of a package's obligations.
    pub const UNIMPLEMENTED: &str = "unimplemented";
    /// A component's port type.
    pub const PORT: &str = "port";
    /// A component's port constructor.
    pub const PORT_NEW: &str = "portnew";
    /// The interface bundling everything a component owes.
    pub const BEHAVIORS: &str = "behaviors";
    /// A component's outbox interface.
    pub const PUBLISHED: &str = "published";
    /// One variant of a component's outbox.
    pub const PUBLISHED_VARIANT: &str = "publishedvariant";
    /// One variant of the system's event log.
    pub const SYSTEM_EVENT: &str = "systemevent";
    /// One variant of the system's invocation record.
    pub const INVOCATION: &str = "invocation";
    /// A binding's generated transformation function.
    pub const TRANSFORM: &str = "transform";
    /// A binding's owed transformation interface.
    pub const TRANSFORMATION: &str = "transformation";
    /// A binding's owed escalation interface.
    pub const ESCALATION: &str = "escalation";
    /// A name the system package declares once.
    pub const SYSTEM: &str = "system";
}

impl Layout {
    /// Derives the layout of a resolved specification for the Go target.
    pub fn of(ir: &EssIr, plan: &SynthesisPlan, refusals: &TargetRefusals) -> Self {
        let module = format!("{MODULE_HOST}/{}", ir.system.segments().join("-"));
        let package = |name: &str, dir: &str| Package {
            name: name.to_owned(),
            dir: dir.to_owned(),
            import: format!("{module}/{dir}"),
        };
        let primitives = package("primitives", "types/primitives");
        let obligation = package("obligation", "types/obligation");
        let conversion = package("conversion", "types/conversion");
        let system = package("system", "system");
        let server = package("server", "server");

        // One namespace for package names across the whole module: the system package imports
        // every other one, so two packages sharing a name is a file that cannot spell one of them.
        let mut taken: BTreeSet<String> = [
            primitives.name.clone(),
            obligation.name.clone(),
            conversion.name.clone(),
            system.name.clone(),
            server.name.clone(),
        ]
        .into();
        let mut domains = BTreeMap::new();
        for (domain, ident) in domain_idents(ir) {
            let name = repair(&mut taken, ident, "domain");
            let dir = format!("types/{name}");
            domains.insert(domain, package(&name.clone(), &dir));
        }
        let mut components = BTreeMap::new();
        for component in ir.components.keys() {
            let name = repair(
                &mut taken,
                name::package_ident(&component.to_string()),
                "component",
            );
            let dir = format!("components/{name}");
            components.insert(component.clone(), package(&name.clone(), &dir));
        }

        let mut owners = BTreeMap::new();
        for domain in ir.domains.values() {
            for declared in &domain.types {
                owners.insert(declared.name().clone(), domain.name.clone());
            }
            for entity in &domain.entities {
                owners.insert(entity.name().clone(), domain.name.clone());
            }
            for command in &domain.commands {
                owners.insert(command.name().clone(), domain.name.clone());
            }
            for event in &domain.events {
                owners.insert(event.name().clone(), domain.name.clone());
            }
            for error in &domain.errors {
                owners.insert(error.name().clone(), domain.name.clone());
            }
            for view in &domain.views {
                owners.insert(view.name().clone(), domain.name.clone());
            }
        }

        let system_events = system_events(ir, plan, refusals);
        let mut layout = Self {
            module,
            domains,
            components,
            primitives,
            obligation,
            conversion,
            system,
            server,
            owners,
            names: BTreeMap::new(),
            system_events,
        };
        layout.allocate_names(ir);
        layout
    }

    /// The module path.
    pub fn module(&self) -> &str {
        &self.module
    }

    /// The package of a bounded context.
    pub fn package(&self, domain: &QualifiedName) -> &Package {
        self.domains.get(domain).unwrap_or_else(|| {
            panic!(
                "`{domain}` is not a domain this layout knows: it was derived from a different IR"
            )
        })
    }

    /// The package of a component.
    pub fn component(&self, component: &ComponentName) -> &Package {
        self.components.get(component).unwrap_or_else(|| {
            panic!("`{component}` is not a component this layout knows: it was derived from a different IR")
        })
    }

    /// Every bounded context and its package, in name order.
    pub fn packages(&self) -> impl Iterator<Item = (&QualifiedName, &Package)> {
        self.domains.iter()
    }

    /// The primitives package.
    pub fn primitives(&self) -> &Package {
        &self.primitives
    }

    /// The package holding the typed refusal of an unmet obligation.
    pub fn obligation(&self) -> &Package {
        &self.obligation
    }

    /// The package holding the owed crossings.
    pub fn conversion(&self) -> &Package {
        &self.conversion
    }

    /// The system package.
    pub fn system(&self) -> &Package {
        &self.system
    }

    /// The server package: the routes, the codecs and the listener.
    ///
    /// Reserved whether or not it is emitted, and reserved before the domain and component
    /// packages are allocated: a bounded context called `server` must not take the name the
    /// server package would have.
    pub fn server(&self) -> &Package {
        &self.server
    }

    /// Every event the system's log can carry.
    pub fn system_events(&self) -> &BTreeSet<EventHandle> {
        &self.system_events
    }

    /// The bounded context that owns a declaration.
    pub fn owner(&self, declared: &QualifiedName) -> &QualifiedName {
        self.owners.get(declared).unwrap_or_else(|| {
            panic!("`{declared}` is not a declaration this layout knows: it was derived from a different IR")
        })
    }

    /// The package a declaration lands in.
    pub fn package_of(&self, declared: &QualifiedName) -> &Package {
        self.package(self.owner(declared))
    }

    /// The Go name of a declared type, entity, command, event, error or view.
    pub fn declared(&self, name: &QualifiedName) -> &str {
        self.name(&[key::DECLARED, &name.to_string()])
    }

    /// The constructor of a newtype or an entity.
    pub fn ctor(&self, name: &QualifiedName) -> &str {
        self.name(&[key::CTOR, &name.to_string()])
    }

    /// One variant type of an enum or a tagged union.
    pub fn variant(&self, name: &QualifiedName, variant: &str) -> &str {
        self.name(&[key::VARIANT, &name.to_string(), variant])
    }

    /// An entity's data struct.
    pub fn data(&self, entity: &QualifiedName) -> &str {
        self.name(&[key::DATA, &entity.to_string()])
    }

    /// An entity resting in one state, as a type.
    pub fn state(&self, entity: &QualifiedName, state: &str) -> &str {
        self.name(&[key::STATE, &entity.to_string(), state])
    }

    /// An entity's boundary shape.
    pub fn snapshot(&self, entity: &QualifiedName) -> &str {
        self.name(&[key::SNAPSHOT, &entity.to_string()])
    }

    /// An entity in whichever state it was found.
    pub fn any(&self, entity: &QualifiedName) -> &str {
        self.name(&[key::ANY, &entity.to_string()])
    }

    /// A command's outcome interface.
    pub fn outcome(&self, command: &QualifiedName) -> &str {
        self.name(&[key::OUTCOME, &command.to_string()])
    }

    /// One variant type of a command's outcome.
    pub fn outcome_variant(&self, command: &QualifiedName, outcome: &str) -> &str {
        self.name(&[key::OUTCOME_VARIANT, &command.to_string(), outcome])
    }

    /// A command's behaviour obligation.
    pub fn behavior(&self, command: &QualifiedName) -> &str {
        self.name(&[key::BEHAVIOR, &command.to_string()])
    }

    /// A view's query obligation.
    pub fn query(&self, view: &QualifiedName) -> &str {
        self.name(&[key::QUERY, &view.to_string()])
    }

    /// The function a generated mechanical conversion is declared as.
    pub fn convert(&self, source: &str) -> &str {
        self.name(&[key::CONVERT, source])
    }

    /// The interface an owed conversion is declared as.
    pub fn owed(&self, source: &str) -> &str {
        self.name(&[key::OWED, source])
    }

    /// The refusing stub of one package's obligations.
    pub fn unimplemented(&self, package: &Package) -> &str {
        self.name(&[key::UNIMPLEMENTED, &package.dir])
    }

    /// A component's port type.
    pub fn port(&self, component: &ComponentName) -> &str {
        self.name(&[key::PORT, &component.to_string()])
    }

    /// A component's port constructor.
    pub fn port_new(&self, component: &ComponentName) -> &str {
        self.name(&[key::PORT_NEW, &component.to_string()])
    }

    /// The interface bundling everything a component owes.
    pub fn behaviors(&self, component: &ComponentName) -> &str {
        self.name(&[key::BEHAVIORS, &component.to_string()])
    }

    /// A component's outbox interface.
    pub fn published(&self, component: &ComponentName) -> &str {
        self.name(&[key::PUBLISHED, &component.to_string()])
    }

    /// One variant of a component's outbox.
    pub fn published_variant(&self, component: &ComponentName, event: &QualifiedName) -> &str {
        self.name(&[
            key::PUBLISHED_VARIANT,
            &component.to_string(),
            &event.to_string(),
        ])
    }

    /// One variant of the system's event log.
    pub fn system_event(&self, event: &QualifiedName) -> &str {
        self.name(&[key::SYSTEM_EVENT, &event.to_string()])
    }

    /// One variant of the system's invocation record.
    pub fn invocation(&self, binding: &str) -> &str {
        self.name(&[key::INVOCATION, binding])
    }

    /// A binding's generated transformation function.
    pub fn transform(&self, binding: &str) -> &str {
        self.name(&[key::TRANSFORM, binding])
    }

    /// A binding's owed transformation interface.
    pub fn transformation(&self, binding: &str) -> &str {
        self.name(&[key::TRANSFORMATION, binding])
    }

    /// A binding's owed escalation interface.
    pub fn escalation(&self, binding: &str) -> &str {
        self.name(&[key::ESCALATION, binding])
    }

    /// One of the names the system package declares once.
    pub fn system_name(&self, what: &str) -> &str {
        self.name(&[key::SYSTEM, what])
    }

    /// One allocated name, by key.
    fn name(&self, parts: &[&str]) -> &str {
        let key = parts.join("\u{1f}");
        self.names.get(&key).map_or_else(
            || panic!("`{key}` was never allocated a Go name; that is a defect in ess-synth"),
            String::as_str,
        )
    }
}

/// Allocates one name per emitted identifier, in a fixed sweep, repairing collisions.
impl Layout {
    /// The sweep, in the order that *is* the repair rule: declared names first and all of them,
    /// because a declared type's name comes from the specification and a derived one is this
    /// emitter's to move; then the derived names, family by family, each in the IR's own order.
    fn allocate_names(&mut self, ir: &EssIr) {
        let mut taken: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        self.allocate_declared(ir, &mut taken);
        self.allocate_type_names(ir, &mut taken);
        self.allocate_entity_names(ir, &mut taken);
        self.allocate_command_names(ir, &mut taken);
        self.allocate_conversion_names(ir, &mut taken);
        self.allocate_stub_names(&mut taken);
        self.allocate_component_names(ir, &mut taken);
        self.allocate_system_names(ir, &mut taken);
    }

    /// Every name the specification itself spells.
    fn allocate_declared(&mut self, ir: &EssIr, taken: &mut BTreeMap<String, BTreeSet<String>>) {
        for declared in ir.types.values() {
            self.declare(taken, &declared.name);
        }
        for entity in ir.entities.values() {
            self.declare(taken, &entity.name);
        }
        for command in ir.commands.values() {
            self.declare(taken, &command.name);
        }
        for event in ir.events.values() {
            self.declare(taken, &event.name);
        }
        for error in ir.errors.values() {
            self.declare(taken, &error.name);
        }
        for view in ir.views.values() {
            self.declare(taken, &view.name);
        }
    }

    /// A newtype's constructor, and one variant type per enum or union alternative.
    fn allocate_type_names(&mut self, ir: &EssIr, taken: &mut BTreeMap<String, BTreeSet<String>>) {
        for declared in ir.types.values() {
            let package = self.package_of(&declared.name).clone();
            let type_name = self.declared(&declared.name).to_owned();
            match &declared.body {
                ResolvedBody::Newtype { .. } => {
                    let candidate = format!("New{type_name}");
                    self.put(
                        taken,
                        &package,
                        &[key::CTOR, &declared.name.to_string()],
                        candidate,
                    );
                }
                ResolvedBody::Enum { variants } => {
                    for variant in variants {
                        let candidate = format!("{type_name}{}", name::exported(variant));
                        self.put(
                            taken,
                            &package,
                            &[key::VARIANT, &declared.name.to_string(), variant],
                            candidate,
                        );
                    }
                }
                ResolvedBody::Union { variants, .. } => {
                    for variant in variants.keys() {
                        let candidate = format!("{type_name}{}", name::exported(variant));
                        self.put(
                            taken,
                            &package,
                            &[key::VARIANT, &declared.name.to_string(), variant],
                            candidate,
                        );
                    }
                }
                ResolvedBody::Struct { .. } => {}
            }
        }
    }

    /// An entity's data struct, its constructor, one type per state, and the runtime boundary.
    fn allocate_entity_names(
        &mut self,
        ir: &EssIr,
        taken: &mut BTreeMap<String, BTreeSet<String>>,
    ) {
        for entity in ir.entities.values() {
            let package = self.package_of(&entity.name).clone();
            let type_name = self.declared(&entity.name).to_owned();
            let subject = entity.name.to_string();
            self.put(
                taken,
                &package,
                &[key::DATA, &subject],
                format!("{type_name}Data"),
            );
            self.put(
                taken,
                &package,
                &[key::CTOR, &subject],
                format!("New{type_name}"),
            );
            // `InvoiceInDraft`, not `InvoiceDraft`: a lifecycle's state names and its events'
            // names overlap constantly — billing declares `InvoiceIssued` as *both* a state and
            // an event — and a repaired `InvoiceIssued_` would be this emitter's collision
            // showing through into an API a person reads.
            for state in &entity.lifecycle.states {
                self.put(
                    taken,
                    &package,
                    &[key::STATE, &subject, state.as_str()],
                    format!("{type_name}In{state}"),
                );
            }
            self.put(
                taken,
                &package,
                &[key::SNAPSHOT, &subject],
                format!("{type_name}Snapshot"),
            );
            self.put(
                taken,
                &package,
                &[key::ANY, &subject],
                format!("Any{type_name}"),
            );
        }
    }

    /// A command's outcome interface and its variants, its behaviour seam, and a view's query seam.
    fn allocate_command_names(
        &mut self,
        ir: &EssIr,
        taken: &mut BTreeMap<String, BTreeSet<String>>,
    ) {
        for command in ir.commands.values() {
            let package = self.package_of(&command.name).clone();
            let type_name = self.declared(&command.name).to_owned();
            let subject = command.name.to_string();
            self.put(
                taken,
                &package,
                &[key::OUTCOME, &subject],
                format!("{type_name}Outcome"),
            );
            for outcome in &command.outcomes {
                let candidate = format!(
                    "{type_name}Outcome{}",
                    name::exported(outcome.name.as_str())
                );
                self.put(
                    taken,
                    &package,
                    &[key::OUTCOME_VARIANT, &subject, outcome.name.as_str()],
                    candidate,
                );
            }
            self.put(
                taken,
                &package,
                &[key::BEHAVIOR, &subject],
                format!("{type_name}Behavior"),
            );
        }

        for view in ir.views.values() {
            let package = self.package_of(&view.name).clone();
            let type_name = self.declared(&view.name).to_owned();
            self.put(
                taken,
                &package,
                &[key::QUERY, &view.name.to_string()],
                format!("{type_name}Query"),
            );
        }
    }

    /// A crossing's generated function, or the interface the owed one is answered by.
    fn allocate_conversion_names(
        &mut self,
        ir: &EssIr,
        taken: &mut BTreeMap<String, BTreeSet<String>>,
    ) {
        for conversion in &ir.conversions {
            let source = conversion_source(conversion);
            if let Some((from, to)) = mechanical_conversion(ir, conversion) {
                let package = self.package_of(to.name()).clone();
                let candidate = format!(
                    "{}From{}",
                    self.declared(to.name()),
                    name::type_fragment(&from.name().to_string())
                );
                self.put(taken, &package, &[key::CONVERT, &source], candidate);
            } else {
                let candidate = format!(
                    "{}To{}Conversion",
                    name::type_fragment(&conversion.from.to_string()),
                    name::type_fragment(&conversion.to.to_string())
                );
                let package = self.conversion.clone();
                self.put(taken, &package, &[key::OWED, &source], candidate);
            }
        }
    }

    /// The refusing stub every package that could ever declare one reserves.
    fn allocate_stub_names(&mut self, taken: &mut BTreeMap<String, BTreeSet<String>>) {
        // `Unimplemented` is reserved in every package that could ever declare one, whether or not
        // this specification gives it obligations: reserving costs a name nobody else wanted, and
        // making the reservation depend on the plan would make a declared type's spelling depend
        // on how many obligations the plan happens to hold.
        let packages: Vec<Package> = self
            .domains
            .values()
            .cloned()
            .chain([self.conversion.clone(), self.system.clone()])
            .collect();
        for package in packages {
            self.put(
                taken,
                &package,
                &[key::UNIMPLEMENTED, &package.dir.clone()],
                "Unimplemented".to_owned(),
            );
        }
    }

    /// A component's port, its constructor, its behaviours interface and its outbox.
    fn allocate_component_names(
        &mut self,
        ir: &EssIr,
        taken: &mut BTreeMap<String, BTreeSet<String>>,
    ) {
        for component in ir.components.values() {
            let package = self.component(&component.name).clone();
            let subject = component.name.to_string();
            self.put(
                taken,
                &package,
                &[key::PORT, &subject],
                name::exported(&subject),
            );
            self.put(
                taken,
                &package,
                &[key::PORT_NEW, &subject],
                "New".to_owned(),
            );
            self.put(
                taken,
                &package,
                &[key::BEHAVIORS, &subject],
                "Behaviors".to_owned(),
            );
            self.put(
                taken,
                &package,
                &[key::PUBLISHED, &subject],
                "PublishedEvent".to_owned(),
            );
            let published: BTreeSet<&EventHandle> = component.publishes.iter().collect();
            for (event, variant) in event_variants(ir, self, &published) {
                self.put(
                    taken,
                    &package,
                    &[key::PUBLISHED_VARIANT, &subject, &event.name().to_string()],
                    format!("PublishedEvent{variant}"),
                );
            }
        }
    }

    /// The system package: its fixed names, its log's variants, and one set per binding.
    fn allocate_system_names(
        &mut self,
        ir: &EssIr,
        taken: &mut BTreeMap<String, BTreeSet<String>>,
    ) {
        let system = self.system.clone();
        for what in [
            "SystemEvent",
            "BindingInvocation",
            "System",
            "NewSystem",
            "Obligations",
        ] {
            self.put(taken, &system, &[key::SYSTEM, what], what.to_owned());
        }
        let events: BTreeSet<&EventHandle> = self.system_events.iter().collect();
        let logged: Vec<(String, String)> = event_variants(ir, self, &events)
            .into_iter()
            .map(|(event, variant)| (event.name().to_string(), variant))
            .collect();
        for (event, variant) in logged {
            self.put(
                taken,
                &system,
                &[key::SYSTEM_EVENT, &event],
                format!("SystemEvent{variant}"),
            );
        }
        for binding in ir.bindings.values() {
            let subject = binding.name.to_string();
            let pascal = name::exported(&subject);
            self.put(
                taken,
                &system,
                &[key::INVOCATION, &subject],
                format!("BindingInvocation{pascal}"),
            );
            self.put(taken, &system, &[key::TRANSFORM, &subject], pascal.clone());
            self.put(
                taken,
                &system,
                &[key::TRANSFORMATION, &subject],
                format!("{pascal}Transformation"),
            );
            self.put(
                taken,
                &system,
                &[key::ESCALATION, &subject],
                format!("{pascal}Escalation"),
            );
        }
    }

    /// Allocates the name of one declaration, from the specification's own spelling.
    fn declare(
        &mut self,
        taken: &mut BTreeMap<String, BTreeSet<String>>,
        declared: &QualifiedName,
    ) {
        let package = self.package_of(declared).clone();
        let candidate = name::type_name(declared, self.owner(declared).segments().len());
        self.put(
            taken,
            &package,
            &[key::DECLARED, &declared.to_string()],
            candidate,
        );
    }

    /// Records one identifier, moving it out of the way of anything already declared in its
    /// package.
    ///
    /// The repair appends `_` and repeats, because a deterministic rename that can itself collide
    /// is a collision with extra steps — the rule the Rust emitter's package names already use.
    fn put(
        &mut self,
        taken: &mut BTreeMap<String, BTreeSet<String>>,
        package: &Package,
        parts: &[&str],
        mut candidate: String,
    ) {
        let used = taken.entry(package.dir.clone()).or_default();
        while used.contains(&candidate) {
            candidate.push('_');
        }
        used.insert(candidate.clone());
        let key = parts.join("\u{1f}");
        assert!(
            self.names.insert(key.clone(), candidate).is_none(),
            "`{key}` was allocated a Go name twice; that is a defect in ess-synth"
        );
    }
}

/// How a resolved type reference is spelled in Go, from inside one package.
impl Layout {
    /// A resolved type reference as a Go type, recording the packages it needs.
    pub fn go_type(
        &self,
        type_ref: &ResolvedTypeRef,
        from: &Package,
        imports: &mut BTreeSet<String>,
    ) -> String {
        match type_ref {
            ResolvedTypeRef::Primitive { name } => self.primitive(*name, from, imports),
            ResolvedTypeRef::Declared { name } => self.reference(name.name(), from, imports),
            // A pointer, because Go has no sum type to spell `Option` with and every other
            // encoding of "absent" is a value the type also uses for something else: the zero
            // string is a legal string, and a zero-length slice is a legal list.
            ResolvedTypeRef::Optional { of } => {
                format!("*{}", self.go_type(of, from, imports))
            }
            ResolvedTypeRef::List { of } => format!("[]{}", self.go_type(of, from, imports)),
            ResolvedTypeRef::Map { key, value } => format!(
                "map[{}]{}",
                self.primitive(*key, from, imports),
                self.go_type(value, from, imports)
            ),
        }
    }

    /// How a declaration is spelled from inside `from` — bare when `from` owns it, qualified and
    /// imported otherwise.
    pub fn reference(
        &self,
        declared: &QualifiedName,
        from: &Package,
        imports: &mut BTreeSet<String>,
    ) -> String {
        let owner = self.package_of(declared);
        let type_name = self.declared(declared);
        if owner.dir == from.dir {
            return type_name.to_owned();
        }
        imports.insert(owner.import.clone());
        format!("{}.{type_name}", owner.name)
    }

    /// The Go representation of one specification primitive.
    ///
    /// Four map onto types that already mean exactly the same thing. The other four have no
    /// standard-library equivalent, and no dependency is taken for them; each gets a wrapper in
    /// the generated `primitives` package, carrying the value in the rendering the published wire
    /// contracts already fix — the same four decisions the Rust emitter takes, because they are
    /// decisions about the *specification's* primitives, not about either language.
    fn primitive(
        &self,
        primitive: Primitive,
        from: &Package,
        imports: &mut BTreeSet<String>,
    ) -> String {
        let wrapper = match primitive {
            Primitive::String => return "string".to_owned(),
            Primitive::Boolean => return "bool".to_owned(),
            Primitive::Integer => return "int64".to_owned(),
            Primitive::Bytes => return "[]byte".to_owned(),
            Primitive::Decimal => "Decimal",
            Primitive::Timestamp => "Timestamp",
            Primitive::Duration => "Duration",
            Primitive::Uuid => "Uuid",
        };
        qualify(&self.primitives, wrapper, from, imports)
    }
}

/// One package identifier per bounded context, collision-free by rule rather than by luck.
///
/// The Rust emitter's module rule, applied to package names: the candidate is the domain's last
/// segment, and when two domains share one, **every** domain switches to its full name minus the
/// system prefix — all of them, not just the colliding pair, so adding one domain cannot silently
/// rename an unrelated package that another package imports.
fn domain_idents(ir: &EssIr) -> Vec<(QualifiedName, String)> {
    let candidates: Vec<(QualifiedName, String)> = ir
        .domains
        .keys()
        .map(|domain| {
            let local = domain
                .segments()
                .last()
                .expect("a qualified name has at least one segment");
            (domain.clone(), name::package_ident(local))
        })
        .collect();
    let distinct: BTreeSet<&String> = candidates.iter().map(|(_, ident)| ident).collect();
    if distinct.len() == candidates.len() {
        return candidates;
    }
    let prefix = ir.system.segments().len();
    ir.domains
        .keys()
        .map(|domain| {
            let segments = domain.segments();
            let full = segments.get(prefix..).unwrap_or(segments).concat();
            (domain.clone(), name::package_ident(&full))
        })
        .collect()
}

/// Moves a package name out of the way of one already taken, by appending a word that says what
/// it is.
fn repair(taken: &mut BTreeSet<String>, mut candidate: String, suffix: &str) -> String {
    while taken.contains(&candidate) {
        candidate.push_str(suffix);
    }
    taken.insert(candidate.clone());
    candidate
}

/// One variant-name fragment per event of a set, collision-free by rule rather than by luck.
///
/// The Rust emitter's rule, and for the same reason: the candidate is the event's own type name,
/// and when two domains of one set declare same-named events, **every** variant switches to the
/// event's full name minus the system prefix.
pub(crate) fn event_variants<'a>(
    ir: &EssIr,
    layout: &Layout,
    events: &BTreeSet<&'a EventHandle>,
) -> BTreeMap<&'a EventHandle, String> {
    let mut candidates: BTreeMap<&EventHandle, String> = events
        .iter()
        .map(|event| (*event, layout.declared(event.name()).to_owned()))
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

/// Every event the system's log can carry: what any component publishes, plus what a delivery the
/// emitter actually writes reacts to and escalates into.
///
/// The same set the system package renders, computed here because the name table has to allocate
/// a variant for each — and two answers to "which events are on the log" would be two spellings
/// of one variant.
fn system_events(
    ir: &EssIr,
    plan: &SynthesisPlan,
    refusals: &TargetRefusals,
) -> BTreeSet<EventHandle> {
    let mut events: BTreeSet<EventHandle> = BTreeSet::new();
    for component in ir.components.values() {
        if refusals.refuses(&Capability {
            kind: crate::plan::CapabilityKind::ComponentPort,
            source: component.name.to_string(),
        }) {
            continue;
        }
        events.extend(component.publishes.iter().cloned());
    }
    for binding in ir.bindings.values() {
        let capability = Capability {
            kind: crate::plan::CapabilityKind::BindingDelivery,
            source: binding.name.to_string(),
        };
        if !plan.is_generated(capability.kind, &capability.source) || refusals.refuses(&capability)
        {
            continue;
        }
        events.insert(binding.event.clone());
        if let Some(escalation) = &binding.escalation {
            events.insert(escalation.clone());
        }
    }
    events
}

/// How any name of one package is spelled from inside another.
///
/// A free function rather than a method: it reads nothing of the layout, only the two packages it
/// is handed, and a method that ignores its receiver invites a reader to look for state that is
/// not there.
pub(crate) fn qualify(
    package: &Package,
    name: &str,
    from: &Package,
    imports: &mut BTreeSet<String>,
) -> String {
    if package.dir == from.dir {
        return name.to_owned();
    }
    imports.insert(package.import.clone());
    format!("{}.{name}", package.name)
}
