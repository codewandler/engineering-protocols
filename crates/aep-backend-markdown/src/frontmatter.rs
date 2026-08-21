//! The YAML block at the top of a planning document.
//!
//! Parse, then validate, as everywhere else in this workspace: [`RawPlanningFrontmatter`]
//! deserializes and [`PlanningFrontmatter`] is what a caller may hold. Validation accumulates, so
//! a block that is wrong about two things reports two errors rather than the first one twice.
//!
//! # The key set, and why it is this short
//!
//! ```yaml
//! format: aep.planning-md/1
//! id: story:passkey-login
//! kind: story
//! status: draft
//! title: Passkey login
//! relations:
//!   - derived_from: epic:passwordless
//! revision: 1
//! ```
//!
//! `id`, `kind` and `status` are required; `title`, `summary`, `owner`, `tags` and `relations` are
//! optional; `format` and `revision` default. Everything else a document carries is **kept** — see
//! [`PlanningFrontmatter::extra`] — because a store that silently drops the field somebody's own
//! tooling writes is a store they will stop trusting after the first round trip.
//!
//! What is deliberately absent is a timestamp. See the crate documentation: git carries
//! authorship, and a second answer beside it is one that goes stale.

use std::collections::{BTreeMap, BTreeSet};

use aep_domain::artifact::{
    Artifact, ArtifactId, ArtifactKind, ArtifactLocation, ArtifactMetadata, ArtifactRelation,
    ArtifactStatus, ArtifactVersion, RelationKind,
};
use aep_domain::error::{ValidationCode, ValidationError, ValidationErrors};
use aep_domain::node::Node;

/// The frontmatter format version this build reads and writes.
///
/// Versioned from the first document rather than from the first breaking change: a file with no
/// version in it is a file whose reader has to guess, and the guess is only ever wrong once the
/// format has already moved.
pub const PLANNING_FORMAT: &str = "aep.planning-md/1";

/// Serde default for [`RawPlanningFrontmatter::format`].
fn default_format() -> String {
    PLANNING_FORMAT.to_owned()
}

/// Serde default for [`RawPlanningFrontmatter::revision`].
///
/// One, not zero: the first written revision of a document is its first revision, and a store
/// whose counter starts below the first write has an off-by-one in every comparison built on it.
fn default_revision() -> u64 {
    1
}

/// The frontmatter of a planning document, as parsed.
///
/// Deliberately permissive about *shape* and strict about nothing: every rule this format has is
/// checked by the [`TryFrom`] into [`PlanningFrontmatter`], so a document with two problems is
/// reported once with both.
#[derive(Debug, Clone, serde::Deserialize, schemars::JsonSchema)]
pub struct RawPlanningFrontmatter {
    /// The format version. Defaults to [`PLANNING_FORMAT`]; any other value is refused.
    #[serde(default = "default_format")]
    pub format: String,
    /// The artifact's identifier, such as `story:passkey-login`.
    pub id: ArtifactId,
    /// What kind of artifact it is. Aliases such as `adr` are accepted.
    pub kind: ArtifactKind,
    /// Where its lifecycle has got to.
    pub status: ArtifactStatus,
    /// Its title, for a listing.
    #[serde(default)]
    pub title: Option<String>,
    /// One-line summary.
    #[serde(default)]
    pub summary: Option<String>,
    /// Who owns it.
    #[serde(default)]
    pub owner: Option<String>,
    /// Free-form labels.
    #[serde(default)]
    pub tags: BTreeSet<String>,
    /// Its outgoing edges, each a single-entry mapping such as `{derived_from: epic:passwordless}`.
    #[serde(default)]
    pub relations: Vec<ArtifactRelation>,
    /// Which revision of this document this is. Bumped by every mutating operation.
    #[serde(default = "default_revision")]
    pub revision: u64,
    /// Every key this format does not name.
    #[serde(default, flatten)]
    pub extra: BTreeMap<String, Node>,
}

/// The frontmatter of a planning document, validated.
///
/// No `Deserialize`, by invariant 2: the only way to obtain one is to validate a
/// [`RawPlanningFrontmatter`], so a value of this type is one whose format version is understood
/// and whose revision counts from a real write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanningFrontmatter {
    /// The artifact's identifier.
    pub id: ArtifactId,
    /// What kind of artifact it is.
    pub kind: ArtifactKind,
    /// Where its lifecycle has got to.
    pub status: ArtifactStatus,
    /// Its title.
    pub title: Option<String>,
    /// One-line summary.
    pub summary: Option<String>,
    /// Who owns it.
    pub owner: Option<String>,
    /// Free-form labels.
    pub tags: BTreeSet<String>,
    /// Its outgoing edges.
    pub relations: Vec<ArtifactRelation>,
    /// Which revision of this document this is.
    pub revision: u64,
    /// Every key this format does not name, kept so a round trip loses nothing.
    ///
    /// A planning file is a file people edit. Somebody's board tool will write `sprint: 42` into
    /// one, and the first time `protocol artifact move` rewrites the file that key has to still be
    /// there — otherwise the tool that wrote it and this one cannot both be used, and this one is
    /// the one that gets deleted.
    pub extra: BTreeMap<String, Node>,
}

