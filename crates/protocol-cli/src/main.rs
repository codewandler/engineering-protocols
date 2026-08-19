//! Reference CLI for AEP.
//!
//! Every subcommand is a thin shell over the library: the CLI parses arguments, loads documents and
//! renders results. It decides nothing, which is the point — if `protocol evaluate` says a transition
//! is blocked, a harness calling the same engine gets the same answer.
//!
//! Exit codes: `0` success, `1` the documents or the execution say no, `2` bad usage.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use aep_backend_memory::{seed, MemoryBackend};
use aep_contract::consistency::QueryConsistency;
use aep_contract::query::{AuditQuery, EntityQuery, QueryService, RelationQuery};
use aep_contract::testing::block_on;
use aep_domain::action::{Action, ActionRequest, ProductionMutate};
use aep_domain::artifact::ArtifactGraph;
use aep_domain::capability::Capability;
use aep_domain::entity::{ActorRef, EntityId, EntityLocator, EntityRef};
use aep_domain::task::Task;
use aep_domain::time::Timestamp;
use aep_engine::engine::{EvidenceSubmission, ProtocolEngine, TransitionResult};
use aep_engine::{load_tree_report, Engine, Registry};
use anyhow::{bail, Context, Result};
use clap::{Args, Parser, Subcommand, ValueEnum};

/// Who the entity surface seeds as.
///
/// Fixed, and a `service:` actor rather than a person: nobody authorised these writes, the CLI made
/// them to have something to answer about.
const SEED_ACTOR: &str = "service:protocol-cli";

/// When the entity surface seeds.
///
/// Fixed so two runs over the same manifest produce byte-identical output. A wall clock here would
/// make every `--format json` diff noise.
const SEED_AT: Timestamp = Timestamp::EPOCH;

/// Reference CLI for the Agentic Engineering Protocol.
#[derive(Debug, Parser)]
#[command(name = "protocol", about, version, disable_help_subcommand = true)]
struct Cli {
    /// What to do.
    #[command(subcommand)]
    command: Command,
}

/// How to render results.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum Format {
    /// Human-readable lines.
    Text,
    /// YAML, for another tool to read.
    Yaml,
    /// JSON, for another tool to read.
    Json,
}

/// Where the documents are.
#[derive(Debug, Args)]
struct RootArgs {
    /// The document tree to load.
    #[arg(long, default_value = ".", global = true)]
    root: PathBuf,
}

/// The inputs an execution needs.
#[derive(Debug, Args)]
struct ExecutionArgs {
    /// The document tree to load.
    #[arg(long, default_value = ".")]
    root: PathBuf,
    /// The task document.
    #[arg(long)]
    task: PathBuf,
    /// An artifact manifest.
    #[arg(long)]
    artifacts: Option<PathBuf>,
    /// Evidence to submit before evaluating, as a list of submissions.
    #[arg(long)]
    evidence: Vec<PathBuf>,
    /// A snapshot to resume from.
    #[arg(long)]
    state: Option<PathBuf>,
    /// Advance the execution as far as the evidence permits before reporting.
    #[arg(long)]
    advance: bool,
    /// How to render the result.
    #[arg(long, value_enum, default_value_t = Format::Text)]
    format: Format,
}

/// What the entity and audit surface needs in order to answer.
///
/// The manifest is required because the backend is in-memory: without one there is nothing to
/// answer about.
#[derive(Debug, Args)]
struct BackendArgs {
    /// The document tree; a relative `--artifacts` path is resolved against it.
    #[arg(long, default_value = ".")]
    root: PathBuf,
    /// The artifact manifest to seed the in-memory backend from.
    #[arg(long)]
    artifacts: PathBuf,
    /// The organisation the seeded locators live under.
    #[arg(long, default_value = "local")]
    organisation: String,
    /// The space the seeded locators live under.
    #[arg(long, default_value = "manifest")]
    space: String,
    /// How to render the result.
    #[arg(long, value_enum, default_value_t = Format::Text)]
    format: Format,
}

