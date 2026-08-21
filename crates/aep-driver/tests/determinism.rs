//! Invariant 9's scan for this crate: no unordered map, no clock, no randomness.
//!
//! § 4.1 makes a purity claim for the driver — *clock-free and randomness-free, the same discipline
//! `aep-domain` holds under invariant 8* — and this is the half of it that can be checked
//! mechanically. It is worth more here than in most crates because the thing being claimed is
//! **replayability**: given the same snapshot and the same evidence, the same routing. A `HashMap`
//! in the router would make the order two builds walk a state's steps in a coin flip, and a clock
//! would make a run's routing depend on when it was started.
//!
//! What the scan cannot see is placed rather than banned: a pid-liveness probe reads ambient OS
//! state and uses none of these tokens, which is why the probe lives in `protocol-cli` and this
//! crate is handed a `LockState` (review finding **F19**). A scan is a floor, not the claim.
//!
//! The scan itself is `aep-driver-spec`'s, which is `ess-gen`'s — comment-skipping and
//! boundary-aware, with both refinements asserted against synthetic samples, so a scan that has
//! stopped seeing violations fails on them instead of passing on everything.

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
fn the_router_holds_no_unordered_map_and_reads_no_clock() {
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
        checked >= 6,
        "only {checked} source files were read; the scan is looking in the wrong place"
    );
    assert!(
        violations.is_empty(),
        "the replay claim is that the same snapshot and the same evidence yield the same routing, \
         and these lines can make two runs disagree:\n{}",
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
        banned_uses("// a HashMap here and two runs route differently").is_empty(),
        "a comment about the rule must not trip it"
    );
    assert!(
        banned_uses("let my_hash_map_like = MyHashMapLike::new();").is_empty(),
        "an identifier merely containing the token must not trip it"
    );
    assert_eq!(
        banned_uses("        let now = SystemTime::now();"),
        vec![(1, "SystemTime")],
        "a clock read is the other half of what this scan is for"
    );
}
