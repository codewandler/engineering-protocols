//! `protocol drive` — walking a workflow by asking the engine, and doing only what the answers
//! permit.
//!
//! The third module split of `main.rs`, on the criterion the first two set: a verb family with its
//! own store — here, its own *run directory* — its own vocabulary, and no shared state with the
//! rest of the binary.
//!
//! # What is here and what is deliberately not
//!
//! The routing core is [`aep_driver`], and it is pure: it consumes an `Evaluation` and a
//! `TransitionResult` verbatim, never re-derives a verdict and never evaluates a gate. **The three
//! things that touch the world are here**, because they are the three things that cannot be in a
//! crate that claims to be deterministic:
//!
//! | this module | why it cannot be in `aep-driver` |
//! |---|---|
//! | running a program and reading its exit status | a process is the world |
//! | invoking a model | a network call, a credential and a transcript |
//! | pausing for a person | a terminal |
//! | the store lock, the pid-liveness probe and the run directory | a liveness probe reads ambient OS state and uses neither `SystemTime::now` nor `rand`, so a banned-token scan would not catch it. Placement is the only thing keeping the pure crate's claim true — review finding **F19** |
//!
//! # Exit codes
//!
//! | code | meaning |
//! |---|---|
//! | `0` | the run completed — or paused at an `operator` step **with** `--pause-on-approval`, which is what makes the flag opt-in: without it a green exit means finished, with it a green exit means finished **or** waiting, and a caller has to choose to be told that |
//! | `1` | the execution says no: blocked, a budget spent, a store that stopped parsing, a lock another run holds, a headless start that would cross a person |
//! | `2` | `clap`'s, for arguments it refuses |
//!
//! # What this driver does not do, stated rather than left to be discovered
//!
//! * **It never constructs an `Evidence::Approval` and never stamps `Producer::Human`**, under any
//!   flag. `approval_recorded` matches on subject and decision and does **not** check who granted
//!   it, so nothing below the driver would stop a harness minting its own approval: the refusal has
//!   to be the driver's, and it is a source scan in `aep-driver` rather than a promise here.
//! * **A command step's evidence carries a verdict, not counts.** An exit status says *the verifier
//!   ran and said yes or no*; it does not say how many tests passed. So a `test_result` minted here
//!   is the smallest result that carries the verdict — one passing or one failing — and a guard
//!   that reads `tests.unit.passed > 40` needs a step kind that reads a report, which this driver
//!   does not have. Named here rather than discovered later.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command as Process, ExitCode, Stdio};

use aep_backend_markdown::store::MarkdownStore;
use aep_domain::capability::Capability;
use aep_domain::evidence::{
    ChangeSet, ContractResult, Evidence, EvidenceKind, Producer, Provenance, StaticAnalysisResult,
    TestResult, TestSuite,
};
use aep_domain::ids::{StateId, TaskId, ToolRef};
use aep_domain::task::Task;
use aep_domain::time::{ObservedAt, Timestamp};
use aep_domain::verification::Verifier;
use aep_driver::executor::{
    CommandStepExecutor, LlmStepExecutor, OperatorStepExecutor, StepContext, StepOutcome,
};
use aep_driver::lock::{Liveness, LockState};
use aep_driver::run::{DriveError, DriverOptions, RunDirectory, RunReport};
use aep_driver_spec::cursor::{DriverCursor, RunId, RunStatus, StolenLock};
use aep_driver_spec::map::{CommandStep, EvidenceMapping, LlmStep, OperatorStep, Step, StepMap};
use aep_driver_spec::tool::ToolConfig;
use aep_engine::engine::EvidenceSubmission;
use aep_engine::project::project_directory;
use aep_engine::{Engine, Registry};
use anyhow::{bail, Context, Result};
use clap::{Args, Subcommand};

/// The directory inside `.engineering` that holds runs.
const RUNS_DIRECTORY: &str = "runs";

/// The one lock file per store.
///
/// **One fixed path, taken before any run id is allocated.** The reviewed first draft put the lock
/// inside `.engineering/runs/<run-id>/`, which is circular: two invocations counting the existing
/// directories at slightly different moments get `3` and `4`, and **both `create_new` succeed**,
/// because they are different paths — two live runs over one store, which is the option D6
/// explicitly rejected, reached by accident. Review finding **F2**.
const LOCK_FILE: &str = "lock.json";

/// The store-level pointer to the run that last held the lock.
const CURRENT_FILE: &str = "current";

/// The transcript directory inside a run.
const TRANSCRIPTS: &str = "transcripts";

/// The per-step context the plugin's hooks read, written into the run directory.
///
/// **The hook↔driver channel, in the direction the design left implicit.** § 4.8 decided how a
/// hook's *decision* reaches the audit trail — an append-only log the driver folds in — and said
/// nothing about how the hook learns which state it is enforcing. It cannot be told on a command
/// line: hooks are configured before the session starts and receive only the tool payload. So the
/// driver writes this document before each `llm` step and points the session at it with
/// [`STEP_CONTEXT_ENV`]; a hook that cannot find one is outside a driven run and says so by doing
/// nothing.
const STEP_CONTEXT_FILE: &str = "step-context.json";

/// The variable naming [`STEP_CONTEXT_FILE`] to the session, and through it to every hook process.
///
/// Belt and braces with the store-level `current` pointer the hooks also know how to read:
/// environment inheritance through the harness into a hook process is the one link in this chain
/// that no documentation states, so the file route exists as well and neither is trusted alone.
const STEP_CONTEXT_ENV: &str = "AEP_DRIVE_STEP_CONTEXT";

/// Where the plugin lives, when no `--plugin-dir` said.
const PLUGIN_DIR_ENV: &str = "AEP_DRIVE_PLUGIN_DIR";

/// The settings file the model invocation is pointed at.
///
/// Written even though it configures nothing yet, because the argv is what is asserted: the driver
/// **never passes `--bare`** — that flag skips hooks, and a future implementer reaching for a clean
/// reproducible environment would silently delete the driver's own enforcement arm — and it always
/// passes `--settings`. The hooks that file will carry are W3.4's; the flag's presence is W3.3's,
/// and it is a test rather than a note (review finding **F15**).
const SETTINGS_FILE: &str = "settings.json";

/// What can be done with a driven run.
#[derive(Debug, Subcommand)]
pub(crate) enum DriveCommand {
    /// Start a new run of a task, allocating a run id.
    Run(RunArgs),
    /// Report what the store's last run is doing, and who holds the lock.
    Status(StatusArgs),
    /// Continue a run that stopped, re-taking the store lock.
    Resume(ResumeArgs),
}

