//! `protocol drive` integration tests.
//!
//! These drive the real binary against a real directory, because that is what the verb family is: a
//! run is a lock file, a run directory, a program that was spawned and a snapshot on disk. A test
//! that called the library would not catch a lock taken after the run id was allocated, a flag that
//! never reaches the driver, or a report that summarised the engine instead of quoting it.
//!
//! The document tree is **this repository's own** — `protocols/`, `workflows/`, `profiles/`,
//! `principles/`, `artifacts/lifecycles/` — and the step map is a fixture, never
//! `drivers/development/default.yaml`. That map's command steps run `cargo test --workspace`, and a
//! test that ran it would be a test that ran itself.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// The repository root.
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the workspace root exists")
}

/// Runs `protocol` with `args` from the repository root.
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

/// A path as an argument.
fn printable(path: &Path) -> &str {
    path.to_str().expect("a printable path")
}

/// Writes a fixture file, creating the directories above it.
fn write(path: &Path, contents: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("the temporary tree is writable");
    }
    std::fs::write(path, contents).expect("the fixture is writable");
}

/// This machine's name, read the way the driver reads it.
fn host() -> String {
    for path in ["/proc/sys/kernel/hostname", "/etc/hostname"] {
        if let Ok(name) = std::fs::read_to_string(path) {
            let name = name.trim();
            if !name.is_empty() {
                return name.to_owned();
            }
        }
    }
    std::env::var("HOSTNAME").unwrap_or_else(|_| "unknown-host".to_owned())
}

/// A project with a planning store, a task and a step map, built from scratch.
struct Fixture {
    directory: PathBuf,
}

impl Fixture {
    /// Builds the fixture. `operator` puts an `operator` step at the head of `verify`.
    fn new(name: &str, operator: bool) -> Self {
        let directory = std::env::temp_dir().join(format!("protocol-drive-{name}"));
        std::fs::remove_dir_all(&directory).ok();
        std::fs::create_dir_all(&directory).expect("the temporary tree is writable");

        write(
            &directory.join(".engineering/planning/specification/passkeys.md"),
            "---\nformat: aep.planning-md/1\nid: specification:passkeys\nkind: specification\n\
             status: approved\ntitle: Passkey sign-in\nsummary: What signing in with a passkey \
             must do.\n---\n# Specification\n\nThe assertion is verified against the stored \
             public key.\n",
        );
        write(
            &directory.join("task.yaml"),
            "id: DRIVE-1\nkind: feature\nobjective: drive-a-workflow\nprotocol: adp/1\n\
             profile: development.standard\n",
        );
        write(&directory.join("steps.yaml"), &step_map(operator));

        Self { directory }
    }

    /// The arguments every verb needs.
    fn location(&self) -> Vec<String> {
        vec![
            "--project".to_owned(),
            printable(&self.directory).to_owned(),
            "--root".to_owned(),
            printable(&root()).to_owned(),
            "--task".to_owned(),
            printable(&self.directory.join("task.yaml")).to_owned(),
            "--map".to_owned(),
            printable(&self.directory.join("steps.yaml")).to_owned(),
        ]
    }

    /// Runs one `protocol drive` verb against this fixture.
    ///
    /// A `run` carries `--allow-evidence-gap`, and that is a statement about the fixture rather
    /// than about the flag. This map declares `test_result`, `diff` and `static_analysis` and
    /// nothing else, so F-W4.2-4's launch check refuses it: `spec-driven` wants a `specification`
    /// record and `provenance-tracking` an independent `verification` one, and no step here mints
    /// either. Every test below is about the routing loop, the lock or the report, none of which
    /// that gap changes — so the tests say *I know* rather than growing two steps that write
    /// evidence documents nobody reads. The refusal itself is tested on its own, without the flag,
    /// in `a_map_that_cannot_produce_demanded_evidence_is_refused_before_the_first_step`.
    fn drive(&self, verb: &[&str], extra: &[&str]) -> Output {
        let mut args: Vec<String> = vec!["drive".to_owned()];
        args.extend(verb.iter().map(ToString::to_string));
        args.extend(self.location());
        if verb == ["run"] {
            args.push("--allow-evidence-gap".to_owned());
        }
        args.extend(extra.iter().map(ToString::to_string));
        let borrowed: Vec<&str> = args.iter().map(String::as_str).collect();
        protocol(&borrowed)
    }

