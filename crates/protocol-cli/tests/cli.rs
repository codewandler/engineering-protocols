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

/// A fixture path as an argument.
fn printable(path: &Path) -> &str {
    path.to_str().expect("a printable path")
}

/// An empty scratch directory to build a fixture in.
fn scratch(name: &str) -> PathBuf {
    let directory = std::env::temp_dir().join(name);
    std::fs::remove_dir_all(&directory).ok();
    std::fs::create_dir_all(&directory).expect("the temporary tree is writable");
    directory
}

/// Writes a fixture file, creating the directories above it.
fn write(path: &Path, contents: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("the temporary tree is writable");
    }
    std::fs::write(path, contents).expect("the fixture is writable");
}

const TASK: &str = "examples/development-passkeys/task.yaml";
const ARTIFACTS: &str = "examples/development-passkeys/artifacts.yaml";
const SPECIFICATION: &str = "examples/billing";

/// The header of a one-domain specification, as `system.yaml` carries it.
const SYSTEM: &str = "format: ess/1\nsystem: shop\nversion: v1\ndomains:\n  - shop.order\n";

/// A domain file, written out because YAML this deeply indented does not survive `\n` escapes.
const DOMAIN: &str = r"domain: shop.order
types:
  - name: shop.order.OrderId
    kind: newtype
    of: Uuid
entities:
  - name: shop.order.Order
    identity:
      name: order_id
      type: shop.order.OrderId
    lifecycle:
      initial: Draft
      states: [Draft, Placed]
      terminal: [Placed]
      transitions:
        - name: Place
          from: [Draft]
          to: Placed
";

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
    let text = stdout(&listing);
    // One line per published schema, checked against what the library publishes rather than
    // against a number: a count only ever fails with "the number changed".
    assert_eq!(
        text.lines().count(),
        aep_schema::generated_schemas().len(),
        "{text}"
    );
    for entry in aep_schema::generated_schemas() {
        assert!(
            text.contains(&entry.filename),
            "{} is not listed: {text}",
            entry.filename
        );
    }

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
fn a_project_is_discovered_so_no_arguments_are_needed() {
    // The first command an adopting team types should not need four paths.
    let project = std::env::temp_dir().join("aep-cli-project");
    std::fs::remove_dir_all(&project).ok();
    std::fs::create_dir_all(project.join(".engineering")).expect("writable");
    std::fs::write(
        project.join(".engineering/project.yaml"),
        format!(
            "protocol: adp/1\nprofile: development.standard\nprotocols: {}\n",
            root().display()
        ),
    )
    .expect("writable");
    std::fs::write(
        project.join(".engineering/task.yaml"),
        "id: LOCAL-1\nkind: feature\nobjective: prove discovery works\nprotocol: adp/1\n\
         profile: development.standard\n",
    )
    .expect("writable");

    let output = Command::new(env!("CARGO_BIN_EXE_protocol"))
        .arg("resolve")
        .current_dir(&project)
        .output()
        .expect("the protocol binary runs");

    assert_eq!(code(&output), 0, "{}", stderr(&output));
    let text = stdout(&output);
    assert!(text.contains("LOCAL-1"), "{text}");
    assert!(text.contains("development.standard"), "{text}");

    // From a subdirectory too: discovery walks up.
    let nested = project.join("src/deep");
    std::fs::create_dir_all(&nested).expect("writable");
    let nested_output = Command::new(env!("CARGO_BIN_EXE_protocol"))
        .arg("resolve")
        .current_dir(&nested)
        .output()
        .expect("the protocol binary runs");
    assert_eq!(code(&nested_output), 0, "{}", stderr(&nested_output));

    std::fs::remove_dir_all(&project).ok();
}

