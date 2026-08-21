// generated from gatepass v1
// model digest f2e0f8ff51c077fa1c713d8151544379bafac36a5a927e71c685042d53ab6e61
// contract digest e6e58e055d24f8f494dcff274f55e723d967f9d1f9aea16641bb8dacbb71171e
// compiler 0.1.0 · generator 0.1.0
// do not edit: regenerate with `protocol ess synthesize`
//! Every generated declaration, as JSON, in the renderings the published wire contracts fix.
//!
//! Generated from the model beside the types it crosses, so a field renamed in the specification
//! is renamed here in the same regeneration. An absent optional field is omitted rather
//! than sent as `null`, which is what the `required` list of the published schema says.

use crate::json;

/// Writes `gatepass.visit.Badge` as JSON.
pub fn encode_gatepass_visit_badge(value: &gatepass_types::visit::Badge, out: &mut String) {
    out.push('{');
    json::member(out, "serial");
    json::push_text(out, &value.serial);
    if let Some(held0) = &value.printed_at {
        json::member(out, "printed_at");
        json::push_text(out, &(*held0).0);
    }
    json::member(out, "signature");
    json::push_base64(out, &value.signature);
    out.push('}');
}

/// Reads `gatepass.visit.Badge` from JSON.
///
/// # Errors
///
/// [`json::DecodeError`] naming the path and what the declaration says belongs there.
pub fn decode_gatepass_visit_badge(value: &json::Value, at: &str) -> Result<gatepass_types::visit::Badge, json::DecodeError> {
    Ok(gatepass_types::visit::Badge {
        serial: {
            let at0 = json::nested(at, "serial");
            let member0 = json::member_at(value, at, "serial")?;
            json::text_at(member0, &at0, "a string")?.to_owned()
        },
        printed_at: match value.member("printed_at") {
            None | Some(json::Value::Null) => None,
            Some(member1) => {
                let at1 = json::nested(at, "printed_at");
                Some(gatepass_types::primitives::Timestamp(json::text_at(member1, &at1, "an RFC 3339 instant")?.to_owned()))
            }
        },
        signature: {
            let at2 = json::nested(at, "signature");
            let member2 = json::member_at(value, at, "signature")?;
            json::bytes_at(member2, &at2, "base64-encoded bytes")?
        },
    })
}

/// Writes `gatepass.visit.Building` as JSON.
pub fn encode_gatepass_visit_building(value: &gatepass_types::visit::Building, out: &mut String) {
    match value {
        gatepass_types::visit::Building::North => json::push_text(out, "North"),
        gatepass_types::visit::Building::South => json::push_text(out, "South"),
        gatepass_types::visit::Building::Annex => json::push_text(out, "Annex"),
    }
}

/// Reads `gatepass.visit.Building` from JSON.
///
/// # Errors
///
/// [`json::DecodeError`] naming the path and what the declaration says belongs there.
pub fn decode_gatepass_visit_building(value: &json::Value, at: &str) -> Result<gatepass_types::visit::Building, json::DecodeError> {
    Ok(match json::text_at(value, at, "one of `North`, `South`, `Annex`")? {
        "North" => gatepass_types::visit::Building::North,
        "South" => gatepass_types::visit::Building::South,
        "Annex" => gatepass_types::visit::Building::Annex,
        other => return Err(json::DecodeError { at: at.to_owned(), expected: "one of `North`, `South`, `Annex`".to_owned(), found: format!("`{other}`") }),
    })
}

/// Writes `gatepass.visit.Deposit` as JSON.
pub fn encode_gatepass_visit_deposit(value: &gatepass_types::visit::Deposit, out: &mut String) {
    out.push('{');
    json::member(out, "amount");
    json::push_text(out, &value.amount.0);
    json::member(out, "currency");
    json::push_text(out, &value.currency);
    out.push('}');
}

