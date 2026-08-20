//! The committed billing suite, unchanged, against the synthesised system — W6.3, executed.
//!
//! Wave 6's only acceptance criterion that matters, and it is executed rather than asserted:
//! `suites/generated/billing/suite.json`, exactly as wave 4 committed it, runs against the
//! generated workspace linked with this crate's hand-written obligation implementations, and
//! passes 29 of 29. Then the falsifiability half: the *same* suite, against the linkage carrying
//! the one deliberately corrupted obligation, fails exactly the scenario that exists to catch it.
//!
//! # The bridge is an adapter, not an implementation
//!
//! [`Synthesized`] implements wave 4's [`ConformanceTarget`] over the linked system. Its whole
//! job is representation: suite values ([`Node`]) into the generated types on the way in, typed
//! events and rows back into nodes on the way out, in the same wire spellings the reference
//! target established — ids and text as `Text`, a `Money` as a map whose `amount` is a `Number`.
//! Every observation it reports is read off the system — the log, the invocation record, the view
//! ports; nothing is computed here that the system did not do.
//!
//! One boundary decision lives here and is argued at [`Synthesized::subject_is_unknown`]: a
//! command against a subject the system has never seen answers "no declared outcome", which the
//! generated behaviour seam cannot spell — a recorded finding about the generator.

use std::cell::RefCell;
use std::collections::BTreeMap;

use aep_contract::consistency::ConsistencyToken;
use aep_domain::facts::Number;
use aep_domain::node::Node;
use billing_realization::corrupted::{CAUGHT_BY, FAULT};
use billing_realization::invoice::positive;
use billing_realization::linker::{self, Assembled};
use billing_system::{BindingInvocation, SystemEvent};
use billing_types::invoice::{
    CancelInvoice, CancelInvoiceOutcome, CreateInvoice, CreateInvoiceOutcome, Email, InvoiceId,
    InvoiceState, IssueInvoice, IssueInvoiceOutcome, Money, PayInvoice, PayInvoiceOutcome,
};
use billing_types::primitives::{Decimal, Uuid};
use ess_conformance::report::{ConformanceStatus, Status};
use ess_conformance::runner::Runner;
use ess_conformance::scenario::{BindingRef, CommandRef, ConformanceSuite, ErrorRef, EventRef};
use ess_conformance::target::{
    ConformanceTarget, DeclaredErrorValue, EventObservationRequest, ExternalOutcomeControl,
    ImplementationIdentity, InvocationObservationRequest, ObservedEvent, ObservedInvocation,
    RedeliveryRequest, ScenarioContext, SemanticCommandRequest, SemanticCommandResult,
    SemanticViewRequest, SemanticViewResult, TargetError, ViewRow,
};

// ---- the names the specification declares ----------------------------------------------------

const CREATE_INVOICE: &str = "billing.invoice.CreateInvoice";
const ISSUE_INVOICE: &str = "billing.invoice.IssueInvoice";
const CANCEL_INVOICE: &str = "billing.invoice.CancelInvoice";
const PAY_INVOICE: &str = "billing.invoice.PayInvoice";
const SEND_EMAIL: &str = "billing.email.SendEmail";

const INVALID_AMOUNT: &str = "billing.invoice.InvalidAmount";
const INVOICE_STATE_CONFLICT: &str = "billing.invoice.InvoiceStateConflict";
const UNDELIVERABLE: &str = "billing.email.Undeliverable";

const INVOICE_CREATED: &str = "billing.invoice.InvoiceCreated";
const INVOICE_ISSUED: &str = "billing.invoice.InvoiceIssued";
const INVOICE_PAID: &str = "billing.invoice.InvoicePaid";
const INVOICE_CANCELLED: &str = "billing.invoice.InvoiceCancelled";
const EMAIL_SENT: &str = "billing.email.EmailSent";
const DELIVERY_ESCALATED: &str = "billing.email.DeliveryEscalated";

