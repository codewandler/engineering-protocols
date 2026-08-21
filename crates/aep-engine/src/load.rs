//! Reading a document tree.
//!
//! A document tree is the conventional layout — `protocols/`, `principles/`, `workflows/`,
//! `profiles/`, `artifacts/lifecycles/`, `drivers/` — but the convention is only about *where to
//! look*: what a file is called has no bearing on what it declares.
//!
//! Loading reports **every** bad file with its path rather than stopping at the first, because
//! fixing a document set one error per run is how a validation step becomes something people avoid
//! running.

use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use aep_domain::error::ValidationErrors;
use aep_schema::parse::{self, DocumentKind};

use crate::registry::Registry;

/// Which directory holds which kind of document.
const TREE: &[(&str, DocumentKind)] = &[
    ("protocols", DocumentKind::Protocol),
    ("principles", DocumentKind::Principle),
    ("workflows", DocumentKind::Workflow),
    ("profiles", DocumentKind::Profile),
    ("artifacts/lifecycles", DocumentKind::Lifecycle),
    // Last, and the order is load-bearing rather than aesthetic: a step map is cross-validated
    // against the workflow it pins, and the workflows are filled in by the row above this one.
    // `Registry::validate` is what runs that check, after the whole tree has been read, so a map
    // read before its workflow is still checked against it — but the reading order is kept honest
    // here so nobody has to know that to see why this row is last.
    ("drivers", DocumentKind::StepMap),
];

/// File extensions treated as documents.
const EXTENSIONS: &[&str] = &["yaml", "yml", "json"];

/// One file that could not be loaded.
#[derive(Debug)]
pub struct LoadFailure {
    /// The file, or `None` for a failure of the document set as a whole.
    pub path: Option<PathBuf>,
    /// What went wrong.
    pub detail: String,
}

impl fmt::Display for LoadFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.path {
            Some(path) => write!(f, "{}: {}", path.display(), self.detail),
            None => f.write_str(&self.detail),
        }
    }
}

/// What a load produced: whatever was readable, plus every failure.
#[derive(Debug)]
pub struct LoadOutcome {
    /// The documents that loaded.
    pub registry: Registry,
    /// How many files were read.
    pub files_read: usize,
    /// What failed.
    pub failures: Vec<LoadFailure>,
}

impl LoadOutcome {
    /// `true` when everything loaded and the document set is consistent.
    pub fn is_clean(&self) -> bool {
        self.failures.is_empty()
    }

    /// The registry, or every failure.
    pub fn into_result(self) -> Result<Registry, LoadErrors> {
        if self.failures.is_empty() {
            Ok(self.registry)
        } else {
            Err(LoadErrors(self.failures))
        }
    }
}

/// Every failure from one load.
#[derive(Debug)]
pub struct LoadErrors(Vec<LoadFailure>);

impl LoadErrors {
    /// Builds an error set from failures collected elsewhere.
    pub fn from_failures(failures: Vec<LoadFailure>) -> Self {
        Self(failures)
    }

    /// The failures.
    pub fn as_slice(&self) -> &[LoadFailure] {
        &self.0
    }

    /// How many failures there are.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// `true` when there are none, which a constructed value never is.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Display for LoadErrors {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{} document problem(s):", self.0.len())?;
        for failure in &self.0 {
            writeln!(f, "  - {failure}")?;
        }
        Ok(())
    }
}

impl std::error::Error for LoadErrors {}

/// Loads a document tree rooted at `root`, reporting everything readable and everything broken.
///
/// Missing directories are not an error: a project with principles of its own but no workflows is
/// perfectly ordinary.
pub fn load_tree_report(root: &Path) -> LoadOutcome {
    let mut registry = Registry::new();
    let mut failures = Vec::new();
    let mut files_read = 0_usize;

    for (directory, kind) in TREE {
        let path = root.join(directory);
        if !path.is_dir() {
            continue;
        }
        let mut files = Vec::new();
        if let Err(error) = collect_documents(&path, &mut files) {
            failures.push(LoadFailure {
                path: Some(path.clone()),
                detail: format!("cannot be read: {error}"),
            });
            continue;
        }
        files.sort();
        for file in files {
            files_read += 1;
            if let Err(failure) = load_file(&mut registry, &file, *kind) {
                failures.push(failure);
            }
        }
    }

    let consistency = registry.validate();
    failures.extend(validation_failures(&consistency));

    LoadOutcome {
        registry,
        files_read,
        failures,
    }
}

/// Loads a document tree, or fails with every problem found.
pub fn load_tree(root: &Path) -> Result<Registry, LoadErrors> {
    load_tree_report(root).into_result()
}

/// Loads one file into `registry`.
fn load_file(registry: &mut Registry, path: &Path, kind: DocumentKind) -> Result<(), LoadFailure> {
    let text = fs::read_to_string(path).map_err(|error| LoadFailure {
        path: Some(path.to_path_buf()),
        detail: format!("cannot be read: {error}"),
    })?;
    let origin = path.display().to_string();

    let outcome = match kind {
        DocumentKind::Protocol => parse::protocol(&text, Some(&origin))
            .map_err(|error| error.to_string())
            .and_then(|document| {
                registry
                    .insert_protocol(document)
                    .map_err(|error| error.to_string())
            }),
        DocumentKind::Principle => parse::principle(&text, Some(&origin))
            .map_err(|error| error.to_string())
            .and_then(|document| {
                registry
                    .insert_principle(document)
                    .map_err(|error| error.to_string())
            }),
        DocumentKind::Workflow => parse::workflow(&text, Some(&origin))
            .map_err(|error| error.to_string())
            .and_then(|document| {
                registry
                    .insert_workflow(document)
                    .map_err(|error| error.to_string())
            }),
        DocumentKind::Profile => parse::profile(&text, Some(&origin))
            .map_err(|error| error.to_string())
            .and_then(|document| {
                registry
                    .insert_profile(document)
                    .map_err(|error| error.to_string())
            }),
        DocumentKind::Lifecycle => parse::lifecycle(&text, Some(&origin))
            .map_err(|error| error.to_string())
            .and_then(|document| {
                registry
                    .insert_lifecycle(document)
                    .map_err(|error| error.to_string())
            }),
        DocumentKind::StepMap => parse::step_map(&text, Some(&origin))
            .map_err(|error| error.to_string())
            .and_then(|document| {
                registry
                    .insert_step_map(document)
                    .map_err(|error| error.to_string())
            }),
        DocumentKind::Task
        | DocumentKind::ArtifactManifest
        | DocumentKind::Evidence
        | DocumentKind::Project => {
            Err(format!("{kind} documents do not belong in a protocol tree"))
        }
    };

    outcome.map_err(|detail| LoadFailure {
        path: Some(path.to_path_buf()),
        detail,
    })
}

/// Turns cross-document validation errors into load failures.
fn validation_failures(errors: &ValidationErrors) -> Vec<LoadFailure> {
    errors
        .as_slice()
        .iter()
        .map(|error| LoadFailure {
            path: None,
            detail: error.to_string(),
        })
        .collect()
}

/// Collects document files under `directory`, recursively.
fn collect_documents(directory: &Path, into: &mut Vec<PathBuf>) -> io::Result<()> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with('.') {
            continue;
        }
        if path.is_dir() {
            collect_documents(&path, into)?;
            continue;
        }
        let is_document = path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| EXTENSIONS.contains(&extension));
        if is_document {
            into.push(path);
        }
    }
    Ok(())
}
