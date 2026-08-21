//! `protocol workflow render`, driven as the binary.
//!
//! These run the real executable, because the interface is the product: a person types
//! `protocol workflow render --id adp/default --format svg > figure.svg` and a harness shells out
//! to the same thing. Calling `aep_render::svg::render` from a test would not catch a flag that
//! never reaches it, a `--out` that writes nowhere, or an exit code that lies.
//!
//! # The run fixture is asserted to be the format the driver writes
//!
//! `--run` reads two documents out of a run directory, and this file writes those two documents by
//! hand. That is only a test of the renderer if the hand-written bytes are the bytes the driver
//! actually produces — so before any of them is written, each is **deserialised into the driver's
//! own type**. The day `DriverCursor` or `Snapshot` grows a required field, these tests fail there,
//! naming the field, rather than passing against a fixture that has quietly become fiction.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use aep_driver_spec::cursor::DriverCursor;
use aep_engine::execution::Snapshot;

/// The repository root, which is the document tree these tests render from.
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the workspace root exists")
}

/// Runs `protocol` with `args`, from the repository root.
fn protocol(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_protocol"))
        .args(args)
        .current_dir(root())
        .output()
        .expect("the protocol binary runs")
}

/// Standard output as a string.
fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// Standard error as a string.
fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// The exit code, which is part of the contract with a calling harness.
fn code(output: &Output) -> i32 {
    output.status.code().expect("the process exited normally")
}

/// An empty scratch directory to build a fixture in.
fn scratch(name: &str) -> PathBuf {
    let directory = std::env::temp_dir().join(name);
    std::fs::remove_dir_all(&directory).ok();
    std::fs::create_dir_all(&directory).expect("the temporary tree is writable");
    directory
}

/// A path as an argument.
fn printable(path: &Path) -> &str {
    path.to_str().expect("a printable path")
}

/// The engine's snapshot of a run that failed verification once and went back to `implement`.
///
/// One evidence record, so `--run` has an evidence count to draw and an empty table would not pass
/// for a working one.
///
/// It carries **two** times, and the fixture is written out rather than generated so that the
/// difference is visible: `produced_at` is when the record entered the log and `observed_at` is when
/// somebody looked. A snapshot written before the second field existed does not deserialize, and the
/// refusal names the field — which is the correct outcome, because a record from before the field
/// existed cannot say when it was observed and inventing a time for it is the defect the field
/// exists to remove.
const SNAPSHOT: &str = r#"{
  "execution": "AUTH-142.1",
  "task": "AUTH-142",
  "state": "implement",
  "entered": ["receive", "specify", "decompose", "establish_verifiers", "implement", "verify", "implement"],
  "evidence": [
    {
      "record": {
        "id": "ev-0001",
        "observed_at": 0,
        "produced_at": 0,
        "producer": { "producer": "verifier", "verifier": "test-runner" },
        "value": { "kind": "test_result", "suite": "unit", "passed": 34, "failed": 2 }
      },
      "state": "verify"
    }
  ],
  "events": [],
  "next_seq": 1
}
"#;

/// The driver's cursor beside it: the same run, blocked, with the engine's reasons.
const CURSOR: &str = r#"{
  "run": "AUTH-142/3",
  "task": "AUTH-142",
  "execution": "AUTH-142.1",
  "workflow": "adp/default/1",
  "map": "development/default",
  "map_digest": "0000000000000000000000000000000000000000000000000000000000000000",
  "engine_version": "0.1.0",
  "state": "implement",
  "step": 0,
  "visits": {
    "receive": 1, "specify": 1, "decompose": 1, "establish_verifiers": 1,
    "implement": 2, "verify": 1
  },
  "attempts": { "implement#0": 2 },
  "iterations": 7,
  "status": "blocked",
  "reasons": [
    "tests.unit.failed > 0 (observed 2)",
    "no transition out of `implement` is permitted: diff.exists is unknown"
  ]
}
"#;

/// Builds a project holding one run directory, and returns the project path.
///
/// Asserts the fixture is the driver's format before writing it; see the module documentation.
fn project_with_a_run(name: &str) -> PathBuf {
    let cursor: DriverCursor = serde_json::from_str(CURSOR)
        .expect("the cursor fixture is the format `aep-driver-spec` reads back");
    let snapshot: Snapshot = serde_json::from_str(SNAPSHOT)
        .expect("the snapshot fixture is the format `aep-engine` reads back");
    assert_eq!(
        cursor.state.as_str(),
        "implement",
        "the fixture must be sitting in the state the overlay is checked against"
    );
    assert_eq!(
        snapshot.entered.len(),
        7,
        "the fixture must have gone round the loop, or the retreat is untested"
    );
    assert_eq!(cursor.reasons.len(), 2);

    let project = scratch(name);
    let run = project.join(".engineering/runs/AUTH-142/3");
    std::fs::create_dir_all(&run).expect("the run directory is writable");
    std::fs::write(run.join("cursor.json"), CURSOR).expect("the cursor is writable");
    std::fs::write(run.join("snapshot.json"), SNAPSHOT).expect("the snapshot is writable");
    project
}

