//! The linker: generated components and hand implementations, assembled — and never chosen among.
//!
//! Gap register D-2, taken as written: the linker does not choose. Zero implementations offered
//! for an obligation is an **unsatisfied obligation**; two is an **ambiguity error naming both**.
//! Selection among alternatives is `Realization` material and stays proposed with it, so there is
//! deliberately no priority, no default, no "first wins" — the only accepted state is exactly one
//! implementation per obligation the plan owes.
//!
//! Errors accumulate (invariant 3): a linker with three empty slots reports three unsatisfied
//! obligations, not the first one it happened to walk.
//!
//! The obligation list here is [`OBLIGATIONS`], spelled as the generated stubs spell it, and a
//! test below holds it equal to what `generated/rust/billing/plan.json` owes — so this module
//! cannot quietly keep linking a plan that has moved.

use std::fmt;

use billing_system::obligations::NotifyOnInvoiceCreatedEscalation;
use billing_types::email::obligations::SendEmailBehavior;
use billing_types::email::{SendEmail, SendEmailOutcome};
use billing_types::invoice::obligations::{
    CancelInvoiceBehavior, CreateInvoiceBehavior, InvoiceByIdQuery, IssueInvoiceBehavior,
    OutstandingInvoicesQuery, PayInvoiceBehavior,
};
use billing_types::invoice::{
    CancelInvoice, CancelInvoiceOutcome, CreateInvoice, CreateInvoiceOutcome, InvoiceById,
    IssueInvoice, IssueInvoiceOutcome, OutstandingInvoices, PayInvoice, PayInvoiceOutcome,
};
use billing_types::obligation::UnmetObligation;

use crate::corrupted::AcceptsAnyAmount;
use crate::email::{EmailControl, EmailRealization};
use crate::escalation::EscalationRealization;
use crate::invoice::{InvoiceRealization, SharedInvoices};

/// Every obligation the billing plan owes, as `(capability, source)` in the stubs' own spelling.
///
/// Held equal to `generated/rust/billing/plan.json` by
/// `the_linkers_obligation_list_is_exactly_the_plans`, so a specification change that moves an
/// obligation fails here instead of leaving the linker resolving a list that no longer exists.
pub const OBLIGATIONS: &[(&str, &str)] = &[
    ("binding escalation", "notify-on-invoice-created"),
    ("command behaviour", "billing.email.SendEmail"),
    ("command behaviour", "billing.invoice.CancelInvoice"),
    ("command behaviour", "billing.invoice.CreateInvoice"),
    ("command behaviour", "billing.invoice.IssueInvoice"),
    ("command behaviour", "billing.invoice.PayInvoice"),
    ("view query", "billing.invoice.InvoiceById"),
    ("view query", "billing.invoice.OutstandingInvoices"),
];

/// How the honest offers name themselves in an ambiguity error.
pub const HONEST: &str = "billing-realization/honest";

// ---- refusals --------------------------------------------------------------------------------

/// Why one obligation could not be resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkError {
    /// Nothing was offered for an obligation the plan owes.
    Unsatisfied {
        /// The capability kind, as the plan spells it.
        capability: &'static str,
        /// The construct that requires it, in the specification's own spelling.
        source: &'static str,
    },
    /// More than one implementation claims one obligation — named in full, chosen among never.
    Ambiguous {
        /// The capability kind, as the plan spells it.
        capability: &'static str,
        /// The construct that requires it, in the specification's own spelling.
        source: &'static str,
        /// Every claimant, in the order offered.
        offered: Vec<&'static str>,
    },
}

impl fmt::Display for LinkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsatisfied { capability, source } => {
                write!(
                    f,
                    "unsatisfied obligation: {capability} `{source}` — nothing was offered for it"
                )
            }
            Self::Ambiguous {
                capability,
                source,
                offered,
            } => {
                write!(
                    f,
                    "ambiguous obligation: {capability} `{source}` is claimed by {} — the linker \
                     does not choose (D-2)",
                    offered
                        .iter()
                        .map(|name| format!("`{name}`"))
                        .collect::<Vec<_>>()
                        .join(" and ")
                )
            }
        }
    }
}

