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

/// The second specification, the one built for the checks billing cannot make fail.
const ORACLE: &str = "examples/oracle-fixture";

/// The committed suite the drift check keeps in step with `examples/billing/`.
const COMMITTED_SUITE: &str = "suites/generated/billing/suite.json";

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
commands:
  - name: shop.order.PlaceOrder
    input:
      - name: order_id
        type: shop.order.OrderId
    outcomes:
      - name: placed
        moves: shop.order.Order.Place
        instance: order_id
        emits: [shop.order.OrderPlaced]
events:
  - name: shop.order.OrderPlaced
    fields: []
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

/// Copies a specification tree, so a test can break one line of a working one.
///
/// The normative example is the only specification that exercises components and bindings, and a
/// fixture written from scratch to test one refusal drifts from it the first time the model moves.
fn copy_tree(from: &Path, into: &Path) {
    for entry in std::fs::read_dir(from).expect("the specification is readable") {
        let entry = entry.expect("the specification is readable");
        let target = into.join(entry.file_name());
        if entry.path().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            let text = std::fs::read_to_string(entry.path()).expect("a readable file");
            write(&target, &text);
        }
    }
}

/// The normative example, copied into a scratch directory to be broken.
fn copied_specification(name: &str) -> PathBuf {
    let directory = scratch(name);
    copy_tree(&root().join(SPECIFICATION), &directory);
    directory
}

#[test]
fn ess_compile_refuses_a_specification_that_does_not_assemble() {
    // A specification that fails wave 1 never reaches the compiler, so it has no `Diagnostic` to
    // report — and the refusal still has to carry the code and the file, because that is what the
    // reader acts on.
    let directory = scratch("aep-cli-uncompilable-spec");
    write(&directory.join("system.yaml"), SYSTEM);
    write(&directory.join("domains/one.yaml"), DOMAIN);
    write(&directory.join("domains/two.yaml"), DOMAIN);

    let output = protocol(&["ess", "compile", "--path", printable(&directory)]);
    assert_eq!(code(&output), 1, "{}", stderr(&output));
    let text = stdout(&output);
    assert!(text.contains("duplicate_declaration"), "{text}");
    assert!(
        text.contains("domains/two.yaml"),
        "a refusal has to say which file to open: {text}"
    );

    std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn ess_compile_renders_a_refusal_as_json() {
    let directory = scratch("aep-cli-uncompilable-json-spec");
    write(&directory.join("system.yaml"), SYSTEM);
    write(
        &directory.join("domains/order.yaml"),
        &DOMAIN.replace("type: shop.order.OrderId", "type: shop.order.Missing"),
    );

    let output = protocol(&[
        "ess",
        "compile",
        "--path",
        printable(&directory),
        "--format",
        "json",
    ]);
    assert_eq!(code(&output), 1, "{}", stderr(&output));
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout(&output)).expect("the compilation report is valid JSON");
    assert_eq!(parsed["compiled"], false);
    assert!(
        parsed["diagnostics"].is_array(),
        "a consumer branches on `compiled` and reads the same two lists either way: {parsed}"
    );
    let problems = parsed["problems"]
        .as_array()
        .expect("problems is a list")
        .iter()
        .map(|problem| problem.as_str().unwrap_or_default().to_owned())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(problems.contains("undeclared_reference"), "{parsed}");

    std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn every_ess_verb_survives_a_reader_that_stops_reading() {
    // `protocol ess graph | head -5` must produce five lines, not a stack trace: DOT is piped into
    // `dot` by definition, and `println!` panics the moment that reader exits first. `generate` is
    // in the list because it is the verb with the most to say — its listing is what someone pipes
    // into `head` to find out where one projection landed.
    use std::process::Stdio;

    for arguments in [
        vec!["ess", "compile", "--path", SPECIFICATION],
        vec!["ess", "generate", "--path", SPECIFICATION],
        vec![
            "ess",
            "generate",
            "--path",
            SPECIFICATION,
            "--format",
            "json",
        ],
        vec![
            "ess",
            "inspect",
            "--path",
            SPECIFICATION,
            "billing.invoice.CreateInvoice",
        ],
        vec!["ess", "graph", "--path", SPECIFICATION],
        vec!["ess", "conform", "synthesize", "--path", SPECIFICATION],
        vec![
            "ess",
            "conform",
            "run",
            "--path",
            SPECIFICATION,
            "--target",
            "billing",
        ],
    ] {
        let mut child = Command::new(env!("CARGO_BIN_EXE_protocol"))
            .args(&arguments)
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
            "a reader that stopped reading is not a crash for {arguments:?}: {errors}"
        );
    }
}

#[test]
fn ess_compile_resolves_the_normative_example() {
    let output = protocol(&["ess", "compile", "--path", SPECIFICATION]);
    assert_eq!(code(&output), 0, "{}", stderr(&output));
    let text = stdout(&output);
    assert!(text.contains("billing v3"), "{text}");
    assert!(text.contains("compiled"), "{text}");
    assert!(
        text.contains("binding(s)") && text.contains("component(s)"),
        "the summary counts what wave 2 added: {text}"
    );
}

#[test]
fn ess_compile_renders_the_ir_as_json() {
    let output = protocol(&[
        "ess",
        "compile",
        "--path",
        SPECIFICATION,
        "--format",
        "json",
    ]);
    assert_eq!(code(&output), 0, "{}", stderr(&output));
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout(&output)).expect("the compilation report is valid JSON");
    assert_eq!(parsed["compiled"], true);
    assert_eq!(parsed["ir"]["system"], "billing");
    assert_eq!(
        parsed["ir"]["bindings"]["notify-on-invoice-created"]["delivery"], "at_least_once",
        "the IR carries the resolved binding, not a summary of it: {parsed}"
    );
    assert!(
        parsed["ir"]["commands"]["billing.invoice.CreateInvoice"]["input"].is_array(),
        "{parsed}"
    );
}

