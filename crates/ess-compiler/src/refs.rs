//! Stable semantic names for every construct a specification declares.
//!
//! Wave 4 minted these inside `ess-conformance`, because a committed suite was the first document
//! that had to name a construct *outside* the process that compiled it. Wave 7 moves them here —
//! one crate down, beside the IR whose handles they are minted from — because they stopped being a
//! conformance vocabulary the day the dependency graph and the generated artifacts started using
//! them too: a suite records what a scenario depends on, a projection now records what an artifact
//! derives from, and those two sentences have to be about the same values. `ess-conformance`
//! re-exports everything from its `scenario` module, so a suite's spelling of a name did not move.
//!
//! # Every reference is a name, never a handle
//!
//! An [`ir::EssIr`](crate::ir::EssIr) handle is valid only inside the IR that minted it; using one
//! against a different IR panics by design. A name resolves against any compilation of the same
//! specification, which is what lets a committed document outlive the process that wrote it.
//! Minting a name *from* a handle is the one-way door: `From<&CommandHandle>` and its siblings take
//! a handle in and give a name out, and nothing carries the handle onward.

use std::fmt;
use std::str::FromStr;

use aep_domain::error::ParseError;
use ess_domain::binding::BindingName;
use ess_domain::command::OutcomeName;
use ess_domain::component::ComponentName;
use ess_domain::name::QualifiedName;

use crate::ir::{
    ActorHandle, CommandHandle, ComponentHandle, DomainHandle, EntityHandle, ErrorHandle,
    EventHandle, TypeHandle, ViewHandle,
};

/// Declares one semantic reference per line: the name it wraps, and the handle it is minted from.
///
/// Generated rather than written out eleven times, for the reason the compiler generates its handle
/// accessors: the parts are one claim, and hand-written copies drift one at a time. Each reference
/// serialises as the name itself, and **parses** from it — which is what makes a suite readable in a
/// process that has no [`EssIr`].
macro_rules! semantic_refs {
    (
        $(
            $(#[$attribute:meta])*
            $reference:ident($inner:ty) from $handle:ty, $what:literal;
        )*
    ) => {
        $(
            $(#[$attribute])*
            ///
            /// A stable ESS name, never a handle: it resolves against any compilation of the same
            /// specification, where a handle is valid only inside the one that minted it.
            #[derive(
                Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize,
            )]
            #[serde(transparent)]
            pub struct $reference($inner);

            impl $reference {
                #[doc = concat!("Names ", $what, ".")]
                pub fn new(name: $inner) -> Self {
                    Self(name)
                }

                /// The name it carries.
                pub fn name(&self) -> &$inner {
                    &self.0
                }
            }

            impl fmt::Display for $reference {
                fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                    write!(f, "{}", self.0)
                }
            }

            impl From<&$handle> for $reference {
                /// Mints a name from a resolved handle: the one-way door.
                ///
                /// A handle goes in and a name comes out, so a generator holding resolved references
                /// can record them in a suite that outlives the IR — and nothing carries the handle
                /// onward, because there is no field here to put one in.
                fn from(handle: &$handle) -> Self {
                    Self(handle.name().clone())
                }
            }

            impl std::str::FromStr for $reference {
                type Err = ParseError;

                fn from_str(value: &str) -> Result<Self, Self::Err> {
                    <$inner>::new(value).map(Self)
                }
            }

            impl<'de> serde::Deserialize<'de> for $reference {
                fn deserialize<D: serde::Deserializer<'de>>(
                    deserializer: D,
                ) -> Result<Self, D::Error> {
                    let raw = String::deserialize(deserializer)?;
                    <$inner>::new(raw).map(Self).map_err(serde::de::Error::custom)
                }
            }
        )*
    };
}

semantic_refs! {
    /// A bounded context.
    DomainRef(QualifiedName) from DomainHandle, "a bounded context";
    /// A declared type.
    ///
    /// Named `DeclaredTypeRef` because `TypeRef` is taken by
    /// [`ess_domain::types::TypeRef`], which is a different thing: a type
    /// *expression* — `Optional<List<Money>>` — where this is the name of one declaration.
    DeclaredTypeRef(QualifiedName) from TypeHandle, "a declared type";
    /// An entity.
    EntityRef(QualifiedName) from EntityHandle, "an entity";
    /// A command.
    CommandRef(QualifiedName) from CommandHandle, "a command";
    /// An event.
    EventRef(QualifiedName) from EventHandle, "an event";
    /// A declared error.
    ErrorRef(QualifiedName) from ErrorHandle, "a declared error";
    /// A view.
    ViewRef(QualifiedName) from ViewHandle, "a view";
    /// An actor.
    ActorRef(QualifiedName) from ActorHandle, "an actor";
    /// A component.
    ComponentRef(ComponentName) from ComponentHandle, "a component";
}

/// A binding.
///
/// Not in the generated family above: a binding has no handle in the IR to be minted from — it is
/// keyed by [`BindingName`] on `EssIr::bindings` — so there is no one-way door to generate.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
#[serde(transparent)]
pub struct BindingRef(BindingName);

impl BindingRef {
    /// Names a binding.
    pub fn new(name: BindingName) -> Self {
        Self(name)
    }

    /// The name it carries.
    pub fn name(&self) -> &BindingName {
        &self.0
    }
}

impl fmt::Display for BindingRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for BindingRef {
    type Err = ParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        BindingName::new(value).map(Self)
    }
}

impl<'de> serde::Deserialize<'de> for BindingRef {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        BindingName::new(raw)
            .map(Self)
            .map_err(serde::de::Error::custom)
    }
}

