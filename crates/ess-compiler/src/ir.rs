//! The normalized IR: what a specification means, with every reference resolved.
//!
//! # A handle is the guarantee
//!
//! A [`Specification`](ess_domain::spec::Specification) holds [`QualifiedName`]s. Every one of them
//! *probably* names something declared, and "probably" is what this module exists to delete.
//!
//! So nothing in the IR is a bare name where a reference is meant. A [`ResolvedField`]'s type is a
//! [`ResolvedTypeRef`] whose named leaves are [`TypeHandle`]s, and a handle has **no public
//! constructor**: the only way to hold one is to have been given it by an [`EssIr`], and the only
//! way to obtain an [`EssIr`] is [`compile`](crate::resolve::compile), which is where the check
//! runs. That makes the lookups total — [`EssIr::named_type`] returns `&ResolvedType`, not
//! `Option<&ResolvedType>` — and it makes an unresolved reference *unrepresentable* rather than
//! merely absent.
//!
//! The rule the shape follows: **a question a projection will ask must have an answer in here.**
//! Where the answer would have been "re-read the source" or "look it up and hope", the field is a
//! handle or the field is not a name at all.
//!
//! | question | answered by |
//! |---|---|
//! | is this field a `List` of a named type? | [`ResolvedTypeRef`], a tree rather than rendered text |
//! | what is that named type made of? | [`EssIr::named_type`], total |
//! | which event does this outcome emit, and which branch is it? | [`ResolvedOutcome`] |
//! | what does this command's error carry? | [`EssIr::errors`], which the skeleton lacked |
//! | what is this field called on the wire? | [`ResolvedField::naming`] |
//! | which component runs how many times? | [`EssIr::workloads`] |
//!
//! # Determinism
//!
//! Every collection here is a [`BTreeMap`], a [`BTreeSet`], or a [`Vec`] in declaration order.
//! Nothing takes a clock or an RNG. Review F8's point is that those sentences are worth nothing
//! unasserted, so `tests/billing.rs` compiles the example twice and compares bytes.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use aep_domain::predicate::Predicate;
use ess_domain::binding::{BindingName, Delivery, Failure};
use ess_domain::command::{OutcomeName, TestStrategy};
use ess_domain::component::ComponentName;
use ess_domain::entity::Invariant;
use ess_domain::name::{Naming, QualifiedName, Version};
use ess_domain::topology::{Replicas, Resource};
use ess_domain::types::Primitive;