/// Reads `gatepass.visit.Deposit` from JSON.
///
/// # Errors
///
/// [`json::DecodeError`] naming the path and what the declaration says belongs there.
pub fn decode_gatepass_visit_deposit(value: &json::Value, at: &str) -> Result<gatepass_types::visit::Deposit, json::DecodeError> {
    Ok(gatepass_types::visit::Deposit {
        amount: {
            let at0 = json::nested(at, "amount");
            let member0 = json::member_at(value, at, "amount")?;
            gatepass_types::primitives::Decimal(json::text_at(member0, &at0, "a decimal string")?.to_owned())
        },
        currency: {
            let at1 = json::nested(at, "currency");
            let member1 = json::member_at(value, at, "currency")?;
            json::text_at(member1, &at1, "a string")?.to_owned()
        },
    })
}

/// Writes `gatepass.visit.EmployeeId` as JSON.
pub fn encode_gatepass_visit_employee_id(value: &gatepass_types::visit::EmployeeId, out: &mut String) {
    json::push_text(out, &value.0);
}

/// Reads `gatepass.visit.EmployeeId` from JSON.
///
/// # Errors
///
/// [`json::DecodeError`] naming the path and what the declaration says belongs there.
pub fn decode_gatepass_visit_employee_id(value: &json::Value, at: &str) -> Result<gatepass_types::visit::EmployeeId, json::DecodeError> {
    Ok(gatepass_types::visit::EmployeeId(json::text_at(value, at, "a string")?.to_owned()))
}

/// Writes `gatepass.visit.Host` as JSON.
pub fn encode_gatepass_visit_host(value: &gatepass_types::visit::Host, out: &mut String) {
    match value {
        gatepass_types::visit::Host::Contractor(held) => {
            out.push('{');
            json::member(out, "kind");
            json::push_text(out, "contractor");
            json::member(out, "value");
            encode_gatepass_visit_vendor_ref(&*held, out);
            out.push('}');
        }
        gatepass_types::visit::Host::Employee(held) => {
            out.push('{');
            json::member(out, "kind");
            json::push_text(out, "employee");
            json::member(out, "value");
            encode_gatepass_visit_employee_id(&*held, out);
            out.push('}');
        }
    }
}

/// Reads `gatepass.visit.Host` from JSON.
///
/// # Errors
///
/// [`json::DecodeError`] naming the path and what the declaration says belongs there.
pub fn decode_gatepass_visit_host(value: &json::Value, at: &str) -> Result<gatepass_types::visit::Host, json::DecodeError> {
    let tag = json::member_at(value, at, "kind")?;
    let at_tag = json::nested(at, "kind");
    Ok(match json::text_at(tag, &at_tag, "one of `contractor`, `employee`")? {
        "contractor" => gatepass_types::visit::Host::Contractor({
            let at0 = json::nested(at, "value");
            let member0 = json::member_at(value, at, "value")?;
            decode_gatepass_visit_vendor_ref(member0, &at0)?
        }),
        "employee" => gatepass_types::visit::Host::Employee({
            let at1 = json::nested(at, "value");
            let member1 = json::member_at(value, at, "value")?;
            decode_gatepass_visit_employee_id(member1, &at1)?
        }),
        other => return Err(json::DecodeError { at: at_tag.clone(), expected: "one of `contractor`, `employee`".to_owned(), found: format!("`{other}`") }),
    })
}

/// Writes `gatepass.visit.VendorRef` as JSON.
pub fn encode_gatepass_visit_vendor_ref(value: &gatepass_types::visit::VendorRef, out: &mut String) {
    json::push_text(out, &value.0);
}

/// Reads `gatepass.visit.VendorRef` from JSON.
///
/// # Errors
///
/// [`json::DecodeError`] naming the path and what the declaration says belongs there.
pub fn decode_gatepass_visit_vendor_ref(value: &json::Value, at: &str) -> Result<gatepass_types::visit::VendorRef, json::DecodeError> {
    Ok(gatepass_types::visit::VendorRef(json::text_at(value, at, "a string")?.to_owned()))
}

/// Writes `gatepass.visit.Visit.State` as JSON.
pub fn encode_gatepass_visit_visit_state(value: &gatepass_types::visit::VisitState, out: &mut String) {
    match value {
        gatepass_types::visit::VisitState::Departed => json::push_text(out, "Departed"),
        gatepass_types::visit::VisitState::Expected => json::push_text(out, "Expected"),
        gatepass_types::visit::VisitState::OnSite => json::push_text(out, "OnSite"),
    }
}

