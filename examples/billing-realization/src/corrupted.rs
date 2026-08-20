//! The deliberately wrong realization, beside the honest one — the falsifiability half.
//!
//! A suite that passes a correct implementation has shown only that it asks nothing a correct
//! implementation cannot answer. `ess-conformance`'s `faulty` module ships the other half for
//! wave 4's references; this module is the same discipline applied to wave 6's linked system: a
//! **named** fault, the one committed scenario that exists to catch it, and a blast-radius claim
//! — all three asserted in `tests/conformance.rs` rather than merely written here.
//!
//! One fault, not a matrix. The matrix's job — every fault class, per-fault blast radii, the
//! uncaught rows — is wave 4's and stays discharged there; what W6.3 owes is the demonstration
//! that *this* linkage of generated code and hand-written obligations is falsifiable by the
//! *unchanged* committed suite.

use billing_types::invoice::obligations::CreateInvoiceBehavior;
use billing_types::invoice::{CreateInvoice, CreateInvoiceOutcome};
use billing_types::obligation::UnmetObligation;

use crate::invoice::{InvoiceRealization, SharedInvoices};

/// How the fault is written, in the linked system's identity and in a report.
pub const FAULT: &str = "accepts-any-amount";

/// The one committed scenario that exists to catch [`AcceptsAnyAmount`].
///
/// And the only one that can: no other scenario in the committed billing suite submits a
/// non-positive amount to `CreateInvoice`, which is the blast-radius claim
/// `tests/conformance.rs` holds mechanically.
pub const CAUGHT_BY: &str = "billing.invoice.CreateInvoice/outcome/rejected";

/// `accepts-any-amount`: the guard `amount.amount > 0` is dropped, so an amount the
/// specification refuses buys an invoice.
///
/// The corruption is exactly one clause wide: it reuses the honest realization's acceptance —
/// same store, same mint, same event — and skips only the guard. A fault that replaced the whole
/// behaviour would be a different implementation; this is the honest one, lying about one rule,
/// which is the defect a conformance suite exists to notice.
#[derive(Clone, Debug)]
pub struct AcceptsAnyAmount {
    honest: InvoiceRealization,
}

impl AcceptsAnyAmount {
    /// The corrupted behaviour, over the same shared store the honest obligations use.
    pub fn over(invoices: SharedInvoices) -> Self {
        Self {
            honest: InvoiceRealization::over(invoices),
        }
    }
}

impl CreateInvoiceBehavior for AcceptsAnyAmount {
    fn create_invoice(
        &mut self,
        input: CreateInvoice,
    ) -> Result<CreateInvoiceOutcome, UnmetObligation> {
        // The corruption: straight to acceptance. The declared guard is never consulted.
        Ok(self.honest.accept(input))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use billing_types::invoice::Email;
    use billing_types::invoice::Money;
    use billing_types::primitives::Decimal;

    #[test]
    fn the_corruption_accepts_the_amount_the_specification_refuses() {
        // The fault must actually be a fault, or the conformance test asserting the suite
        // catches it would be measuring an honest implementation and proving nothing.
        let mut corrupted = AcceptsAnyAmount::over(SharedInvoices::new());
        let outcome = corrupted
            .create_invoice(CreateInvoice {
                customer_email: Email("a@example.com".to_owned()),
                amount: Money {
                    amount: Decimal("0.00".to_owned()),
                    currency: "EUR".to_owned(),
                },
            })
            .expect("the corrupted behaviour still answers the obligation");
        assert!(
            matches!(outcome, CreateInvoiceOutcome::Accepted { .. }),
            "`0.00` must be accepted by the corrupted guard for the fault to exist, got {outcome:?}"
        );
    }
}
