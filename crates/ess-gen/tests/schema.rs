//! The published schemas, checked against instances rather than eyeballed.
//!
//! A generated schema is derived from the IR, so it cannot describe a message the compiler would
//! have refused — but it *can* describe the wrong thing entirely, and it can describe nothing at
//! all. `aep-schema/tests/published.rs` exists because this repository has published a schema that
//! rejected its own normative example, and because it published one that described a Rust
//! representation rather than what an author writes. Both were well formed. Both passed every check
//! that only asked whether the output parsed.
//!
//! So every test here either validates an instance or asserts a property nobody can satisfy by
//! accident. Where a schema is asserted to accept something, a matching malformed instance is
//! asserted to be refused, because a schema that accepts everything is not a schema.
//!
//! # Which validator, and what it is allowed to reach
//!
//! `jsonschema`, a dev-dependency of this crate, declared exactly as `aep-schema` declares it:
//! `default-features = false`. That switch turns off `resolve-http` and `resolve-file`, so a
//! validator built here has no retriever at all — no test in this crate can reach the network, and a
//! reference is either resolvable inside its own document or an error. Every `$ref` this projection
//! emits is `#/$defs/…`, which is what makes that affordable. The 2020-12 meta-schema is bundled in
//! the crate, so `jsonschema::meta::validate` checks a published document against the dialect it
//! declares without fetching it.
//!
//! Formats stay annotations, which is what a conforming 2020-12 validator does unless asked
//! otherwise and what `src/types.rs` designs for: `format` is a hint to a reader, and `pattern` is
//! what refuses a malformed value. So `pattern` is the keyword a real validator buys here — a
//! hand-rolled structural checker has no regex engine, and this file carried one until the
//! dev-dependency was wired in. `a_decimal_amount_is_refused_when_it_is_not_written_the_way_the_pattern_says`,
//! `a_uuid_is_refused_unless_it_is_the_canonical_hyphenated_form`,
//! `a_map_key_that_is_not_the_text_its_key_type_is_spelt_with_is_refused` and
//! `a_bytes_field_refuses_a_string_that_is_not_base64` are the four instances that used to pass
//! unchecked.
//!
//! The one property a validator cannot state is the closed keyword set: a validator *ignores* a
//! keyword it does not know, so "the generator emits nothing outside this list" is an assertion
//! about the documents rather than about any instance. It is made directly, by
//! `no_schema_uses_a_keyword_outside_the_set_this_projection_publishes`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use ess_compiler::resolve::compile;
use ess_compiler::source::SourceMap;
use ess_compiler::EssIr;
use ess_domain::spec::{RawSpecFile, Specification};
use ess_domain::system::Source;
use ess_gen::artifact::{run, Artifact};
use ess_gen::schema::JsonSchema;
use serde_json::{json, Value};

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
    assert!(!found.is_empty(), "the billing example holds no files");
    found.sort();
    found
}

/// The billing example, compiled.
///
/// From the files it lives in rather than a copy inlined here: the design document's own snippets
/// drifted three ways before anyone noticed, and a copy drifts the same way.
fn billing() -> EssIr {
    let mut sources = SourceMap::new();
    let mut parsed = Vec::new();
    for label in files() {
        let text = std::fs::read_to_string(example().join(&label))
            .unwrap_or_else(|error| panic!("{label} is readable: {error}"));
        let raw = RawSpecFile::parse(&text)
            .unwrap_or_else(|error| panic!("{label} is well formed: {error}"));
        sources.insert(label.clone(), text);
        parsed.push((Source::new(label), raw));
    }
    let specification = Specification::assemble(parsed)
        .unwrap_or_else(|errors| panic!("the billing specification validates:\n{errors}"));
    compile(&specification, &sources)
        .unwrap_or_else(|diagnostics| panic!("the billing specification resolves:\n{diagnostics}"))
}

/// A specification written for one test, compiled.
///
/// The billing example exercises the constructs a real system has; a test needs the ones it happens
/// not to reach from a message — `Optional`, `List`, `Map`, and the primitives only its entity uses.
fn compiled(text: &str) -> EssIr {
    let raw = RawSpecFile::parse(text).unwrap_or_else(|error| panic!("well formed: {error}"));
    let mut sources = SourceMap::new();
    sources.insert(Source::DOCUMENT, text.to_owned());
    let specification = Specification::assemble([(Source::document(), raw)])
        .unwrap_or_else(|errors| panic!("validates:\n{errors}"));
    compile(&specification, &sources)
        .unwrap_or_else(|diagnostics| panic!("resolves:\n{diagnostics}"))
}

/// Every artifact this projection produces for an IR, keyed by path.
fn artifacts(ir: &EssIr) -> BTreeMap<String, Artifact> {
    run(&JsonSchema, ir).expect("no two schemas claim one path")
}

