//! The binding escalation obligation of `notify-on-invoice-created`.
//!
//! The specification declares the event — `billing.email.DeliveryEscalated`, with `recipient`
//! and `template` — and declares when it happens: sending was given up on. What it does not say
//! is how the event's fields are filled from the invocation that failed, which is why this is an
//! obligation. The realization's answer is the only one that manufactures nothing: the escalation
//! records exactly the delivery that was given up on, field for field.

use billing_system::obligations::NotifyOnInvoiceCreatedEscalation;
use billing_types::email::{DeliveryEscalated, SendEmail};
use billing_types::obligation::UnmetObligation;

/// The honest realization of the escalation: the failed invocation, recorded as declared.
#[derive(Clone, Copy, Debug, Default)]
pub struct EscalationRealization;

impl EscalationRealization {
    /// The realization. It holds nothing, because the declared event is filled entirely from the
    /// invocation it is about.
    pub fn new() -> Self {
        Self
    }
}

impl NotifyOnInvoiceCreatedEscalation for EscalationRealization {
    fn notify_on_invoice_created_escalation(
        &self,
        failed: &SendEmail,
    ) -> Result<DeliveryEscalated, UnmetObligation> {
        Ok(DeliveryEscalated {
            recipient: failed.recipient.clone(),
            template: failed.template.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use billing_types::email::{EmailAddress, TemplateId};

    #[test]
    fn the_escalation_records_the_delivery_that_was_given_up_on_field_for_field() {
        let failed = SendEmail {
            recipient: EmailAddress("person@example.com".to_owned()),
            template: TemplateId("invoice-created".to_owned()),
        };
        let escalated = EscalationRealization::new()
            .notify_on_invoice_created_escalation(&failed)
            .expect("the honest realization satisfies the obligation");
        assert_eq!(
            escalated.recipient, failed.recipient,
            "a person picking this up must learn who was never reached"
        );
        assert_eq!(
            escalated.template, failed.template,
            "and which message never reached them"
        );
    }
}
