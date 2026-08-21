//! Repository automation.
//!
//! `cargo xtask schema` regenerates the published JSON Schemas from the Rust types;
//! `cargo xtask generate` regenerates the committed projections of the normative specification;
//! `cargo xtask suite` regenerates the committed conformance suites the example specifications
//! oblige; `cargo xtask infra` regenerates the committed infrastructure IR compiled from the
//! example observation bundle. All take `--check`, which verifies the committed files still match instead of
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
/// Three, not one. `examples/billing/` is the normative example, and the suite for it is what
/// design §38 asks to be committed. `examples/oracle-fixture/` is here because `ess-conformance`'s
/// fault matrix names scenario ids from it — `handoff-on-placed/binding/flow` and its siblings —
/// and an id a matrix refers to has to be an id that cannot change by accident.
/// `examples/gatepass/` is the dual-target demonstration, and its suite is committed on the same
/// rule every other committed artifact follows: a specification this repository ships is a
/// specification whose derived documents are reviewable.
const SUITE_SPECIFICATIONS: &[&str] = &[
    "examples/billing",
    "examples/gatepass",
    "examples/oracle-fixture",
];

/// Where those suites are committed.
///
/// Beside `generated/` rather than inside it, for the reason the [module documentation](self) gives:
/// one owner per tree, because the orphan scan deletes what its own task does not produce. The
/// nesting mirrors `schemas/generated/`, which is this repository's existing shape for a committed
/// output tree with a drift check and a CI job of its own.
const SUITES: &str = "suites/generated";

/// The specifications a tree is synthesised for, and the targets each is emitted into.
///
/// Two, and not both into all three. The normative example is the specification wave 6 closes its
/// loop against and it is emitted into every target. `examples/gatepass/` is the dual-target
/// demonstration: a component whose own words say its callers are not deployed with it, so both
/// emitted applications serve the same HTTP surface. It is deliberately **not** emitted for the
/// browser — that target contains a system in one tab, and a surface reached over a network is one
/// a page would *call* rather than contain, which is a fourth target rather than this one.
const SYNTH_SPECIFICATIONS: &[(&str, &[&str])] = &[
    ("examples/billing", &["rust", "go", "web"]),
    ("examples/gatepass", &["rust", "go"]),
];

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

/// Where the Go modules the same specifications determine are committed.
///
/// A sibling of [`SYNTH`], not a subtree of it: one owner per committed tree, and the two are
/// written by one task from one plan. Carved out of the projection task's orphan scan the same
/// way, and held to it by `no_two_tasks_own_one_committed_tree_unless_the_outer_carves_the_inner_out`.
const SYNTH_GO: &str = "generated/go";

/// Where the browser realizations the same specifications determine are committed.
///
/// A third sibling of [`SYNTH`], written by the same task from the same plan, and carved out of
/// the projection task's orphan scan the same way. The compiled `.wasm` is deliberately **not**
/// committed: it is a build artifact, and the check below builds it rather than trusting a binary
/// nobody can diff.
const SYNTH_WEB: &str = "generated/web";

/// The hand-written host that links a realization into each committed browser bridge.
///
/// `(tree directory under SYNTH_WEB, host crate directory, the module file the host produces)`.
/// The bridge chooses no realization — gap register D-2 — so the module it builds on its own
/// refuses every command with the obligation it is owed. This host is what turns the same page
/// into a running system, and the smoke test below drives *its* module, through the page's own
/// glue.
const WEB_REALIZATIONS: &[(&str, &str, &str)] =
    &[("billing", "examples/billing-web", "billing_web_realized")];

/// The target the browser realization is built for.
const WASM: &str = "wasm32-unknown-unknown";

