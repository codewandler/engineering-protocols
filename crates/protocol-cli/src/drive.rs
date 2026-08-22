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
use aep_domain::action::{
    Action, ActionRequest, CommandExecute, NetworkIntent, NetworkRequest, RepositoryRead,
    RepositoryWrite,
};
use aep_domain::capability::Capability;
use aep_domain::evidence::{
    ChangeSet, ContractResult, Evidence, EvidenceKind, Producer, Provenance, StaticAnalysisResult,
    TestResult, TestSuite,
};
use aep_domain::ids::{StateId, TaskId, ToolRef};
use aep_domain::task::Task;
use aep_domain::time::{ObservedAt, Timestamp};
use aep_domain::verification::Verifier;
use aep_driver::coverage::CoverageReport;
use aep_driver::executor::{
    CommandStepExecutor, LlmStepExecutor, OperatorStepExecutor, StepAuthorizer, StepContext,
    StepOutcome,
};
use aep_driver::lock::{Liveness, LockState};
use aep_driver::run::{DriveError, DriverOptions, RunDirectory, RunReport};
use aep_driver_spec::cursor::{DriverCursor, RunId, RunStatus, StolenLock};
use aep_driver_spec::map::{
    placeholders_in, CommandStep, EvidenceMapping, LlmStep, OperatorStep, Step, StepMap,
};
use aep_driver_spec::tool::ToolConfig;
use aep_engine::engine::EvidenceSubmission;
use aep_engine::policy::Decision;
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

/// Where one attempt at one `llm` step leaves its transcript.
///
/// One function rather than two spellings of the same format string: the step that *writes* the
/// transcript and the step that *checks* it are different steps of a map, and a checker pointed at
/// a path the writer never used would report that a session did nothing.
fn transcript_path(run_directory: &Path, state: &StateId, index: usize, attempt: u32) -> PathBuf {
    run_directory
        .join(TRANSCRIPTS)
        .join(format!("{state}-{index}-{attempt}.jsonl"))
}

/// Expands the placeholders a step map admits, or says which one it could not.
///
/// The vocabulary is `aep_driver_spec::map::CommandStep::PLACEHOLDERS` and an unknown name is
/// refused at load, so the only failure reachable here is a `{transcript}` in a run where the
/// `llm` step before it has not run — which is a fact about the run and cannot be decided from the
/// document.
fn expand(word: &str, context: &StepContext<'_>) -> Result<String, String> {
    let mut expanded = word.to_owned();
    for name in placeholders_in(word) {
        let value = match name {
            "run_directory" => context.run_directory.display().to_string(),
            "transcript" => {
                let Some(step) = context.preceding_llm else {
                    return Err(format!(
                        "`{{transcript}}` is the transcript of the `llm` step this one follows, \
                         and no `llm` step of `{}` has run in this run",
                        context.state
                    ));
                };
                transcript_path(
                    context.run_directory,
                    context.state,
                    step.index,
                    step.attempt,
                )
                .display()
                .to_string()
            }
            other => return Err(format!("nothing expands `{{{other}}}`")),
        };
        expanded = expanded.replace(&format!("{{{name}}}"), &value);
    }
    Ok(expanded)
}