/// The available subcommands.
#[derive(Debug, Subcommand)]
enum Command {
    /// Check that a document tree is structurally and semantically valid.
    Validate {
        /// The document tree to load.
        #[arg(long, default_value = ".")]
        root: PathBuf,
        /// An artifact manifest to validate as well.
        #[arg(long)]
        artifacts: Option<PathBuf>,
        /// How to render the result.
        #[arg(long, value_enum, default_value_t = Format::Text)]
        format: Format,
    },
    /// Resolve a task into an execution plan.
    Resolve(ExecutionArgs),
    /// Show what a protocol, principle, workflow or profile declares.
    Inspect {
        /// The document tree to load.
        #[arg(long, default_value = ".")]
        root: PathBuf,
        /// What to inspect, such as `aep/1`, `test-driven` or `development.standard`.
        reference: Option<String>,
        /// How to render the result.
        #[arg(long, value_enum, default_value_t = Format::Text)]
        format: Format,
    },
    /// Evaluate an execution: what is owed, what is permitted, what is missing.
    Evaluate(ExecutionArgs),
    /// Explain a decision, or why a task is incomplete.
    Explain {
        /// The execution inputs.
        #[command(flatten)]
        execution: ExecutionArgs,
        /// A capability to ask about, such as `production.write`.
        #[arg(long)]
        action: Option<String>,
    },
    /// Ask the reference backend about the entities an artifact manifest describes.
    Entity {
        /// Which question to ask about them.
        #[command(subcommand)]
        command: EntityCommand,
    },
    /// Show the audit trail, oldest first.
    ///
    /// The backend is in-memory, so this run seeds it from `--artifacts` and then reads: what you
    /// see is the seeding itself, not a durable past.
    Audit {
        /// Where the manifest is and how to render.
        #[command(flatten)]
        backend: BackendArgs,
        /// Only records from this activity; the seeding run is `seed-manifest`.
        #[arg(long)]
        correlation: Option<String>,
        /// Only records about one entity, by locator or identity.
        #[arg(long)]
        entity: Option<String>,
        /// Only refused attempts — what something tried to do and was stopped from doing.
        #[arg(long)]
        rejected: bool,
    },
    /// Describe an entity type: what it is, whether it may change, and what may target it.
    ///
    /// This is how a harness asks what a design *is* rather than hard-coding it. The manifest is
    /// still seeded, because the answer comes from the same backend that holds the entities.
    Describe {
        /// Where the manifest is and how to render.
        #[command(flatten)]
        backend: BackendArgs,
        /// The type to describe, such as `aep.design/v1`.
        entity_type: String,
    },
    /// Print the generated JSON Schemas.
    Schema {
        /// Which schema, by file stem, such as `workflow`. Omitted lists them all.
        name: Option<String>,
    },
    /// Run the conformance suites against a backend.
    ///
    /// Runs against the in-memory reference backend. `--inject` deliberately breaks one property, to
    /// show that the suite responsible for it actually fails — a suite that passes everything tells
    /// you nothing.
    Conformance {
        /// How much of the contract to check: core, audited or full.
        #[arg(long, default_value = "full")]
        level: String,
        /// Run one suite by name instead of a whole level.
        #[arg(long)]
        suite: Option<String>,
        /// Break one property on purpose, to see which suite catches it.
        #[arg(long)]
        inject: Option<String>,
        /// How to render the result.
        #[arg(long, value_enum, default_value_t = Format::Text)]
        format: Format,
    },
}

/// The questions the entity surface answers.
///
/// Every one of them seeds an in-memory backend from `--artifacts` and then reads it back. Nothing
/// here is durable: `protocol entity history` shows this run's seeding, and running it again
/// produces the same answer rather than a longer history.
#[derive(Debug, Subcommand)]
enum EntityCommand {
    /// List every entity the manifest seeds, with its type, locator and revision.
    ///
    /// The backend is in-memory: this run seeds it from `--artifacts` and then answers. Every
    /// entity here was created moments ago by this process.
    List {
        /// Where the manifest is and how to render.
        #[command(flatten)]
        backend: BackendArgs,
        /// Only entities of this type, such as `aep.design/v1`.
        #[arg(long = "type")]
        entity_type: Option<String>,
    },
    /// Print one entity, addressed by locator or by identity.
    ///
    /// The backend is in-memory: this run seeds it from `--artifacts` and then answers. Exits 1
    /// when nothing the manifest seeds is addressed by what was asked for.
    Get {
        /// Where the manifest is and how to render.
        #[command(flatten)]
        backend: BackendArgs,
        /// A locator such as `ep://local/manifest/design/passkeys-auth`, or an entity identity.
        reference: String,
    },
    /// Show an entity's revision records, oldest first.
    ///
    /// The backend is in-memory: what this shows is *the seeding*, not a durable past. Every
    /// entity is therefore at revision 1, and running the command again does not lengthen it.
    History {
        /// Where the manifest is and how to render.
        #[command(flatten)]
        backend: BackendArgs,
        /// A locator or an entity identity.
        reference: String,
    },
    /// Show what an entity points at, or — with `--incoming` — what points at it.
    ///
    /// The backend is in-memory: this run seeds it from `--artifacts` and then answers. The edges
    /// are the manifest's own `relations`, stored as relation commands.
    Relations {
        /// Where the manifest is and how to render.
        #[command(flatten)]
        backend: BackendArgs,
        /// A locator or an entity identity.
        reference: String,
        /// Answer "what points at this?" instead.
        #[arg(long)]
        incoming: bool,
    },
}

/// Writes to standard output, treating a closed pipe as a normal end rather than a crash.
///
/// Rust's `println!` panics when the reader goes away, so `protocol inspect | head -3` ends in a
/// stack trace instead of three lines. A consumer that stopped reading is not an error this program
/// has anything to say about, so it exits quietly.
fn write_out(text: &str, newline: bool) {
    use std::io::Write;

    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    let outcome = if newline {
        writeln!(handle, "{text}")
    } else {
        write!(handle, "{text}")
    };
    if let Err(error) = outcome {
        if error.kind() == std::io::ErrorKind::BrokenPipe {
            std::process::exit(0);
        }
        eprintln!("error: cannot write to stdout: {error}");
        std::process::exit(1);
    }
}

