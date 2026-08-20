//! Reference CLI for AEP.
//!
//! Every subcommand is a thin shell over the library: the CLI parses arguments, loads documents and
//! renders results. It decides nothing, which is the point — if `protocol evaluate` says a transition
//! is blocked, a harness calling the same engine gets the same answer.
//!
//! Exit codes: `0` success, `1` the documents or the execution say no, `2` bad usage, `3` nobody
//! found out.
//!
//! The fourth is only produced by [`ess conform run`](EssConformCommand::Run), and it exists because
//! collapsing it into `1` would tell a harness that an implementation contradicted its specification
//! when what actually happened is that the run could not be carried out. Those call for different
//! reactions — one is a defect to fix, the other is a target to go and reach — so they are different
//! codes rather than one code and a log line.

use std::collections::BTreeSet;
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
use ess_compiler::diagnostic::Diagnostics;
use ess_compiler::ir::EssIr;
use ess_compiler::source::SourceMap;
use ess_gen::graph::{delivery_word, failure_word, SystemGraph};
use ess_gen::{Artifact, Generator, Provenance};

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

/// The file that makes a directory a specification rather than a directory that holds YAML.
///
/// A specification directory is recognised, not assumed: `--path` defaults to `.`, so without a
/// marker every `protocol ess validate` typed in an ordinary repository would read its CI workflow
/// and its fixtures and call each one a broken specification.
const SPECIFICATION_HEADER: &str = "system.yaml";

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

/// How to render the system graph.
///
/// Its own enum rather than a variant on [`Format`], which every subcommand shares: `protocol
/// validate --format mermaid` and `protocol audit --format mermaid` would parse and mean nothing,
/// and a value a verb cannot honour is worse than a value it does not offer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum GraphFormat {
    /// Graphviz DOT, for `dot -Tsvg`.
    ///
    /// Spelled `dot`, and `text` is still accepted for it. `--format text` was what this verb
    /// called DOT before there was a second diagram to tell it apart from, and a word that no
    /// longer says which of two diagrams you get is a word worth replacing without breaking.
    #[value(alias = "text")]
    Dot,
    /// Mermaid, for a Markdown file, a documentation site or a pull request.
    Mermaid,
    /// YAML, for another tool to read.
    Yaml,
    /// JSON, for another tool to read.
    Json,
}

