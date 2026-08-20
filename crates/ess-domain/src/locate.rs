//! Addressing a declaration from outside the specification.
//!
//! A specification names things three ways, and conflating any two of them costs a rename later
//! (design F5):
//!
//! | name | example | who uses it |
//! |---|---|---|
//! | qualified name | `billing.invoice.CreateInvoice` | the specification, and only it |
//! | wire name | `create-invoice` | HTTP paths, topics, generated JSON |
//! | locator | `ep://acme/billing/ess-command/billing.invoice.CreateInvoice` | anything outside |
//!
//! The locator reuses the protocol's [`EntityLocator`] rather than introducing an `ess://` scheme.
//! One scheme means an approval, a review or an audit entry can address a command in a
//! specification the same way it addresses a design document — which is the point of the join: a
//! task can require conformance to a specification, and the evidence has to be able to say *to
//! which declaration*.

use aep_domain::entity::{EntityLocator, EntityType};
use aep_domain::error::ParseError;

use crate::name::QualifiedName;

/// What sort of declaration a locator addresses.
///
/// The kinds are flat rather than nested under the domain, because a locator is resolved by
/// something that has not parsed the specification: `ess-command` is answerable without knowing
/// that `billing.invoice` is a domain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DeclarationKind {
    /// A whole system.
    System,
    /// A bounded context.
    Domain,
    /// A named type.
    Type,
    /// An entity.
    Entity,
    /// A command.
    Command,
    /// An event.
    Event,
    /// A declared error.
    Error,
    /// A view.
    View,
    /// An actor.
    Actor,
}

impl DeclarationKind {
    /// Every kind, for exhaustive tests and for listing what a locator may address.
    pub const ALL: [Self; 9] = [
        Self::System,
        Self::Domain,
        Self::Type,
        Self::Entity,
        Self::Command,
        Self::Event,
        Self::Error,
        Self::View,
        Self::Actor,
    ];

    /// The wire spelling, used as the locator's kind segment and as the entity type's name.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::System => "ess-system",
            Self::Domain => "ess-domain",
            Self::Type => "ess-type",
            Self::Entity => "ess-entity",
            Self::Command => "ess-command",
            Self::Event => "ess-event",
            Self::Error => "ess-error",
            Self::View => "ess-view",
            Self::Actor => "ess-actor",
        }
    }

    /// Parses the wire spelling.
    pub fn parse(value: &str) -> Result<Self, ParseError> {
        Self::ALL
            .into_iter()
            .find(|kind| kind.as_str() == value)
            .ok_or_else(|| {
                ParseError::identifier(
                    "declaration kind",
                    value,
                    format!(
                        "expected one of {}",
                        Self::ALL
                            .iter()
                            .map(|kind| format!("`{}`", kind.as_str()))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                )
            })
    }

    /// The versioned entity type, so a declaration can be stored as a protocol entity.
    ///
    /// Namespaced `ess` rather than `aep`: a specification's declarations are not protocol
    /// documents, and a backend that stores both must be able to tell them apart without reading
    /// the payload.
    pub fn entity_type(self) -> EntityType {
        EntityType::new("ess", self.as_str(), 1)
            .unwrap_or_else(|error| panic!("declaration kinds are valid type names: {error}"))
    }
}

impl std::fmt::Display for DeclarationKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Where a specification's declarations live, for something outside the specification.
///
/// The organisation and space are not in the specification: the same `billing` specification
/// vendored into two organisations is two sets of entities, and nothing in the source can know
/// which. Whoever loads the specification supplies them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecLocation {
    organisation: String,
    space: String,
}

impl SpecLocation {
    /// Names where a specification is being resolved.
    ///
    /// Validation is deferred to [`EntityLocator::new`], which owns the character rules — a second
    /// copy of them here would be a second thing to keep in step.
    pub fn new(organisation: impl Into<String>, space: impl Into<String>) -> Self {
        Self {
            organisation: organisation.into(),
            space: space.into(),
        }
    }

    /// The organisation.
    pub fn organisation(&self) -> &str {
        &self.organisation
    }

