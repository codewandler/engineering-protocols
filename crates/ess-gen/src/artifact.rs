//! What a generator produces, and the one trait every generator implements.

use std::collections::BTreeMap;

use ess_compiler::EssIr;

use crate::provenance::{ModelSlice, Provenance, ProvenanceMint};

/// One generated file.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Artifact {
    /// Where it goes, relative to the output root. Always `/`-separated.
    pub path: String,
    /// Its contents.
    pub contents: String,
    /// The model slice it derives from.
    ///
    /// [`ModelSlice::WholeModel`] unless the generator narrowed it, and that default is the
    /// polarity of the whole wave: an artifact whose slice nobody thought about is owed
    /// regeneration whenever anything moves, never quietly current.
    pub slice: ModelSlice,
}

impl Artifact {
    /// Builds one that derives from the whole model.
    pub fn new(path: impl Into<String>, contents: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            contents: contents.into(),
            slice: ModelSlice::WholeModel,
        }
    }

    /// Builds one that derives from a narrower slice.
    ///
    /// The slice comes attached to the provenance that was stamped into `contents` —
    /// [`ProvenanceMint::of_seeds`] hands both out as one value — so the recorded slice and the
    /// stamped digest cannot be paired up wrong by a generator, and [`run`] still checks.
    pub fn sliced(path: impl Into<String>, contents: impl Into<String>, slice: ModelSlice) -> Self {
        Self {
            path: path.into(),
            contents: contents.into(),
            slice,
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
    ///
    /// The mint, not a `Provenance`: since wave 7 each artifact carries the digest of the model
    /// slice it derives from, and a single pre-computed provenance could only say "the whole
    /// model" about every file. A generator that has nothing narrower to say still stamps
    /// [`ProvenanceMint::whole`].
    fn generate(&self, ir: &EssIr, mint: &ProvenanceMint) -> Vec<Artifact>;
}

/// Runs one generator and returns its artifacts keyed by path.
///
/// Keyed, and therefore deduplicated: two artifacts claiming one path means the second silently
/// overwrites the first, and the output tree looks complete while missing a file.
/// # Panics
///
/// When an artifact's stamped contract digest disagrees with the digest its recorded slice
/// computes, or when an artifact carries no readable provenance at all. Both are defects in a
/// generator, not in any specification — the same class as two artifacts claiming one path, but
/// worse, because a wrong stamp *ships*: a committed artifact claiming derivation from a slice it
/// was not stamped for is exactly the false claim wave 7's drift check exists to refuse, and this
/// is the one place every artifact of every generator passes through before it can be written.
pub fn run(
    generator: &dyn Generator,
    ir: &EssIr,
) -> Result<BTreeMap<String, Artifact>, DuplicatePath> {
    let mint = ProvenanceMint::new(ir);
    let mut out: BTreeMap<String, Artifact> = BTreeMap::new();
    for artifact in generator.generate(ir, &mint) {
        let stamped = Provenance::read_digests(&artifact.contents).unwrap_or_else(|| {
            panic!(
                "the `{}` generator wrote `{}` without readable provenance; an artifact that \
                 cannot say what it derives from is an artifact nobody can audit",
                generator.name(),
                artifact.path
            )
        });
        let computed = mint.digest_of(&artifact.slice);
        assert_eq!(
            stamped.contract_digest,
            computed,
            "the `{}` generator stamped `{}` with a contract digest its recorded slice does not \
             compute; the stamp and the slice must come from one `ProvenanceMint` call",
            generator.name(),
            artifact.path
        );
        let path = format!("{}/{}", generator.directory(), artifact.path);
        if out.contains_key(&path) {
            return Err(DuplicatePath {
                generator: generator.name(),
                path,
            });
        }
        out.insert(
            path.clone(),
            Artifact::sliced(path, artifact.contents, artifact.slice),
        );
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
