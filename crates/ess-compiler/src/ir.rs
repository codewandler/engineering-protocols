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
//! | which states can this entity move between, and by which named move? | [`ResolvedEntity::lifecycle`] |
//! | does a generated scenario assert this view with `expect` or `eventually`? | [`ResolvedView::assertion_style`] |
//! | which commands may this actor invoke? | [`ResolvedActor::may`] |
//! | which entity does this outcome change, and how? | [`ResolvedOutcome::subject`] |
//! | which command outcome takes this transition? | [`EssIr::drivers`] |
//! | in which states does this command refuse to act at all? | [`EssIr::wrong_states`] |
//! | what does this binding publish when it escalates? | [`ResolvedBinding::escalation`] |
//!
//! # A state is a variant, not a handle
//!
//! [`StateName`] stays a name inside [`ResolvedEntity::lifecycle`],
//! and that is not an exception to the rule above. A handle exists because a reference can point
//! outside the declaration that writes it; a lifecycle's states are declared *by that same
//! lifecycle*, so [`StateMachine::states`] is the whole answer and it travels in the same struct.
//! Minting a `StateHandle` would have meant inventing a qualified name for
//! `billing.invoice.Invoice.State.Draft`, which the model never spells, and an `EssIr` map keyed by
//! it. What a projection actually asks — "what type do I emit for this entity's state" — is
//! [`ResolvedEntity::state_type`], which *is* a handle, because the enum it names is a declaration
//! of its own.
//!
//! # A predicate travels parsed, not resolved
//!
//! [`ResolvedView::filter`] and [`ResolvedEntity::invariants`] carry
//! [`Predicate`] trees — the same shape
//! [`ResolvedCondition::When`] and [`ResolvedBody::Struct`] already carry, and the same one
//! `ess-domain` parsed. So nothing downstream re-parses a filter, and
//! [`Invariant`] keeps the author's own spelling beside the tree for a
//! diagnostic to quote.
//!
//! What a predicate does *not* carry is a handle per leaf. `state == Issued` reads the fact path
//! `state`, and that stays a path rather than becoming a reference to
//! [`ResolvedEntity::state_type`]'s variant. The cost is real and worth naming: a generator that
//! wants the type behind a filter's root matches the path's first segment against
//! [`ResolvedEntity::observable_fields`] by name. `ess-domain` guarantees that match succeeds — it
//! refuses a filter reading something the source does not have — but the IR does not make the
//! failure unrepresentable, so that one lookup returns an `Option`. Buying it back means a mirror of
//! ten `Predicate` variants, `Operand` and `FactValue`, plus a rule for the deep paths (`total.amount`
//! walking into a struct) that `ess-domain` deliberately leaves open — a new rejection class in a
//! pass whose job is to resolve, not to grow rules.
//!
//! # Determinism
//!
//! Every collection here is a [`BTreeMap`], a [`BTreeSet`], or a [`Vec`] in declaration order.
//! Nothing takes a clock or an RNG. Review F8's point is that those sentences are worth nothing
//! unasserted, so `tests/billing.rs` compiles the example twice and compares bytes.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use aep_domain::facts::FactPath;
use aep_domain::predicate::Predicate;
use ess_domain::binding::{BindingName, Delivery, Failure};
use ess_domain::command::{OutcomeName, TestStrategy};
use ess_domain::component::ComponentName;
use ess_domain::entity::{EntitySpec, Invariant, StateMachine, StateName, Transition};
use ess_domain::name::{Naming, QualifiedName, Version};
use ess_domain::topology::{Replicas, Resource};
use ess_domain::types::Primitive;
use ess_domain::view::{AssertionStyle, Consistency};

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
    /// An entity that is declared.
    EntityHandle(QualifiedName) => entity : ResolvedEntity in entities, "entity";
    /// A command that is declared.
    CommandHandle(QualifiedName) => command : ResolvedCommand in commands, "command";
    /// An event that is declared.
    EventHandle(QualifiedName) => event : ResolvedEvent in events, "event";
    /// An error that is declared.
    ErrorHandle(QualifiedName) => error : ResolvedError in errors, "error";
    /// A view that is declared.
    ViewHandle(QualifiedName) => view : ResolvedView in views, "view";
    /// An actor that is declared.
    ActorHandle(QualifiedName) => actor : ResolvedActor in actors, "actor";
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
///
/// # Depth
///
/// At most [`MAX_TYPE_DEPTH`](ess_domain::types::MAX_TYPE_DEPTH). The only way to obtain one is
/// `Resolver::type_ref`, which maps one [`TypeRef`](ess_domain::types::TypeRef) constructor to one
/// of these and so preserves depth exactly, and `spec_type_ref` maps it back the same way; a
/// `TypeRef` in turn can only come from a parser that refuses past 32. That is what lets
/// [`Self::declared`], [`Self::named_leaves`], [`Self::required`], [`Display`](fmt::Display) and the
/// projection walkers in `ess-gen` recurse without counting. Bounding them again would put a second
/// number beside the first with nothing keeping the two in step, and a limit that can only ever fire
/// on a value this crate built itself is a limit that can only ever be wrong.
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

