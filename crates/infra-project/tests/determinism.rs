//! The determinism claim, asserted rather than stated (invariant 9): the same specification and
//! snapshot produce the same tree, byte for byte, and the source scan keeps unordered maps and
//! clocks out of the crate.
//!
//! A projection is committed to a repository and reviewed as a diff. A tree whose bytes move
//! because a `HashMap` iterated differently would show up as a change somebody has to read, and
//! the second time it happens nobody reads any of them.

mod support;

use std::path::Path;

#[test]
fn two_projections_of_one_specification_and_snapshot_are_byte_identical() {
    let spec = support::example_spec();
    let ir = support::example_ir();
    let render = || {
        let projection = infra_project::project(&spec, &ir);
        (
            projection.to_json(),
            projection.artifacts(),
            infra_project::projection_to_text(&projection),
        )
    };
    assert_eq!(
        render(),
        render(),
        "two projections of one pair must not differ in a single byte"
    );
}

#[test]
fn shuffling_a_bundles_items_changes_no_byte_of_the_tree() {
    // Every map in the IR is keyed by identity, so the order a scanner happened to write its items
    // in must not reach a patch file — least of all a container list, which the emitter sorts by
    // name for exactly this reason.
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
        infra_project::project(&spec, &shuffled).to_json(),
        infra_project::project(&spec, &support::example_ir()).to_json(),
        "a reversed bundle is the same cluster, and must project to the same bytes"
    );
}

#[test]
fn the_committed_projection_tree_is_what_the_library_produces_right_now() {
    // The gate's `cargo xtask infra --check` compares the CLI's stdout against the committed tree;
    // this asserts the *library* produces the same bytes, so a drift between the two producers
    // fails here rather than turning up as an unexplained diff under `projection/`.
    let projection = infra_project::project(&support::example_spec(), &support::example_ir());
    for (path, contents) in projection.artifacts() {
        let committed = support::read(&format!("examples/k3d-dev-cluster/projection/{path}"));
        assert_eq!(
            contents, committed,
            "examples/k3d-dev-cluster/projection/{path} is stale; run `cargo xtask infra` and \
             review the diff"
        );
    }
}

#[test]
fn every_file_in_the_committed_tree_is_one_the_library_still_produces() {
    // The other direction, which a comparison of what *is* generated can never see: a patch file
    // for an object nothing patches any more would sit in the repository looking like a proposal
    // somebody still stands behind.
    let projection = infra_project::project(&support::example_spec(), &support::example_ir());
    let produced = projection.artifacts();
    let root = support::root().join("examples/k3d-dev-cluster/projection");
    let mut committed = Vec::new();
    collect(&root, "", &mut committed);
    committed.sort();
    let mut expected: Vec<String> = produced.keys().cloned().collect();
    expected.sort();
    assert_eq!(
        committed, expected,
        "the committed tree and the produced one hold different files; run `cargo xtask infra`"
    );
}

/// Every file under `directory`, as paths relative to it.
fn collect(directory: &Path, prefix: &str, into: &mut Vec<String>) {
    for entry in std::fs::read_dir(directory).expect("the committed tree exists") {
        let entry = entry.expect("an entry");
        let name = entry.file_name().to_string_lossy().into_owned();
        let relative = if prefix.is_empty() {
            name
        } else {
            format!("{prefix}/{name}")
        };
        if entry.file_type().expect("a file type").is_dir() {
            collect(&entry.path(), &relative, into);
        } else {
            into.push(relative);
        }
    }
}

/// What a deterministic crate must not mention in code — the scan the other four infrastructure
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
fn the_projection_crate_uses_no_unordered_map_and_reads_no_clock() {
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
        checked >= 4,
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
        banned_uses("// a HashMap here would wobble the tree").is_empty(),
        "a comment about the rule must not trip it"
    );
}