#[test]
fn a_workflow_renders_to_a_standalone_svg_document_on_standard_output() {
    let output = protocol(&[
        "workflow",
        "render",
        "--id",
        "adp/default",
        "--format",
        "svg",
    ]);
    assert_eq!(code(&output), 0, "{}", stderr(&output));
    let figure = stdout(&output);
    assert!(figure.starts_with("<svg viewBox="));
    assert!(figure.trim_end().ends_with("</svg>"));
    assert!(
        figure.contains("adp/default/1"),
        "the figure names the workflow it drew"
    );
    assert!(
        figure.contains("Adversarial verify"),
        "and every state of it"
    );
}

#[test]
fn the_same_workflow_renders_to_the_same_bytes_twice() {
    let first = protocol(&["workflow", "render", "--id", "adp/default"]);
    let second = protocol(&["workflow", "render", "--id", "adp/default"]);
    assert_eq!(code(&first), 0, "{}", stderr(&first));
    assert_eq!(
        first.stdout, second.stdout,
        "a committed figure that is regenerated must not turn up in a diff"
    );
}

#[test]
fn a_workflow_the_tree_does_not_declare_is_refused_by_name() {
    let output = protocol(&["workflow", "render", "--id", "adp/imaginary"]);
    assert_eq!(code(&output), 1);
    let reason = stderr(&output);
    assert!(
        reason.contains("adp/imaginary"),
        "the refusal names what was asked for: {reason}"
    );
    assert!(
        reason.contains("adp/default"),
        "and what the tree does declare: {reason}"
    );
}

#[test]
fn a_run_directory_paints_the_overlay_and_prints_its_reasons_verbatim() {
    let project = project_with_a_run("render-cli-run");
    let output = protocol(&[
        "workflow",
        "render",
        "--id",
        "adp/default",
        "--project",
        printable(&project),
        "--run",
        "AUTH-142/3",
        "--format",
        "tui",
    ]);
    assert_eq!(code(&output), 0, "{}", stderr(&output));
    let frame = stdout(&output);
    assert!(
        frame.contains("run AUTH-142/3"),
        "the frame names the run: {frame}"
    );
    assert!(frame.contains("blocked"), "and its status: {frame}");
    assert!(
        frame.contains("7 iteration(s)"),
        "the cursor's own counter reaches the picture: {frame}"
    );
    assert!(
        frame.contains("(taken 1×)"),
        "the verify -> implement retreat was taken once and the frame says so: {frame}"
    );
    assert!(
        frame.contains("×2"),
        "`implement` was entered twice: {frame}"
    );
    assert!(
        frame.contains("test_result ×1"),
        "the snapshot's evidence is counted by kind: {frame}"
    );
    for reason in [
        "tests.unit.failed > 0 (observed 2)",
        "no transition out of `implement` is permitted: diff.exists is unknown",
    ] {
        assert!(
            frame.contains(reason),
            "the engine's reason `{reason}` must reach the reader unedited: {frame}"
        );
    }
}

#[test]
fn a_run_id_with_no_directory_behind_it_is_refused_by_path() {
    let project = project_with_a_run("render-cli-missing-run");
    let output = protocol(&[
        "workflow",
        "render",
        "--id",
        "adp/default",
        "--project",
        printable(&project),
        "--run",
        "AUTH-142/9",
    ]);
    assert_eq!(code(&output), 1);
    let reason = stderr(&output);
    assert!(reason.contains("AUTH-142/9"), "{reason}");
    assert!(
        reason.contains("AUTH-142") && reason.contains('9'),
        "the refusal names the path it looked at: {reason}"
    );
}

#[test]
fn a_snapshot_on_its_own_draws_the_path_and_refuses_to_guess_a_status() {
    let directory = scratch("render-cli-state");
    let path = directory.join("snapshot.json");
    std::fs::write(&path, SNAPSHOT).expect("the snapshot is writable");
    let output = protocol(&[
        "workflow",
        "render",
        "--id",
        "adp/default",
        "--state",
        printable(&path),
        "--format",
        "tui",
    ]);
    assert_eq!(code(&output), 0, "{}", stderr(&output));
    let frame = stdout(&output);
    assert!(
        frame.contains("unknown"),
        "a snapshot carries no cursor, so the status is `unknown` and not `running`: {frame}"
    );
    assert!(
        frame.contains("task AUTH-142"),
        "there is no run id in a snapshot, so the task is what identifies it: {frame}"
    );
    assert!(
        frame.contains("×2"),
        "the visit counts are derived from the states entered: {frame}"
    );
    assert!(
        !frame.contains("blocked"),
        "and nothing invents a reason nobody recorded: {frame}"
    );
}

