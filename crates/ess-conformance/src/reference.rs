//! The hand-written billing implementation, and the known-good target the suite is checked against.
//!
//! Design §24. Before a generated suite can be trusted, something has to pass it that is known to be
//! right — otherwise a green run says only that the suite and the implementation agree about
//! whatever they both got wrong. This is that something: `examples/billing/` implemented by hand,
//! in memory, in one file.
//!
//! It is **intentionally boring**. Its purpose is not to show how to build an invoicing service; it
//! is to be a target whose behaviour a reader can check against `examples/billing/domains/*.yaml`
//! line by line. No repository trait, no event bus, no ports and adapters — an entity is a struct in
//! a map, a projection is a filter over that map, and the one binding is a function call.
//!
//! # It is not privileged to cheat the suite
//!
//! §24's last line, and it is the one that matters. Two places where cheating would have been easy
//! and is refused:
//!
//! * **An eventual view is really eventual.** `billing.invoice.InvoiceById` declares
//!   `consistency: eventual`, so a write becomes visible to it only after
//!   [`Billing::DEFAULT_LAG`] further reads. Making it immediate would pass every scenario without
//!   any of them exercising the runner's bounded waiting — a suite that never waits never tests the
//!   word `eventual`, and the first real projection would find that out in production.
//!   `billing.invoice.OutstandingInvoices` declares `read_your_writes` and *is* immediate, because
//!   that is what the specification says.
//! * **A refusal the model does not declare is reported as one.** `IssueInvoice` on a `Paid`
//!   invoice reaches no declared outcome, so this answers
//!   [`SemanticCommandResult::undeclared`] rather than picking a branch that would make the
//!   assertion pass. What that costs is on that field: the model has no way to declare the outcome
//!   of a state violation, and this records the hole instead of papering over it.
//!
//! # Where the identifiers come from
//!
//! §37 forbids the target from minting anything the runner will compare, and an invoice id is the
//! one thing it must mint — the invoice does not exist when the command is issued, which is exactly
//! why `creates:` publishes the new identity in an event and why
//! [`CaptureInstance`](crate::scenario::ScenarioStep::CaptureInstance) exists. So the ids here are a
//! per-scenario counter, reset by [`begin_scenario`](ConformanceTarget::begin_scenario): the runner
//! never compares one against an expected value, and two runs still produce the same ones.

use std::cell::RefCell;
use std::collections::BTreeMap;

use aep_contract::consistency::ConsistencyToken;
use aep_domain::ids::CorrelationId;
use aep_domain::node::Node;

use crate::scenario::{BindingRef, CommandRef, ErrorRef, EventRef, OutcomeRef};
use crate::target::{
    ConformanceTarget, DeclaredErrorValue, EventObservationRequest, ExternalOutcomeControl,
    ImplementationIdentity, InvocationObservationRequest, ObservedEvent, ObservedInvocation,
    RedeliveryRequest, ScenarioContext, SemanticCommandRequest, SemanticCommandResult,
    SemanticViewRequest, SemanticViewResult, TargetError, ViewRow,
};

// ---- the names the specification declares -----------------------------------------------------

/// `billing.invoice.CreateInvoice`.
const CREATE_INVOICE: &str = "billing.invoice.CreateInvoice";
/// `billing.invoice.IssueInvoice`.
const ISSUE_INVOICE: &str = "billing.invoice.IssueInvoice";
/// `billing.invoice.PayInvoice`.
const PAY_INVOICE: &str = "billing.invoice.PayInvoice";
/// `billing.invoice.CancelInvoice`.
const CANCEL_INVOICE: &str = "billing.invoice.CancelInvoice";
/// `billing.email.SendEmail`.
const SEND_EMAIL: &str = "billing.email.SendEmail";

/// `billing.invoice.InvalidAmount`.
const INVALID_AMOUNT: &str = "billing.invoice.InvalidAmount";
/// `billing.email.Undeliverable`.
const UNDELIVERABLE: &str = "billing.email.Undeliverable";

