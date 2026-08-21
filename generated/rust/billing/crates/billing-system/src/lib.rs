// generated from billing v3
// model digest 13577b3ce695932e980d418d5863bcde07f4c362516d53147870d31eaf2ed861
// contract digest d2b48060b7ee32e8f23b1e28972fea39921a25fdcacd635fdf7bbb538e94f367
// compiler 0.1.0 · generator 0.1.0
// do not edit: regenerate with `protocol ess synthesize`

//! The `billing` system, v3: its components assembled, its bindings wired, and its one transport.
//!
//! The transport is derived from the specification, not chosen: `at_least_once` is the only
//! delivery guarantee the model declares, so published events land on an append-only log and a
//! pump delivers each to every binding that reacts to it. The log is the system's observable
//! record, and so is the record of what each binding invoked. What no specification determines
//! — how an escalation event is filled, behaviour behind the ports — stays an obligation; see
//! the `PLAN.md` beside this workspace.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

/// An event on the system's log: everything any component publishes, and everything a binding
/// escalates into.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SystemEvent {
    /// `billing.email.DeliveryEscalated`.
    DeliveryEscalated(billing_types::email::DeliveryEscalated),
    /// `billing.email.EmailSent`.
    EmailSent(billing_types::email::EmailSent),
    /// `billing.invoice.InvoiceCancelled`.
    InvoiceCancelled(billing_types::invoice::InvoiceCancelled),
    /// `billing.invoice.InvoiceCreated`.
    InvoiceCreated(billing_types::invoice::InvoiceCreated),
    /// `billing.invoice.InvoiceIssued`.
    InvoiceIssued(billing_types::invoice::InvoiceIssued),
    /// `billing.invoice.InvoicePaid`.
    InvoicePaid(billing_types::invoice::InvoicePaid),
}

impl From<email_service::PublishedEvent> for SystemEvent {
    fn from(event: email_service::PublishedEvent) -> Self {
        match event {
            email_service::PublishedEvent::DeliveryEscalated(event) => Self::DeliveryEscalated(event),
            email_service::PublishedEvent::EmailSent(event) => Self::EmailSent(event),
        }
    }
}

impl From<invoice_service::PublishedEvent> for SystemEvent {
    fn from(event: invoice_service::PublishedEvent) -> Self {
        match event {
            invoice_service::PublishedEvent::InvoiceCancelled(event) => Self::InvoiceCancelled(event),
            invoice_service::PublishedEvent::InvoiceCreated(event) => Self::InvoiceCreated(event),
            invoice_service::PublishedEvent::InvoiceIssued(event) => Self::InvoiceIssued(event),
            invoice_service::PublishedEvent::InvoicePaid(event) => Self::InvoicePaid(event),
        }
    }
}

/// One command a binding invoked, and the input it passed — the transport's own record.
///
/// Recorded by the pump at the moment of invocation, so what a binding actually passed is
/// observable from outside — a conformance run holds a mapping to its words with exactly this —
/// without instrumenting the component underneath.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BindingInvocation {
    /// `notify-on-invoice-created` invoked `billing.email.SendEmail`.
    NotifyOnInvoiceCreated(billing_types::email::SendEmail),
}

/// The binding `notify-on-invoice-created`: `billing.invoice.InvoiceCreated`, read as `billing.email.SendEmail` input.
///
/// Fully determined by the specification: every input is filled from an event field — through the
/// declared crossing where one is named — from a literal the target admits, or left absent
/// where the input is optional and the binding says nothing.
pub fn notify_on_invoice_created(event: &billing_types::invoice::InvoiceCreated) -> billing_types::email::SendEmail {
    billing_types::email::SendEmail {
        recipient: billing_types::email::EmailAddress::from(event.customer_email.clone()),
        template: billing_types::email::TemplateId("invoice-created".to_owned()),
    }
}

/// What the system itself owes its implementor, as typed seams.
///
/// One trait per owed binding capability in the synthesis plan, each carrying the plan's own
/// contract. [`Unimplemented`](obligations::Unimplemented) satisfies every trait by refusing in the type system.
pub mod obligations {
    /// The escalation of `notify-on-invoice-created` — an implementation obligation.
    ///
    /// Why it is not generated: the contract is declared; the algorithm is not.
    ///
    /// Contract: the declared `billing.email.DeliveryEscalated`, recording that delivering `billing.email.SendEmail` for `notify-on-invoice-created` was given up on — the event is declared; how its fields are filled from the failed invocation is not.
    pub trait NotifyOnInvoiceCreatedEscalation {
        /// Builds the declared `billing.email.DeliveryEscalated` from the invocation that was given up on.
        ///
        /// `Err` is the typed refusal of an obligation nothing has satisfied; a satisfying
        /// implementation never returns it.
        fn notify_on_invoice_created_escalation(&self, failed: &billing_types::email::SendEmail) -> Result<billing_types::email::DeliveryEscalated, billing_types::obligation::UnmetObligation>;
    }

    /// Every obligation of the system, refused in the type system.
    ///
    /// Each method returns the typed refusal naming what is owed — never a panic, never a guessed
    /// value — so a system built on this stub compiles and reports its own gaps.
    pub struct Unimplemented;