#[test]
fn outside_a_project_the_missing_task_is_explained() {
    let elsewhere = std::env::temp_dir().join("aep-cli-not-a-project");
    std::fs::create_dir_all(&elsewhere).expect("writable");

    let output = Command::new(env!("CARGO_BIN_EXE_protocol"))
        .arg("resolve")
        .current_dir(&elsewhere)
        .output()
        .expect("the protocol binary runs");

    assert_eq!(code(&output), 1);
    let errors = stderr(&output);
    assert!(errors.contains(".engineering/project.yaml"), "{errors}");
    assert!(errors.contains("no --task was given"), "{errors}");

    std::fs::remove_dir_all(&elsewhere).ok();
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

#[test]
fn ess_validate_accepts_the_normative_example() {
    let output = protocol(&["ess", "validate", "--path", SPECIFICATION]);
    assert_eq!(code(&output), 0, "{}", stderr(&output));
    let text = stdout(&output);
    assert!(text.contains("billing v3"), "{text}");
    assert!(text.contains("valid"), "{text}");
}

#[test]
fn ess_validate_refuses_a_reference_to_something_nothing_declares() {
    let directory = scratch("aep-cli-broken-spec");
    write(&directory.join("system.yaml"), SYSTEM);
    write(
        &directory.join("domains/order.yaml"),
        &DOMAIN.replace("type: shop.order.OrderId", "type: shop.order.Missing"),
    );

    let output = protocol(&["ess", "validate", "--path", printable(&directory)]);
    assert_eq!(code(&output), 1, "{}", stderr(&output));
    assert!(
        stdout(&output).contains("undeclared_reference"),
        "{}",
        stdout(&output)
    );

    std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn ess_validate_names_the_file_a_problem_is_in() {
    // Named relative to the specification, so the same problem reads the same on two machines —
    // and so it is named at all, which an absolute path stripped of its own prefix is not.
    let directory = scratch("aep-cli-duplicating-spec");
    write(&directory.join("system.yaml"), SYSTEM);
    write(&directory.join("domains/one.yaml"), DOMAIN);
    write(&directory.join("domains/two.yaml"), DOMAIN);

    let output = protocol(&["ess", "validate", "--path", printable(&directory)]);
    assert_eq!(code(&output), 1, "{}", stderr(&output));
    let text = stdout(&output);
    assert!(text.contains("duplicate_declaration"), "{text}");
    assert!(
        text.contains("domains/two.yaml"),
        "a diagnostic has to say which file to open: {text}"
    );
    assert!(
        !text.contains(printable(&directory)),
        "and not where the specification happens to sit on this machine: {text}"
    );

    std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn ess_validate_names_the_one_file_a_specification_is_written_in() {
    // A one-file specification is first class (design §24), and it is the case where a path
    // relative to the specification is empty — so it is the case that loses its filename.
    let directory = scratch("aep-cli-one-file-spec");
    let orphaned = directory.join("shop.yaml");
    write(&orphaned, &DOMAIN.replace("domain: shop.order\n", ""));

    let output = protocol(&["ess", "validate", "--path", printable(&orphaned)]);
    assert_eq!(code(&output), 1, "{}", stderr(&output));
    let text = stdout(&output);
    assert!(text.contains("shop.yaml"), "{text}");

    // And the same for a file that never parses, which is reported on a different path.
    let mangled = directory.join("mangled.yaml");
    write(&mangled, "format: ess/1\nsystem: [shop\n");

    let refusal = protocol(&["ess", "validate", "--path", printable(&mangled)]);
    assert_eq!(code(&refusal), 1, "{}", stderr(&refusal));
    let reason = stdout(&refusal);
    assert!(reason.contains("mangled.yaml"), "{reason}");

    std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn ess_validate_renders_json_for_another_tool() {
    let output = protocol(&[
        "ess",
        "validate",
        "--path",
        SPECIFICATION,
        "--format",
        "json",
    ]);
    assert_eq!(code(&output), 0, "{}", stderr(&output));
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout(&output)).expect("the summary is valid JSON");
    assert_eq!(parsed["system"], "billing");
    assert_eq!(parsed["version"], "v3");
    assert_eq!(parsed["domains"], 2);
    assert_eq!(
        parsed["problems"]
            .as_array()
            .expect("problems is a list")
            .len(),
        0,
        "{parsed}"
    );
}

#[test]
fn ess_validate_refuses_a_directory_that_is_not_a_specification() {
    // `--path` defaults to `.`, so this is what someone typing the command in the wrong directory
    // gets: the repository root holds 50-odd unrelated YAML files, and reporting each of them as a
    // broken specification says nothing about what actually went wrong.
    let output = protocol(&["ess", "validate"]);
    assert_eq!(code(&output), 1);
    let errors = stderr(&output);
    assert!(errors.contains("is not a specification"), "{errors}");
    assert!(errors.contains("system.yaml"), "{errors}");
    assert!(
        stdout(&output).is_empty(),
        "nothing was validated: {}",
        stdout(&output)
    );
}

#[cfg(unix)]
#[test]
fn ess_validate_reads_each_file_once_when_a_symlink_points_back_up_the_tree() {
    let directory = scratch("aep-cli-looping-spec");
    write(&directory.join("system.yaml"), SYSTEM);
    write(&directory.join("domains/order.yaml"), DOMAIN);
    std::os::unix::fs::symlink("..", directory.join("domains/back")).expect("symlinks are allowed");

    let output = protocol(&["ess", "validate", "--path", printable(&directory)]);
    assert_eq!(code(&output), 0, "{}", stderr(&output));
    let text = stdout(&output);
    assert!(
        text.contains("2 file(s)"),
        "a file reachable by two paths is still one file: {text}"
    );
    assert!(!text.contains("duplicate_declaration"), "{text}");

    std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn ess_validate_output_survives_a_reader_that_stops_reading() {
    // The pipe is closed before the first line is written, which is what `protocol ess validate |
    // head -0` does. `println!` would end that shell idiom in a stack trace.
    use std::process::Stdio;

    let mut child = Command::new(env!("CARGO_BIN_EXE_protocol"))
        .args(["ess", "validate", "--path", SPECIFICATION])
        .current_dir(root())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the protocol binary runs");
    drop(child.stdout.take().expect("stdout is piped"));

    let output = child.wait_with_output().expect("the child finishes");
    let errors = String::from_utf8_lossy(&output.stderr);
    assert!(
        !errors.contains("panicked"),
        "a reader that stopped reading is not a crash: {errors}"
    );
}
