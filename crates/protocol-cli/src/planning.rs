//! `protocol artifact` — planning in the markdown store.
//!
//! The verb family an agent uses to plan work: create an epic, decompose it into stories, move one
//! to `active`, ask what the board looks like. Everything it touches is a markdown file in
//! `.engineering/planning/`, so the plan is reviewable in a pull request and its history is
//! `git log` rather than an export.
//!
//! # What lives here and what does not
//!
//! This is the first module split of `main.rs`, and the line is drawn where it costs nothing:
//! `Format`, the output macros, `print_table`, `print_serialised` and `exit_code` stay in the crate
//! root and are reached as `crate::` items, because they are shared with every other verb. What
//! moved is the verb family itself — its arguments, its rendering and its refusals — which shares
//! no state with anything else in the binary.
//!
//! # Exit codes
//!
//! `0` when the answer is yes, `1` when the store or a lifecycle says no. A refusal is *rendered*
//! rather than returned as an error, on the same reasoning `ess diff` gives for a pair it will not
//! compare: "a story cannot go straight from draft to implemented" is an answer about the input,
//! not a failure of this program.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use aep_backend_markdown::{MarkdownStore, PlanningDocument, PlanningFrontmatter, StoreReport};
use aep_domain::artifact::{
    ArtifactGraph, ArtifactId, ArtifactKind, ArtifactLifecycle, ArtifactRef, ArtifactStatus,
    RelationKind,
};
use aep_domain::project::PROJECT_DIRECTORY;
use anyhow::{bail, Context, Result};
use clap::{Args, Subcommand, ValueEnum};

use crate::Format;

/// The directory inside `.engineering` that holds the plan.
const PLANNING_DIRECTORY: &str = "planning";

/// Where a new document's body is seeded from, relative to the document tree.
const TEMPLATE_DIRECTORY: &str = "artifacts/templates";

/// Where the plan is and which documents govern it.
///
/// Split from the rendering choice because one verb needs a different one: `protocol artifact
/// graph` renders `dot` or `json`, which are not values the shared [`Format`] has, and a verb
/// carrying two `--format` flags is not a verb. The same reasoning that gave `protocol ess graph`
/// its own `GraphFormat` — a value a verb cannot honour is worse than one it does not offer.
///
/// `--store` is resolved **lazily**, and that is deliberate: `protocol artifact kinds` and
/// `protocol artifact relations` answer from the vocabulary alone, and a command that refused to
/// list the relation names because the working directory is not a project would be refusing for a
/// reason that has nothing to do with the question.
#[derive(Debug, Args)]
pub(crate) struct StoreLocation {
    /// The planning store. Defaults to `<project>/.engineering/planning`.
    #[arg(long)]
    store: Option<PathBuf>,
    /// The document tree the lifecycles and templates come from.
    #[arg(long, default_value = ".")]
    root: PathBuf,
}

/// Where the plan is, which documents govern it, and how to print the answer.
#[derive(Debug, Args)]
pub(crate) struct StoreArgs {
    /// Where the plan is.
    #[command(flatten)]
    location: StoreLocation,
    /// How to render the result.
    #[arg(long, value_enum, default_value_t = Format::Text)]
    format: Format,
}

impl StoreArgs {
    /// The planning store, from `--store` or from the project this was run in.
    fn store(&self) -> Result<MarkdownStore> {
        self.location.store()
    }

    /// The lifecycles in force.
    fn lifecycles(&self) -> Result<aep_engine::Registry> {
        self.location.lifecycles()
    }
}

impl StoreLocation {
    /// The planning store, from `--store` or from the project this was run in.
    fn store(&self) -> Result<MarkdownStore> {
        if let Some(path) = &self.store {
            return Ok(MarkdownStore::open(path.clone()));
        }
        let here = std::env::current_dir().context("reading the working directory")?;
        let project = aep_engine::project::discover(&here).with_context(|| {
            format!(
                "no `--store` was given and no `{PROJECT_DIRECTORY}/project.yaml` was found in {} \
                 or any parent; pass `--store <dir>` to say where the plan is",
                here.display()
            )
        })?;
        Ok(MarkdownStore::open(
            project.join(PROJECT_DIRECTORY).join(PLANNING_DIRECTORY),
        ))
    }

