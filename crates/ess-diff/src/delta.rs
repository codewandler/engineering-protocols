//! The delta document: what a comparison produces, and what a later run reads back.

use std::fmt;

use aep_domain::error::ParseError;
use aep_domain::evidence::SpecDigest;
use ess_compiler::ir::EssIr;
use ess_domain::name::{QualifiedName, Version};

use crate::change::{ChangeId, SemanticChange, SemanticRelation};

/// Delta format major versions this build implements.
pub const SUPPORTED_DELTA_FORMATS: &[u32] = &[1];

/// The version of the *document shape* a delta is written in — `ess-diff/1`.
///
/// The first thing a reader reads and the first thing it can refuse, on exactly the reasoning
/// [`SuiteFormat`](ess_conformance::scenario::SuiteFormat) is built on: a later format may mean
/// something different by the same words, and a reader that guesses reports a change nobody made.
///
/// It is **not** design §5's precondition 3. That precondition asks the diff to understand "both IR
/// format versions", and the IR has no format version to read — `EssIr::version` is the
/// *specification's* version, not the IR's. A refusal with nothing to read is a refusal that cannot
/// fire, so it is not declared. This versions the one document this crate does own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
#[serde(into = "String")]
pub struct DeltaFormat(Version);

impl DeltaFormat {
    /// The first, and so far only, delta format.
    pub const CURRENT: Self = Self(Version::V1);

    /// How a delta format is written.
    pub const PREFIX: &'static str = "ess-diff/";

    /// The numeric part.
    pub fn major(self) -> u32 {
        self.0.get()
    }

    /// `true` when this build implements it.
    pub fn is_supported(self) -> bool {
        SUPPORTED_DELTA_FORMATS.contains(&self.major())
    }

    /// Parses `ess-diff/1`.
    ///
    /// The digits are read by [`Version::parse`] rather than by a rule written again here, so `01`,
    /// `+1`, `0` and `4294967296` are refused because that function refuses them.
    pub fn parse(value: &str) -> Result<Self, ParseError> {
        let digits = value.strip_prefix(Self::PREFIX).ok_or_else(|| {
            ParseError::reference(
                "delta format",
                value,
                format!("delta formats are written `{}1`", Self::PREFIX),
            )
        })?;
        Version::parse(&format!("v{digits}"))
            .map(Self)
            .map_err(|_| {
                ParseError::reference(
                    "delta format",
                    value,
                    "expected a whole number after the prefix, without a leading zero",
                )
            })
    }
}

impl fmt::Display for DeltaFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", Self::PREFIX, self.0.get())
    }
}

impl From<DeltaFormat> for String {
    fn from(value: DeltaFormat) -> Self {
        value.to_string()
    }
}

impl std::str::FromStr for DeltaFormat {
    type Err = ParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

impl<'de> serde::Deserialize<'de> for DeltaFormat {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        Self::parse(&raw).map_err(serde::de::Error::custom)
    }
}

/// Which resolution of which system one side of a comparison is.
///
/// Three facts, and only one of them is identity. `billing/v3` is a label two different resolutions
/// can share — [`Version`] is major-only on purpose — so the **digest** is what tells two revisions
/// apart, and the version is here for a person reading the report. That is the same sentence
/// [`EssConformanceResult`](aep_domain::evidence::EssConformanceResult) already carries about its own
/// digest, and it is why nothing here orders revisions by version.
///
/// The digest is derived from [`ess_gen::Provenance`], never computed a second way: one model has
/// one digest, or a delta attests to a specification nobody can find again.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct EssRevisionRef {
    /// The system this revision is of.
    pub system: QualifiedName,
    /// The version its specification declares — a label, not the identity.
    pub specification_version: Version,
    /// A digest of the resolved model. The identity.
    pub spec_digest: SpecDigest,
}

