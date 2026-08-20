//! The typed change vocabulary: one variant per thing that can differ, and no untyped escape hatch.
//!
//! Design §10. A generic `path + before JSON + after JSON` change is easy to produce and impossible
//! to reason about: nothing downstream can ask "did an actor lose authority" without re-parsing a
//! path, and a new ESS construct appears in it silently, as one more path nobody decided about. A
//! closed vocabulary makes the opposite true — a construct added to the model does not compile until
//! someone has decided how it compares.
//!
//! # There is no `Modified` catch-all
//!
//! Design §11 sketches `ChangeKind { Added, Removed, Modified }` beside the typed detail. It is not
//! carried, because the detail already says which of the three a change is and a second field
//! saying the same thing is a second field that can disagree — the reasoning
//! [`ResolvedTypeRef::declared`](ess_compiler::ir::ResolvedTypeRef::declared) is derived by, one
//! level up. A consumer filtering for additions reads `kind` on the detail.
//!
//! # Every family is complete over the struct it compares
//!
//! The point of a closed vocabulary is lost if it is closed over the wrong set, so each family's
//! variants are checked against the `Resolved*` struct they describe, field by field:
//!
//! | family | fields covered |
//! |---|---|
//! | [`SystemChange`] | `EssIr::{version, summary}` — `system` is the refusal, and `naming` is a field no document can set |
//! | [`TypeChange`] | `ResolvedType::{body, naming}`, and `body` down to every arm of `ResolvedBody` |
//! | [`EventChange`] | `ResolvedEvent::{domain, fields, naming}` |
//! | [`ErrorChange`] | `ResolvedError::{domain, summary, fields}` — it carries no `Naming` |
//! | [`ActorChange`] | `ResolvedActor::{domain, may, naming}` |
//! | [`ComponentChange`] | `ResolvedComponent::{owns, accepts, publishes, naming}` |
//!
//! `name` is the map key in every case, so it is identity rather than a comparable field: a change
//! of name is an [`Added`](TypeChange::Added) and a [`Removed`](TypeChange::Removed), never a
//! rename. Design §6, and it is deliberate — see [`mod@crate::diff`].

use std::fmt;

use ess_conformance::scenario::{
    ActorRef, CommandRef, ComponentRef, DeclaredTypeRef, DomainRef, ErrorRef, EssSemanticRef,
    EventRef,
};
use ess_domain::name::{QualifiedName, Version};

/// Where a change sits in the canonical order.
///
/// Design §60 makes the category order a **format contract** rather than an accident of iteration,
/// and this enum is that contract: the declaration order below is the sort order, and it is design
/// §60's own list restricted to the six families this slice compares. Alphabetical order would put
/// `actor` before `system`, which no reader of the document asked for.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "kebab-case")]
pub enum ChangeCategory {
    /// The specification itself: its version, its naming, its summary.
    System,
    /// A declared type.
    Type,
    /// An event.
    Event,
    /// A declared error.
    Error,
    /// An actor.
    Actor,
    /// A component.
    Component,
}

impl ChangeCategory {
    /// How it is written, in an id and in the document.
    ///
    /// One spelling, produced once: the id segment and the serialised `category` field are the same
    /// string, so a reader who greps for a change id finds the change.
    pub const fn written(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Type => "type",
            Self::Event => "event",
            Self::Error => "error",
            Self::Actor => "actor",
            Self::Component => "component",
        }
    }
}

impl fmt::Display for ChangeCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.written())
    }
}

/// What one change is called, derived from the change's own content.
///
/// `type/catalog.pricing.Currency/variant-added/CHF`. Four parts, and they are design §60's four
/// ordering keys in design §60's order — semantic category, subject identity, change subtype, nested
/// member identity — so sorting a delta by id *is* putting it in canonical order, rather than being
/// a second rule that has to agree with the first.
///
/// # Why not a counter
///
/// Wave 4 answered this for [`ScenarioId`](ess_conformance::scenario::ScenarioId) and paid for the
/// answer: a monotonic counter renumbers every id after the one that was inserted, so anything
/// stored against those keys — a review comment, a suppression, an impact record — comes to name a
/// different thing without anything having been edited. An id derived from content moves only when
/// the thing it names moves.
///
/// # Why not a hash
///
/// A hashed id is stable too, and it is unreadable. `sha256(…)[..16]` in a review comment tells the
/// next reader nothing, and this repository's own digest documentation makes the same point from the
/// other side — a 64-character line nobody reads is worse than a short one someone checks. The four
/// parts are already the content; rendering them *is* the derivation.
///
/// # It is not parsed for meaning
///
/// Invariant 13's rule applied one level up. The id is for quoting and for ordering; the typed
/// change beside it is what code reads. Nothing in this crate parses an id back into its parts, and
/// [`RawEssDelta`](crate::RawEssDelta) compares a declared id against a derived one as text rather
/// than taking it apart.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ChangeId {
    /// The semantic category — the first ordering key.
    category: ChangeCategory,
    /// Which construct, by its stable name.
    subject: String,
    /// Which kind of change.
    ///
    /// `&'static str` on purpose: the subtype word is a constant of the change variant and is never
    /// read out of a document, which is what keeps an id derivable rather than declarable. There is
    /// no way to build a `ChangeId` carrying a subtype no change produces.
    subtype: &'static str,
    /// The member inside the subject that moved, where there is one: a variant, a field, a granted
    /// command.
    member: Option<String>,
}

impl ChangeId {
    /// The category this change belongs to.
    pub fn category(&self) -> ChangeCategory {
        self.category
    }

    /// The construct the change is about.
    pub fn subject(&self) -> &str {
        &self.subject
    }

    /// Which kind of change it is, as one word.
    pub fn subtype(&self) -> &'static str {
        self.subtype
    }

    /// The member inside the subject that moved, where the change names one.
    pub fn member(&self) -> Option<&str> {
        self.member.as_deref()
    }
}

