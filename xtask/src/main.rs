//! Repository automation.
//!
//! `cargo xtask schema` regenerates the published JSON Schemas from the Rust types;
//! `cargo xtask generate` regenerates the committed projections of the normative specification;
//! `cargo xtask suite` regenerates the committed conformance suites the example specifications
//! oblige. All three take `--check`, which verifies the committed files still match instead of
//! writing them, and that is what CI runs — one job each, so a stale artifact reads as a stale
//! artifact rather than as "the gate failed". All three directories are outputs: editing one by hand
//! is always wrong, because the next regeneration silently reverts it.
//!
//! # One owner per tree
//!
//! Each task owns a directory root and nothing else, and that is why the suites are committed beside
//! `generated/` rather than inside it as design §38 sketched. The orphan scan below is what forces
//! it: it is recursive, and it deletes every committed file the task does not itself produce. Two
//! tasks writing into one tree therefore means each one calling the other's output a file nothing
//! generates — and in write mode, deleting it. An exclusion list would work until somebody adds a
//! third task, and the failure it fails at is silently removing a committed contract.
//!
//! `suites/generated/` also holds suites for **two** specifications, where `generated/` is defined as
//! the projections of the normative example alone: the fault matrix names scenario ids from
//! `examples/oracle-fixture/` as well as from `examples/billing/`, so both have to be stable.
//!
//! A `--check` that only compares what is generated cannot see the other direction — a committed
//! file nothing generates any more. That is a contract this repository no longer stands behind and a
//! consumer still validating against it, so both tasks scan for orphans as well.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};

/// The index of a generated directory, written from the same list the directory is.
const INDEX: &str = "README.md";

/// The specification the committed projections are generated from.
///
/// The normative example (design §31), and only it: it is the one specification in this repository
/// that exercises components and bindings, so it is the one whose projections are worth committing.
const NORMATIVE_EXAMPLE: &str = "examples/billing";

/// Where those projections are committed.
///
/// At the repository root rather than under `examples/billing/`, because they are not part of the
/// specification: a reader opening the example should see what a person wrote, not four directories
/// of output derived from it. And at one path rather than one per projection, because the orphan
/// scan and the index are properties of the whole tree.
const PROJECTIONS: &str = "generated";

/// The specifications a conformance suite is committed for.
///
/// Two, not one. `examples/billing/` is the normative example, and the suite for it is what design
/// §38 asks to be committed. `examples/oracle-fixture/` is here because `ess-conformance`'s fault
/// matrix names scenario ids from it — `handoff-on-placed/binding/flow` and its siblings — and an id
/// a matrix refers to has to be an id that cannot change by accident.
const SUITE_SPECIFICATIONS: &[&str] = &["examples/billing", "examples/oracle-fixture"];

/// Where those suites are committed.
///
/// Beside `generated/` rather than inside it, for the reason the [module documentation](self) gives:
/// one owner per tree, because the orphan scan deletes what its own task does not produce. The
/// nesting mirrors `schemas/generated/`, which is this repository's existing shape for a committed
/// output tree with a drift check and a CI job of its own.
const SUITES: &str = "suites/generated";

/// The specifications a Rust workspace is synthesised for.
///
/// One: the normative example is the specification wave 6 closes its loop against, and a second
/// workspace is committed the day a second specification earns one.
const SYNTH_SPECIFICATIONS: &[&str] = &["examples/billing"];

/// Where those workspaces are committed.
///
/// *Inside* `generated/`, which the module documentation above forbids — and the wave 6 plan page
/// fixes this path, so the conflict is resolved by mechanism rather than by hoping: the nested
/// root is carved out of the projection task's orphan scan through [`PROJECTION_EXCLUSIONS`], the
/// carve-out is derived from this same constant's name, and `no_two_tasks_own_one_committed_tree`
/// now *requires* a nested root to appear in its outer owner's exclusion list. What made an
/// exclusion list dangerous was that nothing checked it; checked, it is just ownership written
/// down.
const SYNTH: &str = "generated/rust";

/// The subtrees of `generated/` the projection task does not own.
///
/// Exactly the nested owners' roots, relative to [`PROJECTIONS`] — the ownership test refuses an
/// entry here that no task owns, because an unowned exclusion is a hole in the drift check that
/// nobody scans.
const PROJECTION_EXCLUSIONS: &[&str] = &["rust"];

/// Repository automation for engineering-protocols.
#[derive(Debug, Parser)]
#[command(name = "xtask", about, version)]
struct Cli {
    /// What to do.
    #[command(subcommand)]
    command: Command,
}

/// The available tasks.
#[derive(Debug, Subcommand)]
enum Command {
    /// Regenerate the published JSON Schemas.
    Schema {
        /// Verify the committed files match instead of writing them.
        #[arg(long)]
        check: bool,
    },
    /// Regenerate the committed projections of the normative specification.
    Generate {
        /// Verify the committed tree matches instead of writing it.
        #[arg(long)]
        check: bool,
    },
    /// Regenerate the committed conformance suites the example specifications oblige.
    Suite {
        /// Verify the committed tree matches instead of writing it.
        #[arg(long)]
        check: bool,
    },
    /// Regenerate the committed Rust workspaces the example specifications determine.
    Synth {
        /// Verify the committed tree matches — and still compiles — instead of writing it.
        #[arg(long)]
        check: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Schema { check } => schema(&workspace_root(), check),
        Command::Generate { check } => {
            let root = workspace_root();
            generate(
                &root.join(NORMATIVE_EXAMPLE),
                &root.join(PROJECTIONS),
                check,
            )
        }
        Command::Suite { check } => {
            let root = workspace_root();
            let specifications: Vec<PathBuf> = SUITE_SPECIFICATIONS
                .iter()
                .map(|specification| root.join(specification))
                .collect();
            suite(&specifications, &root.join(SUITES), check)
        }
        Command::Synth { check } => {
            let root = workspace_root();
            let specifications: Vec<PathBuf> = SYNTH_SPECIFICATIONS
                .iter()
                .map(|specification| root.join(specification))
                .collect();
            synth(&specifications, &root.join(SYNTH), check)
        }
    }
}

/// The repository root, derived from this crate's manifest directory.
fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask lives one level below the workspace root")
        .to_path_buf()
}