/// An entity: something with stable identity, with its fields and its lifecycle resolved.
///
/// The skeleton kept only the *state set*, as the synthesised enum. That is what a state diagram
/// with no arrows is made of: the transitions, the initial state, the terminal states, the
/// invariants and the identity's name were all gone, and a documentation projection had to warn its
/// readers not to read the empty diagram as a claim about what the model permits.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ResolvedEntity {
    /// Its identity — `billing.invoice.Invoice`.
    pub name: QualifiedName,
    /// The bounded context that owns it.
    pub domain: DomainHandle,
    /// How an instance is identified: the field's **name** as well as its type.
    ///
    /// The name is load-bearing, and carrying it was a wave-1 decision rather than a convenience. A
    /// view projects `invoice_id`; without the name in the model, every projection invents one, and
    /// the same field is `id` in the contract, `invoice_id` in the specification and something else
    /// again in the generated code.
    pub identity: ResolvedField,
    /// What it holds, in declaration order.
    pub fields: Vec<ResolvedField>,
    /// The enum its lifecycle forms — `billing.invoice.Invoice.State`.
    ///
    /// A handle, so the states a projection emits and the states a filter compares are the same
    /// declaration. Synthesised by [`EntitySpec::state_type`] rather than written by an author, which
    /// is why it is derived from the lifecycle and cannot disagree with it; the variants of
    /// [`EssIr::named_type`] on this handle are [`StateMachine::states`], rendered.
    pub state_type: TypeHandle,
    /// Its lifecycle: every state, where an instance starts, where it may rest, and every named
    /// move.
    ///
    /// `ess-domain`'s own [`StateMachine`], not a copy of it. It holds no reference that points
    /// outside itself — a transition's `from` and `to` name states this same value declares — so the
    /// questions a projection asks ([`StateMachine::outgoing`], [`StateMachine::can_move`]) are
    /// already answered here, by the code that owns the rule that a move nobody declared is a move
    /// nobody may make.
    pub lifecycle: StateMachine,
    /// What must hold of every instance, at rest, as predicates over its fields.
    pub invariants: Vec<Invariant>,
    /// What it is called on the wire, and shown as.
    pub naming: Naming,
}

impl ResolvedEntity {
    /// The declared field with this name. The identity is not one; it is [`ResolvedEntity::identity`].
    pub fn field(&self, name: &str) -> Option<&ResolvedField> {
        self.fields.iter().find(|field| field.name == name)
    }

    /// The `state` pseudo-field, typed as [`ResolvedEntity::state_type`].
    ///
    /// Derived, and it stays derived: a stored copy would be a second declaration of the one thing
    /// the lifecycle already says. The name is [`EntitySpec::STATE`], so the specification, the
    /// domain crate and every projection spell it the same way.
    pub fn state_field(&self) -> ResolvedField {
        ResolvedField {
            name: EntitySpec::STATE.to_owned(),
            type_ref: ResolvedTypeRef::Declared {
                name: self.state_type.clone(),
            },
            naming: Naming::default(),
        }
    }

