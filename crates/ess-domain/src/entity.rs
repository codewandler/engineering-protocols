//! Entities, their lifecycles and their invariants.
//!
//! An entity has stable identity inside the domain (§4.3): two invoices with the same fields are
//! still two invoices. A **value object** has none — it is its fields — so it is not modelled here
//! at all but as a [`TypeBody::Struct`] in [`crate::types`], which
//! already carries fields and invariants. One concept, one place.
//!
//! # An invariant is a predicate, not a sentence
//!
//! §4.7 offers three example invariants:
//!
//! ```text
//! Paid cannot transition to Cancelled     ← not an invariant at all: see below
//! total.amount >= 0                       ← a predicate over the entity's fields
//! state == Issued implies customer_id exists
//! ```
//!
//! The middle one is what an invariant is: a condition a generator can compile into a guard, a
//! property test and a documentation line. It is parsed here with
//! [`Predicate::parse_expression`](aep_domain::predicate::Predicate::parse_expression) — the same
//! expression language the protocol half evaluates — so the review's F4 holds: "invariants reference
//! valid model fields" is a rule that can actually be checked, because the fields an invariant reads
//! are recoverable from it.
//!
//! The first one is **not an invariant**. "Paid cannot transition to Cancelled" is the *absence* of
//! a transition from `Paid` to `Cancelled`, and a [`StateMachine`] that does not declare that
//! transition already forbids the move. Written as a rule as well, it is a second statement of one
//! fact — two things to keep in step, one of which nothing enforces. Legality is structural; see
//! [`StateMachine::can_move`].
//!
//! The third needs implication, which the expression form does not spell. Write it in the
//! structured form, which [`Invariant`] also accepts:
//!
//! ```yaml
//! invariants:
//!   - total.amount >= 0
//!   - any:
//!       - not: state == Issued
//!       - defined(customer_id)
//! ```

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;
use std::str::FromStr;

use aep_domain::error::{ParseError, ValidationCode, ValidationError, ValidationErrors};
use aep_domain::facts::FactPath;
use aep_domain::node::Node;
use aep_domain::predicate::Predicate;

use crate::name::{Naming, QualifiedName};
use crate::types::{Field, TypeBody, TypeRef, TypeRegistry};

/// The name of one state in an entity's lifecycle, such as `Draft`.
///
/// `UpperCamelCase`, one word, no separators. The rule is narrow on purpose: a state name is
/// projected straight into generated code as an enum variant and into stored data as a value, so
/// `paid_out` and `PaidOut` would be two spellings of one state — and two states to anything that
/// diffs two versions of a specification.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
#[serde(transparent)]
pub struct StateName(String);

impl StateName {
    /// Parses a state name.
    pub fn new(value: impl AsRef<str>) -> Result<Self, ParseError> {
        let value = value.as_ref();
        let reject = |reason: String| Err(ParseError::identifier("state name", value, reason));

        if value.is_empty() {
            return reject("must not be empty".to_owned());
        }
        for character in value.chars() {
            if !character.is_ascii_alphanumeric() {
                return reject(format!(
                    "contains {character:?}; a state name is one UpperCamelCase word, because it \
                     becomes an enum variant and a stored value"
                ));
            }
        }
        let first = value.chars().next().unwrap_or('_');
        if !first.is_ascii_uppercase() {
            return reject(format!(
                "must start with an upper-case letter, as in `Draft`, got {first:?}"
            ));
        }
        Ok(Self(value.to_owned()))
    }

    /// The name as written.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The pattern published in generated JSON Schema.
    pub const PATTERN: &'static str = "^[A-Z][A-Za-z0-9]*$";
}

impl fmt::Display for StateName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl fmt::Debug for StateName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "StateName({})", self.0)
    }
}

impl FromStr for StateName {
    type Err = ParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

impl<'de> serde::Deserialize<'de> for StateName {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        Self::new(raw).map_err(serde::de::Error::custom)
    }
}

impl schemars::JsonSchema for StateName {
    fn schema_name() -> String {
        "StateName".to_owned()
    }

    fn json_schema(_: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        let mut schema = schemars::schema::SchemaObject {
            instance_type: Some(schemars::schema::InstanceType::String.into()),
            ..Default::default()
        };
        schema.string().pattern = Some(Self::PATTERN.to_owned());
        schema.metadata().description = Some(
            "One state of an entity's lifecycle, in UpperCamelCase, such as `Draft`.".to_owned(),
        );
        schema.into()
    }
}

/// A named move between states.
///
/// The name is the transition's own, unqualified, and it is a name *inside the entity*: the entity's
/// own name plus this one is what an outcome writes to say it takes this move, so `settle` on
/// `billing.invoice.Invoice` is written `moves: billing.invoice.Invoice.settle`. It is not a command
/// name, and no command is found by matching one — inferring the driving command from a transition's
/// spelling is exactly the invention the conformance design §19 refuses, which is why the link is
/// declared on the outcome ([`Subject`](crate::command::Subject)) rather than guessed here.
///
/// `from` is a set because one move can start in several states — §4.7's `CancelInvoice` leaves
/// both `Draft` and `Issued` — and `to` is one state because a move with two destinations is two
/// moves with different guards.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct Transition {
    /// Its own name, such as `IssueInvoice`.
    #[serde(deserialize_with = "deserialize_local_name")]
    pub name: String,
    /// The states it may start in.
    pub from: BTreeSet<StateName>,
    /// The state it ends in.
    pub to: StateName,
}

impl Transition {
    /// Builds a transition, checking that the name can become the last segment of a qualified name.
    pub fn new(
        name: impl AsRef<str>,
        from: impl IntoIterator<Item = StateName>,
        to: StateName,
    ) -> Result<Self, ParseError> {
        Ok(Self {
            name: local_name(name.as_ref())?,
            from: from.into_iter().collect(),
            to,
        })
    }
}

/// Checks that `value` is a single qualified-name segment.
fn local_name(value: &str) -> Result<String, ParseError> {
    let parsed = QualifiedName::new(value)?;
    if parsed.segments().len() != 1 {
        return Err(ParseError::identifier(
            "transition name",
            value,
            "must be a single segment; the entity's namespace supplies the rest of the qualified \
             name"
                .to_owned(),
        ));
    }
    Ok(parsed.to_string())
}

/// Serde entry point for [`local_name`], so a malformed transition name is refused while the
/// document is read rather than surviving into the model.
fn deserialize_local_name<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<String, D::Error> {
    let raw = <String as serde::Deserialize>::deserialize(deserializer)?;
    local_name(&raw).map_err(serde::de::Error::custom)
}

/// The lifecycle of an entity (§4.7).
///
/// What a generator needs to emit an enum, a transition function, guards, tests and a diagram — and
/// what makes "can this entity go from here to there?" a question with a structural answer.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct StateMachine {
    /// Every state the entity can be in.
    pub states: BTreeSet<StateName>,
    /// Where a new entity starts.
    pub initial: StateName,
    /// States the entity is allowed to rest in forever.
    ///
    /// Declared rather than inferred from "has no outgoing transition": an entity that can never
    /// leave a state is either finished or stuck, and only the author knows which.
    #[serde(skip_serializing_if = "BTreeSet::is_empty")]
    pub terminal: BTreeSet<StateName>,
    /// The moves that are permitted. Anything not listed is not permitted.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub transitions: Vec<Transition>,
}

impl StateMachine {
    /// Transitions that may be taken from `state`.
    pub fn outgoing(&self, state: &StateName) -> Vec<&Transition> {
        self.transitions
            .iter()
            .filter(|transition| transition.from.contains(state))
            .collect()
    }

    /// Transitions that arrive at `state`.
    pub fn incoming(&self, state: &StateName) -> Vec<&Transition> {
        self.transitions
            .iter()
            .filter(|transition| &transition.to == state)
            .collect()
    }

