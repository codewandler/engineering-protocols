//! What the `AsyncAPI` projection promises, asserted.
//!
//! Two sources, on purpose. `examples/billing/` is the normative model, so the shape of a real
//! document is checked against it rather than against a copy that would drift (review F7). And a
//! second, deliberately small specification is compiled inline for what billing does not put in an
//! *event*: a binding with `on_failure: drop`, a binding whose command no component accepts — both
//! legal, both the cases where a failure policy is easiest to lose — and one event carrying every
//! primitive and every collection the type mapping has an opinion about. Billing's own events carry a
//! newtype and a struct; its union, enum, map and optionals live on an entity, which publishes no
//! payload, so a projection that got a `Decimal` or a union wrong would pass every billing assertion.
//! None of that can be added to the normative example just to suit a test.
//!
//! # "Valid `AsyncAPI`" here means a hand-checked skeleton
//!
//! `a_document_is_a_valid_asyncapi_three_skeleton` checks the version string, the `info` block,
//! `channels`, `operations` and that every operation's `action` is one `AsyncAPI` 3 knows. That is an
//! enumerated list, not a conformance check, and the difference is the whole class of failures nobody
//! enumerated: a keyword whose value has the wrong type, a keyword at the wrong nesting level, a
//! misspelt key silently ignored, a `messages` entry pointing at a channel that declares it under a
//! different name.
//!
//! Closing that needs the `AsyncAPI` 3.0 meta-schema, which is a third-party document this repository
//! does not hold, and a test may not fetch one — `jsonschema` is a dev-dependency of this crate but
//! it is built with `default-features = false`, so it has no retriever and could not fetch it anyway.
//! Nor is the JSON Schema meta-schema a substitute for the payloads: these documents emit no
//! `schemaFormat`, so a payload is an `AsyncAPI` Schema Object rather than a document in a dialect this
//! file can name, and validating it against 2020-12 would be asserting a dialect the document does
//! not declare. So `docs/plan/ess-roadmap.md` § W3.2's "validated against their own schemas" is
//! **not** met for this projection, vendoring the meta-schema is the open decision, and
//! `tests/openapi.rs`'s `assert_valid` carries the same statement for `OpenAPI`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use ess_compiler::{compile, EssIr};
use ess_domain::spec::{RawSpecFile, Specification};
use ess_domain::system::Source;
use ess_gen::artifact::Generator as _;
use ess_gen::asyncapi::AsyncApi;
use ess_gen::provenance::Provenance;
use serde_yaml::Value;

/// The billing example's directory.
fn example() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/billing")
        .canonicalize()
        .expect("the billing example exists")
}

/// Every `.yaml` file in the example, relative to it, in a stable order.
fn files() -> Vec<String> {
    let base = example();
    let mut found = Vec::new();
    let mut pending = vec![base.clone()];
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(&directory).expect("the example is readable") {
            let path = entry.expect("an entry").path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|it| it == "yaml") {
                found.push(
                    path.strip_prefix(&base)
                        .expect("inside the example")
                        .display()
                        .to_string(),
                );
            }
        }
    }
    found.sort();
    found
}

/// The billing example, compiled.
fn billing() -> EssIr {
    let mut parsed = Vec::new();
    for label in files() {
        let text = std::fs::read_to_string(example().join(&label))
            .unwrap_or_else(|error| panic!("{label} is readable: {error}"));
        let raw = RawSpecFile::parse(&text)
            .unwrap_or_else(|error| panic!("{label} is well formed: {error}"));
        parsed.push((Source::new(label), raw));
    }
    let specification = Specification::assemble(parsed)
        .unwrap_or_else(|errors| panic!("the billing specification validates:\n{errors}"));
    compile(&specification, &ess_compiler::source::SourceMap::new())
        .unwrap_or_else(|diagnostics| panic!("the billing specification resolves:\n{diagnostics}"))
}

/// A small specification carrying what billing deliberately does not.
///
/// `handle-on-started` drops on failure. `orphan-on-handled` invokes a command no component accepts,
/// which §5 permits — decomposition is partial — and which is the one shape where a binding's
/// failure policy has no subscriber document to appear in.
const PROBE: &str = r"
format: ess/1
system: probe
version: v1
domain: probe.core

