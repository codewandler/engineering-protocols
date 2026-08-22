//! The loop, exercised against a fake harness — which is the acceptance criterion for the
//! neutrality claim, not a testing convenience.
//!
//! Every claim § 4.9 makes about the adapter surface is a claim that a second implementation of
//! [`StepExecutors`] can drive the same workflow, the same map and the same `tool_config`. The fake
//! here is that second implementation, and it needs no model, no network and no credential.
//!
//! What is asserted, and why each one is the load-bearing form of its rule:
//!
//! * a step that observed nothing submits nothing **and changes nothing** — asserted by comparing
//!   the whole `Evaluation` before and after, because "no evidence" is only half of D5;
//! * an exhausted budget leaves a resumable directory and carries the engine's `Blocked` reasons
//!   **verbatim** — asserted against what `transition()` itself returns, not against a copy;
//! * an unparseable file stops the run, asserted by the fact base being **unchanged** rather than
//!   silently shrunk, because that is precisely the case a `graph()`-only check waves through (F7);
//! * a resume refuses on a moved pin **while `Engine::restore` accepts the same snapshot**, which
//!   is what makes the cursor check load-bearing rather than decorative.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};

use aep_backend_markdown::MarkdownStore;
use aep_domain::artifact::ArtifactGraph;
use aep_domain::evidence::{ChangeSet, Evidence, Producer, TestResult, TestSuite};
use aep_domain::facts::{FactPath, FactSource, FactValue};
use aep_domain::task::Task;
use aep_domain::time::{ObservedAt, Timestamp};
use aep_domain::verification::Verifier;
use aep_driver::executor::{
    CommandStepExecutor, LlmStepExecutor, OperatorStepExecutor, StepContext, StepOutcome,
};
use aep_driver::run::{drive, resume, DriveError, DriverOptions, RunDirectory};
use aep_driver_spec::cursor::RunStatus;
use aep_driver_spec::map::{CommandStep, LlmStep, OperatorStep, StepMap};
use aep_engine::clock::SteppingClock;
use aep_engine::engine::{Engine, EvidenceSubmission, ProtocolEngine, TransitionResult};
use aep_engine::registry::Registry;
use aep_engine::Snapshot;

// ---------------------------------------------------------------------------------------------
// Documents
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
    # A requirement the *current* state owes, which the evaluator reports without it gating the
    # outgoing transition — so a step can be handed a requirement line without the run being stuck.
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
  # The deliberate back-edge: a workflow that can only go forwards is a lie about how engineering
  # works, and the driver must be able to go round again — and must not go round forever.
  - from: verify
    to: implement
    when: tests.unit.failed > 0
";

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

/// The map the whole-run tests use: one `llm` step and one `command` step, then a suite.
const MAP: &str = r"
format: aep.driver-steps/1
id: test/driving
workflow: test/linear/1
states:
  implement:
    steps:
      - kind: llm
        prompt: write the code
      - kind: command
        run: [git, diff]
        retries: 1
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

/// A map whose only step is a command that will not run, so a budget can be spent on it.
const CRASHING_MAP: &str = r"
format: aep.driver-steps/1
id: test/crashing
workflow: test/linear/1
states:
  implement:
    steps:
      - kind: command
        run: [does-not-exist]
        retries: 1
        evidence:
          kind: diff
          verifier: compiler
";

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

fn map(text: &str) -> StepMap {
    aep_schema::parse::step_map(text, Some("test/driving.yaml")).expect("the fixture map validates")
}

// ---------------------------------------------------------------------------------------------
// A second harness, which is what proves the seam is real
// ---------------------------------------------------------------------------------------------

/// What the fake harness does next, scripted by the test so the order is visible in one place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Act {
    /// A verifier saw a change: submit a `ChangeSet` produced by the compiler.
    Diff,
    /// A suite ran to completion: submit a `TestResult`, passing or failing.
    Tests { failed: usize },
    /// The step could not run at all — D5's `Unknown`.
    Crash(&'static str),
    /// The step ran and there is nothing to submit.
    Done,
    /// A person is owed a question.
    Pause(&'static str),
}

/// What the harness was asked to do, so a test can assert on what the driver told it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Asked {
    state: String,
    index: usize,
    attempt: u32,
    tools: usize,
    requirements: usize,
    reaching: Vec<String>,
}