impl GraphFormat {
    /// How a specification that does not compile is reported when this was asked for.
    ///
    /// A diagnostic is not a diagram: `--format mermaid` on a broken specification wants the reason
    /// in words, not a flowchart of nothing. `--format json` and `--format yaml` keep their shape,
    /// because a tool that parses the graph parses the refusal too.
    fn diagnostics(self) -> Format {
        match self {
            Self::Dot | Self::Mermaid => Format::Text,
            Self::Yaml => Format::Yaml,
            Self::Json => Format::Json,
        }
    }
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
    /// The document tree to load. Inside a project, this comes from `.engineering/project.yaml`.
    #[arg(long)]
    root: Option<PathBuf>,
    /// The task document. Inside a project, this comes from `.engineering/task.yaml`.
    #[arg(long)]
    task: Option<PathBuf>,
    /// An artifact manifest. Inside a project, this comes from `.engineering/artifacts.yaml`.
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
    /// Work with an executable system specification.
    Ess {
        /// What to do with it.
        #[command(subcommand)]
        command: EssCommand,
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
    /// Check a storage backend against the AEP contract suites.
    ///
    /// The question is whether a **backend** implements `aep-contract` — commands, queries, audit,
    /// idempotency, consistency — and the answer is about storage, not about any system you have
    /// specified.
    ///
    /// The other conformance verb answers a different question. `protocol ess conform` asks whether
    /// an **implementation** satisfies an executable system specification: whether `CreateInvoice`
    /// with a negative amount is refused, whether a paid invoice can still be cancelled. Design §42
    /// calls this one contract conformance and that one semantic conformance; neither subsumes the
    /// other, and a backend passing here says nothing about a system passing there.
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
        Command::Ess { command } => match command {
            EssCommand::Validate { path, format } => ess_validate(&path, format),
            EssCommand::Compile { path, format } => ess_compile(&path, format),
            EssCommand::Inspect {
                path,
                name,
                kind,
                format,
            } => ess_inspect(&path, &name, kind, format),
            EssCommand::Generate {
                path,
                kind,
                out,
                format,
            } => ess_generate(&path, kind, out.as_deref(), format),
            EssCommand::Graph { path, format } => ess_graph(&path, format),
            EssCommand::Conform { command } => match command {
                EssConformCommand::Synthesize { path, out, format } => {
                    ess_conform_synthesize(&path, out.as_deref(), format)
                }
                EssConformCommand::Run {
                    path,
                    suite,
                    target,
                    inject,
                    untraced,
                    format,
                } => ess_conform_run(
                    &path,
                    suite.as_deref(),
                    target,
                    inject.as_deref(),
                    untraced,
                    format,
                ),
                EssConformCommand::Evidence {
                    path,
                    suite,
                    target,
                    inject,
                    untraced,
                    out,
                    format,
                } => ess_conform_evidence(
                    &path,
                    suite.as_deref(),
                    target,
                    inject.as_deref(),
                    untraced,
                    out.as_deref(),
                    format,
                ),
            },
        },
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

/// What can be done with a specification.
#[derive(Debug, Subcommand)]
enum EssCommand {
    /// Check that a specification is well formed and internally consistent.
    Validate {
        /// The specification: one file, or a directory holding `system.yaml` and `domains/`.
        #[arg(long, default_value = ".")]
        path: PathBuf,
        /// How to render the result.
        #[arg(long, value_enum, default_value_t = Format::Text)]
        format: Format,
    },
    /// Resolve a specification into its IR, or report every diagnostic.
    ///
    /// `validate` answers "is each declaration locally consistent"; this answers "does every
    /// reference in it resolve, and to what". A diagnostic is structured, so `--format json` hands a
    /// harness the two types and the two document paths as fields rather than as a sentence.
    Compile {
        /// The specification: one file, or a directory holding `system.yaml` and `domains/`.
        #[arg(long, default_value = ".")]
        path: PathBuf,
        /// How to render the result. `json` and `yaml` carry the whole IR.
        #[arg(long, value_enum, default_value_t = Format::Text)]
        format: Format,
    },
    /// Show what one declaration is, with every reference in it resolved.
    Inspect {
        /// The specification: one file, or a directory holding `system.yaml` and `domains/`.
        #[arg(long, default_value = ".")]
        path: PathBuf,
        /// What to look up: `billing.invoice.CreateInvoice`, or an identifier such as
        /// `notify-on-invoice-created`.
        name: String,
        /// Which namespace to look in. Only needed when one name is used in two of them.
        #[arg(long, value_enum)]
        kind: Option<EssKind>,
        /// How to render the result.
        #[arg(long, value_enum, default_value_t = Format::Text)]
        format: Format,
    },
    /// Generate every projection of a specification: documentation, schemas, contracts.
    ///
    /// Read-only unless `--out` is given. Without it the artifacts are listed rather than written,
    /// because a verb that scatters files over a working tree the first time someone tries it is a
    /// verb nobody tries twice; `--format json` carries their contents for a consumer that wants
    /// them without a directory.
    Generate {
        /// The specification: one file, or a directory holding `system.yaml` and `domains/`.
        #[arg(long, default_value = ".")]
        path: PathBuf,
        /// Which projection to generate. Every one of them when this is not given.
        #[arg(long, value_enum)]
        kind: Option<EssProjection>,
        /// Where to write the artifacts. Without it nothing is written and they are listed instead.
        #[arg(long)]
        out: Option<PathBuf>,
        /// How to render the result. `json` and `yaml` carry every artifact's contents.
        #[arg(long, value_enum, default_value_t = Format::Text)]
        format: Format,
    },
    /// Print the actor, event and command graph.
    ///
    /// `dot` is for `dot -Tsvg`, and `text` is still accepted as its old name. `mermaid` is the
    /// same graph as a `flowchart`, unfenced, so it can be redirected into a Markdown file, a
    /// documentation site or a pull request — it is the diagram the generated `docs/README.md`
    /// opens with, from the same renderer. `json` and `yaml` are the nodes and edges themselves,
    /// for a consumer that would otherwise have to parse a diagram to get at them.
    Graph {
        /// The specification: one file, or a directory holding `system.yaml` and `domains/`.
        #[arg(long, default_value = ".")]
        path: PathBuf,
        /// How to render the graph.
        #[arg(long, value_enum, default_value_t = GraphFormat::Dot)]
        format: GraphFormat,
    },
    /// Check an implementation against the suite a specification obliges.
    ///
    /// Not `protocol conformance`. That one asks whether a storage **backend** implements the AEP
    /// contract; this one asks whether an **implementation** satisfies an executable system
    /// specification — design §42's contract conformance against its semantic conformance. The two
    /// share a word and nothing else, so they are spelled apart: `protocol conformance` is the
    /// backend, `protocol ess conform` is the system you wrote a specification for.
    Conform {
        /// Synthesise a suite, or run one.
        #[command(subcommand)]
        command: EssConformCommand,
    },
}

/// The two halves of closing the loop: deriving the suite, and running it.
///
/// Two verbs rather than one, because they take different things and produce different things — a
/// specification in and a suite out, against a suite plus an implementation in and a report out. A
/// single verb switched by a flag would have to accept `--out` and `--target` together and refuse
/// most of the combinations, which is a worse way of saying the same thing.
#[derive(Debug, Subcommand)]
enum EssConformCommand {
    /// Derive the conformance suite a specification obliges, and write it or print it.
    ///
    /// The suite is one JSON document per specification, keyed by scenario id, carrying no handle
    /// into any particular compilation — so a runner in another language can read it, and a fault
    /// matrix can refer to a scenario by a name that does not move when a sibling is added.
    ///
    /// Read-only unless `--out` is given, exactly as `protocol ess generate` is, and for the same
    /// reason: a verb that scatters files over a working tree the first time someone tries it is a
    /// verb nobody tries twice. `--format json` carries the document's bytes for a consumer that
    /// wants them without a directory, which is what `cargo xtask suite --check` reads.
    ///
    /// A construct the specification does not say enough about to test appears as a **refusal**
    /// rather than as a silently thinner suite: which construct, why, and what would have to change.
    /// A suite quietly holding fewer checks than the specification requires is the one failure a
    /// passing run cannot show.
    Synthesize {
        /// The specification: one file, or a directory holding `system.yaml` and `domains/`.
        #[arg(long, default_value = ".")]
        path: PathBuf,
        /// Where to write `suite.json`. Without it nothing is written and the suite is summarised.
        #[arg(long)]
        out: Option<PathBuf>,
        /// How to render the result. `json` and `yaml` carry the suite document itself.
        #[arg(long, value_enum, default_value_t = Format::Text)]
        format: Format,
    },
    /// Run a suite against an implementation and report what it found.
    ///
    /// # What this build can run, and what it cannot
    ///
    /// It can run the two reference implementations that ship inside `ess-conformance`:
    /// `--target billing` is `examples/billing/` written by hand and in memory, and
    /// `--target oracle-fixture` is `examples/oracle-fixture/`. They are here so a person can watch
    /// the loop close — specification, suite, implementation, verdict — in one command.
    ///
    /// **It cannot run yours.** A `ConformanceTarget` is a Rust trait, and this binary can only
    /// reach an implementation it was compiled with; nothing in this build speaks to a target over a
    /// socket, and design §41 keeps transport out of the model deliberately. To hold your own system
    /// to a specification today: depend on `ess-conformance`, implement `ConformanceTarget` for it,
    /// read the committed `suites/generated/<system>/suite.json` with `ConformanceSuite::from_json`
    /// and call `Runner::for_suite(&suite).run(&suite, &target)`. That is the whole adapter — the
    /// suite this verb writes is the same document either way.
    ///
    /// # Exit codes
    ///
    /// `0` every scenario passed. `1` the implementation contradicted the specification, or a
    /// scenario the specification requires is one the target cannot expose — §28 makes that a
    /// failure and not a skip. `3` nothing contradicted the specification and at least one scenario
    /// could not be executed, which is a target to go and reach rather than a defect to fix.
    Run {
        /// The specification to synthesise the suite from.
        #[arg(long, default_value = ".")]
        path: PathBuf,
        /// A written suite to run instead, such as `suites/generated/billing/suite.json`.
        #[arg(long, conflicts_with = "path")]
        suite: Option<PathBuf>,
        /// Which built-in reference implementation to run against.
        #[arg(long, value_enum)]
        target: EssTarget,
        /// Break one property on purpose, to see which scenario catches it.
        #[arg(long)]
        inject: Option<String>,
        /// Hide the one observation §16 refuses to require of every implementation.
        ///
        /// The same implementation, unable to say which command a binding invoked. It is not a
        /// fault — a system that answers every semantic question and cannot trace its own
        /// invocations is a legitimate thing to build — and the run still fails, with
        /// `<binding>/binding/mapping` reported `unsupported` rather than passed. That is §28's
        /// fourth word doing the only job it has: a check the target cannot make is not a check
        /// that passed.
        #[arg(long)]
        untraced: bool,
        /// How to render the report.
        #[arg(long, value_enum, default_value_t = Format::Text)]
        format: Format,
    },
    /// Run a suite and write the AEP evidence record the run produced.
    ///
    /// The handoff (design §31): what `run` prints is a report about an implementation, and what the
    /// protocol decides on is an evidence record. This verb produces the second — a document
    /// `protocol evaluate --evidence` reads directly, carrying the specification's digest, the
    /// implementation that answered, the verdict, and `producer: verifier / conformance-runner`,
    /// which is what `principles/verification/ess-conformance.yaml` means by `independent: true`.
    ///
    /// # Why this runs the suite rather than converting a saved report
    ///
    /// A verb that turned a `--report report.json` into evidence would produce a record whose
    /// contents came from a file the caller wrote, and the caller is often the agent under review.
    /// That is design §32's forbidden shape with a JSON file in the middle of it. So the record is
    /// only ever minted in the same process that executed the suite, from the report that process
    /// produced, and there is no input through which a caller can describe the outcome.
    ///
    /// # Exit code
    ///
    /// `0` whenever a record was produced, **including for a failing run** — because a failing run
    /// is exactly what direction two of the loop needs written down. The verdict is in the record
    /// and the engine is what decides on it; the same rule `protocol evaluate` follows when it
    /// reports a blocked execution and exits `0`. Use `run` when you want the verdict as an exit
    /// code.
    Evidence {
        /// The specification to synthesise the suite from.
        #[arg(long, default_value = ".")]
        path: PathBuf,
        /// A written suite to run instead, such as `suites/generated/billing/suite.json`.
        #[arg(long, conflicts_with = "path")]
        suite: Option<PathBuf>,
        /// Which built-in reference implementation to run against.
        #[arg(long, value_enum)]
        target: EssTarget,
        /// Break one property on purpose, and watch the evidence stop satisfying the requirement.
        #[arg(long)]
        inject: Option<String>,
        /// Hide the one observation §16 refuses to require of every implementation.
        #[arg(long)]
        untraced: bool,
        /// Where to write the record. Without it the document goes to standard output.
        #[arg(long)]
        out: Option<PathBuf>,
        /// How to write it. Both are read by `protocol evaluate --evidence`.
        #[arg(long, value_enum, default_value_t = Format::Yaml)]
        format: Format,
    },
}

/// The implementations this build carries.
///
/// Named after the example directory each one implements, so `--target billing` and
/// `--path examples/billing` visibly belong together and a mismatch is readable rather than
/// mysterious. A mismatch is *permitted*, and that is deliberate: running the oracle's suite against
/// the billing implementation is how a reader sees `error` — nobody found out — as something other
/// than `failed`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum EssTarget {
    /// `examples/billing/`, implemented by hand and in memory.
    Billing,
    /// `examples/oracle-fixture/`, the fixture with three bindings and all three failure policies.
    #[value(name = "oracle-fixture")]
    OracleFixture,
}

impl EssTarget {
    /// Which specification it implements, in the vocabulary the fault matrix uses.
    fn system(self) -> ess_conformance::System {
        match self {
            Self::Billing => ess_conformance::System::Billing,
            Self::OracleFixture => ess_conformance::System::Oracle,
        }
    }
}

/// Which namespace a name is looked up in.
///
/// A binding identifier and a qualified name are both legal `QualifiedName` spellings, so the
/// namespaces can collide in principle. `--kind` is how a caller says which one it meant; without it
/// a name found in two namespaces is refused rather than guessed at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum EssKind {
    /// A bounded context.
    Domain,
    /// A named type.
    Type,
    /// A command.
    Command,
    /// An event.
    Event,
    /// An error a command may report.
    Error,
    /// A binding, by its identifier.
    Binding,
    /// A component, by its identifier.
    Component,
}

/// Which projection `ess generate` is asked for.
///
/// Spelled out here rather than taken from [`ess_gen::generators`] because clap needs the values at
/// compile time to put them in `--help`. That makes this a second list, so a test asserts the two
/// agree: a projection `ess-gen` publishes that nothing can ask for is a projection nobody runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum EssProjection {
    /// Markdown and Mermaid.
    Docs,
    /// JSON Schema per command input and event payload.
    Schema,
    /// The HTTP contract for the commands a component accepts.
    #[value(name = "openapi")]
    OpenApi,
    /// The messaging contract for the events a component publishes.
    #[value(name = "asyncapi")]
    AsyncApi,
}