#[test]
fn ess_compile_reports_a_mapping_the_types_refuse() {
    // The example's binding maps `Email` onto `EmailAddress`, which is legal only because
    // `components.yaml` declares the conversion. Cutting the conversion out is design §20's
    // "mapping between incompatible types".
    //
    // It is refused by `ess-domain`, not by the compiler: `Specification::assemble` runs the mapping
    // check, so the document never reaches `compile` and `ESS-BINDING-002` is not what comes back.
    // The rendering of a `Diagnostic` is still what `compile` does with one — `ess-compiler`'s own
    // tests hold that — and this asserts what a user actually sees.
    let directory = copied_specification("aep-cli-unconvertible-spec");
    let components = directory.join("components.yaml");
    let text = std::fs::read_to_string(&components).expect("the example declares components");
    let declaration = text
        .find("components:")
        .expect("the example declares components");
    write(&components, &text[declaration..]);

    let output = protocol(&["ess", "compile", "--path", printable(&directory)]);
    assert_eq!(code(&output), 1, "{}", stderr(&output));
    let report = stdout(&output);
    assert!(report.contains("type_mismatch"), "{report}");
    assert!(
        report.contains("notify-on-invoice-created"),
        "the binding that cannot be built has to be named: {report}"
    );
    assert!(
        report.contains("billing.invoice.Email") && report.contains("billing.email.EmailAddress"),
        "and both types, so the repair does not need a second look: {report}"
    );

    std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn ess_inspect_shows_one_declaration_resolved() {
    let command = protocol(&[
        "ess",
        "inspect",
        "--path",
        SPECIFICATION,
        "billing.invoice.CreateInvoice",
    ]);
    assert_eq!(code(&command), 0, "{}", stderr(&command));
    let text = stdout(&command);
    assert!(
        text.starts_with("command    billing.invoice.CreateInvoice"),
        "{text}"
    );
    assert!(
        text.contains("billing.invoice.InvoiceCreated"),
        "what it emits is part of what it is: {text}"
    );

    let binding = protocol(&[
        "ess",
        "inspect",
        "--path",
        SPECIFICATION,
        "notify-on-invoice-created",
    ]);
    assert_eq!(code(&binding), 0, "{}", stderr(&binding));
    let reaction = stdout(&binding);
    assert!(reaction.contains("at_least_once"), "{reaction}");
    assert!(
        reaction.contains("escalate"),
        "what happens when it fails is not a footnote: {reaction}"
    );
}

#[test]
fn ess_inspect_renders_one_declaration_as_json() {
    let output = protocol(&[
        "ess",
        "inspect",
        "--path",
        SPECIFICATION,
        "billing.invoice.InvoiceCreated",
        "--format",
        "json",
    ]);
    assert_eq!(code(&output), 0, "{}", stderr(&output));
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout(&output)).expect("the declaration is valid JSON");
    assert_eq!(parsed["kind"], "event");
    assert_eq!(parsed["name"], "billing.invoice.InvoiceCreated");
    assert!(parsed["fields"].is_array(), "{parsed}");
}

#[test]
fn ess_inspect_lists_what_is_declared_when_the_name_is_not() {
    let output = protocol(&[
        "ess",
        "inspect",
        "--path",
        SPECIFICATION,
        "billing.invoice.Emial",
    ]);
    assert_eq!(code(&output), 1, "{}", stdout(&output));
    let errors = stderr(&output);
    assert!(errors.contains("is not declared"), "{errors}");
    assert!(
        errors.contains("billing.invoice.Email"),
        "a reader who mistyped needs the list more than the refusal: {errors}"
    );
}

#[test]
fn ess_inspect_refuses_a_name_two_namespaces_declare() {
    // A binding identifier and a component identifier are spelled the same way, so one name can
    // legally mean two things. Showing either one would be a guess.
    let directory = copied_specification("aep-cli-colliding-spec");
    let components = directory.join("components.yaml");
    let text = std::fs::read_to_string(&components).expect("the example declares components");
    write(
        &components,
        &text.replace("id: notify-on-invoice-created", "id: email-service"),
    );

    let ambiguous = protocol(&[
        "ess",
        "inspect",
        "--path",
        printable(&directory),
        "email-service",
    ]);
    assert_eq!(code(&ambiguous), 1, "{}", stdout(&ambiguous));
    let errors = stderr(&ambiguous);
    assert!(errors.contains("binding"), "{errors}");
    assert!(errors.contains("component"), "{errors}");
    assert!(
        errors.contains("--kind"),
        "the caller is one flag away from saying which: {errors}"
    );

    let chosen = protocol(&[
        "ess",
        "inspect",
        "--path",
        printable(&directory),
        "email-service",
        "--kind",
        "binding",
    ]);
    assert_eq!(code(&chosen), 0, "{}", stderr(&chosen));
    assert!(
        stdout(&chosen).starts_with("binding"),
        "{}",
        stdout(&chosen)
    );

    std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn ess_graph_is_dot_by_default_and_two_runs_are_byte_identical() {
    let first = protocol(&["ess", "graph", "--path", SPECIFICATION]);
    assert_eq!(code(&first), 0, "{}", stderr(&first));
    let text = stdout(&first);
    assert!(text.contains("digraph \"billing\" {"), "{text}");
    assert!(
        text.contains("subgraph \"cluster_email-service\""),
        "a component declares a surface, and the graph boxes it: {text}"
    );
    assert!(
        text.contains(
            "\"billing.invoice.Customer\" -> \"billing.invoice.CreateInvoice\" [label=\"may invoke\"]"
        ),
        "a grant is an edge here too, not only on the documentation page: {text}"
    );
    assert!(
        text.contains("\"billing.invoice.CreateInvoice\" -> \"billing.invoice.InvoiceCreated\""),
        "a command and the fact it produces: {text}"
    );
    assert!(
        text.contains("\"billing.invoice.InvoiceCreated\" -> \"billing.email.SendEmail\""),
        "the binding is the edge the interaction layer is about: {text}"
    );

    // Review F8: determinism asserted is determinism untested.
    let second = protocol(&["ess", "graph", "--path", SPECIFICATION]);
    assert_eq!(
        first.stdout, second.stdout,
        "two runs over one specification must produce identical bytes"
    );

    // `text` was this verb's name for DOT before there was a second diagram to tell it apart from.
    // Renaming it to `dot` is not permission to break the scripts that already type the old word.
    let renamed = protocol(&["ess", "graph", "--path", SPECIFICATION, "--format", "text"]);
    assert_eq!(
        first.stdout,
        renamed.stdout,
        "`--format text` still means DOT: {}",
        stderr(&renamed)
    );
}

#[test]
fn ess_graph_renders_nodes_and_edges_as_json() {
    // DOT is for `dot`; a tool that wants the graph should not have to parse it back out.
    let output = protocol(&["ess", "graph", "--path", SPECIFICATION, "--format", "json"]);
    assert_eq!(code(&output), 0, "{}", stderr(&output));
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout(&output)).expect("the graph is valid JSON");
    assert_eq!(parsed["system"], "billing");

    let nodes = parsed["nodes"].as_array().expect("nodes is a list");
    let command = nodes
        .iter()
        .find(|node| node["name"] == "billing.invoice.CreateInvoice")
        .expect("the example declares CreateInvoice");
    assert_eq!(command["kind"], "command");
    assert_eq!(
        command["domain"], "billing.invoice",
        "the context that declares it is part of the graph: {parsed}"
    );

    // Which component holds it is a group rather than a field on the node, because §6 lets two
    // components publish one event: a scalar here would have to pick one of them and call it the
    // answer.
    let groups = parsed["groups"].as_array().expect("groups is a list");
    let unit = groups
        .iter()
        .find(|group| group["label"] == "invoice-service")
        .expect("the example declares invoice-service");
    assert_eq!(unit["kind"], "component");
    assert!(
        unit["members"]
            .as_array()
            .expect("members is a list")
            .contains(&serde_json::json!("billing.invoice.CreateInvoice")),
        "the component that accepts it is the box it is drawn in: {parsed}"
    );

    let edges = parsed["edges"].as_array().expect("edges is a list");
    let reaction = edges
        .iter()
        .find(|edge| edge["kind"] == "binding")
        .expect("the example declares a binding");
    assert_eq!(reaction["label"], "notify-on-invoice-created");
    assert_eq!(reaction["delivery"], "at_least_once");
    assert_eq!(reaction["on_failure"], "escalate");

    let grant = edges
        .iter()
        .find(|edge| edge["kind"] == "grant")
        .expect("the example declares an actor with a grant");
    assert_eq!(grant["from"], "billing.invoice.Customer");
    assert_eq!(grant["to"], "billing.invoice.CreateInvoice");
}