    /// The `.engineering/runs` directory.
    fn runs(&self) -> PathBuf {
        self.directory.join(".engineering/runs")
    }

    /// The cursor of one run.
    fn cursor(&self, run: &str) -> serde_json::Value {
        let (task, ordinal) = run.rsplit_once('/').expect("a run id");
        let path = self.runs().join(task).join(ordinal).join("cursor.json");
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("reading {}: {error}", path.display()));
        serde_json::from_str(&text).expect("the cursor is JSON")
    }
}

/// The fixture step map: the same states as `adp/default`, with commands that cost nothing.
///
/// `sh -c "exit 0"` and `sh -c "exit 1"` are the two verdicts a command step can carry, which is
/// all an exit status has to say. The red run in `establish_verifiers` is deliberate: `test.exists`
/// is what the guard reads, and a test that passed before there was an implementation would be a
/// test of nothing.
///
/// `operator` puts an `operator` step at the head of `verify` — a state this run genuinely reaches,
/// so the pause is exercised rather than only the pre-flight refusal that names it.
fn step_map(operator: bool) -> String {
    let pause = if operator {
        "      - kind: operator\n        prompt: judge this change before the suites run\n"
    } else {
        ""
    };
    format!(
        "format: aep.driver-steps/1\n\
         id: fixture/drive\n\
         workflow: adp/default/1\n\
         states:\n\
        \x20 establish_verifiers:\n\
        \x20   steps:\n\
        \x20     - kind: command\n\
        \x20       description: the red suite\n\
        \x20       run: [sh, -c, \"exit 1\"]\n\
        \x20       evidence:\n\
        \x20         kind: test_result\n\
        \x20         suite: unit\n\
        \x20         verifier: test-runner\n\
        \x20 implement:\n\
        \x20   steps:\n\
        \x20     - kind: command\n\
        \x20       description: the working tree changed\n\
        \x20       run: [sh, -c, \"exit 0\"]\n\
        \x20       evidence:\n\
        \x20         kind: diff\n\
        \x20         verifier: git\n\
        \x20 verify:\n\
        \x20   steps:\n\
         {pause}\
        \x20     - kind: command\n\
        \x20       run: [sh, -c, \"exit 0\"]\n\
        \x20       evidence:\n\
        \x20         kind: test_result\n\
        \x20         suite: unit\n\
        \x20         verifier: test-runner\n\
        \x20     - kind: command\n\
        \x20       run: [sh, -c, \"exit 0\"]\n\
        \x20       evidence:\n\
        \x20         kind: test_result\n\
        \x20         suite: contract\n\
        \x20         verifier: test-runner\n\
        \x20     - kind: command\n\
        \x20       run: [sh, -c, \"exit 0\"]\n\
        \x20       evidence:\n\
        \x20         kind: static_analysis\n\
        \x20         verifier: static-analyzer\n"
    )
}

#[test]
fn every_verb_can_be_asked_for_help() {
    for verb in ["run", "status", "resume"] {
        let output = protocol(&["drive", verb, "--help"]);
        assert_eq!(code(&output), 0, "{}", stderr(&output));
        assert!(
            stdout(&output).contains("--project") || stdout(&output).contains("Usage"),
            "`drive {verb} --help` says nothing useful"
        );
    }
}

#[test]
fn a_run_advances_on_command_step_evidence_and_ends_with_the_engine_speaking() {
    let fixture = Fixture::new("advance", false);
    let output = fixture.drive(&["run"], &[]);

    let text = stdout(&output);
    // Two moves need no evidence — the workflow's first transitions are unguarded or read the
    // store — and the two after them are bought by command steps. The run must reach at least the
    // fourth.
    for movement in [
        "receive -> specify",
        "specify -> decompose",
        "decompose -> establish_verifiers",
        "establish_verifiers -> implement",
    ] {
        assert!(text.contains(movement), "no `{movement}` in:\n{text}");
    }
    assert!(
        text.contains("status     blocked") || text.contains("status     completed"),
        "a run ends by saying which of the two it is:\n{text}"
    );

    // The lock is released on every exit path the driver controls.
    assert!(
        !fixture.runs().join("lock.json").exists(),
        "the lock outlived the run that took it"
    );
    assert_eq!(
        std::fs::read_to_string(fixture.runs().join("current"))
            .expect("a current pointer")
            .trim(),
        "DRIVE-1/1"
    );
}