impl EssProjection {
    /// The name `ess-gen` publishes this projection under.
    fn name(self) -> &'static str {
        match self {
            Self::Docs => "docs",
            Self::Schema => "schema",
            Self::OpenApi => "openapi",
            Self::AsyncApi => "asyncapi",
        }
    }
}

/// Every `.yaml` file that makes up a specification, in a stable order.
///
/// A specification may be one file or a directory (design §24), and the two are told apart by
/// asking the filesystem rather than by a flag: an author who has just split one file into a
/// directory should not also have to change how they invoke the tool.
fn ess_files(path: &Path) -> Result<Vec<PathBuf>> {
    if path.is_file() {
        return Ok(vec![path.to_path_buf()]);
    }
    if !path.join(SPECIFICATION_HEADER).is_file() {
        bail!(
            "{} is not a specification: a specification directory holds `{SPECIFICATION_HEADER}` \
             (point --path at the specification, or at the single file it is written in)",
            path.display()
        );
    }

    let mut found = Vec::new();
    let mut visited = BTreeSet::new();
    let mut directories = vec![path.to_path_buf()];
    while let Some(directory) = directories.pop() {
        // A symlink pointing back up the tree makes the walk re-enter directories it has already
        // read, under an ever longer name, so every file is read again and again and declares
        // everything in it a second time. Left to itself it stops only when the path outgrows what
        // the filesystem will open.
        let identity = directory
            .canonicalize()
            .with_context(|| format!("resolving {}", directory.display()))?;
        if !visited.insert(identity) {
            continue;
        }
        let entries =
            fs::read_dir(&directory).with_context(|| format!("reading {}", directory.display()))?;
        for entry in entries {
            let entry = entry.with_context(|| format!("reading {}", directory.display()))?;
            let child = entry.path();
            if child.is_dir() {
                directories.push(child);
            } else if child
                .extension()
                .is_some_and(|ext| ext == "yaml" || ext == "yml")
            {
                found.push(child);
            }
        }
    }
    found.sort();
    Ok(found)
}

/// A specification's files, parsed, with each file's text kept beside it.
struct EssSources {
    /// How many files were read.
    files_read: usize,
    /// What parsed, in the order the walk found it.
    parsed: Vec<(ess_domain::system::Source, ess_domain::spec::RawSpecFile)>,
    /// Every file's text, keyed by the label its errors carry.
    texts: SourceMap,
    /// Files that did not parse at all.
    problems: Vec<String>,
}

/// Reads and parses every file a specification is written in.
fn ess_sources(path: &Path) -> Result<EssSources> {
    let root = path
        .canonicalize()
        .with_context(|| format!("resolving {}", path.display()))?;
    let files = ess_files(&root)?;

    // What a source is named relative to. For a one-file specification that is the directory
    // holding it: relative to the file itself every path is empty, which leaves each diagnostic
    // naming no file at all — in the one case where there is nothing else to go on.
    let base = if root.is_file() {
        root.parent().unwrap_or(root.as_path())
    } else {
        root.as_path()
    };

    let mut parsed = Vec::new();
    let mut texts = SourceMap::new();
    let mut problems: Vec<String> = Vec::new();
    for file in &files {
        // The source is the path the author typed, not the absolute one: an error that names
        // `/home/someone/checkout/domains/invoice.yaml` is harder to act on than one naming
        // `domains/invoice.yaml`, and impossible to compare between two machines.
        let relative = file.strip_prefix(base).unwrap_or(file.as_path());
        let source = ess_domain::system::Source::new(relative.display().to_string());
        let text =
            fs::read_to_string(file).with_context(|| format!("reading {}", file.display()))?;
        // Under the same label the errors carry, which is what lets the compiler turn a document
        // path into a line and column without anything threading a file handle through.
        texts.insert(source.as_str(), text.as_str());
        // `RawSpecFile::parse`, not `serde_yaml::from_str`: the latter keeps the last of two
        // identical mapping keys, so a file declaring one workload twice silently loses one.
        match ess_domain::spec::RawSpecFile::parse(&text) {
            Ok(raw) => parsed.push((source, raw)),
            Err(error) => problems.push(format!("{}: {error}", source.as_str())),
        }
    }

    Ok(EssSources {
        files_read: files.len(),
        parsed,
        texts,
        problems,
    })
}

/// A specification that assembled, or every reason it did not.
enum EssLoaded {
    /// It assembled and every local rule held.
    Assembled {
        /// The specification itself.
        specification: Box<ess_domain::spec::Specification>,
        /// The text each part was read from, for diagnostics that want a line number.
        texts: SourceMap,
        /// How many files it was written in.
        files_read: usize,
    },
    /// It did not, and these are the reasons.
    Refused {
        /// How many files were read before it was refused.
        files_read: usize,
        /// Every reason, accumulated.
        problems: Vec<String>,
        /// The same reasons with a code, a structured body and a `file:line`.
        ///
        /// Empty when the failure was a parse error, which has no document path to locate and whose
        /// own message already carries a line.
        diagnostics: Diagnostics,
    },
}

/// Reads a specification and validates it, which is the front half of all four `ess` verbs.
fn ess_load(path: &Path) -> Result<EssLoaded> {
    let EssSources {
        files_read,
        parsed,
        texts,
        mut problems,
    } = ess_sources(path)?;
    let labels: Vec<String> = parsed
        .iter()
        .map(|(source, _)| source.to_string())
        .collect();
    let mut diagnostics = Diagnostics::new();

    // A file that did not parse cannot be assembled with the rest, and assembling the remainder
    // would report every reference into it as undeclared — noise on top of the real error.
    if problems.is_empty() {
        match ess_domain::spec::Specification::assemble(parsed) {
            Ok(specification) => {
                return Ok(EssLoaded::Assembled {
                    specification: Box::new(specification),
                    texts,
                    files_read,
                });
            }
            Err(errors) => {
                // Bridged rather than re-checked: the rule that refused this lives in `ess-domain`
                // and is tested there. What the compiler adds is design §29's shape — a stable code
                // and the line the declaration is written on — which a string cannot carry.
                diagnostics = ess_compiler::resolve::diagnose_locating(&errors, &texts, &labels);
                problems.extend(errors.as_slice().iter().map(ToString::to_string));
            }
        }
    }

    Ok(EssLoaded::Refused {
        files_read,
        problems,
        diagnostics,
    })
}

/// `protocol ess validate`
fn ess_validate(path: &Path, format: Format) -> Result<ExitCode> {
    let mut summary = EssSummary::default();
    match ess_load(path)? {
        EssLoaded::Assembled {
            specification,
            files_read,
            ..
        } => {
            summary.files_read = files_read;
            summary.system = Some(specification.system.name.to_string());
            summary.version = Some(specification.system.version.to_string());
            summary.domains = specification.system.domains.len();
            summary.entities = specification.entities.len();
            summary.commands = specification.commands.len();
            summary.events = specification.events.len();
            summary.errors = specification.errors.len();
            summary.views = specification.views.len();
            summary.actors = specification.actors.len();
        }
        EssLoaded::Refused {
            files_read,
            problems,
            diagnostics: _,
        } => {
            // `ess validate` keeps reporting plain refusals. Codes and spans are what `ess compile`
            // adds, and putting them here too would make the two verbs differ only in a header.
            summary.files_read = files_read;
            summary.problems = problems;
        }
    }
    match format {
        Format::Text => {
            if let Some(system) = &summary.system {
                outln!(
                    "{} {} — {} file(s): {} domain(s), {} entit(ies), {} command(s), {} event(s), \
                     {} error(s), {} view(s), {} actor(s)",
                    system,
                    summary.version.as_deref().unwrap_or("?"),
                    summary.files_read,
                    summary.domains,
                    summary.entities,
                    summary.commands,
                    summary.events,
                    summary.errors,
                    summary.views,
                    summary.actors
                );
            } else {
                outln!("{} file(s)", summary.files_read);
            }
            if summary.problems.is_empty() {
                outln!("valid");
            }
            ess_problems(&summary.problems);
        }
        Format::Yaml | Format::Json => print_serialised(&summary, format)?,
    }

    Ok(exit_code(summary.problems.is_empty()))
}

/// Prints an accumulated list of refusals, and nothing at all when there are none.
///
/// Shared by every `ess` verb, so that a specification refused by `validate` reads the same when
/// `compile` refuses it: the same list, in the same order, under the same heading.
fn ess_problems(problems: &[String]) {
    if problems.is_empty() {
        return;
    }
    outln!("{} problem(s):", problems.len());
    for problem in problems {
        outln!("  - {problem}");
    }
}

/// What `ess validate` reports.
#[derive(Debug, Default, serde::Serialize)]
struct EssSummary {
    system: Option<String>,
    version: Option<String>,
    files_read: usize,
    domains: usize,
    entities: usize,
    commands: usize,
    events: usize,
    errors: usize,
    views: usize,
    actors: usize,
    problems: Vec<String>,
}

