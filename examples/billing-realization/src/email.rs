//! The `billing.email.SendEmail` obligation: the provider seam, and the control it exposes.
//!
//! The specification declares `SendEmail`'s `failed` branch `external:` — no predicate over a
//! recipient and a template says whether a provider will accept the mail — which is exactly why
//! the behaviour is an obligation and not generated. This realization stands in for the provider:
//! it sends, unless a conformance run has injected the provider's refusal through
//! [`EmailControl`].
//!
//! The control seam is hand-written by design. The generated tree owns no test controls and never
//! will — a generated system that answered "make the next send fail" would be a system whose
//! transport can lie — so the injection point lives on this side of the ownership boundary, in
//! the code that plays the provider anyway.

use std::cell::RefCell;
use std::rc::Rc;

use billing_types::email::obligations::SendEmailBehavior;
use billing_types::email::{EmailSent, MessageId, SendEmail, SendEmailOutcome, Undeliverable};
use billing_types::obligation::UnmetObligation;
use billing_types::primitives::Uuid;

/// What the stand-in provider holds: the injected refusal, and the message-id mint.
#[derive(Debug, Default)]
struct Provider {
    /// Whether the next send is refused, as an external-outcome injection demands. It applies to
    /// the next invocation and then lapses, which is what the conformance step says.
    fail_next: bool,
    /// The counter every message id comes from — a mint, never randomness (invariant 9).
    sequence: u64,
}

/// A cloneable control over the provider the realization stands in for.
///
/// A conformance bridge holds one clone and the [`EmailRealization`] holds the other, which is
/// how "force the next answer" reaches a behaviour that is otherwise sealed behind the generated
/// port.
#[derive(Clone, Debug, Default)]
pub struct EmailControl {
    provider: Rc<RefCell<Provider>>,
}

impl EmailControl {
    /// A provider that accepts everything until told otherwise.
    pub fn new() -> Self {
        Self::default()
    }

    /// Forces the next `SendEmail` to take its declared `failed` branch, then lapses.
    pub fn fail_next(&self) {
        self.provider.borrow_mut().fail_next = true;
    }
}

/// The honest realization of `billing.email.SendEmail`.
#[derive(Clone, Debug)]
pub struct EmailRealization {
    control: EmailControl,
}

impl EmailRealization {
    /// The realization, answering as the provider `control` steers.
    pub fn over(control: EmailControl) -> Self {
        Self { control }
    }
}

impl SendEmailBehavior for EmailRealization {
    fn send_email(&mut self, input: SendEmail) -> Result<SendEmailOutcome, UnmetObligation> {
        let mut provider = self.control.provider.borrow_mut();
        if provider.fail_next {
            provider.fail_next = false;
            // The provider's refusal is a *domain* outcome — the declared `failed` branch with
            // its declared error — never an `Err`, which would read as this obligation being
            // unmet rather than as the provider saying no.
            return Ok(SendEmailOutcome::Failed {
                error: Undeliverable,
            });
        }
        provider.sequence += 1;
        Ok(SendEmailOutcome::Sent {
            email_sent: EmailSent {
                message_id: MessageId(Uuid(format!(
                    "00000000-0000-4000-9000-{:012}",
                    provider.sequence
                ))),
                recipient: input.recipient,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use billing_types::email::{EmailAddress, TemplateId};

    fn send(realization: &mut EmailRealization) -> SendEmailOutcome {
        realization
            .send_email(SendEmail {
                recipient: EmailAddress("a@example.com".to_owned()),
                template: TemplateId("invoice-created".to_owned()),
            })
            .expect("the honest realization satisfies the obligation")
    }

    #[test]
    fn a_forced_failure_applies_to_the_next_send_and_then_lapses() {
        let control = EmailControl::new();
        let mut realization = EmailRealization::over(control.clone());
        control.fail_next();

        let refused = send(&mut realization);
        assert!(
            matches!(refused, SendEmailOutcome::Failed { .. }),
            "the injected provider refusal must surface as the declared `failed` branch, got \
             {refused:?}"
        );
        let recovered = send(&mut realization);
        assert!(
            matches!(recovered, SendEmailOutcome::Sent { .. }),
            "the injection covers the *next* invocation only; the one after must send, got \
             {recovered:?}"
        );
    }
}