    /// What a view may project or filter on: the identity, the declared fields, and the state.
    ///
    /// The same surface [`EntitySpec::observable_fields`] defines, in the same order — because it is
    /// the surface `ess-domain` validated a view against, and a projection computing a different one
    /// would accept a view the compiler refused or refuse one it accepted.
    pub fn observable_fields(&self) -> Vec<ResolvedField> {
        let mut observable = Vec::with_capacity(self.fields.len() + 2);
        observable.push(self.identity.clone());
        observable.extend(self.fields.iter().cloned());
        observable.push(self.state_field());
        observable
    }

    /// The observable field a name refers to, including the identity and `state`.
    ///
    /// This is the lookup a filter's or an invariant's fact path needs, and it returns an `Option`
    /// because a predicate carries paths rather than handles — the one place in this IR where a
    /// reference is a name. `ess-domain` refuses a predicate reading something absent, so `None`
    /// means a `Specification` nothing validated.
    pub fn observable_field(&self, name: &str) -> Option<ResolvedField> {
        self.observable_fields()
            .into_iter()
            .find(|field| field.name == name)
    }

    /// Every named type this entity reaches, through its identity and its fields.
    ///
    /// [`ResolvedEntity::state_type`] is deliberately not in it: this answers "which declared types
    /// does an author's entity use", and the state enum is one this compiler synthesised.
    pub fn named_leaves(&self) -> Vec<&TypeHandle> {
        let mut leaves = self.identity.type_ref.named_leaves();
        for field in &self.fields {
            leaves.extend(field.type_ref.named_leaves());
        }
        leaves
    }

    /// Every fact an invariant reads, in declaration order.
    pub fn invariant_reads(&self) -> Vec<&FactPath> {
        self.invariants
            .iter()
            .flat_map(|invariant| invariant.predicate.fact_paths())
            .collect()
    }
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
/// not serialisable in the domain crate. Same four cases, no extra meaning.
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
    /// Taken when the subject is resting in a state none of this command's moves start from.
    ///
    /// Carries nothing, like its mirror, and *which* states those are is not a lookup a consumer has
    /// to perform either: [`EssIr::wrong_states`] answers it once for the whole workspace, from the
    /// lifecycles the transitions already declare.
    WrongState,
}

/// What one outcome does to the entity it acts on, resolved.
///
/// A mirror of [`Effect`](ess_domain::command::Effect) with one difference, and the difference is
/// the point: [`Moves`](ResolvedEffect::Moves) carries the [`Transition`] **itself** rather than its
/// name. That follows this module's own rule — *a question a projection will ask must have an answer
/// in here*, and where the answer would have been a lookup, the field is a handle or the field is
/// not a name at all. A transition is declared inside an entity's lifecycle and has no map on
/// [`EssIr`] to be keyed in, so there is no handle to mint for one; resolving it to the value is the
/// same guarantee reached the other way, and a projection asking "which states does this move go
/// between" has `from` and `to` in hand rather than an `Option`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "effect", rename_all = "snake_case")]
pub enum ResolvedEffect {
    /// A new instance comes into existence, at its lifecycle's initial state.
    ///
    /// Which state that is comes from [`ResolvedEntity::lifecycle`], not from a copy here: the
    /// lifecycle already declares it, and a second copy is a second thing to keep in step.
    Creates,
    /// An existing instance moves along this declared transition.
    Moves {
        /// The move, as the entity's lifecycle declares it.
        transition: Transition,
    },
    /// An existing instance changes without moving along its lifecycle.
    Updates,
}

impl ResolvedEffect {
    /// The transition this effect takes, when it takes one.
    pub fn transition(&self) -> Option<&Transition> {
        match self {
            Self::Moves { transition } => Some(transition),
            Self::Creates | Self::Updates => None,
        }
    }