/// The IR, or the fact that the reason there is none has already been reported.
enum EssCompiled {
    /// It compiled.
    Compiled {
        /// The IR.
        ir: Box<EssIr>,
        /// How many files it was written in.
        files_read: usize,
    },
    /// It did not, and every reason has been printed in the format that was asked for.
    Reported,
}

/// Loads, validates and compiles a specification, reporting a failure in the format asked for.
///
/// `inspect` and `graph` have nothing to say about a specification that does not compile, and both
/// have to say so exactly as `compile` does — so the failure path is written once and they share it.
fn ess_compiled(path: &Path, format: Format) -> Result<EssCompiled> {
    let (specification, texts, files_read) = match ess_load(path)? {
        EssLoaded::Assembled {
            specification,
            texts,
            files_read,
        } => (specification, texts, files_read),
        EssLoaded::Refused {
            files_read,
            problems,
            diagnostics,
        } => {
            // The bridged form, not an empty set: a refusal from `ess-domain` carries the same
            // code and line as one this pass would have produced, so `ess compile` reports one
            // shape whichever half noticed the defect.
            ess_report_refusal(files_read, &problems, &diagnostics, format)?;
            return Ok(EssCompiled::Reported);
        }
    };

    match ess_compiler::compile(&specification, &texts) {
        Ok(ir) => Ok(EssCompiled::Compiled {
            ir: Box::new(ir),
            files_read,
        }),
        Err(diagnostics) => {
            ess_report_refusal(files_read, &[], &diagnostics, format)?;
            Ok(EssCompiled::Reported)
        }
    }
}

/// Prints why a specification did not compile.
fn ess_report_refusal(
    files_read: usize,
    problems: &[String],
    diagnostics: &Diagnostics,
    format: Format,
) -> Result<()> {
    match format {
        Format::Text => {
            outln!("{files_read} file(s)");
            ess_problems(problems);
            if !diagnostics.is_empty() {
                outln!("{} diagnostic(s):", diagnostics.len());
                for diagnostic in diagnostics.as_slice() {
                    outln!("{diagnostic}");
                }
            }
            if problems.is_empty() && diagnostics.is_empty() {
                // A refusal that says nothing is a compiler bug, and naming it beats exiting 1 in
                // silence — which is what the caller would otherwise have to interpret.
                outln!("not compiled, and no reason was reported");
            }
        }
        Format::Yaml | Format::Json => print_serialised(
            &EssCompilation {
                compiled: false,
                files_read,
                problems,
                diagnostics,
                ir: None,
            },
            format,
        )?,
    }
    Ok(())
}

/// What `ess compile` reports.
///
/// One shape either way, so a consumer branches on `compiled` rather than on which pass failed.
/// `problems` are the refusals that happen before the compiler is reached: a specification that does
/// not assemble has no `Specification` to resolve, so those cannot be `Diagnostic`s — and they are
/// not dressed up as any, because a code a harness matches on has to mean what it says.
#[derive(serde::Serialize)]
struct EssCompilation<'a> {
    compiled: bool,
    files_read: usize,
    problems: &'a [String],
    diagnostics: &'a Diagnostics,
    #[serde(skip_serializing_if = "Option::is_none")]
    ir: Option<&'a EssIr>,
}

/// `protocol ess compile`
fn ess_compile(path: &Path, format: Format) -> Result<ExitCode> {
    let (ir, files_read) = match ess_compiled(path, format)? {
        EssCompiled::Compiled { ir, files_read } => (ir, files_read),
        EssCompiled::Reported => return Ok(exit_code(false)),
    };

    match format {
        Format::Text => {
            outln!(
                "{} {} — {files_read} file(s): {} domain(s), {} type(s), {} command(s), {} \
                 event(s), {} error(s), {} binding(s), {} component(s)",
                ir.system,
                ir.version,
                ir.domains.len(),
                ir.types.len(),
                ir.commands.len(),
                ir.events.len(),
                ir.errors.len(),
                ir.bindings.len(),
                ir.components.len()
            );
            outln!("compiled");
        }
        Format::Yaml | Format::Json => print_serialised(
            &EssCompilation {
                compiled: true,
                files_read,
                problems: &[],
                diagnostics: &Diagnostics::new(),
                ir: Some(&ir),
            },
            format,
        )?,
    }

    Ok(ExitCode::SUCCESS)
}

/// One projection, as `ess generate` names it on the command line.
#[derive(serde::Serialize)]
struct EssProjectionReport {
    /// What `--kind` calls it.
    name: &'static str,
    /// The subdirectory its artifacts go in.
    directory: &'static str,
    /// One line saying what it proves.
    describes: &'static str,
}

impl EssProjectionReport {
    /// Reads a projection's own description of itself.
    fn of(generator: &dyn Generator) -> Self {
        Self {
            name: generator.name(),
            directory: generator.directory(),
            describes: generator.describes(),
        }
    }
}

/// What `ess generate` reports.
///
/// Provenance sits in the report and not only in the artifacts: a consumer reading this over a pipe
/// has no file header to look at, and "which specification produced this" is the only question
/// anyone asks of generated output. Contents are carried whether or not `--out` was given, so
/// nothing has to pick a directory to get at the bytes — which is exactly what `cargo xtask
/// generate --check` needs in order to compare the committed tree against them.
#[derive(serde::Serialize)]
struct EssGeneration<'a> {
    /// Which specification, resolved by which build.
    provenance: &'a Provenance,
    /// Where the artifacts were written, when they were.
    #[serde(skip_serializing_if = "Option::is_none")]
    written_to: Option<String>,
    /// The projections that ran, which is one of them when `--kind` was given.
    projections: Vec<EssProjectionReport>,
    /// Every artifact, in path order, with its contents.
    artifacts: Vec<&'a Artifact>,
}

/// `protocol ess generate`
///
/// Printed and written artifacts are the same bytes, because both come from one call into `ess-gen`.
/// The drift guard in `cargo xtask generate --check` compares a committed tree against what this
/// prints, and that comparison means nothing unless there is one answer to compare with.
fn ess_generate(
    path: &Path,
    kind: Option<EssProjection>,
    out: Option<&Path>,
    format: Format,
) -> Result<ExitCode> {
    let ir = match ess_compiled(path, format)? {
        EssCompiled::Compiled { ir, .. } => ir,
        EssCompiled::Reported => return Ok(exit_code(false)),
    };

    let (projections, artifacts) = match kind {
        Some(projection) => {
            let generator = ess_gen::generator(projection.name()).with_context(|| {
                format!(
                    "`{}` is not a projection this build publishes",
                    projection.name()
                )
            })?;
            // The same entry point the whole set is run through, so a filtered run cannot produce a
            // byte the unfiltered one does not — which is the only reason `--kind` is safe to use
            // against a tree the drift guard checks.
            let artifacts = ess_gen::artifact::run(generator.as_ref(), &ir)?;
            (vec![EssProjectionReport::of(&*generator)], artifacts)
        }
        None => (
            ess_gen::generators()
                .iter()
                .map(|generator| EssProjectionReport::of(&**generator))
                .collect(),
            ess_gen::generate_all(&ir)?,
        ),
    };

    // Written, and nothing else: an artifact in `--out` that no projection produces any more is
    // drift, but it is not this verb's business — `--out` may be any directory a caller names, and a
    // command that deletes what it did not write is a command nobody points at a working tree.
    // `cargo xtask generate` owns the committed tree, and owns removing from it.
    if let Some(directory) = out {
        for artifact in artifacts.values() {
            let target = directory.join(&artifact.path);
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("creating {}", parent.display()))?;
            }
            fs::write(&target, &artifact.contents)
                .with_context(|| format!("writing {}", target.display()))?;
        }
    }

    let provenance = Provenance::of(&ir);
    let report = EssGeneration {
        provenance: &provenance,
        written_to: out.map(|directory| directory.display().to_string()),
        projections,
        artifacts: artifacts.values().collect(),
    };

    match format {
        Format::Text => {
            outln!(
                "{} — {} projection(s), {} artifact(s)",
                report.provenance,
                report.projections.len(),
                report.artifacts.len()
            );
            for artifact in &report.artifacts {
                outln!("  {} — {} byte(s)", artifact.path, artifact.contents.len());
            }
            // Said rather than implied. A reader who expected files needs to know why there are
            // none, and one who wanted the contents needs to be told where they are.
            match &report.written_to {
                Some(directory) => outln!("written to {directory}"),
                None => outln!(
                    "nothing written: pass --out to write these, or --format json for their \
                     contents"
                ),
            }
        }
        Format::Yaml | Format::Json => print_serialised(&report, format)?,
    }

    Ok(ExitCode::SUCCESS)
}

/// The file a suite is written as, under `--out` and under `suites/generated/<system>/`.
///
/// One document per specification, not one per component: a binding scenario starts with a command
/// one component accepts and ends with an event another publishes, so a per-component filing has no
/// drawer for it.
const SUITE_FILE: &str = "suite.json";

