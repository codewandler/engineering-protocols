//! The second harness — no model, no network, no credential — and therefore the first test the
//! neutrality claim has ever had.
//!
//! § 4.9 bounds the adapter surface at **three** points and says a second harness adopts the whole
//! published set by implementing exactly those three. Until there are two implementations that is a
//! sentence in a design document. This file makes it a gate that can go red, and it does it inside
//! `cargo test --workspace`, which is what `docs/plan/harness-wave-2-driver-decision.md`'s **W3.5**
//! row asks for: the paid eval cannot be a step of `task check`, and this can.
//!
//! What is asserted, and why each one is the load-bearing form of its point:
//!
//! * **point 1** — a second [`LlmStepExecutor`] that is a **real subprocess**, not a mock: `sh`
//!   reads the step's prompt on stdin and writes a transcript at a path the executor named, and the
//!   prompt's byte count comes back *out of the transcript the child wrote*. A fake that returned a
//!   canned string would prove the trait compiles twice and nothing else. It is driven by a real
//!   [`drive`] call over a `StepMap` parsed from YAML and a real `Registry`, so the router, the
//!   cursor and the run directory are exercised rather than stepped around;
//! * **point 2** — the **same** `tool_config` function, rendered into a second vocabulary. The
//!   shared half is the decision and the per-harness half is only the naming table, so the
//!   assertions are that the names differ from Claude Code's, that a shell appears **iff**
//!   `command.execute` is admitted, and that no subagent spawner is ever rendered whatever is
//!   admitted — the three entries § 4.9 point 2 decides rather than leaves to an implementer;
//! * **point 3** — a second transcript reader, and the half of it that actually carries the claim:
//!   `trace_spec::adapter::read_transcript` **fails** on this dialect. Both directions are
//!   asserted, because a dialect the Claude Code adapter happened to read would make the second
//!   reader decorative. `check` and `to_evidence` then mint a `trace_conformance` record from a
//!   transcript no Claude Code wrote, naming `shell-echo/lines` as the reader that judged the run.
//!
//! The trace crates are used and never touched. `TraceIr::new` is public and `AdapterRef`'s fields
//! are public, so the second reader lives here — which is § 4.9 point 3's own decision that a
//! second adapter is a second free function and not a trait added before there was anything to
//! design it against.

use std::collections::{BTreeMap, VecDeque};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use aep_backend_markdown::MarkdownStore;
use aep_domain::capability::{Capability, CapabilityPolicy};
use aep_domain::evidence::{ChangeSet, Evidence, EvidenceKind, Producer, TestResult, TestSuite};
use aep_domain::task::Task;
use aep_domain::verification::{VerificationStatus, Verifier};
use aep_driver::executor::{
    CommandStepExecutor, LlmStepExecutor, OperatorStepExecutor, StepContext, StepOutcome,
};
use aep_driver::run::{drive, DriverOptions, RunDirectory};
use aep_driver::tool::{tool_config, TOOL_CANDIDATES};
use aep_driver_spec::cursor::RunStatus;
use aep_driver_spec::map::{CommandStep, LlmStep, OperatorStep, StepMap};
use aep_driver_spec::tool::ToolConfig;
use aep_engine::clock::SteppingClock;
use aep_engine::engine::{Engine, EvidenceSubmission};
use aep_engine::registry::Registry;
use serde_json::Value;
use trace_domain::code::{TraceCode, ValidationErrors};
use trace_domain::digest::digest_of_bytes;
use trace_domain::ir::{
    AdapterRef, EventKind, RunOutcome, SessionStart, ToolCall, ToolResult, TraceEvent, TraceIr,
};
use trace_domain::spec::TraceSpec;
use trace_spec::adapter::{read_transcript, CLAUDE_CODE_STREAM_JSON};
use trace_spec::check::check;
use trace_spec::evidence::TraceEvidence;
use trace_spec::report::Verdict;

// ---------------------------------------------------------------------------------------------
// Documents — built through the schema crate's parsers, never hand-assembled
// ---------------------------------------------------------------------------------------------

const PROTOCOL: &str = r"
id: aep
version: 1
title: Test protocol
capabilities: [repository.read, repository.write, tests.execute]
evidence_kinds: [test_result, static_analysis, diff, approval, review]
verifiers: [test-runner, static-analyzer, compiler, human-approval, human-review]
artifact_kinds: [specification, design, story]
phases: [implementation, verification, completion]
observables:
  - 'task.**'
  - 'tests.**'
  - 'static_analysis.**'
  - 'diff.**'
  - 'artifact.**'
  - 'evidence.**'
  - 'state.**'
  - 'workflow.**'
  - 'approvals.**'
  - 'review.**'
