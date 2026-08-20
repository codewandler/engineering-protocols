//! Domains: a bounded context, and a statement of what belongs to it.
//!
//! A [`DomainSpec`] lists its entities, commands, events, views, errors and actors **by
//! [`QualifiedName`]**. It does not hold their specifications. That is the load-bearing decision in
//! this module, and every other one follows from it:
//!
//! * A domain stays a **statement of ownership rather than a container**. `billing.invoice` claims
//!   `billing.invoice.CreateInvoice`; it does not carry it.
//! * Because a domain only claims names, a specification can be **split across files** (design §24 —
//!   `system.yaml`, `domains/invoice.yaml`, …) and reassembled into one graph, with the claims from
//!   every file merged and any name claimed twice refused. A container would force every declaration
//!   into the file that holds its domain, which is exactly the decomposition §24 asks for.
//! * "Which domain owns this name?" — the question every later validation, generator and diagnostic
//!   asks — is answerable from the domain list alone, without the declarations being present yet.
//!   See [`SystemSpec::owner_of`](crate::system::SystemSpec::owner_of).
//!
//! The declarations themselves live in their own modules and are assembled by
//! [`SystemSpec`](crate::system::SystemSpec). A domain never imports them, so a change to how a
//! command is written is not a change to what a domain is.
//!
//! ```yaml
//! domain: billing.invoice
//! entities: [Invoice]
//! commands: [CreateInvoice, CancelInvoice]
//! events: [InvoiceCreated]
//! views: [InvoiceById]
//! errors: [InvalidAmount]
//! actors: [Customer]
//! ```
//!
//! A bare name in that document means a member of this domain: `CreateInvoice` is
//! `billing.invoice.CreateInvoice`. A name written out in full is taken literally and checked, which
//! is how `billing.email.SendEmail` is refused here — it belongs to `billing.email`.

use std::collections::{BTreeMap, BTreeSet};

use aep_domain::error::{ValidationCode, ValidationError, ValidationErrors};

use crate::name::{Naming, QualifiedName};
use crate::types::NamedType;

/// What a domain declares a name as.
///
/// Carried so that a refusal can say *what* was declared twice, and so that a later pass can ask a
/// domain for its commands without matching on six fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MemberKind {
    /// A value object local to the domain.
    Type,
    /// Something with identity and a lifecycle.
    Entity,
    /// A requested state change.
    Command,
    /// A fact that happened.
    Event,
    /// An observable projection.
    View,
    /// A named refusal a command may produce.
    Error,
    /// Someone or something commands are invoked on behalf of.
    Actor,
}

impl MemberKind {
    /// Every kind, in declaration order.
    pub const ALL: &'static [Self] = &[
        Self::Type,
        Self::Entity,
        Self::Command,
        Self::Event,
        Self::View,
        Self::Error,
        Self::Actor,
    ];

    /// The kind's name, as it reads in a sentence: `command`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Type => "type",
            Self::Entity => "entity",
            Self::Command => "command",
            Self::Event => "event",
            Self::View => "view",
            Self::Error => "error",
            Self::Actor => "actor",
        }
    }

    /// The document field that carries this kind: `commands`.
    pub fn field(self) -> &'static str {
        match self {
            Self::Type => "types",
            Self::Entity => "entities",
            Self::Command => "commands",
            Self::Event => "events",
            Self::View => "views",
            Self::Error => "errors",
            Self::Actor => "actors",
        }
    }
}

impl std::fmt::Display for MemberKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One bounded context: a namespace, the value objects local to it, and the names it owns.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct DomainSpec {
    /// The domain's namespace — `billing.invoice`. Every name it declares sits inside it.
    pub name: QualifiedName,
    /// Value objects local to this domain, held rather than referenced because nothing else
    /// declares them.
    pub types: Vec<NamedType>,
    /// The entities this domain owns.
    pub entities: Vec<QualifiedName>,
    /// The commands this domain owns.
    pub commands: Vec<QualifiedName>,
    /// The events this domain owns.
    pub events: Vec<QualifiedName>,
    /// The views this domain owns.
    pub views: Vec<QualifiedName>,
    /// The errors this domain's commands may produce.
    pub errors: Vec<QualifiedName>,
    /// The actors this domain owns.
    ///
    /// An actor is a member like any other (design §22): it has a qualified identity, so it needs
    /// an owner for that identity to be inside the system rather than beside it, and its name has
    /// to be claimed so that an actor and a command cannot both answer to it.
    pub actors: Vec<QualifiedName>,
    /// What it is called on the wire, and what a person is shown.
    pub naming: Naming,
}