/// `billing.invoice.InvoiceCreated`.
const INVOICE_CREATED: &str = "billing.invoice.InvoiceCreated";
/// `billing.invoice.InvoiceIssued`.
const INVOICE_ISSUED: &str = "billing.invoice.InvoiceIssued";
/// `billing.invoice.InvoicePaid`.
const INVOICE_PAID: &str = "billing.invoice.InvoicePaid";
/// `billing.invoice.InvoiceCancelled`.
const INVOICE_CANCELLED: &str = "billing.invoice.InvoiceCancelled";
/// `billing.email.EmailSent`.
const EMAIL_SENT: &str = "billing.email.EmailSent";
/// `billing.email.DeliveryEscalated`.
const DELIVERY_ESCALATED: &str = "billing.email.DeliveryEscalated";

/// `billing.invoice.InvoiceById`, the projection.
const INVOICE_BY_ID: &str = "billing.invoice.InvoiceById";
/// `billing.invoice.OutstandingInvoices`, read-your-writes.
const OUTSTANDING: &str = "billing.invoice.OutstandingInvoices";

/// `notify-on-invoice-created`.
const NOTIFY: &str = "notify-on-invoice-created";

/// The template the binding's mapping names.
const TEMPLATE: &str = "invoice-created";

// ---- the implementation -----------------------------------------------------------------------

/// `examples/billing/`, implemented by hand and in memory.
#[derive(Debug)]
pub struct Billing {
    lag: u64,
    state: RefCell<State>,
}

impl Billing {
    /// How many further reads a write takes to reach the `eventual` projection.
    ///
    /// Two, so that a scenario asserting `billing.invoice.InvoiceById` genuinely has to ask more
    /// than once. Nothing sleeps: the runner's clock advances on being read, so "wait" here means
    /// "ask again", which is what §40 means by bounded polling.
    pub const DEFAULT_LAG: u64 = 2;

    /// The implementation, with the default projection lag.
    pub fn new() -> Self {
        Self::with_lag(Self::DEFAULT_LAG)
    }

    /// The implementation, with a projection that catches up after `lag` further reads.
    ///
    /// `0` makes the projection immediate, which is a *weaker* target: every eventual assertion then
    /// passes on its first ask, so nothing exercises the deadline. It exists so a test can hold both
    /// ends of the range rather than as a default.
    pub fn with_lag(lag: u64) -> Self {
        Self {
            lag,
            state: RefCell::new(State::default()),
        }
    }

    /// `billing.invoice.CreateInvoice`: `accepted` when `amount.amount > 0`, `rejected` otherwise.
    fn create_invoice(
        &self,
        state: &mut State,
        request: &SemanticCommandRequest,
    ) -> SemanticCommandResult {
        let amount = request.input.get("amount").cloned().unwrap_or_default();
        if !positive(&amount) {
            return SemanticCommandResult::took(outcome(CREATE_INVOICE, "rejected")).with_error(
                DeclaredErrorValue::new(declared_error(INVALID_AMOUNT)).with("submitted", amount),
            );
        }

        let id = state.identifier();
        let visible_after = state.projection_reads + self.lag;
        state.invoices.insert(
            id.clone(),
            Invoice {
                id: id.clone(),
                total: amount.clone(),
                state: Lifecycle::Draft,
                visible_after,
            },
        );

        let created = ObservedEvent::new(event(INVOICE_CREATED))
            .with("invoice_id", Node::Text(id))
            .with(
                "customer_email",
                request
                    .input
                    .get("customer_email")
                    .cloned()
                    .unwrap_or_default(),
            )
            .with("amount", amount)
            .in_activity(request.correlation.clone());
        state.publish_now(created.clone());
        self.notify(state, &created, &request.correlation);

        let token = state.token();
        SemanticCommandResult::took(outcome(CREATE_INVOICE, "accepted"))
            .with_consistency(token)
            .emitting(created)
    }