impl fmt::Display for ChangeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}/{}", self.category, self.subject, self.subtype)?;
        if let Some(member) = &self.member {
            write!(f, "/{member}")?;
        }
        Ok(())
    }
}

impl serde::Serialize for ChangeId {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

/// How a change relates the two revisions, where that is mechanically decidable.
///
/// Three variants, not design §21's seven. `Equivalent`, `Strengthened`, `Weakened` and `Unknown`
/// each need a comparison this slice does not make — the first three need an ordering over
/// predicates or consistency levels, and `Unknown` only means something once there is a comparison
/// that can fail to decide. A variant nothing can produce is a refusal that cannot fire, which is
/// the defect class `docs/reviews/2026-08-20-guard-efficacy-review.md` was written about.
///
/// So: two ways to widen what the system permits, two ways to narrow it, and everything else is
/// [`Changed`](Self::Changed) — a real answer, not a shrug. It says the revisions differ here and
/// that no direction follows from the difference alone.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "kebab-case")]
pub enum SemanticRelation {
    /// The revision permits something it did not permit before.
    ///
    /// An actor gained a command it may invoke; an enum or a union gained a variant. Both are a
    /// larger set of accepted values, and both are decidable by set membership alone.
    Expanded,
    /// The revision no longer permits something it permitted before.
    ///
    /// The mirror of [`Expanded`](Self::Expanded), and the direction that breaks a deployed caller:
    /// a grant removed is an authorization that now fails, and a variant removed is a value that now
    /// does not parse.
    Narrowed,
    /// The revisions differ, and no direction follows from the difference.
    Changed,
}

impl SemanticRelation {
    /// How it is written.
    pub const fn written(self) -> &'static str {
        match self {
            Self::Expanded => "expanded",
            Self::Narrowed => "narrowed",
            Self::Changed => "changed",
        }
    }

    /// The word a text report uses for it — what it does, rather than what it is called.
    pub const fn verb(self) -> &'static str {
        match self {
            Self::Expanded => "widens",
            Self::Narrowed => "narrows",
            Self::Changed => "changes",
        }
    }
}

impl fmt::Display for SemanticRelation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.written())
    }
}

/// One semantic change: which construct moved, and what happened to it.
///
/// The subject and the detail are paired by the variant rather than carried side by side, which is
/// what makes an impossible combination unrepresentable: there is no way to attach a
/// [`GrantAdded`](ActorChange::GrantAdded) to an event, because the only place an [`ActorChange`]
/// appears is beside an [`ActorRef`].
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "category", rename_all = "kebab-case")]
pub enum SemanticChange {
    /// The specification itself moved.
    System {
        /// Which system — the same name on both sides, because a pair that disagrees is refused.
        subject: QualifiedName,
        /// What moved.
        changed: SystemChange,
    },
    /// A declared type moved.
    Type {
        /// Which type.
        subject: DeclaredTypeRef,
        /// What moved.
        changed: TypeChange,
    },
    /// An event moved.
    Event {
        /// Which event.
        subject: EventRef,
        /// What moved.
        changed: EventChange,
    },
    /// A declared error moved.
    Error {
        /// Which error.
        subject: ErrorRef,
        /// What moved.
        changed: ErrorChange,
    },
    /// An actor moved.
    Actor {
        /// Which actor.
        subject: ActorRef,
        /// What moved.
        changed: ActorChange,
    },
    /// A component moved.
    Component {
        /// Which component.
        subject: ComponentRef,
        /// What moved.
        changed: ComponentChange,
    },
}

impl SemanticChange {
    /// Which category it belongs to.
    pub fn category(&self) -> ChangeCategory {
        match self {
            Self::System { .. } => ChangeCategory::System,
            Self::Type { .. } => ChangeCategory::Type,
            Self::Event { .. } => ChangeCategory::Event,
            Self::Error { .. } => ChangeCategory::Error,
            Self::Actor { .. } => ChangeCategory::Actor,
            Self::Component { .. } => ChangeCategory::Component,
        }
    }

    /// The construct this change is about, as a stable ESS semantic name.
    ///
    /// `None` for a [`System`](Self::System) change, and that is not an oversight: the system is not
    /// a construct declared *inside* the specification, so [`EssSemanticRef`] has no name for one —
    /// and inventing a twelfth variant here would be a name no other tool in this workspace resolves.
    /// [`Self::subject_name`] is what an id and a report use, and it answers for all six.
    pub fn subject(&self) -> Option<EssSemanticRef> {
        match self {
            Self::System { .. } => None,
            Self::Type { subject, .. } => Some(subject.clone().into()),
            Self::Event { subject, .. } => Some(subject.clone().into()),
            Self::Error { subject, .. } => Some(subject.clone().into()),
            Self::Actor { subject, .. } => Some(subject.clone().into()),
            Self::Component { subject, .. } => Some(subject.clone().into()),
        }
    }

    /// The construct's name, as it is written.
    pub fn subject_name(&self) -> String {
        match self {
            Self::System { subject, .. } => subject.to_string(),
            Self::Type { subject, .. } => subject.to_string(),
            Self::Event { subject, .. } => subject.to_string(),
            Self::Error { subject, .. } => subject.to_string(),
            Self::Actor { subject, .. } => subject.to_string(),
            Self::Component { subject, .. } => subject.to_string(),
        }
    }