/// A harness that runs a script instead of a model, a program or a person.
#[derive(Debug)]
struct Fake {
    script: VecDeque<Act>,
    asked: Vec<Asked>,
}

impl Fake {
    fn new(script: &[Act]) -> Self {
        Self {
            script: script.iter().copied().collect(),
            asked: Vec::new(),
        }
    }

    fn act(&mut self, context: &StepContext<'_>) -> StepOutcome {
        self.asked.push(Asked {
            state: context.state.to_string(),
            index: context.index,
            attempt: context.attempt,
            tools: context.tools.capabilities().len(),
            requirements: context.requirements.len(),
            reaching: context.reaching.to_vec(),
        });
        let act = self.script.pop_front().expect(
            "the script has an act for every step the driver runs; an empty script means the \
             driver ran a step the test did not expect",
        );
        match act {
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
                ObservedAt::new(Timestamp::EPOCH),
            ))),
            Act::Tests { failed } => StepOutcome::Observed(Box::new(EvidenceSubmission::new(
                Evidence::TestResult(TestResult::failing(TestSuite::Unit, 7, failed)),
                Producer::Verifier {
                    verifier: Verifier::TestRunner,
                },
                ObservedAt::new(Timestamp::EPOCH),
            ))),
            Act::Crash(reason) => StepOutcome::NoVerdict {
                reason: reason.to_owned(),
            },
            Act::Done => StepOutcome::Nothing,
            Act::Pause(reason) => StepOutcome::Paused {
                reason: reason.to_owned(),
            },
        }
    }
}

impl LlmStepExecutor for Fake {
    fn run_llm(&mut self, _: &LlmStep, context: &StepContext<'_>) -> StepOutcome {
        self.act(context)
    }
}

impl CommandStepExecutor for Fake {
    fn run_command(&mut self, _: &CommandStep, context: &StepContext<'_>) -> StepOutcome {
        self.act(context)
    }
}

impl OperatorStepExecutor for Fake {
    fn run_operator(&mut self, _: &OperatorStep, context: &StepContext<'_>) -> StepOutcome {
        self.act(context)
    }
}

// ---------------------------------------------------------------------------------------------
// Scratch directories
// ---------------------------------------------------------------------------------------------

/// A directory under this test binary's target directory, named for the test that asked for it.
///
/// No temporary-directory crate and no randomness: the name is the test's, the directory is emptied
/// on the way in, and a failed run leaves its files where a person can read them.
fn scratch(name: &str) -> PathBuf {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    if path.exists() {
        std::fs::remove_dir_all(&path).expect("the previous run's directory is removable");
    }
    std::fs::create_dir_all(&path).expect("a writable scratch directory");
    path
}

/// The run directory for task `T-1`, run 1, below `root`.
fn run_directory(root: &Path) -> RunDirectory {
    RunDirectory::at(root.join("runs").join("T-1").join("1"))
}

/// Writes one planning document into a store.
fn write_story(store_root: &Path, name: &str, body: &str) {
    let directory = store_root.join("story");
    std::fs::create_dir_all(&directory).expect("a writable store");
    std::fs::write(directory.join(format!("{name}.md")), body).expect("a writable document");
}

/// A valid planning document for `story:<name>`.
fn story(name: &str) -> String {
    format!(
        "---\nformat: aep.planning-md/1\nid: story:{name}\nkind: story\nstatus: draft\ntitle: \
         {name}\n---\n\n# {name}\n"
    )
}

/// How many stories a fact source says the store holds, for the F7 assertions.
fn story_count(facts: &dyn FactSource) -> Option<f64> {
    match facts.fact(&FactPath::new("artifact.story.count").expect("a fact path")) {
        Some(FactValue::Number(number)) => Some(number.get()),
        _ => None,
    }
}

