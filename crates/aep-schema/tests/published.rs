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

/// The specification directories every file of which has to validate.
///
/// Both, because both are inputs a person edits with this schema loaded: `examples/billing/` is the
/// normative example the guide points at, and `examples/oracle-fixture/` is the corner-case fixture
/// wave 4 reads. A schema that accepts one and refuses the other is a schema for one of them.
const SPECIFICATIONS: &[&str] = &["examples/billing", "examples/oracle-fixture"];

#[test]
fn the_specification_schema_accepts_every_file_of_every_example() {
    // Discovered, not listed. This test used to name three files of a five-file example, so
    // `components.yaml` — the only one carrying a component or a binding, and the only one the
    // schema rejected — was outside the assertion for as long as it existed. A file added to
    // either example is compiled by the CLI, and it has to be checked here for the same reason:
    // `crates/ess-compiler/tests/billing.rs` discovers its inputs on exactly this argument.
    let validator = validator("ess.schema.json");
    for specification in SPECIFICATIONS {
        for file in documents_under(specification) {
            accepts(&validator, &file);
        }
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
        ("driver-steps.schema.json", "drivers"),
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

// ---------------------------------------------------------------------------------------------
// Wire-format aliases
//
// `AGENTS.md` records that both spellings of an aliased key are deliberate, and `schemars` cannot
// see `#[serde(alias = "…")]` — so every derived schema published half the language until
// `aep_schema::alias` put the other half back. What follows is the guard against that happening a
// fourth time: the sources are read for the attribute itself, and each spelling found on a type a
// published schema carries has to be a spelling that schema accepts.
// ---------------------------------------------------------------------------------------------

/// One `#[serde(alias = "…")]`, as the sources declare it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct DeclaredAlias {
    /// The type the attribute sits inside, which is what the schema names its definition.
    type_name: String,
    /// The accepted spelling.
    alias: String,
    /// Where to go and look, when this fails.
    file: String,
    /// Which line of it.
    line: usize,
}

/// Every `.rs` file under `crates/*/src`, repository-relative, in a stable order.
fn crate_sources() -> Vec<(String, String)> {
    let root = root();
    let mut found = Vec::new();
    let mut pending = vec![root.join("crates")];
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(&directory)
            .unwrap_or_else(|error| panic!("{} is readable: {error}", directory.display()))
        {
            let path = entry.expect("an entry").path();
            if path.is_dir() {
                // `src` only: a test may legitimately write the attribute in a fixture string, and
                // a fixture is not a published type.
                if path.ends_with("tests") || path.ends_with("target") {
                    continue;
                }
                pending.push(path);
            } else if path.extension().is_some_and(|it| it == "rs")
                && path.components().any(|it| it.as_os_str() == "src")
            {
                let text = std::fs::read_to_string(&path).expect("a readable source file");
                let name = path
                    .strip_prefix(&root)
                    .expect("inside the repository")
                    .display()
                    .to_string();
                found.push((name, text));
            }
        }
    }
    found.sort();
    found
}

/// The type a line declares, when it declares one.
fn declared_type(line: &str) -> Option<&str> {
    let rest = [
        "pub struct ",
        "pub enum ",
        "pub(crate) struct ",
        "pub(crate) enum ",
    ]
    .iter()
    .find_map(|keyword| line.strip_prefix(keyword))?;
    let end = rest
        .find(|character: char| !character.is_alphanumeric() && character != '_')
        .unwrap_or(rest.len());
    Some(&rest[..end])
}

/// Every `#[serde(alias = "…")]` in the workspace's library sources.
///
/// Text, not syntax: reading it with a parser would cost a dependency for a scan whose failure mode
/// — finding nothing — is asserted against below. Doc comments cannot be mistaken for attributes
/// because an attribute line starts with `#[serde(`, and this crate's own prose about the rule
/// therefore does not read as a breach of it.
fn declared_aliases() -> Vec<DeclaredAlias> {
    let mut found = Vec::new();
    for (file, text) in crate_sources() {
        let lines: Vec<&str> = text.lines().collect();
        let mut type_name = String::new();
        let mut index = 0;
        while index < lines.len() {
            let trimmed = lines[index].trim();
            if let Some(declared) = declared_type(trimmed) {
                declared.clone_into(&mut type_name);
            }
            if trimmed.starts_with("#[serde(") {
                let start = index;
                let mut attribute = trimmed.to_owned();
                while !attribute.ends_with(")]") && index + 1 < lines.len() {
                    index += 1;
                    attribute.push(' ');
                    attribute.push_str(lines[index].trim());
                }
                for alias in spellings(&attribute) {
                    found.push(DeclaredAlias {
                        type_name: type_name.clone(),
                        alias,
                        file: file.clone(),
                        line: start + 1,
                    });
                }
            }
            index += 1;
        }
    }
    found.sort();
    found
}

