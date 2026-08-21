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
//! | [`EntityChange`] | `ResolvedEntity::{domain, identity, fields, lifecycle, invariants, naming}` — `state_type` is derived from the name, and the lifecycle's *state set* is the synthesised `<Entity>.State` enum, which the type family already reports variant by variant |
//! | [`CommandChange`] | `ResolvedCommand::{domain, input, outcomes, naming}`, and each outcome down to `ResolvedOutcome::{condition, subject, emits, payload, error, summary}` — `test_strategy` is a pure function of the condition (`OutcomeCondition::test_strategy`), so comparing it would report one edit twice |
//! | [`EventChange`] | `ResolvedEvent::{domain, fields, naming}` |
//! | [`ErrorChange`] | `ResolvedError::{domain, summary, fields}` — it carries no `Naming` |
//! | [`ViewChange`] | `ResolvedView::{domain, source, fields, filter, consistency, naming}` — `assertion_style` is a pure function of the consistency (`Consistency::assertion_style`) |
//! | [`ActorChange`] | `ResolvedActor::{domain, may, naming}` |
//! | [`ComponentChange`] | `ResolvedComponent::{owns, accepts, publishes, naming}` |
//! | [`BindingChange`] | `ResolvedBinding::{event, command, mapping, failure, escalation, naming}` — `delivery` has one inhabitant, so a change kind for it could never fire (see `a_binding_still_has_one_delivery_a_document_can_write` in `tests/canonical.rs`) |
//!
//! `name` is the map key in every case, so it is identity rather than a comparable field: a change
//! of name is an [`Added`](TypeChange::Added) and a [`Removed`](TypeChange::Removed), never a
//! rename. Design §6, and it is deliberate — see [`mod@crate::diff`].
//!
//! # A predicate can only ever be *changed*
//!
//! Gap register D-1, executed. Where a construct's sub-part is a predicate — an entity's or a
//! type's invariants, an outcome's `when`, a view's filter — the comparison is **conservative
//! canonical equality** over the parsed [`Predicate`](aep_domain::predicate::Predicate), and the
//! only two outcomes are silence (canonically equal) and [`Changed`](SemanticRelation::Changed)
//! (canonically different). No direction is ever derived: implication between predicates is a proof
//! obligation, not a comparison, and it stays refused. That is structural, not remembered — every
//! predicate-bearing change kind's `relation` is a `const fn` returning
//! [`Changed`](SemanticRelation::Changed) without reading the predicates at all, so there is no
//! code path on which the content of a predicate could decide a direction.

use std::fmt;

use ess_conformance::scenario::{
    ActorRef, BindingRef, CommandRef, ComponentRef, DeclaredTypeRef, DomainRef, EntityRef,
    ErrorRef, EssSemanticRef, EventRef, ViewRef,
};
use ess_domain::name::{QualifiedName, Version};

/// Where a change sits in the canonical order.
///
/// Design §60 makes the category order a **format contract** rather than an accident of iteration,
/// and this enum is that contract: the declaration order below is the sort order, and it is design
/// §60's own list restricted to the ten families this crate compares — wave 5's six, and the four
/// W7.2 added in the positions §60 already reserved for them. Alphabetical order would put `actor`
/// before `system`, which no reader of the document asked for.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "kebab-case")]
pub enum ChangeCategory {
    /// The specification itself: its version, its naming, its summary.
    System,
    /// A declared type.
    Type,
    /// An entity.
    Entity,
    /// A command.
    Command,
    /// An event.
    Event,
    /// A declared error.
    Error,
    /// A view.
    View,
    /// An actor.
    Actor,
    /// A component.
    Component,
    /// A binding.
    Binding,
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
            Self::Entity => "entity",
            Self::Command => "command",
            Self::Event => "event",
            Self::Error => "error",
            Self::View => "view",
            Self::Actor => "actor",
            Self::Component => "component",
            Self::Binding => "binding",
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
    /// An entity moved.
    Entity {
        /// Which entity.
        subject: EntityRef,
        /// What moved.
        changed: EntityChange,
    },
    /// A command moved.
    Command {
        /// Which command.
        subject: CommandRef,
        /// What moved.
        changed: CommandChange,
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
    /// A view moved.
    View {
        /// Which view.
        subject: ViewRef,
        /// What moved.
        changed: ViewChange,
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
    /// A binding moved.
    Binding {
        /// Which binding.
        subject: BindingRef,
        /// What moved.
        changed: BindingChange,
    },
}

impl SemanticChange {
    /// Which category it belongs to.
    pub fn category(&self) -> ChangeCategory {
        match self {
            Self::System { .. } => ChangeCategory::System,
            Self::Type { .. } => ChangeCategory::Type,
            Self::Entity { .. } => ChangeCategory::Entity,
            Self::Command { .. } => ChangeCategory::Command,
            Self::Event { .. } => ChangeCategory::Event,
            Self::Error { .. } => ChangeCategory::Error,
            Self::View { .. } => ChangeCategory::View,
            Self::Actor { .. } => ChangeCategory::Actor,
            Self::Component { .. } => ChangeCategory::Component,
            Self::Binding { .. } => ChangeCategory::Binding,
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
            Self::Entity { subject, .. } => Some(subject.clone().into()),
            Self::Command { subject, .. } => Some(subject.clone().into()),
            Self::Event { subject, .. } => Some(subject.clone().into()),
            Self::Error { subject, .. } => Some(subject.clone().into()),
            Self::View { subject, .. } => Some(subject.clone().into()),
            Self::Actor { subject, .. } => Some(subject.clone().into()),
            Self::Component { subject, .. } => Some(subject.clone().into()),
            Self::Binding { subject, .. } => Some(subject.clone().into()),
        }
    }

    /// The construct's name, as it is written.
    pub fn subject_name(&self) -> String {
        match self {
            Self::System { subject, .. } => subject.to_string(),
            Self::Type { subject, .. } => subject.to_string(),
            Self::Entity { subject, .. } => subject.to_string(),
            Self::Command { subject, .. } => subject.to_string(),
            Self::Event { subject, .. } => subject.to_string(),
            Self::Error { subject, .. } => subject.to_string(),
            Self::View { subject, .. } => subject.to_string(),
            Self::Actor { subject, .. } => subject.to_string(),
            Self::Component { subject, .. } => subject.to_string(),
            Self::Binding { subject, .. } => subject.to_string(),
        }
    }

    /// Which kind of change it is, as one word — the same word the document carries as `kind`.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::System { changed, .. } => changed.kind(),
            Self::Type { changed, .. } => changed.kind(),
            Self::Entity { changed, .. } => changed.kind(),
            Self::Command { changed, .. } => changed.kind(),
            Self::Event { changed, .. } => changed.kind(),
            Self::Error { changed, .. } => changed.kind(),
            Self::View { changed, .. } => changed.kind(),
            Self::Actor { changed, .. } => changed.kind(),
            Self::Component { changed, .. } => changed.kind(),
            Self::Binding { changed, .. } => changed.kind(),
        }
    }