impl EssRevisionRef {
    /// Reads a revision reference off a compiled specification.
    ///
    /// # Panics
    ///
    /// If `ess-gen`'s digest stops being a digest. It is the full 64 lower-case hexadecimal characters of a SHA-256
    /// by construction, so the panic is how the two crates disagreeing becomes visible immediately
    /// rather than as a delta carrying an unparsable digest that no evidence record can be matched
    /// against. `SuiteProvenance::of` panics on the same line for the same reason.
    pub fn of(ir: &EssIr) -> Self {
        let projection = ess_gen::Provenance::of(ir);
        let spec_digest =
            SpecDigest::new(projection.source_digest.as_str()).unwrap_or_else(|error| {
                panic!(
                    "`ess-gen` writes a digest `aep-domain` accepts: {error}; the two have \
                     drifted, and a delta carrying an unparsable digest names no revision"
                )
            });
        Self {
            system: ir.system.clone(),
            specification_version: ir.version,
            spec_digest,
        }
    }
}

impl fmt::Display for EssRevisionRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {} ({})",
            self.system, self.specification_version, self.spec_digest
        )
    }
}

/// What moved between two revisions of one system.
///
/// # Why this does not implement `Deserialize`
///
/// Invariant 2. An `EssDelta` carries four claims that no field of it can carry on its own: the
/// document is written in a format this build implements, both sides name one system, every change
/// is named by the id its own content derives, and the changes are in canonical order without
/// duplicates. Possession of an `EssDelta` is the evidence that those four were checked, so the only
/// ways to obtain one are [`diff`](crate::diff()) and
/// `EssDelta::try_from(`[`RawEssDelta`](crate::RawEssDelta)`)`.
///
/// The components *inside* it — [`QualifiedName`], [`Version`], [`SpecDigest`], and every `*Ref` a
/// change carries — do deserialize, because each of them validates while it parses and there is no
/// way to hold one that did not. That is the same door
/// [`SuiteProvenance`](ess_conformance::scenario::SuiteProvenance) goes through, and the line
/// between them is where a cross-field guarantee starts.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct EssDelta {
    /// The shape this document is written in.
    pub format: DeltaFormat,
    /// The revision compared *from*.
    pub before: EssRevisionRef,
    /// The revision compared *to*.
    pub after: EssRevisionRef,
    /// Every change, in canonical order.
    #[serde(serialize_with = "serialize_changes")]
    changes: Vec<SemanticChange>,
}

impl EssDelta {
    /// Assembles a delta, putting its changes in canonical order.
    ///
    /// Crate-private, and sorting here rather than trusting a caller is what makes
    /// [`EssDelta::changes`] a list nobody has to re-sort: design §60's order is a **format
    /// contract**, so it belongs to the one constructor rather than to each producer.
    pub(crate) fn new(
        before: EssRevisionRef,
        after: EssRevisionRef,
        mut changes: Vec<SemanticChange>,
    ) -> Self {
        changes.sort_by_key(SemanticChange::id);
        Self {
            format: DeltaFormat::CURRENT,
            before,
            after,
            changes,
        }
    }

    /// Assembles a delta from changes already known to be in canonical order.
    ///
    /// Crate-private, and separate from [`EssDelta::new`] on purpose: `new` *puts* changes in order,
    /// which is right for a comparison that produced them, and wrong for a document that claims to
    /// be in order already. Sorting there would repair the defect
    /// `EssDelta::try_from(`[`RawEssDelta`](crate::RawEssDelta)`)` exists to report, which is how a
    /// check comes to pass whether or not the rule holds.
    pub(crate) fn assembled(
        format: DeltaFormat,
        before: EssRevisionRef,
        after: EssRevisionRef,
        changes: Vec<SemanticChange>,
    ) -> Self {
        Self {
            format,
            before,
            after,
            changes,
        }
    }

    /// Every change, in canonical order.
    pub fn changes(&self) -> &[SemanticChange] {
        &self.changes
    }

    /// How many changes there are.
    pub fn len(&self) -> usize {
        self.changes.len()
    }

