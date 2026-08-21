// generated from billing v3
// model digest 13577b3ce695932e980d418d5863bcde07f4c362516d53147870d31eaf2ed861
// contract digest d2b48060b7ee32e8f23b1e28972fea39921a25fdcacd635fdf7bbb538e94f367
// compiler 0.1.0 · generator 0.1.0
// do not edit: regenerate with `protocol ess synthesize --target web`

//! The `billing` system, v3, behind a WebAssembly boundary.
//!
//! Invoicing and the notification that follows it: the smallest system that still exercises every construct the model has — two bounded contexts, a command that can be refused, a command with an outcome its input cannot decide, both consistency levels, a state machine, actors, and a type of every kind.
//!
//! Generated, not written: the specification is the source of truth, and the door to changing
//! anything here is `protocol ess synthesize --target web`. This crate emits no behaviour — every
//! command behaviour, view projection and escalation is an obligation, listed with its contract
//! in the `PLAN.md` beside this tree. With nothing installed the page runs against the generated
//! stubs and every command answers with the typed refusal naming what is owed.
//!
//! # No `forbid(unsafe_code)`, and why
//!
//! A WebAssembly export is a `#[no_mangle]` item, which rustc's `unsafe_code` lint flags — so the
//! lint every other generated crate here forbids cannot be declared in this one. There is no
//! `unsafe` block, no `unsafe fn` and no raw-pointer dereference below; the buffer the page
//! writes into is an ordinary `Vec<u8>` this module allocated. `TARGET.md` states the
//! weakening rather than leaving it to be noticed.

#![deny(missing_docs)]

// The exports below pass addresses in a 32-bit linear memory. Built for anything else, that
// cast would silently narrow, so this crate refuses rather than producing a module nobody
// can run.
#[cfg(not(target_family = "wasm"))]
compile_error!(
    "this crate is a browser realization: build it with `--target wasm32-unknown-unknown`"
);

pub mod catalog;
pub mod json;
pub mod wire;

use std::cell::RefCell;

/// Why a request could not be served.
///
/// Every variant is a *value*: a WebAssembly trap tells a page nothing beyond "it failed", so
/// nothing below panics and nothing below unwraps a caller's input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BridgeError {
    /// The request was not well-formed JSON.
    Malformed(json::ParseError),
    /// The request named no kind this bridge serves.
    UnknownRequest(String),
    /// The request named a command this system does not declare, or this target cannot dispatch.
    UnknownCommand(String),
    /// The input did not match the type the command declares.
    Undecodable(json::DecodeError),
    /// An obligation nothing has satisfied was reached — a fact about the realization, never
    /// about the request.
    Unmet {
        /// The capability kind, as the plan spells it.
        capability: String,
        /// The construct that requires it, in the specification's own spelling.
        source: String,
    },
    /// A redelivery named an occurrence the log does not hold.
    NoSuchOccurrence(usize),
}

impl BridgeError {
    /// Writes the refusal as JSON, naming the kind so a page can react to it rather than
    /// display it.
    pub fn encode(&self, out: &mut String) {
        out.push('{');
        match self {
            Self::Malformed(error) => {
                json::member(out, "kind");
                json::push_text(out, "malformed");
                json::member(out, "detail");
                json::push_text(out, &error.to_string());
            }
            Self::UnknownRequest(kind) => {
                json::member(out, "kind");
                json::push_text(out, "unknown-request");
                json::member(out, "request");
                json::push_text(out, kind);
            }
            Self::UnknownCommand(command) => {
                json::member(out, "kind");
                json::push_text(out, "unknown-command");
                json::member(out, "command");
                json::push_text(out, command);
            }
            Self::Undecodable(error) => {
                json::member(out, "kind");
                json::push_text(out, "undecodable");
                json::member(out, "at");
                json::push_text(out, &error.at);
                json::member(out, "expected");
                json::push_text(out, &error.expected);
                json::member(out, "found");
                json::push_text(out, &error.found);
            }
            Self::Unmet { capability, source } => {
                json::member(out, "kind");
                json::push_text(out, "unmet-obligation");
                json::member(out, "capability");
                json::push_text(out, capability);
                json::member(out, "source");
                json::push_text(out, source);
            }
            Self::NoSuchOccurrence(occurrence) => {
                json::member(out, "kind");
                json::push_text(out, "no-such-occurrence");
                json::member(out, "occurrence");
                json::push_integer(out, *occurrence as i64);
            }
        }
        out.push('}');
    }
}

impl From<json::DecodeError> for BridgeError {
    fn from(error: json::DecodeError) -> Self {
        Self::Undecodable(error)
    }
}