    /// The member inside the subject that moved, where the change names one.
    fn member(&self) -> Option<String> {
        match self {
            Self::System { changed, .. } => changed.member(),
            Self::Type { changed, .. } => changed.member(),
            Self::Entity { changed, .. } => changed.member(),
            Self::Command { changed, .. } => changed.member(),
            Self::Event { changed, .. } => changed.member(),
            Self::Error { changed, .. } => changed.member(),
            Self::View { changed, .. } => changed.member(),
            Self::Actor { changed, .. } => changed.member(),
            Self::Component { changed, .. } => changed.member(),
            Self::Binding { changed, .. } => changed.member(),
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
            Self::Entity { changed, .. } => changed.relation(),
            Self::Command { changed, .. } => changed.relation(),
            Self::Event { changed, .. } => changed.relation(),
            Self::Error { changed, .. } => changed.relation(),
            Self::View { changed, .. } => changed.relation(),
            Self::Actor { changed, .. } => changed.relation(),
            Self::Component { changed, .. } => changed.relation(),
            Self::Binding { changed, .. } => changed.relation(),
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
            Self::Entity { changed, .. } => changed.describe(),
            Self::Command { changed, .. } => changed.describe(),
            Self::Event { changed, .. } => changed.describe(),
            Self::Error { changed, .. } => changed.describe(),
            Self::View { changed, .. } => changed.describe(),
            Self::Actor { changed, .. } => changed.describe(),
            Self::Component { changed, .. } => changed.describe(),
            Self::Binding { changed, .. } => changed.describe(),
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
    /// a proof obligation rather than a comparison — the same rule every predicate-bearing kind in
    /// every family follows since W7.2 (gap register D-1).
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

/// What moved about an entity.
///
/// Complete over [`ResolvedEntity`](ess_compiler::ir::ResolvedEntity): its `domain`, its
/// `identity`, its `fields`, its `lifecycle`, its `invariants` and its `naming`. Two fields are
/// deliberately not compared here, and neither is a gap:
///
/// * `state_type` is derived from the entity's own name (`<Entity>.State`), so with the name as the
///   map key it cannot differ between two revisions of one entity.
/// * the lifecycle's **state set** is exactly the variant set of that synthesised enum, which lives
///   in the IR's type map and is compared by the type family — a state added is already
///   `type/<Entity>.State/variant-added`, and reporting it here as well would be one edit reported
///   twice. What the enum cannot say — where an instance starts, where it may rest, which moves are
///   permitted — is what the lifecycle kinds below say.
///
/// The identity has its own kinds rather than travelling through the field walk, because an entity
/// has exactly one and it is not a member of `fields`: reporting a renamed identity as a field
/// removed and a field added would claim the entity lost a field it never listed.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum EntityChange {
    /// The entity is declared in the later revision and not in the earlier one.
    Added,
    /// The entity was declared in the earlier revision and is not in the later one.
    Removed,
    /// A different bounded context owns it.
    DomainChanged {
        /// The context that owned it.
        before: DomainRef,
        /// The context that owns it.
        after: DomainRef,
    },
    /// The field an instance is identified by has a different name.
    ///
    /// A rename, exceptionally — an entity has exactly one identity, so "which declaration is this"
    /// has no room for the ambiguity the no-rename rule exists to refuse.
    IdentityRenamed {
        /// What it was called.
        before: String,
        /// What it is called.
        after: String,
    },
    /// The identity field's type moved.
    IdentityTypeChanged {
        /// The type it had.
        before: String,
        /// The type it has.
        after: String,
    },
    /// The identity field's wire name moved.
    IdentityWireNameChanged {
        /// What it was called on the wire.
        before: String,
        /// What it is called.
        after: String,
    },
    /// The identity field's display name moved.
    IdentityDisplayNameChanged {
        /// What it was shown as.
        before: String,
        /// What it is shown as.
        after: String,
    },
    /// The identity field's one-line summary moved. Documentation only.
    IdentitySummaryChanged {
        /// What it said.
        before: Option<String>,
        /// What it says.
        after: Option<String>,
    },
    /// The entity gained a field.
    FieldAdded {
        /// Which field.
        field: String,
        /// Its type.
        type_ref: String,
    },
    /// The entity lost a field.
    FieldRemoved {
        /// Which field.
        field: String,
    },
    /// A field's type moved.
    FieldTypeChanged {
        /// Which field.
        field: String,
        /// The type it had.
        before: String,
        /// The type it has.
        after: String,
    },
    /// A field's wire name moved.
    FieldWireNameChanged {
        /// Which field.
        field: String,
        /// What it was called on the wire.
        before: String,
        /// What it is called.
        after: String,
    },
    /// A field's display name moved.
    FieldDisplayNameChanged {
        /// Which field.
        field: String,
        /// What it was shown as.
        before: String,
        /// What it is shown as.
        after: String,
    },
    /// A field's one-line summary moved. Documentation only.
    FieldSummaryChanged {
        /// Which field.
        field: String,
        /// What it said.
        before: Option<String>,
        /// What it says.
        after: Option<String>,
    },
    /// The entity's fields are declared in a different order.
    FieldOrderChanged {
        /// The order it had.
        before: Vec<String>,
        /// The order it has.
        after: Vec<String>,
    },
    /// A new instance starts somewhere else.
    InitialStateChanged {
        /// Where one started.
        before: String,
        /// Where one starts.
        after: String,
    },
    /// A state an instance may now rest in forever.
    TerminalAdded {
        /// Which state.
        state: String,
    },
    /// A state an instance may no longer rest in forever.
    TerminalRemoved {
        /// Which state.
        state: String,
    },
    /// The lifecycle permits a move it did not permit.
    ///
    /// [`Changed`](SemanticRelation::Changed), deliberately, although it looks like a widening: what
    /// a lifecycle permits is meaningless apart from the command outcomes that take the move, and
    /// `ess-domain` refuses a transition no outcome takes — so a transition never arrives alone, and
    /// classifying the arrival would classify half of an edit whose other half is a command change.
    TransitionAdded {
        /// The move's own name.
        transition: String,
    },
    /// The lifecycle no longer permits a move it permitted.
    TransitionRemoved {
        /// The move's own name.
        transition: String,
    },
    /// A declared move starts or ends somewhere else.
    TransitionRouteChanged {
        /// The move's own name.
        transition: String,
        /// The route it took, `from, from -> to`.
        before: String,
        /// The route it takes.
        after: String,
    },
    /// What must hold of every instance at rest differs.
    ///
    /// **That they differ, never which is stronger** — gap register D-1's boundary, and the change
    /// kind W7.2 exists to make nameable: before this slice, an edited entity invariant fell into
    /// the fail-closed catch-all and owed everything. The comparison is canonical equality over the
    /// parsed predicates *and* the statements the author wrote, because the model keeps both and a
    /// documentation projection quotes the statement.
    InvariantsChanged {
        /// The conditions it stated, as the author wrote them.
        before: Vec<String>,
        /// The conditions it states.
        after: Vec<String>,
    },
    /// The entity's wire name moved.
    WireNameChanged {
        /// What it was.
        before: String,
        /// What it is.
        after: String,
    },
    /// The entity's display name moved.
    DisplayNameChanged {
        /// What it was.
        before: String,
        /// What it is.
        after: String,
    },
    /// The entity's one-line summary moved. Documentation only.
    SummaryChanged {
        /// What it said.
        before: Option<String>,
        /// What it says.
        after: Option<String>,
    },
}

impl EntityChange {
    /// The subtype word, which is also the document's `kind`.
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Added => "added",
            Self::Removed => "removed",
            Self::DomainChanged { .. } => "domain-changed",
            Self::IdentityRenamed { .. } => "identity-renamed",
            Self::IdentityTypeChanged { .. } => "identity-type-changed",
            Self::IdentityWireNameChanged { .. } => "identity-wire-name-changed",
            Self::IdentityDisplayNameChanged { .. } => "identity-display-name-changed",
            Self::IdentitySummaryChanged { .. } => "identity-summary-changed",
            Self::FieldAdded { .. } => "field-added",
            Self::FieldRemoved { .. } => "field-removed",
            Self::FieldTypeChanged { .. } => "field-type-changed",
            Self::FieldWireNameChanged { .. } => "field-wire-name-changed",
            Self::FieldDisplayNameChanged { .. } => "field-display-name-changed",
            Self::FieldSummaryChanged { .. } => "field-summary-changed",
            Self::FieldOrderChanged { .. } => "field-order-changed",
            Self::InitialStateChanged { .. } => "initial-state-changed",
            Self::TerminalAdded { .. } => "terminal-added",
            Self::TerminalRemoved { .. } => "terminal-removed",
            Self::TransitionAdded { .. } => "transition-added",
            Self::TransitionRemoved { .. } => "transition-removed",
            Self::TransitionRouteChanged { .. } => "transition-route-changed",
            Self::InvariantsChanged { .. } => "invariants-changed",
            Self::WireNameChanged { .. } => "wire-name-changed",
            Self::DisplayNameChanged { .. } => "display-name-changed",
            Self::SummaryChanged { .. } => "summary-changed",
        }
    }