const INVOICE_BY_ID: &str = "billing.invoice.InvoiceById";
const OUTSTANDING: &str = "billing.invoice.OutstandingInvoices";

const NOTIFY: &str = "notify-on-invoice-created";

// ---- the target ------------------------------------------------------------------------------

/// Which linkage a run drives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Build {
    /// Every obligation honest.
    Honest,
    /// `CreateInvoice` swapped for the corrupted guard; everything else honest.
    Corrupted,
}

/// One scenario's linked system, plus the adapter's own token mint.
struct Live {
    assembled: Assembled,
    sequence: u64,
}

/// The generated billing workspace, linked and adapted to wave 4's target interface.
struct Synthesized {
    build: Build,
    live: RefCell<Option<Live>>,
}

impl Synthesized {
    /// The honest linkage.
    fn honest() -> Self {
        Self {
            build: Build::Honest,
            live: RefCell::new(None),
        }
    }

    /// The linkage carrying the one deliberate lie.
    fn corrupted() -> Self {
        Self {
            build: Build::Corrupted,
            live: RefCell::new(None),
        }
    }
}

/// A borrow of the open scenario, or the refusal that none is open.
fn open(live: &mut Option<Live>) -> Result<&mut Live, TargetError> {
    live.as_mut()
        .ok_or_else(|| TargetError::unavailable("driving the system", "no scenario is open (§8)"))
}

/// The next consistency token of this scenario — the adapter's, as §37 places it: derived from a
/// counter, never from a clock.
fn token(live: &mut Live) -> Result<ConsistencyToken, TargetError> {
    live.sequence += 1;
    ConsistencyToken::new(format!("seq:{}", live.sequence))
        .map_err(|error| TargetError::unavailable("minting a consistency token", error.to_string()))
}

/// Settles the system after a command: every outbox collected, every binding delivered.
fn pump(live: &mut Live) -> Result<(), TargetError> {
    live.assembled
        .system
        .pump()
        .map_err(|unmet| TargetError::unavailable("pumping the transport", unmet.to_string()))
}

impl ConformanceTarget for Synthesized {
    fn identity(&self) -> Result<ImplementationIdentity, TargetError> {
        // The corrupted build names its fault, as `ess-conformance`'s faulty targets do: a report
        // that said `billing-synthesized` for a build that is deliberately wrong would attest the
        // opposite of what happened.
        let name = match self.build {
            Build::Honest => "billing-synthesized".to_owned(),
            Build::Corrupted => format!("billing-synthesized-{FAULT}"),
        };
        Ok(ImplementationIdentity::new(name, env!("CARGO_PKG_VERSION")))
    }

    fn begin_scenario(&self, _scenario: &ScenarioContext) -> Result<(), TargetError> {
        // Isolation, the cheapest way §8 permits: a freshly linked system per scenario. The
        // suite runs against what the *linker* produced, every time.
        *self.live.borrow_mut() = Some(Live {
            assembled: match self.build {
                Build::Honest => linker::honest(),
                Build::Corrupted => linker::corrupted(),
            },
            sequence: 0,
        });
        Ok(())
    }

    fn execute_command(
        &self,
        request: SemanticCommandRequest,
    ) -> Result<SemanticCommandResult, TargetError> {
        let mut live = self.live.borrow_mut();
        let live = open(&mut live)?;
        let command = request.command.to_string();
        let result = match command.as_str() {
            CREATE_INVOICE => create_invoice(live, &request)?,
            ISSUE_INVOICE => issue_invoice(live, &request)?,
            CANCEL_INVOICE => cancel_invoice(live, &request)?,
            PAY_INVOICE => pay_invoice(live, &request)?,
            SEND_EMAIL => send_email(live, &request)?,
            other => {
                return Err(TargetError::unavailable(
                    format!("invoking `{other}`"),
                    "this system accepts only the commands `examples/billing/` declares",
                ))
            }
        };
        // Deliver what the command set in motion before the runner observes anything.
        pump(live)?;
        Ok(result)
    }