/// A workload block, which is where one duplicated key silently lost a declaration.
const WORKLOAD: &str = r"topology:
  workloads:
    order-service:
      replicas:
        min: 1
      stateless: true
      requires:
        - publish: order-events
";

#[test]
fn ess_validate_refuses_a_document_that_declares_one_key_twice() {
    // `serde_yaml` keeps the last of two identical mapping keys, so this document used to declare
    // one workload, silently, and the author's other floor was gone.
    let directory = scratch("aep-cli-duplicate-key-spec");
    write(&directory.join("system.yaml"), SYSTEM);
    write(&directory.join("domains/order.yaml"), DOMAIN);
    write(
        &directory.join("topology.yaml"),
        &format!(
            "{WORKLOAD}    order-service:\n      replicas:\n        min: 2\n      stateless: \
             true\n      requires:\n        - publish: order-events\n"
        ),
    );

    let output = protocol(&["ess", "validate", "--path", printable(&directory)]);
    assert_eq!(code(&output), 1, "{}", stderr(&output));
    let text = stdout(&output);
    assert!(
        text.contains("order-service"),
        "a refusal has to name the key that was written twice: {text}"
    );
    assert!(
        text.contains("topology.yaml"),
        "and the file it is in: {text}"
    );

    std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn ess_refuses_a_topology_that_runs_a_component_nobody_declares() {
    // The reason topology is modelled in a wave that deploys nothing: this rejection becomes
    // checkable. `compile` has to refuse it too, because it never reaches the compiler.
    let directory = scratch("aep-cli-phantom-workload-spec");
    write(&directory.join("system.yaml"), SYSTEM);
    write(&directory.join("domains/order.yaml"), DOMAIN);
    write(
        &directory.join("topology.yaml"),
        &WORKLOAD.replace("order-service", "ghost-service"),
    );

    for verb in ["validate", "compile"] {
        let output = protocol(&["ess", verb, "--path", printable(&directory)]);
        assert_eq!(code(&output), 1, "{verb}: {}", stderr(&output));
        let text = stdout(&output);
        assert!(text.contains("undeclared_reference"), "{verb}: {text}");
        assert!(
            text.contains("ghost-service"),
            "{verb}: the workload that names nothing has to be named: {text}"
        );
    }

    std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn ess_compile_gives_a_refusal_a_code_and_the_line_it_is_written_on() {
    // The rule that refuses this lives in `ess-domain` and is tested there. What `compile` adds is
    // design §29's shape: a stable code a harness can match, and a `file:line` a person can open.
    // Without the bridge this prints a sentence and nothing else, which is what it did before.
    let directory = scratch("ess-compile-bridged");
    let example = root().join(SPECIFICATION);
    let source =
        std::fs::read_to_string(example.join("components.yaml")).expect("the example is readable");
    for name in ["system.yaml", "topology.yaml"] {
        let text = std::fs::read_to_string(example.join(name)).expect("the example is readable");
        write(&directory.join(name), &text);
    }
    for name in ["invoice.yaml", "email.yaml"] {
        let text = std::fs::read_to_string(example.join("domains").join(name))
            .expect("the example is readable");
        write(&directory.join("domains").join(name), &text);
    }
    // The declared crossing no longer covers the mapping the binding makes.
    write(
        &directory.join("components.yaml"),
        &source.replace(
            "  - from: billing.invoice.Email",
            "  - from: billing.invoice.InvoiceId",
        ),
    );

    let output = protocol(&[
        "ess",
        "compile",
        "--path",
        directory.to_str().expect("a path"),
    ]);
    assert_eq!(code(&output), 1, "{}", stderr(&output));
    let text = stdout(&output);

    assert!(text.contains("ESS-BINDING-002"), "{text}");
    assert!(text.contains("billing.email.EmailAddress"), "{text}");
    assert!(
        text.contains("components.yaml:"),
        "a diagnostic without a line is one someone has to search from: {text}"
    );
    assert!(
        text.contains("type_mismatch"),
        "the `ess-domain` code has to survive the bridge, or a harness matching on it loses it: \
         {text}"
    );
}

#[test]
fn ess_compile_reports_a_refusal_structurally_in_json() {
    let directory = scratch("ess-compile-bridged-json");
    for (name, text) in [
        ("system.yaml", SYSTEM.to_owned()),
        // The identity names a type nothing declares.
        (
            "orders.yaml",
            DOMAIN.replace("type: shop.order.OrderId", "type: shop.order.Nonexistent"),
        ),
    ] {
        write(&directory.join(name), &text);
    }

    let output = protocol(&[
        "ess",
        "compile",
        "--path",
        directory.to_str().expect("a path"),
        "--format",
        "json",
    ]);
    assert_eq!(code(&output), 1, "{}", stderr(&output));
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout(&output)).expect("the refusal is valid JSON");

    let diagnostics = parsed["diagnostics"]
        .as_array()
        .expect("diagnostics is an array");
    assert!(!diagnostics.is_empty(), "{parsed}");
    // Structured, not a sentence: an agent consuming this as a repair instruction reads the fields.
    let first = &diagnostics[0];
    assert!(first["code"].is_string(), "{first}");
    assert_eq!(first["severity"], "error", "{first}");
    assert!(first["message"].is_string(), "{first}");
}

#[test]
fn ess_generate_projects_the_normative_example() {
    let output = protocol(&["ess", "generate", "--path", SPECIFICATION]);
    assert_eq!(code(&output), 0, "{}", stderr(&output));
    let text = stdout(&output);
    // The provenance line, because an artifact set nobody can attribute is one nobody can audit —
    // and over a pipe the header comments inside the files are not what the reader is looking at.
    assert!(text.contains("billing v3"), "{text}");
    assert!(text.contains("projection(s)"), "{text}");
}

#[test]
fn ess_generate_carries_provenance_and_every_artifacts_contents_in_json() {
    let output = protocol(&[
        "ess",
        "generate",
        "--path",
        SPECIFICATION,
        "--format",
        "json",
    ]);
    assert_eq!(code(&output), 0, "{}", stderr(&output));
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout(&output)).expect("the generation report is valid JSON");

    assert_eq!(parsed["provenance"]["system"], "billing");
    assert_eq!(parsed["provenance"]["specification_version"], "v3");
    assert!(
        parsed["provenance"]["source_digest"]
            .as_str()
            .is_some_and(|digest| !digest.is_empty()),
        "design §10's four facts, or a consumer cannot say which model this came from: {parsed}"
    );

    for artifact in parsed["artifacts"].as_array().expect("artifacts is a list") {
        assert!(artifact["path"].is_string(), "{artifact}");
        // The contents travel with the path, which is what lets a drift check compare a committed
        // tree against this without anything having to write the files first.
        assert!(artifact["contents"].is_string(), "{artifact}");
    }
}