    /// The lifecycles in force, loaded exactly as `protocol validate` loads them.
    ///
    /// A tree with no `artifacts/lifecycles/` is not an error — it yields an empty registry, and
    /// every kind then gets [`ArtifactLifecycle::permissive`]. That is what makes the store usable
    /// in a repository that has not adopted the document tree yet.
    fn lifecycles(&self) -> Result<aep_engine::Registry> {
        crate::load(&self.root)
    }
}

/// What can be done with the plan.
#[derive(Debug, Subcommand)]
pub(crate) enum ArtifactCommand {
    /// Create a plan item, and write it.
    New(NewArgs),
    /// Move a plan item to another status, if its kind's lifecycle permits.
    ///
    /// Refused moves are printed with **every** status the artifact could have moved to instead,
    /// because a refusal that does not answer the question it creates sends the reader to the
    /// lifecycle file to work it out.
    Move {
        /// Where the plan is and how to render.
        #[command(flatten)]
        store: StoreArgs,
        /// The artifact, such as `story:passkey-login`.
        id: String,
        /// The status to move it to, such as `active`.
        #[arg(long)]
        to: String,
    },
    /// Add an edge from one plan item to another.
    Relate {
        /// Where the plan is and how to render.
        #[command(flatten)]
        store: StoreArgs,
        /// The artifact the edge starts at.
        id: String,
        /// What the edge means, such as `decomposes`. `protocol artifact relations` lists them.
        relation: String,
        /// The artifact the edge points at.
        target: String,
    },
    /// List the plan, one line per artifact.
    List {
        /// Where the plan is and how to render.
        #[command(flatten)]
        store: StoreArgs,
        /// Only this kind, and the kinds that specialise it.
        #[arg(long)]
        kind: Option<String>,
        /// Only this status.
        #[arg(long)]
        status: Option<String>,
    },
    /// Show the plan as status columns.
    Board {
        /// Where the plan is and how to render.
        #[command(flatten)]
        store: StoreArgs,
        /// Only this kind, and the kinds that specialise it.
        #[arg(long)]
        kind: Option<String>,
    },
    /// Print the plan's graph.
    ///
    /// `dot` is for `dot -Tsvg`; `json` is the artifact graph itself, for a consumer that would
    /// otherwise have to parse a diagram to get at it. The same split `protocol ess graph` makes.
    Graph {
        /// Where the plan is.
        #[command(flatten)]
        store: StoreLocation,
        /// How to render the graph.
        #[arg(long, value_enum, default_value_t = PlanningGraphFormat::Dot)]
        format: PlanningGraphFormat,
    },
    /// Check the whole plan: every file, every edge, every status.
    ///
    /// Three classes of problem, accumulated into one list rather than reported one run at a time:
    /// a file that cannot be read or sits where its id does not put it; an edge that points at
    /// nothing, a cycle, or an id declared twice; and a status the kind's lifecycle does not have.
    Validate {
        /// Where the plan is and how to render.
        #[command(flatten)]
        store: StoreArgs,
    },
    /// List the artifact kinds, marking the ones that are planning rather than output.
    Kinds {
        /// How to render. `--store` is not read: this answers from the vocabulary.
        #[command(flatten)]
        store: StoreArgs,
    },
    /// List the relation vocabulary, with what each edge means.
    Relations {
        /// How to render. `--store` is not read: this answers from the vocabulary.
        #[command(flatten)]
        store: StoreArgs,
    },
    /// Show one kind's lifecycle: where it starts, and what may follow what.
    Lifecycle {
        /// Where the documents are and how to render.
        #[command(flatten)]
        store: StoreArgs,
        /// The kind, such as `story`.
        kind: String,
    },
}

/// What `protocol artifact new` needs.
///
/// Its own struct rather than nine fields on the variant, because the verb's handler would
/// otherwise take nine arguments and say nothing more than the struct does.
#[derive(Debug, Args)]
pub(crate) struct NewArgs {
    /// Where the plan is and how to render.
    #[command(flatten)]
    store: StoreArgs,
    /// The kind, such as `story`. Aliases such as `adr` are accepted.
    kind: String,
    /// The name, which becomes the id's name part and the file's stem.
    name: String,
    /// The title, which is what a listing shows.
    #[arg(long)]
    title: String,
    /// A one-line summary.
    #[arg(long)]
    summary: Option<String>,
    /// Who owns it.
    #[arg(long)]
    owner: Option<String>,
    /// A label. Repeat for more than one.
    #[arg(long = "tag")]
    tag: Vec<String>,
    /// An edge, written `<relation>:<artifact-id>`, such as `derived_from:epic:passwordless`.
    ///
    /// Split at the **first** colon, which is unambiguous because no relation name contains one
    /// and every artifact id does.
    #[arg(long = "relate")]
    relate: Vec<String>,
}