types:
  - name: probe.core.Ref
    kind: newtype
    of: String
  - name: probe.core.Other
    kind: newtype
    of: String
  - name: probe.core.Mode
    kind: enum
    variants: [Fast, Slow]
  - name: probe.core.Choice
    kind: union
    tag: kind
    variants:
      one: probe.core.Ref
      two: probe.core.Other

commands:
  - name: probe.core.Handle
    input:
      - name: subject
        type: probe.core.Ref
    outcomes:
      - name: done
        emits:
          - probe.core.Handled
  - name: probe.core.Orphan
    input:
      - name: subject
        type: probe.core.Ref
    outcomes:
      - name: done
        emits:
          - probe.core.Handled

events:
  - name: probe.core.Started
    naming:
      wire: probe.started.v1
    fields:
      - name: subject
        type: probe.core.Ref
      - name: amount
        type: Decimal
      - name: note
        type: Optional<String>
      - name: labels
        type: Map<String, String>
      - name: tags
        type: List<probe.core.Ref>
      - name: hints
        type: List<Optional<String>>
      - name: mode
        type: probe.core.Mode
      - name: choice
        type: probe.core.Choice
      - name: identifier
        type: Uuid
      - name: window
        type: Duration
      - name: blob
        type: Bytes
      - name: counts
        type: Map<Integer, Integer>
  - name: probe.core.Handled
    fields:
      - name: subject
        type: probe.core.Ref

components:
  - component: worker
    owns:
      domains:
        - probe.core
    accepts:
      commands:
        - probe.core.Handle
    publishes:
      events:
        - probe.core.Started
        - probe.core.Handled

bindings:
  - id: handle-on-started
    when:
      event: probe.core.Started
    invoke:
      command: probe.core.Handle
    mapping:
      subject: event.subject
    delivery: at_least_once
    on_failure: drop
  - id: orphan-on-handled
    when:
      event: probe.core.Handled
    invoke:
      command: probe.core.Orphan
    mapping:
      subject: event.subject
    delivery: at_least_once
    on_failure: retry
";

/// The probe specification, compiled.
fn probe() -> EssIr {
    let raw = RawSpecFile::parse(PROBE).expect("the probe specification is well formed");
    let specification = Specification::assemble(vec![(Source::new("probe.yaml"), raw)])
        .unwrap_or_else(|errors| panic!("the probe specification validates:\n{errors}"));
    compile(&specification, &ess_compiler::source::SourceMap::new())
        .unwrap_or_else(|diagnostics| panic!("the probe specification resolves:\n{diagnostics}"))
}

/// Every document the projection produces, by artifact path.
fn documents(ir: &EssIr) -> BTreeMap<String, String> {
    let provenance = Provenance::of(ir);
    AsyncApi
        .generate(ir, &provenance)
        .into_iter()
        .map(|artifact| (artifact.path, artifact.contents))
        .collect()
}

/// One document, parsed.
fn parsed(ir: &EssIr, path: &str) -> Value {
    let documents = documents(ir);
    let text = documents
        .get(path)
        .unwrap_or_else(|| panic!("{path} is generated; got {:?}", documents.keys()));
    serde_yaml::from_str(text).unwrap_or_else(|error| panic!("{path} parses as YAML: {error}"))
}

/// The value at a mapping path, or a panic naming the path that was missing.
fn at<'a>(root: &'a Value, path: &[&str]) -> &'a Value {
    let mut node = root;
    for step in path {
        node = node
            .get(*step)
            .unwrap_or_else(|| panic!("`{}` is present", path.join(".")));
    }
    node
}

#[test]
fn every_component_gets_one_document_named_after_it() {
    let ir = billing();
    let documents = documents(&ir);

    assert_eq!(
        documents.keys().cloned().collect::<Vec<_>>(),
        vec![
            "email-service.yaml".to_owned(),
            "invoice-service.yaml".to_owned()
        ],
        "one document per component, and nothing else"
    );
    for component in ir.components.keys() {
        let document = parsed(&ir, &format!("{component}.yaml"));
        assert_eq!(
            at(&document, &["info", "x-ess-component"]).as_str(),
            Some(component.as_str()),
            "the document says which component it is about, by identity and not by display name"
        );
    }
}

