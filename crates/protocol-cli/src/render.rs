//! `protocol workflow render` — drawing a workflow, and a run over it.
//!
//! The fourth module split of `main.rs`, on the criterion the first three set: a verb family with
//! its own vocabulary and no shared state with the rest of the binary. It brings its own `--format`
//! enum, because *how do I draw this* and *how do I serialise this* are not the same question and a
//! `--format yaml` that produced no picture would be a value the verb cannot honour.
//!
//! # What is here and what is in `aep-render`
//!
//! Everything that decides what the picture *looks like* is in the library, and everything that
//! touches the world is here. That boundary is the same one `aep-driver` draws, for the same
//! reason: `aep-render` claims to be clock-free, terminal-free and file-free, and its own
//! determinism scan enforces that — so the three things this verb needs and that crate cannot hold
//! all live in this file.
//!
//! | here | why |
//! |---|---|
//! | the `--watch` poll loop | it reads a clock and the modification time of a directory |
//! | the PNG shell-out | it runs another program |
//! | building a `RunView` | it reads the driver's run directory and the engine's snapshot |
//!
//! # PNG is `rsvg-convert`, and the refusal is deliberate
//!
//! Decision 6 of the renderer plan. A Rust rasteriser (`resvg`/`usvg`) would compile a font stack
//! and a colour pipeline into this binary for a format nothing in the gate reads. So PNG is the
//! SVG this crate already produced, handed to `rsvg-convert` on standard input — and when that
//! program is absent the error **names it** and says what to install, rather than reporting that
//! something went wrong with an image.
//!
//! # Exit codes
//!
//! `0` when a picture was produced. `1` for anything this verb refuses — a workflow the tree does
//! not declare, a run directory that is not there, `--watch` on a format that cannot watch, a
//! missing rasteriser. `2` is `clap`'s, for arguments it will not accept at all.

use std::collections::BTreeMap;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::time::{Duration, SystemTime};

use aep_domain::ids::StateId;
use aep_domain::workflow::Workflow;
use aep_domain::WorkflowRef;
use aep_driver::run::RunDirectory;
use aep_driver_spec::cursor::{DriverCursor, RunId, RunStatus as DriverStatus};
use aep_engine::execution::Snapshot;
use aep_engine::project::project_directory;
use aep_render::run::{RunStatus, RunView};
use aep_render::{ansi, html, scene::Scene, svg};
use anyhow::{bail, Context, Result};
use clap::{Args, Subcommand, ValueEnum};

/// The directory inside `.engineering` that holds runs.
///
/// The same constant `drive` uses, spelled again here rather than shared, because this verb
/// **reads** a run directory and never writes one: a renderer that could reach the driver's
/// path-construction helpers is a renderer one refactor away from creating a directory.
const RUNS_DIRECTORY: &str = "runs";

/// The program that turns an SVG into a PNG.
const RASTERISER: &str = "rsvg-convert";

/// How often `--watch` looks at the run directory.
///
/// 500 ms: fast enough that a state change appears while you are still looking at the terminal,
/// slow enough that watching a run costs nothing. It reads two file modification times per tick and
/// re-renders only when one has moved.
const POLL: Duration = Duration::from_millis(500);

/// What to do with a workflow.
#[derive(Debug, Subcommand)]
pub(crate) enum WorkflowCommand {
    /// Draw a workflow, and optionally a run over it.
    Render(RenderArgs),
}

/// What a rendering is written as.
///
/// Its own enum rather than the binary's shared `Format`, on the reasoning `GraphFormat`,
/// `DiffFormat` and `TraceFormat` already give: a value a verb cannot honour is worse than one it
/// does not offer, and `--format yaml` here would produce no picture at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum RenderFormat {
    /// A standalone SVG document, in the house palette.
    Svg,
    /// One self-contained HTML page: the figure, the tables and nothing fetched from anywhere.
    Html,
    /// A raster image, by way of `rsvg-convert`.
    Png,
    /// One terminal frame, with colour.
    Tui,
}