";

const WORKFLOW: &str = r"
id: test/linear
version: 1
title: Linear
initial: implement
states:
  implement:
    title: Implement
    phases: [implementation]
    requires:
      predicates:
        - artifact.story.exists
  verify:
    title: Verify
    phases: [verification]
  complete:
    title: Complete
    terminal: true
    phases: [completion]
transitions:
  - from: implement
    to: verify
    when: diff.exists
  - from: verify
    to: complete
    when: tests.unit.failed == 0
";

/// A development profile: three capabilities, and deliberately **not** `command.execute`.
///
/// § 4.9's strong property falls out of exactly this document — no development profile grants a
/// shell, so an `llm` step holds none — and the specification below asserts it against the
/// transcript rather than against the policy, which is the only place the claim can be checked
/// after the fact.
const PROFILE: &str = r"
id: test.standard
title: Test standard
protocol: aep/1
workflow: test/linear
capabilities:
  allow: [repository.read, repository.write, tests.execute]
completion:
  - tests.unit.failed == 0
";

const TASK: &str = r"
id: T-1
kind: feature
objective: drive something
protocol: aep/1
profile: test.standard
";

/// The same shape of map the Claude Code executor is driven over, with one word changed.
///
/// `harness: shell-echo` is § 4.9 point 3's selection seam: a second reader is chosen by the step's
/// harness name, not by a trait. The rest of the document is unchanged, which is the claim — the
/// second harness adopts the published map rather than a map written for it.
const MAP: &str = r"
format: aep.driver-steps/1
id: test/shell-echo
workflow: test/linear/1
states:
  implement:
    steps:
      - kind: llm
        harness: shell-echo
        prompt: write the code
      - kind: command
        run: [git, diff]
        evidence:
          kind: diff
          verifier: compiler
  verify:
    steps:
      - kind: command
        run: [cargo, test]
        evidence:
          kind: test_result
          verifier: test-runner
          suite: unit
";

/// A tiny specification over the shell-echo dialect, using existing expectation kinds only.
///
/// Five expectations, each about something the driver decided rather than about something the
/// harness happened to print: the offered set is exactly the protocol's rendering, the suite runner
/// was reached for, no shell was on the table, nothing spawned a subagent, and the run ended
/// cleanly.
const SPEC: &str = r"
format: trace-spec/1
id: shell-echo/acceptance
title: A harness with no model behaved the way the protocol described
expectations:
  - id: the-offered-tools-are-the-protocols-own-rendering
    statement: the tools on the table are exactly what tool_config admitted, in this harness's names
    expect:
      env.tool_available:
        only: [load-skill, read-files, run-tests, write-files]

  - id: no-shell-was-on-the-table
    statement: no development profile grants command.execute, so an llm step holds no shell
    expect:
      env.tool_available:
        tool: shell
        available: false

  - id: the-suite-runner-was-reached-for
    statement: the harness acted with the tool tests.execute admits
    expect:
      tool.called:
        tool: run-tests
        count: {at_least: 1}

  - id: nothing-spawned-a-subagent
    statement: a subagent's tool set is derived by nothing here, so none was ever spawned
    expect:
      subagent.spawned:
        count: {exactly: 0}

  - id: the-run-ended-cleanly
    statement: the terminal record says the harness finished rather than died
    expect:
      result:
        is_error: false
        subtype: success
";

/// Two lines of Claude Code `stream-json`, for the mirror half of the neutrality assertion.
///
/// Real enough to be read: an `init` and a `result`. It is here so the test can show that
/// [`read_transcript`] refuses the shell-echo dialect **because it is a different dialect**, and
/// not because the sample it was handed was broken.
const STREAM_JSON: &str = concat!(
    r#"{"type":"system","subtype":"init","permissionMode":"dontAsk","tools":["Bash","Read"]}"#,
    "\n",
    r#"{"type":"result","subtype":"success","is_error":false}"#,
    "\n"
);

// ---------------------------------------------------------------------------------------------
// The harness itself: a shell script, written to disk at setup
// ---------------------------------------------------------------------------------------------