/// `println!`, but a closed pipe ends the program quietly.
macro_rules! outln {
    () => { write_out("", true) };
    ($($arg:tt)*) => { write_out(&format!($($arg)*), true) };
}

/// `print!`, but a closed pipe ends the program quietly.
macro_rules! out {
    ($($arg:tt)*) => { write_out(&format!($($arg)*), false) };
}

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(error) => {
            eprintln!("error: {error:#}");
            ExitCode::from(1)
        }
    }
}

/// Runs the CLI, returning the process exit code.
fn run() -> Result<ExitCode> {
    let cli = Cli::parse();
    match cli.command {
        Command::Validate {
            root,
            artifacts,
            format,
        } => validate(&root, artifacts.as_deref(), format),
        Command::Resolve(args) => resolve(&args),
        Command::Inspect {
            root,
            reference,
            format,
        } => inspect(&root, reference.as_deref(), format),
        Command::Evaluate(args) => evaluate(&args, None),
        Command::Explain { execution, action } => evaluate(&execution, action.as_deref()),
        Command::Entity { command } => entity(&command),
        Command::Audit {
            backend,
            correlation,
            entity,
            rejected,
        } => audit(
            &backend,
            correlation.as_deref(),
            entity.as_deref(),
            rejected,
        ),
        Command::Describe {
            backend,
            entity_type,
        } => describe(&backend, &entity_type),
        Command::Schema { name } => schema(name.as_deref()),
        Command::Conformance {
            level,
            suite,
            inject,
            format,
        } => conformance(&level, suite.as_deref(), inject.as_deref(), format),
    }
}

/// `protocol conformance`
fn conformance(
    level: &str,
    suite: Option<&str>,
    inject: Option<&str>,
    format: Format,
) -> Result<ExitCode> {
    use aep_conformance::{FaultyBackend, Level};

    let level = Level::parse(level).with_context(|| {
        format!(
            "`{level}` is not a conformance level; expected one of {}",
            Level::ALL
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        )
    })?;

    let fault = match inject {
        None => None,
        Some(name) => Some(parse_fault(name)?),
    };

    let backend = aep_backend_memory::MemoryBackend::new();
    let report = match fault {
        None => run_conformance(&backend, level, suite)?,
        Some(fault) => {
            let faulty = FaultyBackend::new(backend, fault);
            run_conformance(&faulty, level, suite)?
        }
    };

    match format {
        Format::Text => {
            outln!("{report}");
            if let Some(fault) = fault {
                outln!(
                    "injected fault: {} — expected to be caught by the `{}` suite",
                    fault.describe(),
                    fault.caught_by()
                );
            }
        }
        Format::Yaml | Format::Json => print_serialised(&report, format)?,
    }

    Ok(exit_code(report.passed()))
}