impl DomainSpec {
    /// A domain that owns nothing yet.
    pub fn new(name: QualifiedName) -> Self {
        Self {
            name,
            types: Vec::new(),
            entities: Vec::new(),
            commands: Vec::new(),
            events: Vec::new(),
            views: Vec::new(),
            errors: Vec::new(),
            actors: Vec::new(),
            naming: Naming::default(),
        }
    }

    /// Every name this domain declares, with what it declares it as.
    ///
    /// Ordering is declaration order within a kind and [`MemberKind::ALL`] order across kinds, so a
    /// diagnostic reads the same on every run.
    pub fn declared(&self) -> impl Iterator<Item = (MemberKind, &QualifiedName)> + '_ {
        self.types
            .iter()
            .map(|declared| (MemberKind::Type, &declared.name))
            .chain(self.entities.iter().map(|name| (MemberKind::Entity, name)))
            .chain(self.commands.iter().map(|name| (MemberKind::Command, name)))
            .chain(self.events.iter().map(|name| (MemberKind::Event, name)))
            .chain(self.views.iter().map(|name| (MemberKind::View, name)))
            .chain(self.errors.iter().map(|name| (MemberKind::Error, name)))
            .chain(self.actors.iter().map(|name| (MemberKind::Actor, name)))
    }

    /// The names declared as one kind. Empty for [`MemberKind::Type`], whose declarations are held
    /// as [`NamedType`] values rather than as references.
    pub fn names(&self, kind: MemberKind) -> &[QualifiedName] {
        match kind {
            MemberKind::Type => &[],
            MemberKind::Entity => &self.entities,
            MemberKind::Command => &self.commands,
            MemberKind::Event => &self.events,
            MemberKind::View => &self.views,
            MemberKind::Error => &self.errors,
            MemberKind::Actor => &self.actors,
        }
    }

    /// What this domain declares `name` as, if it declares it at all.
    pub fn kind_of(&self, name: &QualifiedName) -> Option<MemberKind> {
        self.declared()
            .find(|(_, declared)| *declared == name)
            .map(|(kind, _)| kind)
    }

    /// `true` when this domain declares `name`.
    pub fn declares(&self, name: &QualifiedName) -> bool {
        self.kind_of(name).is_some()
    }

    /// `true` when `name` sits in this domain's namespace.
    ///
    /// Containment is not ownership: a name may sit inside the namespace and be declared by nobody.
    pub fn contains(&self, name: &QualifiedName) -> bool {
        name.is_within(&self.name)
    }

    /// Checks what one domain can see on its own.
    ///
    /// Two things: every declared name sits inside the domain's own namespace, and no name is
    /// declared twice. Whether some *other* domain also declares it is not a question a domain can
    /// answer — see [`DomainSpec::validate_all`].
    pub fn validate(&self) -> ValidationErrors {
        let mut errors = ValidationErrors::new();
        let mut seen: BTreeMap<&QualifiedName, MemberKind> = BTreeMap::new();

        for (kind, name) in self.declared() {
            if !self.contains(name) {
                errors.push(
                    // Not `UndeclaredReference`: the name *is* declared, right here. What cannot
                    // hold is the pair — this domain's namespace and that name — so a tool reading
                    // the code is told to move the declaration, not to hunt for a missing one.
                    ValidationError::new(
                        ValidationCode::ConflictingDeclaration,
                        format!("domain {}.{}", self.name, kind.field()),
                        format!(
                            "`{name}` is not inside `{}`, so this domain cannot declare it",
                            self.name
                        ),
                    )
                    .with_hint(match name.namespace() {
                        Some(owner) => format!(
                            "`{name}` belongs to `{owner}`; declare it there, or rename it \
                             `{}.{}`",
                            self.name,
                            name.local()
                        ),
                        None => format!(
                            "a domain declares only names in its own namespace; write \
                             `{}.{name}`",
                            self.name
                        ),
                    }),
                );
            }

            if let Some(first) = seen.insert(name, kind) {
                errors.push(ValidationError::new(
                    ValidationCode::DuplicateDeclaration,
                    format!("domain {}.{}", self.name, kind.field()),
                    format!(
                        "`{name}` is declared twice in `{}`, as a {first} and as a {kind}",
                        self.name
                    ),
                ));
            }
        }

        errors
    }

    /// Checks a set of domains against each other.
    ///
    /// Each domain's own [`validate`](DomainSpec::validate), plus the two things only a set can see:
    ///
    /// * **A name declared by two domains.** Ownership is the basis of every later resolution step;
    ///   two owners means the question has two answers and every generator picks its own.
    /// * **A domain nested inside another domain.** `billing.invoice` and `billing.invoice.tax`
    ///   both contain `billing.invoice.tax.Rate`, so ownership would be decided by iteration order.
    ///
    /// Domains are compared in the order given, which is the order they were declared in.
    pub fn validate_all(domains: &[Self]) -> ValidationErrors {
        let mut errors = ValidationErrors::new();
        let mut owner: BTreeMap<&QualifiedName, (&QualifiedName, MemberKind)> = BTreeMap::new();
        let mut declared_domains: BTreeSet<&QualifiedName> = BTreeSet::new();

        for domain in domains {
            errors.extend(domain.validate());

            if !declared_domains.insert(&domain.name) {
                errors.push(ValidationError::new(
                    ValidationCode::DuplicateDeclaration,
                    format!("domain {}", domain.name),
                    format!("`{}` is declared more than once", domain.name),
                ));
            }

            for other in domains {
                if other.name != domain.name && domain.contains(&other.name) {
                    errors.push(
                        // Two different names, neither declared twice: `ConflictingDeclaration` is
                        // what "each is well formed and they cannot both hold" is called.
                        ValidationError::new(
                            ValidationCode::ConflictingDeclaration,
                            format!("domain {}", other.name),
                            format!(
                                "`{}` sits inside `{}`, so a name under it would have two owners",
                                other.name, domain.name
                            ),
                        )
                        .with_hint("domains do not nest; give it a namespace of its own"),
                    );
                }
            }

            for (kind, name) in domain.declared() {
                if let Some((first, first_kind)) = owner.insert(name, (&domain.name, kind)) {
                    errors.push(
                        ValidationError::new(
                            ValidationCode::DuplicateDeclaration,
                            format!("domain {}.{}", domain.name, kind.field()),
                            format!(
                                "`{name}` is declared by `{first}` as a {first_kind} and by `{}` \
                                 as a {kind}",
                                domain.name
                            ),
                        )
                        .with_hint("a name has exactly one owning domain"),
                    );
                }
            }
        }

        errors
    }
}

