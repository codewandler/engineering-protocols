//! Commands, the outcomes they can have, the events they emit and the errors they name.
//!
//! # A command has more than one outcome
//!
//! The design's §4.4 gives a command `preconditions` and `emits`, which is one outcome, and §13
//! then derives `CreateInvoice → expect InvoiceCreated`. Any command with a precondition has at
//! least two results, so a suite generated from that shape asserts only the happy one — a wall of
//! green tests that says nothing about the branch where the money does not move. §17 promises
//! conformance validates *rejected commands* and *specified error behaviour*; neither is expressible
//! until outcome is the unit `emits` hangs off.
//!
//! So [`CommandSpec`] carries [`Outcome`]s, and each outcome says *when* it is taken:
//!
//! | [`OutcomeCondition`] | meaning | how a generated scenario reaches it |
//! |---|---|---|
//! | [`When`](OutcomeCondition::When) | a predicate over the input holds | construct an input satisfying the predicate |
//! | [`Otherwise`](OutcomeCondition::Otherwise) | no conditional outcome matched | construct an input that matches no other branch |
//! | [`External`](OutcomeCondition::External) | something outside the input decided it | inject the fault; no input can produce it |
//! | [`WrongState`](OutcomeCondition::WrongState) | the subject is resting in a state none of this command's moves start from | drive an instance to such a state, then issue the command |
//!
//! The third exists because a provider refusing an address is not a fact about the input. Folding it
//! into `when: false` says the branch is unreachable, which is a lie a generator acts on: it either
//! skips the branch silently or emits a test nobody can make pass. [`Outcome::test_strategy`] is the
//! model's answer to that question, so no generator has to guess it.
//!
//! # The wrong state is a branch, and the author writes only the error
//!
//! `IssueInvoice` on an invoice that is already `Paid` is refused by every correct implementation,
//! and until `wrong_state:` existed the specification said nothing about it: the command declared
//! one outcome, `issued`, so a generated suite could require only that nothing happened. Design §19
//! asks for more than that — "the exact rejection mechanism must come from the declared
//! command/error semantics", and "do not generate vague *operation fails* tests if the domain
//! declares a specific error" — and there was no declaration to come from.
//!
//! **The states are not written down, and writing them would be a defect.** A transition already
//! declares the states it may start from, so the states a command refuses in are `states` minus the
//! union of its moves' `from` sets — computed, never authored. `StateMachine::can_move` says the
//! same thing from the other side and explains why there is no `forbids` counterpart: a rule
//! restating an absence is a second copy of one fact, and nothing keeps the copy honest. So a
//! `wrong_state:` branch names no state, no transition and no entity; it carries the one thing the
//! lifecycle cannot imply, which is **which declared error the refusal reports**.
//!
//! One error for every wrong state, deliberately. A command refusing because its subject is
//! `Cancelled` rather than `Paid` is the same refusal with a different reading, and the model does
//! not yet have a construct for a per-state message. The shape leaves room for one: a second
//! `wrong_state:` branch narrowed to some states would stand to this one exactly as `when:` stands
//! to `otherwise:`, and the rule that refuses two of them today
//! ([`ConflictingDeclaration`](ValidationCode::ConflictingDeclaration)) is what would be relaxed to
//! admit it. Nothing here has to be unwritten first.
//!
//! It is a branch rather than a key on [`CommandSpec`] because everything a suite, a page and an
//! HTTP response need of it is what they already need of a branch: a name to report, an `error:` to
//! carry, and a `summary:` to print. A scalar on the command would have been a fourth place a
//! projection has to look for an error, and a conformance target reporting *which branch it took*
//! would still have had nothing to name.
//!
//! # An outcome says what it changes, and a command does not
//!
//! [`Outcome::subject`] names the entity a branch acts on and the verb it acts with — `creates`,
//! `moves` along a declared transition, or `updates`. It hangs off the *outcome* and not off the
//! [`CommandSpec`], because §10 of the conformance design generates one scenario per outcome and a
//! command's branches do not agree about what they change: `CreateInvoice` creates an invoice on
//! `accepted` and creates nothing on `rejected`. A subject on the command would attach a state
//! change to the refusal, which the model refuses outright — see
//! [`RefusalMutatedState`](ValidationCode::RefusalMutatedState) below.
//!
//! The link is optional here and required on the other side. `billing.email.SendEmail` changes no
//! entity and says so by writing none of the three keys; a *transition* that no outcome performs is
//! refused by [`validate_lifecycle_causes`](crate::entity::validate_lifecycle_causes), because a
//! move nothing can trigger is the lifecycle's version of the type no value can inhabit.
//!
//! # An outcome says *which* instance, and the model does not guess
//!
//! A verb alone says an invoice moved. It does not say *which* invoice, and a conformance scenario
//! that cannot say which instance it acts on cannot be written at all — a fabricated identity fails
//! a correct implementation, which is worse than generating nothing. So a [`Subject`] carries
//! [`instance`](Subject::instance) beside the verb, naming the field that carries the identity, and
//! [`Subject::surface`] says which surface that field belongs to:
//!
//! | verb | `instance:` names | because |
//! |---|---|---|
//! | `creates:` | a field of an event the branch **emits** | the instance did not exist when the command was issued, so the caller could not have named it; what the specification can say is where the new identity becomes observable |
//! | `moves:` | a field of the command's **input** | the caller has to say which instance to move |
//! | `updates:` | a field of the command's **input** | likewise |
//!
//! One key, and the verb beside it decides the surface — so there is no lookup order to remember and
//! no ambiguity to resolve. The field's type must be the entity's identity type, which is what stops
//! `instance: customer_email` naming an invoice.
//!
//! **Declared, not inferred.** "The input field whose type is the identity's type" is cheap and
//! wrong in both directions: a command that takes two of them — a transfer between two accounts, an
//! order whose `contact` and `alternate_contact` are the same type on purpose in
//! `examples/oracle-fixture/` — has no answer, and a command that takes none has no answer either.
//! Worse, it makes the link move when nothing about it changed: adding an unrelated second field of
//! the identity's type to a command's input would take a scenario id out of the suite, and stable
//! scenario ids are what a stored conformance result is matched against. The precedent is already in
//! this model — an entity's `identity:` carries a *name* as well as a type, decided in wave 1 for
//! exactly this reason.
//!
//! The cost is one word per state-changing branch, and only there: a refusal declares no subject
//! (invariant 15), and a command that changes no entity declares none either.
//!
//! # Events carry no transport
//!
//! [`EventSpec`] has a name and fields, and deliberately nothing else. A topic, a partition key, a
//! retention window and a serialisation format are all *projections* of the fact onto one way of
//! carrying it; putting any of them here makes the same specification compile to exactly one
//! deployment shape. `InvoiceCreated` is what happened. `invoices.created.v1` is where somebody
//! chose to put it, and [`Naming::wire`](crate::name::Naming::wire) is where that choice lives.
//!
//! # What each rejection is called
//!
//! [`ValidationCode`] belongs to `aep-domain` and is closed to this crate, so ESS reuses the nearest
//! protocol code rather than opening a parallel vocabulary. The mapping is deliberate and stable:
//!
//! | rule | code |
//! |---|---|
//! | a command declares no outcomes | [`EmptyDeclaration`](ValidationCode::EmptyDeclaration) |
//! | every outcome is decided outside the input | [`UnreachableBranch`](ValidationCode::UnreachableBranch) |
//! | no outcome catches the input every `when` missed | [`NonExhaustiveBranches`](ValidationCode::NonExhaustiveBranches) |
//! | two outcomes are unconditional | [`ConflictingDeclaration`](ValidationCode::ConflictingDeclaration) |
//! | an outcome neither emits nor errors | [`EmptyChange`](ValidationCode::EmptyChange) |
//! | an outcome names an error *and* emits, or names an error *and* declares a subject | [`RefusalMutatedState`](ValidationCode::RefusalMutatedState) |
//! | an external outcome states no cause | [`UnexplainedDecision`](ValidationCode::UnexplainedDecision) |
//! | a `wrong_state` outcome names no error | [`MissingDeclaration`](ValidationCode::MissingDeclaration) |
//! | a command declares two `wrong_state` outcomes | [`ConflictingDeclaration`](ValidationCode::ConflictingDeclaration) |
//! | a `wrong_state` outcome on a command with no state to be wrong in | [`UnreachableBranch`](ValidationCode::UnreachableBranch) |
//! | an outcome is both conditional and external, is `wrong_state` and either, or declares two of `creates`/`moves`/`updates` | [`ConflictingDeclaration`](ValidationCode::ConflictingDeclaration) |
//! | a `moves:` names no entity, only a bare transition | [`MissingDeclaration`](ValidationCode::MissingDeclaration) |
//! | an outcome declares a subject and no `instance:`, or an `instance:` and no subject | [`MissingDeclaration`](ValidationCode::MissingDeclaration) |
//! | an `instance:` names no field of the surface its verb decides | [`UndeclaredReference`](ValidationCode::UndeclaredReference) |
//! | an `instance:` names a field that is not typed as the entity's identity | [`TypeMismatch`](ValidationCode::TypeMismatch) |
//! | a `when` reads the field the outcome's `instance:` names | [`UnobservableFact`](ValidationCode::UnobservableFact) |
//! | an outcome acts on an entity nothing declares, or takes a transition its subject does not declare | [`UndeclaredReference`](ValidationCode::UndeclaredReference) |
//! | a declared transition no outcome performs | [`MissingCausation`](ValidationCode::MissingCausation) |
//! | a `when` reads something that is not an input field | [`UnobservableFact`](ValidationCode::UnobservableFact) |
//! | an emitted event, a named error or a field's type is not declared | [`UndeclaredReference`](ValidationCode::UndeclaredReference) |
//! | a name is declared twice | [`DuplicateDeclaration`](ValidationCode::DuplicateDeclaration) |
//!
//! The two rules about the outcome set as a whole get their own codes rather than borrowing one that
//! is merely close. [`DeadEndState`](ValidationCode::DeadEndState) is what a wedged entity lifecycle
//! emits, so a consumer matching it on a command would read "an entity is stuck" off "a command has
//! no catch-all"; [`EmptyDeclaration`](ValidationCode::EmptyDeclaration) already means a command
//! that declares no outcomes at all. Three different mistakes, three different repairs.
//!
//! [`MissingCausation`](ValidationCode::MissingCausation) is the one code borrowed for a subject the
//! protocol half never had. AEP emits it when an event a command caused does not name that command;
//! ESS emits it when a transition a command must cause is caused by nothing. Both are the same
//! sentence — *this effect has no cause* — and the location tells the two apart, as it already does
//! for [`UnknownState`](ValidationCode::UnknownState), which an AEP workflow and an ESS lifecycle
//! have shared since wave 1.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::str::FromStr;

use aep_domain::error::{ParseError, ValidationCode, ValidationError, ValidationErrors};
use aep_domain::predicate::Predicate;

use crate::name::{Naming, QualifiedName};
use crate::types::{Field, TypeRegistry};

/// The name of one outcome of a command, such as `accepted`, `rejected` or `not-found`.
///
/// Lower-kebab, because this name becomes a generated scenario name, a generated test function and a
/// heading in generated documentation. One spelling here is one spelling in all three; two spellings
/// is a rename nobody notices until a conformance result stops matching its history.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
#[serde(into = "String")]
pub struct OutcomeName(String);

impl OutcomeName {
    /// The pattern published in generated JSON Schema.
    pub const PATTERN: &'static str = "^[a-z][a-z0-9]*(-[a-z0-9]+)*$";

    /// Parses an outcome name.
    pub fn new(value: impl AsRef<str>) -> Result<Self, ParseError> {
        let value = value.as_ref();
        let reject = |reason: String| Err(ParseError::identifier("outcome", value, reason));

        if value.is_empty() {
            return reject("must not be empty".to_owned());
        }
        if !value.starts_with(|character: char| character.is_ascii_lowercase()) {
            return reject(
                "must start with a lower-case letter; outcome names are lower-kebab, as in \
                 `not-found`"
                    .to_owned(),
            );
        }
        if value.ends_with('-') {
            return reject("must not end with a hyphen".to_owned());
        }
        for (index, character) in value.char_indices() {
            if character == '-' {
                // `index` is never zero: the first character was checked to be a letter above.
                if value.as_bytes()[index - 1] == b'-' {
                    return reject("has an empty segment; hyphens must not repeat".to_owned());
                }
                continue;
            }
            if !(character.is_ascii_lowercase() || character.is_ascii_digit()) {
                return reject(format!(
                    "contains {character:?}; outcome names are lower-kebab, as in `not-found`"
                ));
            }
        }
        Ok(Self(value.to_owned()))
    }

    /// The name as written.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for OutcomeName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl fmt::Debug for OutcomeName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "OutcomeName({})", self.0)
    }
}

impl FromStr for OutcomeName {
    type Err = ParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

impl From<OutcomeName> for String {
    fn from(value: OutcomeName) -> Self {
        value.0
    }
}

impl<'de> serde::Deserialize<'de> for OutcomeName {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        Self::new(raw).map_err(serde::de::Error::custom)
    }
}

impl schemars::JsonSchema for OutcomeName {
    fn schema_name() -> String {
        "OutcomeName".to_owned()
    }

    fn json_schema(_: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        let mut schema = schemars::schema::SchemaObject {
            instance_type: Some(schemars::schema::InstanceType::String.into()),
            ..Default::default()
        };
        schema.string().pattern = Some(Self::PATTERN.to_owned());
        schema.metadata().description =
            Some("The lower-kebab name of one outcome, such as `accepted`.".to_owned());
        schema.into()
    }
}

/// What decides that an outcome is the one taken.
///
/// Three cases rather than an `Option<Predicate>`, because a generated conformance scenario has to
/// treat them differently — see the table in the [module documentation](self).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutcomeCondition {
    /// Taken when this predicate over the command's input holds.
    When(Predicate),
    /// The default branch: taken when no conditional outcome matched. At most one per command, and
    /// at least one, so that no input falls through every branch unspecified.
    Otherwise,
    /// Determined by something outside the input: a provider refuses, a network fails, a downstream
    /// service is down.
    ///
    /// No predicate over the input can decide it, so a generated test must *inject* the condition
    /// rather than construct an input that triggers it.
    External {
        /// What outside the input decides this branch, in one phrase — `the provider rejects the
        /// recipient address`. Required: "it can fail" without saying how is not something a test
        /// author can act on.
        cause: String,
    },
    /// Taken when the instance the command would move is resting in a state none of this command's
    /// declared moves start from.
    ///
    /// It carries nothing, and that is the whole design. A [`Transition`](crate::entity::Transition)
    /// already declares its `from` states, so the states this branch answers in are derivable and
    /// listing them here would be the `forbids` rule the lifecycle deliberately refuses. What the
    /// author writes is the branch's `error:`, which no lifecycle can imply — see the
    /// [module documentation](self).
    WrongState,
}