    /// How it reads in a sentence: `creates`, `moves`, `updates`.
    pub fn verb(&self) -> &'static str {
        match self {
            Self::Creates => "creates",
            Self::Moves { .. } => "moves",
            Self::Updates => "updates",
        }
    }
}

/// Where a scenario reads the identity of the instance an outcome acts on, resolved.
///
/// The mirror of [`Subject::instance`](ess_domain::command::Subject::instance) with the lookup
/// already done, and it is two variants rather than one field for the reason this module resolves
/// anything: a consumer must not have to re-derive which surface the name belongs to, and a
/// `creates:` link additionally needs the [`EventHandle`] of the event that publishes the identity —
/// which is a lookup, and lookups are what this crate exists to have already performed.
///
/// The field is carried as a whole [`ResolvedField`], so a projection has the wire name and the type
/// in hand: a generated scenario sends the wire name, and the type is the entity's identity type by
/// construction, because `ess-domain` refuses the link otherwise.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "from", rename_all = "snake_case")]
pub enum ResolvedInstance {
    /// The caller names the instance in this field of the command's input — `moves:`, `updates:`.
    Supplied {
        /// The input field carrying the identity.
        field: ResolvedField,
    },
    /// The new instance's identity is published in this field of this emitted event — `creates:`.
    Observed {
        /// The event that carries it.
        event: EventHandle,
        /// Its field.
        field: ResolvedField,
    },
}

impl ResolvedInstance {
    /// The field carrying the identity, whichever surface it is on.
    pub fn field(&self) -> &ResolvedField {
        match self {
            Self::Supplied { field } | Self::Observed { event: _, field } => field,
        }
    }

    /// The event publishing the identity, where the identity is observed rather than supplied.
    pub fn event(&self) -> Option<&EventHandle> {
        match self {
            Self::Supplied { .. } => None,
            Self::Observed { event, .. } => Some(event),
        }
    }
}

/// The entity an outcome acts on, what it does to it, and which instance.
///
/// Design §19's "subject and verb", resolved: the subject is an [`EntityHandle`], so
/// [`EssIr::entity`] answers *whose* invariants a §20 scenario evaluates after this branch, and the
/// verb is a [`ResolvedEffect`], so a §19 scenario knows which states to move between.
///
/// [`ResolvedSubject::instance`] is the third part, and without it the first two are not enough to
/// write a scenario with: "a `PayInvoice` moves an invoice from `Issued` to `Paid`" does not say
/// *which* invoice, and a synthesised id would be a test that fails a correct implementation.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ResolvedSubject {
    /// The entity this outcome acts on.
    pub entity: EntityHandle,
    /// What it does to it.
    #[serde(flatten)]
    pub effect: ResolvedEffect,
    /// Where the identity of the instance it acts on is read.
    pub instance: ResolvedInstance,
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
    /// The entity this outcome acts on, and what it does to it.
    ///
    /// `None` when the branch changes no entity — `billing.email.SendEmail` does not, and neither
    /// does any refusal. The other direction is total: a transition no outcome takes is refused by
    /// `ess-domain`, so every arrow in a lifecycle diagram has at least one outcome here that draws
    /// it. [`EssIr::drivers`] is that relation, from the entity's side.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject: Option<ResolvedSubject>,
    /// How a generated test reaches this branch.
    ///
    /// Computed once here, from the domain's own answer, so two projections cannot disagree about
    /// whether a branch is reachable by constructing an input.
    pub test_strategy: TestStrategy,
    /// The events it emits, in the order they happen.
    pub emits: Vec<EventHandle>,
    /// Which of those events' payload fields this branch determines, and from what.
    ///
    /// One entry per emitted event the specification says anything about, in event name order.
    /// Empty is the common case and a statement, not a gap: every payload field is then the
    /// implementation's to choose, and a suite may assert presence and type but never a value.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub payload: Vec<ResolvedPayload>,
    /// The error it reports, if it reports one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorHandle>,
    /// One line for generated documentation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
}