/// Declares every handle kind, its accessor on [`EssIr`], and the map it indexes — from one line
/// each, so a handle cannot exist without a total lookup for it.
///
/// The accessor is generated rather than hand-written because the two halves are the same claim: a
/// handle is only mintable by this crate, therefore the map contains it, therefore the lookup does
/// not return an `Option`. Written by hand, the day someone adds a handle and forgets the accessor
/// is the day a projection starts calling `.get()` and handling `None` — which is the state this
/// whole crate exists to leave.
macro_rules! handles {
    (
        $(
            $(#[$attribute:meta])*
            $handle:ident($inner:ty) => $accessor:ident : $resolved:ident in $field:ident,
            $what:literal;
        )*
    ) => {
        $(
            $(#[$attribute])*
            ///
            /// Mintable only by [`compile`](crate::resolve::compile). That is the mechanism, not a
            /// convention: a projection cannot construct one, so it cannot hold a reference to
            /// something the compiler did not resolve.
            #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
            #[serde(transparent)]
            pub struct $handle($inner);

            impl $handle {
                /// Records a resolved reference. Crate-private: minting is the check.
                pub(crate) fn new(name: $inner) -> Self {
                    Self(name)
                }

                /// The identity it carries.
                ///
                /// Readable, because a generator emits identities. Not constructible from one,
                /// because that is the door this type closes.
                pub fn name(&self) -> &$inner {
                    &self.0
                }
            }

            impl fmt::Display for $handle {
                fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                    write!(f, "{}", self.0)
                }
            }

            impl EssIr {
                #[doc = concat!("The ", $what, " a handle names.")]
                ///
                /// Total. It cannot answer "not found", because a handle for something absent
                /// cannot be built. The one way to reach the panic is to use a handle from one
                /// [`EssIr`] against another, which is a programming mistake and not a
                /// specification's problem.
                pub fn $accessor(&self, handle: &$handle) -> &$resolved {
                    self.$field.get(&handle.0).unwrap_or_else(|| {
                        panic!(
                            "`{handle}` is not a {} this IR declares: a handle belongs to the IR \
                             that minted it",
                            $what
                        )
                    })
                }
            }
        )*
    };
}

handles! {
    /// A named type that is declared.
    TypeHandle(QualifiedName) => named_type : ResolvedType in types, "type";
    /// A command that is declared.
    CommandHandle(QualifiedName) => command : ResolvedCommand in commands, "command";
    /// An event that is declared.
    EventHandle(QualifiedName) => event : ResolvedEvent in events, "event";
    /// An error that is declared.
    ErrorHandle(QualifiedName) => error : ResolvedError in errors, "error";
    /// A bounded context that is declared.
    DomainHandle(QualifiedName) => domain : ResolvedDomain in domains, "domain";
    /// A component that is declared.
    ComponentHandle(ComponentName) => component : ResolvedComponent in components, "component";
}

/// A type reference with every named leaf resolved.
///
/// A *tree*, where the skeleton had the rendered string `Optional<billing.invoice.Email>`. Rendering
/// is lossy in the direction that matters: a generator asking "is this a `List` of a named type"
/// would have to re-parse the text this crate had already parsed, and a second parser is a second
/// place for `Map<String, Money>` to mean something slightly different. [`Display`](fmt::Display)
/// still produces exactly that text, so nothing that wanted the rendering lost it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ResolvedTypeRef {
    /// A primitive: `String`, `Decimal`, `Timestamp`.
    Primitive {
        /// Which one.
        name: Primitive,
    },
    /// A named type declared in this specification.
    Declared {
        /// Which one, as a handle rather than a name.
        name: TypeHandle,
    },
    /// A value that may be absent.
    Optional {
        /// What it wraps.
        of: Box<ResolvedTypeRef>,
    },
    /// An ordered sequence.
    List {
        /// What it holds.
        of: Box<ResolvedTypeRef>,
    },
    /// A mapping with a primitive key.
    Map {
        /// The key.
        key: Primitive,
        /// The value.
        value: Box<ResolvedTypeRef>,
    },
}

impl ResolvedTypeRef {
    /// The named type at the root, when there is one. `None` for a primitive.
    ///
    /// Derived rather than stored — the skeleton kept it beside the rendered type as a second
    /// field, and two fields describing one fact are two fields that can disagree.
    pub fn declared(&self) -> Option<&TypeHandle> {
        match self {
            Self::Primitive { .. } => None,
            Self::Declared { name } => Some(name),
            Self::Optional { of } | Self::List { of } | Self::Map { value: of, .. } => {
                of.declared()
            }
        }
    }

    /// Every named type this reference reaches, including through `List` and `Map`.
    pub fn named_leaves(&self) -> Vec<&TypeHandle> {
        match self {
            Self::Primitive { .. } => Vec::new(),
            Self::Declared { name } => vec![name],
            Self::Optional { of } | Self::List { of } | Self::Map { value: of, .. } => {
                of.named_leaves()
            }
        }
    }

    /// `true` when a value of this type may be absent.
    pub fn is_optional(&self) -> bool {
        matches!(self, Self::Optional { .. })
    }

    /// This reference with any `Optional` wrapper removed.
    pub fn required(&self) -> &Self {
        match self {
            Self::Optional { of } => of.required(),
            other => other,
        }
    }
}

impl fmt::Display for ResolvedTypeRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Primitive { name } => write!(f, "{name}"),
            Self::Declared { name } => write!(f, "{name}"),
            Self::Optional { of } => write!(f, "Optional<{of}>"),
            Self::List { of } => write!(f, "List<{of}>"),
            Self::Map { key, value } => write!(f, "Map<{key}, {value}>"),
        }
    }
}

/// A field whose type is resolved.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ResolvedField {
    /// Its name.
    pub name: String,
    /// Its type.
    pub type_ref: ResolvedTypeRef,
    /// What it is called on the wire, and shown as.
    ///
    /// Carried because a projection emitting JSON needs the wire name, and the alternative is that
    /// every projection re-reads the source to find it.
    pub naming: Naming,
}