/// Reads `gatepass.visit.Visit.State` from JSON.
///
/// # Errors
///
/// [`json::DecodeError`] naming the path and what the declaration says belongs there.
pub fn decode_gatepass_visit_visit_state(value: &json::Value, at: &str) -> Result<gatepass_types::visit::VisitState, json::DecodeError> {
    Ok(match json::text_at(value, at, "one of `Departed`, `Expected`, `OnSite`")? {
        "Departed" => gatepass_types::visit::VisitState::Departed,
        "Expected" => gatepass_types::visit::VisitState::Expected,
        "OnSite" => gatepass_types::visit::VisitState::OnSite,
        other => return Err(json::DecodeError { at: at.to_owned(), expected: "one of `Departed`, `Expected`, `OnSite`".to_owned(), found: format!("`{other}`") }),
    })
}

/// Writes `gatepass.visit.VisitId` as JSON.
pub fn encode_gatepass_visit_visit_id(value: &gatepass_types::visit::VisitId, out: &mut String) {
    json::push_text(out, &value.0.0);
}

/// Reads `gatepass.visit.VisitId` from JSON.
///
/// # Errors
///
/// [`json::DecodeError`] naming the path and what the declaration says belongs there.
pub fn decode_gatepass_visit_visit_id(value: &json::Value, at: &str) -> Result<gatepass_types::visit::VisitId, json::DecodeError> {
    Ok(gatepass_types::visit::VisitId(gatepass_types::primitives::Uuid(json::text_at(value, at, "a UUID")?.to_owned())))
}

/// Writes `gatepass.visit.VisitorName` as JSON.
pub fn encode_gatepass_visit_visitor_name(value: &gatepass_types::visit::VisitorName, out: &mut String) {
    json::push_text(out, &value.0);
}

/// Reads `gatepass.visit.VisitorName` from JSON.
///
/// # Errors
///
/// [`json::DecodeError`] naming the path and what the declaration says belongs there.
pub fn decode_gatepass_visit_visitor_name(value: &json::Value, at: &str) -> Result<gatepass_types::visit::VisitorName, json::DecodeError> {
    Ok(gatepass_types::visit::VisitorName(json::text_at(value, at, "a string")?.to_owned()))
}

/// Writes the event `gatepass.visit.VisitRegistered` as JSON.
pub fn encode_event_gatepass_visit_visit_registered(value: &gatepass_types::visit::VisitRegistered, out: &mut String) {
    out.push('{');
    json::member(out, "visit_id");
    encode_gatepass_visit_visit_id(&value.visit_id, out);
    json::member(out, "visitor");
    encode_gatepass_visit_visitor_name(&value.visitor, out);
    json::member(out, "building");
    encode_gatepass_visit_building(&value.building, out);
    out.push('}');
}

/// Writes the event `gatepass.visit.VisitorAdmitted` as JSON.
pub fn encode_event_gatepass_visit_visitor_admitted(value: &gatepass_types::visit::VisitorAdmitted, out: &mut String) {
    out.push('{');
    json::member(out, "visit_id");
    encode_gatepass_visit_visit_id(&value.visit_id, out);
    json::member(out, "badge");
    encode_gatepass_visit_badge(&value.badge, out);
    out.push('}');
}

/// Writes the event `gatepass.visit.VisitorDeparted` as JSON.
pub fn encode_event_gatepass_visit_visitor_departed(value: &gatepass_types::visit::VisitorDeparted, out: &mut String) {
    out.push('{');
    json::member(out, "visit_id");
    encode_gatepass_visit_visit_id(&value.visit_id, out);
    out.push('}');
}

/// Writes the declared error `gatepass.visit.InvalidVisitLength` as JSON.
pub fn encode_error_gatepass_visit_invalid_visit_length(value: &gatepass_types::visit::InvalidVisitLength, out: &mut String) {
    out.push('{');
    json::member(out, "submitted");
    json::push_integer(out, value.submitted);
    out.push('}');
}

/// Writes the declared error `gatepass.visit.VisitStateConflict` as JSON.
pub fn encode_error_gatepass_visit_visit_state_conflict(value: &gatepass_types::visit::VisitStateConflict, out: &mut String) {
    out.push('{');
    json::member(out, "state");
    encode_gatepass_visit_visit_state(&value.state, out);
    out.push('}');
}