#[test]
fn every_projection_ess_gen_publishes_can_be_asked_for_by_name() {
    // `--kind` is a second list of the projections, because clap needs the values at compile time.
    // A projection this build publishes that nothing can ask for is a projection nobody runs, and
    // the only thing that keeps the two lists in step is this test.
    let help = stdout(&protocol(&["ess", "generate", "--help"]));
    for generator in ess_gen::generators() {
        assert!(
            help.contains(&format!("- {}:", generator.name())),
            "`--kind {}` is missing from the help: {help}",
            generator.name()
        );
    }
}

#[test]
fn ess_generate_reports_only_the_projection_it_was_asked_for() {
    let output = protocol(&[
        "ess",
        "generate",
        "--path",
        SPECIFICATION,
        "--kind",
        "docs",
        "--format",
        "json",
    ]);
    assert_eq!(code(&output), 0, "{}", stderr(&output));
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout(&output)).expect("the generation report is valid JSON");
    let projections = parsed["projections"]
        .as_array()
        .expect("projections is a list");
    assert_eq!(projections.len(), 1, "{parsed}");
    assert_eq!(projections[0]["name"], "docs");
}

#[test]
fn ess_generate_keeps_to_the_projection_it_was_asked_for() {
    let filtered = protocol(&[
        "ess",
        "generate",
        "--path",
        SPECIFICATION,
        "--kind",
        "docs",
        "--format",
        "json",
    ]);
    assert_eq!(code(&filtered), 0, "{}", stderr(&filtered));
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout(&filtered)).expect("the generation report is valid JSON");
    let paths: Vec<String> = parsed["artifacts"]
        .as_array()
        .expect("artifacts is a list")
        .iter()
        .map(|artifact| artifact["path"].as_str().unwrap_or_default().to_owned())
        .collect();

    assert!(!paths.is_empty(), "the docs projection produced nothing");
    for path in &paths {
        assert!(
            path.starts_with("docs/"),
            "`--kind docs` produced {path}, which is another projection's file"
        );
    }

    // And filtering subtracts rather than changing: the same artifact, byte for byte, as the run
    // that asked for everything. A `--kind` that produced different bytes would make the committed
    // tree depend on how it was generated.
    let everything = protocol(&[
        "ess",
        "generate",
        "--path",
        SPECIFICATION,
        "--format",
        "json",
    ]);
    let all: serde_json::Value =
        serde_json::from_str(&stdout(&everything)).expect("the generation report is valid JSON");
    let docs: Vec<&serde_json::Value> = all["artifacts"]
        .as_array()
        .expect("artifacts is a list")
        .iter()
        .filter(|artifact| {
            artifact["path"]
                .as_str()
                .is_some_and(|path| path.starts_with("docs/"))
        })
        .collect();
    let filtered_artifacts: Vec<&serde_json::Value> = parsed["artifacts"]
        .as_array()
        .expect("artifacts is a list")
        .iter()
        .collect();
    assert_eq!(docs, filtered_artifacts, "`--kind` may only subtract");
}

#[test]
fn ess_generate_produces_an_artifact_for_every_projection() {
    // The bug this catches is a projection that silently produces nothing: the run still exits 0,
    // the tree still looks complete, and one of the three contracts the wave promised is absent.
    let output = protocol(&[
        "ess",
        "generate",
        "--path",
        SPECIFICATION,
        "--format",
        "json",
    ]);
    assert_eq!(code(&output), 0, "{}", stderr(&output));
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout(&output)).expect("the generation report is valid JSON");

    let artifacts = parsed["artifacts"].as_array().expect("artifacts is a list");
    for projection in parsed["projections"]
        .as_array()
        .expect("projections is a list")
    {
        let directory = format!(
            "{}/",
            projection["directory"].as_str().expect("a directory")
        );
        assert!(
            artifacts.iter().any(|artifact| artifact["path"]
                .as_str()
                .is_some_and(|path| path.starts_with(&directory))),
            "the `{}` projection produced nothing",
            projection["name"]
        );
    }
}