/// One branch of one command: `billing.invoice.CreateInvoice` / `accepted`.
///
/// The pair, because an outcome name alone means nothing — `rejected` is declared by three commands
/// in the billing example, and a reference that could not tell them apart would make a fault matrix
/// ambiguous exactly where it has to be precise.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub struct OutcomeRef {
    /// The command that declares it.
    pub command: CommandRef,
    /// Which branch.
    pub outcome: OutcomeName,
}

impl OutcomeRef {
    /// Names one branch of one command.
    pub fn new(command: CommandRef, outcome: OutcomeName) -> Self {
        Self { command, outcome }
    }
}

impl fmt::Display for OutcomeRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.command, self.outcome)
    }
}

/// One declared move of one entity's lifecycle: `billing.invoice.Invoice` / `settle`.
///
/// The entity is part of it because a transition is declared *inside* a lifecycle and has no
/// qualified name of its own — the model never spells `billing.invoice.Invoice.State.settle`, and
/// inventing one here would be a name no other tool could resolve.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub struct TransitionRef {
    /// The entity whose lifecycle declares it.
    pub entity: EntityRef,
    /// The move's own name, such as `settle`.
    #[serde(deserialize_with = "deserialize_transition_name")]
    pub transition: String,
}

impl TransitionRef {
    /// Names one move of one entity, refusing a name a lifecycle cannot declare.
    pub fn new(entity: EntityRef, transition: impl AsRef<str>) -> Result<Self, ParseError> {
        Ok(Self {
            entity,
            transition: transition_name(transition.as_ref())?,
        })
    }
}

impl fmt::Display for TransitionRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.entity, self.transition)
    }
}

/// Checks that a transition name is a single qualified-name segment.
///
/// The same rule [`Transition::new`](ess_domain::entity::Transition::new) applies when a
/// specification is parsed — written again here only because `ess-domain` keeps its helper private.
/// If that ever changes, delete this and call it: the rule has one owner, and this is a reader of it
/// rather than a second opinion.
fn transition_name(value: &str) -> Result<String, ParseError> {
    let parsed = QualifiedName::new(value)?;
    if parsed.segments().len() != 1 {
        return Err(ParseError::identifier(
            "transition name",
            value,
            "must be a single segment; the entity supplies the rest".to_owned(),
        ));
    }
    Ok(parsed.to_string())
}

/// Serde entry point for [`transition_name`], so a malformed move is refused while the suite is read.
fn deserialize_transition_name<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<String, D::Error> {
    let raw = <String as serde::Deserialize>::deserialize(deserializer)?;
    transition_name(&raw).map_err(serde::de::Error::custom)
}

/// Any construct of a specification, by name.
///
/// The element of a scenario's dependency set. One closed vocabulary rather than a string with a
/// convention, so "which scenarios depend on `billing.invoice.Money`?" is a question a later wave's
/// semantic diff can ask by matching values rather than by parsing prose.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EssSemanticRef {
    /// A bounded context.
    Domain {
        /// Which one.
        name: DomainRef,
    },
    /// A declared type — the one a scenario's input or payload mentions, however deeply.
    Type {
        /// Which one.
        name: DeclaredTypeRef,
    },
    /// An entity a scenario creates, moves or reads.
    Entity {
        /// Which one.
        name: EntityRef,
    },
    /// A command a scenario invokes.
    Command {
        /// Which one.
        name: CommandRef,
    },
    /// A branch a scenario requires.
    Outcome {
        /// Which one.
        name: OutcomeRef,
    },
    /// An event a scenario expects, or requires not to happen.
    Event {
        /// Which one.
        name: EventRef,
    },
    /// A declared error a scenario requires.
    Error {
        /// Which one.
        name: ErrorRef,
    },
    /// A view a scenario asserts.
    View {
        /// Which one.
        name: ViewRef,
    },
    /// An actor a scenario acts as.
    Actor {
        /// Which one.
        name: ActorRef,
    },
    /// A move a scenario takes.
    Transition {
        /// Which one.
        name: TransitionRef,
    },
    /// A binding a scenario's flow crosses.
    Binding {
        /// Which one.
        name: BindingRef,
    },
    /// A component a scenario's flow crosses.
    Component {
        /// Which one.
        name: ComponentRef,
    },
}

impl fmt::Display for EssSemanticRef {
    /// `command billing.invoice.CreateInvoice`, as design §23 writes one.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Domain { name } => write!(f, "domain {name}"),
            Self::Type { name } => write!(f, "type {name}"),
            Self::Entity { name } => write!(f, "entity {name}"),
            Self::Command { name } => write!(f, "command {name}"),
            Self::Outcome { name } => write!(f, "outcome {name}"),
            Self::Event { name } => write!(f, "event {name}"),
            Self::Error { name } => write!(f, "error {name}"),
            Self::View { name } => write!(f, "view {name}"),
            Self::Actor { name } => write!(f, "actor {name}"),
            Self::Transition { name } => write!(f, "transition {name}"),
            Self::Binding { name } => write!(f, "binding {name}"),
            Self::Component { name } => write!(f, "component {name}"),
        }
    }
}

/// Declares `From<X> for EssSemanticRef` per reference kind, so collecting a dependency set while
/// generating is `.into()` rather than a match a caller writes.
macro_rules! semantic_ref_from {
    ($($reference:ident => $variant:ident;)*) => {
        $(
            impl From<$reference> for EssSemanticRef {
                fn from(name: $reference) -> Self {
                    Self::$variant { name }
                }
            }
        )*
    };
}

semantic_ref_from! {
    DomainRef => Domain;
    DeclaredTypeRef => Type;
    EntityRef => Entity;
    CommandRef => Command;
    OutcomeRef => Outcome;
    EventRef => Event;
    ErrorRef => Error;
    ViewRef => View;
    ActorRef => Actor;
    TransitionRef => Transition;
    BindingRef => Binding;
    ComponentRef => Component;
}