/// One artifact's contents, parsed.
fn document(artifacts: &BTreeMap<String, Artifact>, path: &str) -> Value {
    let artifact = artifacts
        .get(path)
        .unwrap_or_else(|| panic!("{path} is published; the tree holds {:?}", artifacts.keys()));
    assert!(
        artifact.contents.ends_with('\n'),
        "{path} lacks a trailing newline"
    );
    serde_json::from_str(&artifact.contents)
        .unwrap_or_else(|error| panic!("{path} is JSON: {error}"))
}

/// A conforming validator for one published document.
///
/// `validator_for` reads the dialect out of the document's own `$schema`, so nothing here picks a
/// draft on the generator's behalf, and building it resolves every reference — with no retriever
/// configured, a reference that does not resolve inside the document fails here rather than turning
/// into a request.
fn validator_for(document: &Value) -> jsonschema::Validator {
    jsonschema::validator_for(document).unwrap_or_else(|error| panic!("a usable schema: {error}"))
}

/// Asserts an instance validates, naming every problem rather than the first.
fn accepts(validator: &jsonschema::Validator, instance: &Value, why: &str) {
    let problems: Vec<String> = validator
        .iter_errors(instance)
        .map(|error| format!("{}: {error}", error.instance_path()))
        .collect();
    assert!(
        problems.is_empty(),
        "{why}, but the schema refused it:\n  - {}",
        problems.join("\n  - ")
    );
}

/// Asserts an instance is refused.
fn refuses(validator: &jsonschema::Validator, instance: &Value, why: &str) {
    assert!(
        !validator.is_valid(instance),
        "{why}, but the schema accepted it"
    );
}

/// What a same-document pointer points at, or a panic naming the pointer that dangles.
fn resolve(document: &Value, pointer: &str) -> Value {
    let path = pointer
        .strip_prefix("#/")
        .unwrap_or_else(|| panic!("{pointer} is a same-document pointer"));
    let mut current = document;
    for segment in path.split('/') {
        current = current
            .get(segment)
            .unwrap_or_else(|| panic!("{pointer} resolves"));
    }
    current.clone()
}

/// Every keyword appearing anywhere in a document, including inside `$defs`.
fn keywords(node: &Value, into: &mut std::collections::BTreeSet<String>) {
    match node {
        Value::Object(members) => {
            for (name, value) in members {
                into.insert(name.clone());
                // Descend only where a schema can be, so a property or a definition *named* `type`
                // is not mistaken for the keyword.
                if matches!(
                    name.as_str(),
                    "$defs" | "properties" | "$ref" | "const" | "enum" | "x-ess-provenance"
                ) {
                    continue;
                }
                keywords(value, into);
            }
            for group in ["$defs", "properties"] {
                for value in members
                    .get(group)
                    .and_then(Value::as_object)
                    .into_iter()
                    .flatten()
                {
                    keywords(value.1, into);
                }
            }
        }
        Value::Array(elements) => {
            for element in elements {
                keywords(element, into);
            }
        }
        _ => {}
    }
}

#[test]
fn every_command_input_event_payload_error_payload_and_named_type_gets_a_schema() {
    // Named rather than counted: a count tells whoever broke it that a number changed, and a list
    // tells them which contract went missing.
    let artifacts = artifacts(&billing());
    for expected in [
        "schema/commands/billing.email.SendEmail.schema.json",
        "schema/commands/billing.invoice.CreateInvoice.schema.json",
        "schema/errors/billing.email.Undeliverable.schema.json",
        "schema/errors/billing.invoice.InvalidAmount.schema.json",
        "schema/events/billing.email.EmailSent.schema.json",
        "schema/events/billing.invoice.InvoiceCreated.schema.json",
        "schema/types/billing.email.EmailAddress.schema.json",
        "schema/types/billing.email.MessageId.schema.json",
        "schema/types/billing.email.TemplateId.schema.json",
        "schema/types/billing.invoice.Channel.schema.json",
        "schema/types/billing.invoice.CompanyRef.schema.json",
        "schema/types/billing.invoice.Email.schema.json",
        "schema/types/billing.invoice.Invoice.State.schema.json",
        "schema/types/billing.invoice.InvoiceId.schema.json",
        "schema/types/billing.invoice.LineItem.schema.json",
        "schema/types/billing.invoice.Money.schema.json",
        "schema/types/billing.invoice.Payee.schema.json",
    ] {
        assert!(
            artifacts.contains_key(expected),
            "{expected} is not published; the tree holds {:?}",
            artifacts.keys()
        );
    }
}