#[test]
fn ess_generate_refuses_a_specification_that_does_not_compile() {
    // Nothing is generated from a model that does not resolve, and the refusal has to name the file
    // and the code — a generator's caller is usually an agent, and that is what it repairs from.
    let directory = scratch("aep-cli-ungeneratable-spec");
    write(&directory.join("system.yaml"), SYSTEM);
    write(&directory.join("domains/one.yaml"), DOMAIN);
    write(&directory.join("domains/two.yaml"), DOMAIN);

    let output = protocol(&["ess", "generate", "--path", printable(&directory)]);
    assert_eq!(code(&output), 1, "{}", stderr(&output));
    let text = stdout(&output);
    assert!(text.contains("duplicate_declaration"), "{text}");
    assert!(
        text.contains("domains/two.yaml"),
        "a refusal has to say which file to open: {text}"
    );

    std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn ess_generate_writes_nothing_until_it_is_asked_for_a_directory() {
    // Typed without `--out` this reads like a question, so it has to behave like one. Run from a
    // directory of its own, because the mistake worth catching is a verb that writes into whatever
    // working tree it was invoked from.
    let directory = scratch("aep-cli-generate-read-only");
    let specification = root().join(SPECIFICATION);
    let output = Command::new(env!("CARGO_BIN_EXE_protocol"))
        .args(["ess", "generate", "--path", printable(&specification)])
        .current_dir(&directory)
        .output()
        .expect("the protocol binary runs");

    assert_eq!(code(&output), 0, "{}", stderr(&output));
    assert!(
        stdout(&output).contains("nothing written"),
        "and it says so, rather than leaving the reader to notice: {}",
        stdout(&output)
    );
    let left_behind: Vec<PathBuf> = std::fs::read_dir(&directory)
        .expect("the scratch directory is readable")
        .map(|entry| entry.expect("a readable entry").path())
        .collect();
    assert!(
        left_behind.is_empty(),
        "a read-only-looking command wrote {left_behind:?}"
    );

    std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn ess_generate_says_where_it_wrote_when_it_was_given_somewhere() {
    let directory = scratch("aep-cli-generate-out");
    let output = protocol(&[
        "ess",
        "generate",
        "--path",
        SPECIFICATION,
        "--out",
        printable(&directory),
        "--format",
        "json",
    ]);
    assert_eq!(code(&output), 0, "{}", stderr(&output));
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout(&output)).expect("the generation report is valid JSON");
    assert_eq!(parsed["written_to"], printable(&directory));

    std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn ess_generate_writes_every_artifact_it_reports_and_prints_no_contents() {
    let directory = scratch("aep-cli-generate-written");
    let output = protocol(&[
        "ess",
        "generate",
        "--path",
        SPECIFICATION,
        "--out",
        printable(&directory),
        "--format",
        "json",
    ]);
    assert_eq!(code(&output), 0, "{}", stderr(&output));
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout(&output)).expect("the generation report is valid JSON");
    let artifacts = parsed["artifacts"].as_array().expect("artifacts is a list");
    assert!(!artifacts.is_empty(), "nothing was generated: {parsed}");

    for artifact in artifacts {
        let path = artifact["path"].as_str().expect("a path");
        let contents = artifact["contents"].as_str().expect("contents");
        let written = std::fs::read_to_string(directory.join(path))
            .unwrap_or_else(|error| panic!("{path} was reported but not written: {error}"));
        assert_eq!(
            written, contents,
            "{path} was written with different bytes from the ones reported"
        );
    }

    // And the text rendering stays a listing: `--out` is how contents leave this program, so a
    // human asking where things went is not handed four directories of Markdown on stdout.
    let listed = protocol(&[
        "ess",
        "generate",
        "--path",
        SPECIFICATION,
        "--out",
        printable(&directory),
    ]);
    let text = stdout(&listed);
    let first = artifacts[0]["contents"].as_str().expect("contents");
    assert!(
        !text.contains(first),
        "the listing carried an artifact's contents: {text}"
    );
    assert!(
        text.contains(artifacts[0]["path"].as_str().expect("a path")),
        "the listing has to name what it wrote: {text}"
    );

    std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn ess_generate_produces_the_same_bytes_twice() {
    // Review F8, one level up: determinism asserted is determinism untested, and a projection that
    // varies between two runs turns every regeneration into a diff nobody can review.
    let first = protocol(&[
        "ess",
        "generate",
        "--path",
        SPECIFICATION,
        "--format",
        "json",
    ]);
    assert_eq!(code(&first), 0, "{}", stderr(&first));
    let second = protocol(&[
        "ess",
        "generate",
        "--path",
        SPECIFICATION,
        "--format",
        "json",
    ]);
    assert_eq!(
        first.stdout, second.stdout,
        "two runs over one specification must produce identical bytes"
    );
}
// ---- ess conform ----------------------------------------------------------------------------------

#[test]
fn ess_conform_synthesize_derives_the_suite_the_normative_example_obliges() {
    let output = protocol(&["ess", "conform", "synthesize", "--path", SPECIFICATION]);
    assert_eq!(code(&output), 0, "{}", stderr(&output));
    let text = stdout(&output);
    assert!(text.contains("billing v3 — 27 scenario(s)"), "{text}");
    // A scenario id, because the ids are the contract: a fault matrix and a stored report both key
    // on them, and a run of this command is where a reader first sees one.
    assert!(
        text.contains("billing.invoice.CreateInvoice/outcome/rejected"),
        "{text}"
    );
    assert!(
        text.contains("nothing written"),
        "a verb that looks read-only has to say it wrote nothing: {text}"
    );
}

#[test]
fn ess_conform_synthesize_names_a_construct_it_cannot_test_rather_than_omitting_it() {
    // §36, and the only failure a passing run cannot show. A suite quietly holding fewer checks than
    // the specification requires looks exactly like a suite that holds them all.
    let output = protocol(&["ess", "conform", "synthesize", "--path", SPECIFICATION]);
    let text = stdout(&output);
    assert!(text.contains("1 refusal(s)"), "{text}");
    assert!(
        text.contains("billing.invoice.Money"),
        "a refusal names the construct it is about: {text}"
    );
    assert!(
        text.contains("help:"),
        "and what would have to change: {text}"
    );

    let parsed: serde_json::Value = serde_json::from_str(&stdout(&protocol(&[
        "ess",
        "conform",
        "synthesize",
        "--path",
        SPECIFICATION,
        "--format",
        "json",
    ])))
    .expect("the synthesis report is valid JSON");
    assert_eq!(parsed["complete"], false);
    let refusals = parsed["refusals"].as_array().expect("refusals is a list");
    assert_eq!(refusals.len(), 1, "{parsed}");
    for refusal in refusals {
        // Fields, not a sentence: this output is read by a coding agent as repair instructions.
        for field in ["code", "subject", "because", "help"] {
            assert!(refusal[field].is_string(), "{field} is missing: {refusal}");
        }
    }
}

#[test]
fn ess_conform_synthesize_writes_the_bytes_it_prints() {
    // The property the drift check rests on. `cargo xtask suite --check` compares a committed tree
    // against what `--format json` carries, and that comparison means nothing unless the file this
    // command writes holds the same bytes.
    let directory = scratch("ess-conform-synthesize-out");
    let written = protocol(&[
        "ess",
        "conform",
        "synthesize",
        "--path",
        SPECIFICATION,
        "--out",
        printable(&directory),
    ]);
    assert_eq!(code(&written), 0, "{}", stderr(&written));

    let parsed: serde_json::Value = serde_json::from_str(&stdout(&protocol(&[
        "ess",
        "conform",
        "synthesize",
        "--path",
        SPECIFICATION,
        "--format",
        "json",
    ])))
    .expect("the synthesis report is valid JSON");
    let artifacts = parsed["artifacts"].as_array().expect("artifacts is a list");
    assert_eq!(
        artifacts.len(),
        1,
        "one document per specification: {parsed}"
    );
    assert_eq!(artifacts[0]["path"], "suite.json");

    let on_disk =
        std::fs::read_to_string(directory.join("suite.json")).expect("the suite was written");
    assert_eq!(
        artifacts[0]["contents"].as_str(),
        Some(on_disk.as_str()),
        "what it prints and what it writes have to be one answer"
    );

    std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn ess_conform_run_closes_the_loop_against_both_reference_implementations() {
    // The whole point of shipping the references: a person can watch a specification become a suite
    // and a suite become a verdict, in one command, against something that is known to be right.
    for (specification, target, scenarios) in [
        (SPECIFICATION, "billing", "27 scenarios"),
        (ORACLE, "oracle-fixture", "31 scenarios"),
    ] {
        let output = protocol(&[
            "ess",
            "conform",
            "run",
            "--path",
            specification,
            "--target",
            target,
        ]);
        assert_eq!(code(&output), 0, "{target}: {}", stderr(&output));
        let text = stdout(&output);
        assert!(text.contains(scenarios), "{target}: {text}");
        assert!(text.contains("conformant:"), "{target}: {text}");
    }
}

#[test]
fn ess_conform_run_reads_the_committed_suite_rather_than_only_a_freshly_derived_one() {
    // §22's promise: a written suite is a document a runner reads, not a value one process happened
    // to hold. If this ever stops working, the committed artifact is decoration.
    let output = protocol(&[
        "ess",
        "conform",
        "run",
        "--suite",
        COMMITTED_SUITE,
        "--target",
        "billing",
    ]);
    assert_eq!(code(&output), 0, "{}", stderr(&output));
    let text = stdout(&output);
    assert!(text.contains("27 scenarios: 27 passed"), "{text}");
    assert!(
        !text.contains("got no scenario"),
        "a written suite carries scenarios and not the refusals recorded when it was made, so \
         reporting a count would be claiming something nobody checked: {text}"
    );
}

#[test]
fn ess_conform_run_fails_and_names_the_scenario_that_caught_a_deliberately_wrong_implementation() {
    let output = protocol(&[
        "ess",
        "conform",
        "run",
        "--path",
        SPECIFICATION,
        "--target",
        "billing",
        "--inject",
        "accept-invalid-amount",
    ]);
    assert_eq!(code(&output), 1, "a contradicted specification is exit 1");
    let text = stdout(&output);
    assert!(
        text.contains("failed billing.invoice.CreateInvoice/outcome/rejected"),
        "the named scenario has to be the one that fails, not merely something: {text}"
    );
    assert!(text.contains("not conformant:"), "{text}");
}

#[test]
fn a_run_that_could_not_be_carried_out_is_not_reported_as_a_contradiction() {
    // The distinction §28 exists for, and the one an exit code has to keep: `failed` says the
    // implementation is wrong, `error` says nobody found out. A harness that collapses them opens a
    // defect against a system it never managed to ask a question of.
    let output = protocol(&[
        "ess", "conform", "run", "--path", ORACLE, "--target", "billing",
    ]);
    assert_eq!(code(&output), 3, "an unanswered run is its own exit code");
    let text = stdout(&output);
    assert!(
        text.contains("31 scenarios: 0 passed, 0 failed, 31 error"),
        "{text}"
    );
    assert!(text.contains("undecided:"), "{text}");
}

#[test]
fn an_observation_the_target_cannot_expose_is_unsupported_rather_than_skipped() {
    // §16 lets an implementation decline to trace the commands its bindings invoke, and §28 refuses
    // to let that pass as a check. Both halves are asserted: the scenario is `unsupported` and not
    // `passed`, and the run still fails.
    let output = protocol(&[
        "ess",
        "conform",
        "run",
        "--path",
        SPECIFICATION,
        "--target",
        "billing",
        "--untraced",
    ]);
    assert_eq!(
        code(&output),
        1,
        "an unsupported required scenario fails the run"
    );
    let text = stdout(&output);
    assert!(
        text.contains("unsupported notify-on-invoice-created/binding/mapping"),
        "{text}"
    );
    assert!(
        text.contains("26 passed, 0 failed, 0 error, 1 unsupported"),
        "the four words stay four words; flattening them loses the finding: {text}"
    );
}

#[test]
fn ess_conform_refuses_a_fault_belonging_to_the_other_specification() {
    // `ess_conformance::faulty::billing` panics on this rather than returning, because injecting an
    // oracle fault into billing produces a green run that proves nothing. A backtrace is a worse way
    // to say so than a sentence.
    let output = protocol(&[
        "ess",
        "conform",
        "run",
        "--path",
        SPECIFICATION,
        "--target",
        "billing",
        "--inject",
        "drop-binding",
    ]);
    assert_eq!(code(&output), 1);
    let reason = stderr(&output);
    assert!(
        reason.contains("is a fault of `oracle-fixture`"),
        "{reason}"
    );

    let unknown = protocol(&[
        "ess",
        "conform",
        "run",
        "--path",
        SPECIFICATION,
        "--target",
        "billing",
        "--inject",
        "nonsense",
    ]);
    assert_eq!(code(&unknown), 1);
    assert!(
        stderr(&unknown).contains("is not a fault"),
        "{}",
        stderr(&unknown)
    );
}

#[test]
fn the_two_conformance_verbs_each_say_which_question_they_answer() {
    // `protocol conformance` and `protocol ess conform` are one word apart and answer different
    // questions. A reader must not have to guess which is which, so each help text names the other.
    let backend = stdout(&protocol(&["conformance", "--help"]));
    assert!(
        backend.contains("protocol ess conform"),
        "the backend verb has to point at the semantic one: {backend}"
    );
    assert!(
        backend.contains("backend"),
        "and say what it is about: {backend}"
    );

    let semantic = stdout(&protocol(&["ess", "conform", "--help"]));
    assert!(
        semantic.contains("protocol conformance"),
        "and the other way round: {semantic}"
    );

    // The top-level listing is where a reader meets both, and a one-line summary that does not name
    // what is being checked is the whole problem.
    let root = stdout(&protocol(&["--help"]));
    assert!(
        root.contains("Check a storage backend against the AEP contract suites"),
        "{root}"
    );
}

#[test]
fn ess_conform_run_says_plainly_that_it_cannot_run_somebody_elses_implementation() {
    // The honest half. A `ConformanceTarget` is a Rust trait; this binary reaches only what it was
    // compiled with. Implying otherwise costs a reader an afternoon looking for the flag.
    let help = stdout(&protocol(&["ess", "conform", "run", "--help"]));
    assert!(help.contains("It cannot run yours"), "{help}");
    assert!(
        help.contains("ConformanceTarget"),
        "and say what implementing one would take: {help}"
    );
    assert!(
        help.contains("suites/generated"),
        "and where the suite to run against it already is: {help}"
    );
}

#[test]
fn ess_conform_synthesize_is_deterministic() {
    // §37. The committed suite is drift-checked, so a second run producing different bytes would
    // make the check fail for no reason anybody could act on.
    let first = protocol(&[
        "ess",
        "conform",
        "synthesize",
        "--path",
        SPECIFICATION,
        "--format",
        "json",
    ]);
    assert_eq!(code(&first), 0, "{}", stderr(&first));
    let second = protocol(&[
        "ess",
        "conform",
        "synthesize",
        "--path",
        SPECIFICATION,
        "--format",
        "json",
    ]);
    assert_eq!(
        first.stdout, second.stdout,
        "two runs over one specification must produce identical bytes"
    );
}

// ---- the closed loop -----------------------------------------------------------------------
//
// Design §33 and §49 step 10: a real task that completes only because an independent conformance
// run says the implementation matches the specification, and refuses when it does not. Everything
// above this line is machinery; these are the tests that say the machinery changed what the
// protocol permits.

/// The worked example this section replays.
const CONFORMANCE_TASK: &str = "examples/billing-conformance/task.yaml";
const CONFORMANCE_ARTIFACTS: &str = "examples/billing-conformance/artifacts.yaml";

/// The evidence that gets the task to the point where only conformance is outstanding.
const PRIOR_EVIDENCE: [&str; 5] = [
    "examples/billing-conformance/evidence/01-red-test.yaml",
    "examples/billing-conformance/evidence/02-implementation.yaml",
    "examples/billing-conformance/evidence/03-verification.yaml",
    "examples/billing-conformance/evidence/04-review.yaml",
    "examples/billing-conformance/evidence/05-verifications.yaml",
];

const PASSING_RECORD: &str = "examples/billing-conformance/evidence/06-conformance.yaml";
const FAILING_RECORD: &str = "examples/billing-conformance/evidence/06-conformance-faulty.yaml";

/// `protocol evaluate --advance` over the worked example, with `records` submitted last.
fn evaluate_conformance_task(artifacts: &str, records: &[&str]) -> Output {
    let mut args = vec![
        "evaluate",
        "--task",
        CONFORMANCE_TASK,
        "--artifacts",
        artifacts,
    ];
    for path in PRIOR_EVIDENCE.iter().chain(records) {
        args.push("--evidence");
        args.push(path);
    }
    args.push("--advance");
    protocol(&args)
}

#[test]
fn the_committed_conformance_evidence_is_what_the_runner_produces_and_not_what_someone_typed() {
    // The one check that makes the rest of this section evidence rather than a fixture. Both
    // records in the worked example are regenerated here and compared byte for byte: if either
    // could be edited by hand without a test noticing, the example would prove nothing that a
    // hardcoded `true` would not.
    for (record, fault) in [
        (PASSING_RECORD, None),
        (FAILING_RECORD, Some("accept-invalid-amount")),
    ] {
        let mut args = vec![
            "ess",
            "conform",
            "evidence",
            "--path",
            SPECIFICATION,
            "--target",
            "billing",
        ];
        if let Some(fault) = fault {
            args.push("--inject");
            args.push(fault);
        }
        let output = protocol(&args);
        assert_eq!(code(&output), 0, "{record}: {}", stderr(&output));
        let committed = std::fs::read_to_string(root().join(record)).expect("the record is there");
        assert_eq!(
            stdout(&output),
            committed,
            "{record} is not what the runner produces; regenerate it with `{}`",
            args.join(" ")
        );
    }
}

#[test]
fn the_conformance_record_names_the_runner_as_its_producer_and_carries_the_spec_digest() {
    // The two fields the requirement is made of. `producer: verifier / conformance-runner` is what
    // `independent: true` and `verifier: conformance-runner` check against, and `spec_digest` is
    // what binds the run to one resolution of the model — gate G19's rule, which fails closed.
    let output = protocol(&[
        "ess",
        "conform",
        "evidence",
        "--path",
        SPECIFICATION,
        "--target",
        "billing",
    ]);
    assert_eq!(code(&output), 0, "{}", stderr(&output));
    let document = stdout(&output);
    for required in [
        "kind: ess_conformance",
        "spec_digest: e19d384dac86219a",
        "producer: verifier",
        "verifier: conformance-runner",
        "status: passed",
    ] {
        assert!(
            document.contains(required),
            "an evidence record carries {required:?}; it read:\n{document}"
        );
    }
    assert!(
        !document.contains("producer: agent"),
        "a record an agent produced does not satisfy `independent: true`: {document}"
    );
}

#[test]
fn the_evidence_verb_writes_a_failing_run_down_rather_than_exiting_on_it() {
    // Direction two needs the record to exist. A verb that refused to write evidence of a failure
    // would make "the task cannot complete" unprovable — the task would simply have no evidence,
    // which is a different reason and a weaker one.
    let output = protocol(&[
        "ess",
        "conform",
        "evidence",
        "--path",
        SPECIFICATION,
        "--target",
        "billing",
        "--inject",
        "accept-invalid-amount",
    ]);
    assert_eq!(
        code(&output),
        0,
        "the verdict belongs in the record, not in this exit code: {}",
        stderr(&output)
    );
    let document = stdout(&output);
    assert!(document.contains("status: failed"), "{document}");
    assert!(
        document.contains("failed billing.invoice.CreateInvoice/outcome/rejected"),
        "the record names the scenario, so the refusal downstream is actionable: {document}"
    );
    assert!(
        document.contains("verifier: conformance-runner"),
        "a failing run is produced by the same verifier as a passing one: {document}"
    );
}

#[test]
fn a_task_governed_by_a_specification_completes_only_once_the_conformance_run_exists() {
    // The fixture has to reach the state the rule is load-bearing in: without the record, every
    // other requirement of `development.critical` is already met, so what stops the task can only
    // be conformance. Then the record arrives and it completes.
    let without = evaluate_conformance_task(CONFORMANCE_ARTIFACTS, &[]);
    assert_eq!(code(&without), 0, "{}", stderr(&without));
    let owed = stdout(&without);
    assert!(
        !owed.contains("Task complete"),
        "a task with no conformance run must not complete: {owed}"
    );
    assert!(
        owed.contains("0 of 1 required record(s) submitted"),
        "and the reason must be the missing run: {owed}"
    );

    let with = evaluate_conformance_task(CONFORMANCE_ARTIFACTS, &[PASSING_RECORD]);
    assert_eq!(code(&with), 0, "{}", stderr(&with));
    let done = stdout(&with);
    assert!(
        done.contains("state       complete (Complete)")
            && done.contains("Task complete in `complete`"),
        "the passing run is the only thing that changed, so it is what completed the task: {done}"
    );
}

#[test]
fn a_faulty_implementation_cannot_complete_the_task_and_the_refusal_names_the_reason() {
    // The half that matters. Same task, same everything else, one implementation that accepts an
    // invoice the specification refuses — and the engine will not let it through.
    let output = evaluate_conformance_task(CONFORMANCE_ARTIFACTS, &[FAILING_RECORD]);
    assert_eq!(code(&output), 0, "{}", stderr(&output));
    let text = stdout(&output);

    assert!(
        !text.contains("Task complete"),
        "a contradicted specification must not close the task: {text}"
    );
    assert!(
        text.contains("state       review (Review)"),
        "and it stops one state short of complete rather than somewhere unrelated: {text}"
    );
    for reason in [
        "ess_conformance.passed = false",
        "ess_conformance.scenarios.failed = 1",
    ] {
        assert!(
            text.contains(reason),
            "the refusal has to name {reason:?} rather than merely refusing: {text}"
        );
    }
    assert!(
        text.contains("[principle ess-conformance]"),
        "and it names the rule a person can go and read: {text}"
    );
    assert!(
        text.contains("✓ evidence ess_conformance from conformance-runner (independent)"),
        "the evidence was submitted and accepted as independent — what fails is what it says, \
         which is a different finding from a missing record: {text}"
    );
}

#[test]
fn a_conformance_run_against_another_revision_of_the_model_does_not_close_the_task() {
    // Gate G19's rule at the level a person meets it. The run passed, the record is independent,
    // and it attests a resolution of the specification that is not the one the manifest pins — so
    // it is a true report about a different model and the requirement stays owed.
    let directory = scratch("aep-cli-stale-conformance");
    let manifest = directory.join("artifacts.yaml");
    let committed =
        std::fs::read_to_string(root().join(CONFORMANCE_ARTIFACTS)).expect("the manifest is there");
    assert!(
        committed.contains("e19d384dac86219a"),
        "the fixture must pin the digest it is about to change, or it tests nothing"
    );
    write(
        &manifest,
        &committed.replace("e19d384dac86219a", "0000000000000000"),
    );

    let output = evaluate_conformance_task(printable(&manifest), &[PASSING_RECORD]);
    assert_eq!(code(&output), 0, "{}", stderr(&output));
    let text = stdout(&output);
    assert!(
        !text.contains("Task complete"),
        "a passing run against yesterday's model must not close today's task: {text}"
    );
    assert!(
        text.contains("e19d384dac86219a") && text.contains("0000000000000000"),
        "the refusal must name both revisions so a person knows what to re-run: {text}"
    );
}

#[test]
fn the_same_run_claimed_by_the_agent_that_wrote_the_code_does_not_close_the_task() {
    // What `independent: true` buys, checked by taking it away. Every number in the record is
    // identical — the same passing run, the same digest, the same 27 scenarios — and only the
    // producer changes. The requirement stops being satisfied, which is design §32's whole point:
    // an agent's report that its own implementation conforms is not a conformance run.
    let directory = scratch("aep-cli-agent-conformance");
    let record = directory.join("06-conformance.yaml");
    let committed =
        std::fs::read_to_string(root().join(PASSING_RECORD)).expect("the record is there");
    assert!(
        committed.contains("verifier: conformance-runner"),
        "the fixture must start from an independent record or it tests nothing"
    );
    write(
        &record,
        &committed.replace(
            "    producer: verifier\n    verifier: conformance-runner",
            "    producer: agent\n    id: opus-5",
        ),
    );

    let output = evaluate_conformance_task(CONFORMANCE_ARTIFACTS, &[printable(&record)]);
    assert_eq!(code(&output), 0, "{}", stderr(&output));
    let text = stdout(&output);
    assert!(
        !text.contains("Task complete"),
        "an agent's own claim of conformance must not close the task: {text}"
    );
    assert!(
        text.contains("0 of 1 required record(s) submitted"),
        "and the reason is that no independent record was submitted, not that the run failed — \
         the facts it carries are true, they are just not evidence: {text}"
    );
    assert!(
        text.contains("ess_conformance.passed"),
        "the facts are still read off the record; what fails is who produced it: {text}"
    );
}
