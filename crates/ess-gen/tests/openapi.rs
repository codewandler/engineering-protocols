//! What the `OpenAPI` projection promises, asserted against the normative example and against
//! specifications written to reach the corners the example does not.
//!
//! Two fixtures, deliberately. `examples/billing/` is the model's own example and the thing a reader
//! checks the generator against, but its commands only carry a newtype and a struct — the enum, the
//! tagged union, the list, the map and the optional live on an *entity*, which has no HTTP surface
//! because a command is the only thing this projection exposes. A generator silently dropping unions
//! would pass every billing assertion, so the type mapping is exercised by a specification built for
//! it.
//!
//! # What "valid" means in here, exactly
//!
//! Two different strengths, and the difference is not an accident. The **schemas** a document embeds
//! are validated against the JSON Schema 2020-12 meta-schema — the real one, bundled inside
//! `jsonschema`, which is a dev-dependency of this crate — because `OpenAPI` 3.1's schema dialect
//! *is* 2020-12. The **envelope** around them is checked by hand, in [`assert_valid`], because the
//! `OpenAPI` 3.1 meta-schema is a third-party document nothing in this repository holds and no test
//! here may fetch. [`assert_valid`] says what that costs and what the open decision is.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use ess_compiler::ir::EssIr;
use ess_compiler::resolve::compile;
use ess_compiler::source::SourceMap;
use ess_domain::spec::{RawSpecFile, Specification};
use ess_domain::system::Source;
use ess_gen::openapi::OpenApi;
use ess_gen::{Artifact, Provenance};
use serde_json::Value;

// ---------------------------------------------------------------------------------------------
// fixtures

/// The billing example's directory.
fn example() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/billing")
        .canonicalize()
        .expect("the billing example exists")
}

/// Every `.yaml` file in the billing example, in a stable order.
///
/// Discovered rather than listed, for the reason `ess-compiler`'s own test discovers them: a file
/// added to the example would otherwise be compiled by the CLI and ignored by the test.
fn billing_files() -> Vec<(String, String)> {
    let base = example();
    let mut found = Vec::new();
    let mut pending = vec![base.clone()];
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(&directory).expect("the example is readable") {
            let path = entry.expect("an entry").path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|it| it == "yaml") {
                let label = path
                    .strip_prefix(&base)
                    .expect("inside the example")
                    .display()
                    .to_string();
                let text = std::fs::read_to_string(&path).expect("readable");
                found.push((label, text));
            }
        }
    }
    found.sort();
    found
}

/// A specification, compiled.
fn compiled(files: &[(String, String)]) -> EssIr {
    let mut sources = SourceMap::new();
    let mut parsed = Vec::new();
    for (label, text) in files {
        let raw = RawSpecFile::parse(text)
            .unwrap_or_else(|error| panic!("{label} is well formed: {error}"));
        sources.insert(label.clone(), text.clone());
        parsed.push((Source::new(label.clone()), raw));
    }
    let specification = Specification::assemble(parsed)
        .unwrap_or_else(|errors| panic!("the specification validates:\n{errors}"));
    compile(&specification, &sources)
        .unwrap_or_else(|diagnostics| panic!("the specification resolves:\n{diagnostics}"))
}

/// The billing example, compiled.
fn billing() -> EssIr {
    compiled(&billing_files())
}

/// One inline specification, compiled.
fn inline(files: &[(&str, &str)]) -> EssIr {
    let owned: Vec<(String, String)> = files
        .iter()
        .map(|(label, text)| ((*label).to_owned(), (*text).to_owned()))
        .collect();
    compiled(&owned)
}

/// Every artifact the projection produces, keyed by path.
fn generated(ir: &EssIr) -> BTreeMap<String, Artifact> {
    ess_gen::artifact::run(&OpenApi, ir).expect("no two documents claim one path")
}

/// One document, parsed back from the bytes that were written.
///
/// Through JSON rather than as a `serde_yaml::Value`, so that the same walk works on the schemas —
/// and because "it parses" is only worth asserting when the assertions afterwards read the result.
fn parsed(artifact: &Artifact) -> Value {
    let yaml: serde_yaml::Value = serde_yaml::from_str(&artifact.contents)
        .unwrap_or_else(|error| panic!("{} is YAML: {error}", artifact.path));
    serde_json::to_value(yaml).expect("YAML converts to JSON")
}

/// Every document of a compiled specification, keyed by artifact path.
fn documents(ir: &EssIr) -> BTreeMap<String, Value> {
    generated(ir)
        .iter()
        .map(|(path, artifact)| (path.clone(), parsed(artifact)))
        .collect()
}

/// One document, by the component it belongs to.
fn document(ir: &EssIr, component: &str) -> Value {
    let path = format!("openapi/{component}.yaml");
    documents(ir)
        .remove(&path)
        .unwrap_or_else(|| panic!("a document at {path}"))
}

