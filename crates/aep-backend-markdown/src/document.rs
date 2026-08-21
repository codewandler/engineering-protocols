//! One planning file: a frontmatter block, and the markdown nobody interprets.
//!
//! ```markdown
//! ---
//! format: aep.planning-md/1
//! id: story:passkey-login
//! kind: story
//! status: draft
//! title: Passkey login
//! relations:
//!   - derived_from: epic:passwordless
//! revision: 1
//! ---
//! # Passkey login
//!
//! Anything at all. The tooling never reads past the closing fence.
//! ```
//!
//! # The body is bytes
//!
//! [`PlanningDocument::render`] writes the body back exactly as it was read — no reflow, no
//! trailing-newline policy, no heading rewrite. A store that reformats prose is a store whose
//! every status move produces a diff nobody can review, and the review is the reason the plan
//! lives in the repository in the first place.
//!
//! # The fence split is hand-rolled
//!
//! Deliberately, and it is four lines: find `---` on the first line, find the next line that is
//! `---`, hand the middle to `serde_yaml` and keep the rest. The crates that do this
//! (`gray_matter`, `matter`, …) bring a second YAML implementation, a second markdown opinion and
//! a dependency the workspace would have to justify — see `AGENTS.md` § Dependencies, which asks
//! for the refusal to be recorded where the refusal happens. This is that record.

use std::collections::BTreeSet;
use std::fmt;

use aep_domain::artifact::{
    ArtifactKind, ArtifactLifecycle, ArtifactRef, ArtifactRelation, ArtifactStatus,
    LifecycleRegistry, RelationKind,
};
use aep_domain::error::ValidationErrors;

use crate::frontmatter::{PlanningFrontmatter, RawPlanningFrontmatter};

/// The line that opens and closes a frontmatter block.
const FENCE: &str = "---";

/// A planning document: validated frontmatter, plus the body as written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanningDocument {
    /// What the tooling reads.
    pub frontmatter: PlanningFrontmatter,
    /// What it does not: everything after the closing fence, byte for byte.
    pub body: String,
}

impl PlanningDocument {
    /// Builds a document from validated frontmatter and a body.
    pub fn new(frontmatter: PlanningFrontmatter, body: impl Into<String>) -> Self {
        Self {
            frontmatter,
            body: body.into(),
        }
    }

    /// Reads one document.
    ///
    /// `origin` appears in error messages only; pass the file path when there is one.
    pub fn parse(text: &str, origin: Option<&str>) -> Result<Self, PlanningDocumentError> {
        let (block, body) =
            split_fences(text).ok_or_else(|| PlanningDocumentError::NoFrontmatter {
                origin: origin.map(ToOwned::to_owned),
            })?;

        let raw: RawPlanningFrontmatter =
            serde_yaml::from_str(block).map_err(|source| PlanningDocumentError::Syntax {
                origin: origin.map(ToOwned::to_owned),
                source,
            })?;
        let frontmatter = PlanningFrontmatter::try_from(raw).map_err(|errors| {
            PlanningDocumentError::Invalid {
                origin: origin.map(ToOwned::to_owned),
                errors,
            }
        })?;

        Ok(Self {
            frontmatter,
            body: body.to_owned(),
        })
    }

    /// Writes the document back out.
    ///
    /// Deterministic: the same document renders to the same bytes every time, and
    /// `parse(&render(&d)) == d`. Both are asserted rather than asserted-to-be-obvious, because a
    /// store whose round trip is lossy corrupts a file on the first status move and nobody notices
    /// until the second.
    pub fn render(&self) -> String {
        let block = serde_yaml::to_string(&self.frontmatter)
            .unwrap_or_else(|error| panic!("validated frontmatter serialises: {error}"));
        format!("{FENCE}\n{block}{FENCE}\n{}", self.body)
    }

