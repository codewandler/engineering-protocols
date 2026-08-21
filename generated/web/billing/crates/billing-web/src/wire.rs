// generated from billing v3
// model digest 13577b3ce695932e980d418d5863bcde07f4c362516d53147870d31eaf2ed861
// contract digest d2b48060b7ee32e8f23b1e28972fea39921a25fdcacd635fdf7bbb538e94f367
// compiler 0.1.0 · generator 0.1.0
// do not edit: regenerate with `protocol ess synthesize --target web`

//! Every generated declaration, as JSON, in the renderings the published wire contracts fix.
//!
//! Generated from the model beside the types it crosses, so a field renamed in the specification
//! is renamed here in the same regeneration. An absent optional field is omitted rather
//! than sent as `null`, which is what the `required` list of the published schema says.

use crate::json;

/// Writes `billing.email.EmailAddress` as JSON.
pub fn encode_billing_email_email_address(value: &billing_types::email::EmailAddress, out: &mut String) {
    json::push_text(out, &value.0);
}

/// Reads `billing.email.EmailAddress` from JSON.
///
/// # Errors
///
/// [`json::DecodeError`] naming the path and what the declaration says belongs there.
pub fn decode_billing_email_email_address(value: &json::Value, at: &str) -> Result<billing_types::email::EmailAddress, json::DecodeError> {
    Ok(billing_types::email::EmailAddress(json::text_at(value, at, "a string")?.to_owned()))
}

/// Writes `billing.email.MessageId` as JSON.
pub fn encode_billing_email_message_id(value: &billing_types::email::MessageId, out: &mut String) {
    json::push_text(out, &value.0.0);
}

/// Reads `billing.email.MessageId` from JSON.
///
/// # Errors
///
/// [`json::DecodeError`] naming the path and what the declaration says belongs there.
pub fn decode_billing_email_message_id(value: &json::Value, at: &str) -> Result<billing_types::email::MessageId, json::DecodeError> {
    Ok(billing_types::email::MessageId(billing_types::primitives::Uuid(json::text_at(value, at, "a UUID")?.to_owned())))
}

/// Writes `billing.email.TemplateId` as JSON.
pub fn encode_billing_email_template_id(value: &billing_types::email::TemplateId, out: &mut String) {
    json::push_text(out, &value.0);
}

/// Reads `billing.email.TemplateId` from JSON.
///
/// # Errors
///
/// [`json::DecodeError`] naming the path and what the declaration says belongs there.
pub fn decode_billing_email_template_id(value: &json::Value, at: &str) -> Result<billing_types::email::TemplateId, json::DecodeError> {
    Ok(billing_types::email::TemplateId(json::text_at(value, at, "a string")?.to_owned()))
}

/// Writes `billing.invoice.Channel` as JSON.
pub fn encode_billing_invoice_channel(value: &billing_types::invoice::Channel, out: &mut String) {
    match value {
        billing_types::invoice::Channel::Email => json::push_text(out, "Email"),
        billing_types::invoice::Channel::Post => json::push_text(out, "Post"),
        billing_types::invoice::Channel::Portal => json::push_text(out, "Portal"),
    }
}

/// Reads `billing.invoice.Channel` from JSON.
///
/// # Errors
///
/// [`json::DecodeError`] naming the path and what the declaration says belongs there.
pub fn decode_billing_invoice_channel(value: &json::Value, at: &str) -> Result<billing_types::invoice::Channel, json::DecodeError> {
    Ok(match json::text_at(value, at, "one of `Email`, `Post`, `Portal`")? {
        "Email" => billing_types::invoice::Channel::Email,
        "Post" => billing_types::invoice::Channel::Post,
        "Portal" => billing_types::invoice::Channel::Portal,
        other => return Err(json::DecodeError { at: at.to_owned(), expected: "one of `Email`, `Post`, `Portal`".to_owned(), found: format!("`{other}`") }),
    })
}

/// Writes `billing.invoice.CompanyRef` as JSON.
pub fn encode_billing_invoice_company_ref(value: &billing_types::invoice::CompanyRef, out: &mut String) {
    json::push_text(out, &value.0);
}