/// How to render the plan's graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum PlanningGraphFormat {
    /// Graphviz DOT, for `dot -Tsvg`.
    Dot,
    /// The artifact graph itself.
    Json,
}

/// `protocol artifact`
pub(crate) fn run(command: ArtifactCommand) -> Result<ExitCode> {
    match command {
        ArtifactCommand::New(args) => create(&args),
        ArtifactCommand::Move { store, id, to } => move_status(&store, &id, &to),
        ArtifactCommand::Relate {
            store,
            id,
            relation,
            target,
        } => relate(&store, &id, &relation, &target),
        ArtifactCommand::List {
            store,
            kind,
            status,
        } => list(&store, kind.as_deref(), status.as_deref()),
        ArtifactCommand::Board { store, kind } => board(&store, kind.as_deref()),
        ArtifactCommand::Graph { store, format } => graph(&store, format),
        ArtifactCommand::Validate { store } => validate(&store),
        ArtifactCommand::Kinds { store } => kinds(&store),
        ArtifactCommand::Relations { store } => relations(&store),
        ArtifactCommand::Lifecycle { store, kind } => lifecycle(&store, &kind),
    }
}

/// The artifact graph a planning store describes, for the entity surface to seed from.
///
/// Refuses an unreadable store rather than seeding a partial one: an entity surface answering
/// about nine of ten artifacts, with nothing saying which one is missing, is worse than an error.
pub(crate) fn graph_at(root: &Path) -> Result<ArtifactGraph> {
    let store = MarkdownStore::open(root);
    let report = store.load();
    require_clean(&store, &report)?;
    report
        .graph()
        .map_err(|errors| anyhow::anyhow!("{errors}"))
        .with_context(|| format!("reading the planning store at {}", root.display()))
}

/// `protocol artifact new`
fn create(args: &NewArgs) -> Result<ExitCode> {
    let kind = ArtifactKind::parse(&args.kind).map_err(|error| anyhow::anyhow!("{error}"))?;
    // The id's namespace is the kind's *canonical* name, whichever spelling was typed, so `adr` and
    // `architecture-decision-record` produce one id rather than two for one thing.
    let id = ArtifactId::new(format!("{}:{}", kind.as_str(), args.name))
        .map_err(|error| anyhow::anyhow!("{error}"))?;

    let registry = args.store.lifecycles()?;
    let status = registry
        .lifecycles()
        .for_kind(&kind)
        .map_or(ArtifactStatus::Draft, |lifecycle| lifecycle.initial);

    let mut frontmatter = PlanningFrontmatter::new(id.clone(), kind.clone(), status);
    frontmatter.title = Some(args.title.clone());
    frontmatter.summary.clone_from(&args.summary);
    frontmatter.owner.clone_from(&args.owner);
    frontmatter.tags = args.tag.iter().cloned().collect();
    for value in &args.relate {
        let (relation, target) = parse_relation(value)?;
        frontmatter
            .relations
            .push(aep_domain::artifact::ArtifactRelation::new(
                relation, target,
            ));
    }

    let body =
        template(&args.store.location.root, &kind).unwrap_or_else(|| format!("# {}\n", args.title));
    let document = PlanningDocument::new(frontmatter, body);

    let store = args.store.store()?;
    let path = store.create(&document)?;
    let relative = store.relative_path_for(&id);

    match args.store.format {
        Format::Text => outln!("created {id} ({status}) at {}", path.display()),
        Format::Yaml | Format::Json => crate::print_serialised(
            &Created {
                id: id.to_string(),
                kind: kind.to_string(),
                status: status.as_str(),
                path: relative,
            },
            args.store.format,
        )?,
    }
    Ok(ExitCode::SUCCESS)
}