/// Every `$ref` string anywhere in a value.
fn references(value: &Value, found: &mut BTreeSet<String>) {
    match value {
        Value::Object(entries) => {
            for (key, child) in entries {
                if key == "$ref" {
                    if let Some(text) = child.as_str() {
                        found.insert(text.to_owned());
                    }
                } else {
                    references(child, found);
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                references(item, found);
            }
        }
        _ => {}
    }
}

/// Everything `OpenAPI` 3.1 requires of a document, checked structurally.
///
/// This stands in for validating the *envelope* against the `OpenAPI` 3.1 meta-schema, which is a
/// third-party document this repository does not hold. The dependency is not what is missing:
/// `jsonschema` is a dev-dependency of this crate, and the schemas a document embeds are validated
/// against the real JSON Schema 2020-12 meta-schema by
/// `every_schema_a_document_embeds_is_valid_in_the_dialect_openapi_31_declares`, because the
/// validator bundles that meta-schema. What is missing is `openapi-3.1.json` itself, which would
/// have to be vendored: a test must not fetch it, and nothing here could — the dependency is built
/// with `default-features = false`, so it has no retriever at all.
///
/// What this function checks is the part that actually breaks, enumerated by hand: a response with no
/// `description`, a status key that is not a status, a `requestBody` with no content, a parameter with
/// no location. What the meta-schema would add is the whole class instead of the enumerated members —
/// a keyword whose value is the wrong type, a keyword at the wrong nesting level, a `servers` entry
/// with no `url`, a `tags` entry with no `name`, a misspelling of any key nobody thought to list
/// here. So `docs/plan/ess-roadmap.md` § W3.2's "validated against their own schemas" holds for the
/// embedded schemas and **not** for the envelope. Vendoring the meta-schema is an open decision, not
/// an oversight.
fn assert_valid(label: &str, document: &Value) {
    assert_eq!(document["openapi"], "3.1.0", "{label}");
    let info = document["info"]
        .as_object()
        .unwrap_or_else(|| panic!("{label}: info"));
    for required in ["title", "version"] {
        assert!(
            info.get(required).is_some_and(Value::is_string),
            "{label}: info.{required} is a required string"
        );
    }
    let paths = document["paths"]
        .as_object()
        .unwrap_or_else(|| panic!("{label}: paths"));
    for (path, item) in paths {
        assert!(path.starts_with('/'), "{label}: `{path}` is not a path");
        for (method, operation) in item
            .as_object()
            .unwrap_or_else(|| panic!("{label}: {path}"))
        {
            let at = format!("{label}: {method} {path}");
            assert!(operation["operationId"].is_string(), "{at}: an operationId");
            for parameter in operation
                .get("parameters")
                .and_then(Value::as_array)
                .unwrap_or(&Vec::new())
            {
                assert!(parameter["name"].is_string(), "{at}: a parameter name");
                assert!(
                    ["header", "query", "path", "cookie"]
                        .contains(&parameter["in"].as_str().unwrap_or_default()),
                    "{at}: a parameter location"
                );
                assert!(parameter["schema"].is_object(), "{at}: a parameter schema");
            }
            if let Some(body) = operation.get("requestBody") {
                assert!(
                    body["content"]["application/json"]["schema"].is_object(),
                    "{at}: a request body with a schema"
                );
            }
            let responses = operation["responses"]
                .as_object()
                .unwrap_or_else(|| panic!("{at}: responses"));
            assert!(!responses.is_empty(), "{at}: at least one response");
            for (status, response) in responses {
                assert!(
                    status.len() == 3 && status.chars().all(|digit| digit.is_ascii_digit()),
                    "{at}: `{status}` is not a status code"
                );
                assert!(
                    response["description"]
                        .as_str()
                        .is_some_and(|text| !text.is_empty()),
                    "{at}: {status} needs a description, which OpenAPI requires"
                );
            }
        }
    }
}

/// A document's operations, keyed by `"{METHOD} {path}"`.
fn operations(document: &Value) -> BTreeMap<String, &Value> {
    let mut out = BTreeMap::new();
    let paths = document["paths"].as_object().expect("a paths object");
    for (path, item) in paths {
        for (method, operation) in item.as_object().expect("a path item") {
            out.insert(format!("{} {path}", method.to_uppercase()), operation);
        }
    }
    out
}

// ---------------------------------------------------------------------------------------------
// the shape of the output

#[test]
fn every_component_gets_one_document_named_after_it() {
    let ir = billing();
    let paths: Vec<String> = generated(&ir).keys().cloned().collect();

    assert_eq!(
        paths,
        vec![
            "openapi/email-service.yaml".to_owned(),
            "openapi/invoice-service.yaml".to_owned(),
        ],
        "a component is the unit of ownership, so it is the unit that has an API"
    );
}

#[test]
fn a_document_is_valid_yaml_with_a_version_an_info_block_and_paths() {
    let ir = billing();

    for (path, document) in documents(&ir) {
        assert_eq!(document["openapi"], "3.1.0", "{path}");
        assert!(document["info"]["title"].is_string(), "{path}");
        assert_eq!(document["info"]["version"], "v3", "{path}");
        assert!(
            document["info"]["description"]
                .as_str()
                .is_some_and(|text| !text.is_empty()),
            "{path}"
        );
        assert!(document["paths"].is_object(), "{path}");
        assert!(document["components"]["schemas"].is_object(), "{path}");
    }
}

#[test]
fn every_reference_resolves_inside_the_document_that_makes_it() {
    // The property that makes each document usable on its own. An external `$ref` would couple this
    // projection to the schema projection's directory layout, and nothing would check the coupling.
    let ir = billing();

    for (path, document) in documents(&ir) {
        let schemas = document["components"]["schemas"]
            .as_object()
            .expect("schemas");
        let mut found = BTreeSet::new();
        references(&document, &mut found);
        assert!(!found.is_empty(), "{path} makes no references at all");
        for reference in found {
            let key = reference
                .strip_prefix("#/components/schemas/")
                .unwrap_or_else(|| panic!("{path}: `{reference}` leaves the document"));
            assert!(
                schemas.contains_key(key),
                "{path}: `{reference}` points at nothing"
            );
        }
    }
}

#[test]
fn every_schema_the_document_declares_is_pointed_at_by_something() {
    // The other direction: a schema nothing references is a type this component's surface does not
    // actually use, and emitting it would make each document a copy of the whole type registry.
    let ir = billing();

    for (path, document) in documents(&ir) {
        let mut found = BTreeSet::new();
        references(&document, &mut found);
        let reached: BTreeSet<&str> = found
            .iter()
            .filter_map(|reference| reference.strip_prefix("#/components/schemas/"))
            .collect();
        for key in document["components"]["schemas"]
            .as_object()
            .expect("schemas")
            .keys()
        {
            assert!(
                reached.contains(key.as_str()),
                "{path}: `{key}` is emitted and referenced by nothing"
            );
        }
    }
}

#[test]
fn regenerating_from_the_same_ir_produces_the_same_bytes() {
    // Review F8: determinism claimed is determinism unasserted. Two independent IRs of the same
    // source, generated separately, compared byte for byte.
    let first = generated(&billing());
    let second = generated(&billing());

    assert_eq!(first, second);
    for (path, artifact) in &first {
        assert_eq!(
            artifact.contents, second[path].contents,
            "{path} differs between two runs"
        );
    }
}

#[test]
fn every_document_carries_its_provenance_as_a_comment_and_as_data() {
    // Design §10. Twice on purpose: the comment is for the person who opened the file, the extension
    // is for the tool that reparsed it and dropped the comments.
    let ir = billing();
    let provenance = Provenance::of(&ir);

    for (path, artifact) in generated(&ir) {
        assert!(
            artifact.contents.starts_with("# generated from billing v3"),
            "{path} does not open with its provenance"
        );
        assert!(
            artifact
                .contents
                .contains(&format!("# model digest {}", provenance.source_digest)),
            "{path}"
        );
        let document = parsed(&artifact);
        let carried = &document["info"]["x-ess-provenance"];
        assert_eq!(carried["system"], "billing", "{path}");
        assert_eq!(carried["specification_version"], "v3", "{path}");
        assert_eq!(carried["source_digest"], provenance.source_digest, "{path}");
        assert_eq!(carried["generator_version"], Provenance::VERSION, "{path}");
    }
}

#[test]
fn every_document_this_generator_can_produce_is_a_valid_openapi_document() {
    // Over every fixture rather than only over billing: the corner cases are exactly where a
    // generator emits something that looks like a document and is not one.
    for (label, ir) in [
        ("billing", billing()),
        ("colliding", inline(COLLIDING)),
        ("unaccepted", inline(UNACCEPTED)),
        ("silent", inline(SILENT)),
        ("two-accepted", inline(TWO_ACCEPTED)),
        ("every-type", inline(EVERY_TYPE)),
        ("no-input", inline(NO_INPUT)),
    ] {
        for (path, document) in documents(&ir) {
            assert_valid(&format!("{label} {path}"), &document);
        }
    }
}

#[test]
fn every_schema_a_document_embeds_is_valid_in_the_dialect_openapi_31_declares() {
    // The half of `assert_valid`'s gap that *can* be closed here. `OpenAPI` 3.1's schema dialect is
    // JSON Schema 2020-12, and `jsonschema` bundles that meta-schema, so every fragment under
    // `components.schemas` and in every `schema` position is checked against the real thing — with
    // no retriever configured, so nothing is fetched. The envelope around them is still checked by
    // hand; see `assert_valid`.
    let mut checked = 0_usize;
    for (label, ir) in [
        ("billing", billing()),
        ("colliding", inline(COLLIDING)),
        ("unaccepted", inline(UNACCEPTED)),
        ("silent", inline(SILENT)),
        ("two-accepted", inline(TWO_ACCEPTED)),
        ("every-type", inline(EVERY_TYPE)),
        ("no-input", inline(NO_INPUT)),
    ] {
        for (path, document) in documents(&ir) {
            let found = schemas(&document);
            // A walk that finds nothing asserts nothing. Every document that exposes an operation
            // embeds a schema; one that exposes none is a component accepting no command, which
            // `a_component_that_accepts_nothing_still_gets_a_document` is about.
            assert_eq!(
                found.is_empty(),
                document["paths"]
                    .as_object()
                    .is_none_or(serde_json::Map::is_empty),
                "{label} {path}: {} operations and {} schemas",
                document["paths"]
                    .as_object()
                    .map_or(0, serde_json::Map::len),
                found.len()
            );
            for (at, schema) in found {
                jsonschema::draft202012::meta::validate(schema).unwrap_or_else(|error| {
                    panic!("{label} {path}: {at} is not a valid 2020-12 schema: {error}")
                });
                checked += 1;
            }
        }
    }
    assert!(checked > 0, "no schema was checked at all");
}

/// Every schema-position value in a document, with the path it sits at.
///
/// `components.schemas` plus the inline schemas: a parameter's, a request body's and a response
/// body's. Listed rather than discovered by walking every `schema` key, because a `properties` entry
/// *named* `schema` would otherwise be validated as one.
fn schemas(document: &Value) -> Vec<(String, &Value)> {
    let mut found = Vec::new();
    for (name, schema) in document["components"]["schemas"]
        .as_object()
        .into_iter()
        .flatten()
    {
        found.push((format!("components.schemas.{name}"), schema));
    }
    for (path, item) in document["paths"].as_object().into_iter().flatten() {
        for (method, operation) in item.as_object().into_iter().flatten() {
            let at = format!("{method} {path}");
            for parameter in operation
                .get("parameters")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                if let Some(schema) = parameter.get("schema") {
                    found.push((format!("{at} parameter {}", parameter["name"]), schema));
                }
            }
            if let Some(schema) = operation
                .get("requestBody")
                .map(|body| &body["content"]["application/json"]["schema"])
                .filter(|schema| schema.is_object())
            {
                found.push((format!("{at} requestBody"), schema));
            }
            for (status, response) in operation["responses"].as_object().into_iter().flatten() {
                let schema = &response["content"]["application/json"]["schema"];
                if schema.is_object() {
                    found.push((format!("{at} {status}"), schema));
                }
            }
        }
    }
    found
}