/// The harness. It runs no model — it echoes the commands it would run.
///
/// Kept inline as a constant and written into the scratch directory at setup, so the repository
/// gains no executable file whose only reader is one test. It takes the transcript path as its one
/// argument, the derived tool vocabulary in `SHELL_ECHO_TOOLS`, and the step's prompt on stdin —
/// which is the whole of adapter point 1's input, spelled in the only vocabulary a shell has.
const HARNESS: &str = r#"
set -eu

transcript="$1"
prompt="$(cat)"

echo 'shell-echo/lines/1' > "$transcript"
echo "session mode=headless tools=$SHELL_ECHO_TOOLS" >> "$transcript"

n=0
for tool in $(echo "$SHELL_ECHO_TOOLS" | tr ',' ' '); do
    n=$((n + 1))
    echo "would run: $tool"
    echo "call id=$n tool=$tool prompt_bytes=${#prompt}" >> "$transcript"
    echo "result id=$n ok=true bytes=${#prompt}" >> "$transcript"
done

echo "outcome status=success turns=$n subagents=0" >> "$transcript"
"#;

/// The first line of a shell-echo transcript: the dialect's own format claim.
const DIALECT: &str = "shell-echo/lines/1";

// ---------------------------------------------------------------------------------------------
// Adapter point 2: the shared decision, rendered into a second vocabulary
// ---------------------------------------------------------------------------------------------

/// Claude Code's table, as `protocol-cli` renders it, for the disjointness assertion.
///
/// Duplicated here rather than imported: `aep-driver` does not depend on `protocol-cli` and must
/// not start to. The point of the assertion is that two harnesses genuinely name their tools
/// differently, and a copy of the names is enough to establish that.
const CLAUDE_CODE_NAMES: &[&str] = &[
    "Bash",
    "Edit",
    "Glob",
    "Grep",
    "NotebookEdit",
    "Read",
    "Skill",
    "WebFetch",
    "WebSearch",
    "Write",
];

/// Every spelling of "spawn another agent" this test knows to look for.
const SUBAGENT_NAMES: &[&str] = &["Task", "spawn-agent", "subagent", "delegate"];

/// This harness's naming table — the **only** per-harness half of adapter point 2.
///
/// It renders a [`ToolConfig`] and never re-decides one: there is no path here that consults a
/// policy, so this harness cannot quietly conclude that `repository.write` admits a shell. The
/// three entries that are not functions of a capability are the ones § 4.9 decides — a shell iff
/// `command.execute`, a skill loader as a named exemption, and a subagent spawner never.
fn shell_echo_tools(config: &ToolConfig) -> Vec<String> {
    let mut tools: Vec<String> = Vec::new();
    if config.admits(&Capability::RepositoryRead) || config.admits(&Capability::ArtifactRead) {
        tools.push("read-files".to_owned());
    }
    if config.admits(&Capability::RepositoryWrite) {
        tools.push("write-files".to_owned());
    }
    if config.admits(&Capability::TestExecution) {
        tools.push("run-tests".to_owned());
    }
    if config.shell_offered() {
        tools.push("shell".to_owned());
    }
    if config.skills_offered() {
        tools.push("load-skill".to_owned());
    }
    // `config.subagents_offered()` is a constant `false` and there is deliberately no name to
    // render it into: a table that carried one would only be a branch away from offering it.
    tools.sort();
    tools.dedup();
    tools
}

// ---------------------------------------------------------------------------------------------
// Adapter point 1: a second `LlmStepExecutor`, which is a real process
// ---------------------------------------------------------------------------------------------

/// What the harness does for a `command` or an `operator` step, scripted by the test.
///
/// Those two step kinds are not what this file is about — § 4.9's three points are all on the
/// `llm` side and the transcript side — so they behave like `driving.rs`'s `Fake` and keep the
/// run walking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Act {
    /// A verifier saw a change.
    Diff,
    /// A suite ran to completion.
    Tests {
        /// How many of it failed.
        failed: usize,
    },
}

/// A second harness: `sh`, a script, and a transcript in a dialect of its own.
#[derive(Debug)]
struct ShellEcho {
    /// Where the script was written at setup.
    script: PathBuf,
    /// What the non-`llm` steps do next.
    acts: VecDeque<Act>,
    /// The tool vocabulary handed to each `llm` step, in order.
    rendered: Vec<Vec<String>>,
    /// The transcript each `llm` step produced, in order.
    transcripts: Vec<PathBuf>,
    /// What each `llm` step echoed on stdout, in order.
    echoed: Vec<String>,
    /// How many requirement lines travelled with each `llm` step.
    requirements: Vec<usize>,
}