/// Reads `billing.invoice.CompanyRef` from JSON.
///
/// # Errors
///
/// [`json::DecodeError`] naming the path and what the declaration says belongs there.
pub fn decode_billing_invoice_company_ref(value: &json::Value, at: &str) -> Result<billing_types::invoice::CompanyRef, json::DecodeError> {
    Ok(billing_types::invoice::CompanyRef(json::text_at(value, at, "a string")?.to_owned()))
}

/// Writes `billing.invoice.Email` as JSON.
pub fn encode_billing_invoice_email(value: &billing_types::invoice::Email, out: &mut String) {
    json::push_text(out, &value.0);
}

/// Reads `billing.invoice.Email` from JSON.
///
/// # Errors
///
/// [`json::DecodeError`] naming the path and what the declaration says belongs there.
pub fn decode_billing_invoice_email(value: &json::Value, at: &str) -> Result<billing_types::invoice::Email, json::DecodeError> {
    Ok(billing_types::invoice::Email(json::text_at(value, at, "a string")?.to_owned()))
}

/// Writes `billing.invoice.Invoice.State` as JSON.
pub fn encode_billing_invoice_invoice_state(value: &billing_types::invoice::InvoiceState, out: &mut String) {
    match value {
        billing_types::invoice::InvoiceState::Cancelled => json::push_text(out, "Cancelled"),
        billing_types::invoice::InvoiceState::Draft => json::push_text(out, "Draft"),
        billing_types::invoice::InvoiceState::Issued => json::push_text(out, "Issued"),
        billing_types::invoice::InvoiceState::Paid => json::push_text(out, "Paid"),
    }
}

/// Reads `billing.invoice.Invoice.State` from JSON.
///
/// # Errors
///
/// [`json::DecodeError`] naming the path and what the declaration says belongs there.
pub fn decode_billing_invoice_invoice_state(value: &json::Value, at: &str) -> Result<billing_types::invoice::InvoiceState, json::DecodeError> {
    Ok(match json::text_at(value, at, "one of `Cancelled`, `Draft`, `Issued`, `Paid`")? {
        "Cancelled" => billing_types::invoice::InvoiceState::Cancelled,
        "Draft" => billing_types::invoice::InvoiceState::Draft,
        "Issued" => billing_types::invoice::InvoiceState::Issued,
        "Paid" => billing_types::invoice::InvoiceState::Paid,
        other => return Err(json::DecodeError { at: at.to_owned(), expected: "one of `Cancelled`, `Draft`, `Issued`, `Paid`".to_owned(), found: format!("`{other}`") }),
    })
}

/// Writes `billing.invoice.InvoiceId` as JSON.
pub fn encode_billing_invoice_invoice_id(value: &billing_types::invoice::InvoiceId, out: &mut String) {
    json::push_text(out, &value.0.0);
}

/// Reads `billing.invoice.InvoiceId` from JSON.
///
/// # Errors
///
/// [`json::DecodeError`] naming the path and what the declaration says belongs there.
pub fn decode_billing_invoice_invoice_id(value: &json::Value, at: &str) -> Result<billing_types::invoice::InvoiceId, json::DecodeError> {
    Ok(billing_types::invoice::InvoiceId(billing_types::primitives::Uuid(json::text_at(value, at, "a UUID")?.to_owned())))
}

/// Writes `billing.invoice.LineItem` as JSON.
pub fn encode_billing_invoice_line_item(value: &billing_types::invoice::LineItem, out: &mut String) {
    out.push('{');
    json::member(out, "description");
    json::push_text(out, &value.description);
    json::member(out, "quantity");
    json::push_integer(out, value.quantity);
    json::member(out, "unit_price");
    encode_billing_invoice_money(&value.unit_price, out);
    out.push('}');
}

