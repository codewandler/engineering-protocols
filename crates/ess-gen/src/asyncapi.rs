//! `AsyncAPI` 3.0, one document per component.
//!
//! # An event is a fact; a channel is one way to carry it
//!
//! The model holds no transport construct. Design §7 sketches one — `transport: {kind: async,
//! implementation: kafka}` — and it is not built, and `ess-domain` deliberately keeps a topic out of
//! an event (`command.rs`'s `an_event_records_a_fact_and_nothing_about_how_it_travels`, which asserts
//! that the topic "lives in naming, where changing it is a wire change and not a model change").
//!
//! So this projection does not know what broker anything runs on, and does not guess. What it knows
//! is a *name*, taken from the model, and a channel is where a name of that kind belongs. Everything
//! a broker would decide — protocol bindings, servers, security, message keys, partitioning,
//! retention, ordering — is absent from the output because it is absent from the model. An empty
//! `servers` is not an oversight; it is the honest projection of a specification that has not said.
//!
//! ## The channel address
//!
//! | the event | its channel address | why |
//! |---|---|---|
//! | declares `naming.wire` | that string, verbatim | the wire name *is* the topic; it is what F5 split it out for |
//! | declares no `naming.wire` | its qualified name, verbatim — `billing.invoice.InvoiceCreated` | a channel address is global, and this is the only globally unique name the model has |
//!
//! The fallback is deliberately **not** [`Naming::wire_or`](ess_domain::name::Naming::wire_or), which
//! falls back to the last segment. That fallback is right where something around the name
//! disambiguates it — a JSON field inside a struct, a path segment under a resource. A topic has
//! nothing around it, so two contexts each declaring a `Created` would collide, and the second
//! channel would quietly replace the first. A reviewer who would rather derive
//! `invoices.InvoiceCreated` from the domain's wire name and the event's own is disagreeing with this
//! row, which is the point of writing it down.
//!
//! No version appears in an address. The model versions the *system*, not the event, so a version in
//! a topic is something an author writes into `naming.wire` — §6's `invoices.created.v1` is an
//! authored name, not a derived one.
//!
//! Every channel carries `x-ess-address-source`, so a reader can tell an address somebody chose from
//! one this generator derived.
//!
//! # `delivery` and `on_failure` do not get lost here
//!
//! Review F3 made both required words, and `Failure::Drop` in particular a word an author has to
//! type. `AsyncAPI` has no field for either, so they travel as extensions — and, because an extension
//! is easy to skim past, also as prose:
//!
//! | where | what a reader finds |
//! |---|---|
//! | `operations.receive.<event>.x-ess-reactions[]` | `delivery`, `on_failure`, the event an `escalate` emits, and a sentence for each saying what it means for the handler |
//! | `operations.receive.<event>.description` | the same facts in prose, one paragraph per binding |
//! | `operations.send.<event>.x-ess-consumed-by[]` | `delivery`, `on_failure` and the escalation event again, in the *publisher's* document |
//!
//! `escalates_with` is on both sides for the same reason `on_failure` is: an escalation is a fact
//! this system publishes, and a consumer of the publisher's document that could not see which event
//! carries it would have to be told out of band — which is the thing a specification exists to stop.
//!
//! The third row is the one that matters for `drop`: the work being abandoned is the publisher's
//! event, so the publisher's document has to be able to say so. `handled_by: null` there is not a
//! bug — it means no component in this specification accepts the command the binding invokes, and a
//! binding whose failure policy applies to nobody is worth seeing.
//!
//! # The mapping is in the document, under `x-ess-`
//!
//! A binding's mapping is system semantics and a channel is transport, so the mapping is kept
//! strictly out of `channels` and `components.messages` — those describe bytes on a wire and nothing
//! else. It is still *in* the document, on the receive operation, because "at least once, so be
//! idempotent" is not actionable without knowing which command runs, and because
//! [`ResolvedMapping::conversion`](ess_compiler::ir::ResolvedMapping::conversion) exists precisely so
//! that a generator emitting a mapping emits the justification for its type crossing too. Dropping
//! the mapping would leave a document that says a component receives an event and never says what
//! the system does with it — the same half-a-system failure as omitting the subscriber side.
//!
//! Mapping targets are rendered as type *names*, never as `$ref`s. A command's input is not a
//! message on this channel, and a `$ref` to a schema for it would claim otherwise.
//!
//! # Payload schemas are inlined, and the mapping is not this file's
//!
//! Every `$ref` resolves inside the document it appears in, so the payload schemas live in
//! `components.schemas` rather than pointing at the JSON Schema projection: a document that only
//! validates when its sibling files are on disk is a document that does not validate in the field.
//!
//! What those schemas *say* is not decided here. It comes from `schema::types`, the
//! one type mapping this crate has, through `openapi::under_components`, which retargets its pointers at
//! this document's own table. This file used to carry a copy of that mapping, and the copy disagreed
//! with the `schema` projection about the same event: a service validating an `InvoiceCreated`
//! against *this* document accepted `{"amount": "abc", "currency": "EUR", "bogus": 1}`, which the
//! JSON Schema published for the same event refuses on all three counts. Neither mapping was buggy.
//! There were two of them. `tests/agreement.rs` is what keeps there being one.
//!
//! Schema keys are `event.<qualified name>` and `type.<qualified name>`. The model's names are unique
//! per kind and not across kinds, so one flat table keyed by qualified name could let an event's
//! payload and a type of the same name silently replace each other. That prefix is why a pointer here
//! is spelt differently from the same pointer under `openapi/`, and it is the only difference
//! `tests/agreement.rs` normalises rather than reporting.
//!
//! ## Four readings this file no longer makes
//!
//! Each was a place where the copy here published a **weaker** contract than the specification
//! states, and every one of them went the same way for one reason: *an extension is a note, and a
//! keyword is an assertion*. Every conforming validator ignores `x-ess-*` — by default and by
//! specification — so answering with an extension where the mapping answers with a keyword publishes
//! a document that refuses less than the model does, which is the one job a published contract has.
//! `src/types.rs` carries the argument for each; this table is what changed here.
//!
//! | it published | it publishes now |
//! |---|---|
//! | `Decimal`, `Duration` and `Bytes` as a bare `string` plus `x-ess-type` | `format: decimal` with the digit pattern, `format: duration`, and `contentEncoding: base64` with the base64 pattern |
//! | a struct with no `additionalProperties` — "an evolution policy the model has not stated" | `additionalProperties: false`, because the same event cannot be closed in one published file and open in another |
//! | `anyOf` over a union's bare variants, so the tag appeared nowhere | `oneOf` of adjacently tagged branches, each pinning its tag with a `const` |
//! | `x-ess-optional` on an `Optional` outside a field | `anyOf: [T, {type: null}]`, because a list element has no key to leave out |
//!
//! The cost is stated rather than hidden: three of those four claim something the model does not
//! state — a union's layout, `null` as the spelling of an absent list element, and a grammar for a
//! `Duration` — and `src/types.rs` is where a reviewer who wants a different claim changes it for all
//! three projections at once. What none of them does any more is claim *less* than the model.
//!
//! ## An annotation is a fact too
//!
//! `title`, `description` and the `x-ess-*` keywords change nothing about which bytes a document
//! accepts, and they are still not this file's to choose. They are facts the *model* states — what a
//! construct is called, what its author wrote about it, which invariants it satisfies — and a fact
//! with two spellings is a fact every consumer has to reconcile. This projection used to answer
//! "which construct is this" with `x-ess-type` where `schema` answered with `x-ess-name` and
//! `x-ess-kind`: one code generator reading both documents got two answers about one construct. So
//! the annotations come from `types` along with everything else, and `tests/agreement.rs` compares
//! them across projections exactly as strictly as it compares the assertions. What is per-document is
//! the furniture *around* a schema — a channel, an operation, a message, a title on one of those —
//! which is what this file is for.
//!
//! # Determinism
//!
//! Ordering comes from the IR — `BTreeMap`, `BTreeSet`, declaration-order `Vec` — and never from a
//! hash. The ordered table this file serialises maps through keeps insertion order, so that a
//! struct's properties read in the order their author declared them rather than alphabetically; the
//! order is still a function of the IR alone. That is also why a payload is carried as a
//! `serde_yaml::Value` rather than a `serde_json::Value`: `serde_json`'s map is a `BTreeMap`, so the
//! round trip would sort a struct's properties and throw away the one ordering its author expressed.