#[test]
fn every_artifact_is_a_json_schema_document_declaring_the_dialect_it_is_written_in() {
    // Every file, not a sample, and every file is a schema: no index, no manifest, nothing in the
    // tree that a validator cannot be pointed at.
    for (path, artifact) in artifacts(&billing()) {
        let parsed: Value = serde_json::from_str(&artifact.contents)
            .unwrap_or_else(|error| panic!("{path} is JSON: {error}"));
        assert_eq!(
            parsed["$schema"].as_str(),
            Some("https://json-schema.org/draft/2020-12/schema"),
            "{path} does not say which dialect it is written in"
        );
        assert!(
            parsed.get("$ref").is_some() || parsed.get("type").is_some(),
            "{path} asserts nothing about any instance"
        );
    }
}

#[test]
fn every_published_document_is_a_valid_json_schema_in_the_dialect_it_declares() {
    // Stronger than "it parses" and stronger than "an instance validated against it": the document
    // itself is checked against the 2020-12 meta-schema, which `jsonschema` bundles — so this runs
    // with no retriever and no network — and a keyword whose *value* has the wrong shape (a
    // `required` holding a number, a `pattern` that is not a string) fails here.
    //
    // This is roadmap W3.2's "validated against their own schemas" for this projection. The OpenAPI
    // and AsyncAPI projections cannot make the same check: their meta-schemas are third-party
    // documents that ship with nothing in this repository, which is why `tests/openapi.rs` checks
    // its envelope by hand and says so.
    for ir in [billing(), compiled(EVERY_SHAPE)] {
        for (path, artifact) in artifacts(&ir) {
            let parsed: Value = serde_json::from_str(&artifact.contents).expect("is JSON");
            jsonschema::meta::validate(&parsed).unwrap_or_else(|error| {
                panic!("{path} is not a valid JSON Schema 2020-12 document: {error}")
            });
            // And it builds into a validator, which is where a `$ref` pointing at something that is
            // not a schema stops being a string and becomes a failure.
            validator_for(&parsed);
        }
    }

    // Not vacuous: the same call refuses a document whose keyword *values* have the wrong shape,
    // which is the class this check exists for and the one nothing here looked at before.
    let broken = json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "type": "object",
        "required": 3,
    });
    assert!(
        jsonschema::meta::validate(&broken).is_err(),
        "a `required` of `3` was accepted, so this test asserts nothing about the ones above"
    );
}

#[test]
fn every_reference_resolves_inside_the_document_that_makes_it() {
    // The failure a self-contained layout invites: a `$defs` holding what the message mentions
    // directly, and a `$ref` into it pointing at nothing. Such a document parses, has every
    // required keyword, and fails the moment anyone validates against it.
    for (path, artifact) in artifacts(&billing()) {
        let parsed: Value = serde_json::from_str(&artifact.contents).expect("is JSON");
        for pointer in references(&parsed) {
            // `resolve` panics when a pointer dangles, which is the assertion.
            let target = resolve(&parsed, &pointer);
            assert!(
                target.is_object(),
                "{path}: {pointer} resolves to something that is not a schema"
            );
        }
    }
}

/// Every `$ref` in a document.
fn references(node: &Value) -> Vec<String> {
    let mut found = Vec::new();
    match node {
        Value::Object(members) => {
            for (name, value) in members {
                if name == "$ref" {
                    if let Some(pointer) = value.as_str() {
                        found.push(pointer.to_owned());
                    }
                } else {
                    found.extend(references(value));
                }
            }
        }
        Value::Array(elements) => {
            for element in elements {
                found.extend(references(element));
            }
        }
        _ => {}
    }
    found
}

#[test]
fn a_newtype_keeps_its_name_instead_of_collapsing_into_its_representation() {
    // `Email` and `EmailAddress` are both a `String` underneath, and the entire value of naming them
    // apart is that the model refuses to conflate them. A schema rendering both as `{"type":
    // "string"}` inline would have thrown that away, and nothing downstream could get it back.
    let artifacts = artifacts(&billing());
    let input = document(
        &artifacts,
        "schema/commands/billing.invoice.CreateInvoice.schema.json",
    );

    assert_eq!(
        input["properties"]["customer_email"]["$ref"].as_str(),
        Some("#/$defs/billing.invoice.Email"),
        "the input inlines its newtype instead of naming it"
    );
    assert_eq!(
        input["$defs"]["billing.invoice.Email"]["x-ess-name"].as_str(),
        Some("billing.invoice.Email")
    );

    let other = document(
        &artifacts,
        "schema/commands/billing.email.SendEmail.schema.json",
    );
    assert_eq!(
        other["properties"]["recipient"]["$ref"].as_str(),
        Some("#/$defs/billing.email.EmailAddress"),
        "two newtypes over `String` must not become one schema"
    );
}