    fn query_view(&self, request: SemanticViewRequest) -> Result<SemanticViewResult, TargetError> {
        let mut live = self.live.borrow_mut();
        let live = open(&mut live)?;
        // Both projections are served straight off the store, so every consistency the request
        // can demand — `Current`, or `AtLeast` a token this adapter minted — is already met.
        match request.view.to_string().as_str() {
            INVOICE_BY_ID => {
                let rows = live
                    .assembled
                    .system
                    .invoice_service
                    .invoice_by_id()
                    .map_err(|unmet| {
                        TargetError::unavailable("reading the projection", unmet.to_string())
                    })?;
                Ok(SemanticViewResult::of(
                    rows.iter().map(|row| row_of(&row.invoice_id, &row.total)),
                ))
            }
            OUTSTANDING => {
                let rows = live
                    .assembled
                    .system
                    .invoice_service
                    .outstanding_invoices()
                    .map_err(|unmet| {
                        TargetError::unavailable("reading the projection", unmet.to_string())
                    })?;
                Ok(SemanticViewResult::of(
                    rows.iter().map(|row| row_of(&row.invoice_id, &row.total)),
                ))
            }
            other => Err(TargetError::unavailable(
                format!("reading `{other}`"),
                "this system projects only the views `examples/billing/` declares",
            )),
        }
    }

