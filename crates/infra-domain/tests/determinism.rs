//! Invariant 9's scan for this crate: the model that feeds canonical bytes holds no unordered map.
//!
//! `infra-compiler` serializes validated types straight into the digested IR, so a `HashMap`
//! *here* would break the byte-identity `infra-compiler` promises — the same reasoning that has
//! `ess-domain`'s collections all ordered. The scan is the one `ess-gen/tests/determinism.rs`
//! uses: comment-skipping and boundary-aware, with both refinements asserted against synthetic
//! samples so a scan that stops seeing violations fails on them instead of passing on everything.

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
fn the_observation_model_uses_no_unordered_map_and_reads_no_clock() {
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
        "these types end up inside a content-addressed IR, and these lines can make its bytes \
         differ between two runs:\n{}",
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
        banned_uses("// iterate a HashMap here and the digest wobbles").is_empty(),
        "a comment about the rule must not trip it"
    );
    assert!(
        banned_uses("let my_hash_map_like = MyHashMapLike::new();").is_empty(),
        "an identifier merely containing the token must not trip it"
    );
}
