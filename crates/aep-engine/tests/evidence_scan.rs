//! Invariant 7, enforced rather than stated: the engine never manufactures evidence.
//!
//! The engine's whole authority rests on a division of labour: verifiers and humans *produce*
//! evidence, the engine *evaluates* it. An engine that can conjure a passing `TestResult` on its
//! own is an engine whose decisions certify nothing — and until this file, the rule lived only in
//! `docs/guide/harness.md` as advice to harness authors, which is exactly the shape of rule this
//! repository exists to replace.
//!
//! # What is banned, and what is not
//!
//! Banned in shipped engine code: constructing any *payload* of
//! [`aep_domain::evidence::Evidence`] — the fourteen record types such as `TestResult` and
//! `ApprovalRecord` — whether through a struct literal (`TestResult { .. }`), a constructor path
//! (`TestResult::passing(..)`), an enum-variant expression (`Evidence::TestResult(..)`) or a
//! variant used as a function (`.map(Evidence::TestResult)`).
//!
//! Deliberately allowed:
//!
//! * **Destructuring.** `let Evidence::Approval(approval) = ..` and match arms read evidence;
//!   reading is the engine's job.
//! * **The envelope.** `submit_evidence` wraps a caller-supplied payload in an `EvidenceRecord`,
//!   stamping the id, the injected clock's time and the submitter's producer
//!   (`src/engine.rs`). The envelope is the engine's to stamp; the payload is never its to invent,
//!   and with every payload constructor banned there is nothing of the engine's own for an
//!   envelope to carry.
//! * **Loading.** `load_tree` deserialises evidence documents from disk. A deserialised record is
//!   something a verifier or a human wrote; deserialisation is not manufacture.
//!
//! # Why a source scan
//!
//! The same reasoning as `crates/aep-domain/tests/invariants.rs`: the failure mode is not
//! observable from inside a running test, no dependency is needed, and the scan's own extractor is
//! checked against synthetic samples *and* against the engine's test modules — which construct
//! evidence legitimately and constantly, so a scan that has silently stopped seeing constructions
//! fails on them instead of passing on everything.

use std::path::{Path, PathBuf};

/// The payload types of `aep_domain::evidence::Evidence`, read off the enum itself.
///
/// Read rather than listed, so a new evidence variant is covered the moment it exists: a
/// hand-maintained list here would be one more place for a fifteenth payload type to be forgotten.
/// Returns `(variant name, payload type)` pairs.
fn payload_types() -> Vec<(String, String)> {
    let evidence = Path::new(env!("CARGO_MANIFEST_DIR")).join("../aep-domain/src/evidence.rs");
    let text =
        std::fs::read_to_string(&evidence).expect("aep-domain's evidence module is readable");
    let mut inside = false;
    let mut found = Vec::new();
    for line in text.lines() {
        if line.starts_with("pub enum Evidence {") {
            inside = true;
            continue;
        }
        if !inside {
            continue;
        }
        if line.starts_with('}') {
            break;
        }
        let trimmed = line.trim();
        if trimmed.starts_with("//") || trimmed.starts_with('#') || trimmed.is_empty() {
            continue;
        }
        let Some((variant, rest)) = trimmed.split_once('(') else {
            panic!(
                "`Evidence` has grown a variant this extractor cannot read: {trimmed:?}. Every \
                 variant so far is `Name(PayloadType),`; teach the extractor the new shape so \
                 invariant 7 keeps covering it"
            );
        };
        let payload = rest
            .strip_suffix("),")
            .unwrap_or_else(|| panic!("a variant payload ends in `),`: {trimmed:?}"));
        found.push((variant.to_owned(), payload.to_owned()));
    }
    found
}

/// Every `.rs` file of the engine's `src/`, as `(path, contents)`, in a stable order.
fn sources() -> Vec<(PathBuf, String)> {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut sources: Vec<(PathBuf, String)> = std::fs::read_dir(&directory)
        .expect("the crate has sources")
        .map(|entry| entry.expect("a readable entry").path())
        .filter(|path| path.extension().is_some_and(|it| it == "rs"))
        .map(|path| {
            let text = std::fs::read_to_string(&path).expect("a readable source file");
            (path, text)
        })
        .collect();
    sources.sort();
    sources
}

