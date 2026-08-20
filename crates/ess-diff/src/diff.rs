//! The comparison itself: six walks over two IRs, and one refusal.
//!
//! # Identity, and why nothing here infers a rename
//!
//! Design §6, and it is the single most consequential rule in the slice. Every map compared below is
//! keyed by a [`QualifiedName`] — the model's stable logical identity — so a construct present on
//! one side and absent on the other is an addition or a removal, full stop.
//!
//! `InvoiceCreated` removed and `InvoiceIssued` added is reported as *removed* and *added*, and
//! never as a rename, however similar the two names look. Similarity is not evidence: a rename and a
//! delete-plus-create have different consequences for every deployed consumer, and a heuristic that
//! guesses between them is a report that is wrong in the direction nobody checks. If a later
//! workflow needs to claim continuity across two identities, that claim is an author's to make
//! explicitly, and this engine will not manufacture it.
//!
//! # What is compared, and what identity means inside a construct
//!
//! Within a construct, a field, a variant and a grant are identified by name too, in the scope that
//! declares them. So a field renamed is a field removed and a field added, for the same reason.
//!
//! # No handle crosses
//!
//! Nothing here calls an [`EssIr`] handle accessor. Every walk reads the name-keyed maps directly,
//! and every handle becomes an
//! [`EssSemanticRef`](ess_conformance::scenario::EssSemanticRef) name through the one-way door
//! `From<&Handle>` before it is compared. That matters because a handle minted by the `before` IR is
//! structurally identical to one minted by `after`, freely interchangeable, and fatal: the accessor
//! panics rather than returning `None`. `tests/canonical.rs` reads this crate's sources for the
//! accessor names, because a discipline nothing checks is a discipline that has already been broken
//! somewhere.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use ess_compiler::ir::{EssIr, ResolvedBody, ResolvedField, ResolvedType, ResolvedTypeRef};
use ess_conformance::scenario::{
    ActorRef, CommandRef, ComponentRef, DeclaredTypeRef, DomainRef, ErrorRef, EventRef,
};
use ess_domain::name::{Naming, QualifiedName};

use crate::change::{
    ActorChange, ComponentChange, ErrorChange, EventChange, SemanticChange, SystemChange,
    TypeChange,
};
use crate::delta::{EssDelta, EssRevisionRef};

/// Why a pair of specifications cannot be compared at all.
///
/// One variant, deliberately. Design §5 lists four preconditions and design §63 proposes a refusal
/// for each; three of the four are not refusals this code can reach:
///
/// * *both compile* — an [`EssIr`] exists only because it compiled, so there is nothing to check;
/// * *no handle crosses* — a discipline of this crate, enforced by a source scan rather than by a
///   runtime answer;
/// * *the implementation understands both IR format versions* — the IR carries no format version, so
///   `UnsupportedIrVersion` would be a refusal with nothing to read, and a refusal that cannot fire
///   is the defect class `docs/reviews/2026-08-20-guard-efficacy-review.md` exists about.
///
/// What is left is the one that can happen and matters: two specifications that describe different
/// systems. Everything else design §63 proposes waits until there is a reason for it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "refused", rename_all = "kebab-case")]
pub enum DiffRefusal {
    /// The two specifications name different systems.
    ///
    /// Comparing revisions of one system is what a delta means. Comparing `billing` with `ordering`
    /// is a different feature — every construct would be reported added and removed, which is a
    /// plausible answer to a question nobody asked — so it is refused rather than answered.
    DifferentSystem {
        /// What the `--from` side calls itself.
        before: QualifiedName,
        /// What the `--to` side calls itself.
        after: QualifiedName,
    },
}

impl fmt::Display for DiffRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DifferentSystem { before, after } => write!(
                f,
                "these are two systems, not two revisions: `{before}` and `{after}`. A delta \
                 answers what moved between revisions of one system"
            ),
        }
    }
}

impl std::error::Error for DiffRefusal {}

