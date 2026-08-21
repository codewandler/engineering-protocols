//! `OpenAPI` 3.1, one document per component.
//!
//! # A command is not an endpoint
//!
//! `CreateInvoice` is a semantic command. `POST /invoices` is one way to expose it, and design §6
//! draws exactly that line: an inner domain of entities, state, invariants and commands, an outer
//! surface of HTTP APIs, topics and queues, and *mappings* between them. That line is what lets one
//! specification compile to a modular monolith or to distributed services without touching the
//! domain model — and a generator that turns commands into paths is precisely where it gets erased,
//! quietly, by a convention nobody wrote down.
//!
//! So the convention is written down here. §6 puts the decision in the model:
//!
//! ```yaml
//! command:
//!   ref: invoice.CreateInvoice
//! exposures:
//!   - kind: http
//!     method: POST
//!     path: /v1/invoices
//!     response:
//!       status: 201
//! ```
//!
//! **The model has no `exposures` construct.** Nothing in [`EssIr`] carries a method, a path or a
//! status, so every row of the table below is *this generator's* decision rather than the
//! specification's, and each row is a rule a reviewer can disagree with. Until `exposures:` exists,
//! the convention is the whole answer, and it is deliberately the least inventive one available:
//! REST resource modelling would require guessing that `Create` is a verb, that `Invoice`
//! pluralises to a collection, and that an invoice is addressable at a URL the specification never
//! mentions. A command endpoint guesses none of that.
//!
//! | question | this generator's answer | what it rests on |
//! |---|---|---|
//! | one document per what? | one per component, at `openapi/{component}.yaml` | §5 — a component is the unit of ownership, so it is the unit that has an API |
//! | which commands? | the ones the component `accepts` | §5 |
//! | a command no component accepts | appears in no document at all | no component answers for it; naming one would be inventing an owner |
//! | a component that accepts nothing | still gets a document, with `paths: {}` | a missing file is indistinguishable from a generator that broke |
//! | method | always `POST` | a command changes state; the model declares no command safe and describes no resource to `PUT` or `DELETE` |
//! | path | `/{domain wire name}/commands/{command wire name}` | both are declared wire names; the `commands` segment is what stops the path pretending to be a resource |
//! | two commands claiming one path | *both* move to `/commands/{qualified name}` | qualified names are unique by construction, so the fallback cannot collide in turn |
//! | a command with no `naming.wire` | its qualified name's last segment, verbatim — `SendEmail`, capitals and all | [`Naming::wire_or`](ess_domain::name::Naming::wire_or). A generator inventing its own kebab-casing would disagree with every other projection about what this command is called on the wire |
//! | `operationId` | the command's qualified name, verbatim | unique already; any prettifying transformation trades that guarantee for cosmetics |
//! | request body | the command's `input`, `required` when any input field is | |
//! | an outcome with no `error` | `202` | below |
//! | an outcome whose `error` the input decides | `422` | below |
//! | an outcome whose `error` is `external` | `502` | below |
//! | an outcome whose `error` is `wrong_state` | `409` | below |
//! | several outcomes on one status | one response, `oneOf` the outcome schemas, discriminated by `outcome` | a status that collapsed two branches would lose the branch |
//! | `Idempotency-Key` | a **required** header, on exactly the commands some binding invokes with `delivery: at_least_once` | below |
//! | schemas | inline in `components.schemas`; no `$ref` ever leaves the document | below |
//!
//! # Outcomes are the interesting part
//!
//! A command's `outcomes` are the reason wave 1 restructured the model (review F1): the accepted
//! branch and the refused branch are different results, and a projection that keeps only the first
//! has thrown away the branch where the money does not move. Each outcome therefore becomes its own
//! response schema, named `{command}.{outcome}.Response`, carrying:
//!
//! * `outcome` — a `const` of the declared outcome name, so a client can tell two branches apart
//!   even when they share a status;
//! * `error` — a `const` of the declared error's qualified name, when the outcome names one;
//! * `payload` — the [`ResolvedError`]'s own fields, when it declares any. An error that carries
//!   nothing gets no `payload`, rather than an empty object that looks like a mistake.
//!
//! Events are *not* in the response. An outcome emits events, and over HTTP those reach consumers
//! through the event transport — the response says which branch ran, and the description names the
//! events, because claiming they are returned here would be a claim about a transport the
//! specification has not chosen.
//!
//! # `external` is a `502`, not a `422`
//!
//! `external` names a branch the input cannot decide — `failed: external: the provider rejects the
//! recipient address`. Reporting that as a `4xx` tells the caller it did something wrong, sends it
//! to fix the one thing it cannot fix, and tells every retry layer in between that retrying is
//! pointless. For an external cause, retrying is exactly the right move. So it is a `5xx`: `502`,
//! attributing the refusal to a dependency, rather than `500`, which would claim a fault in this
//! component, or `503`, which would claim the whole component is unavailable when one provider
//! refused one request.
//!
//! An input-decided refusal is `422` and not `400`: `400` is for a request the server could not
//! parse, which is decided by the schema and would be true of any endpoint. `amount.amount <= 0` is
//! a request the server understood and refused on domain grounds, which is what `422` means, and a
//! client can act on the difference — one means fix the value, the other means fix the serialiser.
//!
//! # `wrong_state` is a `409`, and it is a third thing
//!
//! A `wrong_state:` branch is refused for a reason the caller did not cause and can fix: the invoice
//! it asked to issue is already paid. `422` would tell it to correct a request that was correct, and
//! `502` would blame a dependency that was never involved. `409 Conflict` is the one status that
//! says what happened — the request conflicts with the state of the thing it names — and until the
//! model could declare that branch at all, every projection had to collapse it into one of the other
//! two or into nothing.
//!
//! # Idempotency comes from the bindings, not from the verb
//!
//! `delivery: at_least_once` on a binding says the command it invokes may arrive more than once for
//! one cause. The consequence lands on the receiver: someone will call `SendEmail` twice for one
//! `InvoiceCreated`, and a surface with no way to say "this is the same invocation as the last one"
//! leaves the receiver deduplicating with no key. So the header exists exactly where the model says
//! a repeat is permitted, and it is **required** there — optional, an at-least-once caller could
//! omit it and `at_least_once` would become a word with no mechanism behind it.
//!
//! `CreateInvoice`, which no binding invokes, gets no header: nothing in the specification says
//! anyone may call it twice, and adding it anyway would be this generator inventing a delivery
//! guarantee.
//!
//! # Schemas are inline, and the mapping is not this file's
//!
//! Every `$ref` resolves inside the document it appears in. The alternative — pointing at the
//! `schema` projection's files — couples two generators through a path layout that nothing checks,
//! breaks the moment either directory changes, and produces a document that cannot be pasted into a
//! validator without its siblings.
//!
//! What is *in* the document comes from `schema::types`, which is this crate's one
//! answer to "what is a valid `billing.invoice.Money`". `OpenAPI` 3.1's schema dialect **is** JSON
//! Schema 2020-12, so there is nothing to translate: the fragment this file embeds is the fragment
//! the `schema` projection writes, keyword for keyword, with its pointers retargeted at
//! `components.schemas` by `under_components`. This file used to carry a copy of that mapping and
//! `asyncapi.rs` a second one; both drifted from it and from each other, and the drift was published
//! as contradictory contracts for one event. `tests/agreement.rs` is what holds the three documents
//! to one answer now, so the way to change what a `Decimal` looks like here is to change
//! `src/types.rs` and get all three at once.
//!
//! The one thing this file still decides about a schema is which schemas exist: the named leaves of
//! every accepted command's input and of every declared error's payload, transitively. A component's
//! document describes its own surface and nothing else.
//!
//! Two `OpenAPI`-specific consequences of that dialect are worth naming. `Bytes` is
//! `contentEncoding: base64`, which is 2020-12's own keyword — `format: byte` is `OpenAPI` 3.0's
//! spelling and is not a 3.1 format. And a `title` or a `description` beside a `$ref` is meaningful
//! in 3.1 and would have been silently discarded in 3.0, which is one of the reasons this generator
//! emits 3.1.
//!
//! What this file lost in the unification was prose. It used to fold a construct's invariants and a
//! generated sentence about newtype distinctness into `description`, beside whatever the author had
//! written there. Both facts are still published — `x-ess-invariants` carries the author's own
//! statements and `x-ess-kind: newtype` says which construct it is — and both are now readable by a
//! tool, which is what neither could be as prose. The `description` is the author's words and nothing
//! else, so a reader can tell what the specification said from what the generator said, and a drift
//! check does not have to parse English. `tests/agreement.rs` compares annotations across the three
//! projections as strictly as it compares assertions, for the reason `src/asyncapi.rs` states under
//! "An annotation is a fact too": a fact with two spellings is a fact every consumer has to
//! reconcile.
//!
//! # What this refuses to guess
//!
//! Each of these is something an `OpenAPI` document usually has and this model says nothing about.
//! Emitting a plausible default would put a claim in a contract that no specification backs.
//!
//! | omitted | why |
//! |---|---|
//! | `servers` | the model has no URL for a component; `topology` describes replicas and resources, not addresses |
//! | `security`, `securitySchemes` | the model states no authentication. See below: `may:` has landed, and it answers a different question |
//! | a version in the path | `info.version` carries the specification's version, which is the only version the model has. A path prefix would invent an API version that can disagree with it |
//! | pagination, filtering, sorting | commands, not collections. A view *is* in the IR — its fields, its filter, its consistency — but nothing in the model says how one is *read*: no path, no page size, no cursor, no ordering. Exposing one would invent a query surface, and §6's `exposures:` is where that decision belongs. A view is rendered by the documentation projection and by no path here |
//! | `400`, `401`, `404`, `429`, `500` | no outcome declares them. They are properties of a transport and a deployment, not of a command |
//! | `Location`, `ETag`, cache headers | each asserts a resource at an address; the model describes no addressable resource |
//! | `201 Created` | it claims a resource now exists at a URL this document does not have. `202` claims what the model actually states: the branch was taken |
//!
//! ## `may:` is authorization, and `security` is authentication
//!
//! That row's reason used to be "actors and their `may:` lists are not in this wave's IR". They are
//! now: [`ResolvedActor::may`](ess_compiler::ir::ResolvedActor::may) carries resolved command
//! handles, and [`EssIr::grants`] asks the question in the direction a generator needs it. The
//! omission stands anyway, for a different reason, and the difference matters because the old reason
//! would have made emitting a scheme the obvious next step.
//!
//! `may:` says *which actor may invoke this command*. `securitySchemes` says *how a caller proves it
//! is that actor* — a bearer token, an API key, an OAuth flow, at some URL, issued by somebody. The
//! model states none of that, so a `security` block would be this generator inventing an
//! authentication mechanism and every client generated from the document would implement the
//! invention. That is exactly what this table exists to prevent, and it is worse than a gap: a gap is
//! visible.
//!
//! The grant is not dropped, though — a fact the specification states and no artifact carries is a
//! fact that stops being true. Every operation carries `x-ess-may-invoke`, naming the actors the
//! specification permits. It is an extension and therefore an annotation: no tool enforces it, no
//! request is refused by it, and it makes no claim about how the caller was identified. When the
//! model grows an authentication construct, `security` is what this row changes to, and
//! `x-ess-may-invoke` is what tells whoever writes it which operations need one.
//!
//! # Determinism
//!
//! No clock, no RNG, and every collection a [`BTreeMap`], a [`BTreeSet`](std::collections::BTreeSet) or a [`Vec`] built from
//! one. `tests/openapi.rs` generates twice and compares bytes, because that is the only form of this
//! claim that is worth anything (review F8).