/// Reads `billing.invoice.LineItem` from JSON.
///
/// # Errors
///
/// [`json::DecodeError`] naming the path and what the declaration says belongs there.
pub fn decode_billing_invoice_line_item(value: &json::Value, at: &str) -> Result<billing_types::invoice::LineItem, json::DecodeError> {
    Ok(billing_types::invoice::LineItem {
        description: {
            let at0 = json::nested(at, "description");
            let member0 = json::member_at(value, at, "description")?;
            json::text_at(member0, &at0, "a string")?.to_owned()
        },
        quantity: {
            let at1 = json::nested(at, "quantity");
            let member1 = json::member_at(value, at, "quantity")?;
            json::integer_at(member1, &at1, "an integer")?
        },
        unit_price: {
            let at2 = json::nested(at, "unit_price");
            let member2 = json::member_at(value, at, "unit_price")?;
            decode_billing_invoice_money(member2, &at2)?
        },
    })
}

/// Writes `billing.invoice.Money` as JSON.
pub fn encode_billing_invoice_money(value: &billing_types::invoice::Money, out: &mut String) {
    out.push('{');
    json::member(out, "amount");
    json::push_text(out, &value.amount.0);
    json::member(out, "currency");
    json::push_text(out, &value.currency);
    out.push('}');
}

/// Reads `billing.invoice.Money` from JSON.
///
/// # Errors
///
/// [`json::DecodeError`] naming the path and what the declaration says belongs there.
pub fn decode_billing_invoice_money(value: &json::Value, at: &str) -> Result<billing_types::invoice::Money, json::DecodeError> {
    Ok(billing_types::invoice::Money {
        amount: {
            let at0 = json::nested(at, "amount");
            let member0 = json::member_at(value, at, "amount")?;
            billing_types::primitives::Decimal(json::text_at(member0, &at0, "a decimal string")?.to_owned())
        },
        currency: {
            let at1 = json::nested(at, "currency");
            let member1 = json::member_at(value, at, "currency")?;
            json::text_at(member1, &at1, "a string")?.to_owned()
        },
    })
}

/// Writes `billing.invoice.Payee` as JSON.
pub fn encode_billing_invoice_payee(value: &billing_types::invoice::Payee, out: &mut String) {
    match value {
        billing_types::invoice::Payee::Company(held) => {
            out.push('{');
            json::member(out, "kind");
            json::push_text(out, "company");
            json::member(out, "value");
            encode_billing_invoice_company_ref(&*held, out);
            out.push('}');
        }
        billing_types::invoice::Payee::Person(held) => {
            out.push('{');
            json::member(out, "kind");
            json::push_text(out, "person");
            json::member(out, "value");
            encode_billing_invoice_email(&*held, out);
            out.push('}');
        }
    }
}

/// Reads `billing.invoice.Payee` from JSON.
///
/// # Errors
///
/// [`json::DecodeError`] naming the path and what the declaration says belongs there.
pub fn decode_billing_invoice_payee(value: &json::Value, at: &str) -> Result<billing_types::invoice::Payee, json::DecodeError> {
    let tag = json::member_at(value, at, "kind")?;
    let at_tag = json::nested(at, "kind");
    Ok(match json::text_at(tag, &at_tag, "one of `company`, `person`")? {
        "company" => billing_types::invoice::Payee::Company({
            let at0 = json::nested(at, "value");
            let member0 = json::member_at(value, at, "value")?;
            decode_billing_invoice_company_ref(member0, &at0)?
        }),
        "person" => billing_types::invoice::Payee::Person({
            let at1 = json::nested(at, "value");
            let member1 = json::member_at(value, at, "value")?;
            decode_billing_invoice_email(member1, &at1)?
        }),
        other => return Err(json::DecodeError { at: at_tag.clone(), expected: "one of `company`, `person`".to_owned(), found: format!("`{other}`") }),
    })
}

/// Writes the event `billing.email.DeliveryEscalated` as JSON.
pub fn encode_event_billing_email_delivery_escalated(value: &billing_types::email::DeliveryEscalated, out: &mut String) {
    out.push('{');
    json::member(out, "recipient");
    encode_billing_email_email_address(&value.recipient, out);
    json::member(out, "template");
    encode_billing_email_template_id(&value.template, out);
    out.push('}');
}

/// Writes the event `billing.email.EmailSent` as JSON.
pub fn encode_event_billing_email_email_sent(value: &billing_types::email::EmailSent, out: &mut String) {
    out.push('{');
    json::member(out, "message_id");
    encode_billing_email_message_id(&value.message_id, out);
    json::member(out, "recipient");
    encode_billing_email_email_address(&value.recipient, out);
    out.push('}');
}