/// What moved between two revisions of one system.
///
/// Refuses when the two specifications name different systems, and answers otherwise — including
/// with an empty delta, which is the answer when two source trees differ in every byte and mean the
/// same thing.
pub fn diff(before: &EssIr, after: &EssIr) -> Result<EssDelta, DiffRefusal> {
    if before.system != after.system {
        return Err(DiffRefusal::DifferentSystem {
            before: before.system.clone(),
            after: after.system.clone(),
        });
    }

    let mut changes = Vec::new();
    system_changes(before, after, &mut changes);
    type_changes(before, after, &mut changes);
    event_changes(before, after, &mut changes);
    error_changes(before, after, &mut changes);
    actor_changes(before, after, &mut changes);
    component_changes(before, after, &mut changes);

    Ok(EssDelta::new(
        EssRevisionRef::of(before),
        EssRevisionRef::of(after),
        changes,
    ))
}

// ---- shared comparisons ----------------------------------------------------------------------

/// Every key either side declares, once, in name order.
fn keys<'a, K: Ord, V>(before: &'a BTreeMap<K, V>, after: &'a BTreeMap<K, V>) -> BTreeSet<&'a K> {
    before.keys().chain(after.keys()).collect()
}

/// What a name is called on the wire, with the model's own fallback applied.
///
/// The fallback is why this exists rather than a comparison of two [`Naming`]s: `wire:` left out and
/// `wire:` written out as the declaration's own name are the *same wire name*, and reporting the
/// second as a change would be a delta that fires when an author makes something explicit.
fn wire_name<'a>(naming: &'a Naming, fallback: &'a str) -> &'a str {
    naming.wire.as_deref().unwrap_or(fallback)
}

/// What a name is shown as, with the model's own fallback applied.
fn display_name<'a>(naming: &'a Naming, fallback: &'a str) -> &'a str {
    naming.display.as_deref().unwrap_or(fallback)
}

/// One difference between two [`Naming`]s, before it is attached to a family.
///
/// A private middle step, so the rule that a written-out default is not a change lives in one place
/// and five families read it, rather than in five places that drift apart one at a time.
enum NamingDelta {
    /// The wire name moved.
    Wire(String, String),
    /// The display name moved.
    Display(String, String),
    /// The one-line summary moved.
    Summary(Option<String>, Option<String>),
}

/// Every difference between two namings of one construct.
fn naming_deltas(before: &Naming, after: &Naming, fallback: &str) -> Vec<NamingDelta> {
    let mut deltas = Vec::new();
    let (was, is) = (wire_name(before, fallback), wire_name(after, fallback));
    if was != is {
        deltas.push(NamingDelta::Wire(was.to_owned(), is.to_owned()));
    }
    let (was, is) = (
        display_name(before, fallback),
        display_name(after, fallback),
    );
    if was != is {
        deltas.push(NamingDelta::Display(was.to_owned(), is.to_owned()));
    }
    if before.summary != after.summary {
        deltas.push(NamingDelta::Summary(
            before.summary.clone(),
            after.summary.clone(),
        ));
    }
    deltas
}

/// One difference between two field lists, before it is attached to a family.
///
/// Shared by types, events and errors, which each declare a `Vec<ResolvedField>` and each need the
/// same six answers about it. Written once because three copies of a field walk are three places for
/// "a field renamed is a rename" to be invented independently.
enum FieldDelta {
    /// A field is declared that was not.
    Added(String, String),
    /// A field is no longer declared.
    Removed(String),
    /// A field's type moved.
    TypeChanged(String, String, String),
    /// A field's wire name moved.
    Wire(String, String, String),
    /// A field's display name moved.
    Display(String, String, String),
    /// A field's summary moved.
    Summary(String, Option<String>, Option<String>),
    /// The fields common to both sides are declared in a different order.
    Reordered(Vec<String>, Vec<String>),
}