/// Writes one row of the view `gatepass.visit.ExpectedVisits` as JSON.
pub fn encode_view_gatepass_visit_expected_visits(value: &gatepass_types::visit::ExpectedVisits, out: &mut String) {
    out.push('{');
    json::member(out, "visit_id");
    encode_gatepass_visit_visit_id(&value.visit_id, out);
    json::member(out, "visitor");
    encode_gatepass_visit_visitor_name(&value.visitor, out);
    json::member(out, "building");
    encode_gatepass_visit_building(&value.building, out);
    json::member(out, "deposit");
    encode_gatepass_visit_deposit(&value.deposit, out);
    out.push('}');
}

/// Writes one row of the view `gatepass.visit.VisitById` as JSON.
pub fn encode_view_gatepass_visit_visit_by_id(value: &gatepass_types::visit::VisitById, out: &mut String) {
    out.push('{');
    json::member(out, "visit_id");
    encode_gatepass_visit_visit_id(&value.visit_id, out);
    json::member(out, "visitor");
    encode_gatepass_visit_visitor_name(&value.visitor, out);
    json::member(out, "host");
    encode_gatepass_visit_host(&value.host, out);
    json::member(out, "escorts");
    out.push('[');
    for (index0, item0) in value.escorts.iter().enumerate() {
        if index0 > 0 {
            out.push(',');
        }
        encode_gatepass_visit_visitor_name(&*item0, out);
    }
    out.push(']');
    json::member(out, "notes");
    out.push('{');
    for (index0, (key0, item0)) in value.notes.iter().enumerate() {
        if index0 > 0 {
            out.push(',');
        }
        json::push_text(out, key0);
        out.push(':');
        json::push_text(out, &*item0);
    }
    out.push('}');
    if let Some(held0) = &value.badge {
        json::member(out, "badge");
        encode_gatepass_visit_badge(&*held0, out);
    }
    out.push('}');
}

/// Writes the input of `gatepass.visit.AdmitVisitor` as JSON.
pub fn encode_command_gatepass_visit_admit_visitor(value: &gatepass_types::visit::AdmitVisitor, out: &mut String) {
    out.push('{');
    json::member(out, "visit_id");
    encode_gatepass_visit_visit_id(&value.visit_id, out);
    json::member(out, "badge");
    encode_gatepass_visit_badge(&value.badge, out);
    out.push('}');
}

/// Reads the input of `gatepass.visit.AdmitVisitor` from JSON.
///
/// # Errors
///
/// [`json::DecodeError`] naming the path and what the declaration says belongs there.
pub fn decode_command_gatepass_visit_admit_visitor(value: &json::Value, at: &str) -> Result<gatepass_types::visit::AdmitVisitor, json::DecodeError> {
    Ok(gatepass_types::visit::AdmitVisitor {
        visit_id: {
            let at0 = json::nested(at, "visit_id");
            let member0 = json::member_at(value, at, "visit_id")?;
            decode_gatepass_visit_visit_id(member0, &at0)?
        },
        badge: {
            let at1 = json::nested(at, "badge");
            let member1 = json::member_at(value, at, "badge")?;
            decode_gatepass_visit_badge(member1, &at1)?
        },
    })
}

/// Writes the outcome of `gatepass.visit.AdmitVisitor` as JSON: the branch taken, what it published, and the declared
/// refusal it carries where it carries one.
pub fn encode_outcome_gatepass_visit_admit_visitor(value: &gatepass_types::visit::AdmitVisitorOutcome, out: &mut String) {
    out.push('{');
    match value {
        gatepass_types::visit::AdmitVisitorOutcome::Admitted { visitor_admitted } => {
            json::member(out, "outcome");
            json::push_text(out, "admitted");
            json::member(out, "published");
            out.push('[');
            out.push('{');
            json::member(out, "event");
            json::push_text(out, "gatepass.visit.VisitorAdmitted");
            json::member(out, "payload");
            encode_event_gatepass_visit_visitor_admitted(visitor_admitted, out);
            out.push('}');
            out.push(']');
        }
        gatepass_types::visit::AdmitVisitorOutcome::WrongState { error } => {
            json::member(out, "outcome");
            json::push_text(out, "wrong-state");
            json::member(out, "published");
            out.push('[');
            out.push(']');
            json::member(out, "refusal");
            out.push('{');
            json::member(out, "error");
            json::push_text(out, "gatepass.visit.VisitStateConflict");
            json::member(out, "payload");
            encode_error_gatepass_visit_visit_state_conflict(error, out);
            out.push('}');
        }
    }
    out.push('}');
}