/// Where the run's inputs are.
#[derive(Debug, Args)]
pub(crate) struct DriveLocation {
    /// The project directory — the one holding `.engineering`. Discovered when omitted.
    #[arg(long)]
    project: Option<PathBuf>,
    /// The document tree. Comes from the project when omitted.
    #[arg(long)]
    root: Option<PathBuf>,
    /// The task document. Comes from the project when omitted.
    #[arg(long)]
    task: Option<PathBuf>,
    /// The planning store. Defaults to `<project>/.engineering/planning`.
    #[arg(long)]
    store: Option<PathBuf>,
    /// The step map: a file, or the id of one in the document tree.
    #[arg(long)]
    map: Option<String>,
    /// A plugin directory to load into every `llm` step's session. Repeatable.
    ///
    /// **W3.4's integration seam, and the reason it is a flag rather than a constant.** The
    /// plugin's `hooks/hooks.json` is the driver's enforcement arm — the layer that sees a tool's
    /// *arguments*, which `--allowedTools` cannot — and a session that never loaded the plugin
    /// never loaded the hooks. Where the plugin lives is a property of the machine, not of the
    /// protocol, so it is named here rather than guessed at. `AEP_DRIVE_PLUGIN_DIR` supplies it
    /// when the flag is absent, which is what lets an eval script set it once for a whole run.
    #[arg(long)]
    plugin_dir: Vec<PathBuf>,
}

/// The arguments of `protocol drive run`.
#[derive(Debug, Args)]
pub(crate) struct RunArgs {
    /// Where the run's inputs are.
    #[command(flatten)]
    location: DriveLocation,
    /// Run until the first thing a person owes, then persist and exit 0.
    #[arg(long)]
    pause_on_approval: bool,
    /// Stop after this many loop iterations, whatever the state of the run.
    #[arg(long, default_value_t = 25)]
    max_iterations: u32,
    /// Take the store lock from a holder that is provably dead.
    #[arg(long)]
    take_lock: bool,
}

/// The arguments of `protocol drive status`.
#[derive(Debug, Args)]
pub(crate) struct StatusArgs {
    /// Where the run's inputs are.
    #[command(flatten)]
    location: DriveLocation,
    /// Which run to report on. The store's current one when omitted.
    #[arg(long)]
    run: Option<String>,
}

/// The arguments of `protocol drive resume`.
#[derive(Debug, Args)]
pub(crate) struct ResumeArgs {
    /// The run to continue, such as `AUTH-142/3`.
    run: String,
    /// Where the run's inputs are.
    #[command(flatten)]
    location: DriveLocation,
    /// Run until the first thing a person owes, then persist and exit 0.
    #[arg(long)]
    pause_on_approval: bool,
    /// Stop after this many loop iterations, whatever the state of the run.
    #[arg(long, default_value_t = 25)]
    max_iterations: u32,
    /// Take the store lock from a holder that is provably dead.
    #[arg(long)]
    take_lock: bool,
}

/// Runs one `protocol drive` verb.
pub(crate) fn run(command: DriveCommand) -> Result<ExitCode> {
    match command {
        DriveCommand::Run(args) => start(&args),
        DriveCommand::Status(args) => status(&args),
        DriveCommand::Resume(args) => resume(&args),
    }
}

/// The project this was run in, or a refusal naming what to pass instead.
fn discover_project() -> Result<PathBuf> {
    let here = std::env::current_dir().context("reading the working directory")?;
    let directory = project_directory();
    aep_engine::project::discover(&here).with_context(|| {
        format!(
            "no `--project` was given and no `{directory}/project.yaml` was found in {} or \
             any parent",
            here.display()
        )
    })
}

/// Everything a run needs, resolved from flags or from the project it was run in.
struct Inputs {
    /// The project directory — the one holding `.engineering`.
    project: PathBuf,
    /// The documents in force.
    registry: Registry,
    /// The task being driven.
    task: Task,
    /// The planning store the artifact graph is rebuilt from every iteration.
    store: MarkdownStore,
    /// The step map driving the run.
    map: StepMap,
    /// Where the step map came from, for a report.
    map_origin: String,
    /// The plugin directories every `llm` step's session loads.
    plugin_dirs: Vec<PathBuf>,
}

impl DriveLocation {
    /// Resolves the run's inputs.
    fn inputs(&self) -> Result<Inputs> {
        let project = match &self.project {
            Some(path) => path.clone(),
            None => discover_project()?,
        };

        // The registry is loaded **once per invocation** and the store is rebuilt **per
        // iteration**, and the asymmetry is chosen rather than accidental (review finding F8): a
        // mid-run edit to `workflows/` is not picked up, because the cursor pins the workflow for
        // the life of the run precisely so a governing document cannot move under it; a mid-run
        // edit to the planning store *is*, because that is the work happening.
        let registry = match &self.root {
            Some(root) => crate::load(root)?,
            None => {
                aep_engine::project::load(&project)
                    .map_err(|errors| anyhow::anyhow!("{errors}"))?
                    .registry
            }
        };

        let task = match &self.task {
            Some(path) => crate::read_task(path)?,
            None => aep_engine::project::load(&project)
                .map_err(|errors| anyhow::anyhow!("{errors}"))?
                .task
                .context("the project names no task, and no `--task` was given")?,
        };

        let store = MarkdownStore::open(match &self.store {
            Some(path) => path.clone(),
            None => project.join(project_directory()).join("planning"),
        });

        let (map, map_origin) = self.step_map(&registry, &task)?;

        let plugin_dirs = self.plugin_dirs();

        Ok(Inputs {
            project,
            registry,
            task,
            store,
            map,
            map_origin,
            plugin_dirs,
        })
    }

    /// The plugin directories a session loads: the flags, then the environment.
    ///
    /// The environment is a fallback and never an addition — a caller that named directories meant
    /// those directories, and silently appending one from the ambient environment is how a run
    /// ends up enforcing something its own command line does not mention.
    fn plugin_dirs(&self) -> Vec<PathBuf> {
        if !self.plugin_dir.is_empty() {
            return self.plugin_dir.clone();
        }
        std::env::var_os(PLUGIN_DIR_ENV)
            .map(|value| vec![PathBuf::from(value)])
            .unwrap_or_default()
    }