// ---------------------------------------------------------------------------------------------
// The tests
// ---------------------------------------------------------------------------------------------

#[test]
fn a_command_step_that_produced_no_verdict_submits_nothing_and_changes_nothing() {
    let root = scratch("no-verdict");
    let engine = engine();
    let store = MarkdownStore::open(root.join("planning"));
    let map = map(CRASHING_MAP);
    let run = run_directory(&root);
    let mut fake = Fake::new(&[
        Act::Crash("no such executable"),
        Act::Crash("still no such"),
    ]);

    let before = engine.evaluate(
        &engine
            .initialize_with_artifacts(task(), ArtifactGraph::new())
            .expect("a fresh execution"),
    );

    let report = drive(
        &engine,
        &task(),
        &store,
        &map,
        &run,
        &mut fake,
        &DriverOptions::default(),
    )
    .expect("the run stops with a report rather than an error");

    assert_eq!(
        report.evidence_submitted, 0,
        "the engine has no `Unknown` to submit: a crashed `cargo test` is not `tests.unit.failed \
         > 0`, and submitting one would fabricate an observation"
    );
    assert_eq!(report.steps_run, 2, "one attempt and one retry");

    let restored = engine
        .restore(
            task(),
            ArtifactGraph::new(),
            run.read_snapshot().expect("a snapshot was persisted"),
        )
        .expect("the snapshot restores");
    assert!(
        restored.recorded_evidence().is_empty(),
        "nothing was observed, so the evidence log is empty"
    );
    assert_eq!(
        engine.evaluate(&restored),
        before,
        "D5's other half: a step that observed nothing leaves the evaluation exactly as it was"
    );
}

#[test]
fn a_step_that_spends_its_retry_budget_leaves_a_resumable_run_and_the_engines_own_reasons() {
    let root = scratch("budget");
    let engine = engine();
    let store = MarkdownStore::open(root.join("planning"));
    let map = map(CRASHING_MAP);
    let run = run_directory(&root);
    let mut fake = Fake::new(&[Act::Crash("killed"), Act::Crash("killed again")]);

    let report = drive(
        &engine,
        &task(),
        &store,
        &map,
        &run,
        &mut fake,
        &DriverOptions::default(),
    )
    .expect("a spent budget is a report, not an error");

    assert_eq!(report.status(), RunStatus::BudgetExhausted);
    assert!(
        report.status().is_resumable(),
        "the run directory is where a fixed environment gets picked up again"
    );
    assert!(run.cursor_path().exists(), "the cursor is on disk");
    assert!(run.snapshot_path().exists(), "the snapshot is beside it");
    assert_eq!(
        report
            .cursor
            .attempts_at(&"implement".parse().expect("a state id"), 0),
        2,
        "one attempt plus one retry, spent and recorded"
    );
    assert!(
        report
            .notes
            .iter()
            .any(|note| note.contains("retry budget of 1")),
        "exactly one driver line naming the budget and the step: {:?}",
        report.notes
    );

    // Verbatim means verbatim: the same strings `transition()` itself would return.
    let mut restored = engine
        .restore(
            task(),
            ArtifactGraph::new(),
            run.read_snapshot().expect("a snapshot"),
        )
        .expect("the snapshot restores");
    let TransitionResult::Blocked { reasons, .. } = engine
        .transition(&mut restored)
        .expect("the engine answers")
    else {
        panic!("with no evidence at all the guard is Unknown, so nothing may move");
    };
    assert_eq!(
        report.reasons, reasons,
        "the driver prints the engine's reasons and does not summarise them"
    );
    assert!(
        report.explanation.is_some(),
        "and `CompletionExplanation` travels with them"
    );
}

