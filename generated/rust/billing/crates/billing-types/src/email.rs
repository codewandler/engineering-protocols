// generated from billing v3
// model digest e19d384dac86219a
// compiler 0.1.0 · generator 0.1.0
// do not edit: regenerate with `protocol ess synthesize`

//! email — `billing.email`.
//!
//! Sending the notifications other contexts ask for.
//!
//! Everything this bounded context declares that the synthesis plan marks generated.

/// EmailAddress — `billing.email.EmailAddress`: a distinct wrapper around `String`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmailAddress(pub String);

/// MessageId — `billing.email.MessageId`: a distinct wrapper around `Uuid`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageId(pub crate::primitives::Uuid);

/// TemplateId — `billing.email.TemplateId`: a distinct wrapper around `String`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateId(pub String);

/// SendEmail — the input of `billing.email.SendEmail`.
///
/// Everything it can result in is [`SendEmailOutcome`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SendEmail {
    /// `recipient` — `billing.email.EmailAddress`.
    pub recipient: EmailAddress,
    /// `template` — `billing.email.TemplateId`.
    pub template: TemplateId,
}

/// Everything `billing.email.SendEmail` can result in — one variant per declared outcome.
///
/// An infrastructure failure is deliberately not in here: a refusal is a fact about the domain,
/// a transport fault is a fact about the run, and conflating the two is what the declared
/// outcomes exist to prevent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SendEmailOutcome {
    /// `sent` — otherwise.
    Sent {
        /// The `billing.email.EmailSent` this outcome publishes.
        email_sent: EmailSent,
    },
    /// `failed` — externally decided (the provider rejects the recipient address).
    Failed {
        /// Why it was refused: `billing.email.Undeliverable`.
        error: Undeliverable,
    },
}

/// Delivery escalated — the event `billing.email.DeliveryEscalated`.
///
/// Sending was given up on and handed to a person.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeliveryEscalated {
    /// `recipient` — `billing.email.EmailAddress`.
    pub recipient: EmailAddress,
    /// `template` — `billing.email.TemplateId`.
    pub template: TemplateId,
}

/// EmailSent — the event `billing.email.EmailSent`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmailSent {
    /// `message_id` — `billing.email.MessageId`.
    pub message_id: MessageId,
    /// `recipient` — `billing.email.EmailAddress`.
    pub recipient: EmailAddress,
}

/// The declared error `billing.email.Undeliverable`.
///
/// The address was rejected by the provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Undeliverable;

/// The declared crossing `billing.invoice.Email` → `billing.email.EmailAddress`.
///
/// Permitted by the specification because: An invoice's customer email is a deliverable address; the email context validates it again on the way out, so the invoice context does not have to know how.
impl From<crate::invoice::Email> for EmailAddress {
    fn from(value: crate::invoice::Email) -> Self {
        Self(value.0)
    }
}