#[test]
fn a_document_is_a_valid_asyncapi_three_skeleton() {
    for ir in [billing(), probe()] {
        for (path, text) in documents(&ir) {
            let document: Value = serde_yaml::from_str(&text)
                .unwrap_or_else(|error| panic!("{path} parses as YAML: {error}"));
            assert_eq!(
                document.get("asyncapi").and_then(Value::as_str),
                Some("3.0.0"),
                "{path} declares the version it conforms to"
            );
            assert!(
                at(&document, &["info", "title"]).as_str().is_some(),
                "{path} has an info title"
            );
            assert!(
                at(&document, &["info", "version"]).as_str().is_some(),
                "{path} has an info version"
            );
            assert!(
                document.get("channels").is_some_and(Value::is_mapping),
                "{path} has channels"
            );
            assert!(
                document.get("operations").is_some_and(Value::is_mapping),
                "{path} has operations"
            );
            for (id, operation) in document["operations"]
                .as_mapping()
                .expect("operations is a mapping")
            {
                let action = operation.get("action").and_then(Value::as_str);
                assert!(
                    action == Some("send") || action == Some("receive"),
                    "{path}: operation {id:?} has an action AsyncAPI 3 knows"
                );
            }
        }
    }
}

#[test]
fn a_document_shows_what_the_component_publishes_and_what_it_reacts_to() {
    let ir = billing();

    let publisher = parsed(&ir, "invoice-service.yaml");
    assert!(
        at(
            &publisher,
            &["operations", "send.billing.invoice.InvoiceCreated"]
        )
        .get("action")
        .is_some(),
        "the component that declares the event sends it"
    );
    assert!(
        at(&publisher, &["operations"])
            .get("receive.billing.invoice.InvoiceCreated")
            .is_none(),
        "nothing in invoice-service reacts to its own event"
    );

    // The half a system this test exists for: email-service declares no channel of its own for
    // InvoiceCreated, and reacts to it, so the document has to carry a channel it does not publish.
    let consumer = parsed(&ir, "email-service.yaml");
    assert!(
        at(
            &consumer,
            &["operations", "receive.billing.invoice.InvoiceCreated"]
        )
        .get("action")
        .is_some(),
        "the component whose command the binding invokes receives the event"
    );
    assert!(
        at(&consumer, &["operations", "send.billing.email.EmailSent"])
            .get("action")
            .is_some(),
        "and still sends what it declares"
    );
    assert!(
        at(&consumer, &["channels"])
            .get("billing.invoice.InvoiceCreated")
            .is_some(),
        "a received event needs a channel in the receiving component's document"
    );
}

#[test]
fn an_events_channel_address_is_its_declared_wire_name_or_else_its_qualified_name() {
    let billing = parsed(&billing(), "invoice-service.yaml");
    let channel = at(&billing, &["channels", "billing.invoice.InvoiceCreated"]);
    assert_eq!(
        channel.get("address").and_then(Value::as_str),
        Some("billing.invoice.InvoiceCreated"),
        "billing declares no wire name for the event, so the address is the only globally unique \
         name the model has"
    );
    assert_eq!(
        channel.get("x-ess-address-source").and_then(Value::as_str),
        Some("qualified-name"),
        "and the document says the address was derived rather than chosen"
    );

    let probe = parsed(&probe(), "worker.yaml");
    let channel = at(&probe, &["channels", "probe.core.Started"]);
    assert_eq!(
        channel.get("address").and_then(Value::as_str),
        Some("probe.started.v1"),
        "a declared wire name is the address, verbatim, version and all"
    );
    assert_eq!(
        channel.get("x-ess-address-source").and_then(Value::as_str),
        Some("naming.wire"),
        "and the document says somebody chose it"
    );
}

#[test]
fn a_bindings_delivery_and_failure_reach_the_receiving_operation() {
    let document = parsed(&billing(), "email-service.yaml");
    let reactions = at(
        &document,
        &[
            "operations",
            "receive.billing.invoice.InvoiceCreated",
            "x-ess-reactions",
        ],
    )
    .as_sequence()
    .expect("the reactions are a list");

    assert_eq!(reactions.len(), 1, "billing declares one binding");
    let reaction = &reactions[0];
    assert_eq!(
        reaction.get("binding").and_then(Value::as_str),
        Some("notify-on-invoice-created")
    );
    assert_eq!(
        reaction.get("invokes").and_then(Value::as_str),
        Some("billing.email.SendEmail")
    );
    assert_eq!(
        reaction.get("delivery").and_then(Value::as_str),
        Some("at_least_once"),
        "the word the author wrote, spelt the way they wrote it"
    );
    assert_eq!(
        reaction.get("on_failure").and_then(Value::as_str),
        Some("escalate")
    );
    assert!(
        reaction
            .get("delivery_means")
            .and_then(Value::as_str)
            .is_some_and(|it| it.contains("idempotent")),
        "at_least_once is only actionable if the document says the handler must be idempotent"
    );
}