/// Where a determined payload field's value comes from, resolved.
///
/// The mirror of [`ResolvedMappingValue`], one construct over: a binding fills a command's input
/// from the triggering event, an outcome fills an emitted event's payload from the command's input,
/// and in both the reference variant carries the source's type because the check that admitted it
/// needed the type in hand.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ResolvedPayloadValue {
    /// A field of the command's input.
    InputField {
        /// The field's name.
        field: String,
        /// Its type, which had to be assignable to the event field's or declared convertible.
        type_ref: ResolvedTypeRef,
    },
    /// A value written in the outcome itself.
    ///
    /// Taken on trust past `ess-domain`'s representation check, exactly as a binding's literal is —
    /// and a separate variant for the same reason: a reader can see which fields were verified.
    Literal {
        /// The value, as written.
        value: String,
    },
}

/// One event field an outcome determines, with both ends resolved.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ResolvedPayloadField {
    /// The event field being filled.
    pub target: String,
    /// Its type.
    pub target_type: ResolvedTypeRef,
    /// Where the value comes from.
    pub value: ResolvedPayloadValue,
    /// Why the two types are allowed to meet, when they differ.
    ///
    /// `None` when the source type is already assignable — the same reading
    /// [`ResolvedMapping::conversion`] gives the field.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conversion: Option<String>,
}

/// One emitted event's determined payload fields.
///
/// Only the fields some declaration determines are here; an event field absent from `fields` is
/// **undetermined**, which is a fact about the specification a consumer may report and must not
/// treat as a defect — `ess_domain::command::PayloadSource` carries the argument.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ResolvedPayload {
    /// The event whose payload is being described.
    pub event: EventHandle,
    /// The determined fields, in the event's declaration order.
    pub fields: Vec<ResolvedPayloadField>,
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

/// A view: what the outside world is promised it can observe, and how soon.
///
/// Absent from the skeleton, which is why nothing downstream could see a view's source, the fields
/// it projects, its filter — or its [`Consistency`], the field the whole construct exists to carry.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ResolvedView {
    /// Its identity.
    pub name: QualifiedName,
    /// The bounded context that owns it.
    pub domain: DomainHandle,
    /// The entity it projects, as a handle.
    ///
    /// So "what is this a view *of*" is answered by [`EssIr::entity`] rather than by a name a
    /// projection has to look up and hope about.
    pub source: EntityHandle,
    /// What it exposes, in declaration order.
    ///
    /// The view's own declaration of each field, not the entity's. They are checked to agree by
    /// `ess-domain` — a projection that widens a type is a promise the entity cannot keep — and this
    /// is the side a contract is generated from.
    pub fields: Vec<ResolvedField>,
    /// Which instances it contains, as a parsed predicate. `None` means all of them.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filter: Option<Predicate>,
    /// How soon it reflects a command that has already returned.
    pub consistency: Consistency,
    /// The block a generated scenario must assert this view in.
    ///
    /// Stored, computed once from [`ViewSpec::assertion_style`](ess_domain::view::ViewSpec::assertion_style),
    /// for the reason [`ResolvedOutcome::test_strategy`] is: it is a decision, and a decision made
    /// per projection is a decision made wrong eventually. Asserting an `eventual` view with
    /// `expect` races the projection, and the repair everyone reaches for is a sleep — which makes
    /// the suite a test of the machine it runs on.
    pub assertion_style: AssertionStyle,
    /// What it is called on the wire, and shown as.
    pub naming: Naming,
}

impl ResolvedView {
    /// The projected field with this name.
    pub fn field(&self, name: &str) -> Option<&ResolvedField> {
        self.fields.iter().find(|field| field.name == name)
    }