    /// The transition with this name.
    pub fn transition(&self, name: &str) -> Option<&Transition> {
        self.transitions
            .iter()
            .find(|transition| transition.name == name)
    }

    /// `true` when some transition moves the entity from `from` to `to`.
    ///
    /// There is deliberately no `forbids` counterpart. A move nothing declares is already
    /// impossible, so "Paid cannot transition to Cancelled" is not a rule to write down but a
    /// transition to leave out (review F4). A rule stating it as well is a second copy of one fact,
    /// and nothing keeps the copy honest.
    pub fn can_move(&self, from: &StateName, to: &StateName) -> bool {
        self.transitions
            .iter()
            .any(|transition| transition.from.contains(from) && &transition.to == to)
    }

    /// `true` when the entity may rest in `state` forever.
    pub fn is_terminal(&self, state: &StateName) -> bool {
        self.terminal.contains(state)
    }

    /// States reachable from [`StateMachine::initial`] by following transitions.
    fn reachable(&self) -> BTreeSet<&StateName> {
        let mut seen: BTreeSet<&StateName> = BTreeSet::new();
        let Some(initial) = self.states.get(&self.initial) else {
            return seen;
        };
        let mut queue: VecDeque<&StateName> = [initial].into();
        seen.insert(initial);
        while let Some(current) = queue.pop_front() {
            for transition in self.outgoing(current) {
                if let Some(target) = self.states.get(&transition.to) {
                    if seen.insert(target) {
                        queue.push_back(target);
                    }
                }
            }
        }
        seen
    }

    /// Checks the machine against itself, reporting every problem rather than the first.
    ///
    /// Errors are located under `entity`; use [`StateMachine::validate_at`] to say which entity.
    pub fn validate(&self) -> ValidationErrors {
        self.validate_at("entity")
    }

    /// As [`StateMachine::validate`], with `location` naming the owner of the machine.
    ///
    /// Everything checked here is answerable from the document alone; nothing needs the type
    /// registry or the rest of the specification, which is why it runs on conversion.
    ///
    /// A state this machine does not declare is reported as
    /// [`UnknownState`](ValidationCode::UnknownState) wherever it is named — `terminal`, a `from` or
    /// a `to`. That is the code reserved for a state reference;
    /// [`UndeclaredReference`](ValidationCode::UndeclaredReference) is for the types, events and
    /// entities a specification also names, and a tool reading one off a misspelt state learns the
    /// wrong thing about what to fix. `initial` keeps its own
    /// [`UnknownInitialState`](ValidationCode::UnknownInitialState), which says the same thing about
    /// the one state whose absence stops an entity from ever existing.
    pub fn validate_at(&self, location: &str) -> ValidationErrors {
        let mut errors = ValidationErrors::new();

        if self.states.is_empty() {
            errors.push(ValidationError::new(
                ValidationCode::EmptyDeclaration,
                format!("{location}.states"),
                "an entity must declare at least one state",
            ));
        } else if !self.states.contains(&self.initial) {
            // Suppressed when nothing is declared: "the initial state is undeclared" adds no
            // information to "no state is declared".
            errors.push(
                ValidationError::new(
                    ValidationCode::UnknownInitialState,
                    format!("{location}.initial"),
                    format!("initial state `{}` is not declared", self.initial),
                )
                .with_hint(format!("declared states: {}", self.declared())),
            );
        }

        for state in &self.terminal {
            if !self.states.contains(state) {
                errors.push(
                    ValidationError::new(
                        ValidationCode::UnknownState,
                        format!("{location}.terminal"),
                        format!("`{state}` is declared terminal but is not a declared state"),
                    )
                    .with_hint(format!("declared states: {}", self.declared())),
                );
            }
        }

        errors.extend(self.validate_transitions(location));
        errors.extend(self.validate_states(location));

        errors
    }

    /// Every transition names declared states, and no name is used twice.
    fn validate_transitions(&self, location: &str) -> ValidationErrors {
        let mut errors = ValidationErrors::new();
        let mut named: BTreeSet<&str> = BTreeSet::new();
        for (index, transition) in self.transitions.iter().enumerate() {
            let at = format!("{location}.transitions[{index}]");
            for state in &transition.from {
                if !self.states.contains(state) {
                    errors.push(ValidationError::new(
                        ValidationCode::UnknownState,
                        format!("{at}.from"),
                        format!(
                            "`{}` moves from `{state}`, which is not a declared state",
                            transition.name
                        ),
                    ));
                }
            }
            if !self.states.contains(&transition.to) {
                errors.push(ValidationError::new(
                    ValidationCode::UnknownState,
                    format!("{at}.to"),
                    format!(
                        "`{}` moves to `{}`, which is not a declared state",
                        transition.name, transition.to
                    ),
                ));
            }
            if !named.insert(transition.name.as_str()) {
                errors.push(
                    ValidationError::new(
                        ValidationCode::DuplicateTransition,
                        at,
                        format!(
                            "a second transition named `{}` is declared",
                            transition.name
                        ),
                    )
                    .with_hint(
                        "one name is one move; list every source state under the first `from` \
                         instead",
                    ),
                );
            }
        }

        errors
    }

    /// No state is a dead end, and none is unreachable.
    fn validate_states(&self, location: &str) -> ValidationErrors {
        let mut errors = ValidationErrors::new();
        let reachable = self.reachable();
        for state in &self.states {
            let at = format!("{location}.states.{state}");
            // A transition that arrives where it started is not a way out: an entity taking it is
            // exactly where it was, so a state whose only moves are its own self-loops traps one as
            // surely as a state with no moves at all.
            let outgoing = self.outgoing(state);
            let leaves = outgoing.iter().any(|transition| &transition.to != state);
            if !self.terminal.contains(state) && !leaves {
                let detail = if outgoing.is_empty() {
                    "has no outgoing transition"
                } else {
                    "has no transition that leaves it"
                };
                errors.push(
                    ValidationError::new(
                        ValidationCode::DeadEndState,
                        at.clone(),
                        format!(
                            "`{state}` {detail} and is not declared terminal, so an entity that \
                             reaches it is stuck"
                        ),
                    )
                    .with_hint("add a transition out of it, or list it under `terminal`"),
                );
            }
            if !self.states.is_empty() && !reachable.contains(state) {
                errors.push(
                    ValidationError::new(
                        ValidationCode::UnreachableState,
                        at,
                        format!("`{state}` cannot be reached from `{}`", self.initial),
                    )
                    .with_hint(
                        "add a transition into it; an unreachable state is usually a misspelt `to`",
                    ),
                );
            }
        }

        errors
    }

    /// The declared states, for a diagnostic hint.
    fn declared(&self) -> String {
        self.states
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// A condition that must hold of every instance of an entity, at rest.
///
/// Both halves are kept: [`Invariant::predicate`] is what a generator compiles and what validation
/// checks against the entity's fields, and [`Invariant::statement`] is what the author wrote, so a
/// diagnostic can quote the specification rather than a re-rendering of it.
///
/// This is *not* where transition legality is written. See [`StateMachine::can_move`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invariant {
    /// The condition as the author wrote it.
    pub statement: String,
    /// The same condition, parsed.
    pub predicate: Predicate,
}

impl Invariant {
    /// Parses the compact expression form, such as `total.amount >= 0`.
    pub fn parse(statement: impl Into<String>) -> Result<Self, ParseError> {
        let statement = statement.into();
        let predicate = Predicate::parse_expression(&statement)?;
        Ok(Self {
            statement,
            predicate,
        })
    }