#[test]
fn a_newtype_over_a_string_publishes_no_constraint_the_specification_never_stated() {
    // The failure `aep-schema/tests/published.rs` was written for, in its other direction: a schema
    // that refuses data the model permits. `Email` wraps a `String` and says nothing about its
    // shape, so `"format": "email"` here would reject addresses this specification allows.
    let artifacts = artifacts(&billing());
    let email = document(&artifacts, "schema/types/billing.invoice.Email.schema.json");
    let definition = &email["$defs"]["billing.invoice.Email"];

    assert_eq!(definition["type"].as_str(), Some("string"));
    assert!(definition.get("format").is_none(), "an invented format");
    assert!(definition.get("pattern").is_none(), "an invented pattern");
    assert_eq!(definition["x-ess-kind"].as_str(), Some("newtype"));
}

#[test]
fn a_uuid_newtype_carries_the_format_of_what_it_wraps() {
    let artifacts = artifacts(&billing());
    let id = document(
        &artifacts,
        "schema/types/billing.invoice.InvoiceId.schema.json",
    );
    let definition = &id["$defs"]["billing.invoice.InvoiceId"];

    assert_eq!(definition["type"].as_str(), Some("string"));
    assert_eq!(definition["format"].as_str(), Some("uuid"));
}

#[test]
fn a_uuid_is_refused_unless_it_is_the_canonical_hyphenated_form() {
    // `format: uuid` is the annotation; the pattern is the assertion. The `urn:uuid:` and
    // brace-wrapped forms are both a UUID to a reader, and `src/types.rs` decided neither is what
    // crosses this boundary — a decision only a validator that applies `pattern` enforces.
    let artifacts = artifacts(&billing());
    let validator = validator_for(&document(
        &artifacts,
        "schema/types/billing.invoice.InvoiceId.schema.json",
    ));
    let canonical = "8ba7b810-9dad-11d1-80b4-00c04fd430c8";

    accepts(
        &validator,
        &json!(canonical),
        "the canonical hyphenated form is the one the projection publishes",
    );
    for other in [
        format!("urn:uuid:{canonical}"),
        format!("{{{canonical}}}"),
        canonical.replace('-', ""),
        canonical[..canonical.len() - 1].to_owned(),
    ] {
        refuses(
            &validator,
            &json!(other),
            &format!("`{other}` is not the form this model states"),
        );
    }
}

#[test]
fn a_command_input_accepts_a_filled_instance_and_refuses_a_misspelt_field() {
    let artifacts = artifacts(&billing());
    let validator = validator_for(&document(
        &artifacts,
        "schema/commands/billing.invoice.CreateInvoice.schema.json",
    ));

    accepts(
        &validator,
        &json!({
            "customer_email": "someone@example.com",
            "amount": { "amount": "12.50", "currency": "EUR" },
        }),
        "this is what `create-invoice` is called with",
    );
    refuses(
        &validator,
        &json!({
            "customer_emal": "someone@example.com",
            "amount": { "amount": "12.50", "currency": "EUR" },
        }),
        "a misspelt field must be a rejection, not a key silently ignored",
    );
    refuses(
        &validator,
        &json!({ "customer_email": "someone@example.com" }),
        "`amount` is not optional, so an input without it is not an input",
    );
    refuses(
        &validator,
        &json!({
            "customer_email": "someone@example.com",
            "amount": { "amount": "12.50" },
        }),
        "`Money` has a currency, and half a `Money` is not one",
    );
}

#[test]
fn an_amount_is_written_as_an_exact_decimal_string_and_a_float_is_refused() {
    // Money does not round the way a float does, and a JSON number is read as a float by most of
    // the world. The cost of the string is that a consumer cannot use `minimum`; the cost of the
    // number would have been that `0.1` validated as an exact amount.
    let artifacts = artifacts(&billing());
    let money = document(&artifacts, "schema/types/billing.invoice.Money.schema.json");
    let definition = &money["$defs"]["billing.invoice.Money"];

    assert_eq!(
        definition["properties"]["amount"]["type"].as_str(),
        Some("string")
    );
    assert_eq!(
        definition["properties"]["amount"]["format"].as_str(),
        Some("decimal")
    );

    refuses(
        &validator_for(&money),
        &json!({ "amount": 12.5, "currency": "EUR" }),
        "a float is not an exact decimal",
    );
}

#[test]
fn a_decimal_amount_is_refused_when_it_is_not_written_the_way_the_pattern_says() {
    // The gap the structural checker this file used to carry left open, and the whole reason
    // `jsonschema` is a dev-dependency here. `format: decimal` is not a registered format and a
    // conforming validator ignores it, so `pattern` is the only thing standing between this schema
    // and `1e3` arriving as an amount of money — and a pattern needs a regex engine, which a
    // hand-written checker does not have.
    let artifacts = artifacts(&billing());
    let validator = validator_for(&document(
        &artifacts,
        "schema/types/billing.invoice.Money.schema.json",
    ));

    for exact in ["12.50", "-12.50", "0", "0.01"] {
        accepts(
            &validator,
            &json!({ "amount": exact, "currency": "EUR" }),
            &format!("`{exact}` is an exact decimal"),
        );
    }
    // One value, one spelling: an exponent, a leading zero, a bare point, a comma and a currency
    // sign are each a second way to write an amount, and two systems agreeing on the schema while
    // disagreeing on equality is what `DECIMAL_PATTERN` exists to prevent.
    for spelling in [
        "1e3",
        "012.50",
        "12.",
        ".5",
        "12,50",
        "+12.50",
        "\u{20ac}12.50",
        "",
    ] {
        refuses(
            &validator,
            &json!({ "amount": spelling, "currency": "EUR" }),
            &format!("`{spelling}` is not how this projection spells a decimal"),
        );
    }
}