use std::collections::BTreeMap;

use ess_compiler::ir::{
    ResolvedBinding, ResolvedComponent, ResolvedEffect, ResolvedEvent, ResolvedFailure,
    ResolvedMapping, ResolvedMappingValue, TypeHandle,
};
use ess_compiler::EssIr;
use ess_domain::binding::{Delivery, Failure};
use ess_domain::name::QualifiedName;

use crate::artifact::{Artifact, Generator};
use crate::openapi::under_components;
use ess_compiler::refs::{BindingRef, CommandRef, ComponentRef, EssSemanticRef};

use crate::provenance::{Provenance, ProvenanceMint, SlicedProvenance};
use crate::schema::types::{self, Node};

/// The `AsyncAPI` version every document declares.
const ASYNCAPI_VERSION: &str = "3.0.0";

/// The content type every message carries.
///
/// Not a guess about a broker: payloads are projected as JSON Schema, so the payloads are JSON. What
/// the model has not said is which *transport* carries them, and that stays unsaid.
const CONTENT_TYPE: &str = "application/json";

/// What an event payload's schema key starts with.
///
/// Keyed per kind because the model's names are unique per kind and not across kinds: one flat table
/// would let an event's payload and a type of the same name replace each other silently.
const EVENT_KEY: &str = "event.";

