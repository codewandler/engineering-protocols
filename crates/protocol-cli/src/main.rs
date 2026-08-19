//! Reference CLI for AEP.
//!
//! **Status: skeleton only.** The subcommands are declared so the interface is reviewable, and
//! each reports that it is not yet implemented rather than pretending to succeed. Planned
//! behaviour is in `docs/design/consolidated-design-v0.2.md` §70.

use clap::{Parser, Subcommand};

/// Reference CLI for the Agentic Engineering Protocol.
#[derive(Debug, Parser)]
#[command(name = "protocol", about, version)]
struct Cli {
    /// What to do.
    #[command(subcommand)]
    command: Command,
}

/// The available subcommands.
#[derive(Debug, Subcommand)]
enum Command {
    /// Check that documents are structurally and semantically valid.
    Validate,
    /// Resolve a task into an execution plan.
    Resolve,
    /// Show what a protocol, principle, workflow or profile declares.
    Inspect,
    /// Evaluate an execution: what is owed, what is permitted, what is missing.
    Evaluate,
    /// Explain a decision: why an action was refused, or why a task is incomplete.
    Explain,
    /// Print the generated JSON Schemas.
    Schema,
    /// Run the conformance suites against a backend.
    Conformance,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let name = match cli.command {
        Command::Validate => "validate",
        Command::Resolve => "resolve",
        Command::Inspect => "inspect",
        Command::Evaluate => "evaluate",
        Command::Explain => "explain",
        Command::Schema => "schema",
        Command::Conformance => "conformance",
    };
    anyhow::bail!(
        "`protocol {name}` is not implemented yet; the engine it needs is tracked in \
         docs/design/reconciliation-v0.2.md §4"
    )
}