    /// The step map: the file named by `--map`, the map with that id, or the only one that fits.
    fn step_map(&self, registry: &Registry, task: &Task) -> Result<(StepMap, String)> {
        if let Some(named) = &self.map {
            let path = Path::new(named);
            if path.is_file() {
                let text = fs::read_to_string(path)
                    .with_context(|| format!("reading {}", path.display()))?;
                let origin = path.display().to_string();
                let map = aep_schema::parse::step_map(&text, Some(&origin))
                    .map_err(|error| anyhow::anyhow!("{error}"))?;
                return Ok((map, origin));
            }
            let id = named.parse().map_err(|error| {
                anyhow::anyhow!("{named} is not a file and not a step map id: {error}")
            })?;
            let map = registry
                .step_map(&id)
                .with_context(|| format!("no step map `{named}` is in the document tree"))?;
            return Ok((map.clone(), format!("step map {named}")));
        }

        // No `--map`: the map is the one written against the workflow this task resolves to. More
        // than one is a choice the driver refuses to make on the caller's behalf — the same
        // position `protocol artifact move` takes for an illegal transition, and for the same
        // reason: the refusal names what to do instead.
        let plan = aep_engine::resolve(task, registry)
            .map_err(|errors| anyhow::anyhow!("{errors}"))
            .context("the task cannot be resolved")?;
        let fitting: Vec<&StepMap> = registry
            .step_maps()
            .filter(|map| {
                *map.workflow.id() == plan.workflow.id
                    && map.workflow.accepts(plan.workflow.version)
            })
            .collect();
        match fitting.as_slice() {
            [only] => Ok(((*only).clone(), format!("step map {}", only.id))),
            [] => bail!(
                "no step map in the document tree is written against `{}/{}`; pass `--map <file>`",
                plan.workflow.id,
                plan.workflow.version
            ),
            several => bail!(
                "{} step maps are written against `{}/{}` ({}); pass `--map` to choose one",
                several.len(),
                plan.workflow.id,
                plan.workflow.version,
                several
                    .iter()
                    .map(|map| map.id.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        }
    }
}

/// `protocol drive run`
fn start(args: &RunArgs) -> Result<ExitCode> {
    let inputs = args.location.inputs()?;
    let runs = runs_directory(&inputs.project)?;

    let engine = Engine::new(inputs.registry.clone());
    let plan = aep_engine::resolve(&inputs.task, &inputs.registry)
        .map_err(|errors| anyhow::anyhow!("{errors}"))
        .context("the task cannot be resolved")?;

    // Phase two of the map's cross-validation, run **before the first step executes**. The protocol
    // in force comes from the task, which no document loader has seen, so this cannot have happened
    // at load: without it a map validates and then fails at `ProtocolError::EvidenceRejected`
    // halfway through a run that has already spent a budget.
    let refusals = inputs.map.check_run(&plan.protocol, &plan.workflow);
    if !refusals.is_empty() {
        outln!("{} is not runnable against this task:", inputs.map_origin);
        for refusal in refusals.as_slice() {
            outln!("  - {refusal}");
        }
        return Ok(ExitCode::from(1));
    }

    // D3(c): the headless pre-flight, static and decidable and run before anything executes.
    let owed = owed_to_a_person(&plan, &inputs.map);
    if !owed.is_empty() && !args.pause_on_approval {
        outln!(
            "this run would reach {} thing(s) only a person can answer, and `--pause-on-approval` \
             was not given:",
            owed.len()
        );
        for line in &owed {
            outln!("  - {line}");
        }
        outln!();
        outln!(
            "`--pause-on-approval` runs until the first of them, persists and exits 0. There is no \
             flag that answers one: nothing below the driver checks who granted an approval, so \
             the refusal has to be the driver's."
        );
        return Ok(ExitCode::from(1));
    }

    let lock = take_lock(&runs, args.take_lock)?;
    let run_id = allocate_run(&runs, &inputs.task.id)?;
    lock.record_run(&run_id)?;
    let directory = RunDirectory::at(run_path(&runs, &run_id));
    fs::create_dir_all(directory.path())
        .with_context(|| format!("creating {}", directory.path().display()))?;
    fs::write(runs.join(CURRENT_FILE), format!("{run_id}\n"))
        .with_context(|| format!("writing {}", runs.join(CURRENT_FILE).display()))?;

    let options = DriverOptions {
        max_iterations: args.max_iterations,
        pause_on_approval: args.pause_on_approval,
        headless: true,
    };
    let mut executors = CliExecutors::new(
        inputs.project.clone(),
        directory.path().to_path_buf(),
        inputs.plugin_dirs.clone(),
    );
    let report = aep_driver::run::drive(
        &engine,
        &inputs.task,
        &inputs.store,
        &inputs.map,
        &directory,
        &mut executors,
        &options,
    );

    if let Some(stolen) = lock.stolen() {
        outln!(
            "note: this run took the lock from pid {} of run {}",
            stolen.pid,
            stolen.run
        );
    }
    let outcome = finish(report, &run_id, &inputs.map_origin);
    lock.release();
    outcome
}

/// `protocol drive resume`
fn resume(args: &ResumeArgs) -> Result<ExitCode> {
    let inputs = args.location.inputs()?;
    let runs = runs_directory(&inputs.project)?;
    let run_id: RunId = args
        .run
        .parse()
        .map_err(|error| anyhow::anyhow!("{error}"))?;
    let directory = RunDirectory::at(run_path(&runs, &run_id));
    if !directory.path().is_dir() {
        bail!("no run {run_id} in {}", runs.display());
    }

    // A paused run holds no lock, because the pause has no bound — so a resume must **re-take** it,
    // and must refuse when another run now holds it. The first draft said a pause releases and
    // never said a resume re-acquires, which left the obvious assumption to produce two live runs.
    let lock = take_lock(&runs, args.take_lock)?;
    lock.record_run(&run_id)?;

    let engine = Engine::new(inputs.registry.clone());
    let options = DriverOptions {
        max_iterations: args.max_iterations,
        pause_on_approval: args.pause_on_approval,
        headless: true,
    };
    let mut executors = CliExecutors::new(
        inputs.project.clone(),
        directory.path().to_path_buf(),
        inputs.plugin_dirs.clone(),
    );
    let report = aep_driver::run::resume(
        &engine,
        &inputs.task,
        &inputs.store,
        &inputs.map,
        &directory,
        &mut executors,
        &options,
    );
    let outcome = finish(report, &run_id, &inputs.map_origin);
    lock.release();
    outcome
}

/// `protocol drive status`
fn status(args: &StatusArgs) -> Result<ExitCode> {
    let project = match &args.location.project {
        Some(path) => path.clone(),
        None => discover_project()?,
    };
    let runs = project.join(project_directory()).join(RUNS_DIRECTORY);
    if !runs.is_dir() {
        outln!("no runs in {}", runs.display());
        return Ok(ExitCode::SUCCESS);
    }

    match read_lock(&runs)? {
        Some(holder) => {
            let state = holder.state();
            outln!(
                "lock       held by run {} (pid {} on {}, {})",
                holder.file.run.as_deref().unwrap_or("<unallocated>"),
                holder.file.pid,
                holder.file.host,
                match state.liveness {
                    Liveness::Alive => "alive",
                    Liveness::Dead => "not alive — stale, and still refused without --take-lock",
                    Liveness::OtherHost => "another host, so never stale here",
                }
            );
        }
        None => outln!("lock       free"),
    }

    let named = match &args.run {
        Some(run) => run.clone(),
        None => fs::read_to_string(runs.join(CURRENT_FILE))
            .unwrap_or_default()
            .trim()
            .to_owned(),
    };
    if named.is_empty() {
        outln!("current    none");
        return Ok(ExitCode::SUCCESS);
    }
    let run_id: RunId = named.parse().map_err(|error| anyhow::anyhow!("{error}"))?;
    let directory = RunDirectory::at(run_path(&runs, &run_id));
    let cursor = directory
        .read_cursor()
        .map_err(|error| anyhow::anyhow!("{error}"))?;
    print_cursor(&cursor);
    Ok(ExitCode::SUCCESS)
}

/// Prints a cursor, which is what `status` is for.
fn print_cursor(cursor: &DriverCursor) {
    outln!("run        {}", cursor.run);
    outln!("task       {}", cursor.task);
    outln!("execution  {}", cursor.execution);
    outln!("workflow   {}", cursor.workflow);
    outln!("map        {} ({})", cursor.map, cursor.map_digest);
    outln!("state      {} (step {})", cursor.state, cursor.step);
    outln!("status     {}", cursor.status);
    outln!("iterations {}", cursor.iterations);
    for (state, visits) in &cursor.visits {
        outln!("visits     {state}: {visits}");
    }
    for (step, attempts) in &cursor.attempts {
        outln!("attempts   {step}: {attempts}");
    }
    if let Some(stolen) = &cursor.took_lock_from {
        outln!(
            "took lock  from pid {} of run {} on {}",
            stolen.pid,
            stolen.run,
            stolen.host
        );
    }
    for reason in &cursor.reasons {
        outln!("           {reason}");
    }
}

/// Renders a finished run and chooses the exit code.
fn finish(
    report: Result<RunReport, DriveError>,
    run: &RunId,
    map_origin: &str,
) -> Result<ExitCode> {
    let report = match report {
        Ok(report) => report,
        Err(error) => bail!("{error}"),
    };

    outln!("run        {run}");
    outln!("map        {map_origin}");
    outln!("status     {}", report.cursor.status);
    outln!("state      {}", report.cursor.state);
    outln!(
        "steps      {} run, {} submitted",
        report.steps_run,
        report.evidence_submitted
    );
    for (from, to) in &report.transitions {
        outln!("moved      {from} -> {to}");
    }
    for note in &report.notes {
        outln!("note       {note}");
    }
    // The engine's words, verbatim. The driver adds its own lines beside them and never summarises
    // or re-words them: a report that paraphrased a refusal would be a second, worse protocol.
    if !report.reasons.is_empty() {
        outln!("blocked because:");
        for reason in &report.reasons {
            outln!("  - {reason}");
        }
    }
    if let Some(explanation) = &report.explanation {
        outln!("{explanation}");
    }
    if report.cursor.status.is_resumable() {
        outln!("resume with: protocol drive resume {run}");
    }

    Ok(match report.cursor.status {
        RunStatus::Completed | RunStatus::AwaitingOperator => ExitCode::SUCCESS,
        _ => ExitCode::from(1),
    })
}

/// Everything only a person can answer that this run would reach.
///
/// Two static, decidable sources, and both are checked before the first step because the
/// alternative is starting a run that will certainly wedge:
///
/// * the plan's own reachable approvals — `human: true` approvals and reviews, human verifiers, and
///   capabilities a `command` step would exercise that need one ([`aep_driver::approval`]);
/// * an `operator` step in a state this workflow can reach from where the run starts. The map is
///   saying a person is owed something there, which is the same fact in a different document.
fn owed_to_a_person(plan: &aep_domain::plan::ExecutionPlan, map: &StepMap) -> Vec<String> {
    let mut owed: Vec<String> = aep_driver::approval::reachable_approvals(plan, map)
        .into_iter()
        .map(|approval| format!("{}: {}", approval.source, approval.detail))
        .collect();

    for state in reachable_states(&plan.workflow) {
        for (index, step) in map.steps_for(&state).iter().enumerate() {
            if let Step::Operator(operator) = step {
                owed.push(format!(
                    "step map, state {state} step {index}: an operator step — {}",
                    operator
                        .description
                        .clone()
                        .unwrap_or_else(|| operator.prompt.clone())
                ));
            }
        }
    }
    owed
}

/// Every state reachable from the workflow's initial state, including it.
fn reachable_states(workflow: &aep_domain::workflow::Workflow) -> BTreeSet<StateId> {
    let mut reached: BTreeSet<StateId> = BTreeSet::new();
    let mut frontier = vec![workflow.initial.clone()];
    while let Some(state) = frontier.pop() {
        if !reached.insert(state.clone()) {
            continue;
        }
        for transition in &workflow.transitions {
            if transition.from == state {
                frontier.push(transition.to.clone());
            }
        }
    }
    reached
}

/// The `.engineering/runs/` directory, created if it is not there.
fn runs_directory(project: &Path) -> Result<PathBuf> {
    let runs = project.join(project_directory()).join(RUNS_DIRECTORY);
    fs::create_dir_all(&runs).with_context(|| format!("creating {}", runs.display()))?;
    Ok(runs)
}

/// The directory of one run.
fn run_path(runs: &Path, run: &RunId) -> PathBuf {
    let [task, ordinal] = run.segments();
    runs.join(task).join(ordinal)
}

/// The next run id for a task: one more than the highest that exists.
///
/// Allocated **after** the lock is taken, which is the whole of review finding F2. A run directory
/// is never deleted and never reused, so the count only goes up.
fn allocate_run(runs: &Path, task: &TaskId) -> Result<RunId> {
    let directory = runs.join(task.as_str());
    let mut highest = 0_u32;
    if directory.is_dir() {
        for entry in
            fs::read_dir(&directory).with_context(|| format!("reading {}", directory.display()))?
        {
            let entry = entry.with_context(|| format!("reading {}", directory.display()))?;
            if let Some(ordinal) = entry
                .file_name()
                .to_str()
                .and_then(|name| name.parse::<u32>().ok())
            {
                highest = highest.max(ordinal);
            }
        }
    }
    RunId::new(task, highest + 1).map_err(|error| anyhow::anyhow!("{error}"))
}

/// What `lock.json` holds.
///
/// **No timestamp.** Staleness is decided by liveness rather than by a number somebody wrote into a
/// file: any age threshold has to exceed the longest legitimate step, and the longest legitimate
/// step is an `operator` step waiting for a person, which has no bound. A driver that broke a lock
/// after two hours would break exactly the runs that paused correctly.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct LockFile {
    /// The run it granted, once one has been allocated.
    run: Option<String>,
    /// The process holding it.
    pid: u32,
    /// The host that process is on.
    host: String,
    /// The driver that took it.
    driver: String,
}

/// A lock this process holds, and the run it took it from if it took one.
struct HeldLock {
    path: PathBuf,
    stolen: Option<StolenLock>,
}

impl HeldLock {
    /// Records the run id inside the lock, so a refusal can name it without a second read.
    fn record_run(&self, run: &RunId) -> Result<()> {
        let mut file: LockFile = serde_json::from_str(
            &fs::read_to_string(&self.path)
                .with_context(|| format!("reading {}", self.path.display()))?,
        )
        .with_context(|| format!("reading {}", self.path.display()))?;
        file.run = Some(run.to_string());
        fs::write(&self.path, serde_json::to_string_pretty(&file)?)
            .with_context(|| format!("writing {}", self.path.display()))
    }