/// What a named type is made of, with every reference resolved.
///
/// Not [`TypeBody`](ess_domain::types::TypeBody): that holds
/// [`TypeRef`](ess_domain::types::TypeRef)s, which are names. A `ResolvedType` carrying one would
/// have been an IR node with an unresolved reference inside it, which is the exact thing this crate
/// promises does not exist.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ResolvedBody {
    /// A wrapper around one representation, distinct from it.
    Newtype {
        /// What it wraps.
        of: ResolvedTypeRef,
        /// Conditions every value satisfies, as predicates over `value`.
        invariants: Vec<Invariant>,
    },
    /// Named fields.
    Struct {
        /// Its fields, in declaration order.
        fields: Vec<ResolvedField>,
        /// Conditions every value satisfies, as predicates over those fields.
        invariants: Vec<Invariant>,
    },
    /// One of a fixed set of names.
    Enum {
        /// The variants, in declaration order.
        variants: Vec<String>,
    },
    /// One of several shapes, distinguished by a tag field.
    Union {
        /// The field carrying the variant's name.
        tag: String,
        /// The variants, by tag value.
        variants: BTreeMap<String, ResolvedTypeRef>,
    },
}

/// A type that is known to be declared.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ResolvedType {
    /// Its identity.
    pub name: QualifiedName,
    /// What it is made of.
    pub body: ResolvedBody,
    /// What it is called on the wire, and shown as.
    pub naming: Naming,
}

/// A crossing the specification permits, with both ends resolved.
///
/// In the IR as a whole set, not only where a mapping uses one: "what is this system willing to
/// treat as what, and who said so" is a question about the system, and a conversion nobody uses yet
/// is still an answer to it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ResolvedConversion {
    /// The type a value has.
    pub from: ResolvedTypeRef,
    /// The type it may be used as.
    pub to: ResolvedTypeRef,
    /// Why this crossing is allowed, as the author wrote it.
    pub because: String,
}

/// What decides that an outcome is the one taken.
///
/// A mirror of [`OutcomeCondition`](ess_domain::command::OutcomeCondition), which is deliberately
/// not serialisable in the domain crate. Same three cases, no extra meaning.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ResolvedCondition {
    /// Taken when this predicate over the command's input holds.
    When {
        /// The predicate.
        predicate: Predicate,
    },
    /// The default branch, taken when no conditional outcome matched.
    Otherwise,
    /// Decided by something outside the input.
    External {
        /// What decides it, in one phrase.
        cause: String,
    },
}

/// One thing a command can result in.
///
/// The skeleton flattened every outcome's events into one `emits` list on the command. That answers
/// "what may this command emit" and loses "on which branch" — so a generated test could not tell the
/// refusal path from the happy one, which is the distinction wave 1 restructured the model to keep
/// (review F1).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ResolvedOutcome {
    /// What this outcome is called.
    pub name: OutcomeName,
    /// What decides that this is the outcome taken.
    pub condition: ResolvedCondition,
    /// How a generated test reaches this branch.
    ///
    /// Computed once here, from the domain's own answer, so two projections cannot disagree about
    /// whether a branch is reachable by constructing an input.
    pub test_strategy: TestStrategy,
    /// The events it emits, in the order they happen.
    pub emits: Vec<EventHandle>,
    /// The error it reports, if it reports one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorHandle>,
    /// One line for generated documentation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
}

/// A command, with its input and every outcome resolved.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ResolvedCommand {
    /// Its identity.
    pub name: QualifiedName,
    /// The bounded context that owns it.
    pub domain: DomainHandle,
    /// Its input, in declaration order.
    pub input: Vec<ResolvedField>,
    /// Everything it can result in.
    pub outcomes: Vec<ResolvedOutcome>,
    /// What it is called on the wire, and shown as.
    pub naming: Naming,
}

impl ResolvedCommand {
    /// Every event any outcome emits, in outcome order.
    ///
    /// An event emitted by two outcomes appears twice: the caller asking this question is usually
    /// building a graph, and collapsing it here would hide that two branches produce the same fact.
    pub fn emits(&self) -> impl Iterator<Item = &EventHandle> {
        self.outcomes
            .iter()
            .flat_map(|outcome| outcome.emits.iter())
    }

