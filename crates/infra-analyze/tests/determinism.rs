//! The determinism claims of the analysis crate, asserted rather than stated (invariant 9):
//! the graph document, the Mermaid text and the serialized diagnosis are byte-identical across
//! two runs over one IR, and the source scan keeps unordered maps and clocks out of the crate.

use std::path::Path;

use infra_analyze::{diagnose, GraphDocument, InfraGraph};
use infra_compiler::InfraIr;
use infra_domain::observation::Observation;
use infra_domain::raw::RawBundle;

fn example_ir() -> InfraIr {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/k3d-dev-cluster/observation.json");
    let text = std::fs::read_to_string(&path).expect("the committed observation is readable");
    let raw: RawBundle = serde_json::from_str(&text).expect("the committed observation is JSON");
    let observation = Observation::try_from(raw).expect("the committed observation is valid");
    infra_compiler::compile(&observation)
}

#[test]
fn two_graph_constructions_render_byte_identical_documents_and_diagrams() {
    let ir = example_ir();
    let first = InfraGraph::of(&ir);
    let second = InfraGraph::of(&ir);
    assert_eq!(
        GraphDocument::of(&first, &ir, None).to_json(),
        GraphDocument::of(&second, &ir, None).to_json(),
        "two graph documents of one IR must not differ in a single byte"
    );
    assert_eq!(
        first.mermaid(),
        second.mermaid(),
        "two diagrams of one IR must not differ in a single byte"
    );
}

#[test]
fn two_diagnoses_of_one_ir_serialize_byte_identically() {
    let ir = example_ir();
    let first = serde_json::to_string_pretty(&diagnose(&ir)).expect("a diagnosis serializes");
    let second = serde_json::to_string_pretty(&diagnose(&ir)).expect("a diagnosis serializes");
    assert_eq!(
        first, second,
        "two diagnoses of one IR must not differ in a single byte"
    );
}

#[test]
fn candidates_directions_and_the_html_page_render_byte_identically_across_two_runs() {
    use infra_analyze::{
        candidates, candidates_to_json, candidates_to_text, directions, directions_to_json,
        directions_to_text, properties_with, render_html,
    };

    let ir = example_ir();
    let render = || {
        let graph = InfraGraph::of(&ir);
        let diagnosis = diagnose(&ir);
        let mined = candidates(&ir);
        let ranked = directions(&diagnosis, &mined);
        let all = properties_with(&ir, &graph);
        (
            candidates_to_json(&mined),
            candidates_to_text(&mined),
            directions_to_json(&ranked),
            directions_to_text(&ranked),
            render_html(&graph, &diagnosis, &all, None),
            render_html(&graph, &diagnosis, &all, Some("shop")),
        )
    };
    assert_eq!(
        render(),
        render(),
        "no IW2.5 rendering may differ in a single byte between two runs"
    );
}

/// What a deterministic crate must not mention in code — the scan `infra-compiler` runs,
/// applied to this crate for the same claim.
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
fn the_analysis_uses_no_unordered_map_and_reads_no_clock() {
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
        "this crate promises byte-identical output for the same IR, and these lines can break \
         that between two runs:\n{}",
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
        banned_uses("// a HashMap here would wobble the rendering").is_empty(),
        "a comment about the rule must not trip it"
    );
}
