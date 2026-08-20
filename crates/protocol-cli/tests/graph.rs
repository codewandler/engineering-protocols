//! One system graph, however you ask for it.
//!
//! `ess-gen`'s `tests/agreement.rs` exists because three projections each carried a private copy of
//! one type mapping, all seventeen comparable pairs disagreed, and nothing in the build compared
//! them. The system graph is the same shape of risk with a shorter fuse: `protocol ess graph`
//! prints it and `generated/docs/README.md` opens with it, so two renderings of one picture are
//! published from one repository and a reader who sees both expects them to match.
//!
//! They did not. Before `ess-gen::graph` existed, the CLI built its own graph and the documentation
//! page built another, and the two answered differently:
//!
//! * the page drew the **actors** and their grants; the CLI drew no actor at all, so
//!   `protocol ess graph` answered "what causes what" while hiding who is allowed to start it;
//! * the page put a command in the component that `accepts:` it and an event in every component
//!   that `publishes:` it; the CLI put both in whichever component `owns:` the bounded context —
//!   a decomposition the specification never declared, and one `ess-domain`'s `component` module
//!   deliberately allows to differ.
//!
//! So this file compares what the two *publish*, not what they compute: it runs the real binary for
//! one and the real binary's `generate` verb for the other, and requires the diagram to be equal.
//!
//! # The one difference that is not a disagreement
//!
//! The Markdown fence. A page has to wrap its diagram in ` ```mermaid ` or a renderer shows the
//! source; a CLI writing to a pipe must not, or the first thing anyone does with the output is
//! delete three characters off each end. So the fence lines are removed and **nothing else is** —
//! not whitespace, not node identifiers, not edge order. A normalisation is a claim that a
//! difference does not matter, and the difference this test exists to catch is exactly one edge.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// The repository root.
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the workspace root exists")
}

/// Runs `protocol` with `args`, from the repository root.
fn protocol(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_protocol"))
        .args(args)
        .current_dir(root())
        .output()
        .expect("the protocol binary runs")
}

/// Standard output as a string, or a panic naming the exit code and what was said instead.
fn stdout(args: &[&str]) -> String {
    let output = protocol(args);
    assert_eq!(
        output.status.code(),
        Some(0),
        "`protocol {}` failed: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// The specification both renderings are read from.
const SPECIFICATION: &str = "examples/billing";

/// What a Mermaid block opens with in Markdown.
const FENCE: &str = "```mermaid\n";

/// The diagram `protocol ess graph --format mermaid` prints.
fn cli_diagram() -> String {
    stdout(&[
        "ess",
        "graph",
        "--path",
        SPECIFICATION,
        "--format",
        "mermaid",
    ])
}

/// The diagram the generated `docs/README.md` opens with, unfenced.
///
/// Through `ess generate --format json` rather than through `--out`: the page's *contents* are what
/// is being compared, and a temporary directory in the middle would only add a way for the test to
/// fail for a reason that is not about the graph.
fn page_diagram() -> String {
    let generated = stdout(&[
        "ess",
        "generate",
        "--path",
        SPECIFICATION,
        "--kind",
        "docs",
        "--format",
        "json",
    ]);
    let parsed: serde_json::Value =
        serde_json::from_str(&generated).expect("the generated artifacts are valid JSON");
    let readme = parsed["artifacts"]
        .as_array()
        .expect("artifacts is a list")
        .iter()
        .find(|artifact| artifact["path"] == "docs/README.md")
        .and_then(|artifact| artifact["contents"].as_str())
        .expect("the documentation projection writes an index page")
        .to_owned();

    assert_eq!(
        readme.matches(FENCE).count(),
        1,
        "the index page carries exactly one Mermaid block; with a second, this test would be \
         comparing whichever one it found first"
    );
    let start = readme.find(FENCE).expect("the fence was just counted") + FENCE.len();
    let length = readme[start..]
        .find("```")
        .expect("a fence that opens is closed");
    readme[start..start + length].to_owned()
}

#[test]
fn the_graph_the_cli_prints_is_the_graph_the_documentation_page_shows() {
    let from_cli = cli_diagram();
    let from_page = page_diagram();

    // Reaching the state where the rule is load-bearing: a comparison of two empty strings would
    // pass whatever the renderers did.
    assert!(
        from_cli.starts_with("flowchart TB\n") && from_cli.lines().count() > 10,
        "the CLI printed no diagram to compare: {from_cli}"
    );

    assert_eq!(
        from_cli, from_page,
        "`protocol ess graph --format mermaid` and the diagram on `docs/README.md` are one \
         renderer over one graph; a difference here is two pictures of one system published from \
         one repository"
    );
}

#[test]
fn the_mermaid_the_cli_prints_carries_no_fence_of_its_own() {
    let diagram = cli_diagram();

    assert!(
        !diagram.contains("```"),
        "this output is redirected into a file and fenced by whoever wants it fenced; a fence here \
         would be three characters every caller has to strip: {diagram}"
    );
    assert!(
        diagram.ends_with("| cmd0\n"),
        "the last line is the binding edge, and it ends the output with a newline so a shell \
         redirect produces a well-formed file: {diagram}"
    );
}

#[test]
fn two_mermaid_renderings_of_one_specification_are_byte_identical() {
    // Review F8: determinism asserted is determinism untested. Node identifiers here are indices
    // into a `BTreeMap` order, and an index into an unordered iteration would differ across runs.
    assert_eq!(
        cli_diagram(),
        cli_diagram(),
        "two runs over one specification must produce identical bytes"
    );
}

#[test]
fn the_graph_holds_the_actors_and_the_commands_they_may_invoke() {
    let diagram = cli_diagram();

    // The half the CLI used to omit. `Auditor` is in the example precisely because it may invoke
    // nothing: an actor with no outgoing edge is a grant list the model states, not an arrow
    // somebody forgot, and a renderer that drops unconnected nodes would drop exactly that.
    assert!(
        diagram.contains("        who0[\"billing.invoice.Auditor\"]\n"),
        "`Auditor` may invoke nothing and is still on the graph: {diagram}"
    );
    assert!(
        diagram.contains("    who1 -->|\"may invoke\"| cmd2\n"),
        "`Customer` may invoke `CreateInvoice`, and a grant is an edge: {diagram}"
    );
}