/// Splits `text` into (shipped code, test code), and records file modules gated `#[cfg(test)]`.
///
/// Two shapes exist in this crate and both are handled; any third shape fails loudly rather than
/// being half-scanned. An inline `#[cfg(test)] mod tests {` ends the shipped region — test modules
/// are trailing by convention here. A gated file module (`#[cfg(test)] pub(crate) mod fixtures;`)
/// names a whole file as test code, which the caller excludes via `test_modules`.
fn split(text: &str) -> (String, String, Vec<String>) {
    let lines: Vec<&str> = text.lines().collect();
    let mut shipped = Vec::new();
    let mut test_lines: Vec<&str> = Vec::new();
    let mut test_modules = Vec::new();
    let mut index = 0;
    while index < lines.len() {
        let line = lines[index];
        if line.trim() != "#[cfg(test)]" {
            shipped.push(line);
            index += 1;
            continue;
        }
        let mut next = index + 1;
        while next < lines.len() && lines[next].trim().starts_with("#[") {
            next += 1;
        }
        let declaration = lines.get(next).map(|it| it.trim()).unwrap_or_default();
        assert!(
            declaration.starts_with("mod ")
                || declaration.starts_with("pub mod ")
                || declaration.starts_with("pub(crate) mod "),
            "`#[cfg(test)]` gates something that is not a module ({declaration:?}); this scan \
             only knows how to separate test modules from shipped code, so teach it the new shape \
             rather than letting it under-scan"
        );
        if declaration.ends_with(';') {
            let name = declaration
                .trim_end_matches(';')
                .rsplit(' ')
                .next()
                .expect("a module name")
                .to_owned();
            test_modules.push(name);
            index = next + 1;
            continue;
        }
        test_lines.extend(&lines[index..]);
        break;
    }
    (shipped.join("\n"), test_lines.join("\n"), test_modules)
}

/// Every construction of an evidence payload in `text`, as `(line number, explanation)`.
///
/// Comment lines are skipped: prose about the rule must not read as a breach of it. Destructuring
/// is recognised on the line it happens — `let Evidence::X(..) = ..` with nothing assigned before
/// the pattern, or a match arm whose `=>` follows the variant — and everything else that names a
/// payload constructor is a finding.
fn constructions(text: &str, payloads: &[(String, String)]) -> Vec<(usize, String)> {
    let mut found = Vec::new();
    for (number, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("//") {
            continue;
        }
        for (variant, payload) in payloads {
            for start in occurrences(line, payload) {
                let after = &line[start + payload.len()..];
                if after.starts_with("::") {
                    found.push((
                        number + 1,
                        format!("`{payload}::` — a payload constructor path"),
                    ));
                } else if after.starts_with(" {") {
                    found.push((number + 1, format!("`{payload} {{` — a payload literal")));
                }
            }
            let token = format!("Evidence::{variant}");
            for start in occurrences(line, &token) {
                let after = &line[start + token.len()..];
                let before = &line[..start];
                if !after.starts_with('(') {
                    if after.starts_with(':') || after.starts_with(|c: char| c.is_alphanumeric()) {
                        continue; // a longer path or identifier, not this variant
                    }
                    found.push((
                        number + 1,
                        format!("`{token}` used as a constructor function"),
                    ));
                    continue;
                }
                let let_pattern = before
                    .rfind("let ")
                    .is_some_and(|at| !before[at..].contains('='));
                let arm_pattern = !before.contains("=>") && after.contains("=>");
                if !let_pattern && !arm_pattern {
                    found.push((
                        number + 1,
                        format!("`{token}(` in expression position — a payload construction"),
                    ));
                }
            }
        }
    }
    found
}

/// Byte offsets of every occurrence of `token` in `line` that starts on an identifier boundary.
fn occurrences(line: &str, token: &str) -> Vec<usize> {
    let mut found = Vec::new();
    let mut from = 0;
    while let Some(at) = line[from..].find(token) {
        let start = from + at;
        let boundary = line[..start]
            .chars()
            .next_back()
            .is_none_or(|before| !before.is_alphanumeric() && before != '_');
        if boundary {
            found.push(start);
        }
        from = start + token.len();
    }
    found
}