#[test]
fn an_invariant_travels_with_the_type_and_says_it_is_not_a_constraint() {
    // `amount >= 0` is a predicate over a `Decimal`, and this projection writes a `Decimal` as a
    // string, so `minimum` cannot express it. Published verbatim and visibly unchecked beats a
    // translator that renders the clauses it understands and drops the rest.
    let artifacts = artifacts(&billing());
    let money = document(&artifacts, "schema/types/billing.invoice.Money.schema.json");

    assert_eq!(
        money["$defs"]["billing.invoice.Money"]["x-ess-invariants"],
        json!(["amount >= 0"])
    );
}

#[test]
fn a_tagged_union_round_trips_because_every_branch_pins_its_tag() {
    // The model offers no untagged union, because an untagged one cannot be decoded without
    // guessing. `Payee`'s two branches are both a `String` on the wire, so without the tag a
    // decoder would have exactly the ambiguity the model exists to refuse.
    let artifacts = artifacts(&billing());
    let payee = document(&artifacts, "schema/types/billing.invoice.Payee.schema.json");
    let branches = payee["$defs"]["billing.invoice.Payee"]["oneOf"]
        .as_array()
        .expect("a union is a choice of branches");
    assert_eq!(branches.len(), 2);
    for branch in branches {
        assert!(
            branch["properties"]["kind"]["const"].is_string(),
            "a branch that does not pin its tag is a branch a decoder has to guess"
        );
    }

    let validator = validator_for(&payee);
    accepts(
        &validator,
        &json!({ "kind": "person", "value": "someone@example.com" }),
        "a person payee is an email address under the `person` tag",
    );
    accepts(
        &validator,
        &json!({ "kind": "company", "value": "acme-gmbh" }),
        "a company payee is a company reference under the `company` tag",
    );
    refuses(
        &validator,
        &json!({ "value": "someone@example.com" }),
        "an untagged value is the one thing the model does not permit",
    );
    refuses(
        &validator,
        &json!({ "kind": "partnership", "value": "acme-gmbh" }),
        "a tag no variant declares matches no branch",
    );
    refuses(
        &validator,
        &json!({ "kind": "person", "value": "someone@example.com", "extra": true }),
        "a branch carries its tag and its value and nothing else",
    );
}

#[test]
fn an_error_that_carries_nothing_accepts_an_empty_object_and_nothing_else() {
    let artifacts = artifacts(&billing());
    let validator = validator_for(&document(
        &artifacts,
        "schema/errors/billing.email.Undeliverable.schema.json",
    ));

    accepts(
        &validator,
        &json!({}),
        "`Undeliverable` carries nothing beyond its name",
    );
    refuses(
        &validator,
        &json!({ "reason": "mailbox full" }),
        "a payload the specification does not declare is not this error",
    );
}

#[test]
fn an_event_payload_accepts_what_the_specification_says_it_carries() {
    let artifacts = artifacts(&billing());
    let validator = validator_for(&document(
        &artifacts,
        "schema/events/billing.invoice.InvoiceCreated.schema.json",
    ));

    accepts(
        &validator,
        &json!({
            "invoice_id": "8ba7b810-9dad-11d1-80b4-00c04fd430c8",
            "customer_email": "someone@example.com",
            "amount": { "amount": "12.50", "currency": "EUR" },
        }),
        "this is the fact `InvoiceCreated` states",
    );
    refuses(
        &validator,
        &json!({
            "invoice_id": "8ba7b810-9dad-11d1-80b4-00c04fd430c8",
            "customer_email": "someone@example.com",
        }),
        "an event missing a field it declares is not that event",
    );
}

/// A specification exercising what the billing example's messages happen not to reach.
const EVERY_SHAPE: &str = r"
format: ess/1
system: shapes
version: v1
domain: shapes.core

types:
  - name: shapes.core.Label
    kind: newtype
    of: String

commands:
  - name: shapes.core.Record
    input:
      - name: label
        type: shapes.core.Label
        wire: tag
        display: Label
        summary: What this record is filed under.
      - name: note
        type: Optional<String>
      - name: labels
        type: List<shapes.core.Label>
      - name: metadata
        type: Map<String, String>
      - name: counts
        type: Map<Integer, Integer>
      - name: maybe_each
        type: List<Optional<String>>
      - name: at
        type: Timestamp
      - name: window
        type: Duration
      - name: signature
        type: Bytes
      - name: recurring
        type: Boolean
    outcomes:
      - name: recorded
        emits:
          - shapes.core.Recorded