    fn observe_events(
        &self,
        request: EventObservationRequest,
    ) -> Result<Vec<ObservedEvent>, TargetError> {
        let mut live = self.live.borrow_mut();
        let live = open(&mut live)?;
        Ok(live
            .assembled
            .system
            .published()
            .iter()
            .enumerate()
            .map(|(position, event)| observed(event, position as u64, &request))
            .filter(|event| event.event == request.event)
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
        let mut live = self.live.borrow_mut();
        let live = open(&mut live)?;
        live.assembled.email.fail_next();
        Ok(())
    }

    fn redeliver_event(&self, request: RedeliveryRequest) -> Result<(), TargetError> {
        let mut live = self.live.borrow_mut();
        let live = open(&mut live)?;
        let wanted = request.event.to_string();
        let Some(occurrence) = live
            .assembled
            .system
            .published()
            .iter()
            .rev()
            .find(|event| event_name(event) == wanted)
            .cloned()
        else {
            return Err(TargetError::unavailable(
                format!("delivering `{wanted}` again"),
                "this context has not published that event",
            ));
        };
        live.assembled
            .system
            .redeliver(&occurrence)
            .map_err(|unmet| {
                TargetError::unavailable("delivering the occurrence again", unmet.to_string())
            })
    }

    fn observe_invocations(
        &self,
        request: InvocationObservationRequest,
    ) -> Result<Vec<ObservedInvocation>, TargetError> {
        let mut live = self.live.borrow_mut();
        let live = open(&mut live)?;
        // Read off the transport's own record — the system saying what its bindings passed —
        // never recomputed here, which would report the mapping as written rather than as run.
        Ok(live
            .assembled
            .system
            .invocations()
            .iter()
            .map(|invocation| match invocation {
                BindingInvocation::NotifyOnInvoiceCreated(input) => {
                    ObservedInvocation::new(binding_ref(NOTIFY), command_ref(SEND_EMAIL))
                        .with("recipient", Node::Text(input.recipient.0.clone()))
                        .with("template", Node::Text(input.template.0.clone()))
                }
            })
            .filter(|invocation| {
                invocation.binding == request.binding && invocation.command == request.command
            })
            .collect())
    }

    fn end_scenario(&self, _scenario: &ScenarioContext) -> Result<(), TargetError> {
        *self.live.borrow_mut() = None;
        Ok(())
    }
}

// ---- commands, one function each -------------------------------------------------------------

/// `billing.invoice.CreateInvoice`, through the generated port.
fn create_invoice(
    live: &mut Live,
    request: &SemanticCommandRequest,
) -> Result<SemanticCommandResult, TargetError> {
    let input = CreateInvoice {
        customer_email: Email(text_input(request, "customer_email")?),
        amount: money_input(request, "amount")?,
    };
    let outcome = live
        .assembled
        .system
        .invoice_service
        .create_invoice(input)
        .map_err(|refusal| unmet(&refusal))?;
    Ok(match outcome {
        CreateInvoiceOutcome::Accepted { invoice_created } => {
            let published = ObservedEvent::new(event_ref(INVOICE_CREATED))
                .with(
                    "invoice_id",
                    Node::Text(invoice_created.invoice_id.0 .0.clone()),
                )
                .with(
                    "customer_email",
                    Node::Text(invoice_created.customer_email.0.clone()),
                )
                .with("amount", money_node(&invoice_created.amount))
                .in_activity(request.correlation.clone());
            SemanticCommandResult::took(outcome_ref(CREATE_INVOICE, "accepted"))
                .with_consistency(token(live)?)
                .emitting(published)
        }
        CreateInvoiceOutcome::Rejected { error } => {
            SemanticCommandResult::took(outcome_ref(CREATE_INVOICE, "rejected")).with_error(
                DeclaredErrorValue::new(error_ref(INVALID_AMOUNT))
                    .with("submitted", money_node(&error.submitted)),
            )
        }
    })
}

/// `billing.invoice.IssueInvoice`, through the generated port.
fn issue_invoice(
    live: &mut Live,
    request: &SemanticCommandRequest,
) -> Result<SemanticCommandResult, TargetError> {
    let invoice_id = invoice_id_input(request)?;
    if subject_is_unknown(live, &invoice_id) {
        return Ok(SemanticCommandResult::undeclared());
    }
    let outcome = live
        .assembled
        .system
        .invoice_service
        .issue_invoice(IssueInvoice {
            invoice_id: invoice_id.clone(),
        })
        .map_err(|refusal| unmet(&refusal))?;
    Ok(match outcome {
        IssueInvoiceOutcome::Issued { invoice_issued } => {
            let published = ObservedEvent::new(event_ref(INVOICE_ISSUED))
                .with(
                    "invoice_id",
                    Node::Text(invoice_issued.invoice_id.0 .0.clone()),
                )
                .in_activity(request.correlation.clone());
            SemanticCommandResult::took(outcome_ref(ISSUE_INVOICE, "issued"))
                .with_consistency(token(live)?)
                .emitting(published)
        }
        IssueInvoiceOutcome::WrongState { error } => wrong_state(ISSUE_INVOICE, error.state),
    })
}

/// `billing.invoice.CancelInvoice`, through the generated port.
fn cancel_invoice(
    live: &mut Live,
    request: &SemanticCommandRequest,
) -> Result<SemanticCommandResult, TargetError> {
    let invoice_id = invoice_id_input(request)?;
    if subject_is_unknown(live, &invoice_id) {
        return Ok(SemanticCommandResult::undeclared());
    }
    let outcome = live
        .assembled
        .system
        .invoice_service
        .cancel_invoice(CancelInvoice {
            invoice_id: invoice_id.clone(),
        })
        .map_err(|refusal| unmet(&refusal))?;
    Ok(match outcome {
        CancelInvoiceOutcome::Cancelled { invoice_cancelled } => {
            let published = ObservedEvent::new(event_ref(INVOICE_CANCELLED))
                .with(
                    "invoice_id",
                    Node::Text(invoice_cancelled.invoice_id.0 .0.clone()),
                )
                .in_activity(request.correlation.clone());
            SemanticCommandResult::took(outcome_ref(CANCEL_INVOICE, "cancelled"))
                .with_consistency(token(live)?)
                .emitting(published)
        }
        CancelInvoiceOutcome::WrongState { error } => wrong_state(CANCEL_INVOICE, error.state),
    })
}

/// `billing.invoice.PayInvoice`, through the generated port.
///
/// The one command where the boundary check has an order to respect: `rejected` is decided by the
/// guard alone, so the port answers a non-positive amount whatever the subject is — including one
/// that does not exist — and only a payment the guard admits asks whether the subject does.
fn pay_invoice(
    live: &mut Live,
    request: &SemanticCommandRequest,
) -> Result<SemanticCommandResult, TargetError> {
    let invoice_id = invoice_id_input(request)?;
    let amount = money_input(request, "amount")?;
    if positive(&amount) && subject_is_unknown(live, &invoice_id) {
        return Ok(SemanticCommandResult::undeclared());
    }
    let outcome = live
        .assembled
        .system
        .invoice_service
        .pay_invoice(PayInvoice {
            invoice_id: invoice_id.clone(),
            amount,
        })
        .map_err(|refusal| unmet(&refusal))?;
    Ok(match outcome {
        PayInvoiceOutcome::Settled { invoice_paid } => {
            let published = ObservedEvent::new(event_ref(INVOICE_PAID))
                .with(
                    "invoice_id",
                    Node::Text(invoice_paid.invoice_id.0 .0.clone()),
                )
                .with("amount", money_node(&invoice_paid.amount))
                .in_activity(request.correlation.clone());
            SemanticCommandResult::took(outcome_ref(PAY_INVOICE, "settled"))
                .with_consistency(token(live)?)
                .emitting(published)
        }
        PayInvoiceOutcome::Rejected { error } => {
            SemanticCommandResult::took(outcome_ref(PAY_INVOICE, "rejected")).with_error(
                DeclaredErrorValue::new(error_ref(INVALID_AMOUNT))
                    .with("submitted", money_node(&error.submitted)),
            )
        }
        PayInvoiceOutcome::WrongState { error } => wrong_state(PAY_INVOICE, error.state),
    })
}

/// `billing.email.SendEmail`, invoked directly rather than by the binding.
fn send_email(
    live: &mut Live,
    request: &SemanticCommandRequest,
) -> Result<SemanticCommandResult, TargetError> {
    let input = billing_types::email::SendEmail {
        recipient: billing_types::email::EmailAddress(text_input(request, "recipient")?),
        template: billing_types::email::TemplateId(text_input(request, "template")?),
    };
    let outcome = live
        .assembled
        .system
        .email_service
        .send_email(input)
        .map_err(|refusal| unmet(&refusal))?;
    Ok(match outcome {
        billing_types::email::SendEmailOutcome::Sent { email_sent } => {
            let published = ObservedEvent::new(event_ref(EMAIL_SENT))
                .with("message_id", Node::Text(email_sent.message_id.0 .0.clone()))
                .with("recipient", Node::Text(email_sent.recipient.0.clone()))
                .in_activity(request.correlation.clone());
            SemanticCommandResult::took(outcome_ref(SEND_EMAIL, "sent"))
                .with_consistency(token(live)?)
                .emitting(published)
        }
        billing_types::email::SendEmailOutcome::Failed { error: _ } => {
            SemanticCommandResult::took(outcome_ref(SEND_EMAIL, "failed"))
                .with_error(DeclaredErrorValue::new(error_ref(UNDELIVERABLE)))
        }
    })
}

/// Whether the system has never seen this invoice — the boundary where §9's "no declared
/// outcome" is answered.
///
/// It has to be answered *here*, before the port, because the generated behaviour seam cannot
/// spell it: the seam's `Ok` is the outcome enum, whose `wrong-state` variant demands the state
/// the invoice is really in, and an invoice that does not exist has none. The store handle the
/// linkage exposes answers existence and nothing else. That the adapter needs this pre-check at
/// all is a recorded W6.3 finding about the generator, argued in full at
/// `billing_realization::invoice`.
fn subject_is_unknown(live: &Live, invoice_id: &InvoiceId) -> bool {
    !live.assembled.invoices.knows(invoice_id)
}

/// The branch each lifecycle command declares for an invoice it will not act on, with the state
/// the invoice is really in.
fn wrong_state(command: &str, state: InvoiceState) -> SemanticCommandResult {
    let declared = match state {
        InvoiceState::Cancelled => "Cancelled",
        InvoiceState::Draft => "Draft",
        InvoiceState::Issued => "Issued",
        InvoiceState::Paid => "Paid",
    };
    SemanticCommandResult::took(outcome_ref(command, "wrong-state")).with_error(
        DeclaredErrorValue::new(error_ref(INVOICE_STATE_CONFLICT))
            .with("state", Node::Text(declared.to_owned())),
    )
}

// ---- representation: nodes in, nodes out -----------------------------------------------------

/// A required text input, as the suite carries it.
fn text_input(request: &SemanticCommandRequest, field: &str) -> Result<String, TargetError> {
    request
        .input
        .get(field)
        .and_then(Node::as_text)
        .map(ToOwned::to_owned)
        .ok_or_else(|| {
            TargetError::unavailable(
                format!("reading the input `{field}`"),
                "the suite carries text there, and this adapter converts nothing else",
            )
        })
}

/// The invoice a command's input names.
fn invoice_id_input(request: &SemanticCommandRequest) -> Result<InvoiceId, TargetError> {
    Ok(InvoiceId(Uuid(text_input(request, "invoice_id")?)))
}

/// A `Money` input: a map holding a numeric `amount` and a text `currency`.
fn money_input(request: &SemanticCommandRequest, field: &str) -> Result<Money, TargetError> {
    let refuse = || {
        TargetError::unavailable(
            format!("reading the input `{field}`"),
            "a Money is a map holding a numeric `amount` and a text `currency`",
        )
    };
    let fields = request
        .input
        .get(field)
        .and_then(Node::as_map)
        .ok_or_else(refuse)?;
    let amount = match fields.get("amount") {
        Some(Node::Number(number)) => decimal_rendering(*number),
        _ => return Err(refuse()),
    };
    let currency = fields
        .get("currency")
        .and_then(Node::as_text)
        .ok_or_else(refuse)?
        .to_owned();
    Ok(Money {
        amount: Decimal(amount),
        currency,
    })
}

/// A number as the `Decimal` wire rendering this adapter writes.
///
/// `format!("{}")` on the value: `1.0` becomes `1`, `10.5` becomes `10.5` — deterministic, and
/// parsed back by [`decimal_node`] on the way out, so a value round-trips within one run.
fn decimal_rendering(number: Number) -> String {
    format!("{}", number.get())
}

/// A `Decimal` back into the numeric node the suite's shapes expect.
///
/// A rendering this adapter did not mint may not parse; that surfaces as a text node, which the
/// payload shape check then reports readably instead of this adapter guessing a number.
fn decimal_node(decimal: &Decimal) -> Node {
    decimal
        .0
        .parse::<f64>()
        .ok()
        .and_then(|value| Number::new(value).ok())
        .map_or_else(|| Node::Text(decimal.0.clone()), Node::Number)
}

/// A `Money` as the map node the suite's shapes expect.
fn money_node(money: &Money) -> Node {
    let mut fields = BTreeMap::new();
    fields.insert("amount".to_owned(), decimal_node(&money.amount));
    fields.insert("currency".to_owned(), Node::Text(money.currency.clone()));
    Node::Map(fields)
}

/// One view row: what both declared projections publish.
fn row_of(invoice_id: &InvoiceId, total: &Money) -> ViewRow {
    let mut row = ViewRow::new();
    row.insert("invoice_id".to_owned(), Node::Text(invoice_id.0 .0.clone()));
    row.insert("total".to_owned(), money_node(total));
    row
}

/// The declared name of one logged occurrence.
fn event_name(event: &SystemEvent) -> &'static str {
    match event {
        SystemEvent::DeliveryEscalated(_) => DELIVERY_ESCALATED,
        SystemEvent::EmailSent(_) => EMAIL_SENT,
        SystemEvent::InvoiceCancelled(_) => INVOICE_CANCELLED,
        SystemEvent::InvoiceCreated(_) => INVOICE_CREATED,
        SystemEvent::InvoiceIssued(_) => INVOICE_ISSUED,
        SystemEvent::InvoicePaid(_) => INVOICE_PAID,
    }
}