// ---------------------------------------------------------------------------------------------
// a command is not a resource

#[test]
fn a_command_is_exposed_at_its_wire_name_under_its_domains() {
    let ir = billing();

    assert_eq!(
        operations(&document(&ir, "invoice-service"))
            .keys()
            .cloned()
            .collect::<Vec<String>>(),
        vec!["POST /invoices/commands/create-invoice".to_owned()],
        "`invoices` is the domain's wire name and `create-invoice` is the command's"
    );
}

#[test]
fn a_command_with_no_wire_name_is_exposed_under_the_name_the_model_gives_it() {
    // `billing.email.SendEmail` declares no `naming.wire`, and the domain declares none either, so
    // both fall back to the qualified name's last segment — verbatim, capitals included. A generator
    // inventing `send-email` here would be the only thing in the repository that thinks that is this
    // command's wire name.
    let ir = billing();

    assert_eq!(
        operations(&document(&ir, "email-service"))
            .keys()
            .cloned()
            .collect::<Vec<String>>(),
        vec!["POST /email/commands/SendEmail".to_owned()],
    );
}

#[test]
fn the_operation_id_is_the_commands_qualified_name() {
    let ir = billing();
    let document = document(&ir, "invoice-service");
    let operations = operations(&document);
    let operation = operations["POST /invoices/commands/create-invoice"];

    assert_eq!(operation["operationId"], "billing.invoice.CreateInvoice");
}