#[test]
fn a_dropped_failure_is_stated_in_prose_and_not_only_in_an_extension() {
    let document = parsed(&probe(), "worker.yaml");
    let operation = at(&document, &["operations", "receive.probe.core.Started"]);

    assert_eq!(
        at(operation, &["x-ess-reactions"])
            .as_sequence()
            .and_then(|it| it.first())
            .and_then(|it| it.get("on_failure"))
            .and_then(Value::as_str),
        Some("drop")
    );

    // An extension is easy to skim past, and `drop` is the one word review F3 required an author to
    // type. So it also has to be in the prose a person actually reads.
    let description = operation
        .get("description")
        .and_then(Value::as_str)
        .expect("the operation is described");
    assert!(
        description.contains("drop"),
        "the failure word is in the description: {description}"
    );
    assert!(
        description.contains("nobody is told"),
        "and so is what it costs: {description}"
    );
}

#[test]
fn the_publisher_of_an_event_sees_who_reacts_to_it_and_under_what_failure_policy() {
    let document = parsed(&billing(), "invoice-service.yaml");
    let consumers = at(
        &document,
        &[
            "operations",
            "send.billing.invoice.InvoiceCreated",
            "x-ess-consumed-by",
        ],
    )
    .as_sequence()
    .expect("the consumers are a list");

    assert_eq!(consumers.len(), 1);
    assert_eq!(
        consumers[0].get("handled_by").and_then(Value::as_str),
        Some("email-service"),
        "the publisher can see which component handles its event"
    );
    assert_eq!(
        consumers[0].get("on_failure").and_then(Value::as_str),
        Some("escalate"),
        "and under what failure policy, without opening the other document"
    );
}

#[test]
fn a_binding_no_component_handles_still_states_its_failure_policy() {
    // `probe.core.Orphan` is accepted by nobody, so there is no receiving document for this binding
    // to appear in. If the publisher's side did not carry it, `on_failure: retry` would exist in the
    // model and in no generated artifact at all.
    let document = parsed(&probe(), "worker.yaml");
    let consumers = at(
        &document,
        &["operations", "send.probe.core.Handled", "x-ess-consumed-by"],
    )
    .as_sequence()
    .expect("the consumers are a list");

    assert_eq!(consumers.len(), 1);
    assert_eq!(
        consumers[0].get("binding").and_then(Value::as_str),
        Some("orphan-on-handled")
    );
    assert!(
        consumers[0].get("handled_by").is_some_and(Value::is_null),
        "null, not absent: no component accepts the command this binding invokes"
    );
    assert_eq!(
        consumers[0].get("on_failure").and_then(Value::as_str),
        Some("retry")
    );
}

#[test]
fn a_bindings_mapping_and_the_reason_for_its_type_crossing_reach_the_document() {
    let document = parsed(&billing(), "email-service.yaml");
    let mapping = at(
        &document,
        &[
            "operations",
            "receive.billing.invoice.InvoiceCreated",
            "x-ess-reactions",
        ],
    )
    .as_sequence()
    .and_then(|it| it.first())
    .and_then(|it| it.get("mapping"))
    .and_then(Value::as_sequence)
    .expect("the mapping is a list");

    let recipient = mapping
        .iter()
        .find(|entry| entry.get("target").and_then(Value::as_str) == Some("recipient"))
        .expect("`recipient` is mapped");
    assert_eq!(
        recipient.get("type").and_then(Value::as_str),
        Some("billing.email.EmailAddress")
    );
    assert_eq!(
        at(recipient, &["source", "kind"]).as_str(),
        Some("event_field")
    );
    assert_eq!(
        at(recipient, &["source", "field"]).as_str(),
        Some("customer_email")
    );
    assert!(
        recipient
            .get("conversion")
            .and_then(Value::as_str)
            .is_some_and(|it| it.contains("deliverable address")),
        "a crossing between two newtypes carries the reason its author gave for allowing it"
    );

    let template = mapping
        .iter()
        .find(|entry| entry.get("target").and_then(Value::as_str) == Some("template"))
        .expect("`template` is mapped");
    assert_eq!(at(template, &["source", "kind"]).as_str(), Some("literal"));
    assert_eq!(
        at(template, &["source", "value"]).as_str(),
        Some("invoice-created"),
        "a value written into the binding is marked as one, because the compiler took its type on \
         trust"
    );
}