/// Writes the event `billing.invoice.InvoiceCancelled` as JSON.
pub fn encode_event_billing_invoice_invoice_cancelled(value: &billing_types::invoice::InvoiceCancelled, out: &mut String) {
    out.push('{');
    json::member(out, "invoice_id");
    encode_billing_invoice_invoice_id(&value.invoice_id, out);
    out.push('}');
}

/// Writes the event `billing.invoice.InvoiceCreated` as JSON.
pub fn encode_event_billing_invoice_invoice_created(value: &billing_types::invoice::InvoiceCreated, out: &mut String) {
    out.push('{');
    json::member(out, "invoice_id");
    encode_billing_invoice_invoice_id(&value.invoice_id, out);
    json::member(out, "customer_email");
    encode_billing_invoice_email(&value.customer_email, out);
    json::member(out, "amount");
    encode_billing_invoice_money(&value.amount, out);
    out.push('}');
}

/// Writes the event `billing.invoice.InvoiceIssued` as JSON.
pub fn encode_event_billing_invoice_invoice_issued(value: &billing_types::invoice::InvoiceIssued, out: &mut String) {
    out.push('{');
    json::member(out, "invoice_id");
    encode_billing_invoice_invoice_id(&value.invoice_id, out);
    out.push('}');
}

/// Writes the event `billing.invoice.InvoicePaid` as JSON.
pub fn encode_event_billing_invoice_invoice_paid(value: &billing_types::invoice::InvoicePaid, out: &mut String) {
    out.push('{');
    json::member(out, "invoice_id");
    encode_billing_invoice_invoice_id(&value.invoice_id, out);
    json::member(out, "amount");
    encode_billing_invoice_money(&value.amount, out);
    out.push('}');
}

/// Writes the declared error `billing.email.Undeliverable` as JSON.
pub fn encode_error_billing_email_undeliverable(_value: &billing_types::email::Undeliverable, out: &mut String) {
    out.push('{');
    out.push('}');
}

/// Writes the declared error `billing.invoice.InvalidAmount` as JSON.
pub fn encode_error_billing_invoice_invalid_amount(value: &billing_types::invoice::InvalidAmount, out: &mut String) {
    out.push('{');
    json::member(out, "submitted");
    encode_billing_invoice_money(&value.submitted, out);
    out.push('}');
}

/// Writes the declared error `billing.invoice.InvoiceStateConflict` as JSON.
pub fn encode_error_billing_invoice_invoice_state_conflict(value: &billing_types::invoice::InvoiceStateConflict, out: &mut String) {
    out.push('{');
    json::member(out, "state");
    encode_billing_invoice_invoice_state(&value.state, out);
    out.push('}');
}

/// Writes one row of the view `billing.invoice.InvoiceById` as JSON.
pub fn encode_view_billing_invoice_invoice_by_id(value: &billing_types::invoice::InvoiceById, out: &mut String) {
    out.push('{');
    json::member(out, "invoice_id");
    encode_billing_invoice_invoice_id(&value.invoice_id, out);
    json::member(out, "total");
    encode_billing_invoice_money(&value.total, out);
    out.push('}');
}

/// Writes one row of the view `billing.invoice.OutstandingInvoices` as JSON.
pub fn encode_view_billing_invoice_outstanding_invoices(value: &billing_types::invoice::OutstandingInvoices, out: &mut String) {
    out.push('{');
    json::member(out, "invoice_id");
    encode_billing_invoice_invoice_id(&value.invoice_id, out);
    json::member(out, "total");
    encode_billing_invoice_money(&value.total, out);
    out.push('}');
}

/// Writes the input of `billing.email.SendEmail` as JSON.
pub fn encode_command_billing_email_send_email(value: &billing_types::email::SendEmail, out: &mut String) {
    out.push('{');
    json::member(out, "recipient");
    encode_billing_email_email_address(&value.recipient, out);
    json::member(out, "template");
    encode_billing_email_template_id(&value.template, out);
    out.push('}');
}