    /// Every error any outcome names, in outcome order.
    pub fn errors(&self) -> impl Iterator<Item = &ErrorHandle> {
        self.outcomes
            .iter()
            .filter_map(|outcome| outcome.error.as_ref())
    }

    /// The input field with this name.
    pub fn input_field(&self, name: &str) -> Option<&ResolvedField> {
        self.input.iter().find(|field| field.name == name)
    }
}

/// An event, with its payload resolved.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ResolvedEvent {
    /// Its identity.
    pub name: QualifiedName,
    /// The bounded context that owns it.
    pub domain: DomainHandle,
    /// What it carries, in declaration order.
    pub fields: Vec<ResolvedField>,
    /// What it is called on the wire, and shown as.
    pub naming: Naming,
}

impl ResolvedEvent {
    /// The field with this name.
    pub fn field(&self, name: &str) -> Option<&ResolvedField> {
        self.fields.iter().find(|field| field.name == name)
    }
}

/// An error a command may report, with its payload resolved.
///
/// Absent from the skeleton, where a command's `errors` were [`QualifiedName`]s pointing at nothing
/// in the IR — so "what does this failure carry" was a question the IR could not answer, and an
/// error-response generator would have had to re-read the source.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ResolvedError {
    /// Its identity.
    pub name: QualifiedName,
    /// The bounded context that owns it.
    pub domain: DomainHandle,
    /// One line saying what went wrong, for the person who receives it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    /// What it carries beyond its name.
    pub fields: Vec<ResolvedField>,
}

/// Where a mapped value comes from, resolved.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ResolvedMappingValue {
    /// A field of the triggering event.
    EventField {
        /// The field's name.
        field: String,
        /// Its type, which had to be assignable to the target's or declared convertible to it.
        type_ref: ResolvedTypeRef,
    },
    /// A value written in the binding itself.
    ///
    /// Its type is *not* checked against the target — nothing in the model says how to read
    /// `invoice-created` as a `TemplateId` — and it is a separate variant so that a reader can see
    /// exactly which mappings the compiler verified and which it took on trust.
    Literal {
        /// The value, as written.
        value: String,
    },
}

/// One filled command input.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ResolvedMapping {
    /// The command input being filled.
    pub target: String,
    /// Its type.
    pub target_type: ResolvedTypeRef,
    /// Where the value comes from.
    pub value: ResolvedMappingValue,
    /// Why the two types are allowed to meet, when they differ.
    ///
    /// `None` when the source type is already assignable to the target. `Some` carries the declared
    /// conversion's stated reason — where the skeleton had `converted: bool`. The reason is the
    /// entire point of making a crossing a declaration: a generator emitting this mapping has to
    /// emit the conversion too, and a reader auditing the system wants to find who justified it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conversion: Option<String>,
}

/// A binding whose mapping is known to typecheck.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ResolvedBinding {
    /// Its identifier.
    pub name: BindingName,
    /// The event it reacts to.
    pub event: EventHandle,
    /// The command it invokes.
    pub command: CommandHandle,
    /// One entry per mapped command input, in the command's declaration order.
    ///
    /// The command's order, not the document's: a generator emitting a call needs the argument
    /// order the command declares, and a mapping written in a different order is the same mapping.
    pub mapping: Vec<ResolvedMapping>,
    /// How many times the command may run.
    pub delivery: Delivery,
    /// What happens when it does not.
    pub failure: Failure,
    /// What it is called on the wire, and shown as.
    pub naming: Naming,
}

/// A component: one unit of ownership, with its surface resolved.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ResolvedComponent {
    /// Its name.
    pub name: ComponentName,
    /// The domains it owns.
    pub owns: BTreeSet<DomainHandle>,
    /// The commands it accepts.
    pub accepts: BTreeSet<CommandHandle>,
    /// The events it publishes.
    pub publishes: BTreeSet<EventHandle>,
    /// What it is called on the wire, and shown as.
    pub naming: Naming,
}