#[test]
fn the_channel_and_its_message_say_nothing_about_the_binding() {
    // A channel is transport and a binding is system semantics. If a mapping or a failure policy
    // ever leaks into `channels` or `components.messages`, that distinction has quietly gone.
    let document = parsed(&billing(), "email-service.yaml");
    for section in ["channels", "components"] {
        let rendered =
            serde_yaml::to_string(at(&document, &[section])).expect("the section serialises");
        for forbidden in ["on_failure", "delivery", "mapping", "x-ess-reactions"] {
            assert!(
                !rendered.contains(forbidden),
                "`{forbidden}` appears under `{section}`, where only transport belongs:\n{rendered}"
            );
        }
    }
}

#[test]
fn every_ref_resolves_inside_the_document_that_holds_it() {
    for ir in [billing(), probe()] {
        for (path, text) in documents(&ir) {
            let document: Value =
                serde_yaml::from_str(&text).unwrap_or_else(|error| panic!("{path}: {error}"));
            let mut refs = Vec::new();
            collect_refs(&document, &mut refs);
            assert!(!refs.is_empty(), "{path} references something");
            for reference in refs {
                let pointer = reference
                    .strip_prefix("#/")
                    .unwrap_or_else(|| panic!("{path}: `{reference}` is a local JSON pointer"));
                assert!(
                    resolve(&document, pointer).is_some(),
                    "{path}: `{reference}` resolves nowhere"
                );
            }
        }
    }
}

/// Every `$ref` string anywhere in a document.
fn collect_refs(node: &Value, into: &mut Vec<String>) {
    match node {
        Value::Mapping(mapping) => {
            for (key, value) in mapping {
                if key.as_str() == Some("$ref") {
                    into.push(value.as_str().expect("a `$ref` is a string").to_owned());
                } else {
                    collect_refs(value, into);
                }
            }
        }
        Value::Sequence(items) => {
            for item in items {
                collect_refs(item, into);
            }
        }
        _ => {}
    }
}

/// Walks a JSON pointer, whose tokens need no unescaping here because a qualified name's segments
/// are letters, digits, `-` and `_` joined by dots.
fn resolve<'a>(root: &'a Value, pointer: &str) -> Option<&'a Value> {
    let mut node = root;
    for token in pointer.split('/') {
        node = node.get(token)?;
    }
    Some(node)
}

/// The `probe.core.Started` payload, as the worker's document publishes it.
///
/// Three tests read it, because it is the one construct in this file carrying every primitive and
/// every collection the mapping has an opinion about, and one test asserting all of it is a test
/// nobody reads the failure of.
fn started_payload(document: &Value) -> &Value {
    at(
        document,
        &["components", "schemas", "event.probe.core.Started"],
    )
}

#[test]
fn a_payload_refuses_an_undeclared_field_and_spells_absence_by_leaving_it_out_of_required() {
    // This assertion used to be its opposite: `additionalProperties` absent, because "whether an
    // unknown field is an error is an evolution policy the model has not stated". The `schema`
    // projection closed the same event, so one published file refused `{"bogus": 1}` and another
    // accepted it — and an evolution policy stated in one artifact and not the other is not a policy.
    let document = parsed(&probe(), "worker.yaml");
    let payload = started_payload(&document);

    assert_eq!(payload.get("type").and_then(Value::as_str), Some("object"));
    assert_eq!(
        payload.get("additionalProperties").and_then(Value::as_bool),
        Some(false),
        "an undeclared field is refused here exactly as the JSON Schema for this event refuses it"
    );
    assert_eq!(
        at(payload, &["x-ess-name"]).as_str(),
        Some("probe.core.Started"),
        "the payload says which fact it describes, in the spelling every projection uses"
    );
    assert_eq!(
        at(payload, &["x-ess-kind"]).as_str(),
        Some("event-payload"),
        "and which kind of construct it is, so a schema lifted out of this document still says"
    );

    let required: Vec<&str> = at(payload, &["required"])
        .as_sequence()
        .expect("required is a list")
        .iter()
        .filter_map(Value::as_str)
        .collect();
    assert!(
        !required.contains(&"note"),
        "an Optional field is expressed by absence from `required`: {required:?}"
    );
    assert!(required.contains(&"subject"), "{required:?}");
    assert!(
        at(payload, &["properties", "note"])
            .get("x-ess-optional")
            .is_none(),
        "inside a struct, `required` already says it; saying it twice invites the two to disagree"
    );
}