    /// Moves the artifact to `to`, or says what it could have moved to instead.
    ///
    /// The lifecycle comes from the document tree, through
    /// [`LifecycleRegistry::for_kind`] — so a kind that declares none inherits its parent's, and a
    /// kind with no lifecycle anywhere in its lineage gets
    /// [`ArtifactLifecycle::permissive`], which permits every move. Permissive is the honest
    /// default: refusing a transition because nobody wrote a ladder for `runbook` would make the
    /// store unusable for the kinds it has no opinion about.
    ///
    /// On success the revision is bumped, because the file on disk is about to change.
    pub fn move_status(
        &mut self,
        to: ArtifactStatus,
        lifecycles: &LifecycleRegistry,
    ) -> Result<(), MoveRefusal> {
        let permissive = ArtifactLifecycle::permissive();
        let lifecycle = lifecycles
            .for_kind(&self.frontmatter.kind)
            .unwrap_or(&permissive);
        let from = self.frontmatter.status;

        if !lifecycle.permits_transition(from, to) {
            return Err(MoveRefusal {
                kind: self.frontmatter.kind.clone(),
                from,
                to,
                legal: lifecycle
                    .transitions
                    .get(&from)
                    .cloned()
                    .unwrap_or_default(),
            });
        }

        self.frontmatter.status = to;
        self.bump();
        Ok(())
    }

    /// Adds an outgoing edge, and says whether anything changed.
    ///
    /// `false` means the document already declared exactly this edge: adding it again would
    /// produce a revision nobody can explain and a diff with nothing in it, so the revision is
    /// left alone. The graph itself is not checked here — whether the target exists and whether
    /// the edge closes a cycle are questions about the *store*, and
    /// [`StoreReport::graph`](crate::store::StoreReport::graph) is what answers them.
    pub fn add_relation(&mut self, kind: RelationKind, target: ArtifactRef) -> bool {
        let relation = ArtifactRelation::new(kind, target);
        if self.frontmatter.declares(&relation) {
            return false;
        }
        self.frontmatter.relations.push(relation);
        self.bump();
        true
    }

    /// Records that this document has been written again.
    fn bump(&mut self) {
        self.frontmatter.revision = self.frontmatter.revision.saturating_add(1);
    }
}

/// Splits `---` fences off the front of a document.
///
/// Returns the frontmatter block and the body, or `None` when the text does not open with a fence
/// or never closes one. Line endings are tolerated on both sides; the body is whatever follows the
/// newline after the closing fence, unaltered.
fn split_fences(text: &str) -> Option<(&str, &str)> {
    let rest = text
        .strip_prefix("---\n")
        .or_else(|| text.strip_prefix("---\r\n"))?;

    let mut offset = 0_usize;
    for line in rest.split_inclusive('\n') {
        if line.trim_end_matches(['\r', '\n']) == FENCE {
            return Some((&rest[..offset], &rest[offset + line.len()..]));
        }
        offset += line.len();
    }
    None
}

/// Why a planning document could not be read.
///
/// Three variants rather than one string, on the split `aep-schema::parse` already draws and for
/// the same reason: a missing fence is a defect a person fixes in one keystroke, a YAML error is a
/// typo on a line the message names, and a validation failure is a document that parses and means
/// something this build refuses. They call for three different reactions.
#[derive(Debug, thiserror::Error)]
pub enum PlanningDocumentError {
    /// The text does not open with a `---` fence, or never closes one.
    #[error(
        "planning document{} has no frontmatter: it must open with a `---` line and close the \
         block with another",
        context(origin.as_deref())
    )]
    NoFrontmatter {
        /// Where the text came from, when known.
        origin: Option<String>,
    },

    /// The frontmatter block is not well-formed YAML, or does not match the document's shape.
    #[error("planning document{}: {source}", context(origin.as_deref()))]
    Syntax {
        /// Where the text came from, when known.
        origin: Option<String>,
        /// The underlying parse error, which carries the line and column.
        source: serde_yaml::Error,
    },

    /// The frontmatter parses but is not valid.
    #[error("planning document{} is not valid: {errors}", context(origin.as_deref()))]
    Invalid {
        /// Where the text came from, when known.
        origin: Option<String>,
        /// Every problem found, not the first.
        errors: ValidationErrors,
    },
}

impl PlanningDocumentError {
    /// The validation errors, when this is a semantic failure.
    pub fn validation_errors(&self) -> Option<&ValidationErrors> {
        match self {
            Self::Invalid { errors, .. } => Some(errors),
            Self::NoFrontmatter { .. } | Self::Syntax { .. } => None,
        }
    }

    /// Where the document came from, when known.
    pub fn origin(&self) -> Option<&str> {
        match self {
            Self::NoFrontmatter { origin }
            | Self::Syntax { origin, .. }
            | Self::Invalid { origin, .. } => origin.as_deref(),
        }
    }
}

