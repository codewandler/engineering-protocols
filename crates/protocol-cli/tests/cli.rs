//! CLI integration tests.
//!
//! These drive the real binary, because the interface is the product here: a harness shells out to
//! `protocol` and reads its exit code. Testing the library instead would not catch an argument that
//! never reaches it.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// The repository root.
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the workspace root exists")
}

/// Runs `protocol` with `args`, always against the repository's own document tree.
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

const TASK: &str = "examples/development-passkeys/task.yaml";
const ARTIFACTS: &str = "examples/development-passkeys/artifacts.yaml";

#[test]
fn validate_accepts_the_repositorys_own_documents() {
    let output = protocol(&["validate"]);
    assert_eq!(code(&output), 0, "{}", stderr(&output));
    let text = stdout(&output);
    assert!(text.contains("valid"), "{text}");
    assert!(text.contains("protocol(s)"), "{text}");
}

#[test]
fn validate_reports_a_broken_document_with_its_path_and_fails() {
    let directory = std::env::temp_dir().join("aep-cli-broken-tree/workflows");
    std::fs::create_dir_all(&directory).expect("the temporary tree is writable");
    let file = directory.join("broken.yaml");
    std::fs::write(
        &file,
        "id: broken\ntitle: Broken\ninitial: nowhere\nstates:\n  a:\n    title: A\n    terminal: true\n",
    )
    .expect("the fixture is writable");

    let output = protocol(&[
        "validate",
        "--root",
        directory
            .parent()
            .expect("the tree root")
            .to_str()
            .expect("a printable path"),
    ]);
    assert_eq!(code(&output), 1);
    let text = stdout(&output);
    assert!(
        text.contains("broken.yaml"),
        "the path must be in the report: {text}"
    );
    assert!(text.contains("unknown_initial_state"), "{text}");

    std::fs::remove_dir_all(directory.parent().expect("the tree root")).ok();
}

#[test]
fn validate_checks_an_artifact_manifest_against_the_lifecycles() {
    let output = protocol(&["validate", "--artifacts", ARTIFACTS]);
    assert_eq!(code(&output), 0, "{}", stderr(&output));
    assert!(stdout(&output).contains("valid"));
}

#[test]
fn resolve_prints_the_plan() {
    let output = protocol(&["resolve", "--task", TASK]);
    assert_eq!(code(&output), 0, "{}", stderr(&output));
    let text = stdout(&output);
    assert!(text.contains("development.standard"), "{text}");
    assert!(text.contains("adp/default"), "{text}");
    assert!(text.contains("test-driven"), "{text}");
    assert!(
        text.contains("requires_approval"),
        "the capability summary must show what is gated: {text}"
    );
}

#[test]
fn resolve_fails_when_the_task_names_a_profile_that_does_not_exist() {
    let path = std::env::temp_dir().join("aep-cli-bad-task.yaml");
    std::fs::write(
        &path,
        "id: T-1\nkind: feature\nobjective: nothing\nprotocol: adp/1\nprofile: development.imaginary\n",
    )
    .expect("the fixture is writable");

    let output = protocol(&[
        "resolve",
        "--task",
        path.to_str().expect("a printable path"),
    ]);
    assert_eq!(code(&output), 1);
    assert!(
        stderr(&output).contains("development.imaginary"),
        "{}",
        stderr(&output)
    );
    std::fs::remove_file(&path).ok();
}

#[test]
fn evaluate_reports_the_state_and_why_a_transition_is_blocked() {
    let output = protocol(&["evaluate", "--task", TASK]);
    assert_eq!(code(&output), 0, "{}", stderr(&output));
    let text = stdout(&output);
    assert!(text.contains("state       receive"), "{text}");
    assert!(text.contains("Task incomplete"), "{text}");
}

#[test]
fn evaluate_advances_with_the_examples_evidence() {
    let output = protocol(&[
        "evaluate",
        "--task",
        TASK,
        "--artifacts",
        ARTIFACTS,
        "--evidence",
        "examples/development-passkeys/evidence/01-red-test.yaml",
        "--advance",
    ]);
    assert_eq!(code(&output), 0, "{}", stderr(&output));
    let text = stdout(&output);
    assert!(
        text.contains("state       implement"),
        "a failing test and an approved specification are enough to reach implementation: {text}"
    );
}

#[test]
fn evaluate_reads_every_evidence_file_in_the_example() {
    let output = protocol(&[
        "evaluate",
        "--task",
        TASK,
        "--artifacts",
        ARTIFACTS,
        "--evidence",
        "examples/development-passkeys/evidence/01-red-test.yaml",
        "--evidence",
        "examples/development-passkeys/evidence/02-implementation.yaml",
        "--evidence",
        "examples/development-passkeys/evidence/03-verification.yaml",
        "--evidence",
        "examples/development-passkeys/evidence/04-review.yaml",
        "--advance",
    ]);
    assert_eq!(code(&output), 0, "{}", stderr(&output));
    let text = stdout(&output);
    assert!(
        text.contains("adversarial_verify") || text.contains("review"),
        "the example's evidence should carry the work well past implementation: {text}"
    );
}