/// `protocol artifact move`
fn move_status(args: &StoreArgs, id: &str, to: &str) -> Result<ExitCode> {
    let id = artifact_id(id)?;
    let to = parse_status(to)?;
    let registry = args.lifecycles()?;

    let store = args.store()?;
    let mut report = store.load();
    require_clean(&store, &report)?;

    let stored = report
        .documents
        .get_mut(&id)
        .with_context(|| missing(&store, &id))?;
    let from = stored.document.frontmatter.status;

    if let Err(refusal) = stored.document.move_status(to, registry.lifecycles()) {
        outln!("{id} is {from}; {refusal}");
        return Ok(crate::exit_code(false));
    }

    let relative = stored.relative_path.clone();
    let document = stored.document.clone();
    let path = store.update(&relative, &document)?;

    match args.format {
        Format::Text => outln!(
            "{id} moved {from} -> {to} (revision {})",
            document.frontmatter.revision
        ),
        Format::Yaml | Format::Json => crate::print_serialised(
            &Moved {
                id: id.to_string(),
                from: from.as_str(),
                to: to.as_str(),
                revision: document.frontmatter.revision,
                path: path.display().to_string(),
            },
            args.format,
        )?,
    }
    Ok(ExitCode::SUCCESS)
}

/// `protocol artifact relate`
fn relate(args: &StoreArgs, id: &str, relation: &str, target: &str) -> Result<ExitCode> {
    let id = artifact_id(id)?;
    let relation = RelationKind::parse(relation).map_err(|error| anyhow::anyhow!("{error}"))?;
    let target = ArtifactRef::parse(target).map_err(|error| anyhow::anyhow!("{error}"))?;

    let store = args.store()?;
    let mut report = store.load();
    require_clean(&store, &report)?;

    if !report.documents.contains_key(target.id()) {
        bail!(
            "{} does not hold `{}`, so `{id} {relation} {target}` would be an edge to nothing",
            store.root().display(),
            target.id()
        );
    }

    let stored = report
        .documents
        .get_mut(&id)
        .with_context(|| missing(&store, &id))?;
    if !stored.document.add_relation(relation, target.clone()) {
        outln!("{id} already declares {relation} {target}; nothing to do");
        return Ok(ExitCode::SUCCESS);
    }
    let relative = stored.relative_path.clone();
    let document = stored.document.clone();

    // Checked before it is written, not after: a cycle is only visible from the whole graph, and a
    // store that has to be repaired by hand after an edge went in is a store people stop using.
    if let Err(errors) = report.graph() {
        outln!("`{id} {relation} {target}` would not build a graph:");
        for error in errors.as_slice() {
            outln!("  - {error}");
        }
        return Ok(crate::exit_code(false));
    }

    store.update(&relative, &document)?;
    match args.format {
        Format::Text => outln!(
            "{id} {relation} {target} (revision {})",
            document.frontmatter.revision
        ),
        Format::Yaml | Format::Json => crate::print_serialised(
            &Related {
                id: id.to_string(),
                relation: relation.as_str(),
                target: target.to_string(),
                revision: document.frontmatter.revision,
            },
            args.format,
        )?,
    }
    Ok(ExitCode::SUCCESS)
}

/// `protocol artifact list`
fn list(args: &StoreArgs, kind: Option<&str>, status: Option<&str>) -> Result<ExitCode> {
    let store = args.store()?;
    let report = store.load();
    warn_unclean(&report);

    let listed = select(&report, kind, status)?;

    match args.format {
        Format::Text => crate::print_table(
            &listed
                .iter()
                .map(|entry| {
                    vec![
                        entry.id.clone(),
                        entry.kind.clone(),
                        entry.status.to_owned(),
                        entry.title.clone().unwrap_or_default(),
                    ]
                })
                .collect::<Vec<_>>(),
        ),
        Format::Yaml | Format::Json => crate::print_serialised(&listed, args.format)?,
    }
    Ok(ExitCode::SUCCESS)
}