/// Every difference between two field lists.
///
/// Order is compared over the fields **both** sides declare, not over the whole lists: inserting a
/// field necessarily changes the whole list, and reporting that as a reordering on top of the
/// addition would be one edit reported twice.
fn field_deltas(
    before: &[ResolvedField],
    after: &[ResolvedField],
    mut push: impl FnMut(FieldDelta),
) {
    let was: BTreeMap<&str, &ResolvedField> = before
        .iter()
        .map(|field| (field.name.as_str(), field))
        .collect();
    let is: BTreeMap<&str, &ResolvedField> = after
        .iter()
        .map(|field| (field.name.as_str(), field))
        .collect();

    for name in keys(&was, &is) {
        match (was.get(name), is.get(name)) {
            (None, Some(field)) => push(FieldDelta::Added(
                (*name).to_owned(),
                field.type_ref.to_string(),
            )),
            (Some(_), None) => push(FieldDelta::Removed((*name).to_owned())),
            (Some(old), Some(new)) => {
                if old.type_ref != new.type_ref {
                    push(FieldDelta::TypeChanged(
                        (*name).to_owned(),
                        old.type_ref.to_string(),
                        new.type_ref.to_string(),
                    ));
                }
                for delta in naming_deltas(&old.naming, &new.naming, name) {
                    match delta {
                        NamingDelta::Wire(a, b) => {
                            push(FieldDelta::Wire((*name).to_owned(), a, b));
                        }
                        NamingDelta::Display(a, b) => {
                            push(FieldDelta::Display((*name).to_owned(), a, b));
                        }
                        NamingDelta::Summary(a, b) => {
                            push(FieldDelta::Summary((*name).to_owned(), a, b));
                        }
                    }
                }
            }
            (None, None) => unreachable!("a key came from one of the two maps"),
        }
    }

    let common: BTreeSet<&str> = was
        .keys()
        .filter(|name| is.contains_key(*name))
        .copied()
        .collect();
    let order = |fields: &[ResolvedField]| -> Vec<String> {
        fields
            .iter()
            .filter(|field| common.contains(field.name.as_str()))
            .map(|field| field.name.clone())
            .collect()
    };
    let (was_order, is_order) = (order(before), order(after));
    if was_order != is_order {
        push(FieldDelta::Reordered(was_order, is_order));
    }
}

// ---- the six families ------------------------------------------------------------------------

/// The specification itself: version, naming, summary.
fn system_changes(before: &EssIr, after: &EssIr, changes: &mut Vec<SemanticChange>) {
    let subject = after.system.clone();
    let mut push = |changed: SystemChange| {
        changes.push(SemanticChange::System {
            subject: subject.clone(),
            changed,
        });
    };

    if before.version != after.version {
        push(SystemChange::VersionChanged {
            before: before.version,
            after: after.version,
        });
    }
    // `EssIr::naming` is not compared. No document shape an author can write populates it — see
    // `SystemChange` — so comparing it would be a walk that can only ever find nothing, wearing the
    // costume of a check.
    if before.summary != after.summary {
        push(SystemChange::SummaryChanged {
            before: before.summary.clone(),
            after: after.summary.clone(),
        });
    }
}

/// How a type body is written, for a change that says one became another.
fn body_kind(body: &ResolvedBody) -> &'static str {
    match body {
        ResolvedBody::Newtype { .. } => "newtype",
        ResolvedBody::Struct { .. } => "struct",
        ResolvedBody::Enum { .. } => "enum",
        ResolvedBody::Union { .. } => "union",
    }
}

/// The conditions a body states, as the author wrote them.
fn invariants(body: &ResolvedBody) -> Vec<String> {
    match body {
        ResolvedBody::Newtype { invariants, .. } | ResolvedBody::Struct { invariants, .. } => {
            invariants
                .iter()
                .map(|invariant| invariant.statement.clone())
                .collect()
        }
        ResolvedBody::Enum { .. } | ResolvedBody::Union { .. } => Vec::new(),
    }
}