impl From<billing_types::obligation::UnmetObligation> for BridgeError {
    fn from(unmet: billing_types::obligation::UnmetObligation) -> Self {
        Self::Unmet {
            capability: unmet.capability.to_owned(),
            source: unmet.source.to_owned(),
        }
    }
}

/// The running system, behind a boundary that erases which realization assembled it.
///
/// Implemented once below, generically, over the generated `System` — so a host links its own
/// implementations of every obligation, hands the assembled system to [`install`], and this
/// bridge never chooses one. Zero implementations for an obligation is an unsatisfied
/// obligation and two is an ambiguity; neither is a decision for the machinery (gap register
/// D-2), so there is no registry and no default beyond the generated stubs that refuse.
pub trait Bound {
    /// Runs one declared command from its JSON input, then pumps the transport until quiescent.
    ///
    /// # Errors
    ///
    /// [`BridgeError`] for a command this system does not accept, an input that does not match
    /// the declared type, or an obligation nothing has satisfied. A *declared* refusal is not an
    /// error: it comes back as the outcome it is.
    fn run(&mut self, command: &str, input: &json::Value) -> Result<String, BridgeError>;

    /// Delivers one already-published occurrence again — the duplicate `at_least_once` permits.
    ///
    /// # Errors
    ///
    /// [`BridgeError::NoSuchOccurrence`] for an index the log does not hold, and
    /// [`BridgeError::Unmet`] for an obligation redelivery reached.
    fn replay(&mut self, occurrence: usize) -> Result<(), BridgeError>;

    /// Everything published so far, in publication order — the system's observable record.
    fn log(&self) -> String;

    /// Every command a binding invoked, with the input it passed.
    fn invoked(&self) -> String;

    /// Every declared view's rows, or the refusal serving it answered with.
    fn projected(&self) -> String;
}

