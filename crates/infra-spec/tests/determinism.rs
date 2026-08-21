//! The determinism claims of the desired-state crate, asserted rather than stated (invariant 9):
//! the simulation document, the drift document and both text renderings are byte-identical
//! across two runs, and the source scan keeps unordered maps and clocks out of the crate.

mod support;

use std::path::Path;

use infra_spec::{drift, drift_to_text, simulate, simulation_to_text};

#[test]
fn two_simulations_of_one_specification_and_snapshot_are_byte_identical() {
    let spec = support::example_spec();
    let ir = support::example_ir();
    let render = || {
        let simulation = simulate(&spec, &ir);
        (simulation.to_json(), simulation_to_text(&simulation))
    };
    assert_eq!(
        render(),
        render(),
        "two simulations of one pair must not differ in a single byte"
    );
}

#[test]
fn two_drift_reports_of_one_pair_are_byte_identical() {
    let before = support::example_ir();
    let after = support::drifted_ir();
    let render = || {
        let report = drift(&before, &after).expect("one cluster");
        (report.to_json(), drift_to_text(&report))
    };
    assert_eq!(
        render(),
        render(),
        "two drift reports of one pair must not differ in a single byte"
    );
}

#[test]
fn the_committed_documents_are_what_the_library_produces_right_now() {
    // The gate's `cargo xtask infra --check` compares the CLI's stdout against these files; this
    // asserts the *library* produces the same bytes, so a drift between the two producers fails
    // here rather than turning up as an unexplained diff in the committed report.
    let simulation = simulate(&support::example_spec(), &support::example_ir());
    assert_eq!(
        simulation.to_json(),
        support::read("examples/k3d-dev-cluster/simulation.json"),
        "run `cargo xtask infra` and review the diff"
    );
    let report = drift(&support::example_ir(), &support::drifted_ir()).expect("one cluster");
    assert_eq!(
        report.to_json(),
        support::read("examples/k3d-dev-cluster/drift.json"),
        "run `cargo xtask infra` and review the diff"
    );
}

#[test]
fn shuffling_a_bundles_items_changes_no_byte_of_either_document() {
    // Every map in the IR is keyed by identity, so the order a scanner happened to write its
    // items in must not reach a report.
    let original = support::read("examples/k3d-dev-cluster/observation.json");
    let mut document: serde_json::Value = serde_json::from_str(&original).expect("JSON");
    let kinds = document["kinds"].as_object_mut().expect("a kinds map");
    for (_, kind) in kinds.iter_mut() {
        if let Some(items) = kind["items"].as_array_mut() {
            items.reverse();
        }
    }
    let shuffled = support::compile(&document.to_string());
    let spec = support::example_spec();
    assert_eq!(
        simulate(&spec, &shuffled).to_json(),
        simulate(&spec, &support::example_ir()).to_json(),
        "a reversed bundle is the same cluster, and must simulate to the same bytes"
    );
    assert_eq!(
        drift(&shuffled, &support::drifted_ir())
            .expect("one cluster")
            .to_json(),
        drift(&support::example_ir(), &support::drifted_ir())
            .expect("one cluster")
            .to_json(),
        "a reversed bundle must drift to the same bytes"
    );
}

/// What a deterministic crate must not mention in code — the scan the other three infrastructure
/// crates run, applied here for the same claim.
const BANNED: &[&str] = &[
    "HashMap",
    "HashSet",
    "SystemTime",
    "Instant::now",
    "rand::",
    "getrandom",
    "thread_rng",
];

/// Every banned token `text` uses in code, as `(line number, token)` — comment-skipping and
/// boundary-aware, so prose about the rule is not a breach of it.
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
fn the_desired_state_crate_uses_no_unordered_map_and_reads_no_clock() {
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
        "this crate promises byte-identical output for the same inputs, and these lines can \
         break that between two runs:\n{}",
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
        banned_uses("// a HashMap here would wobble the report").is_empty(),
        "a comment about the rule must not trip it"
    );
}

#[test]
fn nothing_in_this_crate_can_read_a_wall_clock_because_no_expectation_names_a_duration() {
    // Review finding I7, kept enforceable rather than promised. The banned-token scan above
    // already forbids the clock; this forbids the *vocabulary* that would make one necessary, so
    // a future expectation kind called `observed_within` fails here before it is implemented.
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut violations = Vec::new();
    for entry in std::fs::read_dir(&directory).expect("the crate has sources") {
        let path = entry.expect("an entry").path();
        if path.extension().is_none_or(|it| it != "rs") {
            continue;
        }
        let text = std::fs::read_to_string(&path).expect("a readable source file");
        for (number, line) in text.lines().enumerate() {
            if line.trim().starts_with("//") {
                continue;
            }
            for token in ["Duration", "elapsed", "scanned_at", "Timestamp"] {
                if line.contains(token) {
                    violations.push(format!("{}:{}: `{token}`", path.display(), number + 1));
                }
            }
        }
    }
    assert!(
        violations.is_empty(),
        "every expectation here is snapshot-relative; these lines reach for a clock:\n{}",
        violations.join("\n")
    );
}
