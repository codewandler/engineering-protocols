//! Invariant 2 for this crate's document: a validated type does not implement `Deserialize`.
//!
//! The same scan, for the same reason, as `crates/aep-domain/tests/invariants.rs`. Adding
//! `serde::Deserialize` to [`StepMap`] — the shortcut that saves one conversion — would let anyone
//! hand the driver a step map whose format version, whose workflow pin and whose steps nothing ever
//! checked, while the type system still called it a `StepMap`. That edit compiles, passes the
//! tests, passes clippy and passes both drift checks.
//!
//! It asserts the inverse too, which is what keeps it honest: the same extractor that must find
//! nothing on `StepMap` must find `Deserialize` on `RawStepMap`, so a scan that has silently
//! stopped working fails instead of passing.
//!
//! [`PinnedWorkflowRef`] is in the list for the sharper form of the same rule: it is the type F6
//! asked for, and a `Deserialize` on it would mean an unpinned reference could enter the driver
//! through any record that happened to hold one.

use std::path::Path;

/// Every document type of this crate, as the pair the two-stage model is made of.
const DOCUMENT_TYPES: &[(&str, &str)] = &[
    ("RawStepMap", "StepMap"),
    ("RawStateSteps", "StateSteps"),
    ("RawStep", "Step"),
    ("RawCommandStep", "CommandStep"),
    ("RawLlmStep", "LlmStep"),
    ("RawOperatorStep", "OperatorStep"),
    ("RawEvidenceMapping", "EvidenceMapping"),
];

/// Validated types with no `Raw*` counterpart that must still never deserialise.
const VALIDATED_ONLY: &[&str] = &["PinnedWorkflowRef"];

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
        line.strip_prefix(keyword).is_some_and(|rest| {
            rest.starts_with(name) && rest[name.len()..].starts_with([' ', '(', '{', '<', ';'])
        })
    })
}

/// `true` when `text` derives or implements `Deserialize` for `name`.
fn deserializes(text: &str, name: &str) -> bool {
    let mut derive_lines: Vec<String> = Vec::new();
    let mut pending: Vec<String> = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("#[derive(")
            || trimmed.starts_with("#[serde")
            || pending_open(&pending)
        {
            pending.push(trimmed.to_owned());
            continue;
        }
        if declares(trimmed, name) && !pending.is_empty() {
            derive_lines.push(pending.join(" "));
        }
        pending.clear();
    }
    let derived = derive_lines
        .iter()
        .any(|block| block.contains("Deserialize"));
    let implemented = text.contains(&format!("Deserialize<'de> for {name} "))
        || text.contains(&format!("Deserialize<'de> for {name}\n"));
    derived || implemented
}

/// `true` while an attribute block is still open across lines.
fn pending_open(pending: &[String]) -> bool {
    if pending.is_empty() {
        return false;
    }
    let joined = pending.join("");
    joined.matches('(').count() != joined.matches(')').count()
        || joined.matches('[').count() != joined.matches(']').count()
}

#[test]
fn a_validated_type_never_deserialises_and_its_raw_counterpart_always_does() {
    let sources = sources();
    for (raw, validated) in DOCUMENT_TYPES {
        let raw_file = sources
            .iter()
            .find(|(_, text)| text.lines().any(|line| declares(line.trim(), raw)))
            .unwrap_or_else(|| {
                panic!("{raw} is declared nowhere; the scan is looking in the wrong place")
            });
        assert!(
            deserializes(&raw_file.1, raw),
            "{raw} must deserialise: it is the wire surface. If this fails, the extractor stopped \
             working rather than the rule being kept"
        );

        let validated_file = sources
            .iter()
            .find(|(_, text)| text.lines().any(|line| declares(line.trim(), validated)))
            .unwrap_or_else(|| panic!("{validated} is declared nowhere"));
        assert!(
            !deserializes(&validated_file.1, validated),
            "{validated} implements Deserialize, so a caller can obtain one without validating it; \
             validation is the only way this type is supposed to exist"
        );
    }

    for name in VALIDATED_ONLY {
        let file = sources
            .iter()
            .find(|(_, text)| text.lines().any(|line| declares(line.trim(), name)))
            .unwrap_or_else(|| panic!("{name} is declared nowhere"));
        assert!(
            !deserializes(&file.1, name),
            "{name} implements Deserialize, which would let an unpinned reference in through any \
             record that holds one"
        );
    }
}

#[test]
fn the_scan_sees_both_answers_on_synthetic_samples() {
    let derived = "#[derive(Debug, serde::Deserialize)]\npub struct Sample {\n}\n";
    assert!(deserializes(derived, "Sample"));
    let serialize_only = "#[derive(Debug, serde::Serialize)]\npub struct Sample {\n}\n";
    assert!(!deserializes(serialize_only, "Sample"));
    let hand_written = "pub struct Sample;\nimpl<'de> serde::Deserialize<'de> for Sample {\n}\n";
    assert!(deserializes(hand_written, "Sample"));
    assert!(
        !deserializes("pub struct SampleOther {\n}\n", "Sample"),
        "a type whose name merely starts with the one being looked for is a different type"
    );
}
