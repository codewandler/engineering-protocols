// generated from gatepass v1
// model digest f2e0f8ff51c077fa1c713d8151544379bafac36a5a927e71c685042d53ab6e61
// contract digest e6e58e055d24f8f494dcff274f55e723d967f9d1f9aea16641bb8dacbb71171e
// compiler 0.1.0 · generator 0.1.0
// do not edit: regenerate with `protocol ess synthesize`

//! The `pass-service` component of `gatepass` v1, on the wire.
//!
//! The specification says this component's callers are not deployed with it, so its surface
//! exists on a wire. Which wire is derived rather than chosen: the one contract this model
//! projects for a command surface is the `OpenAPI` document, and an `OpenAPI` document is an
//! HTTP contract. The document is beside this file, served verbatim at `/openapi.json`.

use crate::{http, json, wire};

/// The contract this surface answers, byte for byte as `generated/` commits it.
///
/// Embedded rather than rebuilt at run time: a server that regenerated its own contract could
/// publish one the repository never reviewed.
pub const OPENAPI: &str = include_str!("pass-service.openapi.json");

/// The prose the same model produced, byte for byte as the documentation projection wrote it.
pub const DOCS: &str = include_str!("pass-service.docs.md");

/// Every route this surface answers, in path order.
///
/// The same set the `OpenAPI` document declares, plus the two documents about the surface
/// itself, which no specification construct names and nothing can therefore derive. A path
/// absent from this table is answered with `404`, including one the document declares and this
/// table forgot — which is the failure a table computed twice would hide.
pub const ROUTES: &[(&str, &str)] = &[
    ("GET", "/docs"),
    ("GET", "/openapi.json"),
    ("POST", "/visits/commands/admit-visitor"),
    ("POST", "/visits/commands/register-visit"),
    ("POST", "/visits/commands/sign-out-visitor"),
    ("GET", "/visits/views/by-id"),
    ("GET", "/visits/views/expected"),
];

/// What this process says about itself as it starts, before it answers anything.
///
/// Three lines of JSON on standard output, in this order, every member of them derived from the
/// specification — except `runtime`, which is appended by the emitted code below and holds what
/// is true of *this process*: the language it was synthesised into, and the address it bound.
/// Everything outside `runtime` is the same in every language this plan is emitted into, and
/// `cargo xtask synth --check` starts both and compares them.
pub const STARTUP: &[&str] = &[
    "{\"log\":\"ess/1\",\"event\":\"system.starting\",\"system\":\"gatepass\",\"version\":\"v1\",\"model_digest\":\"f2e0f8ff51c077fa1c713d8151544379bafac36a5a927e71c685042d53ab6e61\",\"contract_digest\":\"e6e58e055d24f8f494dcff274f55e723d967f9d1f9aea16641bb8dacbb71171e\",\"components\":[\"pass-service\"],\"capabilities\":{\"generated\":22,\"obligations\":5,\"refused\":2}",
    "{\"log\":\"ess/1\",\"event\":\"surface.serving\",\"component\":\"pass-service\",\"reached_by\":\"network\",\"transport\":\"http/1.1\",\"routes\":7,\"paths\":[{\"method\":\"GET\",\"path\":\"/docs\",\"serves\":\"documentation\",\"name\":\"docs\"},{\"method\":\"GET\",\"path\":\"/openapi.json\",\"serves\":\"contract\",\"name\":\"openapi\"},{\"method\":\"POST\",\"path\":\"/visits/commands/admit-visitor\",\"serves\":\"command\",\"name\":\"gatepass.visit.AdmitVisitor\"},{\"method\":\"POST\",\"path\":\"/visits/commands/register-visit\",\"serves\":\"command\",\"name\":\"gatepass.visit.RegisterVisit\"},{\"method\":\"POST\",\"path\":\"/visits/commands/sign-out-visitor\",\"serves\":\"command\",\"name\":\"gatepass.visit.SignOutVisitor\"},{\"method\":\"GET\",\"path\":\"/visits/views/by-id\",\"serves\":\"view\",\"name\":\"gatepass.visit.VisitById\"},{\"method\":\"GET\",\"path\":\"/visits/views/expected\",\"serves\":\"view\",\"name\":\"gatepass.visit.ExpectedVisits\"}]",
    "{\"log\":\"ess/1\",\"event\":\"system.ready\",\"system\":\"gatepass\",\"surfaces\":1",
];

/// Writes the startup record, with this process's own facts closing each line.
fn announce(address: &std::net::SocketAddr) {
    for facts in STARTUP {
        let mut line = String::from(*facts);
        line.push_str(",\"runtime\":{\"address\":");
        json::push_text(&mut line, &address.to_string());
        line.push_str(",\"language\":\"rust\",\"port\":");
        json::push_integer(&mut line, i64::from(address.port()));
        line.push_str("}}");
        println!("{line}");
    }
}