    /// Parses either form: a string expression, or the structured mapping that spells `any`, `all`
    /// and `not` — which is how an implication is written, since the expression form has no
    /// `implies`.
    pub fn from_node(node: &Node) -> Result<Self, ParseError> {
        let predicate = Predicate::from_node(node)?;
        let statement = match node {
            Node::Text(text) => text.clone(),
            _ => predicate.to_string(),
        };
        Ok(Self {
            statement,
            predicate,
        })
    }
}

impl fmt::Display for Invariant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.statement)
    }
}

impl serde::Serialize for Invariant {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        // The author's own spelling is emitted whenever it still parses to the same predicate, so a
        // document survives a round trip looking the way it was written. A structured invariant is
        // emitted structurally, because its rendered form (`(not (a) or b)`) is not something the
        // expression parser reads back.
        if Predicate::parse_expression(&self.statement).ok().as_ref() == Some(&self.predicate) {
            serializer.serialize_str(&self.statement)
        } else {
            self.predicate.serialize(serializer)
        }
    }
}

/// An invariant, as parsed.
///
/// Well-formedness is settled here — an unparsable predicate is a [`ParseError`] reported by serde
/// with document context — so that by the time [`EntitySpec::validate`] runs, the only question
/// left is whether the fields it reads exist. A value object's invariants
/// ([`TypeBody`]) are read through this same type, because one language for
/// invariants is the point of writing them as predicates at all.
#[derive(Debug, Clone)]
pub struct RawInvariant(Invariant);

impl<'de> serde::Deserialize<'de> for RawInvariant {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let node = Node::deserialize(deserializer)?;
        Invariant::from_node(&node)
            .map(Self)
            .map_err(serde::de::Error::custom)
    }
}

impl schemars::JsonSchema for RawInvariant {
    fn schema_name() -> String {
        "Invariant".to_owned()
    }

    fn json_schema(generator: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        match <Predicate as schemars::JsonSchema>::json_schema(generator) {
            schemars::schema::Schema::Object(mut schema) => {
                schema.metadata().description = Some(
                    "A condition every instance must satisfy, as a predicate over its fields — an \
                     entity's may also read the pseudo-field `state`, a newtype's reads `value`. \
                     Transition legality is not written here: a move that is not declared is \
                     already impossible."
                        .to_owned(),
                );
                schema.into()
            }
            boolean @ schemars::schema::Schema::Bool(_) => boolean,
        }
    }
}

impl From<RawInvariant> for Invariant {
    fn from(raw: RawInvariant) -> Self {
        raw.0
    }
}

/// An entity: something with stable identity inside the domain (§4.3).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct EntitySpec {
    /// Its stable logical identity, such as `billing.invoice.Invoice`.
    pub name: QualifiedName,
    /// How the entity is identified — the field's name as well as its type.
    ///
    /// The name matters as much as the type. A view projecting `invoice_id` needs that name to exist
    /// somewhere, and if the model does not carry it, every generator invents one — so the
    /// projection is called `id` in the `OpenAPI` projection, `invoice_id` in the specification, and something
    /// else again in the Rust.
    pub identity: Field,
    /// What it holds.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<Field>,
    /// Its lifecycle.
    #[serde(rename = "lifecycle")]
    pub states: StateMachine,
    /// What must hold of every instance, at rest.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub invariants: Vec<Invariant>,
    /// What it is called on the wire and shown as.
    #[serde(skip_serializing_if = "Naming::is_empty")]
    pub naming: Naming,
}

impl EntitySpec {
    /// The pseudo-field an invariant reads to talk about the lifecycle.
    ///
    /// `state` is not in [`EntitySpec::fields`] — it is the state machine — but §4.7's
    /// `state == Issued implies customer_id exists` needs to name it, and inventing a real field
    /// would let a specification declare a second, contradictory one.
    pub const STATE: &'static str = "state";

    /// What a view may project: the identity, the declared fields, and the state.
    ///
    /// Wider than [`EntitySpec::fields`] on purpose. A view is an *observable*, and the two things
    /// most often observed — which invoice this is, and what has happened to it — are not declared
    /// fields. Leaving them out would make every realistic view fail validation for projecting
    /// something the entity "does not have".
    ///
    /// The state is typed as the entity's own lifecycle, so a view projecting it and a filter
    /// comparing it are checked against the same set of names.
    pub fn observable_fields(&self) -> Vec<Field> {
        let mut observable = vec![self.identity.clone()];
        observable.extend(self.fields.iter().cloned());
        observable.push(Field::new(
            Self::STATE,
            TypeRef::Named(self.name.child(Self::STATE_TYPE)),
        ));
        observable
    }

    /// The name of the synthesised type holding this entity's states.
    ///
    /// `billing.invoice.Invoice.State` — derived rather than declared, because a lifecycle already
    /// names every one of its states and a second declaration could disagree with it.
    pub const STATE_TYPE: &'static str = "State";

    /// The enum type this entity's states form, for a registry to hold.
    pub fn state_type(&self) -> crate::types::NamedType {
        crate::types::NamedType {
            name: self.name.child(Self::STATE_TYPE),
            body: crate::types::TypeBody::Enum {
                variants: self.states.states.iter().map(ToString::to_string).collect(),
            },
            naming: Naming::default(),
        }
    }

    /// The field with this name.
    pub fn field(&self, name: &str) -> Option<&Field> {
        self.fields.iter().find(|field| field.name == name)
    }

    /// Every named type this entity depends on, identity included.
    pub fn dependencies(&self) -> Vec<&QualifiedName> {
        self.identity
            .type_ref
            .named_dependencies()
            .into_iter()
            .chain(
                self.fields
                    .iter()
                    .flat_map(|field| field.type_ref.named_dependencies()),
            )
            .collect()
    }

    /// Checks the entity against the rest of the specification.
    ///
    /// * the identity type and every field type is declared;
    /// * every invariant reads only what this entity has: its fields, its identity and
    ///   [`EntitySpec::STATE`].
    ///
    /// The lifecycle is *not* re-checked here: it needs nothing but the document, so it is settled
    /// when [`RawEntitySpec`] is converted and an [`EntitySpec`] that failed it does not exist.
    pub fn validate(&self, registry: &TypeRegistry) -> ValidationErrors {
        let mut errors = ValidationErrors::new();
        let location = format!("entity {}", self.name);

        errors.extend(registry.resolve(&self.identity.type_ref, &format!("{location}.identity")));
        for field in &self.fields {
            errors.extend(registry.resolve(
                &field.type_ref,
                &format!("{location}.fields.{}", field.name),
            ));
        }

        for (index, invariant) in self.invariants.iter().enumerate() {
            let at = format!("{location}.invariants[{index}]");
            for path in invariant.predicate.fact_paths() {
                let root = path.namespace();
                if root == Self::STATE {
                    continue;
                }
                let Some(field) = self.readable_field(root) else {
                    errors.push(
                        ValidationError::new(
                            ValidationCode::UnobservableFact,
                            at.clone(),
                            format!(
                                "`{invariant}` reads `{path}`, and `{root}` is not a field of `{}`",
                                self.name
                            ),
                        )
                        .with_hint(format!("readable here: {}", self.readable())),
                    );
                    continue;
                };
                check_nested(registry, &at, invariant, path, field, &mut errors);
            }
        }

        errors
    }

    /// The field an invariant may read under this name.
    ///
    /// Wider than [`EntitySpec::field`] by the identity, for the same reason
    /// [`EntitySpec::observable_fields`] is: an entity that declares `invoice_id` as its identity
    /// has `invoice_id`, and a rule that may not name it would be refusing a specification for
    /// saying something true about itself.
    fn readable_field(&self, name: &str) -> Option<&Field> {
        if self.identity.name == name {
            return Some(&self.identity);
        }
        self.field(name)
    }