use std::collections::BTreeMap;

use ess_compiler::ir::{
    CommandHandle, EssIr, ResolvedActor, ResolvedCommand, ResolvedComponent, ResolvedCondition,
    ResolvedEffect, ResolvedError, ResolvedOutcome, ResolvedView, TypeHandle, ViewHandle,
};
use ess_domain::binding::Delivery;
use ess_domain::component::Reach;
use ess_domain::view::Consistency;
use serde_json::{json, Map, Value};

use crate::artifact::{Artifact, Generator};
use crate::http::{self, status, CONFLICT, READ, REFUSED, UPSTREAM};
use ess_compiler::refs::{ActorRef, BindingRef, ComponentRef, EssSemanticRef};

use crate::provenance::{Provenance, ProvenanceMint, SlicedProvenance};
use crate::schema::types::{self, Message, Node};

/// The specification version emitted. 3.1 rather than 3.0 because 3.1's schema dialect is JSON
/// Schema 2020-12, which is the dialect the type mapping below actually produces — a 3.0 document
/// would have to lie about `$ref` siblings, `null` and `contentEncoding`.
const VERSION: &str = "3.1.0";

/// The one media type. The model describes payloads as typed fields, and nothing in it selects an
/// encoding, so there is exactly one and it is the one every other projection here uses.
const MEDIA_TYPE: &str = "application/json";