/// Reads the input of `billing.email.SendEmail` from JSON.
///
/// # Errors
///
/// [`json::DecodeError`] naming the path and what the declaration says belongs there.
pub fn decode_command_billing_email_send_email(value: &json::Value, at: &str) -> Result<billing_types::email::SendEmail, json::DecodeError> {
    Ok(billing_types::email::SendEmail {
        recipient: {
            let at0 = json::nested(at, "recipient");
            let member0 = json::member_at(value, at, "recipient")?;
            decode_billing_email_email_address(member0, &at0)?
        },
        template: {
            let at1 = json::nested(at, "template");
            let member1 = json::member_at(value, at, "template")?;
            decode_billing_email_template_id(member1, &at1)?
        },
    })
}

/// Writes the outcome of `billing.email.SendEmail` as JSON: the branch taken, what it published, and the declared
/// refusal it carries where it carries one.
pub fn encode_outcome_billing_email_send_email(value: &billing_types::email::SendEmailOutcome, out: &mut String) {
    out.push('{');
    match value {
        billing_types::email::SendEmailOutcome::Sent { email_sent } => {
            json::member(out, "outcome");
            json::push_text(out, "sent");
            json::member(out, "published");
            out.push('[');
            out.push('{');
            json::member(out, "event");
            json::push_text(out, "billing.email.EmailSent");
            json::member(out, "payload");
            encode_event_billing_email_email_sent(email_sent, out);
            out.push('}');
            out.push(']');
        }
        billing_types::email::SendEmailOutcome::Failed { error } => {
            json::member(out, "outcome");
            json::push_text(out, "failed");
            json::member(out, "published");
            out.push('[');
            out.push(']');
            json::member(out, "refusal");
            out.push('{');
            json::member(out, "error");
            json::push_text(out, "billing.email.Undeliverable");
            json::member(out, "payload");
            encode_error_billing_email_undeliverable(error, out);
            out.push('}');
        }
    }
    out.push('}');
}

/// Writes the input of `billing.invoice.CancelInvoice` as JSON.
pub fn encode_command_billing_invoice_cancel_invoice(value: &billing_types::invoice::CancelInvoice, out: &mut String) {
    out.push('{');
    json::member(out, "invoice_id");
    encode_billing_invoice_invoice_id(&value.invoice_id, out);
    out.push('}');
}

/// Reads the input of `billing.invoice.CancelInvoice` from JSON.
///
/// # Errors
///
/// [`json::DecodeError`] naming the path and what the declaration says belongs there.
pub fn decode_command_billing_invoice_cancel_invoice(value: &json::Value, at: &str) -> Result<billing_types::invoice::CancelInvoice, json::DecodeError> {
    Ok(billing_types::invoice::CancelInvoice {
        invoice_id: {
            let at0 = json::nested(at, "invoice_id");
            let member0 = json::member_at(value, at, "invoice_id")?;
            decode_billing_invoice_invoice_id(member0, &at0)?
        },
    })
}

/// Writes the outcome of `billing.invoice.CancelInvoice` as JSON: the branch taken, what it published, and the declared
/// refusal it carries where it carries one.
pub fn encode_outcome_billing_invoice_cancel_invoice(value: &billing_types::invoice::CancelInvoiceOutcome, out: &mut String) {
    out.push('{');
    match value {
        billing_types::invoice::CancelInvoiceOutcome::Cancelled { invoice_cancelled } => {
            json::member(out, "outcome");
            json::push_text(out, "cancelled");
            json::member(out, "published");
            out.push('[');
            out.push('{');
            json::member(out, "event");
            json::push_text(out, "billing.invoice.InvoiceCancelled");
            json::member(out, "payload");
            encode_event_billing_invoice_invoice_cancelled(invoice_cancelled, out);
            out.push('}');
            out.push(']');
        }
        billing_types::invoice::CancelInvoiceOutcome::WrongState { error } => {
            json::member(out, "outcome");
            json::push_text(out, "wrong-state");
            json::member(out, "published");
            out.push('[');
            out.push(']');
            json::member(out, "refusal");
            out.push('{');
            json::member(out, "error");
            json::push_text(out, "billing.invoice.InvoiceStateConflict");
            json::member(out, "payload");
            encode_error_billing_invoice_invoice_state_conflict(error, out);
            out.push('}');
        }
    }
    out.push('}');
}