#[test]
fn a_payload_field_carries_the_grammar_the_model_states_and_not_a_note_naming_it() {
    // The four rows this projection used to answer with `x-ess-type` and no keyword. A conforming
    // validator ignores an extension by specification and asserts a keyword, so each of those rows
    // published a document that accepted values the specification refuses — `"abc"` as a Decimal
    // being the one that reached a report.
    let document = parsed(&probe(), "worker.yaml");
    let properties = at(started_payload(&document), &["properties"]);

    assert_eq!(
        at(properties, &["amount", "type"]).as_str(),
        Some("string"),
        "a Decimal is text: a JSON number is a float in most parsers, and money does not round \
         the way a float does"
    );
    assert_eq!(
        at(properties, &["amount", "format"]).as_str(),
        Some("decimal"),
        "and says so in the keyword a validator reads, not only in an extension it ignores"
    );
    assert!(
        at(properties, &["amount", "pattern"]).as_str().is_some(),
        "and carries the grammar, so a decimal not written as one is refused rather than noted"
    );
    assert_eq!(
        at(properties, &["identifier", "pattern"]).as_str(),
        Some("^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$"),
        "a Uuid is the canonical hyphenated form, checked rather than described"
    );
    assert_eq!(
        at(properties, &["window", "format"]).as_str(),
        Some("duration"),
        "a Duration names a registered format instead of an extension no validator reads"
    );
    assert_eq!(
        at(properties, &["blob", "contentEncoding"]).as_str(),
        Some("base64"),
        "Bytes says how it is encoded"
    );
    assert!(
        at(properties, &["blob", "pattern"]).as_str().is_some(),
        "and carries the pattern, because `contentEncoding` is an annotation in 2020-12 and \
         describes the encoding without checking it"
    );
}

#[test]
fn a_collection_says_what_it_holds_and_an_absent_element_is_null_because_it_has_no_key_to_omit() {
    let document = parsed(&probe(), "worker.yaml");
    let properties = at(started_payload(&document), &["properties"]);

    assert_eq!(
        at(properties, &["tags", "type"]).as_str(),
        Some("array"),
        "a List is an array"
    );
    assert_eq!(
        at(properties, &["labels", "x-ess-map-key"]).as_str(),
        Some("String"),
        "a map records its key type, which a JSON object cannot express"
    );
    assert!(
        at(properties, &["labels"]).get("propertyNames").is_none(),
        "a String key constrains nothing, and a rule that checks nothing invites a reader to \
         believe something was checked"
    );
    assert_eq!(
        at(properties, &["counts", "propertyNames", "pattern"]).as_str(),
        Some("^-?(0|[1-9][0-9]*)$"),
        "an Integer-keyed map is an object whose keys are the text an integer is spelt with, so \
         `7` and `007` cannot both be entries for one key"
    );
    // Outside a struct there is no `required` to carry absence, so the projection names `null` as the
    // spelling of an absent element. That claims something the model does not state — and the
    // `x-ess-optional` it replaced claimed *less* than the model does, in a keyword no validator
    // reads, which is the worse of the two.
    let branches = at(properties, &["hints", "items", "anyOf"])
        .as_sequence()
        .expect("an Optional inside a List is a choice of two");
    assert_eq!(branches.len(), 2);
    assert_eq!(
        branches[0].get("type").and_then(Value::as_str),
        Some("string")
    );
    assert_eq!(
        branches[1].get("type").and_then(Value::as_str),
        Some("null")
    );
    assert_eq!(
        at(properties, &["subject", "$ref"]).as_str(),
        Some("#/components/schemas/type.probe.core.Ref"),
        "a named type is referenced once and defined once"
    );
}