/// Every difference between two enums' variants.
///
/// The set difference decides the two relations; the order of the variants **both** sides declare
/// decides whether anything else moved. Order is compared over the common variants only, for the
/// reason a field list is: inserting one necessarily moves the rest, and reporting that as a
/// reordering on top of the addition is one edit reported twice.
fn enum_changes(was: &[String], is: &[String], push: &mut impl FnMut(TypeChange)) {
    let (declared, now): (BTreeSet<&String>, BTreeSet<&String>) =
        (was.iter().collect(), is.iter().collect());
    for variant in now.difference(&declared) {
        push(TypeChange::VariantAdded {
            variant: (*variant).clone(),
        });
    }
    for variant in declared.difference(&now) {
        push(TypeChange::VariantRemoved {
            variant: (*variant).clone(),
        });
    }

    let common = |variants: &[String], other: &BTreeSet<&String>| -> Vec<String> {
        variants
            .iter()
            .filter(|variant| other.contains(variant))
            .cloned()
            .collect()
    };
    let (was_order, is_order) = (common(was, &now), common(is, &declared));
    if was_order != is_order {
        push(TypeChange::VariantOrderChanged {
            before: was_order,
            after: is_order,
        });
    }
}

/// Every difference between two unions' variants.
///
/// Keyed by tag value, which is a union's own identity for a variant, so a payload type that moved
/// is reported as a moved payload rather than as one variant removed and another added.
fn union_changes(
    was: &BTreeMap<String, ResolvedTypeRef>,
    is: &BTreeMap<String, ResolvedTypeRef>,
    push: &mut impl FnMut(TypeChange),
) {
    for variant in keys(was, is) {
        match (was.get(variant), is.get(variant)) {
            (None, Some(_)) => push(TypeChange::VariantAdded {
                variant: variant.clone(),
            }),
            (Some(_), None) => push(TypeChange::VariantRemoved {
                variant: variant.clone(),
            }),
            (Some(old), Some(new)) if old != new => push(TypeChange::VariantTypeChanged {
                variant: variant.clone(),
                before: old.to_string(),
                after: new.to_string(),
            }),
            _ => {}
        }
    }
}

/// Every difference between two type bodies.
///
/// The one place in the slice where a construct's *insides* are compared arm by arm. It is complete
/// over [`ResolvedBody`]'s four arms, and a fifth arm added to the model stops this function
/// compiling rather than falling into a catch-all — which is design §10's argument for a typed
/// change model, applied to the code that produces one.
fn body_changes(before: &ResolvedBody, after: &ResolvedBody, mut push: impl FnMut(TypeChange)) {
    if body_kind(before) != body_kind(after) {
        push(TypeChange::KindChanged {
            before: body_kind(before).to_owned(),
            after: body_kind(after).to_owned(),
        });
        return;
    }

    match (before, after) {
        (ResolvedBody::Newtype { of: was, .. }, ResolvedBody::Newtype { of: is, .. }) => {
            if was != is {
                push(TypeChange::RepresentationChanged {
                    before: was.to_string(),
                    after: is.to_string(),
                });
            }
        }
        (ResolvedBody::Struct { fields: was, .. }, ResolvedBody::Struct { fields: is, .. }) => {
            field_deltas(was, is, |delta| match delta {
                FieldDelta::Added(field, type_ref) => {
                    push(TypeChange::FieldAdded { field, type_ref });
                }
                FieldDelta::Removed(field) => push(TypeChange::FieldRemoved { field }),
                FieldDelta::TypeChanged(field, a, b) => push(TypeChange::FieldTypeChanged {
                    field,
                    before: a,
                    after: b,
                }),
                FieldDelta::Wire(field, a, b) => push(TypeChange::FieldWireNameChanged {
                    field,
                    before: a,
                    after: b,
                }),
                FieldDelta::Display(field, a, b) => push(TypeChange::FieldDisplayNameChanged {
                    field,
                    before: a,
                    after: b,
                }),
                FieldDelta::Summary(field, a, b) => push(TypeChange::FieldSummaryChanged {
                    field,
                    before: a,
                    after: b,
                }),
                FieldDelta::Reordered(a, b) => push(TypeChange::FieldOrderChanged {
                    before: a,
                    after: b,
                }),
            });
        }
        (ResolvedBody::Enum { variants: was }, ResolvedBody::Enum { variants: is }) => {
            enum_changes(was, is, &mut push);
        }
        (
            ResolvedBody::Union {
                tag: was_tag,
                variants: was,
            },
            ResolvedBody::Union {
                tag: is_tag,
                variants: is,
            },
        ) => {
            if was_tag != is_tag {
                push(TypeChange::UnionTagChanged {
                    before: was_tag.clone(),
                    after: is_tag.clone(),
                });
            }
            union_changes(was, is, &mut push);
        }
        _ => unreachable!("the two bodies are the same kind: it was checked above"),
    }

    let (was, is) = (invariants(before), invariants(after));
    if was != is {
        push(TypeChange::InvariantsChanged {
            before: was,
            after: is,
        });
    }
}