/// One component's runtime requirements, with the component resolved.
///
/// Semantic, not a deployment: `replicas.min: 2` says the system is not correct with one instance.
/// Nothing in this wave generates a manifest from it — it is in the IR because "the topology names a
/// component nobody declared" is a design §20 rejection, and a rejection about something the IR does
/// not hold is a rejection nothing downstream can be held to.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ResolvedWorkload {
    /// The component this runs.
    pub component: ComponentHandle,
    /// How many instances.
    pub replicas: Replicas,
    /// Whether an instance holds state that outlives a request.
    pub stateless: bool,
    /// What it needs in order to run.
    pub requires: Vec<Resource>,
}

/// A bounded context, and what the IR holds of it.
///
/// Member lists name only kinds the IR itself holds. Entities, views and actors are absent from
/// this wave's IR, so they are absent here too rather than being listed as [`QualifiedName`]s
/// pointing at nothing — a name in the IR is exactly what this crate exists to remove.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ResolvedDomain {
    /// Its namespace — `billing.invoice`.
    pub name: QualifiedName,
    /// What it is called on the wire, and shown as.
    ///
    /// A documentation or contract projection groups by domain and titles the group with this. The
    /// skeleton
    /// held no domains at all, so that title had to come from the source.
    pub naming: Naming,
    /// The types declared inside it.
    pub types: BTreeSet<TypeHandle>,
    /// The commands it owns.
    pub commands: BTreeSet<CommandHandle>,
    /// The events it owns.
    pub events: BTreeSet<EventHandle>,
    /// The errors its commands may report.
    pub errors: BTreeSet<ErrorHandle>,
}

/// The whole specification, resolved.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct EssIr {
    /// The system's name.
    pub system: QualifiedName,
    /// Its version.
    pub version: Version,
    /// What it is called on the wire, and shown as.
    pub naming: Naming,
    /// What the system is, in one paragraph.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    /// Every bounded context, by name.
    pub domains: BTreeMap<QualifiedName, ResolvedDomain>,
    /// Every type, by name.
    pub types: BTreeMap<QualifiedName, ResolvedType>,
    /// Every crossing the specification permits.
    pub conversions: Vec<ResolvedConversion>,
    /// Every command, by name.
    pub commands: BTreeMap<QualifiedName, ResolvedCommand>,
    /// Every event, by name.
    pub events: BTreeMap<QualifiedName, ResolvedEvent>,
    /// Every error, by name.
    pub errors: BTreeMap<QualifiedName, ResolvedError>,
    /// Every binding, by name.
    pub bindings: BTreeMap<BindingName, ResolvedBinding>,
    /// Every component, by name.
    pub components: BTreeMap<ComponentName, ResolvedComponent>,
    /// The runtime shape: one entry per component that runs.
    pub workloads: BTreeMap<ComponentName, ResolvedWorkload>,
}

impl EssIr {
    /// Every event that causes a command, and the bindings that make it so.
    ///
    /// On the IR rather than computed by each consumer because it is the graph the whole interaction
    /// layer is about, and two consumers computing it separately would disagree the first time a
    /// second binding reacted to one event.
    pub fn reactions(&self) -> BTreeMap<&EventHandle, Vec<&ResolvedBinding>> {
        let mut out: BTreeMap<&EventHandle, Vec<&ResolvedBinding>> = BTreeMap::new();
        for binding in self.bindings.values() {
            out.entry(&binding.event).or_default().push(binding);
        }
        out
    }

    /// The IR as canonical JSON, with a trailing newline.
    ///
    /// Canonical means: key order comes from [`BTreeMap`], so it is the same on every machine and in
    /// every run; the indentation is `serde_json`'s two spaces; and the last byte is a newline,
    /// because a file without one is a file that shows up as modified in the next diff. This is the
    /// artifact review F8 asks be compared byte-for-byte, and `tests/billing.rs` compares it.
    ///
    /// Serialisation cannot fail: every map key here serialises as a string and no float is
    /// involved.
    pub fn to_canonical_json(&self) -> String {
        let mut json = serde_json::to_string_pretty(self)
            .unwrap_or_else(|error| panic!("the IR serialises: {error}"));
        json.push('\n');
        json
    }
}