    /// What an invariant may name, for a diagnostic hint.
    fn readable(&self) -> String {
        std::iter::once(self.identity.name.clone())
            .chain(self.fields.iter().map(|field| field.name.clone()))
            .chain([Self::STATE.to_owned()])
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// Follows `field.a.b` through the registry for as long as each step is a declared struct.
///
/// Only the segments that stay inside declared structs can be checked. A path into a newtype, an
/// enum, a union or a primitive stops being a field lookup — `Money` may well expose `amount`
/// through a representation this model does not describe — and guessing past that point would
/// refuse specifications that are fine.
fn check_nested(
    registry: &TypeRegistry,
    at: &str,
    invariant: &Invariant,
    path: &FactPath,
    field: &Field,
    errors: &mut ValidationErrors,
) {
    let mut current = field.type_ref.required().clone();
    for segment in path.segments().iter().skip(1) {
        let TypeRef::Named(name) = &current else {
            return;
        };
        let Some(declared) = registry.get(name) else {
            return; // Already reported where the field's own type failed to resolve.
        };
        let TypeBody::Struct { fields, .. } = &declared.body else {
            return;
        };
        let Some(next) = fields.iter().find(|candidate| &candidate.name == segment) else {
            errors.push(
                ValidationError::new(
                    ValidationCode::UnobservableFact,
                    at.to_owned(),
                    format!("`{invariant}` reads `{path}`, and `{name}` has no field `{segment}`"),
                )
                .with_hint(format!(
                    "`{name}` has: {}",
                    fields
                        .iter()
                        .map(|candidate| candidate.name.clone())
                        .collect::<Vec<_>>()
                        .join(", ")
                )),
            );
            return;
        };
        current = next.type_ref.required().clone();
    }
}

/// Checks the link between a command's outcomes and the lifecycles they drive, in both directions.
///
/// One pass, because it is one relation. From the command's side it resolves the reference: an
/// outcome that acts on an entity nothing declares, or takes a transition its subject's lifecycle
/// does not declare, is [`UndeclaredReference`](ValidationCode::UndeclaredReference) — the code every
/// other well-formed name pointing at nothing already gets. From the entity's side it checks the
/// relation is total: **a declared transition no outcome performs is
/// [`MissingCausation`](ValidationCode::MissingCausation)**.
///
/// # Why the transition side is required and the outcome side is not
///
/// `billing.email.SendEmail` changes no entity, and a model that made it name one would be a model
/// that made an author invent a subject. So an outcome may say nothing.
///
/// A transition may not. `Issued → Paid` is a state change the specification promises the system can
/// make; if nothing can trigger it, the promise is unkeepable, and the reason it stays unkeepable is
/// that nothing looks. That is exactly the shape the inhabitation check in
/// [`crate::system`] refuses on the type graph — a
/// declaration no value can reach — read on the lifecycle instead, and it is what design §19 needs
/// in order to be total: every transition it must prove legal has a command that takes it.
///
/// The symmetric rule for *creation* is deliberately not here. An entity no outcome creates is a
/// real gap for scenario synthesis, and refusing it would also refuse a specification whose entities
/// arrive from a migration or from a system outside this document — a wider claim than G14 makes,
/// and one worth arguing on its own rather than smuggling in beside this one.
///
/// # A refusal's subject is not a cause
///
/// An outcome that names an error and declares a subject is already refused by
/// [`CommandSpec::validate`](crate::command::CommandSpec::validate) as [`RefusalMutatedState`](ValidationCode::RefusalMutatedState), and it
/// is *not* counted here as performing its transition. So an author whose only mover is a refusal
/// learns both facts in one run rather than one per run: the refusal must lose its subject, and the
/// transition still needs a cause.
pub fn validate_lifecycle_causes(
    entities: &BTreeMap<QualifiedName, EntitySpec>,
    commands: &BTreeMap<QualifiedName, crate::command::CommandSpec>,
) -> ValidationErrors {
    let mut errors = ValidationErrors::new();
    let mut performed: BTreeSet<(&QualifiedName, &str)> = BTreeSet::new();

    for command in commands.values() {
        for outcome in &command.outcomes {
            let Some(subject) = &outcome.subject else {
                continue;
            };
            let at = format!(
                "command.{}.outcomes.{}.{}",
                command.name,
                outcome.name,
                subject.effect.verb()
            );
            let Some(entity) = entities.get(&subject.entity) else {
                errors.push(
                    ValidationError::new(
                        ValidationCode::UndeclaredReference,
                        at,
                        format!(
                            "outcome `{}` of `{}` {} `{}`, which is not a declared entity",
                            outcome.name,
                            command.name,
                            subject.effect.verb(),
                            subject.entity
                        ),
                    )
                    .with_hint(format!(
                        "declared entities: {}",
                        names(entities.keys().map(ToString::to_string))
                    )),
                );
                continue;
            };
            let Some(transition) = subject.effect.transition() else {
                continue;
            };
            if entity.states.transition(transition).is_none() {
                errors.push(
                    ValidationError::new(
                        ValidationCode::UndeclaredReference,
                        at,
                        format!(
                            "outcome `{}` of `{}` takes `{}`, which `{}` does not declare as a \
                             transition",
                            outcome.name, command.name, transition, subject.entity
                        ),
                    )
                    .with_hint(format!(
                        "`{}` declares: {}",
                        subject.entity,
                        names(
                            entity
                                .states
                                .transitions
                                .iter()
                                .map(|declared| declared.name.clone())
                        )
                    )),
                );
                continue;
            }
            if outcome.is_refusal() {
                continue;
            }
            performed.insert((&subject.entity, transition));
        }
    }

    for entity in entities.values() {
        for (index, transition) in entity.states.transitions.iter().enumerate() {
            if performed.contains(&(&entity.name, transition.name.as_str())) {
                continue;
            }
            errors.push(
                ValidationError::new(
                    ValidationCode::MissingCausation,
                    format!("entity {}.transitions[{index}]", entity.name),
                    format!(
                        "`{}` moves `{}` to `{}`, and no command outcome takes it, so nothing in \
                         this specification can make that state change happen",
                        transition.name, entity.name, transition.to
                    ),
                )
                .with_hint(format!(
                    "give some outcome `moves: {}`, or delete the transition",
                    entity.name.child(&transition.name)
                )),
            );
        }
    }

    errors
}

/// Renders a list of names for a diagnostic hint, saying so when the list is empty.
fn names(items: impl Iterator<Item = String>) -> String {
    let rendered: Vec<String> = items.collect();
    if rendered.is_empty() {
        "none are declared".to_owned()
    } else {
        rendered.join(", ")
    }
}

/// An entity's lifecycle, as parsed.
///
/// Transitions are a sequence rather than §4.7's mapping keyed by name, because a mapping cannot
/// hold two transitions with the same name: a duplicate would be swallowed by the YAML parser
/// instead of reported, and the rule against it would be unenforceable.
#[derive(Debug, Clone, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RawStateMachine {
    /// Every state the entity can be in.
    pub states: BTreeSet<StateName>,
    /// Where a new entity starts.
    pub initial: StateName,
    /// States the entity may rest in forever.
    #[serde(default)]
    pub terminal: BTreeSet<StateName>,
    /// The moves that are permitted.
    #[serde(default)]
    pub transitions: Vec<Transition>,
}

impl From<RawStateMachine> for StateMachine {
    fn from(raw: RawStateMachine) -> Self {
        Self {
            states: raw.states,
            initial: raw.initial,
            terminal: raw.terminal,
            transitions: raw.transitions,
        }
    }
}

/// An entity, as parsed.
///
/// A lifecycle is required rather than defaulted. An entity with one state says so in three lines;
/// a default would invent a state name the author never wrote, and every generated enum, diagram
/// and stored value would then carry a word from this file rather than from the specification.
#[derive(Debug, Clone, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RawEntitySpec {
    /// Its stable logical identity.
    pub name: QualifiedName,
    /// How the entity is identified.
    pub identity: Field,
    /// What it holds.
    #[serde(default)]
    pub fields: Vec<Field>,
    /// What must hold of every instance.
    #[serde(default)]
    pub invariants: Vec<RawInvariant>,
    /// What it is called on the wire and shown as.
    #[serde(default)]
    pub naming: Naming,
    /// Its lifecycle, under `lifecycle`.
    ///
    /// §4.7 writes `states` and `transitions` as keys of the entity itself. They are nested here
    /// instead, because flattening them would cost every unknown-key check in the document: serde
    /// ignores `deny_unknown_fields` on a struct that flattens *and* on the struct flattened into
    /// it, so `invariant:` for `invariants:` would silently drop every invariant an author wrote.
    /// A nested key is a small deviation; a specification that quietly means something else is not.
    #[serde(rename = "lifecycle")]
    pub states: RawStateMachine,
}