    /// Which kind of change it is, as one word — the same word the document carries as `kind`.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::System { changed, .. } => changed.kind(),
            Self::Type { changed, .. } => changed.kind(),
            Self::Event { changed, .. } => changed.kind(),
            Self::Error { changed, .. } => changed.kind(),
            Self::Actor { changed, .. } => changed.kind(),
            Self::Component { changed, .. } => changed.kind(),
        }
    }

    /// The member inside the subject that moved, where the change names one.
    fn member(&self) -> Option<String> {
        match self {
            Self::System { changed, .. } => changed.member(),
            Self::Type { changed, .. } => changed.member(),
            Self::Event { changed, .. } => changed.member(),
            Self::Error { changed, .. } => changed.member(),
            Self::Actor { changed, .. } => changed.member(),
            Self::Component { changed, .. } => changed.member(),
        }
    }

    /// How this change relates the two revisions.
    ///
    /// Derived from the detail, never stored beside it. The document carries the answer so that a
    /// consumer in another language does not have to reimplement the classification, and
    /// [`RawEssDelta`](crate::RawEssDelta) checks the declared answer against this one when a delta
    /// is read back.
    pub fn relation(&self) -> SemanticRelation {
        match self {
            Self::System { changed, .. } => changed.relation(),
            Self::Type { changed, .. } => changed.relation(),
            Self::Event { changed, .. } => changed.relation(),
            Self::Error { changed, .. } => changed.relation(),
            Self::Actor { changed, .. } => changed.relation(),
            Self::Component { changed, .. } => changed.relation(),
        }
    }

    /// What this change is called: category, subject, subtype, member.
    pub fn id(&self) -> ChangeId {
        ChangeId {
            category: self.category(),
            subject: self.subject_name(),
            subtype: self.kind(),
            member: self.member(),
        }
    }

    /// One clause saying what moved, for a person reading a report.
    ///
    /// Reads after `<category> <subject>`, so a renderer writes
    /// `actor catalog.pricing.Auditor: may no longer invoke …`.
    pub fn describe(&self) -> String {
        match self {
            Self::System { changed, .. } => changed.describe(),
            Self::Type { changed, .. } => changed.describe(),
            Self::Event { changed, .. } => changed.describe(),
            Self::Error { changed, .. } => changed.describe(),
            Self::Actor { changed, .. } => changed.describe(),
            Self::Component { changed, .. } => changed.describe(),
        }
    }
}

/// Renders `Option<String>` for a report, so an absent summary reads as something rather than as a
/// gap in the sentence.
fn optional(value: Option<&String>) -> String {
    value.map_or_else(|| "(none)".to_owned(), |text| format!("`{text}`"))
}

/// What moved about the specification itself.
///
/// Two variants, and the second one is why: [`EssIr`](ess_compiler::ir::EssIr) has four fields that
/// are not a construct — `system`, `version`, `naming` and `summary` — and only two of them can
/// differ between two revisions a person can write.
///
/// * `system` is not compared. A pair that disagrees about it is
///   [refused](crate::DiffRefusal::DifferentSystem), which is the one refusal this slice has.
/// * `naming` is not compared, because **no shipped document shape can set it.** `SystemSpec`
///   carries a [`Naming`](ess_domain::name::Naming), and the reader that populates one —
///   `RawSystemSpec` in `ess-domain` — is `#[cfg(test)]`; the shape an author actually writes builds
///   the header with `Naming::default()`, so a system's wire name is its own last segment in every
///   specification that exists. Change kinds for it would be three refusals that cannot fire, which
///   is the defect class `docs/reviews/2026-08-20-guard-efficacy-review.md` was written about — so
///   they are not declared, and
///   `a_system_still_has_no_naming_a_document_can_set` asserts the gap is still there rather than
///   leaving the omission to be rediscovered.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum SystemChange {
    /// The specification's major version moved.
    ///
    /// A label, not the identity: [`Version`] is major-only on purpose, so two revisions of one
    /// system usually share it. What tells two resolutions apart is
    /// [`EssRevisionRef::spec_digest`](crate::EssRevisionRef::spec_digest).
    VersionChanged {
        /// What it was.
        before: Version,
        /// What it is.
        after: Version,
    },
    /// The paragraph saying what the system is moved.
    SummaryChanged {
        /// What it was.
        before: Option<String>,
        /// What it is.
        after: Option<String>,
    },
}

impl SystemChange {
    /// The subtype word, which is also the document's `kind`.
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::VersionChanged { .. } => "version-changed",
            Self::SummaryChanged { .. } => "summary-changed",
        }
    }

    /// Nothing: a system change is about the whole specification, not a member of it.
    #[allow(clippy::unused_self)]
    fn member(&self) -> Option<String> {
        None
    }

    /// How it relates the revisions. None of the four is a widening or a narrowing.
    pub const fn relation(&self) -> SemanticRelation {
        SemanticRelation::Changed
    }

    /// One clause saying what moved.
    pub fn describe(&self) -> String {
        match self {
            Self::VersionChanged { before, after } => format!("version {before} → {after}"),
            Self::SummaryChanged { before, after } => format!(
                "summary {} → {}",
                optional(before.as_ref()),
                optional(after.as_ref())
            ),
        }
    }
}