/// Renders an optional origin as ` (path)`.
fn context(origin: Option<&str>) -> String {
    match origin {
        Some(origin) => format!(" ({origin})"),
        None => String::new(),
    }
}

/// A status move the kind's lifecycle does not permit, with every move it does.
///
/// The legal set is carried rather than left for the caller to look up, because the refusal a
/// person reads has to answer the question the refusal creates. "`active` is not a legal move" is
/// a dead end; "a story may move to: proposed, archived" is the next thing to type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MoveRefusal {
    /// The kind whose lifecycle refused.
    pub kind: ArtifactKind,
    /// Where the artifact is.
    pub from: ArtifactStatus,
    /// Where the move would have taken it.
    pub to: ArtifactStatus,
    /// Every status it may move to from here. Empty when the status is terminal.
    pub legal: BTreeSet<ArtifactStatus>,
}

impl MoveRefusal {
    /// The legal targets, comma-separated, in status order.
    pub fn legal_targets(&self) -> String {
        self.legal
            .iter()
            .map(|status| status.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

impl std::error::Error for MoveRefusal {}

impl fmt::Display for MoveRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.legal.is_empty() {
            write!(
                f,
                "a {} in {} is at the end of its lifecycle and may not move",
                self.kind, self.from
            )
        } else {
            write!(f, "a {} may move to: {}", self.kind, self.legal_targets())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aep_domain::artifact::ArtifactId;

    const STORY: &str = "---\nformat: aep.planning-md/1\nid: story:passkey-login\nkind: story\n\
                         status: draft\ntitle: Passkey login\nrevision: 1\n---\n# Passkey login\n\n\
                         Body text.\n";

    fn document(text: &str) -> PlanningDocument {
        PlanningDocument::parse(text, Some("fixture.md")).expect("the fixture is a valid document")
    }

    fn story_lifecycle() -> LifecycleRegistry {
        let mut registry = LifecycleRegistry::new();
        registry.insert(
            ArtifactKind::Story,
            serde_yaml::from_str(
                "kind: story\ninitial: draft\ntransitions:\n  draft: [proposed, archived]\n  \
                 proposed: [draft, active, rejected]\n  active: [implemented, archived]\n  \
                 implemented: [archived]\n  rejected: [archived]\n  archived: []\n",
            )
            .expect("the fixture lifecycle parses"),
        );
        registry
    }

    #[test]
    fn the_body_survives_a_round_trip_byte_for_byte() {
        let parsed = document(STORY);
        assert_eq!(parsed.body, "# Passkey login\n\nBody text.\n");
        assert_eq!(parsed.render(), STORY, "rendering restores the input");
    }

    #[test]
    fn rendering_twice_produces_the_same_bytes() {
        // Determinism, invariant 9. A second rendering that differs would make every `git diff`
        // over the plan noise, which is the thing keeping the plan in the repository buys.
        let parsed = document(STORY);
        let once = parsed.render();
        let twice = PlanningDocument::parse(&once, None)
            .expect("what render writes, parse reads")
            .render();
        assert_eq!(once, twice);
    }

    #[test]
    fn an_unknown_key_is_still_there_after_a_status_move() {
        let text = STORY.replace("revision: 1\n", "revision: 1\nsprint: 42\n");
        let mut parsed = document(&text);
        parsed
            .move_status(ArtifactStatus::Proposed, &story_lifecycle())
            .expect("draft to proposed is a legal story move");
        let rendered = parsed.render();
        assert!(rendered.contains("sprint: 42"), "{rendered}");
        assert!(rendered.contains("status: proposed"), "{rendered}");
        assert!(rendered.contains("revision: 2"), "{rendered}");
    }

    #[test]
    fn a_document_with_no_fence_is_refused_by_variant() {
        let error = PlanningDocument::parse("# Just markdown\n", Some("loose.md"))
            .expect_err("a file with no frontmatter is not a planning document");
        assert!(
            matches!(error, PlanningDocumentError::NoFrontmatter { .. }),
            "{error}"
        );
        assert!(error.to_string().contains("loose.md"), "{error}");
    }

    #[test]
    fn a_fence_that_never_closes_is_refused_rather_than_read_to_the_end() {
        // Otherwise the whole file becomes the frontmatter and the failure is a YAML error about
        // prose, which sends the reader to the wrong line.
        let error = PlanningDocument::parse("---\nid: story:x\nkind: story\nstatus: draft\n", None)
            .expect_err("an unterminated block is not a block");
        assert!(
            matches!(error, PlanningDocumentError::NoFrontmatter { .. }),
            "{error}"
        );
    }

    #[test]
    fn an_invalid_document_reports_validation_errors_not_a_syntax_error() {
        let error = PlanningDocument::parse(
            "---\nformat: aep.planning-md/9\nid: story:x\nkind: story\nstatus: draft\n---\n",
            None,
        )
        .expect_err("a version this build cannot read is refused");
        let errors = error
            .validation_errors()
            .expect("a document that parses and is refused fails semantically");
        assert_eq!(errors.len(), 1, "{errors}");
    }

    #[test]
    fn an_illegal_move_names_every_legal_target() {
        let mut parsed = document(STORY);
        let refusal = parsed
            .move_status(ArtifactStatus::Implemented, &story_lifecycle())
            .expect_err("a draft story cannot jump to implemented");

        assert_eq!(refusal.from, ArtifactStatus::Draft);
        assert_eq!(refusal.to, ArtifactStatus::Implemented);
        assert_eq!(
            refusal.legal,
            [ArtifactStatus::Proposed, ArtifactStatus::Archived]
                .into_iter()
                .collect::<BTreeSet<_>>()
        );
        assert_eq!(
            refusal.to_string(),
            "a story may move to: proposed, archived"
        );
        assert_eq!(
            parsed.frontmatter.status,
            ArtifactStatus::Draft,
            "a refused move changes nothing"
        );
        assert_eq!(
            parsed.frontmatter.revision, 1,
            "and does not bump the revision"
        );
    }

    #[test]
    fn a_terminal_status_says_so_rather_than_listing_nothing() {
        let text = STORY.replace("status: draft", "status: archived");
        let mut parsed = document(&text);
        let refusal = parsed
            .move_status(ArtifactStatus::Draft, &story_lifecycle())
            .expect_err("archived is the end of the story ladder");
        assert!(refusal.legal.is_empty());
        assert_eq!(
            refusal.to_string(),
            "a story in archived is at the end of its lifecycle and may not move"
        );
    }

    #[test]
    fn a_kind_with_no_lifecycle_anywhere_may_move_freely() {
        // Permissive is the fallback, and it has to be reached through an empty registry rather
        // than through the story ladder — otherwise this test passes on a lookup that never runs.
        let text = STORY
            .replace("kind: story", "kind: runbook")
            .replace("id: story:passkey-login", "id: runbook:passkey-rollout");
        let mut parsed = document(&text);
        assert!(
            LifecycleRegistry::new()
                .for_kind(&ArtifactKind::Runbook)
                .is_none(),
            "the fixture registry has to hold no runbook lifecycle for this to test the fallback"
        );
        parsed
            .move_status(ArtifactStatus::Implemented, &LifecycleRegistry::new())
            .expect("a kind nobody wrote a ladder for is not blocked by one");
        assert_eq!(parsed.frontmatter.revision, 2);
    }

    #[test]
    fn a_relation_that_is_already_declared_does_not_bump_the_revision() {
        let mut parsed = document(STORY);
        let target = ArtifactRef::unpinned(ArtifactId::new("epic:passwordless").expect("an id"));

        assert!(
            parsed.add_relation(RelationKind::DerivedFrom, target.clone()),
            "the first edge is new"
        );
        assert_eq!(parsed.frontmatter.revision, 2);
        assert!(
            !parsed.add_relation(RelationKind::DerivedFrom, target),
            "the second is the same edge"
        );
        assert_eq!(
            parsed.frontmatter.revision, 2,
            "a write that changes nothing is not a revision"
        );
        assert_eq!(parsed.frontmatter.relations.len(), 1);
    }

    #[test]
    fn a_crlf_document_is_read_and_its_body_kept() {
        let text = "---\r\nid: story:x\r\nkind: story\r\nstatus: draft\r\n---\r\n# Title\r\n";
        let parsed = PlanningDocument::parse(text, None).expect("CRLF is a line ending too");
        assert_eq!(parsed.body, "# Title\r\n");
    }
}