/// The inputs of one rendering.
#[derive(Debug, Args)]
pub(crate) struct RenderArgs {
    /// Which workflow, such as `adp/default` or `adp/default/1`.
    #[arg(long)]
    id: String,
    /// The document tree to load the workflow from.
    #[arg(long, default_value = ".")]
    root: PathBuf,
    /// Draw a driver run over it, by run id — `AUTH-142/3`.
    #[arg(long, conflicts_with = "state")]
    run: Option<String>,
    /// Draw an engine snapshot over it, from a file.
    #[arg(long)]
    state: Option<PathBuf>,
    /// The project holding `.engineering/runs/`. Without it, the project of the working directory.
    #[arg(long)]
    project: Option<PathBuf>,
    /// What to write.
    #[arg(long, value_enum, default_value_t = RenderFormat::Svg)]
    format: RenderFormat,
    /// Where to write it. Without this, everything but `png` goes to standard output.
    #[arg(long)]
    out: Option<PathBuf>,
    /// Redraw the frame as the run advances. `--format tui` with `--run` only.
    #[arg(long)]
    watch: bool,
}

/// Runs the `workflow` verb family.
pub(crate) fn run(command: WorkflowCommand) -> Result<ExitCode> {
    match command {
        WorkflowCommand::Render(args) => render(&args),
    }
}

/// `protocol workflow render`
fn render(args: &RenderArgs) -> Result<ExitCode> {
    let workflow = workflow(args)?;

    if args.watch {
        if args.format != RenderFormat::Tui {
            bail!(
                "`--watch` redraws a terminal frame, so it needs `--format tui`; \
                 `--format {:?}` writes a document once",
                args.format
            );
        }
        let Some(run) = &args.run else {
            bail!("`--watch` follows a run, so it needs `--run <run-id>`");
        };
        return watch(&workflow, args, run);
    }

    let view = view(args, &workflow)?;
    emit(&workflow, view.as_ref(), args)
}

/// The workflow named by `--id`, from the document tree.
fn workflow(args: &RenderArgs) -> Result<Workflow> {
    let reference: WorkflowRef = args
        .id
        .parse()
        .with_context(|| format!("`{}` is not a workflow reference", args.id))?;
    let registry = crate::load(&args.root)?;
    registry.workflow(&reference).cloned().ok_or_else(|| {
        let known: Vec<String> = registry
            .workflows()
            .map(|workflow| workflow.id.to_string())
            .collect();
        anyhow::anyhow!(
            "no workflow `{}` in {}; the tree declares: {}",
            args.id,
            args.root.display(),
            if known.is_empty() {
                "none".to_owned()
            } else {
                known.join(", ")
            }
        )
    })
}

/// The run overlay, from `--run`, from `--state`, or nothing at all.
fn view(args: &RenderArgs, workflow: &Workflow) -> Result<Option<RunView>> {
    if let Some(run) = &args.run {
        return Ok(Some(from_run_directory(&run_directory(args, run)?)?));
    }
    if let Some(path) = &args.state {
        return Ok(Some(from_snapshot(path, workflow)?));
    }
    Ok(None)
}

/// The run directory of `run`, under the project.
fn run_directory(args: &RenderArgs, run: &str) -> Result<RunDirectory> {
    let id: RunId = run
        .parse()
        .with_context(|| format!("`{run}` is not a run id; they are written `<task>/<n>`"))?;
    let project = if let Some(given) = &args.project {
        given.clone()
    } else {
        let here = std::env::current_dir().context("reading the working directory")?;
        let directory = project_directory();
        aep_engine::project::discover(&here).with_context(|| {
            format!(
                "no `--project` was given and no `{directory}/project.yaml` was found in \
                 {} or any parent",
                here.display()
            )
        })?
    };
    let [task, ordinal] = id.segments();
    let path = project
        .join(project_directory())
        .join(RUNS_DIRECTORY)
        .join(task)
        .join(ordinal);
    if !path.is_dir() {
        bail!(
            "no run `{run}` at {}; `protocol drive list` says which runs exist",
            path.display()
        );
    }
    Ok(RunDirectory::at(path))
}

/// A [`RunView`] from the two documents a driver run leaves behind.
///
/// Both are read, because neither answers on its own. The **snapshot** is the engine's and holds
/// the states entered in order and the evidence — the history the overlay draws. The **cursor** is
/// the driver's and holds the thing a picture most needs and a snapshot has no field for: *why the
/// run stopped*.
fn from_run_directory(directory: &RunDirectory) -> Result<RunView> {
    let cursor: DriverCursor = directory
        .read_cursor()
        .with_context(|| format!("reading {}", directory.cursor_path().display()))?;
    let snapshot: Snapshot = directory
        .read_snapshot()
        .with_context(|| format!("reading {}", directory.snapshot_path().display()))?;
    Ok(RunView {
        run: Some(cursor.run.to_string()),
        task: Some(cursor.task.to_string()),
        status: status_of(cursor.status),
        // The cursor's, not the snapshot's: the cursor is the driver's own record of where it is,
        // and it is what a resume reads.
        current: Some(cursor.state.clone()),
        path: snapshot.entered.clone(),
        visits: cursor.visits.clone(),
        evidence: evidence_counts(&snapshot),
        // Verbatim. The engine wrote these sentences; nothing here edits them.
        reasons: cursor.reasons.clone(),
        iterations: Some(cursor.iterations),
    })
}