/// What moved about a declared type.
///
/// Complete over [`ResolvedType`](ess_compiler::ir::ResolvedType): its `naming`, and its `body` down
/// to every arm of [`ResolvedBody`](ess_compiler::ir::ResolvedBody) — a newtype's representation and
/// invariants, a struct's fields and invariants, an enum's variants and their order, a union's tag
/// and its variants' payloads. There is no `BodyChanged` catch-all: a catch-all carrying two
/// rendered strings is the untyped change design §10 refuses, wearing a typed name.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum TypeChange {
    /// The type is declared in the later revision and not in the earlier one.
    Added,
    /// The type was declared in the earlier revision and is not in the later one.
    Removed,
    /// The type is made of a different kind of thing: a newtype became a struct, an enum a union.
    KindChanged {
        /// What it was — `newtype`, `struct`, `enum`, `union`.
        before: String,
        /// What it is.
        after: String,
    },
    /// A newtype wraps something else now.
    RepresentationChanged {
        /// The type it wrapped.
        before: String,
        /// The type it wraps.
        after: String,
    },
    /// A struct gained a field.
    FieldAdded {
        /// Which field.
        field: String,
        /// Its type.
        type_ref: String,
    },
    /// A struct lost a field.
    FieldRemoved {
        /// Which field.
        field: String,
    },
    /// A struct field's type moved.
    FieldTypeChanged {
        /// Which field.
        field: String,
        /// The type it had.
        before: String,
        /// The type it has.
        after: String,
    },
    /// A struct field's wire name moved.
    FieldWireNameChanged {
        /// Which field.
        field: String,
        /// What it was called on the wire.
        before: String,
        /// What it is called.
        after: String,
    },
    /// A struct field's display name moved.
    FieldDisplayNameChanged {
        /// Which field.
        field: String,
        /// What it was shown as.
        before: String,
        /// What it is shown as.
        after: String,
    },
    /// A struct field's one-line summary moved. Documentation only.
    FieldSummaryChanged {
        /// Which field.
        field: String,
        /// What it said.
        before: Option<String>,
        /// What it says.
        after: Option<String>,
    },
    /// A struct's fields are declared in a different order.
    ///
    /// The model keeps declaration order, and every projection emits properties in it, so this is a
    /// real difference between two revisions rather than source noise. It is
    /// [`Changed`](SemanticRelation::Changed): reordering fields neither widens nor narrows what the
    /// type accepts.
    FieldOrderChanged {
        /// The order it had.
        before: Vec<String>,
        /// The order it has.
        after: Vec<String>,
    },
    /// An enum or a union gained a variant. The first of the two widenings.
    VariantAdded {
        /// Which variant.
        variant: String,
    },
    /// An enum or a union lost a variant. The first of the two narrowings.
    VariantRemoved {
        /// Which variant.
        variant: String,
    },
    /// An enum's variants are declared in a different order, with the same set of variants.
    VariantOrderChanged {
        /// The order it had.
        before: Vec<String>,
        /// The order it has.
        after: Vec<String>,
    },
    /// A union variant carries a different payload type.
    VariantTypeChanged {
        /// Which variant.
        variant: String,
        /// What it carried.
        before: String,
        /// What it carries.
        after: String,
    },
    /// A union's tag field is spelt differently.
    UnionTagChanged {
        /// The field that carried the variant name.
        before: String,
        /// The field that carries it.
        after: String,
    },
    /// A newtype's or a struct's own invariants differ.
    ///
    /// **That they differ, never which is stronger.** A type invariant is a
    /// [`Predicate`](aep_domain::predicate::Predicate), and deciding whether one implies another is
    /// a proof obligation rather than a comparison — the boundary this slice is drawn at, met from
    /// inside one of the six families it does compare.
    InvariantsChanged {
        /// The conditions it stated, as the author wrote them.
        before: Vec<String>,
        /// The conditions it states.
        after: Vec<String>,
    },
    /// The type's wire name moved — the name deployed consumers use.
    WireNameChanged {
        /// What it was.
        before: String,
        /// What it is.
        after: String,
    },
    /// The type's display name moved.
    DisplayNameChanged {
        /// What it was.
        before: String,
        /// What it is.
        after: String,
    },
    /// The type's one-line summary moved. Documentation only.
    SummaryChanged {
        /// What it said.
        before: Option<String>,
        /// What it says.
        after: Option<String>,
    },
}

impl TypeChange {
    /// The subtype word, which is also the document's `kind`.
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Added => "added",
            Self::Removed => "removed",
            Self::KindChanged { .. } => "kind-changed",
            Self::RepresentationChanged { .. } => "representation-changed",
            Self::FieldAdded { .. } => "field-added",
            Self::FieldRemoved { .. } => "field-removed",
            Self::FieldTypeChanged { .. } => "field-type-changed",
            Self::FieldWireNameChanged { .. } => "field-wire-name-changed",
            Self::FieldDisplayNameChanged { .. } => "field-display-name-changed",
            Self::FieldSummaryChanged { .. } => "field-summary-changed",
            Self::FieldOrderChanged { .. } => "field-order-changed",
            Self::VariantAdded { .. } => "variant-added",
            Self::VariantRemoved { .. } => "variant-removed",
            Self::VariantOrderChanged { .. } => "variant-order-changed",
            Self::VariantTypeChanged { .. } => "variant-type-changed",
            Self::UnionTagChanged { .. } => "union-tag-changed",
            Self::InvariantsChanged { .. } => "invariants-changed",
            Self::WireNameChanged { .. } => "wire-name-changed",
            Self::DisplayNameChanged { .. } => "display-name-changed",
            Self::SummaryChanged { .. } => "summary-changed",
        }
    }

    /// The member inside the type that moved, where the change names one.
    fn member(&self) -> Option<String> {
        match self {
            Self::FieldAdded { field, .. }
            | Self::FieldRemoved { field }
            | Self::FieldTypeChanged { field, .. }
            | Self::FieldWireNameChanged { field, .. }
            | Self::FieldDisplayNameChanged { field, .. }
            | Self::FieldSummaryChanged { field, .. } => Some(field.clone()),
            Self::VariantAdded { variant }
            | Self::VariantRemoved { variant }
            | Self::VariantTypeChanged { variant, .. } => Some(variant.clone()),
            _ => None,
        }
    }

    /// How it relates the revisions.
    ///
    /// A variant added widens what the type accepts and a variant removed narrows it, by set
    /// membership alone — no proof required, which is why these two are among the four the slice
    /// classifies. A field added to a struct is deliberately **not** a widening: a required field is
    /// a value every producer must now supply, which narrows what is accepted, and an optional one
    /// does not — and telling those apart is a rule this slice does not have.
    pub const fn relation(&self) -> SemanticRelation {
        match self {
            Self::VariantAdded { .. } => SemanticRelation::Expanded,
            Self::VariantRemoved { .. } => SemanticRelation::Narrowed,
            _ => SemanticRelation::Changed,
        }
    }

    /// One clause saying what moved.
    pub fn describe(&self) -> String {
        match self {
            Self::Added => "declared".to_owned(),
            Self::Removed => "no longer declared".to_owned(),
            Self::KindChanged { before, after } => format!("a {before} became a {after}"),
            Self::RepresentationChanged { before, after } => {
                format!("wraps `{after}` instead of `{before}`")
            }
            Self::FieldAdded { field, type_ref } => format!("field `{field}: {type_ref}` added"),
            Self::FieldRemoved { field } => format!("field `{field}` removed"),
            Self::FieldTypeChanged {
                field,
                before,
                after,
            } => format!("field `{field}` is `{after}`, was `{before}`"),
            Self::FieldWireNameChanged {
                field,
                before,
                after,
            } => format!("field `{field}` is `{after}` on the wire, was `{before}`"),
            Self::FieldDisplayNameChanged {
                field,
                before,
                after,
            } => format!("field `{field}` is shown as `{after}`, was `{before}`"),
            Self::FieldSummaryChanged { field, .. } => format!("field `{field}` summary changed"),
            Self::FieldOrderChanged { before, after } => {
                format!(
                    "fields reordered: {} → {}",
                    before.join(", "),
                    after.join(", ")
                )
            }
            Self::VariantAdded { variant } => format!("variant `{variant}` added"),
            Self::VariantRemoved { variant } => format!("variant `{variant}` removed"),
            Self::VariantOrderChanged { before, after } => {
                format!(
                    "variants reordered: {} → {}",
                    before.join(", "),
                    after.join(", ")
                )
            }
            Self::VariantTypeChanged {
                variant,
                before,
                after,
            } => format!("variant `{variant}` carries `{after}`, carried `{before}`"),
            Self::UnionTagChanged { before, after } => {
                format!("tag field `{before}` → `{after}`")
            }
            Self::InvariantsChanged { before, after } => format!(
                "invariants [{}] → [{}]",
                before.join("; "),
                after.join("; ")
            ),
            Self::WireNameChanged { before, after } => format!("wire name `{before}` → `{after}`"),
            Self::DisplayNameChanged { before, after } => {
                format!("display name `{before}` → `{after}`")
            }
            Self::SummaryChanged { before, after } => format!(
                "summary {} → {}",
                optional(before.as_ref()),
                optional(after.as_ref())
            ),
        }
    }
}