impl<EmailServiceBehaviors, InvoiceServiceBehaviors, Obligations> Bound for billing_system::System<EmailServiceBehaviors, InvoiceServiceBehaviors, Obligations>
where
    EmailServiceBehaviors: billing_types::email::obligations::SendEmailBehavior,
    InvoiceServiceBehaviors: billing_types::invoice::obligations::CancelInvoiceBehavior + billing_types::invoice::obligations::CreateInvoiceBehavior + billing_types::invoice::obligations::IssueInvoiceBehavior + billing_types::invoice::obligations::PayInvoiceBehavior + billing_types::invoice::obligations::InvoiceByIdQuery + billing_types::invoice::obligations::OutstandingInvoicesQuery,
    Obligations: billing_system::obligations::NotifyOnInvoiceCreatedEscalation,
{
    fn run(&mut self, command: &str, input: &json::Value) -> Result<String, BridgeError> {
        let mut out = String::new();
        match command {
            "billing.email.SendEmail" => {
                let input = wire::decode_command_billing_email_send_email(input, "input")?;
                let outcome = self.email_service.send_email(input)?;
                wire::encode_outcome_billing_email_send_email(&outcome, &mut out);
            }
            "billing.invoice.CancelInvoice" => {
                let input = wire::decode_command_billing_invoice_cancel_invoice(input, "input")?;
                let outcome = self.invoice_service.cancel_invoice(input)?;
                wire::encode_outcome_billing_invoice_cancel_invoice(&outcome, &mut out);
            }
            "billing.invoice.CreateInvoice" => {
                let input = wire::decode_command_billing_invoice_create_invoice(input, "input")?;
                let outcome = self.invoice_service.create_invoice(input)?;
                wire::encode_outcome_billing_invoice_create_invoice(&outcome, &mut out);
            }
            "billing.invoice.IssueInvoice" => {
                let input = wire::decode_command_billing_invoice_issue_invoice(input, "input")?;
                let outcome = self.invoice_service.issue_invoice(input)?;
                wire::encode_outcome_billing_invoice_issue_invoice(&outcome, &mut out);
            }
            "billing.invoice.PayInvoice" => {
                let input = wire::decode_command_billing_invoice_pay_invoice(input, "input")?;
                let outcome = self.invoice_service.pay_invoice(input)?;
                wire::encode_outcome_billing_invoice_pay_invoice(&outcome, &mut out);
            }
            other => return Err(BridgeError::UnknownCommand(other.to_owned())),
        }
        self.pump()?;
        Ok(out)
    }

    fn replay(&mut self, occurrence: usize) -> Result<(), BridgeError> {
        let Some(event) = billing_system::System::published(self).get(occurrence).cloned() else {
            return Err(BridgeError::NoSuchOccurrence(occurrence));
        };
        self.redeliver(&event)?;
        Ok(())
    }

    fn log(&self) -> String {
        let mut out = String::new();
        out.push('[');
        for (occurrence, event) in billing_system::System::published(self).iter().enumerate() {
            if occurrence > 0 {
                out.push(',');
            }
            out.push('{');
            json::member(&mut out, "occurrence");
            json::push_integer(&mut out, occurrence as i64);
            json::member(&mut out, "event");
            match event {
                billing_system::SystemEvent::DeliveryEscalated(payload) => {
                    json::push_text(&mut out, "billing.email.DeliveryEscalated");
                    json::member(&mut out, "payload");
                    wire::encode_event_billing_email_delivery_escalated(payload, &mut out);
                }
                billing_system::SystemEvent::EmailSent(payload) => {
                    json::push_text(&mut out, "billing.email.EmailSent");
                    json::member(&mut out, "payload");
                    wire::encode_event_billing_email_email_sent(payload, &mut out);
                }
                billing_system::SystemEvent::InvoiceCancelled(payload) => {
                    json::push_text(&mut out, "billing.invoice.InvoiceCancelled");
                    json::member(&mut out, "payload");
                    wire::encode_event_billing_invoice_invoice_cancelled(payload, &mut out);
                }
                billing_system::SystemEvent::InvoiceCreated(payload) => {
                    json::push_text(&mut out, "billing.invoice.InvoiceCreated");
                    json::member(&mut out, "payload");
                    wire::encode_event_billing_invoice_invoice_created(payload, &mut out);
                }
                billing_system::SystemEvent::InvoiceIssued(payload) => {
                    json::push_text(&mut out, "billing.invoice.InvoiceIssued");
                    json::member(&mut out, "payload");
                    wire::encode_event_billing_invoice_invoice_issued(payload, &mut out);
                }
                billing_system::SystemEvent::InvoicePaid(payload) => {
                    json::push_text(&mut out, "billing.invoice.InvoicePaid");
                    json::member(&mut out, "payload");
                    wire::encode_event_billing_invoice_invoice_paid(payload, &mut out);
                }
            }
            out.push('}');
        }
        out.push(']');
        out
    }

    fn invoked(&self) -> String {
        let mut out = String::new();
        out.push('[');
        for (position, invocation) in billing_system::System::invocations(self).iter().enumerate() {
            if position > 0 {
                out.push(',');
            }
            out.push('{');
            match invocation {
                billing_system::BindingInvocation::NotifyOnInvoiceCreated(input) => {
                    json::member(&mut out, "binding");
                    json::push_text(&mut out, "notify-on-invoice-created");
                    json::member(&mut out, "event");
                    json::push_text(&mut out, "billing.invoice.InvoiceCreated");
                    json::member(&mut out, "command");
                    json::push_text(&mut out, "billing.email.SendEmail");
                    json::member(&mut out, "input");
                    wire::encode_command_billing_email_send_email(input, &mut out);
                }
            }
            out.push('}');
        }
        out.push(']');
        out
    }

    fn projected(&self) -> String {
        let mut out = String::new();
        out.push('{');
        json::member(&mut out, "billing.invoice.InvoiceById");
        match self.invoice_service.invoice_by_id() {
            Ok(rows) => {
                out.push('{');
                json::member(&mut out, "rows");
                out.push('[');
                for (position, row) in rows.iter().enumerate() {
                    if position > 0 {
                        out.push(',');
                    }
                    wire::encode_view_billing_invoice_invoice_by_id(row, &mut out);
                }
                out.push(']');
                out.push('}');
            }
            Err(unmet) => {
                out.push('{');
                json::member(&mut out, "unmet");
                out.push('{');
                json::member(&mut out, "capability");
                json::push_text(&mut out, unmet.capability);
                json::member(&mut out, "source");
                json::push_text(&mut out, unmet.source);
                out.push('}');
                out.push('}');
            }
        }
        json::member(&mut out, "billing.invoice.OutstandingInvoices");
        match self.invoice_service.outstanding_invoices() {
            Ok(rows) => {
                out.push('{');
                json::member(&mut out, "rows");
                out.push('[');
                for (position, row) in rows.iter().enumerate() {
                    if position > 0 {
                        out.push(',');
                    }
                    wire::encode_view_billing_invoice_outstanding_invoices(row, &mut out);
                }
                out.push(']');
                out.push('}');
            }
            Err(unmet) => {
                out.push('{');
                json::member(&mut out, "unmet");
                out.push('{');
                json::member(&mut out, "capability");
                json::push_text(&mut out, unmet.capability);
                json::member(&mut out, "source");
                json::push_text(&mut out, unmet.source);
                out.push('}');
                out.push('}');
            }
        }
        out.push('}');
        out
    }
}