/// One construct the specification does not say enough about to test.
///
/// Flattened out of `ess_conformance::Refusal`, which carries no `Serialize`, into the four fields
/// §36 asks a refusal to answer: a stable code, the element it is about, why, and what would have to
/// change. Rendered rather than borrowed, because `--format json` is read by a coding agent as
/// repair instructions and a nested cause type would make it guess at the shape.
#[derive(serde::Serialize)]
struct EssRefusalReport {
    /// The stable code, such as `ESS-CF-011`.
    code: String,
    /// The ESS element that has no scenario.
    subject: String,
    /// The scenario that would have existed, where the refusal is about one.
    #[serde(skip_serializing_if = "Option::is_none")]
    scenario: Option<String>,
    /// Why, in the refusal's own words.
    because: String,
    /// What would have to change for this construct to become testable.
    help: &'static str,
}

impl EssRefusalReport {
    /// Reads a refusal.
    fn of(refusal: &ess_conformance::Refusal) -> Self {
        Self {
            code: refusal.code().to_string(),
            subject: refusal.subject.to_string(),
            scenario: refusal.scenario.as_ref().map(ToString::to_string),
            because: refusal.cause.to_string(),
            help: refusal.hint(),
        }
    }
}

/// The suite document, as an artifact with its bytes beside it.
///
/// The same `{ path, contents }` shape `ess generate` reports, and for the same reason: it is what
/// lets `cargo xtask suite --check` compare a committed tree against what this command produces
/// without anything having to write a file first. One answer to "what should be committed", not two.
#[derive(serde::Serialize)]
struct EssSuiteArtifact {
    /// Where it goes, relative to `--out`.
    path: &'static str,
    /// Its bytes, canonical and newline-terminated.
    contents: String,
}

/// What `ess conform synthesize` reports.
#[derive(serde::Serialize)]
struct EssSynthesis<'a> {
    /// Which specification, resolved and synthesised by which builds.
    provenance: &'a ess_conformance::SuiteProvenance,
    /// Where the suite was written, when it was.
    #[serde(skip_serializing_if = "Option::is_none")]
    written_to: Option<String>,
    /// How many scenarios the suite holds.
    scenarios: usize,
    /// Whether every construct of the specification produced one.
    complete: bool,
    /// Every construct that did not, in the order the model declares them.
    refusals: Vec<EssRefusalReport>,
    /// The suite document itself.
    artifacts: Vec<EssSuiteArtifact>,
}

/// `protocol ess conform synthesize`
///
/// Printed and written bytes are the same bytes, from one call into `ess-conformance`. The drift
/// guard in `cargo xtask suite --check` compares the committed suites against what this prints, and
/// that comparison means nothing unless there is one answer to compare with.
fn ess_conform_synthesize(path: &Path, out: Option<&Path>, format: Format) -> Result<ExitCode> {
    let ir = match ess_compiled(path, format)? {
        EssCompiled::Compiled { ir, .. } => ir,
        EssCompiled::Reported => return Ok(exit_code(false)),
    };

    let synthesis = ess_conformance::synthesize(&ir);
    let contents = synthesis.suite.to_canonical_json();

    // Written, and nothing else. `--out` may be any directory a caller names, and a command that
    // deletes what it did not write is a command nobody points at a working tree; `cargo xtask
    // suite` owns the committed tree, and owns removing from it.
    if let Some(directory) = out {
        fs::create_dir_all(directory)
            .with_context(|| format!("creating {}", directory.display()))?;
        let target = directory.join(SUITE_FILE);
        fs::write(&target, &contents).with_context(|| format!("writing {}", target.display()))?;
    }

    let report = EssSynthesis {
        provenance: &synthesis.suite.provenance,
        written_to: out.map(|directory| directory.display().to_string()),
        scenarios: synthesis.suite.len(),
        complete: synthesis.is_complete(),
        refusals: synthesis
            .refusals
            .iter()
            .map(EssRefusalReport::of)
            .collect(),
        artifacts: vec![EssSuiteArtifact {
            path: SUITE_FILE,
            contents,
        }],
    };

    match format {
        Format::Text => {
            let provenance = report.provenance;
            outln!(
                "{} {} — {} scenario(s), {} refusal(s), model digest {}",
                provenance.system,
                provenance.specification_version,
                report.scenarios,
                report.refusals.len(),
                provenance.spec_digest
            );
            for id in synthesis.suite.scenarios.keys() {
                outln!("  {id}");
            }
            // Said out loud and in full, never as a count. A construct with no scenario is the one
            // defect a green run cannot show, so it is printed beside the scenarios that exist
            // rather than left for whoever thinks to ask for JSON.
            if !synthesis.refusals.is_empty() {
                outln!("{} refusal(s):", synthesis.refusals.len());
                for refusal in &synthesis.refusals {
                    outln!("{refusal}");
                }
            }
            match &report.written_to {
                Some(directory) => outln!("written to {directory}/{SUITE_FILE}"),
                None => outln!(
                    "nothing written: pass --out to write {SUITE_FILE}, or --format json for its \
                     contents"
                ),
            }
        }
        Format::Yaml | Format::Json => print_serialised(&report, format)?,
    }

    Ok(ExitCode::SUCCESS)
}

/// What `ess conform run` reports.
///
/// The report is wrapped rather than printed bare, so that the two facts a verdict is worthless
/// without travel with it: which implementation was deliberately broken, and how many constructs of
/// the specification the suite could not check at all. A bare `ConformanceReport` says neither, and
/// a run that passed 24 of the 27 checks a specification obliges is not the same claim as a run that
/// passed all of them.
#[derive(serde::Serialize)]
struct EssConformance<'a> {
    /// Which built-in implementation answered.
    target: &'a str,
    /// Whether it was asked to hide the invocations its bindings made (§16).
    untraced: bool,
    /// The fault injected into it, where one was.
    #[serde(skip_serializing_if = "Option::is_none")]
    injected: Option<&'a str>,
    /// Where the suite came from.
    suite_source: String,
    /// How many constructs got no scenario, when the suite was synthesised here.
    ///
    /// Absent for `--suite`, because a written suite carries scenarios and not the refusals that
    /// were recorded when it was made — and reporting `0` would be a claim nobody checked.
    #[serde(skip_serializing_if = "Option::is_none")]
    refusals: Option<usize>,
    /// The verdict, scenario by scenario.
    report: &'a ess_conformance::ConformanceReport,
}

/// `protocol ess conform run`
fn ess_conform_run(
    path: &Path,
    suite_file: Option<&Path>,
    target: EssTarget,
    inject: Option<&str>,
    untraced: bool,
    format: Format,
) -> Result<ExitCode> {
    let fault = match inject {
        None => None,
        Some(name) => Some(ess_fault(name, target)?),
    };

    let Some(Run {
        report,
        refusals,
        suite_source,
    }) = ess_conform_perform(path, suite_file, target, fault, untraced, format)?
    else {
        return Ok(exit_code(false));
    };

    let rendered = EssConformance {
        target: ess_target_name(target),
        untraced,
        injected: fault.map(ess_conformance::Fault::written),
        suite_source,
        refusals,
        report: &report,
    };

    match format {
        Format::Text => {
            out!("{report}");
            if let Some(count) = refusals.filter(|count| *count > 0) {
                outln!(
                    "  {count} construct(s) of the specification got no scenario — run `protocol \
                     ess conform synthesize` to see which"
                );
            }
            if let Some(fault) = fault {
                match fault.caught() {
                    ess_conformance::Caught::By(scenario) => outln!(
                        "injected fault: {} — expected to be caught by `{scenario}`",
                        fault.describe()
                    ),
                    // The row worth reading. A fault the suite does not catch is a statement about
                    // what the model can express, and printing it as though it were caught would
                    // make a green run look like evidence.
                    ess_conformance::Caught::Nothing(why) => outln!(
                        "injected fault: {} — caught by nothing, because {why}",
                        fault.describe()
                    ),
                }
            }
            // The four scenario words are already in the count line above; what a reader still needs
            // is what the verdict *means*, because `unsupported` and `failed` are different findings
            // that come to the same exit code, and `error` is neither.
            outln!("{}", ess_conform_verdict(&report));
        }
        Format::Yaml | Format::Json => print_serialised(&rendered, format)?,
    }

    Ok(ess_conform_exit(report.status))
}

/// One run: what it found, where the suite came from, and how much of the specification it covers.
///
/// A struct rather than a tuple because three of its fields are the same shape at a call site and
/// two of them mean opposite things — `refusals: None` says nobody asked, `Some(0)` says nobody was
/// refused, and a tuple position does not say which is which.
struct Run {
    /// The verdict, scenario by scenario.
    report: ess_conformance::ConformanceReport,
    /// How many constructs got no scenario, when the suite was synthesised here.
    refusals: Option<usize>,
    /// Where the suite came from.
    suite_source: String,
}