impl TryFrom<RawEntitySpec> for EntitySpec {
    type Error = ValidationErrors;

    fn try_from(raw: RawEntitySpec) -> Result<Self, Self::Error> {
        let spec = Self {
            name: raw.name,
            identity: raw.identity,
            fields: raw.fields,
            states: raw.states.into(),
            invariants: raw.invariants.into_iter().map(Invariant::from).collect(),
            naming: raw.naming,
        };
        let location = format!("entity {}", spec.name);

        let mut errors = spec.states.validate_at(&location);

        // A duplicate field is document-local, and it has to be caught here: the second declaration
        // would be invisible to every later lookup, so an invariant reading it would be checked
        // against a field nobody can see.
        let mut seen: BTreeMap<&str, usize> = BTreeMap::new();
        for field in &spec.fields {
            *seen.entry(field.name.as_str()).or_default() += 1;
        }
        for (name, count) in seen {
            if count > 1 {
                errors.push(ValidationError::new(
                    ValidationCode::DuplicateDeclaration,
                    format!("{location}.fields.{name}"),
                    format!("`{name}` is declared {count} times"),
                ));
            }
        }

        // Nor may a field shadow the identity. `observable_fields` offers both, so a view resolving
        // the name would get whichever came first — and the two declarations need not agree on a
        // type.
        for field in &spec.fields {
            if field.name == spec.identity.name {
                errors.push(
                    ValidationError::new(
                        ValidationCode::DuplicateDeclaration,
                        format!("{location}.fields.{}", field.name),
                        format!("`{}` is already the entity's identity", field.name),
                    )
                    .with_hint(
                        "an identity is observable under its own name; a field of that name would \
                         give one name two types",
                    ),
                );
            }
        }

        errors.into_result(spec)
    }
}

/// Lets a set of entities answer what a view may project.
///
/// The adapter lives here rather than in `view.rs` because the answer is a property of an entity —
/// its observable surface — and a view should not have to know how one is assembled.
#[derive(Debug, Default)]
pub struct EntityCatalogue {
    entities: std::collections::BTreeMap<QualifiedName, Vec<Field>>,
}

impl EntityCatalogue {
    /// Builds a catalogue from every entity in a specification.
    pub fn new<'a, I: IntoIterator<Item = &'a EntitySpec>>(entities: I) -> Self {
        Self {
            entities: entities
                .into_iter()
                .map(|entity| (entity.name.clone(), entity.observable_fields()))
                .collect(),
        }
    }
}

impl crate::view::EntityFields for EntityCatalogue {
    fn entity_fields(&self, name: &QualifiedName) -> Option<&[Field]> {
        self.entities.get(name).map(Vec::as_slice)
    }

