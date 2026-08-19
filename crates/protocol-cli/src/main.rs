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

use aep_domain::action::{Action, ActionRequest, ProductionMutate};
use aep_domain::artifact::ArtifactGraph;
use aep_domain::capability::Capability;
use aep_domain::task::Task;
use aep_engine::engine::{EvidenceSubmission, ProtocolEngine, TransitionResult};
use aep_engine::{load_tree_report, Engine, Registry};
use anyhow::{bail, Context, Result};
use clap::{Args, Parser, Subcommand, ValueEnum};

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
    /// Print the generated JSON Schemas.
    Schema {
        /// Which schema, by file stem, such as `workflow`. Omitted lists them all.
        name: Option<String>,
    },
    /// Run the conformance suites against a backend.
    Conformance,
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
        Command::Schema { name } => schema(name.as_deref()),
        Command::Conformance => bail!(
            "conformance suites are not implemented yet; they need the command/query contract \
             (docs/design/reconciliation-v0.2.md §4)"
        ),
    }
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
            println!(
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
                println!("valid");
            } else {
                println!("{} problem(s):", problems.len());
                for problem in &problems {
                    println!("  - {problem}");
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
            println!("task        {} ({})", plan.task.id, plan.task.kind);
            println!("objective   {}", plan.task.objective);
            println!("protocol    {}", plan.protocol.reference());
            println!("profile     {}", plan.profile.id);
            println!(
                "workflow    {} (initial: {})",
                plan.workflow.id, plan.workflow.initial
            );
            println!(
                "principles  {}",
                plan.principles
                    .iter()
                    .map(|principle| principle.id.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            if !plan.dropped_principles.is_empty() {
                println!(
                    "dropped     {}",
                    plan.dropped_principles
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(", ")
                );
            }
            println!("obligations {}", plan.obligations.len());
            println!("capabilities");
            for (capability, decision) in plan.capability_summary() {
                // `Display` for the decision writes directly, which ignores a width specifier, so
                // the padding has to happen on an owned string.
                println!("  {:<18} {capability}", decision.to_string());
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
            println!("protocol   {}", protocol.reference());
        }
        for principle in registry.principles() {
            println!("principle  {}  {}", principle.id, principle.title);
        }
        for workflow in registry.workflows() {
            println!("workflow   {}  {}", workflow.id, workflow.title);
        }
        for profile in registry.profiles() {
            println!("profile    {}  {}", profile.id, profile.title);
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
            Format::Text => println!("{explanation}"),
            Format::Yaml | Format::Json => print_serialised(&explanation, args.format)?,
        }
        return Ok(exit_code(decision.is_allowed()));
    }

    let evaluation = engine.evaluate(&execution);
    match args.format {
        Format::Text => {
            println!(
                "state       {} ({})",
                evaluation.state, evaluation.state_title
            );
            if !evaluation.requirements.is_empty() {
                println!("owed here");
                for requirement in &evaluation.requirements {
                    println!("  {}", requirement.line());
                }
            }
            println!("transitions");
            if evaluation.transitions.is_empty() {
                println!("  (none: this state is terminal)");
            }
            for transition in &evaluation.transitions {
                let mark = if transition.permitted {
                    "permitted"
                } else {
                    "blocked"
                };
                println!("  {} -> {} [{mark}]", evaluation.state, transition.to);
                for reason in transition.unmet() {
                    println!("      {reason}");
                }
            }
            println!("{}", engine.explain_completion(&execution));
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
                println!("{:<24} {}", entry.filename, entry.describes);
            }
        }
        Some(name) => {
            let wanted = format!("{name}.schema.json");
            let entry = schemas
                .into_iter()
                .find(|entry| entry.filename == wanted || entry.name == name)
                .with_context(|| format!("no schema is called `{name}`"))?;
            print!("{}", entry.to_json().context("serialising the schema")?);
        }
    }
    Ok(ExitCode::SUCCESS)
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
            print!(
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
            println!(
                "{}",
                serde_json::to_string_pretty(value).context("rendering as JSON")?
            );
        }
        _ => print!(
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