#[test]
fn a_blocked_run_prints_the_engine_reasons_without_rewording_them() {
    let fixture = Fixture::new("blocked", false);
    let output = fixture.drive(&["run"], &[]);
    let text = stdout(&output);
    assert_eq!(code(&output), 1, "a blocked run is the execution saying no");
    assert!(text.contains("blocked because:"), "{text}");

    // Verbatim across two surfaces: what the cursor recorded is what was printed, character for
    // character. A report that paraphrased a refusal would be a second, worse protocol.
    let cursor = fixture.cursor("DRIVE-1/1");
    let reasons = cursor["reasons"].as_array().expect("recorded reasons");
    assert!(!reasons.is_empty(), "a blocked run records why: {cursor}");
    for reason in reasons {
        let reason = reason.as_str().expect("a reason is a line");
        assert!(
            text.contains(reason),
            "the report reworded `{reason}`:\n{text}"
        );
    }
    assert_eq!(cursor["status"], "blocked");
}

#[test]
fn a_second_driver_is_refused_by_name_and_writes_nothing() {
    let fixture = Fixture::new("locked", false);
    std::fs::create_dir_all(fixture.runs()).expect("the runs directory is writable");
    // A live pid: this test process. The rule is liveness, never age — any age threshold has to
    // exceed the longest legitimate step, and the longest legitimate step is a person.
    write(
        &fixture.runs().join("lock.json"),
        &format!(
            "{{\"run\":\"DRIVE-1/7\",\"pid\":{},\"host\":\"{}\",\"driver\":\"the test\"}}\n",
            std::process::id(),
            host()
        ),
    );

    let output = fixture.drive(&["run"], &[]);
    let said = format!("{}{}", stdout(&output), stderr(&output));
    assert_eq!(code(&output), 1);
    assert!(
        said.contains("DRIVE-1/7"),
        "the holder's run is not named:\n{said}"
    );
    assert!(
        said.contains(&std::process::id().to_string()),
        "the holder's pid is not named:\n{said}"
    );
    assert!(
        said.contains("--take-lock"),
        "a refusal names the routes out:\n{said}"
    );
    assert!(
        !fixture.runs().join("DRIVE-1").exists(),
        "a refused run allocated a directory anyway"
    );
}

#[test]
fn a_run_stopped_by_its_iteration_bound_resumes_where_it_stopped() {
    let fixture = Fixture::new("resume", false);
    let first = fixture.drive(&["run"], &["--max-iterations", "2"]);
    let opening = stdout(&first);
    assert!(opening.contains("run        DRIVE-1/1"), "{opening}");
    let before = fixture.cursor("DRIVE-1/1");
    assert!(
        !fixture.runs().join("lock.json").exists(),
        "a stopped run keeps the lock"
    );

    let second = fixture.drive(&["resume", "DRIVE-1/1"], &[]);
    let text = stdout(&second);
    assert!(
        text.contains("run        DRIVE-1/1"),
        "a resume continues the same run rather than allocating a new one:\n{text}"
    );
    let after = fixture.cursor("DRIVE-1/1");
    assert!(
        after["iterations"].as_u64().unwrap_or(0) > before["iterations"].as_u64().unwrap_or(0),
        "the resumed run did nothing: {before} then {after}"
    );
    assert_eq!(
        after["map_digest"], before["map_digest"],
        "a resume is pinned to the map it started under"
    );
    assert!(
        !fixture.runs().join("DRIVE-1/2").exists(),
        "a resume allocated a second run directory"
    );
}

#[test]
fn a_headless_start_refuses_what_only_a_person_can_answer_and_the_flag_is_the_route_through() {
    let fixture = Fixture::new("operator", true);

    let refused = fixture.drive(&["run"], &[]);
    let said = stdout(&refused);
    assert_eq!(code(&refused), 1);
    assert!(
        said.contains("operator step"),
        "the refusal names what is owed:\n{said}"
    );
    assert!(
        said.contains("--pause-on-approval"),
        "the refusal names the route through:\n{said}"
    );
    assert!(
        !fixture.runs().join("lock.json").exists(),
        "a refused start left a lock behind"
    );

    let paused = fixture.drive(&["run"], &["--pause-on-approval"]);
    let text = stdout(&paused);
    assert_eq!(
        code(&paused),
        0,
        "with the flag, a green exit means finished or waiting:\n{text}"
    );
    assert!(
        text.contains("resume with: protocol drive resume DRIVE-1/1"),
        "a pause ends with the one word that continues it:\n{text}"
    );
}