/// What moved about an event.
///
/// Complete over [`ResolvedEvent`](ess_compiler::ir::ResolvedEvent): its `domain`, its `fields` and
/// its `naming`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum EventChange {
    /// The event is declared in the later revision and not in the earlier one.
    Added,
    /// The event was declared in the earlier revision and is not in the later one.
    Removed,
    /// A different bounded context owns it.
    DomainChanged {
        /// The context that owned it.
        before: DomainRef,
        /// The context that owns it.
        after: DomainRef,
    },
    /// The payload gained a field.
    FieldAdded {
        /// Which field.
        field: String,
        /// Its type.
        type_ref: String,
    },
    /// The payload lost a field.
    FieldRemoved {
        /// Which field.
        field: String,
    },
    /// A payload field's type moved.
    FieldTypeChanged {
        /// Which field.
        field: String,
        /// The type it had.
        before: String,
        /// The type it has.
        after: String,
    },
    /// A payload field's wire name moved.
    FieldWireNameChanged {
        /// Which field.
        field: String,
        /// What it was called on the wire.
        before: String,
        /// What it is called.
        after: String,
    },
    /// A payload field's display name moved.
    FieldDisplayNameChanged {
        /// Which field.
        field: String,
        /// What it was shown as.
        before: String,
        /// What it is shown as.
        after: String,
    },
    /// A payload field's one-line summary moved. Documentation only.
    FieldSummaryChanged {
        /// Which field.
        field: String,
        /// What it said.
        before: Option<String>,
        /// What it says.
        after: Option<String>,
    },
    /// The payload's fields are declared in a different order.
    FieldOrderChanged {
        /// The order it had.
        before: Vec<String>,
        /// The order it has.
        after: Vec<String>,
    },
    /// The event's wire name moved.
    ///
    /// The change design §41 uses as its worked example, and the reason logical identity and wire
    /// name are separate in the model: this breaks every deployed consumer and moves no reference
    /// inside the specification, where renaming the event itself does the opposite.
    WireNameChanged {
        /// What it was.
        before: String,
        /// What it is.
        after: String,
    },
    /// The event's display name moved.
    DisplayNameChanged {
        /// What it was.
        before: String,
        /// What it is.
        after: String,
    },
    /// The event's one-line summary moved. Documentation only.
    SummaryChanged {
        /// What it said.
        before: Option<String>,
        /// What it says.
        after: Option<String>,
    },
}

impl EventChange {
    /// The subtype word, which is also the document's `kind`.
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Added => "added",
            Self::Removed => "removed",
            Self::DomainChanged { .. } => "domain-changed",
            Self::FieldAdded { .. } => "field-added",
            Self::FieldRemoved { .. } => "field-removed",
            Self::FieldTypeChanged { .. } => "field-type-changed",
            Self::FieldWireNameChanged { .. } => "field-wire-name-changed",
            Self::FieldDisplayNameChanged { .. } => "field-display-name-changed",
            Self::FieldSummaryChanged { .. } => "field-summary-changed",
            Self::FieldOrderChanged { .. } => "field-order-changed",
            Self::WireNameChanged { .. } => "wire-name-changed",
            Self::DisplayNameChanged { .. } => "display-name-changed",
            Self::SummaryChanged { .. } => "summary-changed",
        }
    }

    /// The field inside the event that moved, where the change names one.
    fn member(&self) -> Option<String> {
        match self {
            Self::FieldAdded { field, .. }
            | Self::FieldRemoved { field }
            | Self::FieldTypeChanged { field, .. }
            | Self::FieldWireNameChanged { field, .. }
            | Self::FieldDisplayNameChanged { field, .. }
            | Self::FieldSummaryChanged { field, .. } => Some(field.clone()),
            _ => None,
        }
    }

    /// How it relates the revisions. No event change is a widening or a narrowing in this slice.
    pub const fn relation(&self) -> SemanticRelation {
        SemanticRelation::Changed
    }

    /// One clause saying what moved.
    pub fn describe(&self) -> String {
        match self {
            Self::Added => "declared".to_owned(),
            Self::Removed => "no longer declared".to_owned(),
            Self::DomainChanged { before, after } => format!("owned by {after}, was {before}"),
            Self::FieldAdded { field, type_ref } => format!("field `{field}: {type_ref}` added"),
            Self::FieldRemoved { field } => format!("field `{field}` removed"),
            Self::FieldTypeChanged {
                field,
                before,
                after,
            } => format!("field `{field}` is `{after}`, was `{before}`"),
            Self::FieldWireNameChanged {
                field,
                before,
                after,
            } => format!("field `{field}` is `{after}` on the wire, was `{before}`"),
            Self::FieldDisplayNameChanged {
                field,
                before,
                after,
            } => format!("field `{field}` is shown as `{after}`, was `{before}`"),
            Self::FieldSummaryChanged { field, .. } => format!("field `{field}` summary changed"),
            Self::FieldOrderChanged { before, after } => {
                format!(
                    "fields reordered: {} → {}",
                    before.join(", "),
                    after.join(", ")
                )
            }
            Self::WireNameChanged { before, after } => format!("wire name `{before}` → `{after}`"),
            Self::DisplayNameChanged { before, after } => {
                format!("display name `{before}` → `{after}`")
            }
            Self::SummaryChanged { before, after } => format!(
                "summary {} → {}",
                optional(before.as_ref()),
                optional(after.as_ref())
            ),
        }
    }
}