    /// What this lock was taken from, when it was taken from somebody.
    fn stolen(&self) -> Option<&StolenLock> {
        self.stolen.as_ref()
    }

    /// Releases the lock.
    ///
    /// Called on every exit path the driver controls, including the approval pause and budget
    /// exhaustion: a paused run does not hold a lock, because the pause has no bound. What a paused
    /// run keeps is `current`, so resuming is one word.
    fn release(self) {
        let _ = fs::remove_file(&self.path);
    }
}

/// A lock somebody else holds.
struct Holder {
    file: LockFile,
}

impl Holder {
    /// The holder as the router sees it — a value, never a probe.
    fn state(&self) -> LockState {
        LockState {
            run: self
                .file
                .run
                .clone()
                .unwrap_or_else(|| "<unallocated>".to_owned()),
            pid: self.file.pid,
            host: self.file.host.clone(),
            liveness: liveness(&self.file),
        }
    }
}

/// Reads the lock, when there is one.
fn read_lock(runs: &Path) -> Result<Option<Holder>> {
    let path = runs.join(LOCK_FILE);
    let Ok(text) = fs::read_to_string(&path) else {
        return Ok(None);
    };
    let file: LockFile =
        serde_json::from_str(&text).with_context(|| format!("reading {}", path.display()))?;
    Ok(Some(Holder { file }))
}

/// Takes the store lock, or refuses and names the holder.
fn take_lock(runs: &Path, force: bool) -> Result<HeldLock> {
    let path = runs.join(LOCK_FILE);
    let mine = LockFile {
        run: None,
        pid: std::process::id(),
        host: host(),
        driver: format!("protocol-cli {}", env!("CARGO_PKG_VERSION")),
    };
    let body = serde_json::to_string_pretty(&mine)?;

    // One `create_new` syscall: atomic on every filesystem that matters, and it needs no advisory
    // locking. `flock` was rejected because its semantics differ across the filesystems people keep
    // repositories on, NFS in particular.
    match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
    {
        Ok(mut handle) => {
            use std::io::Write as _;
            handle
                .write_all(body.as_bytes())
                .with_context(|| format!("writing {}", path.display()))?;
            return Ok(HeldLock { path, stolen: None });
        }
        Err(error) if error.kind() != std::io::ErrorKind::AlreadyExists => {
            return Err(error).with_context(|| format!("creating {}", path.display()));
        }
        Err(_) => {}
    }

    let holder = read_lock(runs)?.context("the lock exists and cannot be read")?;
    let state = holder.state();
    if !force || !state.is_stale() {
        bail!("{}", state.refusal(force));
    }

    // `--take-lock` supersedes rather than erases: what was there goes into the new run's cursor,
    // so *"this run took the lock from pid 4711"* is in the record rather than in nobody's memory.
    fs::write(&path, &body).with_context(|| format!("writing {}", path.display()))?;
    Ok(HeldLock {
        path,
        stolen: Some(StolenLock {
            run: state.run.clone(),
            pid: state.pid,
            host: state.host.clone(),
        }),
    })
}

/// Whether the process named in a lock is alive, dead, or somebody else's problem.
///
/// **Liveness, never age.** A pid on another host says nothing to this one's process table, so a
/// lock naming another host is never stale here whatever the local table says.
fn liveness(file: &LockFile) -> Liveness {
    if file.host != host() {
        return Liveness::OtherHost;
    }
    if Path::new("/proc").is_dir() {
        return if Path::new(&format!("/proc/{}", file.pid)).exists() {
            Liveness::Alive
        } else {
            Liveness::Dead
        };
    }
    // No `/proc` to read: the honest answer is that this build cannot tell, and the safe one is to
    // treat the holder as alive. A lock nobody can prove is dead is a lock nobody may take.
    Liveness::Alive
}

/// This machine's name, for the lock.
fn host() -> String {
    for path in ["/proc/sys/kernel/hostname", "/etc/hostname"] {
        if let Ok(name) = fs::read_to_string(path) {
            let name = name.trim();
            if !name.is_empty() {
                return name.to_owned();
            }
        }
    }
    std::env::var("HOSTNAME").unwrap_or_else(|_| "unknown-host".to_owned())
}

/// The three things that touch the world.
struct CliExecutors {
    /// Where a command step runs.
    working_directory: PathBuf,
    /// Where transcripts and logs go.
    run_directory: PathBuf,
    /// The plugins every `llm` step's session loads — and with them, the hooks.
    plugin_dirs: Vec<PathBuf>,
}

impl CliExecutors {
    /// Builds the executors for one run.
    fn new(working_directory: PathBuf, run_directory: PathBuf, plugin_dirs: Vec<PathBuf>) -> Self {
        Self {
            working_directory,
            run_directory,
            plugin_dirs,
        }
    }