/// One logged occurrence, rendered for the runner.
fn observed(
    event: &SystemEvent,
    position: u64,
    request: &EventObservationRequest,
) -> ObservedEvent {
    let rendered = match event {
        SystemEvent::DeliveryEscalated(event) => ObservedEvent::new(event_ref(DELIVERY_ESCALATED))
            .with("recipient", Node::Text(event.recipient.0.clone()))
            .with("template", Node::Text(event.template.0.clone())),
        SystemEvent::EmailSent(event) => ObservedEvent::new(event_ref(EMAIL_SENT))
            .with("message_id", Node::Text(event.message_id.0 .0.clone()))
            .with("recipient", Node::Text(event.recipient.0.clone())),
        SystemEvent::InvoiceCancelled(event) => ObservedEvent::new(event_ref(INVOICE_CANCELLED))
            .with("invoice_id", Node::Text(event.invoice_id.0 .0.clone())),
        SystemEvent::InvoiceCreated(event) => ObservedEvent::new(event_ref(INVOICE_CREATED))
            .with("invoice_id", Node::Text(event.invoice_id.0 .0.clone()))
            .with("customer_email", Node::Text(event.customer_email.0.clone()))
            .with("amount", money_node(&event.amount)),
        SystemEvent::InvoiceIssued(event) => ObservedEvent::new(event_ref(INVOICE_ISSUED))
            .with("invoice_id", Node::Text(event.invoice_id.0 .0.clone())),
        SystemEvent::InvoicePaid(event) => ObservedEvent::new(event_ref(INVOICE_PAID))
            .with("invoice_id", Node::Text(event.invoice_id.0 .0.clone()))
            .with("amount", money_node(&event.amount)),
    };
    rendered
        .in_activity(request.correlation.clone())
        .at(position)
}