#[test]
fn a_command_is_only_ever_a_post() {
    let ir = billing();

    for (path, document) in documents(&ir) {
        for item in document["paths"].as_object().expect("paths").values() {
            let methods: Vec<&String> = item.as_object().expect("a path item").keys().collect();
            assert_eq!(methods, vec!["post"], "{path}");
        }
    }
}

#[test]
fn two_commands_claiming_one_path_both_move_to_their_qualified_names() {
    // Two domains may share a wire name, and `Naming::wire` is free text the model does not
    // constrain. When the convention collides, *both* commands move: a path whose spelling depends
    // on which other commands happen to exist is a path that changes when an unrelated one is added.
    let ir = inline(COLLIDING);

    assert_eq!(
        operations(&document(&ir, "one-service"))
            .keys()
            .cloned()
            .collect::<Vec<String>>(),
        vec![
            "POST /commands/s.one.Ship".to_owned(),
            "POST /commands/s.two.Ship".to_owned(),
        ],
    );
}

#[test]
fn a_command_no_component_accepts_appears_in_no_document() {
    // No component answers for it, so it has no owner and therefore no API. Naming one would be
    // this generator inventing an owner the specification declined to name.
    let ir = inline(UNACCEPTED);
    let mut seen = BTreeSet::new();
    for document in documents(&ir).values() {
        for operation in operations(document).values() {
            seen.insert(operation["operationId"].as_str().expect("an id").to_owned());
        }
    }

    assert!(seen.contains("t.core.Accepted"));
    assert!(
        !seen.contains("t.core.Orphan"),
        "a command nobody accepts must not appear: {seen:?}"
    );
}

#[test]
fn a_component_that_accepts_nothing_still_gets_a_document() {
    // A missing file cannot be told apart from a generator that broke, so a component with no HTTP
    // surface says so in a document rather than by being absent.
    let ir = inline(SILENT);
    let document = document(&ir, "silent-service");

    assert!(operations(&document).is_empty());
    assert_eq!(document["openapi"], "3.1.0");
}