/// Writes or checks `schemas/generated/`.
fn schema(root: &Path, check: bool) -> Result<()> {
    let directory = root.join("schemas/generated");
    if !check {
        fs::create_dir_all(&directory)
            .with_context(|| format!("creating {}", directory.display()))?;
    }

    let mut differing = Vec::new();
    let mut expected = BTreeSet::new();
    let mut written = 0_usize;
    let mut removed = 0_usize;

    for entry in aep_schema::generated_schemas() {
        expected.insert(entry.filename.clone());
        let path = directory.join(&entry.filename);
        let generated = entry
            .to_json()
            .with_context(|| format!("serialising the {} schema", entry.name))?;

        if check {
            let committed =
                fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
            if committed != generated {
                differing.push(entry.filename.clone());
            }
        } else {
            let unchanged = fs::read_to_string(&path).is_ok_and(|committed| committed == generated);
            if !unchanged {
                fs::write(&path, &generated)
                    .with_context(|| format!("writing {}", path.display()))?;
                written += 1;
            }
        }
    }

    // The index is generated from the same list, so a schema cannot be added without appearing in
    // the documentation that tells a reader it exists.
    let index_path = directory.join(INDEX);
    let index = schema_index();
    expected.insert(INDEX.to_owned());
    if check {
        let committed = fs::read_to_string(&index_path)
            .with_context(|| format!("reading {}", index_path.display()))?;
        if committed != index {
            differing.push(INDEX.to_owned());
        }
    } else if !fs::read_to_string(&index_path).is_ok_and(|committed| committed == index) {
        fs::write(&index_path, &index)
            .with_context(|| format!("writing {}", index_path.display()))?;
        written += 1;
    }

    // Every file here is an output, so one that nothing generates is drift the other direction: a
    // schema that was renamed or withdrawn leaves its file behind, and a consumer validating
    // against that file goes on passing against a contract this repository no longer publishes.
    let mut orphaned = Vec::new();
    for entry in
        fs::read_dir(&directory).with_context(|| format!("reading {}", directory.display()))?
    {
        let entry = entry.with_context(|| format!("reading {}", directory.display()))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if expected.contains(&name) || !entry.path().is_file() {
            continue;
        }
        if check {
            orphaned.push(name);
        } else {
            fs::remove_file(entry.path())
                .with_context(|| format!("removing {}", entry.path().display()))?;
            removed += 1;
        }
    }
    orphaned.sort();

    if check {
        if differing.is_empty() && orphaned.is_empty() {
            println!("schemas are up to date");
            return Ok(());
        }
        let mut detail = String::new();
        if !differing.is_empty() {
            let _ = writeln!(
                detail,
                "{} file(s) differ from the Rust types: {}",
                differing.len(),
                differing.join(", ")
            );
        }
        if !orphaned.is_empty() {
            let _ = writeln!(
                detail,
                "{} file(s) are generated by nothing any more: {}",
                orphaned.len(),
                orphaned.join(", ")
            );
        }
        bail!("{detail}run `cargo xtask schema` and commit the result");
    }

    println!("schemas written: {written} changed, {removed} no longer generated");
    Ok(())
}

/// The index of `schemas/generated/`.
fn schema_index() -> String {
    let mut out = String::from(
        "# Generated schemas\n\n**Do not edit these files.** They are generated from the Rust \
         types by `cargo xtask schema`, and CI\nfails if they differ from what the types \
         produce.\n\nThey are the interoperability contract: anything that produces or consumes \
         these documents can\nvalidate them without linking the Rust crates.\n\n| file | Rust type \
         | describes |\n| --- | --- | --- |\n",
    );
    for entry in aep_schema::generated_schemas() {
        let _ = writeln!(
            out,
            "| [`{}`]({}) | `{}` | {} |",
            entry.filename, entry.filename, entry.name, entry.describes
        );
    }
    out
}

/// One projection, as `protocol ess generate` reports it.
struct Projection {
    /// What `--kind` calls it.
    name: String,
    /// The subdirectory its artifacts sit in.
    directory: String,
    /// One line saying what it proves.
    describes: String,
}

/// What the projections of a specification produced.
struct Generated {
    /// The specification and the build that produced this, for the index to attribute it.
    provenance: String,
    /// Each projection that ran, in the order the generator crate publishes them.
    projections: Vec<Projection>,
    /// Every artifact, keyed by its path relative to the output root.
    artifacts: BTreeMap<String, String>,
}

/// Writes or checks `generated/`.
///
/// The specification and the output tree are separate arguments because they are separate concerns:
/// the input is this repository's normative example and nothing else, while the output is what a test
/// has to be able to point somewhere harmless. Nothing here derives one from the other, so no test
/// can rewrite the committed tree by accident.
fn generate(spec: &Path, out: &Path, check: bool) -> Result<()> {
    let generated = projections(spec)?;

    // The index is written from the same report as the tree, so a projection cannot land
    // undocumented — and, being generated, it is not an orphan either.
    let mut expected = generated.artifacts.clone();
    expected.insert(INDEX.to_owned(), projection_index(&generated));

    let excluded: Vec<String> = PROJECTION_EXCLUSIONS
        .iter()
        .map(|subtree| (*subtree).to_owned())
        .collect();
    sync(
        out,
        &expected,
        check,
        &excluded,
        "projections",
        "the specification",
        "cargo xtask generate",
    )
}

/// Writes or checks a committed output tree against what a generator produced.
///
/// Shared by [`generate`], [`suite`] and [`synth`], because the rule is one rule: write only the
/// files whose content differs, delete the ones nothing generates any more, prune the directories
/// that leaves empty, and name the command that fixes it. A second copy of this would be a second
/// answer to "is the tree clean", which is the drift these tasks exist to catch, one level up.
///
/// `excluded` names subtrees and files under `out` that this task does not own — a nested task's
/// root, or what `cargo` writes while checking a generated workspace — and they are excluded from
/// the orphan scan and the prune, never from the expected-file comparison: an excluded path is
/// someone else's to check, not nobody's.
///
/// `noun`, `against` and `fix` are the only things that differ, and they are words in a message
/// rather than behaviour: a reader who runs the wrong task needs to be told which one to run, and
/// "the gate failed" is not that.
fn sync(
    out: &Path,
    expected: &BTreeMap<String, String>,
    check: bool,
    excluded: &[String],
    noun: &str,
    against: &str,
    fix: &str,
) -> Result<()> {
    let mut differing = Vec::new();
    let mut written = 0_usize;
    let mut removed = 0_usize;

    for (path, contents) in expected {
        let target = out.join(path);
        // A file that is not there is a file that differs, rather than an error. The first run of
        // `--check` on a tree nobody has written yet is the case that matters, and "no such file" is
        // a worse answer to it than the name of the task that fixes it.
        let committed = fs::read_to_string(&target).ok();
        if committed.as_deref() == Some(contents.as_str()) {
            continue;
        }
        if check {
            differing.push(path.clone());
            continue;
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
        }
        fs::write(&target, contents).with_context(|| format!("writing {}", target.display()))?;
        written += 1;
    }

    // The other direction: a committed artifact nothing produces any more. It is a contract this
    // repository has stopped standing behind, and a consumer validating against it goes on passing —
    // which a check that only compares what *is* generated will never notice.
    let mut orphaned = Vec::new();
    for path in committed_files(out, excluded)? {
        if expected.contains_key(&path) {
            continue;
        }
        if check {
            orphaned.push(path);
        } else {
            let target = out.join(&path);
            fs::remove_file(&target).with_context(|| format!("removing {}", target.display()))?;
            removed += 1;
        }
    }

    if check {
        if differing.is_empty() && orphaned.is_empty() {
            println!("{noun} are up to date");
            return Ok(());
        }
        let mut detail = String::new();
        if !differing.is_empty() {
            let _ = writeln!(
                detail,
                "{} file(s) differ from {against}: {}",
                differing.len(),
                differing.join(", ")
            );
        }
        if !orphaned.is_empty() {
            let _ = writeln!(
                detail,
                "{} file(s) are generated by nothing any more: {}",
                orphaned.len(),
                orphaned.join(", ")
            );
        }
        bail!("{detail}run `{fix}` and commit the result");
    }

    // Only if the tree exists: a write that produced nothing at all has no directories to prune, and
    // reading a directory that is not there is a different failure from a tree that is clean.
    if out.is_dir() {
        prune_empty_directories(out, "", excluded)?;
    }
    println!("{noun} written: {written} changed, {removed} no longer generated");
    Ok(())
}

