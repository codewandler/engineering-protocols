//! Invariant 9's scan for this crate: no unordered map, no clock, no randomness.
//!
//! § 4.1 makes a purity claim for the driver that is stronger than `aep-engine`'s, and this is the
//! half of it that can be checked mechanically. The map's **digest** is what a resumed run is
//! compared against, so a `HashMap` here would make two builds of the same document disagree about
//! whether the map moved; and a step map that read a clock would make a run's routing depend on
//! when it was started.
//!
//! The scan is the one `ess-gen/tests/determinism.rs` uses — comment-skipping and boundary-aware,
//! with both refinements asserted against synthetic samples, so a scan that has stopped seeing
//! violations fails on them instead of passing on everything.

use std::path::Path;

/// What this crate must not mention in code.
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
        let trimmed = line.trim();
        if trimmed.starts_with("//") {
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
fn the_step_map_and_the_cursor_hold_no_unordered_map_and_read_no_clock() {
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
        checked >= 5,
        "only {checked} source files were read; the scan is looking in the wrong place"
    );
    assert!(
        violations.is_empty(),
        "a step map's digest is what a resumed run is compared against, and these lines can make \
         two builds of one document disagree:\n{}",
        violations.join("\n")
    );
}

#[test]
fn the_scan_sees_a_real_violation_and_ignores_prose_and_substrings() {
    assert_eq!(
        banned_uses("use std::collections::HashMap;"),
        vec![(1, "HashMap")],
        "a real use must trip the scan"
    );
    assert!(
        banned_uses("// a HashMap here and the digest wobbles").is_empty(),
        "a comment about the rule must not trip it"
    );
    assert!(
        banned_uses("let my_hash_map_like = MyHashMapLike::new();").is_empty(),
        "an identifier merely containing the token must not trip it"
    );
}