/// An unmet obligation surfacing to the runner: the adapter could not carry out the request,
/// and the detail names what is owed.
fn unmet(refusal: &billing_types::obligation::UnmetObligation) -> TargetError {
    TargetError::unavailable("driving the linked system", refusal.to_string())
}

// ---- names, parsed once ----------------------------------------------------------------------

/// Declares one `&str` → reference helper per line, each panicking on a name this file wrote
/// wrong — a defect in this module, not a condition a caller can act on.
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
    command_ref -> CommandRef, "a command";
    event_ref -> EventRef, "an event";
    error_ref -> ErrorRef, "a declared error";
    binding_ref -> BindingRef, "a binding";
}

/// One branch of one command, from the two names this module writes.
fn outcome_ref(command: &str, branch: &str) -> ess_conformance::scenario::OutcomeRef {
    ess_conformance::scenario::OutcomeRef::new(
        command_ref(command),
        branch
            .parse()
            .unwrap_or_else(|error| panic!("`{branch}` is a well-formed outcome name: {error}")),
    )
}

// ---- the suite, unchanged --------------------------------------------------------------------

/// The committed suite, exactly as wave 4 wrote it.
fn committed_suite() -> ConformanceSuite {
    ConformanceSuite::from_json(include_str!("../../../suites/generated/billing/suite.json"))
        .expect("the committed billing suite parses")
}

