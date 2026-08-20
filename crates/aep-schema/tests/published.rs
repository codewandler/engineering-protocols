//! The published schemas, checked against the documents this repository ships.
//!
//! A generated schema is derived from the Rust types, so it cannot describe a document the parser
//! would reject on structure — but it *can* describe the wrong thing entirely. A type with a
//! hand-written `Deserialize` gets a derived schema describing its **representation**: `Version` is
//! a `u32` inside, and every document ever written says `v3`. The result was a schema that declared
//! the normative example invalid, which is the one failure a schema published for editors must not
//! have, and nothing in the build noticed.
//!
//! So the check is not "is the schema well formed" — it is "does the schema accept the documents we
//! ship". Those documents are also the ones the parser is tested against, which makes the two
//! agree by construction rather than by review.

use std::path::{Path, PathBuf};

use jsonschema::Validator;
use serde_json::Value;

/// The repository root.
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the workspace root exists")
}

/// One generated schema, compiled.
fn validator(filename: &str) -> Validator {
    let entry = aep_schema::generated_schemas()
        .into_iter()
        .find(|entry| entry.filename == filename)
        .unwrap_or_else(|| panic!("{filename} is published"));
    let json: Value = serde_json::from_str(&entry.to_json().expect("serialises")).expect("is JSON");
    jsonschema::validator_for(&json)
        .unwrap_or_else(|error| panic!("{filename} is a usable schema: {error}"))
}

/// A YAML document as JSON.
fn document(relative: &str) -> Value {
    let path = root().join(relative);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{} is readable: {error}", path.display()));
    serde_yaml::from_str(&text)
        .unwrap_or_else(|error| panic!("{} is well formed: {error}", path.display()))
}

/// Asserts a document validates, naming every problem rather than the first.
fn accepts(validator: &Validator, relative: &str) {
    let instance = document(relative);
    let problems: Vec<String> = validator
        .iter_errors(&instance)
        .map(|error| format!("{}: {error}", error.instance_path()))
        .collect();
    assert!(
        problems.is_empty(),
        "the published schema rejects {relative}, which this repository ships as valid:\n  - {}",
        problems.join("\n  - ")
    );
}

#[test]
fn the_specification_schema_accepts_the_normative_example() {
    let validator = validator("ess.schema.json");
    for file in [
        "examples/billing/system.yaml",
        "examples/billing/domains/invoice.yaml",
        "examples/billing/domains/email.yaml",
    ] {
        accepts(&validator, file);
    }
}

#[test]
fn the_specification_schema_refuses_a_misspelt_key() {
    // `deny_unknown_fields` is what makes the schema worth loading into an editor: a typo becomes a
    // red squiggle rather than a field silently ignored.
    let validator = validator("ess.schema.json");
    let mut instance = document("examples/billing/domains/invoice.yaml");
    let object = instance.as_object_mut().expect("a mapping");
    let entities = object
        .remove("entities")
        .expect("the example declares entities");
    object.insert("entitites".to_owned(), entities);

    assert!(
        validator.iter_errors(&instance).next().is_some(),
        "a misspelt top-level key validated clean"
    );
}

/// Every `.yaml` under a directory, recursively, relative to the repository root.
fn documents_under(directory: &str) -> Vec<String> {
    let base = root().join(directory);
    let mut found = Vec::new();
    let mut pending = vec![base.clone()];
    while let Some(current) = pending.pop() {
        for entry in std::fs::read_dir(&current)
            .unwrap_or_else(|error| panic!("{} is readable: {error}", current.display()))
        {
            let path = entry.expect("an entry").path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|ext| ext == "yaml") {
                found.push(
                    path.strip_prefix(root())
                        .expect("inside the repository")
                        .display()
                        .to_string(),
                );
            }
        }
    }
    assert!(!found.is_empty(), "{directory} holds no documents");
    found.sort();
    found
}

#[test]
fn the_document_schemas_accept_every_document_this_repository_ships() {
    // Every document of each kind, not a sample: a schema that accepts the one file someone
    // remembered to list is not a schema anyone can rely on.
    for (filename, directory) in [
        ("protocol.schema.json", "protocols"),
        ("principle.schema.json", "principles"),
        ("workflow.schema.json", "workflows"),
        ("profile.schema.json", "profiles"),
        ("artifact-lifecycle.schema.json", "artifacts/lifecycles"),
    ] {
        let validator = validator(filename);
        for file in documents_under(directory) {
            accepts(&validator, &file);
        }
    }

    accepts(
        &validator("task.schema.json"),
        "examples/development-passkeys/task.yaml",
    );
    accepts(
        &validator("artifact-manifest.schema.json"),
        "examples/development-passkeys/artifacts.yaml",
    );
}