/// What a named type's schema key starts with, and therefore what a pointer to one carries.
const TYPE_KEY: &str = "type.";

/// One `AsyncAPI` 3.0 document per component: what it publishes, and what it reacts to.
pub struct AsyncApi;

impl Generator for AsyncApi {
    fn name(&self) -> &'static str {
        "asyncapi"
    }

    fn describes(&self) -> &'static str {
        "an AsyncAPI 3.0 document per component, covering what it publishes and what it reacts to"
    }

    fn directory(&self) -> &'static str {
        "asyncapi"
    }

    fn generate(&self, ir: &EssIr, mint: &ProvenanceMint) -> Vec<Artifact> {
        ir.components
            .values()
            .map(|component| {
                let sliced = component_slice(ir, component, mint);
                let document = document(ir, component, &sliced.provenance);
                let body = serde_yaml::to_string(&document)
                    .unwrap_or_else(|error| panic!("an AsyncAPI document serialises: {error}"));
                Artifact::sliced(
                    format!("{}.yaml", component.name),
                    format!("{}{body}", sliced.provenance.commented("#")),
                    sliced.slice,
                )
            })
            .collect()
    }
}

/// A map that keeps the order it was built in.
///
/// A `BTreeMap` would sort a struct's properties alphabetically, which is deterministic and also
/// throws away the one ordering the author expressed. This keeps the IR's order and is deterministic
/// for the same reason: the order is a function of the IR, not of a hash.
struct Table<T>(Vec<(String, T)>);

impl<T> Table<T> {
    /// An empty table.
    fn new() -> Self {
        Self(Vec::new())
    }

    /// Appends an entry.
    fn push(&mut self, key: impl Into<String>, value: T) {
        self.0.push((key.into(), value));
    }
}

impl<T: serde::Serialize> serde::Serialize for Table<T> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap as _;

        let mut map = serializer.serialize_map(Some(self.0.len()))?;
        for (key, value) in &self.0 {
            map.serialize_entry(key, value)?;
        }
        map.end()
    }
}

/// One component's `AsyncAPI` document.
#[derive(serde::Serialize)]
struct Document {
    asyncapi: &'static str,
    info: Info,
    #[serde(rename = "defaultContentType")]
    default_content_type: &'static str,
    channels: Table<Channel>,
    operations: Table<Operation>,
    components: Components,
}

/// Who this document is about, and where it came from.
#[derive(serde::Serialize)]
struct Info {
    title: String,
    version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(rename = "x-ess-component")]
    component: String,
    #[serde(rename = "x-ess-provenance")]
    provenance: Provenance,
}

/// A `$ref` into this same document.
///
/// Always internal. Qualified-name segments are `[A-Za-z][A-Za-z0-9_-]*` joined by dots, so no
/// pointer token here can contain the `/` or `~` that would need escaping.
#[derive(serde::Serialize)]
struct Reference {
    #[serde(rename = "$ref")]
    reference: String,
}