/// The header that carries a caller's invocation identity, spelt as the ecosystem spells it.
const IDEMPOTENCY_KEY: &str = "Idempotency-Key";

/// The response property naming which declared branch was taken.
const OUTCOME: &str = "outcome";

/// The keyword a reference is spelt under, in every dialect this crate emits.
const REFERENCE: &str = "$ref";

/// The pointer prefix [`types`] emits, because the `schema` projection keeps its definitions in
/// `$defs`. No `components.schemas` document uses it, so every one of them is retargeted.
const DEFS: &str = "#/$defs/";

/// Who the specification permits to invoke each command, as [`EssIr::grants`] answers it.
///
/// `may` is declared on the actor, so the question an operation asks — who may invoke *this* command
/// — is the inversion, and the IR is where the inversion lives so that two projections cannot
/// disagree about it.
type Grants<'a> = BTreeMap<&'a CommandHandle, Vec<&'a ResolvedActor>>;

/// an `OpenAPI` document for every command a component accepts.
pub struct OpenApi;

impl Generator for OpenApi {
    fn name(&self) -> &'static str {
        "openapi"
    }

    fn describes(&self) -> &'static str {
        "an OpenAPI document for every command a component accepts"
    }

    fn directory(&self) -> &'static str {
        "openapi"
    }

    fn generate(&self, ir: &EssIr, mint: &ProvenanceMint) -> Vec<Artifact> {
        // `ir.components` is a `BTreeMap`, so this order is the same on every machine. The file name
        // comes from the component's own name rather than its wire name: a wire name is free text
        // and a file name is not, and this path is not something a consumer reads off the wire.
        ir.components
            .values()
            .map(|component| {
                let sliced = component_slice(ir, component, mint);
                Artifact::sliced(
                    format!("{}.yaml", component.name),
                    render(
                        &document(ir, component, &sliced.provenance),
                        &sliced.provenance,
                    ),
                    sliced.slice,
                )
            })
            .collect()
    }
}

/// One component's document as JSON, which is what a server that answers it serves.
///
/// The same document the committed YAML projection carries — same title, same paths, same
/// schemas, same provenance — rendered in the other dialect `OpenAPI` 3.1 permits. Not a second
/// document: `tests/openapi.rs` parses both and compares the values, so a change that reached one
/// and not the other fails rather than shipping a server whose published contract disagrees with
/// the committed one.
///
/// JSON rather than YAML on the wire because every HTTP client parses JSON and `application/yaml`
/// is a media type a caller has to go and find a library for. The provenance is `info`'s
/// `x-ess-provenance`, which survives the crossing where the YAML file's comment header does not.
pub fn json(ir: &EssIr, component: &ResolvedComponent) -> String {
    let mint = ProvenanceMint::new(ir);
    let sliced = component_slice(ir, component, &mint);
    let mut out = serde_json::to_string_pretty(&document(ir, component, &sliced.provenance))
        .unwrap_or_else(|error| panic!("an OpenAPI document serialises as JSON: {error}"));
    out.push('\n');
    out
}

/// The slice one component's document derives from: the component, every actor and every binding.
///
/// The component's own closure brings in what it accepts, owns and publishes — commands with their
/// outcomes, the domains, the input and error payload types. The two flat additions are the
/// constructs this document reads by *inversion*, which no forward walk from the component can
/// reach: grants live on the actors (`ir.grants()` is walked for every operation's security
/// answer), and the `Idempotency-Key` header exists exactly where some binding invokes a command
/// `at_least_once`. Every actor and every binding rather than the currently-relevant ones,
/// because "which ones are relevant" is itself an answer that changes — an actor granted its
/// first accepted command tomorrow must move this digest today.
fn component_slice(
    ir: &EssIr,
    component: &ResolvedComponent,
    mint: &ProvenanceMint,
) -> SlicedProvenance {
    let mut seeds: Vec<EssSemanticRef> = vec![ComponentRef::new(component.name.clone()).into()];
    seeds.extend(
        ir.actors
            .keys()
            .map(|name| ActorRef::new(name.clone()).into()),
    );
    seeds.extend(
        ir.bindings
            .keys()
            .map(|name| BindingRef::new(name.clone()).into()),
    );
    mint.of_seeds(seeds)
}

