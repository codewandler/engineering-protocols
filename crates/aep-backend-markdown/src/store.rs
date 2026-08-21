//! A directory of planning documents.
//!
//! ```text
//! <root>/<kind>/<name>.md
//! ```
//!
//! `story:passkey-login` lives at `<root>/story/passkey-login.md`, and that is a rule the store
//! **checks** rather than a convention it assumes. A file whose directory does not name its
//! declared kind, or whose stem is not its declared name, is reported as a problem with its path
//! on it — not quietly re-derived. Re-deriving would make the id in the file and the id in the
//! path two sources of truth, and the loser would be whichever one the next tool happened to read.
//!
//! # Everything broken is reported, with its path
//!
//! [`MarkdownStore::load`] never stops at the first bad file. Fixing a plan one error per run is
//! how a validation step becomes something people stop running — the same argument
//! `aep_engine::load` makes for the document tree, and the same shape: [`StoreFailure`] is that
//! module's `LoadFailure` with a store's vocabulary.
//!
//! # There is no delete
//!
//! Not on [`MarkdownStore`], not on [`StoreReport`], not anywhere in this crate. Retiring a plan
//! item is `status: archived`, which leaves the file, its body and its history in place. See the
//! crate documentation.

use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use aep_domain::artifact::{ArtifactGraph, ArtifactId, ArtifactKind};
use aep_domain::error::ValidationErrors;

use crate::document::PlanningDocument;

/// The extension a planning document has.
const EXTENSION: &str = "md";

/// A directory of planning documents.
///
/// Holding one costs nothing and touches no disk: every operation reads or writes at the moment it
/// is called, so two processes editing the same store see each other's writes without a cache to
/// invalidate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkdownStore {
    root: PathBuf,
}