/// A domain document, as parsed.
///
/// The member lists carry names, not declarations: see the module documentation for why. A
/// single-segment name is read as a member of this domain, so `CreateInvoice` in `billing.invoice`
/// means `billing.invoice.CreateInvoice`; anything already qualified is taken at face value and
/// checked against the namespace.
#[derive(Debug, Clone, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RawDomainSpec {
    /// The domain's namespace, written in full: `billing.invoice`.
    #[serde(alias = "domain")]
    pub name: QualifiedName,
    /// Value objects local to this domain.
    #[serde(default)]
    pub types: Vec<crate::types::RawNamedType>,
    /// The entities it owns.
    #[serde(default)]
    pub entities: Vec<QualifiedName>,
    /// The commands it owns.
    #[serde(default)]
    pub commands: Vec<QualifiedName>,
    /// The events it owns.
    #[serde(default)]
    pub events: Vec<QualifiedName>,
    /// The views it owns.
    #[serde(default)]
    pub views: Vec<QualifiedName>,
    /// The errors its commands may produce.
    #[serde(default)]
    pub errors: Vec<QualifiedName>,
    /// The actors it owns.
    #[serde(default)]
    pub actors: Vec<QualifiedName>,
    /// What it is called on the wire, and what a person is shown.
    #[serde(default)]
    pub naming: Naming,
}

/// Reads a single-segment name as a member of `namespace`.
fn qualify(namespace: &QualifiedName, name: QualifiedName) -> QualifiedName {
    if name.segments().len() == 1 {
        namespace.child(name.local())
    } else {
        name
    }
}

impl TryFrom<RawDomainSpec> for DomainSpec {
    type Error = ValidationErrors;

    fn try_from(raw: RawDomainSpec) -> Result<Self, Self::Error> {
        let namespace = raw.name.clone();
        let qualify_all = |names: Vec<QualifiedName>| {
            names
                .into_iter()
                .map(|name| qualify(&namespace, name))
                .collect()
        };

        // A type that does not convert is left out, so the domain's own rules still run and report
        // what they can. Reporting "the domain declares no `Money`" on top of "`Money` is malformed"
        // would be two errors for one mistake.
        let mut type_errors = ValidationErrors::new();
        let mut types = Vec::with_capacity(raw.types.len());
        for declared in raw.types {
            match NamedType::try_from(declared) {
                Ok(mut declared) => {
                    declared.name = qualify(&namespace, declared.name);
                    types.push(declared);
                }
                Err(reported) => type_errors.extend(reported),
            }
        }

        let domain = Self {
            types,
            entities: qualify_all(raw.entities),
            commands: qualify_all(raw.commands),
            events: qualify_all(raw.events),
            views: qualify_all(raw.views),
            errors: qualify_all(raw.errors),
            actors: qualify_all(raw.actors),
            naming: raw.naming,
            name: raw.name,
        };

        let mut errors = type_errors;
        errors.extend(domain.validate());
        errors.into_result(domain)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Primitive, TypeBody, TypeRef};

