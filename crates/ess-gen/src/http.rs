//! The HTTP surface one component's specification determines: one route table, read by everything.
//!
//! # Why this is a module and not a paragraph in `openapi.rs`
//!
//! Two artifacts have to agree about what `POST /invoices/commands/create-invoice` means: the
//! `OpenAPI` document that publishes the contract, and the synthesised server that answers it. When
//! each computed its own paths they would agree on the day they were written and drift the first
//! time a wire name moved — and the drift is invisible, because a server serving a path no document
//! declares looks exactly like a server that works. So the mapping lives here, once, and both read
//! it. It is the same argument `src/types.rs` settles for schemas, and this crate has already paid
//! for learning it: `openapi.rs` and `asyncapi.rs` each carried a copy of the type mapping, both
//! drifted, and the drift was published as two contradictory contracts for one event.
//!
//! # A component is only served when the specification says it is
//!
//! [`routes`] answers for every component, because every component has an `OpenAPI` document. What
//! changes with [`Reach::Network`] is the **view** half: a
//! view has no path at all until the specification says something outside the process reads it.
//! Until then, exposing one would invent a query surface — which is the row `openapi.rs`'s "what
//! this refuses to guess" table has always carried, and this is that row being closed by a
//! declaration rather than by a generator's opinion.
//!
//! # What is still not invented
//!
//! No pagination, no cursor, no ordering, no filter parameter: a view's filter is declared in the
//! model and its rows are what the projection holds. No `servers`, because the model still has no
//! URL. No path version, because `info.version` is the only version the model has. The two segments
//! a route is built from — the domain's wire name and the construct's — are both declared, and the
//! `commands`/`views` segment between them is what stops a path from reading as a resource.

use ess_compiler::ir::{
    CommandHandle, EssIr, ResolvedComponent, ResolvedCondition, ResolvedOutcome, ViewHandle,
};
use ess_domain::component::Reach;

/// Where the served component publishes the contract it answers.
///
/// A fixed path rather than a derived one: it names no construct of any specification, so there is
/// nothing to derive it from, and a served document a caller cannot find is a document that is not
/// published. JSON rather than the committed YAML because this is the machine's copy — the same
/// document, in the dialect every HTTP client already parses.
pub const OPENAPI: &str = "/openapi.json";

/// Where it publishes the prose the same model produced.
pub const DOCS: &str = "/docs";

/// The branch was taken; its events reach consumers elsewhere.
pub const TAKEN: &str = "202";

/// The projection was read.
///
/// The one `2xx` here that is not a `202`: a `GET` on a view returns the rows, so the response
/// *is* the answer rather than a receipt for a branch that was taken.
pub const READ: &str = "200";

/// The input was understood and refused on domain grounds.
///
/// A `422` and not a `400`: `400` is for a request the server could not parse, which is decided by
/// the schema and would be true of any endpoint. A refusal on domain grounds is a request the
/// server understood, and a client can act on the difference — one means fix the value, the other
/// means fix the serialiser.
pub const REFUSED: &str = "422";

/// Something outside the request refused; the input was acceptable.
///
/// A `5xx` because `external` names a branch the input cannot decide. Reporting it as a `4xx` sends
/// the caller to fix the one thing it cannot fix and tells every retry layer between that retrying
/// is pointless. `502` rather than `500`, which would claim a fault in this component, or `503`,
/// which would claim the whole component is unavailable when one provider refused one request.
pub const UPSTREAM: &str = "502";

/// The input was acceptable and the subject was in a state the command does not act from.
///
/// A `409` and not a `422`, and the difference is what a caller does next. `422` says the request
/// was wrong and resending it unchanged is pointless; `409` says the request was fine and the world
/// was not — the same visit, admitted before it was signed out, would have been accepted.
pub const CONFLICT: &str = "409";

/// Which status one declared outcome is.
///
/// The whole mapping, in one place, so that "which HTTP status does this refusal get" has exactly
/// one answer for the document that publishes it and the server that answers it. A server whose
/// statuses were computed separately would agree on the day it was written.
pub fn status(outcome: &ResolvedOutcome) -> &'static str {
    match (&outcome.condition, outcome.error.is_some()) {
        (ResolvedCondition::External { .. }, true) => UPSTREAM,
        (ResolvedCondition::WrongState, true) => CONFLICT,
        (ResolvedCondition::When { .. } | ResolvedCondition::Otherwise, true) => REFUSED,
        // An external branch that emits rather than errors is still a branch that was taken; what
        // decided it does not change what happened.
        (_, false) => TAKEN,
    }
}

