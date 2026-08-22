//! `protocol trace` — judging an agent run against a typed specification.
//!
//! The second module split of `main.rs`, on the criterion the first one set: a verb family with
//! its own store — here, its own *observation domain* — its own vocabulary, and no shared state
//! with the rest of the binary. `Format` is not shared, and that is deliberate; see
//! [`TraceFormat`].
//!
//! # Exit codes
//!
//! `trace check` mirrors `ess conform`, which is the existing precedent in this binary:
//!
//! | code | meaning |
//! |---|---|
//! | `0` | every gating expectation holds |
//! | `1` | at least one gating gap — the run contradicted the specification |
//! | `3` | no gating gaps, and at least one gating expectation could not be judged |
//!
//! `2` is `clap`'s, for arguments it refuses. Everything this module rejects itself — a file that
//! is not a `trace-spec/1` specification, a file that is not a transcript, an `--advisory` id the
//! document does not declare — leaves through the binary's top-level error handler as `1`, with
//! the reason on stderr. A caller that needs to tell *"the run contradicted the specification"*
//! from *"I passed you the wrong file"* reads stderr or checks that the report has rows in it;
//! `run.sh` does the latter.
//!
//! **Exit 3 is not a softer exit 1.** A CI job may choose to treat it as a failure; the checker
//! refuses to make that choice on the job's behalf, because *"the agent did the wrong thing"* and
//! *"the transcript format moved under us"* want different people to be woken up.
//!
//! `trace inspect` exits `0` whatever the census says. A census is a report, not a gate — the same
//! position `protocol infra simulate` takes for the same reason.
//!
//! `trace evidence` also exits `0` for a run it judged badly, and that is not an oversight: the
//! verdict belongs in the record, and the engine is what decides on it. Its exit code answers
//! *"was a record produced?"*, which is the same split `ess conform evidence` makes.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{bail, Context, Result};
use clap::{Args, Subcommand, ValueEnum};
use trace_domain::ir::TraceIr;
use trace_domain::spec::TraceSpec;
use trace_spec::check::check;
use trace_spec::reader::{detect, read_any};
use trace_spec::render::{census_to_text, report_to_text, verdict_sentence};
use trace_spec::report::CheckReport;

use crate::Format;

/// How to render a trace answer.
///
/// Its own enum rather than the crate's shared `Format`, on the reasoning `GraphFormat` and
/// `DiffFormat` already give: a value a verb cannot honour is worse than one it does not offer.
/// What is missing here is `yaml`. A check report is either read by a person, in which case it is
/// a table with one line per expectation, or parsed by a program, in which case it is JSON — and a
/// third rendering of the same value is a third thing to keep in step with the other two.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum TraceFormat {
    /// Human-readable lines: one per expectation, with the events it cites.
    Text,
    /// JSON, for another tool to read.
    Json,
}

/// What can be done with an agent-run transcript.
#[derive(Debug, Subcommand)]
pub(crate) enum TraceCommand {
    /// Judge a transcript against a `trace-spec/1` specification.
    ///
    /// Reads a file and evaluates typed predicates over it. No clock is read — every duration and
    /// every cost comes out of the transcript — and no model is called, which is the property that
    /// makes a report reproducible, diffable and usable as evidence.
    Check(CheckArgs),
    /// Report what is in a transcript: events, tool traffic and per-step timings.
    ///
    /// The census, with no opinions in it. This is the eval's informational metrics block as a
    /// verb: it states quantities, and [`TraceCommand::Check`] is where an opinion about one
    /// belongs.
    Inspect {
        /// The run's record: a Claude Code `stream-json` transcript, or the
        /// `metaharness.event/1` event stream a driven `llm` step writes.
        ///
        /// Which reader it gets is decided from the first line's `format` tag, so both take the
        /// same arguments.
        #[arg(long)]
        transcript: PathBuf,
        /// How to render the census.
        #[arg(long, value_enum, default_value_t = TraceFormat::Text)]
        format: TraceFormat,
    },
    /// Run the check and write the AEP evidence record it produced.
    ///
    /// The join the whole family exists for. [`TraceCommand::Check`] answers a person; this
    /// answers the protocol — a document `protocol evaluate --evidence` reads directly, carrying
    /// the specification's digest, the transcript's digest, the verdict, the three counts and
    /// `producer: verifier / trace-checker`.
    ///
    /// The record is minted **in the same process that ran the check**, exactly as `ess conform
    /// evidence` does, so no caller can author its own verdict. A verb that turned a
    /// `--report report.json` into evidence would produce a record whose only witness is a file
    /// someone handed it, and the independence the record claims would be a claim about that file.
    ///
    /// The consequence is the one worth stating plainly: **a behavioural claim about an agent
    /// becomes admissible evidence without the agent minting anything.** The model does not report
    /// that it consulted the CLI before editing; a deterministic checker reads the transcript the
    /// model produced and establishes it.
    Evidence(EvidenceArgs),
}

