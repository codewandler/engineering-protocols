//! `protocol trace evidence`, and the loop it closes.
//!
//! The claim these tests exist for is one sentence: **what the checker writes, the engine reads.**
//! A verb that minted a record the evidence loader could not parse, or one the protocol did not
//! declare, would look correct in every unit test in `trace-spec` and be useless — the two halves
//! run in different processes and the only thing joining them is a file.
//!
//! So the round trip is asserted end to end, through the binary, twice: the document is written to
//! disk and fed back to `protocol evaluate --evidence`, and both renderings the verb offers are
//! shown to be readable by it.

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

/// The eval's own specification, which the repository ships.
const SPEC: &str = "conformance/trace/expectations.trace.yaml";

/// A committed real run of the planning plugin.
const TRANSCRIPT: &str = "crates/trace-spec/tests/fixtures/plugin-eval-7hTYjT.jsonl";

/// A driven `llm` step's record: a `metaharness.event/1` event stream, not `stream-json`.
const EVENT_STREAM: &str = "crates/trace-spec/tests/fixtures/metaharness-driven-honest-step.jsonl";

/// The specification written for a driven step, which the event stream is checked against.
const DRIVEN_SPEC: &str = "conformance/trace/expectations.driven-step.trace.yaml";

/// A development task, so the protocol in force is the one that declares `trace_conformance`.
const TASK: &str = "examples/billing-conformance/task.yaml";

/// Its artifact graph.
const ARTIFACTS: &str = "examples/billing-conformance/artifacts.yaml";

#[test]
fn the_record_the_checker_writes_is_one_the_engine_accepts() {
    // The whole loop, through the binary, in one test: check a real transcript, write the record,
    // and hand the file to the engine. Anything that made the document unreadable — a kind the
    // protocol does not declare, a producer the loader refuses, a shape that is not a list —
    // surfaces here and nowhere else, because `trace-spec` never sees the evidence loader and the
    // engine never sees the checker.
    let out = scratch("aep-trace-evidence-roundtrip").join("trace.yaml");

    let minted = protocol(&[
        "trace",
        "evidence",
        "--spec",
        SPEC,
        "--transcript",
        TRANSCRIPT,
        "--out",
        printable(&out),
    ]);
    assert_eq!(code(&minted), 0, "{}", stderr(&minted));
    assert!(
        stdout(&minted).contains("passed"),
        "the verb reports the verdict it wrote down: {}",
        stdout(&minted)
    );

    let document = std::fs::read_to_string(&out).expect("the record was written");
    assert!(
        document.starts_with("- kind: trace_conformance\n"),
        "the document is a list of records tagged with the declared wire name: {document}"
    );
    assert!(
        document.contains("verifier: trace-checker"),
        "the producer is the class the evidence kind names: {document}"
    );
    assert!(
        !document.contains("producer: agent"),
        "an agent's own claim never mints this kind: {document}"
    );
    assert!(
        document.contains("spec_digest:") && document.contains("transcript_digest:"),
        "the digest pair is what makes the record mean something later: {document}"
    );

    let evaluated = protocol(&[
        "evaluate",
        "--task",
        TASK,
        "--artifacts",
        ARTIFACTS,
        "--evidence",
        printable(&out),
    ]);
    assert_eq!(
        code(&evaluated),
        0,
        "the engine must accept the record the checker wrote: {}",
        stderr(&evaluated)
    );
    assert!(
        !stderr(&evaluated).contains("does not declare evidence of kind"),
        "`protocols/adp/1.yaml` declares `trace_conformance`, so submission must not be refused: {}",
        stderr(&evaluated)
    );
}

#[test]
fn the_json_rendering_is_read_by_the_same_loader_as_the_yaml_one() {
    // Both spellings are offered, so both have to be readable — an option that produced a file the
    // engine refuses is worse than no option.
    let out = scratch("aep-trace-evidence-json").join("trace.json");

    let minted = protocol(&[
        "trace",
        "evidence",
        "--spec",
        SPEC,
        "--transcript",
        TRANSCRIPT,
        "--format",
        "json",
        "--out",
        printable(&out),
    ]);
    assert_eq!(code(&minted), 0, "{}", stderr(&minted));

    let document = std::fs::read_to_string(&out).expect("the record was written");
    assert!(
        document.trim_start().starts_with('['),
        "a list of one, the shape `--evidence` reads: {document}"
    );
    assert!(
        document.ends_with('\n'),
        "the file ends in a newline, as every other document this binary writes does"
    );

    let evaluated = protocol(&[
        "evaluate",
        "--task",
        TASK,
        "--artifacts",
        ARTIFACTS,
        "--evidence",
        printable(&out),
    ]);
    assert_eq!(
        code(&evaluated),
        0,
        "the JSON rendering must be readable too: {}",
        stderr(&evaluated)
    );
}