    /// Writes the document the session's hooks read, and returns where it went.
    ///
    /// Rewritten before **every** `llm` step rather than once per run, because the thing it
    /// describes changes at every `Moved`: `effective_policy` grants the state's capabilities on
    /// top of the plan's, so the legal surface in `implement` is not the legal surface in `review`.
    /// A run-scoped context would be a per-state rule enforced with per-run facts.
    fn write_step_context(&self, context: &StepContext<'_>) -> Result<PathBuf, String> {
        let path = self.run_directory.join(STEP_CONTEXT_FILE);
        let document = StepContextFile {
            format: STEP_CONTEXT_FORMAT,
            run_directory: self.run_directory.clone(),
            store: self
                .working_directory
                .join(project_directory())
                .join("planning"),
            state: context.state.to_string(),
            step_index: context.index,
            attempt: context.attempt,
            shell_offered: context.tools.shell_offered(),
            capabilities: context
                .tools
                .capabilities()
                .iter()
                .map(ToString::to_string)
                .collect(),
            tools: allowed_tools(context.tools),
        };
        let text = serde_json::to_string_pretty(&document)
            .map_err(|error| format!("the step context would not serialise: {error}"))?;
        fs::write(&path, format!("{text}\n"))
            .map_err(|error| format!("cannot write {}: {error}", path.display()))?;
        Ok(path)
    }
}

/// The format claim on [`STEP_CONTEXT_FILE`].
const STEP_CONTEXT_FORMAT: &str = "aep.drive-step-context/1";

/// What a hook is told about the step it is adjudicating.
///
/// It carries **facts about the state**, never the rules: the surface a shell is held to is
/// declared in the hook that enforces it, not here. A run that could name its own allowed surface
/// could widen it, and a widening the run authored is a route around the constraint rather than a
/// check on it.
#[derive(Debug, serde::Serialize)]
struct StepContextFile {
    /// The format claim.
    format: &'static str,
    /// Where the run's records live — and where a hook appends its decisions.
    run_directory: PathBuf,
    /// The planning store this run is over.
    store: PathBuf,
    /// The workflow state the step belongs to.
    state: String,
    /// Which step of that state's list this is.
    step_index: usize,
    /// Which attempt at that step this is.
    attempt: u32,
    /// Whether `command.execute` is admitted here at all.
    shell_offered: bool,
    /// Every admitted capability, as the protocol spells it.
    capabilities: Vec<String>,
    /// The harness's own names for them, as passed to `--allowedTools`.
    tools: Vec<String>,
}

impl CommandStepExecutor for CliExecutors {
    fn run_command(&mut self, step: &CommandStep, context: &StepContext<'_>) -> StepOutcome {
        let rendered = step.run.join(" ");
        let outcome = Process::new(step.program())
            .args(step.arguments())
            .current_dir(&self.working_directory)
            .stdin(Stdio::null())
            .output();

        let output = match outcome {
            Ok(output) => output,
            // Nothing was observed: a missing executable is not a failing suite. Submitting a
            // failing `TestResult` for a suite that never ran would fabricate an observation, which
            // is invariant 7's failure one layer above the engine.
            Err(error) => {
                return StepOutcome::NoVerdict {
                    reason: format!("`{rendered}` could not be run: {error}"),
                }
            }
        };

        let log = self.run_directory.join(format!(
            "{}-{}-{}.log",
            context.state, context.index, context.attempt
        ));
        let mut body = String::new();
        body.push_str(&String::from_utf8_lossy(&output.stdout));
        body.push_str(&String::from_utf8_lossy(&output.stderr));
        let _ = fs::write(&log, body);

        let Some(code) = output.status.code() else {
            // Killed by a signal: a partial suite is not a failing suite.
            return StepOutcome::NoVerdict {
                reason: format!("`{rendered}` was killed before it produced a verdict"),
            };
        };

        let Some(mapping) = &step.evidence else {
            return if code == 0 {
                StepOutcome::Nothing
            } else {
                StepOutcome::NoVerdict {
                    reason: format!(
                        "`{rendered}` exited {code} and declares no evidence, so \
                                     nothing was observed"
                    ),
                }
            };
        };

        match mint(mapping, code == 0, &rendered, observed_now()) {
            Some(submission) => StepOutcome::Observed(Box::new(submission)),
            None => StepOutcome::NoVerdict {
                reason: format!(
                    "`{rendered}` exited {code}, and a `{}` record has no form that says so",
                    mapping.kind.as_str()
                ),
            },
        }
    }
}

impl OperatorStepExecutor for CliExecutors {
    fn run_operator(&mut self, step: &OperatorStep, context: &StepContext<'_>) -> StepOutcome {
        outln!();
        outln!("this run needs a person, in state {}:", context.state);
        outln!("  {}", step.prompt);
        // Verbatim, one line per requirement, because that is what the explanation is *for*: a
        // summary of what is outstanding is a second opinion about it.
        if !context.requirements.is_empty() {
            outln!();
            outln!("what is outstanding here:");
            for line in context.requirements {
                outln!("  {line}");
            }
        }
        StepOutcome::Paused {
            reason: format!("an operator step in {} is owed an answer", context.state),
        }
    }
}

impl LlmStepExecutor for CliExecutors {
    fn run_llm(&mut self, step: &LlmStep, context: &StepContext<'_>) -> StepOutcome {
        // The seam § 4.9 point 3 names, and the reason it is a name rather than a trait: a second
        // harness is a second executor selected by this string, chosen when there is a second
        // implementation to design the selection against. W3.5's shell-echo harness is the first
        // one that will use it.
        if step.harness != LlmStep::DEFAULT_HARNESS {
            return StepOutcome::NoVerdict {
                reason: format!(
                    "the step names harness `{}`, and this build only invokes `{}`",
                    step.harness,
                    LlmStep::DEFAULT_HARNESS
                ),
            };
        }

        let transcripts = self.run_directory.join(TRANSCRIPTS);
        if let Err(error) = fs::create_dir_all(&transcripts) {
            return StepOutcome::NoVerdict {
                reason: format!(
                    "cannot write transcripts to {}: {error}",
                    transcripts.display()
                ),
            };
        }
        let settings = self.run_directory.join(SETTINGS_FILE);
        if !settings.exists() {
            let _ = fs::write(&settings, "{}\n");
        }
        let transcript = transcripts.join(format!(
            "{}-{}-{}.jsonl",
            context.state, context.index, context.attempt
        ));

        // Before the session, never during it: a hook fires inside a process the driver has already
        // launched, so a context written late is a context the first tool call never saw.
        let step_context = match self.write_step_context(context) {
            Ok(path) => path,
            Err(reason) => return StepOutcome::NoVerdict { reason },
        };

        let argv = claude_argv(
            step,
            context,
            &settings,
            &self.plugin_dirs,
            &prompt_for(step, context),
        );
        let outcome = Process::new(&argv[0])
            .args(&argv[1..])
            .current_dir(&self.working_directory)
            .env(STEP_CONTEXT_ENV, &step_context)
            .stdin(Stdio::null())
            .output();

        let output = match outcome {
            Ok(output) => output,
            Err(error) => {
                return StepOutcome::NoVerdict {
                    reason: format!("`{}` could not be run: {error}", argv.join(" ")),
                }
            }
        };
        let _ = fs::write(&transcript, &output.stdout);

        if output.status.success() {
            // An `llm` step never carries evidence, and the type is what makes that true. What the
            // model achieved that is checkable is observed by the command step after it.
            StepOutcome::Nothing
        } else {
            StepOutcome::NoVerdict {
                reason: format!(
                    "the model invocation exited {}; the transcript is at {}",
                    output
                        .status
                        .code()
                        .map_or_else(|| "on a signal".to_owned(), |code| code.to_string()),
                    transcript.display()
                ),
            }
        }
    }
}

/// The prompt one `llm` step is given.
///
/// Assembled from the step map's own prompt and the state's requirement lines, each of which names
/// the document that asked for it. Everything an `llm` step knows is either in a file or in this
/// string — which is the property that makes a step's input a function of persisted state, and
/// therefore the property the narrow replay claim rests on.
fn prompt_for(step: &LlmStep, context: &StepContext<'_>) -> String {
    let mut prompt = String::new();
    prompt.push_str(&step.prompt);
    // The skills the step names, in the prompt rather than on the command line. `--agents` takes a
    // JSON object of *agent definitions* and is not a skill selector; a step map's `skills:` list
    // reaches the session by being asked for, and the `Skill` tool — a named exemption in the tool
    // table, because loading instructions takes no action — is what answers.
    if !step.skills.is_empty() {
        prompt.push_str("\n\nLoad ");
        for (position, skill) in step.skills.iter().enumerate() {
            if position > 0 {
                prompt.push_str(" and ");
            }
            prompt.push_str("the `");
            prompt.push_str(skill);
            prompt.push('`');
        }
        prompt.push_str(if step.skills.len() == 1 {
            " skill before you act, with the `Skill` tool.\n"
        } else {
            " skills before you act, with the `Skill` tool.\n"
        });
    }
    prompt.push_str("\n\nYou are in workflow state `");
    prompt.push_str(context.state.as_str());
    prompt.push_str("`.\n");
    if !context.requirements.is_empty() {
        prompt.push_str("\nWhat must hold here, one line per requirement:\n");
        for line in context.requirements {
            prompt.push_str("  ");
            prompt.push_str(line);
            prompt.push('\n');
        }
    }
    prompt.push_str(
        "\nYou cannot submit evidence, and nothing you say is evidence. What you achieve is \
         observed by the verifier the driver runs after this step.\n",
    );
    prompt
}

/// The command line one `llm` step is invoked with.
///
/// Two rules are asserted over this value rather than left as notes, because both failures would be
/// silent: it **never** contains `--bare`, which skips hooks and would delete the driver's own
/// enforcement arm, and it **always** carries `--settings`. Review finding **F15**.
fn claude_argv(
    step: &LlmStep,
    context: &StepContext<'_>,
    settings: &Path,
    plugin_dirs: &[PathBuf],
    prompt: &str,
) -> Vec<String> {
    let mut argv = vec![
        "claude".to_owned(),
        "-p".to_owned(),
        prompt.to_owned(),
        "--output-format".to_owned(),
        "stream-json".to_owned(),
        "--verbose".to_owned(),
        "--settings".to_owned(),
        settings.display().to_string(),
    ];
    let tools = allowed_tools(context.tools);
    if !tools.is_empty() {
        argv.push("--allowedTools".to_owned());
        argv.push(tools.join(","));
    }
    // The plugin, and with it `hooks/hooks.json` — the layer that sees a tool's arguments. A
    // session launched without it is a session where every § 4.8 row whose mechanism is "plugin
    // hook" is a claim with nothing behind it, which is the same silent, partial failure `--bare`
    // would cause.
    for directory in plugin_dirs {
        argv.push("--plugin-dir".to_owned());
        argv.push(directory.display().to_string());
    }
    // `step.skills` is deliberately not on this line: it is in the prompt. See `prompt_for`.
    let _ = step;
    argv
}

/// The harness's tool names for an admitted capability set.
///
/// The rendering half of adapter point 2: the *decision* about which capabilities admit which
/// actions is the protocol's and is shared; only this table is Claude Code's. Three entries are not
/// functions of a capability and each is decided rather than left to an implementer — a shell is
/// offered only with `command.execute`, `Skill` is a named exemption, and `Task` is never offered,
/// because a subagent's tool set is derived by nothing in these decisions and would be a route
/// around the per-state allowlist.
fn allowed_tools(config: &ToolConfig) -> Vec<String> {
    let mut tools: Vec<String> = Vec::new();
    if config.admits(&Capability::RepositoryRead) || config.admits(&Capability::ArtifactRead) {
        tools.extend(["Read", "Glob", "Grep"].map(ToOwned::to_owned));
    }
    if config.admits(&Capability::RepositoryWrite) {
        tools.extend(["Edit", "Write", "NotebookEdit"].map(ToOwned::to_owned));
    }
    if config.admits(&Capability::NetworkRead) {
        tools.extend(["WebFetch", "WebSearch"].map(ToOwned::to_owned));
    }
    if config.shell_offered() {
        tools.push("Bash".to_owned());
    }
    if config.skills_offered() {
        tools.push("Skill".to_owned());
    }
    tools.sort();
    tools.dedup();
    tools
}

/// The instant the driver just observed something, from the wall clock.
///
/// The driver runs the program and reads its exit status, so *now* is the truthful observation
/// time — this is the one case where the two times an evidence record carries legitimately
/// coincide, and it is stated rather than assumed. It lives in `protocol-cli` and not in a pure
/// crate for the reason the store lock does: reading ambient OS state is this binary's job.
fn observed_now() -> ObservedAt {
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| {
            u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
        });
    ObservedAt::new(Timestamp::from_epoch_millis(millis))
}

