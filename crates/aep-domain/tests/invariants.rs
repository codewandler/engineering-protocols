//! Invariant 2, enforced rather than stated: a validated type does not implement `Deserialize`.
//!
//! Every other guarantee in this repository sits on top of this one. A document deserialises into
//! a `Raw*` type and becomes a domain type through `TryFrom`, so possession of a `Protocol` is
//! itself the evidence that its approval floor, its vocabulary and its observables were checked.
//! Add `serde::Deserialize` to `Protocol` — the shortcut that saves one conversion — and anyone
//! can hand the engine a protocol nothing ever validated, while the type system still calls it a
//! `Protocol`. That edit compiles, passes the tests, passes clippy and passes both drift checks;
//! before this file, nothing in the workspace could see it.
//!
//! # Why a source scan
//!
//! Three mechanisms were available, and this one needs no new dependency:
//!
//! * **A source scan** — this file. It reads the derive list and the `impl` blocks attached to
//!   each validated type, and it can also assert the inverse for the `Raw*` counterpart, which is
//!   what keeps it honest: the same extractor that must find nothing on `Protocol` must find
//!   `Deserialize` on `RawProtocol`, so a scan that has silently stopped working fails loudly
//!   instead of passing. Its weakness is that it reads text, so it is checked against synthetic
//!   samples below rather than trusted.
//! * **`trybuild`** — a compile-fail case per type. It observes the property exactly, but it costs
//!   a dev-dependency and a fixture file per type, and it asserts against compiler diagnostics,
//!   which change wording between toolchains.
//! * **A marker trait plus a negative assertion** — stable Rust has no negative trait bound, so
//!   this needs the inherent-versus-trait resolution trick. It is arcane to read, easy to defeat
//!   by accident, and still only covers the types someone remembered to list.
//!
//! The precedent is `crates/ess-compiler/tests/billing.rs`, which scans its crate's sources for
//! banned tokens on the same reasoning: the failure mode is not observable from inside a running
//! test, so the source is read for it.

use std::path::Path;

/// Every document type of this crate, as the pair the two-stage model is made of.
///
/// The left-hand type is the wire surface and must deserialise. The right-hand type is what
/// validation produces and must not. A new document type belongs here; the test below fails until
/// it is added.
const DOCUMENT_TYPES: &[(&str, &str)] = &[
    ("RawProtocol", "Protocol"),
    ("RawProfile", "Profile"),
    ("RawPrinciple", "Principle"),
    ("RawObligation", "Obligation"),
    ("RawWorkflow", "Workflow"),
    ("RawState", "State"),
    ("RawTransition", "Transition"),
    ("RawTask", "Task"),
    ("RawProjectConfig", "ProjectConfig"),
    ("RawArtifactManifest", "ArtifactGraph"),
];

/// Every `.rs` file of the crate, as `(file name, contents)`, in a stable order.
fn sources() -> Vec<(String, String)> {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut sources: Vec<(String, String)> = std::fs::read_dir(&directory)
        .expect("the crate has sources")
        .map(|entry| entry.expect("a readable directory entry").path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "rs"))
        .map(|path| {
            let text = std::fs::read_to_string(&path).expect("a readable source file");
            (path.display().to_string(), text)
        })
        .collect();
    sources.sort();
    sources
}

/// `true` when `line` is the top-level declaration of `name`.
fn declares(line: &str, name: &str) -> bool {
    ["pub struct ", "pub enum "].iter().any(|keyword| {
        line.strip_prefix(keyword)
            .and_then(|rest| rest.strip_prefix(name))
            .is_some_and(|tail| tail.is_empty() || tail.starts_with([' ', '{', '(', '<', ';']))
    })
}

/// The attributes attached to `name`'s declaration, with doc comments dropped.
///
/// Doc comments are dropped deliberately: this crate's prose says the word `Deserialize` in
/// several places precisely because the invariant is worth explaining, and prose about a rule must
/// not read as a breach of it.
fn attributes_of(text: &str, name: &str) -> Option<String> {
    let lines: Vec<&str> = text.lines().collect();
    let declaration = lines.iter().position(|line| declares(line, name))?;
    let mut attributes = Vec::new();
    for line in lines[..declaration].iter().rev() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.ends_with('}') || trimmed.ends_with(';') {
            break;
        }
        if trimmed.starts_with("//") {
            continue;
        }
        attributes.push(trimmed);
    }
    Some(attributes.join("\n"))
}

/// `true` when `text` hand-writes a `Deserialize` implementation for exactly `name`.
fn implements_deserialize(text: &str, name: &str) -> bool {
    text.lines().any(|line| {
        let trimmed = line.trim();
        if !trimmed.starts_with("impl") || !trimmed.contains("Deserialize") {
            return false;
        }
        trimmed
            .split_once(" for ")
            .is_some_and(|(_, target)| target.trim_end_matches('{').trim() == name)
    })
}

