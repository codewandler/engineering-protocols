//! Who may ask the system for what (§4.2).
//!
//! ```yaml
//! actors:
//!   Customer:
//!     may:
//!       - billing.invoice.CreateInvoice
//!   FinanceOperator:
//!     may:
//!       - billing.invoice.CancelInvoice
//!       - billing.invoice.MarkInvoicePaid
//! ```
//!
//! # These are semantic authorization concepts, not one authorization technology
//!
//! An [`ActorSpec`] says *who* may invoke *which command*, and nothing else. It carries no scopes,
//! no roles table, no JWT claim, no policy language and no HTTP method, because each of those is a
//! projection of this statement into one technology, and a model that names one of them has chosen
//! it for every consumer of the specification. A generator may compile `may` into an RBAC role
//! binding, a permission matrix, an `OpenAPI` security requirement or a test that asserts a refusal —
//! and the same model has to be able to produce all four, which it cannot do if it was written in
//! the vocabulary of one of them.
//!
//! # There is no `RoleSpec`
//!
//! §4.2 is titled "actors and roles" but only ever declares actors. A role would be a named set of
//! command grants that an actor then holds — which is what an actor already is, so at v0.1 the two
//! concepts would have one definition and one meaning, and every specification would have to choose
//! arbitrarily between them. The place a role earns its keep is *inside a deployment*: RBAC groups,
//! a permissions table, an org chart. That is the generator's target, not the domain's model. When
//! actors are found to be sharing large identical grant sets in a real specification, a role becomes
//! a named grant set that actors include, and it can be added then, with the evidence that made it
//! necessary — rather than shipped now as an empty concept nobody knows when to reach for.

use std::collections::BTreeSet;

use aep_domain::error::{ValidationCode, ValidationError, ValidationErrors};

use crate::name::{Naming, QualifiedName};

/// Someone or something that interacts with the system.
///
/// A human role at a keyboard, another system, a scheduled job: what makes it an actor is that
/// commands are invoked *on its behalf*, so its grants are what an authorization check has to
/// answer against.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ActorSpec {
    /// Its stable logical identity, such as `billing.Customer`.
    pub name: QualifiedName,
    /// The commands it may invoke, by qualified command name.
    ///
    /// A set rather than a list: the same grant written twice means the same thing, and iteration
    /// order has to be stable for anything generated from it to be diffable.
    #[serde(skip_serializing_if = "BTreeSet::is_empty")]
    pub may: BTreeSet<QualifiedName>,
    /// What it is called on the wire and shown as.
    #[serde(skip_serializing_if = "Naming::is_empty")]
    pub naming: Naming,
}

impl ActorSpec {
    /// `true` when this actor may invoke `command`.
    ///
    /// Matched on the qualified name, so renaming a command's wire form — the HTTP path, the topic
    /// — cannot silently move a permission (see [`crate::name`]).
    pub fn may_invoke(&self, command: &QualifiedName) -> bool {
        self.may.contains(command)
    }

    /// Checks every grant against the commands the specification declares.
    ///
    /// A grant naming a command nobody declared is the failure worth catching: it reads as an
    /// authorization decision and grants nothing. Nothing refuses a request because of it, no
    /// generated permission matrix has a row for it, and the specification looks like it says
    /// something it does not. It is usually a misspelling or a command that was renamed on one side
    /// only.
    ///
    /// Every bad grant is reported, not the first, so one pass fixes the document.
    pub fn validate(&self, commands: &BTreeSet<QualifiedName>) -> ValidationErrors {
        let mut errors = ValidationErrors::new();
        for granted in &self.may {
            if !commands.contains(granted) {
                errors.push(
                    // Not `UndeclaredCapability`: that code is about a capability from the
                    // protocol half's fixed vocabulary, and a grant here names a command this
                    // specification declares. The fault is a reference with nothing behind it.
                    ValidationError::new(
                        ValidationCode::UndeclaredReference,
                        format!("actor {}.may", self.name),
                        format!(
                            "`{}` may invoke `{granted}`, which no domain declares as a command",
                            self.name
                        ),
                    )
                    .with_hint(format!(
                        "declared commands: {}",
                        if commands.is_empty() {
                            "none".to_owned()
                        } else {
                            commands
                                .iter()
                                .map(ToString::to_string)
                                .collect::<Vec<_>>()
                                .join(", ")
                        }
                    )),
                );
            }
        }
        errors
    }
}

/// An actor, as parsed.
#[derive(Debug, Clone, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RawActorSpec {
    /// Its stable logical identity.
    pub name: QualifiedName,
    /// The commands it may invoke.
    ///
    /// Absent means it may invoke none, which is a legitimate thing to say: an actor that only
    /// observes still belongs in the model, because "who is in this picture" is part of what the
    /// specification describes.
    #[serde(default)]
    pub may: BTreeSet<QualifiedName>,
    /// What it is called on the wire and shown as.
    #[serde(default)]
    pub naming: Naming,
}

impl TryFrom<RawActorSpec> for ActorSpec {
    type Error = ValidationErrors;