#[test]
fn the_html_page_is_written_whole_and_fetches_nothing() {
    let directory = scratch("render-cli-html");
    let page = directory.join("figure.html");
    let output = protocol(&[
        "workflow",
        "render",
        "--id",
        "adp/default",
        "--format",
        "html",
        "--out",
        printable(&page),
    ]);
    assert_eq!(code(&output), 0, "{}", stderr(&output));
    assert!(
        stdout(&output).contains("figure.html"),
        "the path is printed so a script can pick it up"
    );
    let text = std::fs::read_to_string(&page).expect("the page was written");
    assert!(text.starts_with("<!DOCTYPE html>"));
    assert!(text.contains("<svg viewBox="), "the figure is embedded");
    for forbidden in ["<link", "<img", "src=\"http", "href=\"http"] {
        assert!(
            !text.contains(forbidden),
            "a self-contained page must not carry `{forbidden}`"
        );
    }
}

#[test]
fn png_without_an_output_file_is_refused_and_names_the_flag_that_fixes_it() {
    let output = protocol(&[
        "workflow",
        "render",
        "--id",
        "adp/default",
        "--format",
        "png",
    ]);
    assert_eq!(code(&output), 1);
    let reason = stderr(&output);
    assert!(reason.contains("--out"), "{reason}");
    assert!(
        reason.contains("--format svg"),
        "and says what does go to standard output: {reason}"
    );
}

#[test]
fn png_without_the_rasteriser_names_the_program_and_what_to_install() {
    // An empty `PATH`, which is the only honest way to test the absence of a system tool on a
    // machine that has it: `rsvg-convert` is installed here, and a test that only ran where it is
    // missing would be a test that never ran.
    let empty = scratch("render-cli-no-tools");
    let out = empty.join("figure.png");
    let output = Command::new(env!("CARGO_BIN_EXE_protocol"))
        .args([
            "workflow",
            "render",
            "--id",
            "adp/default",
            "--format",
            "png",
            "--out",
            printable(&out),
        ])
        .current_dir(root())
        .env("PATH", printable(&empty))
        .output()
        .expect("the protocol binary runs");
    assert_eq!(code(&output), 1);
    let reason = stderr(&output);
    assert!(
        reason.contains("rsvg-convert"),
        "the refusal names the program: {reason}"
    );
    assert!(reason.contains("librsvg"), "and where to get it: {reason}");
    assert!(
        reason.contains("--format svg"),
        "and the way out that needs nothing: {reason}"
    );
    assert!(!out.exists(), "nothing was written");
}

#[test]
fn watch_is_refused_on_a_format_that_writes_a_document_once() {
    let output = protocol(&[
        "workflow",
        "render",
        "--id",
        "adp/default",
        "--format",
        "svg",
        "--watch",
    ]);
    assert_eq!(code(&output), 1);
    let reason = stderr(&output);
    assert!(
        reason.contains("--format tui"),
        "the refusal says which format can watch: {reason}"
    );
}

#[test]
fn watch_without_a_run_is_refused_because_there_would_be_nothing_to_follow() {
    let output = protocol(&[
        "workflow",
        "render",
        "--id",
        "adp/default",
        "--format",
        "tui",
        "--watch",
    ]);
    assert_eq!(code(&output), 1);
    assert!(stderr(&output).contains("--run"), "{}", stderr(&output));
}

#[test]
fn a_frame_written_to_a_file_carries_no_control_characters() {
    let directory = scratch("render-cli-frame");
    let path = directory.join("frame.txt");
    let output = protocol(&[
        "workflow",
        "render",
        "--id",
        "adp/default",
        "--format",
        "tui",
        "--out",
        printable(&path),
    ]);
    assert_eq!(code(&output), 0, "{}", stderr(&output));
    let text = std::fs::read_to_string(&path).expect("the frame was written");
    assert!(
        !text.contains('\u{1b}'),
        "a saved frame is text, not a terminal recording"
    );
    assert!(text.contains("Establish verifiers"));
}

#[test]
fn every_committed_workflow_renders() {
    // Four documents, and only one of them has a retreat: a layout that only ever met `adp/default`
    // would be a layout nobody had checked against a plain chain.
    for id in [
        "adp/default",
        "incident/standard",
        "release/progressive",
        "migration/forward-only",
    ] {
        let output = protocol(&["workflow", "render", "--id", id]);
        assert_eq!(code(&output), 0, "rendering {id}: {}", stderr(&output));
        assert!(
            stdout(&output).starts_with("<svg viewBox="),
            "{id} produced no figure"
        );
    }
}