events:
  - name: shapes.core.Recorded
    fields:
      - name: at
        type: Timestamp
";

/// An instance of `shapes.core.Record` with every field filled in, and every one of them the
/// spelling the projection publishes.
///
/// Shared, because the tests below each break exactly one field: a copy per test would have let the
/// copies drift, and a test whose *baseline* is refused proves nothing about the field it changed.
fn record() -> Value {
    json!({
        "tag": "urgent",
        "note": "a note",
        "labels": ["urgent"],
        "metadata": { "source": "portal" },
        "counts": { "-3": 4 },
        "maybe_each": ["one", null],
        "at": "2026-08-20T09:00:00Z",
        "window": "P30D",
        "signature": "3q2+7w==",
        "recurring": true,
    })
}

#[test]
fn an_optional_field_may_be_absent_and_a_required_field_may_not() {
    // Both directions, because getting one right proves nothing: a schema that requires nothing
    // accepts an absent optional too.
    let ir = compiled(EVERY_SHAPE);
    let artifacts = artifacts(&ir);
    let input = document(&artifacts, "schema/commands/shapes.core.Record.schema.json");

    let required = input["required"]
        .as_array()
        .expect("some fields are required");
    assert!(
        !required.contains(&json!("note")),
        "`Optional<String>` is not a required field"
    );
    assert!(
        required.contains(&json!("labels")),
        "`List<Label>` is required, since the model has no way to say it may be missing"
    );

    let filled = record();
    let validator = validator_for(&input);
    accepts(&validator, &filled, "every field is filled in");

    let mut without_note = filled.clone();
    without_note
        .as_object_mut()
        .expect("an object")
        .remove("note");
    accepts(
        &validator,
        &without_note,
        "an absent optional field is absent",
    );

    let mut null_note = filled.clone();
    null_note["note"] = Value::Null;
    refuses(
        &validator,
        &null_note,
        "`Optional` means absent; an explicit null would be a second spelling of one fact",
    );

    let mut without_labels = filled.clone();
    without_labels
        .as_object_mut()
        .expect("an object")
        .remove("labels");
    refuses(
        &validator,
        &without_labels,
        "a required field may not be absent",
    );
}

#[test]
fn a_list_element_may_be_null_where_a_field_may_only_be_absent() {
    // The split the mapping has to make: a missing object property is a thing JSON can say, and a
    // missing array element is not, so `List<Optional<String>>` is the one place a null appears.
    let artifacts = artifacts(&compiled(EVERY_SHAPE));
    let input = document(&artifacts, "schema/commands/shapes.core.Record.schema.json");
    let branches = input["properties"]["maybe_each"]["items"]["anyOf"]
        .as_array()
        .expect("an optional element is a choice");

    assert_eq!(branches.len(), 2);
    assert_eq!(branches[1]["type"].as_str(), Some("null"));
}

#[test]
fn a_map_is_an_object_whose_keys_are_the_text_its_key_type_is_spelt_with() {
    // An object rather than an array of pairs: the model restricts a key to a primitive exactly so
    // the map has this wire form, and an array of pairs would carry a non-string key faithfully
    // while throwing away key uniqueness, which nothing then checks.
    let artifacts = artifacts(&compiled(EVERY_SHAPE));
    let input = document(&artifacts, "schema/commands/shapes.core.Record.schema.json");

    let counts = &input["properties"]["counts"];
    assert_eq!(counts["type"].as_str(), Some("object"));
    assert_eq!(
        counts["additionalProperties"]["type"].as_str(),
        Some("integer")
    );
    assert_eq!(
        counts["propertyNames"]["pattern"].as_str(),
        Some(r"^-?(0|[1-9][0-9]*)$"),
        "an integer-keyed map says how its keys are spelt"
    );

    refuses(
        &validator_for(&input),
        &json!({
            "tag": "urgent",
            "labels": [],
            "metadata": {},
            "counts": { "-3": "four" },
            "maybe_each": [],
            "at": "2026-08-20T09:00:00Z",
            "window": "P30D",
            "signature": "",
            "recurring": false,
        }),
        "a map's values all have the type the model gave them",
    );
}