impl Reference {
    /// A reference to `pointer`, which is already a whole JSON pointer.
    fn to(pointer: impl Into<String>) -> Self {
        Self {
            reference: pointer.into(),
        }
    }
}

/// Where one event travels.
#[derive(serde::Serialize)]
struct Channel {
    address: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    summary: Option<String>,
    messages: Table<Reference>,
    #[serde(rename = "x-ess-event")]
    event: String,
    #[serde(rename = "x-ess-address-source")]
    address_source: &'static str,
}

/// Something this component does with a channel: `send` it, or `receive` from it.
#[derive(serde::Serialize)]
struct Operation {
    action: &'static str,
    channel: Reference,
    title: String,
    summary: String,
    description: String,
    messages: Vec<Reference>,
    /// Only on a `receive`: what this component does because the event arrived.
    #[serde(rename = "x-ess-reactions", skip_serializing_if = "Vec::is_empty")]
    reactions: Vec<Reaction>,
    /// Only on a `send`: who reacts to this event, and under what failure policy.
    #[serde(rename = "x-ess-consumed-by", skip_serializing_if = "Vec::is_empty")]
    consumed_by: Vec<Consumer>,
}

/// A binding, from the side that handles it.
#[derive(serde::Serialize)]
struct Reaction {
    binding: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    summary: Option<String>,
    invokes: String,
    delivery: Delivery,
    /// What the delivery guarantee obliges the handler to be.
    delivery_means: &'static str,
    /// The word an author wrote, spelt as they wrote it.
    on_failure: Failure,
    /// The event an `escalate` publishes, so a handler knows what it owes the rest of the system.
    ///
    /// `None` for `retry` and `drop`, which publish nothing — a retry because it is already
    /// observable as another invocation, a drop because being unobservable is the whole word.
    #[serde(skip_serializing_if = "Option::is_none")]
    escalates_with: Option<String>,
    /// What that word costs, in a sentence.
    on_failure_means: String,
    mapping: Vec<MappedInput>,
}

/// A binding, from the side that published the event.
///
/// `handled_by` is `null` when no component accepts the invoked command. That is a legal model — §5
/// makes decomposition partial — and it is also the one shape in which a binding's failure policy
/// would otherwise appear in no document at all.
#[derive(serde::Serialize)]
struct Consumer {
    binding: String,
    handled_by: Option<String>,
    invokes: String,
    delivery: Delivery,
    on_failure: Failure,
    /// The event an `escalate` publishes.
    #[serde(skip_serializing_if = "Option::is_none")]
    escalates_with: Option<String>,
}

/// One filled command input.
#[derive(serde::Serialize)]
struct MappedInput {
    target: String,
    #[serde(rename = "type")]
    target_type: String,
    source: MappedSource,
    /// The declared reason two different types are allowed to meet.
    #[serde(skip_serializing_if = "Option::is_none")]
    conversion: Option<String>,
}

/// Where a mapped value comes from.
#[derive(serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum MappedSource {
    /// A field of the event that arrived.
    EventField {
        field: String,
        #[serde(rename = "type")]
        type_ref: String,
    },
    /// A value written into the binding, whose type the compiler took on trust.
    Literal { value: String },
}

/// The document's reusable halves.
#[derive(serde::Serialize)]
struct Components {
    messages: Table<Message>,
    schemas: Table<Fragment>,
}

/// One event, as something that arrives.
#[derive(serde::Serialize)]
struct Message {
    /// The event's qualified name: the identity, not the wire name.
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    summary: Option<String>,
    #[serde(rename = "contentType")]
    content_type: &'static str,
    payload: Reference,
}

/// One payload schema, as this document carries it.
///
/// [`types`] builds it and [`under_components`] retargets its pointers; nothing here shapes one. The
/// struct this file used to hold instead — one field per keyword this projection "commits to" — is
/// exactly how the drift happened: a keyword the mapping emits and this struct had no field for was
/// unrepresentable, so it was silently dropped from the published contract rather than failing
/// anything.
type Fragment = serde_yaml::Value;