/// Everything that kept a linkage from resolving, accumulated rather than truncated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkErrors {
    /// Every refusal, in obligation order.
    pub errors: Vec<LinkError>,
}

impl fmt::Display for LinkErrors {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (position, error) in self.errors.iter().enumerate() {
            if position > 0 {
                writeln!(f)?;
            }
            write!(f, "{error}")?;
        }
        Ok(())
    }
}

impl std::error::Error for LinkErrors {}

// ---- one slot per obligation ------------------------------------------------------------------

/// The offers made for one obligation.
struct Slot<T> {
    capability: &'static str,
    source: &'static str,
    offered: Vec<(&'static str, T)>,
}

impl<T> Slot<T> {
    /// An empty slot for one plan entry.
    fn owed(capability: &'static str, source: &'static str) -> Self {
        Self {
            capability,
            source,
            offered: Vec::new(),
        }
    }

    /// Records one named offer. Recording never refuses — refusal is `resolve`'s, so that two
    /// offers are visible as the ambiguity they are rather than racing for the slot.
    fn offer(&mut self, provider: &'static str, implementation: T) {
        self.offered.push((provider, implementation));
    }

    /// Exactly one offer, or the refusal D-2 prescribes for zero and for many.
    fn resolve(mut self) -> Result<T, LinkError> {
        if self.offered.len() > 1 {
            return Err(LinkError::Ambiguous {
                capability: self.capability,
                source: self.source,
                offered: self.offered.iter().map(|(name, _)| *name).collect(),
            });
        }
        match self.offered.pop() {
            Some((_, implementation)) => Ok(implementation),
            None => Err(LinkError::Unsatisfied {
                capability: self.capability,
                source: self.source,
            }),
        }
    }
}

// ---- the linker -------------------------------------------------------------------------------

/// Collects offers, one slot per obligation the plan owes, and resolves them without choosing.
pub struct Linker {
    send_email: Slot<Box<dyn SendEmailBehavior>>,
    cancel_invoice: Slot<Box<dyn CancelInvoiceBehavior>>,
    create_invoice: Slot<Box<dyn CreateInvoiceBehavior>>,
    issue_invoice: Slot<Box<dyn IssueInvoiceBehavior>>,
    pay_invoice: Slot<Box<dyn PayInvoiceBehavior>>,
    invoice_by_id: Slot<Box<dyn InvoiceByIdQuery>>,
    outstanding_invoices: Slot<Box<dyn OutstandingInvoicesQuery>>,
    escalation: Slot<Box<dyn NotifyOnInvoiceCreatedEscalation>>,
}

impl Default for Linker {
    fn default() -> Self {
        Self::new()
    }
}

impl Linker {
    /// A linker with every obligation of the plan owed and nothing offered yet.
    pub fn new() -> Self {
        Self {
            send_email: Slot::owed("command behaviour", "billing.email.SendEmail"),
            cancel_invoice: Slot::owed("command behaviour", "billing.invoice.CancelInvoice"),
            create_invoice: Slot::owed("command behaviour", "billing.invoice.CreateInvoice"),
            issue_invoice: Slot::owed("command behaviour", "billing.invoice.IssueInvoice"),
            pay_invoice: Slot::owed("command behaviour", "billing.invoice.PayInvoice"),
            invoice_by_id: Slot::owed("view query", "billing.invoice.InvoiceById"),
            outstanding_invoices: Slot::owed("view query", "billing.invoice.OutstandingInvoices"),
            escalation: Slot::owed("binding escalation", "notify-on-invoice-created"),
        }
    }