/// What moved about a declared error.
///
/// Complete over [`ResolvedError`](ess_compiler::ir::ResolvedError): its `domain`, its `summary` and
/// its `fields`. It carries no [`Naming`](ess_domain::name::Naming) — an error is reported by its
/// declared name — so there is no wire or display name to move, and this family has one fewer
/// variant than the others rather than three that could never fire.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ErrorChange {
    /// The error is declared in the later revision and not in the earlier one.
    Added,
    /// The error was declared in the earlier revision and is not in the later one.
    Removed,
    /// A different bounded context owns it.
    DomainChanged {
        /// The context that owned it.
        before: DomainRef,
        /// The context that owns it.
        after: DomainRef,
    },
    /// The payload gained a field.
    FieldAdded {
        /// Which field.
        field: String,
        /// Its type.
        type_ref: String,
    },
    /// The payload lost a field.
    FieldRemoved {
        /// Which field.
        field: String,
    },
    /// A payload field's type moved.
    FieldTypeChanged {
        /// Which field.
        field: String,
        /// The type it had.
        before: String,
        /// The type it has.
        after: String,
    },
    /// A payload field's wire name moved.
    FieldWireNameChanged {
        /// Which field.
        field: String,
        /// What it was called on the wire.
        before: String,
        /// What it is called.
        after: String,
    },
    /// A payload field's display name moved.
    FieldDisplayNameChanged {
        /// Which field.
        field: String,
        /// What it was shown as.
        before: String,
        /// What it is shown as.
        after: String,
    },
    /// A payload field's one-line summary moved. Documentation only.
    FieldSummaryChanged {
        /// Which field.
        field: String,
        /// What it said.
        before: Option<String>,
        /// What it says.
        after: Option<String>,
    },
    /// The payload's fields are declared in a different order.
    FieldOrderChanged {
        /// The order it had.
        before: Vec<String>,
        /// The order it has.
        after: Vec<String>,
    },
    /// The line the person who receives the error is shown moved.
    SummaryChanged {
        /// What it said.
        before: Option<String>,
        /// What it says.
        after: Option<String>,
    },
}

impl ErrorChange {
    /// The subtype word, which is also the document's `kind`.
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Added => "added",
            Self::Removed => "removed",
            Self::DomainChanged { .. } => "domain-changed",
            Self::FieldAdded { .. } => "field-added",
            Self::FieldRemoved { .. } => "field-removed",
            Self::FieldTypeChanged { .. } => "field-type-changed",
            Self::FieldWireNameChanged { .. } => "field-wire-name-changed",
            Self::FieldDisplayNameChanged { .. } => "field-display-name-changed",
            Self::FieldSummaryChanged { .. } => "field-summary-changed",
            Self::FieldOrderChanged { .. } => "field-order-changed",
            Self::SummaryChanged { .. } => "summary-changed",
        }
    }

    /// The field inside the error that moved, where the change names one.
    fn member(&self) -> Option<String> {
        match self {
            Self::FieldAdded { field, .. }
            | Self::FieldRemoved { field }
            | Self::FieldTypeChanged { field, .. }
            | Self::FieldWireNameChanged { field, .. }
            | Self::FieldDisplayNameChanged { field, .. }
            | Self::FieldSummaryChanged { field, .. } => Some(field.clone()),
            _ => None,
        }
    }

    /// How it relates the revisions. No error change is a widening or a narrowing in this slice.
    pub const fn relation(&self) -> SemanticRelation {
        SemanticRelation::Changed
    }

    /// One clause saying what moved.
    pub fn describe(&self) -> String {
        match self {
            Self::Added => "declared".to_owned(),
            Self::Removed => "no longer declared".to_owned(),
            Self::DomainChanged { before, after } => format!("owned by {after}, was {before}"),
            Self::FieldAdded { field, type_ref } => format!("field `{field}: {type_ref}` added"),
            Self::FieldRemoved { field } => format!("field `{field}` removed"),
            Self::FieldTypeChanged {
                field,
                before,
                after,
            } => format!("field `{field}` is `{after}`, was `{before}`"),
            Self::FieldWireNameChanged {
                field,
                before,
                after,
            } => format!("field `{field}` is `{after}` on the wire, was `{before}`"),
            Self::FieldDisplayNameChanged {
                field,
                before,
                after,
            } => format!("field `{field}` is shown as `{after}`, was `{before}`"),
            Self::FieldSummaryChanged { field, .. } => format!("field `{field}` summary changed"),
            Self::FieldOrderChanged { before, after } => {
                format!(
                    "fields reordered: {} → {}",
                    before.join(", "),
                    after.join(", ")
                )
            }
            Self::SummaryChanged { before, after } => format!(
                "summary {} → {}",
                optional(before.as_ref()),
                optional(after.as_ref())
            ),
        }
    }
}