impl OutcomeCondition {
    /// The predicate this condition tests, when it tests one.
    pub fn predicate(&self) -> Option<&Predicate> {
        match self {
            Self::When(predicate) => Some(predicate),
            Self::Otherwise | Self::External { .. } | Self::WrongState => None,
        }
    }

    /// What decides this branch from outside the input, when something does.
    pub fn cause(&self) -> Option<&str> {
        match self {
            Self::External { cause } => Some(cause),
            Self::When(_) | Self::Otherwise | Self::WrongState => None,
        }
    }

    /// How a generated test has to reach this branch.
    pub fn test_strategy(&self) -> TestStrategy {
        match self {
            Self::When(_) => TestStrategy::ConstructInput,
            Self::Otherwise => TestStrategy::DefaultBranch,
            Self::External { .. } => TestStrategy::InjectFault,
            Self::WrongState => TestStrategy::ArrangeState,
        }
    }
}

/// How a generated conformance scenario reaches one outcome.
///
/// Exposed on the model rather than decided in each generator, for the same reason
/// [`ViewSpec::assertion_style`](crate::view::ViewSpec::assertion_style) is: a decision every
/// generator has to make identically belongs in one place, where it can be wrong once instead of
/// once per target.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum TestStrategy {
    /// Build an input satisfying the outcome's `when`.
    ConstructInput,
    /// Build an input that matches no other outcome's `when`.
    DefaultBranch,
    /// Fault-inject the declared cause; no input reaches this branch.
    InjectFault,
    /// Drive an instance into a state this command's moves do not start from, then issue it.
    ///
    /// The fourth strategy, and the one that is neither an input nor an injection: the branch is
    /// decided by the *subject*, so a scenario reaches it by arranging the world rather than by
    /// choosing what to send. Which states those are is a question the lifecycle answers, not the
    /// command — see [`OutcomeCondition::WrongState`].
    ArrangeState,
}

impl TestStrategy {
    /// The strategy as it appears in generated output.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ConstructInput => "construct_input",
            Self::DefaultBranch => "default_branch",
            Self::InjectFault => "inject_fault",
            Self::ArrangeState => "arrange_state",
        }
    }
}

impl fmt::Display for TestStrategy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What one outcome does to the entity it acts on.
///
/// Three verbs, because a scenario generator has to do three different things with them:
///
/// | variant | what a generated scenario does |
/// |---|---|
/// | [`Creates`](Effect::Creates) | expects an instance to exist afterwards, at the lifecycle's `initial` state |
/// | [`Moves`](Effect::Moves) | brings an instance to one of the transition's `from` states, then expects `to` |
/// | [`Updates`](Effect::Updates) | expects the instance's state to be *unchanged*, and its invariants still to hold |
///
/// Creation is not a transition and is not written as one. A [`Transition`](crate::entity::Transition)
/// has a `from`; an instance that does not yet exist has no state to move out of, so folding the two
/// together would need a phantom source state that the lifecycle never declares and every projection
/// would then have to draw.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "effect", rename_all = "snake_case")]
pub enum Effect {
    /// A new instance comes into existence, at its lifecycle's initial state.
    Creates,
    /// An existing instance moves along a declared transition, named without the entity's namespace.
    ///
    /// The name is checked against the entity's own lifecycle by
    /// [`validate_lifecycle_causes`](crate::entity::validate_lifecycle_causes) — a transition is
    /// declared inside the entity, so this is the one reference in the model that points *into*
    /// another declaration rather than at it.
    Moves {
        /// The transition's own name, such as `settle`.
        transition: String,
    },
    /// An existing instance changes without moving along its lifecycle.
    ///
    /// The variant that keeps the link honest for a command such as `RenameCustomer`: it changes an
    /// entity, so an invariant scenario has something to evaluate afterwards, and it changes no
    /// state, so a lifecycle scenario must not claim it moved one.
    Updates,
}

impl Effect {
    /// The transition this effect takes, when it takes one.
    pub fn transition(&self) -> Option<&str> {
        match self {
            Self::Moves { transition } => Some(transition.as_str()),
            Self::Creates | Self::Updates => None,
        }
    }

    /// How it reads in a sentence: `creates`, `moves`, `updates`.
    ///
    /// The word the document is written with, so a diagnostic quotes the key an author would go and
    /// edit rather than a synonym for it.
    pub fn verb(&self) -> &'static str {
        match self {
            Self::Creates => "creates",
            Self::Moves { .. } => "moves",
            Self::Updates => "updates",
        }
    }
}

/// The entity an outcome acts on, and what it does to it.
///
/// Design §19 asks for a lifecycle scenario with "a subject and a verb"; this is both. It hangs off
/// an [`Outcome`] rather than off a [`CommandSpec`] because a command has several outcomes and they
/// do not all change the same thing — `CreateInvoice` creates an invoice on `accepted` and creates
/// nothing on `rejected`. §10 generates one scenario per *outcome*, so a subject on the command
/// would attach a state change to every branch including the refusal, which invariant 15 forbids
/// outright.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Subject {
    /// The entity this outcome acts on — `billing.invoice.Invoice`.
    pub entity: QualifiedName,
    /// What it does to it.
    #[serde(flatten)]
    pub effect: Effect,
    /// Which field carries the identity of the instance, on the surface [`Subject::effect`] decides.
    ///
    /// Not optional, so the model cannot hold a subject whose instance nobody can name. See
    /// [`Subject::instance`] and the [module documentation](self) for which surface the field is
    /// read from, and why it is declared rather than found by matching the identity's type.
    pub instance: String,
}

impl Subject {
    /// An outcome that brings a new instance of `entity` into existence, whose identity the event
    /// field `instance` publishes.
    pub fn creates(entity: QualifiedName, instance: impl Into<String>) -> Self {
        Self {
            entity,
            effect: Effect::Creates,
            instance: instance.into(),
        }
    }

    /// An outcome that moves the instance named by the input field `instance` along `transition`.
    pub fn moves(
        entity: QualifiedName,
        transition: impl Into<String>,
        instance: impl Into<String>,
    ) -> Self {
        Self {
            entity,
            effect: Effect::Moves {
                transition: transition.into(),
            },
            instance: instance.into(),
        }
    }

    /// An outcome that changes the instance named by the input field `instance`, without moving it.
    pub fn updates(entity: QualifiedName, instance: impl Into<String>) -> Self {
        Self {
            entity,
            effect: Effect::Updates,
            instance: instance.into(),
        }
    }

    /// Where a scenario reads the instance's identity: the command's input, or an emitted event.
    ///
    /// A function of the verb and of nothing else, which is what keeps one key unambiguous. An
    /// outcome that `moves` or `updates` acts on an instance the caller has to *name*, so the field
    /// is an input; an outcome that `creates` one acts on an instance that did not exist when the
    /// command was issued, so the field is the one an emitted event publishes the new identity in.
    pub fn surface(&self) -> InstanceSurface {
        match self.effect {
            Effect::Creates => InstanceSurface::EmittedEvent,
            Effect::Moves { .. } | Effect::Updates => InstanceSurface::CommandInput,
        }
    }
}

/// Where the field named by [`Subject::instance`] lives.
///
/// Two surfaces, and which one applies is read off the verb rather than written a second time:
/// `creates:` publishes an identity, `moves:` and `updates:` consume one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstanceSurface {
    /// A field of the command's declared input.
    CommandInput,
    /// A field of one of the events the outcome emits.
    EmittedEvent,
}

impl InstanceSurface {
    /// How it reads in a diagnostic.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CommandInput => "input field",
            Self::EmittedEvent => "field of an emitted event",
        }
    }
}

impl fmt::Display for InstanceSurface {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---- where a payload field's value comes from ---------------------------------------------------

/// Where one field of an emitted event's payload comes from.
///
/// The model's second mapping, and deliberately the same shape as a binding's
/// [`MappingSource`](crate::binding::MappingSource): a prefix marks a reference, anything else is
/// literal text, and the reference variant is the one the type check verifies. What differs is only
/// the surface the reference reads — a binding fills a command's input *from the triggering event*,
/// an outcome fills an event's payload *from the command's input* — so the prefix is `input.` where
/// the binding's is `event.`.
///
/// It exists because of what its absence cost. An outcome could say *which* events it emits and
/// never what fills their fields, so an `InvoiceCreated` carrying an amount nobody submitted
/// contradicted nothing the model licensed — `wrong-event-payload`, the one fault in
/// `ess-conformance`'s matrix that was caught by nothing. Asserting
/// `InvoiceCreated.amount == CreateInvoice.amount` without a declaration would be a match on a
/// shared field name, which is the inference this workspace refuses everywhere else; this is the
/// declaration that licenses it.
///
/// # Per field, and optional per field
///
/// A binding must fill every required input of the command it invokes, because an unfilled input is
/// a command that cannot run. An event field with no declared source is nothing of the kind: the
/// value is the implementation's to choose — `InvoiceCreated.invoice_id` is exactly that, an
/// identity the caller cannot know — so it stays **undetermined**, a fact a synthesized suite shows
/// by asserting the field's presence and type and never its value. There is no
/// `unmapped_payload_field` refusal, and that is a decision rather than an omission.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum PayloadSource {
    /// A field of the command's declared input: `input.amount`.
    InputField {
        /// The field's name.
        field: String,
    },
    /// A value written in the outcome itself.
    ///
    /// Checked exactly as far as a binding's literal is — text, or a variant of the enum the event
    /// field is underneath, never a value with structure — and a distinct variant for the same
    /// reason: a reader can see which payload fields were verified against the input and which were
    /// taken on trust.
    Literal {
        /// The value, as written.
        value: String,
    },
}

impl PayloadSource {
    /// The prefix that marks a field of the command's input.
    pub const INPUT_PREFIX: &'static str = "input.";

    /// Reads `input.amount` as a field, anything else as a literal.
    pub fn parse(value: &str) -> Self {
        match value.strip_prefix(Self::INPUT_PREFIX) {
            Some(field) => Self::InputField {
                field: field.to_owned(),
            },
            None => Self::Literal {
                value: value.to_owned(),
            },
        }
    }
}

impl fmt::Display for PayloadSource {
    /// As the document wrote it, so a diagnostic quotes the author rather than the model.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InputField { field } => write!(f, "{}{field}", Self::INPUT_PREFIX),
            Self::Literal { value } => f.write_str(value),
        }
    }
}

/// One filled field of an emitted event's payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PayloadField {
    /// The event field being filled.
    pub target: String,
    /// Where the value comes from.
    pub source: PayloadSource,
}

/// One event's payload sources, as a document says them: one entry per line, in the order written.
///
/// A `BTreeMap` here would repeat the defect [`MappingTable`](crate::binding::MappingTable) exists
/// to catch: `serde_yaml` accepts a repeated key and keeps the last, so a document that filled one
/// field two contradictory ways would parse clean and silently lose a line. The entries stay a list
/// until [`TryFrom<RawOutcome>`] has reported any duplicate; past that point [`Outcome::payload`]
/// is keyed by field, so a duplicate is unrepresentable.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PayloadTable(pub Vec<PayloadField>);

impl<'de> serde::Deserialize<'de> for PayloadTable {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Entries;

        impl<'de> serde::de::Visitor<'de> for Entries {
            type Value = PayloadTable;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a mapping of event field to source, as in `amount: input.amount`")
            }

            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut map: A,
            ) -> Result<Self::Value, A::Error> {
                let mut entries = Vec::new();
                while let Some((target, source)) = map.next_entry::<String, String>()? {
                    entries.push(PayloadField {
                        target,
                        source: PayloadSource::parse(&source),
                    });
                }
                Ok(PayloadTable(entries))
            }
        }

        deserializer.deserialize_map(Entries)
    }
}

impl serde::Serialize for PayloadTable {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;

        let mut map = serializer.serialize_map(Some(self.0.len()))?;
        for entry in &self.0 {
            map.serialize_entry(&entry.target, &entry.source.to_string())?;
        }
        map.end()
    }
}

impl schemars::JsonSchema for PayloadTable {
    // Inlined, as `MappingTable`'s is: the list is an implementation detail of catching a repeated
    // key, not something a document can see.
    fn is_referenceable() -> bool {
        false
    }

    fn schema_name() -> String {
        "PayloadTable".to_owned()
    }

    fn json_schema(generator: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        let mut schema = schemars::schema::SchemaObject {
            instance_type: Some(schemars::schema::InstanceType::Object.into()),
            ..Default::default()
        };
        schema.object().additional_properties = Some(Box::new(generator.subschema_for::<String>()));
        schema.into()
    }
}

/// An outcome's whole `payload:` block: one table per emitted event, in the order written.
///
/// A list of pairs rather than a map, for the reason [`PayloadTable`] is: the outer keys can be
/// repeated in a document too, and the second block would silently replace the first.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PayloadDeclaration(pub Vec<(QualifiedName, PayloadTable)>);

impl PayloadDeclaration {
    /// `true` when no event is given any source.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl<'de> serde::Deserialize<'de> for PayloadDeclaration {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Entries;

        impl<'de> serde::de::Visitor<'de> for Entries {
            type Value = PayloadDeclaration;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a mapping of emitted event to its payload sources")
            }

            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut map: A,
            ) -> Result<Self::Value, A::Error> {
                let mut entries = Vec::new();
                while let Some((event, table)) = map.next_entry::<QualifiedName, PayloadTable>()? {
                    entries.push((event, table));
                }
                Ok(PayloadDeclaration(entries))
            }
        }

        deserializer.deserialize_map(Entries)
    }
}

impl serde::Serialize for PayloadDeclaration {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;

        let mut map = serializer.serialize_map(Some(self.0.len()))?;
        for (event, table) in &self.0 {
            map.serialize_entry(event, table)?;
        }
        map.end()
    }
}

impl schemars::JsonSchema for PayloadDeclaration {
    fn is_referenceable() -> bool {
        false
    }

    fn schema_name() -> String {
        "PayloadDeclaration".to_owned()
    }

    fn json_schema(generator: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        let mut schema = schemars::schema::SchemaObject {
            instance_type: Some(schemars::schema::InstanceType::Object.into()),
            ..Default::default()
        };
        schema.object().additional_properties =
            Some(Box::new(generator.subschema_for::<PayloadTable>()));
        schema.into()
    }
}

/// One thing a command can do.
///
/// An outcome is observable or it is not an outcome: it emits events, or it names an error, and a
/// branch that does neither is one no test can check.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(into = "RawOutcome")]
pub struct Outcome {
    /// What this outcome is called.
    pub name: OutcomeName,
    /// What decides that this is the outcome taken.
    pub condition: OutcomeCondition,
    /// The entity this outcome acts on, and what it does to it.
    ///
    /// Optional, and deliberately so on this side: `billing.email.SendEmail` changes no entity in
    /// the specification, and a required subject would make an author invent one. The *other* side
    /// of the link is not optional — a transition no outcome performs is refused as
    /// [`MissingCausation`](ValidationCode::MissingCausation), because a move nothing can trigger is
    /// the lifecycle's version of the type no value can inhabit.
    pub subject: Option<Subject>,
    /// The events this outcome emits, as facts, in the order they happen.
    pub emits: Vec<QualifiedName>,
    /// Where the fields of those events' payloads come from, for the fields some declaration
    /// determines.
    ///
    /// Keyed by emitted event, then by that event's field. Sparse on both levels by design — see
    /// [`PayloadSource`]: an event or a field absent here is *undetermined*, which is a fact about
    /// the specification rather than a defect in it.
    pub payload: BTreeMap<QualifiedName, BTreeMap<String, PayloadSource>>,
    /// The error this outcome reports, from the domain's declared error vocabulary.
    pub error: Option<QualifiedName>,
    /// One line for generated documentation and for the generated scenario's title.
    pub summary: Option<String>,
}