/// `true` when `path` — relative, `/`-separated — is one of the excluded entries or inside one.
fn is_excluded(path: &str, excluded: &[String]) -> bool {
    excluded
        .iter()
        .any(|entry| path == entry || path.starts_with(&format!("{entry}/")))
}

/// Runs `protocol ess generate` over a specification and reads its report.
///
/// Through the command line rather than by linking `ess-gen`: what has to be committed is what
/// `protocol ess generate` produces, and a second in-process path to the same artifacts is a second
/// answer. Two answers is the drift this task exists to catch, one level up — the check would pass
/// while the command a person runs wrote something else.
fn projections(spec: &Path) -> Result<Generated> {
    let report = protocol_json(
        &["ess", "generate", "--format", "json", "--path"],
        spec,
        "generating the projections",
    )?;

    let mut projections = Vec::new();
    for projection in array(&report, "projections")? {
        projections.push(Projection {
            name: text(projection, "name")?,
            directory: text(projection, "directory")?,
            describes: text(projection, "describes")?,
        });
    }

    let mut artifacts = BTreeMap::new();
    for artifact in array(&report, "artifacts")? {
        artifacts.insert(text(artifact, "path")?, text(artifact, "contents")?);
    }

    let provenance = &report["provenance"];
    Ok(Generated {
        provenance: format!(
            "{} {} (model digest {})",
            text(provenance, "system")?,
            text(provenance, "specification_version")?,
            text(provenance, "source_digest")?
        ),
        projections,
        artifacts,
    })
}