    fn name(value: &str) -> QualifiedName {
        QualifiedName::new(value).expect("a valid name")
    }

    fn invoice() -> DomainSpec {
        DomainSpec {
            name: name("billing.invoice"),
            types: Vec::new(),
            entities: vec![name("billing.invoice.Invoice")],
            commands: vec![name("billing.invoice.CreateInvoice")],
            events: vec![name("billing.invoice.InvoiceCreated")],
            views: vec![name("billing.invoice.InvoiceById")],
            errors: vec![name("billing.invoice.InvalidAmount")],
            actors: vec![name("billing.invoice.Clerk")],
            naming: Naming::default(),
        }
    }

    fn money() -> NamedType {
        NamedType {
            name: name("billing.invoice.Money"),
            body: TypeBody::Newtype {
                of: TypeRef::Primitive(Primitive::Decimal),
                invariants: vec![
                    crate::entity::Invariant::parse("value >= 0").expect("a predicate")
                ],
            },
            naming: Naming::default(),
        }
    }

    #[test]
    fn a_domain_owns_names_rather_than_holding_declarations() {
        let domain = invoice();
        assert!(domain.validate().is_empty());
        assert_eq!(domain.declared().count(), 6);
        assert_eq!(
            domain.kind_of(&name("billing.invoice.CreateInvoice")),
            Some(MemberKind::Command)
        );
        assert_eq!(
            domain.kind_of(&name("billing.invoice.Clerk")),
            Some(MemberKind::Actor),
            "an actor is a member of the domain that declares it, like everything else"
        );
        assert!(domain.declares(&name("billing.invoice.InvoiceById")));
        assert!(
            !domain.declares(&name("billing.invoice.Nothing")),
            "containment is not ownership"
        );
        assert!(
            domain.contains(&name("billing.invoice.Nothing")),
            "but the namespace still contains it"
        );
        assert_eq!(
            domain.names(MemberKind::Event),
            &[name("billing.invoice.InvoiceCreated")]
        );
    }

    #[test]
    fn a_member_outside_the_domains_namespace_is_refused() {
        let mut domain = invoice();
        domain.commands.push(name("billing.email.SendEmail"));

        let errors = domain.validate();
        assert_eq!(errors.len(), 1);
        let error = &errors.as_slice()[0];
        assert_eq!(
            error.code,
            ValidationCode::ConflictingDeclaration,
            "the name is declared; what is wrong is where: {error}"
        );
        assert_eq!(error.location, "domain billing.invoice.commands");
        assert!(
            error.message.contains("billing.email.SendEmail")
                && error.message.contains("billing.invoice"),
            "the refusal names the member and the domain: {error}"
        );
        assert!(
            error
                .hint
                .as_deref()
                .expect("a hint")
                .contains("belongs to `billing.email`"),
            "and says where it does belong: {error}"
        );
    }

    #[test]
    fn a_domain_local_type_is_owned_by_that_domain() {
        let mut domain = invoice();
        domain.types.push(money());
        assert!(domain.validate().is_empty());

        let mut stranger = invoice();
        let mut foreign = money();
        foreign.name = name("billing.email.Money");
        stranger.types.push(foreign);

        let errors = stranger.validate();
        assert!(
            errors.contains(ValidationCode::ConflictingDeclaration),
            "{errors}"
        );
        assert_eq!(
            errors.as_slice()[0].location,
            "domain billing.invoice.types"
        );
    }

    #[test]
    fn a_name_declared_twice_in_one_domain_is_refused() {
        let mut domain = invoice();
        domain.events.push(name("billing.invoice.CreateInvoice"));

        let errors = domain.validate();
        assert_eq!(errors.len(), 1);
        assert!(
            errors.contains(ValidationCode::DuplicateDeclaration),
            "{errors}"
        );
        assert!(
            errors.to_string().contains("as a command and as a event"),
            "the refusal says what it was declared as, twice: {errors}"
        );
    }