/// One channel's worth of reasons to exist.
struct Plan<'a> {
    event: &'a ResolvedEvent,
    /// This component declares the event.
    publishes: bool,
    /// The bindings this component handles because the event arrived.
    handles: Vec<&'a ResolvedBinding>,
}

/// Every event that causes a command, keyed by the event's identity.
///
/// The slice one component's document derives from: the component, every binding, every command
/// and every component.
///
/// Wider than `OpenAPI`'s on purpose, because this document reads across the whole interaction
/// layer by inversion: the channels come from `ir.reactions()` (every binding), a send operation
/// names the components that consume the event (every component), and the state-change notes scan
/// every command's outcomes for the ones that emit the message — none of which a forward walk
/// from this component alone can reach. The commands' closure brings the entities and the
/// transitions the notes name. What is deliberately *not* here: views and actors, which nothing
/// in this document reads — those are the narrowing this slice still buys.
fn component_slice(
    ir: &EssIr,
    component: &ResolvedComponent,
    mint: &ProvenanceMint,
) -> SlicedProvenance {
    let mut seeds: Vec<EssSemanticRef> = vec![ComponentRef::new(component.name.clone()).into()];
    seeds.extend(
        ir.bindings
            .keys()
            .map(|name| BindingRef::new(name.clone()).into()),
    );
    seeds.extend(
        ir.commands
            .keys()
            .map(|name| CommandRef::new(name.clone()).into()),
    );
    seeds.extend(
        ir.components
            .keys()
            .map(|name| ComponentRef::new(name.clone()).into()),
    );
    mint.of_seeds(seeds)
}

/// [`EssIr::reactions`] keyed by handle, re-keyed by name: a handle has no public constructor, so a
/// map keyed by one can only be searched linearly from a projection, and this is asked per event.
type Reactions<'a> = BTreeMap<&'a QualifiedName, Vec<&'a ResolvedBinding>>;

/// Builds one component's document.
fn document(ir: &EssIr, component: &ResolvedComponent, provenance: &Provenance) -> Document {
    let reactions: Reactions<'_> = ir
        .reactions()
        .into_iter()
        .map(|(handle, bindings)| (handle.name(), bindings))
        .collect();
    let plans = plans(ir, component, &reactions);
    let mut channels = Table::new();
    let mut operations = Table::new();
    let mut messages = Table::new();

    for plan in &plans {
        let event = plan.event;
        let identity = event.name.to_string();
        channels.push(identity.clone(), channel(event));
        messages.push(identity.clone(), message(event));
        if plan.publishes {
            operations.push(format!("send.{identity}"), send(ir, event, &reactions));
        }
        if !plan.handles.is_empty() {
            operations.push(format!("receive.{identity}"), receive(ir, component, plan));
        }
    }

    Document {
        asyncapi: ASYNCAPI_VERSION,
        info: Info {
            title: component_title(component).to_owned(),
            version: ir.version.to_string(),
            description: Some(describe(ir, component)),
            component: component.name.to_string(),
            provenance: provenance.clone(),
        },
        default_content_type: CONTENT_TYPE,
        channels,
        operations,
        components: Components {
            messages,
            schemas: schemas(ir, &plans),
        },
    }
}

/// Every event this component has a reason to name, in qualified-name order.
///
/// Published and received in one pass: an event a component both publishes and reacts to is one
/// channel with two operations, and computing the two sets separately would give it two channels.
fn plans<'a>(
    ir: &'a EssIr,
    component: &ResolvedComponent,
    reactions: &Reactions<'a>,
) -> Vec<Plan<'a>> {
    let mut by_event: BTreeMap<&'a QualifiedName, Plan<'a>> = BTreeMap::new();
    for handle in &component.publishes {
        let event = ir.event(handle);
        by_event.insert(
            &event.name,
            Plan {
                event,
                publishes: true,
                handles: Vec::new(),
            },
        );
    }
    for bindings in reactions.values() {
        let handled: Vec<&'a ResolvedBinding> = bindings
            .iter()
            .copied()
            .filter(|binding| component.accepts.contains(&binding.command))
            .collect();
        let Some(first) = handled.first() else {
            continue;
        };
        // Through the binding's own handle rather than by name: the lookup stays total, which is
        // what `ess-compiler` minted the handle for.
        let event = ir.event(&first.event);
        by_event
            .entry(&event.name)
            .or_insert(Plan {
                event,
                publishes: false,
                handles: Vec::new(),
            })
            .handles = handled;
    }
    by_event.into_values().collect()
}