/// Every difference between the two revisions' declared types.
fn type_changes(before: &EssIr, after: &EssIr, changes: &mut Vec<SemanticChange>) {
    for name in keys(&before.types, &after.types) {
        let subject = DeclaredTypeRef::new(name.clone());
        let mut push = |changed: TypeChange| {
            changes.push(SemanticChange::Type {
                subject: subject.clone(),
                changed,
            });
        };
        match (before.types.get(name), after.types.get(name)) {
            (None, Some(_)) => push(TypeChange::Added),
            (Some(_), None) => push(TypeChange::Removed),
            (Some(was), Some(is)) => compare_types(was, is, name, &mut push),
            (None, None) => unreachable!("a key came from one of the two maps"),
        }
    }
}

/// One type against its counterpart.
fn compare_types(
    before: &ResolvedType,
    after: &ResolvedType,
    name: &QualifiedName,
    push: &mut impl FnMut(TypeChange),
) {
    body_changes(&before.body, &after.body, &mut *push);
    for delta in naming_deltas(&before.naming, &after.naming, name.local()) {
        match delta {
            NamingDelta::Wire(a, b) => push(TypeChange::WireNameChanged {
                before: a,
                after: b,
            }),
            NamingDelta::Display(a, b) => push(TypeChange::DisplayNameChanged {
                before: a,
                after: b,
            }),
            NamingDelta::Summary(a, b) => push(TypeChange::SummaryChanged {
                before: a,
                after: b,
            }),
        }
    }
}

/// Every difference between the two revisions' events.
fn event_changes(before: &EssIr, after: &EssIr, changes: &mut Vec<SemanticChange>) {
    for name in keys(&before.events, &after.events) {
        let subject = EventRef::new(name.clone());
        let mut push = |changed: EventChange| {
            changes.push(SemanticChange::Event {
                subject: subject.clone(),
                changed,
            });
        };
        match (before.events.get(name), after.events.get(name)) {
            (None, Some(_)) => push(EventChange::Added),
            (Some(_), None) => push(EventChange::Removed),
            (Some(was), Some(is)) => {
                let (owned, owns) = (DomainRef::from(&was.domain), DomainRef::from(&is.domain));
                if owned != owns {
                    push(EventChange::DomainChanged {
                        before: owned,
                        after: owns,
                    });
                }
                field_deltas(&was.fields, &is.fields, |delta| match delta {
                    FieldDelta::Added(field, type_ref) => {
                        push(EventChange::FieldAdded { field, type_ref });
                    }
                    FieldDelta::Removed(field) => push(EventChange::FieldRemoved { field }),
                    FieldDelta::TypeChanged(field, a, b) => push(EventChange::FieldTypeChanged {
                        field,
                        before: a,
                        after: b,
                    }),
                    FieldDelta::Wire(field, a, b) => push(EventChange::FieldWireNameChanged {
                        field,
                        before: a,
                        after: b,
                    }),
                    FieldDelta::Display(field, a, b) => {
                        push(EventChange::FieldDisplayNameChanged {
                            field,
                            before: a,
                            after: b,
                        });
                    }
                    FieldDelta::Summary(field, a, b) => push(EventChange::FieldSummaryChanged {
                        field,
                        before: a,
                        after: b,
                    }),
                    FieldDelta::Reordered(a, b) => push(EventChange::FieldOrderChanged {
                        before: a,
                        after: b,
                    }),
                });
                for delta in naming_deltas(&was.naming, &is.naming, name.local()) {
                    match delta {
                        NamingDelta::Wire(a, b) => push(EventChange::WireNameChanged {
                            before: a,
                            after: b,
                        }),
                        NamingDelta::Display(a, b) => push(EventChange::DisplayNameChanged {
                            before: a,
                            after: b,
                        }),
                        NamingDelta::Summary(a, b) => push(EventChange::SummaryChanged {
                            before: a,
                            after: b,
                        }),
                    }
                }
            }
            (None, None) => unreachable!("a key came from one of the two maps"),
        }
    }
}