/// `protocol artifact board`
fn board(args: &StoreArgs, kind: Option<&str>) -> Result<ExitCode> {
    let store = args.store()?;
    let report = store.load();
    warn_unclean(&report);

    let listed = select(&report, kind, None)?;
    // Every status, in the vocabulary's own order, and only the ones with something in them: an
    // empty column is a column a reader has to skip on every glance.
    let columns: Vec<Column> = ArtifactStatus::ALL
        .iter()
        .map(|status| Column {
            status: status.as_str(),
            artifacts: listed
                .iter()
                .filter(|entry| entry.status == status.as_str())
                .cloned()
                .collect(),
        })
        .filter(|column| !column.artifacts.is_empty())
        .collect();

    match args.format {
        Format::Text => {
            for (index, column) in columns.iter().enumerate() {
                if index > 0 {
                    outln!();
                }
                outln!("{} ({})", column.status, column.artifacts.len());
                for entry in &column.artifacts {
                    outln!(
                        "  {}  {}",
                        entry.id,
                        entry.title.clone().unwrap_or_default()
                    );
                }
            }
        }
        Format::Yaml | Format::Json => crate::print_serialised(&columns, args.format)?,
    }
    Ok(ExitCode::SUCCESS)
}

/// `protocol artifact graph`
fn graph(args: &StoreLocation, format: PlanningGraphFormat) -> Result<ExitCode> {
    let store = args.store()?;
    let report = store.load();
    warn_unclean(&report);

    let graph = match report.graph() {
        Ok(graph) => graph,
        Err(errors) => {
            outln!("the plan does not build a graph:");
            for error in errors.as_slice() {
                outln!("  - {error}");
            }
            return Ok(crate::exit_code(false));
        }
    };

    match format {
        PlanningGraphFormat::Dot => out!("{}", dot(&graph)),
        PlanningGraphFormat::Json => crate::print_serialised(&graph, Format::Json)?,
    }
    Ok(ExitCode::SUCCESS)
}

/// The plan as Graphviz DOT.
///
/// Ids need no escaping: [`ArtifactId`] accepts only alphanumerics and `-_./`, so neither a quote
/// nor a backslash can reach the output. Said here because the alternative is a reader wondering
/// whether it was forgotten.
fn dot(graph: &ArtifactGraph) -> String {
    let mut rendered = String::from("digraph planning {\n  rankdir=LR;\n  node [shape=box];\n");
    for artifact in graph.artifacts() {
        let _ = writeln!(
            rendered,
            "  \"{}\" [label=\"{}\\n{} · {}\"];",
            artifact.id, artifact.id, artifact.kind, artifact.status
        );
    }
    for artifact in graph.artifacts() {
        for relation in &artifact.relations {
            let _ = writeln!(
                rendered,
                "  \"{}\" -> \"{}\" [label=\"{}\"];",
                artifact.id,
                relation.target.id(),
                relation.kind
            );
        }
    }
    rendered.push_str("}\n");
    rendered
}