impl Outcome {
    /// An outcome taken when `predicate` holds, emitting `emits`.
    pub fn when(name: OutcomeName, predicate: Predicate, emits: Vec<QualifiedName>) -> Self {
        Self {
            name,
            condition: OutcomeCondition::When(predicate),
            subject: None,
            emits,
            payload: BTreeMap::new(),
            error: None,
            summary: None,
        }
    }

    /// The default outcome, emitting `emits`.
    pub fn otherwise(name: OutcomeName, emits: Vec<QualifiedName>) -> Self {
        Self {
            name,
            condition: OutcomeCondition::Otherwise,
            subject: None,
            emits,
            payload: BTreeMap::new(),
            error: None,
            summary: None,
        }
    }

    /// The branch a command answers when its subject is in a state none of its moves start from.
    ///
    /// Takes the error rather than accepting one later, because a wrong-state branch without one is
    /// the vague "operation fails" check design §19 refuses: the constructor cannot build the value
    /// the validator would then have to reject.
    pub fn wrong_state(name: OutcomeName, error: QualifiedName) -> Self {
        Self {
            name,
            condition: OutcomeCondition::WrongState,
            subject: None,
            emits: Vec::new(),
            payload: BTreeMap::new(),
            error: Some(error),
            summary: None,
        }
    }

    /// The same outcome, acting on `subject`.
    #[must_use]
    pub fn acting_on(mut self, subject: Subject) -> Self {
        self.subject = Some(subject);
        self
    }

    /// The same outcome, with `event`'s field `target` determined by `source`.
    #[must_use]
    pub fn determining(
        mut self,
        event: QualifiedName,
        target: impl Into<String>,
        source: PayloadSource,
    ) -> Self {
        self.payload
            .entry(event)
            .or_default()
            .insert(target.into(), source);
        self
    }

    /// `true` when nothing about the input distinguishes this outcome from any other.
    ///
    /// `when: true` counts: it is the default branch written the long way, and two of those are as
    /// non-deterministic as two outcomes with no `when` at all.
    pub fn is_unconditional(&self) -> bool {
        match &self.condition {
            OutcomeCondition::Otherwise => true,
            OutcomeCondition::When(predicate) => predicate.is_trivially_true(),
            OutcomeCondition::External { .. } | OutcomeCondition::WrongState => false,
        }
    }

    /// `true` when this outcome reports an error rather than a change.
    pub fn is_refusal(&self) -> bool {
        self.error.is_some()
    }

    /// How a generated test has to reach this outcome.
    pub fn test_strategy(&self) -> TestStrategy {
        self.condition.test_strategy()
    }

    /// `true` when some input reaches this outcome, so a test can be written by constructing one.
    ///
    /// Two strategies say no, for two different reasons: an [`External`](OutcomeCondition::External)
    /// branch is decided outside the system, and a [`WrongState`](OutcomeCondition::WrongState)
    /// branch is decided by the subject the command arrives at. Neither is a branch a caller can
    /// select by choosing what to send, which is what this question is asked for.
    pub fn is_testable_from_input(&self) -> bool {
        matches!(
            self.test_strategy(),
            TestStrategy::ConstructInput | TestStrategy::DefaultBranch
        )
    }

    /// `true` when this is the branch taken because the subject is in a state no move starts from.
    pub fn is_wrong_state(&self) -> bool {
        self.condition == OutcomeCondition::WrongState
    }
}

/// A requested state change, and everything it can result in.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(into = "RawCommandSpec")]
pub struct CommandSpec {
    /// Its stable identity.
    pub name: QualifiedName,
    /// What the caller supplies, in declaration order.
    pub input: Vec<Field>,
    /// Everything this command can result in. At least one, exactly one of them unconditional.
    pub outcomes: Vec<Outcome>,
    /// What it is called on the wire and shown as.
    pub naming: Naming,
}

impl CommandSpec {
    /// The input field with this name.
    pub fn input_field(&self, name: &str) -> Option<&Field> {
        self.input.iter().find(|field| field.name == name)
    }

    /// The outcome with this name.
    pub fn outcome(&self, name: &OutcomeName) -> Option<&Outcome> {
        self.outcomes.iter().find(|outcome| &outcome.name == name)
    }

    /// The branch taken when no conditional outcome matched.
    pub fn default_outcome(&self) -> Option<&Outcome> {
        self.outcomes
            .iter()
            .find(|outcome| outcome.is_unconditional())
    }

    /// Every event any outcome emits, in name order.
    pub fn emitted_events(&self) -> BTreeSet<&QualifiedName> {
        self.outcomes
            .iter()
            .flat_map(|outcome| &outcome.emits)
            .collect()
    }

    /// Every error any outcome names, in name order.
    pub fn named_errors(&self) -> BTreeSet<&QualifiedName> {
        self.outcomes
            .iter()
            .filter_map(|outcome| outcome.error.as_ref())
            .collect()
    }

    /// Checks everything that can be checked without knowing what else the domain declares.
    ///
    /// Run by [`TryFrom<RawCommandSpec>`], so a `CommandSpec` obtained by parsing is already
    /// coherent on its own; run again by [`validate`](Self::validate), because the fields are public
    /// and a hand-built value has not been through the conversion.
    fn validate_shape(&self) -> ValidationErrors {
        let mut errors = ValidationErrors::new();
        let at = |suffix: &str| format!("command.{}.{suffix}", self.name);

        if self.outcomes.is_empty() {
            errors.push(
                ValidationError::new(
                    ValidationCode::EmptyDeclaration,
                    at("outcomes"),
                    format!(
                        "`{}` declares no outcomes, so nothing is specified to happen when it is \
                         issued",
                        self.name
                    ),
                )
                .with_hint("give it at least one outcome that emits an event or names an error"),
            );
        }

        let (inputs, input_errors) = self.declared_input();
        errors.extend(input_errors);

        let mut seen: BTreeSet<&str> = BTreeSet::new();
        for outcome in &self.outcomes {
            if !seen.insert(outcome.name.as_str()) {
                errors.push(
                    ValidationError::new(
                        ValidationCode::DuplicateDeclaration,
                        at(&format!("outcomes.{}", outcome.name)),
                        format!(
                            "outcome `{}` is declared more than once; two branches with one name \
                             generate one scenario and lose the other",
                            outcome.name
                        ),
                    )
                    .with_hint("name the second branch after what makes it different"),
                );
            }
            errors.extend(self.validate_outcome(outcome, &inputs));
        }

        errors.extend(self.validate_branch_coverage());
        errors
    }

    /// The declared input field names, reporting any name used twice.
    fn declared_input(&self) -> (BTreeSet<&str>, ValidationErrors) {
        let mut inputs: BTreeSet<&str> = BTreeSet::new();
        let mut errors = ValidationErrors::new();
        for (index, field) in self.input.iter().enumerate() {
            if !inputs.insert(field.name.as_str()) {
                errors.push(ValidationError::new(
                    ValidationCode::DuplicateDeclaration,
                    format!("command.{}.input[{index}]", self.name),
                    format!("input field `{}` is declared more than once", field.name),
                ));
            }
        }
        (inputs, errors)
    }

    /// Checks one outcome: that it is observable, that a refusal changes nothing, and that its
    /// condition is decidable from what the caller supplied.
    fn validate_outcome(&self, outcome: &Outcome, inputs: &BTreeSet<&str>) -> ValidationErrors {
        let mut errors = ValidationErrors::new();
        let location = format!("command.{}.outcomes.{}", self.name, outcome.name);

        if outcome.emits.is_empty() && outcome.error.is_none() {
            errors.push(
                ValidationError::new(
                    ValidationCode::EmptyChange,
                    location.clone(),
                    format!(
                        "outcome `{}` neither emits an event nor names an error, so nothing \
                             about it is observable and no test can check it",
                        outcome.name
                    ),
                )
                .with_hint("emit the fact it produces, or name the error it reports"),
            );
        }

        // A refusal that also emits is two outcomes wearing one name. AEP already holds the
        // rule this mirrors: a refused command changes nothing and is still recorded, so the
        // record of a refusal cannot carry a change.
        if let Some(error) = &outcome.error {
            if !outcome.emits.is_empty() {
                errors.push(
                    ValidationError::new(
                        ValidationCode::RefusalMutatedState,
                        location.clone(),
                        format!(
                            "outcome `{}` reports `{error}` and also emits {}; a refused \
                                 command changes nothing, so this is two outcomes wearing one name",
                            outcome.name,
                            join(outcome.emits.iter())
                        ),
                    )
                    .with_hint(
                        "split it: one outcome that emits, one that errors, each with its own \
                             condition",
                    ),
                );
            }
            // The same rule read on the lifecycle. AEP's own version of it — a refused command
            // changes nothing and is still recorded — is what `AuditRecord::validate` enforces at
            // runtime; this is the specification refusing to *promise* the thing that record would
            // have to refuse.
            if let Some(subject) = &outcome.subject {
                errors.push(
                    ValidationError::new(
                        ValidationCode::RefusalMutatedState,
                        location.clone(),
                        format!(
                            "outcome `{}` reports `{error}` and also {} `{}`; a refused command \
                             changes nothing, so a refusal has no subject",
                            outcome.name,
                            subject.effect.verb(),
                            subject.entity
                        ),
                    )
                    .with_hint(
                        "drop the subject from the refusal, and declare it on the branch that \
                         succeeds",
                    ),
                );
            }
        }

        // §19: "do not generate vague *operation fails* tests if the domain declares a specific
        // error". A wrong-state branch exists precisely to name that error, and the states it
        // answers in are already declared by the transitions it does not run from — so the error is
        // the one thing the branch carries, and a branch without one carries nothing at all.
        if outcome.condition == OutcomeCondition::WrongState && outcome.error.is_none() {
            errors.push(
                ValidationError::new(
                    ValidationCode::MissingDeclaration,
                    format!("{location}.error"),
                    format!(
                        "outcome `{}` is the branch taken when the subject is in a state no move \
                         starts from, and names no error; the states are already declared and the \
                         error is the only thing this branch can add",
                        outcome.name
                    ),
                )
                .with_hint(
                    "give it `error:`, naming what the command reports when it will not act from \
                     this state",
                ),
            );
        }

        if let OutcomeCondition::External { cause } = &outcome.condition {
            if cause.trim().is_empty() {
                errors.push(
                    ValidationError::new(
                        ValidationCode::UnexplainedDecision,
                        format!("{location}.external"),
                        format!(
                            "outcome `{}` is decided outside the input but states no cause; a \
                             test runner cannot inject a fault nobody named",
                            outcome.name
                        ),
                    )
                    .with_hint("write what fails, as in `the provider rejects the address`"),
                );
            }
        }

        errors.extend(self.validate_payload_shape(outcome, inputs, &location));
        errors.extend(self.validate_guard(outcome, inputs, &location));
        errors
    }

    /// The payload's local half: each block is about an event this branch emits, and each
    /// `input.` source reads a field the caller supplies. The other half — the event's fields and
    /// the two types — needs the domain's event declarations and runs in [`validate_payloads`].
    fn validate_payload_shape(
        &self,
        outcome: &Outcome,
        inputs: &BTreeSet<&str>,
        location: &str,
    ) -> ValidationErrors {
        let mut errors = ValidationErrors::new();
        for (event, fields) in &outcome.payload {
            if !outcome.emits.contains(event) {
                errors.push(
                    ValidationError::new(
                        ValidationCode::UndeclaredReference,
                        format!("{location}.payload.{event}"),
                        format!(
                            "outcome `{}` says where `{event}`'s payload comes from and does not \
                             emit it, so the sources describe an event this branch never publishes",
                            outcome.name
                        ),
                    )
                    .with_hint(if outcome.emits.is_empty() {
                        "this branch emits nothing; a refusal has no payload to determine"
                            .to_owned()
                    } else {
                        format!("this branch emits: {}", join(outcome.emits.iter()))
                    }),
                );
            }
            for (target, source) in fields {
                let PayloadSource::InputField { field } = source else {
                    continue;
                };
                if !inputs.contains(field.as_str()) {
                    errors.push(
                        ValidationError::new(
                            ValidationCode::UndeclaredReference,
                            format!("{location}.payload.{event}.{target}"),
                            format!(
                                "`{source}` reads `{field}`, which `{}` does not declare as input",
                                self.name
                            ),
                        )
                        .with_hint(format!("declared input: {}", join(inputs.iter()))),
                    );
                }
            }
        }
        errors
    }

    /// Checks that a branch's condition reads only what the caller supplied, and never an identity.
    ///
    /// A `when` is a predicate over *this command's input* and nothing else. Only the first segment
    /// is resolved here: a deeper path such as `amount.amount` walks into a named struct, and
    /// resolving that belongs with the IR, which knows every type in the system.
    fn validate_guard(
        &self,
        outcome: &Outcome,
        inputs: &BTreeSet<&str>,
        location: &str,
    ) -> ValidationErrors {
        let mut errors = ValidationErrors::new();
        let Some(predicate) = outcome.condition.predicate() else {
            return errors;
        };

        for path in predicate.fact_paths() {
            let root = path.namespace();
            if !inputs.contains(root) {
                errors.push(
                    ValidationError::new(
                        ValidationCode::UnobservableFact,
                        format!("{location}.when"),
                        format!(
                            "`{path}` reads `{root}`, which `{}` does not declare as input; a \
                             condition on something the caller never supplied cannot be decided \
                             when the command arrives",
                            self.name
                        ),
                    )
                    .with_hint(format!("declared input: {}", join(inputs.iter()))),
                );
                continue;
            }
            // Invariant 13, read on a command's branches: an identity is opaque, so nothing may
            // decide anything by looking inside one. A branch chosen by reading the field that names
            // the instance would also be a branch a generated scenario cannot honestly reach — it
            // decides the guard against a witness id and then sends the id of the instance an
            // earlier step created, which are two different values.
            let Some(subject) = &outcome.subject else {
                continue;
            };
            if subject.surface() == InstanceSurface::CommandInput && root == subject.instance {
                errors.push(
                    ValidationError::new(
                        ValidationCode::UnobservableFact,
                        format!("{location}.when"),
                        format!(
                            "outcome `{}` is decided by `{path}`, and `{}` names the instance it \
                             acts on; an identity is opaque, so no branch may be chosen by reading \
                             one",
                            outcome.name, subject.instance
                        ),
                    )
                    .with_hint(
                        "decide the branch on what the caller supplied about the change, not on \
                         the identity of what is being changed",
                    ),
                );
            }
        }
        errors
    }