/// Every difference between the two revisions' declared errors.
fn error_changes(before: &EssIr, after: &EssIr, changes: &mut Vec<SemanticChange>) {
    for name in keys(&before.errors, &after.errors) {
        let subject = ErrorRef::new(name.clone());
        let mut push = |changed: ErrorChange| {
            changes.push(SemanticChange::Error {
                subject: subject.clone(),
                changed,
            });
        };
        match (before.errors.get(name), after.errors.get(name)) {
            (None, Some(_)) => push(ErrorChange::Added),
            (Some(_), None) => push(ErrorChange::Removed),
            (Some(was), Some(is)) => {
                let (owned, owns) = (DomainRef::from(&was.domain), DomainRef::from(&is.domain));
                if owned != owns {
                    push(ErrorChange::DomainChanged {
                        before: owned,
                        after: owns,
                    });
                }
                if was.summary != is.summary {
                    push(ErrorChange::SummaryChanged {
                        before: was.summary.clone(),
                        after: is.summary.clone(),
                    });
                }
                field_deltas(&was.fields, &is.fields, |delta| match delta {
                    FieldDelta::Added(field, type_ref) => {
                        push(ErrorChange::FieldAdded { field, type_ref });
                    }
                    FieldDelta::Removed(field) => push(ErrorChange::FieldRemoved { field }),
                    FieldDelta::TypeChanged(field, a, b) => push(ErrorChange::FieldTypeChanged {
                        field,
                        before: a,
                        after: b,
                    }),
                    FieldDelta::Wire(field, a, b) => push(ErrorChange::FieldWireNameChanged {
                        field,
                        before: a,
                        after: b,
                    }),
                    FieldDelta::Display(field, a, b) => {
                        push(ErrorChange::FieldDisplayNameChanged {
                            field,
                            before: a,
                            after: b,
                        });
                    }
                    FieldDelta::Summary(field, a, b) => push(ErrorChange::FieldSummaryChanged {
                        field,
                        before: a,
                        after: b,
                    }),
                    FieldDelta::Reordered(a, b) => push(ErrorChange::FieldOrderChanged {
                        before: a,
                        after: b,
                    }),
                });
            }
            (None, None) => unreachable!("a key came from one of the two maps"),
        }
    }
}