#[test]
fn a_retried_step_that_then_succeeded_keeps_its_first_attempt_in_the_cursor() {
    let root = scratch("retry-then-green");
    let engine = engine();
    let store = MarkdownStore::open(root.join("planning"));
    let map = map(MAP);
    let run = run_directory(&root);
    let mut fake = Fake::new(&[
        Act::Done,
        Act::Crash("the process died"),
        Act::Diff,
        Act::Tests { failed: 0 },
    ]);

    let report = drive(
        &engine,
        &task(),
        &store,
        &map,
        &run,
        &mut fake,
        &DriverOptions::default(),
    )
    .expect("the run finishes");

    assert_eq!(report.status(), RunStatus::Completed);
    assert_eq!(
        report
            .cursor
            .attempts_at(&"implement".parse().expect("a state id"), 1),
        2,
        "there is no evidence to erase — the failed attempt produced none — but the count stays, \
         so `green on the second try` is in the record"
    );
    assert!(
        report
            .notes
            .iter()
            .any(|note| note.contains("the process died")),
        "and the reason it failed is in the report: {:?}",
        report.notes
    );
    assert_eq!(report.evidence_submitted, 2);
    assert_eq!(
        fake.asked[1].attempt, 1,
        "the first attempt at a step is attempt 1"
    );
    assert_eq!(
        fake.asked[2].attempt, 2,
        "and the retry is told it is the second, so its transcript does not overwrite the first"
    );
}

#[test]
fn a_run_over_two_states_advances_and_the_report_names_both_moves() {
    let root = scratch("two-states");
    let engine = engine();
    let store = MarkdownStore::open(root.join("planning"));
    let map = map(MAP);
    let run = run_directory(&root);
    let mut fake = Fake::new(&[Act::Done, Act::Diff, Act::Tests { failed: 0 }]);

    let report = drive(
        &engine,
        &task(),
        &store,
        &map,
        &run,
        &mut fake,
        &DriverOptions::default(),
    )
    .expect("the run finishes");

    let moves: Vec<(String, String)> = report
        .transitions
        .iter()
        .map(|(from, to)| (from.to_string(), to.to_string()))
        .collect();
    assert_eq!(
        moves,
        vec![
            ("implement".to_owned(), "verify".to_owned()),
            ("verify".to_owned(), "complete".to_owned()),
        ],
        "order between states is the workflow's, and the driver never overrides it"
    );
    assert_eq!(report.status(), RunStatus::Completed);
    assert_eq!(
        report.cursor.visits_of(&"verify".parse().expect("a state")),
        1
    );
    assert_eq!(
        fake.asked
            .iter()
            .map(|asked| asked.state.as_str())
            .collect::<Vec<_>>(),
        vec!["implement", "implement", "verify"],
        "each state's steps ran in the state they belong to"
    );
    assert_eq!(
        fake.asked[0].requirements, 1,
        "the requirement lines in force travel with the step — one line per requirement, which is          the guide's rule and the reason an explanation is not a summary"
    );
    assert_eq!(
        fake.asked[0].tools, 3,
        "the tool set is `tool_config` over the effective policy: the three the profile allows"
    );
    assert_eq!(
        fake.asked[0].reaching,
        vec!["-> verify: guard: diff.exists".to_owned()],
        "the guard on the way out travels with the step, labelled by where it goes: the step is in \
         `implement`, `diff.exists` does not hold yet, and nothing in `requirements` says so"
    );
    assert_eq!(
        fake.asked[2].reaching,
        vec![
            "-> complete: guard: tests.unit.failed == 0".to_owned(),
            "-> implement: guard: tests.unit.failed > 0".to_owned(),
            "-> implement: ? artifact.story.exists — unobserved: artifact.story.exists \
             [state implement]"
                .to_owned(),
        ],
        "every way out is named, back-edge included, and what the *target* state requires on entry \
         comes with it: the suite has not run at this point, so neither guard holds"
    );
}