impl PlanningFrontmatter {
    /// The format version this frontmatter is written in.
    ///
    /// A constant rather than a field: the only value a validated frontmatter can have is the one
    /// [`TryFrom`] accepted, so storing it would create a second place for it to be wrong.
    pub fn format(&self) -> &'static str {
        PLANNING_FORMAT
    }

    /// A minimal frontmatter, at revision 1.
    pub fn new(id: ArtifactId, kind: ArtifactKind, status: ArtifactStatus) -> Self {
        Self {
            id,
            kind,
            status,
            title: None,
            summary: None,
            owner: None,
            tags: BTreeSet::new(),
            relations: Vec::new(),
            revision: default_revision(),
            extra: BTreeMap::new(),
        }
    }

    /// Sets the title, builder-style.
    #[must_use]
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Adds a relation, builder-style.
    #[must_use]
    pub fn with_relation(mut self, relation: ArtifactRelation) -> Self {
        self.relations.push(relation);
        self
    }

    /// `true` when this frontmatter already declares `relation`.
    pub fn declares(&self, relation: &ArtifactRelation) -> bool {
        self.relations.contains(relation)
    }

    /// Every target of `kind`.
    pub fn targets(&self, kind: RelationKind) -> impl Iterator<Item = &ArtifactRelation> {
        self.relations
            .iter()
            .filter(move |relation| relation.kind == kind)
    }

    /// The domain artifact this document describes, located at `relative_path`.
    ///
    /// The mapping is the whole point of this crate: what the protocol reasons about is an
    /// [`Artifact`] in an [`ArtifactGraph`](aep_domain::artifact::ArtifactGraph), and the file
    /// format is how one is written down here.
    ///
    /// Two things are worth saying out loud about what comes out:
    ///
    /// * the location is a [`ArtifactLocation::RepositoryPath`] with no `repository`, because the
    ///   document is a file in *this* repository and naming one would claim it is somewhere else;
    /// * there is **no provenance**, so no `created_at`. Git holds authorship; see the crate
    ///   documentation for why a second copy of it is worse than none.
    pub fn to_artifact(&self, relative_path: &str) -> Artifact {
        let mut artifact = Artifact::new(
            self.id.clone(),
            self.kind.clone(),
            self.status,
            ArtifactLocation::RepositoryPath {
                repository: None,
                path: relative_path.to_owned(),
            },
        );
        // The document revision is the version this record describes: it moves with every write,
        // which is exactly what a version label is for here. It is not a digest and does not
        // pretend to be one — `model_digest` stays empty, because a plan item has no compiled
        // model to hash.
        artifact.version = Some(ArtifactVersion::new(self.revision.to_string()));
        artifact.relations.clone_from(&self.relations);
        artifact.metadata = ArtifactMetadata {
            title: self.title.clone(),
            summary: self.summary.clone(),
            owner: self.owner.clone(),
            tags: self.tags.clone(),
            extra: self.extra.clone(),
        };
        artifact
    }
}

/// Written by hand rather than derived, for two reasons that both come down to bytes.
///
/// `format` is a constant, and a struct field holding it would be a second place for it to be
/// wrong. And the **key order is the file's key order**: a derived implementation orders by field
/// declaration, which is the same thing, but a reader of this type would have to know that to
/// trust it. Here the order is written down where the rendering is, which is what makes two
/// renderings of one document byte-identical and a round trip a comparison rather than a hope.
impl serde::Serialize for PlanningFrontmatter {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;

        let length = 5
            + usize::from(self.title.is_some())
            + usize::from(self.summary.is_some())
            + usize::from(self.owner.is_some())
            + usize::from(!self.tags.is_empty())
            + usize::from(!self.relations.is_empty())
            + self.extra.len();

        let mut map = serializer.serialize_map(Some(length))?;
        map.serialize_entry("format", PLANNING_FORMAT)?;
        map.serialize_entry("id", &self.id)?;
        map.serialize_entry("kind", &self.kind)?;
        map.serialize_entry("status", &self.status)?;
        if let Some(title) = &self.title {
            map.serialize_entry("title", title)?;
        }
        if let Some(summary) = &self.summary {
            map.serialize_entry("summary", summary)?;
        }
        if let Some(owner) = &self.owner {
            map.serialize_entry("owner", owner)?;
        }
        if !self.tags.is_empty() {
            map.serialize_entry("tags", &self.tags)?;
        }
        if !self.relations.is_empty() {
            map.serialize_entry("relations", &self.relations)?;
        }
        map.serialize_entry("revision", &self.revision)?;
        // Last, and in `BTreeMap` order: an unrecognised key keeps its value and loses only its
        // position, which is the most a reader that does not know what it means can promise.
        for (key, value) in &self.extra {
            map.serialize_entry(key, value)?;
        }
        map.end()
    }
}

impl TryFrom<RawPlanningFrontmatter> for PlanningFrontmatter {
    type Error = ValidationErrors;