#[test]
fn a_map_key_that_is_not_the_text_its_key_type_is_spelt_with_is_refused() {
    // `propertyNames` was inside the keyword set the old structural checker walked, but the
    // `pattern` inside `propertyNames` was not — so an `Integer`-keyed map accepted `"three"` as a
    // key, which is exactly what a producer whose language stringifies map keys its own way sends.
    let artifacts = artifacts(&compiled(EVERY_SHAPE));
    let validator = validator_for(&document(
        &artifacts,
        "schema/commands/shapes.core.Record.schema.json",
    ));

    accepts(
        &validator,
        &record(),
        "an integer-keyed map keyed by integers is the baseline",
    );
    // `-0` is deliberately absent: `INTEGER_TEXT_PATTERN` accepts it, so it is a second spelling of
    // `0` that the published pattern permits. That is the generator's decision to change, not this
    // test's to contradict.
    for keys in [json!({ "three": 4 }), json!({ "3.0": 4 }), json!({ "": 4 })] {
        let mut instance = record();
        instance["counts"] = keys.clone();
        refuses(
            &validator,
            &instance,
            &format!("{keys} is not how `Map<Integer, Integer>` spells its keys"),
        );
    }
}

#[test]
fn a_timestamp_and_a_duration_publish_a_format_and_no_pattern_they_could_be_wrong_about() {
    // A hand-written RFC 3339 or ISO 8601 regex that is subtly too strict rejects data the
    // specification permits, which is the failure `published.rs` exists to prevent. A consumer that
    // ignores `format` sees any string, which is the milder problem.
    let artifacts = artifacts(&compiled(EVERY_SHAPE));
    let input = document(&artifacts, "schema/commands/shapes.core.Record.schema.json");

    for (field, format) in [("at", "date-time"), ("window", "duration")] {
        let property = &input["properties"][field];
        assert_eq!(property["type"].as_str(), Some("string"));
        assert_eq!(property["format"].as_str(), Some(format));
        assert!(
            property.get("pattern").is_none(),
            "{field} publishes a pattern nobody verified"
        );
    }

    let signature = &input["properties"]["signature"];
    assert_eq!(signature["contentEncoding"].as_str(), Some("base64"));
    assert!(
        signature.get("pattern").is_some(),
        "`contentEncoding` is an annotation, so the pattern is what refuses a non-base64 string"
    );
}

#[test]
fn a_bytes_field_refuses_a_string_that_is_not_base64() {
    // `contentEncoding` is an annotation in 2020-12: a conforming validator does not decode, so it
    // refuses nothing on its own. The test above asserts the pattern is published beside it; this
    // one asserts the pattern is what does the work.
    let artifacts = artifacts(&compiled(EVERY_SHAPE));
    let validator = validator_for(&document(
        &artifacts,
        "schema/commands/shapes.core.Record.schema.json",
    ));

    for text in ["not base64!", "3q2+7w=", "3q2+7w===", "3q2+7", "3q2 7w=="] {
        let mut instance = record();
        instance["signature"] = json!(text);
        refuses(
            &validator,
            &instance,
            &format!("`{text}` is not base64, and `contentEncoding` alone would have let it past"),
        );
    }
}

#[test]
fn every_schema_says_which_specification_it_came_from() {
    // Design §10. JSON has no comments, and `$comment` is discardable by a conforming
    // implementation, so provenance goes in a keyword a drift check can read without parsing prose.
    let ir = billing();
    let provenance = ess_gen::Provenance::of(&ir);
    for (path, artifact) in artifacts(&ir) {
        let parsed: Value = serde_json::from_str(&artifact.contents).expect("is JSON");
        let attribution = &parsed["x-ess-provenance"];
        assert_eq!(attribution["system"].as_str(), Some("billing"), "{path}");
        assert_eq!(
            attribution["source_digest"].as_str(),
            Some(provenance.source_digest.as_str()),
            "{path} claims a different model"
        );
        assert_eq!(
            attribution["specification_version"].as_str(),
            Some("v3"),
            "{path}"
        );
        assert!(
            attribution["regenerate"].is_string(),
            "{path} does not say how to reproduce itself"
        );
    }
}

#[test]
fn generation_is_byte_identical_between_runs() {
    // Determinism is claimed by three rules — no clock, no RNG, ordered collections only — and none
    // of them is checkable by reading.
    let ir = billing();
    let first = artifacts(&ir);
    let second = artifacts(&ir);
    assert_eq!(
        first, second,
        "schema generation must be byte-identical between runs"
    );

    // And between two compilations of the same source, which is where a digest or a map iteration
    // order would show up.
    let again = artifacts(&billing());
    assert_eq!(
        first, again,
        "two compilations of one specification must project to the same bytes"
    );
}

