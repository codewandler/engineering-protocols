//! CLI integration tests for the entity and audit surface.
//!
//! These drive the real binary for the same reason the rest of the CLI tests do: a harness shells
//! out to `protocol` and reads its exit code, so an argument that never reaches the library is a
//! failure the library's own tests cannot see.
//!
//! Everything here goes through the in-memory backend, seeded from the example manifest on each
//! invocation. That is what the assertions are about: `entity history` showing exactly one record
//! is the surface telling the truth about itself, not a thin history.

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

const ARTIFACTS: &str = "examples/development-passkeys/artifacts.yaml";
const DESIGN: &str = "ep://local/manifest/design/passkeys-auth";
const SPECIFICATION: &str = "ep://local/manifest/specification/passkeys-auth";

#[test]
fn entity_list_shows_one_line_per_artifact_in_the_manifest() {
    let output = protocol(&["entity", "list", "--artifacts", ARTIFACTS]);
    assert_eq!(code(&output), 0, "{}", stderr(&output));
    let text = stdout(&output);

    assert_eq!(
        text.lines().count(),
        6,
        "the example manifest declares six artifacts: {text}"
    );
    assert!(text.contains("aep.design/v1"), "{text}");
    assert!(text.contains(DESIGN), "{text}");
    assert!(
        text.contains("aep.architecture-decision-record/v1"),
        "an ADR is an entity like anything else: {text}"
    );
}

#[test]
fn entity_list_narrows_to_one_type() {
    let output = protocol(&[
        "entity",
        "list",
        "--artifacts",
        ARTIFACTS,
        "--type",
        "aep.design/v1",
    ]);
    assert_eq!(code(&output), 0, "{}", stderr(&output));
    let text = stdout(&output);
    assert_eq!(text.lines().count(), 1, "{text}");
    assert!(text.contains(DESIGN), "{text}");
}

#[test]
fn entity_get_by_locator_prints_the_design_the_manifest_declares() {
    let output = protocol(&["entity", "get", "--artifacts", ARTIFACTS, DESIGN]);
    assert_eq!(code(&output), 0, "{}", stderr(&output));
    let text = stdout(&output);

    assert!(text.contains("type       aep.design/v1"), "{text}");
    assert!(text.contains(&format!("locator    {DESIGN}")), "{text}");
    assert!(text.contains("revision   1"), "{text}");
    assert!(
        text.contains("status: approved"),
        "the body carries the manifest's status: {text}"
    );
    assert!(
        text.contains("version: '7'"),
        "and the version it is at: {text}"
    );
}

#[test]
fn entity_get_by_an_unknown_locator_refuses_rather_than_printing_nothing() {
    let output = protocol(&[
        "entity",
        "get",
        "--artifacts",
        ARTIFACTS,
        "ep://local/manifest/design/no-such-design",
    ]);
    assert_eq!(
        code(&output),
        1,
        "an unanswerable question is a non-zero exit"
    );
    assert!(
        stderr(&output).contains("no-such-design"),
        "the message names what was not found: {}",
        stderr(&output)
    );
}

#[test]
fn entity_history_shows_the_seeding_and_nothing_else() {
    let output = protocol(&["entity", "history", "--artifacts", ARTIFACTS, DESIGN]);
    assert_eq!(code(&output), 0, "{}", stderr(&output));
    let text = stdout(&output);

    assert_eq!(
        text.lines().count(),
        1,
        "the backend is in-memory: the only history is this run's own seeding: {text}"
    );
    assert!(text.contains("r1"), "{text}");
    assert!(text.contains("service:protocol-cli"), "{text}");
    assert!(
        text.contains("seed-design-passkeys-auth"),
        "the command that created it is named: {text}"
    );
}

#[test]
fn entity_relations_shows_what_the_design_designs() {
    let output = protocol(&["entity", "relations", "--artifacts", ARTIFACTS, DESIGN]);
    assert_eq!(code(&output), 0, "{}", stderr(&output));
    let text = stdout(&output);

    assert_eq!(text.lines().count(), 1, "{text}");
    assert!(text.contains("designs"), "{text}");
    assert!(
        text.contains(SPECIFICATION),
        "the edge names the other end: {text}"
    );
}

#[test]
fn entity_relations_incoming_answers_what_points_at_this() {
    let output = protocol(&[
        "entity",
        "relations",
        "--artifacts",
        ARTIFACTS,
        "--incoming",
        DESIGN,
    ]);
    assert_eq!(code(&output), 0, "{}", stderr(&output));
    let text = stdout(&output);

    assert!(
        text.contains("reviews"),
        "the review points at the design, and the design does not point back: {text}"
    );
    assert!(
        text.contains("review-result/design-passkeys-auth"),
        "{text}"
    );
    assert!(
        text.contains("decides"),
        "so does the ADR that decided it: {text}"
    );
}

#[test]
fn audit_lists_the_commands_that_seeded_the_manifest() {
    let output = protocol(&["audit", "--artifacts", ARTIFACTS]);
    assert_eq!(code(&output), 0, "{}", stderr(&output));
    let text = stdout(&output);

    assert_eq!(
        text.lines().count(),
        11,
        "six creations and five relations: {text}"
    );
    assert!(text.contains("command_accepted"), "{text}");
    assert!(text.contains("seed-design-passkeys-auth"), "{text}");
    assert!(
        text.contains("seed-rel-design-passkeys-auth-designs-spec-passkeys-auth"),
        "a relation is a command too, and leaves a record: {text}"
    );
}

#[test]
fn audit_rejected_is_empty_when_nothing_was_refused() {
    let output = protocol(&["audit", "--artifacts", ARTIFACTS, "--rejected"]);
    assert_eq!(code(&output), 0, "{}", stderr(&output));
    assert!(
        stdout(&output).trim().is_empty(),
        "a clean seed refuses nothing: {}",
        stdout(&output)
    );
}

#[test]
fn describe_says_a_design_accepts_an_approval() {
    let output = protocol(&["describe", "--artifacts", ARTIFACTS, "aep.design/v1"]);
    assert_eq!(code(&output), 0, "{}", stderr(&output));
    let text = stdout(&output);

    assert!(text.contains("type       aep.design/v1"), "{text}");
    assert!(
        text.contains("aep.design.approve/v1"),
        "this is how a harness learns a design can be approved: {text}"
    );
    assert!(text.contains("mutable    yes"), "{text}");
}

#[test]
fn json_output_is_machine_readable() {
    let output = protocol(&[
        "entity",
        "list",
        "--artifacts",
        ARTIFACTS,
        "--format",
        "json",
    ]);
    assert_eq!(code(&output), 0, "{}", stderr(&output));

    let parsed: serde_json::Value =
        serde_json::from_str(&stdout(&output)).expect("the listing is valid JSON");
    let entities = parsed.as_array().expect("a list of entities");
    assert_eq!(entities.len(), 6);

    let design = entities
        .iter()
        .find(|entity| entity["metadata"]["type"] == "aep.design/v1")
        .expect("the design is in the listing");
    assert_eq!(design["metadata"]["locator"], DESIGN);
    assert_eq!(design["data"]["status"], "approved");
}