    /// Runs the one binding this system declares: `notify-on-invoice-created`.
    ///
    /// `when: InvoiceCreated` → `invoke: SendEmail` with `recipient: event.customer_email` and
    /// `template: invoice-created`, `delivery: at_least_once`, and on failure `escalate`, which
    /// publishes `billing.email.DeliveryEscalated`.
    fn notify(&self, state: &mut State, created: &ObservedEvent, correlation: &CorrelationId) {
        let recipient = created
            .payload
            .get("customer_email")
            .cloned()
            .unwrap_or_default();
        let template = Node::Text(TEMPLATE.to_owned());
        state.invocations.push(
            ObservedInvocation::new(binding(NOTIFY), command(SEND_EMAIL))
                .with("recipient", recipient.clone())
                .with("template", template.clone()),
        );

        let published = match state.send_email(&recipient, correlation) {
            Some(sent) => sent,
            None => ObservedEvent::new(event(DELIVERY_ESCALATED))
                .with("recipient", recipient)
                .with("template", template)
                .in_activity(correlation.clone()),
        };
        // A consequence of the binding is not a consequence of the command the caller issued, so it
        // becomes observable through `observe_events` and after the same lag any other
        // cross-component consequence takes.
        state.publish_later(published, self.lag);
    }
}

impl Default for Billing {
    fn default() -> Self {
        Self::new()
    }
}

// ---- what it holds ------------------------------------------------------------------------------

/// One invoice, and when a write to it reaches the projection.
#[derive(Debug, Clone)]
struct Invoice {
    id: String,
    total: Node,
    state: Lifecycle,
    visible_after: u64,
}

/// `billing.invoice.Invoice`'s declared states.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Lifecycle {
    Draft,
    Issued,
    Paid,
    Cancelled,
}

/// One event this implementation published, and when it becomes observable.
#[derive(Debug, Clone)]
struct Published {
    event: ObservedEvent,
    visible_after: u64,
}

/// One scenario's isolated execution context.
#[derive(Debug, Default)]
struct State {
    invoices: BTreeMap<String, Invoice>,
    log: Vec<Published>,
    invocations: Vec<ObservedInvocation>,
    forced: Option<OutcomeRef>,
    sequence: u64,
    event_reads: u64,
    projection_reads: u64,
}

impl State {
    /// The next value of this context's counter, which every generated identifier comes from.
    fn tick(&mut self) -> u64 {
        self.sequence += 1;
        self.sequence
    }

    /// An identifier of the shape the model's `Uuid` primitive takes.
    fn identifier(&mut self) -> String {
        format!("00000000-0000-4000-8000-{:012}", self.tick())
    }

    /// The token a read may demand a view no older than.
    ///
    /// # Panics
    ///
    /// It does not: the token is `seq:` and decimal digits, which `ConsistencyToken` accepts.
    fn token(&mut self) -> ConsistencyToken {
        let sequence = self.tick();
        ConsistencyToken::new(format!("seq:{sequence}"))
            .unwrap_or_else(|error| panic!("a generated consistency token is well formed: {error}"))
    }

    /// Publishes an event that is observable at once — one the caller's own command produced.
    fn publish_now(&mut self, event: ObservedEvent) {
        let sequence = self.tick();
        self.log.push(Published {
            event: event.at(sequence),
            visible_after: 0,
        });
    }

    /// Publishes an event that becomes observable after `lag` further observations.
    fn publish_later(&mut self, event: ObservedEvent, lag: u64) {
        let sequence = self.tick();
        let visible_after = self.event_reads + lag;
        self.log.push(Published {
            event: event.at(sequence),
            visible_after,
        });
    }

    /// Answers `billing.email.SendEmail`, honouring a forced external failure.
    ///
    /// `None` is the declared `failed` branch: `external: the provider rejects the recipient
    /// address`, which no input decides and the suite therefore injects.
    fn send_email(
        &mut self,
        recipient: &Node,
        correlation: &CorrelationId,
    ) -> Option<ObservedEvent> {
        if self.take_forced(SEND_EMAIL, "failed") {
            return None;
        }
        let message_id = self.identifier();
        Some(
            ObservedEvent::new(event(EMAIL_SENT))
                .with("message_id", Node::Text(message_id))
                .with("recipient", recipient.clone())
                .in_activity(correlation.clone()),
        )
    }

    /// Consumes a forced outcome, if one was configured for this command and branch.
    ///
    /// It applies to the **next** invocation and then lapses, because that is what
    /// [`ConfigureExternalOutcome`](crate::scenario::ScenarioStep::ConfigureExternalOutcome) says:
    /// "the outcome the adapter must produce next".
    fn take_forced(&mut self, command_name: &str, branch: &str) -> bool {
        let wanted = self
            .forced
            .as_ref()
            .is_some_and(|forced| forced.to_string() == format!("{command_name}/{branch}"));
        if wanted {
            self.forced = None;
        }
        wanted
    }