/// The component's display name, falling back to the name a workload would be called.
fn component_title(component: &ResolvedComponent) -> &str {
    component
        .naming
        .display
        .as_deref()
        .unwrap_or_else(|| component.name.as_str())
}

/// What the document says about itself.
///
/// The transport caveat lives here, once, rather than on every operation: it is a fact about the
/// specification, and repeating it beside each channel would train a reader to skip it.
fn describe(ir: &EssIr, component: &ResolvedComponent) -> String {
    let mut parts = vec![format!(
        "`{}` in the `{}` specification.",
        component.name, ir.system
    )];
    if let Some(summary) = component.naming.summary.as_deref() {
        parts.push(summary.to_owned());
    }
    if let Some(summary) = ir.summary.as_deref() {
        parts.push(summary.to_owned());
    }
    parts.push(
        "The specification declares no transport, so each address below is a name and not a topic \
         on a named broker. Servers, protocol bindings, security schemes, message keys, \
         partitioning, retention and ordering are absent because the model does not state them."
            .to_owned(),
    );
    parts.join("\n\n")
}

/// The channel one event travels on.
fn channel(event: &ResolvedEvent) -> Channel {
    let (address, address_source) = address(event);
    let identity = event.name.to_string();
    let mut messages = Table::new();
    messages.push(
        identity.clone(),
        Reference::to(format!("#/components/messages/{identity}")),
    );
    Channel {
        address,
        title: Some(display_of(event).to_owned()),
        summary: event.naming.summary.clone(),
        messages,
        event: identity,
        address_source,
    }
}

/// An event's channel address, and where it came from.
///
/// The two cases are the module doc's table. The fallback is the qualified name rather than the last
/// segment because an address has no surrounding context to disambiguate it.
fn address(event: &ResolvedEvent) -> (String, &'static str) {
    match event.naming.wire.as_deref() {
        Some(wire) => (wire.to_owned(), "naming.wire"),
        None => (event.name.to_string(), "qualified-name"),
    }
}

/// The event as something that arrives.
fn message(event: &ResolvedEvent) -> Message {
    Message {
        name: event.name.to_string(),
        title: Some(display_of(event).to_owned()),
        summary: event.naming.summary.clone(),
        content_type: CONTENT_TYPE,
        payload: Reference::to(format!("#/components/schemas/{EVENT_KEY}{}", event.name)),
    }
}

/// What a person is shown for an event.
fn display_of(event: &ResolvedEvent) -> &str {
    event.naming.display_or(&event.name)
}