/// Pins that the suite and the generated workspace derive from the same resolved model.
///
/// Without this, a green run could be a suite for one specification passing a workspace
/// synthesised from another — the verdict would name nothing.
fn assert_same_model(suite: &ConformanceSuite) {
    let plan: serde_json::Value =
        serde_json::from_str(include_str!("../../../generated/rust/billing/plan.json"))
            .expect("the committed plan parses");
    assert_eq!(
        suite.provenance.spec_digest.as_str(),
        plan["provenance"]["source_digest"]
            .as_str()
            .expect("the plan names its source digest"),
        "the committed suite and the committed workspace derive from different models; \
         regenerate whichever is stale"
    );
}

// ---- the acceptance criterion, executed ------------------------------------------------------

#[test]
fn the_committed_suite_unchanged_passes_the_linked_synthesized_system() {
    let suite = committed_suite();
    assert_eq!(
        suite.len(),
        29,
        "the criterion is the whole committed suite; fewer scenarios would prove less than wave \
         6 claims"
    );
    assert_same_model(&suite);

    let report = Runner::for_suite(&suite).run(&suite, &Synthesized::honest());

    let failures: Vec<String> = report
        .failures()
        .map(|result| format!("{} — {}", result.scenario, result.status))
        .collect();
    assert!(
        failures.is_empty(),
        "the generated system linked with the honest realization fails a scenario its own \
         specification obliges:\n{}\n\nfirst diagnostic:\n{}",
        failures.join("\n"),
        report
            .diagnostics()
            .next()
            .map_or_else(|| "none".to_owned(), ToString::to_string)
    );
    assert_eq!(report.scenarios.len(), 29);
    assert_eq!(report.status, ConformanceStatus::Passed);
    assert!(report.is_conformant());
    assert_eq!(
        report.implementation,
        ImplementationIdentity::new("billing-synthesized", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn two_runs_against_the_linked_system_produce_byte_identical_reports() {
    // Invariant 9, held at the seam where it is easiest to lose: the linked system mints ids and
    // tokens from counters, the runner owns the clock, and nothing else varies — so the whole
    // report is reproducible to the byte, which is what makes a red run debuggable.
    let suite = committed_suite();
    let first = Runner::for_suite(&suite).run(&suite, &Synthesized::honest());
    let second = Runner::for_suite(&suite).run(&suite, &Synthesized::honest());
    assert_eq!(
        first.to_canonical_json(),
        second.to_canonical_json(),
        "two runs of one suite against one linkage differ, so something varies outside the \
         runner's owned sources"
    );
}

#[test]
fn the_same_suite_fails_the_corrupted_linkage_exactly_where_the_lie_is() {
    // The falsifiability half. Without it, 29 green scenarios show only that the suite asks
    // nothing an honest linkage cannot answer — nothing about whether it would notice a wrong
    // one. The corrupted linkage differs from the honest one by exactly one obligation, so the
    // suite's verdict about it is attributable to the one lie.
    let suite = committed_suite();
    let report = Runner::for_suite(&suite).run(&suite, &Synthesized::corrupted());

    assert_eq!(
        report.status,
        ConformanceStatus::Failed,
        "a corrupted obligation must fail conformance"
    );
    assert_eq!(
        report
            .scenarios
            .iter()
            .find(|result| result.scenario.to_string() == CAUGHT_BY)
            .map(|result| result.status),
        Some(Status::Failed),
        "`{CAUGHT_BY}` exists to catch `{FAULT}` and must be the scenario that fails"
    );

    // Blast radius: exactly one. No other committed scenario submits a non-positive amount to
    // `CreateInvoice`, so a second failure would mean a scenario is over-reaching — or the
    // corruption is wider than the one clause it claims to be.
    let broken: Vec<String> = report
        .failures()
        .map(|result| result.scenario.to_string())
        .collect();
    assert_eq!(
        broken,
        vec![CAUGHT_BY.to_owned()],
        "the blast radius of `{FAULT}` is the one scenario that exists to catch it"
    );

    // And the diagnostic repairs, rather than reporting that something broke: it names the
    // branch that was expected and the one the corrupted guard took.
    let diagnostic = report
        .diagnostics()
        .next()
        .map(ToString::to_string)
        .expect("a failed scenario carries a diagnostic");
    assert!(
        diagnostic.contains("rejected") && diagnostic.contains("accepted"),
        "the diagnostic names the expected branch and the observed one: {diagnostic}"
    );
}