    /// Checks that exactly one branch catches the input no other branch claims, and that at least
    /// one branch is reachable by choosing an input at all.
    fn validate_branch_coverage(&self) -> ValidationErrors {
        let mut errors = ValidationErrors::new();
        let at = |suffix: &str| format!("command.{}.{suffix}", self.name);

        if self.outcomes.is_empty() {
            // Already reported as a command with no outcomes; saying it three more ways is noise.
            return errors;
        }

        let unconditional: Vec<&OutcomeName> = self
            .outcomes
            .iter()
            .filter(|outcome| outcome.is_unconditional())
            .map(|outcome| &outcome.name)
            .collect();
        let decidable_from_input = self
            .outcomes
            .iter()
            .filter(|outcome| outcome.is_testable_from_input())
            .count();

        match unconditional.len() {
            0 => errors.push(
                ValidationError::new(
                    // Not `DeadEndState`: that is what an entity whose lifecycle wedges emits, and
                    // one code for two subjects is a consumer that cannot tell which to repair.
                    ValidationCode::NonExhaustiveBranches,
                    at("outcomes"),
                    format!(
                        "every outcome of `{}` is conditional, so there is input the \
                             specification says nothing about",
                        self.name
                    ),
                )
                .with_hint(
                    "drop the `when` from the branch that catches everything else — usually \
                         the rejection",
                ),
            ),
            1 => {}
            _ => errors.push(
                ValidationError::new(
                    ValidationCode::ConflictingDeclaration,
                    at("outcomes"),
                    format!(
                        "outcomes {} are all unconditional, so the result of `{}` is not \
                             determined by its input",
                        join(unconditional.iter()),
                        self.name
                    ),
                )
                .with_hint("give all but one of them a `when`"),
            ),
        }

        if decidable_from_input == 0 {
            errors.push(
                ValidationError::new(
                    // Not `EmptyDeclaration`: this command declares outcomes, and a consumer that
                    // cannot separate "declares nothing" from "declares only faults" repairs the
                    // wrong one.
                    ValidationCode::UnreachableBranch,
                    at("outcomes"),
                    format!(
                        "every outcome of `{}` is decided outside its input, so the command has \
                         no specified behaviour a test can construct an input for",
                        self.name
                    ),
                )
                .with_hint("declare what it does when nothing goes wrong"),
            );
        }

        // One wrong-state branch, because the model has one wrong-state *condition*: two of them
        // would both be taken by the same instance in the same state, and nothing here says which
        // wins. Narrowing one of them to a set of states is the extension that would make two
        // meaningful, and it is not written yet — see the module documentation.
        let refusing: Vec<&OutcomeName> = self
            .outcomes
            .iter()
            .filter(|outcome| outcome.is_wrong_state())
            .map(|outcome| &outcome.name)
            .collect();
        if refusing.len() > 1 {
            errors.push(
                ValidationError::new(
                    ValidationCode::ConflictingDeclaration,
                    at("outcomes"),
                    format!(
                        "outcomes {} are all `wrong_state`, so `{}` declares more than one answer \
                         for one situation",
                        join(refusing.iter()),
                        self.name
                    ),
                )
                .with_hint(
                    "keep one, naming the error the command reports whenever its subject is in a \
                     state it will not act from",
                ),
            );
        }

        errors
    }

    /// Checks this command against the domain it lives in.
    ///
    /// `events` and `errors` are the domain's declared vocabularies; an outcome may only emit or
    /// report something that is in them, because a generated scenario asserting an undeclared event
    /// asserts nothing at all.
    pub fn validate(
        &self,
        types: &TypeRegistry,
        events: &BTreeSet<QualifiedName>,
        errors: &BTreeSet<QualifiedName>,
    ) -> Result<(), ValidationErrors> {
        let mut found = self.validate_shape();
        let at = |suffix: &str| format!("command.{}.{suffix}", self.name);

        for (index, field) in self.input.iter().enumerate() {
            found.extend(types.resolve(&field.type_ref, &at(&format!("input[{index}].type"))));
        }

        for outcome in &self.outcomes {
            let location = at(&format!("outcomes.{}", outcome.name));
            for event in &outcome.emits {
                if !events.contains(event) {
                    found.push(
                        ValidationError::new(
                            ValidationCode::UndeclaredReference,
                            format!("{location}.emits"),
                            format!("`{event}` is not a declared event"),
                        )
                        .with_hint(format!("declared events: {}", join(events.iter()))),
                    );
                }
            }
            if let Some(error) = &outcome.error {
                if !errors.contains(error) {
                    found.push(
                        ValidationError::new(
                            ValidationCode::UndeclaredReference,
                            format!("{location}.error"),
                            format!("`{error}` is not a declared error"),
                        )
                        .with_hint(format!("declared errors: {}", join(errors.iter()))),
                    );
                }
            }
        }

        found.into_result(())
    }
}

/// Checks every outcome's `payload:` block against the events it fills and the inputs it reads.
///
/// The cross-declaration half of the construct, mirroring [`validate_bindings`](crate::binding::validate_bindings)
/// clause for clause because the two are one rule read in two directions: a binding fills a
/// command's input from an event, an outcome fills an event's payload from a command's input, and
/// in both the target field must exist, the two types must agree or a conversion must be declared,
/// and a literal is text checked as far as text can be.
///
/// What is deliberately *not* here is an `unmapped_payload_field`: an event field with no source is
/// undetermined, not incomplete — see [`PayloadSource`].
pub fn validate_payloads(
    commands: &BTreeMap<QualifiedName, CommandSpec>,
    events: &BTreeMap<QualifiedName, EventSpec>,
    types: &TypeRegistry,
    conversions: &crate::types::ConversionRegistry,
) -> ValidationErrors {
    let mut errors = ValidationErrors::new();
    for command in commands.values() {
        for outcome in &command.outcomes {
            for (event_name, fields) in &outcome.payload {
                // An event nothing declares was already reported by `CommandSpec::validate` —
                // either the `emits:` check or the local `payload ⊆ emits` one — and resolving its
                // fields would report the same repair a second way.
                let Some(event) = events.get(event_name) else {
                    continue;
                };
                for (target, source) in fields {
                    let at = format!(
                        "command.{}.outcomes.{}.payload.{event_name}.{target}",
                        command.name, outcome.name
                    );
                    errors.extend(check_payload_entry(
                        &at,
                        command,
                        event,
                        target,
                        source,
                        types,
                        conversions,
                    ));
                }
            }
        }
    }
    errors
}

/// One payload entry: the event field it fills, the value it takes, and whether the two agree.
fn check_payload_entry(
    at: &str,
    command: &CommandSpec,
    event: &EventSpec,
    target: &str,
    source: &PayloadSource,
    types: &TypeRegistry,
    conversions: &crate::types::ConversionRegistry,
) -> ValidationErrors {
    let mut errors = ValidationErrors::new();

    let Some(filled) = event.field(target) else {
        errors.push(
            ValidationError::new(
                ValidationCode::UndeclaredReference,
                at.to_owned(),
                format!("`{target}` is not a field `{}` carries", event.name),
            )
            .with_hint(crate::binding::readable(event)),
        );
        return errors;
    };

    match source {
        PayloadSource::InputField { field } => {
            // An input nothing declares was already reported by the outcome's own shape check,
            // and the type of a field that does not exist is not a second finding.
            let Some(read) = command.input_field(field) else {
                return errors;
            };
            if conversions.permits(&read.type_ref, &filled.type_ref) {
                return errors;
            }
            errors.push(
                ValidationError::new(
                    ValidationCode::TypeMismatch,
                    at.to_owned(),
                    format!(
                        "`{}.{field}` has type `{}`, and `{}.{target}` requires `{}`; no \
                         conversion is declared",
                        command.name, read.type_ref, event.name, filled.type_ref
                    ),
                )
                .with_hint(format!(
                    "declare the crossing — `conversions: [{{from: {}, to: {}, because: …}}]` — \
                     or make the two types agree. The reason is required, because a crossing \
                     nobody explained is the silent widening this refusal exists to catch",
                    read.type_ref, filled.type_ref
                )),
            );
        }
        PayloadSource::Literal { value } => {
            errors.extend(check_payload_literal(
                at, command, event, target, filled, value, types,
            ));
        }
    }
    errors
}

/// A payload literal, against the representation of the event field it fills.
///
/// The same three guards a binding's literal gets, because the mistake is the same one wearing the
/// other prefix: a reference meant and text written. `amount: amount` names the input without its
/// prefix; `amount: inptu.amount` misspells the prefix; and a well-meant literal still has to be
/// spellable as the field's representation, which only text and an enum variant are.
fn check_payload_literal(
    at: &str,
    command: &CommandSpec,
    event: &EventSpec,
    target: &str,
    filled: &Field,
    value: &str,
    types: &TypeRegistry,
) -> ValidationErrors {
    use crate::binding::{is_field_name, near_miss, representation, Representation};

    let mut errors = ValidationErrors::new();
    let prefix = PayloadSource::INPUT_PREFIX;

    if command.input_field(value).is_some() {
        errors.push(
            ValidationError::new(
                ValidationCode::MisspelledReference,
                at.to_owned(),
                format!(
                    "`{value}` is an input of `{}` and is written here as literal text",
                    command.name
                ),
            )
            .with_hint(format!(
                "write `{prefix}{value}` to read the input; without the prefix the value is the \
                 text `{value}` itself"
            )),
        );
        return errors;
    }

    if let Some((written, rest)) = value.split_once('.') {
        let meant_input = near_miss(written, "input") && is_field_name(rest);
        // `event.amount` in a payload is the binding's prefix carried over: it reads nothing here,
        // because the event is what is being *filled*.
        let meant_event = written == "event" && is_field_name(rest);
        if meant_input || meant_event {
            errors.push(
                ValidationError::new(
                    ValidationCode::MisspelledReference,
                    at.to_owned(),
                    format!(
                        "`{value}` reads as the literal text `{value}`; a payload source reads \
                         the command's input, written `{prefix}<field>`"
                    ),
                )
                .with_hint(format!(
                    "write `{prefix}{rest}` to read the input field, or quote the text if the \
                     dot is really part of the value"
                )),
            );
            return errors;
        }
    }

    let refuse = |reason: String| {
        ValidationError::new(ValidationCode::TypeMismatch, at.to_owned(), reason).with_hint(
            format!(
                "only text and the variants of an enum can be written as a literal; take the \
                 value from an input of `{}` instead",
                command.name
            ),
        )
    };
    match representation(&filled.type_ref, types) {
        Some(Representation::Text) | None => {}
        Some(Representation::Variants(variants)) => {
            if !variants.iter().any(|variant| variant == value) {
                errors.push(
                    ValidationError::new(
                        ValidationCode::TypeMismatch,
                        at.to_owned(),
                        format!(
                            "`{value}` is not a variant of what `{}.{target}` carries",
                            event.name
                        ),
                    )
                    .with_hint(format!("variants: {}", variants.join(", "))),
                );
            }
        }
        Some(Representation::Primitive(primitive)) => {
            errors.push(refuse(format!(
                "`{}.{target}` is `{primitive}` underneath, and a literal in a payload is text",
                event.name
            )));
        }
        Some(Representation::Structured) => {
            errors.push(refuse(format!(
                "`{}.{target}` has structure, and a literal in a payload is one piece of text",
                event.name
            )));
        }
    }
    errors
}

/// An immutable fact: something that happened, named in the domain's own words.
///
/// There is nothing here about how it travels. See the [module documentation](self).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(into = "RawEventSpec")]
pub struct EventSpec {
    /// Its stable identity.
    pub name: QualifiedName,
    /// What the fact records, in declaration order.
    pub fields: Vec<Field>,
    /// What it is called on the wire and shown as.
    pub naming: Naming,
}

impl EventSpec {
    /// The field with this name.
    pub fn field(&self, name: &str) -> Option<&Field> {
        self.fields.iter().find(|field| field.name == name)
    }

    /// Checks that every field names a declared type and that no name is used twice.
    pub fn validate(&self, types: &TypeRegistry) -> Result<(), ValidationErrors> {
        let mut errors = field_shape(&self.fields, &format!("event.{}", self.name));
        for (index, field) in self.fields.iter().enumerate() {
            errors.extend(types.resolve(
                &field.type_ref,
                &format!("event.{}.fields[{index}].type", self.name),
            ));
        }
        errors.into_result(())
    }
}

/// One error in a domain's vocabulary — what an [`Outcome`] names when it refuses.
///
/// A declared vocabulary rather than free text, so that a rejection is a fact a conformance runner
/// can assert, and so that renaming one is a change the model can see.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(into = "RawErrorSpec")]
pub struct ErrorSpec {
    /// Its stable identity.
    pub name: QualifiedName,
    /// One line saying what went wrong, for the person who receives it.
    pub summary: Option<String>,
    /// What the error carries, when it carries anything beyond its name.
    pub fields: Vec<Field>,
}

impl ErrorSpec {
    /// An error with no payload.
    pub fn new(name: QualifiedName, summary: impl Into<String>) -> Self {
        Self {
            name,
            summary: Some(summary.into()),
            fields: Vec::new(),
        }
    }

    /// Checks that every field names a declared type and that no name is used twice.
    pub fn validate(&self, types: &TypeRegistry) -> Result<(), ValidationErrors> {
        let mut errors = field_shape(&self.fields, &format!("error.{}", self.name));
        for (index, field) in self.fields.iter().enumerate() {
            errors.extend(types.resolve(
                &field.type_ref,
                &format!("error.{}.fields[{index}].type", self.name),
            ));
        }
        errors.into_result(())
    }
}

/// Reports a field name used twice in one place.
fn field_shape(fields: &[Field], location: &str) -> ValidationErrors {
    let mut errors = ValidationErrors::new();
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for (index, field) in fields.iter().enumerate() {
        if !seen.insert(field.name.as_str()) {
            errors.push(ValidationError::new(
                ValidationCode::DuplicateDeclaration,
                format!("{location}.fields[{index}]"),
                format!("field `{}` is declared more than once", field.name),
            ));
        }
    }
    errors
}

/// Renders a list of names for a diagnostic, saying so when the list is empty.
fn join<T: fmt::Display>(items: impl Iterator<Item = T>) -> String {
    let rendered: Vec<String> = items.map(|item| format!("`{item}`")).collect();
    if rendered.is_empty() {
        "none are declared".to_owned()
    } else {
        rendered.join(", ")
    }
}

/// A command as written in a document, before validation.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RawCommandSpec {
    /// Its stable identity.
    pub name: QualifiedName,
    /// What the caller supplies.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub input: Vec<Field>,
    /// Everything it can result in.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub outcomes: Vec<RawOutcome>,
    /// What it is called on the wire and shown as.
    #[serde(default, skip_serializing_if = "Naming::is_empty")]
    pub naming: Naming,
}

