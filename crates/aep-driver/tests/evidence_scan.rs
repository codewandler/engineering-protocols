//! Invariant 7 at the driver's layer: the driver never mints an approval, and never signs as a
//! person.
//!
//! # Why this has to be a rule with a scan behind it
//!
//! `aep_engine::policy::approval_recorded` matches an `Evidence::Approval` on its subject or
//! approval id and on `ApprovalDecision::Granted`. It does **not** check who granted it.
//! `ApprovalRequirement::evaluate` does — it skips a record whose approver is not human when
//! `human: true` — but the **capability** gate does not. So nothing below the driver would stop a
//! harness from writing its own approval and unlocking a capability with it, which means the refusal
//! has to be the driver's, and a refusal that lives only in prose is the shape of rule this
//! repository exists to replace. There is no auto-approve under any flag, ever: a gate a caller's
//! own flag can satisfy is not a gate.
//!
//! Banned in shipped driver code:
//!
//! * constructing an `Evidence::Approval` — the only route into a run is a document a person wrote;
//! * constructing a `Producer::Human` — anything the driver mints for itself carries
//!   `Producer::Harness { id }`, which satisfies neither `independent: true` nor `human: true`.
//!
//! Counted rather than banned: `Producer::Verifier`, which is legitimate in **at most one** place —
//! a command-step evidence builder filling it from the verifier the step map named. Today this
//! crate builds no evidence at all (the `command` executor is `protocol-cli`'s and hands over a
//! finished `EvidenceSubmission`), so the count is zero and the bound is what keeps it from
//! becoming several.
//!
//! Deliberately allowed: **destructuring**. `let Evidence::Approval(approval) = ..` and match arms
//! read evidence, and reading is the driver's job — it has to be able to tell an approval from a
//! test result to report one.
//!
//! # Why a source scan
//!
//! The failure mode is not observable from inside a running test: a driver that minted an approval
//! would make every test that needed one *pass*. The scan's own extractor is therefore checked
//! against synthetic samples, so a scan that has silently stopped seeing constructions fails on
//! them instead of passing on everything.

use std::path::{Path, PathBuf};

/// Variant paths that may never be constructed in shipped driver code.
const BANNED: &[&str] = &["Evidence::Approval", "Producer::Human"];

/// A variant path that may be constructed in at most one place.
const BOUNDED: &str = "Producer::Verifier";

/// How many places may construct [`BOUNDED`].
const BOUND: usize = 1;

/// Every `.rs` file of this crate's `src/`, as `(path, contents)`, in a stable order.
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

/// Every construction of `token` in `text`, as line numbers.
///
/// Comment lines are skipped, so prose about the rule does not read as a breach of it. A tuple
/// variant is constructed when `(` follows and the line is not a `let` pattern or a match arm's
/// pattern; a struct variant is constructed when `{` follows, under the same two exceptions. A
/// longer path or a longer identifier — `EvidenceKind::Approval`, `Producer::HumanReview` — is not
/// this variant and is not counted.
fn constructions(text: &str, token: &str) -> Vec<usize> {
    let mut found = Vec::new();
    for (number, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("//") {
            continue;
        }
        for start in occurrences(line, token) {
            let after = line[start + token.len()..].trim_start();
            let before = &line[..start];
            if after.starts_with(':') || after.starts_with(|c: char| c.is_alphanumeric()) {
                continue;
            }
            let let_pattern = before
                .rfind("let ")
                .is_some_and(|at| !before[at..].contains('='));
            let arm_pattern = !before.contains("=>") && line[start..].contains("=>");
            if let_pattern || arm_pattern {
                continue;
            }
            if after.starts_with('(') || after.starts_with('{') {
                found.push(number + 1);
            }
        }
    }
    found
}

#[test]
fn the_driver_mints_no_approval_and_signs_as_no_person() {
    let mut scanned = 0;
    let mut violations = Vec::new();
    let mut verifiers = Vec::new();

    for (path, text) in sources() {
        scanned += 1;
        for token in BANNED {
            for line in constructions(&text, token) {
                violations.push(format!("{}:{line}: `{token}`", path.display()));
            }
        }
        for line in constructions(&text, BOUNDED) {
            verifiers.push(format!("{}:{line}", path.display()));
        }
    }

    assert!(
        scanned >= 6,
        "only {scanned} source files were scanned; the scan is looking in the wrong place"
    );
    assert!(
        violations.is_empty(),
        "the only route to an approval is a document a person wrote, and nothing below the driver \
         would stop a harness minting its own — `approval_recorded` does not check who granted \
         it. Shipped driver code constructs one here:\n{}",
        violations.join("\n")
    );
    assert!(
        verifiers.len() <= BOUND,
        "`{BOUNDED}` is what makes `independent: true` honestly satisfiable, so it belongs in one \
         place — the command-step evidence builder, filling it from the verifier the step map \
         named. It is constructed in {} places:\n{}",
        verifiers.len(),
        verifiers.join("\n")
    );
}

#[test]
fn the_scan_reads_constructions_and_not_patterns_or_prose() {
    let construction = "        let owed = Evidence::Approval(record);";
    assert_eq!(
        constructions(construction, "Evidence::Approval"),
        vec![1],
        "a variant expression is a finding"
    );

    let struct_variant = "        let producer = Producer::Human { id: who.clone() };";
    assert_eq!(
        constructions(struct_variant, "Producer::Human"),
        vec![1],
        "a struct-variant literal is a finding"
    );

    let let_else = "        let Evidence::Approval(approval) = &record.value else {";
    assert!(
        constructions(let_else, "Evidence::Approval").is_empty(),
        "destructuring reads evidence, which the driver has to be able to do"
    );

    let arm = "            Evidence::Approval(approval) => approval.decision,";
    assert!(
        constructions(arm, "Evidence::Approval").is_empty(),
        "a match arm's pattern reads evidence"
    );

    let arm_body = "            Kind::Sign => Producer::Human { id: whoever() },";
    assert_eq!(
        constructions(arm_body, "Producer::Human"),
        vec![1],
        "a construction in a match arm's *body* is not excused by the arrow before it"
    );

    let prose = "    // Never construct an Evidence::Approval(record) here: D3 forbids it.";
    assert!(
        constructions(prose, "Evidence::Approval").is_empty(),
        "prose about the rule is not a breach of it"
    );

    let vocabulary = "        let kind = EvidenceKind::Approval;";
    assert!(
        constructions(vocabulary, "Evidence::Approval").is_empty(),
        "`EvidenceKind` is vocabulary, not a payload"
    );

    let longer = "        let review = Producer::HumanReview { id };";
    assert!(
        constructions(longer, "Producer::Human").is_empty(),
        "a longer identifier is not this variant"
    );

    let verifier = "        Producer::Verifier { verifier: mapping.verifier.clone() }";
    assert_eq!(
        constructions(verifier, BOUNDED),
        vec![1],
        "the bounded construction must be visible to the scan, or the bound counts nothing"
    );
}