// ---------------------------------------------------------------------------------------------
// outcomes are the interesting part

#[test]
fn each_declared_outcome_is_its_own_response_and_no_status_is_invented() {
    let ir = billing();
    let document = document(&ir, "invoice-service");
    let operations = operations(&document);
    let responses = operations["POST /invoices/commands/create-invoice"]["responses"]
        .as_object()
        .expect("responses");

    assert_eq!(
        responses.keys().cloned().collect::<Vec<String>>(),
        vec!["202".to_owned(), "422".to_owned()],
        "two declared outcomes, two statuses, and nothing else"
    );
}

#[test]
fn a_refusal_the_input_decides_carries_the_declared_error_payload() {
    // The branch that matters: `rejected` names `InvalidAmount`, which carries the `Money` that was
    // submitted. A generator emitting only the accepted branch would have thrown this away.
    let ir = billing();
    let document = document(&ir, "invoice-service");
    let operations = operations(&document);
    let refused = &operations["POST /invoices/commands/create-invoice"]["responses"]["422"];
    let reference = refused["content"]["application/json"]["schema"]["$ref"]
        .as_str()
        .expect("a schema reference");

    assert_eq!(
        reference,
        "#/components/schemas/billing.invoice.CreateInvoice.rejected.Response"
    );

    let schemas = &document["components"]["schemas"];
    let body = &schemas["billing.invoice.CreateInvoice.rejected.Response"];
    assert_eq!(body["properties"]["outcome"]["const"], "rejected");
    assert_eq!(
        body["properties"]["error"]["const"],
        "billing.invoice.InvalidAmount"
    );
    assert_eq!(
        body["properties"]["payload"]["$ref"],
        "#/components/schemas/billing.invoice.InvalidAmount.Error"
    );
    assert_eq!(
        body["required"],
        serde_json::json!(["outcome", "error", "payload"])
    );

    let payload = &schemas["billing.invoice.InvalidAmount.Error"];
    assert_eq!(
        payload["properties"]["submitted"]["$ref"],
        "#/components/schemas/billing.invoice.Money"
    );
    assert_eq!(payload["required"], serde_json::json!(["submitted"]));
}

#[test]
fn an_outcome_that_emits_says_so_without_claiming_to_return_the_events() {
    let ir = billing();
    let document = document(&ir, "invoice-service");
    let accepted =
        &document["components"]["schemas"]["billing.invoice.CreateInvoice.accepted.Response"];
    let description = accepted["description"].as_str().expect("a description");

    assert!(
        description.contains("`InvoiceCreated`"),
        "the emitted event is named: {description}"
    );
    assert!(
        description.contains("Taken when `amount.amount > 0`"),
        "the condition is stated: {description}"
    );
    assert_eq!(
        accepted["properties"]
            .as_object()
            .expect("properties")
            .len(),
        1,
        "the body says which branch ran and nothing more"
    );
}

#[test]
fn an_external_outcome_is_an_upstream_failure_and_not_a_validation_refusal() {
    // `failed: external: the provider rejects the recipient address`. A 4xx would tell the caller to
    // fix input that was fine, and would tell every retry layer that retrying is pointless.
    let ir = billing();
    let document = document(&ir, "email-service");
    let operations = operations(&document);
    let responses = operations["POST /email/commands/SendEmail"]["responses"]
        .as_object()
        .expect("responses");

    assert_eq!(
        responses.keys().cloned().collect::<Vec<String>>(),
        vec!["202".to_owned(), "502".to_owned()],
    );

    let body = &document["components"]["schemas"]["billing.email.SendEmail.failed.Response"];
    assert_eq!(
        body["properties"]["error"]["const"],
        "billing.email.Undeliverable"
    );
    assert!(
        body["properties"].get("payload").is_none(),
        "`Undeliverable` declares no fields, so there is no payload to invent"
    );
    assert!(
        body["description"]
            .as_str()
            .expect("a description")
            .contains("Decided outside the request: the provider rejects the recipient address"),
        "the declared cause reaches the contract"
    );
}

#[test]
fn several_outcomes_on_one_status_stay_distinguishable() {
    // A status that collapsed two branches into one response would lose the branch, which is the
    // whole thing the model's `outcomes` exist to keep.
    let ir = inline(TWO_ACCEPTED);
    let document = document(&ir, "core-service");
    let operations = operations(&document);
    let taken = &operations["POST /core/commands/Store"]["responses"]["202"];
    let schema = &taken["content"]["application/json"]["schema"];

    assert_eq!(
        schema["oneOf"],
        serde_json::json!([
            {"$ref": "#/components/schemas/t.core.Store.created.Response"},
            {"$ref": "#/components/schemas/t.core.Store.unchanged.Response"},
        ]),
    );
    assert_eq!(schema["discriminator"]["propertyName"], "outcome");
    assert_eq!(
        schema["discriminator"]["mapping"]["created"],
        "#/components/schemas/t.core.Store.created.Response"
    );
}

// ---------------------------------------------------------------------------------------------
// idempotency comes from the bindings