/// The arguments of `protocol trace check`.
#[derive(Debug, Args)]
pub(crate) struct CheckArgs {
    /// The specification the run is judged against.
    #[arg(long)]
    spec: PathBuf,
    /// The run's record: a Claude Code `stream-json` transcript, or the `metaharness.event/1`
    /// event stream a driven `llm` step writes.
    ///
    /// Which reader it gets is decided from the first line's `format` tag, so both take the same
    /// arguments.
    #[arg(long)]
    transcript: PathBuf,
    /// How to render the report.
    #[arg(long, value_enum, default_value_t = TraceFormat::Text)]
    format: TraceFormat,
    /// Cite event indices and digests only — no command strings, no file paths, no text.
    ///
    /// A transcript contains the prompt, the model's reasoning, file contents it read and commands
    /// it ran, and a report is a thing people paste into pull requests. Opt-in rather than the
    /// default (design decision D3): a report is most useful with its evidence visible, and a
    /// checker that hides evidence by default is one people stop trusting. The un-redacted
    /// rendering carries a footer naming what it contains, so pasting one somewhere public is a
    /// decision rather than an accident.
    #[arg(long)]
    redact: bool,
    /// Downgrade a named expectation to advisory for this run: evaluated and printed, gating
    /// nothing.
    ///
    /// For the expectation that is about *the environment the run was given* rather than about the
    /// agent — the eval's `billed-to-the-session` under `EVAL_USE_API_KEY=1` is the motivating
    /// case. It is deliberately not a way to skip a check: the row is still evaluated, still
    /// printed, and the report names every id that was downgraded.
    ///
    /// An id the specification does not declare is a **usage error**, not a silent no-op. A typo
    /// here would otherwise relax nothing while the caller believed it had, or — worse — relax
    /// nothing while the caller believed it had not.
    #[arg(long = "advisory", value_name = "EXPECTATION_ID")]
    advisory: Vec<String>,
}

/// The arguments of `protocol trace evidence`.
///
/// The same two inputs as [`CheckArgs`], because the record is a check — plus where to write it.
/// What it deliberately does **not** carry is `--redact`: a record holds counts, ids and two
/// digests and never quotes the transcript, so there is nothing in it for redaction to remove and
/// an option that did nothing would suggest otherwise.
#[derive(Debug, Args)]
pub(crate) struct EvidenceArgs {
    /// The specification the run is judged against.
    #[arg(long)]
    spec: PathBuf,
    /// The run's record: a Claude Code `stream-json` transcript, or the `metaharness.event/1`
    /// event stream a driven `llm` step writes.
    ///
    /// Which reader it gets is decided from the first line's `format` tag, so both take the same
    /// arguments.
    #[arg(long)]
    transcript: PathBuf,
    /// Where to write the record. Without it the document goes to standard output.
    #[arg(long)]
    out: Option<PathBuf>,
    /// How to write it. Both are read by `protocol evaluate --evidence`.
    ///
    /// The shared `Format` rather than [`TraceFormat`], and the difference is the argument
    /// [`TraceFormat`] itself makes: a check report has no third rendering worth keeping in step,
    /// and an evidence record has exactly one meaning in two spellings the engine already reads.
    #[arg(long, value_enum, default_value_t = Format::Yaml)]
    format: Format,
    /// Downgrade a named expectation to advisory for this run, as `trace check` does.
    ///
    /// The record names every id downgraded, so the narrowing is visible to a later reader — and
    /// `trace_conformance.passed` ignores the downgrade, because a flag the caller passed must not
    /// be able to satisfy a requirement the protocol asked for.
    #[arg(long = "advisory", value_name = "EXPECTATION_ID")]
    advisory: Vec<String>,
    /// When the check is to be recorded as having happened, as a date or epoch milliseconds.
    ///
    /// Defaults to now, which is the truth: the transcript is checked by this process, in this
    /// second. It is settable so that a committed record can be regenerated byte for byte, which is
    /// the one legitimate reason to pin an observation time.
    #[arg(long, value_name = "DATE")]
    observed_at: Option<String>,
}

/// The `trace` verb family, one arm per subcommand.
pub(crate) fn run(command: TraceCommand) -> Result<ExitCode> {
    match command {
        TraceCommand::Check(args) => check_transcript(&args),
        TraceCommand::Inspect { transcript, format } => inspect(&transcript, format),
        TraceCommand::Evidence(args) => mint_evidence(&args),
    }
}

/// Reads a specification through its validation. One reader, so a harness and this cannot
/// disagree about what a document means.
fn load_spec(path: &Path) -> Result<TraceSpec> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("reading the specification at {}", path.display()))?;
    trace_domain::raw::read_spec(&text).map_err(|errors| {
        anyhow::anyhow!(
            "{} is not a trace-spec/1 specification — {} refusal(s):\n{errors}",
            path.display(),
            errors.len()
        )
    })
}