#[test]
fn a_run_that_gapped_is_written_down_rather_than_exited_on() {
    // The verdict belongs in the record, not in this exit code. A verb that exited 1 on a gap
    // would make a CI job choose between recording what happened and reporting it.
    let out = scratch("aep-trace-evidence-gap");
    let spec = out.join("forbids-bash.trace.yaml");
    std::fs::write(
        &spec,
        "format: trace-spec/1\n\
         id: trace-cli/forbids-bash\n\
         expectations:\n\
        \x20 - id: nothing-shelled-out\n\
        \x20   expect:\n\
        \x20     tool.absent:\n\
        \x20       tool: Bash\n",
    )
    .expect("the fixture is writable");

    let checked = protocol(&[
        "trace",
        "check",
        "--spec",
        printable(&spec),
        "--transcript",
        TRANSCRIPT,
    ]);
    assert_eq!(
        code(&checked),
        1,
        "the fixture reaches the gapping state: {}",
        stdout(&checked)
    );

    let minted = protocol(&[
        "trace",
        "evidence",
        "--spec",
        printable(&spec),
        "--transcript",
        TRANSCRIPT,
    ]);
    assert_eq!(
        code(&minted),
        0,
        "the verdict belongs in the record, not in this exit code: {}",
        stderr(&minted)
    );
    let document = stdout(&minted);
    assert!(
        document.contains("status: failed"),
        "the record says what happened: {document}"
    );
    assert!(
        document.contains("nothing-shelled-out"),
        "and it names the expectation that gapped, so the failure is actionable: {document}"
    );
}

#[test]
fn a_driven_event_stream_is_checked_with_the_same_arguments_as_a_recorded_transcript() {
    // The caller-visible half of the event-stream adapter: no `--format`, no second verb, no flag
    // to get wrong. Which reader runs is decided from the file's own first line, and the report
    // says which one it was — so a verdict that changed because the *reader* changed stays visible
    // as that rather than as a change in the agent's behaviour.
    let checked = protocol(&[
        "trace",
        "check",
        "--spec",
        DRIVEN_SPEC,
        "--transcript",
        EVENT_STREAM,
    ]);
    assert_eq!(
        code(&checked),
        0,
        "{}\n{}",
        stdout(&checked),
        stderr(&checked)
    );
    assert!(
        stdout(&checked).contains("adapter metaharness/event-stream"),
        "the report names the reader that judged the run: {}",
        stdout(&checked)
    );

    // And the record it mints is the same kind of record, from the same loop.
    let out = scratch("aep-trace-evidence-event-stream").join("trace.yaml");
    let minted = protocol(&[
        "trace",
        "evidence",
        "--spec",
        DRIVEN_SPEC,
        "--transcript",
        EVENT_STREAM,
        "--out",
        printable(&out),
    ]);
    assert_eq!(code(&minted), 0, "{}", stderr(&minted));
    let document = std::fs::read_to_string(&out).expect("the record was written");
    assert!(
        document.starts_with("- kind: trace_conformance\n"),
        "{document}"
    );
    assert!(
        document.contains("transcript_digest:"),
        "the record names the driven run it judged: {document}"
    );
}

#[test]
fn a_file_that_is_neither_wire_is_refused_with_the_format_it_was_read_as() {
    // A caller who passed the wrong path gets told which reader was chosen for it, because
    // "this is not a transcript" and "this is not the transcript you meant" are different
    // mistakes and only one of them is fixed by looking at the file.
    let stray = scratch("aep-trace-not-a-transcript").join("notes.md");
    std::fs::write(&stray, "# notes\n\nnothing here is a transcript\n").expect("the scratch tree");
    let refused = protocol(&[
        "trace",
        "check",
        "--spec",
        DRIVEN_SPEC,
        "--transcript",
        printable(&stray),
    ]);
    assert_eq!(code(&refused), 1, "{}", stdout(&refused));
    assert!(
        stderr(&refused).contains("claude-code/stream-json"),
        "the refusal names the wire it fell back to: {}",
        stderr(&refused)
    );
}

#[test]
fn a_downgrade_the_specification_does_not_declare_is_refused_by_the_evidence_verb_too() {
    // `trace check` refuses a `--advisory` id the document does not declare, because a downgrade
    // that matched nothing would relax nothing while looking as though it had. The verb that mints
    // a record must refuse it for the stronger reason: the record names the downgrades, and one
    // naming an id nobody declared would be a false statement about what the run gated on.
    let refused = protocol(&[
        "trace",
        "evidence",
        "--spec",
        SPEC,
        "--transcript",
        TRANSCRIPT,
        "--advisory",
        "no-such-expectation",
    ]);
    assert_eq!(code(&refused), 1, "{}", stdout(&refused));
    assert!(
        stderr(&refused).contains("no-such-expectation"),
        "the refusal names the id it could not find: {}",
        stderr(&refused)
    );
    assert!(
        stdout(&refused).is_empty(),
        "and nothing was written: {}",
        stdout(&refused)
    );
}