/// Writes the input of `billing.invoice.CreateInvoice` as JSON.
pub fn encode_command_billing_invoice_create_invoice(value: &billing_types::invoice::CreateInvoice, out: &mut String) {
    out.push('{');
    json::member(out, "customer_email");
    encode_billing_invoice_email(&value.customer_email, out);
    json::member(out, "amount");
    encode_billing_invoice_money(&value.amount, out);
    out.push('}');
}

/// Reads the input of `billing.invoice.CreateInvoice` from JSON.
///
/// # Errors
///
/// [`json::DecodeError`] naming the path and what the declaration says belongs there.
pub fn decode_command_billing_invoice_create_invoice(value: &json::Value, at: &str) -> Result<billing_types::invoice::CreateInvoice, json::DecodeError> {
    Ok(billing_types::invoice::CreateInvoice {
        customer_email: {
            let at0 = json::nested(at, "customer_email");
            let member0 = json::member_at(value, at, "customer_email")?;
            decode_billing_invoice_email(member0, &at0)?
        },
        amount: {
            let at1 = json::nested(at, "amount");
            let member1 = json::member_at(value, at, "amount")?;
            decode_billing_invoice_money(member1, &at1)?
        },
    })
}

/// Writes the outcome of `billing.invoice.CreateInvoice` as JSON: the branch taken, what it published, and the declared
/// refusal it carries where it carries one.
pub fn encode_outcome_billing_invoice_create_invoice(value: &billing_types::invoice::CreateInvoiceOutcome, out: &mut String) {
    out.push('{');
    match value {
        billing_types::invoice::CreateInvoiceOutcome::Accepted { invoice_created } => {
            json::member(out, "outcome");
            json::push_text(out, "accepted");
            json::member(out, "published");
            out.push('[');
            out.push('{');
            json::member(out, "event");
            json::push_text(out, "billing.invoice.InvoiceCreated");
            json::member(out, "payload");
            encode_event_billing_invoice_invoice_created(invoice_created, out);
            out.push('}');
            out.push(']');
        }
        billing_types::invoice::CreateInvoiceOutcome::Rejected { error } => {
            json::member(out, "outcome");
            json::push_text(out, "rejected");
            json::member(out, "published");
            out.push('[');
            out.push(']');
            json::member(out, "refusal");
            out.push('{');
            json::member(out, "error");
            json::push_text(out, "billing.invoice.InvalidAmount");
            json::member(out, "payload");
            encode_error_billing_invoice_invalid_amount(error, out);
            out.push('}');
        }
    }
    out.push('}');
}

/// Writes the input of `billing.invoice.IssueInvoice` as JSON.
pub fn encode_command_billing_invoice_issue_invoice(value: &billing_types::invoice::IssueInvoice, out: &mut String) {
    out.push('{');
    json::member(out, "invoice_id");
    encode_billing_invoice_invoice_id(&value.invoice_id, out);
    out.push('}');
}

/// Reads the input of `billing.invoice.IssueInvoice` from JSON.
///
/// # Errors
///
/// [`json::DecodeError`] naming the path and what the declaration says belongs there.
pub fn decode_command_billing_invoice_issue_invoice(value: &json::Value, at: &str) -> Result<billing_types::invoice::IssueInvoice, json::DecodeError> {
    Ok(billing_types::invoice::IssueInvoice {
        invoice_id: {
            let at0 = json::nested(at, "invoice_id");
            let member0 = json::member_at(value, at, "invoice_id")?;
            decode_billing_invoice_invoice_id(member0, &at0)?
        },
    })
}

