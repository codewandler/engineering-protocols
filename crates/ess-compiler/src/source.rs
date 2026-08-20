//! Where a diagnostic points.
//!
//! `serde_yaml` gives a line and column for a *syntax* error and nothing for a semantic one, so a
//! semantic diagnostic would point at the top of the file — which is a diagnostic someone has to
//! search from, and design §29 asks for `--> domains/bindings.yaml:14:18`.
//!
//! What this module does instead is keep the source text beside the document path that every
//! validation error already carries, and locate the path in the text on demand. It is a heuristic,
//! and it says so: [`Span::located`] is `None` when the path cannot be found, and a diagnostic with
//! no line is still a diagnostic. A confidently wrong line number would be worse than none, because
//! the reader would edit there.

use std::collections::BTreeMap;

/// The text of every file a specification was read from.
///
/// Keyed by the same [`Source`](ess_domain::system::Source) label the validation errors carry, so a
/// diagnostic can find its own file without anything having to thread a handle through.
#[derive(Debug, Clone, Default)]
pub struct SourceMap {
    files: BTreeMap<String, String>,
}

impl SourceMap {
    /// An empty map. Diagnostics still render, without a line.
    pub fn new() -> Self {
        Self::default()
    }

    /// Records one file's text.
    pub fn insert(&mut self, source: impl Into<String>, text: impl Into<String>) {
        self.files.insert(source.into(), text.into());
    }

    /// One file's text.
    pub fn get(&self, source: &str) -> Option<&str> {
        self.files.get(source).map(String::as_str)
    }

    /// How many files.
    pub fn len(&self) -> usize {
        self.files.len()
    }

    /// `true` when nothing was recorded.
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }
}

/// Where in a file something is.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Span {
    /// The file, as the specification labelled it.
    pub source: String,
    /// The document path, such as `bindings[0].mapping.recipient`.
    pub path: String,
    /// The line and column, when the path could be found in the text.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub located: Option<Location>,
}

/// A line and column, both 1-based, as an editor counts them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct Location {
    /// The line.
    pub line: usize,
    /// The column.
    pub column: usize,
}

impl std::fmt::Display for Span {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.located {
            Some(Location { line, column }) => write!(f, "{}:{line}:{column}", self.source),
            None => write!(f, "{} ({})", self.source, self.path),
        }
    }
}
