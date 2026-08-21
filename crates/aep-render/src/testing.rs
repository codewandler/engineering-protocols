//! Fixtures the crate's own tests share.
//!
//! Compiled only under `cfg(test)`. Two kinds of fixture live here and the difference matters:
//!
//! * [`fixture_workflow`] reads `workflows/development/default.yaml` — the repository's **real**
//!   workflow, through the same `RawWorkflow` → `TryFrom` path the loader uses. A snapshot test
//!   against a hand-written copy of that document tests the copy; against the document itself it
//!   tests the renderer, and it fails loudly the day somebody adds a state.
//! * [`workflow_with`] builds a small synthetic graph for the layout cases the committed workflows
//!   do not contain — a diamond, a parallel branch — because a layering rule that only ever meets a
//!   chain is a rule nothing has checked.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use aep_domain::ids::StateId;
use aep_domain::workflow::{RawWorkflow, Workflow};

use crate::run::{RunStatus, RunView};

/// A state id, or a panic naming the fixture that is wrong.
pub fn state(id: &str) -> StateId {
    StateId::new(id).expect("the fixture declares a legal state id")
}

/// The repository root, from this crate's manifest.
pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the workspace root exists")
}

/// `adp/default`, read from the document the repository ships.
pub fn fixture_workflow() -> Workflow {
    workflow_at("workflows/development/default.yaml")
}

/// The workflow document at `relative`, validated.
pub fn workflow_at(relative: &str) -> Workflow {
    let path = repo_root().join(relative);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("reading {}: {error}", path.display()));
    let raw: RawWorkflow =
        serde_yaml::from_str(&text).unwrap_or_else(|error| panic!("parsing {relative}: {error}"));
    Workflow::try_from(raw).unwrap_or_else(|errors| panic!("validating {relative}: {errors}"))
}

/// A synthetic workflow over `states`, wired by `edges`, ending at `terminal`.
///
/// Written as a document and validated, rather than assembled as a struct literal: invariant 2 says
/// a validated type is obtained by validating, and a fixture that dodged that could describe a
/// workflow the loader would refuse.
pub fn workflow_with(states: &[&str], edges: &[(&str, &str)], terminal: &str) -> Workflow {
    let mut document = String::from("id: test/synthetic\nversion: 1\ntitle: Synthetic\ninitial: ");
    document.push_str(states.first().expect("at least one state"));
    document.push_str("\nstates:\n");
    for state in states {
        let _ = writeln!(document, "  {state}:\n    title: {state}");
        if *state == terminal {
            document.push_str("    terminal: true\n");
        }
    }
    document.push_str("transitions:\n");
    for (from, to) in edges {
        let _ = writeln!(document, "  - from: {from}\n    to: {to}");
    }
    let raw: RawWorkflow = serde_yaml::from_str(&document).expect("the fixture document parses");
    Workflow::try_from(raw).expect("the fixture document validates")
}

/// Compares `produced` against the committed snapshot `name`, or writes it when there is none.
///
/// # How to update one
///
/// Delete the file and run the tests again: a missing snapshot is written and the test fails once,
/// naming the file, so a regeneration is always a reviewable diff rather than a silent overwrite.
/// There is deliberately **no environment variable** that rewrites snapshots in place — this crate
/// scans its own sources for ambient reads, and a test helper that quietly accepted whatever the
/// code now produces is how a golden file stops guarding anything.
///
/// The failure names the first line that differs, because `assert_eq!` on a 200-line SVG prints two
/// walls of angle brackets and tells the reader nothing.
pub fn snapshot(name: &str, produced: &str) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join(name);
    let Ok(expected) = std::fs::read_to_string(&path) else {
        std::fs::create_dir_all(path.parent().expect("the snapshot has a directory"))
            .expect("the fixture directory is writable");
        std::fs::write(&path, produced).expect("the snapshot is writable");
        panic!(
            "no snapshot at {}; it has been written — review it and run again",
            path.display()
        );
    };
    if expected == produced {
        return;
    }
    let (line, want, got) = expected
        .lines()
        .zip(produced.lines())
        .enumerate()
        .find(|(_, (want, got))| want != got)
        .map_or_else(
            || {
                (
                    expected.lines().count().min(produced.lines().count()) + 1,
                    "<end of file>",
                    "<more lines>",
                )
            },
            |(index, (want, got))| (index + 1, want, got),
        );
    panic!(
        "{} differs at line {line}\n  committed: {want}\n  produced:  {got}\n\
         delete the file and re-run to accept the new rendering",
        path.display()
    );
}

/// A run of `adp/default` that failed verification once and is back in `implement`, blocked.
///
/// Every overlay rule the plan names is exercised by this one fixture: a current state, states
/// behind it, the `verify → implement` retreat actually taken, evidence counted by kind, and two
/// reasons that must reach the output unedited.
pub fn mid_run() -> RunView {
    RunView {
        run: Some("AUTH-142/3".to_owned()),
        task: Some("AUTH-142".to_owned()),
        status: RunStatus::Blocked,
        current: Some(state("implement")),
        path: vec![
            state("receive"),
            state("specify"),
            state("decompose"),
            state("establish_verifiers"),
            state("implement"),
            state("verify"),
            state("implement"),
        ],
        visits: [
            (state("receive"), 1),
            (state("specify"), 1),
            (state("decompose"), 1),
            (state("establish_verifiers"), 1),
            (state("implement"), 2),
            (state("verify"), 1),
        ]
        .into_iter()
        .collect(),
        evidence: [
            ("test_result".to_owned(), 3),
            ("static_analysis".to_owned(), 1),
        ]
        .into_iter()
        .collect(),
        reasons: vec![
            "tests.unit.failed > 0 (observed 2)".to_owned(),
            "no transition out of `implement` is permitted: diff.exists is unknown".to_owned(),
        ],
        iterations: Some(7),
    }
}