/// The document as YAML, behind the provenance header.
///
/// YAML rather than JSON so that the provenance can be a comment a person reads at the top of the
/// file, and repeated as `info.x-ess-provenance` so that it survives a tool reparsing the document —
/// a comment is stripped by everything that round-trips YAML, and provenance nobody can read after
/// one round trip is provenance nobody can audit (design §10).
fn render(document: &Document, provenance: &Provenance) -> String {
    let mut out = provenance.commented("#");
    out.push_str(
        &serde_yaml::to_string(document)
            .unwrap_or_else(|error| panic!("an OpenAPI document serialises: {error}")),
    );
    out
}

/// One component's whole surface.
fn document(ir: &EssIr, component: &ResolvedComponent, provenance: &Provenance) -> Document {
    Document {
        openapi: VERSION,
        info: Info {
            title: component
                .naming
                .display
                .clone()
                .unwrap_or_else(|| component.name.to_string()),
            summary: component.naming.summary.clone(),
            description: description(ir, component),
            version: ir.version.to_string(),
            provenance: provenance.clone(),
            reached_by: (component.reached_by == Reach::Network)
                .then(|| component.reached_by.as_str()),
        },
        tags: tags(ir, component),
        paths: paths(ir, component),
        components: Components {
            schemas: schemas(ir, component),
        },
    }
}

/// What a reader of this file needs to know before reading the paths.
///
/// The convention is restated in the artifact and not only in this module's documentation, because
/// the person who has to argue with the mapping is holding the generated file, not the generator.
fn description(ir: &EssIr, component: &ResolvedComponent) -> String {
    let mut text = format!(
        "The HTTP surface of `{}`, one of the components of `{}` {}.\n\n",
        component.name, ir.system, ir.version
    );
    text.push_str(
        "Every path here is one semantic command, so the method is always POST and the path is the \
         command's wire name under its domain's: a command is not a resource, and this document \
         does not invent one. A status code is the outcome the specification declares — 202 for a \
         branch that was taken, 422 for a refusal the input decides, 502 for a refusal decided \
         outside the request — and the `outcome` property of every response body names the branch. \
         Events emitted by a branch are published to consumers through the event transport; they \
         are not returned here.",
    );
    if component.reached_by == Reach::Network {
        text.push_str(
            "\n\nThis component declares `reached_by: network`, so its callers are not deployed \
             with it and this contract is the surface they reach it through. That declaration is \
             also why the views its domains declare have paths here: a GET under `views` per \
             projection, answering every row it holds. There is no page size, no cursor, no \
             ordering and no filter parameter, because the specification states none — a view's \
             filter is part of the projection, not of the request.",
        );
    }
    text
}

/// One tag per domain any accepted command belongs to.
///
/// Keyed by the tag name while collecting, so two domains sharing a wire name produce one tag rather
/// than a document with a duplicate tag.
fn tags(ir: &EssIr, component: &ResolvedComponent) -> Vec<Tag> {
    let mut named: BTreeMap<String, Option<String>> = BTreeMap::new();
    for handle in &component.accepts {
        let domain = ir.domain(&ir.command(handle).domain);
        named.insert(
            domain.naming.wire_or(&domain.name).to_owned(),
            domain.naming.summary.clone(),
        );
    }
    named
        .into_iter()
        .map(|(name, description)| Tag { name, description })
        .collect()
}

/// The path each accepted command is exposed at, and the operation there.
///
/// Two commands can derive the same path — two domains may share a wire name, and `Naming::wire` is
/// free text with no charset the model enforces. When that happens *both* move to their qualified
/// names, rather than one keeping the short path: a path whose meaning depends on which other
/// commands exist is a path that changes when an unrelated command is added.
fn paths(ir: &EssIr, component: &ResolvedComponent) -> BTreeMap<String, PathItem> {
    // Once for the document rather than once per operation: the IR holds `may` on the actor, so the
    // question a path asks — who may invoke *this* command — is an inversion, and inverting it per
    // operation would make the cost quadratic in a specification with many actors.
    let grants = ir.grants();

    let mut out: BTreeMap<String, PathItem> = BTreeMap::new();
    for route in http::routes(ir, component) {
        let item = out.entry(route.path).or_default();
        match route.serves {
            http::Served::Command(handle) => item.post = Some(operation(ir, handle, &grants)),
            http::Served::View(handle) => item.get = Some(query(ir, handle)),
        }
    }
    out
}

/// One command, as an operation.
fn operation(ir: &EssIr, handle: &CommandHandle, grants: &Grants<'_>) -> Operation {
    let command = ir.command(handle);
    let domain = ir.domain(&command.domain);
    Operation {
        id: command.name.to_string(),
        summary: Some(command.naming.display_or(&command.name).to_owned()),
        description: command.naming.summary.clone(),
        tags: vec![domain.naming.wire_or(&domain.name).to_owned()],
        may_invoke: may_invoke(handle, grants),
        consistency: None,
        parameters: idempotency(ir, command).into_iter().collect(),
        request_body: request_body(command),
        responses: responses(command),
    }
}

