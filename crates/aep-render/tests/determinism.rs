//! Invariant 9's scan for this crate: no unordered map, no clock, no randomness — and no floats.
//!
//! The renderer's acceptance criterion is stronger than "the same decision twice": the same
//! workflow and the same `RunView` must produce **byte-identical** output, because a figure that is
//! generated, committed and regenerated must not turn up in a diff. A `HashMap` over states would
//! reorder the boxes between builds of the same binary; a clock in `ansi::frame` would make a
//! terminal frame depend on when it was drawn, which is exactly what `--watch` must not be.
//!
//! Floats are on the list for the same reason and it is the one addition to the house list.
//! Coordinates here are `i32` by design — `f32` arithmetic plus `f32` formatting is two places for
//! two builds to disagree about `190.00000001`, and a diagram needs no sub-pixel precision to be
//! worth looking at.
//!
//! The scan itself is the one `aep-driver-spec/tests/determinism.rs` uses — comment-skipping and
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
    // The loop, the poll interval and the terminal belong to `protocol-cli`, and this is what
    // keeps them there rather than merely asking.
    "sleep",
    // Ambient reads. `env!` is a compile-time macro and is spelled differently, so it survives.
    "std::env",
    "var_os",
    // Coordinates are integers; see the module documentation.
    "f32",
    "f64",
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
fn the_renderer_holds_no_unordered_map_reads_no_clock_and_positions_nothing_with_a_float() {
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
        checked >= 7,
        "only {checked} source files were read; the scan is looking in the wrong place"
    );
    assert!(
        violations.is_empty(),
        "a rendered figure is committed and regenerated, and these lines can make two runs of one \
         binary disagree about its bytes:\n{}",
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
    assert_eq!(
        banned_uses("let x: f64 = 1.0;"),
        vec![(1, "f64")],
        "a float coordinate must trip it"
    );
    assert!(
        banned_uses("// a HashMap here and two renderings disagree").is_empty(),
        "a comment about the rule must not trip it"
    );
    assert!(
        banned_uses("/// Positions are never `f64`.").is_empty(),
        "a doc comment about the rule must not trip it either"
    );
    assert!(
        banned_uses("let my_hash_map_like = MyHashMapLike::new();").is_empty(),
        "an identifier merely containing the token must not trip it"
    );
}
