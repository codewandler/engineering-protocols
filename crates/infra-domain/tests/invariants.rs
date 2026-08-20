//! Invariant 2's scan for this crate: parse, then validate — and no way around it.
//!
//! The raw half (`src/raw.rs`) is the only module that may implement
//! [`Deserialize`](serde::Deserialize); everything else holds validated types, and a validated
//! type that deserializes is a validation that can be skipped. The scan is source-level, the
//! mechanism `crates/aep-domain/tests/invariants.rs` weighed against `trybuild` and chose for
//! costing no dependency; like there, it skips comment lines, because this crate's docs *talk*
//! about `Deserialize` in exactly the module docs that must not implement it.
//!
//! The inverse is asserted too: the same extractor must *find* the derive in `raw.rs`, so a scan
//! that silently stops seeing derives fails on the module that has them instead of passing on
//! everything.

use std::path::Path;

/// Lines of `text` that mention `Deserialize` outside comments, as 1-based numbers.
fn deserialize_mentions(text: &str) -> Vec<usize> {
    text.lines()
        .enumerate()
        .filter(|(_, line)| {
            let trimmed = line.trim();
            !trimmed.starts_with("//") && trimmed.contains("Deserialize")
        })
        .map(|(number, _)| number + 1)
        .collect()
}

#[test]
fn only_the_raw_module_deserializes_so_the_only_way_to_a_validated_type_runs_the_rules() {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut checked = 0;
    let mut violations = Vec::new();
    let mut raw_derives = 0;
    for entry in std::fs::read_dir(&directory).expect("the crate has sources") {
        let path = entry.expect("an entry").path();
        if path.extension().is_none_or(|it| it != "rs") {
            continue;
        }
        let text = std::fs::read_to_string(&path).expect("a readable source file");
        let mentions = deserialize_mentions(&text);
        if path.file_name().is_some_and(|name| name == "raw.rs") {
            raw_derives = mentions.len();
        } else {
            for line in mentions {
                violations.push(format!("{}:{line}", path.display()));
            }
        }
        checked += 1;
    }
    assert!(
        checked >= 6,
        "only {checked} source files were read; the scan is looking in the wrong place"
    );
    assert!(
        raw_derives > 0,
        "the extractor found no `Deserialize` in raw.rs, which derives it dozens of times — \
         the scan has stopped seeing derives and cannot be trusted about the other modules"
    );
    assert!(
        violations.is_empty(),
        "a validated type must not deserialize — the only way in is `TryFrom`, where the rules \
         run. These lines put `Deserialize` outside src/raw.rs:\n{}",
        violations.join("\n")
    );
}

#[test]
fn the_extractor_skips_comments_because_prose_about_the_rule_is_not_a_breach_of_it() {
    assert!(
        deserialize_mentions("//! validated types do not implement Deserialize").is_empty(),
        "a doc comment mentioning the rule must not trip the scan"
    );
    assert_eq!(
        deserialize_mentions("#[derive(serde::Deserialize)]"),
        vec![1],
        "a real derive must trip it"
    );
}