#[test]
fn a_union_pins_its_tag_so_exactly_one_branch_matches_rather_than_none_or_both() {
    // This projection published `anyOf` over the bare variant payloads plus `x-ess-union-tag`, on the
    // grounds that the model does not say how the tag and the value sit together. But a schema in
    // which the tag does not appear at all is not a refusal to state a layout — it is a different
    // layout, one with no tag, in which a bare string validates as a Choice. The objection to
    // `oneOf`, that two variants which are both a String underneath match twice, is only true once
    // the tag has been dropped.
    let document = parsed(&probe(), "worker.yaml");
    let mode = at(
        &document,
        &["components", "schemas", "type.probe.core.Mode"],
    );
    assert_eq!(
        at(mode, &["enum"]).as_sequence().map(Vec::len),
        Some(2),
        "an enum's variants survive"
    );

    let choice = at(
        &document,
        &["components", "schemas", "type.probe.core.Choice"],
    );
    assert_eq!(
        at(choice, &["x-ess-union-tag"]).as_str(),
        Some("kind"),
        "a union carries the field its variant is named in"
    );
    assert!(
        choice.get("anyOf").is_none(),
        "an `anyOf` over the bare variants publishes a schema in which the tag does not appear at \
         all, so a bare string validates as a Choice"
    );
    let variants = at(choice, &["oneOf"])
        .as_sequence()
        .expect("a union is a choice of tagged branches");
    assert_eq!(variants.len(), 2);
    assert_eq!(
        at(&variants[0], &["properties", "kind", "const"]).as_str(),
        Some("one"),
        "each branch pins its tag, so exactly one branch matches and no decoder has to guess"
    );
    assert_eq!(
        at(&variants[0], &["properties", "value", "$ref"]).as_str(),
        Some("#/components/schemas/type.probe.core.Ref"),
        "the payload sits beside the tag, because a variant may be a scalar with nowhere to put one"
    );
    assert_eq!(
        at(&variants[0], &["additionalProperties"]).as_bool(),
        Some(false)
    );
}

#[test]
fn every_document_carries_the_provenance_of_the_model_it_came_from() {
    let ir = billing();
    let provenance = Provenance::of(&ir);
    for (path, text) in documents(&ir) {
        for line in provenance.lines() {
            assert!(
                text.contains(&line),
                "{path} is missing the provenance line `{line}`"
            );
        }
        let document: Value = serde_yaml::from_str(&text).expect("parses");
        let recorded = at(&document, &["info", "x-ess-provenance"]);
        assert_eq!(
            recorded.get("source_digest").and_then(Value::as_str),
            Some(provenance.source_digest.as_str()),
            "{path} carries the digest as data, not only as a comment a parser throws away"
        );
        assert_eq!(
            recorded.get("generator_version").and_then(Value::as_str),
            Some(Provenance::VERSION),
        );
    }
}

#[test]
fn regenerating_from_the_same_model_produces_the_same_bytes() {
    // Review F8's point: determinism claimed by "BTreeMap only, no clock, no RNG" is worth nothing
    // unasserted. Two independent compilations, so the IR is rebuilt too and not merely reused.
    for (first, second) in [(billing(), billing()), (probe(), probe())] {
        assert_eq!(
            documents(&first),
            documents(&second),
            "the same specification twice must project to the same bytes"
        );
    }
}

#[test]
fn every_event_in_the_billing_example_appears_in_some_document() {
    // A projection silently dropping a construct is the bug the roadmap's W3.2 asks to catch. Every
    // event billing declares is published by a component, so every one of them owes a channel.
    let ir = billing();
    let documents = documents(&ir);
    let rendered = documents.values().cloned().collect::<Vec<_>>().join("\n");
    for name in ir.events.keys() {
        assert!(
            rendered.contains(&format!("x-ess-event: {name}")),
            "`{name}` has no channel in any document"
        );
    }
    for binding in ir.bindings.values() {
        assert!(
            rendered.contains(&format!("binding: {}", binding.name)),
            "binding `{}` appears in no document, so its delivery and failure semantics are lost",
            binding.name
        );
    }
    assert!(
        ir.conversions
            .iter()
            .all(|conversion| rendered.contains(&conversion.because)),
        "a declared conversion used by a mapping carries its reason into the projection"
    );
}