/// Resolves the suite and runs it, or reports why the specification could not be compiled.
///
/// `Ok(None)` means the specification did not compile and the diagnostics have been printed — the
/// caller's only remaining job is the exit code. Shared by `run` and `evidence` so that the record
/// one produces and the report the other prints can never come from different executions.
fn ess_conform_perform(
    path: &Path,
    suite_file: Option<&Path>,
    target: EssTarget,
    fault: Option<ess_conformance::Fault>,
    untraced: bool,
    format: Format,
) -> Result<Option<Run>> {
    // A written suite reports no refusals, and that is not the same as reporting none: the document
    // holds the scenarios that were synthesised and never the constructs that were not, so `None`
    // says nobody asked rather than claiming zero.
    let (suite, refusals, suite_source) = if let Some(file) = suite_file {
        let text = fs::read_to_string(file)
            .with_context(|| format!("reading the suite {}", file.display()))?;
        let suite = ess_conformance::ConformanceSuite::from_json(&text)
            .with_context(|| format!("reading the suite {}", file.display()))?;
        (suite, None, file.display().to_string())
    } else {
        let ir = match ess_compiled(path, format)? {
            EssCompiled::Compiled { ir, .. } => ir,
            EssCompiled::Reported => return Ok(None),
        };
        let synthesis = ess_conformance::synthesize(&ir);
        (
            synthesis.suite,
            Some(synthesis.refusals.len()),
            path.display().to_string(),
        )
    };

    // Four arms rather than a boxed target, because `Faulty<Billing>` and `Faulty<Oracle>` are
    // different types and the wrapper is generic — the alternative is a trait object for the sake of
    // saving two lines.
    let report = match (target, fault) {
        (EssTarget::Billing, None) => {
            ess_conform_execute(&suite, ess_conformance::reference::Billing::new(), untraced)
        }
        (EssTarget::Billing, Some(fault)) => {
            ess_conform_execute(&suite, ess_conformance::faulty::billing(fault), untraced)
        }
        (EssTarget::OracleFixture, None) => {
            ess_conform_execute(&suite, ess_conformance::reference::Oracle::new(), untraced)
        }
        (EssTarget::OracleFixture, Some(fault)) => {
            ess_conform_execute(&suite, ess_conformance::faulty::oracle(fault), untraced)
        }
    };

    Ok(Some(Run {
        report,
        refusals,
        suite_source,
    }))
}

/// `protocol ess conform evidence`
///
/// The whole handoff, in one place: run the suite, ask the runner's own crate for the record that
/// run produced, and write it. Nothing here reads the verdict and decides what to say about it —
/// [`ess_conformance::ConformanceReport::to_evidence`] does that, on the producing side of the
/// boundary invariant 7 draws, and this function cannot influence the outcome it writes down.
fn ess_conform_evidence(
    path: &Path,
    suite_file: Option<&Path>,
    target: EssTarget,
    inject: Option<&str>,
    untraced: bool,
    out: Option<&Path>,
    format: Format,
) -> Result<ExitCode> {
    let fault = match inject {
        None => None,
        Some(name) => Some(ess_fault(name, target)?),
    };

    let Some(run) = ess_conform_perform(path, suite_file, target, fault, untraced, format)? else {
        return Ok(exit_code(false));
    };

    let record = run
        .report
        .to_evidence()
        .obtained_by(ess_conform_invocation(
            path, suite_file, target, inject, untraced,
        ))
        .from_input(run.suite_source);

    // A list of one, because that is the shape `--evidence` reads: a file holding several records
    // and a file holding one are the same document, and a bare record would be a second shape to
    // support.
    let document = match format {
        // There is no second rendering of an evidence record. `text` gets the document too, rather
        // than a summary a person might paste into a file and find the engine will not read.
        Format::Json => serde_json::to_string_pretty(&[&record])
            .map(|mut json| {
                json.push('\n');
                json
            })
            .context("rendering the evidence record")?,
        Format::Text | Format::Yaml => {
            serde_yaml::to_string(&[&record]).context("rendering the evidence record")?
        }
    };

    match out {
        Some(file) => {
            if let Some(parent) = file
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
            {
                fs::create_dir_all(parent)
                    .with_context(|| format!("creating {}", parent.display()))?;
            }
            fs::write(file, &document).with_context(|| format!("writing {}", file.display()))?;
            outln!("{} — {}", file.display(), record.result().status);
        }
        None => out!("{document}"),
    }

    // Exit 0 for a failing run as well. The verdict is in the record; the engine decides on it.
    Ok(ExitCode::SUCCESS)
}

/// The command line that produced a record, for its provenance.
///
/// Reconstructed rather than read off `std::env::args`, so that the record says what was *done* —
/// one canonical spelling of the run — instead of whatever shell alias or absolute path happened to
/// invoke it. Provenance a reader cannot compare across two records is provenance nobody uses.
fn ess_conform_invocation(
    path: &Path,
    suite_file: Option<&Path>,
    target: EssTarget,
    inject: Option<&str>,
    untraced: bool,
) -> String {
    let source = match suite_file {
        Some(file) => format!("--suite {}", file.display()),
        None => format!("--path {}", path.display()),
    };
    let mut words = vec![
        "protocol ess conform evidence".to_owned(),
        source,
        format!("--target {}", ess_target_name(target)),
    ];
    if let Some(fault) = inject {
        words.push(format!("--inject {fault}"));
    }
    if untraced {
        words.push("--untraced".to_owned());
    }
    words.join(" ")
}

/// Runs a suite against one target, under a runner seeded from the suite itself.
///
/// Nothing here reaches for a clock or a random device: two runs of one suite against a
/// deterministic target produce byte-identical reports, which is what makes `--format json` output
/// worth storing.
///
/// `untraced` is applied here rather than at each call site so that the wrapper cannot be forgotten
/// for one of the four target/fault combinations — which would report a run as though the target had
/// answered a question it was never asked.
fn ess_conform_execute<T: ess_conformance::ConformanceTarget>(
    suite: &ess_conformance::ConformanceSuite,
    target: T,
    untraced: bool,
) -> ess_conformance::ConformanceReport {
    if untraced {
        let target = ess_conformance::reference::Untraced(target);
        ess_conformance::Runner::for_suite(suite).run(suite, &target)
    } else {
        ess_conformance::Runner::for_suite(suite).run(suite, &target)
    }
}

/// What `--target` calls one of the built-in implementations.
fn ess_target_name(target: EssTarget) -> &'static str {
    match target {
        EssTarget::Billing => "billing",
        EssTarget::OracleFixture => "oracle-fixture",
    }
}

/// The sentence that says what the verdict means and which exit code it produced.
fn ess_conform_verdict(report: &ess_conformance::ConformanceReport) -> String {
    let unsupported = report
        .scenarios
        .iter()
        .filter(|result| result.status == ess_conformance::Status::Unsupported)
        .count();
    match report.status {
        ess_conformance::ConformanceStatus::Passed => {
            "conformant: every scenario the specification obliges passed (exit 0)".to_owned()
        }
        ess_conformance::ConformanceStatus::Failed if unsupported > 0 => format!(
            "not conformant: the implementation contradicted the specification, or could not \
             expose what {unsupported} required scenario(s) check — an unsupported required \
             scenario is a failure and not a skip (exit 1)"
        ),
        ess_conformance::ConformanceStatus::Failed => {
            "not conformant: the implementation contradicted the specification (exit 1)".to_owned()
        }
        // Deliberately not exit 1. Nothing contradicted the specification here; the run did not
        // happen, and a harness that treats the two the same will open a defect against a system
        // nobody managed to ask a question of.
        ess_conformance::ConformanceStatus::Error => {
            "undecided: nothing contradicted the specification and at least one scenario could not \
             be executed — the target could not answer, so there is no verdict about it (exit 3)"
                .to_owned()
        }
    }
}

/// `0` conformant, `1` contradicted, `3` nobody found out.
fn ess_conform_exit(status: ess_conformance::ConformanceStatus) -> ExitCode {
    match status {
        ess_conformance::ConformanceStatus::Passed => ExitCode::SUCCESS,
        ess_conformance::ConformanceStatus::Failed => ExitCode::from(1),
        ess_conformance::ConformanceStatus::Error => ExitCode::from(3),
    }
}