    /// Records a change to an invoice, and when the projection will show it.
    fn touch(&mut self, id: &str, lag: u64) {
        let visible_after = self.projection_reads + lag;
        if let Some(invoice) = self.invoices.get_mut(id) {
            invoice.visible_after = visible_after;
        }
    }
}

// ---- the target ---------------------------------------------------------------------------------

impl ConformanceTarget for Billing {
    fn identity(&self) -> Result<ImplementationIdentity, TargetError> {
        Ok(ImplementationIdentity::new(
            "billing-reference",
            env!("CARGO_PKG_VERSION"),
        ))
    }

    fn begin_scenario(&self, _scenario: &ScenarioContext) -> Result<(), TargetError> {
        // Isolation, the cheapest way §8 permits: a fresh runtime. Nothing from the previous
        // scenario survives, so no observation can satisfy the wrong one.
        *self.state.borrow_mut() = State::default();
        Ok(())
    }

    fn execute_command(
        &self,
        request: SemanticCommandRequest,
    ) -> Result<SemanticCommandResult, TargetError> {
        let mut state = self.state.borrow_mut();
        match request.command.to_string().as_str() {
            CREATE_INVOICE => Ok(self.create_invoice(&mut state, &request)),
            ISSUE_INVOICE => Ok(issue_invoice(&mut state, &request, self.lag)),
            CANCEL_INVOICE => Ok(cancel_invoice(&mut state, &request, self.lag)),
            PAY_INVOICE => Ok(pay_invoice(&mut state, &request, self.lag)),
            SEND_EMAIL => Ok(send_email(&mut state, &request)),
            other => Err(TargetError::unavailable(
                format!("invoking `{other}`"),
                "this implementation accepts only the commands `examples/billing/` declares"
                    .to_owned(),
            )),
        }
    }

    fn query_view(&self, request: SemanticViewRequest) -> Result<SemanticViewResult, TargetError> {
        let mut state = self.state.borrow_mut();
        match request.view.to_string().as_str() {
            // `read_your_writes`: a caller that has just issued an invoice and cannot see it in its
            // own list has been told a lie about what it did. There is nothing to wait for.
            OUTSTANDING => Ok(SemanticViewResult::of(
                state
                    .invoices
                    .values()
                    .filter(|invoice| invoice.state == Lifecycle::Issued)
                    .map(row),
            )),
            // `eventual`. A read demanding `AtLeast(token)` is the one case where the target waits
            // until it can answer (§14, §15) — here, by catching the projection up rather than by
            // sleeping. A read at `Current` gets what the projection has, which is the point.
            INVOICE_BY_ID => {
                if request.consistency.token().is_some() {
                    let caught_up = state
                        .invoices
                        .values()
                        .map(|invoice| invoice.visible_after)
                        .max()
                        .unwrap_or(0);
                    state.projection_reads = state.projection_reads.max(caught_up);
                } else {
                    state.projection_reads += 1;
                }
                let reads = state.projection_reads;
                Ok(SemanticViewResult::of(
                    state
                        .invoices
                        .values()
                        .filter(|invoice| invoice.visible_after <= reads)
                        .map(row),
                ))
            }
            other => Err(TargetError::unavailable(
                format!("reading `{other}`"),
                "this implementation projects only the views `examples/billing/` declares"
                    .to_owned(),
            )),
        }
    }

    fn observe_events(
        &self,
        request: EventObservationRequest,
    ) -> Result<Vec<ObservedEvent>, TargetError> {
        let mut state = self.state.borrow_mut();
        state.event_reads += 1;
        let reads = state.event_reads;
        Ok(state
            .log
            .iter()
            .filter(|published| {
                published.event.event == request.event && published.visible_after <= reads
            })
            .map(|published| published.event.clone())
            .collect())
    }