#[test]
fn a_store_with_one_unparseable_file_stops_the_run_with_its_fact_base_unchanged() {
    let root = scratch("broken-store");
    let planning = root.join("planning");
    let engine = engine();
    let store = MarkdownStore::open(&planning);
    let map = map(MAP);
    let run = run_directory(&root);

    write_story(&planning, "one", &story("one"));
    write_story(&planning, "two", &story("two"));
    let clean = store.load().graph().expect("a clean store builds a graph");
    assert_eq!(story_count(&clean.facts()), Some(2.0));

    // A first run that stops on its own, so there is a snapshot to be unchanged.
    let mut fake = Fake::new(&[Act::Done]);
    let first = drive(
        &engine,
        &task(),
        &store,
        &map,
        &run,
        &mut fake,
        &DriverOptions {
            max_iterations: 1,
            ..DriverOptions::default()
        },
    )
    .expect("the first run reports");
    assert_eq!(first.steps_run, 1);
    let before = std::fs::read(run.snapshot_path()).expect("a snapshot on disk");

    // Now one file stops parsing. `graph()` alone would wave this straight through.
    write_story(&planning, "two", "---\nid: story:two\n  kind: [oops\n---\n");
    let report = store.load();
    assert!(!report.is_clean(), "the store knows one file failed");
    let shrunk = report
        .graph()
        .expect("`graph()` returns Ok for a store that lost a document — that is the hazard");
    assert_eq!(
        story_count(&shrunk.facts()),
        Some(1.0),
        "a `graph()`-only check would hand the engine a fact base that shrank because of a typo"
    );

    let mut fake = Fake::new(&[]);
    let stopped = drive(
        &engine,
        &task(),
        &store,
        &map,
        &run,
        &mut fake,
        &DriverOptions::default(),
    )
    .expect("a broken store is a report, not an error, once a run exists");

    assert_eq!(
        stopped.status(),
        RunStatus::StoreBroken,
        "not `Blocked`: `Blocked` is the engine's word for *the protocol says no*, and a typo is \
         not that"
    );
    assert_eq!(
        stopped.steps_run, 0,
        "nothing ran against the shrunken store"
    );
    assert!(
        stopped
            .reasons
            .iter()
            .any(|reason| reason.contains("two.md")),
        "the store's own failures, verbatim, with the path on them: {:?}",
        stopped.reasons
    );
    assert_eq!(
        std::fs::read(run.snapshot_path()).expect("a snapshot on disk"),
        before,
        "the run's own record is unchanged"
    );

    let restored = engine
        .restore(task(), clean, run.read_snapshot().expect("a snapshot"))
        .expect("the snapshot restores");
    assert_eq!(
        story_count(restored.fact_store()),
        Some(2.0),
        "the fact base the run was evaluating against never shrank, which is the whole of F7"
    );
}

#[test]
fn a_resume_refuses_every_moved_pin_on_a_snapshot_the_engine_would_have_accepted() {
    let root = scratch("moved-pins");
    let engine = engine();
    let store = MarkdownStore::open(root.join("planning"));
    let map = map(MAP);
    let run = run_directory(&root);

    let mut fake = Fake::new(&[Act::Done]);
    drive(
        &engine,
        &task(),
        &store,
        &map,
        &run,
        &mut fake,
        &DriverOptions {
            max_iterations: 1,
            ..DriverOptions::default()
        },
    )
    .expect("a run to resume");

    let snapshot: Snapshot = run.read_snapshot().expect("a snapshot");
    let cursor = run.read_cursor().expect("a cursor");

    for (what, moved) in [
        ("workflow", {
            let mut moved = cursor.clone();
            moved.workflow = "test/linear/2".to_owned();
            moved
        }),
        ("map digest", {
            let mut moved = cursor.clone();
            moved.map_digest = "sha256:something-else".to_owned();
            moved
        }),
        ("engine version", {
            let mut moved = cursor.clone();
            moved.engine_version = "0.0.1".to_owned();
            moved
        }),
    ] {
        run.persist(&snapshot, &moved)
            .expect("the cursor is written");
        let mut fake = Fake::new(&[]);
        let refusal = resume(
            &engine,
            &task(),
            &store,
            &map,
            &run,
            &mut fake,
            &DriverOptions::default(),
        )
        .expect_err("a moved pin is refused");
        let DriveError::Refused(message) = refusal else {
            panic!("a moved {what} is a refusal, not another kind of failure: {refusal:?}");
        };
        assert!(
            message.contains("--restart"),
            "the refusal names both routes out: {message}"
        );

        // The point of the whole mechanism: the engine would have taken this snapshot happily.
        // `Execution::restore` checks the task and that the state name still exists — it carries no
        // workflow id, no version and no engine version — so without the cursor a workflow that
        // renamed nothing and rewrote every guard would silently re-govern the run.
        engine
            .restore(task(), ArtifactGraph::new(), snapshot.clone())
            .unwrap_or_else(|error| {
                panic!("the {what} check is only load-bearing if `restore` accepts this: {error}")
            });
    }
}