impl ShellEcho {
    /// A harness whose script lives at `script`.
    fn new(script: PathBuf, acts: &[Act]) -> Self {
        Self {
            script,
            acts: acts.iter().copied().collect(),
            rendered: Vec::new(),
            transcripts: Vec::new(),
            echoed: Vec::new(),
            requirements: Vec::new(),
        }
    }
}

impl LlmStepExecutor for ShellEcho {
    fn run_llm(&mut self, step: &LlmStep, context: &StepContext<'_>) -> StepOutcome {
        assert_eq!(
            step.harness, "shell-echo",
            "the map selected this harness by name, which is point 3's selection seam"
        );
        let tools = shell_echo_tools(context.tools);
        std::fs::create_dir_all(context.run_directory).expect("a writable run directory");
        let transcript = context.run_directory.join(format!(
            "shell-echo-{}-{}-{}.trace",
            context.state, context.index, context.attempt
        ));

        let mut child = Command::new("sh")
            .arg(&self.script)
            .arg(&transcript)
            .env("SHELL_ECHO_TOOLS", tools.join(","))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("`sh` starts");
        child
            .stdin
            .take()
            .expect("a piped stdin")
            .write_all(step.prompt.as_bytes())
            .expect("the prompt reaches the harness on stdin");
        let output = child.wait_with_output().expect("the harness exits");
        assert!(
            output.status.success(),
            "the shell-echo harness failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        self.rendered.push(tools);
        self.transcripts.push(transcript);
        self.echoed
            .push(String::from_utf8_lossy(&output.stdout).into_owned());
        self.requirements.push(context.requirements.len());
        // An `llm` step has no other honest outcome: an agent's own statement never satisfies an
        // independence requirement, and this one is not even an agent.
        StepOutcome::Nothing
    }
}

impl CommandStepExecutor for ShellEcho {
    fn run_command(&mut self, _: &CommandStep, _: &StepContext<'_>) -> StepOutcome {
        match self
            .acts
            .pop_front()
            .expect("an act for every command step")
        {
            Act::Diff => StepOutcome::Observed(Box::new(EvidenceSubmission::new(
                Evidence::Diff(ChangeSet {
                    files_changed: 1,
                    lines_added: 4,
                    lines_removed: 0,
                    revision_before: None,
                    revision_after: None,
                    paths: vec!["src/lib.rs".to_owned()],
                }),
                Producer::Verifier {
                    verifier: Verifier::Compiler,
                },
            ))),
            Act::Tests { failed } => StepOutcome::Observed(Box::new(EvidenceSubmission::new(
                Evidence::TestResult(TestResult::failing(TestSuite::Unit, 7, failed)),
                Producer::Verifier {
                    verifier: Verifier::TestRunner,
                },
            ))),
        }
    }
}

impl OperatorStepExecutor for ShellEcho {
    fn run_operator(&mut self, step: &OperatorStep, _: &StepContext<'_>) -> StepOutcome {
        StepOutcome::Paused {
            reason: step.prompt.clone(),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Adapter point 3: a second reader, for a dialect the first one cannot read
// ---------------------------------------------------------------------------------------------

/// This reader, and the dialect version it was written against.
///
/// Its own [`AdapterRef`], which is what makes a report say *which* reader judged the run — the
/// same reason `CLAUDE_CODE_STREAM_JSON` carries one.
const SHELL_ECHO_LINES: AdapterRef = AdapterRef {
    name: "shell-echo/lines",
    written_against: &[DIALECT],
};

/// The value of one `key=value` word on a record line.
fn field<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    line.split_whitespace()
        .find_map(|word| word.strip_prefix(key)?.strip_prefix('='))
}

/// Reads a shell-echo transcript into the neutral IR.
///
/// Line-oriented and not JSON at all, which is the point: `trace_spec::adapter::read_transcript`
/// refuses it, and everything downstream of [`TraceIr`] — `check`, `CheckReport`, `to_evidence` —
/// takes it without having heard of either harness.
///
/// Correlation and indexing are [`TraceIr::new`]'s, exactly as they are for the first adapter: a
/// second reader that numbered its own events would be a second owner of what a verdict cites.
fn read_shell_echo(bytes: &[u8]) -> Result<TraceIr, ValidationErrors> {
    let mut errors = ValidationErrors::new();
    let Ok(text) = std::str::from_utf8(bytes) else {
        errors.refuse(
            TraceCode::AdapterMalformedTranscript,
            "transcript",
            "the transcript's bytes are not UTF-8",
        );
        return Err(errors);
    };
    let mut lines = text.lines().enumerate();
    if lines.next().map(|(_, first)| first) != Some(DIALECT) {
        errors.refuse(
            TraceCode::AdapterMalformedTranscript,
            "transcript:1",
            format!("a shell-echo transcript opens with `{DIALECT}`"),
        );
        return Err(errors);
    }

    let mut events: Vec<TraceEvent> = Vec::new();
    for (offset, line) in lines {
        let at = offset + 1;
        let kind = match line.split_whitespace().next() {
            Some("session") => EventKind::SessionStart(Box::new(SessionStart {
                harness_version: Some(DIALECT.to_owned()),
                permission_mode: field(line, "mode").map(ToOwned::to_owned),
                tools: field(line, "tools")
                    .map(|list| list.split(',').map(ToOwned::to_owned).collect()),
                ..SessionStart::default()
            })),
            Some("call") => EventKind::ToolCall(Box::new(ToolCall {
                call_id: field(line, "id").map(ToOwned::to_owned),
                name: field(line, "tool").unwrap_or_default().to_owned(),
                input: field(line, "prompt_bytes")
                    .map(|bytes| ("prompt_bytes".to_owned(), Value::from(bytes)))
                    .into_iter()
                    .collect(),
                input_bytes: line.len(),
                result_event: None,
            })),
            Some("result") => EventKind::ToolResult(Box::new(ToolResult {
                call_id: field(line, "id").map(ToOwned::to_owned),
                is_error: Some(field(line, "ok") != Some("true")),
                content_bytes: field(line, "bytes")
                    .and_then(|n| n.parse().ok())
                    .unwrap_or(0),
                content: None,
                fields: BTreeMap::new(),
            })),
            Some("outcome") => EventKind::RunOutcome(Box::new(RunOutcome {
                is_error: Some(field(line, "status") != Some("success")),
                subtype: field(line, "status").map(ToOwned::to_owned),
                num_turns: field(line, "turns").and_then(|n| n.parse().ok()),
                subagents_spawned: field(line, "subagents").and_then(|n| n.parse().ok()),
                ..RunOutcome::default()
            })),
            _ => {
                errors.refuse(
                    TraceCode::AdapterMalformedTranscript,
                    format!("transcript:{at}"),
                    format!("no record this reader knows: {line}"),
                );
                continue;
            }
        };
        events.push(TraceEvent::new(at, None, kind));
    }
    if events.is_empty() {
        errors.refuse(
            TraceCode::AdapterEmptyTranscript,
            "transcript",
            "a transcript with no records at all",
        );
    }
    errors.into_result(TraceIr::new(
        digest_of_bytes(bytes),
        SHELL_ECHO_LINES,
        events,
        Vec::new(),
    ))
}

// ---------------------------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------------------------

fn registry() -> Registry {
    let mut registry = Registry::new();
    registry
        .insert_protocol(aep_schema::parse::protocol(PROTOCOL, None).expect("the protocol parses"))
        .expect("the protocol is unique");
    registry
        .insert_workflow(aep_schema::parse::workflow(WORKFLOW, None).expect("the workflow parses"))
        .expect("the workflow is unique");
    registry
        .insert_profile(aep_schema::parse::profile(PROFILE, None).expect("the profile parses"))
        .expect("the profile is unique");
    registry
}

fn engine() -> Engine<SteppingClock> {
    Engine::with_clock(registry(), SteppingClock::new(1_000, 10))
}

fn task() -> Task {
    aep_schema::parse::task(TASK, None).expect("the task parses")
}

fn map() -> StepMap {
    aep_schema::parse::step_map(MAP, Some("test/shell-echo.yaml"))
        .expect("the fixture map validates")
}

fn spec() -> TraceSpec {
    trace_domain::raw::read_spec(SPEC)
        .unwrap_or_else(|errors| panic!("the fixture specification must validate:\n{errors}"))
}

/// A directory under this test binary's target directory, named for the test that asked for it.
///
/// `driving.rs`'s helper verbatim: no temporary-directory crate, no randomness, no `/tmp`, and a
/// failed run leaves its files where a person can read them.
fn scratch(name: &str) -> PathBuf {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    if path.exists() {
        std::fs::remove_dir_all(&path).expect("the previous run's directory is removable");
    }
    std::fs::create_dir_all(&path).expect("a writable scratch directory");
    path
}

/// Writes the harness script into `root` and returns where it landed.
fn install_harness(root: &Path) -> PathBuf {
    let script = root.join("shell-echo.sh");
    std::fs::write(&script, HARNESS).expect("a writable script");
    script
}

/// Drives the whole map with the shell-echo harness and hands back what it produced.
fn driven(name: &str) -> (ShellEcho, RunDirectory) {
    let root = scratch(name);
    let run = RunDirectory::at(root.join("runs").join("T-1").join("1"));
    let mut harness = ShellEcho::new(
        install_harness(&root),
        &[Act::Diff, Act::Tests { failed: 0 }],
    );
    let report = drive(
        &engine(),
        &task(),
        &MarkdownStore::open(root.join("planning")),
        &map(),
        &run,
        &mut harness,
        &DriverOptions::default(),
    )
    .expect("the run finishes");
    assert_eq!(
        report.status(),
        RunStatus::Completed,
        "the second harness walks the published map to the end, or nothing below means anything"
    );
    (harness, run)
}

/// The transcript the one `llm` step of a driven run produced.
fn transcript_of(harness: &ShellEcho) -> Vec<u8> {
    std::fs::read(&harness.transcripts[0]).expect("the harness wrote its transcript")
}

// ---------------------------------------------------------------------------------------------
// Point 1 — a second `LlmStepExecutor`, and it is a process
// ---------------------------------------------------------------------------------------------

#[test]
fn a_second_llm_executor_that_is_a_real_subprocess_walks_the_published_map_to_completion() {
    let (harness, run) = driven("shell-echo-run");

    assert_eq!(
        harness.transcripts.len(),
        1,
        "one model session per step (D4), and the map has one `llm` step"
    );
    assert!(
        harness.transcripts[0].starts_with(run.path()),
        "the transcript belongs to the run directory the driver named, not to a path the harness \
         invented: {:?}",
        harness.transcripts[0]
    );

    let transcript = String::from_utf8(transcript_of(&harness)).expect("the transcript is text");
    assert!(
        transcript.starts_with(DIALECT),
        "a dialect of its own, declared on its first line: {transcript}"
    );
    // The load-bearing half of "a harness rather than a mock": this number was computed by `sh`
    // from bytes that travelled down a pipe into another process, and read back out of a file that
    // process wrote. No in-Rust fake can produce it.
    let prompt = "write the code";
    assert!(
        transcript.contains(&format!("prompt_bytes={}", prompt.len())),
        "the child read the step's prompt on stdin and measured it: {transcript}"
    );
    assert!(
        harness.echoed[0].contains("would run: run-tests"),
        "the harness acts by echoing the commands it would run: {}",
        harness.echoed[0]
    );
    assert_eq!(
        harness.requirements[0], 1,
        "the requirement lines in force travel to the second harness exactly as they do to the \
         first — one line per requirement, never a summary"
    );
}

// ---------------------------------------------------------------------------------------------
// Point 2 — one `tool_config`, two vocabularies
// ---------------------------------------------------------------------------------------------

#[test]
fn the_shared_tool_config_renders_into_this_harnesss_own_names_and_never_into_claude_codes() {
    let (harness, _) = driven("shell-echo-tools");
    assert_eq!(
        harness.rendered[0],
        ["load-skill", "read-files", "run-tests", "write-files"],
        "the profile's three capabilities, in this harness's names, plus the skill loader"
    );
    for name in &harness.rendered[0] {
        assert!(
            !CLAUDE_CODE_NAMES.contains(&name.as_str()),
            "`{name}` is Claude Code's word, and a second harness that reused the table would be \
             testing one rendering twice"
        );
    }
}

#[test]
fn a_shell_is_rendered_exactly_when_command_execute_is_admitted_and_never_otherwise() {
    // Both directions over the *real* `tool_config`, so the iff is a property of the shared
    // decision and not of this file's table.
    for capabilities in [
        vec![Capability::RepositoryRead],
        vec![Capability::RepositoryRead, Capability::RepositoryWrite],
        vec![Capability::TestExecution],
        vec![Capability::CommandExecution],
        vec![Capability::RepositoryWrite, Capability::CommandExecution],
    ] {
        let admitted = capabilities.contains(&Capability::CommandExecution);
        let config = tool_config(&CapabilityPolicy::allowing(capabilities.clone()));
        assert_eq!(
            shell_echo_tools(&config).contains(&"shell".to_owned()),
            admitted,
            "a shell is offered iff `command.execute` is admitted, and {capabilities:?} admits it: \
             {admitted}"
        );
    }
}

#[test]
fn no_subagent_spawner_is_rendered_however_much_the_policy_admits() {
    let everything = tool_config(&CapabilityPolicy::allowing(TOOL_CANDIDATES.iter().cloned()));
    assert!(
        !everything.is_empty(),
        "the widest policy admits something, or the assertion below is vacuous"
    );
    assert!(
        !everything.subagents_offered(),
        "the decision is the shared one and this harness cannot re-take it"
    );
    for name in shell_echo_tools(&everything) {
        assert!(
            !SUBAGENT_NAMES.contains(&name.as_str()),
            "`{name}` spawns an agent whose tool set is derived by nothing here, which is a route \
             around the per-state allowlist"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Point 3 — a second reader, and a record from a transcript no Claude Code wrote
// ---------------------------------------------------------------------------------------------

#[test]
fn the_claude_code_adapter_refuses_the_dialect_this_files_own_reader_understands() {
    let (harness, _) = driven("shell-echo-dialect");
    let bytes = transcript_of(&harness);

    // The load-bearing half of the neutrality claim. If the first adapter could read this, the
    // second reader below would be decorative and the seam would be untested.
    let refusal = read_transcript(&bytes)
        .expect_err("the Claude Code adapter must not be able to read another harness's dialect");
    assert!(
        refusal.contains(TraceCode::AdapterMalformedTranscript),
        "and it refuses by code rather than by guessing: {refusal}"
    );

    // The mirror, so the refusal above is about the dialect and not about a broken sample: the
    // first adapter reads `stream-json` happily, and this file's reader refuses it.
    read_transcript(STREAM_JSON.as_bytes()).expect("the sample really is a stream-json transcript");
    read_shell_echo(STREAM_JSON.as_bytes())
        .expect_err("and this reader has never heard of stream-json");

    let ir = read_shell_echo(&bytes).expect("its own reader reads it");
    assert_eq!(ir.adapter.name, "shell-echo/lines");
    assert_ne!(
        ir.adapter.name, CLAUDE_CODE_STREAM_JSON.name,
        "two readers, two names — which is how a report says which one judged a run"
    );
    assert_eq!(ir.format, trace_domain::ir::IR_FORMAT, "one neutral IR");
}

#[test]
fn a_transcript_no_claude_code_wrote_is_checked_and_mints_a_trace_conformance_record() {
    let (harness, _) = driven("shell-echo-evidence");
    let bytes = transcript_of(&harness);
    let ir = read_shell_echo(&bytes).expect("the second reader reads it");

    let report = check(&spec(), &ir, &[]);
    assert_eq!(
        report.verdict,
        Verdict::Ok,
        "the checker has never heard of either harness: {}",
        trace_spec::render::report_to_text(&report)
    );
    assert_eq!(report.exit_code(), 0);
    assert_eq!(report.summary.total, 5);
    assert_eq!(report.summary.gap, 0, "nothing was contradicted");
    assert_eq!(
        report.summary.unknown, 0,
        "and nothing was undecidable, which is what makes the pass mean something"
    );

    let evidence = report
        .to_evidence()
        .expect("the report's digests are digests");
    assert_eq!(
        evidence.evidence().kind(),
        EvidenceKind::TraceConformance,
        "the record the protocol decides on, minted from a transcript no Claude Code wrote"
    );
    assert_eq!(
        evidence.producer(),
        &TraceEvidence::PRODUCER,
        "the trace checker as a verifier, and not this test describing itself as one"
    );
    assert_eq!(evidence.result().status, VerificationStatus::Passed);
    assert_eq!(evidence.result().specification, "shell-echo/acceptance");
    assert!(
        evidence
            .result()
            .adapter
            .as_deref()
            .is_some_and(|adapter| adapter.starts_with("shell-echo/lines")),
        "the record names the reader that judged the run: {:?}",
        evidence.result().adapter
    );
}