/// The two methods this surface uses, and no others.
///
/// A command changes state, so it is a `POST`; a view is a projection a caller reads, so it is a
/// `GET`. Nothing here is a resource, so there is no `PUT` and no `DELETE` — the model describes no
/// addressable thing to replace or remove, and inventing one is exactly what the command-endpoint
/// convention exists to avoid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Method {
    /// Reads a projection.
    Get,
    /// Issues a command.
    Post,
}

impl Method {
    /// The verb as it appears on the wire and in the document.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
        }
    }

    /// The `OpenAPI` path-item key, which is the verb in lower case.
    pub fn keyword(self) -> &'static str {
        match self {
            Self::Get => "get",
            Self::Post => "post",
        }
    }
}

/// What one route serves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Served<'a> {
    /// One command the component accepts.
    Command(&'a CommandHandle),
    /// One view a domain the component owns declares.
    View(&'a ViewHandle),
}

/// One route of a component's HTTP surface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Route<'a> {
    /// The verb.
    pub method: Method,
    /// The path, from the declared wire names.
    pub path: String,
    /// The construct it serves.
    pub serves: Served<'a>,
}

/// Every route one component's surface has, in path order.
///
/// The command routes are the ones `openapi.rs` has always published, unchanged and for every
/// component. The view routes exist only where the component declares that something outside the
/// process reaches it, because that is the declaration that turns "how is a view read" from a
/// question this generator would have to answer into one the specification has answered.
///
/// # Collisions
///
/// Two commands can derive one path — two domains may share a wire name, and a wire name is free
/// text. When that happens *both* move to their qualified names rather than one keeping the short
/// path, because a path whose meaning depends on which other commands exist is a path that changes
/// when an unrelated command is added. Views collide by the same rule and move the same way. A
/// command and a view cannot collide with each other: the segment between the domain and the name
/// is `commands` for one and `views` for the other.
pub fn routes<'a>(ir: &'a EssIr, component: &'a ResolvedComponent) -> Vec<Route<'a>> {
    let mut out: Vec<Route<'a>> = Vec::new();

    let mut claimed: std::collections::BTreeMap<String, Vec<&'a CommandHandle>> =
        std::collections::BTreeMap::new();
    for handle in &component.accepts {
        claimed
            .entry(command_path(ir, handle))
            .or_default()
            .push(handle);
    }
    for (path, handles) in claimed {
        let contested = handles.len() > 1;
        for handle in handles {
            out.push(Route {
                method: Method::Post,
                path: if contested {
                    format!("/commands/{}", ir.command(handle).name)
                } else {
                    path.clone()
                },
                serves: Served::Command(handle),
            });
        }
    }

    if component.reached_by == Reach::Network {
        let mut claimed: std::collections::BTreeMap<String, Vec<&'a ViewHandle>> =
            std::collections::BTreeMap::new();
        for domain in &component.owns {
            for handle in &ir.domain(domain).views {
                claimed
                    .entry(view_path(ir, handle))
                    .or_default()
                    .push(handle);
            }
        }
        for (path, handles) in claimed {
            let contested = handles.len() > 1;
            for handle in handles {
                out.push(Route {
                    method: Method::Get,
                    path: if contested {
                        format!("/views/{}", ir.view(handle).name)
                    } else {
                        path.clone()
                    },
                    serves: Served::View(handle),
                });
            }
        }
    }

    out.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then(left.method.cmp(&right.method))
    });
    out
}

/// `/{domain wire name}/commands/{command wire name}`.
fn command_path(ir: &EssIr, handle: &CommandHandle) -> String {
    let command = ir.command(handle);
    let domain = ir.domain(&command.domain);
    format!(
        "/{}/commands/{}",
        domain.naming.wire_or(&domain.name),
        command.naming.wire_or(&command.name)
    )
}

/// `/{domain wire name}/views/{view wire name}`.
///
/// The `views` segment does the job `commands` does: it keeps the path from reading as a collection
/// resource, which is the claim this generator has no grounds for. A view *is* a collection of rows
/// — but it is a projection of entities the model never gives an address, and `/invoices/outstanding`
/// would invite a caller to expect `/invoices/{id}` beside it.
fn view_path(ir: &EssIr, handle: &ViewHandle) -> String {
    let view = ir.view(handle);
    let domain = ir.domain(&view.domain);
    format!(
        "/{}/views/{}",
        domain.naming.wire_or(&domain.name),
        view.naming.wire_or(&view.name)
    )
}