    fn entity_names(&self) -> Vec<String> {
        self.entities.keys().map(ToString::to_string).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{NamedType, Primitive};

    /// §4.7's `Invoice`, which every structural test starts from and then breaks.
    const INVOICE: &str = "\
name: billing.invoice.Invoice
identity:
  name: invoice_id
  type: billing.invoice.InvoiceId
fields:
  - name: customer_id
    type: billing.CustomerId
  - name: total
    type: billing.Money
lifecycle:
  states: [Draft, Issued, Paid, Cancelled]
  initial: Draft
  terminal: [Paid, Cancelled]
  transitions:
    - name: IssueInvoice
      from: [Draft]
      to: Issued
    - name: PayInvoice
      from: [Issued]
      to: Paid
    - name: CancelInvoice
      from: [Draft, Issued]
      to: Cancelled
invariants:
  - total.amount >= 0
";

    fn entity(yaml: &str) -> Result<EntitySpec, ValidationErrors> {
        let raw: RawEntitySpec = serde_yaml::from_str(yaml).expect("the document is well formed");
        EntitySpec::try_from(raw)
    }

    fn state(value: &str) -> StateName {
        StateName::new(value).expect("a valid state name")
    }

    fn name(value: &str) -> QualifiedName {
        QualifiedName::new(value).expect("a valid name")
    }

    /// A registry with everything §4.7's `Invoice` refers to.
    fn registry() -> TypeRegistry {
        let mut registry = TypeRegistry::new();
        for newtype in ["billing.invoice.InvoiceId", "billing.CustomerId"] {
            registry
                .insert(NamedType {
                    name: name(newtype),
                    body: TypeBody::Newtype {
                        of: TypeRef::Primitive(Primitive::Uuid),
                        invariants: Vec::new(),
                    },
                    naming: Naming::default(),
                })
                .expect("new");
        }
        registry
            .insert(NamedType {
                name: name("billing.Money"),
                body: TypeBody::Struct {
                    fields: vec![
                        Field::new("amount", TypeRef::Primitive(Primitive::Decimal)),
                        Field::new("currency", TypeRef::Primitive(Primitive::String)),
                    ],
                    invariants: Vec::new(),
                },
                naming: Naming::default(),
            })
            .expect("new");
        registry
    }

    #[test]
    fn the_billing_invoice_from_the_design_document_validates() {
        let invoice = entity(INVOICE).expect("§4.7's Invoice is the normative fixture");
        let errors = invoice.validate(&registry());
        assert!(errors.is_empty(), "{errors}");

        assert_eq!(invoice.states.states.len(), 4);
        assert_eq!(invoice.states.initial, state("Draft"));
        assert!(invoice.states.is_terminal(&state("Paid")));
        assert_eq!(
            invoice
                .states
                .transition("CancelInvoice")
                .expect("declared")
                .from
                .len(),
            2,
            "cancelling leaves both Draft and Issued"
        );
    }

    #[test]
    fn a_move_that_is_not_declared_is_forbidden_by_its_absence() {
        let invoice = entity(INVOICE).expect("valid");
        let paid = state("Paid");
        let cancelled = state("Cancelled");

        assert!(!invoice.states.can_move(&paid, &cancelled));
        assert!(
            invoice.states.outgoing(&paid).is_empty(),
            "nothing leaves Paid, which is the whole statement"
        );
        assert!(
            invoice
                .invariants
                .iter()
                .all(|invariant| !invariant.statement.contains("Cancelled")),
            "no rule says it, because the machine already does (review F4)"
        );
        assert!(
            invoice.validate(&registry()).is_empty(),
            "and saying nothing about it is not itself an error"
        );
    }

    #[test]
    fn a_state_name_is_one_upper_camel_case_word() {
        assert_eq!(state("Draft").as_str(), "Draft");

        let lower = StateName::new("draft").expect_err("lower case");
        assert!(lower.to_string().contains("upper-case"), "{lower}");

        for spelling in ["Paid_Out", "paid-out", "Paid Out"] {
            let error = StateName::new(spelling).expect_err(spelling);
            assert!(
                error.to_string().contains("UpperCamelCase"),
                "{spelling}: {error}"
            );
        }
        assert!(StateName::new("").is_err(), "empty");
        assert!(StateName::new("1Draft").is_err(), "leading digit");
    }

    #[test]
    fn an_initial_state_that_is_not_declared_is_refused() {
        let errors = entity(&INVOICE.replace("initial: Draft", "initial: Drafted"))
            .expect_err("the initial state is misspelt");
        assert!(
            errors.contains(ValidationCode::UnknownInitialState),
            "{errors}"
        );
        let rendered = errors.to_string();
        assert!(rendered.contains("`Drafted` is not declared"), "{rendered}");
        assert!(
            rendered.contains("declared states: Cancelled, Draft"),
            "the refusal lists what was available: {rendered}"
        );
    }

    #[test]
    fn a_transition_to_a_state_that_does_not_exist_is_refused() {
        let errors =
            entity(&INVOICE.replace("    to: Paid\n", "    to: Payed\n")).expect_err("misspelt");
        assert!(errors.contains(ValidationCode::UnknownState), "{errors}");
        let rendered = errors.to_string();
        assert!(
            rendered.contains("`PayInvoice` moves to `Payed`"),
            "the refusal names the transition, not just the state: {rendered}"
        );
    }

    #[test]
    fn a_transition_from_a_state_that_does_not_exist_is_refused() {
        let errors = entity(&INVOICE.replace("    from: [Issued]\n", "    from: [Sent]\n"))
            .expect_err("misspelt");
        assert!(errors.contains(ValidationCode::UnknownState), "{errors}");
        assert!(
            errors
                .to_string()
                .contains("`PayInvoice` moves from `Sent`"),
            "{errors}"
        );
    }

    #[test]
    fn two_transitions_with_the_same_name_are_refused() {
        let errors = entity(&INVOICE.replace("name: PayInvoice", "name: IssueInvoice"))
            .expect_err("two moves, one name");
        assert!(
            errors.contains(ValidationCode::DuplicateTransition),
            "{errors}"
        );
        let rendered = errors.to_string();
        assert!(
            rendered.contains("a second transition named `IssueInvoice`"),
            "{rendered}"
        );
        assert!(
            rendered.contains("list every source state under the first `from`"),
            "the hint says what to do instead: {rendered}"
        );
    }

    #[test]
    fn a_state_nothing_reaches_is_refused() {
        let errors = entity(
            "\
name: billing.invoice.Invoice
identity:
  name: invoice_id
  type: billing.invoice.InvoiceId
lifecycle:
  states: [Draft, Issued, Refunded]
  initial: Draft
  terminal: [Issued]
  transitions:
    - name: IssueInvoice
      from: [Draft]
      to: Issued
    - name: RefundInvoice
      from: [Refunded]
      to: Issued
",
        )
        .expect_err("nothing moves into Refunded");
        assert_eq!(errors.len(), 1, "only reachability is wrong here: {errors}");
        assert!(
            errors.contains(ValidationCode::UnreachableState),
            "{errors}"
        );
        let rendered = errors.to_string();
        assert!(
            rendered.contains("`Refunded` cannot be reached from `Draft`"),
            "{rendered}"
        );
        assert!(
            rendered.contains("misspelt `to`"),
            "the hint names the usual cause: {rendered}"
        );
    }

    #[test]
    fn a_state_with_no_way_out_must_say_it_is_terminal() {
        let errors =
            entity(&INVOICE.replace("terminal: [Paid, Cancelled]", "terminal: [Cancelled]"))
                .expect_err("Paid is a dead end that nobody declared");
        assert!(errors.contains(ValidationCode::DeadEndState), "{errors}");
        let rendered = errors.to_string();
        assert!(
            rendered.contains("`Paid` has no outgoing transition"),
            "{rendered}"
        );
        assert!(
            rendered.contains("an entity that reaches it is stuck"),
            "{rendered}"
        );

        assert!(
            entity(INVOICE).is_ok(),
            "declaring it terminal is the fix, and it is the only difference"
        );
    }

    #[test]
    fn a_terminal_state_that_is_not_declared_is_refused() {
        let errors =
            entity(&INVOICE.replace("terminal: [Paid, Cancelled]", "terminal: [Paid, Void]"))
                .expect_err("Void is not a state");
        assert!(errors.contains(ValidationCode::UnknownState), "{errors}");
        assert!(
            !errors.contains(ValidationCode::UndeclaredReference),
            "a state reference is `unknown_state`; `undeclared_reference` is for the types, events \
             and entities a specification also names: {errors}"
        );
        assert!(
            errors
                .to_string()
                .contains("`Void` is declared terminal but is not a declared state"),
            "{errors}"
        );
        assert!(
            errors.contains(ValidationCode::DeadEndState),
            "and Cancelled, no longer terminal, is now a dead end — both are reported: {errors}"
        );
    }

    #[test]
    fn every_undeclared_state_in_one_lifecycle_is_reported_under_the_same_code() {
        let errors = entity(
            &INVOICE
                .replace(
                    "terminal: [Paid, Cancelled]",
                    "terminal: [Paid, Void, Cancelled]",
                )
                .replace("    to: Paid\n", "    to: Payed\n")
                .replace("    from: [Issued]\n", "    from: [Sent]\n"),
        )
        .expect_err("three states nobody declared");
        let states: Vec<ValidationCode> = errors
            .as_slice()
            .iter()
            .filter(|error| error.message.contains("state"))
            .map(|error| error.code)
            .collect();
        assert!(
            states
                .iter()
                .all(|code| *code == ValidationCode::UnknownState),
            "one defect class, one code: {errors}"
        );
        assert!(states.len() >= 3, "{errors}");
    }

    #[test]
    fn a_state_whose_only_transition_returns_to_it_is_a_dead_end() {
        let errors = entity(
            "\
name: billing.invoice.Invoice
identity:
  name: invoice_id
  type: billing.invoice.InvoiceId
lifecycle:
  states: [Draft, Stuck]
  initial: Draft
  transitions:
    - name: GetStuck
      from: [Draft]
      to: Stuck
    - name: StayStuck
      from: [Stuck]
      to: Stuck
",
        )
        .expect_err("an entity that reaches `Stuck` never leaves it");
        assert_eq!(errors.len(), 1, "{errors}");
        assert!(errors.contains(ValidationCode::DeadEndState), "{errors}");
        let rendered = errors.to_string();
        assert!(
            rendered.contains("`Stuck` has no transition that leaves it"),
            "a move that arrives where it started is not a way out: {rendered}"
        );
    }

    #[test]
    fn an_entity_with_no_states_is_refused() {
        let errors = entity(
            "\
name: billing.invoice.Invoice
identity:
  name: invoice_id
  type: billing.invoice.InvoiceId
lifecycle:
  states: []
  initial: Draft
",
        )
        .expect_err("no lifecycle at all");
        assert_eq!(errors.len(), 1, "{errors}");
        assert!(
            errors.contains(ValidationCode::EmptyDeclaration),
            "{errors}"
        );
        assert!(
            !errors.contains(ValidationCode::UnknownInitialState),
            "`initial` is not also reported: it adds nothing to `no state is declared` — {errors}"
        );
    }

    #[test]
    fn transition_legality_is_not_written_as_an_invariant() {
        let error = Invariant::parse("Paid cannot transition to Cancelled")
            .expect_err("a sentence is not a predicate");
        let rendered = error.to_string();
        assert!(
            rendered.contains("a predicate is either a comparison"),
            "the refusal says what an invariant is: {rendered}"
        );

        let real = Invariant::parse("total.amount >= 0").expect("a predicate");
        assert_eq!(
            real.statement, "total.amount >= 0",
            "the author's text is kept"
        );
        assert!(
            matches!(real.predicate, Predicate::Compare { .. }),
            "and it became something checkable: {:?}",
            real.predicate
        );
    }

    #[test]
    fn an_invariant_that_reads_a_field_the_entity_does_not_have_is_refused() {
        let invoice = entity(&INVOICE.replace(
            "  - total.amount >= 0\n",
            "  - total.amount >= 0\n  - discount.amount >= 0\n",
        ))
        .expect("structurally fine");
        let errors = invoice.validate(&registry());
        assert_eq!(errors.len(), 1, "{errors}");
        let error = &errors.as_slice()[0];
        assert_eq!(error.code, ValidationCode::UnobservableFact);
        assert_eq!(
            error.location,
            "entity billing.invoice.Invoice.invariants[1]"
        );
        assert!(
            error.message.contains("`discount` is not a field of"),
            "{error}"
        );
        assert!(
            error
                .hint
                .as_deref()
                .unwrap_or_default()
                .contains("customer_id, total, state"),
            "the hint lists what an invariant may name: {error}"
        );
    }

    #[test]
    fn an_invariant_that_misspells_a_nested_field_is_refused() {
        let invoice = entity(&INVOICE.replace("total.amount >= 0", "total.amont >= 0"))
            .expect("structurally fine");
        let errors = invoice.validate(&registry());
        assert!(
            errors.contains(ValidationCode::UnobservableFact),
            "{errors}"
        );
        assert!(
            errors
                .to_string()
                .contains("`billing.Money` has no field `amont`"),
            "{errors}"
        );
    }

    #[test]
    fn an_invariant_may_read_the_lifecycle_as_state() {
        let invoice = entity(&INVOICE.replace(
            "  - total.amount >= 0\n",
            "  - total.amount >= 0\n  - state == Issued\n",
        ))
        .expect("structurally fine");
        let errors = invoice.validate(&registry());
        assert!(
            errors.is_empty(),
            "`state` is not a field, and an invariant must still be able to name it: {errors}"
        );
    }

    #[test]
    fn an_invariant_may_read_the_identity() {
        let invoice = entity(&INVOICE.replace(
            "  - total.amount >= 0\n",
            "  - total.amount >= 0\n  - defined(invoice_id)\n",
        ))
        .expect("structurally fine");
        let errors = invoice.validate(&registry());
        assert!(
            errors.is_empty(),
            "the entity declares `invoice_id` as its identity two lines above, and a view may \
             project it: {errors}"
        );
    }

    #[test]
    fn an_implication_is_written_in_the_structured_form() {
        // §4.7 writes `state == Issued implies customer_id exists`. The expression form has no
        // `implies`; the mapping form spells the same thing and round-trips.
        let yaml = INVOICE.replace(
            "  - total.amount >= 0\n",
            "  - total.amount >= 0\n  - any:\n      - not: state == Issued\n      - defined(customer_id)\n",
        );
        let invoice = entity(&yaml).expect("structurally fine");
        let errors = invoice.validate(&registry());
        assert!(errors.is_empty(), "{errors}");

        let implication = &invoice.invariants[1];
        assert!(
            matches!(implication.predicate, Predicate::Any(_)),
            "{:?}",
            implication.predicate
        );

        let round_tripped: RawEntitySpec =
            serde_yaml::from_str(&serde_yaml::to_string(&invoice).expect("serialises"))
                .expect("parses");
        let round_tripped = EntitySpec::try_from(round_tripped).expect("valid");
        assert_eq!(
            round_tripped.invariants[1].predicate, implication.predicate,
            "a structured invariant survives serialisation, where its rendered text would not"
        );
    }

    #[test]
    fn an_unresolved_identity_or_field_type_is_refused() {
        let invoice = entity(&INVOICE.replace("type: billing.Money", "type: billing.Cash"))
            .expect("structurally fine");
        let mut empty = TypeRegistry::new();
        empty
            .insert(NamedType {
                name: name("billing.CustomerId"),
                body: TypeBody::Newtype {
                    of: TypeRef::Primitive(Primitive::Uuid),
                    invariants: Vec::new(),
                },
                naming: Naming::default(),
            })
            .expect("new");

        let errors = invoice.validate(&empty);
        let rendered = errors.to_string();
        assert!(
            rendered.contains("billing.invoice.InvoiceId"),
            "the identity type is resolved too: {rendered}"
        );
        assert!(rendered.contains("billing.Cash"), "{rendered}");
        assert!(
            errors.len() >= 2,
            "validation accumulates rather than stopping at the identity: {errors}"
        );
        assert!(
            errors
                .as_slice()
                .iter()
                .any(|error| error.location == "entity billing.invoice.Invoice.identity"),
            "{errors}"
        );
    }

    #[test]
    fn an_entity_that_declares_the_same_field_twice_is_refused() {
        let errors = entity(&INVOICE.replace("name: customer_id", "name: total"))
            .expect_err("two fields, one name");
        assert!(
            errors.contains(ValidationCode::DuplicateDeclaration),
            "{errors}"
        );
        assert!(
            errors.to_string().contains("`total` is declared 2 times"),
            "{errors}"
        );
    }

    #[test]
    fn a_field_that_shadows_the_identity_is_refused() {
        let errors = entity(&INVOICE.replace(
            "fields:\n",
            "fields:\n  - name: invoice_id\n    type: billing.invoice.Email\n",
        ))
        .expect_err("`invoice_id` would be observable twice, with two types");
        assert!(
            errors.contains(ValidationCode::DuplicateDeclaration),
            "{errors}"
        );
        assert!(
            errors
                .to_string()
                .contains("`invoice_id` is already the entity's identity"),
            "{errors}"
        );
    }

    #[test]
    fn an_entity_round_trips_through_yaml() {
        let invoice = entity(INVOICE).expect("valid");
        let rendered = serde_yaml::to_string(&invoice).expect("serialises");
        assert!(
            rendered.contains("lifecycle:") && rendered.contains("initial: Draft"),
            "the lifecycle round-trips whole: {rendered}"
        );

        let raw: RawEntitySpec = serde_yaml::from_str(&rendered).expect("parses back");
        let again = EntitySpec::try_from(raw).expect("still valid");
        assert_eq!(again, invoice);
    }

    #[test]
    fn a_malformed_transition_name_is_refused_when_the_document_is_read() {
        let error = serde_yaml::from_str::<RawEntitySpec>(
            &INVOICE.replace("name: PayInvoice", "name: Pay Invoice"),
        )
        .expect_err("a transition name is one qualified-name segment");
        assert!(error.to_string().contains("Pay Invoice"), "{error}");

        let dotted = serde_yaml::from_str::<RawEntitySpec>(
            &INVOICE.replace("name: PayInvoice", "name: invoice.PayInvoice"),
        )
        .expect_err("already qualified");
        assert!(
            dotted.to_string().contains("single segment"),
            "the namespace comes from the entity: {dotted}"
        );
    }

    #[test]
    fn a_key_the_model_does_not_know_is_refused() {
        let error = serde_yaml::from_str::<RawEntitySpec>(&format!("{INVOICE}stats: [Draft]\n"))
            .expect_err("`stats` is nothing");
        assert!(error.to_string().contains("stats"), "{error}");
    }
    // ---- the link between a command's outcomes and the moves they take ------------------------

    /// A command whose one outcome does `effect` to `entity`, and emits nothing.
    ///
    /// Deliberately built field by field rather than parsed: these tests are about
    /// [`validate_lifecycle_causes`], and routing them through a document would also exercise every
    /// other rule a command has to satisfy.
    fn driver(
        command: &str,
        outcome: &str,
        subject: Option<crate::command::Subject>,
    ) -> crate::command::CommandSpec {
        crate::command::CommandSpec {
            name: name(command),
            input: Vec::new(),
            outcomes: vec![crate::command::Outcome {
                name: crate::command::OutcomeName::new(outcome).expect("a valid outcome name"),
                condition: crate::command::OutcomeCondition::Otherwise,
                subject,
                emits: vec![name("billing.invoice.Moved")],
                error: None,
                summary: None,
            }],
            naming: Naming::default(),
        }
    }

    /// A refusal that also claims a subject, which is the one thing a refusal may not do.
    fn refusing_driver(
        command: &str,
        subject: crate::command::Subject,
    ) -> crate::command::CommandSpec {
        let mut spec = driver(command, "rejected", Some(subject));
        spec.outcomes[0].emits.clear();
        spec.outcomes[0].error = Some(name("billing.invoice.InvalidAmount"));
        spec
    }

    fn lifecycle(entity: EntitySpec) -> BTreeMap<QualifiedName, EntitySpec> {
        [(entity.name.clone(), entity)].into_iter().collect()
    }

    fn commands(
        specs: Vec<crate::command::CommandSpec>,
    ) -> BTreeMap<QualifiedName, crate::command::CommandSpec> {
        specs
            .into_iter()
            .map(|spec| (spec.name.clone(), spec))
            .collect()
    }

    #[test]
    fn a_transition_no_command_outcome_takes_is_refused_as_uncaused() {
        // The rule gate G14 exists for: `Issued -> Paid` is a state change the specification
        // promises, and a promise nothing can keep is the lifecycle's version of a type no value
        // can inhabit. Two of the three moves are driven here, so the message must name the third
        // and only the third.
        let invoice = entity(INVOICE).expect("§4.7's Invoice");
        let errors = validate_lifecycle_causes(
            &lifecycle(invoice),
            &commands(vec![
                driver(
                    "billing.invoice.Issue",
                    "issued",
                    Some(crate::command::Subject::moves(
                        name("billing.invoice.Invoice"),
                        "IssueInvoice",
                    )),
                ),
                driver(
                    "billing.invoice.Cancel",
                    "cancelled",
                    Some(crate::command::Subject::moves(
                        name("billing.invoice.Invoice"),
                        "CancelInvoice",
                    )),
                ),
            ]),
        );

        let uncaused: Vec<&ValidationError> = errors
            .as_slice()
            .iter()
            .filter(|error| error.code == ValidationCode::MissingCausation)
            .collect();
        assert_eq!(uncaused.len(), 1, "{errors}");
        assert!(
            uncaused[0].message.contains("PayInvoice"),
            "the uncaused move is named, not merely counted: {errors}"
        );
        assert_eq!(
            uncaused[0].location, "entity billing.invoice.Invoice.transitions[1]",
            "a refusal points at the declaration to edit: {errors}"
        );
    }

    #[test]
    fn every_transition_with_a_command_that_takes_it_is_accepted() {
        let invoice = entity(INVOICE).expect("§4.7's Invoice");
        let errors = validate_lifecycle_causes(
            &lifecycle(invoice),
            &commands(vec![
                driver(
                    "billing.invoice.Issue",
                    "issued",
                    Some(crate::command::Subject::moves(
                        name("billing.invoice.Invoice"),
                        "IssueInvoice",
                    )),
                ),
                driver(
                    "billing.invoice.Pay",
                    "settled",
                    Some(crate::command::Subject::moves(
                        name("billing.invoice.Invoice"),
                        "PayInvoice",
                    )),
                ),
                driver(
                    "billing.invoice.Cancel",
                    "cancelled",
                    Some(crate::command::Subject::moves(
                        name("billing.invoice.Invoice"),
                        "CancelInvoice",
                    )),
                ),
            ]),
        );
        assert!(errors.is_empty(), "{errors}");
    }

    #[test]
    fn an_outcome_that_moves_an_entity_nobody_declares_is_refused_as_a_dangling_reference() {
        let invoice = entity(INVOICE).expect("§4.7's Invoice");
        let errors = validate_lifecycle_causes(
            &lifecycle(invoice),
            &commands(vec![driver(
                "billing.invoice.Issue",
                "issued",
                Some(crate::command::Subject::moves(
                    name("billing.invoice.Receipt"),
                    "IssueInvoice",
                )),
            )]),
        );
        let dangling = errors
            .as_slice()
            .iter()
            .find(|error| error.code == ValidationCode::UndeclaredReference)
            .unwrap_or_else(|| panic!("an entity nothing declares: {errors}"));
        assert!(
            dangling.message.contains("billing.invoice.Receipt"),
            "{errors}"
        );
        assert_eq!(
            dangling.location,
            "command.billing.invoice.Issue.outcomes.issued.moves"
        );
    }

    #[test]
    fn an_outcome_that_takes_a_move_the_entity_does_not_declare_is_refused() {
        // The typo case, and the reason the transition side of the link is resolved rather than
        // trusted: `settle` is not `PayInvoice`, and a specification that let it through would
        // generate a scenario for a move nothing in the lifecycle draws.
        let invoice = entity(INVOICE).expect("§4.7's Invoice");
        let errors = validate_lifecycle_causes(
            &lifecycle(invoice),
            &commands(vec![driver(
                "billing.invoice.Pay",
                "settled",
                Some(crate::command::Subject::moves(
                    name("billing.invoice.Invoice"),
                    "settle",
                )),
            )]),
        );
        let misspelt = errors
            .as_slice()
            .iter()
            .find(|error| error.code == ValidationCode::UndeclaredReference)
            .unwrap_or_else(|| panic!("a move the lifecycle does not declare: {errors}"));
        assert!(
            misspelt
                .hint
                .as_deref()
                .is_some_and(|hint| hint.contains("PayInvoice")),
            "the hint lists what the entity does declare: {errors}"
        );
    }

    #[test]
    fn a_refusals_subject_does_not_count_as_taking_the_move_it_claims() {
        // Both facts in one run: the refusal must lose its subject, and the move it claimed still
        // has nothing that takes it. Counting it would have hidden the second until the first was
        // fixed.
        let invoice = entity(INVOICE).expect("§4.7's Invoice");
        let errors = validate_lifecycle_causes(
            &lifecycle(invoice),
            &commands(vec![refusing_driver(
                "billing.invoice.Pay",
                crate::command::Subject::moves(name("billing.invoice.Invoice"), "PayInvoice"),
            )]),
        );
        assert!(
            errors
                .as_slice()
                .iter()
                .any(|error| error.code == ValidationCode::MissingCausation
                    && error.message.contains("PayInvoice")),
            "a refusal is not a cause: {errors}"
        );
    }

    #[test]
    fn a_creation_is_not_read_as_taking_any_move() {
        // `creates` has no transition, so it satisfies no transition's need for a cause: an entity
        // whose only command creates it still has three uncaused moves.
        let invoice = entity(INVOICE).expect("§4.7's Invoice");
        let errors = validate_lifecycle_causes(
            &lifecycle(invoice),
            &commands(vec![driver(
                "billing.invoice.Create",
                "accepted",
                Some(crate::command::Subject::creates(name(
                    "billing.invoice.Invoice",
                ))),
            )]),
        );
        assert_eq!(
            errors
                .as_slice()
                .iter()
                .filter(|error| error.code == ValidationCode::MissingCausation)
                .count(),
            3,
            "{errors}"
        );
    }

    #[test]
    fn an_entity_with_no_transitions_needs_no_command_at_all() {
        // The other half of "required on the transition side, optional on the outcome side": an
        // entity that never moves is not asked to name a mover, so a specification of pure
        // reference data is not refused for being small.
        let settled = entity(
            "\
name: billing.invoice.Ledger
identity:
  name: ledger_id
  type: billing.invoice.InvoiceId
lifecycle:
  states: [Open]
  initial: Open
  terminal: [Open]
",
        )
        .expect("a one-state entity");
        let errors = validate_lifecycle_causes(&lifecycle(settled), &BTreeMap::new());
        assert!(errors.is_empty(), "{errors}");
    }
}