    /// `true` when the two revisions mean the same thing.
    ///
    /// Design §67's acceptance criterion: identical semantic IR produces an empty delta. That is
    /// what makes reflowing a comment, renaming a file or moving a declaration between files cost
    /// nothing — and it is a claim `examples/revision-pair/` is built to test rather than to state.
    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }

    /// The change with this id, if the delta holds one.
    pub fn change(&self, id: &ChangeId) -> Option<&SemanticChange> {
        self.changes.iter().find(|change| &change.id() == id)
    }

    /// How many changes carry this relation.
    pub fn count(&self, relation: SemanticRelation) -> usize {
        self.changes
            .iter()
            .filter(|change| change.relation() == relation)
            .count()
    }

    /// The delta as canonical JSON, with a trailing newline.
    ///
    /// Canonical means the same three things it means for the IR and for a conformance suite: the
    /// order comes from the format contract rather than from iteration, the indentation is
    /// `serde_json`'s two spaces, and the last byte is a newline, because a file without one shows
    /// up as modified in the next diff.
    ///
    /// # Panics
    ///
    /// It does not. `serde_json` has exactly one error of its own — a map key that is not a string —
    /// and this document holds no map at all: every collection in it is a sequence. The
    /// `unwrap_or_else` names the impossible case rather than hiding it.
    pub fn to_canonical_json(&self) -> String {
        let mut json = serde_json::to_string_pretty(self)
            .unwrap_or_else(|error| panic!("the delta serialises: {error}"));
        json.push('\n');
        json
    }
}

#[cfg(test)]
mod tests {
    use ess_conformance::scenario::{ActorRef, DeclaredTypeRef};

    use super::*;
    use crate::change::{ActorChange, TypeChange};

    /// A revision reference for a fixture. The digest is a plausible one, not a real model's.
    fn revision(digest: &str) -> EssRevisionRef {
        EssRevisionRef {
            system: QualifiedName::new("catalog").expect("a valid name"),
            specification_version: Version::V1,
            spec_digest: SpecDigest::new(digest).expect("a full SHA-256 in lower-case hex"),
        }
    }

    #[test]
    fn a_delta_puts_its_changes_in_canonical_order_however_they_arrive() {
        // The constructor is what makes design §60's order a contract rather than a property of
        // whichever walk happened to produce the changes. Today the walk already emits them in that
        // order, so nothing else in this crate would notice if the sort went away — which is exactly
        // why the fixture here hands them over backwards.
        let delta = EssDelta::new(
            revision("0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"),
            revision("fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210"),
            vec![
                SemanticChange::Actor {
                    subject: ActorRef::new(QualifiedName::new("catalog.a.Z").expect("a name")),
                    changed: ActorChange::Added,
                },
                SemanticChange::Type {
                    subject: DeclaredTypeRef::new(
                        QualifiedName::new("catalog.a.T").expect("a name"),
                    ),
                    changed: TypeChange::Added,
                },
            ],
        );

        let ids: Vec<String> = delta
            .changes()
            .iter()
            .map(|change| change.id().to_string())
            .collect();
        assert_eq!(
            ids,
            vec![
                "type/catalog.a.T/added".to_owned(),
                "actor/catalog.a.Z/added".to_owned()
            ],
            "a type change comes before an actor change, which is neither the order they arrived in \
             nor the order the alphabet would give"
        );
    }
}

/// One change as the document writes it: the derived id and relation, then the change itself.
///
/// A borrowed wrapper rather than three fields on [`SemanticChange`], because the id and the
/// relation are *derived* from the change and a stored copy of a derived fact is a second fact that
/// can disagree with the first. They are in the document all the same, and for a stated reason: an
/// id absent from the artifact is an id nobody can quote in a review, and a relation absent from it
/// is a classification every consumer in another language has to reimplement.
///
/// [`RawEssDelta`](crate::RawEssDelta) closes the loop by checking both against what the change
/// derives when the document is read back.
#[derive(serde::Serialize)]
struct WrittenChange<'a> {
    /// What this change is called.
    id: ChangeId,
    /// How it relates the two revisions.
    relation: SemanticRelation,
    /// Which construct moved, and what happened to it.
    change: &'a SemanticChange,
}

/// Writes each change with its derived id and relation.
fn serialize_changes<S: serde::Serializer>(
    changes: &[SemanticChange],
    serializer: S,
) -> Result<S::Ok, S::Error> {
    use serde::ser::SerializeSeq as _;

    let mut sequence = serializer.serialize_seq(Some(changes.len()))?;
    for change in changes {
        sequence.serialize_element(&WrittenChange {
            id: change.id(),
            relation: change.relation(),
            change,
        })?;
    }
    sequence.end()
}
