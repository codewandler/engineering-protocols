// generated from billing v3
// model digest e19d384dac86219a38b673f7ac5a9775eba834643b4e19ddbdc61767fb8a46f5
// compiler 0.1.0 · generator 0.1.0
// do not edit: regenerate with `protocol ess synthesize`

//! invoice-service — the `invoice-service` component of `billing` v3.
//!
//! Issues invoices and tracks payment.
//!
//! The component's outer surface exactly as the specification declares it: accepted commands as
//! handlers, declared views as queries, published events as a typed outbox. The behaviour behind
//! every handler is an implementation obligation — see the `PLAN.md` beside this workspace — and
//! until one is satisfied, its stub answers with a typed refusal naming what is owed.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

/// An event this component declares it publishes, on its way to the system's transport.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PublishedEvent {
    /// `billing.invoice.InvoiceCancelled`.
    InvoiceCancelled(billing_types::invoice::InvoiceCancelled),
    /// `billing.invoice.InvoiceCreated`.
    InvoiceCreated(billing_types::invoice::InvoiceCreated),
    /// `billing.invoice.InvoiceIssued`.
    InvoiceIssued(billing_types::invoice::InvoiceIssued),
    /// `billing.invoice.InvoicePaid`.
    InvoicePaid(billing_types::invoice::InvoicePaid),
}

/// invoice-service — the port over the component's obligations.
///
/// `B` bundles every behaviour and query this component owes; constructing it over the domain's
/// `obligations::Unimplemented` yields a component that compiles and refuses, in the type system,
/// everything not yet implemented.
pub struct InvoiceService<B> {
    behaviors: B,
    outbox: Vec<PublishedEvent>,
}

impl<B> InvoiceService<B> {
    /// A new port over the given obligation implementations.
    pub fn new(behaviors: B) -> Self {
        Self {
            behaviors,
            outbox: Vec::new(),
        }
    }

    /// Hands over everything published since the last drain, in publication order.
    ///
    /// The system's transport calls this; anything else reading it is taking events the transport
    /// will then never deliver.
    pub fn drain_outbox(&mut self) -> Vec<PublishedEvent> {
        core::mem::take(&mut self.outbox)
    }
}

impl<B> InvoiceService<B>
where
    B: billing_types::invoice::obligations::CancelInvoiceBehavior + billing_types::invoice::obligations::CreateInvoiceBehavior + billing_types::invoice::obligations::IssueInvoiceBehavior + billing_types::invoice::obligations::PayInvoiceBehavior + billing_types::invoice::obligations::InvoiceByIdQuery + billing_types::invoice::obligations::OutstandingInvoicesQuery,
{
    /// Accepts `billing.invoice.CancelInvoice`: runs the behaviour obligation, then publishes the declared events
    /// the outcome carries.
    ///
    /// `Err` is the typed refusal of an unmet obligation — never a domain outcome, which always
    /// arrives as a variant of the outcome type, refusals included.
    pub fn cancel_invoice(&mut self, input: billing_types::invoice::CancelInvoice) -> Result<billing_types::invoice::CancelInvoiceOutcome, billing_types::obligation::UnmetObligation> {
        let outcome = self.behaviors.cancel_invoice(input)?;
        match &outcome {
            billing_types::invoice::CancelInvoiceOutcome::Cancelled { invoice_cancelled, .. } => {
                self.outbox.push(PublishedEvent::InvoiceCancelled(invoice_cancelled.clone()));
            }
            billing_types::invoice::CancelInvoiceOutcome::WrongState { .. } => {}
        }
        Ok(outcome)
    }

    /// Accepts `billing.invoice.CreateInvoice`: runs the behaviour obligation, then publishes the declared events
    /// the outcome carries.
    ///
    /// `Err` is the typed refusal of an unmet obligation — never a domain outcome, which always
    /// arrives as a variant of the outcome type, refusals included.
    pub fn create_invoice(&mut self, input: billing_types::invoice::CreateInvoice) -> Result<billing_types::invoice::CreateInvoiceOutcome, billing_types::obligation::UnmetObligation> {
        let outcome = self.behaviors.create_invoice(input)?;
        match &outcome {
            billing_types::invoice::CreateInvoiceOutcome::Accepted { invoice_created, .. } => {
                self.outbox.push(PublishedEvent::InvoiceCreated(invoice_created.clone()));
            }
            billing_types::invoice::CreateInvoiceOutcome::Rejected { .. } => {}
        }
        Ok(outcome)
    }

    /// Accepts `billing.invoice.IssueInvoice`: runs the behaviour obligation, then publishes the declared events
    /// the outcome carries.
    ///
    /// `Err` is the typed refusal of an unmet obligation — never a domain outcome, which always
    /// arrives as a variant of the outcome type, refusals included.
    pub fn issue_invoice(&mut self, input: billing_types::invoice::IssueInvoice) -> Result<billing_types::invoice::IssueInvoiceOutcome, billing_types::obligation::UnmetObligation> {
        let outcome = self.behaviors.issue_invoice(input)?;
        match &outcome {
            billing_types::invoice::IssueInvoiceOutcome::Issued { invoice_issued, .. } => {
                self.outbox.push(PublishedEvent::InvoiceIssued(invoice_issued.clone()));
            }
            billing_types::invoice::IssueInvoiceOutcome::WrongState { .. } => {}
        }
        Ok(outcome)
    }

    /// Accepts `billing.invoice.PayInvoice`: runs the behaviour obligation, then publishes the declared events
    /// the outcome carries.
    ///
    /// `Err` is the typed refusal of an unmet obligation — never a domain outcome, which always
    /// arrives as a variant of the outcome type, refusals included.
    pub fn pay_invoice(&mut self, input: billing_types::invoice::PayInvoice) -> Result<billing_types::invoice::PayInvoiceOutcome, billing_types::obligation::UnmetObligation> {
        let outcome = self.behaviors.pay_invoice(input)?;
        match &outcome {
            billing_types::invoice::PayInvoiceOutcome::Settled { invoice_paid, .. } => {
                self.outbox.push(PublishedEvent::InvoicePaid(invoice_paid.clone()));
            }
            billing_types::invoice::PayInvoiceOutcome::Rejected { .. } => {}
            billing_types::invoice::PayInvoiceOutcome::WrongState { .. } => {}
        }
        Ok(outcome)
    }

    /// Serves `billing.invoice.InvoiceById` at `eventual` consistency, from the owed projection.
    pub fn invoice_by_id(&self) -> Result<Vec<billing_types::invoice::InvoiceById>, billing_types::obligation::UnmetObligation> {
        self.behaviors.invoice_by_id()
    }

    /// Serves `billing.invoice.OutstandingInvoices` at `read_your_writes` consistency, from the owed projection.
    pub fn outstanding_invoices(&self) -> Result<Vec<billing_types::invoice::OutstandingInvoices>, billing_types::obligation::UnmetObligation> {
        self.behaviors.outstanding_invoices()
    }
}