/// What moved about an actor.
///
/// Complete over [`ResolvedActor`](ess_compiler::ir::ResolvedActor): its `domain`, its `may` and its
/// `naming`. `may` is where two of the four mechanically decidable relations live.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ActorChange {
    /// The actor is declared in the later revision and not in the earlier one.
    Added,
    /// The actor was declared in the earlier revision and is not in the later one.
    Removed,
    /// A different bounded context declares it.
    DomainChanged {
        /// The context that declared it.
        before: DomainRef,
        /// The context that declares it.
        after: DomainRef,
    },
    /// The actor may invoke a command it could not invoke before. The second of the two widenings.
    GrantAdded {
        /// Which command it may now invoke.
        command: CommandRef,
    },
    /// The actor may no longer invoke a command it could invoke before. The second of the two
    /// narrowings, and the one that breaks a caller in production.
    GrantRemoved {
        /// Which command it may no longer invoke.
        command: CommandRef,
    },
    /// The actor's wire name moved.
    WireNameChanged {
        /// What it was.
        before: String,
        /// What it is.
        after: String,
    },
    /// The actor's display name moved.
    DisplayNameChanged {
        /// What it was.
        before: String,
        /// What it is.
        after: String,
    },
    /// The actor's one-line summary moved. Documentation only.
    SummaryChanged {
        /// What it said.
        before: Option<String>,
        /// What it says.
        after: Option<String>,
    },
}

impl ActorChange {
    /// The subtype word, which is also the document's `kind`.
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Added => "added",
            Self::Removed => "removed",
            Self::DomainChanged { .. } => "domain-changed",
            Self::GrantAdded { .. } => "grant-added",
            Self::GrantRemoved { .. } => "grant-removed",
            Self::WireNameChanged { .. } => "wire-name-changed",
            Self::DisplayNameChanged { .. } => "display-name-changed",
            Self::SummaryChanged { .. } => "summary-changed",
        }
    }

    /// The command a grant change is about, where the change is one.
    fn member(&self) -> Option<String> {
        match self {
            Self::GrantAdded { command } | Self::GrantRemoved { command } => {
                Some(command.to_string())
            }
            _ => None,
        }
    }

    /// How it relates the revisions.
    ///
    /// A grant added widens what the system permits and a grant removed narrows it, by set
    /// membership on [`ResolvedActor::may`](ess_compiler::ir::ResolvedActor::may) alone. Nothing
    /// else about an actor decides a direction.
    pub const fn relation(&self) -> SemanticRelation {
        match self {
            Self::GrantAdded { .. } => SemanticRelation::Expanded,
            Self::GrantRemoved { .. } => SemanticRelation::Narrowed,
            _ => SemanticRelation::Changed,
        }
    }

    /// One clause saying what moved.
    pub fn describe(&self) -> String {
        match self {
            Self::Added => "declared".to_owned(),
            Self::Removed => "no longer declared".to_owned(),
            Self::DomainChanged { before, after } => format!("declared by {after}, was {before}"),
            Self::GrantAdded { command } => format!("may invoke `{command}`"),
            Self::GrantRemoved { command } => format!("may no longer invoke `{command}`"),
            Self::WireNameChanged { before, after } => format!("wire name `{before}` → `{after}`"),
            Self::DisplayNameChanged { before, after } => {
                format!("display name `{before}` → `{after}`")
            }
            Self::SummaryChanged { before, after } => format!(
                "summary {} → {}",
                optional(before.as_ref()),
                optional(after.as_ref())
            ),
        }
    }
}

/// What moved about a component.
///
/// Complete over [`ResolvedComponent`](ess_compiler::ir::ResolvedComponent): its `owns`, its
/// `accepts`, its `publishes` and its `naming`.
///
/// # Why a component's sets are not grants
///
/// `accepts` and `publishes` are set memberships that look like
/// [`ActorChange::GrantAdded`] and are not classified as one. A component's
/// surface is an **assignment of work**: moving `CreateInvoice` from one component to another says
/// which process serves it, and the system permits exactly what it permitted before. An actor's
/// `may` is an **authority**, and adding one is a caller that could not do something and now can.
/// Calling the first a widening would put an ownership refactor and a permission grant in the same
/// bucket, which is the bucket a reviewer reads first.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ComponentChange {
    /// The component is declared in the later revision and not in the earlier one.
    Added,
    /// The component was declared in the earlier revision and is not in the later one.
    Removed,
    /// It owns a bounded context it did not own.
    OwnsAdded {
        /// Which context.
        domain: DomainRef,
    },
    /// It no longer owns a bounded context it owned.
    OwnsRemoved {
        /// Which context.
        domain: DomainRef,
    },
    /// It accepts a command it did not accept.
    AcceptsAdded {
        /// Which command.
        command: CommandRef,
    },
    /// It no longer accepts a command it accepted.
    AcceptsRemoved {
        /// Which command.
        command: CommandRef,
    },
    /// It publishes an event it did not publish.
    PublishesAdded {
        /// Which event.
        event: EventRef,
    },
    /// It no longer publishes an event it published.
    PublishesRemoved {
        /// Which event.
        event: EventRef,
    },
    /// The component's wire name moved.
    WireNameChanged {
        /// What it was.
        before: String,
        /// What it is.
        after: String,
    },
    /// The component's display name moved.
    DisplayNameChanged {
        /// What it was.
        before: String,
        /// What it is.
        after: String,
    },
    /// The component's one-line summary moved. Documentation only.
    SummaryChanged {
        /// What it said.
        before: Option<String>,
        /// What it says.
        after: Option<String>,
    },
}