/// A resumed run does not ask the person again, and that is what makes a pause a pause.
///
/// The design says a paused run "resumes" (§ 4.6, wave 3's test list), and the shipped map's review
/// step tells the person to "record your review as evidence and resume it". A cursor left pointing
/// at the step that paused re-presents the same question on every resume, so the run would stop at
/// the same person forever and no `operator` step before the last state could ever be passed — the
/// approval of a specification, in particular, would wedge the run three states before the one it
/// gates. Whether the person actually did what was asked is decided by the guard on the way out,
/// which is the only thing that can decide it.
#[test]
fn a_resumed_run_does_not_ask_the_person_again_and_carries_on_from_the_step_after() {
    let with_a_person = r"
format: aep.driver-steps/1
id: test/asked-once
workflow: test/linear/1
states:
  implement:
    steps:
      - kind: operator
        prompt: approve the specification before implementation begins
      - kind: command
        run: [git, diff]
        evidence:
          kind: diff
          verifier: compiler
";
    let root = scratch("asked-once");
    let engine = engine();
    let store = MarkdownStore::open(root.join("planning"));
    let map = map(with_a_person);
    let run = RunDirectory::at(root.join("runs").join("T-1").join("1"));
    let options = DriverOptions {
        pause_on_approval: true,
        ..DriverOptions::default()
    };

    let mut fake = Fake::new(&[Act::Pause("a specification is owed an approval")]);
    let paused = drive(&engine, &task(), &store, &map, &run, &mut fake, &options)
        .expect("a pause is a report");
    assert_eq!(paused.status(), RunStatus::AwaitingOperator);
    assert_eq!(
        paused.cursor.step, 1,
        "the pause is the step's completion, so the cursor is past it and the resume has \
         somewhere to go"
    );

    let mut fake = Fake::new(&[Act::Diff]);
    let resumed =
        resume(&engine, &task(), &store, &map, &run, &mut fake, &options).expect("the run resumes");
    assert_eq!(
        fake.asked
            .iter()
            .map(|asked| asked.index)
            .collect::<Vec<_>>(),
        vec![1],
        "the resumed run ran the step *after* the pause and asked nobody anything a second time"
    );
    assert_eq!(
        resumed
            .transitions
            .iter()
            .map(|(from, to)| (from.to_string(), to.to_string()))
            .collect::<Vec<_>>(),
        vec![("implement".to_owned(), "verify".to_owned())],
        "and the evidence the step after the pause produced is what moved the run"
    );
}