/// Runs `protocol` over a specification with `--format json` and reads what it printed.
///
/// The one place this file starts a process, for the reason [`projections`] gives: what gets
/// committed has to be what the command a person runs produces, so both tasks go through the command
/// line rather than linking the library a second time.
fn protocol_json(args: &[&str], spec: &Path, doing: &str) -> Result<serde_json::Value> {
    // `CARGO` is set by the cargo that invoked this task, so the artifacts are produced by the
    // toolchain the caller is already on rather than by whichever cargo comes first on their PATH.
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let output = std::process::Command::new(&cargo)
        .args(["run", "--quiet", "--package", "protocol-cli", "--"])
        .args(args)
        .arg(spec)
        // The binary is built from this checkout, always: the output tree is a parameter, the
        // workspace it is generated by is not.
        .current_dir(workspace_root())
        .output()
        .with_context(|| format!("running {cargo:?} for {doing}"))?;

    if !output.status.success() {
        bail!(
            "`protocol {}` refused {}:\n{}{}",
            args.join(" "),
            spec.display(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    serde_json::from_slice(&output.stdout)
        .with_context(|| format!("reading what `protocol {}` printed", args.join(" ")))
}

// ---- the conformance suites --------------------------------------------------------------------

/// One specification's suite, as `protocol ess conform synthesize` reports it.
struct Suite {
    /// The directory under `examples/` it was synthesised from, which is also where it is filed.
    directory: String,
    /// The system and version the suite checks, and the digest of the model it was derived from.
    provenance: String,
    /// How many scenarios it holds.
    scenarios: u64,
    /// Every construct of the specification that got no scenario.
    refusals: Vec<Refused>,
    /// Its files, keyed by path relative to the suite's own directory.
    artifacts: BTreeMap<String, String>,
}

/// One construct the specification does not say enough about to test.
struct Refused {
    /// The stable code, such as `ESS-SYNTH-006`.
    code: String,
    /// The ESS element that has no scenario.
    subject: String,
    /// The scenario that would have existed, where the refusal is about one.
    scenario: Option<String>,
    /// What would have to change for it to become testable.
    help: String,
}

/// Writes or checks `suites/generated/`.
///
/// The specifications and the output tree are separate arguments for the reason [`generate`] gives:
/// a test has to be able to point the output somewhere harmless without also pointing the input at a
/// copy of the specification, because a copy is a second specification that drifts.
fn suite(specifications: &[PathBuf], out: &Path, check: bool) -> Result<()> {
    let mut suites = Vec::new();
    for specification in specifications {
        suites.push(suite_of(specification)?);
    }

    // Filed under the example directory rather than under the system name inside the specification.
    // Both are stable, and this one is the half a reader already knows: `suites/generated/billing/`
    // sits opposite `examples/billing/`, and finding one from the other takes no lookup.
    let mut expected = BTreeMap::new();
    for suite in &suites {
        for (path, contents) in &suite.artifacts {
            expected.insert(format!("{}/{path}", suite.directory), contents.clone());
        }
    }
    // Written from the same reports as the tree, so a suite cannot land undocumented — and, being
    // generated, it is not an orphan either.
    expected.insert(INDEX.to_owned(), suite_index(&suites));

    sync(
        out,
        &expected,
        check,
        &[],
        "suites",
        "the specifications",
        "cargo xtask suite",
    )
}

// ---- the synthesised workspaces ----------------------------------------------------------------

/// One specification's synthesised workspace, as `protocol ess synthesize` reports it.
struct Synthesized {
    /// The directory under `examples/` it was synthesised from, which is also where it is filed.
    directory: String,
    /// The system and version it was synthesised from, and the digest of the model.
    provenance: String,
    /// How many capabilities the plan marks generated.
    generated: u64,
    /// How many are the implementor's, each with a contract in the workspace's `PLAN.md`.
    obligations: u64,
    /// How many the synthesis refuses to represent.
    refused: u64,
    /// Its files, keyed by path relative to the workspace's own directory.
    artifacts: BTreeMap<String, String>,
}

/// Writes or checks `generated/rust/`, then proves each workspace still compiles.
///
/// The compile step runs in *both* modes and after the tree is settled, because the acceptance
/// criterion the wave sets is executed rather than asserted: a committed workspace that drifted
/// fails the diff, and one that matches but no longer compiles — a toolchain moved, a hand edit
/// slipped through a force-add — fails the check that actually claims "this builds".
fn synth(specifications: &[PathBuf], out: &Path, check: bool) -> Result<()> {
    let mut workspaces = Vec::new();
    for specification in specifications {
        workspaces.push(synth_of(specification)?);
    }

    // Filed under the example directory, exactly as the suites are: `generated/rust/billing/`
    // sits opposite `examples/billing/`, and finding one from the other takes no lookup.
    let mut expected = BTreeMap::new();
    let mut excluded = Vec::new();
    for workspace in &workspaces {
        for (path, contents) in &workspace.artifacts {
            expected.insert(format!("{}/{path}", workspace.directory), contents.clone());
        }
        // What `cargo check` writes beside a workspace while proving it compiles. Excluded from
        // the orphan scan — deleting the lock on every run would make the check fight the very
        // step below — and ignored by git, so neither is ever part of the committed tree.
        excluded.push(format!("{}/Cargo.lock", workspace.directory));
        excluded.push(format!("{}/target", workspace.directory));
    }
    expected.insert(INDEX.to_owned(), synth_index(&workspaces));

    sync(
        out,
        &expected,
        check,
        &excluded,
        "synthesised workspaces",
        "the specifications",
        "cargo xtask synth",
    )?;

    for workspace in &workspaces {
        check_generated_workspace(&out.join(&workspace.directory))?;
    }
    Ok(())
}

/// Runs `protocol ess synthesize` over one specification and reads its report.
fn synth_of(spec: &Path) -> Result<Synthesized> {
    let report = protocol_json(
        &["ess", "synthesize", "--format", "json", "--path"],
        spec,
        "synthesising the workspace",
    )?;

    let mut artifacts = BTreeMap::new();
    for artifact in array(&report, "artifacts")? {
        artifacts.insert(text(artifact, "path")?, text(artifact, "contents")?);
    }

    let provenance = &report["provenance"];
    Ok(Synthesized {
        directory: spec
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .with_context(|| {
                format!(
                    "{} has no directory name to file its workspace under",
                    spec.display()
                )
            })?,
        provenance: format!(
            "{} {} (model digest {})",
            text(provenance, "system")?,
            text(provenance, "specification_version")?,
            text(provenance, "source_digest")?
        ),
        generated: number(&report, "generated")?,
        obligations: number(&report, "obligations")?,
        refused: number(&report, "refused")?,
        artifacts,
    })
}

/// Runs `cargo check` inside one generated workspace.
///
/// The target directory is redirected into this repository's `target/` so the committed tree stays
/// source-only; the lock file cargo writes beside the generated manifest is gitignored and carved
/// out of the orphan scan, because it is the toolchain's answer, not the specification's.
fn check_generated_workspace(workspace: &Path) -> Result<()> {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let output = std::process::Command::new(&cargo)
        .arg("check")
        .current_dir(workspace)
        .env(
            "CARGO_TARGET_DIR",
            workspace_root().join("target/ess-synth"),
        )
        .output()
        .with_context(|| format!("running {cargo:?} in {}", workspace.display()))?;
    if !output.status.success() {
        bail!(
            "the synthesised workspace at {} does not compile — that is a defect in `ess-synth`, \
             not in the specification:\n{}{}",
            workspace.display(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(())
}

/// What `generated/rust/README.md` opens with, one line per line.
const SYNTH_INDEX_PREAMBLE: &[&str] = &[
    "# Synthesised Rust workspaces",
    "",
    "**Do not edit these files.** They are synthesised from the specifications under",
    "[`examples/`](../../examples) by `cargo xtask synth`, and CI fails if they differ from what",
    "the specifications determine — or if a workspace stops compiling.",
    "",
    "A workspace here is the part of an implementation that was never anyone's to write: the",
    "types, the states whose illegal transitions do not compile, the contracts. What remains",
    "deliberately unwritten is each workspace's `PLAN.md` — every capability of the specification",
    "with exactly one disposition: generated, an obligation carrying its contract, or a refusal",
    "carrying its reason. `Cargo.lock` and `target/` inside a workspace are written by `cargo",
    "check` and are not part of the committed tree.",
    "",
    "| workspace | generated from | generated | obligations | refused | plan |",
    "| --- | --- | --- | --- | --- | --- |",
];

/// The index of `generated/rust/`.
///
/// It lists the obligation and refusal counts because those are the numbers a change moves: a
/// specification that stops saying enough about a capability does not remove code noisily — the
/// plan quietly gains an obligation, which nothing but a diff of this table would surface.
fn synth_index(workspaces: &[Synthesized]) -> String {
    let mut out = String::new();
    for line in SYNTH_INDEX_PREAMBLE {
        out.push_str(line);
        out.push('\n');
    }
    for workspace in workspaces {
        let _ = writeln!(
            out,
            "| [`{directory}/`]({directory}) | {} | {} | {} | {} | \
             [`{directory}/PLAN.md`]({directory}/PLAN.md) |",
            workspace.provenance,
            workspace.generated,
            workspace.obligations,
            workspace.refused,
            directory = workspace.directory,
        );
    }
    out
}

/// Runs `protocol ess conform synthesize` over one specification and reads its report.
fn suite_of(spec: &Path) -> Result<Suite> {
    let report = protocol_json(
        &["ess", "conform", "synthesize", "--format", "json", "--path"],
        spec,
        "synthesising the conformance suite",
    )?;

    let mut artifacts = BTreeMap::new();
    for artifact in array(&report, "artifacts")? {
        artifacts.insert(text(artifact, "path")?, text(artifact, "contents")?);
    }

    let mut refusals = Vec::new();
    for refusal in array(&report, "refusals")? {
        refusals.push(Refused {
            code: text(refusal, "code")?,
            subject: text(refusal, "subject")?,
            // Absent for a refusal that is about no single scenario — a binding has four aspects,
            // and a refusal can be about the construct rather than about one of them.
            scenario: refusal["scenario"].as_str().map(ToOwned::to_owned),
            help: text(refusal, "help")?,
        });
    }

    let provenance = &report["provenance"];
    Ok(Suite {
        directory: spec
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .with_context(|| {
                format!(
                    "{} has no directory name to file its suite under",
                    spec.display()
                )
            })?,
        provenance: format!(
            "{} {} (model digest {})",
            text(provenance, "system")?,
            text(provenance, "specification_version")?,
            text(provenance, "spec_digest")?
        ),
        scenarios: number(&report, "scenarios")?,
        refusals,
        artifacts,
    })
}

/// What `suites/generated/README.md` opens with, one line per line.
///
/// A list rather than one long literal with escaped line breaks, because this file is Markdown a
/// person reads: where a line ends is a decision, and a `\`-continued literal hides it behind the
/// Rust source's own wrapping.
const SUITE_INDEX_PREAMBLE: &[&str] = &[
    "# Generated conformance suites",
    "",
    "**Do not edit these files.** They are generated from the specifications under",
    "[`examples/`](../../examples) by `cargo xtask suite`, and CI fails if they differ from what",
    "those specifications oblige.",
    "",
    "A suite is the other half of a specification: every check an implementation has to pass for",
    "the word *conformant* to mean anything about it. One JSON document per specification, keyed by",
    "scenario id, holding no handle into any particular compilation — so a runner in another",
    "language can read it, and a fault matrix can name a scenario by an id that does not move when",
    "a sibling is added.",
    "",
    "```console",
    "protocol ess conform run --suite suites/generated/billing/suite.json --target billing",
    "```",
    "",
    "| suite | checks | scenarios | no scenario | generated from |",
    "| --- | --- | --- | --- | --- |",
];

/// The heading the refusal tables sit under.
const SUITE_INDEX_REFUSALS: &[&str] = &[
    "",
    "## What no scenario covers",
    "",
    "A construct the specification does not say enough about to test is refused rather than quietly",
    "omitted (design §36). A refusal is a fact about the specification, not a gap in this file — and",
    "it is listed here rather than left in a command's output because a suite holding fewer checks",
    "than the specification requires is the one failure a passing run cannot show. Here it is a line",
    "in a diff instead.",
];

/// The index of `suites/generated/`.
///
/// It lists the refusals as well as the scenarios, and that is the part worth having. A construct a
/// specification stops saying enough about does not remove a scenario noisily — the suite simply
/// holds one fewer check, which no passing run can show. Written into the index, it becomes a line
/// in a diff that somebody has to approve.
fn suite_index(suites: &[Suite]) -> String {
    let mut out = String::new();
    for line in SUITE_INDEX_PREAMBLE {
        out.push_str(line);
        out.push('\n');
    }

    for suite in suites {
        for path in suite.artifacts.keys() {
            let _ = writeln!(
                out,
                "| [`{directory}/{path}`]({directory}/{path}) | {} | {} | {} | \
                 [`examples/{directory}`](../../examples/{directory}) |",
                suite.provenance,
                suite.scenarios,
                suite.refusals.len(),
                directory = suite.directory,
            );
        }
    }

    for line in SUITE_INDEX_REFUSALS {
        out.push_str(line);
        out.push('\n');
    }

    for suite in suites {
        let _ = write!(out, "\n### `{}`\n\n", suite.directory);
        if suite.refusals.is_empty() {
            // Said out loud: a heading with nothing under it reads as a rendering fault rather than
            // as a specification every construct of which produced a check.
            let _ = writeln!(
                out,
                "Every construct produced a scenario, and nothing is refused."
            );
            continue;
        }
        let _ = writeln!(out, "| code | element | the scenario that is missing |");
        let _ = writeln!(out, "| --- | --- | --- |");
        for refusal in &suite.refusals {
            // The scenario id, because it is the only thing that tells two refusals about one
            // element apart: an entity with five invariants no view publishes produces five rows
            // that are otherwise the same line five times.
            let _ = writeln!(
                out,
                "| `{}` | `{}` | {} |",
                refusal.code,
                refusal.subject,
                refusal
                    .scenario
                    .as_deref()
                    .map_or_else(|| "—".to_owned(), |id| format!("`{id}`"))
            );
        }
        let mut hints: BTreeSet<&str> = BTreeSet::new();
        for refusal in &suite.refusals {
            hints.insert(refusal.help.as_str());
        }
        let _ = writeln!(out, "\nWhat would close them:\n");
        for hint in hints {
            let _ = writeln!(out, "* {hint}");
        }
    }

    out
}

/// One string field of a report, or a message naming what was not there.
fn text(value: &serde_json::Value, field: &str) -> Result<String> {
    value[field]
        .as_str()
        .map(ToOwned::to_owned)
        .with_context(|| format!("the report has no string `{field}`"))
}

/// One numeric field of a report, or a message naming what was not there.
fn number(value: &serde_json::Value, field: &str) -> Result<u64> {
    value[field]
        .as_u64()
        .with_context(|| format!("the report has no whole number `{field}`"))
}

/// One list field of a report, or a message naming what was not there.
fn array<'a>(value: &'a serde_json::Value, field: &str) -> Result<&'a [serde_json::Value]> {
    value[field]
        .as_array()
        .map(Vec::as_slice)
        .with_context(|| format!("the report has no list `{field}`"))
}

/// Every file under `directory`, as `/`-separated paths relative to it, minus the excluded
/// subtrees — which belong to another task or to `cargo`, and are therefore not this scan's
/// orphans to report or delete.
///
/// Recursive, unlike the schema directory's flat scan, because a projection's artifacts sit in a
/// subdirectory of its own: a scan that read only the top level would report nothing at all and call
/// the tree clean.
fn committed_files(directory: &Path, excluded: &[String]) -> Result<BTreeSet<String>> {
    let mut found = BTreeSet::new();
    // A tree nobody has written yet holds no orphans, which is a different answer from "unreadable".
    if !directory.is_dir() {
        return Ok(found);
    }
    let mut pending = vec![(directory.to_path_buf(), String::new())];
    while let Some((path, prefix)) = pending.pop() {
        for entry in fs::read_dir(&path).with_context(|| format!("reading {}", path.display()))? {
            let entry = entry.with_context(|| format!("reading {}", path.display()))?;
            let name = entry.file_name().to_string_lossy().into_owned();
            let relative = if prefix.is_empty() {
                name
            } else {
                format!("{prefix}/{name}")
            };
            if is_excluded(&relative, excluded) {
                continue;
            }
            if entry.path().is_dir() {
                pending.push((entry.path(), relative));
            } else {
                found.insert(relative);
            }
        }
    }
    Ok(found)
}

/// Removes directories left holding nothing, and says whether `directory` itself is now empty.
///
/// A withdrawn projection's files are orphans and its directory is what survives them. An empty
/// `openapi/` in a committed tree reads as a projection that produces nothing, which is a different
/// claim from one this repository no longer publishes. An excluded subtree is neither entered nor
/// counted empty: what it holds is another owner's business.
fn prune_empty_directories(directory: &Path, prefix: &str, excluded: &[String]) -> Result<bool> {
    let mut empty = true;
    for entry in
        fs::read_dir(directory).with_context(|| format!("reading {}", directory.display()))?
    {
        let entry = entry.with_context(|| format!("reading {}", directory.display()))?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        let relative = if prefix.is_empty() {
            name
        } else {
            format!("{prefix}/{name}")
        };
        // A file keeps its directory, and so does an excluded subtree — not entered, because what
        // it holds is another owner's business, and never counted empty for the same reason.
        if !path.is_dir() || is_excluded(&relative, excluded) {
            empty = false;
        } else if prune_empty_directories(&path, &relative, excluded)? {
            fs::remove_dir(&path).with_context(|| format!("removing {}", path.display()))?;
        } else {
            empty = false;
        }
    }
    Ok(empty)
}

/// The index of `generated/`.
///
/// Written from the report rather than from a hand-kept list, for the same reason the schema index
/// is: a projection that produces files nothing tells a reader about is a projection nobody knows to
/// look at, and a list maintained by hand is a list that is wrong by the second projection.
fn projection_index(generated: &Generated) -> String {
    let mut out = format!(
        "# Generated projections\n\n**Do not edit these files.** They are generated from \
         [`{NORMATIVE_EXAMPLE}`](../{NORMATIVE_EXAMPLE}) by\n`cargo xtask generate`, and CI fails \
         if they differ from what the specification produces.\n\nEvery file here is a projection \
         of one model, so two of them disagreeing is a bug in one of them —\nand a file nothing \
         generates any more is a contract this repository no longer stands behind.\n\nGenerated \
         from {}.\n\n| projection | files | describes |\n| --- | --- | --- |\n",
        generated.provenance
    );

    for projection in &generated.projections {
        let _ = writeln!(
            out,
            "| `{}` | {} | {} |",
            projection.name,
            files_of(generated, projection).count(),
            projection.describes
        );
    }

    for projection in &generated.projections {
        let _ = write!(out, "\n## `{}`\n\n", projection.name);
        let mut listed = false;
        for path in files_of(generated, projection) {
            let _ = writeln!(out, "* [`{path}`]({path})");
            listed = true;
        }
        if !listed {
            // Said out loud, because a heading with nothing under it reads as a rendering fault
            // rather than as a projection that produced nothing.
            let _ = writeln!(out, "This projection produced no artifacts.");
        }
    }

    out
}

/// Every artifact one projection produced, in path order.
fn files_of<'a>(
    generated: &'a Generated,
    projection: &'a Projection,
) -> impl Iterator<Item = &'a str> {
    let prefix = format!("{}/", projection.directory);
    generated
        .artifacts
        .keys()
        .filter(move |path| path.starts_with(&prefix))
        .map(String::as_str)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{
        generate, schema, suite, synth, workspace_root, INDEX, NORMATIVE_EXAMPLE, PROJECTIONS,
        PROJECTION_EXCLUSIONS, SUITES, SUITE_SPECIFICATIONS, SYNTH, SYNTH_SPECIFICATIONS,
    };

    /// A scratch tree with a freshly generated `schemas/generated/` in it.
    fn generated(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(name);
        std::fs::remove_dir_all(&root).ok();
        schema(&root, false).expect("the schemas are written");
        root
    }

    #[test]
    fn the_check_refuses_a_schema_that_nothing_generates_any_more() {
        let root = generated("xtask-orphaned-schema");
        let orphan = root.join("schemas/generated/obsolete.schema.json");
        std::fs::write(&orphan, "{}\n").expect("the fixture is writable");

        let refusal = schema(&root, true).expect_err("a file nobody generates is drift");
        let reason = format!("{refusal:#}");
        assert!(reason.contains("obsolete.schema.json"), "{reason}");

        schema(&root, false).expect("the schemas are rewritten");
        assert!(
            !orphan.exists(),
            "what the check refuses, writing the schemas has to fix"
        );
        schema(&root, true).expect("the check passes once the orphan is gone");

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn the_index_is_generated_rather_than_orphaned() {
        // `README.md` is written by this task too, so the orphan scan must not report the very file
        // it just wrote — which would leave `--check` failing with no way to make it pass.
        let root = generated("xtask-generated-index");
        assert!(root.join("schemas/generated/README.md").is_file());
        schema(&root, true).expect("a freshly written tree is up to date");

        std::fs::remove_dir_all(&root).ok();
    }

    /// The normative example, read where it lives.
    ///
    /// Never a copy: this is the input, and a copy of it is a second specification that drifts from
    /// the one the repository publishes. Only the *output* tree is redirected below.
    fn specification() -> PathBuf {
        workspace_root().join(NORMATIVE_EXAMPLE)
    }

    /// A scratch tree holding freshly written projections of it.
    fn projected(name: &str) -> PathBuf {
        let out = std::env::temp_dir().join(name);
        std::fs::remove_dir_all(&out).ok();
        generate(&specification(), &out, false).expect("the projections are written");
        out
    }

    #[test]
    fn the_check_passes_on_a_freshly_written_tree() {
        let out = projected("xtask-projections-fresh");
        generate(&specification(), &out, true).expect("a freshly written tree is up to date");

        std::fs::remove_dir_all(&out).ok();
    }

    #[test]
    fn the_check_refuses_a_generated_file_somebody_edited() {
        // The whole point of the task: a hand-edited artifact is reverted by the next regeneration,
        // so it has to fail before it is committed rather than be silently undone afterwards.
        let out = projected("xtask-projections-edited");
        let edited = out.join(INDEX);
        std::fs::write(&edited, "# Notes I made in the wrong file\n")
            .expect("the fixture is writable");

        let refusal = generate(&specification(), &out, true)
            .expect_err("an edited artifact differs from the specification");
        let reason = format!("{refusal:#}");
        assert!(reason.contains(INDEX), "{reason}");
        assert!(
            reason.contains("cargo xtask generate"),
            "a refusal has to name what fixes it: {reason}"
        );

        generate(&specification(), &out, false).expect("the projections are rewritten");
        generate(&specification(), &out, true).expect("the check passes once they are");

        std::fs::remove_dir_all(&out).ok();
    }

    #[test]
    fn the_check_refuses_a_projection_that_nothing_generates_any_more() {
        // Drift the other direction, which comparing only what is generated cannot see: a withdrawn
        // projection leaves its files behind, and a consumer validating against them goes on passing
        // against a contract this repository no longer publishes.
        let out = projected("xtask-projections-orphaned");
        let orphan = out.join("withdrawn/service.yaml");
        std::fs::create_dir_all(orphan.parent().expect("a parent"))
            .expect("the fixture is writable");
        std::fs::write(&orphan, "openapi: 3.1.0\n").expect("the fixture is writable");

        let refusal =
            generate(&specification(), &out, true).expect_err("a file nobody generates is drift");
        let reason = format!("{refusal:#}");
        assert!(reason.contains("withdrawn/service.yaml"), "{reason}");

        generate(&specification(), &out, false).expect("the projections are rewritten");
        assert!(
            !orphan.exists(),
            "what the check refuses, writing the projections has to fix"
        );
        assert!(
            !orphan.parent().expect("a parent").exists(),
            "and an empty directory left behind still reads as a projection that produces nothing"
        );
        generate(&specification(), &out, true).expect("the check passes once the orphan is gone");

        std::fs::remove_dir_all(&out).ok();
    }

    #[test]
    fn the_projection_index_is_generated_rather_than_orphaned() {
        // The index is written by this task too, so the orphan scan must not report the very file it
        // just wrote — which would leave `--check` failing with no way to make it pass.
        let out = projected("xtask-projections-index");
        let index = std::fs::read_to_string(out.join(INDEX)).expect("the index is written");
        assert!(
            index.contains("cargo xtask generate"),
            "the index has to say what regenerates the tree: {index}"
        );
        generate(&specification(), &out, true).expect("a freshly written tree is up to date");

        std::fs::remove_dir_all(&out).ok();
    }

    #[test]
    fn every_projection_lands_in_the_written_tree_and_in_the_index() {
        // A projection that produces nothing still exits 0 and still leaves the tree looking
        // complete. What it does not do is appear as a directory with files in it.
        let out = projected("xtask-projections-complete");
        let index = std::fs::read_to_string(out.join(INDEX)).expect("the index is written");

        for projection in ["docs", "schema", "openapi", "asyncapi"] {
            let directory = out.join(projection);
            assert!(
                directory.is_dir(),
                "the `{projection}` projection produced no files"
            );
            assert!(
                std::fs::read_dir(&directory)
                    .expect("the projection is readable")
                    .next()
                    .is_some(),
                "the `{projection}` projection produced an empty directory"
            );
            assert!(
                index.contains(&format!("`{projection}`")),
                "the `{projection}` projection is missing from the index: {index}"
            );
        }

        std::fs::remove_dir_all(&out).ok();
    }
    // ---- the committed conformance suites --------------------------------------------------------

    /// The example specifications, read where they live.
    ///
    /// Never a copy, for the reason [`specification`] gives: a copy is a second specification, and it
    /// drifts. Only the *output* tree is redirected below.
    fn specifications() -> Vec<PathBuf> {
        SUITE_SPECIFICATIONS
            .iter()
            .map(|specification| workspace_root().join(specification))
            .collect()
    }

    /// A scratch tree holding freshly written suites.
    fn suited(name: &str) -> PathBuf {
        let out = std::env::temp_dir().join(name);
        std::fs::remove_dir_all(&out).ok();
        suite(&specifications(), &out, false).expect("the suites are written");
        out
    }

    #[test]
    fn the_suite_check_passes_on_a_freshly_written_tree() {
        let out = suited("xtask-suites-fresh");
        suite(&specifications(), &out, true).expect("a freshly written tree is up to date");

        std::fs::remove_dir_all(&out).ok();
    }

    #[test]
    fn every_specification_lands_as_its_own_suite_under_the_directory_it_was_written_in() {
        // The filing rule, asserted rather than described: `suites/generated/billing/suite.json`
        // opposite `examples/billing/`. A suite filed under the *system* name instead would put the
        // oracle fixture's suite in `oracle/`, and finding one from the other would take a lookup.
        let out = suited("xtask-suites-layout");
        for specification in SUITE_SPECIFICATIONS {
            let directory = specification
                .rsplit('/')
                .next()
                .expect("an example directory name");
            let written = out.join(directory).join("suite.json");
            assert!(
                written.is_file(),
                "`{specification}` produced no suite at {}",
                written.display()
            );
        }

        std::fs::remove_dir_all(&out).ok();
    }

    #[test]
    fn the_suite_check_refuses_a_suite_somebody_edited() {
        // The whole point of the task, and it bites harder here than it does for a projection: a
        // hand-edited suite is a check somebody removed from a contract, which no run of that suite
        // can ever report, because the check is simply not in it any more.
        let out = suited("xtask-suites-edited");
        let edited = out.join("billing/suite.json");
        let mut committed = std::fs::read_to_string(&edited).expect("the suite is readable");
        committed = committed.replace("\"scenarios\": {", "\"scenarios\": {\n    \"x\": null,");
        std::fs::write(&edited, committed).expect("the fixture is writable");

        let refusal = suite(&specifications(), &out, true)
            .expect_err("an edited suite differs from what the specification obliges");
        let reason = format!("{refusal:#}");
        assert!(reason.contains("billing/suite.json"), "{reason}");
        assert!(
            reason.contains("cargo xtask suite"),
            "a refusal has to name what fixes it: {reason}"
        );

        suite(&specifications(), &out, false).expect("the suites are rewritten");
        suite(&specifications(), &out, true).expect("the check passes once they are");

        std::fs::remove_dir_all(&out).ok();
    }

    #[test]
    fn the_suite_check_refuses_a_suite_that_nothing_generates_any_more() {
        // Drift the other direction: a specification that was withdrawn leaves its suite behind, and
        // an implementation goes on being held to a contract this repository no longer publishes —
        // which comparing only what *is* generated will never notice.
        let out = suited("xtask-suites-orphaned");
        let orphan = out.join("withdrawn/suite.json");
        std::fs::create_dir_all(orphan.parent().expect("a parent"))
            .expect("the fixture is writable");
        std::fs::write(&orphan, "{}\n").expect("the fixture is writable");

        let refusal =
            suite(&specifications(), &out, true).expect_err("a file nobody generates is drift");
        let reason = format!("{refusal:#}");
        assert!(reason.contains("withdrawn/suite.json"), "{reason}");

        suite(&specifications(), &out, false).expect("the suites are rewritten");
        assert!(
            !orphan.exists(),
            "what the check refuses, writing the suites has to fix"
        );
        assert!(
            !orphan.parent().expect("a parent").exists(),
            "and an empty directory left behind still reads as a specification that obliges nothing"
        );
        suite(&specifications(), &out, true).expect("the check passes once the orphan is gone");

        std::fs::remove_dir_all(&out).ok();
    }

    #[test]
    fn the_suite_index_is_generated_rather_than_orphaned_and_names_what_no_scenario_covers() {
        // Two claims in one fixture. The index is written by this task, so the orphan scan must not
        // report the very file it just wrote — and it has to carry the refusals, because a construct
        // the specification stops saying enough about removes a check silently otherwise.
        let out = suited("xtask-suites-index");
        let index = std::fs::read_to_string(out.join(INDEX)).expect("the index is written");
        assert!(
            index.contains("cargo xtask suite"),
            "the index has to say what regenerates the tree: {index}"
        );
        assert!(
            index.contains("What no scenario covers"),
            "a refusal is a fact about the specification and belongs in the diff: {index}"
        );
        assert!(
            index.contains("ESS-SYNTH-"),
            "the refusals themselves, with their codes, not just a heading: {index}"
        );
        suite(&specifications(), &out, true).expect("a freshly written tree is up to date");

        std::fs::remove_dir_all(&out).ok();
    }

    #[test]
    fn no_two_tasks_own_one_committed_tree_unless_the_outer_carves_the_inner_out() {
        // Both orphan scans are recursive and both delete what their own task did not produce, so
        // a tree with two owners is a tree where each task removes the other's committed contract.
        // Nesting is therefore only legal when the outer owner's scan *provably* does not enter
        // the inner root — the exclusion list is the mechanism, and this test is what makes an
        // exclusion list safe to have: an uncovered nesting fails, and so does an exclusion that
        // no task owns, because an unowned exclusion is a subtree nobody checks for drift.
        let owners: &[(&str, &[&str])] = &[
            ("schemas/generated", &[]),
            (PROJECTIONS, PROJECTION_EXCLUSIONS),
            (SUITES, &[]),
            (SYNTH, &[]),
        ];

        let covered = |exclusions: &[&str], relative: &str| {
            exclusions
                .iter()
                .any(|entry| relative == *entry || relative.starts_with(&format!("{entry}/")))
        };
        for (index, (one, exclusions_of_one)) in owners.iter().enumerate() {
            for (other, exclusions_of_other) in owners.iter().skip(index + 1) {
                if let Some(relative) = other.strip_prefix(&format!("{one}/")) {
                    assert!(
                        covered(exclusions_of_one, relative),
                        "`{other}` nests inside `{one}` without a carve-out, so `{one}`'s orphan \
                         scan deletes the other task's committed output"
                    );
                } else if let Some(relative) = one.strip_prefix(&format!("{other}/")) {
                    assert!(
                        covered(exclusions_of_other, relative),
                        "`{one}` nests inside `{other}` without a carve-out, so `{other}`'s \
                         orphan scan deletes the other task's committed output"
                    );
                }
            }
        }

        for (root, exclusions) in owners {
            for entry in *exclusions {
                let excluded_root = format!("{root}/{entry}");
                assert!(
                    owners.iter().any(|(owner, _)| *owner == excluded_root),
                    "`{root}` excludes `{entry}`, but no task owns `{excluded_root}` — an \
                     unowned exclusion is a subtree nothing checks for drift"
                );
            }
        }
    }

    // ---- the synthesised workspaces --------------------------------------------------------------

    /// The specifications a workspace is synthesised for, read where they live.
    fn synth_specifications() -> Vec<PathBuf> {
        SYNTH_SPECIFICATIONS
            .iter()
            .map(|specification| workspace_root().join(specification))
            .collect()
    }

    /// A scratch tree holding freshly synthesised workspaces, already compile-checked once.
    fn synthed(name: &str) -> PathBuf {
        let out = std::env::temp_dir().join(name);
        std::fs::remove_dir_all(&out).ok();
        synth(&synth_specifications(), &out, false).expect("the workspaces are written");
        out
    }

    #[test]
    fn the_synth_check_passes_on_a_freshly_written_tree() {
        let out = synthed("xtask-synth-fresh");
        synth(&synth_specifications(), &out, true).expect("a freshly written tree is up to date");

        std::fs::remove_dir_all(&out).ok();
    }

    #[test]
    fn the_synth_check_refuses_a_generated_file_somebody_edited() {
        // It bites harder here than for a projection: a hand edit in generated code is reverted by
        // the next regeneration, and in the meantime the committed workspace is code nobody's
        // specification stands behind.
        let out = synthed("xtask-synth-edited");
        let edited = out.join("billing/crates/billing-types/src/invoice.rs");
        let mut committed = std::fs::read_to_string(&edited).expect("the module is readable");
        committed.push_str("\n// a note in the wrong place\n");
        std::fs::write(&edited, committed).expect("the fixture is writable");

        let refusal = synth(&synth_specifications(), &out, true)
            .expect_err("an edited workspace differs from what the specification determines");
        let reason = format!("{refusal:#}");
        assert!(reason.contains("invoice.rs"), "{reason}");
        assert!(
            reason.contains("cargo xtask synth"),
            "a refusal has to name what fixes it: {reason}"
        );

        synth(&synth_specifications(), &out, false).expect("the workspaces are rewritten");
        synth(&synth_specifications(), &out, true).expect("the check passes once they are");

        std::fs::remove_dir_all(&out).ok();
    }

    #[test]
    fn the_synth_check_refuses_a_workspace_file_that_nothing_generates_any_more() {
        let out = synthed("xtask-synth-orphaned");
        let orphan = out.join("billing/crates/billing-types/src/withdrawn.rs");
        std::fs::write(&orphan, "// abandoned\n").expect("the fixture is writable");

        let refusal = synth(&synth_specifications(), &out, true)
            .expect_err("a file nobody generates is drift");
        let reason = format!("{refusal:#}");
        assert!(reason.contains("withdrawn.rs"), "{reason}");

        synth(&synth_specifications(), &out, false).expect("the workspaces are rewritten");
        assert!(
            !orphan.exists(),
            "what the check refuses, writing the workspaces has to fix"
        );
        synth(&synth_specifications(), &out, true).expect("the check passes once it is gone");

        std::fs::remove_dir_all(&out).ok();
    }

    #[test]
    fn what_cargo_writes_while_checking_is_not_this_tasks_orphan() {
        // `cargo check` writes `Cargo.lock` beside the generated manifest — the compile step above
        // has already done so by the time the *next* check runs. Treating either as an orphan
        // would make the check fight its own compile step: every second run red, fixed by the
        // deletion that makes the run after red again.
        let out = synthed("xtask-synth-transients");
        let lock = out.join("billing/Cargo.lock");
        assert!(
            lock.is_file(),
            "the compile step wrote a lock file; if it stopped, this test is checking nothing"
        );
        let scratch = out.join("billing/target/debug/marker");
        std::fs::create_dir_all(scratch.parent().expect("a parent"))
            .expect("the fixture is writable");
        std::fs::write(&scratch, "cargo writes here\n").expect("the fixture is writable");

        synth(&synth_specifications(), &out, true)
            .expect("what cargo writes is not drift in the committed tree");
        synth(&synth_specifications(), &out, false).expect("the workspaces are rewritten");
        assert!(
            lock.is_file() && scratch.is_file(),
            "and writing the workspaces leaves the toolchain's files alone"
        );

        std::fs::remove_dir_all(&out).ok();
    }

    #[test]
    fn the_projection_check_leaves_the_synthesised_tree_to_its_own_task() {
        // The carve-out, load-bearing: `generated/rust/` nests inside the projection task's root,
        // and before the exclusion existed this exact fixture would have been reported — and in
        // write mode deleted — as the projection task's orphan.
        let out = projected("xtask-projections-synth-carveout");
        let foreign = out.join("rust/billing/crates/billing-types/src/lib.rs");
        std::fs::create_dir_all(foreign.parent().expect("a parent"))
            .expect("the fixture is writable");
        std::fs::write(&foreign, "// the synth task's output, not this task's\n")
            .expect("the fixture is writable");

        generate(&specification(), &out, true)
            .expect("a file under the synth task's root is not the projection task's orphan");
        generate(&specification(), &out, false).expect("the projections are rewritten");
        assert!(
            foreign.is_file(),
            "writing the projections must not delete the synth task's committed output"
        );

        std::fs::remove_dir_all(&out).ok();
    }
}