impl ComponentChange {
    /// The subtype word, which is also the document's `kind`.
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Added => "added",
            Self::Removed => "removed",
            Self::OwnsAdded { .. } => "owns-added",
            Self::OwnsRemoved { .. } => "owns-removed",
            Self::AcceptsAdded { .. } => "accepts-added",
            Self::AcceptsRemoved { .. } => "accepts-removed",
            Self::PublishesAdded { .. } => "publishes-added",
            Self::PublishesRemoved { .. } => "publishes-removed",
            Self::WireNameChanged { .. } => "wire-name-changed",
            Self::DisplayNameChanged { .. } => "display-name-changed",
            Self::SummaryChanged { .. } => "summary-changed",
        }
    }

    /// The construct inside the component's surface that moved, where the change names one.
    fn member(&self) -> Option<String> {
        match self {
            Self::OwnsAdded { domain } | Self::OwnsRemoved { domain } => Some(domain.to_string()),
            Self::AcceptsAdded { command } | Self::AcceptsRemoved { command } => {
                Some(command.to_string())
            }
            Self::PublishesAdded { event } | Self::PublishesRemoved { event } => {
                Some(event.to_string())
            }
            _ => None,
        }
    }

    /// How it relates the revisions. See the type's own documentation for why a component's sets are
    /// not grants.
    pub const fn relation(&self) -> SemanticRelation {
        SemanticRelation::Changed
    }

    /// One clause saying what moved.
    pub fn describe(&self) -> String {
        match self {
            Self::Added => "declared".to_owned(),
            Self::Removed => "no longer declared".to_owned(),
            Self::OwnsAdded { domain } => format!("owns {domain}"),
            Self::OwnsRemoved { domain } => format!("no longer owns {domain}"),
            Self::AcceptsAdded { command } => format!("accepts `{command}`"),
            Self::AcceptsRemoved { command } => format!("no longer accepts `{command}`"),
            Self::PublishesAdded { event } => format!("publishes `{event}`"),
            Self::PublishesRemoved { event } => format!("no longer publishes `{event}`"),
            Self::WireNameChanged { before, after } => format!("wire name `{before}` → `{after}`"),
            Self::DisplayNameChanged { before, after } => {
                format!("display name `{before}` → `{after}`")
            }
            Self::SummaryChanged { before, after } => format!(
                "summary {} → {}",
                optional(before.as_ref()),
                optional(after.as_ref())
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds a command name for a fixture.
    fn command(name: &str) -> CommandRef {
        CommandRef::new(QualifiedName::new(name).expect("a valid qualified name"))
    }

    /// Builds a type name for a fixture.
    fn declared_type(name: &str) -> DeclaredTypeRef {
        DeclaredTypeRef::new(QualifiedName::new(name).expect("a valid qualified name"))
    }

    /// Builds an actor name for a fixture.
    fn actor(name: &str) -> ActorRef {
        ActorRef::new(QualifiedName::new(name).expect("a valid qualified name"))
    }

    #[test]
    fn a_change_id_names_its_category_subject_subtype_and_member_in_that_order() {
        let change = SemanticChange::Actor {
            subject: actor("catalog.pricing.PricingManager"),
            changed: ActorChange::GrantAdded {
                command: command("catalog.pricing.RetirePriceList"),
            },
        };

        assert_eq!(
            change.id().to_string(),
            "actor/catalog.pricing.PricingManager/grant-added/catalog.pricing.RetirePriceList"
        );
    }

    #[test]
    fn a_change_with_no_member_renders_three_parts_rather_than_a_trailing_slash() {
        let change = SemanticChange::Type {
            subject: declared_type("catalog.pricing.Currency"),
            changed: TypeChange::Added,
        };

        assert_eq!(
            change.id().to_string(),
            "type/catalog.pricing.Currency/added"
        );
    }

    #[test]
    fn the_canonical_order_is_the_category_order_and_not_the_alphabet() {
        // Alphabetically `actor` precedes `system` and `type`. Design §60's order does not, and the
        // whole point of writing the order down as a contract is that it is not the accident a
        // sort on the rendered id would produce — so the fixture is exactly the pair that would
        // come out backwards under one.
        let mut ids = [
            SemanticChange::Actor {
                subject: actor("catalog.pricing.Auditor"),
                changed: ActorChange::Added,
            }
            .id(),
            SemanticChange::System {
                subject: QualifiedName::new("catalog").expect("a valid name"),
                changed: SystemChange::VersionChanged {
                    before: Version::V1,
                    after: Version::new(2).expect("v2"),
                },
            }
            .id(),
        ];
        ids.sort();

        assert_eq!(ids[0].category(), ChangeCategory::System);
        assert_eq!(ids[1].category(), ChangeCategory::Actor);
        assert!(
            ids[0].to_string() > ids[1].to_string(),
            "the fixture must be one a lexicographic sort would order the other way, or this test \
             passes whether the contract holds or not"
        );
    }

    #[test]
    fn only_a_grant_and_a_variant_decide_a_direction() {
        assert_eq!(
            ActorChange::GrantAdded {
                command: command("catalog.pricing.RetirePriceList")
            }
            .relation(),
            SemanticRelation::Expanded
        );
        assert_eq!(
            ActorChange::GrantRemoved {
                command: command("catalog.pricing.RetirePriceList")
            }
            .relation(),
            SemanticRelation::Narrowed
        );
        assert_eq!(
            TypeChange::VariantAdded {
                variant: "CHF".to_owned()
            }
            .relation(),
            SemanticRelation::Expanded
        );
        assert_eq!(
            TypeChange::VariantRemoved {
                variant: "GBP".to_owned()
            }
            .relation(),
            SemanticRelation::Narrowed
        );

        // The two nearest misses, and they are the reason the rule is written down rather than
        // inferred: both are set memberships that look exactly like a grant.
        assert_eq!(
            ComponentChange::AcceptsAdded {
                command: command("catalog.pricing.RetirePriceList")
            }
            .relation(),
            SemanticRelation::Changed,
            "a component accepting a command is an assignment of work, not an authority"
        );
        assert_eq!(
            TypeChange::FieldAdded {
                field: "note".to_owned(),
                type_ref: "String".to_owned()
            }
            .relation(),
            SemanticRelation::Changed,
            "a required field added narrows what is accepted and an optional one does not; this \
             slice has no rule that tells them apart"
        );
    }
}
