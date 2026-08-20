// generated from billing v3
// model digest 13577b3ce695932e980d418d5863bcde07f4c362516d53147870d31eaf2ed861
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

/// What this bounded context owes its implementor, as typed seams.
///
/// One trait per obligation in the synthesis plan, each carrying the plan's own contract.
/// [`Unimplemented`](obligations::Unimplemented) satisfies every trait by refusing in the type system, so the workspace builds —
/// and says exactly what it cannot yet do — before a line is hand-written.
pub mod obligations {
    /// The behaviour `billing.email.SendEmail` — an implementation obligation.
    ///
    /// Why it is not generated: decided outside the system: the provider rejects the recipient address.
    ///
    /// Contract: given `billing.email.SendEmail` input, decide and enact exactly one outcome — `sent` otherwise, emits `billing.email.EmailSent`; `failed` externally decided (the provider rejects the recipient address), error `billing.email.Undeliverable`.
    pub trait SendEmailBehavior {
        /// Decides and enacts exactly one declared outcome of `billing.email.SendEmail`.
        ///
        /// `Err` is the typed refusal of an obligation nothing has satisfied; a satisfying
        /// implementation never returns it.
        fn send_email(&mut self, input: super::SendEmail) -> Result<super::SendEmailOutcome, crate::obligation::UnmetObligation>;
    }

    /// Every obligation of this bounded context, refused in the type system.
    ///
    /// Each method returns the typed refusal naming what is owed — never a panic, never a guessed
    /// value — so a workspace built on this stub compiles and reports its own gaps.
    pub struct Unimplemented;

    impl SendEmailBehavior for Unimplemented {
        fn send_email(&mut self, _input: super::SendEmail) -> Result<super::SendEmailOutcome, crate::obligation::UnmetObligation> {
            Err(crate::obligation::UnmetObligation { capability: "command behaviour", source: "billing.email.SendEmail" })
        }
    }
}