    /// The member inside the entity that moved, where the change names one.
    fn member(&self) -> Option<String> {
        match self {
            Self::FieldAdded { field, .. }
            | Self::FieldRemoved { field }
            | Self::FieldTypeChanged { field, .. }
            | Self::FieldWireNameChanged { field, .. }
            | Self::FieldDisplayNameChanged { field, .. }
            | Self::FieldSummaryChanged { field, .. } => Some(field.clone()),
            Self::TerminalAdded { state } | Self::TerminalRemoved { state } => Some(state.clone()),
            Self::TransitionAdded { transition }
            | Self::TransitionRemoved { transition }
            | Self::TransitionRouteChanged { transition, .. } => Some(transition.clone()),
            _ => None,
        }
    }

    /// How it relates the revisions. No entity change decides a direction — see
    /// [`TransitionAdded`](Self::TransitionAdded) for the nearest miss, and the module
    /// documentation for why an invariant never does.
    pub const fn relation(&self) -> SemanticRelation {
        SemanticRelation::Changed
    }

    /// One clause saying what moved.
    pub fn describe(&self) -> String {
        match self {
            Self::Added => "declared".to_owned(),
            Self::Removed => "no longer declared".to_owned(),
            Self::DomainChanged { before, after } => format!("owned by {after}, was {before}"),
            Self::IdentityRenamed { before, after } => {
                format!("identified by `{after}`, was `{before}`")
            }
            Self::IdentityTypeChanged { before, after } => {
                format!("identity is `{after}`, was `{before}`")
            }
            Self::IdentityWireNameChanged { before, after } => {
                format!("identity is `{after}` on the wire, was `{before}`")
            }
            Self::IdentityDisplayNameChanged { before, after } => {
                format!("identity is shown as `{after}`, was `{before}`")
            }
            Self::IdentitySummaryChanged { .. } => "identity summary changed".to_owned(),
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
            Self::InitialStateChanged { before, after } => {
                format!("a new instance starts in `{after}`, started in `{before}`")
            }
            Self::TerminalAdded { state } => format!("may rest in `{state}` forever"),
            Self::TerminalRemoved { state } => format!("may no longer rest in `{state}` forever"),
            Self::TransitionAdded { transition } => format!("permits the move `{transition}`"),
            Self::TransitionRemoved { transition } => {
                format!("no longer permits the move `{transition}`")
            }
            Self::TransitionRouteChanged {
                transition,
                before,
                after,
            } => format!("move `{transition}` goes {after}, went {before}"),
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

/// What moved about a command.
///
/// Complete over [`ResolvedCommand`](ess_compiler::ir::ResolvedCommand): its `domain`, its `input`,
/// its `outcomes` and its `naming`, with each outcome compared down to every field of
/// [`ResolvedOutcome`](ess_compiler::ir::ResolvedOutcome) except `test_strategy` — which is a pure
/// function of the condition ([`OutcomeCondition::test_strategy`](ess_domain::command::OutcomeCondition::test_strategy)),
/// so a strategy that moved is a condition that moved, already reported.
///
/// An outcome's name is its identity inside the command, exactly as a field's name is inside a
/// struct: an outcome renamed is an outcome removed and an outcome added.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum CommandChange {
    /// The command is declared in the later revision and not in the earlier one.
    Added,
    /// The command was declared in the earlier revision and is not in the later one.
    Removed,
    /// A different bounded context owns it.
    DomainChanged {
        /// The context that owned it.
        before: DomainRef,
        /// The context that owns it.
        after: DomainRef,
    },
    /// The input gained a field.
    InputAdded {
        /// Which field.
        field: String,
        /// Its type.
        type_ref: String,
    },
    /// The input lost a field.
    InputRemoved {
        /// Which field.
        field: String,
    },
    /// An input field's type moved.
    InputTypeChanged {
        /// Which field.
        field: String,
        /// The type it had.
        before: String,
        /// The type it has.
        after: String,
    },
    /// An input field's wire name moved.
    InputWireNameChanged {
        /// Which field.
        field: String,
        /// What it was called on the wire.
        before: String,
        /// What it is called.
        after: String,
    },
    /// An input field's display name moved.
    InputDisplayNameChanged {
        /// Which field.
        field: String,
        /// What it was shown as.
        before: String,
        /// What it is shown as.
        after: String,
    },
    /// An input field's one-line summary moved. Documentation only.
    InputSummaryChanged {
        /// Which field.
        field: String,
        /// What it said.
        before: Option<String>,
        /// What it says.
        after: Option<String>,
    },
    /// The input's fields are declared in a different order.
    InputOrderChanged {
        /// The order it had.
        before: Vec<String>,
        /// The order it has.
        after: Vec<String>,
    },
    /// The command can result in something it could not.
    OutcomeAdded {
        /// Which branch.
        outcome: String,
    },
    /// The command can no longer result in something it could.
    OutcomeRemoved {
        /// Which branch.
        outcome: String,
    },
    /// What decides that a branch is the one taken differs.
    ///
    /// The change kind gap register D-1 unblocked, and the polarity is the whole point: where both
    /// conditions are `when:` predicates, the comparison is canonical equality over the parsed
    /// [`Predicate`](aep_domain::predicate::Predicate)s — a `when:` rewritten with different
    /// spacing resolves to the same predicate and says nothing here, and a genuinely different one
    /// is *changed* with no direction, because whether the new guard accepts everything the old
    /// one did is a proof obligation, not a comparison.
    OutcomeConditionChanged {
        /// Which branch.
        outcome: String,
        /// What decided it, rendered canonically.
        before: String,
        /// What decides it.
        after: String,
    },
    /// What a branch does to which entity differs.
    OutcomeSubjectChanged {
        /// Which branch.
        outcome: String,
        /// What it did, rendered — `None` when it changed no entity.
        before: Option<String>,
        /// What it does.
        after: Option<String>,
    },
    /// The events a branch emits differ.
    OutcomeEmitsChanged {
        /// Which branch.
        outcome: String,
        /// What it emitted, in the order it happened.
        before: Vec<String>,
        /// What it emits.
        after: Vec<String>,
    },
    /// Which emitted payload fields a branch determines, or from what, differs.
    OutcomePayloadChanged {
        /// Which branch.
        outcome: String,
        /// What it determined, one line per field.
        before: Vec<String>,
        /// What it determines.
        after: Vec<String>,
    },
    /// The error a branch reports differs.
    OutcomeErrorChanged {
        /// Which branch.
        outcome: String,
        /// What it reported — `None` when it reported none.
        before: Option<String>,
        /// What it reports.
        after: Option<String>,
    },
    /// A branch's one-line summary moved. Documentation only.
    OutcomeSummaryChanged {
        /// Which branch.
        outcome: String,
        /// What it said.
        before: Option<String>,
        /// What it says.
        after: Option<String>,
    },
    /// The outcomes are declared in a different order.
    ///
    /// A real difference, not source noise: a conditional outcome is decided in declaration order,
    /// so two `when:` branches swapped can take different inputs to different branches.
    OutcomeOrderChanged {
        /// The order it had.
        before: Vec<String>,
        /// The order it has.
        after: Vec<String>,
    },
    /// The command's wire name moved.
    WireNameChanged {
        /// What it was.
        before: String,
        /// What it is.
        after: String,
    },
    /// The command's display name moved.
    DisplayNameChanged {
        /// What it was.
        before: String,
        /// What it is.
        after: String,
    },
    /// The command's one-line summary moved. Documentation only.
    SummaryChanged {
        /// What it said.
        before: Option<String>,
        /// What it says.
        after: Option<String>,
    },
}

impl CommandChange {
    /// The subtype word, which is also the document's `kind`.
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Added => "added",
            Self::Removed => "removed",
            Self::DomainChanged { .. } => "domain-changed",
            Self::InputAdded { .. } => "input-added",
            Self::InputRemoved { .. } => "input-removed",
            Self::InputTypeChanged { .. } => "input-type-changed",
            Self::InputWireNameChanged { .. } => "input-wire-name-changed",
            Self::InputDisplayNameChanged { .. } => "input-display-name-changed",
            Self::InputSummaryChanged { .. } => "input-summary-changed",
            Self::InputOrderChanged { .. } => "input-order-changed",
            Self::OutcomeAdded { .. } => "outcome-added",
            Self::OutcomeRemoved { .. } => "outcome-removed",
            Self::OutcomeConditionChanged { .. } => "outcome-condition-changed",
            Self::OutcomeSubjectChanged { .. } => "outcome-subject-changed",
            Self::OutcomeEmitsChanged { .. } => "outcome-emits-changed",
            Self::OutcomePayloadChanged { .. } => "outcome-payload-changed",
            Self::OutcomeErrorChanged { .. } => "outcome-error-changed",
            Self::OutcomeSummaryChanged { .. } => "outcome-summary-changed",
            Self::OutcomeOrderChanged { .. } => "outcome-order-changed",
            Self::WireNameChanged { .. } => "wire-name-changed",
            Self::DisplayNameChanged { .. } => "display-name-changed",
            Self::SummaryChanged { .. } => "summary-changed",
        }
    }