/// One outcome as written in a document, before validation.
///
/// `when`, `external` and `wrong_state` are the three spellings of a condition; writing none of them
/// is the default branch, and writing two is refused.
///
/// `creates`, `moves` and `updates` are the three spellings of a [`Subject`], and follow the same
/// shape for the same reason: three keys an author writes at most one of, rather than one key whose
/// value is sometimes a keyword and sometimes a name. Writing none of them says the outcome changes
/// no entity, which is the honest answer for `SendEmail`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RawOutcome {
    /// What this outcome is called.
    pub name: OutcomeName,
    /// A predicate over the command's input.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub when: Option<Predicate>,
    /// What outside the input decides this branch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external: Option<String>,
    /// `true` when this is the branch taken because the subject is in a state no move starts from.
    ///
    /// A flag and not a list of states: the states are already declared, once, as the `from` sets of
    /// this command's transitions. `wrong_state: false` is the same document as leaving the key out,
    /// because that is what the word means; what makes the branch a declaration is the `error:`
    /// beside it, which is required.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub wrong_state: bool,
    /// The entity a new instance of which this outcome brings into existence.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub creates: Option<QualifiedName>,
    /// The transition this outcome takes, written as the entity's name followed by the transition's
    /// own — `billing.invoice.Invoice.settle`.
    ///
    /// One name rather than an entity and a transition written separately, because a transition is
    /// declared *inside* an entity: `billing.invoice.Invoice.State` is already spelt this way by
    /// [`EntitySpec::state_type`](crate::entity::EntitySpec::state_type), and two keys that only
    /// mean something together are two keys an author can write half of.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub moves: Option<QualifiedName>,
    /// The entity this outcome changes without moving along its lifecycle.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updates: Option<QualifiedName>,
    /// Which field carries the identity of the instance this outcome acts on.
    ///
    /// Required beside `creates`, `moves` and `updates`, and meaningless without one. The verb
    /// decides which surface the name is read from — see [`Subject::surface`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instance: Option<String>,
    /// The events it emits.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub emits: Vec<QualifiedName>,
    /// Where the emitted events' payload fields come from, for the fields the author determines.
    ///
    /// Keyed by emitted event, then by that event's field, and sparse on both levels — see
    /// [`PayloadSource`] for why an absent field is a statement rather than an omission.
    #[serde(default, skip_serializing_if = "PayloadDeclaration::is_empty")]
    pub payload: PayloadDeclaration,
    /// The error it reports.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<QualifiedName>,
    /// One line for generated documentation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
}

/// An event as written in a document, before validation.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RawEventSpec {
    /// Its stable identity.
    pub name: QualifiedName,
    /// What the fact records.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<Field>,
    /// What it is called on the wire and shown as.
    #[serde(default, skip_serializing_if = "Naming::is_empty")]
    pub naming: Naming,
}

/// An error as written in a document, before validation.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RawErrorSpec {
    /// Its stable identity.
    pub name: QualifiedName,
    /// One line saying what went wrong.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    /// What the error carries.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<Field>,
}

impl TryFrom<RawOutcome> for Outcome {
    type Error = ValidationErrors;

    fn try_from(raw: RawOutcome) -> Result<Self, Self::Error> {
        // The nearest protocol code for "two declarations contradict each other", and the location
        // is the key an author would go and edit rather than the outcome as a whole.
        let conflict = |key: &str, message: String, hint: &str| {
            ValidationErrors::from(
                ValidationError::new(
                    ValidationCode::ConflictingDeclaration,
                    format!("outcomes.{}.{key}", raw.name),
                    message,
                )
                .with_hint(hint.to_owned()),
            )
        };
        let condition = match (raw.when, raw.external, raw.wrong_state) {
            (Some(_), Some(_), _) => {
                return Err(conflict(
                    "when",
                    format!(
                        "outcome `{}` declares both a `when` predicate and an `external` cause; a \
                         branch is either decided by the input or it is not",
                        raw.name
                    ),
                    "keep `external` and drop the predicate, or the other way round",
                ));
            }
            (Some(_), None, true) => {
                return Err(conflict(
                    "when",
                    format!(
                        "outcome `{}` declares both a `when` predicate and `wrong_state`; a branch \
                         the subject's state decides is not one the input decides",
                        raw.name
                    ),
                    "keep `wrong_state` and drop the predicate, or the other way round",
                ));
            }
            (None, Some(_), true) => {
                return Err(conflict(
                    "external",
                    format!(
                        "outcome `{}` declares both an `external` cause and `wrong_state`; the \
                         subject's own state is not something outside the system",
                        raw.name
                    ),
                    "keep `wrong_state` and drop `external`, or the other way round",
                ));
            }
            (Some(predicate), None, false) => OutcomeCondition::When(predicate),
            (None, Some(cause), false) => OutcomeCondition::External { cause },
            (None, None, true) => OutcomeCondition::WrongState,
            (None, None, false) => OutcomeCondition::Otherwise,
        };
        let subject = subject_of(&raw.name, raw.creates, raw.moves, raw.updates, raw.instance)?;
        let payload = keyed_payload(&raw.name, raw.payload)?;
        Ok(Self {
            name: raw.name,
            condition,
            subject,
            emits: raw.emits,
            payload,
            error: raw.error,
            summary: raw.summary,
        })
    }
}

/// The `payload:` block keyed by event and field, or every duplicate the document wrote.
///
/// The one check that has to happen while the entries are still a list: `serde_yaml` accepts a
/// repeated key, so two sources for one field — or two blocks for one event — reach this function
/// as two entries, and keying them without looking would keep one and lose the author's conflict.
/// Accumulating rather than stopping at the first (invariant 3): a document with three duplicated
/// lines is three repairs.
fn keyed_payload(
    name: &OutcomeName,
    declared: PayloadDeclaration,
) -> Result<BTreeMap<QualifiedName, BTreeMap<String, PayloadSource>>, ValidationErrors> {
    let mut errors = ValidationErrors::new();
    let mut payload: BTreeMap<QualifiedName, BTreeMap<String, PayloadSource>> = BTreeMap::new();
    for (event, table) in declared.0 {
        if payload.contains_key(&event) {
            errors.push(
                ValidationError::new(
                    ValidationCode::DuplicateDeclaration,
                    format!("outcomes.{name}.payload.{event}"),
                    format!(
                        "`{event}` is given payload sources more than once; two blocks for one \
                         event leave nothing downstream able to tell which the author meant"
                    ),
                )
                .with_hint("merge the two blocks into one"),
            );
            continue;
        }
        let mut fields: BTreeMap<String, PayloadSource> = BTreeMap::new();
        for entry in table.0 {
            if fields.contains_key(&entry.target) {
                errors.push(
                    ValidationError::new(
                        ValidationCode::DuplicateDeclaration,
                        format!("outcomes.{name}.payload.{event}.{}", entry.target),
                        format!(
                            "`{event}.{}` is given a source more than once; the parser would keep \
                             the last line and silently drop the author's conflict",
                            entry.target
                        ),
                    )
                    .with_hint("keep the line that says where the value really comes from"),
                );
                continue;
            }
            fields.insert(entry.target, entry.source);
        }
        payload.insert(event, fields);
    }
    errors.into_result(payload)
}

/// The one subject an outcome declares, or a refusal naming the ones it declared instead.
///
/// Written out rather than folded into a tuple match because the message has to name *which* two
/// keys collided: "creates and moves" and "moves and updates" are different mistakes with different
/// repairs, and a message saying only "more than one" leaves the author to find them.
fn subject_of(
    name: &OutcomeName,
    creates: Option<QualifiedName>,
    moves: Option<QualifiedName>,
    updates: Option<QualifiedName>,
    instance: Option<String>,
) -> Result<Option<Subject>, ValidationErrors> {
    let declared: Vec<&'static str> = [
        creates.as_ref().map(|_| "creates"),
        moves.as_ref().map(|_| "moves"),
        updates.as_ref().map(|_| "updates"),
    ]
    .into_iter()
    .flatten()
    .collect();

    if declared.len() > 1 {
        return Err(ValidationError::new(
            // The same code two contradictory spellings of a condition get: each key is well
            // formed, and what is wrong is that both were written.
            ValidationCode::ConflictingDeclaration,
            format!("outcomes.{name}.{}", declared[0]),
            format!(
                "outcome `{name}` declares {}; one outcome does one thing to one entity",
                join(declared.iter().map(|key| format!("`{key}`")))
            ),
        )
        .with_hint(
            "an outcome that both creates and moves is two outcomes; split it, or say which of the \
             two this branch is",
        )
        .into());
    }

    // One key without the other is half a statement. A subject with no instance is an entity the
    // specification says a branch changes and gives no way to say *which one*, which is the gap that
    // stopped every lifecycle scenario being synthesised; an instance with no subject names a field
    // as the identity of nothing.
    let instance = match (declared.first(), instance) {
        (Some(_), Some(field)) => field,
        (Some(verb), None) => {
            return Err(ValidationError::new(
                ValidationCode::MissingDeclaration,
                format!("outcomes.{name}.{verb}"),
                format!(
                    "outcome `{name}` {verb} an entity and declares no `instance`, so nothing says \
                     which instance it acts on"
                ),
            )
            .with_hint(
                "add `instance:` naming the input field that carries the identity — or, for \
                 `creates:`, the field of an emitted event the new identity is published in",
            )
            .into());
        }
        (None, Some(field)) => {
            return Err(ValidationError::new(
                ValidationCode::MissingDeclaration,
                format!("outcomes.{name}.instance"),
                format!(
                    "outcome `{name}` declares `instance: {field}` and none of `creates`, `moves` \
                     or `updates`, so the field names the identity of nothing"
                ),
            )
            .with_hint("say what this branch does to which entity, or drop the `instance`")
            .into());
        }
        (None, None) => return Ok(None),
    };

    if let Some(entity) = creates {
        return Ok(Some(Subject::creates(entity, instance)));
    }
    if let Some(entity) = updates {
        return Ok(Some(Subject::updates(entity, instance)));
    }
    let Some(qualified) = moves else {
        unreachable!("one of the three keys is declared, and the other two were taken above")
    };
    // `billing.invoice.Invoice.settle` is the entity plus the transition's own name. A bare name
    // with no namespace names no entity at all, and reading it as one would produce a diagnostic
    // about an entity called `settle`.
    let Some(entity) = qualified.namespace() else {
        return Err(ValidationError::new(
            ValidationCode::MissingDeclaration,
            format!("outcomes.{name}.moves"),
            format!(
                "outcome `{name}` moves `{qualified}`, which names no entity; a move is written as \
                 the entity followed by the transition"
            ),
        )
        .with_hint("write it as `billing.invoice.Invoice.settle`")
        .into());
    };
    Ok(Some(Subject::moves(entity, qualified.local(), instance)))
}

impl TryFrom<RawCommandSpec> for CommandSpec {
    type Error = ValidationErrors;

    fn try_from(raw: RawCommandSpec) -> Result<Self, Self::Error> {
        let mut errors = ValidationErrors::new();
        let mut outcomes = Vec::with_capacity(raw.outcomes.len());
        for raw_outcome in raw.outcomes {
            match Outcome::try_from(raw_outcome) {
                Ok(outcome) => outcomes.push(outcome),
                Err(nested) => {
                    for mut error in nested {
                        error.location = format!("command.{}.{}", raw.name, error.location);
                        errors.push(error);
                    }
                }
            }
        }

        let spec = Self {
            name: raw.name,
            input: raw.input,
            outcomes,
            naming: raw.naming,
        };
        errors.extend(spec.validate_shape());
        errors.into_result(spec)
    }
}

impl TryFrom<RawEventSpec> for EventSpec {
    type Error = ValidationErrors;

    fn try_from(raw: RawEventSpec) -> Result<Self, Self::Error> {
        let spec = Self {
            name: raw.name,
            fields: raw.fields,
            naming: raw.naming,
        };
        field_shape(&spec.fields, &format!("event.{}", spec.name)).into_result(spec)
    }
}

impl TryFrom<RawErrorSpec> for ErrorSpec {
    type Error = ValidationErrors;

    fn try_from(raw: RawErrorSpec) -> Result<Self, Self::Error> {
        let spec = Self {
            name: raw.name,
            summary: raw.summary,
            fields: raw.fields,
        };
        field_shape(&spec.fields, &format!("error.{}", spec.name)).into_result(spec)
    }
}

impl From<Outcome> for RawOutcome {
    fn from(outcome: Outcome) -> Self {
        let (when, external, wrong_state) = match outcome.condition {
            OutcomeCondition::When(predicate) => (Some(predicate), None, false),
            OutcomeCondition::Otherwise => (None, None, false),
            OutcomeCondition::External { cause } => (None, Some(cause), false),
            OutcomeCondition::WrongState => (None, None, true),
        };
        let (creates, moves, updates, instance) = match outcome.subject {
            None => (None, None, None, None),
            Some(Subject {
                entity,
                effect: Effect::Creates,
                instance,
            }) => (Some(entity), None, None, Some(instance)),
            Some(Subject {
                entity,
                effect: Effect::Moves { transition },
                instance,
            }) => (None, Some(entity.child(&transition)), None, Some(instance)),
            Some(Subject {
                entity,
                effect: Effect::Updates,
                instance,
            }) => (None, None, Some(entity), Some(instance)),
        };
        let payload = PayloadDeclaration(
            outcome
                .payload
                .into_iter()
                .map(|(event, fields)| {
                    (
                        event,
                        PayloadTable(
                            fields
                                .into_iter()
                                .map(|(target, source)| PayloadField { target, source })
                                .collect(),
                        ),
                    )
                })
                .collect(),
        );
        Self {
            name: outcome.name,
            when,
            external,
            wrong_state,
            creates,
            moves,
            updates,
            instance,
            emits: outcome.emits,
            payload,
            error: outcome.error,
            summary: outcome.summary,
        }
    }
}

impl From<CommandSpec> for RawCommandSpec {
    fn from(command: CommandSpec) -> Self {
        Self {
            name: command.name,
            input: command.input,
            outcomes: command.outcomes.into_iter().map(RawOutcome::from).collect(),
            naming: command.naming,
        }
    }
}

impl From<EventSpec> for RawEventSpec {
    fn from(event: EventSpec) -> Self {
        Self {
            name: event.name,
            fields: event.fields,
            naming: event.naming,
        }
    }
}