/// `protocol artifact validate`
fn validate(args: &StoreArgs) -> Result<ExitCode> {
    let store = args.store()?;
    let report = store.load();
    let registry = args.lifecycles()?;

    let mut problems: Vec<String> = report.failures.iter().map(ToString::to_string).collect();
    match report.graph() {
        Ok(graph) => problems.extend(
            graph
                .validate_lifecycles(registry.lifecycles())
                .as_slice()
                .iter()
                .map(ToString::to_string),
        ),
        Err(errors) => problems.extend(errors.as_slice().iter().map(ToString::to_string)),
    }

    let summary = Summary {
        store: store.root().display().to_string(),
        files_read: report.files_read,
        artifacts: report.documents.len(),
        problems: problems.clone(),
    };

    match args.format {
        Format::Text => {
            outln!(
                "{} file(s) in {}: {} artifact(s)",
                summary.files_read,
                summary.store,
                summary.artifacts
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
        Format::Yaml | Format::Json => crate::print_serialised(&summary, args.format)?,
    }
    Ok(crate::exit_code(problems.is_empty()))
}

/// `protocol artifact kinds`
fn kinds(args: &StoreArgs) -> Result<ExitCode> {
    let listed: Vec<KindRow> = ArtifactKind::NAMED
        .iter()
        .map(|kind| KindRow {
            kind: kind.as_str().to_owned(),
            layer: if kind.is_planning() {
                "planning"
            } else {
                "output"
            },
            planning: kind.is_planning(),
        })
        .collect();

    match args.format {
        Format::Text => crate::print_table(
            &listed
                .iter()
                .map(|entry| vec![entry.kind.clone(), entry.layer.to_owned()])
                .collect::<Vec<_>>(),
        ),
        Format::Yaml | Format::Json => crate::print_serialised(&listed, args.format)?,
    }
    Ok(ExitCode::SUCCESS)
}

/// `protocol artifact relations`
fn relations(args: &StoreArgs) -> Result<ExitCode> {
    let listed: Vec<RelationRow> = RelationKind::ALL
        .iter()
        .map(|relation| RelationRow {
            relation: relation.as_str(),
            meaning: meaning(*relation),
            inverse: relation.inverse_label(),
        })
        .collect();

    match args.format {
        Format::Text => crate::print_table(
            &listed
                .iter()
                .map(|entry| vec![entry.relation.to_owned(), entry.meaning.to_owned()])
                .collect::<Vec<_>>(),
        ),
        Format::Yaml | Format::Json => crate::print_serialised(&listed, args.format)?,
    }
    Ok(ExitCode::SUCCESS)
}

/// `protocol artifact lifecycle`
fn lifecycle(args: &StoreArgs, kind: &str) -> Result<ExitCode> {
    let kind = ArtifactKind::parse(kind).map_err(|error| anyhow::anyhow!("{error}"))?;
    let registry = args.lifecycles()?;
    let declared = registry.lifecycles().for_kind(&kind);
    let permissive = ArtifactLifecycle::permissive();
    let lifecycle = declared.unwrap_or(&permissive);

    let view = Lifecycle {
        kind: kind.to_string(),
        declared: declared.is_some(),
        initial: lifecycle.initial.as_str(),
        transitions: lifecycle
            .transitions
            .iter()
            .map(|(from, to)| {
                (
                    from.as_str().to_owned(),
                    to.iter().map(|status| status.as_str()).collect(),
                )
            })
            .collect(),
    };

    match args.format {
        Format::Text => {
            if view.declared {
                outln!("{} starts at {}", view.kind, view.initial);
            } else {
                outln!(
                    "{} declares no lifecycle, so every status and every move is permitted",
                    view.kind
                );
            }
            for (from, to) in &view.transitions {
                outln!("  {from} -> {}", render_list(to));
            }
        }
        Format::Yaml | Format::Json => crate::print_serialised(&view, args.format)?,
    }
    Ok(ExitCode::SUCCESS)
}

// ---------------------------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------------------------

/// Refuses to write while any document in the store cannot be read.
///
/// A mutation is a read followed by a write, and the read has to have worked. Two files claiming
/// one id is the case that makes this more than caution: whichever one this command chose to
/// rewrite, the other would still be there afterwards saying something different.
fn require_clean(store: &MarkdownStore, report: &StoreReport) -> Result<()> {
    if report.is_clean() {
        return Ok(());
    }
    let mut detail = format!(
        "{} planning document(s) in {} cannot be used, so nothing was written:",
        report.failures.len(),
        store.root().display()
    );
    for failure in &report.failures {
        let _ = write!(detail, "\n  - {failure}");
    }
    detail.push_str("\nfix them, or run `protocol artifact validate` for the whole list");
    bail!("{detail}")
}

/// Says on standard error that a read answered from part of the store.
///
/// A warning rather than a refusal, because a listing of nine artifacts is more useful than a
/// refusal over the tenth — and on standard error, so `--format json` stays a document.
fn warn_unclean(report: &StoreReport) {
    if !report.is_clean() {
        eprintln!(
            "warning: {} planning document(s) could not be read and are missing from this \
             answer; run `protocol artifact validate`",
            report.failures.len()
        );
    }
}

/// Why an id names nothing here.
fn missing(store: &MarkdownStore, id: &ArtifactId) -> String {
    format!(
        "{} holds no `{id}`; it would be at {}",
        store.root().display(),
        store.relative_path_for(id)
    )
}

/// Parses an artifact id, or says what one looks like.
fn artifact_id(value: &str) -> Result<ArtifactId> {
    ArtifactId::new(value).map_err(|error| anyhow::anyhow!("{error}"))
}

/// Parses a status name, listing the vocabulary when it is not one.
fn parse_status(value: &str) -> Result<ArtifactStatus> {
    ArtifactStatus::ALL
        .iter()
        .copied()
        .find(|status| status.as_str() == value)
        .with_context(|| {
            format!(
                "`{value}` is not a status; expected one of {}",
                render_list(
                    &ArtifactStatus::ALL
                        .iter()
                        .map(|status| status.as_str())
                        .collect::<Vec<_>>()
                )
            )
        })
}

/// Parses `<relation>:<artifact-id>`.
fn parse_relation(value: &str) -> Result<(RelationKind, ArtifactRef)> {
    let (relation, target) = value.split_once(':').with_context(|| {
        format!(
            "`{value}` is not an edge; write `<relation>:<artifact-id>`, such as \
             `derived_from:epic:passwordless`"
        )
    })?;
    Ok((
        RelationKind::parse(relation).map_err(|error| anyhow::anyhow!("{error}"))?,
        ArtifactRef::parse(target).map_err(|error| anyhow::anyhow!("{error}"))?,
    ))
}

/// The artifacts a listing verb was asked for, in id order.
fn select(report: &StoreReport, kind: Option<&str>, status: Option<&str>) -> Result<Vec<Listed>> {
    let kind = match kind {
        Some(value) => {
            Some(ArtifactKind::parse(value).map_err(|error| anyhow::anyhow!("{error}"))?)
        }
        None => None,
    };
    let status = match status {
        Some(value) => Some(parse_status(value)?),
        None => None,
    };

    Ok(report
        .documents
        .values()
        .filter(|stored| {
            let frontmatter = &stored.document.frontmatter;
            // `is_a`, not equality, so `--kind design` answers for an `architecture-design` the way
            // `ArtifactGraph::of_kind` does. One question, one answer, wherever it is asked.
            kind.as_ref()
                .is_none_or(|wanted| frontmatter.kind.is_a(wanted))
                && status.is_none_or(|wanted| frontmatter.status == wanted)
        })
        .map(|stored| Listed {
            id: stored.document.frontmatter.id.to_string(),
            kind: stored.document.frontmatter.kind.to_string(),
            status: stored.document.frontmatter.status.as_str(),
            title: stored.document.frontmatter.title.clone(),
            path: stored.relative_path.clone(),
        })
        .collect())
}

/// The template body for a new artifact of `kind`, when the tree has one.
///
/// Alias-aware, because the templates are named the way a person writes the kind: the file for an
/// `architecture-decision-record` is `adr.md`. The canonical spelling is tried first, so a tree
/// that adds `architecture-decision-record.md` wins over the shorter name rather than being
/// ignored.
fn template(root: &Path, kind: &ArtifactKind) -> Option<String> {
    let alias = match kind {
        ArtifactKind::ArchitectureDecisionRecord => Some("adr"),
        ArtifactKind::ExecutableSystemSpecification => Some("ess"),
        ArtifactKind::ProductRequirements => Some("prd"),
        ArtifactKind::Specification => Some("spec"),
        ArtifactKind::ReviewResult => Some("review"),
        _ => None,
    };
    [Some(kind.as_str()), alias]
        .into_iter()
        .flatten()
        .find_map(|name| {
            std::fs::read_to_string(root.join(TEMPLATE_DIRECTORY).join(format!("{name}.md"))).ok()
        })
}

/// What one relation means, as `artifacts/relations/relations.yaml` states it.
///
/// Duplicated from that document rather than read out of it, and the duplication is deliberate:
/// this verb answers about the vocabulary the *binary* implements, which is
/// [`RelationKind`], and a tree without an `artifacts/` directory would otherwise make
/// `protocol artifact relations` print nothing. `delivers` has no entry in that file — the sentence
/// here matches its declaration in `aep-domain`.
fn meaning(relation: RelationKind) -> &'static str {
    match relation {
        RelationKind::InformedBy => {
            "Shaped by, without being derived from — read and taken into account."
        }
        RelationKind::DerivedFrom => {
            "Produced from a higher-level artifact, whose intent it carries down."
        }
        RelationKind::Decomposes => "Breaks a larger artifact into smaller work.",
        RelationKind::Specifies => "States the required behaviour of something.",
        RelationKind::Designs => "Proposes how to satisfy something.",
        RelationKind::Implements => "Realises something in the system.",
        RelationKind::Decides => "Records a decision taken within something.",
        RelationKind::Reviews => "Assesses something.",
        RelationKind::Verifies => "Establishes that something holds.",
        RelationKind::Blocks => "Prevents progress on something.",
        RelationKind::DependsOn => "Needs something else first.",
        RelationKind::Supersedes => "Replaces something, which becomes superseded.",
        RelationKind::Delivers => "Produces the outcome something asked for.",
    }
}

/// A comma-separated list, or `nothing` when there is none.
fn render_list(values: &[&str]) -> String {
    if values.is_empty() {
        return "nothing".to_owned();
    }
    values.join(", ")
}

// ---------------------------------------------------------------------------------------------
// What the machine-readable formats carry
// ---------------------------------------------------------------------------------------------

/// One artifact in a listing.
#[derive(Debug, Clone, serde::Serialize)]
struct Listed {
    id: String,
    kind: String,
    status: &'static str,
    title: Option<String>,
    path: String,
}

/// One status column of the board.
#[derive(Debug, serde::Serialize)]
struct Column {
    status: &'static str,
    artifacts: Vec<Listed>,
}

/// What `new` wrote.
#[derive(Debug, serde::Serialize)]
struct Created {
    id: String,
    kind: String,
    status: &'static str,
    path: String,
}

/// What `move` did.
#[derive(Debug, serde::Serialize)]
struct Moved {
    id: String,
    from: &'static str,
    to: &'static str,
    revision: u64,
    path: String,
}

/// What `relate` did.
#[derive(Debug, serde::Serialize)]
struct Related {
    id: String,
    relation: &'static str,
    target: String,
    revision: u64,
}

/// What `validate` found.
#[derive(Debug, serde::Serialize)]
struct Summary {
    store: String,
    files_read: usize,
    artifacts: usize,
    problems: Vec<String>,
}

/// One kind in the vocabulary.
#[derive(Debug, serde::Serialize)]
struct KindRow {
    kind: String,
    layer: &'static str,
    planning: bool,
}

/// One relation in the vocabulary.
#[derive(Debug, serde::Serialize)]
struct RelationRow {
    relation: &'static str,
    meaning: &'static str,
    inverse: &'static str,
}

/// One kind's ladder.
#[derive(Debug, serde::Serialize)]
struct Lifecycle {
    kind: String,
    declared: bool,
    initial: &'static str,
    transitions: BTreeMap<String, Vec<&'static str>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_edge_argument_splits_at_the_first_colon_only() {
        // `derived_from:epic:passwordless` has three segments and one meaning: the target id keeps
        // its own colon. Splitting at the last would produce the relation `derived_from:epic`.
        let (relation, target) =
            parse_relation("derived_from:epic:passwordless").expect("a well-formed edge");
        assert_eq!(relation, RelationKind::DerivedFrom);
        assert_eq!(target.to_string(), "epic:passwordless");
    }

    #[test]
    fn an_edge_argument_with_no_colon_says_what_one_looks_like() {
        let error = parse_relation("derived_from").expect_err("there is no target");
        assert!(
            error.to_string().contains("<relation>:<artifact-id>"),
            "{error}"
        );
    }

    #[test]
    fn an_unknown_status_lists_the_ones_that_exist() {
        let error = parse_status("in-progress").expect_err("that is not a status");
        let message = error.to_string();
        assert!(message.contains("in_review"), "{message}");
        assert!(message.contains("implemented"), "{message}");
    }

    #[test]
    fn every_relation_in_the_vocabulary_has_a_meaning() {
        // `protocol artifact relations` is how an agent learns the edge names. A blank cell in that
        // table is a relation nobody will use correctly.
        for relation in RelationKind::ALL {
            let meaning = meaning(*relation);
            assert!(!meaning.is_empty(), "{relation} means nothing");
            assert!(
                meaning.ends_with('.'),
                "{relation}'s meaning is not a sentence: {meaning}"
            );
        }
    }

    #[test]
    fn the_adr_template_is_found_under_its_alias() {
        // The kind is `architecture-decision-record` and the file is `adr.md`; a lookup by
        // canonical name alone would silently fall back to a one-line body.
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let body = template(&root, &ArtifactKind::ArchitectureDecisionRecord)
            .expect("this repository ships artifacts/templates/adr.md");
        assert!(body.contains("## Decision"), "{body}");
    }

    #[test]
    fn a_kind_with_no_template_gets_none_rather_than_a_wrong_one() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        assert!(
            template(&root, &ArtifactKind::Vision).is_none(),
            "there is no vision template, and no other template may stand in for one"
        );
    }
}