    impl NotifyOnInvoiceCreatedEscalation for Unimplemented {
        fn notify_on_invoice_created_escalation(&self, _failed: &billing_types::email::SendEmail) -> Result<billing_types::email::DeliveryEscalated, billing_types::obligation::UnmetObligation> {
            Err(billing_types::obligation::UnmetObligation { capability: "binding escalation", source: "notify-on-invoice-created" })
        }
    }
}

/// The `billing` system: every component behind its port, and the transport between them.
///
/// The component fields are public because commands enter the system through a component's own
/// port; the log and its delivery cursor are not, because publishing happens by pumping, not by
/// writing history directly.
pub struct System<EmailServiceBehaviors, InvoiceServiceBehaviors, Obligations> {
    /// The `email-service` component.
    pub email_service: email_service::EmailService<EmailServiceBehaviors>,
    /// The `invoice-service` component.
    pub invoice_service: invoice_service::InvoiceService<InvoiceServiceBehaviors>,
    obligations: Obligations,
    invocations: Vec<BindingInvocation>,
    published: Vec<SystemEvent>,
    cursor: usize,
}

impl<EmailServiceBehaviors, InvoiceServiceBehaviors, Obligations> System<EmailServiceBehaviors, InvoiceServiceBehaviors, Obligations> {
    /// Assembles the system from its components and the owed obligations.
    pub fn new(email_service: email_service::EmailService<EmailServiceBehaviors>, invoice_service: invoice_service::InvoiceService<InvoiceServiceBehaviors>, obligations: Obligations) -> Self {
        Self {
            email_service,
            invoice_service,
            obligations,
            invocations: Vec::new(),
            published: Vec::new(),
            cursor: 0,
        }
    }

    /// Everything published so far, in publication order — the system's observable record.
    pub fn published(&self) -> &[SystemEvent] {
        &self.published
    }

    /// Every command a binding invoked so far, in invocation order, with what it passed.
    pub fn invocations(&self) -> &[BindingInvocation] {
        &self.invocations
    }
}

impl<EmailServiceBehaviors, InvoiceServiceBehaviors, Obligations> System<EmailServiceBehaviors, InvoiceServiceBehaviors, Obligations>
where
    EmailServiceBehaviors: billing_types::email::obligations::SendEmailBehavior,
    InvoiceServiceBehaviors: billing_types::invoice::obligations::CancelInvoiceBehavior + billing_types::invoice::obligations::CreateInvoiceBehavior + billing_types::invoice::obligations::IssueInvoiceBehavior + billing_types::invoice::obligations::PayInvoiceBehavior + billing_types::invoice::obligations::InvoiceByIdQuery + billing_types::invoice::obligations::OutstandingInvoicesQuery,
    Obligations: obligations::NotifyOnInvoiceCreatedEscalation,
{
    /// Delivers until quiescent: collects every component's outbox onto the log, then delivers
    /// each logged event to every binding that reacts to it — at least once each, which is the
    /// guarantee the specification declares.
    ///
    /// `Err` carries the first unmet obligation that delivery could not route around; the log
    /// keeps everything already published. A specification whose bindings feed each other
    /// without end will not quiesce, and this pump will not pretend otherwise.
    pub fn pump(&mut self) -> Result<(), billing_types::obligation::UnmetObligation> {
        loop {
            self.collect();
            if self.cursor == self.published.len() {
                return Ok(());
            }
            let event = self.published[self.cursor].clone();
            self.cursor += 1;
            self.deliver(&event)?;
        }
    }

    /// Delivers one already-published occurrence to every binding that reacts to it, again,
    /// then pumps until quiescent.
    ///
    /// The duplicate a delivery guarantee of at least once explicitly permits: the occurrence is
    /// not published a second time — a second occurrence would be a different claim — but
    /// every reacting binding runs again, and what that causes lands on the log as usual.
    pub fn redeliver(&mut self, event: &SystemEvent) -> Result<(), billing_types::obligation::UnmetObligation> {
        self.deliver(event)?;
        self.pump()
    }

    /// Moves every component's outbox onto the log, in component order.
    fn collect(&mut self) {
        for event in self.email_service.drain_outbox() {
            self.published.push(SystemEvent::from(event));
        }
        for event in self.invoice_service.drain_outbox() {
            self.published.push(SystemEvent::from(event));
        }
    }

    /// Delivers one logged event to every binding that reacts to it.
    fn deliver(&mut self, event: &SystemEvent) -> Result<(), billing_types::obligation::UnmetObligation> {
        match event {
            SystemEvent::DeliveryEscalated(_) => {}
            SystemEvent::EmailSent(_) => {}
            SystemEvent::InvoiceCancelled(_) => {}
            SystemEvent::InvoiceCreated(event) => {
                // `notify-on-invoice-created`: at_least_once, on failure escalate.
                let input = notify_on_invoice_created(event);
                self.invocations.push(BindingInvocation::NotifyOnInvoiceCreated(input.clone()));
                match self.email_service.send_email(input.clone())? {
                    billing_types::email::SendEmailOutcome::Sent { .. } => {}
                    billing_types::email::SendEmailOutcome::Failed { .. } => {
                        // The declared refusal is the failure the policy names: escalate.
                        let escalation = self.obligations.notify_on_invoice_created_escalation(&input)?;
                        self.published.push(SystemEvent::DeliveryEscalated(escalation));
                    }
                }
            }
            SystemEvent::InvoiceIssued(_) => {}
            SystemEvent::InvoicePaid(_) => {}
        }
        Ok(())
    }
}
