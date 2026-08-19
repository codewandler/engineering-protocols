//! Repository automation.
//!
//! `cargo xtask schema` regenerates the published JSON Schemas from the Rust types;
//! `cargo xtask schema --check` verifies the committed files still match, which is what CI runs.
//! Schemas are outputs: editing `schemas/generated/` by hand is always wrong, because the next
//! regeneration silently reverts it.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};

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
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Schema { check } => schema(&workspace_root(), check),
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
    let mut written = 0_usize;

    for entry in aep_schema::generated_schemas() {
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

    if check {
        if differing.is_empty() {
            println!("schemas are up to date");
            return Ok(());
        }
        bail!(
            "{} schema(s) differ from the Rust types: {}\nrun `cargo xtask schema` and commit the \
             result",
            differing.len(),
            differing.join(", ")
        );
    }

    println!("schemas written: {written} changed");
    Ok(())
}