/// Where `name` is declared, and the attributes attached to it.
fn declaration<'a>(sources: &'a [(String, String)], name: &str) -> (&'a str, String) {
    sources
        .iter()
        .find_map(|(file, text)| attributes_of(text, name).map(|found| (file.as_str(), found)))
        .unwrap_or_else(|| panic!("`{name}` is declared somewhere in this crate"))
}

#[test]
fn a_validated_document_type_cannot_be_deserialised_straight_off_the_wire() {
    let sources = sources();
    assert!(
        sources.len() >= 20,
        "only {} source files were read; the scan is looking in the wrong place",
        sources.len()
    );

    for (raw, validated) in DOCUMENT_TYPES {
        let (raw_file, raw_attributes) = declaration(&sources, raw);
        assert!(
            raw_attributes.contains("Deserialize"),
            "{raw_file}: `{raw}` is the wire surface and must deserialise. It does not, which \
             means either the two-stage model has changed or this scan has stopped seeing derives \
             — and a scan that sees nothing reports no breach of anything. Attributes read:\n{raw_attributes}"
        );

        let (validated_file, validated_attributes) = declaration(&sources, validated);
        assert!(
            !validated_attributes.contains("Deserialize"),
            "{validated_file}: `{validated}` is a validated type and derives `Deserialize`, so a \
             `{validated}` can now be conjured straight from a document that nothing checked. \
             Parse into `{raw}` and go through `TryFrom` instead. Attributes read:\n{validated_attributes}"
        );

        for (file, text) in &sources {
            assert!(
                !implements_deserialize(text, validated),
                "{file}: `{validated}` is a validated type and implements `Deserialize` by hand, \
                 which puts a second way to obtain one beside `TryFrom<{raw}>` — and a second way \
                 in is a second place to forget what validation checked"
            );
        }
    }
}

#[test]
fn every_raw_document_type_is_checked_against_its_validated_counterpart() {
    let sources = sources();
    let mut found = Vec::new();
    for (file, text) in &sources {
        for line in text.lines() {
            let Some(rest) = line.strip_prefix("pub struct Raw") else {
                continue;
            };
            let name = format!(
                "Raw{}",
                rest.split([' ', '{', '(', '<', ';']).next().unwrap_or(rest)
            );
            assert!(
                DOCUMENT_TYPES.iter().any(|(raw, _)| *raw == name),
                "{file}: `{name}` is a document type nothing pairs with a validated counterpart, \
                 so invariant 2 is unchecked for whatever it validates into. Add the pair to \
                 `DOCUMENT_TYPES`"
            );
            found.push(name);
        }
    }
    assert_eq!(
        found.len(),
        DOCUMENT_TYPES.len(),
        "the scan found {} raw document types and {} are listed: {found:?}",
        found.len(),
        DOCUMENT_TYPES.len()
    );
}

#[test]
fn the_scan_reads_derives_and_impls_and_not_the_prose_about_them() {
    // The scan is text, so its power is asserted against samples rather than assumed. Each of
    // these is a shape that appears in this crate.
    let single_line =
        "/// A document, as parsed.\n#[derive(Debug, serde::Deserialize)]\npub struct Sample {\n";
    assert!(attributes_of(single_line, "Sample")
        .expect("the declaration is found")
        .contains("Deserialize"));

    let wrapped = "#[derive(\n    Debug,\n    Clone,\n    serde::Deserialize,\n)]\n#[serde(deny_unknown_fields)]\npub struct Wide {\n";
    assert!(
        attributes_of(wrapped, "Wide")
            .expect("the declaration is found")
            .contains("Deserialize"),
        "a derive list wrapped over several lines is still one attribute"
    );

    let prose = "/// Deliberately does not implement Deserialize.\n#[derive(Debug, serde::Serialize)]\npub struct Documented {\n";
    assert!(
        !attributes_of(prose, "Documented")
            .expect("the declaration is found")
            .contains("Deserialize"),
        "a doc comment explaining the invariant is not a breach of it"
    );

    let previous_item = "impl<'de> serde::Deserialize<'de> for Other {\n    fn deserialize() {}\n}\n\n#[derive(Debug)]\npub struct After {\n";
    assert!(
        !attributes_of(previous_item, "After")
            .expect("the declaration is found")
            .contains("Deserialize"),
        "the attributes of one type must not be read from the item above it"
    );

    assert_eq!(attributes_of(single_line, "Absent"), None);

    let hand_written = "impl<'de> serde::Deserialize<'de> for Sample {";
    assert!(implements_deserialize(hand_written, "Sample"));
    assert!(
        !implements_deserialize(hand_written, "Sam"),
        "a prefix of the type name is a different type"
    );
    assert!(
        !implements_deserialize("impl serde::Serialize for Sample {", "Sample"),
        "serialising out is not the same as deserialising in"
    );
}