/// Runs a level, or one named suite within it.
fn run_conformance<B: aep_conformance::Backend>(
    backend: &B,
    level: aep_conformance::Level,
    suite: Option<&str>,
) -> Result<aep_conformance::ConformanceReport> {
    match suite {
        None => Ok(aep_conformance::run(backend, level)),
        Some(name) => {
            let report = aep_conformance::run_suite(backend, name).with_context(|| {
                format!(
                    "`{name}` is not a suite; known suites are {}",
                    aep_conformance::suites::all()
                        .iter()
                        .map(|suite| suite.name)
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            })?;
            Ok(aep_conformance::ConformanceReport {
                level,
                suites: vec![report],
            })
        }
    }
}

/// Parses a fault name, such as `replay-applies`.
fn parse_fault(name: &str) -> Result<aep_conformance::Fault> {
    // `replay-applies`, `replay_applies` and `ReplayApplies` all name the same fault; separators are
    // a spelling choice, not part of the name.
    let normalised = name.replace(['-', '_'], "").to_ascii_lowercase();
    aep_conformance::Fault::ALL
        .iter()
        .copied()
        .find(|fault| format!("{fault:?}").to_ascii_lowercase() == normalised)
        .with_context(|| {
            format!(
                "`{name}` is not a fault; known faults are {}",
                aep_conformance::Fault::ALL
                    .iter()
                    .map(|fault| kebab(&format!("{fault:?}")))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })
}

/// Renders a `CamelCase` name in kebab-case, for command-line use.
fn kebab(value: &str) -> String {
    let mut rendered = String::new();
    for (index, character) in value.chars().enumerate() {
        if character.is_ascii_uppercase() {
            if index > 0 {
                rendered.push('-');
            }
            rendered.push(character.to_ascii_lowercase());
        } else {
            rendered.push(character);
        }
    }
    rendered
}

/// `protocol validate`
fn validate(root: &Path, artifacts: Option<&Path>, format: Format) -> Result<ExitCode> {
    let outcome = load_tree_report(root);
    let mut problems: Vec<String> = outcome.failures.iter().map(ToString::to_string).collect();

    if let Some(path) = artifacts {
        let graph = read_artifacts(path)?;
        let lifecycle_errors = graph.validate_lifecycles(outcome.registry.lifecycles());
        problems.extend(lifecycle_errors.as_slice().iter().map(ToString::to_string));
    }

    let summary = Summary {
        files_read: outcome.files_read,
        protocols: outcome.registry.protocols().count(),
        principles: outcome.registry.principles().count(),
        workflows: outcome.registry.workflows().count(),
        profiles: outcome.registry.profiles().count(),
        lifecycles: outcome.registry.lifecycles().len(),
        problems: problems.clone(),
    };

    match format {
        Format::Text => {
            outln!(
                "{} file(s): {} protocol(s), {} principle(s), {} workflow(s), {} profile(s), {} \
                 lifecycle(s)",
                summary.files_read,
                summary.protocols,
                summary.principles,
                summary.workflows,
                summary.profiles,
                summary.lifecycles
            );
            if problems.is_empty() {
                outln!("valid");
            } else {
                outln!("{} problem(s):", problems.len());
                for problem in &problems {
                    outln!("  - {problem}");
                }
            }
        }
        Format::Yaml | Format::Json => print_serialised(&summary, format)?,
    }

    Ok(exit_code(problems.is_empty()))
}

/// What `validate` reports.
#[derive(Debug, serde::Serialize)]
struct Summary {
    files_read: usize,
    protocols: usize,
    principles: usize,
    workflows: usize,
    profiles: usize,
    lifecycles: usize,
    problems: Vec<String>,
}

/// `protocol resolve`
fn resolve(args: &ExecutionArgs) -> Result<ExitCode> {
    let registry = load(&args.root)?;
    let task = read_task(&args.task)?;
    let plan = aep_engine::resolve(&task, &registry)
        .map_err(|errors| anyhow::anyhow!("{errors}"))
        .context("the task cannot be resolved")?;

    match args.format {
        Format::Text => {
            outln!("task        {} ({})", plan.task.id, plan.task.kind);
            outln!("objective   {}", plan.task.objective);
            outln!("protocol    {}", plan.protocol.reference());
            outln!("profile     {}", plan.profile.id);
            outln!(
                "workflow    {} (initial: {})",
                plan.workflow.id,
                plan.workflow.initial
            );
            outln!(
                "principles  {}",
                plan.principles
                    .iter()
                    .map(|principle| principle.id.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            if !plan.dropped_principles.is_empty() {
                outln!(
                    "dropped     {}",
                    plan.dropped_principles
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(", ")
                );
            }
            outln!("obligations {}", plan.obligations.len());
            outln!("capabilities");
            for (capability, decision) in plan.capability_summary() {
                // `Display` for the decision writes directly, which ignores a width specifier, so
                // the padding has to happen on an owned string.
                outln!("  {:<18} {capability}", decision.to_string());
            }
        }
        Format::Yaml | Format::Json => print_serialised(&plan, args.format)?,
    }
    Ok(ExitCode::SUCCESS)
}

/// `protocol inspect`
fn inspect(root: &Path, reference: Option<&str>, format: Format) -> Result<ExitCode> {
    let registry = load(root)?;

    let Some(reference) = reference else {
        for protocol in registry.protocols() {
            outln!("protocol   {}", protocol.reference());
        }
        for principle in registry.principles() {
            outln!("principle  {}  {}", principle.id, principle.title);
        }
        for workflow in registry.workflows() {
            outln!("workflow   {}  {}", workflow.id, workflow.title);
        }
        for profile in registry.profiles() {
            outln!("profile    {}  {}", profile.id, profile.title);
        }
        return Ok(ExitCode::SUCCESS);
    };

    if let Ok(protocol_ref) = reference.parse() {
        if let Ok(protocol) = registry.resolved_protocol(&protocol_ref) {
            return print_document(&protocol, format).map(|()| ExitCode::SUCCESS);
        }
    }
    if let Ok(principle_ref) = reference.parse() {
        if let Some(principle) = registry.principle(&principle_ref) {
            return print_document(principle, format).map(|()| ExitCode::SUCCESS);
        }
    }
    if let Ok(workflow_ref) = reference.parse() {
        if let Some(workflow) = registry.workflow(&workflow_ref) {
            return print_document(workflow, format).map(|()| ExitCode::SUCCESS);
        }
    }
    if let Ok(profile_ref) = reference.parse() {
        if let Ok(profile) = registry.resolved_profile(&profile_ref) {
            return print_document(&profile, format).map(|()| ExitCode::SUCCESS);
        }
    }

    bail!("nothing in {} declares `{reference}`", root.display())
}

/// `protocol evaluate` and `protocol explain`
fn evaluate(args: &ExecutionArgs, action: Option<&str>) -> Result<ExitCode> {
    let registry = load(&args.root)?;
    let task = read_task(&args.task)?;
    let artifacts = match &args.artifacts {
        Some(path) => read_artifacts(path)?,
        None => ArtifactGraph::new(),
    };

    let engine = Engine::new(registry);
    let mut execution = match &args.state {
        Some(path) => {
            let text =
                fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
            let snapshot = serde_yaml::from_str(&text)
                .with_context(|| format!("parsing the snapshot in {}", path.display()))?;
            engine
                .restore(task, artifacts, snapshot)
                .context("restoring the execution")?
        }
        None => engine
            .initialize_with_artifacts(task, artifacts)
            .map_err(|error| anyhow::anyhow!("{error}"))
            .context("initialising the execution")?,
    };

    for path in &args.evidence {
        for submission in read_evidence(path)? {
            engine
                .submit_evidence(&mut execution, submission)
                .map_err(|error| anyhow::anyhow!("{error}"))
                .with_context(|| format!("submitting evidence from {}", path.display()))?;
        }
    }

    if args.advance {
        // Advance until nothing more can move, stopping at the first state seen twice. A workflow
        // with a back-edge — `verify -> implement` in `adp/default` — would otherwise ping-pong
        // until the loop bound, which looks like progress and is not: no evidence arrives in here,
        // so the second visit can only repeat the first.
        let mut seen = vec![execution.state_id().clone()];
        while let Ok(TransitionResult::Moved { to, .. }) = engine.transition(&mut execution) {
            if seen.contains(&to) {
                break;
            }
            seen.push(to);
        }
    }

    if let Some(action) = action {
        let request = action_request(action)?;
        let decision = engine.authorize(&mut execution, &request);
        let explanation = Engine::<aep_engine::SystemClock>::explain_decision(&decision);
        match args.format {
            Format::Text => outln!("{explanation}"),
            Format::Yaml | Format::Json => print_serialised(&explanation, args.format)?,
        }
        return Ok(exit_code(decision.is_allowed()));
    }

    let evaluation = engine.evaluate(&execution);
    match args.format {
        Format::Text => {
            outln!(
                "state       {} ({})",
                evaluation.state,
                evaluation.state_title
            );
            if !evaluation.requirements.is_empty() {
                outln!("owed here");
                for requirement in &evaluation.requirements {
                    outln!("  {}", requirement.line());
                }
            }
            outln!("transitions");
            if evaluation.transitions.is_empty() {
                outln!("  (none: this state is terminal)");
            }
            for transition in &evaluation.transitions {
                let mark = if transition.permitted {
                    "permitted"
                } else {
                    "blocked"
                };
                outln!("  {} -> {} [{mark}]", evaluation.state, transition.to);
                for reason in transition.unmet() {
                    outln!("      {reason}");
                }
            }
            outln!("{}", engine.explain_completion(&execution));
        }
        Format::Yaml | Format::Json => print_serialised(&evaluation, args.format)?,
    }

    // Exit 0: the report was produced. Whether the execution is blocked is in the report, and a
    // harness that wants to branch on it reads `blocked` or `is_complete` from the JSON — a blocked
    // execution is the normal case, not an error.
    Ok(ExitCode::SUCCESS)
}

/// `protocol schema`
fn schema(name: Option<&str>) -> Result<ExitCode> {
    let schemas = aep_schema::generated_schemas();
    match name {
        None => {
            for entry in schemas {
                outln!("{:<24} {}", entry.filename, entry.describes);
            }
        }
        Some(name) => {
            let wanted = format!("{name}.schema.json");
            let entry = schemas
                .into_iter()
                .find(|entry| entry.filename == wanted || entry.name == name)
                .with_context(|| format!("no schema is called `{name}`"))?;
            out!("{}", entry.to_json().context("serialising the schema")?);
        }
    }
    Ok(ExitCode::SUCCESS)
}

/// `protocol entity`
fn entity(command: &EntityCommand) -> Result<ExitCode> {
    match command {
        EntityCommand::List {
            backend,
            entity_type,
        } => entity_list(backend, entity_type.as_deref()),
        EntityCommand::Get { backend, reference } => entity_get(backend, reference),
        EntityCommand::History { backend, reference } => entity_history(backend, reference),
        EntityCommand::Relations {
            backend,
            reference,
            incoming,
        } => entity_relations(backend, reference, *incoming),
    }
}

/// `protocol entity list`
fn entity_list(args: &BackendArgs, entity_type: Option<&str>) -> Result<ExitCode> {
    let backend = seeded(args)?;

    let mut query = EntityQuery::default();
    if let Some(name) = entity_type {
        query.entity_type = Some(name.parse().map_err(|error| anyhow::anyhow!("{error}"))?);
    }
    let page = block_on(backend.query(&query)).map_err(|error| anyhow::anyhow!("{error}"))?;

    match args.format {
        Format::Text => print_table(
            &page
                .items
                .iter()
                .map(|entity| {
                    vec![
                        entity.metadata.id.to_string(),
                        entity.metadata.entity_type.to_string(),
                        entity.metadata.locator.to_string(),
                        format!("r{}", entity.metadata.revision),
                    ]
                })
                .collect::<Vec<_>>(),
        ),
        Format::Yaml | Format::Json => print_serialised(&page.items, args.format)?,
    }
    Ok(ExitCode::SUCCESS)
}

/// `protocol entity get`
fn entity_get(args: &BackendArgs, reference: &str) -> Result<ExitCode> {
    let backend = seeded(args)?;
    let target = resolve_entity(&backend, reference)?;
    let entity = block_on(backend.get(&target, QueryConsistency::Current))
        .map_err(|error| anyhow::anyhow!("{error}"))?;

    match args.format {
        Format::Text => {
            outln!("id         {}", entity.metadata.id);
            outln!("type       {}", entity.metadata.entity_type);
            outln!("locator    {}", entity.metadata.locator);
            outln!("revision   {}", entity.metadata.revision);
            outln!(
                "created    {} by {}",
                entity.metadata.created_at,
                entity.metadata.provenance.created_by
            );
            outln!("body");
            let body = serde_yaml::to_string(&entity.data).context("rendering the body")?;
            for line in body.lines() {
                outln!("  {line}");
            }
        }
        Format::Yaml | Format::Json => print_serialised(&entity, args.format)?,
    }
    Ok(ExitCode::SUCCESS)
}

/// `protocol entity history`
fn entity_history(args: &BackendArgs, reference: &str) -> Result<ExitCode> {
    let backend = seeded(args)?;
    let target = resolve_entity(&backend, reference)?;
    let history = block_on(backend.history(&target)).map_err(|error| anyhow::anyhow!("{error}"))?;

    match args.format {
        Format::Text => print_table(
            &history
                .iter()
                .map(|record| {
                    vec![
                        format!("r{}", record.revision),
                        record.at.to_string(),
                        record.actor.to_string(),
                        record
                            .command_id
                            .as_ref()
                            .map_or_else(|| "-".to_owned(), ToString::to_string),
                    ]
                })
                .collect::<Vec<_>>(),
        ),
        Format::Yaml | Format::Json => print_serialised(&history, args.format)?,
    }
    Ok(ExitCode::SUCCESS)
}

/// `protocol entity relations`
fn entity_relations(args: &BackendArgs, reference: &str, incoming: bool) -> Result<ExitCode> {
    let backend = seeded(args)?;
    let target = resolve_entity(&backend, reference)?;

    let query = if incoming {
        RelationQuery::to(target)
    } else {
        RelationQuery::from(target)
    };
    let page = block_on(backend.relations(&query)).map_err(|error| anyhow::anyhow!("{error}"))?;

    match args.format {
        Format::Text => {
            let mut rows = Vec::new();
            for relation in &page.items {
                // The other end, since one end is what was asked about.
                let other = if incoming {
                    &relation.source
                } else {
                    &relation.target
                };
                rows.push(vec![
                    relation.kind.to_string(),
                    if incoming { "<-" } else { "->" }.to_owned(),
                    other.id.to_string(),
                    locator_of(&backend, other),
                ]);
            }
            print_table(&rows);
        }
        Format::Yaml | Format::Json => print_serialised(&page.items, args.format)?,
    }
    Ok(ExitCode::SUCCESS)
}

/// `protocol audit`
fn audit(
    args: &BackendArgs,
    correlation: Option<&str>,
    entity: Option<&str>,
    rejected: bool,
) -> Result<ExitCode> {
    let backend = seeded(args)?;

    let mut query = AuditQuery {
        rejected_only: rejected,
        ..AuditQuery::default()
    };
    if let Some(correlation) = correlation {
        query.correlation_id = Some(
            correlation
                .parse()
                .map_err(|error| anyhow::anyhow!("{error}"))?,
        );
    }
    if let Some(entity) = entity {
        query.entity = Some(resolve_entity(&backend, entity)?);
    }
    let page = block_on(backend.audit(&query)).map_err(|error| anyhow::anyhow!("{error}"))?;

    match args.format {
        Format::Text => print_table(
            &page
                .items
                .iter()
                .map(|record| {
                    vec![
                        record.audit_id.to_string(),
                        record.kind.as_str().to_owned(),
                        record.occurred_at.to_string(),
                        record.actor.to_string(),
                        record
                            .command_id
                            .as_ref()
                            .map_or_else(|| "-".to_owned(), ToString::to_string),
                        record
                            .subject
                            .as_ref()
                            .map_or_else(|| "-".to_owned(), |subject| subject.id.to_string()),
                    ]
                })
                .collect::<Vec<_>>(),
        ),
        Format::Yaml | Format::Json => print_serialised(&page.items, args.format)?,
    }
    Ok(ExitCode::SUCCESS)
}

/// `protocol describe`
fn describe(args: &BackendArgs, entity_type: &str) -> Result<ExitCode> {
    let backend = seeded(args)?;
    let entity_type = entity_type
        .parse()
        .map_err(|error| anyhow::anyhow!("{error}"))?;
    let descriptor = block_on(backend.describe_type(&entity_type))
        .map_err(|error| anyhow::anyhow!("{error}"))?;

    match args.format {
        Format::Text => {
            outln!("type       {}", descriptor.entity_type);
            outln!("summary    {}", descriptor.summary);
            outln!(
                "mutable    {}",
                if descriptor.mutable { "yes" } else { "no" }
            );
            if !descriptor.commands.is_empty() {
                outln!("commands");
                for command in &descriptor.commands {
                    let guard = if command.revision_guarded {
                        "revision-guarded"
                    } else {
                        "unguarded"
                    };
                    outln!(
                        "  {:<28} {guard:<17} {}",
                        command.command_type,
                        command.summary
                    );
                }
            }
            if !descriptor.relations.is_empty() {
                outln!("relations");
                for relation in &descriptor.relations {
                    outln!("  {}", relation.kind);
                }
            }
        }
        Format::Yaml | Format::Json => print_serialised(&descriptor, args.format)?,
    }
    Ok(ExitCode::SUCCESS)
}

/// Seeds an in-memory backend from the manifest, so there is something to answer about.
///
/// Every invocation starts from nothing: the backend keeps no state between runs, which is why the
/// seeding is visible in the history and the audit trail rather than hidden.
fn seeded(args: &BackendArgs) -> Result<MemoryBackend> {
    let path = if args.artifacts.is_absolute() {
        args.artifacts.clone()
    } else {
        args.root.join(&args.artifacts)
    };
    let graph = read_artifacts(&path)?;
    let actor: ActorRef = SEED_ACTOR
        .parse()
        .map_err(|error| anyhow::anyhow!("{error}"))?;

    let backend = MemoryBackend::new();
    seed::from_manifest(
        &backend,
        &graph,
        &args.organisation,
        &args.space,
        SEED_AT,
        &actor,
    )
    .map_err(|error| anyhow::anyhow!("{error}"))
    .with_context(|| format!("seeding the backend from {}", path.display()))?;
    Ok(backend)
}

/// Resolves a locator or a raw identity to an entity that exists.
///
/// Both spellings are accepted because both are how people arrive: a locator is what an
/// organisation knows a thing by, an identity is what a previous command printed.
fn resolve_entity(backend: &MemoryBackend, reference: &str) -> Result<EntityRef> {
    if let Ok(locator) = reference.parse::<EntityLocator>() {
        return match block_on(backend.resolve(&locator)) {
            Ok(id) => Ok(EntityRef::new(id)),
            Err(_) => bail!("nothing seeded from this manifest is addressed by `{reference}`"),
        };
    }
    if let Ok(id) = reference.parse::<EntityId>() {
        let target = EntityRef::new(id);
        if block_on(backend.get(&target, QueryConsistency::Current)).is_ok() {
            return Ok(target);
        }
        bail!("no entity seeded from this manifest has the identity `{reference}`");
    }
    bail!("`{reference}` is neither a locator (`ep://…`) nor an entity identity")
}

/// The locator an entity is addressed by, for output that names both ends of an edge.
fn locator_of(backend: &MemoryBackend, reference: &EntityRef) -> String {
    block_on(backend.get(reference, QueryConsistency::Current)).map_or_else(
        |_| "-".to_owned(),
        |entity| entity.metadata.locator.to_string(),
    )
}

/// Prints one record per line, in columns wide enough for the widest cell.
///
/// Aligned because the surface exists to be scanned: a reader looking for one design among sixty
/// entities finds it by column position, not by reading every line.
fn print_table(rows: &[Vec<String>]) {
    let mut widths: Vec<usize> = Vec::new();
    for row in rows {
        for (index, cell) in row.iter().enumerate() {
            let width = cell.chars().count();
            match widths.get_mut(index) {
                Some(current) => *current = (*current).max(width),
                None => widths.push(width),
            }
        }
    }

    for row in rows {
        let cells: Vec<String> = row
            .iter()
            .enumerate()
            .map(|(index, cell)| {
                if index + 1 == row.len() {
                    cell.clone()
                } else {
                    format!("{cell:width$}", width = widths[index])
                }
            })
            .collect();
        outln!("{}", cells.join("  "));
    }
}

/// Loads a document tree, failing if anything is wrong with it.
fn load(root: &Path) -> Result<Registry> {
    let outcome = load_tree_report(root);
    if outcome.failures.is_empty() {
        return Ok(outcome.registry);
    }
    let detail = outcome
        .failures
        .iter()
        .map(|failure| format!("  - {failure}"))
        .collect::<Vec<_>>()
        .join("\n");
    bail!(
        "{} document problem(s) in {}:\n{detail}",
        outcome.failures.len(),
        root.display()
    )
}

/// Reads a task document.
fn read_task(path: &Path) -> Result<Task> {
    let text = fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let origin = path.display().to_string();
    aep_schema::parse::task(&text, Some(&origin)).map_err(|error| anyhow::anyhow!("{error}"))
}

/// Reads an artifact manifest.
fn read_artifacts(path: &Path) -> Result<ArtifactGraph> {
    let text = fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let origin = path.display().to_string();
    aep_schema::parse::artifact_manifest(&text, Some(&origin))
        .map_err(|error| anyhow::anyhow!("{error}"))
}

/// Reads a list of evidence submissions.
fn read_evidence(path: &Path) -> Result<Vec<EvidenceSubmission>> {
    let text = fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let origin = path.display().to_string();
    let inputs = aep_schema::parse::evidence_list(&text, Some(&origin))
        .map_err(|error| anyhow::anyhow!("{error}"))?;
    Ok(inputs.into_iter().map(submission).collect())
}

/// Turns a parsed evidence input into a submission.
fn submission(input: aep_schema::parse::EvidenceInput) -> EvidenceSubmission {
    let mut submission = EvidenceSubmission::new(input.evidence, input.producer);
    submission.subject = input.about;
    if let Some(provenance) = input.provenance {
        submission.provenance = provenance;
    }
    submission
}

/// Builds a stand-in action for a capability named on the command line.
///
/// `explain --action` asks about a *capability*, so the CLI wraps it in the simplest action that
/// requires it. The decision depends on the capability, not on the action's details.
fn action_request(capability: &str) -> Result<ActionRequest> {
    let capability: Capability = capability
        .parse()
        .map_err(|error| anyhow::anyhow!("{error}"))?;
    let action = match &capability {
        Capability::RepositoryRead => Action::RepositoryRead(aep_domain::action::RepositoryRead {
            paths: vec![".".to_owned()],
        }),
        Capability::RepositoryWrite => {
            Action::RepositoryWrite(aep_domain::action::RepositoryWrite {
                paths: vec![".".to_owned()],
                intent: None,
            })
        }
        Capability::TestExecution => Action::TestExecute(aep_domain::action::TestExecute {
            suite: aep_domain::evidence::TestSuite::Unit,
            selector: None,
        }),
        Capability::CommandExecution => {
            Action::CommandExecute(aep_domain::action::CommandExecute {
                program: "true".to_owned(),
                args: Vec::new(),
            })
        }
        Capability::NetworkRead | Capability::NetworkWrite => {
            Action::NetworkRequest(aep_domain::action::NetworkRequest {
                url: "https://example.test/".to_owned(),
                intent: if capability == Capability::NetworkWrite {
                    aep_domain::action::NetworkIntent::Write
                } else {
                    aep_domain::action::NetworkIntent::Read
                },
            })
        }
        Capability::TelemetryRead => Action::TelemetryQuery(aep_domain::action::TelemetryQuery {
            query: "up".to_owned(),
            service: None,
        }),
        Capability::ProductionRead | Capability::ProductionWrite => {
            Action::ProductionMutate(ProductionMutate {
                target: "state".to_owned(),
                change: None,
            })
        }
        Capability::Deploy(environment) => Action::Deploy(aep_domain::action::Deploy {
            environment: environment.clone(),
            revision: "HEAD".to_owned(),
            strategy: None,
        }),
        Capability::Rollback(environment) => Action::Rollback(aep_domain::action::Rollback {
            environment: environment.clone(),
            to_revision: None,
        }),
        Capability::SecretRead => Action::SecretRead(aep_domain::action::SecretRead {
            secret: "secret".to_owned(),
        }),
        Capability::ArtifactRead | Capability::ArtifactWrite => {
            Action::ArtifactWrite(aep_domain::action::ArtifactWrite {
                artifact: "design:example"
                    .parse()
                    .map_err(|error| anyhow::anyhow!("{error}"))?,
                kind: aep_domain::artifact::ArtifactKind::Design,
            })
        }
        Capability::ReviewRequest => Action::ReviewRequest(aep_domain::action::ReviewRequest {
            subject: "design:example"
                .parse()
                .map_err(|error| anyhow::anyhow!("{error}"))?,
            reviewer: None,
        }),
        Capability::ApprovalRequest => {
            Action::ApprovalRequest(aep_domain::action::ApprovalRequest {
                approval: "production-change"
                    .parse()
                    .map_err(|error| anyhow::anyhow!("{error}"))?,
                reason: None,
            })
        }
        other => bail!("`{other}` cannot be asked about directly yet"),
    };
    Ok(ActionRequest::new(action))
}

/// Prints a validated document in the requested format.
fn print_document<T: serde::Serialize>(document: &T, format: Format) -> Result<()> {
    match format {
        Format::Text | Format::Yaml => {
            out!(
                "{}",
                serde_yaml::to_string(document).context("rendering the document")?
            );
        }
        Format::Json => print_serialised(document, format)?,
    }
    Ok(())
}

/// Prints a value as YAML or JSON.
fn print_serialised<T: serde::Serialize>(value: &T, format: Format) -> Result<()> {
    match format {
        Format::Json => {
            outln!(
                "{}",
                serde_json::to_string_pretty(value).context("rendering as JSON")?
            );
        }
        _ => out!(
            "{}",
            serde_yaml::to_string(value).context("rendering as YAML")?
        ),
    }
    Ok(())
}

/// `0` when the answer is yes, `1` when it is no.
fn exit_code(ok: bool) -> ExitCode {
    if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}
