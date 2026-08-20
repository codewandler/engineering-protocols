//! Invariant 8's scan for this crate: the generators read no clock and no unordered map.
//!
//! `src/lib.rs` claims it outright — "Same IR in, byte-identical bytes out. No clock, no RNG,
//! `BTreeMap`/`BTreeSet` only" — and each generator has a test that generates twice and compares
//! bytes. That test observes the property on today's code paths; this scan is what keeps the claim
//! load-bearing when a new code path is added, which is the same division of labour
//! `ess-compiler` (`tests/billing.rs`), `ess-diff` (`tests/canonical.rs`) and `ess-synth`
//! (`tests/synthesis.rs`) already practise on themselves.
//!
//! Unlike those three, this scan skips comment lines and requires an identifier boundary: this
//! crate's own doc comments say "no `HashMap`" in two places (`src/docs.rs`, `src/graph.rs`),
//! and prose about the rule must not read as a breach of it — the choice
//! `crates/aep-domain/tests/invariants.rs` records for `Deserialize`. Both refinements are
//! asserted against synthetic samples below.

use std::path::Path;

/// What a deterministic crate must not mention in code.
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
fn no_generator_reads_a_clock_or_an_unordered_map() {
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
        checked >= 8,
        "only {checked} source files were read; the scan is looking in the wrong place"
    );
    assert!(
        violations.is_empty(),
        "this crate promises byte-identical output for the same IR, and these lines can break \
         that between two runs:\n{}",
        violations.join("\n")
    );
}

#[test]
fn the_determinism_scan_sees_code_and_not_prose() {
    let clock = "    let stamp = SystemTime::now();";
    assert_eq!(
        banned_uses(clock),
        vec![(1, "SystemTime")],
        "a clock read is a finding"
    );

    let map = "    let mut index = HashMap::new();";
    assert_eq!(
        banned_uses(map),
        vec![(1, "HashMap")],
        "an unordered map is a finding"
    );

    let boundary = "                left: Operand::Fact(path),";
    assert!(
        banned_uses(boundary).is_empty(),
        "`Operand::` contains `rand::`; the match requires an identifier boundary"
    );

    let prose = "//! No clock, no RNG, no `HashMap`. Every list is a `BTreeMap` iteration.";
    assert!(
        banned_uses(prose).is_empty(),
        "this crate's own doc comments say `HashMap` while forbidding it; prose is not a breach"
    );
}