/// Where the plugin lives, when no `--plugin-dir` said.
const PLUGIN_DIR_ENV: &str = "AEP_DRIVE_PLUGIN_DIR";

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
    /// Start even though the map cannot produce evidence the plan will demand.
    ///
    /// **This weakens no rule the engine enforces.** The pre-flight it turns off is an *economic*
    /// check, not a protocol one: without it a run walks every state and blocks at the guard that
    /// wanted the record, which for `W4-2/1` cost $31.46 and 76 minutes. With this flag the gap is
    /// still printed and the run still blocks at that guard — the caller has simply said they know,
    /// which is the position somebody driving a run to a `--pause-on-approval` stop and supplying
    /// the record by hand is legitimately in.
    #[arg(long)]
    allow_evidence_gap: bool,
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

    // The static pre-flights, both checked before the lock is taken for the same reason: a run
    // that cannot spawn its `llm` steps — or that no map step can ever evidence out of — should
    // not own a run id and a lock to find that out.
    if let Some(refusal) = metaharness_preflight(&inputs.map) {
        outln!("{refusal}");
        return Ok(ExitCode::from(1));
    }

    // F-W4.2-4: the other half of `check_run`, and the half that was missing. `check_run` asks
    // whether every kind the map declares is one the protocol knows; this asks whether every kind
    // the *plan* will demand is one some step can produce. Both questions were answerable from the
    // same two documents before `W4-2/1` spent $31.46 and 76 minutes discovering the second one at
    // a guard, six states in.
    let coverage = aep_driver::evidence_coverage(&plan, &inputs.map);
    if !coverage.is_covered() {
        report_evidence_gap(&coverage, &inputs.map_origin, args.allow_evidence_gap);
        if !args.allow_evidence_gap {
            return Ok(ExitCode::from(1));
        }
    }
    for warning in &coverage.warnings {
        // Printed and never blocking. Each of these is a question nobody can answer from documents
        // — who will have produced a record when the step runs, or whether a person will hand one
        // over between runs — and refusing on an undecided question is what invariant 5 forbids.
        outln!("note: {warning}");
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
        inputs.map.workflow.id().to_string(),
        inputs.map.workflow.major().to_string(),
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

    // The same pre-flight `run` does, and a resume needs it just as much: a resume re-takes the
    // lock, so discovering the missing binary mid-step costs a lock and an attempt in the cursor of
    // a run that was already stopped once.
    if let Some(refusal) = metaharness_preflight(&inputs.map) {
        outln!("{refusal}");
        return Ok(ExitCode::from(1));
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
        inputs.map.workflow.id().to_string(),
        inputs.map.workflow.major().to_string(),
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

/// Prints the evidence the plan will demand and no step of the map can produce.
///
/// One line per **kind**, not per requirement: two principles wanting the same missing kind are one
/// thing to fix. Every line names who asked and what stays shut, so the refusal can be navigated to
/// rather than argued with — and the paragraph after it says what to do, because a refusal that does
/// not answer the question it creates is a wall.
fn report_evidence_gap(report: &CoverageReport, origin: &str, allowed: bool) {
    if allowed {
        outln!("{origin} cannot produce evidence this task's plan will demand, and `--allow-evidence-gap` was given:");
    } else {
        outln!("{origin} cannot produce evidence this task's plan will demand:");
    }
    for entry in &report.missing {
        outln!(
            "  - `{}`: demanded by {}, and no step of the map declares it",
            entry.kind.as_str(),
            entry.demanded_by.join("; ")
        );
        if !entry.blocks.is_empty() {
            outln!("      blocks: {}", entry.blocks.join(", "));
        }
    }
    outln!();
    if allowed {
        outln!(
            "the run will walk every state before the guard that wants these and stop there. That \
             is the cost the flag accepts; nothing about the guard itself has changed."
        );
        return;
    }
    outln!(
        "no run under this map can reach `evidence.missing == 0`, so it would walk every state \
         before that guard and stop at it. Three ways forward: add a `command` step whose \
         `evidence:` declares the kind — one outside the driver's mintable set needs `record: \
         <path>` and a verifier that writes the document, the way this repository's `checks` map \
         mints `trace_conformance`; drive the task under a map that has one; or, if the record \
         will arrive from outside the run, pass `--allow-evidence-gap` and accept that the run \
         stops at the guard."
    );
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
    /// The workflow the run resolved to, for the frame the `metaharness` executor writes.
    workflow_id: String,
    /// Its pinned major version, as the step map states it.
    workflow_version: String,
}

impl CliExecutors {
    /// Builds the executors for one run.
    fn new(
        working_directory: PathBuf,
        run_directory: PathBuf,
        plugin_dirs: Vec<PathBuf>,
        workflow_id: String,
        workflow_version: String,
    ) -> Self {
        Self {
            working_directory,
            run_directory,
            plugin_dirs,
            workflow_id,
            workflow_version,
        }
    }

    /// The step's sealed frame document, written beside the transcript it governs.
    fn write_frame_document(
        &self,
        context: &StepContext<'_>,
        transcripts: &Path,
    ) -> Result<PathBuf, String> {
        let frame = metaharness_frame(context, &self.workflow_id, &self.workflow_version);
        let path = transcripts.join(format!(
            "{}-{}-{}.frame.json",
            context.state, context.index, context.attempt
        ));
        let document = serde_json::to_string_pretty(&frame)
            .map_err(|error| format!("the frame would not serialise: {error}"))?;
        fs::write(&path, format!("{document}\n"))
            .map_err(|error| format!("cannot write {}: {error}", path.display()))?;
        Ok(path)
    }

    /// The one `llm` executor: the vendor is driven through the metaharness seam, in ask mode.
    ///
    /// The step's surface travels twice, deliberately (F9's "both halves"): the sealed
    /// `metaharness.frame/1` document pins what the step *is*, and this process answers every
    /// `tool.requested` event at decision time through [`decide_tool`] — the two retired shell
    /// hooks, ported, plus the per-state allowlist that used to ride on `--allowedTools` — and then
    /// through the **engine**, which is what `authorize` is. The decisions and denials arrive as
    /// `tool.decided` events in the event stream this executor writes as the transcript, never in a
    /// side-channel log a forgotten flag can silence: run `W4-2` lost all eight of its post-fix
    /// sessions to exactly that, a resume that dropped `--plugin-dir` and ran unenforced while
    /// looking clean.
    fn run_llm_metaharness(
        &mut self,
        step: &LlmStep,
        context: &StepContext<'_>,
        authorize: StepAuthorizer<'_>,
    ) -> StepOutcome {
        let transcripts = self.run_directory.join(TRANSCRIPTS);
        if let Err(error) = fs::create_dir_all(&transcripts) {
            return StepOutcome::NoVerdict {
                reason: format!(
                    "cannot write transcripts to {}: {error}",
                    transcripts.display()
                ),
            };
        }
        let transcript = transcript_path(
            self.run_directory.as_path(),
            context.state,
            context.index,
            context.attempt,
        );

        let frame_file = match self.write_frame_document(context, &transcripts) {
            Ok(path) => path,
            Err(reason) => return StepOutcome::NoVerdict { reason },
        };

        let argv = metaharness_argv(
            &frame_file,
            &self.working_directory,
            &self.plugin_dirs,
            &prompt_for(step, context),
        );
        // No `current_dir`: the working directory travels as `--cwd` and metaharness spawns the
        // vendor there itself, with a constructed environment nothing here needs to reach into.
        let spawned = Process::new(&argv[0])
            .args(&argv[1..])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn();
        let mut child = match spawned {
            Ok(child) => child,
            Err(error) => {
                return StepOutcome::NoVerdict {
                    reason: format!("`{}` could not be run: {error}", argv.join(" ")),
                }
            }
        };
        let mut commands = child.stdin.take().expect("stdin was piped");
        let events = child.stdout.take().expect("stdout was piped");
        let stderr = child.stderr.take().expect("stderr was piped");
        // Drained on its own thread: a child blocked writing a full stderr pipe while this loop
        // blocks reading stdout is a deadlock, not a slow run.
        let stderr_thread = std::thread::spawn(move || {
            let mut text = String::new();
            let _ = std::io::Read::read_to_string(&mut std::io::BufReader::new(stderr), &mut text);
            text
        });

        let mut transcript_file = match fs::File::create(&transcript) {
            Ok(file) => file,
            Err(error) => {
                let _ = child.kill();
                return StepOutcome::NoVerdict {
                    reason: format!("cannot write {}: {error}", transcript.display()),
                };
            }
        };
        answer_events(
            context,
            events,
            &mut commands,
            &mut transcript_file,
            authorize,
        );
        drop(commands);
        let status = child.wait();
        let stderr_text = stderr_thread.join().unwrap_or_default();

        match status {
            Ok(status) if status.success() => {
                // An `llm` step never carries evidence, and the type is what makes that true.
                // What the model achieved that is checkable is observed by the command step
                // after it.
                StepOutcome::Nothing
            }
            Ok(status) => {
                let tail: String = stderr_text
                    .lines()
                    .rev()
                    .take(3)
                    .collect::<Vec<_>>()
                    .join(" | ");
                StepOutcome::NoVerdict {
                    reason: format!(
                        "metaharness exited {}; {}the event stream is at {}",
                        status
                            .code()
                            .map_or_else(|| "on a signal".to_owned(), |code| code.to_string()),
                        if tail.is_empty() {
                            String::new()
                        } else {
                            format!("it said: {tail}; ")
                        },
                        transcript.display()
                    ),
                }
            }
            Err(error) => StepOutcome::NoVerdict {
                reason: format!("waiting on metaharness failed: {error}"),
            },
        }
    }
}

impl CommandStepExecutor for CliExecutors {
    fn run_command(&mut self, step: &CommandStep, context: &StepContext<'_>) -> StepOutcome {
        // Expanded before anything is spawned, and a placeholder that cannot be filled is D5's
        // `Unknown`: a command line carrying the literal characters `{transcript}` would run, fail
        // to open that file and be recorded as a verdict about the subject.
        let words: Vec<String> = match step.run.iter().map(|word| expand(word, context)).collect() {
            Ok(words) => words,
            Err(reason) => return StepOutcome::NoVerdict { reason },
        };
        let rendered = words.join(" ");
        let outcome = Process::new(&words[0])
            .args(&words[1..])
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

        // A verifier that wrote its own record: read what it wrote. The exit status is not
        // consulted at all — `protocol trace evidence` exits 0 on a run that gapped, because the
        // verdict is in the document and the engine is what decides on it.
        if let Some(mapping) = &step.evidence {
            if let Some(record) = &mapping.record {
                return read_record(record, mapping, &rendered, context);
            }
        }

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
    fn run_llm(
        &mut self,
        step: &LlmStep,
        context: &StepContext<'_>,
        authorize: StepAuthorizer<'_>,
    ) -> StepOutcome {
        // The seam § 4.9 point 3 names, and the reason it is a name rather than a trait: a
        // second harness is a second executor selected by this string. Since
        // `epic:metaharness-migration` there is no bare-argv path left to select — `claude-code`
        // names the vendor, and the vendor is only ever driven through the metaharness seam.
        // `metaharness` stays accepted as the name the executor first landed under.
        if step.harness != LlmStep::DEFAULT_HARNESS && step.harness != METAHARNESS_HARNESS {
            return StepOutcome::NoVerdict {
                reason: format!(
                    "the step names harness `{}`, and this build only invokes `{}` and `{}`",
                    step.harness,
                    LlmStep::DEFAULT_HARNESS,
                    METAHARNESS_HARNESS
                ),
            };
        }
        self.run_llm_metaharness(step, context, authorize)
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
    // The other half of the same question, and the half no step was ever told: what the state is
    // trying to *reach*. Under its own heading rather than merged into the list above, because the
    // two are different obligations — one is owed while here, the other is owed before the run may
    // leave — and a step that cannot tell them apart cannot tell which one it is being refused on.
    if !context.reaching.is_empty() {
        prompt.push_str(
            "\nWhat this state is trying to reach, one line per requirement that does not hold yet \
             on the way out:\n",
        );
        for line in context.reaching {
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

/// One vendor tool call as the `ActionRequest` the engine decides on, or nothing when no honest one
/// exists.
///
/// **The reverse direction of [`allowed_tools`], and it lives beside it for that reason** — the
/// answer to `story:metaharness-executor`'s open question. `allowed_tools` renders *capability →
/// tool names*; this renders *one call → the action it is*. Neither decides anything: the protocol
/// owns which capability an action needs (`Action::required_capability`), and a table here that
/// tried to be clever would be a second, weaker policy.
///
/// | tool | action | capability it therefore needs |
/// |---|---|---|
/// | `Read` | `repository.read` of the named file | `repository.read` |
/// | `Glob`, `Grep` | `repository.read` of the searched directory | `repository.read` |
/// | `Edit`, `Write` | `repository.write` of the named file | `repository.write` |
/// | `NotebookEdit` | `repository.write` of the named notebook | `repository.write` |
/// | `Bash` | `command.execute` of the program and its arguments | `command.execute` |
/// | `WebFetch` | a reading network request to the named URL | `network.read` |
///
/// **Two offered tools deliberately return `None`, and the engine is not consulted about them:**
///
/// * `Skill` — it loads instructions and takes no action. It is a named exemption in
///   [`allowed_tools`] for the same reason, and everything it *causes* is a subsequent, governed
///   call that arrives here on its own.
/// * `WebSearch` — a search names no URL, and a `NetworkRequest` carrying a query string in its
///   `url` field would state a destination nobody requested. The capability layer still gates it:
///   the tool is only offered when `network.read` is admitted.
///
/// Everything else — `Task` above all — never reaches this function, because [`decide_tool`] has
/// already refused a tool the state does not offer.
///
/// One disagreement is worth naming rather than discovering: [`allowed_tools`] offers `Read`,
/// `Glob` and `Grep` when **either** `repository.read` **or** `artifact.read` is admitted, and this
/// renders all three as a repository read. A state admitting only `artifact.read` therefore has the
/// engine refuse what the rendering table offered — and the engine wins, which is the right way
/// round: reading a file is a repository read whatever tool asked for it.
fn action_for(tool: &str, input: &serde_json::Value) -> Option<ActionRequest> {
    /// Every path a payload names under `keys`, in the order the keys are given.
    fn paths(input: &serde_json::Value, keys: &[&str]) -> Vec<String> {
        keys.iter()
            .filter_map(|key| input[*key].as_str())
            .map(ToOwned::to_owned)
            .collect()
    }

    let action = match tool {
        "Read" => Action::RepositoryRead(RepositoryRead {
            paths: paths(input, &["file_path"]),
        }),
        // A search with no `path` is a search of the working directory, which is what it is
        // recorded as rather than as a read of nothing.
        "Glob" | "Grep" => Action::RepositoryRead(RepositoryRead {
            paths: match paths(input, &["path"]) {
                empty if empty.is_empty() => vec![".".to_owned()],
                named => named,
            },
        }),
        "Edit" | "Write" => Action::RepositoryWrite(RepositoryWrite {
            paths: paths(input, &["file_path"]),
            intent: None,
        }),
        "NotebookEdit" => Action::RepositoryWrite(RepositoryWrite {
            paths: paths(input, &["notebook_path"]),
            intent: None,
        }),
        // Splitting on whitespace is honest **here and only here**: `driven_surface` has already
        // refused anything that composes, redirects or substitutes, so what is left is one simple
        // invocation and its arguments.
        "Bash" => {
            let mut words = input["command"]
                .as_str()
                .unwrap_or_default()
                .split_whitespace();
            Action::CommandExecute(CommandExecute {
                program: words.next().unwrap_or_default().to_owned(),
                args: words.map(ToOwned::to_owned).collect(),
            })
        }
        "WebFetch" => Action::NetworkRequest(NetworkRequest {
            url: input["url"].as_str().unwrap_or_default().to_owned(),
            intent: NetworkIntent::Read,
        }),
        _ => return None,
    };
    Some(ActionRequest::new(action))
}

/// The engine's refusal as one line the model can act on.
///
/// The engine's own `DecisionExplanation` is four lines and belongs in a terminal; a `tool.decided`
/// event carries one reason string. Nothing is re-worded — the operation, the capability, the
/// decision, the document that decided and what is missing are all the engine's — and the layer is
/// named, so the event stream says who refused.
fn engine_refusal(decision: &Decision) -> String {
    let rule = decision.reason.as_ref().map_or_else(String::new, |reason| {
        format!(" ({} rule {})", reason.source, reason.rule)
    });
    let missing = if decision.missing.is_empty() {
        String::new()
    } else {
        format!(". Missing: {}", decision.missing.join("; "))
    };
    format!(
        "the engine refuses this call: `{}` needs the capability `{}`, which is {} in state \
         `{}`{rule}{missing}",
        decision.operation, decision.capability, decision.decision, decision.current_state
    )
}

/// The session loop: every event line into the transcript, every decision back down stdin.
///
/// A free function of its streams so the executor stays under its own roof: nothing here knows a
/// process, only a reader of event lines, a writer of command lines, and the engine.
///
/// # Two layers, in this order, and the reason it is this one
///
/// 1. **[`decide_tool`]** — the ported hooks and the per-state allowlist. It runs first because it
///    is the only layer that sees a call's *arguments*: `protocol artifact list | tee out` and
///    `protocol artifact list` need the same capability and are not the same act, and no
///    `ActionRequest` can express the difference.
/// 2. **the engine** — [`action_for`] renders the call as an `ActionRequest` and `authorize`
///    decides. Asked only about calls layer 1 admitted, so a refusal is attributed to the layer
///    that took it rather than to both, and **the engine's deny wins**: the two layers read the
///    same effective policy, so a disagreement means the rendering table is looser than the
///    protocol, and the protocol is what governs.
///
/// Every reason names its layer, because the event stream is where a person finds out who refused.
fn answer_events(
    context: &StepContext<'_>,
    events: impl std::io::Read,
    commands: &mut impl std::io::Write,
    transcript: &mut impl std::io::Write,
    authorize: StepAuthorizer<'_>,
) {
    for line in std::io::BufRead::lines(std::io::BufReader::new(events)) {
        let Ok(line) = line else { break };
        let _ = transcript
            .write_all(line.as_bytes())
            .and_then(|()| transcript.write_all(b"\n"));
        let Ok(event) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };
        if event["event"] == "tool.requested" && event["decision_required"] == true {
            let call_id = event["call_id"].as_str().unwrap_or_default();
            let name = event["name"].as_str().unwrap_or_default();
            let deny = |reason: String| serde_json::json!({ "decision": "deny", "reason": reason });
            let decision = match decide_tool(context, name, &event["input"]) {
                Err(reason) => deny(format!("the driver's per-call policy refuses: {reason}")),
                // Nothing renders this call as an action — `Skill` and `WebSearch` are the two, and
                // [`action_for`] says why — so the engine is not consulted and the policy's allow
                // stands. Inventing a request would put an act nobody performed in the engine's
                // record, which is invariant 7's failure one layer up.
                Ok(()) => match action_for(name, &event["input"]) {
                    None => serde_json::json!({ "decision": "allow" }),
                    Some(request) => {
                        let verdict = authorize(&request);
                        if verdict.is_allowed() {
                            serde_json::json!({ "decision": "allow" })
                        } else {
                            deny(engine_refusal(&verdict))
                        }
                    }
                },
            };
            let command = serde_json::json!({
                "format": "metaharness.command/1",
                "id": format!("decide-{call_id}"),
                "command": "tool.decide",
                "call_id": call_id,
                "decision": decision,
            });
            // A write that fails means the child is gone; the caller's wait reports how.
            if commands
                .write_all(format!("{command}\n").as_bytes())
                .and_then(|()| commands.flush())
                .is_err()
            {
                break;
            }
        }
    }
}

/// The per-call policy: the retired shell hooks, in the driver's own process.
///
/// This is the § 10.1 shape the hooks existed to approximate: the layer that sees a call's
/// *arguments* is the embedder, in Rust, and its verdict reaches the child through the
/// metaharness seam before the call runs. Three checks, first refusal wins, every reason written
/// for the model to act on rather than as a wall:
///
/// 1. **the driven surface** (`Bash`): one simple `protocol artifact|trace` invocation — no
///    pipes, no redirection, no substitution — and no shell at all in a state that does not
///    admit `command.execute`;
/// 2. **the per-state allowlist**: the tool must render from a capability this state admits,
///    which is what `--allowedTools` used to carry (and can no longer, because a bare
///    `--allowedTools` entry auto-approves the whole tool before any seam is consulted);
/// 3. **store integrity** (`Edit`/`Write`/`NotebookEdit`): the planning store's frontmatter is
///    the `protocol` CLI's, in every state of every workflow.
fn decide_tool(
    context: &StepContext<'_>,
    tool: &str,
    input: &serde_json::Value,
) -> Result<(), String> {
    if tool == "Bash" {
        return driven_surface(context, input);
    }
    let offered = allowed_tools(context.tools);
    if !offered.iter().any(|name| name == tool) {
        return Err(format!(
            "`{tool}` is not offered in state `{}`; this state's tools are: {}",
            context.state,
            offered.join(", ")
        ));
    }
    match tool {
        "Edit" | "Write" | "NotebookEdit" => store_integrity(tool, input),
        _ => Ok(()),
    }
}

/// Guardrail 1, made mechanical: the planning store's frontmatter is the CLI's.
///
/// `Write` and `NotebookEdit` replace whole files and are denied under the store outright; an
/// `Edit` is allowed only when neither string touches the `---` fence or a machine-owned key —
/// decidable from the payload alone, which is exactly what the retired `store-integrity.sh`
/// decided. The audit that does not depend on this firing is `protocol artifact validate`.
fn store_integrity(tool: &str, input: &serde_json::Value) -> Result<(), String> {
    let target = match tool {
        "NotebookEdit" => input["notebook_path"].as_str().unwrap_or_default(),
        _ => input["file_path"].as_str().unwrap_or_default(),
    };
    if !target.contains(".engineering/planning/") {
        return Ok(());
    }
    if tool == "Write" || tool == "NotebookEdit" {
        return Err(format!(
            "`{tool}` replaces the whole of {target}, and the planning store's frontmatter is \
             owned by the `protocol` CLI. Write the body with a targeted `Edit` below the \
             closing `---`, and change frontmatter through `protocol artifact` — `new`, `move`, \
             `relate`. A hand-retyped frontmatter is indistinguishable from a silently-altered \
             one."
        ));
    }
    for field in ["old_string", "new_string"] {
        let Some(value) = input[field].as_str() else {
            continue;
        };
        for line in value.lines() {
            let trimmed = line.trim();
            if trimmed == "---" {
                return Err(format!(
                    "the edit's `{field}` crosses the `---` frontmatter fence of {target}. Edit \
                     only below the closing fence; the frontmatter is the CLI's."
                ));
            }
            if let Some(key) = machine_owned_key(trimmed) {
                return Err(format!(
                    "the edit's `{field}` writes the machine-owned field `{key}` of {target}. \
                     `status` moves only through `protocol artifact move`, which validates the \
                     move against the kind's lifecycle; `id`, `kind`, `revision`, `relations` \
                     and `format` are written by `protocol artifact new` and `protocol artifact \
                     relate`. A hand-edited status is an unvalidated one."
                ));
            }
        }
    }
    Ok(())
}

/// The machine-owned frontmatter key a line writes, if it writes one.
fn machine_owned_key(line: &str) -> Option<&'static str> {
    for key in ["id", "kind", "status", "revision", "relations", "format"] {
        if let Some(rest) = line.strip_prefix(key) {
            if rest.trim_start().starts_with(':') {
                return Some(key);
            }
        }
    }
    None
}

/// The per-state shell surface: one simple invocation of `protocol artifact …` or
/// `protocol trace …`, exactly what the retired `driven-surface.sh` held the grant to.
///
/// The surface lives here and not in any document the run can reach, deliberately: a run that
/// could name its own allowed surface could widen it. Pattern-based and best-effort, as § 4.8
/// says — granting `command.execute` grants a superset of the shell's reach, and this narrows it.
fn driven_surface(context: &StepContext<'_>, input: &serde_json::Value) -> Result<(), String> {
    if !context.tools.shell_offered() {
        return Err(format!(
            "state `{}` does not admit `command.execute`, so this step holds no shell. Anything \
             a suite must observe is run by the driver as a `command` step and recorded with a \
             verifier's provenance, not with yours.",
            context.state
        ));
    }
    let command = input["command"].as_str().unwrap_or_default();
    if command.contains("$(")
        || command
            .chars()
            .any(|c| matches!(c, ';' | '&' | '|' | '`' | '>' | '<' | '\n'))
    {
        return Err(format!(
            "the command composes or redirects, and this run admits one simple invocation at a \
             time: `{command}`. Run the `protocol` verbs one call per Bash tool use."
        ));
    }
    let mut words = command.split_whitespace();
    let program = words.next().unwrap_or_default();
    let verb = words.next().unwrap_or_default();
    if program.rsplit('/').next().unwrap_or(program) != "protocol" {
        return Err(format!(
            "`{}` is outside the surface this state admits. A driven step's shell exists so the \
             `protocol` CLI is reachable; it is not a general shell. Build, test and inspection \
             commands are `command` steps the driver runs, and their records carry a verifier's \
             provenance rather than yours.",
            if program.is_empty() {
                "(nothing)"
            } else {
                program
            }
        ));
    }
    if verb != "artifact" && verb != "trace" {
        return Err(format!(
            "`protocol {}` is outside the surface this state admits: `protocol artifact …` and \
             `protocol trace …`. Driving a run from inside a driven step, or moving the store's \
             own governing documents, is not this step's business.",
            if verb.is_empty() { "(no verb)" } else { verb }
        ));
    }
    Ok(())
}

/// The harness name that selects the metaharness executor.
const METAHARNESS_HARNESS: &str = "metaharness";

/// The binary every `llm` step is spawned through.
const METAHARNESS_BINARY: &str = "metaharness";

/// Refuses a run before it is allocated when the seam's binary is not installed.
///
/// **A launch-time check for a launch-time fact.** Without it the missing binary is discovered at
/// the first `llm` step, as a [`StepOutcome::NoVerdict`] — by which point the run has a directory,
/// an id, the store lock and a snapshot, and the report says *no verdict* for something that was
/// never a verdict: nothing was observed because nothing was ever run. `NoVerdict` is D5's
/// `Unknown` and this is not unknown, it is decidable from `PATH` before a cent or a lock is spent.
///
/// Scoped to maps that have an `llm` step, because that is the only kind of step that spawns it: a
/// map of `command` and `operator` steps drives correctly on a machine with no vendor and no
/// metaharness, and refusing that run would be refusing work the driver can do.
///
/// The refusal answers the question it creates, which is this repository's posture for every
/// refusal — it names the one command that installs the binary.
fn metaharness_preflight(map: &StepMap) -> Option<String> {
    let llm_steps = map
        .states
        .values()
        .flat_map(|state| state.steps.iter())
        .filter(|step| matches!(step, Step::Llm(_)))
        .count();
    if llm_steps == 0 || on_path(METAHARNESS_BINARY) {
        return None;
    }
    Some(format!(
        "this map has {llm_steps} `llm` step(s) and `{METAHARNESS_BINARY}` is not on PATH.\n\
         \n\
         Every `llm` step is spawned through `{METAHARNESS_BINARY} run claude --decisions ask`: the \
         step's surface travels as a sealed frame document and this process answers every tool call \
         the session makes. There is no path around it — the bare vendor argv was retired with \
         `epic:metaharness-migration`, because a second way to launch a session is a second policy \
         to forget.\n\
         \n\
         Install it with `cargo install --path crates/metaharness-cli` from a metaharness checkout, \
         or drive a map whose steps are all `command` and `operator` steps, which needs neither."
    ))
}

/// Whether `program` is on `PATH` as a file that is there to be executed.
///
/// A lookup and never a spawn: running the binary to find out whether it exists is a side effect in
/// a pre-flight, and a binary that exists and then fails is a different finding — that one is a
/// step with no verdict, which is what the retry budget is for.
fn on_path(program: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|paths| {
        std::env::split_paths(&paths).any(|directory| directory.join(program).is_file())
    })
}

/// The format tag the frame document carries, as the metaharness design § 5.5 spells it.
const METAHARNESS_FRAME_FORMAT: &str = "metaharness.frame/1";

/// The metaharness operations for an admitted capability set.
///
/// The same decisions as [`allowed_tools`], spelled in metaharness's § 5.2 vocabulary instead of
/// the vendor's: the protocol decides what a capability admits, both tables only render it, and
/// `subagent.spawn` is never offered for the same reason `Task` never is.
fn metaharness_operations(config: &ToolConfig) -> Vec<&'static str> {
    let mut operations: Vec<&'static str> = Vec::new();
    if config.admits(&Capability::RepositoryRead) || config.admits(&Capability::ArtifactRead) {
        operations.extend(["file.read", "dir.list", "search"]);
    }
    if config.admits(&Capability::RepositoryWrite) {
        operations.extend(["file.write", "file.edit"]);
    }
    if config.admits(&Capability::NetworkRead) {
        operations.push("web.read");
    }
    if config.shell_offered() {
        operations.push("shell");
    }
    if config.skills_offered() {
        operations.push("skill.load");
    }
    operations.sort_unstable();
    operations.dedup();
    operations
}

/// The step as a sealed `metaharness.frame/1` document.
///
/// Built as plain JSON and sealed by the document's own rule — SHA-256, hex, over the compact
/// serialization with keys sorted at every level (`serde_json`'s default map order) and the
/// `digest` and `format` fields absent — so this binary produces byte-for-byte what metaharness
/// verifies, without linking its crates. The obligations and reaching lines are the engine's own
/// words, verbatim, on the same rule as the prompt: a summary here would be the only place the
/// summary existed.
fn metaharness_frame(
    context: &StepContext<'_>,
    workflow_id: &str,
    workflow_version: &str,
) -> serde_json::Value {
    let line = |text: &String| serde_json::json!({ "text": text, "asked_by": null });
    let mut frame = serde_json::json!({
        "workflow": { "id": workflow_id, "version": workflow_version },
        "node": { "id": context.state.to_string() },
        "step": {
            "workflow": workflow_id,
            "state": context.state.to_string(),
            "index": context.index,
            "attempt": context.attempt,
        },
        "prior": [],
        "obligations": context.requirements.iter().map(line).collect::<Vec<_>>(),
        "reaching": context.reaching.iter().map(line).collect::<Vec<_>>(),
        "next": [],
        "handoff": { "handoff": "none" },
        "operations": metaharness_operations(context.tools)
            .iter()
            .map(|operation| serde_json::json!({ "op": operation }))
            .collect::<Vec<_>>(),
        "entities": null,
    });
    let digest = {
        use sha2::{Digest as _, Sha256};
        let bytes = serde_json::to_vec(&frame).expect("a frame value serialises");
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        format!("{:x}", hasher.finalize())
    };
    let object = frame.as_object_mut().expect("a frame is an object");
    object.insert("digest".into(), digest.into());
    object.insert("format".into(), METAHARNESS_FRAME_FORMAT.into());
    frame
}

/// The `metaharness run claude` invocation for one step.
///
/// `--cwd` is the metaharness a6 declaration: the session works in the governed tree, and
/// metaharness attests the two hermetic rows that costs instead of claiming them. `--decisions
/// frame` makes metaharness the per-call decider from the frame's admitted set. The plugins
/// still travel for their skills; their hooks read a step context this launch does not carry and
/// no-op, which is the intended shape — one policy, one enforcer.
fn metaharness_argv(
    frame: &Path,
    working_directory: &Path,
    plugin_dirs: &[PathBuf],
    prompt: &str,
) -> Vec<String> {
    let mut argv = vec![
        METAHARNESS_BINARY.to_owned(),
        "run".to_owned(),
        "claude".to_owned(),
        "--hermetic".to_owned(),
        "--cwd".to_owned(),
        working_directory.display().to_string(),
        "--frame".to_owned(),
        frame.display().to_string(),
        "--decisions".to_owned(),
        "ask".to_owned(),
        "-p".to_owned(),
        prompt.to_owned(),
    ];
    for directory in plugin_dirs {
        argv.push("--plugin-dir".to_owned());
        argv.push(directory.display().to_string());
    }
    argv
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

/// Reads the record a verifier wrote for itself, and submits what the document says.
///
/// The other half of `mint`, and the reason both exist. `mint` builds a record from an exit status,
/// which is honest for a suite and impossible for a check whose record carries digests and counts:
/// a `trace_conformance` minted from `exit 0` would state a specification digest nobody computed.
/// So a verifier that can write its own record does, and the driver's whole job here is to read it
/// — which is the same thing `protocol evaluate --evidence` does with a file a person points at.
///
/// Three refusals, each of them D5's `Unknown` rather than a failing verdict:
///
/// * **no document** — the program was to write one and did not, so nothing was observed;
/// * **more than one record** — a step establishes one thing, and picking one of several would be
///   the driver choosing what the run is about;
/// * **an approval, or anything a person is recorded as having produced** — invariant 7 at this
///   layer. A run's own step must not be able to hand the engine a human's approval read out of a
///   file; that record enters through a person and `protocol evaluate --evidence`, never here.
fn read_record(
    declared: &str,
    mapping: &EvidenceMapping,
    command: &str,
    context: &StepContext<'_>,
) -> StepOutcome {
    let path = match expand(declared, context) {
        Ok(path) => PathBuf::from(path),
        Err(reason) => return StepOutcome::NoVerdict { reason },
    };
    let no_verdict = |reason: String| StepOutcome::NoVerdict { reason };
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) => {
            return no_verdict(format!(
                "`{command}` was to write a `{}` record at {} and {error}, so nothing was observed",
                mapping.kind.as_str(),
                path.display()
            ))
        }
    };
    let origin = path.display().to_string();
    let inputs = match aep_schema::parse::evidence_list(&text, Some(&origin)) {
        Ok(inputs) => inputs,
        Err(error) => {
            return no_verdict(format!(
                "the record `{command}` wrote does not read: {error}"
            ))
        }
    };
    let held = inputs.len();
    let Some(input) = inputs.into_iter().next().filter(|_| held == 1) else {
        return no_verdict(format!(
            "a step establishes one thing, and the record `{command}` wrote at {} holds {held}",
            path.display()
        ));
    };
    if input.evidence.kind() != mapping.kind {
        return no_verdict(format!(
            "the step declares `{}` and the record `{command}` wrote is a `{}`",
            mapping.kind.as_str(),
            input.evidence.kind().as_str()
        ));
    }
    if matches!(input.evidence, Evidence::Approval(_))
        || matches!(input.producer, Producer::Human { .. })
    {
        return no_verdict(format!(
            "the record at {} is an approval or is recorded as a person's, and a driven step \
             cannot submit one: an approval reaches an execution through a person running \
             `protocol evaluate --evidence`",
            path.display()
        ));
    }
    StepOutcome::Observed(Box::new(crate::submission(input)))
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
    use aep_driver::executor::StepAttempt;

    fn config(capabilities: &[Capability]) -> ToolConfig {
        ToolConfig::new(capabilities.iter().cloned().collect())
    }

    /// The prompt the driver would build for one `llm` step.
    fn prompt_with_skills(skills: &[&str]) -> String {
        let step = LlmStep {
            description: None,
            harness: LlmStep::DEFAULT_HARNESS.to_owned(),
            skills: skills.iter().map(ToString::to_string).collect(),
            prompt: "do the thing".to_owned(),
        };
        let tools = config(&[Capability::RepositoryRead, Capability::CommandExecution]);
        let state: StateId = "specify".parse().expect("a state id");
        let requirements: Vec<String> = Vec::new();
        let reaching: Vec<String> = Vec::new();
        let context = StepContext {
            state: &state,
            index: 0,
            attempt: 1,
            tools: &tools,
            run_directory: Path::new("/runs/T-1/1"),
            requirements: &requirements,
            reaching: &reaching,
            preceding_llm: None,
        };
        prompt_for(&step, &context)
    }

    /// What the step is trying to reach reaches the step, under a heading of its own.
    ///
    /// Run `W4-1/1` spent $8.36 in `establish_verifiers` writing checks the guard out of that state
    /// then refused, because the prompt carried `Evaluation::requirements` — what must hold *while
    /// in* the state — and never `Evaluation::transitions[].requirements`, which is what the state
    /// is trying to reach. The two lines are asserted apart rather than together: a prompt that
    /// merged them would tell a step that its outgoing guard is already in force here, which is a
    /// different instruction.
    #[test]
    fn an_unmet_outgoing_guard_is_named_in_the_prompt_under_the_reaching_heading() {
        let step = LlmStep {
            description: None,
            harness: LlmStep::DEFAULT_HARNESS.to_owned(),
            skills: Vec::new(),
            prompt: "write the checks".to_owned(),
        };
        let tools = config(&[Capability::RepositoryRead]);
        let state: StateId = "establish_verifiers".parse().expect("a state id");
        let requirements = vec!["✓ artifact story (any) [state establish_verifiers]".to_owned()];
        let reaching = vec![
            "-> implement: guard: test.exists".to_owned(),
            "-> implement: ✗ test.first_result == failed [principle test-driven]".to_owned(),
        ];
        let context = StepContext {
            state: &state,
            index: 0,
            attempt: 1,
            tools: &tools,
            run_directory: Path::new("/runs/W4-1/1"),
            requirements: &requirements,
            reaching: &reaching,
            preceding_llm: None,
        };

        let prompt = prompt_for(&step, &context);
        let (held, reached) = prompt
            .split_once("What this state is trying to reach")
            .expect("the reaching lines are under their own heading");
        assert!(
            held.contains("artifact story (any)"),
            "what must hold here stays under its own heading: {prompt}"
        );
        for line in &reaching {
            assert!(
                reached.contains(line.as_str()),
                "`{line}` is what the state is trying to reach and belongs in the prompt: {prompt}"
            );
            assert!(
                !held.contains(line.as_str()),
                "`{line}` guards the way out and must not read as a rule in force here: {prompt}"
            );
        }
    }

    /// A scratch directory under this crate's target directory, named for the test that asked.
    fn scratch(name: &str) -> PathBuf {
        let directory = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/drive-records")
            .join(name);
        std::fs::remove_dir_all(&directory).ok();
        std::fs::create_dir_all(&directory).expect("the scratch directory is writable");
        directory
    }

    /// A `trace_conformance` document of the shape `protocol trace evidence` writes.
    const TRACE_RECORD: &str = "\
- kind: trace_conformance
  specification: driven-eval/honest-step
  spec_digest: c2114acdc5782176f7149da41bf1baab6266305ce77d31f813da9de8f93e7aeb
  transcript_digest: 6522e1ebe318da1e0a604e595ecc9afed1d1041c6e418a1382e4f1600a17640b
  status: passed
  expectations_total: 12
  expectations_gapped: 0
  expectations_unknown: 0
  observed_at: 1787355862391
  producer:
    producer: verifier
    verifier: trace-checker
";

    /// The record a verifier wrote is submitted as the verifier's, with nothing minted here.
    ///
    /// `trace_conformance` is not in `EvidenceMapping::MINTABLE` and must never be: its record
    /// carries a specification digest, a transcript digest and three counts, and an exit status
    /// carries none of them. So the check writes the document and the driver reads it — and the
    /// producer that arrives at the engine is the checker's, not this binary's, which is what makes
    /// the record admissible at all.
    #[test]
    fn a_record_a_verifier_wrote_is_submitted_as_that_verifiers_and_never_minted_here() {
        let directory = scratch("trace");
        let record = directory.join("trace-implement.yaml");
        std::fs::write(&record, TRACE_RECORD).expect("the record is writable");
        let mapping = EvidenceMapping {
            kind: EvidenceKind::TraceConformance,
            verifier: Verifier::TraceChecker,
            suite: None,
            subject: None,
            tool: None,
            record: Some("{run_directory}/trace-implement.yaml".to_owned()),
        };
        let tools = config(&[Capability::RepositoryRead]);
        let state: StateId = "implement".parse().expect("a state id");
        let requirements: Vec<String> = Vec::new();
        let reaching: Vec<String> = Vec::new();
        let context = StepContext {
            state: &state,
            index: 1,
            attempt: 1,
            tools: &tools,
            run_directory: &directory,
            requirements: &requirements,
            reaching: &reaching,
            preceding_llm: Some(StepAttempt {
                index: 0,
                attempt: 1,
            }),
        };

        let outcome = read_record(
            mapping.record.as_deref().expect("a declared record"),
            &mapping,
            "protocol trace evidence",
            &context,
        );
        let StepOutcome::Observed(submission) = outcome else {
            panic!("a record that reads is a verdict: {outcome:?}");
        };
        assert_eq!(
            submission.evidence.kind(),
            EvidenceKind::TraceConformance,
            "what the document says it is, is what is submitted"
        );
        assert!(
            matches!(
                submission.producer,
                Producer::Verifier {
                    verifier: Verifier::TraceChecker
                }
            ),
            "the producer is the checker's own: {:?}",
            submission.producer
        );

        // `{transcript}` is a run-time fact, so a step that names one in a run where no `llm` step
        // has run is D5's `Unknown` rather than a verdict about a file that is not there.
        let empty: Vec<String> = Vec::new();
        let unrun = StepContext {
            state: &state,
            index: 1,
            attempt: 1,
            tools: &tools,
            run_directory: &directory,
            requirements: &empty,
            reaching: &empty,
            preceding_llm: None,
        };
        let outcome = expand("{transcript}", &unrun).expect_err("there is no transcript to name");
        assert!(outcome.contains("transcript"), "{outcome}");
    }

    /// Invariant 7 at the layer a `record:` path opens: a run cannot submit a person's approval.
    ///
    /// The path a step writes to is a path a step can also write *to*, and an approval read out of
    /// a file would unlock a capability gate with a document the run itself could have authored.
    /// The engine's capability check matches on the decision and not on who granted it, so the
    /// refusal has to be here.
    #[test]
    fn an_approval_read_out_of_a_file_is_refused_however_well_formed_it_is() {
        let directory = scratch("approval");
        let record = directory.join("approval.yaml");
        std::fs::write(
            &record,
            "- kind: approval\n  approval: release\n  decision: granted\n  \
             observed_at: 1787355862391\n  producer:\n    producer: human\n    id: a-person\n",
        )
        .expect("the record is writable");
        let mapping = EvidenceMapping {
            kind: EvidenceKind::Approval,
            verifier: Verifier::HumanApproval,
            suite: None,
            subject: None,
            tool: None,
            record: Some("{run_directory}/approval.yaml".to_owned()),
        };
        let tools = config(&[Capability::RepositoryRead]);
        let state: StateId = "review".parse().expect("a state id");
        let empty: Vec<String> = Vec::new();
        let context = StepContext {
            state: &state,
            index: 0,
            attempt: 1,
            tools: &tools,
            run_directory: &directory,
            requirements: &empty,
            reaching: &empty,
            preceding_llm: None,
        };

        let outcome = read_record(
            mapping.record.as_deref().expect("a declared record"),
            &mapping,
            "cat approval.yaml",
            &context,
        );
        let StepOutcome::NoVerdict { reason } = outcome else {
            panic!("an approval read out of a file is refused: {outcome:?}");
        };
        assert!(
            reason.contains("approval"),
            "the refusal says what it refused: {reason}"
        );
    }

    /// A step map's `skills:` list is a request to the model, not a command-line flag.
    ///
    /// The skill reaches the session by being asked for, and the `Skill` tool answers; nothing
    /// about the invocation carries it, which is what keeps a skill list from becoming a second
    /// tool surface.
    #[test]
    fn a_steps_skills_are_asked_for_in_the_prompt() {
        let prompt = prompt_with_skills(&["planning"]);
        assert!(
            prompt.contains("Load the `planning` skill"),
            "the step's skill has to be asked for somewhere: {prompt}"
        );
    }

    // ------------------------------------------------------------ the per-call policy

    /// One context for the policy tests.
    fn policy_context<'a>(state: &'a StateId, tools: &'a ToolConfig) -> StepContext<'a> {
        StepContext {
            state,
            index: 0,
            attempt: 1,
            tools,
            run_directory: Path::new("/runs/T-1/1"),
            requirements: &[],
            reaching: &[],
            preceding_llm: None,
        }
    }

    /// The retired `driven-surface.sh`, case for case: the grant is held to one simple
    /// `protocol artifact|trace` invocation, and a state with no shell says so by name.
    #[test]
    fn the_shell_surface_is_one_simple_protocol_invocation() {
        let state: StateId = "implement".parse().expect("a state id");
        let shell = config(&[Capability::CommandExecution]);
        let context = policy_context(&state, &shell);
        let bash = |command: &str| {
            decide_tool(&context, "Bash", &serde_json::json!({ "command": command }))
        };

        assert!(bash("protocol artifact list").is_ok());
        assert!(bash("protocol trace check t.jsonl").is_ok());
        assert!(bash("/usr/local/bin/protocol artifact list").is_ok());

        assert!(
            bash("protocol artifact list | tee out").is_err(),
            "composition"
        );
        assert!(
            bash("protocol artifact list; rm -rf /").is_err(),
            "chaining"
        );
        assert!(bash("protocol artifact list > out").is_err(), "redirection");
        assert!(bash("protocol artifact $(cat x)").is_err(), "substitution");
        assert!(bash("cargo test").is_err(), "another program");
        assert!(bash("protocol drive run").is_err(), "another verb");
        assert!(bash("").is_err(), "an empty command");

        let no_shell = config(&[Capability::RepositoryRead]);
        let context = policy_context(&state, &no_shell);
        let refusal = decide_tool(
            &context,
            "Bash",
            &serde_json::json!({ "command": "protocol artifact list" }),
        )
        .expect_err("no shell in this state");
        assert!(
            refusal.contains("does not admit `command.execute`"),
            "{refusal}"
        );
    }

    /// The retired `store-integrity.sh`, case for case: whole-file writes under the store are
    /// denied, an `Edit` is denied when it touches the fence or a machine-owned key, and a body
    /// edit below the fence passes.
    #[test]
    fn the_planning_stores_frontmatter_is_the_clis() {
        let state: StateId = "implement".parse().expect("a state id");
        let writing = config(&[Capability::RepositoryWrite, Capability::RepositoryRead]);
        let context = policy_context(&state, &writing);
        let store_file = "/repo/.engineering/planning/story/one.md";

        let write = serde_json::json!({ "file_path": store_file, "content": "x" });
        assert!(decide_tool(&context, "Write", &write).is_err());
        let notebook = serde_json::json!({ "notebook_path": store_file });
        assert!(decide_tool(&context, "NotebookEdit", &notebook).is_err());

        let edit = |old: &str, new: &str| {
            decide_tool(
                &context,
                "Edit",
                &serde_json::json!({ "file_path": store_file, "old_string": old, "new_string": new }),
            )
        };
        assert!(edit("a body sentence", "a better body sentence").is_ok());
        assert!(edit("---", "-- -").is_err(), "the fence");
        assert!(edit("  ---  ", "x").is_err(), "the fence, padded");
        assert!(
            edit("a line", "status: done").is_err(),
            "a machine-owned key"
        );
        assert!(edit("revision: 1", "x").is_err(), "owned key in old_string");
        assert!(
            edit("the status: of things", "x").is_ok(),
            "mid-line mention of a key name is not a frontmatter write"
        );

        let elsewhere = serde_json::json!({ "file_path": "/repo/src/lib.rs", "content": "x" });
        assert!(decide_tool(&context, "Write", &elsewhere).is_ok());
    }

    /// The allowlist that used to ride on `--allowedTools`, now a decision with a reason: a tool
    /// no admitted capability renders to is denied naming the state's actual surface.
    #[test]
    fn a_tool_outside_the_states_surface_is_denied_with_the_surface_named() {
        let state: StateId = "specify".parse().expect("a state id");
        let reading = config(&[Capability::RepositoryRead]);
        let context = policy_context(&state, &reading);

        assert!(decide_tool(&context, "Read", &serde_json::json!({})).is_ok());
        assert!(decide_tool(&context, "Skill", &serde_json::json!({})).is_ok());
        let refusal = decide_tool(&context, "Edit", &serde_json::json!({ "file_path": "/x" }))
            .expect_err("no write capability in this state");
        assert!(
            refusal.contains("not offered in state `specify`"),
            "{refusal}"
        );
        assert!(
            decide_tool(&context, "Task", &serde_json::json!({})).is_err(),
            "a subagent is never offered"
        );
    }

    // ------------------------------------------------------------ the metaharness executor

    /// One `StepContext` for the frame tests, with the engine's lines present.
    fn metaharness_context<'a>(
        state: &'a StateId,
        tools: &'a ToolConfig,
        requirements: &'a [String],
        reaching: &'a [String],
    ) -> StepContext<'a> {
        StepContext {
            state,
            index: 2,
            attempt: 3,
            tools,
            run_directory: Path::new("/runs/T-1/1"),
            requirements,
            reaching,
            preceding_llm: None,
        }
    }

    #[test]
    fn the_metaharness_operations_mirror_the_allowed_tools_decisions() {
        let reading = config(&[Capability::RepositoryRead]);
        assert_eq!(
            metaharness_operations(&reading),
            ["dir.list", "file.read", "search", "skill.load"]
        );
        assert!(!metaharness_operations(&reading).contains(&"shell"));

        let shell = config(&[Capability::CommandExecution]);
        assert!(metaharness_operations(&shell).contains(&"shell"));

        let everything = config(&[
            Capability::RepositoryRead,
            Capability::RepositoryWrite,
            Capability::CommandExecution,
            Capability::NetworkRead,
        ]);
        assert!(
            !metaharness_operations(&everything).contains(&"subagent.spawn"),
            "a subagent's tool set is derived by nothing in these decisions"
        );
    }

    /// The seal is the metaharness § 5.5 rule, reproduced here without its crates: SHA-256 over
    /// the compact key-sorted serialization with `digest` and `format` absent. A document this
    /// test passes is a document metaharness's parser accepts byte-for-byte; one it fails is a
    /// run refused before a cent is spent.
    #[test]
    fn the_frame_document_is_sealed_by_the_rule_metaharness_verifies() {
        let tools = config(&[Capability::RepositoryRead, Capability::CommandExecution]);
        let state: StateId = "implement".parse().expect("a state id");
        let requirements = vec!["the suite is red before the implementation".to_owned()];
        let reaching = vec!["to verify: the suite is green".to_owned()];
        let context = metaharness_context(&state, &tools, &requirements, &reaching);

        let frame = metaharness_frame(&context, "development/default", "1");
        assert_eq!(frame["format"], METAHARNESS_FRAME_FORMAT);

        let mut unsealed = frame.clone();
        let object = unsealed.as_object_mut().expect("an object");
        let stated = object.remove("digest").expect("a digest");
        object.remove("format");
        let recomputed = {
            use sha2::{Digest as _, Sha256};
            let bytes = serde_json::to_vec(&unsealed).expect("serialises");
            let mut hasher = Sha256::new();
            hasher.update(&bytes);
            format!("{:x}", hasher.finalize())
        };
        assert_eq!(stated, serde_json::Value::String(recomputed));
    }

    /// The engine's lines travel verbatim, on the same rule as the prompt: the frame is the only
    /// place they exist for the seam, and a summary here would be the only summary.
    #[test]
    fn the_frame_carries_the_engines_lines_and_the_steps_coordinates() {
        let tools = config(&[Capability::RepositoryRead]);
        let state: StateId = "specify".parse().expect("a state id");
        let requirements = vec!["an approved specification exists".to_owned()];
        let reaching = vec!["to implement: the suite is red".to_owned()];
        let context = metaharness_context(&state, &tools, &requirements, &reaching);

        let frame = metaharness_frame(&context, "development/default", "1");
        assert_eq!(frame["node"]["id"], "specify");
        assert_eq!(frame["step"]["index"], 2);
        assert_eq!(frame["step"]["attempt"], 3);
        assert_eq!(frame["workflow"]["version"], "1");
        assert_eq!(
            frame["obligations"][0]["text"],
            "an approved specification exists"
        );
        assert_eq!(
            frame["reaching"][0]["text"],
            "to implement: the suite is red"
        );
        assert_eq!(frame["handoff"]["handoff"], "none");
        let operations: Vec<&str> = frame["operations"]
            .as_array()
            .expect("a list")
            .iter()
            .map(|entry| entry["op"].as_str().expect("a name"))
            .collect();
        assert_eq!(
            operations,
            ["dir.list", "file.read", "search", "skill.load"]
        );
    }

    #[test]
    fn the_metaharness_argv_drives_the_seam_with_the_declared_directory_and_frame() {
        let argv = metaharness_argv(
            Path::new("/runs/T-1/1/transcripts/implement-2-3.frame.json"),
            Path::new("/operator/repo"),
            &[PathBuf::from("/plugins/claude-code")],
            "do the thing",
        );
        assert_eq!(argv[0], "metaharness");
        assert_eq!(argv[1], "run");
        assert_eq!(argv[2], "claude");
        let has = |flag: &str, value: &str| {
            argv.windows(2)
                .any(|pair| pair[0] == flag && pair[1] == value)
        };
        assert!(has("--cwd", "/operator/repo"));
        assert!(has(
            "--frame",
            "/runs/T-1/1/transcripts/implement-2-3.frame.json"
        ));
        assert!(has("--decisions", "ask"));
        assert!(has("-p", "do the thing"));
        assert!(has("--plugin-dir", "/plugins/claude-code"));
        assert!(argv.contains(&"--hermetic".to_owned()));
    }

    // ------------------------------------------------------------ the engine at decision time

    /// A protocol declaring more than the profile below grants, so a capability can be *known* and
    /// still not be granted — which is the state a `NotGranted` decision needs.
    const AUTHORIZE_PROTOCOL: &str = r"
id: aep
version: 1
title: Test protocol
capabilities: [repository.read, repository.write, command.execute, tests.execute]
evidence_kinds: [test_result, diff, approval]
verifiers: [test-runner, compiler, human-approval]
artifact_kinds: [story]
phases: [implementation]
observables:
  - 'task.**'
  - 'tests.**'
  - 'diff.**'
  - 'artifact.**'
  - 'evidence.**'
  - 'state.**'
  - 'workflow.**'
  - 'approvals.**'
";

    const AUTHORIZE_WORKFLOW: &str = r"
id: test/linear
version: 1
title: Linear
initial: implement
states:
  implement:
    title: Implement
    phases: [implementation]
  complete:
    title: Complete
    terminal: true
    phases: [implementation]
transitions:
  - from: implement
    to: complete
    when: diff.exists
";

    /// The profile that makes the fixture load-bearing: it grants `repository.read` and **not**
    /// `repository.write`, so a state whose rendered surface offers `Edit` is a state where the two
    /// layers disagree and the engine is the one that refuses.
    const AUTHORIZE_PROFILE: &str = r"
id: test.reading
title: Reading only
protocol: aep/1
workflow: test/linear
capabilities:
  allow: [repository.read]
completion:
  - diff.exists
";

    const AUTHORIZE_TASK: &str = r"
id: T-1
kind: feature
objective: drive something
protocol: aep/1
profile: test.reading
";

    /// An engine over those documents, and an execution of that task in `implement`.
    fn authorizing_execution() -> (Engine, aep_engine::execution::Execution) {
        use aep_engine::ProtocolEngine as _;
        let mut registry = Registry::new();
        registry
            .insert_protocol(
                aep_schema::parse::protocol(AUTHORIZE_PROTOCOL, None).expect("the protocol parses"),
            )
            .expect("the protocol is unique");
        registry
            .insert_workflow(
                aep_schema::parse::workflow(AUTHORIZE_WORKFLOW, None).expect("the workflow parses"),
            )
            .expect("the workflow is unique");
        registry
            .insert_profile(
                aep_schema::parse::profile(AUTHORIZE_PROFILE, None).expect("the profile parses"),
            )
            .expect("the profile is unique");
        let engine = Engine::new(registry);
        let execution = engine
            .initialize(aep_schema::parse::task(AUTHORIZE_TASK, None).expect("the task parses"))
            .expect("the task resolves");
        (engine, execution)
    }

    /// One `tool.requested` event line of the shape metaharness writes in ask mode.
    fn requested(tool: &str, input: &serde_json::Value) -> String {
        format!(
            "{}\n",
            serde_json::json!({
                "format": "metaharness.event/1",
                "event": "tool.requested",
                "decision_required": true,
                "call_id": "call-1",
                "name": tool,
                "input": input,
            })
        )
    }

    /// Runs one scripted call through the whole seam and returns the decision written back down
    /// stdin — the same object metaharness reads, never a summary of it.
    fn decide_through_the_seam(
        context: &StepContext<'_>,
        engine: &Engine,
        execution: &mut aep_engine::execution::Execution,
        tool: &str,
        input: &serde_json::Value,
    ) -> serde_json::Value {
        use aep_engine::ProtocolEngine as _;
        let mut commands: Vec<u8> = Vec::new();
        let mut transcript: Vec<u8> = Vec::new();
        {
            let mut authorize =
                |request: &ActionRequest| engine.authorize(&mut *execution, request);
            answer_events(
                context,
                requested(tool, input).as_bytes(),
                &mut commands,
                &mut transcript,
                &mut authorize,
            );
        }
        assert!(
            !transcript.is_empty(),
            "every event line reaches the transcript, decided or not"
        );
        serde_json::from_slice(&commands).expect("one `tool.decide` command line")
    }

    /// Every event the execution recorded, by name.
    fn event_names(execution: &aep_engine::execution::Execution) -> Vec<String> {
        execution
            .events()
            .iter()
            .map(|envelope| envelope.event.name().to_owned())
            .collect()
    }

    /// The gap the guide called *"a decision is in the run's record, not yet in the engine's"*,
    /// closed: the engine refuses the call **and** the refusal is in the execution's own events.
    ///
    /// The fixture reaches the state where the rule is load-bearing before asserting the outcome —
    /// the policy layer is asserted to *allow* this call first, because a test where both layers
    /// refuse would pass whether or not the engine was ever asked.
    #[test]
    fn a_call_the_engine_refuses_is_denied_and_the_refusal_is_in_the_executions_event_record() {
        let (engine, mut execution) = authorizing_execution();
        let state: StateId = "implement".parse().expect("a state id");
        let writing = config(&[Capability::RepositoryRead, Capability::RepositoryWrite]);
        let context = policy_context(&state, &writing);
        let input = serde_json::json!({
            "file_path": "/repo/src/lib.rs",
            "old_string": "a",
            "new_string": "b",
        });
        assert!(
            decide_tool(&context, "Edit", &input).is_ok(),
            "the policy layer admits this call, so the engine is the layer under test"
        );

        let command = decide_through_the_seam(&context, &engine, &mut execution, "Edit", &input);

        assert_eq!(command["command"], "tool.decide");
        assert_eq!(command["call_id"], "call-1");
        assert_eq!(command["decision"]["decision"], "deny");
        let reason = command["decision"]["reason"]
            .as_str()
            .expect("a denial says why");
        assert!(
            reason.contains("the engine refuses this call"),
            "the reason names the layer that refused: {reason}"
        );
        assert!(
            reason.contains("repository.write") && reason.contains("not_granted"),
            "and carries the engine's own words: {reason}"
        );

        let denied = execution
            .events()
            .iter()
            .find(|envelope| envelope.event.name() == "action_denied")
            .expect(
                "the refusal is in the execution's event record, which is what authorize is for",
            );
        let json = serde_json::to_value(&denied.event).expect("the event serialises");
        assert_eq!(json["capability"], "repository.write");
        assert_eq!(json["decision"], "not_granted");
        assert!(
            event_names(&execution).contains(&"action_requested".to_owned()),
            "the request is recorded beside the refusal: {:?}",
            event_names(&execution)
        );
    }

    /// Policy first, and a call it refuses never reaches the engine.
    ///
    /// The order matters in both directions: the argument-level rules are the only layer that can
    /// tell `protocol artifact list` from `cargo test`, and an engine asked about a call the driver
    /// already refused would record an action nobody was allowed to attempt.
    #[test]
    fn a_call_the_policy_refuses_is_attributed_to_the_policy_and_never_reaches_the_engine() {
        let (engine, mut execution) = authorizing_execution();
        let state: StateId = "implement".parse().expect("a state id");
        let shell = config(&[Capability::CommandExecution]);
        let context = policy_context(&state, &shell);
        let input = serde_json::json!({ "command": "cargo test" });

        let command = decide_through_the_seam(&context, &engine, &mut execution, "Bash", &input);

        assert_eq!(command["decision"]["decision"], "deny");
        let reason = command["decision"]["reason"]
            .as_str()
            .expect("a denial says why");
        assert!(
            reason.contains("the driver's per-call policy refuses"),
            "the reason names the layer that refused: {reason}"
        );
        assert!(
            !event_names(&execution).contains(&"action_requested".to_owned()),
            "a call the policy refused is not an action the engine was asked about: {:?}",
            event_names(&execution)
        );
    }

    /// The `None` arm of the table, exercised: a `Skill` load is admitted by the policy and the
    /// engine is not consulted, because no `ActionRequest` describes loading instructions.
    #[test]
    fn a_skill_load_is_admitted_without_the_engine_being_asked_to_invent_an_action() {
        let (engine, mut execution) = authorizing_execution();
        let state: StateId = "implement".parse().expect("a state id");
        let reading = config(&[Capability::RepositoryRead]);
        let context = policy_context(&state, &reading);
        let input = serde_json::json!({ "skill": "planning" });

        let command = decide_through_the_seam(&context, &engine, &mut execution, "Skill", &input);

        assert_eq!(command["decision"]["decision"], "allow");
        assert!(
            !event_names(&execution).contains(&"action_requested".to_owned()),
            "loading instructions is not an action, and the record must not claim one: {:?}",
            event_names(&execution)
        );
    }

    /// The table itself: which tool is which action, and what each therefore needs.
    ///
    /// Asserted as capabilities rather than as variants, because the capability is the only thing
    /// the engine decides on — and asserted on the *payload* too, so a request that reached the
    /// record naming the wrong file would fail here rather than mislead an audit.
    #[test]
    fn each_offered_tool_renders_as_the_action_it_is_and_two_render_as_none() {
        let needs = |tool: &str, input: serde_json::Value| {
            action_for(tool, &input).map(|request| request.required_capability().to_string())
        };
        let read = serde_json::json!({ "file_path": "/repo/src/lib.rs" });
        assert_eq!(needs("Read", read.clone()), Some("repository.read".into()));
        assert_eq!(
            needs("Grep", serde_json::json!({ "pattern": "fn main" })),
            Some("repository.read".into()),
            "a search with no path is a search of the working directory"
        );
        assert_eq!(needs("Edit", read.clone()), Some("repository.write".into()));
        assert_eq!(needs("Write", read), Some("repository.write".into()));
        assert_eq!(
            needs(
                "NotebookEdit",
                serde_json::json!({ "notebook_path": "/n.ipynb" })
            ),
            Some("repository.write".into())
        );
        assert_eq!(
            needs(
                "Bash",
                serde_json::json!({ "command": "protocol artifact list" })
            ),
            Some("command.execute".into())
        );
        assert_eq!(
            needs(
                "WebFetch",
                serde_json::json!({ "url": "https://example.test/" })
            ),
            Some("network.read".into())
        );

        assert!(
            action_for("Skill", &serde_json::json!({ "skill": "planning" })).is_none(),
            "loading instructions takes no action"
        );
        assert!(
            action_for("WebSearch", &serde_json::json!({ "query": "aep" })).is_none(),
            "a search names no URL, and a request stating one nobody asked for is a fiction"
        );

        let request = action_for(
            "Bash",
            &serde_json::json!({ "command": "protocol artifact list --kind story" }),
        )
        .expect("a shell call renders");
        assert_eq!(
            request.action.summary(),
            "run `protocol artifact list --kind story`",
            "what the engine records is the call that was made"
        );
        let request = action_for("Read", &serde_json::json!({ "file_path": "/repo/x.rs" }))
            .expect("a read renders");
        assert_eq!(request.action.summary(), "read /repo/x.rs");
    }

    /// The launch-time refusal, and the one case it must not fire in.
    ///
    /// A map of `command` steps drives on a machine with no metaharness and no vendor, so the check
    /// is scoped to maps that would spawn one. `PATH` is not manipulated here — the assertion is
    /// about what is checked, and the refusal's text is what an operator has to act on.
    #[test]
    fn a_map_with_an_llm_step_is_refused_at_launch_when_the_seams_binary_is_missing() {
        let commands_only = aep_schema::parse::step_map(
            "format: aep.driver-steps/1\nid: test/commands\nworkflow: test/linear/1\n\
             states:\n  implement:\n    steps:\n      - kind: command\n        run: [\"true\"]\n",
            None,
        )
        .expect("the map validates");
        assert!(
            metaharness_preflight(&commands_only).is_none(),
            "a map that spawns no session needs no seam binary, whatever is on PATH"
        );

        let with_llm = aep_schema::parse::step_map(
            "format: aep.driver-steps/1\nid: test/llm\nworkflow: test/linear/1\n\
             states:\n  implement:\n    steps:\n      - kind: llm\n        prompt: do the thing\n",
            None,
        )
        .expect("the map validates");
        match metaharness_preflight(&with_llm) {
            None => assert!(
                on_path(METAHARNESS_BINARY),
                "the only reason to allow an `llm` map is that the binary is installed"
            ),
            Some(refusal) => {
                assert!(
                    refusal.contains("cargo install --path crates/metaharness-cli"),
                    "a refusal answers the question it creates: {refusal}"
                );
                assert!(
                    refusal.contains("not on PATH"),
                    "and says what it found: {refusal}"
                );
            }
        }
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
            record: None,
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
            record: None,
        };
        assert!(
            mint(&diff, false, "git diff", observed_now()).is_none(),
            "a ChangeSet has no form that says no change happened, so the honest answer is to \
             submit nothing"
        );
    }
}