/// Serves `pass-service` at `address`, and does not return while it can answer.
///
/// `address` may name port `0`, which binds an ephemeral port; the startup record says which one
/// was taken, because a caller that cannot learn the port cannot make a request.
///
/// It chooses no realization. Every command reaches the port, and a port over unimplemented
/// obligations answers the typed refusal this surface reports as `501` — the honest empty
/// state rather than a server that pretends.
///
/// # Errors
///
/// Anything the listener refuses: the address is taken, the port is privileged, the socket
/// died.
pub fn serve<PassServiceBehaviors>(system: &mut gatepass_system::System<PassServiceBehaviors>, address: &str) -> std::io::Result<()>
where
    PassServiceBehaviors: gatepass_types::visit::obligations::AdmitVisitorBehavior + gatepass_types::visit::obligations::RegisterVisitBehavior + gatepass_types::visit::obligations::SignOutVisitorBehavior + gatepass_types::visit::obligations::ExpectedVisitsQuery + gatepass_types::visit::obligations::VisitByIdQuery,
{
    let listener = std::net::TcpListener::bind(address)?;
    announce(&listener.local_addr()?);
    for connection in listener.incoming() {
        let mut reader = std::io::BufReader::new(connection?);
        let answer = match http::read(&mut reader) {
            Ok(request) => dispatch(system, &request),
            Err(refusal) => refusal,
        };
        let mut stream = reader.into_inner();
        http::write(&mut stream, &answer)?;
    }
    Ok(())
}

/// Answers one request.
///
/// A path this table does not hold is a `404` naming where the whole table is published; a
/// path it holds under a different method is a `405` naming the one it answers. Neither is a
/// status the contract declares, and neither should be: both are facts about a transport rather
/// than about any command.
fn dispatch<PassServiceBehaviors>(system: &mut gatepass_system::System<PassServiceBehaviors>, request: &http::Request) -> http::Response
where
    PassServiceBehaviors: gatepass_types::visit::obligations::AdmitVisitorBehavior + gatepass_types::visit::obligations::RegisterVisitBehavior + gatepass_types::visit::obligations::SignOutVisitorBehavior + gatepass_types::visit::obligations::ExpectedVisitsQuery + gatepass_types::visit::obligations::VisitByIdQuery,
{
    match request.path.as_str() {
        "/docs" => {
            if request.method != "GET" {
                return http::method_not_allowed("GET");
            }
            http::Response::new(200, http::MARKDOWN, DOCS)
        }
        "/openapi.json" => {
            if request.method != "GET" {
                return http::method_not_allowed("GET");
            }
            http::Response::new(200, http::JSON, OPENAPI)
        }
        "/visits/commands/admit-visitor" => {
            if request.method != "POST" {
                return http::method_not_allowed("POST");
            }
            serve_gatepass_visit_admit_visitor(system, &request.body)
        }
        "/visits/commands/register-visit" => {
            if request.method != "POST" {
                return http::method_not_allowed("POST");
            }
            serve_gatepass_visit_register_visit(system, &request.body)
        }
        "/visits/commands/sign-out-visitor" => {
            if request.method != "POST" {
                return http::method_not_allowed("POST");
            }
            serve_gatepass_visit_sign_out_visitor(system, &request.body)
        }
        "/visits/views/by-id" => {
            if request.method != "GET" {
                return http::method_not_allowed("GET");
            }
            serve_gatepass_visit_visit_by_id(system)
        }
        "/visits/views/expected" => {
            if request.method != "GET" {
                return http::method_not_allowed("GET");
            }
            serve_gatepass_visit_expected_visits(system)
        }
        other => http::Response::refusal(
            404,
            &format!("`{other}` is not a path this surface declares; `GET /openapi.json` publishes every one that is"),
        ),
    }
}

/// `POST` `gatepass.visit.AdmitVisitor`: reads the declared input, runs the port, answers the declared outcome.
fn serve_gatepass_visit_admit_visitor<PassServiceBehaviors>(system: &mut gatepass_system::System<PassServiceBehaviors>, body: &[u8]) -> http::Response
where
    PassServiceBehaviors: gatepass_types::visit::obligations::AdmitVisitorBehavior + gatepass_types::visit::obligations::RegisterVisitBehavior + gatepass_types::visit::obligations::SignOutVisitorBehavior + gatepass_types::visit::obligations::ExpectedVisitsQuery + gatepass_types::visit::obligations::VisitByIdQuery,
{
    let text = match std::str::from_utf8(body) {
        Ok(text) => text,
        Err(error) => {
            return http::Response::refusal(400, &format!("the body is not UTF-8: {error}"));
        }
    };
    let value = match json::parse(text) {
        Ok(value) => value,
        Err(error) => {
            return http::Response::refusal(400, &format!("the body is not JSON: {error}"));
        }
    };
    let input = match wire::decode_command_gatepass_visit_admit_visitor(&value, "body") {
        Ok(input) => input,
        Err(error) => {
            // `400` and not `422`: this is a body the schema decides, which is the difference
            // between fixing a value and fixing a serialiser.
            return http::Response::refusal(400, &format!("{error}"));
        }
    };
    match system.pass_service.admit_visitor(input) {
        Ok(outcome) => answer_gatepass_visit_admit_visitor(&outcome),
        Err(unmet) => http::Response::refusal(501, &format!("{unmet}")),
    }
}