/// Every difference between the two revisions' actors — including the two grant relations.
fn actor_changes(before: &EssIr, after: &EssIr, changes: &mut Vec<SemanticChange>) {
    for name in keys(&before.actors, &after.actors) {
        let subject = ActorRef::new(name.clone());
        let mut push = |changed: ActorChange| {
            changes.push(SemanticChange::Actor {
                subject: subject.clone(),
                changed,
            });
        };
        match (before.actors.get(name), after.actors.get(name)) {
            (None, Some(_)) => push(ActorChange::Added),
            (Some(_), None) => push(ActorChange::Removed),
            (Some(was), Some(is)) => {
                let (owned, owns) = (DomainRef::from(&was.domain), DomainRef::from(&is.domain));
                if owned != owns {
                    push(ActorChange::DomainChanged {
                        before: owned,
                        after: owns,
                    });
                }
                // The whole of the authority comparison: two name sets, and set membership decides
                // the direction. Nothing is proved, and nothing has to be.
                let granted: BTreeSet<CommandRef> = was.may.iter().map(CommandRef::from).collect();
                let grants: BTreeSet<CommandRef> = is.may.iter().map(CommandRef::from).collect();
                for command in grants.difference(&granted) {
                    push(ActorChange::GrantAdded {
                        command: command.clone(),
                    });
                }
                for command in granted.difference(&grants) {
                    push(ActorChange::GrantRemoved {
                        command: command.clone(),
                    });
                }
                for delta in naming_deltas(&was.naming, &is.naming, name.local()) {
                    match delta {
                        NamingDelta::Wire(a, b) => push(ActorChange::WireNameChanged {
                            before: a,
                            after: b,
                        }),
                        NamingDelta::Display(a, b) => push(ActorChange::DisplayNameChanged {
                            before: a,
                            after: b,
                        }),
                        NamingDelta::Summary(a, b) => push(ActorChange::SummaryChanged {
                            before: a,
                            after: b,
                        }),
                    }
                }
            }
            (None, None) => unreachable!("a key came from one of the two maps"),
        }
    }
}

/// Every difference between the two revisions' components.
fn component_changes(before: &EssIr, after: &EssIr, changes: &mut Vec<SemanticChange>) {
    for name in keys(&before.components, &after.components) {
        let subject = ComponentRef::new(name.clone());
        let mut push = |changed: ComponentChange| {
            changes.push(SemanticChange::Component {
                subject: subject.clone(),
                changed,
            });
        };
        match (before.components.get(name), after.components.get(name)) {
            (None, Some(_)) => push(ComponentChange::Added),
            (Some(_), None) => push(ComponentChange::Removed),
            (Some(was), Some(is)) => {
                let (owned, owns): (BTreeSet<DomainRef>, BTreeSet<DomainRef>) = (
                    was.owns.iter().map(DomainRef::from).collect(),
                    is.owns.iter().map(DomainRef::from).collect(),
                );
                for domain in owns.difference(&owned) {
                    push(ComponentChange::OwnsAdded {
                        domain: domain.clone(),
                    });
                }
                for domain in owned.difference(&owns) {
                    push(ComponentChange::OwnsRemoved {
                        domain: domain.clone(),
                    });
                }

                let (accepted, accepts): (BTreeSet<CommandRef>, BTreeSet<CommandRef>) = (
                    was.accepts.iter().map(CommandRef::from).collect(),
                    is.accepts.iter().map(CommandRef::from).collect(),
                );
                for command in accepts.difference(&accepted) {
                    push(ComponentChange::AcceptsAdded {
                        command: command.clone(),
                    });
                }
                for command in accepted.difference(&accepts) {
                    push(ComponentChange::AcceptsRemoved {
                        command: command.clone(),
                    });
                }

                let (was_events, is_events): (BTreeSet<EventRef>, BTreeSet<EventRef>) = (
                    was.publishes.iter().map(EventRef::from).collect(),
                    is.publishes.iter().map(EventRef::from).collect(),
                );
                for event in is_events.difference(&was_events) {
                    push(ComponentChange::PublishesAdded {
                        event: event.clone(),
                    });
                }
                for event in was_events.difference(&is_events) {
                    push(ComponentChange::PublishesRemoved {
                        event: event.clone(),
                    });
                }

                for delta in naming_deltas(&was.naming, &is.naming, name.as_str()) {
                    match delta {
                        NamingDelta::Wire(a, b) => push(ComponentChange::WireNameChanged {
                            before: a,
                            after: b,
                        }),
                        NamingDelta::Display(a, b) => push(ComponentChange::DisplayNameChanged {
                            before: a,
                            after: b,
                        }),
                        NamingDelta::Summary(a, b) => push(ComponentChange::SummaryChanged {
                            before: a,
                            after: b,
                        }),
                    }
                }
            }
            (None, None) => unreachable!("a key came from one of the two maps"),
        }
    }
}