impl MarkdownStore {
    /// Opens the store rooted at `root`.
    ///
    /// The directory need not exist: a project that has planned nothing yet has no
    /// `.engineering/planning/`, and that is an empty plan rather than an error.
    pub fn open(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// The directory the documents live in.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Where the document for `id` belongs, as a repository-relative path.
    ///
    /// The id's **namespace** names the directory, which is what makes the layout derivable from
    /// the id alone: `story:passkey-login` is `story/passkey-login.md`, and `adr:passkeys` is
    /// `adr/passkeys.md` because `adr` is an accepted spelling of its kind. A document whose
    /// namespace names no kind, or names a different one from its `kind:`, is reported by
    /// [`Self::load`] rather than filed somewhere a reader would not look for it.
    pub fn relative_path_for(&self, id: &ArtifactId) -> String {
        format!("{}/{}.{EXTENSION}", id.namespace(), id.name())
    }

    /// Where the document for `id` belongs on disk.
    pub fn path_for(&self, id: &ArtifactId) -> PathBuf {
        let mut path = self.root.clone();
        for segment in self.relative_path_for(id).split('/') {
            path.push(segment);
        }
        path
    }

    /// Reads every document in the store, reporting everything readable and everything broken.
    pub fn load(&self) -> StoreReport {
        let mut report = StoreReport::default();

        if !self.root.exists() {
            return report;
        }

        let mut files = Vec::new();
        if let Err(error) = collect_documents(&self.root, &mut files) {
            report.failures.push(StoreFailure {
                path: self.root.clone(),
                detail: format!("cannot be read: {error}"),
            });
            return report;
        }
        // Sorted, so the report's failure order is the tree's order and two runs over one store
        // print the same lines. `read_dir` promises no order at all.
        files.sort();

        for path in files {
            report.files_read += 1;
            let relative = relative_path(&self.root, &path);

            let text = match fs::read_to_string(&path) {
                Ok(text) => text,
                Err(error) => {
                    report.failures.push(StoreFailure {
                        path,
                        detail: format!("cannot be read: {error}"),
                    });
                    continue;
                }
            };

            let document = match PlanningDocument::parse(&text, Some(&relative)) {
                Ok(document) => document,
                Err(error) => {
                    report.failures.push(StoreFailure {
                        path,
                        detail: error.to_string(),
                    });
                    continue;
                }
            };

            if let Some(detail) = self.path_disagreement(&relative, &document) {
                report.failures.push(StoreFailure {
                    path: path.clone(),
                    detail,
                });
            }

            let id = document.frontmatter.id.clone();
            if let Some(existing) = report.documents.get(&id) {
                report.failures.push(StoreFailure {
                    path,
                    detail: format!(
                        "declares `{id}`, which {} already declares; an id addresses one document",
                        existing.relative_path
                    ),
                });
                continue;
            }
            report.documents.insert(
                id,
                StoredDocument {
                    relative_path: relative,
                    document,
                },
            );
        }

        report
    }

    /// Why `relative` is not where `document` belongs, when it is not.
    ///
    /// Two checks, and they are separate on purpose. The directory is checked against the
    /// **kind**, not against the id's namespace, so a tree filed under `adr/` is accepted for
    /// `architecture-decision-record` — an alias is a spelling of one kind, and refusing it would
    /// make the store reject a layout its own vocabulary permits. The stem is checked against the
    /// id's **name**, because that is the half a person types.
    fn path_disagreement(&self, relative: &str, document: &PlanningDocument) -> Option<String> {
        let frontmatter = &document.frontmatter;
        let stem = relative.strip_suffix(".md").unwrap_or(relative);
        let Some((directory, name)) = stem.split_once('/') else {
            return Some(format!(
                "sits at the top of the store, where no directory names its kind; `{}` belongs at \
                 `{}`",
                frontmatter.id,
                self.relative_path_for(&frontmatter.id)
            ));
        };

        match ArtifactKind::parse(directory) {
            Ok(kind) if kind == frontmatter.kind => {}
            Ok(kind) => {
                return Some(format!(
                    "sits in `{directory}/`, which names the kind `{kind}`, but declares `kind: \
                     {}`",
                    frontmatter.kind
                ))
            }
            Err(error) => {
                return Some(format!(
                    "sits in `{directory}/`, which names no kind: {error}"
                ))
            }
        }

        if name != frontmatter.id.name() {
            return Some(format!(
                "is named `{name}.md` but declares `id: {}`; the file name is the id's name part, \
                 so `{}` belongs at `{}`",
                frontmatter.id,
                frontmatter.id,
                self.relative_path_for(&frontmatter.id)
            ));
        }

        None
    }

    /// Writes a document that does not exist yet, and returns where it went.
    ///
    /// Refuses an existing file rather than overwriting it. Creating over a document is how a plan
    /// item's body disappears, and there is no undo in a tool that has not committed anything.
    pub fn create(&self, document: &PlanningDocument) -> Result<PathBuf, StoreError> {
        let path = self.path_for(&document.frontmatter.id);
        if path.exists() {
            return Err(StoreError::Exists { path });
        }
        self.write(&path, &document.render())?;
        Ok(path)
    }

    /// Rewrites an existing document in place, and returns where it went.
    ///
    /// Takes the path the document was **loaded from** rather than deriving one from the id, so a
    /// document filed under an accepted alias is rewritten where it lives instead of being
    /// silently duplicated at the canonical path. [`StoredDocument::relative_path`] is what to
    /// pass.
    pub fn update(
        &self,
        relative_path: &str,
        document: &PlanningDocument,
    ) -> Result<PathBuf, StoreError> {
        let mut path = self.root.clone();
        for segment in relative_path.split('/') {
            path.push(segment);
        }
        if !path.is_file() {
            return Err(StoreError::Missing { path });
        }
        self.write(&path, &document.render())?;
        Ok(path)
    }

    /// Writes `contents` to `path` through a temporary file in the same directory.
    ///
    /// Same directory, because `rename` is only atomic within a filesystem and `/tmp` is routinely
    /// a different one — a cross-device rename fails, and the fallback everyone writes instead is
    /// a copy that can be interrupted half way. The temporary name starts with a dot, which is
    /// exactly what [`Self::load`]'s walk skips, so an interrupted write leaves nothing the store
    /// will later try to parse.
    fn write(&self, path: &Path, contents: &str) -> Result<(), StoreError> {
        let parent = path.parent().unwrap_or(&self.root);
        fs::create_dir_all(parent).map_err(|source| StoreError::Io {
            path: parent.to_path_buf(),
            source,
        })?;

        let name = path.file_name().map_or_else(
            || "document.md".to_owned(),
            |name| name.to_string_lossy().into_owned(),
        );
        let temporary = parent.join(format!(".{name}.tmp"));

        fs::write(&temporary, contents).map_err(|source| StoreError::Io {
            path: temporary.clone(),
            source,
        })?;
        fs::rename(&temporary, path).map_err(|source| StoreError::Io {
            path: path.to_path_buf(),
            source,
        })
    }
}

/// One document the store holds, and where it was read from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredDocument {
    /// Its path relative to the store root, with `/` separators.
    ///
    /// This is what becomes the artifact's location, and what [`MarkdownStore::update`] writes
    /// back to.
    pub relative_path: String,
    /// The document itself.
    pub document: PlanningDocument,
}