#[test]
fn status_reports_the_run_and_whether_the_lock_is_free() {
    let fixture = Fixture::new("status", false);
    fixture.drive(&["run"], &[]);
    let output = fixture.drive(&["status"], &[]);
    let text = stdout(&output);
    assert_eq!(code(&output), 0, "{}", stderr(&output));
    assert!(text.contains("lock       free"), "{text}");
    assert!(text.contains("run        DRIVE-1/1"), "{text}");
    assert!(text.contains("map        fixture/drive"), "{text}");
}

#[test]
fn the_committed_step_map_loads_and_is_refused_when_a_state_is_renamed() {
    // The real map, cross-validated against the real workflow by the document loader.
    let output = protocol(&["validate", "--root", "."]);
    assert_eq!(code(&output), 0, "{}", stderr(&output));
    assert!(
        stdout(&output).contains("2 step map(s)"),
        "both shipped maps load and cross-validate: `development/default` is the cargo one and \
         `development/checks` runs the checks a story carries\n{}",
        stdout(&output)
    );

    // And the negative: a map naming a state the workflow does not have is refused at load, with
    // the workflow's own states listed. A tree is built from scratch so the repository's own
    // `drivers/` is not what is being read.
    let tree = std::env::temp_dir().join("protocol-drive-tree");
    std::fs::remove_dir_all(&tree).ok();
    for directory in ["protocols", "workflows"] {
        std::fs::create_dir_all(tree.join(directory)).expect("the temporary tree is writable");
    }
    std::fs::copy(
        root().join("protocols/adp/1.yaml"),
        tree.join("protocols/adp.yaml"),
    )
    .expect("the protocol is readable");
    std::fs::copy(
        root().join("protocols/aep/1.yaml"),
        tree.join("protocols/aep.yaml"),
    )
    .expect("the protocol is readable");
    std::fs::copy(
        root().join("workflows/development/default.yaml"),
        tree.join("workflows/default.yaml"),
    )
    .expect("the workflow is readable");
    write(
        &tree.join("drivers/broken.yaml"),
        "format: aep.driver-steps/1\nid: broken/map\nworkflow: adp/default/1\n\
         states:\n  polishing:\n    steps: []\n",
    );

    let output = protocol(&["validate", "--root", printable(&tree)]);
    let text = stdout(&output);
    assert_eq!(code(&output), 1, "{text}");
    assert!(text.contains("unknown_state"), "{text}");
    assert!(text.contains("polishing"), "{text}");
    assert!(
        text.contains("implement"),
        "the refusal lists the states the workflow does declare:\n{text}"
    );
}

/// Two maps fit `adp/default/1`, and the driver refuses to pick one on the caller's behalf.
///
/// This changed when `development/checks` shipped: before it, a run with no `--map` was given the
/// only map that fitted, and the wave-4 run `W4-1/1` was started that way. The refusal is the
/// wanted outcome rather than a regression — which map a run is under decides how its evidence is
/// obtained, and guessing that is the one thing the driver does not do — but it is a change to what
/// a bare `protocol drive run` does, so it is asserted rather than left to be discovered.
#[test]
fn two_maps_fit_the_workflow_so_the_driver_refuses_to_choose_and_names_both() {
    let fixture = Fixture::new("two-maps", false);
    let output = protocol(&[
        "drive",
        "run",
        "--project",
        printable(&fixture.directory),
        "--root",
        printable(&root()),
        "--task",
        printable(&fixture.directory.join("task.yaml")),
    ]);
    let said = format!("{}{}", stdout(&output), stderr(&output));
    assert_eq!(code(&output), 1, "{said}");
    for named in ["development/default", "development/checks", "--map"] {
        assert!(
            said.contains(named),
            "the refusal names both maps and the flag that chooses one; `{named}` is missing:\n{said}"
        );
    }
    assert!(
        !fixture.runs().join("lock.json").exists(),
        "a refused start left a lock behind"
    );
}