/// The operation for publishing an event.
///
/// It carries the consumers because a publisher that cannot see who depends on its event cannot know
/// that reshaping it is a breaking change — design §12 pairs publisher with consumer by the event's
/// identity, and this is that pairing, from the publisher's side.
fn send(ir: &EssIr, event: &ResolvedEvent, reactions: &Reactions<'_>) -> Operation {
    let identity = event.name.to_string();
    let consumed_by: Vec<Consumer> = reactions
        .get(&event.name)
        .map(|bindings| {
            bindings
                .iter()
                .map(|binding| consumer(ir, binding))
                .collect()
        })
        .unwrap_or_default();
    let mut description = if consumed_by.is_empty() {
        format!("Nothing in this specification reacts to `{identity}`.")
    } else {
        consumed_by
            .iter()
            .map(|consumer| {
                format!(
                    "`{}` reacts by invoking `{}`. On failure: `{}`.",
                    consumer.binding,
                    consumer.invokes,
                    word_for(consumer.on_failure)
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    for change in state_changes(ir, event) {
        description.push('\n');
        description.push_str(&change);
    }
    Operation {
        action: "send",
        channel: Reference::to(format!("#/channels/{identity}")),
        title: format!("Publish {}", display_of(event)),
        summary: format!("Publishes `{identity}`."),
        description,
        messages: vec![Reference::to(format!(
            "#/channels/{identity}/messages/{identity}"
        ))],
        reactions: Vec::new(),
        consumed_by,
    }
}

/// What the outcomes that publish this event did to an entity, one line each.
///
/// A subscriber's real question about `InvoiceCreated` is not only what the payload holds but what is
/// now true of the system: an invoice exists, and it is in `Draft`. The model states it — on the
/// outcome that emits the event — and a document that carried the payload and dropped the state
/// change would leave every consumer to rediscover it from a name.
///
/// Silent when no publishing outcome changes an entity: `billing.email.EmailSent` reports something
/// that happened outside, and inventing a sentence for it would be this projection speaking for the
/// model.
fn state_changes(ir: &EssIr, event: &ResolvedEvent) -> Vec<String> {
    let mut out = Vec::new();
    for command in ir.commands.values() {
        for outcome in &command.outcomes {
            if !outcome.emits.iter().any(|it| it.name() == &event.name) {
                continue;
            }
            let Some(subject) = &outcome.subject else {
                continue;
            };
            let entity = ir.entity(&subject.entity);
            // Only where *this* message is the one the model says publishes the identity. A branch
            // may emit several events and the link names one of them; claiming it of the others
            // would be this projection speaking for the model.
            let publishes = subject
                .instance
                .event()
                .filter(|handle| handle.name() == &event.name)
                .map(|_| {
                    format!(
                        " It carries the new instance's identity in `{}`.",
                        subject.instance.field().name
                    )
                })
                .unwrap_or_default();
            out.push(match &subject.effect {
                ResolvedEffect::Creates => format!(
                    "`{}` emits it on `{}`, which creates a `{}` in `{}`.{publishes}",
                    command.name, outcome.name, entity.name, entity.lifecycle.initial
                ),
                ResolvedEffect::Moves { transition } => format!(
                    "`{}` emits it on `{}`, which moves a `{}` to `{}` along `{}`.",
                    command.name, outcome.name, entity.name, transition.to, transition.name
                ),
                ResolvedEffect::Updates => format!(
                    "`{}` emits it on `{}`, which changes a `{}` without moving it.",
                    command.name, outcome.name, entity.name
                ),
            });
        }
    }
    out
}

/// The operation for reacting to an event.
fn receive(ir: &EssIr, component: &ResolvedComponent, plan: &Plan<'_>) -> Operation {
    let identity = plan.event.name.to_string();
    let reactions: Vec<Reaction> = plan
        .handles
        .iter()
        .map(|binding| reaction(ir, binding))
        .collect();
    let description = reactions
        .iter()
        .map(|reaction| {
            format!(
                "`{}` receives `{identity}` and, under binding `{}`, invokes `{}`.\nDelivery is \
                 {}: {}.\nOn failure the system will {}: {}.",
                component.name,
                reaction.binding,
                reaction.invokes,
                word_for(reaction.delivery),
                reaction.delivery_means,
                word_for(reaction.on_failure),
                reaction.on_failure_means,
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n");
    Operation {
        action: "receive",
        channel: Reference::to(format!("#/channels/{identity}")),
        title: format!("React to {}", display_of(plan.event)),
        summary: format!("Receives `{identity}` and invokes a command."),
        description,
        messages: vec![Reference::to(format!(
            "#/channels/{identity}/messages/{identity}"
        ))],
        reactions,
        consumed_by: Vec::new(),
    }
}

/// A binding as the handling component sees it.
fn reaction(ir: &EssIr, binding: &ResolvedBinding) -> Reaction {
    Reaction {
        binding: binding.name.to_string(),
        summary: binding.naming.summary.clone(),
        invokes: ir.command(&binding.command).name.to_string(),
        delivery: binding.delivery,
        delivery_means: delivery_means(binding.delivery),
        on_failure: binding.failure,
        escalates_with: escalates_with(ir, binding),
        on_failure_means: failure_means(ir, binding),
        mapping: binding.mapping.iter().map(mapped_input).collect(),
    }
}

/// A binding as the publishing component sees it.
fn consumer(ir: &EssIr, binding: &ResolvedBinding) -> Consumer {
    // One acceptor, ordinarily: `validate_components` refuses a component accepting a command whose
    // domain another component owns, and a domain has one owner. Two acceptors survive only where
    // nothing owns the domain at all, which §5 permits while decomposition is partial, and the first
    // by name is then taken so the projection is at least deterministic about which it names.
    let handled_by = ir
        .components
        .values()
        .find(|component| component.accepts.contains(&binding.command))
        .map(|component| component.name.to_string());
    Consumer {
        binding: binding.name.to_string(),
        handled_by,
        invokes: ir.command(&binding.command).name.to_string(),
        delivery: binding.delivery,
        on_failure: binding.failure,
        escalates_with: escalates_with(ir, binding),
    }
}

/// The event a binding's escalation publishes, when it escalates.
fn escalates_with(ir: &EssIr, binding: &ResolvedBinding) -> Option<String> {
    match binding.on_failure() {
        ResolvedFailure::Escalate { emits } => Some(ir.event(emits).name.to_string()),
        ResolvedFailure::Retry | ResolvedFailure::Drop => None,
    }
}

/// One entry of a binding's mapping.
fn mapped_input(mapping: &ResolvedMapping) -> MappedInput {
    MappedInput {
        target: mapping.target.clone(),
        target_type: mapping.target_type.to_string(),
        source: match &mapping.value {
            ResolvedMappingValue::EventField { field, type_ref } => MappedSource::EventField {
                field: field.clone(),
                type_ref: type_ref.to_string(),
            },
            ResolvedMappingValue::Literal { value } => MappedSource::Literal {
                value: value.clone(),
            },
        },
        conversion: mapping.conversion.clone(),
    }
}

/// What a delivery guarantee obliges of the handler.
fn delivery_means(delivery: Delivery) -> &'static str {
    match delivery {
        Delivery::AtLeastOnce => {
            "the command may run more than once for a single event, so its handler must be \
             idempotent"
        }
    }
}

/// What a failure policy costs, in one sentence.
///
/// `Drop` gets the longest one on purpose. It is the word review F3 insisted an author has to type,
/// and a projection in which it reads like the other two would have undone that.
///
/// `Escalate` used to end "the specification does not say through what, so an implementation has to
/// choose", which was true and was the defect: it named an effect and left nothing to check. It now
/// names the event, because there now is one.
fn failure_means(ir: &EssIr, binding: &ResolvedBinding) -> String {
    match binding.on_failure() {
        ResolvedFailure::Retry => "the invocation is retried on whatever schedule the transport \
             provides; the specification does not say how often, for how long, or where it goes if \
             the retries run out"
            .to_owned(),
        ResolvedFailure::Escalate { emits } => format!(
            "the failure is surfaced to a person, and `{}` is published so that the escalation is \
             observable from inside the system; the specification does not say what surfaces it to \
             whom, so an implementation chooses that and not whether to say it happened",
            ir.event(emits).name
        ),
        ResolvedFailure::Drop => "the work is abandoned — the command does not run, nothing \
             retries it, and nobody is told, so the event's effect is lost and this specification \
             says that is acceptable"
            .to_owned(),
    }
}

/// The word an author wrote, for use in prose.
///
/// Via the enum's own serialisation, so the sentence and the extension can never spell it
/// differently.
fn word_for(value: impl serde::Serialize) -> String {
    serde_yaml::to_string(&value)
        .unwrap_or_default()
        .trim()
        .to_owned()
}

/// Every schema the document's messages reach, payloads first, then the named types they use.
///
/// The reach comes from [`types`] too, so that a type this document references and does not define is
/// not a thing this file can get wrong on its own.
fn schemas<'a>(ir: &'a EssIr, plans: &[Plan<'a>]) -> Table<Fragment> {
    let mut roots: Vec<&'a TypeHandle> = Vec::new();
    let mut out = Table::new();
    for plan in plans {
        roots.extend(types::field_leaves(&plan.event.fields));
        out.push(
            format!("{EVENT_KEY}{}", plan.event.name),
            fragment(&types::message(&types::Message::of_event(plan.event))),
        );
    }
    for (name, definition) in types::definitions(ir, roots) {
        out.push(format!("{TYPE_KEY}{name}"), fragment(&definition));
    }
    out
}

/// One fragment of this document's schema table.
fn fragment(node: &Node) -> Fragment {
    under_components(node, TYPE_KEY)
}