#[test]
fn a_command_a_binding_delivers_at_least_once_requires_an_idempotency_key() {
    let ir = billing();
    let document = document(&ir, "email-service");
    let operations = operations(&document);
    let parameters = operations["POST /email/commands/SendEmail"]["parameters"]
        .as_array()
        .expect("parameters");

    assert_eq!(parameters.len(), 1);
    assert_eq!(parameters[0]["name"], "Idempotency-Key");
    assert_eq!(parameters[0]["in"], "header");
    assert_eq!(parameters[0]["required"], true);
    assert!(
        parameters[0]["description"]
            .as_str()
            .expect("a description")
            .contains("`notify-on-invoice-created`"),
        "the binding that imposed the obligation is named"
    );
}

#[test]
fn a_command_no_binding_invokes_carries_no_idempotency_header() {
    // Nothing in the specification says anyone may call `CreateInvoice` twice, so a key here would
    // be this generator inventing a delivery guarantee.
    let ir = billing();
    let document = document(&ir, "invoice-service");
    let operations = operations(&document);

    assert!(operations["POST /invoices/commands/create-invoice"]
        .get("parameters")
        .is_none());
}

// ---------------------------------------------------------------------------------------------
// a grant is not a security scheme

#[test]
fn a_command_names_the_actors_permitted_to_invoke_it_and_no_authentication_mechanism() {
    // `may:` is in the IR, and it answers *who may invoke this command*. `securitySchemes` answers
    // *how a caller proves it is that actor* — a token, a key, a flow, at some URL, issued by
    // somebody — and the model states none of that. So the grant is published as an annotation and
    // no scheme is invented: a client generated from this document implements no authentication this
    // specification never described.
    let ir = billing();
    let document = document(&ir, "invoice-service");
    let operations = operations(&document);

    assert_eq!(
        operations["POST /invoices/commands/create-invoice"]["x-ess-may-invoke"],
        serde_json::json!(["billing.invoice.Customer"]),
        "the specification says `billing.invoice.Customer` may invoke it, so the contract does too"
    );
    assert!(
        document.get("security").is_none(),
        "a `security` requirement would claim an authentication mechanism the model has not stated"
    );
    assert!(
        document["components"].get("securitySchemes").is_none(),
        "and so would a scheme to satisfy it"
    );
}

#[test]
fn a_command_no_actor_names_carries_no_grant_rather_than_a_grant_to_everybody() {
    // An empty list would read as "these are the actors: none", and an absent keyword reads as "the
    // specification does not say" — which is what is true. Neither one is a grant to anybody, and
    // the difference matters the day this becomes a `security` requirement.
    let ir = billing();
    let document = document(&ir, "email-service");
    let operations = operations(&document);

    assert!(
        operations["POST /email/commands/SendEmail"]
            .get("x-ess-may-invoke")
            .is_none(),
        "`billing.email` declares no actor, so nothing here says who may invoke `SendEmail`"
    );
}

// ---------------------------------------------------------------------------------------------
// the type mapping

#[test]
fn a_commands_input_becomes_a_closed_object_over_its_declared_fields() {
    let ir = billing();
    let document = document(&ir, "invoice-service");
    let operations = operations(&document);
    let body = &operations["POST /invoices/commands/create-invoice"]["requestBody"];

    assert_eq!(body["required"], true);
    assert_eq!(
        body["content"]["application/json"]["schema"]["$ref"],
        "#/components/schemas/billing.invoice.CreateInvoice.Input"
    );

    let input = &document["components"]["schemas"]["billing.invoice.CreateInvoice.Input"];
    assert_eq!(input["type"], "object");
    assert_eq!(input["additionalProperties"], false);
    assert_eq!(
        input["required"],
        serde_json::json!(["customer_email", "amount"])
    );
    assert_eq!(
        input["properties"]["customer_email"]["$ref"],
        "#/components/schemas/billing.invoice.Email"
    );
}

#[test]
fn a_newtype_stays_a_schema_of_its_own_rather_than_becoming_its_representation() {
    // `Email` is not `String`. Erasing the wrapper would make the generated contract agree that an
    // invoice's email and any other string are the same thing, which is the one claim the type
    // system exists to refuse.
    let ir = billing();
    let schemas = document(&ir, "invoice-service")["components"]["schemas"].clone();
    let email = &schemas["billing.invoice.Email"];

    assert_eq!(email["type"], "string");
    assert_eq!(email["title"], "Email");
    // The distinctness used to be a sentence this generator wrote into `description`. It is a
    // keyword now, for two reasons: a tool cannot read prose, and a `description` carrying both the
    // author's words and the generator's cannot be read as either. `Email` declares no summary, so
    // it publishes no description at all rather than one nobody wrote.
    assert_eq!(
        email["x-ess-name"], "billing.invoice.Email",
        "the document says which type this is, not merely what it is made of"
    );
    assert_eq!(
        email["x-ess-kind"], "newtype",
        "and that it is a type of its own over its representation, in a form a code generator can \
         act on"
    );
    assert!(
        email["description"].is_null(),
        "`description` is the author's words and nothing else; `Email` declares no summary"
    );
}