    fn configure_external_outcome(
        &self,
        request: ExternalOutcomeControl,
    ) -> Result<(), TargetError> {
        let forced = request.force.to_string();
        if forced != format!("{SEND_EMAIL}/failed") {
            return Err(TargetError::unavailable(
                format!("forcing `{forced}`"),
                format!("`{SEND_EMAIL}/failed` is the only outcome this system declares external"),
            ));
        }
        self.state.borrow_mut().forced = Some(request.force);
        Ok(())
    }

    fn redeliver_event(&self, request: RedeliveryRequest) -> Result<(), TargetError> {
        let mut state = self.state.borrow_mut();
        let Some(occurrence) = state
            .log
            .iter()
            .rev()
            .find(|published| published.event.event == request.event)
            .map(|published| published.event.clone())
        else {
            return Err(TargetError::unavailable(
                format!("delivering `{}` again", request.event),
                "this context has not published that event".to_owned(),
            ));
        };
        // Only the bindings run again. Re-publishing the occurrence would be a second occurrence,
        // which is a different claim from the one `at_least_once` makes.
        if occurrence.event == event(INVOICE_CREATED) {
            self.notify(&mut state, &occurrence, &request.correlation);
        }
        Ok(())
    }

    fn observe_invocations(
        &self,
        request: InvocationObservationRequest,
    ) -> Result<Vec<ObservedInvocation>, TargetError> {
        let state = self.state.borrow();
        Ok(state
            .invocations
            .iter()
            .filter(|invocation| {
                invocation.binding == request.binding && invocation.command == request.command
            })
            .cloned()
            .collect())
    }

    fn end_scenario(&self, _scenario: &ScenarioContext) -> Result<(), TargetError> {
        *self.state.borrow_mut() = State::default();
        Ok(())
    }
}

/// `billing.invoice.IssueInvoice`: `issue` runs from `Draft` and from nowhere else.
///
/// A `Draft` invoice becomes `Issued`. Any other state reaches no declared outcome — see
/// [`SemanticCommandResult::outcome`], which is where the model's hole in that case is recorded.
fn issue_invoice(
    state: &mut State,
    request: &SemanticCommandRequest,
    lag: u64,
) -> SemanticCommandResult {
    let Some(id) = instance(request) else {
        return SemanticCommandResult::undeclared();
    };
    if state.invoices.get(&id).map(|invoice| invoice.state) != Some(Lifecycle::Draft) {
        return SemanticCommandResult::undeclared();
    }
    if let Some(invoice) = state.invoices.get_mut(&id) {
        invoice.state = Lifecycle::Issued;
    }
    state.touch(&id, lag);

    let published = ObservedEvent::new(event(INVOICE_ISSUED))
        .with("invoice_id", Node::Text(id))
        .in_activity(request.correlation.clone());
    state.publish_now(published.clone());
    let token = state.token();
    SemanticCommandResult::took(outcome(ISSUE_INVOICE, "issued"))
        .with_consistency(token)
        .emitting(published)
}

/// `billing.invoice.CancelInvoice`: `cancel` runs from `Draft` and from `Issued`.
fn cancel_invoice(
    state: &mut State,
    request: &SemanticCommandRequest,
    lag: u64,
) -> SemanticCommandResult {
    let Some(id) = instance(request) else {
        return SemanticCommandResult::undeclared();
    };
    let current = state.invoices.get(&id).map(|invoice| invoice.state);
    if !matches!(current, Some(Lifecycle::Draft | Lifecycle::Issued)) {
        return SemanticCommandResult::undeclared();
    }
    if let Some(invoice) = state.invoices.get_mut(&id) {
        invoice.state = Lifecycle::Cancelled;
    }
    state.touch(&id, lag);

    let published = ObservedEvent::new(event(INVOICE_CANCELLED))
        .with("invoice_id", Node::Text(id))
        .in_activity(request.correlation.clone());
    state.publish_now(published.clone());
    let token = state.token();
    SemanticCommandResult::took(outcome(CANCEL_INVOICE, "cancelled"))
        .with_consistency(token)
        .emitting(published)
}

