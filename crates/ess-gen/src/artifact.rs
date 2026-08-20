//! What a generator produces, and the one trait every generator implements.

use std::collections::BTreeMap;

use ess_compiler::EssIr;

use crate::provenance::Provenance;

/// One generated file.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Artifact {
    /// Where it goes, relative to the output root. Always `/`-separated.
    pub path: String,
    /// Its contents.
    pub contents: String,
}

impl Artifact {
    /// Builds one.
    pub fn new(path: impl Into<String>, contents: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            contents: contents.into(),
        }
    }
}

/// A projection of the model.
///
/// One trait rather than one crate per projection (review F9): what differs between `OpenAPI` and
/// Markdown is the body, not how it is invoked, and eleven crates cost review attention every time
/// someone opens the tree.
pub trait Generator {
    /// What this projection is called on the command line — `docs`, `openapi`.
    fn name(&self) -> &'static str;

    /// One line for `--help` and for the generated index.
    fn describes(&self) -> &'static str;

    /// The subdirectory its artifacts go in, relative to the output root.
    fn directory(&self) -> &'static str;

    /// Generates every artifact, in a stable order.
    ///
    /// Infallible on purpose. A generator reaching a construct it cannot project is a gap in this
    /// crate, not a fault in the specification — and the specification has already been refused if it
    /// was wrong, because this takes an [`EssIr`] and there is no way to hold one that did not
    /// resolve. So there is nothing left for a `Result` to report.
    fn generate(&self, ir: &EssIr, provenance: &Provenance) -> Vec<Artifact>;
}

/// Runs one generator and returns its artifacts keyed by path.
///
/// Keyed, and therefore deduplicated: two artifacts claiming one path means the second silently
/// overwrites the first, and the output tree looks complete while missing a file.
pub fn run(
    generator: &dyn Generator,
    ir: &EssIr,
) -> Result<BTreeMap<String, Artifact>, DuplicatePath> {
    let provenance = Provenance::of(ir);
    let mut out: BTreeMap<String, Artifact> = BTreeMap::new();
    for artifact in generator.generate(ir, &provenance) {
        let path = format!("{}/{}", generator.directory(), artifact.path);
        if out.contains_key(&path) {
            return Err(DuplicatePath {
                generator: generator.name(),
                path,
            });
        }
        out.insert(path.clone(), Artifact::new(path, artifact.contents));
    }
    Ok(out)
}

/// Two artifacts claimed one path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicatePath {
    /// Which generator.
    pub generator: &'static str,
    /// The path claimed twice.
    pub path: String,
}

impl std::fmt::Display for DuplicatePath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "the `{}` generator produced two artifacts at `{}`; the second would overwrite the \
             first and the output would look complete",
            self.generator, self.path
        )
    }
}

impl std::error::Error for DuplicatePath {}