    /// Every fact the filter reads, or nothing when there is no filter.
    ///
    /// The roots of these paths are observable fields of [`ResolvedView::source`]; resolve one with
    /// [`ResolvedEntity::observable_field`].
    pub fn filter_reads(&self) -> Vec<&FactPath> {
        self.filter
            .as_ref()
            .map(Predicate::fact_paths)
            .unwrap_or_default()
    }
}

/// An actor: who may ask the system for what, with every grant resolved.
///
/// Absent from the skeleton, which is why an interface generator had no source for a security
/// scheme: `may` *is* that source, and it was not in the IR to read.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ResolvedActor {
    /// Its identity.
    pub name: QualifiedName,
    /// The bounded context that owns it.
    pub domain: DomainHandle,
    /// The commands it may invoke.
    ///
    /// Handles, so a grant naming a command nobody declares cannot be in here — which is the one
    /// failure worth catching about a grant, because it reads as an authorization decision and
    /// authorizes nothing.
    ///
    /// A set, ordered by name: the same grant written twice means the same thing, and anything
    /// generated from it has to be diffable.
    pub may: BTreeSet<CommandHandle>,
    /// What it is called on the wire, and shown as.
    pub naming: Naming,
}

impl ResolvedActor {
    /// `true` when this actor may invoke `command`.
    pub fn may_invoke(&self, command: &CommandHandle) -> bool {
        self.may.contains(command)
    }
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
    /// The event an escalation publishes, resolved.
    ///
    /// `Some` exactly when [`Self::failure`] is [`Failure::Escalate`] — `ess-domain` refuses either
    /// half without the other — so a projection reading `escalate` off the word always has an event
    /// to name, and never has to write "the specification does not say".
    ///
    /// A handle, like every other reference in here, because that is the guarantee: a binding
    /// cannot escalate into an event nobody declares, so [`EssIr::event`] answers what a scenario
    /// asserts and what shape the assertion has.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub escalation: Option<EventHandle>,
    /// What it is called on the wire, and shown as.
    pub naming: Naming,
}

/// What happens when a binding's command does not run, with whatever that publishes.
///
/// [`ResolvedBinding::failure`] is the word and [`ResolvedBinding::escalation`] is the event, in
/// that shape because the word is what a document writes and what every projection prints. This
/// pairs the two, and it exists so that a projection *rendering* an escalation cannot forget to
/// name the event: there is no `Option` to unwrap and no arm to leave out. `ess-gen`'s
/// documentation projection matches it with no wildcard arm, so a fourth policy stops that file
/// compiling rather than being silently dropped from a page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolvedFailure<'a> {
    /// Try again, on whatever schedule the transport provides.
    ///
    /// Publishes nothing, and needs to publish nothing: a retry is another invocation of the
    /// command, which [`Delivery::AtLeastOnce`] already obliges the handler to survive.
    Retry,
    /// Surface it to a person, and publish this event to say so.
    Escalate {
        /// The event the escalation emits.
        emits: &'a EventHandle,
    },
    /// Give up silently.
    ///
    /// Publishes nothing, deliberately: an event here would make it a notification, which is a
    /// different decision with a different word.
    Drop,
}

impl ResolvedBinding {
    /// What happens when the command does not run, paired with what it publishes.
    ///
    /// Total. An `escalate` that names no event is refused by `ess-domain`, and
    /// [`compile`](crate::resolve::compile) keeps a binding whose escalation did not resolve out of
    /// the IR rather than building one that says it escalates and cannot say into what — so the one
    /// way to reach the panic is to assemble a [`ResolvedBinding`] by hand out of step with itself,
    /// which is a programming mistake and not a specification's problem. The same reasoning, and
    /// the same wording, as the handle accessors above.
    pub fn on_failure(&self) -> ResolvedFailure<'_> {
        match self.failure {
            Failure::Retry => ResolvedFailure::Retry,
            Failure::Drop => ResolvedFailure::Drop,
            Failure::Escalate => {
                let Some(emits) = &self.escalation else {
                    panic!(
                        "binding `{}` escalates and names no event: `ess-domain` refuses that, so \
                         this `ResolvedBinding` did not come from `compile`",
                        self.name
                    )
                };
                ResolvedFailure::Escalate { emits }
            }
        }
    }
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