impl From<ErrorSpec> for RawErrorSpec {
    fn from(error: ErrorSpec) -> Self {
        Self {
            name: error.name,
            summary: error.summary,
            fields: error.fields,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::types::{NamedType, Primitive, TypeBody, TypeRef};

    fn name(value: &str) -> QualifiedName {
        QualifiedName::new(value).expect("a valid name")
    }

    fn outcome_name(value: &str) -> OutcomeName {
        OutcomeName::new(value).expect("a valid outcome name")
    }

    /// The §31 example's types, as `examples/billing/domains/invoice.yaml` declares them.
    fn registry() -> TypeRegistry {
        let mut registry = TypeRegistry::new();
        registry
            .insert(NamedType {
                name: name("billing.invoice.Email"),
                body: TypeBody::Newtype {
                    of: TypeRef::Primitive(Primitive::String),
                    invariants: Vec::new(),
                },
                naming: Naming::default(),
            })
            .expect("new");
        registry
            .insert(NamedType {
                name: name("billing.invoice.Money"),
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

    fn events() -> BTreeSet<QualifiedName> {
        [name("billing.invoice.InvoiceCreated")].into()
    }

    fn errors() -> BTreeSet<QualifiedName> {
        [name("billing.invoice.InvalidAmount")].into()
    }

    /// `CreateInvoice` as the normative fixture writes it: one conditional acceptance, one default
    /// rejection.
    const CREATE_INVOICE: &str = r"
name: billing.invoice.CreateInvoice
input:
  - name: customer_email
    type: billing.invoice.Email
  - name: amount
    type: billing.invoice.Money
outcomes:
  - name: accepted
    when: amount.amount > 0
    emits:
      - billing.invoice.InvoiceCreated
    summary: The invoice is created in Draft.
  - name: rejected
    error: billing.invoice.InvalidAmount
    summary: The amount was not positive, and nothing was created.
";

    fn create_invoice() -> CommandSpec {
        let raw: RawCommandSpec = serde_yaml::from_str(CREATE_INVOICE).expect("parses");
        CommandSpec::try_from(raw).expect("a valid command")
    }

    fn command_with(outcomes: Vec<Outcome>) -> CommandSpec {
        CommandSpec {
            name: name("billing.invoice.CreateInvoice"),
            input: vec![Field::new(
                "amount",
                TypeRef::Named(name("billing.invoice.Money")),
            )],
            outcomes,
            naming: Naming::default(),
        }
    }

    fn refuse(command: &CommandSpec) -> ValidationErrors {
        command
            .validate(&registry(), &events(), &errors())
            .expect_err("expected a refusal")
    }

    #[test]
    fn a_command_with_an_accepted_and_a_rejected_outcome_round_trips_through_yaml() {
        let command = create_invoice();
        assert!(
            command.validate(&registry(), &events(), &errors()).is_ok(),
            "the normative example must validate"
        );

        let rendered = serde_yaml::to_string(&command).expect("serialises");
        let reparsed: RawCommandSpec = serde_yaml::from_str(&rendered).expect("re-parses");
        let round_tripped = CommandSpec::try_from(reparsed).expect("still valid");

        assert_eq!(
            round_tripped, command,
            "a validated command must survive a trip through the document form: {rendered}"
        );
        assert_eq!(command.outcomes.len(), 2, "both branches survive");
        assert_eq!(
            command
                .outcome(&outcome_name("accepted"))
                .map(Outcome::test_strategy),
            Some(TestStrategy::ConstructInput)
        );
        assert_eq!(
            command
                .default_outcome()
                .map(|outcome| outcome.name.as_str()),
            Some("rejected"),
            "the branch with no `when` is the one that catches everything else"
        );
        assert!(command
            .outcome(&outcome_name("rejected"))
            .expect("declared")
            .is_refusal());
        assert_eq!(command.emitted_events().len(), 1);
        assert_eq!(command.named_errors().len(), 1);
        assert!(command.input_field("customer_email").is_some());
    }

    #[test]
    fn a_command_with_no_outcomes_is_refused() {
        let errors = refuse(&command_with(Vec::new()));
        assert!(errors.contains(ValidationCode::EmptyDeclaration));
        assert!(
            !errors.contains(ValidationCode::EmptyChange),
            "declaring nothing is one mistake and declaring only faults is another: {errors}"
        );
        assert!(
            errors.to_string().contains("declares no outcomes"),
            "the refusal must say which rule broke: {errors}"
        );
    }

    #[test]
    fn two_unconditional_outcomes_are_refused_as_non_deterministic() {
        let errors = refuse(&command_with(vec![
            Outcome::otherwise(
                outcome_name("accepted"),
                vec![name("billing.invoice.InvoiceCreated")],
            ),
            Outcome::otherwise(
                outcome_name("also-accepted"),
                vec![name("billing.invoice.InvoiceCreated")],
            ),
        ]));

        assert!(errors.contains(ValidationCode::ConflictingDeclaration));
        let rendered = errors.to_string();
        assert!(rendered.contains("`accepted`"), "{rendered}");
        assert!(
            rendered.contains("`also-accepted`"),
            "both names, not just a count: {rendered}"
        );
        assert!(
            rendered.contains("not determined by its input"),
            "{rendered}"
        );
    }

    #[test]
    fn a_when_that_is_trivially_true_is_the_default_branch_written_the_long_way() {
        let errors = refuse(&command_with(vec![
            Outcome::when(
                outcome_name("accepted"),
                Predicate::Always,
                vec![name("billing.invoice.InvoiceCreated")],
            ),
            Outcome::otherwise(
                outcome_name("fallback"),
                vec![name("billing.invoice.InvoiceCreated")],
            ),
        ]));

        assert!(
            errors.contains(ValidationCode::ConflictingDeclaration),
            "`when: true` is unconditional, so this is two default branches: {errors}"
        );
    }

    #[test]
    fn a_command_with_no_catch_all_branch_is_not_refused_as_a_wedged_state_machine() {
        let positive = Predicate::parse_expression("amount.amount > 0").expect("parses");
        let negative = Predicate::parse_expression("amount.amount <= 0").expect("parses");
        let errors = refuse(&command_with(vec![
            Outcome::when(
                outcome_name("accepted"),
                positive,
                vec![name("billing.invoice.InvoiceCreated")],
            ),
            Outcome {
                name: outcome_name("rejected"),
                condition: OutcomeCondition::When(negative),
                subject: None,
                emits: Vec::new(),
                payload: BTreeMap::new(),
                error: Some(name("billing.invoice.InvalidAmount")),
                summary: None,
            },
        ]));

        assert!(errors.contains(ValidationCode::NonExhaustiveBranches));
        assert!(
            !errors.contains(ValidationCode::DeadEndState),
            "`dead_end_state` is an entity whose lifecycle wedges; a command missing a catch-all is \
             a different subject with a different repair: {errors}"
        );
        assert!(
            errors.to_string().contains("says nothing about"),
            "the refusal must say what is missing, not just that something is: {errors}"
        );
    }

    #[test]
    fn an_outcome_that_neither_emits_nor_errors_is_refused() {
        let errors = refuse(&command_with(vec![Outcome::otherwise(
            outcome_name("accepted"),
            Vec::new(),
        )]));

        assert!(errors.contains(ValidationCode::EmptyChange));
        assert!(
            errors
                .to_string()
                .contains("nothing about it is observable"),
            "{errors}"
        );
    }

    #[test]
    fn an_outcome_that_names_an_error_and_also_emits_is_refused() {
        let errors = refuse(&command_with(vec![Outcome {
            name: outcome_name("rejected"),
            condition: OutcomeCondition::Otherwise,
            subject: None,
            emits: vec![name("billing.invoice.InvoiceCreated")],
            payload: BTreeMap::new(),
            error: Some(name("billing.invoice.InvalidAmount")),
            summary: None,
        }]));

        assert!(errors.contains(ValidationCode::RefusalMutatedState));
        let rendered = errors.to_string();
        assert!(
            rendered.contains("a refused command changes nothing"),
            "{rendered}"
        );
        assert!(
            rendered.contains("billing.invoice.InvoiceCreated"),
            "{rendered}"
        );
    }

    #[test]
    fn an_external_outcome_must_say_what_fails() {
        let errors = refuse(&command_with(vec![
            Outcome::otherwise(
                outcome_name("sent"),
                vec![name("billing.invoice.InvoiceCreated")],
            ),
            Outcome {
                name: outcome_name("failed"),
                condition: OutcomeCondition::External {
                    cause: "  ".to_owned(),
                },
                subject: None,
                emits: Vec::new(),
                payload: BTreeMap::new(),
                error: Some(name("billing.invoice.InvalidAmount")),
                summary: None,
            },
        ]));

        assert!(errors.contains(ValidationCode::UnexplainedDecision));
        assert!(
            errors
                .to_string()
                .contains("cannot inject a fault nobody named"),
            "{errors}"
        );
    }

    #[test]
    fn a_command_whose_every_outcome_is_external_specifies_no_change_rather_than_nothing_at_all() {
        let errors = refuse(&command_with(vec![Outcome {
            name: outcome_name("failed"),
            condition: OutcomeCondition::External {
                cause: "the provider is down".to_owned(),
            },
            subject: None,
            emits: Vec::new(),
            payload: BTreeMap::new(),
            error: Some(name("billing.invoice.InvalidAmount")),
            summary: None,
        }]));

        assert!(errors.contains(ValidationCode::UnreachableBranch));
        assert!(
            !errors.contains(ValidationCode::EmptyDeclaration),
            "this command does declare an outcome; `empty_declaration` is for one that declares \
             none, and a consumer that cannot tell them apart repairs the wrong thing: {errors}"
        );
        assert!(
            errors.to_string().contains("decided outside its input"),
            "{errors}"
        );
    }

    #[test]
    fn an_outcome_cannot_be_both_conditional_and_external() {
        let raw: RawCommandSpec = serde_yaml::from_str(
            r"
name: billing.email.SendEmail
input:
  - name: recipient
    type: billing.invoice.Email
outcomes:
  - name: failed
    when: recipient == none
    external: the provider rejects the address
    error: billing.invoice.InvalidAmount
",
        )
        .expect("parses");

        let errors = CommandSpec::try_from(raw).expect_err("contradictory condition");
        assert!(errors.contains(ValidationCode::ConflictingDeclaration));
        assert!(
            errors
                .to_string()
                .contains("either decided by the input or it is not"),
            "{errors}"
        );
    }

    #[test]
    fn a_wrong_state_branch_that_names_no_error_is_refused() {
        // The one thing a `wrong_state:` branch carries. The states are already declared — a
        // transition says what it runs `from:` — so a branch without an error adds nothing at all,
        // and a generated suite would be back to the vague "operation fails" check design §19
        // refuses in as many words.
        let errors = refuse(&command_with(vec![
            Outcome::otherwise(
                outcome_name("accepted"),
                vec![name("billing.invoice.InvoiceCreated")],
            ),
            Outcome {
                name: outcome_name("wrong-state"),
                condition: OutcomeCondition::WrongState,
                subject: None,
                emits: Vec::new(),
                payload: BTreeMap::new(),
                error: None,
                summary: None,
            },
        ]));

        assert!(errors.contains(ValidationCode::MissingDeclaration));
        let rendered = errors.to_string();
        assert!(
            rendered.contains("the error is the only thing this branch can add"),
            "{rendered}"
        );
        assert!(
            rendered.contains("hint: give it `error:`"),
            "the repair is one key, and the message has to say which: {rendered}"
        );
    }

    #[test]
    fn a_command_declares_at_most_one_wrong_state_branch() {
        // Two of them are two answers to one question: an instance is in one state, and both
        // branches claim it. Narrowing one to a set of states is the extension that would make two
        // meaningful, and nothing in the model expresses it yet — so this refuses rather than
        // picking one.
        let errors = refuse(&command_with(vec![
            Outcome::otherwise(
                outcome_name("accepted"),
                vec![name("billing.invoice.InvoiceCreated")],
            ),
            Outcome::wrong_state(
                outcome_name("too-late"),
                name("billing.invoice.InvalidAmount"),
            ),
            Outcome::wrong_state(
                outcome_name("too-early"),
                name("billing.invoice.InvalidAmount"),
            ),
        ]));

        assert!(errors.contains(ValidationCode::ConflictingDeclaration));
        let rendered = errors.to_string();
        assert!(
            rendered.contains("`too-early`") && rendered.contains("`too-late`"),
            "the refusal names both branches, because the repair is to delete one of them: \
             {rendered}"
        );
    }

    #[test]
    fn a_wrong_state_branch_is_neither_the_catch_all_nor_reachable_by_choosing_an_input() {
        // The two questions branch coverage asks, answered for the fourth condition. It must not
        // count as the unconditional branch — an input that matches no `when` has to land somewhere
        // that is not "the subject was in the wrong state" — and it must not count as a branch a
        // caller can reach by choosing what to send, or a command with nothing but a `when` and a
        // `wrong_state:` would look exhaustive.
        let refusal = Outcome::wrong_state(
            outcome_name("wrong-state"),
            name("billing.invoice.InvalidAmount"),
        );
        assert!(!refusal.is_unconditional());
        assert!(!refusal.is_testable_from_input());
        assert!(refusal.is_wrong_state());
        assert_eq!(refusal.test_strategy(), TestStrategy::ArrangeState);

        let errors = refuse(&command_with(vec![
            Outcome::when(
                outcome_name("accepted"),
                Predicate::parse_expression("amount.amount > 0").expect("parses"),
                vec![name("billing.invoice.InvoiceCreated")],
            ),
            refusal,
        ]));
        assert!(
            errors.contains(ValidationCode::NonExhaustiveBranches),
            "a `when` and a `wrong_state:` leave every other input unspecified: {errors}"
        );
    }

    #[test]
    fn an_outcome_cannot_be_both_wrong_state_and_decided_by_the_input() {
        for (second, expected) in [
            (
                "    when: amount.amount > 0",
                "is not one the input decides",
            ),
            (
                "    external: the provider is down",
                "not something outside the system",
            ),
        ] {
            let raw: RawCommandSpec = serde_yaml::from_str(&format!(
                r"
name: billing.invoice.IssueInvoice
input:
  - name: amount
    type: billing.invoice.Money
outcomes:
  - name: wrong-state
    wrong_state: true
{second}
    error: billing.invoice.InvalidAmount
"
            ))
            .expect("parses");

            let errors = CommandSpec::try_from(raw).expect_err("contradictory condition");
            assert!(errors.contains(ValidationCode::ConflictingDeclaration));
            assert!(
                errors.to_string().contains(expected),
                "the message has to say which two keys collided: {errors}"
            );
        }
    }

    #[test]
    fn writing_wrong_state_false_is_the_same_document_as_leaving_the_key_out() {
        // `false` means "this is not the wrong-state branch", which is what an absent key means, so
        // there is no third state to refuse. Asserted rather than assumed: a `bool` that silently
        // meant something else in one of its two values would be a trap in the parser.
        let raw: RawCommandSpec = serde_yaml::from_str(
            r"
name: billing.invoice.IssueInvoice
outcomes:
  - name: accepted
    wrong_state: false
    emits:
      - billing.invoice.InvoiceCreated
",
        )
        .expect("parses");

        let command = CommandSpec::try_from(raw).expect("a valid command");
        assert_eq!(
            command.outcomes[0].condition,
            OutcomeCondition::Otherwise,
            "an explicit `false` is the default branch, exactly as writing no key at all is"
        );
    }

    #[test]
    fn a_wrong_state_branch_survives_a_trip_through_the_document_form() {
        let command = command_with(vec![
            Outcome::otherwise(
                outcome_name("accepted"),
                vec![name("billing.invoice.InvoiceCreated")],
            ),
            Outcome::wrong_state(
                outcome_name("wrong-state"),
                name("billing.invoice.InvalidAmount"),
            ),
        ]);

        let rendered = serde_yaml::to_string(&command).expect("serialises");
        assert!(
            rendered.contains("wrong_state: true"),
            "the key has to be written back out, or a round trip loses the branch: {rendered}"
        );
        let reparsed: RawCommandSpec = serde_yaml::from_str(&rendered).expect("re-parses");
        assert_eq!(
            CommandSpec::try_from(reparsed).expect("still valid"),
            command
        );
    }

    #[test]
    fn a_when_predicate_may_only_read_declared_input_fields() {
        let errors = refuse(&command_with(vec![
            Outcome::when(
                outcome_name("accepted"),
                Predicate::parse_expression("customer.tier == gold").expect("parses"),
                vec![name("billing.invoice.InvoiceCreated")],
            ),
            Outcome {
                name: outcome_name("rejected"),
                condition: OutcomeCondition::Otherwise,
                subject: None,
                emits: Vec::new(),
                payload: BTreeMap::new(),
                error: Some(name("billing.invoice.InvalidAmount")),
                summary: None,
            },
        ]));

        assert!(errors.contains(ValidationCode::UnobservableFact));
        let rendered = errors.to_string();
        assert!(rendered.contains("customer.tier"), "{rendered}");
        assert!(rendered.contains("does not declare as input"), "{rendered}");
        assert!(
            rendered.contains("hint: declared input: `amount`"),
            "{rendered}"
        );
    }

    #[test]
    fn an_undeclared_event_and_an_undeclared_error_are_both_reported() {
        let errors = refuse(&command_with(vec![
            Outcome::when(
                outcome_name("accepted"),
                Predicate::parse_expression("amount.amount > 0").expect("parses"),
                vec![name("billing.invoice.InvoiceIssued")],
            ),
            Outcome {
                name: outcome_name("rejected"),
                condition: OutcomeCondition::Otherwise,
                subject: None,
                emits: Vec::new(),
                payload: BTreeMap::new(),
                error: Some(name("billing.invoice.AmountTooLarge")),
                summary: None,
            },
        ]));

        assert_eq!(errors.len(), 2, "validation accumulates: {errors}");
        let rendered = errors.to_string();
        assert!(
            rendered.contains("`billing.invoice.InvoiceIssued` is not a declared event"),
            "{rendered}"
        );
        assert!(
            rendered.contains("`billing.invoice.AmountTooLarge` is not a declared error"),
            "{rendered}"
        );
        assert!(
            rendered.contains("billing.invoice.InvoiceCreated"),
            "and what was available: {rendered}"
        );
    }

    #[test]
    fn an_outcome_declared_twice_is_refused() {
        let errors = refuse(&command_with(vec![
            Outcome::otherwise(
                outcome_name("accepted"),
                vec![name("billing.invoice.InvoiceCreated")],
            ),
            Outcome::when(
                outcome_name("accepted"),
                Predicate::parse_expression("amount.amount > 0").expect("parses"),
                vec![name("billing.invoice.InvoiceCreated")],
            ),
        ]));

        assert!(errors.contains(ValidationCode::DuplicateDeclaration));
        assert!(
            errors
                .to_string()
                .contains("generate one scenario and lose the other"),
            "{errors}"
        );
    }

    #[test]
    fn an_input_field_declared_twice_is_refused() {
        let mut command = create_invoice();
        command
            .input
            .push(Field::new("amount", TypeRef::Primitive(Primitive::Decimal)));

        let errors = refuse(&command);
        assert!(errors.contains(ValidationCode::DuplicateDeclaration));
        assert!(
            errors
                .to_string()
                .contains("`amount` is declared more than once"),
            "{errors}"
        );
    }

    #[test]
    fn an_input_field_must_name_a_declared_type() {
        let mut command = create_invoice();
        command.input.push(Field::new(
            "discount",
            TypeRef::Named(name("billing.invoice.Percentage")),
        ));

        let errors = refuse(&command);
        assert!(errors.contains(ValidationCode::UndeclaredReference));
        assert!(
            errors
                .to_string()
                .contains("`billing.invoice.Percentage` is not a declared type"),
            "{errors}"
        );
    }

    #[test]
    fn outcome_names_are_lower_kebab() {
        assert_eq!(outcome_name("not-found").as_str(), "not-found");
        assert_eq!(outcome_name("accepted2").as_str(), "accepted2");

        for rejected in [
            "Accepted",
            "not_found",
            "-rejected",
            "rejected-",
            "not--found",
            "",
        ] {
            let error = OutcomeName::new(rejected)
                .expect_err("an outcome name that is not lower-kebab must be refused");
            assert!(
                error.to_string().contains("outcome"),
                "the refusal must name what was being parsed: {error}"
            );
        }
        assert!(
            OutcomeName::new("Accepted")
                .expect_err("upper case")
                .to_string()
                .contains("lower-kebab"),
            "the refusal must say what the spelling is"
        );
    }

    #[test]
    fn a_test_strategy_is_decided_by_the_model_and_not_by_a_generator() {
        let inject = Outcome {
            name: outcome_name("failed"),
            condition: OutcomeCondition::External {
                cause: "the provider rejects the address".to_owned(),
            },
            subject: None,
            emits: Vec::new(),
            payload: BTreeMap::new(),
            error: Some(name("billing.invoice.InvalidAmount")),
            summary: None,
        };
        assert_eq!(inject.test_strategy(), TestStrategy::InjectFault);
        assert_eq!(inject.test_strategy().as_str(), "inject_fault");
        assert!(
            !inject.is_testable_from_input(),
            "no input reaches a branch the provider decides"
        );
        assert_eq!(
            inject.condition.cause(),
            Some("the provider rejects the address")
        );

        let default = Outcome::otherwise(
            outcome_name("sent"),
            vec![name("billing.invoice.InvoiceCreated")],
        );
        assert_eq!(default.test_strategy(), TestStrategy::DefaultBranch);
        assert!(default.is_testable_from_input());

        let conditional = Outcome::when(
            outcome_name("accepted"),
            Predicate::parse_expression("amount.amount > 0").expect("parses"),
            vec![name("billing.invoice.InvoiceCreated")],
        );
        assert_eq!(conditional.test_strategy(), TestStrategy::ConstructInput);
        assert!(conditional.condition.predicate().is_some());
    }

    #[test]
    fn an_event_records_a_fact_and_nothing_about_how_it_travels() {
        let raw: RawEventSpec = serde_yaml::from_str(
            r"
name: billing.invoice.InvoiceCreated
fields:
  - name: amount
    type: billing.invoice.Money
naming:
  wire: invoices.created.v1
",
        )
        .expect("parses");
        let event = EventSpec::try_from(raw).expect("valid");
        assert!(event.validate(&registry()).is_ok());

        let rendered = serde_yaml::to_string(&event).expect("serialises");
        assert!(
            !rendered.contains("topic"),
            "a transport is a projection of the fact, not part of it: {rendered}"
        );
        assert_eq!(
            event.naming.wire_or(&event.name),
            "invoices.created.v1",
            "the topic lives in naming, where changing it is a wire change and not a model change"
        );
        assert!(event.field("amount").is_some());
    }

    #[test]
    fn an_event_field_must_name_a_declared_type() {
        let event = EventSpec {
            name: name("billing.invoice.InvoiceCreated"),
            fields: vec![Field::new(
                "invoice_id",
                TypeRef::Named(name("billing.invoice.InvoiceId")),
            )],
            naming: Naming::default(),
        };

        let errors = event.validate(&registry()).expect_err("undeclared type");
        assert!(errors.contains(ValidationCode::UndeclaredReference));
        assert!(
            errors
                .to_string()
                .contains("`billing.invoice.InvoiceId` is not a declared type"),
            "{errors}"
        );
    }

    #[test]
    fn an_event_field_declared_twice_is_refused() {
        let raw: RawEventSpec = serde_yaml::from_str(
            r"
name: billing.invoice.InvoiceCreated
fields:
  - name: amount
    type: billing.invoice.Money
  - name: amount
    type: Decimal
",
        )
        .expect("parses");

        let errors = EventSpec::try_from(raw).expect_err("duplicate field");
        assert!(errors.contains(ValidationCode::DuplicateDeclaration));
        assert!(
            errors
                .to_string()
                .contains("`amount` is declared more than once"),
            "{errors}"
        );
    }

    #[test]
    fn an_error_is_part_of_a_vocabulary_an_outcome_can_name() {
        let raw: RawErrorSpec = serde_yaml::from_str(
            r"
name: billing.invoice.InvalidAmount
summary: The requested amount is not positive.
",
        )
        .expect("parses");
        let declared = ErrorSpec::try_from(raw).expect("valid");
        assert!(declared.validate(&registry()).is_ok());
        assert_eq!(
            declared.summary.as_deref(),
            Some("The requested amount is not positive.")
        );

        let vocabulary: BTreeSet<QualifiedName> = [declared.name.clone()].into();
        let command = create_invoice();
        assert!(
            command
                .validate(&registry(), &events(), &vocabulary)
                .is_ok(),
            "the outcome names exactly this error"
        );

        let rendered = serde_yaml::to_string(&declared).expect("serialises");
        let reparsed: RawErrorSpec = serde_yaml::from_str(&rendered).expect("re-parses");
        assert_eq!(ErrorSpec::try_from(reparsed).expect("valid"), declared);
    }

    #[test]
    fn an_error_payload_field_must_name_a_declared_type() {
        let declared = ErrorSpec {
            name: name("billing.invoice.InvalidAmount"),
            summary: Some("The requested amount is not positive.".to_owned()),
            fields: vec![Field::new(
                "limit",
                TypeRef::Named(name("billing.invoice.Limit")),
            )],
        };

        let errors = declared.validate(&registry()).expect_err("undeclared type");
        assert!(errors.contains(ValidationCode::UndeclaredReference));
        assert!(
            errors.to_string().contains("`billing.invoice.Limit`"),
            "{errors}"
        );
    }
    // ---- the entity an outcome acts on ---------------------------------------------------------

    #[test]
    fn an_outcome_names_the_entity_it_changes_and_the_move_it_takes() {
        let command: RawCommandSpec = serde_yaml::from_str(
            r"
name: billing.invoice.PayInvoice
input:
  - name: invoice_id
    type: billing.invoice.InvoiceId
  - name: amount
    type: billing.invoice.Money
outcomes:
  - name: settled
    when: amount.amount > 0
    moves: billing.invoice.Invoice.settle
    instance: invoice_id
    emits: [billing.invoice.InvoiceCreated]
  - name: rejected
    error: billing.invoice.InvalidAmount
",
        )
        .expect("well formed");
        let command = CommandSpec::try_from(command).expect("valid");

        let settled = command
            .outcome(&outcome_name("settled"))
            .expect("the branch that moves it");
        let subject = settled.subject.as_ref().expect("a subject");
        assert_eq!(subject.entity, name("billing.invoice.Invoice"));
        assert_eq!(
            subject.effect,
            Effect::Moves {
                transition: "settle".to_owned()
            },
            "the entity and the transition are split from one written name"
        );
        assert!(
            command
                .outcome(&outcome_name("rejected"))
                .expect("the refusal")
                .subject
                .is_none(),
            "a refusal changes nothing, so it names nothing"
        );

        // The written form survives a round trip: `moves:` back out as one qualified name.
        let rendered = serde_yaml::to_string(&command).expect("serialises");
        assert!(
            rendered.contains("moves: billing.invoice.Invoice.settle"),
            "{rendered}"
        );
        let reparsed: RawCommandSpec = serde_yaml::from_str(&rendered).expect("re-parses");
        assert_eq!(CommandSpec::try_from(reparsed).expect("valid"), command);
    }

    #[test]
    fn an_outcome_that_reports_an_error_and_also_changes_an_entity_is_refused() {
        // The same rule as "reports an error and also emits", read on the lifecycle. A refused
        // command changes nothing, so a branch cannot both refuse and move an invoice.
        let errors = refuse(&command_with(vec![
            Outcome::when(
                outcome_name("accepted"),
                Predicate::parse_expression("amount.amount > 0").expect("a predicate"),
                vec![name("billing.invoice.InvoiceCreated")],
            ),
            Outcome {
                name: outcome_name("rejected"),
                condition: OutcomeCondition::Otherwise,
                subject: Some(Subject::moves(
                    name("billing.invoice.Invoice"),
                    "settle",
                    "invoice_id",
                )),
                emits: Vec::new(),
                payload: BTreeMap::new(),
                error: Some(name("billing.invoice.InvalidAmount")),
                summary: None,
            },
        ]));
        assert!(
            errors.contains(ValidationCode::RefusalMutatedState),
            "{errors}"
        );
        assert!(errors.to_string().contains("moves"), "{errors}");
    }

    #[test]
    fn an_outcome_that_moves_an_entity_without_naming_an_instance_is_refused() {
        // The gap this gate closes, at the door. A branch that says an invoice moved and not *which*
        // invoice is a branch no scenario can be written for, and the two answers available to a
        // generator are both wrong: invent an id and the test fails a correct implementation, or
        // generate nothing and the suite is quietly short of a check nobody notices.
        //
        // The fixture is otherwise complete — a declared entity, a declared move, an emitted event —
        // so the refusal is about the missing key and not about the rest of it.
        let raw: RawCommandSpec = serde_yaml::from_str(
            r"
name: billing.invoice.PayInvoice
input:
  - name: invoice_id
    type: billing.invoice.InvoiceId
outcomes:
  - name: settled
    moves: billing.invoice.Invoice.settle
    emits: [billing.invoice.InvoiceCreated]
",
        )
        .expect("well formed");

        let errors = CommandSpec::try_from(raw).expect_err("a subject with no instance");
        assert!(
            errors.contains(ValidationCode::MissingDeclaration),
            "{errors}"
        );
        let refusal = errors
            .as_slice()
            .iter()
            .find(|error| error.code == ValidationCode::MissingDeclaration)
            .expect("the refusal");
        assert!(
            refusal.location.ends_with("outcomes.settled.moves"),
            "it points at the key an author would go and edit: {}",
            refusal.location
        );
        assert!(
            refusal
                .hint
                .as_deref()
                .is_some_and(|hint| hint.contains("instance:")),
            "and says which key to write: {refusal:?}"
        );
    }

    #[test]
    fn an_instance_named_by_a_branch_that_changes_nothing_is_refused() {
        // The other half of the same rule. `instance:` alone names the identity of nothing — there
        // is no entity for it to identify — and reading it as harmless would let a `creates:` be
        // deleted without the link that depends on it being noticed.
        let raw: RawCommandSpec = serde_yaml::from_str(
            r"
name: billing.invoice.PayInvoice
input:
  - name: invoice_id
    type: billing.invoice.InvoiceId
outcomes:
  - name: settled
    instance: invoice_id
    emits: [billing.invoice.InvoiceCreated]
",
        )
        .expect("well formed");

        let errors = CommandSpec::try_from(raw).expect_err("an instance with no subject");
        assert!(
            errors.contains(ValidationCode::MissingDeclaration),
            "{errors}"
        );
        assert!(
            errors.to_string().contains("identity of nothing"),
            "the message says what is wrong with it, not merely that something is: {errors}"
        );
    }

    #[test]
    fn a_branch_decided_by_the_field_that_names_its_instance_is_refused() {
        // Invariant 13 — identity is opaque — read on a command's branches. Two things go wrong at
        // once if this is allowed: the specification decides behaviour by looking inside an id, and
        // a generated scenario decides the guard against a witness id and then sends the id of the
        // instance an earlier step created, which are two different values.
        //
        // The fixture reaches the state where the rule bites: the guard is well formed, `invoice_id`
        // *is* a declared input field, and the only thing wrong with reading it is this rule.
        let command = CommandSpec {
            name: name("billing.invoice.PayInvoice"),
            input: vec![
                Field::new("invoice_id", TypeRef::Named(name("billing.invoice.Email"))),
                Field::new("amount", TypeRef::Named(name("billing.invoice.Money"))),
            ],
            outcomes: vec![
                Outcome::when(
                    outcome_name("settled"),
                    Predicate::parse_expression("invoice_id != \"\"").expect("a predicate"),
                    vec![name("billing.invoice.InvoiceCreated")],
                )
                .acting_on(Subject::moves(
                    name("billing.invoice.Invoice"),
                    "settle",
                    "invoice_id",
                )),
                Outcome {
                    name: outcome_name("rejected"),
                    condition: OutcomeCondition::Otherwise,
                    subject: None,
                    emits: Vec::new(),
                    payload: BTreeMap::new(),
                    error: Some(name("billing.invoice.InvalidAmount")),
                    summary: None,
                },
            ],
            naming: Naming::default(),
        };

        let errors = refuse(&command);
        assert!(
            errors.contains(ValidationCode::UnobservableFact),
            "a guard over the field that names the instance is refused: {errors}"
        );
        assert!(
            errors.to_string().contains("opaque"),
            "and the message says why, so the rule is learnable from one diagnostic: {errors}"
        );
    }

    #[test]
    fn an_outcome_that_both_creates_and_moves_is_refused_naming_both_keys() {
        let raw: RawCommandSpec = serde_yaml::from_str(
            r"
name: billing.invoice.CreateInvoice
outcomes:
  - name: accepted
    creates: billing.invoice.Invoice
    moves: billing.invoice.Invoice.settle
    emits: [billing.invoice.InvoiceCreated]
",
        )
        .expect("well formed");
        let errors = CommandSpec::try_from(raw).expect_err("two verbs for one branch");
        assert!(
            errors.contains(ValidationCode::ConflictingDeclaration),
            "{errors}"
        );
        let message = errors.to_string();
        assert!(
            message.contains("creates") && message.contains("moves"),
            "both keys are named, because which two collided is the repair: {message}"
        );
    }

    #[test]
    fn a_move_written_without_an_entity_is_refused_rather_than_read_as_one() {
        let raw: RawCommandSpec = serde_yaml::from_str(
            r"
name: billing.invoice.PayInvoice
outcomes:
  - name: settled
    moves: settle
    emits: [billing.invoice.InvoiceCreated]
",
        )
        .expect("well formed");
        let errors = CommandSpec::try_from(raw).expect_err("no entity is named");
        assert!(
            errors.contains(ValidationCode::MissingDeclaration),
            "{errors}"
        );
    }

    #[test]
    fn an_outcome_that_changes_no_entity_says_so_by_saying_nothing() {
        // `SendEmail` changes nothing in the model, and the link is optional on this side for
        // exactly that reason: a required subject would make an author invent one.
        let raw: RawCommandSpec = serde_yaml::from_str(
            r"
name: billing.invoice.CreateInvoice
outcomes:
  - name: accepted
    emits: [billing.invoice.InvoiceCreated]
",
        )
        .expect("well formed");
        let command = CommandSpec::try_from(raw).expect("valid without a subject");
        assert!(command.outcomes[0].subject.is_none());
    }

    // ---- the payload construct ---------------------------------------------------------------

    /// `CreateInvoice` with the `payload:` block `examples/billing/` declares: two fields
    /// determined from the input, the identity left to the implementation.
    const CREATE_INVOICE_WITH_PAYLOAD: &str = r"
name: billing.invoice.CreateInvoice
input:
  - name: customer_email
    type: billing.invoice.Email
  - name: amount
    type: billing.invoice.Money
outcomes:
  - name: accepted
    when: amount.amount > 0
    emits:
      - billing.invoice.InvoiceCreated
    payload:
      billing.invoice.InvoiceCreated:
        customer_email: input.customer_email
        amount: input.amount
  - name: rejected
    error: billing.invoice.InvalidAmount
";

    /// `InvoiceCreated` as the fixture declares it, for the cross-declaration half.
    fn invoice_created() -> EventSpec {
        let raw: RawEventSpec = serde_yaml::from_str(
            r"
name: billing.invoice.InvoiceCreated
fields:
  - name: invoice_id
    type: billing.invoice.Email
  - name: customer_email
    type: billing.invoice.Email
  - name: amount
    type: billing.invoice.Money
",
        )
        .expect("parses");
        EventSpec::try_from(raw).expect("a valid event")
    }

    /// The two maps [`validate_payloads`] walks, built from one command and one event.
    fn payload_context(
        command: CommandSpec,
    ) -> (
        BTreeMap<QualifiedName, CommandSpec>,
        BTreeMap<QualifiedName, EventSpec>,
    ) {
        let event = invoice_created();
        (
            [(command.name.clone(), command)].into(),
            [(event.name.clone(), event)].into(),
        )
    }

    #[test]
    fn a_payload_mapping_parses_validates_and_round_trips() {
        let raw: RawCommandSpec =
            serde_yaml::from_str(CREATE_INVOICE_WITH_PAYLOAD).expect("parses");
        let command = CommandSpec::try_from(raw).expect("a valid command");
        assert!(
            command.validate(&registry(), &events(), &errors()).is_ok(),
            "the fixture's own payload block must validate"
        );
        let accepted = command
            .outcome(&outcome_name("accepted"))
            .expect("the branch exists");
        assert_eq!(
            accepted.payload[&name("billing.invoice.InvoiceCreated")]["amount"],
            PayloadSource::InputField {
                field: "amount".to_owned()
            }
        );

        let (commands, declared_events) = payload_context(command.clone());
        let cross = validate_payloads(
            &commands,
            &declared_events,
            &registry(),
            &crate::types::ConversionRegistry::default(),
        );
        assert!(cross.is_empty(), "{cross}");

        let rendered = serde_yaml::to_string(&command).expect("serialises");
        let reparsed: RawCommandSpec = serde_yaml::from_str(&rendered).expect("re-parses");
        let round_tripped = CommandSpec::try_from(reparsed).expect("still valid");
        assert_eq!(round_tripped, command, "{rendered}");
    }

    #[test]
    fn an_event_field_with_no_declared_source_is_undetermined_and_not_an_error() {
        // The decision, pinned: `invoice_id` has no source in the fixture above — the identity is
        // the implementation's to assign — and neither half of validation calls that incomplete.
        // A binding must fill every required input; a payload determines exactly what the author
        // says it determines.
        let raw: RawCommandSpec =
            serde_yaml::from_str(CREATE_INVOICE_WITH_PAYLOAD).expect("parses");
        let command = CommandSpec::try_from(raw).expect("a valid command");
        let determined = &command
            .outcome(&outcome_name("accepted"))
            .expect("the branch exists")
            .payload[&name("billing.invoice.InvoiceCreated")];
        assert!(
            !determined.contains_key("invoice_id"),
            "the fixture leaves the identity undetermined"
        );
        assert!(command.validate(&registry(), &events(), &errors()).is_ok());
    }

    #[test]
    fn a_payload_for_an_event_the_branch_does_not_emit_is_refused() {
        let raw: RawCommandSpec = serde_yaml::from_str(
            r"
name: billing.invoice.CreateInvoice
input:
  - name: amount
    type: billing.invoice.Money
outcomes:
  - name: accepted
    when: amount.amount > 0
    emits:
      - billing.invoice.InvoiceCreated
  - name: rejected
    error: billing.invoice.InvalidAmount
    payload:
      billing.invoice.InvoiceCreated:
        amount: input.amount
",
        )
        .expect("parses");
        let errors = CommandSpec::try_from(raw).expect_err("the refusal emits nothing");
        assert_eq!(errors.len(), 1, "{errors}");
        assert!(errors.contains(ValidationCode::UndeclaredReference));
        assert!(errors.to_string().contains("does not emit it"), "{errors}");
    }

    #[test]
    fn a_payload_reading_an_undeclared_input_accumulates_beside_its_neighbours() {
        // Invariant 3: two bad sources are two repairs, reported together.
        let raw: RawCommandSpec = serde_yaml::from_str(
            r"
name: billing.invoice.CreateInvoice
input:
  - name: amount
    type: billing.invoice.Money
outcomes:
  - name: accepted
    when: amount.amount > 0
    emits:
      - billing.invoice.InvoiceCreated
    payload:
      billing.invoice.InvoiceCreated:
        amount: input.amout
        customer_email: input.email
  - name: rejected
    error: billing.invoice.InvalidAmount
",
        )
        .expect("parses");
        let errors = CommandSpec::try_from(raw).expect_err("two sources read nothing");
        assert_eq!(errors.len(), 2, "{errors}");
        assert!(errors.contains(ValidationCode::UndeclaredReference));
        assert!(
            errors.to_string().contains("does not declare as input"),
            "{errors}"
        );
    }

    #[test]
    fn a_repeated_payload_line_is_refused_rather_than_silently_last_wins() {
        // `serde_yaml` accepts both repeated keys below and would keep the last of each; the list
        // representation is what lets this be reported instead. One duplicated field and one
        // duplicated event block are two repairs (invariant 3).
        let raw: RawOutcome = serde_yaml::from_str(
            r"
name: accepted
emits:
  - billing.invoice.InvoiceCreated
payload:
  billing.invoice.InvoiceCreated:
    amount: input.amount
    amount: input.other
  billing.invoice.InvoiceCreated:
    amount: input.amount
",
        )
        .expect("parses, which is the problem being caught");
        let errors = Outcome::try_from(raw).expect_err("both duplicates are reported");
        assert_eq!(errors.len(), 2, "{errors}");
        assert!(errors.contains(ValidationCode::DuplicateDeclaration));
    }

    #[test]
    fn a_payload_filling_a_field_the_event_does_not_carry_is_refused() {
        let command = create_invoice_determining("total", "input.amount");
        let (commands, declared_events) = payload_context(command);
        let found = validate_payloads(
            &commands,
            &declared_events,
            &registry(),
            &crate::types::ConversionRegistry::default(),
        );
        assert_eq!(found.len(), 1, "{found}");
        assert!(found.contains(ValidationCode::UndeclaredReference));
        assert!(found.to_string().contains("is not a field"), "{found}");
    }

    #[test]
    fn a_payload_whose_two_types_disagree_needs_a_declared_conversion() {
        // `customer_email` (an `Email`) into `InvoiceCreated.amount` (a `Money`): refused without
        // a conversion, permitted with one — the same two halves the binding mapping's check has.
        let command = create_invoice_determining("amount", "input.customer_email");
        let (commands, declared_events) = payload_context(command);
        let found = validate_payloads(
            &commands,
            &declared_events,
            &registry(),
            &crate::types::ConversionRegistry::default(),
        );
        assert_eq!(found.len(), 1, "{found}");
        assert!(found.contains(ValidationCode::TypeMismatch));
        assert!(
            found.to_string().contains("no conversion is declared"),
            "{found}"
        );

        let mut conversions = crate::types::ConversionRegistry::default();
        conversions
            .insert(crate::types::Conversion {
                from: TypeRef::Named(name("billing.invoice.Email")),
                to: TypeRef::Named(name("billing.invoice.Money")),
                because: "a test crossing".to_owned(),
            })
            .expect("a new crossing");
        let (commands, declared_events) =
            payload_context(create_invoice_determining("amount", "input.customer_email"));
        let permitted = validate_payloads(&commands, &declared_events, &registry(), &conversions);
        assert!(permitted.is_empty(), "{permitted}");
    }

    #[test]
    fn a_payload_literal_that_names_an_input_is_a_misspelled_reference() {
        let command = create_invoice_determining("customer_email", "customer_email");
        let (commands, declared_events) = payload_context(command);
        let found = validate_payloads(
            &commands,
            &declared_events,
            &registry(),
            &crate::types::ConversionRegistry::default(),
        );
        assert_eq!(found.len(), 1, "{found}");
        assert!(found.contains(ValidationCode::MisspelledReference));
        assert!(
            found.to_string().contains("input.customer_email"),
            "the hint writes the repair: {found}"
        );
    }

    #[test]
    fn a_payload_literal_with_a_misspelt_prefix_is_a_misspelled_reference() {
        // `inptu.amount` and the binding's own `event.amount` both read as literal text, and both
        // are a reference meant: the first misspells this construct's prefix, the second carries
        // the binding's prefix into a mapping that reads the other surface.
        for written in ["inptu.amount", "event.amount"] {
            let command = create_invoice_determining("customer_email", written);
            let (commands, declared_events) = payload_context(command);
            let found = validate_payloads(
                &commands,
                &declared_events,
                &registry(),
                &crate::types::ConversionRegistry::default(),
            );
            assert_eq!(found.len(), 1, "{written}: {found}");
            assert!(
                found.contains(ValidationCode::MisspelledReference),
                "{written}: {found}"
            );
        }
    }

    #[test]
    fn a_payload_literal_cannot_fill_a_field_with_structure() {
        // `Money` is a struct, and a literal is one piece of text.
        let command = create_invoice_determining("amount", "one hundred");
        let (commands, declared_events) = payload_context(command);
        let found = validate_payloads(
            &commands,
            &declared_events,
            &registry(),
            &crate::types::ConversionRegistry::default(),
        );
        assert_eq!(found.len(), 1, "{found}");
        assert!(found.contains(ValidationCode::TypeMismatch));
        assert!(found.to_string().contains("has structure"), "{found}");
    }

    /// The valid fixture command with one payload entry swapped in, for each cross-check to break.
    fn create_invoice_determining(target: &str, source: &str) -> CommandSpec {
        let mut command = create_invoice();
        let accepted = command
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.name == outcome_name("accepted"))
            .expect("the branch exists");
        accepted.payload.insert(
            name("billing.invoice.InvoiceCreated"),
            [(target.to_owned(), PayloadSource::parse(source))].into(),
        );
        command
    }
}