/// Every `alias = "…"` in one attribute.
fn spellings(attribute: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut rest = attribute;
    while let Some((_, tail)) = rest.split_once("alias = \"") {
        let Some((spelling, remainder)) = tail.split_once('"') else {
            break;
        };
        found.push(spelling.to_owned());
        rest = remainder;
    }
    found
}

/// Every published schema, as `(filename, document)`.
fn published() -> Vec<(String, Value)> {
    aep_schema::generated_schemas()
        .into_iter()
        .map(|entry| {
            let json =
                serde_json::from_str(&entry.to_json().expect("serialises")).expect("is JSON");
            (entry.filename, json)
        })
        .collect()
}

/// The subschema a published document uses for `type_name`, when it carries one.
fn carries<'a>(schema: &'a Value, type_name: &str) -> Option<&'a Value> {
    if schema["title"].as_str() == Some(type_name) {
        return Some(schema);
    }
    schema["definitions"].get(type_name)
}

/// `true` when `target` accepts `alias`, in any of the three positions an alias can occupy.
///
/// A **property name**, for a struct field. An **internally tagged** variant's tag value, which is
/// a single-valued `enum` on a required property. Or an **externally tagged** variant's key, which
/// is the single required property of a `oneOf` member — the shape `trace-spec/1` writes an
/// expectation kind in.
fn publishes_alias(target: &Value, alias: &str) -> bool {
    if target["properties"].get(alias).is_some() {
        return true;
    }
    target["oneOf"].as_array().is_some_and(|variants| {
        variants.iter().any(|variant| {
            variant["properties"].as_object().is_some_and(|properties| {
                properties.contains_key(alias)
                    || properties.values().any(|property| {
                        property["enum"].as_array().is_some_and(|values| {
                            values.iter().any(|value| value.as_str() == Some(alias))
                        })
                    })
            })
        })
    })
}

#[test]
fn the_alias_scan_finds_the_attributes_it_is_supposed_to_find() {
    // The inverse guard, on the pattern of `crates/aep-domain/tests/invariants.rs`: a scan that has
    // silently stopped working finds nothing and passes every assertion built on it. So it is
    // checked against attributes that exist, in three different crates and in both positions the
    // attribute can hold — a struct field and an enum variant.
    let declared = declared_aliases();
    for (type_name, alias) in [
        ("RawComponentSpec", "component"),
        ("RawBindingSpec", "id"),
        ("RawTask", "type"),
        ("RawTask", "principles"),
        ("CapabilityPolicy", "require_approval"),
        ("Evidence", "test_execution"),
        ("EvidenceInput", "envelope_subject"),
    ] {
        assert!(
            declared
                .iter()
                .any(|found| found.type_name == type_name && found.alias == alias),
            "the source scan no longer finds `{alias}` on `{type_name}`, so nothing below is \
             checking anything"
        );
    }
    assert!(
        declared.len() >= 20,
        "the source scan found only {} aliases; it used to find twenty-one",
        declared.len()
    );
}

#[test]
fn no_type_carries_an_alias_no_published_schema_would_ever_be_asked_about() {
    // The exemption below — a type no published schema carries — has to be reachable, or the rule
    // it exempts is being applied to everything and the exemption is dead text. `EvidenceInput` is
    // `aep-schema`'s own parse surface: it accepts `envelope_subject`, and nothing publishes it.
    assert!(
        published()
            .iter()
            .all(|(_, schema)| carries(schema, "EvidenceInput").is_none()),
        "`EvidenceInput` is published now, so it is no longer the case that some aliased types are \
         not"
    );
}