/// Writes the input of `gatepass.visit.RegisterVisit` as JSON.
pub fn encode_command_gatepass_visit_register_visit(value: &gatepass_types::visit::RegisterVisit, out: &mut String) {
    out.push('{');
    json::member(out, "visitor");
    encode_gatepass_visit_visitor_name(&value.visitor, out);
    json::member(out, "building");
    encode_gatepass_visit_building(&value.building, out);
    json::member(out, "host");
    encode_gatepass_visit_host(&value.host, out);
    json::member(out, "expected_minutes");
    json::push_integer(out, value.expected_minutes);
    json::member(out, "expected_stay");
    json::push_text(out, &value.expected_stay.0);
    json::member(out, "deposit");
    encode_gatepass_visit_deposit(&value.deposit, out);
    json::member(out, "escorts");
    out.push('[');
    for (index0, item0) in value.escorts.iter().enumerate() {
        if index0 > 0 {
            out.push(',');
        }
        encode_gatepass_visit_visitor_name(&*item0, out);
    }
    out.push(']');
    json::member(out, "notes");
    out.push('{');
    for (index0, (key0, item0)) in value.notes.iter().enumerate() {
        if index0 > 0 {
            out.push(',');
        }
        json::push_text(out, key0);
        out.push(':');
        json::push_text(out, &*item0);
    }
    out.push('}');
    json::member(out, "on_watchlist");
    json::push_bool(out, value.on_watchlist);
    out.push('}');
}

/// Reads the input of `gatepass.visit.RegisterVisit` from JSON.
///
/// # Errors
///
/// [`json::DecodeError`] naming the path and what the declaration says belongs there.
pub fn decode_command_gatepass_visit_register_visit(value: &json::Value, at: &str) -> Result<gatepass_types::visit::RegisterVisit, json::DecodeError> {
    Ok(gatepass_types::visit::RegisterVisit {
        visitor: {
            let at0 = json::nested(at, "visitor");
            let member0 = json::member_at(value, at, "visitor")?;
            decode_gatepass_visit_visitor_name(member0, &at0)?
        },
        building: {
            let at1 = json::nested(at, "building");
            let member1 = json::member_at(value, at, "building")?;
            decode_gatepass_visit_building(member1, &at1)?
        },
        host: {
            let at2 = json::nested(at, "host");
            let member2 = json::member_at(value, at, "host")?;
            decode_gatepass_visit_host(member2, &at2)?
        },
        expected_minutes: {
            let at3 = json::nested(at, "expected_minutes");
            let member3 = json::member_at(value, at, "expected_minutes")?;
            json::integer_at(member3, &at3, "an integer")?
        },
        expected_stay: {
            let at4 = json::nested(at, "expected_stay");
            let member4 = json::member_at(value, at, "expected_stay")?;
            gatepass_types::primitives::Duration(json::text_at(member4, &at4, "an ISO 8601 duration")?.to_owned())
        },
        deposit: {
            let at5 = json::nested(at, "deposit");
            let member5 = json::member_at(value, at, "deposit")?;
            decode_gatepass_visit_deposit(member5, &at5)?
        },
        escorts: {
            let at6 = json::nested(at, "escorts");
            let member6 = json::member_at(value, at, "escorts")?;
            {
                let mut items6 = Vec::new();
                for (index6, element6) in json::items_at(member6, &at6, "an array")?.iter().enumerate() {
                    let nested6 = json::nested(&at6, &index6.to_string());
                    items6.push(decode_gatepass_visit_visitor_name(element6, &nested6)?);
                }
                items6
            }
        },
        notes: {
            let at7 = json::nested(at, "notes");
            let member7 = json::member_at(value, at, "notes")?;
            {
                let mut entries7 = std::collections::BTreeMap::new();
                for (key7, element7) in json::members_at(member7, &at7, "an object")? {
                    let nested7 = json::nested(&at7, key7);
                    entries7.insert(key7.clone(), json::text_at(element7, &nested7, "a string")?.to_owned());
                }
                entries7
            }
        },
        on_watchlist: {
            let at8 = json::nested(at, "on_watchlist");
            let member8 = json::member_at(value, at, "on_watchlist")?;
            json::bool_at(member8, &at8, "a boolean")?
        },
    })
}