/// One file the store could not use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreFailure {
    /// The file it is about. Always a real path: a failure nobody can navigate to is a rumour.
    pub path: PathBuf,
    /// What is wrong with it.
    pub detail: String,
}

impl fmt::Display for StoreFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.path.display(), self.detail)
    }
}

/// What one read of the store produced: whatever was readable, plus every failure.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StoreReport {
    /// The documents that loaded, by id.
    pub documents: BTreeMap<ArtifactId, StoredDocument>,
    /// How many files were read.
    pub files_read: usize,
    /// What failed, in path order.
    pub failures: Vec<StoreFailure>,
}

impl StoreReport {
    /// `true` when every file in the store parsed and sits where it says it does.
    pub fn is_clean(&self) -> bool {
        self.failures.is_empty()
    }

    /// The artifact graph the store describes.
    ///
    /// Through [`ArtifactGraph::build`], which is where duplicate ids, edges pointing at nothing,
    /// self-supersession and cycles are refused. None of that is re-implemented here: the store's
    /// job is to say what the files contain, and the graph's job is to say whether that is a
    /// graph.
    ///
    /// A store with [failures](Self::failures) still produces a graph of what did load. That is
    /// deliberate for reading — a listing of nine artifacts is more useful than a refusal because
    /// the tenth file has a typo — and it is why every verb that *writes* checks
    /// [`Self::is_clean`] first.
    pub fn graph(&self) -> Result<ArtifactGraph, ValidationErrors> {
        ArtifactGraph::build(self.documents.values().map(|stored| {
            stored
                .document
                .frontmatter
                .to_artifact(&stored.relative_path)
        }))
    }
}

/// Why a write could not happen.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    /// The document already exists, and creating over it would lose its body.
    #[error("{} already exists; a planning document is created once and then moved", path.display())]
    Exists {
        /// The file that is in the way.
        path: PathBuf,
    },

    /// The document does not exist, so there is nothing to rewrite.
    #[error("{} does not exist; nothing to update", path.display())]
    Missing {
        /// The file that is not there.
        path: PathBuf,
    },

    /// The filesystem refused.
    #[error("{}: {source}", path.display())]
    Io {
        /// What was being written.
        path: PathBuf,
        /// Why it could not be.
        source: io::Error,
    },
}

/// Collects `.md` files under `directory`, recursively.
///
/// Dot-entries are skipped: a `.git` directory is not a plan, and the temporary file a write goes
/// through is not a document yet.
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
        if path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension == EXTENSION)
        {
            into.push(path);
        }
    }
    Ok(())
}