/// A bounded context, and what it owns.
///
/// Every member list is handles, never names: "who owns this" has to be answerable, and a roster of
/// [`QualifiedName`]s pointing at nothing is exactly what this crate exists to remove.
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
    ///
    /// Including the enum each of its entities' lifecycles forms, which this compiler synthesises.
    pub types: BTreeSet<TypeHandle>,
    /// The entities it owns.
    pub entities: BTreeSet<EntityHandle>,
    /// The commands it owns.
    pub commands: BTreeSet<CommandHandle>,
    /// The events it owns.
    pub events: BTreeSet<EventHandle>,
    /// The errors its commands may report.
    pub errors: BTreeSet<ErrorHandle>,
    /// The views it publishes.
    pub views: BTreeSet<ViewHandle>,
    /// The actors it declares.
    pub actors: BTreeSet<ActorHandle>,
}

/// One command outcome that changes an entity, seen from the entity's side.
///
/// What [`EssIr::drivers`] hands back. Borrowed rather than owned because every part of it is
/// already in the IR: this is an index, not a second copy of the model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Driver<'a> {
    /// The command whose outcome this is.
    pub command: &'a ResolvedCommand,
    /// The branch that does it. `None` of a command's other branches need do the same thing.
    pub outcome: &'a ResolvedOutcome,
    /// What it does to the entity.
    pub effect: &'a ResolvedEffect,
}