/// Every obligation of this system, refused in the type system.
///
/// Not a second copy of the generated stubs: each method below delegates to the one the
/// Rust target emitted, so the refusal a page shows is the plan entry that target names. It
/// exists because a component may accept commands from more than one bounded context, and
/// no single generated `Unimplemented` covers two.
pub struct Unrealized;

impl billing_types::email::obligations::SendEmailBehavior for Unrealized {
    fn send_email(&mut self, input: billing_types::email::SendEmail) -> Result<billing_types::email::SendEmailOutcome, billing_types::obligation::UnmetObligation> {
        billing_types::email::obligations::Unimplemented.send_email(input)
    }
}

impl billing_types::invoice::obligations::CancelInvoiceBehavior for Unrealized {
    fn cancel_invoice(&mut self, input: billing_types::invoice::CancelInvoice) -> Result<billing_types::invoice::CancelInvoiceOutcome, billing_types::obligation::UnmetObligation> {
        billing_types::invoice::obligations::Unimplemented.cancel_invoice(input)
    }
}

impl billing_types::invoice::obligations::CreateInvoiceBehavior for Unrealized {
    fn create_invoice(&mut self, input: billing_types::invoice::CreateInvoice) -> Result<billing_types::invoice::CreateInvoiceOutcome, billing_types::obligation::UnmetObligation> {
        billing_types::invoice::obligations::Unimplemented.create_invoice(input)
    }
}

impl billing_types::invoice::obligations::IssueInvoiceBehavior for Unrealized {
    fn issue_invoice(&mut self, input: billing_types::invoice::IssueInvoice) -> Result<billing_types::invoice::IssueInvoiceOutcome, billing_types::obligation::UnmetObligation> {
        billing_types::invoice::obligations::Unimplemented.issue_invoice(input)
    }
}

impl billing_types::invoice::obligations::PayInvoiceBehavior for Unrealized {
    fn pay_invoice(&mut self, input: billing_types::invoice::PayInvoice) -> Result<billing_types::invoice::PayInvoiceOutcome, billing_types::obligation::UnmetObligation> {
        billing_types::invoice::obligations::Unimplemented.pay_invoice(input)
    }
}

impl billing_types::invoice::obligations::InvoiceByIdQuery for Unrealized {
    fn invoice_by_id(&self) -> Result<Vec<billing_types::invoice::InvoiceById>, billing_types::obligation::UnmetObligation> {
        billing_types::invoice::obligations::Unimplemented.invoice_by_id()
    }
}

impl billing_types::invoice::obligations::OutstandingInvoicesQuery for Unrealized {
    fn outstanding_invoices(&self) -> Result<Vec<billing_types::invoice::OutstandingInvoices>, billing_types::obligation::UnmetObligation> {
        billing_types::invoice::obligations::Unimplemented.outstanding_invoices()
    }
}

thread_local! {
    /// The one system this module drives, or nothing until something installs one.
    static SYSTEM: RefCell<Option<Box<dyn Bound>>> = const { RefCell::new(None) };
    /// The buffer the page writes a request into.
    static INPUT: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
    /// The response the last dispatch produced, held so its address stays valid.
    static OUTPUT: RefCell<String> = const { RefCell::new(String::new()) };
}

/// Installs the assembled system this module drives.
///
/// Called by a host that has linked one implementation per obligation — never by this crate,
/// which has none to offer. Installing twice replaces: the last system installed is the one
/// serving, and there is no merge, because merging two realizations would be choosing between
/// them.
pub fn install(system: Box<dyn Bound>) {
    SYSTEM.with(|held| {
        *held.borrow_mut() = Some(system);
    });
}

/// The system nobody has realized: generated ports over stubs that refuse.
///
/// The honest empty state. Every command answers with the typed refusal naming what is owed,
/// which is exactly what `PLAN.md` says is owed.
fn unrealized() -> Box<dyn Bound> {
    Box::new(billing_system::System::new(email_service::EmailService::new(Unrealized), invoice_service::InvoiceService::new(Unrealized), billing_system::obligations::Unimplemented))
}

/// Runs one action against the installed system, installing the unrealized one if none is.
fn with_system<T>(action: impl FnOnce(&mut dyn Bound) -> T) -> T {
    SYSTEM.with(|held| {
        let mut held = held.borrow_mut();
        let system = held.get_or_insert_with(unrealized);
        action(system.as_mut())
    })
}