/// A [`RunView`] from an engine snapshot on its own.
///
/// A snapshot carries no run id, no budgets and no reasons, so the view says so rather than
/// guessing: the status is [`RunStatus::Unknown`] unless the state the snapshot is in is a terminal
/// state of this workflow, which is the one thing about a run's standing that the documents settle
/// without a cursor.
fn from_snapshot(path: &Path, workflow: &Workflow) -> Result<RunView> {
    let text =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    // YAML, which also reads the JSON the driver writes — one parser rather than a guess from a
    // file extension.
    let snapshot: Snapshot = serde_yaml::from_str(&text)
        .with_context(|| format!("{} is not an execution snapshot", path.display()))?;
    let terminal = workflow
        .state(&snapshot.state)
        .is_some_and(aep_domain::workflow::State::is_terminal);
    let mut visits: BTreeMap<StateId, u32> = BTreeMap::new();
    for state in &snapshot.entered {
        *visits.entry(state.clone()).or_insert(0) += 1;
    }
    Ok(RunView {
        run: None,
        task: Some(snapshot.task.clone()),
        status: if terminal {
            RunStatus::Completed
        } else {
            RunStatus::Unknown
        },
        current: Some(snapshot.state.clone()),
        path: snapshot.entered.clone(),
        visits,
        evidence: evidence_counts(&snapshot),
        reasons: Vec::new(),
        iterations: None,
    })
}

/// How many records of each evidence kind a snapshot holds.
fn evidence_counts(snapshot: &Snapshot) -> BTreeMap<String, u32> {
    let mut counts: BTreeMap<String, u32> = BTreeMap::new();
    for recorded in &snapshot.evidence {
        *counts
            .entry(recorded.record.kind().to_string())
            .or_insert(0) += 1;
    }
    counts
}

/// The driver's status as the renderer's.
///
/// A translation and not a re-export, because `aep-render` depends on `aep-domain` alone — the
/// whole reason the overlay is a plain struct. The mapping is one to one apart from
/// `RunStatus::Unknown`, which the driver has no need for and a caller holding only a snapshot
/// does.
fn status_of(status: DriverStatus) -> RunStatus {
    match status {
        DriverStatus::Running => RunStatus::Running,
        DriverStatus::Completed => RunStatus::Completed,
        DriverStatus::Blocked => RunStatus::Blocked,
        DriverStatus::AwaitingOperator => RunStatus::Waiting,
        DriverStatus::BudgetExhausted => RunStatus::Exhausted,
        DriverStatus::StoreBroken => RunStatus::Broken,
    }
}

/// Builds the scene and writes it in the asked-for format.
fn emit(workflow: &Workflow, view: Option<&RunView>, args: &RenderArgs) -> Result<ExitCode> {
    let scene = Scene::build(workflow, view);
    match args.format {
        RenderFormat::Svg => write_text(&svg::render(&scene), args.out.as_deref()),
        RenderFormat::Html => write_text(&html::render(&scene), args.out.as_deref()),
        RenderFormat::Tui => {
            let frame = ansi::frame(&scene);
            // Colour is for a terminal. A file gets the text, because a saved frame full of control
            // characters is a file nothing can read back.
            if let Some(path) = args.out.as_deref() {
                write_text(&ansi::strip(&frame), Some(path))
            } else {
                crate::write_out(&frame, false);
                Ok(ExitCode::SUCCESS)
            }
        }
        RenderFormat::Png => {
            let Some(path) = args.out.as_deref() else {
                bail!(
                    "`--format png` writes an image, so it needs `--out FILE`; \
                     `--format svg` is what goes to standard output"
                );
            };
            rasterise(&svg::render(&scene), path)?;
            crate::write_out(&format!("{}\n", path.display()), false);
            Ok(ExitCode::SUCCESS)
        }
    }
}

/// Writes text to `out`, or to standard output when there is none.
fn write_text(text: &str, out: Option<&Path>) -> Result<ExitCode> {
    match out {
        Some(path) => {
            if let Some(parent) = path
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
            {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("creating {}", parent.display()))?;
            }
            std::fs::write(path, text).with_context(|| format!("writing {}", path.display()))?;
            crate::write_out(&format!("{}\n", path.display()), false);
        }
        None => crate::write_out(text, false),
    }
    Ok(ExitCode::SUCCESS)
}