#[test]
fn no_schema_uses_a_keyword_outside_the_set_this_projection_publishes() {
    // What a consumer has to implement in order to read this output. A keyword appearing here that
    // is not in this list is either a new decision nobody wrote down or a typo in a keyword name,
    // and a typo'd keyword is silently ignored by every validator.
    let known = [
        "$defs",
        "$ref",
        "$schema",
        "additionalProperties",
        "anyOf",
        "const",
        "contentEncoding",
        "description",
        "enum",
        "format",
        "items",
        "oneOf",
        "pattern",
        "properties",
        "propertyNames",
        "required",
        "title",
        "type",
        "x-ess-invariants",
        "x-ess-kind",
        "x-ess-name",
        "x-ess-provenance",
        // The field a union's variant is named in, which the branches pin with `const` and a reader
        // otherwise has to infer from them (`src/types.rs`, `Node::ess_union_tag`).
        "x-ess-union-tag",
    ];

    let mut found = std::collections::BTreeSet::new();
    for artifact in artifacts(&billing()).values() {
        let parsed: Value = serde_json::from_str(&artifact.contents).expect("is JSON");
        keywords(&parsed, &mut found);
    }
    for keyword in &found {
        assert!(
            known.contains(&keyword.as_str()),
            "`{keyword}` is emitted but undeclared; found {found:?}"
        );
    }
}

#[test]
fn a_field_is_called_what_the_specification_says_it_is_called_on_the_wire() {
    // `naming.wire` is in the IR so that no projection has to re-read the source for it. A schema
    // keyed on the model's field name instead would refuse every message a producer actually sends.
    let artifacts = artifacts(&compiled(EVERY_SHAPE));
    let input = document(&artifacts, "schema/commands/shapes.core.Record.schema.json");
    let properties = input["properties"].as_object().expect("an object");

    assert!(properties.contains_key("tag"), "the wire name is the key");
    assert!(
        !properties.contains_key("label"),
        "the model's own name is not on the wire when the specification renames it"
    );
    assert!(input["required"]
        .as_array()
        .expect("some fields are required")
        .contains(&json!("tag")));
}

#[test]
fn a_field_carries_its_own_words_beside_the_reference_to_its_type() {
    // The second reason this projection is 2020-12 rather than draft-07, and the one the type
    // documents depend on: draft-07 discards every keyword sitting next to a `$ref`, so a field's
    // summary would vanish — and a type document, whose root *is* a `$ref`, would lose its `$defs`
    // and stop resolving at all.
    let artifacts = artifacts(&compiled(EVERY_SHAPE));
    let input = document(&artifacts, "schema/commands/shapes.core.Record.schema.json");
    let property = &input["properties"]["tag"];

    assert_eq!(property["$ref"].as_str(), Some("#/$defs/shapes.core.Label"));
    assert_eq!(property["title"].as_str(), Some("Label"));
    assert_eq!(
        property["description"].as_str(),
        Some("What this record is filed under.")
    );
}

#[test]
fn every_message_accepts_an_instance_of_itself_and_refuses_one_that_is_wrong() {
    // One row per command input, event payload and error payload the billing example declares — the
    // whole set, because a projection is only as good as its worst schema, and the schema nobody
    // wrote an instance for is the one that accepts everything.
    let artifacts = artifacts(&billing());
    let invoice = json!({ "amount": "12.50", "currency": "EUR" });
    let uuid = "8ba7b810-9dad-11d1-80b4-00c04fd430c8";

    for (path, accepted, refused, why) in [
        (
            "schema/commands/billing.email.SendEmail.schema.json",
            json!({ "recipient": "someone@example.com", "template": "invoice-created" }),
            json!({ "recipient": "someone@example.com" }),
            "`SendEmail` needs both a recipient and a template",
        ),
        (
            "schema/commands/billing.invoice.CreateInvoice.schema.json",
            json!({ "customer_email": "someone@example.com", "amount": invoice.clone() }),
            json!({ "customer_email": "someone@example.com", "amount": "12.50" }),
            "`Money` is a struct, not the amount on its own",
        ),
        (
            "schema/events/billing.email.EmailSent.schema.json",
            json!({ "message_id": uuid, "recipient": "someone@example.com" }),
            json!({ "message_id": uuid, "recipient": "someone@example.com", "provider": "smtp" }),
            "an event carries what it declares and nothing more",
        ),
        (
            "schema/events/billing.invoice.InvoiceCreated.schema.json",
            json!({
                "invoice_id": uuid,
                "customer_email": "someone@example.com",
                "amount": invoice.clone(),
            }),
            json!({
                "invoice_id": uuid,
                "customer_email": "someone@example.com",
                "amount": { "amount": "12.50", "currency": 978 },
            }),
            "a currency is text in this model, and a numeric code is not it",
        ),
        (
            "schema/errors/billing.email.Undeliverable.schema.json",
            json!({}),
            json!({ "detail": "mailbox full" }),
            "`Undeliverable` carries nothing beyond its name",
        ),
        (
            "schema/errors/billing.invoice.InvalidAmount.schema.json",
            json!({ "submitted": invoice.clone() }),
            json!({}),
            "the caller cannot react to a refusal that does not say what was submitted",
        ),
    ] {
        let validator = validator_for(&document(&artifacts, path));
        accepts(
            &validator,
            &accepted,
            &format!("{path} describes this message"),
        );
        refuses(&validator, &refused, why);
    }
}