#[test]
fn every_published_schema_accepts_every_spelling_its_parser_does() {
    // The guard. `#[serde(alias = "component")]` on `RawComponentSpec` made the parser accept a
    // spelling the schema refused, and the only thing relating the two was that somebody
    // remembered. This relates them mechanically: the attribute is read out of the source, and the
    // schema that carries its type has to publish it.
    let declared = declared_aliases();
    let published = published();
    let mut missing = Vec::new();

    for found in &declared {
        for (filename, schema) in &published {
            let Some(target) = carries(schema, &found.type_name) else {
                continue;
            };
            if !publishes_alias(target, &found.alias) {
                missing.push(format!(
                    "{filename} describes `{}` without `{}`, which {}:{} accepts",
                    found.type_name, found.alias, found.file, found.line
                ));
            }
        }
    }

    assert!(
        missing.is_empty(),
        "the published schemas refuse spellings the parsers read. Add each to \
         `WIRE_ALIASES` in crates/aep-schema/src/alias.rs:\n  - {}",
        missing.join("\n  - ")
    );
}

#[test]
fn no_published_alias_is_a_spelling_the_parsers_refuse() {
    // The other direction. A list entry with no attribute behind it publishes a schema that
    // promises something the parser rejects, which is the same defect pointing the other way.
    let declared = declared_aliases();
    for entry in aep_schema::WIRE_ALIASES {
        assert!(
            declared
                .iter()
                .any(|found| found.type_name == entry.type_name && found.alias == entry.alias),
            "WIRE_ALIASES publishes `{}` for `{}`, and no `#[serde(alias = …)]` in the workspace \
             accepts it",
            entry.alias,
            entry.type_name
        );
    }
}

#[test]
fn an_aliased_field_is_never_left_required_under_its_canonical_spelling() {
    // Publishing the second spelling is not enough on its own: while `name` stays in `required`, a
    // document writing `component:` still fails, and one writing both still passes. The `oneOf`
    // that replaces the entry is what makes exactly one of them the rule.
    for entry in aep_schema::WIRE_ALIASES {
        for (filename, schema) in published() {
            let Some(target) = carries(&schema, entry.type_name) else {
                continue;
            };
            let required = target["required"].as_array().cloned().unwrap_or_default();
            assert!(
                !required
                    .iter()
                    .any(|it| it.as_str() == Some(entry.canonical)),
                "{filename} still requires `{}` on `{}`, so `{}` can never be written on its own",
                entry.canonical,
                entry.type_name,
                entry.alias
            );
        }
    }
}

/// A component, written with whichever spellings `keys` names.
fn component(keys: &[&str]) -> Value {
    let mut written = serde_json::Map::new();
    for key in keys {
        written.insert((*key).to_owned(), Value::from("invoice-service"));
    }
    serde_json::json!({ "components": [written] })
}

#[test]
fn the_specification_schema_requires_exactly_one_spelling_of_a_components_name() {
    // The state the rule is load-bearing in is the one an alias creates and an optional property
    // does not: a document naming the component neither way. Both spellings made optional would
    // accept it, and the parser refuses it with `missing field \`name\``.
    let validator = validator("ess.schema.json");
    for spelling in [&["name"][..], &["component"][..]] {
        let instance = component(spelling);
        let problems: Vec<String> = validator
            .iter_errors(&instance)
            .map(|error| error.to_string())
            .collect();
        assert!(
            problems.is_empty(),
            "the schema refuses `{}:`, which the parser reads: {}",
            spelling[0],
            problems.join("; ")
        );
    }
    assert!(
        !validator.is_valid(&component(&["name", "component"])),
        "serde reads the two spellings into one field, so a document giving both is a duplicate key"
    );
    assert!(
        !validator.is_valid(&component(&[])),
        "a component that names itself neither way is refused by the parser and must be refused here"
    );
}

/// A binding, written with whichever spelling of its name `key` names.
fn binding(key: Option<&str>) -> Value {
    let mut written = serde_json::json!({
        "when": { "event": "billing.invoice.InvoiceCreated" },
        "invoke": { "command": "billing.email.SendEmail" },
        "delivery": "at_least_once",
        "on_failure": "escalate",
    });
    if let Some(key) = key {
        written[key] = Value::from("notify-on-invoice-created");
    }
    serde_json::json!({ "bindings": [written] })
}

#[test]
fn the_specification_schema_requires_exactly_one_spelling_of_a_bindings_name() {
    let validator = validator("ess.schema.json");
    for spelling in ["name", "id"] {
        let instance = binding(Some(spelling));
        let problems: Vec<String> = validator
            .iter_errors(&instance)
            .map(|error| error.to_string())
            .collect();
        assert!(
            problems.is_empty(),
            "the schema refuses `{spelling}:`, which the parser reads: {}",
            problems.join("; ")
        );
    }
    assert!(
        !validator.is_valid(&binding(None)),
        "a binding that names itself neither way is refused by the parser and must be refused here"
    );
}