/// Writes the outcome of `gatepass.visit.RegisterVisit` as JSON: the branch taken, what it published, and the declared
/// refusal it carries where it carries one.
pub fn encode_outcome_gatepass_visit_register_visit(value: &gatepass_types::visit::RegisterVisitOutcome, out: &mut String) {
    out.push('{');
    match value {
        gatepass_types::visit::RegisterVisitOutcome::Registered { visit_registered } => {
            json::member(out, "outcome");
            json::push_text(out, "registered");
            json::member(out, "published");
            out.push('[');
            out.push('{');
            json::member(out, "event");
            json::push_text(out, "gatepass.visit.VisitRegistered");
            json::member(out, "payload");
            encode_event_gatepass_visit_visit_registered(visit_registered, out);
            out.push('}');
            out.push(']');
        }
        gatepass_types::visit::RegisterVisitOutcome::Refused { error } => {
            json::member(out, "outcome");
            json::push_text(out, "refused");
            json::member(out, "published");
            out.push('[');
            out.push(']');
            json::member(out, "refusal");
            out.push('{');
            json::member(out, "error");
            json::push_text(out, "gatepass.visit.InvalidVisitLength");
            json::member(out, "payload");
            encode_error_gatepass_visit_invalid_visit_length(error, out);
            out.push('}');
        }
    }
    out.push('}');
}

/// Writes the input of `gatepass.visit.SignOutVisitor` as JSON.
pub fn encode_command_gatepass_visit_sign_out_visitor(value: &gatepass_types::visit::SignOutVisitor, out: &mut String) {
    out.push('{');
    json::member(out, "visit_id");
    encode_gatepass_visit_visit_id(&value.visit_id, out);
    out.push('}');
}

/// Reads the input of `gatepass.visit.SignOutVisitor` from JSON.
///
/// # Errors
///
/// [`json::DecodeError`] naming the path and what the declaration says belongs there.
pub fn decode_command_gatepass_visit_sign_out_visitor(value: &json::Value, at: &str) -> Result<gatepass_types::visit::SignOutVisitor, json::DecodeError> {
    Ok(gatepass_types::visit::SignOutVisitor {
        visit_id: {
            let at0 = json::nested(at, "visit_id");
            let member0 = json::member_at(value, at, "visit_id")?;
            decode_gatepass_visit_visit_id(member0, &at0)?
        },
    })
}

/// Writes the outcome of `gatepass.visit.SignOutVisitor` as JSON: the branch taken, what it published, and the declared
/// refusal it carries where it carries one.
pub fn encode_outcome_gatepass_visit_sign_out_visitor(value: &gatepass_types::visit::SignOutVisitorOutcome, out: &mut String) {
    out.push('{');
    match value {
        gatepass_types::visit::SignOutVisitorOutcome::SignedOut { visitor_departed } => {
            json::member(out, "outcome");
            json::push_text(out, "signed-out");
            json::member(out, "published");
            out.push('[');
            out.push('{');
            json::member(out, "event");
            json::push_text(out, "gatepass.visit.VisitorDeparted");
            json::member(out, "payload");
            encode_event_gatepass_visit_visitor_departed(visitor_departed, out);
            out.push('}');
            out.push(']');
        }
        gatepass_types::visit::SignOutVisitorOutcome::WrongState { error } => {
            json::member(out, "outcome");
            json::push_text(out, "wrong-state");
            json::member(out, "published");
            out.push('[');
            out.push(']');
            json::member(out, "refusal");
            out.push('{');
            json::member(out, "error");
            json::push_text(out, "gatepass.visit.VisitStateConflict");
            json::member(out, "payload");
            encode_error_gatepass_visit_visit_state_conflict(error, out);
            out.push('}');
        }
    }
    out.push('}');
}