/// **F-W4.2-4** at the surface an operator meets: the map is checked against the plan it will
/// drive, before anything runs.
///
/// `W4-2/1` learned this at a guard six states in, having spent ten model sessions, 76 minutes and
/// $31.46. Everything the refusal below prints was in two documents on disk at launch.
#[test]
fn a_map_that_cannot_produce_demanded_evidence_is_refused_before_the_first_step() {
    let fixture = Fixture::new("evidence-gap", false);
    let mut args: Vec<String> = vec!["drive".to_owned(), "run".to_owned()];
    args.extend(fixture.location());
    let borrowed: Vec<&str> = args.iter().map(String::as_str).collect();
    let output = protocol(&borrowed);
    let said = format!("{}{}", stdout(&output), stderr(&output));

    assert_eq!(code(&output), 1, "{said}");
    for named in [
        "specification",
        "spec-driven",
        "verification",
        "provenance-tracking",
        "adversarial_verify -> review",
        "--allow-evidence-gap",
    ] {
        assert!(
            said.contains(named),
            "the refusal names the kind, who asked, what it blocks and the way through; `{named}` \
             is missing:\n{said}"
        );
    }
    assert!(
        !fixture.runs().join("lock.json").exists(),
        "a refused start left a lock behind"
    );
    assert!(
        !fixture.runs().join("DRIVE-1").exists(),
        "a refused start allocated a run directory, so the refusal was not before everything"
    );

    // And the way through, which is what makes this a refusal rather than a wall: the same command
    // with the flag starts, prints the same gap, and gets as far as the run would have got anyway.
    let allowed = fixture.drive(&["run"], &[]);
    let told = stdout(&allowed);
    assert!(
        told.contains("--allow-evidence-gap` was given") && told.contains("specification"),
        "the flag acknowledges the gap rather than hiding it:\n{told}"
    );
    assert!(
        fixture.runs().join("DRIVE-1").exists(),
        "the flagged run allocated a run directory:\n{told}"
    );
}

/// The checks map plans against this repository's own task, which is the check `drive run` makes
/// before it executes anything.
///
/// Planning-level on purpose: running it would run nine shell checks, a model session and this
/// repository's own document validation. `check_run` is phase two of the map's cross-validation —
/// every evidence kind against the protocol the **task** resolves to, and the workflow pin against
/// the workflow in the tree — and it is what stands between a map that loads and a map that fails
/// halfway through a run that has already spent a budget.
#[test]
fn the_checks_map_plans_against_the_repositorys_own_task() {
    let registry = aep_engine::load::load_tree(&root()).expect("the document tree loads");
    let text = std::fs::read_to_string(root().join(".engineering/task.yaml"))
        .expect("the repository's own task document is readable");
    let task = aep_schema::parse::task(&text, Some(".engineering/task.yaml")).expect("it parses");
    let plan = aep_engine::resolve(&task, &registry).expect("it resolves");
    let id = "development/checks".parse().expect("a step map id");
    let map = registry
        .step_map(&id)
        .expect("the checks map is in the tree");

    let refusals = map.check_run(&plan.protocol, &plan.workflow);
    assert!(
        refusals.is_empty(),
        "the map is not runnable against the task the repository drives: {refusals}"
    );

    // The property the whole map exists for: the first suite a run records is the story's own
    // checks, red, in the state before implementation. `test.first_result` is the first result ever
    // recorded and never changes, so a map that ran anything else first would wedge exactly as
    // `W4-1/1` did.
    let establish = "establish_verifiers".parse().expect("a state id");
    let first_suite = map
        .steps_for(&establish)
        .iter()
        .find_map(|step| match step {
            aep_driver_spec::map::Step::Command(command) => command
                .evidence
                .as_ref()
                .filter(|mapping| mapping.kind == aep_domain::evidence::EvidenceKind::TestResult)
                .map(|_| command.run.clone()),
            _ => None,
        })
        .expect("`establish_verifiers` records a test result");
    assert_eq!(
        first_suite,
        vec!["bash".to_owned(), ".engineering/checks/run.sh".to_owned()],
        "the first suite a run under this map records is the story's own checks"
    );

    // And the trace step, which is the other thing this map has that its sibling does not: the
    // record is read from the document the checker wrote, never minted from an exit status.
    let implement = "implement".parse().expect("a state id");
    let record = map
        .steps_for(&implement)
        .iter()
        .find_map(|step| match step {
            aep_driver_spec::map::Step::Command(command) => command
                .evidence
                .as_ref()
                .and_then(|mapping| mapping.record.clone()),
            _ => None,
        })
        .expect("`implement` reads a record a verifier wrote");
    assert_eq!(record, "{run_directory}/trace-implement.yaml");
}