/// Serves one request, answering JSON whatever happens.
///
/// Four requests, and the protocol is the same for every specification:
///
/// | request | what comes back |
/// | --- | --- |
/// | `{"request":"catalog"}` | the model this page renders itself from |
/// | `{"request":"observe"}` | the log, the binding invocations, and every view's rows |
/// | `{"request":"command","command":…,"input":{…}}` | the outcome, then the same observation |
/// | `{"request":"redeliver","occurrence":n}` | the occurrence delivered again, then the observation |
///
/// A refusal comes back as `{"ok":false,"error":{…}}` with a `kind` a page can react to, never as
/// a trap.
pub fn serve(request: &str) -> String {
    let mut out = String::new();
    match answer(request, &mut out) {
        Ok(()) => out,
        Err(error) => {
            let mut refusal = String::new();
            refusal.push('{');
            json::member(&mut refusal, "ok");
            json::push_bool(&mut refusal, false);
            json::member(&mut refusal, "error");
            error.encode(&mut refusal);
            refusal.push('}');
            refusal
        }
    }
}

/// One request, served into `out`.
fn answer(request: &str, out: &mut String) -> Result<(), BridgeError> {
    let request = json::parse(request).map_err(BridgeError::Malformed)?;
    let kind = json::text_at(
        json::member_at(&request, "", "request")?,
        "request",
        "a request kind",
    )?
    .to_owned();
    out.push('{');
    json::member(out, "ok");
    json::push_bool(out, true);
    match kind.as_str() {
        "catalog" => {
            json::member(out, "catalog");
            out.push_str(catalog::CATALOG);
        }
        "observe" => observe(out),
        "command" => {
            let command = json::text_at(
                json::member_at(&request, "", "command")?,
                "command",
                "a command name",
            )?
            .to_owned();
            let input = request
                .member("input")
                .cloned()
                .unwrap_or(json::Value::Object(Vec::new()));
            let outcome = with_system(|system| system.run(&command, &input))?;
            json::member(out, "command");
            json::push_text(out, &command);
            json::member(out, "outcome");
            out.push_str(&outcome);
            observe(out);
        }
        "redeliver" => {
            let occurrence = json::integer_at(
                json::member_at(&request, "", "occurrence")?,
                "occurrence",
                "an occurrence index",
            )?;
            let occurrence = usize::try_from(occurrence)
                .map_err(|_| BridgeError::NoSuchOccurrence(usize::MAX))?;
            with_system(|system| system.replay(occurrence))?;
            observe(out);
        }
        other => return Err(BridgeError::UnknownRequest(other.to_owned())),
    }
    out.push('}');
    Ok(())
}

/// The whole observable surface, written into an answer already in progress.
fn observe(out: &mut String) {
    let (log, invoked, projected) =
        with_system(|system| (system.log(), system.invoked(), system.projected()));
    json::member(out, "log");
    out.push_str(&log);
    json::member(out, "invocations");
    out.push_str(&invoked);
    json::member(out, "views");
    out.push_str(&projected);
}

// ---- the boundary --------------------------------------------------------------------------------
//
// Three exports and no code generation on either side. A caller reserves a buffer of the request's
// byte length, writes UTF-8 into the module's memory at the address it gets back, calls
// `ess_dispatch`, and reads `ess_output_len` bytes from the address that returns. The buffers are
// ordinary `Vec<u8>` and `String` this module owns; nothing here dereferences a raw pointer.

/// Reserves a buffer of `length` bytes for the next request and answers its address.
///
/// The buffer is zeroed and owned by this module until the next reservation, so a caller may write
/// into it and then call [`ess_dispatch`]. Reserving again discards whatever was there.
#[no_mangle]
pub extern "C" fn ess_input_reserve(length: u32) -> u32 {
    INPUT.with(|held| {
        let mut held = held.borrow_mut();
        *held = vec![0; length as usize];
        held.as_ptr() as usize as u32
    })
}

/// Serves the request in the reserved buffer and answers the address of the JSON response.
///
/// Its length is [`ess_output_len`]. The response stays valid until the next dispatch.
#[no_mangle]
pub extern "C" fn ess_dispatch() -> u32 {
    let request = INPUT.with(|held| String::from_utf8_lossy(&held.borrow()).into_owned());
    let response = serve(&request);
    OUTPUT.with(|held| {
        let mut held = held.borrow_mut();
        *held = response;
        held.as_ptr() as usize as u32
    })
}

/// The length in bytes of the response [`ess_dispatch`] last produced.
#[no_mangle]
pub extern "C" fn ess_output_len() -> u32 {
    OUTPUT.with(|held| held.borrow().len() as u32)
}