    /// The space.
    pub fn space(&self) -> &str {
        &self.space
    }

    /// The locator for one declaration.
    ///
    /// Fails when the organisation or space is empty or contains a character a locator segment may
    /// not hold. A qualified name is already restricted to those characters, so the name itself
    /// cannot be the reason.
    pub fn locate(
        &self,
        kind: DeclarationKind,
        name: &QualifiedName,
    ) -> Result<EntityLocator, ParseError> {
        EntityLocator::new(
            &self.organisation,
            &self.space,
            kind.as_str(),
            name.to_string(),
        )
    }
}

/// Reads a locator back into the kind and name it addresses.
///
/// Round-tripping matters because the locator is what crosses the boundary: an approval recorded
/// against `ep://acme/billing/ess-command/billing.invoice.CreateInvoice` has to be resolvable back
/// to a declaration when the specification is next loaded, or the approval is unattributable.
pub fn resolve(locator: &EntityLocator) -> Result<(DeclarationKind, QualifiedName), ParseError> {
    let kind = DeclarationKind::parse(locator.kind())?;
    let name = QualifiedName::new(locator.key())?;
    Ok((kind, name))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn name(value: &str) -> QualifiedName {
        QualifiedName::new(value).expect("a valid name")
    }

    #[test]
    fn a_declaration_is_addressed_with_the_protocols_own_scheme() {
        let location = SpecLocation::new("acme", "billing");
        let locator = location
            .locate(
                DeclarationKind::Command,
                &name("billing.invoice.CreateInvoice"),
            )
            .expect("a valid locator");
        assert_eq!(
            locator.to_string(),
            "ep://acme/billing/ess-command/billing.invoice.CreateInvoice"
        );
    }

    #[test]
    fn a_locator_resolves_back_to_what_it_addresses() {
        let location = SpecLocation::new("acme", "billing");
        for kind in DeclarationKind::ALL {
            let original = name("billing.invoice.Thing");
            let locator = location.locate(kind, &original).expect("a valid locator");
            let parsed = aep_domain::entity::EntityLocator::parse(&locator.to_string())
                .expect("round trips through text");
            let (resolved_kind, resolved_name) = resolve(&parsed).expect("resolves");
            assert_eq!(resolved_kind, kind);
            assert_eq!(resolved_name, original);
        }
    }

    #[test]
    fn the_same_specification_in_two_organisations_is_two_sets_of_entities() {
        // The point of keeping organisation and space out of the source: a vendored specification
        // must not collide with the vendor's own.
        let ours = SpecLocation::new("acme", "billing");
        let theirs = SpecLocation::new("globex", "billing");
        let subject = name("billing.invoice.CreateInvoice");
        assert_ne!(
            ours.locate(DeclarationKind::Command, &subject)
                .expect("valid"),
            theirs
                .locate(DeclarationKind::Command, &subject)
                .expect("valid")
        );
    }

    #[test]
    fn an_empty_organisation_is_refused_rather_than_producing_a_broken_locator() {
        let location = SpecLocation::new("", "billing");
        let error = location
            .locate(DeclarationKind::Entity, &name("billing.invoice.Invoice"))
            .expect_err("an empty organisation cannot be addressed");
        assert!(error.to_string().contains("organisation"), "{error}");
    }

    #[test]
    fn every_kind_has_a_distinct_spelling_and_a_valid_entity_type() {
        let mut seen = std::collections::BTreeSet::new();
        for kind in DeclarationKind::ALL {
            assert!(seen.insert(kind.as_str()), "{kind} is spelt twice");
            let entity_type = kind.entity_type();
            assert_eq!(entity_type.to_string(), format!("ess.{}/v1", kind.as_str()));
            assert_eq!(DeclarationKind::parse(kind.as_str()).expect("parses"), kind);
        }
    }

    #[test]
    fn an_unknown_kind_is_refused_with_the_known_ones_listed() {
        let error = DeclarationKind::parse("ess-widget").expect_err("not a kind");
        let rendered = error.to_string();
        assert!(rendered.contains("ess-command"), "{rendered}");
    }
}