#[test]
fn explain_refuses_a_production_change_and_names_the_rule() {
    let output = protocol(&[
        "explain",
        "--task",
        TASK,
        "--artifacts",
        ARTIFACTS,
        "--action",
        "production.write",
    ]);
    assert_eq!(code(&output), 1, "a refusal is a non-zero exit");
    let text = stdout(&output);
    assert!(text.contains("production.write denied"), "{text}");
    assert!(text.contains("requires-approval"), "{text}");
    assert!(
        text.contains("approval for capability production.write"),
        "{text}"
    );
}

#[test]
fn explain_allows_what_the_profile_grants() {
    let output = protocol(&["explain", "--task", TASK, "--action", "tests.execute"]);
    assert_eq!(code(&output), 0, "{}", stderr(&output));
    assert!(
        stdout(&output).contains("is allowed"),
        "{}",
        stdout(&output)
    );
}

#[test]
fn inspect_lists_documents_and_shows_one() {
    let listing = protocol(&["inspect"]);
    assert_eq!(code(&listing), 0, "{}", stderr(&listing));
    let text = stdout(&listing);
    assert!(text.contains("principle  test-driven"), "{text}");
    assert!(text.contains("workflow   adp/default"), "{text}");

    let single = protocol(&["inspect", "test-driven"]);
    assert_eq!(code(&single), 0, "{}", stderr(&single));
    let document = stdout(&single);
    assert!(document.contains("id: test-driven"), "{document}");
    assert!(document.contains("obligations"), "{document}");
}

#[test]
fn schema_lists_and_prints_generated_schemas() {
    let listing = protocol(&["schema"]);
    assert_eq!(code(&listing), 0);
    assert_eq!(
        stdout(&listing).lines().count(),
        10,
        "one line per published schema"
    );

    let single = protocol(&["schema", "workflow"]);
    assert_eq!(code(&single), 0);
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout(&single)).expect("the schema is valid JSON");
    assert_eq!(parsed["title"], "RawWorkflow");
}

#[test]
fn json_output_is_machine_readable() {
    let output = protocol(&["evaluate", "--task", TASK, "--format", "json"]);
    assert_eq!(code(&output), 0, "{}", stderr(&output));
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout(&output)).expect("the evaluation is valid JSON");
    assert_eq!(parsed["state"], "receive");
    assert!(parsed["transitions"].is_array());
    assert_eq!(parsed["is_complete"], false);
}

#[test]
fn conformance_runs_the_suites_against_the_reference_backend() {
    let output = protocol(&["conformance", "--level", "full"]);
    assert_eq!(code(&output), 0, "{}", stderr(&output));
    let text = stdout(&output);
    assert!(text.contains("conformance full"), "{text}");
    assert!(text.contains("properties hold"), "{text}");
}

#[test]
fn conformance_fails_when_a_property_is_deliberately_broken() {
    // The point of shipping a faulty backend: a suite that passes everything tells you nothing, and
    // this is how a reader checks that for themselves in one command.
    let output = protocol(&[
        "conformance",
        "--suite",
        "idempotency",
        "--inject",
        "replay-applies",
    ]);
    assert_eq!(code(&output), 1, "a broken property is a non-zero exit");
    let text = stdout(&output);
    assert!(text.contains("do not hold"), "{text}");
    assert!(
        text.contains("expected to be caught by the `idempotency` suite"),
        "{text}"
    );
}

#[test]
fn output_survives_a_reader_that_stops_reading() {
    // `protocol inspect | head -3` must produce three lines, not a stack trace. Rust's `println!`
    // panics on a closed pipe, which turns an ordinary shell idiom into a crash report.
    use std::process::Stdio;

    let mut child = Command::new(env!("CARGO_BIN_EXE_protocol"))
        .args(["conformance", "--level", "full"])
        .current_dir(root())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the protocol binary runs");

    // Read one line, then drop the pipe while the child is still writing.
    {
        use std::io::{BufRead, BufReader};
        let stdout = child.stdout.take().expect("stdout is piped");
        let mut reader = BufReader::new(stdout);
        let mut first = String::new();
        reader
            .read_line(&mut first)
            .expect("the first line arrives");
        assert!(!first.is_empty());
    }

    let output = child.wait_with_output().expect("the child finishes");
    let errors = String::from_utf8_lossy(&output.stderr);
    assert!(
        !errors.contains("panicked"),
        "a reader that stopped reading is not a crash: {errors}"
    );
}

#[test]
fn conformance_rejects_an_unknown_level_or_fault() {
    let level = protocol(&["conformance", "--level", "thorough"]);
    assert_eq!(code(&level), 1);
    assert!(
        stderr(&level).contains("is not a conformance level"),
        "{}",
        stderr(&level)
    );

    let fault = protocol(&["conformance", "--inject", "nonsense"]);
    assert_eq!(code(&fault), 1);
    assert!(
        stderr(&fault).contains("is not a fault"),
        "{}",
        stderr(&fault)
    );
}