/// The demonstrations the gate executes: one specification, two synthesised applications, one
/// surface.
///
/// The proof W7.5 owes, and it is run rather than asserted. Both binaries are built from the
/// committed trees plus their hand-written realizations, started on ephemeral ports, driven through
/// the same exchanges, and compared — their startup records outside `runtime`, their status codes,
/// their bodies as values, and the two documents they publish about themselves byte for byte.
const DEMONSTRATIONS: &[Demonstration] = &[Demonstration {
    directory: "gatepass",
    component: "pass-service",
    package: "gatepass-realization",
    binary: "gatepass-server",
    module: "examples/gatepass-go-realization",
    command: "./cmd/gatepass-server",
    exchanges: &[
        Exchange {
            what: "a visit is registered",
            method: "POST",
            path: "/visits/commands/register-visit",
            body: Some(
                r#"{"visitor":"Ada Lovelace","building":"North","host":{"kind":"employee","value":"e-42"},"expected_minutes":90,"expected_stay":"PT90M","deposit":{"amount":"25.00","currency":"EUR"},"escorts":["Grace Hopper"],"notes":{"badge":"visitor"},"on_watchlist":false}"#,
            ),
            status: 202,
        },
        Exchange {
            what: "a visit of no length is refused, on domain grounds",
            method: "POST",
            path: "/visits/commands/register-visit",
            body: Some(
                r#"{"visitor":"Nobody At All","building":"South","host":{"kind":"contractor","value":"v-9"},"expected_minutes":0,"expected_stay":"PT0M","deposit":{"amount":"0.00","currency":"EUR"},"escorts":[],"notes":{},"on_watchlist":true}"#,
            ),
            status: 422,
        },
        Exchange {
            what: "the read-your-writes projection holds the visit that was just registered",
            method: "GET",
            path: "/visits/views/expected",
            body: None,
            status: 200,
        },
        Exchange {
            what: "and so does the unfiltered one, with every field the row declares",
            method: "GET",
            path: "/visits/views/by-id",
            body: None,
            status: 200,
        },
        Exchange {
            what: "a body the schema refuses is a bad request, not a domain refusal",
            method: "POST",
            path: "/visits/commands/register-visit",
            body: Some(r#"{"visitor":"Ada Lovelace"}"#),
            status: 400,
        },
        Exchange {
            what: "a path the contract does not declare is answered by neither",
            method: "GET",
            path: "/visits/commands/cancel-visit",
            body: None,
            status: 404,
        },
        Exchange {
            what: "and a declared path under an undeclared method is a 405",
            method: "GET",
            path: "/visits/commands/register-visit",
            body: None,
            status: 405,
        },
    ],
}];

/// One dual-target demonstration.
struct Demonstration {
    /// The tree under `generated/rust/` and `generated/go/`, which is the example's directory name.
    directory: &'static str,
    /// The served component, whose surface both applications answer.
    component: &'static str,
    /// The source-workspace package holding the Rust realization and its binary.
    package: &'static str,
    /// That package's binary.
    binary: &'static str,
    /// The Go realization module, relative to the repository root.
    module: &'static str,
    /// The command inside it that links the realization into the generated surface.
    command: &'static str,
    /// What both applications are driven through, in order.
    exchanges: &'static [Exchange],
}

/// One request both applications answer, and the status the contract says they answer it with.
struct Exchange {
    /// What this exchange proves, for the line the gate prints.
    what: &'static str,
    /// The method.
    method: &'static str,
    /// The path.
    path: &'static str,
    /// The body, where the request has one.
    body: Option<&'static str>,
    /// The status both must answer with.
    status: u16,
}

/// The committed realization each synthesised workspace is linked with and judged through.
///
/// `(workspace directory under SYNTH, source-workspace package)`. Wave 6's acceptance criterion is
/// executed here rather than asserted: `billing-realization`'s tests run the committed conformance
/// suite — unchanged, digest-checked against the workspace's plan — against the system its linker
/// assembles, and also hold that the deliberately corrupted linkage fails exactly the scenario that
/// exists to catch it. `gatepass-realization`'s hold its linker's obligation list equal to the
/// committed plan's, so a specification change that moves an obligation fails here rather than
/// leaving a linker resolving a list that no longer exists. The tests already run in the gate's
/// `test` step; they run here too because "the generated code passes the tests it did not write" is
/// this tree's acceptance criterion, and a check named `synth` that did not check it would certify
/// bytes rather than behaviour.
const REALIZATIONS: &[(&str, &str)] = &[
    ("billing", "billing-realization"),
    ("gatepass", "gatepass-realization"),
];

/// The subtrees of `generated/` the projection task does not own.
///
/// Exactly the nested owners' roots, relative to [`PROJECTIONS`] — the ownership test refuses an
/// entry here that no task owns, because an unowned exclusion is a hole in the drift check that
/// nobody scans.
const PROJECTION_EXCLUSIONS: &[&str] = &["go", "rust", "web"];

/// The example observation bundle, and the IR document committed beside it.
///
/// A pair rather than a tree: `observation.json` is an input (derived from a real scan, trimmed
/// by hand — see the fixture's README), `cluster.ir.json` is the one output `infra` owns, and a
/// single-file comparison needs no orphan scan. The IR is committed for the reason the suites
/// are: it is what IW2's graph and diagnosis will be built against, so its bytes must not move
/// unless the observation or the compiler moved them.
const OBSERVATION_FIXTURE: &str = "examples/k3d-dev-cluster/observation.json";

/// Where the compiled IR of [`OBSERVATION_FIXTURE`] is committed.
const OBSERVATION_IR: &str = "examples/k3d-dev-cluster/cluster.ir.json";

/// The second observation beside it: the same cluster one working day later.
///
/// Hand-derived from [`OBSERVATION_FIXTURE`] by twenty documented mutations — the fixture's
/// README lists each with the drift change kind it exists to exercise. An input, like its
/// original, and never regenerated by this task.
const OBSERVATION_DRIFTED: &str = "examples/k3d-dev-cluster/observation.drifted.json";

/// The desired-state specification committed beside the observation. An input.
const OBSERVATION_SPEC: &str = "examples/k3d-dev-cluster/expected.yaml";

/// Where the simulation of [`OBSERVATION_SPEC`] against [`OBSERVATION_FIXTURE`] is committed.
///
/// Committed and drift-checked for the reason the IR is: a three-valued verdict that changes
/// because an evaluator changed, rather than because a cluster or a specification changed, is
/// exactly the drift a reviewer must be shown rather than told about. It is also the only place
/// the `unknown` arm is held to its *bytes* — a rule that quietly starts answering `false` where
/// it answered `unknown` moves this file and nothing else.
const OBSERVATION_SIMULATION: &str = "examples/k3d-dev-cluster/simulation.json";

/// Where the drift between the two observations is committed.
const OBSERVATION_DRIFT: &str = "examples/k3d-dev-cluster/drift.json";

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
    /// Regenerate the committed Rust workspaces and Go modules the example specifications
    /// determine.
    Synth {
        /// Verify the committed tree matches — and still compiles — instead of writing it.
        #[arg(long)]
        check: bool,
    },
    /// Regenerate the committed infrastructure IR of the example observation.
    Infra {
        /// Verify the committed document matches instead of writing it.
        #[arg(long)]
        check: bool,
    },
    /// Format the source workspace's members — and only them.
    Fmt {
        /// Verify formatting instead of rewriting it.
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
        Command::Infra { check } => infra(&workspace_root(), check),
        Command::Fmt { check } => fmt(check),
        Command::Synth { check } => {
            let root = workspace_root();
            let specifications: Vec<(PathBuf, &[&str])> = SYNTH_SPECIFICATIONS
                .iter()
                .map(|(specification, targets)| (root.join(specification), *targets))
                .collect();
            synth(
                &specifications,
                &root.join(SYNTH),
                &root.join(SYNTH_GO),
                &root.join(SYNTH_WEB),
                check,
            )?;
            // After the trees are settled and built, never inside `synth`: a demonstration is a
            // property of the *committed* applications — the two binaries a reader can start — and
            // running it against a scratch copy would be running it against something nobody ships.
            for demonstration in DEMONSTRATIONS {
                demonstrate(demonstration)?;
            }
            Ok(())
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
    let mut stale_contracts = Vec::new();
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
            // A drifted file whose *contract digest* drifted is called out by name: the byte
            // comparison already fails it, but "this file differs" and "this file claims to derive
            // from a slice its slice no longer computes" are different findings, and the second is
            // a false claim about derivation that deserves its own sentence.
            if let (Some(committed), Some(fresh)) = (
                committed.as_deref().and_then(contract_digest_in),
                contract_digest_in(contents),
            ) {
                if committed != fresh {
                    stale_contracts
                        .push(format!("{path} (committed {committed}, computed {fresh})"));
                }
            }
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
            // The noun is in this line and not only in the "up to date" one, because two owned
            // trees can hold a file of the same relative name — `billing/PLAN.md` is in both
            // synthesised trees — and a reader told only the path does not know which drifted.
            let _ = writeln!(
                detail,
                "{} {noun} file(s) differ from {against}: {}",
                differing.len(),
                differing.join(", ")
            );
        }
        if !stale_contracts.is_empty() {
            let _ = writeln!(
                detail,
                "{} of them carry a stale contract digest — a false claim about the model slice \
                 they derive from: {}",
                stale_contracts.len(),
                stale_contracts.join(", ")
            );
        }
        if !orphaned.is_empty() {
            let _ = writeln!(
                detail,
                "{} {noun} file(s) are generated by nothing any more: {}",
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

/// The contract digest stamped in one artifact's text, or `None`.
///
/// The same two spellings `ess_gen::Provenance::read_digests` emits and reads — the comment-line
/// form and the serialized-field form — scanned here rather than through that crate, because this
/// task deliberately reaches every artifact through the command line and not by linking the
/// generators (see [`projections`]). The scan is deliberately dumb: 64 lower-case hex characters
/// after a marker, or nothing. `None` never fails a check by itself — the byte comparison decides
/// that — it only withholds the sharper message.
fn contract_digest_in(text: &str) -> Option<&str> {
    for marker in ["contract digest ", "\"contract_digest\": \""] {
        let Some(at) = text.find(marker) else {
            continue;
        };
        let rest = &text[at + marker.len()..];
        let hex: usize = rest
            .bytes()
            .take_while(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
            .count();
        if hex == 64 {
            return Some(&rest[..64]);
        }
    }
    None
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
            "{} {} (model digest {}, contract digest {})",
            text(provenance, "system")?,
            text(provenance, "specification_version")?,
            text(provenance, "source_digest")?,
            text(provenance, "contract_digest")?
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

/// Runs the protocol CLI and returns raw stdout — for output that is a document, not a value.
fn protocol_stdout(args: &[&str], doing: &str) -> Result<Vec<u8>> {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let output = std::process::Command::new(&cargo)
        .args(["run", "--quiet", "--package", "protocol-cli", "--"])
        .args(args)
        .current_dir(workspace_root())
        .output()
        .with_context(|| format!("running {cargo:?} for {doing}"))?;
    if !output.status.success() {
        bail!(
            "`protocol {}` refused:\n{}{}",
            args.join(" "),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(output.stdout)
}

// ---- the infrastructure IR ---------------------------------------------------------------------

/// Writes or checks the committed IR document compiled from the example observation.
///
/// The compiled bytes come from `protocol infra compile --format json`, which prints exactly what
/// `--out` persists — one producer, so this check can never disagree with the CLI about what a
/// compilation looks like. Byte comparison, not semantic: the document is content-addressed and
/// deterministic by construction, so any byte drift is a real change in the model, the compiler
/// or the fixture, and each of those must arrive as a reviewed diff of `cluster.ir.json`.
fn infra(root: &Path, check: bool) -> Result<()> {
    let printable = |relative: &str| -> Result<String> {
        root.join(relative)
            .to_str()
            .context("the fixture path is printable")
            .map(ToOwned::to_owned)
    };
    let fixture = printable(OBSERVATION_FIXTURE)?;
    let drifted = printable(OBSERVATION_DRIFTED)?;
    let spec = printable(OBSERVATION_SPEC)?;

    // Three outputs, one input set, one owner. Each is the CLI's own stdout, so this check can
    // never disagree with `protocol` about what a document looks like.
    let outputs = [
        (
            OBSERVATION_IR,
            protocol_stdout(
                &["infra", "compile", "--path", &fixture, "--format", "json"],
                "compiling the example observation",
            )?,
            OBSERVATION_FIXTURE,
        ),
        (
            OBSERVATION_SIMULATION,
            protocol_stdout(
                &[
                    "infra", "simulate", "--spec", &spec, "--path", &fixture, "--format", "json",
                ],
                "simulating the example specification",
            )?,
            OBSERVATION_SPEC,
        ),
        (
            OBSERVATION_DRIFT,
            protocol_stdout(
                &[
                    "infra", "diff", "--from", &fixture, "--to", &drifted, "--format", "json",
                ],
                "comparing the two example observations",
            )?,
            OBSERVATION_DRIFTED,
        ),
    ];

    for (relative, produced, source) in outputs {
        let committed = root.join(relative);
        if check {
            let existing = fs::read(&committed).with_context(|| {
                format!("reading {}; run `cargo xtask infra`", committed.display())
            })?;
            if existing != produced {
                bail!(
                    "{} no longer matches what {source} produces; run `cargo xtask infra` and \
                     review the diff",
                    committed.display()
                );
            }
            println!("ok: {relative} matches {source}");
        } else {
            fs::write(&committed, &produced)
                .with_context(|| format!("writing {}", committed.display()))?;
            println!("wrote {relative}");
        }
    }
    Ok(())
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
    /// How many guarantees the emission target holds more weakly than the first target does.
    ///
    /// Zero for a target that carried the plan whole, which is what `rust` does today. A number
    /// here is not a defect: it is the parity question answered in the index, where a reader
    /// choosing a target will look.
    weakened: usize,
    /// How many capabilities the target could not represent at all, and therefore did not emit.
    target_refused: usize,
    /// Its files, keyed by path relative to the module's own directory.
    artifacts: BTreeMap<String, String>,
}

/// Writes or checks `generated/rust/` and `generated/go/`, then proves each still builds.
///
/// One task, two trees, one plan: the emitters are siblings behind the synthesis seam, and a
/// check that regenerated only one of them would let the other drift silently until someone
/// happened to look. The build step runs in *both* modes and after each tree is settled, because
/// the acceptance criterion the waves set is executed rather than asserted: a committed tree that
/// drifted fails the diff, and one that matches but no longer compiles — a toolchain moved, a hand
/// edit slipped through a force-add — fails the check that actually claims "this builds".
fn synth(
    specifications: &[(PathBuf, &[&str])],
    out: &Path,
    go_out: &Path,
    web_out: &Path,
    check: bool,
) -> Result<()> {
    let mut workspaces = Vec::new();
    let mut modules = Vec::new();
    let mut pages = Vec::new();
    for (specification, targets) in specifications {
        for target in *targets {
            let synthesised = synth_of(specification, target)?;
            match *target {
                "rust" => workspaces.push(synthesised),
                "go" => modules.push(synthesised),
                "web" => pages.push(synthesised),
                other => bail!(
                    "`{other}` is not an emission target `cargo xtask synth` knows; the emitters \
                     are `rust`, `go` and `web`"
                ),
            }
        }
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
        "synthesised Rust workspaces",
        "the specifications",
        "cargo xtask synth",
    )?;

    // The Go tree needs no exclusions: `go build` writes into the build cache, never beside the
    // sources, so everything under this root is the emitter's and the orphan scan may see all of it.
    let mut go_expected = BTreeMap::new();
    for module in &modules {
        for (path, contents) in &module.artifacts {
            go_expected.insert(format!("{}/{path}", module.directory), contents.clone());
        }
    }
    go_expected.insert(INDEX.to_owned(), go_index(&modules));

    sync(
        go_out,
        &go_expected,
        check,
        &[],
        "synthesised Go modules",
        "the specifications",
        "cargo xtask synth",
    )?;

    // The browser tree needs the same two exclusions the Rust one does, and for the same reason:
    // `cargo build` writes a lock file and a target directory beside the manifest, and neither is
    // the emitter's. The compiled module lives under `target/` too, which is why nothing binary is
    // ever part of the committed tree.
    let mut web_expected = BTreeMap::new();
    let mut web_excluded = Vec::new();
    for page in &pages {
        for (path, contents) in &page.artifacts {
            web_expected.insert(format!("{}/{path}", page.directory), contents.clone());
        }
        web_excluded.push(format!("{}/Cargo.lock", page.directory));
        web_excluded.push(format!("{}/target", page.directory));
        // And the module itself, wherever a reader put it. The emitted `README.md` tells them to
        // copy a realized build in beside `index.html`, because that is the page's last candidate
        // path — so following the instructions the tree ships must not make the tree drift.
        web_excluded.push(format!(
            "{}/{}.wasm",
            page.directory,
            module_stem(&page.directory)
        ));
    }
    web_expected.insert(INDEX.to_owned(), web_index(&pages));

    sync(
        web_out,
        &web_expected,
        check,
        &web_excluded,
        "synthesised browser realizations",
        "the specifications",
        "cargo xtask synth",
    )?;

    for workspace in &workspaces {
        check_generated_workspace(&out.join(&workspace.directory))?;
    }
    for (workspace, package) in REALIZATIONS {
        check_realization(workspace, package)?;
    }
    for module in &modules {
        check_generated_module(&go_out.join(&module.directory))?;
    }
    for page in &pages {
        check_generated_page(&web_out.join(&page.directory), &page.directory)?;
    }
    Ok(())
}

// ---- the dual-target demonstration -------------------------------------------------------------

/// Runs one demonstration: two applications, one specification, one surface.
///
/// The claim W7.5 makes is not "both trees compile" — `synth-check` already had that — but "both
/// *behave the same way through the surface the specification determines*", and behaviour is only
/// ever checked by running it. So both binaries are built from the committed trees plus their
/// hand-written realizations, started on an ephemeral port each, driven through the same exchanges,
/// and compared four ways: the startup record outside `runtime`, the status of every answer, every
/// body as a value, and the two documents they publish about themselves byte for byte.
///
/// **Never skips.** A missing toolchain fails and names itself, exactly as every other step here
/// does. Ephemeral ports throughout, a generous readiness timeout, and both children reaped by a
/// guard whose `Drop` runs on every path out of this function.
fn demonstrate(demonstration: &Demonstration) -> Result<()> {
    let root = workspace_root();
    let rust_binary = build_rust_demonstration(demonstration, &root)?;
    let go_binary = build_go_demonstration(demonstration, &root)?;

    let mut rust = Application::start("rust", &rust_binary, &root)?;
    let mut go = Application::start("go", &go_binary, &root)?;

    let rust_startup = rust.startup()?;
    let go_startup = go.startup()?;
    compare_startup(demonstration, &rust_startup, &go_startup)?;

    for exchange in demonstration.exchanges {
        let from_rust = rust.exchange(exchange)?;
        let from_go = go.exchange(exchange)?;
        if from_rust.status != exchange.status || from_go.status != exchange.status {
            bail!(
                "`{} {}` — {} — was answered {} by the Rust application and {} by the Go one; the \
                 committed contract declares {}. A status is the outcome the specification \
                 declares, so two applications answering differently means one of them is not \
                 serving this specification",
                exchange.method,
                exchange.path,
                exchange.what,
                from_rust.status,
                from_go.status,
                exchange.status
            );
        }
        let rust_body = parse_body(&from_rust, "rust", exchange)?;
        let go_body = parse_body(&from_go, "go", exchange)?;
        // Values rather than bytes: a JSON object is unordered, and the two languages build one
        // through two writers. What must agree is what the body *says* — every member, at every
        // depth, including the order of any array, because an array's order is a claim.
        if rust_body != go_body {
            bail!(
                "`{} {}` — {} — was answered with two different bodies:\n  rust: {}\n  go:   {}",
                exchange.method,
                exchange.path,
                exchange.what,
                from_rust.body,
                from_go.body
            );
        }
    }

    compare_documents(demonstration, &root, &mut rust, &mut go)?;

    println!(
        "dual-target demonstration for `{}`: {} exchange(s) answered identically by both \
         applications, one startup record outside `runtime`, both published documents byte-identical",
        demonstration.directory,
        demonstration.exchanges.len()
    );
    Ok(())
}

/// Builds the Rust half: the committed workspace linked with its hand-written realization.
fn build_rust_demonstration(demonstration: &Demonstration, root: &Path) -> Result<PathBuf> {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let target = root.join("target/ess-synth-demonstration");
    let output = std::process::Command::new(&cargo)
        .args([
            "build",
            "--package",
            demonstration.package,
            "--bin",
            demonstration.binary,
        ])
        .current_dir(root)
        .env("CARGO_TARGET_DIR", &target)
        .output()
        .with_context(|| format!("running {cargo:?} build --bin {}", demonstration.binary))?;
    if !output.status.success() {
        bail!(
            "`{}` does not build, so the Rust half of the demonstration cannot run:\n{}{}",
            demonstration.binary,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(target.join("debug").join(demonstration.binary))
}

/// Builds the Go half: the committed module linked with its hand-written realization.
fn build_go_demonstration(demonstration: &Demonstration, root: &Path) -> Result<PathBuf> {
    let module = root.join(demonstration.module);
    let binary = root
        .join("target/ess-synth-demonstration")
        .join(format!("{}-go", demonstration.binary));
    if let Some(parent) = binary.parent() {
        fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
    }
    // The hand-written half is held to the same bar the generated one is. It is not emitted, so
    // `gofmt` may legitimately rewrite it — which is exactly why the check belongs here: a
    // realization that drifts out of `gofmt` or fails `go vet` is a realization nobody would
    // notice, because nothing else in this repository compiles it.
    let formatting = go_tool("gofmt", &["-l", "."], &module, "checking the formatting")?;
    let unformatted = String::from_utf8_lossy(&formatting.stdout);
    if !unformatted.trim().is_empty() {
        bail!(
            "`gofmt` would rewrite {} file(s) in `{}`:\n{unformatted}run `gofmt -w .` there",
            unformatted.split_whitespace().count(),
            demonstration.module
        );
    }
    let vetting = go_tool("go", &["vet", "./..."], &module, "vetting")?;
    if !vetting.status.success() {
        bail!(
            "`go vet ./...` fails in `{}`:\n{}{}",
            demonstration.module,
            String::from_utf8_lossy(&vetting.stdout),
            String::from_utf8_lossy(&vetting.stderr)
        );
    }
    let output = go_tool(
        "go",
        &[
            "build",
            "-o",
            &binary.to_string_lossy(),
            demonstration.command,
        ],
        &module,
        "building the Go half of the demonstration",
    )?;
    if !output.status.success() {
        bail!(
            "`{}` does not build, so the Go half of the demonstration cannot run:\n{}{}",
            demonstration.module,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(binary)
}

/// One running application, and the promise that it is reaped.
///
/// The child is killed and waited for by [`Drop`], so every path out of [`demonstrate`] — including
/// the `?` on the first comparison that fails — leaves no process behind. A gate that leaks a
/// listener leaks it into the next job.
struct Application {
    /// Which target it was synthesised into, for a message.
    language: &'static str,
    /// The process.
    child: std::process::Child,
    /// Its standard output, which is where the startup record arrives.
    lines: std::sync::mpsc::Receiver<String>,
    /// The port it bound, once the record has been read.
    port: u16,
}

/// How long to wait for an application to say it is listening.
///
/// Generous on purpose: this runs on CI machines under load, and a readiness timeout that is tight
/// enough to be occasionally wrong is a flaky gate, which is worse than a slow one.
const READY: std::time::Duration = std::time::Duration::from_secs(60);

/// How long to wait for one answer.
const ANSWER: std::time::Duration = std::time::Duration::from_secs(30);

impl Application {
    /// Starts one on an ephemeral port and begins reading its standard output.
    fn start(language: &'static str, binary: &Path, root: &Path) -> Result<Self> {
        let mut child = std::process::Command::new(binary)
            .current_dir(root)
            // Port 0, always: two applications run side by side here and a fixed port would make
            // this step fail whenever anything else on the machine happened to hold it.
            .env("PORT", "0")
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .with_context(|| format!("starting {}", binary.display()))?;
        let stdout = child
            .stdout
            .take()
            .context("the application was started with a piped standard output")?;
        let (sender, lines) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            use std::io::BufRead as _;
            for line in std::io::BufReader::new(stdout).lines() {
                let Ok(line) = line else { return };
                if sender.send(line).is_err() {
                    return;
                }
            }
        });
        Ok(Self {
            language,
            child,
            lines,
            port: 0,
        })
    }

    /// Reads the three startup lines, and learns the port from them.
    fn startup(&mut self) -> Result<Vec<serde_json::Value>> {
        let mut record = Vec::new();
        for expected in ["system.starting", "surface.serving", "system.ready"] {
            let line = self.lines.recv_timeout(READY).with_context(|| {
                format!(
                    "the {} application did not write its `{expected}` line within {} seconds; a \
                     startup record is how this step learns the port, so there is nothing to \
                     connect to",
                    self.language,
                    READY.as_secs()
                )
            })?;
            let value: serde_json::Value = serde_json::from_str(&line).with_context(|| {
                format!(
                    "the {} application's startup line is not JSON: {line}",
                    self.language
                )
            })?;
            if value["event"] != serde_json::Value::String(expected.to_owned()) {
                bail!(
                    "the {} application wrote `{}` where the startup record declares `{expected}`",
                    self.language,
                    value["event"]
                );
            }
            record.push(value);
        }
        self.port = u16::try_from(
            record[1]["runtime"]["port"]
                .as_u64()
                .context("the `surface.serving` line carries the port it bound in `runtime`")?,
        )
        .context("a port is sixteen bits")?;
        Ok(record)
    }

    /// Sends one request and reads the whole answer.
    fn exchange(&mut self, exchange: &Exchange) -> Result<Answer> {
        request(
            self.port,
            exchange.method,
            exchange.path,
            exchange.body,
            self.language,
        )
    }
}

impl Drop for Application {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// One answer, as much of it as this step reads.
struct Answer {
    /// The status code.
    status: u16,
    /// The media type the answer declared.
    content_type: String,
    /// The body.
    body: String,
}

/// One HTTP/1.1 request, over a fresh connection.
///
/// Hand-written for the reason everything else in this repository is: `task check` takes no
/// dependency it does not need, and a client that sends one request and reads one answer is sixty
/// lines. Both applications answer `Connection: close`, so the body is whatever arrives before the
/// socket does.
fn request(
    port: u16,
    method: &str,
    path: &str,
    body: Option<&str>,
    language: &str,
) -> Result<Answer> {
    use std::io::{Read as _, Write as _};

    let address = format!("127.0.0.1:{port}");
    let mut stream = std::net::TcpStream::connect(&address)
        .with_context(|| format!("connecting to the {language} application at {address}"))?;
    stream
        .set_read_timeout(Some(ANSWER))
        .context("setting a read timeout")?;
    let mut head = format!("{method} {path} HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\n");
    if let Some(body) = body {
        let _ = write!(
            head,
            "Content-Type: application/json\r\nContent-Length: {}\r\n",
            body.len()
        );
    }
    head.push_str("\r\n");
    stream
        .write_all(head.as_bytes())
        .with_context(|| format!("sending {method} {path} to the {language} application"))?;
    if let Some(body) = body {
        stream
            .write_all(body.as_bytes())
            .with_context(|| format!("sending the body of {method} {path}"))?;
    }
    stream.flush().context("flushing the request")?;

    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).with_context(|| {
        format!("reading the {language} application's answer to {method} {path}")
    })?;
    let text = String::from_utf8_lossy(&raw).into_owned();
    let (head, body) = text
        .split_once("\r\n\r\n")
        .with_context(|| format!("the {language} answer to {method} {path} has no header break"))?;
    let mut lines = head.split("\r\n");
    let status_line = lines
        .next()
        .with_context(|| format!("the {language} answer to {method} {path} has no status line"))?;
    let status: u16 = status_line
        .split(' ')
        .nth(1)
        .and_then(|code| code.parse().ok())
        .with_context(|| format!("`{status_line}` is not an HTTP status line"))?;
    let content_type = lines
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-type")
                .then(|| value.trim().to_owned())
        })
        .unwrap_or_default();
    Ok(Answer {
        status,
        content_type,
        body: body.to_owned(),
    })
}

/// One answer's body, as a value.
fn parse_body(answer: &Answer, language: &str, exchange: &Exchange) -> Result<serde_json::Value> {
    serde_json::from_str(&answer.body).with_context(|| {
        format!(
            "the {language} application's answer to `{} {}` is not JSON: {}",
            exchange.method, exchange.path, answer.body
        )
    })
}

/// Holds two startup records to being the same record outside `runtime`.
fn compare_startup(
    demonstration: &Demonstration,
    rust: &[serde_json::Value],
    go: &[serde_json::Value],
) -> Result<()> {
    for (position, (left, right)) in rust.iter().zip(go).enumerate() {
        let stripped_left = without_runtime(left)?;
        let stripped_right = without_runtime(right)?;
        if stripped_left != stripped_right {
            bail!(
                "the two applications synthesised from `{}` disagree about startup line {}:\n  \
                 rust: {stripped_left}\n  go:   {stripped_right}\nEverything outside `runtime` is \
                 derived from the specification, so a difference here is a difference in what the \
                 two believe they are",
                demonstration.directory,
                position + 1
            );
        }
    }
    Ok(())
}

/// One startup line without the member that is a fact about the process rather than the model.
///
/// It **removes** `runtime` and refuses a line that has none, rather than comparing a chosen list
/// of members: a normalizer that listed what to compare would silently stop comparing a member the
/// record gains tomorrow, which is exactly the drift this whole step exists to catch.
fn without_runtime(line: &serde_json::Value) -> Result<serde_json::Value> {
    let mut object = line
        .as_object()
        .context("a startup line is a JSON object")?
        .clone();
    object
        .remove("runtime")
        .context("a startup line carries a `runtime` member holding what is true of the process")?;
    Ok(serde_json::Value::Object(object))
}

/// Holds the two published documents to being the committed bytes, on both applications.
fn compare_documents(
    demonstration: &Demonstration,
    root: &Path,
    rust: &mut Application,
    go: &mut Application,
) -> Result<()> {
    for (path, committed, media) in [
        (
            "/openapi.json",
            root.join(SYNTH)
                .join(demonstration.directory)
                .join("crates")
                .join(format!("{}-server", demonstration.directory))
                .join("src")
                .join(format!("{}.openapi.json", demonstration.component)),
            "application/json",
        ),
        (
            "/docs",
            root.join(SYNTH)
                .join(demonstration.directory)
                .join("crates")
                .join(format!("{}-server", demonstration.directory))
                .join("src")
                .join(format!("{}.docs.md", demonstration.component)),
            "text/markdown; charset=utf-8",
        ),
    ] {
        let exchange = Exchange {
            what: "the document the surface publishes about itself",
            method: "GET",
            path,
            body: None,
            status: 200,
        };
        let from_rust = rust.exchange(&exchange)?;
        let from_go = go.exchange(&exchange)?;
        let expected = fs::read_to_string(&committed)
            .with_context(|| format!("reading {}", committed.display()))?;
        for (language, answer) in [("rust", &from_rust), ("go", &from_go)] {
            if answer.status != 200 {
                bail!(
                    "the {language} application answered `GET {path}` with {}",
                    answer.status
                );
            }
            if answer.content_type != media {
                bail!(
                    "the {language} application serves `{path}` as `{}` where the other serves \
                     `{media}`",
                    answer.content_type
                );
            }
            if answer.body != expected {
                bail!(
                    "the {language} application serves `{path}` with bytes that are not the \
                     committed {}'s. A served document that is not the reviewed one is a contract \
                     nobody approved",
                    committed.display()
                );
            }
        }
        // Asserted separately from the comparison against the committed file, because two
        // applications agreeing on the wrong bytes and two applications disagreeing are different
        // defects and deserve different sentences.
        if from_rust.body != from_go.body {
            bail!(
                "the two applications publish different bytes at `{path}`, so a caller reading the \
                 contract gets a different answer depending on which one it reached"
            );
        }
        // The Go tree carries its own copy, embedded from its own package directory. It must be
        // the same file: two committed copies of one document are two documents the moment one is
        // regenerated and the other is not.
        let go_copy = root
            .join(SYNTH_GO)
            .join(demonstration.directory)
            .join("server")
            .join(committed.file_name().unwrap_or_default());
        let go_committed = fs::read_to_string(&go_copy)
            .with_context(|| format!("reading {}", go_copy.display()))?;
        if go_committed != expected {
            bail!(
                "{} and {} are two copies of one document and their bytes differ",
                committed.display(),
                go_copy.display()
            );
        }
    }
    Ok(())
}

// ---- the browser realization ----------------------------------------------------------------------

/// Builds one committed browser tree, then holds the page and the module to each other.
///
/// Three questions, three messages. Does the emitted bridge still compile for the browser's
/// target? Does the page call exactly the exports the module has — which is this format's version
/// of a dangling reference, and the one nothing in HTML would refuse? And does the whole crossing
/// still work end to end, driven through the page's own glue by the host that links a realization?
fn check_generated_page(tree: &Path, directory: &str) -> Result<()> {
    let release = build_for_the_browser(tree, &format!("ess-synth-web/{directory}"))?;
    let bridge_module = release.join(format!("{}.wasm", bridge_module_stem(tree)?));
    let bridge_exports = wasm_exports(&bridge_module)?;

    let Some((_, host, host_module)) = WEB_REALIZATIONS
        .iter()
        .find(|(committed, _, _)| *committed == directory)
    else {
        // A committed tree with no host is a page nothing can drive, and this check would then be
        // certifying a module against a page neither of them ever answered.
        bail!(
            "`{SYNTH_WEB}/{directory}` is committed and no entry of `WEB_REALIZATIONS` links a              realization into it, so nothing in the gate ever runs it"
        );
    };
    let host_root = workspace_root().join(host);
    let host_release =
        build_for_the_browser(&host_root, &format!("ess-synth-web/{directory}-realized"))?;
    let realized_path = host_release.join(format!("{host_module}.wasm"));
    let realized_exports = wasm_exports(&realized_path)?;

    let referenced = page_references(tree)?;
    let offered: BTreeSet<String> = bridge_exports.union(&realized_exports).cloned().collect();
    if referenced != offered {
        let dangling: Vec<&String> = referenced.difference(&offered).collect();
        let unused: Vec<&String> = offered.difference(&referenced).collect();
        let mut detail = String::new();
        if !dangling.is_empty() {
            let _ = writeln!(
                detail,
                "the page calls {} export(s) no module has: {} — in HTML that fails at the \
                 click, silently, which is why it is checked here",
                dangling.len(),
                dangling
                    .iter()
                    .map(|name| format!("`{name}`"))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
        if !unused.is_empty() {
            let _ = writeln!(
                detail,
                "{} export(s) no page names: {} — an export nothing calls is a boundary that \
                 has outlived its caller",
                unused.len(),
                unused
                    .iter()
                    .map(|name| format!("`{name}`"))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
        bail!(
            "`{SYNTH_WEB}/{directory}` and its module disagree about the boundary:\n{detail}\
             that is a defect in `ess-synth`, not in any specification"
        );
    }
    if !bridge_exports.is_subset(&realized_exports) {
        bail!(
            "`{host}` does not re-export the bridge's own boundary, so the page cannot drive \
             it: {:?} are missing. A host's `cdylib` carries the `#[no_mangle]` items of \
             every `rlib` it links; if it stopped, the page now answers two different modules",
            bridge_exports
                .difference(&realized_exports)
                .collect::<Vec<_>>()
        );
    }

    smoke_the_boundary(tree, &host_root, &realized_path)
}

/// The module file name a reader is told to copy in beside the page.
///
/// Derived from the tree's own directory the way the emitter derives the crate name from the
/// system: `billing` builds `billing-web`, which builds `billing_web.wasm`. Kept here rather than
/// read off the tree because the exclusion list is computed before the tree is written, and a
/// carve-out that needed the tree to exist would be a carve-out that does not apply on a first run.
fn module_stem(directory: &str) -> String {
    format!("{directory}_web").replace('-', "_")
}

/// The file name stem of the module one committed browser tree builds.
///
/// Derived from the tree rather than fixed here: the emitter names the bridge crate after the
/// system, and this task is not the place a second answer to that question lives. Exactly one
/// crate, by construction — a browser tab holds one system, and the transport between components
/// is what the system crate already is.
fn bridge_module_stem(tree: &Path) -> Result<String> {
    let crates = tree.join("crates");
    let mut found: Vec<String> = fs::read_dir(&crates)
        .with_context(|| format!("reading {}", crates.display()))?
        .filter_map(std::result::Result::ok)
        .filter(|entry| entry.path().is_dir())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    found.sort();
    match found.len() {
        1 => Ok(found[0].replace('-', "_")),
        other => bail!(
            "{} holds {other} crate(s) and a browser tree has exactly one: {}",
            crates.display(),
            found.join(", ")
        ),
    }
}

/// Builds one crate for `wasm32-unknown-unknown`, or says what is missing.
///
/// **Never skips.** A gate step that quietly passes without its toolchain reads exactly like a
/// step that passed — the same rule the Go steps hold — so a missing target is a failure that
/// names the one command that installs it.
fn build_for_the_browser(crate_root: &Path, scratch: &str) -> Result<PathBuf> {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let target_dir = workspace_root().join("target").join(scratch);
    let output = std::process::Command::new(&cargo)
        .args(["build", "--release", "--target", WASM])
        .current_dir(crate_root)
        .env("CARGO_TARGET_DIR", &target_dir)
        .output()
        .with_context(|| {
            format!(
                "running {cargo:?} build --target {WASM} in {}",
                crate_root.display()
            )
        })?;
    if !output.status.success() {
        let reason = String::from_utf8_lossy(&output.stderr);
        if reason.contains("target may not be installed")
            || reason.contains("can't find crate for `core`")
        {
            bail!(
                "the `{WASM}` target is not installed, and `cargo xtask synth` needs it: the \
                 committed browser realization under `{SYNTH_WEB}/` is held to building for \
                 it. Run `rustup target add {WASM}`. This check never skips — a skipped \
                 check reads exactly like a passing one."
            );
        }
        bail!(
            "{} does not build for `{WASM}`:\n{}{reason}",
            crate_root.display(),
            String::from_utf8_lossy(&output.stdout),
        );
    }
    Ok(target_dir.join(WASM).join("release"))
}

/// Every `ess_*` symbol one `WebAssembly` module exports.
///
/// Read out of the module's own export section rather than out of the source that produced it,
/// because the question is what a browser will find — a `#[no_mangle]` item that the linker
/// dropped is exactly the failure this catches. Filtered to the emitter's own prefix: `memory` is
/// the module's, not a name any specification chose.
fn wasm_exports(module: &Path) -> Result<BTreeSet<String>> {
    let bytes = fs::read(module).with_context(|| format!("reading {}", module.display()))?;
    if bytes.len() < 8 || &bytes[0..4] != b"\0asm" {
        bail!("{} is not a WebAssembly module", module.display());
    }
    let mut at = 8;
    let mut exports = BTreeSet::new();
    while at < bytes.len() {
        let id = bytes[at];
        at += 1;
        let size = leb128(&bytes, &mut at)
            .with_context(|| format!("reading a section length in {}", module.display()))?;
        let end = at + size;
        if id == 7 {
            let mut cursor = at;
            let count = leb128(&bytes, &mut cursor).context("reading the export count")?;
            for _ in 0..count {
                let length = leb128(&bytes, &mut cursor).context("reading an export name")?;
                let name = String::from_utf8_lossy(&bytes[cursor..cursor + length]).into_owned();
                cursor += length + 1;
                let _ = leb128(&bytes, &mut cursor).context("reading an export index")?;
                if name.starts_with("ess_") {
                    exports.insert(name);
                }
            }
        }
        at = end;
    }
    Ok(exports)
}

/// One unsigned LEB128 integer, as `WebAssembly`'s binary format spells every length.
fn leb128(bytes: &[u8], at: &mut usize) -> Result<usize> {
    let mut value = 0_usize;
    let mut shift = 0;
    loop {
        let Some(byte) = bytes.get(*at).copied() else {
            bail!("the module ends inside a number");
        };
        *at += 1;
        value |= usize::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return Ok(value);
        }
        shift += 7;
        if shift > 63 {
            bail!("a length in the module does not fit in 64 bits");
        }
    }
}

/// Every `ess_*` symbol the committed page and its glue name.
fn page_references(tree: &Path) -> Result<BTreeSet<String>> {
    let mut referenced = BTreeSet::new();
    for file in ["index.html", "bridge.js"] {
        let path = tree.join(file);
        let text =
            fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
        let bytes = text.as_bytes();
        let mut at = 0;
        while let Some(found) = text[at..].find("ess_") {
            let start = at + found;
            let mut end = start + 4;
            while bytes
                .get(end)
                .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
            {
                end += 1;
            }
            referenced.insert(text[start..end].to_owned());
            at = end;
        }
    }
    Ok(referenced)
}

/// Runs the boundary smoke test: the page's own glue, the realized module, one Node process.
///
/// A boundary test and not a suite. The billing system's twenty-seven scenarios are the committed
/// conformance suite's, and [`check_realization`] runs them natively against the same realization;
/// what nothing else covers is the crossing this target adds. **Never skips**, for the reason
/// [`go_tool`] does not.
fn smoke_the_boundary(tree: &Path, host: &Path, module: &Path) -> Result<()> {
    let script = host.join("smoke.mjs");
    let output = std::process::Command::new("node")
        .arg(&script)
        .arg(tree.join("bridge.js"))
        .arg(module)
        .current_dir(workspace_root())
        .output()
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                anyhow::anyhow!(
                    "`node` is not on PATH, and `cargo xtask synth` needs it: the committed \
                     browser realization is proved by loading its module outside a browser \
                     and driving it through the page's own glue ({}). Install Node 18 or \
                     newer. This check never skips — a skipped check reads exactly like a \
                     passing one.",
                    script.display()
                )
            } else {
                anyhow::Error::new(error).context(format!("running {}", script.display()))
            }
        })?;
    if !output.status.success() {
        bail!(
            "the browser boundary no longer holds for `{}`:\n{}{}",
            tree.display(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    print!("{}", String::from_utf8_lossy(&output.stdout));
    Ok(())
}

/// Runs one realization package's tests: the committed suite against the linked system.
///
/// Both halves of the criterion live in those tests — the honest linkage passes 27 of 27, the
/// corrupted one fails exactly the scenario that exists to catch it — so a failure here means
/// the committed workspace, the committed suite and the hand-written realization no longer
/// agree, which no byte-diff can see.
fn check_realization(workspace: &str, package: &str) -> Result<()> {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let output = std::process::Command::new(&cargo)
        .args(["test", "--package", package])
        .current_dir(workspace_root())
        .output()
        .with_context(|| format!("running {cargo:?} test --package {package}"))?;
    if !output.status.success() {
        bail!(
            "`generated/rust/{workspace}` linked with `{package}` no longer holds what that \
             realization asserts about it — a scenario of the committed suite, the plan's \
             obligation list, or the corrupted linkage failing exactly the scenario that exists \
             to catch it:\n{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(())
}

/// Formats (or checks) exactly the source workspace's members.
///
/// Not `cargo fmt --all`: that flag also formats every member's *local path dependencies*, and
/// `examples/billing-realization` depends by path on the synthesised crates under
/// `generated/rust/` — a tree with one owner, the synth task, whose bytes are the emitter's and
/// are held byte-identical by `synth-check`. Two tasks owning one tree is exactly what this
/// file's module documentation forbids, so the member list comes from `cargo metadata` and the
/// generated workspaces are never rustfmt's to touch.
fn fmt(check: bool) -> Result<()> {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let output = std::process::Command::new(&cargo)
        .args(["metadata", "--no-deps", "--format-version", "1"])
        .current_dir(workspace_root())
        .output()
        .with_context(|| format!("running {cargo:?} metadata"))?;
    if !output.status.success() {
        bail!(
            "reading the workspace members failed:
{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let metadata: serde_json::Value =
        serde_json::from_slice(&output.stdout).context("parsing `cargo metadata` output")?;
    let mut arguments: Vec<String> = vec!["fmt".to_owned()];
    for package in array(&metadata, "packages")? {
        arguments.push("--package".to_owned());
        arguments.push(text(package, "name")?);
    }
    if check {
        arguments.push("--".to_owned());
        arguments.push("--check".to_owned());
    }
    let status = std::process::Command::new(&cargo)
        .args(&arguments)
        .current_dir(workspace_root())
        .status()
        .with_context(|| format!("running {cargo:?} fmt over the workspace members"))?;
    if !status.success() {
        bail!("formatting {}", if check { "differs" } else { "failed" });
    }
    Ok(())
}

/// Runs `protocol ess synthesize` over one specification, for one target, and reads its report.
///
/// Through the command line rather than by linking `ess-synth`, for the reason [`projections`]
/// gives: what has to be committed is what the command a person runs produces, and a second
/// in-process path to the same bytes is a second answer.
fn synth_of(spec: &Path, target: &str) -> Result<Synthesized> {
    let report = protocol_json(
        &[
            "ess",
            "synthesize",
            "--format",
            "json",
            "--target",
            target,
            "--path",
        ],
        spec,
        "synthesising the module",
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
            "{} {} (model digest {}, contract digest {})",
            text(provenance, "system")?,
            text(provenance, "specification_version")?,
            text(provenance, "source_digest")?,
            text(provenance, "contract_digest")?
        ),
        generated: number(&report, "generated")?,
        obligations: number(&report, "obligations")?,
        refused: number(&report, "refused")?,
        weakened: report["target_notes"]["weakenings"]
            .as_array()
            .map_or(0, Vec::len),
        target_refused: report["target_notes"]["refusals"]
            .as_array()
            .map_or(0, Vec::len),
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

/// Runs the Go toolchain over one generated module: already formatted, builds, vets.
///
/// Three questions, three messages, because they fail for different reasons and a reader who is
/// told "the Go step failed" has learned nothing. Formatting first: the emitter writes
/// `gofmt`-clean source by construction rather than shelling out to `gofmt` while emitting, so a
/// file `gofmt` would rewrite is a defect in `ess-synth` and not something a formatter should
/// quietly fix under a committed tree.
fn check_generated_module(module: &Path) -> Result<()> {
    let formatting = go_tool("gofmt", &["-l", "."], module, "checking the formatting")?;
    let unformatted = String::from_utf8_lossy(&formatting.stdout);
    if !unformatted.trim().is_empty() {
        bail!(
            "`gofmt` would rewrite {} in {} — the emitter's job is to write already-formatted \
             source, so this is a defect in `ess-synth` rather than something to reformat under a \
             committed tree:\n{unformatted}",
            unformatted.split_whitespace().count(),
            module.display()
        );
    }
    for (arguments, doing) in [
        (["build", "./..."], "building"),
        (["vet", "./..."], "vetting"),
    ] {
        let output = go_tool("go", &arguments, module, doing)?;
        if !output.status.success() {
            bail!(
                "`go {}` failed in {} — the module matches its specification and no longer {}, \
                 which is a defect in `ess-synth`, not in any specification:\n{}{}",
                arguments.join(" "),
                module.display(),
                if arguments[0] == "build" {
                    "compiles"
                } else {
                    "vets"
                },
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }
    Ok(())
}

/// Runs one Go toolchain program in a generated module, or says what is missing.
///
/// **Never skips.** A check that quietly passes when its toolchain is absent reads exactly like a
/// check that passed, which is the one failure mode a gate must not have — so a missing `go` is a
/// failure that names the toolchain and what it was for.
///
/// The environment is pinned to keep the promise `task check` makes: `GOPROXY=off` so no step can
/// reach a network (the module has no dependencies, and this makes that a rule rather than a
/// coincidence), and `GOTOOLCHAIN=local` so a `go` directive can never make the toolchain download
/// another one.
fn go_tool(
    program: &str,
    arguments: &[&str],
    module: &Path,
    doing: &str,
) -> Result<std::process::Output> {
    std::process::Command::new(program)
        .args(arguments)
        .current_dir(module)
        .env("GOPROXY", "off")
        .env("GOTOOLCHAIN", "local")
        .output()
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                anyhow::anyhow!(
                    "`{program}` is not on PATH, and `cargo xtask synth` needs the Go toolchain: \
                     the committed module under `{SYNTH_GO}/` is held to `gofmt -l`, `go build \
                     ./...` and `go vet ./...`. Install Go 1.21 or newer. This check never skips — \
                     a skipped check reads exactly like a passing one."
                )
            } else {
                anyhow::Error::new(error)
                    .context(format!("{doing} {} with {program}", module.display()))
            }
        })
}

/// What `generated/go/README.md` opens with, one line per line.
const GO_INDEX_PREAMBLE: &[&str] = &[
    "# Synthesised Go modules",
    "",
    "**Do not edit these files.** They are synthesised from the specifications under",
    "[`examples/`](../../examples) by `cargo xtask synth`, and CI fails if they differ from what",
    "the specifications determine — or if a module stops being `gofmt`-clean, stops compiling, or",
    "stops passing `go vet`.",
    "",
    "This tree is the **second emitter** behind the synthesis seam, and the reason it exists is",
    "that a claim about language-neutrality is worth exactly one test. Go was chosen because it",
    "has no sum type: every tagged union, every enum and every command outcome has to be encoded",
    "by hand — as a sealed interface, one unexported marker method and one struct per variant — or",
    "refused out loud. The plan did not change to admit it: each module's `PLAN.md` and `plan.json`",
    "are **byte-identical** to the ones in [`../rust`](../rust).",
    "",
    "What Go holds more weakly than Rust, and what it cannot represent at all, is in each module's",
    "`TARGET.md` — never in the plan, because a weakening is a fact about a language and the plan",
    "is a fact about the model. Standard library only, and a module path under the reserved",
    "`example.invalid` domain, so nothing here can be mistaken for something publishable.",
    "",
    "## The second transport, and the record two applications write",
    "",
    "A component whose specification says `reached_by: network` has callers that are not",
    "deployed with it, so its surface exists on a wire. *Which* wire is derived rather than",
    "chosen: this repository projects exactly one contract for a command surface — the OpenAPI",
    "document under [`generated/openapi/`](../openapi) — and an OpenAPI document is an HTTP",
    "contract, so a server speaking anything else would contradict the document committed beside",
    "it. The emitted surface answers exactly the paths that document declares, plus",
    "`GET /openapi.json` and `GET /docs`, which serve the committed contract and the committed",
    "prose byte for byte. A path the contract does not declare is a 404; a declared path under",
    "another method is a 405; neither is a status the contract declares, because both are facts",
    "about a transport rather than about a command.",
    "",
    "**The startup record.** Every served application writes three lines of JSON to standard",
    "output before it answers anything, and every member of them is derived from the",
    "specification — except `runtime`, which is the process's own: the language it was",
    "synthesised into, the address it bound and the port it took.",
    "",
    "| line | carries |",
    "| --- | --- |",
    "| `system.starting` | the system, its version, the model digest, the contract digest, every component, and the plan's disposition counts |",
    "| `surface.serving` | the served component, its declared reach, the transport, the number of routes, and every route as method, path, what it serves and the construct it serves |",
    "| `system.ready` | the system, and how many surfaces this process serves |",
    "",
    "The split is the whole comparison. Two applications synthesised from one specification must",
    "agree on every byte **outside** `runtime`, and `cargo xtask synth` starts both, reads their",
    "records, strips `runtime` and compares — so a member that moved into `runtime` to make a",
    "comparison pass would be a member that stopped being compared, and a member the record",
    "gains tomorrow is compared without anyone editing the comparison.",
    "",
    "The Go half of a served surface is `net/http` and `encoding/json`, both standard library, and",
    "generated codecs beside them: a generated type carries an unexported field, which",
    "`encoding/json` cannot see, and exporting it would undo the distinctness the newtype encoding",
    "exists for. The hand-written realization that links into it is a module of its own —",
    "[`examples/gatepass-go-realization`](../../examples/gatepass-go-realization) — reaching this",
    "tree through a filesystem `replace`, so nothing here resolves over a network either.",
    "",
    "| module | generated from | generated | obligations | refused | weakened | target-refused | plan | target notes |",
    "| --- | --- | --- | --- | --- | --- | --- | --- | --- |",
];

/// The index of `generated/go/`.
///
/// It carries two columns the Rust index does not, and they are the parity answer: how many
/// guarantees this target holds more weakly, and how many capabilities it could not represent. A
/// reader comparing targets reads this row before opening anything.
fn go_index(modules: &[Synthesized]) -> String {
    let mut out = String::new();
    for line in GO_INDEX_PREAMBLE {
        out.push_str(line);
        out.push('\n');
    }
    for module in modules {
        let _ = writeln!(
            out,
            "| [`{directory}/`]({directory}) | {} | {} | {} | {} | {} | {} | \
             [`{directory}/PLAN.md`]({directory}/PLAN.md) | \
             [`{directory}/TARGET.md`]({directory}/TARGET.md) |",
            module.provenance,
            module.generated,
            module.obligations,
            module.refused,
            module.weakened,
            module.target_refused,
            directory = module.directory,
        );
    }
    out
}

/// What `generated/web/README.md` opens with, one line per line.
const WEB_INDEX_PREAMBLE: &[&str] = &[
    "# Synthesised browser realizations",
    "",
    "**Do not edit these files.** They are synthesised from the specifications under",
    "[`examples/`](../../examples) by `cargo xtask synth`, and CI fails if they differ from what",
    "the specifications determine, if a tree stops building for `wasm32-unknown-unknown`, or if a",
    "page calls an export its module does not have.",
    "",
    "This tree is the **third emitter** behind the synthesis seam, and the first one a person can",
    "click. It is not a fourth rendering of the model: it is the *boundary* around the Rust",
    "target's system — JSON in over linear memory, JSON out — beside a `catalog.json` the page",
    "builds itself from. Nothing about any system is typed into the HTML: the command list, the",
    "input forms, the event names, the views and the lifecycles all come from the catalogue, so a",
    "specification that changes changes the page in the same regeneration.",
    "",
    "The plan did not change to admit it: each tree's `PLAN.md` and `plan.json` are",
    "**byte-identical** to the ones in [`../rust`](../rust) and [`../go`](../go). What a browser",
    "holds more weakly — a boundary that carries no types, instances observable only through",
    "declared views, a number format narrower than the model's — is in each tree's `TARGET.md`.",
    "",
    "The compiled `.wasm` is **not committed**: it is a build artifact, and `cargo xtask synth`",
    "builds it rather than trusting a binary nobody can diff. The bridge chooses no realization",
    "(gap register D-2), so the module it builds alone answers every command with the obligation",
    "it is owed; [`examples/billing-web`](../../examples/billing-web) is the hand-written host that",
    "links one in, and the gate drives *its* module through the page's own `bridge.js`.",
    "",
    "| tree | generated from | generated | obligations | refused | weakened | target-refused | plan | target notes |",
    "| --- | --- | --- | --- | --- | --- | --- | --- | --- |",
];

/// The index of `generated/web/`.
fn web_index(pages: &[Synthesized]) -> String {
    let mut out = String::new();
    for line in WEB_INDEX_PREAMBLE {
        out.push_str(line);
        out.push('\n');
    }
    for page in pages {
        let _ = writeln!(
            out,
            "| [`{directory}/`]({directory}) | {} | {} | {} | {} | {} | {} | \
             [`{directory}/PLAN.md`]({directory}/PLAN.md) | \
             [`{directory}/TARGET.md`]({directory}/TARGET.md) |",
            page.provenance,
            page.generated,
            page.obligations,
            page.refused,
            page.weakened,
            page.target_refused,
            directory = page.directory,
        );
    }
    out
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
    "types, the states whose illegal transitions do not compile, the contracts, one crate per",
    "component holding its port, and one system crate holding the bindings and the one transport",
    "the specification's own delivery words require. What remains deliberately unwritten is each",
    "workspace's `PLAN.md` — every capability of the specification with exactly one disposition:",
    "generated, an obligation carrying its contract, or a refusal carrying its reason — and every",
    "obligation is also a typed stub in the workspace, refusing with a value that names it.",
    "`Cargo.lock` and `target/` inside a workspace are written by `cargo check` and are not part",
    "of the committed tree.",
    "",
    "The other half of the bargain is hand-written, and lives outside this tree because the",
    "ownership boundary is absolute: [`examples/billing-realization`](../../examples/billing-realization)",
    "implements each obligation against its contract, and its linker assembles components and",
    "implementations into a runnable system without ever choosing — zero implementations for an",
    "obligation is an unsatisfied obligation, two is an ambiguity error naming both (gap register",
    "D-2). `cargo xtask synth` then executes the committed conformance suite, unchanged, against",
    "that linked system: 27 of 27 scenarios must pass, and the deliberately corrupted variant",
    "beside the honest one must fail exactly the scenario that exists to catch it.",
    "",
    "## The second transport, and the record two applications write",
    "",
    "A component whose specification says `reached_by: network` has callers that are not",
    "deployed with it, so its surface exists on a wire. *Which* wire is derived rather than",
    "chosen: this repository projects exactly one contract for a command surface — the OpenAPI",
    "document under [`generated/openapi/`](../openapi) — and an OpenAPI document is an HTTP",
    "contract, so a server speaking anything else would contradict the document committed beside",
    "it. The emitted surface answers exactly the paths that document declares, plus",
    "`GET /openapi.json` and `GET /docs`, which serve the committed contract and the committed",
    "prose byte for byte. A path the contract does not declare is a 404; a declared path under",
    "another method is a 405; neither is a status the contract declares, because both are facts",
    "about a transport rather than about a command.",
    "",
    "**The startup record.** Every served application writes three lines of JSON to standard",
    "output before it answers anything, and every member of them is derived from the",
    "specification — except `runtime`, which is the process's own: the language it was",
    "synthesised into, the address it bound and the port it took.",
    "",
    "| line | carries |",
    "| --- | --- |",
    "| `system.starting` | the system, its version, the model digest, the contract digest, every component, and the plan's disposition counts |",
    "| `surface.serving` | the served component, its declared reach, the transport, the number of routes, and every route as method, path, what it serves and the construct it serves |",
    "| `system.ready` | the system, and how many surfaces this process serves |",
    "",
    "The split is the whole comparison. Two applications synthesised from one specification must",
    "agree on every byte **outside** `runtime`, and `cargo xtask synth` starts both, reads their",
    "records, strips `runtime` and compares — so a member that moved into `runtime` to make a",
    "comparison pass would be a member that stopped being compared, and a member the record",
    "gains tomorrow is compared without anyone editing the comparison.",
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
            "{} {} (model digest {}, contract digest {})",
            text(provenance, "system")?,
            text(provenance, "specification_version")?,
            text(provenance, "spec_digest")?,
            text(provenance, "contract_digest")?
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
        contract_digest_in, generate, go_tool, schema, suite, synth, workspace_root, INDEX,
        NORMATIVE_EXAMPLE, PROJECTIONS, PROJECTION_EXCLUSIONS, SUITES, SUITE_SPECIFICATIONS, SYNTH,
        SYNTH_GO, SYNTH_SPECIFICATIONS, SYNTH_WEB,
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
    fn the_contract_digest_scanner_reads_both_stamped_forms_and_refuses_damage() {
        let hex = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        assert_eq!(
            contract_digest_in(&format!("# contract digest {hex}\n")),
            Some(hex)
        );
        assert_eq!(
            contract_digest_in(&format!("    \"contract_digest\": \"{hex}\",\n")),
            Some(hex)
        );
        // Damage reads as nothing, so the sharper message is withheld and the byte comparison
        // still decides — never the other way round.
        assert_eq!(
            contract_digest_in(&format!("# contract digest {}", &hex[..16])),
            None
        );
        assert_eq!(contract_digest_in("# model digest only"), None);
    }

    #[test]
    fn a_stale_contract_digest_is_called_out_as_a_false_claim_about_derivation() {
        // W7.1's drift-check extension, verified by doing exactly what it exists to catch: edit a
        // committed artifact's contract digest and nothing else. The byte comparison would already
        // fail the file; the check must additionally say *why this failure is worse* — a claim of
        // derivation the slice no longer computes.
        let out = projected("xtask-projections-stale-contract");
        let target = out.join("docs/README.md");
        let honest = std::fs::read_to_string(&target).expect("the projection exists");
        let stamped = contract_digest_in(&honest)
            .expect("a generated artifact carries its contract digest")
            .to_owned();
        std::fs::write(
            &target,
            honest.replace(
                &stamped,
                "0000000000000000000000000000000000000000000000000000000000000000",
            ),
        )
        .expect("the fixture is writable");

        let refusal = generate(&specification(), &out, true)
            .expect_err("a stale contract digest fails the check");
        let reason = format!("{refusal:#}");
        assert!(
            reason.contains("stale contract digest"),
            "the refusal names the defect class: {reason}"
        );
        assert!(
            reason.contains("false claim"),
            "the refusal says why it is worse than drift: {reason}"
        );
        assert!(
            reason.contains("docs/README.md"),
            "the refusal names the file: {reason}"
        );

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
            (SYNTH_GO, &[]),
            (SYNTH_WEB, &[]),
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

    /// The specifications a tree is synthesised for, read where they live, with their targets.
    fn synth_specifications() -> Vec<(PathBuf, &'static [&'static str])> {
        SYNTH_SPECIFICATIONS
            .iter()
            .map(|(specification, targets)| (workspace_root().join(specification), *targets))
            .collect()
    }

    /// A scratch root holding all three freshly synthesised trees, already built once.
    ///
    /// One root with the three owners under it, so a test naming a file says which target it is
    /// about — `rust/billing/...`, `go/billing/...` or `web/billing/...` — and the checks are
    /// exercised together the way the task runs them.
    fn synthed(name: &str) -> PathBuf {
        let out = std::env::temp_dir().join(name);
        std::fs::remove_dir_all(&out).ok();
        synth_both(&out, false).expect("all three trees are written");
        out
    }

    /// Checks or rewrites all three trees under one scratch root.
    ///
    /// One root with the three owners under it, which is also what makes the browser tree
    /// buildable in a scratch directory: its manifest reaches the Rust target's crates by a
    /// relative path, and the layout it expects is exactly this one.
    fn synth_both(out: &std::path::Path, check: bool) -> super::Result<()> {
        synth(
            &synth_specifications(),
            &out.join("rust"),
            &out.join("go"),
            &out.join("web"),
            check,
        )
    }

    #[test]
    fn the_synth_check_passes_on_a_freshly_written_tree() {
        let out = synthed("xtask-synth-fresh");
        synth_both(&out, true).expect("a freshly written tree is up to date");

        std::fs::remove_dir_all(&out).ok();
    }

    #[test]
    fn the_synth_check_refuses_a_generated_file_somebody_edited() {
        // It bites harder here than for a projection: a hand edit in generated code is reverted by
        // the next regeneration, and in the meantime the committed workspace is code nobody's
        // specification stands behind.
        let out = synthed("xtask-synth-edited");
        let edited = out.join("rust/billing/crates/billing-types/src/invoice.rs");
        let mut committed = std::fs::read_to_string(&edited).expect("the module is readable");
        committed.push_str("\n// a note in the wrong place\n");
        std::fs::write(&edited, committed).expect("the fixture is writable");

        let refusal = synth_both(&out, true)
            .expect_err("an edited workspace differs from what the specification determines");
        let reason = format!("{refusal:#}");
        assert!(reason.contains("invoice.rs"), "{reason}");
        assert!(
            reason.contains("cargo xtask synth"),
            "a refusal has to name what fixes it: {reason}"
        );

        synth_both(&out, false).expect("the workspaces are rewritten");
        synth_both(&out, true).expect("the check passes once they are");

        std::fs::remove_dir_all(&out).ok();
    }

    #[test]
    fn the_synth_check_refuses_a_workspace_file_that_nothing_generates_any_more() {
        let out = synthed("xtask-synth-orphaned");
        let orphan = out.join("rust/billing/crates/billing-types/src/withdrawn.rs");
        std::fs::write(&orphan, "// abandoned\n").expect("the fixture is writable");

        let refusal = synth_both(&out, true).expect_err("a file nobody generates is drift");
        let reason = format!("{refusal:#}");
        assert!(reason.contains("withdrawn.rs"), "{reason}");

        synth_both(&out, false).expect("the workspaces are rewritten");
        assert!(
            !orphan.exists(),
            "what the check refuses, writing the workspaces has to fix"
        );
        synth_both(&out, true).expect("the check passes once it is gone");

        std::fs::remove_dir_all(&out).ok();
    }

    #[test]
    fn what_cargo_writes_while_checking_is_not_this_tasks_orphan() {
        // `cargo check` writes `Cargo.lock` beside the generated manifest — the compile step above
        // has already done so by the time the *next* check runs. Treating either as an orphan
        // would make the check fight its own compile step: every second run red, fixed by the
        // deletion that makes the run after red again.
        let out = synthed("xtask-synth-transients");
        let lock = out.join("rust/billing/Cargo.lock");
        assert!(
            lock.is_file(),
            "the compile step wrote a lock file; if it stopped, this test is checking nothing"
        );
        let scratch = out.join("rust/billing/target/debug/marker");
        std::fs::create_dir_all(scratch.parent().expect("a parent"))
            .expect("the fixture is writable");
        std::fs::write(&scratch, "cargo writes here\n").expect("the fixture is writable");

        synth_both(&out, true).expect("what cargo writes is not drift in the committed tree");
        synth_both(&out, false).expect("the workspaces are rewritten");
        assert!(
            lock.is_file() && scratch.is_file(),
            "and writing the workspaces leaves the toolchain's files alone"
        );

        std::fs::remove_dir_all(&out).ok();
    }

    #[test]
    fn a_missing_go_toolchain_fails_naming_it_rather_than_skipping() {
        // The one behaviour a gate must never have: a check that quietly passes because its
        // toolchain is absent reads exactly like a check that passed. Verified by asking for a
        // program that cannot exist, which is the only way to reach this path on a machine that
        // has Go installed.
        let refusal = go_tool(
            "go-that-is-not-installed",
            &["version"],
            &workspace_root(),
            "probing",
        )
        .expect_err("a toolchain that is not on PATH cannot be used");
        let reason = format!("{refusal:#}");
        assert!(
            reason.contains("go-that-is-not-installed"),
            "the failure names the missing program: {reason}"
        );
        assert!(
            reason.contains("never skips"),
            "and says why it is a failure rather than a skip: {reason}"
        );
    }

    #[test]
    fn the_synth_check_refuses_an_edited_go_file_the_way_it_refuses_an_edited_rust_one() {
        // The second emitter's tree is committed on the same terms as the first's: a hand edit is
        // reverted by the next regeneration, and until then the committed module is code nobody's
        // specification stands behind. Without this the Go tree would be written and never checked.
        let out = synthed("xtask-synth-go-edited");
        let edited = out.join("go/billing/types/invoice/invoice.go");
        let mut committed = std::fs::read_to_string(&edited).expect("the package is readable");
        committed.push_str("\n// a note in the wrong place\n");
        std::fs::write(&edited, committed).expect("the fixture is writable");

        let refusal = synth_both(&out, true)
            .expect_err("an edited module differs from what the specification determines");
        let reason = format!("{refusal:#}");
        assert!(reason.contains("invoice.go"), "{reason}");
        assert!(
            reason.contains("synthesised Go modules"),
            "the refusal has to say which of the two trees drifted: {reason}"
        );
        assert!(
            reason.contains("cargo xtask synth"),
            "a refusal has to name what fixes it: {reason}"
        );

        synth_both(&out, false).expect("the modules are rewritten");
        synth_both(&out, true).expect("the check passes once they are");

        std::fs::remove_dir_all(&out).ok();
    }

    #[test]
    fn the_two_targets_commit_the_same_plan_byte_for_byte() {
        // The seam's own claim, checked where the committed bytes are rather than only where the
        // emitter runs: if the plan had to change to admit the second target, these two files
        // would differ and the language-neutrality the whole design rests on would be prose.
        let out = synthed("xtask-synth-plan-parity");
        for plan in ["PLAN.md", "plan.json"] {
            let rust = std::fs::read_to_string(out.join(format!("rust/billing/{plan}")))
                .expect("the Rust tree carries the plan");
            let go = std::fs::read_to_string(out.join(format!("go/billing/{plan}")))
                .expect("the Go tree carries the plan");
            assert_eq!(
                rust, go,
                "`{plan}` differs between the committed trees, so the plan is not \
                 language-neutral after all"
            );
        }

        std::fs::remove_dir_all(&out).ok();
    }

    #[test]
    fn the_projection_check_leaves_the_go_tree_to_its_own_task() {
        // The second carve-out, for the reason the first exists: `generated/go/` nests inside the
        // projection task's root, and an unexcluded nested owner is a tree the outer task deletes.
        let out = projected("xtask-projections-go-carveout");
        let foreign = out.join("go/billing/types/invoice/invoice.go");
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
