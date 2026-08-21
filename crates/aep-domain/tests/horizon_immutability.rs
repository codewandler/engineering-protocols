//! The API offers no horizon mutation at all, enforced rather than stated.
//!
//! `examples/evidence-horizons-corpus/corpus/05-traps.md` states the rule this file exists to keep:
//!
//! > **make the observation date the identity of the fact, and offer no operation that mutates a
//! > horizon in place.** If `extend` is as easy to call as `re-check`, it is the one that gets
//! > called — every time, under pressure, by whoever is trying to get a gate green.
//!
//! Three mechanisms hold it, in decreasing order of strength, and only the third needs a test:
//!
//! | # | mechanism | strength |
//! |---|---|---|
//! | 1 | an evidence record has **no horizon field** | absolute — there is nothing on a record to mutate |
//! | 2 | a requirement's horizon comes from a parsed document, re-read on every resolve | strong — an in-memory change does not survive |
//! | 3 | this scan | a guard on future edits |
//!
//! Mechanism 1 is checked here too, because it is one line to check and it is the mechanism the
//! other two lean on. Mechanism 3 is the house pattern — invariants 2, 7, 8 and 9 are all held by
//! source scans — and it exists because `set_horizon(&mut self, ..)` is a reasonable-looking helper
//! that a later edit will add without an argument unless something refuses it.
//!
//! # The scan checks itself first
//!
//! Its own failure mode is silence: an extractor that has stopped matching passes everything. So
//! every test plants the construct it is looking for and asserts the extractor finds *that* before
//! asserting it finds nothing real — the same shape `crates/aep-engine/tests/evidence_scan.rs` uses.

use std::path::{Path, PathBuf};

/// The crates a horizon can be reached from.
///
/// `aep-domain` declares the type; `aep-engine` reads it on every evaluation; the markdown backend
/// parses one out of a document; the driver and the CLI are the two callers that hold a mutable
/// execution. A sixth crate that learns about horizons and is not listed here is outside this
/// guard, which is why the list is asserted to be non-empty and every path is asserted to exist.
const SCANNED: &[&str] = &[
    "../aep-domain/src",
    "../aep-engine/src",
    "../aep-backend-markdown/src",
    "../aep-driver/src",
    "../protocol-cli/src",
];

/// Every line of shipped Rust under `SCANNED`, as `(path, line number, text)`.
///
/// Comment lines are skipped, because prose about the rule — this file's own doc comment, and the
/// arguments recorded on `EvidenceRequirement::horizon` — is not a breach of it. That refinement is
/// asserted below rather than trusted.
fn code_lines() -> Vec<(PathBuf, usize, String)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut lines = Vec::new();
    for relative in SCANNED {
        let directory = root.join(relative);
        assert!(
            directory.is_dir(),
            "{} is scanned but does not exist; fix the list rather than the assertion",
            directory.display()
        );
        collect(&directory, &mut lines);
    }
    assert!(
        lines.len() > 10_000,
        "the scan read {} lines, which is too few to be the workspace it claims to read",
        lines.len()
    );
    lines
}

fn collect(directory: &Path, into: &mut Vec<(PathBuf, usize, String)>) {
    let entries = std::fs::read_dir(directory).expect("a scanned directory is readable");
    let mut paths: Vec<PathBuf> = entries
        .map(|entry| entry.expect("a directory entry is readable").path())
        .collect();
    paths.sort();
    for path in paths {
        if path.is_dir() {
            collect(&path, into);
            continue;
        }
        if path.extension().is_none_or(|extension| extension != "rs") {
            continue;
        }
        let text = std::fs::read_to_string(&path).expect("a source file is readable");
        for (number, line) in text.lines().enumerate() {
            if line.trim_start().starts_with("//") {
                continue;
            }
            into.push((path.clone(), number + 1, line.to_owned()));
        }
    }
}

/// Whether `line` assigns to a `horizon` field — `x.horizon = ..`, but not `x.horizon == ..`.
///
/// Assignment is the mutation this file forbids. A struct literal `horizon: Some(..)` is
/// *construction* and is deliberately not matched: building a requirement from a document is the
/// one way a horizon is ever set, and refusing that would refuse the feature.
fn assigns_a_horizon(line: &str) -> bool {
    let Some(at) = line.find(".horizon") else {
        return false;
    };
    let rest = line[at + ".horizon".len()..].trim_start();
    let Some(after) = rest.strip_prefix('=') else {
        return false;
    };
    !after.starts_with('=')
}

