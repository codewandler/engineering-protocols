//! Reading a delta back: the wire surface, and the four claims it has to make good on.
//!
//! # Why there is a pair at all
//!
//! Invariant 2. A delta is written to a file, quoted in a review, and read by a later process on a
//! later checkout — so it is a *document*, and a document becomes a domain type by validating rather
//! than by deserializing. [`EssDelta`] therefore does not implement `Deserialize`; this module holds
//! the type that does, and the [`TryFrom`] that turns one into the other.
//!
//! # What deserialization alone cannot check
//!
//! Every scalar in the document validates while it parses — a [`QualifiedName`](ess_domain::name::QualifiedName)
//! refuses a malformed name, a [`SpecDigest`](aep_domain::evidence::SpecDigest) refuses upper case,
//! a [`DeltaFormat`] refuses a spelling that is not a format. None of that can
//! see the four claims an `EssDelta` makes, because each is about the document as a whole:
//!
//! | claim | code when it fails |
//! |---|---|
//! | the format is one this build implements | `unsupported_format_version` |
//! | both sides name one system | `conflicting_declaration` |
//! | every change is named by the id its own content derives, and carries the relation its own content derives | `conflicting_declaration` |
//! | the changes are in canonical order, with no id twice | `conflicting_declaration`, `duplicate_declaration` |
//!
//! The third is what makes a derived id worth writing down. The document carries `id` and `relation`
//! so that a reviewer can quote one and a consumer in another language does not have to reimplement
//! the classification — and carrying a derived fact is only safe if reading it back checks it. A
//! delta whose ids were edited by hand, or produced by an older build that classified a grant
//! differently, is refused rather than believed.
//!
//! # Errors accumulate
//!
//! Invariant 3. A delta with four doctored ids reports four errors, not the first one.

use aep_domain::error::{ValidationCode, ValidationError, ValidationErrors};

use crate::change::{ChangeId, SemanticChange, SemanticRelation};
use crate::delta::{DeltaFormat, EssDelta, EssRevisionRef};

/// A delta document as it is written, before anything has checked what it claims.
///
/// The only way into an [`EssDelta`] other than running a comparison.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawEssDelta {
    /// The shape the document says it is written in.
    pub format: DeltaFormat,
    /// The revision it says it compared from.
    pub before: EssRevisionRef,
    /// The revision it says it compared to.
    pub after: EssRevisionRef,
    /// The changes it says it found.
    pub changes: Vec<RawSemanticChange>,
}

/// One change as the document writes it: the change, plus the two facts derived from it.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawSemanticChange {
    /// What the document calls this change.
    pub id: String,
    /// How the document says it relates the two revisions.
    pub relation: SemanticRelation,
    /// Which construct moved, and what happened to it.
    pub change: SemanticChange,
}

impl TryFrom<RawEssDelta> for EssDelta {
    type Error = ValidationErrors;

    fn try_from(raw: RawEssDelta) -> Result<Self, Self::Error> {
        let mut errors = ValidationErrors::new();

        if !raw.format.is_supported() {
            errors.push(
                ValidationError::new(
                    ValidationCode::UnsupportedFormatVersion,
                    "delta.format",
                    format!(
                        "this build reads {}, and the document is written in `{}`",
                        supported(),
                        raw.format
                    ),
                )
                .with_hint("a later format may mean something different by the same words"),
            );
        }

        if raw.before.system != raw.after.system {
            errors.push(ValidationError::new(
                ValidationCode::ConflictingDeclaration,
                "delta.after.system",
                format!(
                    "the two sides name different systems, `{}` and `{}`: a delta answers what \
                     moved between revisions of one system",
                    raw.before.system, raw.after.system
                ),
            ));
        }

        // Compared as `ChangeId`s rather than as text, because the canonical order is the
        // *category* order and the category order is not the alphabet: `system` precedes `actor`,
        // and a string comparison here would call a correctly ordered delta out of order.
        let mut previous: Option<ChangeId> = None;
        for (index, written) in raw.changes.iter().enumerate() {
            let id = written.change.id();
            let derived = id.to_string();
            if written.id != derived {
                errors.push(
                    ValidationError::new(
                        ValidationCode::ConflictingDeclaration,
                        format!("delta.changes[{index}].id"),
                        format!(
                            "the document calls this change `{}` and its own content derives \
                             `{derived}`",
                            written.id
                        ),
                    )
                    .with_hint(
                        "a change id is derived, not declared; regenerate the delta rather than \
                         editing it",
                    ),
                );
            }

            let relation = written.change.relation();
            if written.relation != relation {
                errors.push(ValidationError::new(
                    ValidationCode::ConflictingDeclaration,
                    format!("delta.changes[{index}].relation"),
                    format!(
                        "the document calls `{derived}` `{}` and its own content derives `{relation}`",
                        written.relation
                    ),
                ));
            }

            if let Some(before) = &previous {
                if &id == before {
                    errors.push(ValidationError::new(
                        ValidationCode::DuplicateDeclaration,
                        format!("delta.changes[{index}].id"),
                        format!("`{derived}` appears twice, and one change is one change"),
                    ));
                } else if &id < before {
                    errors.push(
                        ValidationError::new(
                            ValidationCode::ConflictingDeclaration,
                            format!("delta.changes[{index}]"),
                            format!("`{derived}` is written after `{before}`, which is not the order the format defines"),
                        )
                        .with_hint(
                            "changes are ordered by category, then subject, then subtype, then member",
                        ),
                    );
                }
            }
            previous = Some(id);
        }

        let changes: Vec<SemanticChange> = raw
            .changes
            .into_iter()
            .map(|written| written.change)
            .collect();

        errors.into_result(Self::assembled(raw.format, raw.before, raw.after, changes))
    }
}

/// The formats this build reads, for a message that says what to do about a refusal.
fn supported() -> String {
    crate::delta::SUPPORTED_DELTA_FORMATS
        .iter()
        .map(|major| format!("`{}{major}`", DeltaFormat::PREFIX))
        .collect::<Vec<_>>()
        .join(", ")
}
