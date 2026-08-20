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
//!
//! The third exists because a provider refusing an address is not a fact about the input. Folding it
//! into `when: false` says the branch is unreachable, which is a lie a generator acts on: it either
//! skips the branch silently or emits a test nobody can make pass. [`Outcome::test_strategy`] is the
//! model's answer to that question, so no generator has to guess it.
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
//! | an outcome is both conditional and external, or declares two of `creates`/`moves`/`updates` | [`ConflictingDeclaration`](ValidationCode::ConflictingDeclaration) |
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

use std::collections::BTreeSet;
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
}

impl OutcomeCondition {
    /// The predicate this condition tests, when it tests one.
    pub fn predicate(&self) -> Option<&Predicate> {
        match self {
            Self::When(predicate) => Some(predicate),
            Self::Otherwise | Self::External { .. } => None,
        }
    }

    /// What decides this branch from outside the input, when something does.
    pub fn cause(&self) -> Option<&str> {
        match self {
            Self::External { cause } => Some(cause),
            Self::When(_) | Self::Otherwise => None,
        }
    }

    /// How a generated test has to reach this branch.
    pub fn test_strategy(&self) -> TestStrategy {
        match self {
            Self::When(_) => TestStrategy::ConstructInput,
            Self::Otherwise => TestStrategy::DefaultBranch,
            Self::External { .. } => TestStrategy::InjectFault,
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
}

impl TestStrategy {
    /// The strategy as it appears in generated output.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ConstructInput => "construct_input",
            Self::DefaultBranch => "default_branch",
            Self::InjectFault => "inject_fault",
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
            error: None,
            summary: None,
        }
    }

    /// The same outcome, acting on `subject`.
    #[must_use]
    pub fn acting_on(mut self, subject: Subject) -> Self {
        self.subject = Some(subject);
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
            OutcomeCondition::External { .. } => false,
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
    pub fn is_testable_from_input(&self) -> bool {
        self.test_strategy() != TestStrategy::InjectFault
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

        errors.extend(self.validate_guard(outcome, inputs, &location));
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
/// `when` and `external` are the two spellings of a condition; writing neither is the default
/// branch, and writing both is refused.
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
        let condition = match (raw.when, raw.external) {
            (Some(_), Some(_)) => {
                return Err(ValidationError::new(
                    // The nearest protocol code for "two declarations contradict each other".
                    ValidationCode::ConflictingDeclaration,
                    format!("outcomes.{}.when", raw.name),
                    format!(
                        "outcome `{}` declares both a `when` predicate and an `external` cause; a \
                         branch is either decided by the input or it is not",
                        raw.name
                    ),
                )
                .with_hint("keep `external` and drop the predicate, or the other way round")
                .into());
            }
            (Some(predicate), None) => OutcomeCondition::When(predicate),
            (None, Some(cause)) => OutcomeCondition::External { cause },
            (None, None) => OutcomeCondition::Otherwise,
        };
        let subject = subject_of(&raw.name, raw.creates, raw.moves, raw.updates, raw.instance)?;
        Ok(Self {
            name: raw.name,
            condition,
            subject,
            emits: raw.emits,
            error: raw.error,
            summary: raw.summary,
        })
    }
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
        let (when, external) = match outcome.condition {
            OutcomeCondition::When(predicate) => (Some(predicate), None),
            OutcomeCondition::Otherwise => (None, None),
            OutcomeCondition::External { cause } => (None, Some(cause)),
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
        Self {
            name: outcome.name,
            when,
            external,
            creates,
            moves,
            updates,
            instance,
            emits: outcome.emits,
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
}