/// One view, as an operation.
///
/// It exists only because the component declares that something outside the process reaches it —
/// see [`http::routes`]. What it does *not* carry is as deliberate as what it does: no page size,
/// no cursor, no ordering and no filter parameter, because the model states none of them. The
/// view's filter is declared in the specification and is a property of the projection, not of the
/// request, so a caller cannot vary it and the document does not pretend otherwise.
fn query(ir: &EssIr, handle: &ViewHandle) -> Operation {
    let view = ir.view(handle);
    let domain = ir.domain(&view.domain);
    Operation {
        id: view.name.to_string(),
        summary: Some(view.naming.display_or(&view.name).to_owned()),
        description: view.naming.summary.clone(),
        tags: vec![domain.naming.wire_or(&domain.name).to_owned()],
        may_invoke: Vec::new(),
        consistency: Some(view.consistency.as_str()),
        parameters: Vec::new(),
        request_body: None,
        responses: [(
            READ.to_owned(),
            Response {
                description: view_description(ir, view),
                content: Some(content(json!({"$ref": reference(&view_key(view))}))),
            },
        )]
        .into_iter()
        .collect(),
    }
}

/// What the specification says about one view, as the response's required description.
fn view_description(ir: &EssIr, view: &ResolvedView) -> String {
    let mut parts = vec![format!(
        "Every row of `{}`, a projection of `{}`.",
        view.name,
        ir.entity(&view.source).name
    )];
    if let Some(filter) = &view.filter {
        parts.push(format!("Contains the instances where `{filter}` holds."));
    } else {
        parts.push("Contains every instance.".to_owned());
    }
    parts.push(match view.consistency {
        Consistency::ReadYourWrites => {
            "Read-your-writes: a caller that has just issued a command sees its effect here."
                .to_owned()
        }
        Consistency::Eventual => {
            "Eventually consistent: a row may not yet reflect a command that has already returned."
                .to_owned()
        }
    });
    parts.join(" ")
}

/// The actors the specification permits to invoke this command, by qualified name.
///
/// The identity rather than the display name, for the reason every other `x-ess-` field carries one:
/// a display name is free text an author may change without changing who may do what, and a
/// generator reading this document needs the name the rest of the model uses. Empty for a command no
/// actor names — which is a legal specification and not a grant of "anyone", and the difference is
/// why the keyword is then absent rather than an empty list.
fn may_invoke(handle: &CommandHandle, grants: &Grants<'_>) -> Vec<String> {
    grants
        .get(handle)
        .map(|actors| {
            actors
                .iter()
                .map(|actor| actor.name.to_string())
                .collect::<Vec<String>>()
        })
        .unwrap_or_default()
}

/// The `Idempotency-Key` header, when the model says this command may arrive twice.
///
/// The bindings are named in the description on purpose: a required header is an obligation, and the
/// reader's first question is who imposed it.
fn idempotency(ir: &EssIr, command: &ResolvedCommand) -> Option<Parameter> {
    let causes: Vec<String> = ir
        .bindings
        .values()
        .filter(|binding| binding.command.name() == &command.name)
        .filter(|binding| match binding.delivery {
            // Matched rather than defaulted: when the model grows a delivery guarantee that does not
            // permit a repeat, this arm is where the compiler asks about it.
            Delivery::AtLeastOnce => true,
        })
        .map(|binding| format!("`{}`", binding.name))
        .collect();
    if causes.is_empty() {
        return None;
    }
    Some(Parameter {
        name: IDEMPOTENCY_KEY.to_owned(),
        location: "header",
        description: format!(
            "{} {} this command with `delivery: at_least_once`, so it may arrive more than once for \
             one cause. The key names the invocation, so a repeat can be recognised as the same one \
             rather than performed again. It is required rather than optional because a retrying \
             caller could otherwise omit it, and the declared guarantee would have no mechanism \
             behind it.",
            list(&causes),
            if causes.len() == 1 {
                "invokes"
            } else {
                "invoke"
            }
        ),
        required: true,
        schema: written(json!({"type": "string", "minLength": 1})),
    })
}

/// The command's input, as a body.
///
/// A command with no input gets no body at all rather than an empty object: `{}` in a request would
/// be a shape a client has to construct for no reason.
fn request_body(command: &ResolvedCommand) -> Option<RequestBody> {
    if command.input.is_empty() {
        return None;
    }
    Some(RequestBody {
        description: format!("The input `{}` declares.", command.name),
        required: command
            .input
            .iter()
            .any(|field| !field.type_ref.is_optional()),
        content: content(json!({"$ref": reference(&format!("{}.Input", command.name))})),
    })
}

/// One response per status the command's outcomes reach.
fn responses(command: &ResolvedCommand) -> BTreeMap<String, Response> {
    let mut grouped: BTreeMap<&'static str, Vec<&ResolvedOutcome>> = BTreeMap::new();
    for outcome in &command.outcomes {
        grouped.entry(status(outcome)).or_default().push(outcome);
    }

    grouped
        .into_iter()
        .map(|(status, outcomes)| {
            let names: Vec<String> = outcomes
                .iter()
                .map(|outcome| format!("`{}`", outcome.name))
                .collect();
            let schema = if let [only] = outcomes[..] {
                json!({"$ref": reference(&response_key(command, only))})
            } else {
                json!({
                    "oneOf": outcomes
                        .iter()
                        .map(|outcome| json!({"$ref": reference(&response_key(command, outcome))}))
                        .collect::<Vec<Value>>(),
                    // The branch is what a shared status would otherwise lose, so the document says
                    // in machine-readable form which property distinguishes them.
                    "discriminator": {
                        "propertyName": OUTCOME,
                        "mapping": outcomes
                            .iter()
                            .map(|outcome| (
                                outcome.name.to_string(),
                                reference(&response_key(command, outcome)),
                            ))
                            .collect::<BTreeMap<String, String>>(),
                    },
                })
            };
            (
                status.to_owned(),
                Response {
                    description: format!(
                        "{} {}: {}",
                        if names.len() == 1 {
                            "Outcome"
                        } else {
                            "Outcomes"
                        },
                        list(&names),
                        meaning(status)
                    ),
                    content: Some(content(schema)),
                },
            )
        })
        .collect()
}

