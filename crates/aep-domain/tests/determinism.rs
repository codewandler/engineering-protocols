//! Invariant 8, enforced rather than stated: the domain crate is clock-free and randomness-free.
//!
//! Replayability rests here. The engine takes a `Clock` precisely so that time is an input, and a
//! decision is a function of validated state plus evidence (invariant 9) only while nothing in
//! this crate can reach for `SystemTime::now`, an RNG, or a `HashMap` whose iteration order
//! changes between processes. Until this file the scan that would catch a violation covered
//! `ess-compiler` only; `crates/aep-domain/src` happened to be clean, which is a fact about today,
//! not a guard.
//!
//! The workspace's other deterministic crates carry the same scan beside their own determinism
//! tests: `ess-compiler` (`tests/billing.rs`), `ess-diff` (`tests/canonical.rs`), `ess-synth`
//! (`tests/synthesis.rs`) and `ess-gen` (`tests/determinism.rs`). Deliberately *not* scanned,
//! because each owns a clock on purpose: `aep-engine`, whose `src/clock.rs` is the one place
//! `SystemTime::now` is allowed to live, behind the `Clock` trait; and `ess-conformance`, whose
//! runner is fallible and takes a clock by decision 3 of the wave 3.5 reconciliation. `ess-domain`
//! states no determinism claim of its own, so it is not scanned — the claim, not the crate list,
//! is what this test enforces.
//!
//! # Why the scan is not a plain `contains`
//!
//! Two lessons from pointing the older scans at this crate. `Operand::` contains the substring
//! `rand::`, so the match requires an identifier boundary. And prose is allowed to name the banned
//! tokens — this file does, and so do doc comments in the crates that explain the rule — so
//! comment lines are skipped, the same choice `crates/aep-domain/tests/invariants.rs` records for
//! `Deserialize`. Both refinements are asserted against synthetic samples below, so the scan
//! cannot silently rot into one that sees nothing.

use std::path::Path;

/// What a deterministic crate must not mention in code.
///
/// The union of the tokens the sibling scans ban: unordered maps (iteration order varies per
/// process), both clock reads, and every spelling of randomness the standard ecosystem offers.
const BANNED: &[&str] = &[
    "HashMap",
    "HashSet",
    "SystemTime",
    "Instant::now",
    "rand::",
    "getrandom",
    "thread_rng",
];

/// Every banned token `text` uses in code, as `(line number, token)`.
///
/// Comment lines are skipped and each match must start on an identifier boundary; see the module
/// documentation for why both refinements exist.
fn banned_uses(text: &str) -> Vec<(usize, &'static str)> {
    let mut found = Vec::new();
    for (number, line) in text.lines().enumerate() {
        if line.trim().starts_with("//") {
            continue;
        }
        for token in BANNED {
            let mut from = 0;
            while let Some(at) = line[from..].find(token) {
                let start = from + at;
                let boundary = line[..start]
                    .chars()
                    .next_back()
                    .is_none_or(|before| !before.is_alphanumeric() && before != '_');
                if boundary {
                    found.push((number + 1, *token));
                }
                from = start + token.len();
            }
        }
    }
    found
}

#[test]
fn the_domain_crate_reads_no_clock_no_randomness_and_no_unordered_map() {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut checked = 0;
    let mut violations = Vec::new();
    for entry in std::fs::read_dir(&directory).expect("the crate has sources") {
        let path = entry.expect("an entry").path();
        if path.extension().is_none_or(|it| it != "rs") {
            continue;
        }
        let text = std::fs::read_to_string(&path).expect("a readable source file");
        for (line, token) in banned_uses(&text) {
            violations.push(format!("{}:{line}: `{token}`", path.display()));
        }
        checked += 1;
    }
    assert!(
        checked >= 20,
        "only {checked} source files were read; the scan is looking in the wrong place"
    );
    assert!(
        violations.is_empty(),
        "invariant 8: the domain crate is clock-free and randomness-free, and invariant 9 needs \
         its iteration orders stable. Found:\n{}\nTime is an input here — take a timestamp as a \
         parameter, keep collections `BTreeMap`/`BTreeSet`",
        violations.join("\n")
    );
}

#[test]
fn the_determinism_scan_sees_code_and_not_prose_and_not_operand() {
    let clock = "    let started = SystemTime::now();";
    assert_eq!(
        banned_uses(clock),
        vec![(1, "SystemTime")],
        "a clock read is a finding"
    );

    let map = "    use std::collections::HashMap;\n    let mut order = HashMap::new();";
    assert_eq!(
        banned_uses(map).len(),
        2,
        "an unordered map is a finding on the import and on the use"
    );

    let rng = "    let choice = rand::random::<u64>();";
    assert_eq!(banned_uses(rng), vec![(1, "rand::")], "an RNG is a finding");

    let operand = "                left: Operand::Fact(path),";
    assert!(
        banned_uses(operand).is_empty(),
        "`Operand::` contains `rand::` and is exactly why the match requires a boundary"
    );

    let prose = "    // Never a HashMap here: iteration order must not depend on the process.";
    assert!(
        banned_uses(prose).is_empty(),
        "prose about the rule is not a breach of it"
    );
}