#[test]
fn a_decimal_is_a_string_because_a_json_number_is_a_float() {
    let ir = billing();
    let schemas = document(&ir, "invoice-service")["components"]["schemas"].clone();
    let money = &schemas["billing.invoice.Money"];

    assert_eq!(money["properties"]["amount"]["type"], "string");
    assert_eq!(money["properties"]["amount"]["format"], "decimal");
    assert!(money["properties"]["amount"]["pattern"].is_string());
    assert_eq!(
        money["x-ess-invariants"],
        serde_json::json!(["amount >= 0"]),
        "the invariant the author wrote reaches the contract — as a keyword a tool can read, \
         verbatim, rather than folded into a sentence anything checking it would have to parse. \
         It is an annotation and not an assertion because `amount >= 0` is a predicate over a \
         Decimal, which this mapping publishes as a string, so `minimum` cannot express it"
    );
}

#[test]
fn every_kind_of_type_the_model_has_projects_into_a_schema() {
    let ir = inline(EVERY_TYPE);
    let schemas = document(&ir, "core-service")["components"]["schemas"].clone();

    // A primitive of every kind, at a field position.
    let input = &schemas["t.core.Take.Input"];
    let properties = &input["properties"];
    assert_eq!(properties["text"]["type"], "string");
    assert_eq!(properties["flag"]["type"], "boolean");
    assert_eq!(properties["count"]["type"], "integer");
    assert_eq!(properties["moment"]["format"], "date-time");
    assert_eq!(properties["window"]["format"], "duration");
    assert_eq!(properties["identifier"]["format"], "uuid");
    assert_eq!(properties["blob"]["contentEncoding"], "base64");

    // A list, a map and an optional.
    assert_eq!(properties["many"]["type"], "array");
    assert_eq!(
        properties["many"]["items"]["$ref"],
        "#/components/schemas/t.core.Wrapped"
    );
    assert_eq!(properties["table"]["type"], "object");
    assert!(
        properties["table"].get("propertyNames").is_none(),
        "a `String` key constrains nothing, and a rule that checks nothing invites a reader to \
         believe something was checked"
    );
    assert_eq!(
        properties["table"]["additionalProperties"]["type"],
        "integer"
    );
    assert_eq!(
        properties["note"]["type"], "string",
        "an optional field describes the value that is there when it is there"
    );
    assert_eq!(
        input["required"],
        serde_json::json!([
            "text",
            "flag",
            "count",
            "moment",
            "window",
            "identifier",
            "blob",
            "many",
            "table",
            "shape",
            "choice",
            "nested"
        ]),
        "`note` is the only optional field, so it is the only one missing"
    );

    // An enum.
    assert_eq!(schemas["t.core.Choice"]["type"], "string");
    assert_eq!(
        schemas["t.core.Choice"]["enum"],
        serde_json::json!(["First", "Second"])
    );

    // A tagged union: adjacently tagged, because a variant may be a scalar and there is nowhere
    // inside it to put the tag.
    let shape = &schemas["t.core.Shape"];
    let variants = shape["oneOf"].as_array().expect("variants");
    assert_eq!(variants.len(), 2);
    assert_eq!(variants[0]["properties"]["kind"]["const"], "boxed");
    assert_eq!(
        variants[0]["properties"]["value"]["$ref"],
        "#/components/schemas/t.core.Wrapped"
    );
    assert_eq!(
        variants[0]["required"],
        serde_json::json!(["kind", "value"])
    );

    // An optional nested where there is no key to omit.
    assert_eq!(
        schemas["t.core.Nest"]["properties"]["maybe"]["items"]["anyOf"],
        serde_json::json!([{"type": "string"}, {"type": "null"}]),
        "inside a list there is no key to leave out, so absence has to be `null`"
    );
}

#[test]
fn a_map_with_a_non_string_key_says_the_key_is_still_a_string() {
    // A JSON object key is always a string. `{type: integer}` under `propertyNames` is a constraint
    // no document could ever satisfy.
    let ir = inline(EVERY_TYPE);
    let schemas = document(&ir, "core-service")["components"]["schemas"].clone();
    let keyed = &schemas["t.core.Nest"]["properties"]["keyed"];

    assert_eq!(keyed["propertyNames"]["type"], "string");
    assert_eq!(keyed["propertyNames"]["pattern"], r"^-?(0|[1-9][0-9]*)$");
}

#[test]
fn a_command_with_no_input_is_exposed_without_a_body() {
    let ir = inline(NO_INPUT);
    let document = document(&ir, "core-service");
    let operations = operations(&document);
    let operation = operations["POST /core/commands/Ping"];

    assert!(
        operation.get("requestBody").is_none(),
        "an empty object is a shape a client would construct for no reason"
    );
    assert!(operation["responses"]["202"].is_object());
}

// ---------------------------------------------------------------------------------------------
// specifications written for the corners the example does not reach
//
// Each is the smallest specification that reaches one corner. Inline rather than in
// `examples/`, because the billing example is normative and adding a `Shape` to it to satisfy a
// generator's test would make the example serve the test instead of the model.

