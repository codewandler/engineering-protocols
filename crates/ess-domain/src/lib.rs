//! The typed model for an **Executable System Specification**.
//!
//! A system described once, semantically, so that its contracts, documentation, tests and structural
//! code can be derived from one document rather than maintained beside it.
//!
//! ```text
//! ESS              what must exist
//!  │
//!  ▼
//! ADP              governs the work toward it
//!  │
//!  ▼
//! conformance      checks the result against the same document
//!  │
//!  ▼
//! evidence         which the protocol decides on
//! ```
//!
//! # The rule everything else follows
//!
//! **Semantic concepts are primary; transports are projections.** `CreateInvoice` is a command;
//! `POST /v1/invoices` is one way to expose it. `InvoiceCreated` is a fact; a Kafka topic is one way
//! to carry it. Losing that distinction is how a domain model becomes a description of an HTTP API,
//! and it is why the same specification can compile to a modular monolith or to distributed services
//! without the domain changing.
//!
//! | module | contents |
//! |---|---|
//! | [`name`] | identity, wire names, display names and versions — three things, kept apart |
//! | [`types`] | the type system: primitives, composites, tagged unions, named types |
//! | [`entity`] | entities, their lifecycles and their invariants |
//! | [`command`] | commands with outcomes, the events they emit and the errors they name |
//! | [`view`] | observable projections, and the consistency that decides how they are asserted |
//! | [`actor`] | who may invoke what |
//! | [`domain`] | a bounded context: what belongs to it |
//! | [`system`] | the system and its domains: what belongs where |
//! | [`spec`] | the members themselves, assembled from however many files they were written in |
//! | [`locate`] | how something outside the specification addresses one declaration inside it |
//!
//! # Two stages, and what is exempt
//!
//! Everything here follows the same discipline as the protocol half: documents parse into `Raw*`
//! types and become validated ones through `TryFrom`, and validated types do not implement
//! [`Deserialize`](serde::Deserialize). That is what stops a rule from being skippable — a
//! [`NamedType`](types::NamedType) whose invariant reads a field it does not have cannot be
//! constructed, because the only route in runs the check.
//!
//! The exemptions are scalars that validate themselves: [`QualifiedName`](name::QualifiedName),
//! [`Version`](name::Version), [`StateName`](entity::StateName), [`Field`](types::Field) and their
//! kind. Their entire validity is a question about their own spelling, answerable with nothing else
//! in hand, so a hand-written `Deserialize` *is* the conversion rather than a way around it. The
//! line is whether validating needs to look at anything else: if it does, the type gets a `Raw*`
//! form, so that the errors accumulate and arrive together instead of one per run.

pub mod actor;
pub mod command;
pub mod domain;
pub mod entity;
pub mod locate;
pub mod name;
pub mod spec;
pub mod system;
pub mod types;
pub mod view;

pub use actor::ActorSpec;
pub use command::{
    CommandSpec, ErrorSpec, EventSpec, Outcome, OutcomeCondition, OutcomeName, TestStrategy,
};
pub use domain::DomainSpec;
pub use entity::{EntityCatalogue, EntitySpec, Invariant, StateMachine, StateName, Transition};
pub use name::{Naming, QualifiedName, Version};
pub use spec::{RawSpecFile, Specification};
pub use system::{FormatVersion, SystemSpec};
pub use types::{Field, NamedType, Primitive, TypeBody, TypeRef, TypeRegistry};
pub use view::{AssertionStyle, Consistency, EntityFields, ViewSpec};