    /// Offers an implementation of the `billing.email.SendEmail` behaviour.
    pub fn offer_send_email(
        &mut self,
        provider: &'static str,
        implementation: Box<dyn SendEmailBehavior>,
    ) {
        self.send_email.offer(provider, implementation);
    }

    /// Offers an implementation of the `billing.invoice.CancelInvoice` behaviour.
    pub fn offer_cancel_invoice(
        &mut self,
        provider: &'static str,
        implementation: Box<dyn CancelInvoiceBehavior>,
    ) {
        self.cancel_invoice.offer(provider, implementation);
    }

    /// Offers an implementation of the `billing.invoice.CreateInvoice` behaviour.
    pub fn offer_create_invoice(
        &mut self,
        provider: &'static str,
        implementation: Box<dyn CreateInvoiceBehavior>,
    ) {
        self.create_invoice.offer(provider, implementation);
    }

    /// Offers an implementation of the `billing.invoice.IssueInvoice` behaviour.
    pub fn offer_issue_invoice(
        &mut self,
        provider: &'static str,
        implementation: Box<dyn IssueInvoiceBehavior>,
    ) {
        self.issue_invoice.offer(provider, implementation);
    }

    /// Offers an implementation of the `billing.invoice.PayInvoice` behaviour.
    pub fn offer_pay_invoice(
        &mut self,
        provider: &'static str,
        implementation: Box<dyn PayInvoiceBehavior>,
    ) {
        self.pay_invoice.offer(provider, implementation);
    }

    /// Offers an implementation of the `billing.invoice.InvoiceById` projection.
    pub fn offer_invoice_by_id(
        &mut self,
        provider: &'static str,
        implementation: Box<dyn InvoiceByIdQuery>,
    ) {
        self.invoice_by_id.offer(provider, implementation);
    }

    /// Offers an implementation of the `billing.invoice.OutstandingInvoices` projection.
    pub fn offer_outstanding_invoices(
        &mut self,
        provider: &'static str,
        implementation: Box<dyn OutstandingInvoicesQuery>,
    ) {
        self.outstanding_invoices.offer(provider, implementation);
    }

    /// Offers an implementation of the `notify-on-invoice-created` escalation.
    pub fn offer_escalation(
        &mut self,
        provider: &'static str,
        implementation: Box<dyn NotifyOnInvoiceCreatedEscalation>,
    ) {
        self.escalation.offer(provider, implementation);
    }