/// `billing.invoice.PayInvoice`: the amount decides the branch, and the state decides the move.
///
/// The order matters and it is the specification's: `settled` is guarded by
/// `when: amount.amount > 0` and `rejected` is the branch with no guard, so a non-positive amount is
/// refused whatever state the invoice is in — which is what the suite's
/// `billing.invoice.PayInvoice/outcome/rejected` scenario relies on when it pays an invoice that
/// does not exist.
fn pay_invoice(
    state: &mut State,
    request: &SemanticCommandRequest,
    lag: u64,
) -> SemanticCommandResult {
    let amount = request.input.get("amount").cloned().unwrap_or_default();
    if !positive(&amount) {
        return SemanticCommandResult::took(outcome(PAY_INVOICE, "rejected")).with_error(
            DeclaredErrorValue::new(declared_error(INVALID_AMOUNT)).with("submitted", amount),
        );
    }
    let Some(id) = instance(request) else {
        return SemanticCommandResult::undeclared();
    };
    if state.invoices.get(&id).map(|invoice| invoice.state) != Some(Lifecycle::Issued) {
        return SemanticCommandResult::undeclared();
    }
    if let Some(invoice) = state.invoices.get_mut(&id) {
        invoice.state = Lifecycle::Paid;
    }
    state.touch(&id, lag);

    let published = ObservedEvent::new(event(INVOICE_PAID))
        .with("invoice_id", Node::Text(id))
        .with("amount", amount)
        .in_activity(request.correlation.clone());
    state.publish_now(published.clone());
    let token = state.token();
    SemanticCommandResult::took(outcome(PAY_INVOICE, "settled"))
        .with_consistency(token)
        .emitting(published)
}

/// `billing.email.SendEmail`, invoked directly rather than by the binding.
fn send_email(state: &mut State, request: &SemanticCommandRequest) -> SemanticCommandResult {
    let recipient = request.input.get("recipient").cloned().unwrap_or_default();
    match state.send_email(&recipient, &request.correlation) {
        Some(sent) => {
            state.publish_now(sent.clone());
            let token = state.token();
            SemanticCommandResult::took(outcome(SEND_EMAIL, "sent"))
                .with_consistency(token)
                .emitting(sent)
        }
        None => SemanticCommandResult::took(outcome(SEND_EMAIL, "failed"))
            .with_error(DeclaredErrorValue::new(declared_error(UNDELIVERABLE))),
    }
}

/// The invoice a command's `instance:` field names.
fn instance(request: &SemanticCommandRequest) -> Option<String> {
    request
        .input
        .get("invoice_id")
        .and_then(|value| value.as_text())
        .map(ToOwned::to_owned)
}

/// One row of either view: what both of them project.
fn row(invoice: &Invoice) -> ViewRow {
    let mut fields = ViewRow::new();
    fields.insert("invoice_id".to_owned(), Node::Text(invoice.id.clone()));
    fields.insert("total".to_owned(), invoice.total.clone());
    fields
}

/// `amount.amount > 0`, read off a `billing.invoice.Money`.
fn positive(amount: &Node) -> bool {
    amount
        .as_map()
        .and_then(|fields| fields.get("amount"))
        .is_some_and(|value| match value {
            Node::Number(number) => number.get() > 0.0,
            _ => false,
        })
}

// ---- names, parsed once -------------------------------------------------------------------------

/// Declares one `&str` → reference helper per line, each panicking on a name this crate wrote wrong.
///
/// A panic rather than a `Result`: these are literals in this file, checked against
/// `examples/billing/`, so a malformed one is a defect in this module and not a condition a caller
/// can do anything about.
macro_rules! parsed {
    ($($name:ident -> $kind:ty, $what:literal;)*) => {
        $(
            #[doc = concat!("Parses ", $what, " this module names as a literal.")]
            fn $name(value: &str) -> $kind {
                value.parse().unwrap_or_else(|error| {
                    panic!("`{value}` is a well-formed {}: {error}", $what)
                })
            }
        )*
    };
}

parsed! {
    command -> CommandRef, "a command";
    event -> EventRef, "an event";
    declared_error -> ErrorRef, "a declared error";
    binding -> BindingRef, "a binding";
}

/// One branch of one command, from the two names this module writes.
fn outcome(command_name: &str, branch: &str) -> OutcomeRef {
    OutcomeRef::new(
        command(command_name),
        branch
            .parse()
            .unwrap_or_else(|error| panic!("`{branch}` is a well-formed outcome name: {error}")),
    )
}