/// What a status means here, for the response's required description.
fn meaning(status: &str) -> &'static str {
    match status {
        REFUSED => {
            "the request was understood and refused on domain grounds. The body names the declared \
             error and carries whatever that error declares."
        }
        UPSTREAM => {
            "something outside the request refused. The input was acceptable, so the caller has \
             nothing to correct and a retry is meaningful."
        }
        CONFLICT => {
            "the input was acceptable and the subject is in a state this command does not act \
             from. Resending the same request changes nothing until something else moves it."
        }
        _ => {
            "the branch the specification declares for this input. Events this branch emits are \
             published to consumers, not returned here."
        }
    }
}

/// Every schema the document's `$ref`s point at, and nothing else.
///
/// Every shape here comes from [`types`]; what this function decides is which of them the document
/// needs. The one schema shaped by this file is an outcome's response body, because an outcome is
/// not a message the model declares — it is this projection's rendering of a branch.
fn schemas(ir: &EssIr, component: &ResolvedComponent) -> BTreeMap<String, Fragment> {
    let mut out: BTreeMap<String, Fragment> = BTreeMap::new();
    let mut roots: Vec<&TypeHandle> = Vec::new();

    for handle in &component.accepts {
        let command = ir.command(handle);
        if !command.input.is_empty() {
            out.insert(
                format!("{}.Input", command.name),
                embedded(&types::message(&Message::of_command(command))),
            );
        }
        roots.extend(types::field_leaves(&command.input));

        for outcome in &command.outcomes {
            out.insert(
                response_key(command, outcome),
                written(outcome_schema(ir, outcome)),
            );
            if let Some(error) = &outcome.error {
                let declared = ir.error(error);
                if !declared.fields.is_empty() {
                    out.insert(
                        error_key(declared),
                        embedded(&types::message(&Message::of_error(declared))),
                    );
                    roots.extend(types::field_leaves(&declared.fields));
                }
            }
        }
    }

    for route in http::routes(ir, component) {
        let http::Served::View(handle) = route.serves else {
            continue;
        };
        let view = ir.view(handle);
        out.insert(
            row_key(view),
            embedded(&types::message(&Message::of_view(view))),
        );
        out.insert(view_key(view), written(view_schema(view)));
        roots.extend(types::field_leaves(&view.fields));
    }

    for (name, definition) in types::definitions(ir, roots) {
        out.insert(name, embedded(&definition));
    }
    out
}

/// The response body for one view: the rows, under a key.
///
/// An object rather than a bare array, because a bare array is a body with nowhere to put a second
/// fact — and the first one a view will need is how much of the projection this answer is. There is
/// no such fact today and none is invented here; what the shape buys is that adding one later is
/// not a breaking change to every client.
fn view_schema(view: &ResolvedView) -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "description": format!("The rows of `{}`.", view.name),
        "required": ["rows"],
        "properties": {
            "rows": {
                "type": "array",
                "description": "Every row the projection holds, in the order it holds them.",
                "items": {"$ref": reference(&row_key(view))},
            },
        },
    })
}

/// The `components.schemas` key for one view's row.
fn row_key(view: &ResolvedView) -> String {
    format!("{}.Row", view.name)
}

/// The `components.schemas` key for one view's response body.
fn view_key(view: &ResolvedView) -> String {
    format!("{}.Response", view.name)
}

/// The response body for one outcome.
fn outcome_schema(ir: &EssIr, outcome: &ResolvedOutcome) -> Value {
    let mut required = vec![Value::String(OUTCOME.to_owned())];
    let mut properties = Map::new();
    properties.insert(
        OUTCOME.to_owned(),
        json!({
            "const": outcome.name.as_str(),
            "description": "Which declared outcome the command took.",
        }),
    );

    if let Some(handle) = &outcome.error {
        let declared = ir.error(handle);
        required.push(Value::String("error".to_owned()));
        let mut identity = json!({
            "const": declared.name.to_string(),
            "type": "string",
        });
        if let (Some(object), Some(summary)) = (identity.as_object_mut(), &declared.summary) {
            object.insert("description".to_owned(), Value::String(summary.clone()));
        }
        properties.insert("error".to_owned(), identity);

        if !declared.fields.is_empty() {
            required.push(Value::String("payload".to_owned()));
            properties.insert(
                "payload".to_owned(),
                json!({"$ref": reference(&error_key(declared))}),
            );
        }
    }

    json!({
        "type": "object",
        "additionalProperties": false,
        "description": outcome_description(ir, outcome),
        "required": required,
        "properties": properties,
    })
}