/// Turns a verdict into the evidence the map said it establishes.
///
/// The per-kind rule, in one place: three kinds carry a verdict and can therefore say *no*; `diff`
/// has no failing form — a `ChangeSet` cannot state that no change happened — so a failed
/// observation of one is an absence rather than a `False`, and absence is spelled *submit nothing*.
fn mint(
    mapping: &EvidenceMapping,
    passed: bool,
    command: &str,
    observed_at: ObservedAt,
) -> Option<EvidenceSubmission> {
    let evidence = match mapping.kind {
        EvidenceKind::TestResult => {
            let suite = mapping.suite.clone().unwrap_or(TestSuite::Unit);
            Evidence::TestResult(if passed {
                TestResult::passing(suite, 1)
            } else {
                TestResult::failing(suite, 0, 1)
            })
        }
        EvidenceKind::StaticAnalysis => Evidence::StaticAnalysis(StaticAnalysisResult {
            tool: mapping.tool.clone(),
            errors: usize::from(!passed),
            warnings: 0,
        }),
        EvidenceKind::ContractResult => Evidence::ContractResult(ContractResult {
            checked: 1,
            failed: usize::from(!passed),
            breaking_changes: 0,
            consumer: None,
            provider: None,
        }),
        // The counts are zero because nothing read them: an exit status carries no numbers, and a
        // fabricated count is worse than a missing one — the engine cannot tell an invented number
        // apart from an observed one. What the record establishes is `diff.exists`, which is what
        // the shipped workflow's guard reads.
        EvidenceKind::Diff if passed => Evidence::Diff(ChangeSet {
            files_changed: 0,
            lines_added: 0,
            lines_removed: 0,
            revision_before: None,
            revision_after: None,
            paths: Vec::new(),
        }),
        _ => return None,
    };

    let mut submission = EvidenceSubmission::new(
        evidence,
        // A verifier produced it, because a verifier produced it: the driver ran the program and
        // read its exit status. Nothing about a model's opinion of the run enters the record, which
        // is how `independent: true` is honestly satisfied.
        Producer::Verifier {
            verifier: mapping.verifier.clone(),
        },
        // And the observation happened when the program ran, which for this driver is now. That is
        // the honest value here and it is passed in rather than read here, so the one place this
        // binary reads a wall clock stays countable.
        observed_at,
    );
    submission.subject.clone_from(&mapping.subject);
    submission.provenance = Provenance {
        command: Some(command.to_owned()),
        tool: mapping.tool.clone().or_else(|| tool_of(&mapping.verifier)),
        ..Provenance::default()
    };
    Some(submission)
}