    /// Nothing here can be wrong on its own.
    ///
    /// An actor is a name and a set of references, and both are settled by the parser. Whether the
    /// references point at anything needs the command vocabulary, which arrives with the rest of
    /// the specification — [`ActorSpec::validate`]. The conversion exists anyway, because an
    /// [`ActorSpec`] must not be reachable except through it.
    fn try_from(raw: RawActorSpec) -> Result<Self, Self::Error> {
        Ok(Self {
            name: raw.name,
            may: raw.may,
            naming: raw.naming,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn actor(yaml: &str) -> ActorSpec {
        let raw: RawActorSpec = serde_yaml::from_str(yaml).expect("the document is well formed");
        ActorSpec::try_from(raw).expect("an actor is valid on its own")
    }

    fn name(value: &str) -> QualifiedName {
        QualifiedName::new(value).expect("a valid name")
    }

    /// §4.2's command vocabulary.
    fn commands() -> BTreeSet<QualifiedName> {
        [
            "billing.invoice.CreateInvoice",
            "billing.invoice.CancelInvoice",
            "billing.invoice.MarkInvoicePaid",
        ]
        .into_iter()
        .map(name)
        .collect()
    }

    #[test]
    fn the_actors_from_the_design_document_validate() {
        let customer = actor(
            "\
name: billing.Customer
may:
  - billing.invoice.CreateInvoice
",
        );
        let operator = actor(
            "\
name: billing.FinanceOperator
may:
  - billing.invoice.CancelInvoice
  - billing.invoice.MarkInvoicePaid
",
        );

        for spec in [&customer, &operator] {
            let errors = spec.validate(&commands());
            assert!(errors.is_empty(), "{errors}");
        }
        assert!(customer.may_invoke(&name("billing.invoice.CreateInvoice")));
        assert!(
            !customer.may_invoke(&name("billing.invoice.CancelInvoice")),
            "a grant nobody wrote is not a grant"
        );
    }

    #[test]
    fn an_actor_permitted_a_command_that_does_not_exist_is_refused() {
        let spec = actor(
            "\
name: billing.Customer
may:
  - billing.invoice.CreateInvoice
  - billing.invoice.MarkInvoicePayed
",
        );
        let errors = spec.validate(&commands());
        assert_eq!(errors.len(), 1, "{errors}");

        let error = &errors.as_slice()[0];
        assert_eq!(error.code, ValidationCode::UndeclaredReference);
        assert_eq!(error.location, "actor billing.Customer.may");
        assert!(
            error
                .message
                .contains("`billing.invoice.MarkInvoicePayed`, which no domain declares"),
            "{error}"
        );
        assert!(
            error
                .hint
                .as_deref()
                .unwrap_or_default()
                .contains("billing.invoice.MarkInvoicePaid"),
            "the hint lists what was available, which is where the typo shows: {error}"
        );
    }

    #[test]
    fn every_grant_that_names_nothing_is_reported_not_just_the_first() {
        let spec = actor(
            "\
name: billing.FinanceOperator
may:
  - billing.invoice.ArchiveInvoice
  - billing.invoice.CancelInvoice
  - billing.invoice.ReopenInvoice
",
        );
        let errors = spec.validate(&commands());
        assert_eq!(errors.len(), 2, "one pass reports both: {errors}");
        let rendered = errors.to_string();
        assert!(rendered.contains("ArchiveInvoice"), "{rendered}");
        assert!(rendered.contains("ReopenInvoice"), "{rendered}");
    }

    #[test]
    fn an_actor_that_may_invoke_nothing_is_not_an_error() {
        let spec = actor("name: billing.Auditor\n");
        assert!(spec.may.is_empty());
        let errors = spec.validate(&commands());
        assert!(
            errors.is_empty(),
            "an actor that only observes still belongs in the picture: {errors}"
        );
    }

    #[test]
    fn a_grant_follows_the_command_identity_and_not_its_wire_name() {
        let spec = actor(
            "\
name: billing.Customer
may:
  - billing.invoice.CreateInvoice
naming:
  wire: customers
  display: Customer
",
        );
        assert_eq!(spec.naming.wire_or(&spec.name), "customers");
        assert!(
            spec.may_invoke(&name("billing.invoice.CreateInvoice")),
            "a permission is keyed on identity, so reshaping a transport cannot move it"
        );
        assert!(spec.validate(&commands()).is_empty());
    }

    #[test]
    fn an_actor_round_trips_through_yaml() {
        let spec = actor(
            "\
name: billing.FinanceOperator
may:
  - billing.invoice.MarkInvoicePaid
  - billing.invoice.CancelInvoice
naming:
  display: Finance operator
",
        );
        let rendered = serde_yaml::to_string(&spec).expect("serialises");
        let raw: RawActorSpec = serde_yaml::from_str(&rendered).expect("parses back");
        let again = ActorSpec::try_from(raw).expect("valid");
        assert_eq!(again, spec);
        assert!(
            rendered.find("CancelInvoice").unwrap() < rendered.find("MarkInvoicePaid").unwrap(),
            "grants come back in name order however they were written: {rendered}"
        );
    }

    #[test]
    fn a_malformed_command_name_is_refused_when_the_document_is_read() {
        let error = serde_yaml::from_str::<RawActorSpec>(
            "\
name: billing.Customer
may:
  - billing..CreateInvoice
",
        )
        .expect_err("an empty segment");
        assert!(
            error.to_string().contains("empty segment"),
            "a grant that is not a name is refused before validation, not carried into it: {error}"
        );
    }

    #[test]
    fn a_key_the_model_does_not_know_is_refused() {
        let error = serde_yaml::from_str::<RawActorSpec>(
            "\
name: billing.Customer
can:
  - billing.invoice.CreateInvoice
",
        )
        .expect_err("`can` is nothing");
        assert!(
            error.to_string().contains("can"),
            "a misspelt key would otherwise be a grant that silently does not exist: {error}"
        );
    }
}