/// Rasterises an SVG through `rsvg-convert`.
///
/// The SVG goes in on standard input rather than through a temporary file: there is nothing to
/// clean up if the program fails, and no path to collide with a parallel run.
fn rasterise(document: &str, out: &Path) -> Result<()> {
    let mut child = Command::new(RASTERISER)
        .arg("--output")
        .arg(out)
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                anyhow::anyhow!(
                    "`{RASTERISER}` is not on PATH, and PNG is rasterised by it rather than by a \
                     rasteriser compiled into this binary — a font stack and a colour pipeline are \
                     a lot to carry for one format. Install it (it ships with librsvg: \
                     `librsvg2-bin` on Debian, `librsvg` on Arch and Homebrew), or use \
                     `--format svg`, which needs nothing."
                )
            } else {
                anyhow::Error::new(error).context(format!("running `{RASTERISER}`"))
            }
        })?;
    child
        .stdin
        .take()
        .context("`{RASTERISER}` accepted no standard input")?
        .write_all(document.as_bytes())
        .with_context(|| format!("writing the figure to `{RASTERISER}`"))?;
    let status = child
        .wait()
        .with_context(|| format!("waiting for `{RASTERISER}`"))?;
    if !status.success() {
        bail!(
            "`{RASTERISER}` exited {} without writing {}",
            status
                .code()
                .map_or("on a signal".to_owned(), |code| code.to_string()),
            out.display()
        );
    }
    Ok(())
}

/// Redraws the frame whenever the run directory moves.
///
/// The loop is here and not in `aep-render` because it reads a clock and a modification time —
/// exactly the two things that crate's determinism scan refuses. It ends when the run reaches a
/// status it cannot be resumed from, which today means `completed`: a watch that never returned
/// would be a watch nobody could put in a script.
fn watch(workflow: &Workflow, args: &RenderArgs, run: &str) -> Result<ExitCode> {
    let directory = run_directory(args, run)?;
    let mut last: Option<SystemTime> = None;
    loop {
        let stamp = touched(&directory);
        if stamp != last {
            last = stamp;
            let view = from_run_directory(&directory)?;
            let frame = ansi::frame(&Scene::build(workflow, Some(&view)));
            // Clear, home the cursor, draw. No alternate screen: what was on the terminal before
            // is what somebody was reading, and a watch that swallowed it would be a watch that
            // lost the command they typed.
            crate::write_out(&format!("\u{1b}[2J\u{1b}[H{frame}"), false);
            if !resumable(view.status) {
                return Ok(ExitCode::SUCCESS);
            }
        }
        std::thread::sleep(POLL);
    }
}

/// The most recent modification time of the two documents a run writes.
///
/// `None` when neither can be read, which is a run directory that is being written to right now —
/// the next tick will find it.
fn touched(directory: &RunDirectory) -> Option<SystemTime> {
    [directory.cursor_path(), directory.snapshot_path()]
        .iter()
        .filter_map(|path| std::fs::metadata(path).ok()?.modified().ok())
        .max()
}

/// Whether a run could still move, and so whether there is any point looking again.
fn resumable(status: RunStatus) -> bool {
    !matches!(status, RunStatus::Completed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_driver_status_has_a_rendering_status_and_none_of_them_is_unknown() {
        for status in [
            DriverStatus::Running,
            DriverStatus::Completed,
            DriverStatus::Blocked,
            DriverStatus::AwaitingOperator,
            DriverStatus::BudgetExhausted,
            DriverStatus::StoreBroken,
        ] {
            assert_ne!(
                status_of(status),
                RunStatus::Unknown,
                "`{status}` is something the driver knows, so the picture must not say `unknown`"
            );
        }
        // And the two crates agree about which of them means *nothing more will happen by itself*.
        assert!(status_of(DriverStatus::Blocked).is_stopped());
        assert!(status_of(DriverStatus::BudgetExhausted).is_stopped());
        assert!(!status_of(DriverStatus::Completed).is_stopped());
    }

    #[test]
    fn a_watch_stops_at_a_completed_run_and_keeps_looking_at_a_blocked_one() {
        assert!(!resumable(RunStatus::Completed));
        assert!(
            resumable(RunStatus::Blocked),
            "a blocked run is resumable, so the frame must keep following it"
        );
        assert!(resumable(RunStatus::Waiting));
    }
}