    /// Resolves every slot and assembles the system, or reports every refusal at once.
    ///
    /// # Errors
    ///
    /// [`LinkErrors`] carrying one [`LinkError`] per obligation that resolved to zero offers or
    /// to more than one — never a partial system, and never a choice.
    pub fn link(self) -> Result<LinkedSystem, LinkErrors> {
        let mut errors = Vec::new();
        let send_email = keep(self.send_email.resolve(), &mut errors);
        let cancel_invoice = keep(self.cancel_invoice.resolve(), &mut errors);
        let create_invoice = keep(self.create_invoice.resolve(), &mut errors);
        let issue_invoice = keep(self.issue_invoice.resolve(), &mut errors);
        let pay_invoice = keep(self.pay_invoice.resolve(), &mut errors);
        let invoice_by_id = keep(self.invoice_by_id.resolve(), &mut errors);
        let outstanding_invoices = keep(self.outstanding_invoices.resolve(), &mut errors);
        let escalation = keep(self.escalation.resolve(), &mut errors);

        let (
            Some(send_email),
            Some(cancel_invoice),
            Some(create_invoice),
            Some(issue_invoice),
            Some(pay_invoice),
            Some(invoice_by_id),
            Some(outstanding_invoices),
            Some(escalation),
        ) = (
            send_email,
            cancel_invoice,
            create_invoice,
            issue_invoice,
            pay_invoice,
            invoice_by_id,
            outstanding_invoices,
            escalation,
        )
        else {
            return Err(LinkErrors { errors });
        };

        Ok(billing_system::System::new(
            email_service::EmailService::new(LinkedEmailServiceBehaviors { send_email }),
            invoice_service::InvoiceService::new(LinkedInvoiceServiceBehaviors {
                cancel_invoice,
                create_invoice,
                issue_invoice,
                pay_invoice,
                invoice_by_id,
                outstanding_invoices,
            }),
            LinkedObligations { escalation },
        ))
    }
}

/// Keeps a resolution, or files its refusal — which is what lets `link` report all of them.
fn keep<T>(resolution: Result<T, LinkError>, errors: &mut Vec<LinkError>) -> Option<T> {
    match resolution {
        Ok(implementation) => Some(implementation),
        Err(error) => {
            errors.push(error);
            None
        }
    }
}

// ---- what a linkage produces ------------------------------------------------------------------

/// The billing system, linked: generated components over exactly one implementation per
/// obligation.
pub type LinkedSystem = billing_system::System<
    LinkedEmailServiceBehaviors,
    LinkedInvoiceServiceBehaviors,
    LinkedObligations,
>;

/// The email component's obligations, as the linker resolved them.
pub struct LinkedEmailServiceBehaviors {
    send_email: Box<dyn SendEmailBehavior>,
}

impl SendEmailBehavior for LinkedEmailServiceBehaviors {
    fn send_email(&mut self, input: SendEmail) -> Result<SendEmailOutcome, UnmetObligation> {
        self.send_email.send_email(input)
    }
}

/// The invoice component's obligations, as the linker resolved them.
pub struct LinkedInvoiceServiceBehaviors {
    cancel_invoice: Box<dyn CancelInvoiceBehavior>,
    create_invoice: Box<dyn CreateInvoiceBehavior>,
    issue_invoice: Box<dyn IssueInvoiceBehavior>,
    pay_invoice: Box<dyn PayInvoiceBehavior>,
    invoice_by_id: Box<dyn InvoiceByIdQuery>,
    outstanding_invoices: Box<dyn OutstandingInvoicesQuery>,
}

impl CancelInvoiceBehavior for LinkedInvoiceServiceBehaviors {
    fn cancel_invoice(
        &mut self,
        input: CancelInvoice,
    ) -> Result<CancelInvoiceOutcome, UnmetObligation> {
        self.cancel_invoice.cancel_invoice(input)
    }
}

impl CreateInvoiceBehavior for LinkedInvoiceServiceBehaviors {
    fn create_invoice(
        &mut self,
        input: CreateInvoice,
    ) -> Result<CreateInvoiceOutcome, UnmetObligation> {
        self.create_invoice.create_invoice(input)
    }
}

impl IssueInvoiceBehavior for LinkedInvoiceServiceBehaviors {
    fn issue_invoice(
        &mut self,
        input: IssueInvoice,
    ) -> Result<IssueInvoiceOutcome, UnmetObligation> {
        self.issue_invoice.issue_invoice(input)
    }
}

impl PayInvoiceBehavior for LinkedInvoiceServiceBehaviors {
    fn pay_invoice(&mut self, input: PayInvoice) -> Result<PayInvoiceOutcome, UnmetObligation> {
        self.pay_invoice.pay_invoice(input)
    }
}

impl InvoiceByIdQuery for LinkedInvoiceServiceBehaviors {
    fn invoice_by_id(&self) -> Result<Vec<InvoiceById>, UnmetObligation> {
        self.invoice_by_id.invoice_by_id()
    }
}

impl OutstandingInvoicesQuery for LinkedInvoiceServiceBehaviors {
    fn outstanding_invoices(&self) -> Result<Vec<OutstandingInvoices>, UnmetObligation> {
        self.outstanding_invoices.outstanding_invoices()
    }
}

/// The system-level obligations, as the linker resolved them.
pub struct LinkedObligations {
    escalation: Box<dyn NotifyOnInvoiceCreatedEscalation>,
}

impl NotifyOnInvoiceCreatedEscalation for LinkedObligations {
    fn notify_on_invoice_created_escalation(
        &self,
        failed: &SendEmail,
    ) -> Result<billing_types::email::DeliveryEscalated, UnmetObligation> {
        self.escalation.notify_on_invoice_created_escalation(failed)
    }
}

// ---- the two linkages this repository ships ---------------------------------------------------

/// A linked system and the hand-written control seams a conformance run injects through.
pub struct Assembled {
    /// The linked system.
    pub system: LinkedSystem,
    /// The provider control of the email realization.
    pub email: EmailControl,
    /// The shared invoice store — for the adapter's subject-existence check, nothing else.
    pub invoices: SharedInvoices,
}

/// Offers every honest implementation except `CreateInvoice`, which the two linkages differ on.
fn offer_honest_except_create(
    linker: &mut Linker,
    invoices: &SharedInvoices,
    email: &EmailControl,
) {
    linker.offer_send_email(HONEST, Box::new(EmailRealization::over(email.clone())));
    linker.offer_cancel_invoice(HONEST, Box::new(InvoiceRealization::over(invoices.clone())));
    linker.offer_issue_invoice(HONEST, Box::new(InvoiceRealization::over(invoices.clone())));
    linker.offer_pay_invoice(HONEST, Box::new(InvoiceRealization::over(invoices.clone())));
    linker.offer_invoice_by_id(HONEST, Box::new(InvoiceRealization::over(invoices.clone())));
    linker.offer_outstanding_invoices(HONEST, Box::new(InvoiceRealization::over(invoices.clone())));
    linker.offer_escalation(HONEST, Box::new(EscalationRealization::new()));
}

/// Links the honest realization of every obligation.
///
/// # Panics
///
/// It does not: exactly one implementation is offered per obligation below, which is the one
/// state `link` accepts. The `expect` names the invariant so a refactor that breaks it fails
/// readably.
pub fn honest() -> Assembled {
    let invoices = SharedInvoices::new();
    let email = EmailControl::new();
    let mut linker = Linker::new();
    offer_honest_except_create(&mut linker, &invoices, &email);
    linker.offer_create_invoice(HONEST, Box::new(InvoiceRealization::over(invoices.clone())));
    let system = linker
        .link()
        .expect("the honest linkage offers every obligation exactly once");
    Assembled {
        system,
        email,
        invoices,
    }
}

/// Links the honest realization with exactly one obligation swapped for the corrupted one.
///
/// Everything else — store, provider, escalation — is the honest linkage, which is what makes a
/// failure of the suite against this system attributable to the one lie.
///
/// # Panics
///
/// It does not, for [`honest`]'s reason.
pub fn corrupted() -> Assembled {
    let invoices = SharedInvoices::new();
    let email = EmailControl::new();
    let mut linker = Linker::new();
    offer_honest_except_create(&mut linker, &invoices, &email);
    linker.offer_create_invoice(
        "billing-realization/accepts-any-amount",
        Box::new(AcceptsAnyAmount::over(invoices.clone())),
    );
    let system = linker
        .link()
        .expect("the corrupted linkage offers every obligation exactly once");
    Assembled {
        system,
        email,
        invoices,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    #[test]
    fn a_linker_nothing_was_offered_to_reports_every_obligation_not_the_first() {
        // Invariant 3, applied to linking: eight empty slots are eight findings, and a linker
        // that stopped at the first would make the second visible only after the first is fixed.
        let Err(refused) = Linker::new().link() else {
            panic!("an empty linker satisfies nothing, yet it linked");
        };
        let named: BTreeSet<(&str, &str)> = refused
            .errors
            .iter()
            .map(|error| match error {
                LinkError::Unsatisfied { capability, source } => (*capability, *source),
                LinkError::Ambiguous { .. } => {
                    panic!("nothing was offered, so nothing can be ambiguous: {error}")
                }
            })
            .collect();
        let owed: BTreeSet<(&str, &str)> = OBLIGATIONS.iter().copied().collect();
        assert_eq!(
            named, owed,
            "every obligation of the plan is reported unsatisfied, by name"
        );
    }

    #[test]
    fn one_missing_obligation_is_an_unsatisfied_obligation_naming_it() {
        // D-2's zero case, one slot wide: everything else resolves, so the report is exactly the
        // gap and not noise around it.
        let invoices = SharedInvoices::new();
        let email = EmailControl::new();
        let mut linker = Linker::new();
        offer_honest_except_create(&mut linker, &invoices, &email);
        // `CreateInvoice` is deliberately never offered.
        let Err(refused) = linker.link() else {
            panic!("an obligation with no implementation cannot link, yet it linked");
        };
        assert_eq!(
            refused.errors,
            vec![LinkError::Unsatisfied {
                capability: "command behaviour",
                source: "billing.invoice.CreateInvoice",
            }],
            "the one missing obligation is the whole report"
        );
    }

    #[test]
    fn two_implementations_for_one_obligation_is_an_ambiguity_naming_both() {
        // D-2's many case: the linker never chooses, and the error names every claimant — an
        // ambiguity error naming one implementation would leave the reader hunting the other.
        let invoices = SharedInvoices::new();
        let email = EmailControl::new();
        let mut linker = Linker::new();
        offer_honest_except_create(&mut linker, &invoices, &email);
        linker.offer_create_invoice(HONEST, Box::new(InvoiceRealization::over(invoices.clone())));
        linker.offer_create_invoice(
            "billing-realization/accepts-any-amount",
            Box::new(AcceptsAnyAmount::over(invoices.clone())),
        );
        let Err(refused) = linker.link() else {
            panic!("two claimants for one obligation must refuse, never race — yet it linked");
        };
        assert_eq!(
            refused.errors,
            vec![LinkError::Ambiguous {
                capability: "command behaviour",
                source: "billing.invoice.CreateInvoice",
                offered: vec![HONEST, "billing-realization/accepts-any-amount"],
            }],
            "the ambiguity names both claimants and nothing else failed"
        );
        let rendered = refused.to_string();
        assert!(
            rendered.contains(HONEST)
                && rendered.contains("billing-realization/accepts-any-amount")
                && rendered.contains("does not choose"),
            "the rendered refusal names both claimants and the rule: {rendered}"
        );
    }

    #[test]
    fn the_linkers_obligation_list_is_exactly_the_plans() {
        // The plan is the authority on what is owed; this list is only its spelling here. Held
        // equal mechanically, because a linker resolving a stale list would report an obligation
        // the plan no longer owes — or silently not owe one it does.
        let plan: serde_json::Value =
            serde_json::from_str(include_str!("../../../generated/rust/billing/plan.json"))
                .expect("the committed plan parses");
        let owed: BTreeSet<(String, String)> = plan["capabilities"]
            .as_array()
            .expect("the plan lists capabilities")
            .iter()
            .filter(|capability| capability["disposition"]["disposition"] == "obligation")
            .map(|capability| {
                let kind = match capability["kind"].as_str().expect("a capability kind") {
                    "command_behavior" => "command behaviour",
                    "view_query" => "view query",
                    "binding_escalation" => "binding escalation",
                    other => panic!(
                        "the plan owes a capability kind this linker has no slot family for: \
                         `{other}` — extend the linker"
                    ),
                };
                (
                    kind.to_owned(),
                    capability["source"]
                        .as_str()
                        .expect("a capability source")
                        .to_owned(),
                )
            })
            .collect();
        let listed: BTreeSet<(String, String)> = OBLIGATIONS
            .iter()
            .map(|(capability, source)| ((*capability).to_owned(), (*source).to_owned()))
            .collect();
        assert_eq!(
            listed, owed,
            "the linker's obligation list has drifted from the committed plan"
        );
    }
}