    #[test]
    fn a_name_declared_in_two_domains_is_refused() {
        let email = DomainSpec {
            name: name("billing.email"),
            commands: vec![name("billing.invoice.CreateInvoice")],
            ..DomainSpec::new(name("billing.email"))
        };

        let errors = DomainSpec::validate_all(&[invoice(), email]);
        assert!(
            errors.contains(ValidationCode::DuplicateDeclaration),
            "{errors}"
        );
        let rendered = errors.to_string();
        assert!(
            rendered.contains("declared by `billing.invoice`")
                && rendered.contains("by `billing.email`"),
            "the refusal names both claimants: {rendered}"
        );
        assert!(
            errors.contains(ValidationCode::ConflictingDeclaration),
            "and the claim is also outside the claimant's namespace: {errors}"
        );
    }

    #[test]
    fn a_domain_nested_inside_another_is_refused_as_a_conflict_and_not_as_a_repeat() {
        let tax = DomainSpec::new(name("billing.invoice.tax"));
        let errors = DomainSpec::validate_all(&[invoice(), tax]);

        assert!(
            errors.contains(ValidationCode::ConflictingDeclaration),
            "two different names, neither declared twice: {errors}"
        );
        assert!(
            !errors.contains(ValidationCode::DuplicateDeclaration),
            "nothing here is declared more than once: {errors}"
        );
        assert!(
            errors.to_string().contains("would have two owners"),
            "{errors}"
        );
    }

    #[test]
    fn the_same_domain_declared_twice_is_refused() {
        let errors = DomainSpec::validate_all(&[invoice(), invoice()]);
        assert!(
            errors.contains(ValidationCode::DuplicateDeclaration),
            "{errors}"
        );
        assert!(
            errors
                .to_string()
                .contains("`billing.invoice` is declared more than once"),
            "{errors}"
        );
    }

    #[test]
    fn a_bare_member_name_means_a_member_of_this_domain() {
        let raw: RawDomainSpec = serde_yaml::from_str(
            r"
domain: billing.invoice
commands: [CreateInvoice]
events: [InvoiceCreated]
types:
  - name: Money
    kind: newtype
    of: Decimal
",
        )
        .expect("document parses");
        let domain = DomainSpec::try_from(raw).expect("validates");

        assert_eq!(
            domain.commands,
            vec![name("billing.invoice.CreateInvoice")],
            "a bare name is a member of the domain it is written in"
        );
        assert_eq!(domain.types[0].name, name("billing.invoice.Money"));
        assert!(domain.declares(&name("billing.invoice.InvoiceCreated")));
    }

    #[test]
    fn a_qualified_member_from_another_domain_is_refused_when_the_document_is_read() {
        let raw: RawDomainSpec = serde_yaml::from_str(
            r"
domain: billing.invoice
commands: [billing.email.SendEmail]
",
        )
        .expect("document parses");

        let errors = DomainSpec::try_from(raw).expect_err("not this domain's command");
        assert!(
            errors.contains(ValidationCode::ConflictingDeclaration),
            "{errors}"
        );
    }

    #[test]
    fn a_bare_actor_name_is_qualified_and_owned_like_every_other_member() {
        let raw: RawDomainSpec = serde_yaml::from_str(
            r"
domain: billing.invoice
actors: [Clerk]
",
        )
        .expect("document parses");
        let domain = DomainSpec::try_from(raw).expect("validates");

        assert_eq!(domain.actors, vec![name("billing.invoice.Clerk")]);
        assert!(domain.declares(&name("billing.invoice.Clerk")));
    }

    #[test]
    fn an_actor_outside_the_domains_namespace_is_refused() {
        let mut domain = invoice();
        domain.actors.push(name("evil.Hacker"));

        let errors = domain.validate();
        assert_eq!(errors.len(), 1, "{errors}");
        let error = &errors.as_slice()[0];
        assert_eq!(error.code, ValidationCode::ConflictingDeclaration);
        assert_eq!(error.location, "domain billing.invoice.actors");
    }

    #[test]
    fn an_actor_and_a_command_may_not_share_a_name() {
        let mut domain = invoice();
        domain.actors.push(name("billing.invoice.CreateInvoice"));

        let errors = domain.validate();
        assert_eq!(errors.len(), 1, "{errors}");
        assert_eq!(
            errors.as_slice()[0].code,
            ValidationCode::DuplicateDeclaration
        );
        assert!(
            errors.to_string().contains("as a command and as a actor"),
            "{errors}"
        );
    }

    #[test]
    fn every_broken_member_is_reported_not_only_the_first() {
        let mut domain = invoice();
        domain.entities.push(name("billing.email.Message"));
        domain.views.push(name("reporting.Ledger"));
        domain.errors.push(name("billing.invoice.CreateInvoice"));

        let errors = domain.validate();
        assert_eq!(
            errors.len(),
            3,
            "two foreign names and one duplicate, in one run: {errors}"
        );
    }
}
