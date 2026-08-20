//! Where a generated artifact came from.
//!
//! Four facts, because each of them moves independently and each one alone is insufficient: the same
//! specification version compiled by two builds can differ, and the same compiler run through two
//! generator versions certainly does. Design §10.

use std::fmt;

use ess_compiler::EssIr;

/// What produced an artifact.
///
/// `Deserialize` as well as `Serialize`, and the two version fields are owned rather than
/// `&'static str`, because provenance is read back and not only written. A conformance suite is
/// committed and re-read by a later run, and a document that has been on disk cannot hold a
/// `&'static str` that came from someone else's `env!`. Costing two allocations per artifact to let
/// the same type serve both directions is a better trade than a second provenance type that drifts
/// from this one — which is what happened to the schema mapping this crate publishes.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Provenance {
    /// The system the artifact describes.
    pub system: String,
    /// The version of that system's specification.
    pub specification_version: String,
    /// A digest of the resolved model.
    ///
    /// Of the IR rather than of the source files, deliberately: two source trees that differ only in
    /// comments and file layout mean the same system, and a digest that changes when a comment does
    /// makes every reader ignore it.
    pub source_digest: String,
    /// The build that resolved it.
    pub compiler_version: String,
    /// The build that wrote the artifact.
    pub generator_version: String,
}

impl Provenance {
    /// This crate's version, as both the compiler and generator version.
    ///
    /// One number while the two ship together. When they stop shipping together this becomes two
    /// numbers, and the field names already say which is which.
    pub const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    /// Derives provenance from the model an artifact is generated from.
    pub fn of(ir: &EssIr) -> Self {
        Self {
            system: ir.system.to_string(),
            specification_version: ir.version.to_string(),
            source_digest: digest(ir),
            compiler_version: Self::VERSION.to_owned(),
            generator_version: Self::VERSION.to_owned(),
        }
    }

    /// The command that regenerates every projection this crate publishes.
    ///
    /// The default for [`Provenance::lines`] and its comment renderings. A stamped artifact from a
    /// *different* generator names its own command through the `*_for` forms below — `ess-synth`
    /// writes `protocol ess synthesize` — because a header that tells the reader to run the wrong
    /// verb is worse than no header at all.
    pub const REGENERATE: &'static str = "protocol ess generate";

    /// The provenance as comment lines, each already prefixed.
    ///
    /// Takes the prefix because every projection has a different one — `#` for YAML, `//` for Rust —
    /// and a generator that hand-assembles these lines gets one of them wrong.
    ///
    /// **Not for HTML or Markdown.** A per-line prefix cannot *close* an HTML comment, so
    /// `commented("<!--")` produces four unterminated openers and a renderer swallows the page.
    /// Use [`Provenance::html_comment`], which is the whole block. This method's own documentation
    /// used to recommend the broken form, which is how the docs generator found it.
    pub fn commented(&self, prefix: &str) -> String {
        self.commented_for(prefix, Self::REGENERATE)
    }

    /// [`Provenance::commented`], naming the command that actually regenerates the artifact.
    pub fn commented_for(&self, prefix: &str, regenerate: &str) -> String {
        use std::fmt::Write as _;

        let mut out = String::new();
        for line in self.lines_for(regenerate) {
            let _ = writeln!(out, "{prefix} {line}");
        }
        out
    }

    /// The provenance as one HTML comment block, for Markdown and HTML.
    ///
    /// One `<!--` and one `-->` around the whole thing, because a comment that is opened per line
    /// and never closed makes the rest of the document disappear — silently, in a renderer, which is
    /// the worst place to find out.
    pub fn html_comment(&self) -> String {
        self.html_comment_for(Self::REGENERATE)
    }

    /// [`Provenance::html_comment`], naming the command that actually regenerates the artifact.
    pub fn html_comment_for(&self, regenerate: &str) -> String {
        use std::fmt::Write as _;

        let mut out = String::from("<!--\n");
        for line in self.lines_for(regenerate) {
            let _ = writeln!(out, "  {line}");
        }
        out.push_str("-->\n");
        out
    }

    /// The provenance as plain lines.
    pub fn lines(&self) -> Vec<String> {
        self.lines_for(Self::REGENERATE)
    }

    /// [`Provenance::lines`], naming the command that actually regenerates the artifact.
    pub fn lines_for(&self, regenerate: &str) -> Vec<String> {
        vec![
            format!(
                "generated from {} {}",
                self.system, self.specification_version
            ),
            format!("model digest {}", self.source_digest),
            format!(
                "compiler {} · generator {}",
                self.compiler_version, self.generator_version
            ),
            format!("do not edit: regenerate with `{regenerate}`"),
        ]
    }
}

impl fmt::Display for Provenance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {} ({})",
            self.system, self.specification_version, self.source_digest
        )
    }
}

/// A digest of the resolved model.
///
/// Over the IR's canonical JSON, so it is stable under anything that does not change what the system
/// means. Truncated to 16 hex characters: this is for telling two models apart in a comment header,
/// not for resisting an adversary, and a 64-character line nobody reads is worse than a short one
/// someone checks.
fn digest(ir: &EssIr) -> String {
    use sha2::{Digest, Sha256};

    use std::fmt::Write as _;

    let json = serde_json::to_vec(ir).unwrap_or_default();
    let hash = Sha256::digest(&json);
    let mut out = String::with_capacity(16);
    for byte in hash.iter().take(8) {
        let _ = write!(out, "{byte:02x}");
    }
    out
}