/// The tool a verifier names, when it names one.
fn tool_of(verifier: &Verifier) -> Option<ToolRef> {
    match verifier {
        Verifier::ExternalTool(tool) => Some(tool.clone()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aep_domain::capability::Environment;
    use aep_domain::ids::StateId;

    fn config(capabilities: &[Capability]) -> ToolConfig {
        ToolConfig::new(capabilities.iter().cloned().collect())
    }

    /// The command line the driver would build for one `llm` step.
    fn argv_for(skills: &[&str], plugins: &[&str]) -> Vec<String> {
        let step = LlmStep {
            description: None,
            harness: LlmStep::DEFAULT_HARNESS.to_owned(),
            skills: skills.iter().map(ToString::to_string).collect(),
            prompt: "do the thing".to_owned(),
        };
        let tools = config(&[Capability::RepositoryRead, Capability::CommandExecution]);
        let state: StateId = "specify".parse().expect("a state id");
        let requirements: Vec<String> = Vec::new();
        let context = StepContext {
            state: &state,
            index: 0,
            attempt: 1,
            tools: &tools,
            run_directory: Path::new("/runs/T-1/1"),
            requirements: &requirements,
        };
        let plugin_dirs: Vec<PathBuf> = plugins.iter().map(PathBuf::from).collect();
        let prompt = prompt_for(&step, &context);
        claude_argv(
            &step,
            &context,
            Path::new("/runs/T-1/1/settings.json"),
            &plugin_dirs,
            &prompt,
        )
    }

    /// Review finding **F15**, as a test rather than a note, because both failures are silent.
    ///
    /// `--bare` skips hooks. A future implementer reaching for a clean, reproducible environment —
    /// a reasonable instinct in a repository this deterministic — would silently delete the
    /// driver's own enforcement arm, and every enforcement row whose layer is "plugin hook" would
    /// become a claim with nothing behind it. The tool set would still be constrained by
    /// `--allowedTools`, so the failure is partial and silent, which is the worst shape it has.
    #[test]
    fn the_model_invocation_never_skips_hooks_and_always_carries_its_settings() {
        let argv = argv_for(&["planning"], &["/plugins/engineering-protocols"]);
        assert!(
            !argv.iter().any(|word| word == "--bare"),
            "`--bare` skips hooks, which is the driver's own enforcement arm: {argv:?}"
        );
        let settings = argv
            .iter()
            .position(|word| word == "--settings")
            .expect("the settings flag is always present");
        assert_eq!(argv[settings + 1], "/runs/T-1/1/settings.json");
    }

    #[test]
    fn a_named_plugin_directory_reaches_the_session_so_the_hooks_do() {
        let argv = argv_for(&[], &["/plugins/a", "/plugins/b"]);
        let named: Vec<&String> = argv
            .iter()
            .enumerate()
            .filter(|(index, _)| *index > 0 && argv[index - 1] == "--plugin-dir")
            .map(|(_, word)| word)
            .collect();
        assert_eq!(named, ["/plugins/a", "/plugins/b"]);
    }

    /// A step map's `skills:` list is a request to the model, not a command-line flag.
    ///
    /// `--agents` takes a JSON object of *agent definitions*; passing a skill name to it is a usage
    /// error that fails the whole invocation, and the first draft of this function did exactly
    /// that. The skill reaches the session by being asked for, and the `Skill` tool answers.
    #[test]
    fn a_steps_skills_are_asked_for_in_the_prompt_and_never_passed_as_agent_definitions() {
        let argv = argv_for(&["planning"], &[]);
        assert!(
            !argv.iter().any(|word| word == "--agents"),
            "`--agents` defines agents from JSON; it is not a skill selector: {argv:?}"
        );
        let prompt = &argv[2];
        assert!(
            prompt.contains("Load the `planning` skill"),
            "the step's skill has to be asked for somewhere: {prompt}"
        );
    }

    #[test]
    fn the_rendering_offers_a_shell_only_when_the_capability_is_admitted() {
        let reading = config(&[Capability::RepositoryRead]);
        assert_eq!(allowed_tools(&reading), ["Glob", "Grep", "Read", "Skill"]);
        assert!(!allowed_tools(&reading).contains(&"Bash".to_owned()));

        let shell = config(&[Capability::CommandExecution]);
        assert!(allowed_tools(&shell).contains(&"Bash".to_owned()));
    }

    #[test]
    fn a_subagent_spawner_is_never_rendered_whatever_is_admitted() {
        let everything = config(&[
            Capability::RepositoryRead,
            Capability::RepositoryWrite,
            Capability::CommandExecution,
            Capability::NetworkRead,
            Capability::Deploy(Environment::Production),
        ]);
        assert!(
            !allowed_tools(&everything).contains(&"Task".to_owned()),
            "a subagent's tool set is derived by nothing in D1-D6, so it is a route around the \
             per-state allowlist"
        );
    }

    #[test]
    fn a_failing_command_mints_a_record_that_says_so_and_a_failed_diff_mints_nothing() {
        let mapping = EvidenceMapping {
            kind: EvidenceKind::TestResult,
            verifier: Verifier::TestRunner,
            suite: Some(TestSuite::Unit),
            subject: None,
            tool: None,
        };
        let failed = mint(&mapping, false, "cargo test", observed_now()).expect("a verdict");
        match &failed.evidence {
            Evidence::TestResult(result) => assert_eq!(result.failed, 1),
            other => panic!("expected a test result, got {other:?}"),
        }
        assert_eq!(
            failed.producer,
            Producer::Verifier {
                verifier: Verifier::TestRunner
            }
        );

        let diff = EvidenceMapping {
            kind: EvidenceKind::Diff,
            verifier: Verifier::parse("git").expect("a verifier"),
            suite: None,
            subject: None,
            tool: None,
        };
        assert!(
            mint(&diff, false, "git diff", observed_now()).is_none(),
            "a ChangeSet has no form that says no change happened, so the honest answer is to \
             submit nothing"
        );
    }
}