#[test]
fn the_engine_constructs_no_evidence_payload_outside_its_test_modules() {
    let payloads = payload_types();
    assert!(
        payloads.len() >= 14,
        "only {} payload types were read off `Evidence`; the extractor is looking at the wrong \
         enum",
        payloads.len()
    );
    assert!(
        payloads.iter().any(|(_, payload)| payload == "TestResult"),
        "`TestResult` is the payload the rule exists for, and the extractor did not find it"
    );

    let mut test_files = Vec::new();
    let mut shipped_files = 0;
    let mut violations = Vec::new();
    let mut constructions_in_tests = 0;

    for (path, text) in sources() {
        let (shipped, test, test_modules) = split(&text);
        for module in test_modules {
            test_files.push(format!("{module}.rs"));
        }
        let file = path
            .file_name()
            .expect("a file name")
            .to_string_lossy()
            .into_owned();
        if test_files.contains(&file) {
            constructions_in_tests += constructions(&text, &payloads).len();
            continue;
        }
        shipped_files += 1;
        for (line, what) in constructions(&shipped, &payloads) {
            violations.push(format!("{}:{line}: {what}", path.display()));
        }
        constructions_in_tests += constructions(&test, &payloads).len();
    }

    assert!(
        shipped_files >= 10,
        "only {shipped_files} shipped source files were scanned; the scan is looking in the wrong \
         place"
    );
    assert!(
        test_files.contains(&"fixtures.rs".to_owned()),
        "`lib.rs` gates `mod fixtures` behind `#[cfg(test)]`; the split no longer sees that, so \
         it can no longer tell test code from shipped code"
    );
    assert!(
        constructions_in_tests >= 5,
        "the engine's own test modules construct evidence constantly and the scan found only \
         {constructions_in_tests} constructions there — a scan that sees no construction anywhere \
         reports no breach of anything"
    );
    assert!(
        violations.is_empty(),
        "invariant 7: the engine evaluates evidence, it does not manufacture it. Shipped engine \
         code constructs an evidence payload here:\n{}\nIf the engine genuinely needs to *carry* \
         a new payload, it still arrives through `EvidenceSubmission`; only the envelope is the \
         engine's to build",
        violations.join("\n")
    );
}

#[test]
fn the_scan_reads_constructions_and_not_patterns_or_prose() {
    let payloads = payload_types();

    let construction =
        "        let record = Evidence::TestResult(TestResult::passing(TestSuite::Unit, 4));";
    assert_eq!(
        constructions(construction, &payloads).len(),
        2,
        "a variant expression and a constructor path are each a finding"
    );

    let literal = "            Evidence::Diff(ChangeSet {";
    assert_eq!(
        constructions(literal, &payloads).len(),
        2,
        "a struct literal is a finding beside the variant that wraps it"
    );

    let mapped = "        submitted.map(Evidence::TestResult)";
    assert_eq!(
        constructions(mapped, &payloads).len(),
        1,
        "a variant used as a function is still a construction"
    );

    let arm_body = "            Kind::Test => Evidence::TestResult(fabricate()),";
    assert_eq!(
        constructions(arm_body, &payloads).len(),
        1,
        "a construction in a match arm's body is not excused by the arrow before it"
    );

    let let_else = "        let Evidence::Approval(approval) = &recorded.record.value else {";
    assert!(
        constructions(let_else, &payloads).is_empty(),
        "destructuring reads evidence, which is the engine's job"
    );

    let arm = "            Evidence::TestResult(result) => Some(result.status()),";
    assert!(
        constructions(arm, &payloads).is_empty(),
        "a match arm's pattern reads evidence"
    );

    let guarded =
        "        Evidence::TestResult(result) if result.status() == Failed => escalate(),";
    assert!(
        constructions(guarded, &payloads).is_empty(),
        "a guard's `==` does not turn its pattern into a construction"
    );

    let full_path =
        "        aep_domain::evidence::Evidence::Verification(record) => &record.claim,";
    assert!(
        constructions(full_path, &payloads).is_empty(),
        "a fully qualified pattern is still a pattern"
    );

    let prose = "    // Never construct a TestResult { .. } here: see invariant 7.";
    assert!(
        constructions(prose, &payloads).is_empty(),
        "prose about the rule is not a breach of it"
    );

    let other_type = "        let kind = EvidenceKind::TestResult;";
    assert!(
        constructions(other_type, &payloads).is_empty(),
        "`EvidenceKind` is vocabulary, not a payload"
    );
}
