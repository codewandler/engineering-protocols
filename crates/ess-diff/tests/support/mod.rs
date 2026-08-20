//! Compiling an example specification, for the tests that need a real model rather than a fixture.
//!
//! In a subdirectory so that `cargo test` does not treat it as a test binary of its own, and shared
//! by `graph.rs` and `impact.rs` because both need exactly this and a third copy of a directory walk
//! is a third place for "which files are part of a specification" to be answered differently.

use std::path::{Path, PathBuf};

use ess_compiler::ir::EssIr;
use ess_compiler::resolve::compile;
use ess_compiler::source::SourceMap;
use ess_domain::spec::{RawSpecFile, Specification};
use ess_domain::system::Source;

/// Compiles a specification under `examples/`, by the path it lives at.
///
/// Read from the tree rather than inlined, on the argument `crates/ess-compiler/tests/billing.rs`
/// makes about the same choice: a copy of a specification inside a test drifts from the one a person
/// reads, and these fixtures' whole value is that a person can audit them by hand.
///
/// # Panics
///
/// If the example is missing, malformed or does not resolve. Each is a defect in the fixture rather
/// than in what is being tested, so it fails loudly and names the file.
pub fn compiled(example: &str) -> EssIr {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(example)
        .canonicalize()
        .unwrap_or_else(|error| panic!("`{example}` exists: {error}"));

    let mut files: Vec<PathBuf> = Vec::new();
    let mut pending = vec![root.clone()];
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(&directory).expect("the fixture is readable") {
            let path = entry.expect("an entry").path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|it| it == "yaml") {
                files.push(path);
            }
        }
    }
    files.sort();

    let parsed: Vec<(Source, RawSpecFile)> = files
        .iter()
        .map(|path| {
            let text = std::fs::read_to_string(path).expect("a readable document");
            let label = path
                .strip_prefix(&root)
                .expect("under the fixture root")
                .display()
                .to_string();
            let raw = RawSpecFile::parse(&text)
                .unwrap_or_else(|error| panic!("{label} is well formed: {error}"));
            (Source::new(label), raw)
        })
        .collect();

    let specification = Specification::assemble(parsed)
        .unwrap_or_else(|errors| panic!("`{example}` validates:\n{errors}"));
    compile(&specification, &SourceMap::new())
        .unwrap_or_else(|diagnostics| panic!("`{example}` resolves:\n{diagnostics}"))
}