/// Writes the outcome of `billing.invoice.IssueInvoice` as JSON: the branch taken, what it published, and the declared
/// refusal it carries where it carries one.
pub fn encode_outcome_billing_invoice_issue_invoice(value: &billing_types::invoice::IssueInvoiceOutcome, out: &mut String) {
    out.push('{');
    match value {
        billing_types::invoice::IssueInvoiceOutcome::Issued { invoice_issued } => {
            json::member(out, "outcome");
            json::push_text(out, "issued");
            json::member(out, "published");
            out.push('[');
            out.push('{');
            json::member(out, "event");
            json::push_text(out, "billing.invoice.InvoiceIssued");
            json::member(out, "payload");
            encode_event_billing_invoice_invoice_issued(invoice_issued, out);
            out.push('}');
            out.push(']');
        }
        billing_types::invoice::IssueInvoiceOutcome::WrongState { error } => {
            json::member(out, "outcome");
            json::push_text(out, "wrong-state");
            json::member(out, "published");
            out.push('[');
            out.push(']');
            json::member(out, "refusal");
            out.push('{');
            json::member(out, "error");
            json::push_text(out, "billing.invoice.InvoiceStateConflict");
            json::member(out, "payload");
            encode_error_billing_invoice_invoice_state_conflict(error, out);
            out.push('}');
        }
    }
    out.push('}');
}

/// Writes the input of `billing.invoice.PayInvoice` as JSON.
pub fn encode_command_billing_invoice_pay_invoice(value: &billing_types::invoice::PayInvoice, out: &mut String) {
    out.push('{');
    json::member(out, "invoice_id");
    encode_billing_invoice_invoice_id(&value.invoice_id, out);
    json::member(out, "amount");
    encode_billing_invoice_money(&value.amount, out);
    out.push('}');
}

/// Reads the input of `billing.invoice.PayInvoice` from JSON.
///
/// # Errors
///
/// [`json::DecodeError`] naming the path and what the declaration says belongs there.
pub fn decode_command_billing_invoice_pay_invoice(value: &json::Value, at: &str) -> Result<billing_types::invoice::PayInvoice, json::DecodeError> {
    Ok(billing_types::invoice::PayInvoice {
        invoice_id: {
            let at0 = json::nested(at, "invoice_id");
            let member0 = json::member_at(value, at, "invoice_id")?;
            decode_billing_invoice_invoice_id(member0, &at0)?
        },
        amount: {
            let at1 = json::nested(at, "amount");
            let member1 = json::member_at(value, at, "amount")?;
            decode_billing_invoice_money(member1, &at1)?
        },
    })
}

/// Writes the outcome of `billing.invoice.PayInvoice` as JSON: the branch taken, what it published, and the declared
/// refusal it carries where it carries one.
pub fn encode_outcome_billing_invoice_pay_invoice(value: &billing_types::invoice::PayInvoiceOutcome, out: &mut String) {
    out.push('{');
    match value {
        billing_types::invoice::PayInvoiceOutcome::Settled { invoice_paid } => {
            json::member(out, "outcome");
            json::push_text(out, "settled");
            json::member(out, "published");
            out.push('[');
            out.push('{');
            json::member(out, "event");
            json::push_text(out, "billing.invoice.InvoicePaid");
            json::member(out, "payload");
            encode_event_billing_invoice_invoice_paid(invoice_paid, out);
            out.push('}');
            out.push(']');
        }
        billing_types::invoice::PayInvoiceOutcome::Rejected { error } => {
            json::member(out, "outcome");
            json::push_text(out, "rejected");
            json::member(out, "published");
            out.push('[');
            out.push(']');
            json::member(out, "refusal");
            out.push('{');
            json::member(out, "error");
            json::push_text(out, "billing.invoice.InvalidAmount");
            json::member(out, "payload");
            encode_error_billing_invoice_invalid_amount(error, out);
            out.push('}');
        }
        billing_types::invoice::PayInvoiceOutcome::WrongState { error } => {
            json::member(out, "outcome");
            json::push_text(out, "wrong-state");
            json::member(out, "published");
            out.push('[');
            out.push(']');
            json::member(out, "refusal");
            out.push('{');
            json::member(out, "error");
            json::push_text(out, "billing.invoice.InvoiceStateConflict");
            json::member(out, "payload");
            encode_error_billing_invoice_invoice_state_conflict(error, out);
            out.push('}');
        }
    }
    out.push('}');
}