/// Reads a transcript through whichever of the two adapters its first line calls for.
///
/// The detection is [`trace_spec::reader::read_any`]'s and deliberately not a flag: a driven run's
/// transcript is a `metaharness.event/1` event stream and a recorded fixture is Claude Code
/// `stream-json`, and a caller checking both against the same specification should pass the same
/// arguments for both. A `--format` argument would be wrong exactly when somebody is in a hurry.
/// The report names the adapter that read the run, so which one it was stays visible.
fn load_transcript(path: &Path) -> Result<TraceIr> {
    let bytes = std::fs::read(path)
        .with_context(|| format!("reading the transcript at {}", path.display()))?;
    let format = detect(&bytes);
    read_any(&bytes).map_err(|errors| {
        anyhow::anyhow!(
            "{} is not a transcript this build can read as {} — {} refusal(s):\n{errors}",
            path.display(),
            format.as_str(),
            errors.len()
        )
    })
}

/// Reads both files, applies the downgrades and evaluates the specification.
///
/// One function, shared by `check` and `evidence`, so the record cannot be built from a different
/// evaluation than the one a reader was shown. It is the same reason `ess_conform_perform` exists
/// beside `ess conform run` and `ess conform evidence`.
fn perform(spec_path: &Path, transcript_path: &Path, advisory: &[String]) -> Result<CheckReport> {
    let mut spec = load_spec(spec_path)?;
    let ir = load_transcript(transcript_path)?;

    let requested: BTreeSet<String> = advisory.iter().cloned().collect();
    if !requested.is_empty() {
        let unknown = spec.mark_advisory(&requested);
        if !unknown.is_empty() {
            bail!(
                "--advisory names {} expectation(s) `{}` does not declare: {}. A downgrade that \
                 matched nothing would relax nothing while looking as though it had",
                unknown.len(),
                spec.id,
                unknown.join(", ")
            );
        }
    }

    Ok(check(&spec, &ir, advisory))
}

/// `protocol trace check`
fn check_transcript(args: &CheckArgs) -> Result<ExitCode> {
    let mut report = perform(&args.spec, &args.transcript, &args.advisory)?;
    if args.redact {
        report = report.redact();
    }

    match args.format {
        TraceFormat::Text => {
            out!("{}", report_to_text(&report));
            outln!("{}", verdict_sentence(&report));
        }
        TraceFormat::Json => {
            outln!(
                "{}",
                serde_json::to_string_pretty(&report).context("rendering the report as JSON")?
            );
        }
    }
    Ok(ExitCode::from(report.exit_code()))
}

/// `protocol trace evidence`
///
/// The check runs here, and the conversion happens on the producing side —
/// [`CheckReport::to_evidence`] in `trace-spec` — because invariant 7 is that the engine never
/// manufactures evidence and this binary is not allowed to either. What this function does is read
/// two files, hand the report over, and write the document down.
fn mint_evidence(args: &EvidenceArgs) -> Result<ExitCode> {
    let report = perform(&args.spec, &args.transcript, &args.advisory)?;
    let record = report
        .to_evidence(crate::observation_time(args.observed_at.as_deref())?)
        .with_context(|| format!("building the evidence record for `{}`", report.spec_id))?
        .obtained_by(invocation(args))
        .from_input(args.spec.display().to_string())
        .from_input(args.transcript.display().to_string());

    // A list of one, because that is the shape `--evidence` reads: a file holding several records
    // and a file holding one are the same document, and a bare record would be a second shape to
    // support.
    let document = match args.format {
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

    match &args.out {
        Some(file) => {
            if let Some(parent) = file
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
            {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("creating {}", parent.display()))?;
            }
            std::fs::write(file, &document)
                .with_context(|| format!("writing {}", file.display()))?;
            outln!("{} — {}", file.display(), record.result().status);
        }
        None => out!("{document}"),
    }

    // Exit 0 for a run that gapped as well. The verdict is in the record; the engine decides on it,
    // and a caller that wants the verdict as an exit code runs `trace check`.
    Ok(ExitCode::SUCCESS)
}

/// The command line, as the record's provenance reports it.
///
/// Reconstructed rather than read from `std::env::args`, so the record says what was *asked for*
/// in the vocabulary of this verb rather than however the caller's shell spelled it.
fn invocation(args: &EvidenceArgs) -> String {
    let mut rendered = format!(
        "protocol trace evidence --spec {} --transcript {}",
        args.spec.display(),
        args.transcript.display()
    );
    for id in &args.advisory {
        rendered.push_str(" --advisory ");
        rendered.push_str(id);
    }
    rendered
}

/// `protocol trace inspect`
fn inspect(transcript: &Path, format: TraceFormat) -> Result<ExitCode> {
    let ir = load_transcript(transcript)?;
    let census = ir.census();
    match format {
        TraceFormat::Text => out!("{}", census_to_text(&census)),
        TraceFormat::Json => outln!(
            "{}",
            serde_json::to_string_pretty(&census).context("rendering the census as JSON")?
        ),
    }
    // A census is a report, not a gate.
    Ok(ExitCode::SUCCESS)
}
