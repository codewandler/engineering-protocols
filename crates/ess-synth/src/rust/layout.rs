//! Where every declaration lands in the generated workspace, decided once.
//!
//! The plan names an item, the generator declares it, and a later wave's port crate imports it —
//! three readers of one decision. So the decision is a value ([`Layout`]) computed one way from the
//! IR, rather than a convention each renderer re-implements. It is deterministic because its inputs
//! are: [`EssIr`]'s collections are ordered, and nothing here reads anything else.

use std::collections::BTreeMap;

use ess_compiler::ir::{EssIr, ResolvedTypeRef};
use ess_domain::name::QualifiedName;
use ess_domain::types::Primitive;

use super::name;

/// The shape of the generated workspace: one types crate, one module per bounded context.
///
/// One crate rather than one per domain, deliberately: the domains of one system reference each
/// other's types — the billing example's declared conversion crosses two of them — and modules make
/// that a `crate::` path where separate crates would make it a dependency graph this slice has no
/// reason to invent. Component crates arrive with the ports, in the next slice.
pub struct Layout {
    /// The package name of the types crate — `billing-types`.
    package: String,
    /// Module identifier per bounded context, keyed by the domain's qualified name.
    modules: BTreeMap<QualifiedName, String>,
    /// The bounded context that owns each declaration.
    owners: BTreeMap<QualifiedName, QualifiedName>,
}

impl Layout {
    /// Derives the layout of a resolved specification.
    pub fn of(ir: &EssIr) -> Self {
        let package = format!("{}-types", ir.system.segments().join("-"));

        let modules = module_idents(ir);
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
        Self {
            package,
            modules,
            owners,
        }
    }

    /// The package name of the types crate.
    pub fn package(&self) -> &str {
        &self.package
    }

    /// Every bounded context and its module identifier, in name order.
    pub fn modules(&self) -> impl Iterator<Item = (&QualifiedName, &str)> {
        self.modules
            .iter()
            .map(|(domain, module)| (domain, module.as_str()))
    }

    /// The bounded context that owns a declaration.
    ///
    /// Total for the same reason the IR's accessors are: every declaration the IR carries sits in
    /// exactly one domain's roster, so a miss is a name from a different compilation, which is a
    /// programming mistake and not a specification's problem.
    pub fn owner(&self, declared: &QualifiedName) -> &QualifiedName {
        self.owners.get(declared).unwrap_or_else(|| {
            panic!("`{declared}` is not a declaration this layout knows: it was derived from a different IR")
        })
    }

    /// The module identifier of a bounded context.
    pub fn module(&self, domain: &QualifiedName) -> &str {
        self.modules.get(domain).map_or_else(
            || panic!("`{domain}` is not a domain this layout knows: it was derived from a different IR"),
            String::as_str,
        )
    }

    /// The source file a bounded context's declarations are generated into.
    pub fn module_path(&self, domain: &QualifiedName) -> String {
        format!("crates/{}/src/{}.rs", self.package, self.module(domain))
    }

    /// The Rust type name of a declaration, unqualified.
    pub fn type_name(&self, declared: &QualifiedName) -> String {
        name::type_name(declared, self.owner(declared).segments().len())
    }

    /// How a declaration is spelled from inside `from` — bare when `from` owns it, `crate::`
    /// otherwise.
    pub fn reference(&self, declared: &QualifiedName, from: &QualifiedName) -> String {
        let owner = self.owner(declared);
        let type_name = self.type_name(declared);
        if owner == from {
            type_name
        } else {
            format!("crate::{}::{type_name}", self.module(owner))
        }
    }

    /// A resolved type reference as a Rust type, from inside the module of `from`.
    pub fn rust_type(&self, type_ref: &ResolvedTypeRef, from: &QualifiedName) -> String {
        match type_ref {
            ResolvedTypeRef::Primitive { name } => primitive(*name).to_owned(),
            ResolvedTypeRef::Declared { name } => self.reference(name.name(), from),
            ResolvedTypeRef::Optional { of } => format!("Option<{}>", self.rust_type(of, from)),
            ResolvedTypeRef::List { of } => format!("Vec<{}>", self.rust_type(of, from)),
            ResolvedTypeRef::Map { key, value } => format!(
                "std::collections::BTreeMap<{}, {}>",
                primitive(*key),
                self.rust_type(value, from)
            ),
        }
    }
}

/// One module identifier per bounded context, collision-free by rule rather than by luck.
///
/// The candidate is the domain's last segment (`billing.invoice` → `invoice`). Two rules repair
/// the two ways that can collide, both deterministic because they depend only on the whole set:
///
/// 1. When two domains share a last segment, **every** domain switches to its full name joined
///    with underscores — all of them, not just the colliding pair, so adding one domain cannot
///    silently rename an unrelated module's path in generated output that another crate imports.
/// 2. A module that would be spelled `primitives` gets `_domain` appended, because that name is
///    reserved for the representation module every generated crate carries.
fn module_idents(ir: &EssIr) -> BTreeMap<QualifiedName, String> {
    let mut candidates: BTreeMap<QualifiedName, String> = ir
        .domains
        .keys()
        .map(|domain| {
            let local = domain
                .segments()
                .last()
                .expect("a qualified name has at least one segment");
            (domain.clone(), name::value_ident(local))
        })
        .collect();

    let distinct: std::collections::BTreeSet<&String> = candidates.values().collect();
    if distinct.len() != candidates.len() {
        // Minus the system prefix, which every domain carries (`ess-domain` refuses one outside
        // the system's namespace): `duo.alpha.pay` and `duo.beta.pay` become `alpha_pay` and
        // `beta_pay`, not two modules that both start with the one word every module would share.
        let prefix = ir.system.segments().len();
        candidates = ir
            .domains
            .keys()
            .map(|domain| {
                let segments = domain.segments();
                let full = segments.get(prefix..).unwrap_or(segments).join("_");
                (domain.clone(), name::value_ident(&full))
            })
            .collect();
    }

    for module in candidates.values_mut() {
        if module == "primitives" {
            module.push_str("_domain");
        }
    }
    candidates
}

/// The Rust representation of each specification primitive.
///
/// Four map onto types that already mean exactly the same thing. The other four have no `std`
/// equivalent, and taking a dependency for them would put a third-party type on every generated
/// signature; each gets a transparent wrapper in the generated crate's own `primitives` module,
/// carrying the value in the rendering the published wire contracts already fix — the JSON Schema
/// projection writes `Decimal` as a decimal string, `Timestamp` as `date-time`, `Duration` as an
/// ISO 8601 duration and `Uuid` as a UUID string, and two projections of one model must not
/// disagree about what a value looks like.
pub fn primitive(name: Primitive) -> &'static str {
    match name {
        Primitive::String => "String",
        Primitive::Boolean => "bool",
        Primitive::Integer => "i64",
        Primitive::Bytes => "Vec<u8>",
        Primitive::Decimal => "crate::primitives::Decimal",
        Primitive::Timestamp => "crate::primitives::Timestamp",
        Primitive::Duration => "crate::primitives::Duration",
        Primitive::Uuid => "crate::primitives::Uuid",
    }
}