/// One declared outcome of `gatepass.visit.AdmitVisitor`, as the contract publishes it: the branch that was taken,
/// the declared error where there is one, and that error's own payload.
fn answer_gatepass_visit_admit_visitor(outcome: &gatepass_types::visit::AdmitVisitorOutcome) -> http::Response {
    let mut body = String::from("{");
    let status = match outcome {
        gatepass_types::visit::AdmitVisitorOutcome::Admitted { .. } => {
            json::member(&mut body, "outcome");
            json::push_text(&mut body, "admitted");
            202
        }
        gatepass_types::visit::AdmitVisitorOutcome::WrongState { error, .. } => {
            json::member(&mut body, "outcome");
            json::push_text(&mut body, "wrong-state");
            json::member(&mut body, "error");
            json::push_text(&mut body, "gatepass.visit.VisitStateConflict");
            json::member(&mut body, "payload");
            wire::encode_error_gatepass_visit_visit_state_conflict(error, &mut body);
            409
        }
    };
    body.push('}');
    http::Response::new(status, http::JSON, body)
}

/// `POST` `gatepass.visit.RegisterVisit`: reads the declared input, runs the port, answers the declared outcome.
fn serve_gatepass_visit_register_visit<PassServiceBehaviors>(system: &mut gatepass_system::System<PassServiceBehaviors>, body: &[u8]) -> http::Response
where
    PassServiceBehaviors: gatepass_types::visit::obligations::AdmitVisitorBehavior + gatepass_types::visit::obligations::RegisterVisitBehavior + gatepass_types::visit::obligations::SignOutVisitorBehavior + gatepass_types::visit::obligations::ExpectedVisitsQuery + gatepass_types::visit::obligations::VisitByIdQuery,
{
    let text = match std::str::from_utf8(body) {
        Ok(text) => text,
        Err(error) => {
            return http::Response::refusal(400, &format!("the body is not UTF-8: {error}"));
        }
    };
    let value = match json::parse(text) {
        Ok(value) => value,
        Err(error) => {
            return http::Response::refusal(400, &format!("the body is not JSON: {error}"));
        }
    };
    let input = match wire::decode_command_gatepass_visit_register_visit(&value, "body") {
        Ok(input) => input,
        Err(error) => {
            // `400` and not `422`: this is a body the schema decides, which is the difference
            // between fixing a value and fixing a serialiser.
            return http::Response::refusal(400, &format!("{error}"));
        }
    };
    match system.pass_service.register_visit(input) {
        Ok(outcome) => answer_gatepass_visit_register_visit(&outcome),
        Err(unmet) => http::Response::refusal(501, &format!("{unmet}")),
    }
}

/// One declared outcome of `gatepass.visit.RegisterVisit`, as the contract publishes it: the branch that was taken,
/// the declared error where there is one, and that error's own payload.
fn answer_gatepass_visit_register_visit(outcome: &gatepass_types::visit::RegisterVisitOutcome) -> http::Response {
    let mut body = String::from("{");
    let status = match outcome {
        gatepass_types::visit::RegisterVisitOutcome::Registered { .. } => {
            json::member(&mut body, "outcome");
            json::push_text(&mut body, "registered");
            202
        }
        gatepass_types::visit::RegisterVisitOutcome::Refused { error, .. } => {
            json::member(&mut body, "outcome");
            json::push_text(&mut body, "refused");
            json::member(&mut body, "error");
            json::push_text(&mut body, "gatepass.visit.InvalidVisitLength");
            json::member(&mut body, "payload");
            wire::encode_error_gatepass_visit_invalid_visit_length(error, &mut body);
            422
        }
    };
    body.push('}');
    http::Response::new(status, http::JSON, body)
}