    /// The member inside the command that moved, where the change names one.
    fn member(&self) -> Option<String> {
        match self {
            Self::InputAdded { field, .. }
            | Self::InputRemoved { field }
            | Self::InputTypeChanged { field, .. }
            | Self::InputWireNameChanged { field, .. }
            | Self::InputDisplayNameChanged { field, .. }
            | Self::InputSummaryChanged { field, .. } => Some(field.clone()),
            Self::OutcomeAdded { outcome }
            | Self::OutcomeRemoved { outcome }
            | Self::OutcomeConditionChanged { outcome, .. }
            | Self::OutcomeSubjectChanged { outcome, .. }
            | Self::OutcomeEmitsChanged { outcome, .. }
            | Self::OutcomePayloadChanged { outcome, .. }
            | Self::OutcomeErrorChanged { outcome, .. }
            | Self::OutcomeSummaryChanged { outcome, .. } => Some(outcome.clone()),
            _ => None,
        }
    }

    /// How it relates the revisions. No command change decides a direction: an outcome added looks
    /// like a widening and is not one — whether callers can reach it depends on every other
    /// branch's condition, which is exactly the proof this slice refuses to attempt.
    pub const fn relation(&self) -> SemanticRelation {
        SemanticRelation::Changed
    }

    /// One clause saying what moved.
    pub fn describe(&self) -> String {
        match self {
            Self::Added => "declared".to_owned(),
            Self::Removed => "no longer declared".to_owned(),
            Self::DomainChanged { before, after } => format!("owned by {after}, was {before}"),
            Self::InputAdded { field, type_ref } => format!("input `{field}: {type_ref}` added"),
            Self::InputRemoved { field } => format!("input `{field}` removed"),
            Self::InputTypeChanged {
                field,
                before,
                after,
            } => format!("input `{field}` is `{after}`, was `{before}`"),
            Self::InputWireNameChanged {
                field,
                before,
                after,
            } => format!("input `{field}` is `{after}` on the wire, was `{before}`"),
            Self::InputDisplayNameChanged {
                field,
                before,
                after,
            } => format!("input `{field}` is shown as `{after}`, was `{before}`"),
            Self::InputSummaryChanged { field, .. } => format!("input `{field}` summary changed"),
            Self::InputOrderChanged { before, after } => {
                format!(
                    "inputs reordered: {} → {}",
                    before.join(", "),
                    after.join(", ")
                )
            }
            Self::OutcomeAdded { outcome } => format!("outcome `{outcome}` added"),
            Self::OutcomeRemoved { outcome } => format!("outcome `{outcome}` removed"),
            Self::OutcomeConditionChanged {
                outcome,
                before,
                after,
            } => format!("outcome `{outcome}` is decided by `{after}`, was `{before}`"),
            Self::OutcomeSubjectChanged {
                outcome,
                before,
                after,
            } => format!(
                "outcome `{outcome}` {}, {} before",
                after.as_deref().unwrap_or("changes no entity"),
                before.as_deref().unwrap_or("changed none")
            ),
            Self::OutcomeEmitsChanged {
                outcome,
                before,
                after,
            } => format!(
                "outcome `{outcome}` emits [{}], emitted [{}]",
                after.join(", "),
                before.join(", ")
            ),
            Self::OutcomePayloadChanged { outcome, .. } => {
                format!("outcome `{outcome}` determines different payload fields")
            }
            Self::OutcomeErrorChanged {
                outcome,
                before,
                after,
            } => format!(
                "outcome `{outcome}` reports {}, reported {}",
                optional(after.as_ref()),
                optional(before.as_ref())
            ),
            Self::OutcomeSummaryChanged { outcome, .. } => {
                format!("outcome `{outcome}` summary changed")
            }
            Self::OutcomeOrderChanged { before, after } => {
                format!(
                    "outcomes reordered: {} → {}",
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

/// What moved about a view.
///
/// Complete over [`ResolvedView`](ess_compiler::ir::ResolvedView): its `domain`, its `source`, its
/// `fields`, its `filter`, its `consistency` and its `naming`. `assertion_style` is a pure function
/// of the consistency ([`Consistency::assertion_style`](ess_domain::view::Consistency::assertion_style)),
/// so it is not compared: a style that moved is a consistency that moved, already reported.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ViewChange {
    /// The view is declared in the later revision and not in the earlier one.
    Added,
    /// The view was declared in the earlier revision and is not in the later one.
    Removed,
    /// A different bounded context owns it.
    DomainChanged {
        /// The context that owned it.
        before: DomainRef,
        /// The context that owns it.
        after: DomainRef,
    },
    /// The view projects a different entity.
    SourceChanged {
        /// The entity it projected.
        before: EntityRef,
        /// The entity it projects.
        after: EntityRef,
    },
    /// The view exposes a field it did not.
    FieldAdded {
        /// Which field.
        field: String,
        /// Its type.
        type_ref: String,
    },
    /// The view no longer exposes a field it did.
    FieldRemoved {
        /// Which field.
        field: String,
    },
    /// An exposed field's type moved.
    FieldTypeChanged {
        /// Which field.
        field: String,
        /// The type it had.
        before: String,
        /// The type it has.
        after: String,
    },
    /// An exposed field's wire name moved.
    FieldWireNameChanged {
        /// Which field.
        field: String,
        /// What it was called on the wire.
        before: String,
        /// What it is called.
        after: String,
    },
    /// An exposed field's display name moved.
    FieldDisplayNameChanged {
        /// Which field.
        field: String,
        /// What it was shown as.
        before: String,
        /// What it is shown as.
        after: String,
    },
    /// An exposed field's one-line summary moved. Documentation only.
    FieldSummaryChanged {
        /// Which field.
        field: String,
        /// What it said.
        before: Option<String>,
        /// What it says.
        after: Option<String>,
    },
    /// The exposed fields are declared in a different order.
    FieldOrderChanged {
        /// The order it had.
        before: Vec<String>,
        /// The order it has.
        after: Vec<String>,
    },
    /// Which instances the view contains differs.
    ///
    /// The third predicate-bearing change kind, under D-1's rule: canonical equality over the
    /// parsed filters, `None` meaning *all instances*, and any difference is *changed* — a filter
    /// that admits more rows and one that admits fewer are the same answer here, because telling
    /// them apart is implication.
    FilterChanged {
        /// The filter it had, rendered canonically — `None` when it contained every instance.
        before: Option<String>,
        /// The filter it has.
        after: Option<String>,
    },
    /// How soon the view reflects a command that has returned differs.
    ///
    /// [`Changed`](SemanticRelation::Changed), although `read_your_writes` is strictly the stronger
    /// promise: the four directional relations are about what the *system permits*, and a
    /// consistency level is a promise about when an observation holds, which is a different axis —
    /// classifying it would put a timing promise in the bucket a reviewer reads for authority.
    ConsistencyChanged {
        /// The promise it made.
        before: String,
        /// The promise it makes.
        after: String,
    },
    /// The view's wire name moved.
    WireNameChanged {
        /// What it was.
        before: String,
        /// What it is.
        after: String,
    },
    /// The view's display name moved.
    DisplayNameChanged {
        /// What it was.
        before: String,
        /// What it is.
        after: String,
    },
    /// The view's one-line summary moved. Documentation only.
    SummaryChanged {
        /// What it said.
        before: Option<String>,
        /// What it says.
        after: Option<String>,
    },
}

impl ViewChange {
    /// The subtype word, which is also the document's `kind`.
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Added => "added",
            Self::Removed => "removed",
            Self::DomainChanged { .. } => "domain-changed",
            Self::SourceChanged { .. } => "source-changed",
            Self::FieldAdded { .. } => "field-added",
            Self::FieldRemoved { .. } => "field-removed",
            Self::FieldTypeChanged { .. } => "field-type-changed",
            Self::FieldWireNameChanged { .. } => "field-wire-name-changed",
            Self::FieldDisplayNameChanged { .. } => "field-display-name-changed",
            Self::FieldSummaryChanged { .. } => "field-summary-changed",
            Self::FieldOrderChanged { .. } => "field-order-changed",
            Self::FilterChanged { .. } => "filter-changed",
            Self::ConsistencyChanged { .. } => "consistency-changed",
            Self::WireNameChanged { .. } => "wire-name-changed",
            Self::DisplayNameChanged { .. } => "display-name-changed",
            Self::SummaryChanged { .. } => "summary-changed",
        }
    }

    /// The field inside the view that moved, where the change names one.
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

    /// How it relates the revisions. No view change decides a direction — see
    /// [`FilterChanged`](Self::FilterChanged) and [`ConsistencyChanged`](Self::ConsistencyChanged)
    /// for the two nearest misses.
    pub const fn relation(&self) -> SemanticRelation {
        SemanticRelation::Changed
    }

    /// One clause saying what moved.
    pub fn describe(&self) -> String {
        match self {
            Self::Added => "declared".to_owned(),
            Self::Removed => "no longer declared".to_owned(),
            Self::DomainChanged { before, after } => format!("owned by {after}, was {before}"),
            Self::SourceChanged { before, after } => {
                format!("projects `{after}`, projected `{before}`")
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
            Self::FilterChanged { before, after } => format!(
                "contains {}, contained {}",
                after
                    .as_ref()
                    .map_or_else(|| "every instance".to_owned(), |it| format!("`{it}`")),
                before
                    .as_ref()
                    .map_or_else(|| "every instance".to_owned(), |it| format!("`{it}`"))
            ),
            Self::ConsistencyChanged { before, after } => {
                format!("consistency `{before}` → `{after}`")
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

/// What moved about a binding.
///
/// Complete over [`ResolvedBinding`](ess_compiler::ir::ResolvedBinding): its `event`, its
/// `command`, its `mapping`, its `failure` with its `escalation`, and its `naming`. Two absences
/// are deliberate:
///
/// * `delivery` has one inhabitant ([`Delivery::AtLeastOnce`](ess_domain::binding::Delivery::AtLeastOnce)),
///   so a `delivery-changed` kind could never fire — the defect class
///   `docs/reviews/2026-08-20-guard-efficacy-review.md` exists about — and
///   `a_binding_still_has_one_delivery_a_document_can_write` in `tests/canonical.rs` asserts the
///   gap is still there rather than leaving it to be rediscovered.
/// * the mapping's order is the invoked command's declaration order by construction, so a mapping
///   reordered without an entry changing is a command input reordered, reported there.
///
/// The escalation travels inside [`FailureChanged`](Self::FailureChanged) rather than having a kind
/// of its own, because `ess-domain` refuses either half without the other: an escalation that moved
/// *is* the failure answer having moved.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum BindingChange {
    /// The binding is declared in the later revision and not in the earlier one.
    Added,
    /// The binding was declared in the earlier revision and is not in the later one.
    Removed,
    /// It reacts to a different event.
    EventChanged {
        /// The event it reacted to.
        before: EventRef,
        /// The event it reacts to.
        after: EventRef,
    },
    /// It invokes a different command.
    CommandChanged {
        /// The command it invoked.
        before: CommandRef,
        /// The command it invokes.
        after: CommandRef,
    },
    /// A command input is filled that was not.
    MappingAdded {
        /// Which command input.
        target: String,
        /// Where its value comes from, rendered.
        value: String,
    },
    /// A command input is no longer filled.
    MappingRemoved {
        /// Which command input.
        target: String,
    },
    /// A filled command input is filled differently.
    ///
    /// From a different source, through a different conversion, or with a source whose resolved
    /// type moved — the comparison is over the whole resolved mapping entry, because a narrower one
    /// would have to decide which differences "count", and deciding that wrongly is silence about a
    /// model that moved.
    MappingValueChanged {
        /// Which command input.
        target: String,
        /// How it was filled, rendered.
        before: String,
        /// How it is filled.
        after: String,
    },
    /// What happens when the command does not run differs.
    FailureChanged {
        /// The policy it had, rendered — an escalation names the event it publishes.
        before: String,
        /// The policy it has.
        after: String,
    },
    /// The binding's wire name moved.
    WireNameChanged {
        /// What it was.
        before: String,
        /// What it is.
        after: String,
    },
    /// The binding's display name moved.
    DisplayNameChanged {
        /// What it was.
        before: String,
        /// What it is.
        after: String,
    },
    /// The binding's one-line summary moved. Documentation only.
    SummaryChanged {
        /// What it said.
        before: Option<String>,
        /// What it says.
        after: Option<String>,
    },
}

impl BindingChange {
    /// The subtype word, which is also the document's `kind`.
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Added => "added",
            Self::Removed => "removed",
            Self::EventChanged { .. } => "event-changed",
            Self::CommandChanged { .. } => "command-changed",
            Self::MappingAdded { .. } => "mapping-added",
            Self::MappingRemoved { .. } => "mapping-removed",
            Self::MappingValueChanged { .. } => "mapping-value-changed",
            Self::FailureChanged { .. } => "failure-changed",
            Self::WireNameChanged { .. } => "wire-name-changed",
            Self::DisplayNameChanged { .. } => "display-name-changed",
            Self::SummaryChanged { .. } => "summary-changed",
        }
    }

    /// The command input a mapping change is about, where the change is one.
    fn member(&self) -> Option<String> {
        match self {
            Self::MappingAdded { target, .. }
            | Self::MappingRemoved { target }
            | Self::MappingValueChanged { target, .. } => Some(target.clone()),
            _ => None,
        }
    }

    /// How it relates the revisions. No binding change decides a direction: a binding is a wiring
    /// of work, like a component's surface, not an authority.
    pub const fn relation(&self) -> SemanticRelation {
        SemanticRelation::Changed
    }

    /// One clause saying what moved.
    pub fn describe(&self) -> String {
        match self {
            Self::Added => "declared".to_owned(),
            Self::Removed => "no longer declared".to_owned(),
            Self::EventChanged { before, after } => {
                format!("reacts to `{after}`, reacted to `{before}`")
            }
            Self::CommandChanged { before, after } => {
                format!("invokes `{after}`, invoked `{before}`")
            }
            Self::MappingAdded { target, value } => format!("fills `{target}` from {value}"),
            Self::MappingRemoved { target } => format!("no longer fills `{target}`"),
            Self::MappingValueChanged {
                target,
                before,
                after,
            } => format!("fills `{target}` from {after}, filled it from {before}"),
            Self::FailureChanged { before, after } => {
                format!("on failure {after}, was {before}")
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