impl Driver<'_> {
    /// `true` when this driver takes the transition named `transition`.
    pub fn takes(&self, transition: &str) -> bool {
        self.effect
            .transition()
            .is_some_and(|declared| declared.name == transition)
    }
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
    /// Every entity, by name.
    pub entities: BTreeMap<QualifiedName, ResolvedEntity>,
    /// Every command, by name.
    pub commands: BTreeMap<QualifiedName, ResolvedCommand>,
    /// Every event, by name.
    pub events: BTreeMap<QualifiedName, ResolvedEvent>,
    /// Every error, by name.
    pub errors: BTreeMap<QualifiedName, ResolvedError>,
    /// Every view, by name.
    pub views: BTreeMap<QualifiedName, ResolvedView>,
    /// Every actor, by name.
    pub actors: BTreeMap<QualifiedName, ResolvedActor>,
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

    /// Every view of each entity, by the entity it projects.
    ///
    /// On the IR rather than computed per consumer for the same reason [`EssIr::reactions`] is: two
    /// consumers computing it separately disagree the first time a second view projects one entity.
    pub fn projections(&self) -> BTreeMap<&EntityHandle, Vec<&ResolvedView>> {
        let mut out: BTreeMap<&EntityHandle, Vec<&ResolvedView>> = BTreeMap::new();
        for view in self.views.values() {
            out.entry(&view.source).or_default().push(view);
        }
        out
    }

    /// Every command outcome that changes each entity, by the entity it changes.
    ///
    /// The link written the way a *reader of a lifecycle* asks about it. An outcome names the entity
    /// it acts on, which is the direction an author writes and a scenario generator executes; a
    /// state diagram needs the reverse — for this arrow, which command draws it — and computing that
    /// per projection is how two of them would come to disagree about who issues an invoice.
    ///
    /// Every declared transition appears in some [`Driver`] here, because `ess-domain` refuses a
    /// transition no outcome takes. An entity that nothing changes at all is simply absent from the
    /// map.
    pub fn drivers(&self) -> BTreeMap<&EntityHandle, Vec<Driver<'_>>> {
        let mut out: BTreeMap<&EntityHandle, Vec<Driver<'_>>> = BTreeMap::new();
        for command in self.commands.values() {
            for outcome in &command.outcomes {
                if let Some(subject) = &outcome.subject {
                    out.entry(&subject.entity).or_default().push(Driver {
                        command,
                        outcome,
                        effect: &subject.effect,
                    });
                }
            }
        }
        out
    }

    /// The states each entity a command moves may be in for that command to refuse it (§19).
    ///
    /// The other half of [`EssIr::drivers`], and the reason
    /// [`ResolvedCondition::WrongState`] needs no fields: an outcome's `moves:` names a
    /// [`Transition`], a transition declares the states it may start from, and everything else the
    /// entity declares is therefore a state this command will not act from. Nobody writes that set
    /// down and nobody may, because a written copy of an absence is a copy that drifts —
    /// [`StateMachine::wrong_states`](ess_domain::entity::StateMachine::wrong_states) is the one
    /// place the subtraction happens.
    ///
    /// Here rather than in each consumer for the reason [`EssIr::drivers`] is: the synthesizer needs
    /// it to know which illegal-move scenarios exist, and the documentation projection needs it to
    /// say on the page which states a refusal branch answers in. Two of them computing it separately
    /// is two answers to one question the first time a transition gains a second `from`.
    ///
    /// A command that moves nothing is absent from the map, and so is an entity whose every state
    /// some move of this command starts from. Both are the shapes `validate_lifecycle_causes`
    /// refuses to let a `wrong_state:` branch be declared against.
    pub fn wrong_states<'a>(
        &'a self,
        command: &'a ResolvedCommand,
    ) -> BTreeMap<&'a EntityHandle, BTreeSet<&'a StateName>> {
        let mut moves: BTreeMap<&EntityHandle, BTreeSet<&str>> = BTreeMap::new();
        for outcome in &command.outcomes {
            if let Some(subject) = &outcome.subject {
                if let Some(transition) = subject.effect.transition() {
                    moves
                        .entry(&subject.entity)
                        .or_default()
                        .insert(transition.name.as_str());
                }
            }
        }

        let mut out: BTreeMap<&EntityHandle, BTreeSet<&StateName>> = BTreeMap::new();
        for (handle, taken) in moves {
            let wrong = self.entity(handle).lifecycle.wrong_states(taken);
            if !wrong.is_empty() {
                out.insert(handle, wrong);
            }
        }
        out
    }

    /// Every actor that may invoke each command, by command.
    ///
    /// The map an interface generator needs to emit a security requirement per operation, which is
    /// the direction `may` is not written in: an actor lists its commands, and the question asked of
    /// the IR is the other way round.
    pub fn grants(&self) -> BTreeMap<&CommandHandle, Vec<&ResolvedActor>> {
        let mut out: BTreeMap<&CommandHandle, Vec<&ResolvedActor>> = BTreeMap::new();
        for actor in self.actors.values() {
            for command in &actor.may {
                out.entry(command).or_default().push(actor);
            }
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
    /// Serialisation cannot fail. `serde_json` has exactly one error of its own — a map key that is
    /// not a string — and every map in this tree is keyed by [`QualifiedName`], [`BindingName`] or
    /// [`ComponentName`], each of which serialises as one. A float is *not* the second: the earlier
    /// claim that none is involved was wrong — `{amount: {any_of: [1.5]}}` in a guard reaches here
    /// as a number — but `serde_json` writes a float rather than refusing one.
    ///
    /// What it does refuse to write faithfully is a non-finite float, which it emits as `null`
    /// instead. `1e400` in a predicate literal parses to `+inf`
    /// ([`FactValue::parse_literal`](aep_domain::facts::FactValue::parse_literal) rejects only NaN),
    /// so that guard is published as `any_of: [null]`. That is a defect in what the model accepts,
    /// not in this function, and it is filed as such: the fix belongs in `aep-domain`'s `Number`,
    /// which promises a total order it does not enforce on input.
    pub fn to_canonical_json(&self) -> String {
        let mut json = serde_json::to_string_pretty(self)
            .unwrap_or_else(|error| panic!("the IR serialises: {error}"));
        json.push('\n');
        json
    }
}