/// `POST` `gatepass.visit.SignOutVisitor`: reads the declared input, runs the port, answers the declared outcome.
fn serve_gatepass_visit_sign_out_visitor<PassServiceBehaviors>(system: &mut gatepass_system::System<PassServiceBehaviors>, body: &[u8]) -> http::Response
where
    PassServiceBehaviors: gatepass_types::visit::obligations::AdmitVisitorBehavior + gatepass_types::visit::obligations::RegisterVisitBehavior + gatepass_types::visit::obligations::SignOutVisitorBehavior + gatepass_types::visit::obligations::ExpectedVisitsQuery + gatepass_types::visit::obligations::VisitByIdQuery,
{
    let text = match std::str::from_utf8(body) {
        Ok(text) => text,
        Err(error) => {
            return http::Response::refusal(400, &format!("the body is not UTF-8: {error}"));
        }
    };
    let value = match json::parse(text) {
        Ok(value) => value,
        Err(error) => {
            return http::Response::refusal(400, &format!("the body is not JSON: {error}"));
        }
    };
    let input = match wire::decode_command_gatepass_visit_sign_out_visitor(&value, "body") {
        Ok(input) => input,
        Err(error) => {
            // `400` and not `422`: this is a body the schema decides, which is the difference
            // between fixing a value and fixing a serialiser.
            return http::Response::refusal(400, &format!("{error}"));
        }
    };
    match system.pass_service.sign_out_visitor(input) {
        Ok(outcome) => answer_gatepass_visit_sign_out_visitor(&outcome),
        Err(unmet) => http::Response::refusal(501, &format!("{unmet}")),
    }
}

/// One declared outcome of `gatepass.visit.SignOutVisitor`, as the contract publishes it: the branch that was taken,
/// the declared error where there is one, and that error's own payload.
fn answer_gatepass_visit_sign_out_visitor(outcome: &gatepass_types::visit::SignOutVisitorOutcome) -> http::Response {
    let mut body = String::from("{");
    let status = match outcome {
        gatepass_types::visit::SignOutVisitorOutcome::SignedOut { .. } => {
            json::member(&mut body, "outcome");
            json::push_text(&mut body, "signed-out");
            202
        }
        gatepass_types::visit::SignOutVisitorOutcome::WrongState { error, .. } => {
            json::member(&mut body, "outcome");
            json::push_text(&mut body, "wrong-state");
            json::member(&mut body, "error");
            json::push_text(&mut body, "gatepass.visit.VisitStateConflict");
            json::member(&mut body, "payload");
            wire::encode_error_gatepass_visit_visit_state_conflict(error, &mut body);
            409
        }
    };
    body.push('}');
    http::Response::new(status, http::JSON, body)
}

/// `GET` `gatepass.visit.VisitById` at `eventual` consistency: every row the owed projection holds.
fn serve_gatepass_visit_visit_by_id<PassServiceBehaviors>(system: &gatepass_system::System<PassServiceBehaviors>) -> http::Response
where
    PassServiceBehaviors: gatepass_types::visit::obligations::AdmitVisitorBehavior + gatepass_types::visit::obligations::RegisterVisitBehavior + gatepass_types::visit::obligations::SignOutVisitorBehavior + gatepass_types::visit::obligations::ExpectedVisitsQuery + gatepass_types::visit::obligations::VisitByIdQuery,
{
    match system.pass_service.visit_by_id() {
        Ok(rows) => {
            let mut body = String::from("{");
            json::member(&mut body, "rows");
            body.push('[');
            for (position, row) in rows.iter().enumerate() {
                if position > 0 {
                    body.push(',');
                }
                wire::encode_view_gatepass_visit_visit_by_id(row, &mut body);
            }
            body.push(']');
            body.push('}');
            http::Response::new(200, http::JSON, body)
        }
        Err(unmet) => http::Response::refusal(501, &format!("{unmet}")),
    }
}

/// `GET` `gatepass.visit.ExpectedVisits` at `read_your_writes` consistency: every row the owed projection holds.
fn serve_gatepass_visit_expected_visits<PassServiceBehaviors>(system: &gatepass_system::System<PassServiceBehaviors>) -> http::Response
where
    PassServiceBehaviors: gatepass_types::visit::obligations::AdmitVisitorBehavior + gatepass_types::visit::obligations::RegisterVisitBehavior + gatepass_types::visit::obligations::SignOutVisitorBehavior + gatepass_types::visit::obligations::ExpectedVisitsQuery + gatepass_types::visit::obligations::VisitByIdQuery,
{
    match system.pass_service.expected_visits() {
        Ok(rows) => {
            let mut body = String::from("{");
            json::member(&mut body, "rows");
            body.push('[');
            for (position, row) in rows.iter().enumerate() {
                if position > 0 {
                    body.push(',');
                }
                wire::encode_view_gatepass_visit_expected_visits(row, &mut body);
            }
            body.push(']');
            body.push('}');
            http::Response::new(200, http::JSON, body)
        }
        Err(unmet) => http::Response::refusal(501, &format!("{unmet}")),
    }
}