#[test]
fn an_operator_step_pauses_a_run_that_may_pause_and_is_refused_by_one_that_may_not() {
    let paused_map = r"
format: aep.driver-steps/1
id: test/operator
workflow: test/linear/1
states:
  implement:
    steps:
      - kind: operator
        prompt: is this change worth shipping?
";
    let root = scratch("operator");
    let engine = engine();
    let store = MarkdownStore::open(root.join("planning"));
    let map = map(paused_map);

    let waiting = RunDirectory::at(root.join("runs").join("T-1").join("1"));
    let mut fake = Fake::new(&[Act::Pause("a person is owed a question")]);
    let report = drive(
        &engine,
        &task(),
        &store,
        &map,
        &waiting,
        &mut fake,
        &DriverOptions {
            pause_on_approval: true,
            ..DriverOptions::default()
        },
    )
    .expect("a pause is a report");
    assert_eq!(report.status(), RunStatus::AwaitingOperator);
    assert!(
        report.status().is_resumable(),
        "the snapshot is already a queue that survives a reboot; there is no waiting process"
    );

    let refused = RunDirectory::at(root.join("runs").join("T-1").join("2"));
    let mut fake = Fake::new(&[]);
    let error = drive(
        &engine,
        &task(),
        &store,
        &map,
        &refused,
        &mut fake,
        &DriverOptions::default(),
    )
    .expect_err("a headless run has nobody to answer an operator step");
    assert!(
        matches!(&error, DriveError::Refused(message) if message.contains("--pause-on-approval")),
        "the refusal names the flag that would have allowed it: {error}"
    );
}

#[test]
fn a_run_directory_that_is_not_shaped_like_a_run_id_is_refused_rather_than_guessed_at() {
    let root = scratch("bad-directory");
    let engine = engine();
    let store = MarkdownStore::open(root.join("planning"));
    let map = map(MAP);
    let run = RunDirectory::at(root.join("runs").join("T-1").join("latest"));
    let mut fake = Fake::new(&[]);

    let error = drive(
        &engine,
        &task(),
        &store,
        &map,
        &run,
        &mut fake,
        &DriverOptions::default(),
    )
    .expect_err("`latest` is not a run number");
    assert!(
        matches!(&error, DriveError::Refused(message) if message.contains("<task>/<n>")),
        "inventing a run id would produce a record nobody can join back up: {error}"
    );
}

#[test]
fn a_failing_suite_is_routed_by_the_engine_and_the_visit_budget_ends_the_cycle() {
    let root = scratch("back-edge");
    let planning = root.join("planning");
    // `implement.requires` gates entry *into* implement, so the back-edge needs the story the state
    // asks for. Without it the run is `Blocked` rather than cycling, which is the engine being
    // right about a different thing.
    write_story(&planning, "one", &story("one"));
    let engine = engine();
    let store = MarkdownStore::open(&planning);
    let map = map(MAP);
    let run = run_directory(&root);
    // Three times round: write, observe a diff, run a suite that fails. The driver never decides
    // that a red suite means *go back* — it submits the failing record and the workflow routes.
    let round = [Act::Done, Act::Diff, Act::Tests { failed: 2 }];
    let script: Vec<Act> = round.iter().chain(&round).chain(&round).copied().collect();
    let mut fake = Fake::new(&script);

    let report = drive(
        &engine,
        &task(),
        &store,
        &map,
        &run,
        &mut fake,
        &DriverOptions::default(),
    )
    .expect("a spent visit budget is a report");

    assert_eq!(
        report
            .transitions
            .iter()
            .filter(|(from, to)| from.as_str() == "verify" && to.as_str() == "implement")
            .count(),
        3,
        "the engine took the back-edge on every failing suite; a driver that collapsed the failure into its own routing \
         would be a second protocol implementation: {:?}",
        report.transitions
    );
    assert_eq!(
        report.evidence_submitted, 6,
        "every failing suite was submitted — `False` is an observation, and the record of it is \
         what the back-edge guard reads"
    );
    assert_eq!(report.status(), RunStatus::BudgetExhausted);
    assert!(
        report
            .notes
            .iter()
            .any(|note| note.contains("`implement`") && note.contains("visit budget is 3")),
        "the run stops naming the state it was cycling in, rather than burning a token budget in \
         silence: {:?}",
        report.notes
    );
    assert_eq!(
        report
            .cursor
            .visits_of(&"implement".parse().expect("a state")),
        4,
        "the fourth entry is the one the budget of three refuses"
    );
    assert!(
        fake.script.is_empty(),
        "the run stopped on the visit budget, not because the script ran out"
    );
}