    fn try_from(raw: RawPlanningFrontmatter) -> Result<Self, Self::Error> {
        let mut errors = ValidationErrors::new();

        if raw.format != PLANNING_FORMAT {
            errors.push(
                ValidationError::new(
                    ValidationCode::UnsupportedFormatVersion,
                    "planning.format",
                    format!(
                        "this build reads planning documents written as `{PLANNING_FORMAT}`, not \
                         `{}`",
                        raw.format
                    ),
                )
                .with_hint(
                    "upgrade the tooling rather than reinterpreting the document: a reader that \
                     guesses at an unknown version writes back a file it has already lost part of",
                ),
            );
        }

        if raw.revision == 0 {
            errors.push(
                ValidationError::new(
                    ValidationCode::TypeMismatch,
                    "planning.revision",
                    "`revision: 0` names a state before the document was written",
                )
                .with_hint("revisions count from 1; omit the key to get it"),
            );
        }

        errors.into_result(Self {
            id: raw.id,
            kind: raw.kind,
            status: raw.status,
            title: raw.title,
            summary: raw.summary,
            owner: raw.owner,
            tags: raw.tags,
            relations: raw.relations,
            revision: raw.revision,
            extra: raw.extra,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(text: &str) -> RawPlanningFrontmatter {
        serde_yaml::from_str(text).expect("the fixture parses as YAML")
    }

    const MINIMAL: &str = "id: story:passkey-login\nkind: story\nstatus: draft\n";

    #[test]
    fn a_document_without_a_format_key_is_read_as_the_current_format() {
        // The default exists so the first hand-written file does not need a header nobody would
        // think to write. It has to be the *current* version, not "unknown".
        let frontmatter =
            PlanningFrontmatter::try_from(raw(MINIMAL)).expect("the minimal document is valid");
        assert_eq!(frontmatter.format(), PLANNING_FORMAT);
        assert_eq!(
            frontmatter.revision, 1,
            "the revision defaults to the first"
        );
    }

    #[test]
    fn a_format_version_this_build_does_not_read_is_refused_by_code() {
        let errors =
            PlanningFrontmatter::try_from(raw(&format!("format: aep.planning-md/2\n{MINIMAL}")))
                .expect_err("a version this build cannot read is not silently reinterpreted");
        assert_eq!(errors.len(), 1, "{errors}");
        assert!(
            errors.contains(ValidationCode::UnsupportedFormatVersion),
            "{errors}"
        );
    }

    #[test]
    fn a_document_wrong_about_two_things_reports_both() {
        // Invariant 3: validation accumulates. An exact count, because "is an error" would pass
        // with a validator that returned on the first problem.
        let errors = PlanningFrontmatter::try_from(raw(&format!(
            "format: aep.planning-md/2\nrevision: 0\n{MINIMAL}"
        )))
        .expect_err("both problems are problems");
        assert_eq!(errors.len(), 2, "{errors}");
        assert!(
            errors.contains(ValidationCode::UnsupportedFormatVersion),
            "{errors}"
        );
        assert!(errors.contains(ValidationCode::TypeMismatch), "{errors}");
    }

    #[test]
    fn revision_zero_is_refused_because_no_write_produced_it() {
        let errors = PlanningFrontmatter::try_from(raw(&format!("revision: 0\n{MINIMAL}")))
            .expect_err("a revision below the first write is not a revision");
        assert_eq!(errors.len(), 1, "{errors}");
        assert_eq!(errors.as_slice()[0].location, "planning.revision");
    }

    #[test]
    fn a_kind_alias_is_accepted_and_canonicalised() {
        let frontmatter =
            PlanningFrontmatter::try_from(raw("id: adr:passkeys\nkind: adr\nstatus: proposed\n"))
                .expect("`adr` is an accepted spelling of the kind");
        assert_eq!(frontmatter.kind, ArtifactKind::ArchitectureDecisionRecord);
    }

    #[test]
    fn an_unknown_key_survives_validation() {
        let frontmatter =
            PlanningFrontmatter::try_from(raw(&format!("{MINIMAL}sprint: 42\n"))).expect("valid");
        assert_eq!(
            frontmatter.extra.get("sprint"),
            Some(&Node::Number(42_i64.into())),
            "an unrecognised key is carried, not dropped"
        );
    }

    #[test]
    fn the_artifact_it_maps_to_is_located_at_its_file_and_carries_no_timestamp() {
        let frontmatter = PlanningFrontmatter::try_from(raw(&format!(
            "{MINIMAL}title: Passkey login\nrelations:\n  - derived_from: epic:passwordless\n"
        )))
        .expect("valid");
        let artifact = frontmatter.to_artifact("story/passkey-login.md");

        assert_eq!(
            artifact.location,
            ArtifactLocation::RepositoryPath {
                repository: None,
                path: "story/passkey-login.md".to_owned(),
            }
        );
        assert!(
            artifact.provenance.is_none(),
            "git carries authorship; a second copy of it goes stale"
        );
        assert_eq!(artifact.metadata.title.as_deref(), Some("Passkey login"));
        assert_eq!(artifact.relations.len(), 1);
        assert_eq!(
            artifact.version.as_ref().map(ArtifactVersion::as_str),
            Some("1")
        );
    }
}
