//! `protocol artifact` integration tests.
//!
//! These drive the real binary against a real directory, because that is what the verb family is:
//! a plan is a tree of files, and a test that called the library would not catch an argument that
//! never reaches it, a `--format` declared twice, or a document written to the wrong path.

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
    protocol_in(&root(), args)
}

/// Runs `protocol` with `args` from `directory`, for the verbs that discover a project.
fn protocol_in(directory: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_protocol"))
        .args(args)
        .current_dir(directory)
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

/// An empty scratch directory to build a store in.
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

/// The committed planning store: one initiative, one epic, three stories, two tasks.
const FIXTURE: &str = "examples/planning-passkeys/.engineering/planning";

/// How many artifacts that fixture holds.
const FIXTURE_ARTIFACTS: usize = 7;

/// Every verb in the family, for the sweeps that have to cover all of them.
const VERBS: &[&str] = &[
    "new",
    "move",
    "relate",
    "list",
    "board",
    "graph",
    "validate",
    "kinds",
    "relations",
    "lifecycle",
];

#[test]
fn every_verb_can_be_built_and_asked_for_help() {
    // `clap` refuses a subcommand with two arguments of one name — but only when that subcommand is
    // built, which happens when it is invoked. `protocol artifact graph` panicked exactly that way
    // during development, because `--format` arrived both from the shared arguments and from the
    // graph's own `dot|json`. `--help` builds every one of them.
    for verb in VERBS {
        let output = protocol(&["artifact", verb, "--help"]);
        assert_eq!(
            code(&output),
            0,
            "`protocol artifact {verb} --help` failed: {}",
            stderr(&output)
        );
        assert!(
            stdout(&output).contains("--store") || *verb == "help",
            "`protocol artifact {verb} --help` does not mention --store"
        );
    }
}

#[test]
fn a_new_story_is_written_where_its_id_says_and_validates_clean() {
    let store = scratch("aep-planning-new");

    let created = protocol(&[
        "artifact",
        "new",
        "story",
        "demo",
        "--title",
        "Demo",
        "--store",
        printable(&store),
    ]);
    assert_eq!(code(&created), 0, "{}", stderr(&created));
    assert!(
        stdout(&created).contains("story:demo"),
        "{}",
        stdout(&created)
    );

    let written = store.join("story/demo.md");
    assert!(written.is_file(), "nothing was written to {written:?}");
    let text = std::fs::read_to_string(&written).expect("readable");
    assert!(
        text.starts_with("---\nformat: aep.planning-md/1\n"),
        "{text}"
    );
    assert!(
        text.contains("status: draft"),
        "the story lifecycle starts at draft: {text}"
    );

    let validated = protocol(&["artifact", "validate", "--store", printable(&store)]);
    assert_eq!(code(&validated), 0, "{}", stderr(&validated));
    assert!(
        stdout(&validated).contains("valid"),
        "{}",
        stdout(&validated)
    );
}

#[test]
fn creating_the_same_artifact_twice_is_refused_rather_than_overwriting_it() {
    let store = scratch("aep-planning-twice");
    let arguments = [
        "artifact",
        "new",
        "story",
        "demo",
        "--title",
        "Demo",
        "--store",
        printable(&store),
    ];

    assert_eq!(code(&protocol(&arguments)), 0);
    // Something a person would have lost: the body they wrote after creating it.
    let written = store.join("story/demo.md");
    write(
        &written,
        "---\nid: story:demo\nkind: story\nstatus: draft\n---\n# Hand-written\n",
    );

    let again = protocol(&arguments);
    assert_eq!(code(&again), 1, "{}", stdout(&again));
    assert!(
        stderr(&again).contains("already exists"),
        "{}",
        stderr(&again)
    );
    assert!(
        std::fs::read_to_string(&written)
            .expect("readable")
            .contains("Hand-written"),
        "the refused create overwrote the document anyway"
    );
}

#[test]
fn a_legal_move_rewrites_the_document_and_bumps_the_revision() {
    let store = scratch("aep-planning-move-legal");
    assert_eq!(
        code(&protocol(&[
            "artifact",
            "new",
            "story",
            "demo",
            "--title",
            "Demo",
            "--store",
            printable(&store),
        ])),
        0
    );

    let moved = protocol(&[
        "artifact",
        "move",
        "story:demo",
        "--to",
        "proposed",
        "--store",
        printable(&store),
    ]);
    assert_eq!(code(&moved), 0, "{}", stderr(&moved));

    let text = std::fs::read_to_string(store.join("story/demo.md")).expect("readable");
    assert!(text.contains("status: proposed"), "{text}");
    assert!(
        text.contains("revision: 2"),
        "a write that changed the file did not move the revision: {text}"
    );
}

#[test]
fn an_illegal_move_exits_one_and_names_every_legal_target() {
    // The refusal has to answer the question it creates. A reader told only "no" goes and opens
    // `artifacts/lifecycles/story.yaml`; a reader told what is legal types the next command.
    let store = scratch("aep-planning-move-illegal");
    assert_eq!(
        code(&protocol(&[
            "artifact",
            "new",
            "story",
            "demo",
            "--title",
            "Demo",
            "--store",
            printable(&store),
        ])),
        0
    );

    let refused = protocol(&[
        "artifact",
        "move",
        "story:demo",
        "--to",
        "implemented",
        "--store",
        printable(&store),
    ]);
    assert_eq!(
        code(&refused),
        1,
        "an illegal move is a refusal, not a success"
    );

    let said = format!("{}{}", stdout(&refused), stderr(&refused));
    assert!(said.contains("story:demo is draft"), "{said}");
    for legal in ["proposed", "archived"] {
        assert!(
            said.contains(legal),
            "the refusal does not name `{legal}`, which is a legal move: {said}"
        );
    }

    let text = std::fs::read_to_string(store.join("story/demo.md")).expect("readable");
    assert!(
        text.contains("status: draft"),
        "a refused move changed the file: {text}"
    );
    assert!(
        text.contains("revision: 1"),
        "a refused move bumped the revision: {text}"
    );
}

#[test]
fn an_edge_to_an_artifact_the_store_does_not_hold_is_refused() {
    let store = scratch("aep-planning-dangling");
    assert_eq!(
        code(&protocol(&[
            "artifact",
            "new",
            "story",
            "demo",
            "--title",
            "Demo",
            "--store",
            printable(&store),
        ])),
        0
    );

    let refused = protocol(&[
        "artifact",
        "relate",
        "story:demo",
        "decomposes",
        "epic:absent",
        "--store",
        printable(&store),
    ]);
    assert_eq!(code(&refused), 1);
    assert!(
        stderr(&refused).contains("epic:absent"),
        "{}",
        stderr(&refused)
    );

    let text = std::fs::read_to_string(store.join("story/demo.md")).expect("readable");
    assert!(
        !text.contains("decomposes"),
        "the refused edge was written anyway: {text}"
    );
}

#[test]
fn validate_lists_every_problem_in_a_broken_store() {
    // Three problems of three different classes, and an exact count: "some problems" would pass
    // with a validator that reported the first and stopped, which is the failure this whole
    // accumulate-everything shape exists to prevent.
    let store = scratch("aep-planning-broken");
    write(
        &store.join("story/good.md"),
        &story("story:good", "draft", ""),
    );
    // 1. a file that is not a planning document at all
    write(&store.join("story/loose.md"), "# Just markdown\n");
    // 2. a document filed under a directory that names another kind
    write(
        &store.join("epic/misfiled.md"),
        &story("story:misfiled", "draft", ""),
    );
    // 3. a status the story lifecycle does not have
    write(
        &store.join("story/odd.md"),
        &story("story:odd", "in_review", ""),
    );

    let output = protocol(&["artifact", "validate", "--store", printable(&store)]);
    assert_eq!(code(&output), 1, "a broken store is not valid");

    let text = stdout(&output);
    assert!(text.contains("3 problem(s):"), "{text}");
    for expected in ["loose.md", "misfiled.md", "in_review"] {
        assert!(
            text.contains(expected),
            "no problem mentions `{expected}`: {text}"
        );
    }
    assert_eq!(
        text.lines().filter(|line| line.starts_with("  - ")).count(),
        3,
        "{text}"
    );
}

#[test]
fn a_store_that_cannot_be_read_whole_is_never_written_to() {
    // The rule that makes the refusal above more than a report: two files claiming one id means
    // whichever one a mutation picked, the other would still be there afterwards saying something
    // different.
    let store = scratch("aep-planning-unclean-write");
    write(
        &store.join("story/demo.md"),
        &story("story:demo", "draft", ""),
    );
    write(
        &store.join("story/copy.md"),
        &story("story:demo", "draft", ""),
    );

    let refused = protocol(&[
        "artifact",
        "move",
        "story:demo",
        "--to",
        "proposed",
        "--store",
        printable(&store),
    ]);
    assert_eq!(code(&refused), 1);
    let said = stderr(&refused);
    assert!(said.contains("nothing was written"), "{said}");
    assert!(
        std::fs::read_to_string(store.join("story/demo.md"))
            .expect("readable")
            .contains("status: draft"),
        "the store was written to anyway"
    );
}

#[test]
fn the_fixture_store_validates_clean() {
    let output = protocol(&["artifact", "validate", "--store", FIXTURE]);
    assert_eq!(code(&output), 0, "{}", stdout(&output));
    let text = stdout(&output);
    assert!(
        text.contains(&format!("{FIXTURE_ARTIFACTS} artifact(s)")),
        "{text}"
    );
    assert!(text.contains("valid"), "{text}");
}

#[test]
fn listing_the_fixture_as_json_is_byte_identical_across_two_runs() {
    // Invariant 9 at the command line. Nothing here reads a clock or a hash map, so two runs over
    // one store have to produce one document — otherwise every `--format json` diff is noise and
    // nobody can commit the output of this verb.
    let once = protocol(&["artifact", "list", "--store", FIXTURE, "--format", "json"]);
    let twice = protocol(&["artifact", "list", "--store", FIXTURE, "--format", "json"]);
    assert_eq!(code(&once), 0, "{}", stderr(&once));
    assert_eq!(once.stdout, twice.stdout, "two runs, two documents");

    let text = stdout(&once);
    assert!(text.contains("\"story:passkey-login\""), "{text}");
    assert_eq!(
        text.matches("\"kind\":").count(),
        FIXTURE_ARTIFACTS,
        "{text}"
    );
}

#[test]
fn listing_narrows_by_kind_and_by_status() {
    let by_kind = protocol(&["artifact", "list", "--store", FIXTURE, "--kind", "task"]);
    assert_eq!(code(&by_kind), 0, "{}", stderr(&by_kind));
    assert_eq!(stdout(&by_kind).lines().count(), 2, "{}", stdout(&by_kind));

    let by_status = protocol(&[
        "artifact", "list", "--store", FIXTURE, "--status", "proposed",
    ]);
    assert_eq!(code(&by_status), 0);
    let text = stdout(&by_status);
    assert_eq!(text.lines().count(), 1, "{text}");
    assert!(text.contains("story:passkey-recovery"), "{text}");
}

#[test]
fn the_board_groups_the_fixture_into_status_columns() {
    let output = protocol(&["artifact", "board", "--store", FIXTURE]);
    assert_eq!(code(&output), 0, "{}", stderr(&output));
    let text = stdout(&output);
    for column in ["proposed (1)", "active (4)", "implemented (2)"] {
        assert!(text.contains(column), "no `{column}` column: {text}");
    }
}

#[test]
fn the_graph_draws_every_artifact_and_every_edge() {
    let output = protocol(&["artifact", "graph", "--store", FIXTURE]);
    assert_eq!(code(&output), 0, "{}", stderr(&output));
    let text = stdout(&output);
    assert!(text.starts_with("digraph planning {"), "{text}");
    assert_eq!(
        text.matches(" -> ").count(),
        8,
        "the fixture declares eight edges: {text}"
    );
    assert!(
        text.contains("\"epic:passkey-sign-in\" -> \"initiative:passwordless-authentication\""),
        "{text}"
    );
}

#[test]
fn the_entity_surface_counts_the_fixtures_artifacts() {
    // The same seeder the manifest goes through, fed from the store instead. What the entity
    // surface answers must not depend on which of the two sources it came from.
    let output = protocol(&["entity", "list", "--planning", FIXTURE]);
    assert_eq!(code(&output), 0, "{}", stderr(&output));
    let text = stdout(&output);
    assert_eq!(text.lines().count(), FIXTURE_ARTIFACTS, "{text}");
    assert!(text.contains("aep.initiative/v1"), "{text}");
    assert!(
        text.contains("ep://local/manifest/story/passkey-login"),
        "{text}"
    );
}

#[test]
fn the_entity_surface_refuses_both_sources_and_neither() {
    let neither = protocol(&["entity", "list"]);
    assert_eq!(code(&neither), 2, "a missing source is a usage error");
    assert!(
        stderr(&neither).contains("--artifacts"),
        "{}",
        stderr(&neither)
    );
    assert!(
        stderr(&neither).contains("--planning"),
        "{}",
        stderr(&neither)
    );

    let both = protocol(&[
        "entity",
        "list",
        "--artifacts",
        "examples/development-passkeys/artifacts.yaml",
        "--planning",
        FIXTURE,
    ]);
    assert_eq!(code(&both), 2, "two sources cannot be merged");
}

#[test]
fn the_store_defaults_to_the_planning_directory_of_the_project_it_is_run_in() {
    // The first command an adopting team types should not need a path.
    let project = root().join("examples/planning-passkeys");
    let output = protocol_in(&project, &["artifact", "list"]);
    assert_eq!(code(&output), 0, "{}", stderr(&output));
    assert_eq!(
        stdout(&output).lines().count(),
        FIXTURE_ARTIFACTS,
        "{}",
        stdout(&output)
    );
}

#[test]
fn outside_a_project_the_missing_store_says_what_to_pass() {
    let elsewhere = scratch("aep-planning-not-a-project");
    let output = protocol_in(&elsewhere, &["artifact", "list"]);
    assert_eq!(code(&output), 1);
    let said = stderr(&output);
    assert!(said.contains("--store"), "{said}");
    assert!(said.contains("project.yaml"), "{said}");
}

#[test]
fn the_vocabulary_verbs_answer_without_a_store() {
    // `kinds` and `relations` are questions about the vocabulary. Refusing them because the working
    // directory is not a project would be refusing for a reason unrelated to the question.
    let elsewhere = scratch("aep-planning-vocabulary");

    let kinds = protocol_in(&elsewhere, &["artifact", "kinds"]);
    assert_eq!(code(&kinds), 0, "{}", stderr(&kinds));
    let text = stdout(&kinds);
    assert!(text.contains("story"), "{text}");
    assert_eq!(
        text.lines()
            .filter(|line| line.contains("planning"))
            .count(),
        6,
        "six kinds are intent decomposition: {text}"
    );

    let relations = protocol_in(&elsewhere, &["artifact", "relations"]);
    assert_eq!(code(&relations), 0, "{}", stderr(&relations));
    assert_eq!(
        stdout(&relations).lines().count(),
        13,
        "{}",
        stdout(&relations)
    );
}

#[test]
fn a_lifecycle_is_printed_from_the_documents_the_tree_declares() {
    let output = protocol(&["artifact", "lifecycle", "story"]);
    assert_eq!(code(&output), 0, "{}", stderr(&output));
    let text = stdout(&output);
    assert!(text.contains("story starts at draft"), "{text}");
    assert!(text.contains("draft -> proposed, archived"), "{text}");

    // A kind nobody wrote a ladder for says so, rather than printing an empty one.
    let permissive = protocol(&["artifact", "lifecycle", "runbook"]);
    assert_eq!(code(&permissive), 0);
    assert!(
        stdout(&permissive).contains("declares no lifecycle"),
        "{}",
        stdout(&permissive)
    );
}

#[test]
fn the_new_kinds_have_the_ladder_the_store_needs() {
    // The three lifecycles this store made necessary. Before them an epic could be moved anywhere,
    // because a kind with no ladder is permissive — which reads exactly like a ladder that permits
    // everything, and is why they had to be written rather than assumed.
    for kind in ["epic", "task", "initiative"] {
        let output = protocol(&["artifact", "lifecycle", kind]);
        assert_eq!(code(&output), 0, "{}", stderr(&output));
        let text = stdout(&output);
        assert!(
            text.contains(&format!("{kind} starts at draft")),
            "{kind} declares no lifecycle: {text}"
        );
        assert!(text.contains("implemented -> archived"), "{text}");
    }
}

/// A planning document, for a fixture that needs a broken or an ordinary one.
fn story(id: &str, status: &str, extra: &str) -> String {
    format!("---\nid: {id}\nkind: story\nstatus: {status}\n{extra}---\n# {id}\n")
}