/// What the model says about one outcome, as prose.
fn outcome_description(ir: &EssIr, outcome: &ResolvedOutcome) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(summary) = &outcome.summary {
        parts.push(summary.clone());
    }
    parts.push(match &outcome.condition {
        ResolvedCondition::When { predicate } => {
            format!("Taken when `{predicate}` holds of the input.")
        }
        ResolvedCondition::Otherwise => {
            "Taken when no other outcome's condition matched.".to_owned()
        }
        ResolvedCondition::External { cause } => {
            format!("Decided outside the request: {cause}.")
        }
        ResolvedCondition::WrongState => {
            "Taken when the subject is in a state none of this command's declared moves start \
             from. Which states those are is the lifecycle's answer, not this command's."
                .to_owned()
        }
    });
    // What the caller's request did to the system's state, in the response that reports it. A
    // caller reading `202` learns that a branch was taken; without this it does not learn that an
    // invoice now exists, and the specification does say so.
    if let Some(subject) = &outcome.subject {
        let entity = ir.entity(&subject.entity);
        parts.push(match &subject.effect {
            ResolvedEffect::Creates => format!(
                "A `{}` now exists, in `{}`.",
                entity.name, entity.lifecycle.initial
            ),
            ResolvedEffect::Moves { transition } => format!(
                "A `{}` has moved to `{}`, along `{}`.",
                entity.name, transition.to, transition.name
            ),
            ResolvedEffect::Updates => format!(
                "A `{}` has changed, and its state is unchanged.",
                entity.name
            ),
        });
        // Which one. A caller reading "an invoice has moved" and not which invoice has been told
        // half of what the specification says, and the half it is missing is the half it supplied.
        let field = subject.instance.field();
        parts.push(match subject.instance.event() {
            None => format!("The instance is the one `{}` names.", field.name),
            Some(event) => format!(
                "Its identity is published as `{}` on `{}`.",
                field.name,
                ir.event(event).name
            ),
        });
    }
    if !outcome.emits.is_empty() {
        let events: Vec<String> = outcome
            .emits
            .iter()
            .map(|handle| {
                let event = ir.event(handle);
                format!("`{}`", event.naming.wire_or(&event.name))
            })
            .collect();
        parts.push(format!(
            "Emits {}, published to consumers rather than returned here.",
            list(&events)
        ));
    }
    parts.join(" ")
}

/// A schema fragment, as a document carries it.
///
/// A YAML value rather than a `serde_json::Value` because a YAML mapping keeps the order it was
/// built in and `serde_json`'s map is a `BTreeMap`: through JSON, a struct's properties would come
/// out alphabetical, and the one ordering the specification's author expressed would be gone from
/// the published contract. Both are deterministic; only one of them still shows the model.
type Fragment = serde_yaml::Value;

/// The fragment [`types`] publishes for one construct, aimed at this document's own table.
///
/// No prefix, because this table already spells a message's key as `{name}.Input` or `{name}.Error`:
/// a named type can be filed under its bare qualified name without colliding with either, so a
/// pointer to one is the name. `asyncapi` keys its table differently and passes `type.` for that
/// reason.
fn embedded(node: &Node) -> Fragment {
    under_components(node, "")
}

/// A schema this file wrote by hand, as the document carries it.
///
/// An outcome's response body, the `Idempotency-Key`'s value, and the pointers a request body and a
/// response wrap. Every one of them describes something the model has no construct for — a branch
/// rendered as HTTP, a header, a reference — and nothing describing a *model* construct comes through
/// here. Anything that did would be a third copy of the mapping starting.
fn written(schema: Value) -> Fragment {
    serde_yaml::to_value(schema)
        .unwrap_or_else(|error| panic!("a hand-written schema converts to YAML: {error}"))
}

/// One fragment from [`types`], with every pointer retargeted at a `components.schemas` key.
///
/// [`types::pointer`] spells one pointer — `#/$defs/{name}` — because the `schema` projection writes
/// self-contained documents whose definitions live in `$defs`. The same reference is
/// `#/components/schemas/{name}` here and `#/components/schemas/type.{name}` in `asyncapi`, whose
/// table is keyed per kind so that an event and a type sharing a name cannot replace each other.
/// Each spelling is right for the document it appears in, which is why `tests/agreement.rs`
/// normalises a pointer and nothing else.
///
/// Shared with `asyncapi` rather than copied into it, and `pub(crate)` only for that: a second copy
/// of this rewrite would be the same class of defect as the two type mappings this crate has just
/// finished deleting. It belongs beside [`types::pointer`], as that function's own documentation
/// says, and moves there the next time `src/types.rs` is open for editing.
pub(crate) fn under_components(node: &Node, prefix: &str) -> Fragment {
    retargeted(
        serde_yaml::to_value(node)
            .unwrap_or_else(|error| panic!("a schema fragment converts to YAML: {error}")),
        prefix,
    )
}

/// The same fragment with every `$ref` pointing into `components.schemas`.
///
/// Recurses as deep as the fragment nests and does not count. The fragment is a [`Node`] this crate
/// built, so its depth is a small multiple of the type reference it describes — an `Optional` costs
/// an `anyOf`, a `List` an `items` — and a type reference is at most
/// [`MAX_TYPE_DEPTH`](ess_domain::types::MAX_TYPE_DEPTH) deep, refused there rather than here. A
/// named type is a `$ref` into a flat table, not an inlined subtree, so nothing compounds.
fn retargeted(fragment: Fragment, prefix: &str) -> Fragment {
    match fragment {
        Fragment::Mapping(entries) => Fragment::Mapping(
            entries
                .into_iter()
                .map(|(keyword, value)| {
                    let value = if keyword.as_str() == Some(REFERENCE) {
                        Fragment::String(reference(&format!("{prefix}{}", pointed_at(&value))))
                    } else {
                        retargeted(value, prefix)
                    };
                    (keyword, value)
                })
                .collect(),
        ),
        Fragment::Sequence(items) => Fragment::Sequence(
            items
                .into_iter()
                .map(|item| retargeted(item, prefix))
                .collect(),
        ),
        other => other,
    }
}

/// The qualified name a pointer from [`types`] resolves to.
///
/// A panic rather than a pass-through for anything else: a pointer this function does not recognise
/// is a reference into a table this document does not have, and copying it through would publish a
/// document whose own `$ref` resolves to nothing — which parses, validates as a document, and fails
/// the first time somebody validates a payload against it.
fn pointed_at(pointer: &Fragment) -> &str {
    pointer
        .as_str()
        .and_then(|text| text.strip_prefix(DEFS))
        .unwrap_or_else(|| {
            panic!("a `$ref` from the type mapping is `{DEFS}…`, not {pointer:?}");
        })
}