/// Whether `line` declares a method that takes `&mut self` and has `horizon` in its name.
fn mutates_a_horizon(line: &str) -> bool {
    let trimmed = line.trim_start();
    let Some(rest) = trimmed
        .strip_prefix("pub fn ")
        .or_else(|| trimmed.strip_prefix("fn "))
    else {
        return false;
    };
    let Some((name, tail)) = rest.split_once('(') else {
        return false;
    };
    name.contains("horizon") && tail.replace(' ', "").starts_with("&mutself")
}

#[test]
fn the_extractors_see_the_constructs_they_exist_to_refuse() {
    // The scan's own guard. Both matchers are shown a planted positive and every near miss that
    // must not fire, so a matcher that has silently stopped working fails here instead of passing
    // on everything.
    assert!(assigns_a_horizon(
        "        requirement.horizon = Some(longer);"
    ));
    assert!(assigns_a_horizon("self.horizon=h;"));
    assert!(
        !assigns_a_horizon("        if requirement.horizon == other.horizon {"),
        "a comparison is not an assignment"
    );
    assert!(
        !assigns_a_horizon("            horizon: Some(Horizon::days(7)?),"),
        "constructing a requirement from a document is how a horizon is set at all"
    );

    assert!(mutates_a_horizon(
        "    pub fn set_horizon(&mut self, horizon: Horizon) {"
    ));
    assert!(mutates_a_horizon("fn extend_horizon(&mut self)"));
    assert!(
        !mutates_a_horizon("    pub fn horizon(&self) -> Option<Horizon> {"),
        "reading one is not mutating one"
    );
    assert!(
        !mutates_a_horizon("    pub fn with_horizon(self, horizon: Horizon) -> Self {"),
        "a by-value builder constructs a new value; it does not extend an existing record"
    );
}

#[test]
fn no_shipped_code_assigns_to_a_horizon_after_it_has_been_read_from_a_document() {
    let offenders: Vec<String> = code_lines()
        .into_iter()
        .filter(|(_, _, line)| assigns_a_horizon(line))
        .map(|(path, number, line)| format!("{}:{number}: {}", path.display(), line.trim()))
        .collect();
    assert!(
        offenders.is_empty(),
        "a horizon is set once, by the parse that read it from a reviewed document, and never \
         reassigned. If `extend` is as easy to call as `re-check`, it is the one that gets \
         called:\n{}",
        offenders.join("\n")
    );
}

#[test]
fn no_shipped_code_offers_an_operation_that_mutates_a_horizon_in_place() {
    let offenders: Vec<String> = code_lines()
        .into_iter()
        .filter(|(_, _, line)| mutates_a_horizon(line))
        .map(|(path, number, line)| format!("{}:{number}: {}", path.display(), line.trim()))
        .collect();
    assert!(
        offenders.is_empty(),
        "the API offers no horizon mutation at all. Shortening a horizon is an edit to the \
         document that declares it — one reviewed line, with the reading that justified it in the \
         same diff:\n{}",
        offenders.join("\n")
    );
}

#[test]
fn an_evidence_record_has_no_horizon_field_for_anything_to_mutate() {
    // The strongest of the three mechanisms, and the cheapest to check. A record that could carry
    // its own expiry is a record that can extend itself, so the field does not exist — and the
    // absence is asserted against the type's source rather than believed.
    let envelope = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/evidence.rs");
    let text = std::fs::read_to_string(&envelope).expect("the evidence module is readable");
    let start = text
        .find("pub struct EvidenceEnvelope<T> {")
        .expect("the envelope is declared in this file");
    let body = &text[start
        ..start
            + text[start..]
                .find("\n}")
                .expect("the envelope's declaration ends")];

    assert!(
        body.contains("pub observed_at: ObservedAt"),
        "the envelope carries the observation time, which is the identity of the fact"
    );
    let fields: Vec<&str> = body
        .lines()
        .filter(|line| line.trim_start().starts_with("pub "))
        .collect();
    assert!(
        !fields.iter().any(|line| line.contains("horizon")),
        "an evidence record has no horizon; the requirement declares it. Fields found: {fields:?}"
    );
}