/// `path` relative to `root`, with `/` separators.
fn relative_path(root: &Path, path: &Path) -> String {
    let relative = path.strip_prefix(root).unwrap_or(path);
    relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An empty scratch store, named after the test that owns it.
    fn scratch(name: &str) -> MarkdownStore {
        let root = std::env::temp_dir().join("aep-markdown-store").join(name);
        std::fs::remove_dir_all(&root).ok();
        std::fs::create_dir_all(&root).expect("the scratch tree is writable");
        MarkdownStore::open(root)
    }

    fn write(store: &MarkdownStore, relative: &str, contents: &str) {
        let path = store.root().join(relative);
        std::fs::create_dir_all(path.parent().expect("a parent")).expect("writable");
        std::fs::write(path, contents).expect("writable");
    }

    fn story(id: &str, extra: &str) -> String {
        format!("---\nid: {id}\nkind: story\nstatus: draft\n{extra}---\n# {id}\n")
    }

    #[test]
    fn a_document_is_filed_under_its_namespace() {
        let store = MarkdownStore::open("/plans");
        let id = ArtifactId::new("story:passkey-login").expect("an id");
        assert_eq!(store.relative_path_for(&id), "story/passkey-login.md");
        assert_eq!(
            store.path_for(&id),
            PathBuf::from("/plans/story/passkey-login.md")
        );
    }

    #[test]
    fn an_absent_store_is_an_empty_plan_rather_than_a_failure() {
        let store = MarkdownStore::open("/nowhere-that-exists/planning");
        let report = store.load();
        assert!(report.is_clean(), "{:?}", report.failures);
        assert_eq!(report.files_read, 0);
        assert!(report.documents.is_empty());
    }

    #[test]
    fn every_broken_file_is_reported_with_its_path() {
        // Invariant 3's shape for a directory: an exact count, because "some failures" would pass
        // with a loader that stopped at the first one.
        let store = scratch("broken");
        write(&store, "story/good.md", &story("story:good", ""));
        write(&store, "story/no-fence.md", "# Just markdown\n");
        write(&store, "story/bad-yaml.md", "---\nid: [\n---\n");
        write(
            &store,
            "story/old.md",
            "---\nformat: aep.planning-md/9\nid: story:old\nkind: story\nstatus: draft\n---\n",
        );

        let report = store.load();
        assert_eq!(report.files_read, 4);
        assert_eq!(report.failures.len(), 3, "{:?}", report.failures);
        assert_eq!(
            report.documents.len(),
            1,
            "the readable document still loads"
        );
        for failure in &report.failures {
            assert!(
                failure.path.starts_with(store.root()),
                "{failure} names no navigable path"
            );
        }
    }

    #[test]
    fn a_file_in_the_wrong_directory_is_reported_rather_than_re_derived() {
        let store = scratch("wrong-directory");
        write(&store, "epic/misfiled.md", &story("story:misfiled", ""));

        let report = store.load();
        assert_eq!(report.failures.len(), 1, "{:?}", report.failures);
        let detail = &report.failures[0].detail;
        assert!(detail.contains("epic/"), "{detail}");
        assert!(detail.contains("kind: story"), "{detail}");
        assert_eq!(
            report.documents.len(),
            1,
            "the document is still readable; only its filing is wrong"
        );
    }

    #[test]
    fn a_file_whose_stem_is_not_its_name_is_reported_with_where_it_belongs() {
        let store = scratch("wrong-stem");
        write(
            &store,
            "story/renamed.md",
            &story("story:passkey-login", ""),
        );

        let report = store.load();
        assert_eq!(report.failures.len(), 1, "{:?}", report.failures);
        assert!(
            report.failures[0].detail.contains("story/passkey-login.md"),
            "{}",
            report.failures[0]
        );
    }

    #[test]
    fn a_directory_named_by_an_alias_of_the_kind_is_accepted() {
        // `adr/` is a spelling of `architecture-decision-record`, and refusing a layout the
        // vocabulary permits would be this store inventing a rule the protocol does not have.
        let store = scratch("alias-directory");
        write(
            &store,
            "adr/passkeys.md",
            "---\nid: adr:passkeys\nkind: architecture-decision-record\nstatus: proposed\n---\n",
        );

        let report = store.load();
        assert!(report.is_clean(), "{:?}", report.failures);
        assert_eq!(report.documents.len(), 1);
    }

    #[test]
    fn two_files_claiming_one_id_are_both_named() {
        let store = scratch("duplicate-id");
        write(&store, "story/a.md", &story("story:a", ""));
        write(&store, "story/b.md", &story("story:a", ""));

        let report = store.load();
        // Two problems, not one: the second file both duplicates an id and sits under a stem that
        // is not its name. A reader fixing this has to be told both.
        assert_eq!(report.failures.len(), 2, "{:?}", report.failures);
        let details = report
            .failures
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(details.contains("story/a.md"), "{details}");
        assert!(details.contains("already declares"), "{details}");
    }

    #[test]
    fn creating_over_an_existing_document_is_refused() {
        let store = scratch("create-twice");
        let document = PlanningDocument::parse(&story("story:once", ""), None).expect("valid");

        let path = store.create(&document).expect("the first create writes");
        assert!(path.is_file());

        let error = store
            .create(&document)
            .expect_err("the second would lose the body of the first");
        assert!(matches!(error, StoreError::Exists { .. }), "{error}");
        assert!(
            error.to_string().contains("already exists"),
            "{error} does not say what happened"
        );
    }

    #[test]
    fn an_update_writes_where_the_document_was_read_from() {
        let store = scratch("update");
        let document = PlanningDocument::parse(&story("story:moving", ""), None).expect("valid");
        store.create(&document).expect("written");

        let mut report = store.load();
        let stored = report
            .documents
            .get_mut(&ArtifactId::new("story:moving").expect("an id"))
            .expect("the document loaded");
        stored.document.frontmatter.status = aep_domain::artifact::ArtifactStatus::Proposed;
        let relative = stored.relative_path.clone();
        let document = stored.document.clone();

        store.update(&relative, &document).expect("rewritten");

        let reread = store.load();
        assert!(reread.is_clean(), "{:?}", reread.failures);
        let moved = reread
            .documents
            .get(&ArtifactId::new("story:moving").expect("an id"))
            .expect("still there");
        assert_eq!(
            moved.document.frontmatter.status,
            aep_domain::artifact::ArtifactStatus::Proposed
        );
        assert_eq!(reread.files_read, 1, "no temporary file is left behind");
    }

    #[test]
    fn updating_a_document_that_is_not_there_is_refused() {
        let store = scratch("update-missing");
        let document = PlanningDocument::parse(&story("story:ghost", ""), None).expect("valid");
        let error = store
            .update("story/ghost.md", &document)
            .expect_err("an update that creates is a create");
        assert!(matches!(error, StoreError::Missing { .. }), "{error}");
    }

    #[test]
    fn the_graph_refuses_an_edge_pointing_at_nothing() {
        let store = scratch("dangling-edge");
        write(
            &store,
            "story/orphan.md",
            &story(
                "story:orphan",
                "relations:\n  - derived_from: epic:absent\n",
            ),
        );

        let report = store.load();
        assert!(report.is_clean(), "the file itself is fine");
        let errors = report
            .graph()
            .expect_err("an edge to an artifact the store does not hold is not a graph");
        assert_eq!(errors.len(), 1, "{errors}");
        assert!(errors.to_string().contains("epic:absent"), "{errors}");
    }

    #[test]
    fn a_clean_store_becomes_a_graph_whose_artifacts_are_located_at_their_files() {
        let store = scratch("graph");
        write(
            &store,
            "epic/passwordless.md",
            "---\nid: epic:passwordless\nkind: epic\nstatus: active\n---\n",
        );
        write(
            &store,
            "story/passkey-login.md",
            &story(
                "story:passkey-login",
                "relations:\n  - derived_from: epic:passwordless\n",
            ),
        );

        let report = store.load();
        assert!(report.is_clean(), "{:?}", report.failures);
        let graph = report.graph().expect("the edges resolve");
        assert_eq!(graph.len(), 2);
        let listed = graph
            .get(&ArtifactId::new("story:passkey-login").expect("an id"))
            .expect("in the graph");
        assert_eq!(
            listed.location.local_path(),
            Some("story/passkey-login.md"),
            "the location is the file it was read from"
        );
    }
}