/// Two domains with the same wire name, each declaring a command with the same wire name.
const COLLIDING: &[(&str, &str)] = &[
    (
        "system.yaml",
        r"
format: ess/1
system: s
version: v1
domains: [s.one, s.two]
components:
  - component: one-service
    owns:
      domains: [s.one, s.two]
    accepts:
      commands: [s.one.Ship, s.two.Ship]
    publishes:
      events: [s.one.Shipped, s.two.Shipped]
",
    ),
    (
        "one.yaml",
        r"
domain: s.one
naming:
  wire: parcels
commands:
  - name: s.one.Ship
    naming:
      wire: ship
    outcomes:
      - name: done
        emits: [s.one.Shipped]
events:
  - name: s.one.Shipped
    fields:
      - name: id
        type: String
",
    ),
    (
        "two.yaml",
        r"
domain: s.two
naming:
  wire: parcels
commands:
  - name: s.two.Ship
    naming:
      wire: ship
    outcomes:
      - name: done
        emits: [s.two.Shipped]
events:
  - name: s.two.Shipped
    fields:
      - name: id
        type: String
",
    ),
];

/// A command no component accepts, beside one a component does.
const UNACCEPTED: &[(&str, &str)] = &[(
    "core.yaml",
    r"
format: ess/1
system: t
version: v1
domains: [t.core]
domain: t.core
commands:
  - name: t.core.Accepted
    outcomes:
      - name: done
        emits: [t.core.Happened]
  - name: t.core.Orphan
    outcomes:
      - name: done
        emits: [t.core.Happened]
events:
  - name: t.core.Happened
    fields:
      - name: id
        type: String
components:
  - component: core-service
    owns:
      domains: [t.core]
    accepts:
      commands: [t.core.Accepted]
    publishes:
      events: [t.core.Happened]
",
)];

/// A component that owns a domain and accepts nothing.
const SILENT: &[(&str, &str)] = &[(
    "quiet.yaml",
    r"
format: ess/1
system: t
version: v1
domains: [t.quiet]
domain: t.quiet
types:
  - name: t.quiet.Token
    kind: newtype
    of: String
components:
  - component: silent-service
    owns:
      domains: [t.quiet]
",
)];

/// One command with two outcomes, neither of which is an error.
const TWO_ACCEPTED: &[(&str, &str)] = &[(
    "core.yaml",
    r"
format: ess/1
system: t
version: v1
domains: [t.core]
domain: t.core
commands:
  - name: t.core.Store
    input:
      - name: count
        type: Integer
    outcomes:
      - name: created
        when: count > 0
        emits: [t.core.Stored]
      - name: unchanged
        emits: [t.core.Skipped]
events:
  - name: t.core.Stored
    fields:
      - name: id
        type: String
  - name: t.core.Skipped
    fields:
      - name: id
        type: String
components:
  - component: core-service
    owns:
      domains: [t.core]
    accepts:
      commands: [t.core.Store]
    publishes:
      events: [t.core.Stored, t.core.Skipped]
",
)];

/// One command whose input reaches every kind of type the model has.
const EVERY_TYPE: &[(&str, &str)] = &[(
    "core.yaml",
    r"
format: ess/1
system: t
version: v1
domains: [t.core]
domain: t.core
types:
  - name: t.core.Wrapped
    kind: newtype
    of: String
  - name: t.core.Choice
    kind: enum
    variants: [First, Second]
  - name: t.core.Shape
    kind: union
    tag: kind
    variants:
      boxed: t.core.Wrapped
      plain: String
  - name: t.core.Nest
    kind: struct
    fields:
      - name: maybe
        type: List<Optional<String>>
      - name: keyed
        type: Map<Integer, String>
commands:
  - name: t.core.Take
    input:
      - name: text
        type: String
      - name: flag
        type: Boolean
      - name: count
        type: Integer
      - name: moment
        type: Timestamp
      - name: window
        type: Duration
      - name: identifier
        type: Uuid
      - name: blob
        type: Bytes
      - name: many
        type: List<t.core.Wrapped>
      - name: table
        type: Map<String, Integer>
      - name: shape
        type: t.core.Shape
      - name: choice
        type: t.core.Choice
      - name: nested
        type: t.core.Nest
      - name: note
        type: Optional<String>
    outcomes:
      - name: done
        emits: [t.core.Taken]
events:
  - name: t.core.Taken
    fields:
      - name: id
        type: t.core.Wrapped
components:
  - component: core-service
    owns:
      domains: [t.core]
    accepts:
      commands: [t.core.Take]
    publishes:
      events: [t.core.Taken]
",
)];

/// A command with no input at all.
const NO_INPUT: &[(&str, &str)] = &[(
    "core.yaml",
    r"
format: ess/1
system: t
version: v1
domains: [t.core]
domain: t.core
commands:
  - name: t.core.Ping
    outcomes:
      - name: done
        emits: [t.core.Pinged]
events:
  - name: t.core.Pinged
    fields:
      - name: id
        type: String
components:
  - component: core-service
    owns:
      domains: [t.core]
    accepts:
      commands: [t.core.Ping]
    publishes:
      events: [t.core.Pinged]
",
)];