/// Parses an ESS fault name and checks it belongs to the target it is being injected into.
///
/// The second half is not politeness. `ess_conformance::faulty::billing` panics on a fault of the
/// other specification, because injecting one would produce a green run that proves nothing — so the
/// refusal has to happen here, with a message, rather than as a backtrace.
fn ess_fault(name: &str, target: EssTarget) -> Result<ess_conformance::Fault> {
    // `wrong-event`, `wrong_event` and `WrongEvent` all name the same fault; separators are a
    // spelling choice, not part of the name.
    let normalised = name.replace(['-', '_'], "").to_ascii_lowercase();
    let fault = ess_conformance::Fault::ALL
        .iter()
        .copied()
        .find(|fault| fault.written().replace('-', "") == normalised)
        .with_context(|| {
            format!(
                "`{name}` is not a fault; known faults are {}",
                ess_conformance::Fault::ALL
                    .iter()
                    .map(|fault| format!(
                        "{} (--target {})",
                        fault.written(),
                        fault.system().directory()
                    ))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })?;

    if fault.system() != target.system() {
        bail!(
            "`{}` is a fault of `{}`, not of `{}`: injecting it into the wrong implementation \
             produces a green run that proves nothing",
            fault.written(),
            fault.system().directory(),
            ess_target_name(target)
        );
    }
    Ok(fault)
}

/// One declaration, resolved, tagged with the namespace it was found in.
#[derive(serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum EssDeclaration<'a> {
    /// A bounded context.
    Domain(&'a ess_compiler::ir::ResolvedDomain),
    /// A named type.
    Type(&'a ess_compiler::ir::ResolvedType),
    /// A command.
    Command(&'a ess_compiler::ir::ResolvedCommand),
    /// An event.
    Event(&'a ess_compiler::ir::ResolvedEvent),
    /// An error a command may report.
    Error(&'a ess_compiler::ir::ResolvedError),
    /// A binding.
    Binding(&'a ess_compiler::ir::ResolvedBinding),
    /// A component.
    Component(&'a ess_compiler::ir::ResolvedComponent),
}

impl EssDeclaration<'_> {
    /// Which namespace it came from.
    fn kind(&self) -> EssKind {
        match self {
            Self::Domain(_) => EssKind::Domain,
            Self::Type(_) => EssKind::Type,
            Self::Command(_) => EssKind::Command,
            Self::Event(_) => EssKind::Event,
            Self::Error(_) => EssKind::Error,
            Self::Binding(_) => EssKind::Binding,
            Self::Component(_) => EssKind::Component,
        }
    }
}

impl EssKind {
    /// The word a reader sees, which is also the value `--kind` takes.
    fn label(self) -> &'static str {
        match self {
            Self::Domain => "domain",
            Self::Type => "type",
            Self::Command => "command",
            Self::Event => "event",
            Self::Error => "error",
            Self::Binding => "binding",
            Self::Component => "component",
        }
    }
}

/// Every declaration this name could mean, in namespace order.
///
/// A binding identifier such as `notify-on-invoice-created` is also a legal qualified name, so the
/// namespaces overlap in principle. Looking in all of them and refusing an ambiguous answer is the
/// only reading that cannot silently show the wrong declaration.
fn ess_lookup<'a>(ir: &'a EssIr, name: &str, kind: Option<EssKind>) -> Vec<EssDeclaration<'a>> {
    let wanted = |candidate: EssKind| kind.is_none_or(|only| only == candidate);
    let mut found = Vec::new();

    if let Ok(qualified) = ess_domain::name::QualifiedName::new(name) {
        if wanted(EssKind::Domain) {
            found.extend(ir.domains.get(&qualified).map(EssDeclaration::Domain));
        }
        if wanted(EssKind::Type) {
            found.extend(ir.types.get(&qualified).map(EssDeclaration::Type));
        }
        if wanted(EssKind::Command) {
            found.extend(ir.commands.get(&qualified).map(EssDeclaration::Command));
        }
        if wanted(EssKind::Event) {
            found.extend(ir.events.get(&qualified).map(EssDeclaration::Event));
        }
        if wanted(EssKind::Error) {
            found.extend(ir.errors.get(&qualified).map(EssDeclaration::Error));
        }
    }
    if wanted(EssKind::Binding) {
        if let Ok(binding) = ess_domain::binding::BindingName::new(name) {
            found.extend(ir.bindings.get(&binding).map(EssDeclaration::Binding));
        }
    }
    if wanted(EssKind::Component) {
        if let Ok(component) = ess_domain::component::ComponentName::new(name) {
            found.extend(ir.components.get(&component).map(EssDeclaration::Component));
        }
    }

    found
}

/// How many names a "did you mean" list shows before it stops being one.
const ESS_LISTING_CAP: usize = 8;

/// Names a reader could have meant, capped so the message stays readable.
fn ess_listing<T: std::fmt::Display>(names: impl Iterator<Item = T>) -> String {
    let rendered: Vec<String> = names.map(|name| format!("`{name}`")).collect();
    if rendered.is_empty() {
        return "none are declared".to_owned();
    }
    let shown = rendered
        .iter()
        .take(ESS_LISTING_CAP)
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    if rendered.len() > ESS_LISTING_CAP {
        format!("{shown}, and {} more", rendered.len() - ESS_LISTING_CAP)
    } else {
        shown
    }
}

/// The message for a name nothing declares, which says what *is* declared.
///
/// A reader who mistyped needs the list more than the refusal, and a coding agent needs it instead
/// of a second round trip.
fn ess_undeclared(ir: &EssIr, name: &str, kind: Option<EssKind>) -> String {
    let wanted = |candidate: EssKind| kind.is_none_or(|only| only == candidate);
    let mut lines = vec![match kind {
        Some(kind) => format!(
            "`{name}` is not a declared {} in {} {}",
            kind.label(),
            ir.system,
            ir.version
        ),
        None => format!("`{name}` is not declared in {} {}", ir.system, ir.version),
    }];
    if wanted(EssKind::Domain) {
        lines.push(format!("  domains: {}", ess_listing(ir.domains.keys())));
    }
    if wanted(EssKind::Type) {
        lines.push(format!("  types: {}", ess_listing(ir.types.keys())));
    }
    if wanted(EssKind::Command) {
        lines.push(format!("  commands: {}", ess_listing(ir.commands.keys())));
    }
    if wanted(EssKind::Event) {
        lines.push(format!("  events: {}", ess_listing(ir.events.keys())));
    }
    if wanted(EssKind::Error) {
        lines.push(format!("  errors: {}", ess_listing(ir.errors.keys())));
    }
    if wanted(EssKind::Binding) {
        lines.push(format!("  bindings: {}", ess_listing(ir.bindings.keys())));
    }
    if wanted(EssKind::Component) {
        lines.push(format!(
            "  components: {}",
            ess_listing(ir.components.keys())
        ));
    }
    lines.join("\n")
}

/// `protocol ess inspect`
fn ess_inspect(path: &Path, name: &str, kind: Option<EssKind>, format: Format) -> Result<ExitCode> {
    let ir = match ess_compiled(path, format)? {
        EssCompiled::Compiled { ir, .. } => ir,
        EssCompiled::Reported => return Ok(exit_code(false)),
    };

    let found = ess_lookup(&ir, name, kind);
    let [declaration] = found.as_slice() else {
        if found.is_empty() {
            bail!("{}", ess_undeclared(&ir, name, kind));
        }
        // One spelling, two namespaces: showing either declaration would be a guess, and the caller
        // is one flag away from saying which they meant.
        let kinds: Vec<&str> = found.iter().map(|entry| entry.kind().label()).collect();
        bail!(
            "`{name}` is declared as a {} — say which with `--kind {}`",
            kinds.join(" and as a "),
            kinds[0]
        );
    };

    match format {
        Format::Text => ess_render_declaration(&ir, declaration),
        Format::Yaml | Format::Json => print_serialised(declaration, format)?,
    }

    Ok(ExitCode::SUCCESS)
}

/// One labelled line of a declaration, indented and aligned so the values line up.
fn ess_line_at(indent: usize, label: &str, value: &str) {
    outln!("{:indent$}{label:<10} {value}", "", indent = indent);
}

/// One labelled line of a declaration.
fn ess_line(label: &str, value: &str) {
    ess_line_at(2, label, value);
}

/// What is overridden about a name, when anything is.
fn ess_render_naming(naming: &ess_domain::name::Naming) {
    if let Some(wire) = &naming.wire {
        ess_line("wire", wire);
    }
    if let Some(display) = &naming.display {
        ess_line("display", display);
    }
    if let Some(summary) = &naming.summary {
        ess_line("summary", summary);
    }
}

/// A declaration in the shape a person reads.
///
/// Text is an orientation: what this is, what it refers to, what refers into it. `--format yaml`
/// hands over the whole declaration, so nothing here has to be exhaustive.
fn ess_render_declaration(ir: &EssIr, declaration: &EssDeclaration<'_>) {
    match declaration {
        EssDeclaration::Domain(domain) => {
            outln!("domain     {}", domain.name);
            for handle in &domain.types {
                ess_line("type", &handle.to_string());
            }
            for handle in &domain.commands {
                ess_line("command", &handle.to_string());
            }
            for handle in &domain.events {
                ess_line("event", &handle.to_string());
            }
            for handle in &domain.errors {
                ess_line("error", &handle.to_string());
            }
            ess_render_naming(&domain.naming);
        }
        EssDeclaration::Type(declared) => {
            outln!("type       {}", declared.name);
            ess_render_body(&declared.body);
            ess_render_naming(&declared.naming);
        }
        EssDeclaration::Command(command) => {
            outln!("command    {}", command.name);
            ess_line("domain", &command.domain.to_string());
            for field in &command.input {
                ess_line("input", &ess_field(field));
            }
            for outcome in &command.outcomes {
                ess_render_outcome(outcome);
            }
            ess_render_naming(&command.naming);
        }
        EssDeclaration::Event(event) => {
            outln!("event      {}", event.name);
            ess_line("domain", &event.domain.to_string());
            for field in &event.fields {
                ess_line("field", &ess_field(field));
            }
            // What reacts to it is the question this event is usually being looked up to answer.
            for binding in ir.bindings.values() {
                if binding.event.name() == &event.name {
                    ess_line(
                        "triggers",
                        &format!("{} through `{}`", binding.command, binding.name),
                    );
                }
            }
            ess_render_naming(&event.naming);
        }
        EssDeclaration::Error(error) => {
            outln!("error      {}", error.name);
            ess_line("domain", &error.domain.to_string());
            if let Some(summary) = &error.summary {
                ess_line("summary", summary);
            }
            for field in &error.fields {
                ess_line("field", &ess_field(field));
            }
        }
        EssDeclaration::Binding(binding) => {
            outln!("binding    {}", binding.name);
            ess_line("when", &format!("{} occurs", binding.event));
            ess_line("invoke", &binding.command.to_string());
            for entry in &binding.mapping {
                ess_line(
                    "mapping",
                    &format!(
                        "{}: {} = {}",
                        entry.target,
                        entry.target_type,
                        ess_mapping_value(&entry.value)
                    ),
                );
                // The reason a crossing is allowed, where the crossing is: a generator emitting this
                // mapping has to emit the conversion, and an auditor is looking for exactly this.
                if let Some(because) = &entry.conversion {
                    ess_line_at(4, "converted", because);
                }
            }
            ess_line("delivery", delivery_word(binding.delivery));
            ess_line("on failure", failure_word(binding.failure));
            ess_render_naming(&binding.naming);
        }
        EssDeclaration::Component(component) => {
            outln!("component  {}", component.name);
            for domain in &component.owns {
                ess_line("owns", &domain.to_string());
            }
            for command in &component.accepts {
                ess_line("accepts", &command.to_string());
            }
            for event in &component.publishes {
                ess_line("publishes", &event.to_string());
            }
            ess_render_naming(&component.naming);
        }
    }
}

/// A field as `name: Type`, with the wire name when it differs.
fn ess_field(field: &ess_compiler::ir::ResolvedField) -> String {
    match &field.naming.wire {
        Some(wire) if wire != &field.name => {
            format!("{}: {} (wire `{wire}`)", field.name, field.type_ref)
        }
        _ => format!("{}: {}", field.name, field.type_ref),
    }
}

/// One outcome, and what reaching it produces.
fn ess_render_outcome(outcome: &ess_compiler::ir::ResolvedOutcome) {
    use ess_compiler::ir::ResolvedCondition;

    let condition = match &outcome.condition {
        ResolvedCondition::When { predicate } => format!("when {predicate}"),
        ResolvedCondition::Otherwise => "otherwise".to_owned(),
        ResolvedCondition::External { cause } => format!("external: {cause}"),
        ResolvedCondition::WrongState => "wrong state".to_owned(),
    };
    ess_line(
        "outcome",
        &format!(
            "{} — {condition} (test: {})",
            outcome.name, outcome.test_strategy
        ),
    );
    for event in &outcome.emits {
        ess_line_at(4, "emits", &event.to_string());
    }
    if let Some(error) = &outcome.error {
        ess_line_at(4, "reports", &error.to_string());
    }
}

/// What a type is made of.
fn ess_render_body(body: &ess_compiler::ir::ResolvedBody) {
    use ess_compiler::ir::ResolvedBody;

    match body {
        ResolvedBody::Newtype { of, invariants } => {
            ess_line("kind", &format!("newtype of {of}"));
            ess_render_invariants(invariants);
        }
        ResolvedBody::Struct { fields, invariants } => {
            ess_line("kind", "struct");
            for field in fields {
                ess_line("field", &ess_field(field));
            }
            ess_render_invariants(invariants);
        }
        ResolvedBody::Enum { variants } => {
            ess_line("kind", "enum");
            for variant in variants {
                ess_line("variant", variant);
            }
        }
        ResolvedBody::Union { tag, variants } => {
            ess_line("kind", &format!("union tagged `{tag}`"));
            for (value, type_ref) in variants {
                ess_line("variant", &format!("{value}: {type_ref}"));
            }
        }
    }
}

/// The conditions every value of a type satisfies, as the author wrote them.
fn ess_render_invariants(invariants: &[ess_domain::entity::Invariant]) {
    for invariant in invariants {
        ess_line("invariant", &invariant.statement);
    }
}

/// Where a mapped value comes from, in one phrase.
fn ess_mapping_value(value: &ess_compiler::ir::ResolvedMappingValue) -> String {
    use ess_compiler::ir::ResolvedMappingValue;

    match value {
        ResolvedMappingValue::EventField { field, type_ref } => {
            format!("event.{field}: {type_ref}")
        }
        // Marked as written rather than checked: nothing in the model says how to read
        // `invoice-created` as a `TemplateId`, and a reader should see which is which.
        ResolvedMappingValue::Literal { value } => format!("`{value}` (literal)"),
    }
}

/// `protocol ess graph`
///
/// The graph itself comes from `ess-gen`, which is where the documentation page's copy of it comes
/// from too. Both renderings project one `SystemGraph`, so this verb and the generated
/// `docs/README.md` cannot come to draw two different pictures of one system.
fn ess_graph(path: &Path, format: GraphFormat) -> Result<ExitCode> {
    let ir = match ess_compiled(path, format.diagnostics())? {
        EssCompiled::Compiled { ir, .. } => ir,
        EssCompiled::Reported => return Ok(exit_code(false)),
    };

    let graph = SystemGraph::of(&ir);
    match format {
        // No fence around the Mermaid: this is what a reader redirects into a file or pastes into a
        // pull request, and three backticks they did not ask for are three characters to delete.
        GraphFormat::Mermaid => out!("{}", graph.mermaid()),
        GraphFormat::Dot => out!("{}", graph.dot()),
        GraphFormat::Yaml => print_serialised(&graph, Format::Yaml)?,
        GraphFormat::Json => print_serialised(&graph, Format::Json)?,
    }

    Ok(ExitCode::SUCCESS)
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

/// What an execution needs, from flags or from the project the command was run in.
struct Inputs {
    registry: Registry,
    task: Task,
    artifacts: ArtifactGraph,
    /// Where these came from, for a report.
    origin: String,
}

/// Resolves execution inputs: explicit flags first, then the project this was run in.
///
/// The order matters. A flag is an instruction; discovery is a convenience. Silently preferring the
/// project would make `--task other.yaml` do something other than what it says.
fn inputs(args: &ExecutionArgs) -> Result<Inputs> {
    if let (Some(task), root) = (&args.task, &args.root) {
        let root = root.clone().unwrap_or_else(|| PathBuf::from("."));
        let registry = load(&root)?;
        let artifacts = match &args.artifacts {
            Some(path) => read_artifacts(path)?,
            None => ArtifactGraph::new(),
        };
        return Ok(Inputs {
            registry,
            task: read_task(task)?,
            artifacts,
            origin: format!("{} and {}", root.display(), task.display()),
        });
    }

    let here = std::env::current_dir().context("reading the working directory")?;
    let root = aep_engine::project::discover(&here).with_context(|| {
        format!(
            "no `.engineering/project.yaml` in {} or any parent, and no --task was given",
            here.display()
        )
    })?;
    let project = aep_engine::project::load(&root).map_err(|errors| anyhow::anyhow!("{errors}"))?;

    // A flag still overrides what the project says, so a one-off run needs no edit to the project.
    let task = match &args.task {
        Some(path) => read_task(path)?,
        None => project
            .require_task()
            .map_err(|reason| anyhow::anyhow!("{reason}"))?
            .clone(),
    };
    let artifacts = match &args.artifacts {
        Some(path) => read_artifacts(path)?,
        None => project.artifacts,
    };

    Ok(Inputs {
        registry: project.registry,
        task,
        artifacts,
        origin: format!("project {}", root.display()),
    })
}

/// `protocol resolve`
fn resolve(args: &ExecutionArgs) -> Result<ExitCode> {
    let Inputs {
        registry,
        task,
        origin,
        ..
    } = inputs(args)?;
    let plan = aep_engine::resolve(&task, &registry)
        .map_err(|errors| anyhow::anyhow!("{errors}"))
        .context("the task cannot be resolved")?;

    match args.format {
        Format::Text => {
            outln!("inputs      {origin}");
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
    let Inputs {
        registry,
        task,
        artifacts,
        origin,
    } = inputs(args)?;

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
            outln!("inputs      {origin}");
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