/// The `components.schemas` key for one outcome's response body.
fn response_key(command: &ResolvedCommand, outcome: &ResolvedOutcome) -> String {
    format!("{}.{}.Response", command.name, outcome.name)
}

/// The `components.schemas` key for one error's payload.
fn error_key(declared: &ResolvedError) -> String {
    format!("{}.Error", declared.name)
}

/// A pointer to a schema in this document.
///
/// Always in-document. A qualified name contains neither `/` nor `~`, so no JSON Pointer escaping is
/// needed and the key is the name a reader is looking for.
fn reference(key: &str) -> String {
    format!("#/components/schemas/{key}")
}

/// One media type, holding one schema.
fn content(schema: Value) -> BTreeMap<String, MediaType> {
    let mut out = BTreeMap::new();
    out.insert(
        MEDIA_TYPE.to_owned(),
        MediaType {
            schema: written(schema),
        },
    );
    out
}

/// `a`, `a and b`, `a, b and c`.
fn list(items: &[String]) -> String {
    match items {
        [] => String::new(),
        [only] => only.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

/// An `OpenAPI` 3.1 document.
///
/// Typed rather than a `serde_json::Value` tree so that field order in the emitted YAML is a
/// decision rather than an alphabetical accident: `openapi` belongs at the top of the file, and a
/// reader looking for it should not have to scroll past `components`.
#[derive(Debug, serde::Serialize)]
struct Document {
    openapi: &'static str,
    info: Info,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tags: Vec<Tag>,
    paths: BTreeMap<String, PathItem>,
    components: Components,
}

/// What this document describes.
#[derive(Debug, serde::Serialize)]
struct Info {
    title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    summary: Option<String>,
    description: String,
    /// The specification's version, which is the only version the model has.
    version: String,
    /// Design §10, as structured data rather than only as a comment: a comment is stripped by
    /// everything that round-trips YAML, and it is a tool that asks which model a document came from.
    #[serde(rename = "x-ess-provenance")]
    provenance: Provenance,
    /// Where the specification says this component's callers are.
    ///
    /// Present only where the specification said something — an absent keyword is the model's
    /// silence, and writing `in_process` into every document would put a deployment claim in
    /// thirty-six contracts no author made. Where it says `network`, this document is not
    /// documentation of an internal surface: it is the surface.
    #[serde(rename = "x-ess-reached-by", skip_serializing_if = "Option::is_none")]
    reached_by: Option<&'static str>,
}

/// One domain, as a grouping.
#[derive(Debug, serde::Serialize)]
struct Tag {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
}

/// One path, and the one method it answers.
///
/// Both fields are optional and exactly one is ever set, because a path is one construct: a
/// command's path is a `POST` and a view's is a `GET`, and the two cannot collide — the segment
/// between the domain and the name is `commands` for one and `views` for the other. Two fields
/// rather than an enum so that the emitted YAML keeps `OpenAPI`'s own spelling, where the method is
/// the key.
#[derive(Debug, Default, serde::Serialize)]
struct PathItem {
    #[serde(skip_serializing_if = "Option::is_none")]
    get: Option<Operation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    post: Option<Operation>,
}

/// One command, exposed.
#[derive(Debug, serde::Serialize)]
struct Operation {
    #[serde(rename = "operationId")]
    id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tags: Vec<String>,
    /// The actors the specification permits to invoke this command.
    ///
    /// An annotation, not a requirement: `security` would claim an authentication mechanism the
    /// model does not state, and this says only what the specification says — who may, not how they
    /// prove it. See the module documentation's "What this refuses to guess".
    #[serde(rename = "x-ess-may-invoke", skip_serializing_if = "Vec::is_empty")]
    may_invoke: Vec<String>,
    /// How soon a view reflects a command that has already returned.
    ///
    /// An annotation and not a header, because it is a property of the projection rather than of
    /// this request: `eventual` says a caller may read a row that does not yet reflect its own
    /// write, and `read_your_writes` says it may not. A `Cache-Control` here would claim a caching
    /// policy the model does not state. Absent on a command, which projects nothing.
    #[serde(rename = "x-ess-consistency", skip_serializing_if = "Option::is_none")]
    consistency: Option<&'static str>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    parameters: Vec<Parameter>,
    #[serde(rename = "requestBody", skip_serializing_if = "Option::is_none")]
    request_body: Option<RequestBody>,
    /// Keyed by status as text, which sorts numerically while every status is three digits.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    responses: BTreeMap<String, Response>,
}

/// A header, in this crate's case always a header.
#[derive(Debug, serde::Serialize)]
struct Parameter {
    name: String,
    #[serde(rename = "in")]
    location: &'static str,
    description: String,
    required: bool,
    schema: Fragment,
}

/// The command's input.
#[derive(Debug, serde::Serialize)]
struct RequestBody {
    description: String,
    required: bool,
    content: BTreeMap<String, MediaType>,
}

/// One outcome, or a `oneOf` of the outcomes sharing a status.
#[derive(Debug, serde::Serialize)]
struct Response {
    description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<BTreeMap<String, MediaType>>,
}

/// One encoding of one schema.
#[derive(Debug, serde::Serialize)]
struct MediaType {
    schema: Fragment,
}

/// Everything the document's `$ref`s resolve against.
#[derive(Debug, serde::Serialize)]
struct Components {
    schemas: BTreeMap<String, Fragment>,
}
