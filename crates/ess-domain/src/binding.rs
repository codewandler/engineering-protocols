//! The interaction layer: what happens when an event occurs.
//!
//! A binding is `on event → invoke command`, with a mapping from the event's fields onto the
//! command's input (design §7). It is the one place in the model where two independently-written
//! declarations have to agree about a type, which makes it the one place a rename in one bounded
//! context can silently break another — and so the place mapping validation earns its keep.
//!
//! # A binding says what happens when it fails
//!
//! [`Delivery`] and [`Failure`] are required, not defaulted (review F3). §7 gave `on event → invoke
//! command` and a transport, and said nothing about the case where the command does not run: no
//! guarantee, no retry, no dead letter. A binding that can fail silently is the difference between
//! specifying a system and specifying a demo.
//!
//! [`Failure::Drop`] in particular has to be a word someone typed. Getting it by default is how a
//! system loses mail and nobody can find the decision that allowed it.
//!
//! | rule | code |
//! |---|---|
//! | it reacts to an event nothing declares | [`UndeclaredReference`](ValidationCode::UndeclaredReference) |
//! | it invokes a command nothing declares | [`UndeclaredReference`](ValidationCode::UndeclaredReference) |
//! | it escalates and does not say what that emits | [`MissingDeclaration`](ValidationCode::MissingDeclaration) |
//! | it escalates into an event nothing declares | [`UndeclaredReference`](ValidationCode::UndeclaredReference) |
//! | it names an escalation event and does not escalate | [`ConflictingDeclaration`](ValidationCode::ConflictingDeclaration) |
//! | a mapping reads a field the event does not have | [`UnobservableFact`](ValidationCode::UnobservableFact) |
//! | a mapping writes a field the command does not take | [`UndeclaredReference`](ValidationCode::UndeclaredReference) |
//! | the two types differ and no conversion is declared | [`TypeMismatch`](ValidationCode::TypeMismatch) |
//! | a required input of the command is unmapped | [`MissingDeclaration`](ValidationCode::MissingDeclaration) |
//! | one input is mapped twice | [`DuplicateDeclaration`](ValidationCode::DuplicateDeclaration) |
//! | a literal fills an input that is not text | [`TypeMismatch`](ValidationCode::TypeMismatch) |
//! | a misspelt `event.` prefix, which would otherwise be text | [`MisspelledReference`](ValidationCode::MisspelledReference) |
//! | a source reads a path through a field | [`UnsupportedConstruct`](ValidationCode::UnsupportedConstruct) |
//!
//! A delivery guarantee this build does not implement needs no code: [`Delivery`] has one variant,
//! so `delivery: exactly_once` is refused while the document is read.
//!
//! [`MissingDeclaration`](ValidationCode::MissingDeclaration) is written twice in that table — an
//! unmapped command input is the other one — and both are the same sentence: a document did not
//! write a key that what it *did* write makes required. The compiler bridges both to
//! `ESS-BINDING-005`, because a diagnostic code names a kind of defect and not a rule, and
//! `Detail`s carry which key it was.
//!
//! # `escalate` says what it emits
//!
//! [`Failure::Escalate`] used to name a consequence outside the system — surface it to a person —
//! and say nothing about how the system shows that it happened. No event, no command, no view
//! field, no state: so a conformance target could not be asked to prove escalation occurred, and
//! `examples/billing/` carried a requirement no oracle could check.
//!
//! That is the same silent failure `on_failure:` exists to prevent, one variant along. So an
//! escalation now publishes a **declared event**, and is observed by exactly the mechanism every
//! other fact in the system is:
//!
//! ```yaml
//! on_failure:
//!   escalate:
//!     emits: billing.email.DeliveryEscalated
//! ```
//!
//! An escalation nobody can observe stays writable — `on_failure: escalate` still parses — and is
//! refused during validation rather than while the document is read, so it accumulates beside the
//! binding's other errors instead of hiding them (invariant 3).
//!
//! **`retry` and `drop` are deliberately left alone.** A retry is already observable: it is another
//! invocation of a declared command, which is what [`Delivery::AtLeastOnce`] obliges the handler to
//! survive, so there is nothing to declare that the model does not already carry. A drop is
//! deliberately *un*observable — the whole content of the word is that the work is lost and nobody
//! is told — and giving it an event would turn it into a notification, which is a different policy
//! that already has a name. Its accountability is a document someone signed, not a runtime fact.
//!
//! # Where each rule runs
//!
//! [`BindingSpec::validate`] is everything a binding can be wrong about on its own — the shape of
//! its mapping sources. [`validate_bindings`] is everything that needs the rest of the
//! specification, and is what [`Specification::validate`](crate::spec::Specification::validate)
//! calls. Both accumulate; neither returns on the first failure.
//!
//! # A required input is one that is not `Optional`
//!
//! [`TypeRef::Optional`] is the model's only way of saying that a
//! value may be absent — a [`Field`] carries no `default:` and no `required:` — so an unmapped
//! optional input is not an omission, it is the decision that the value is absent, already stated.
//! Every other input left unmapped leaves a generator with no argument to pass and no statement of
//! what to pass instead, which is [`MissingDeclaration`](ValidationCode::MissingDeclaration).
//!
//! # What a literal is checked for, and what it is not
//!
//! `template: invoice-created` is a [`MappingSource::Literal`], and a literal reaches the model as
//! text. So the target's *representation* is what decides whether the text can fill it: `Optional`
//! and newtype wrappers are removed, and what is underneath must be `String` or an enum. That is
//! why the example's literal fills a `TemplateId` — a `TemplateId` is a `String` underneath, and
//! there is no other way to write one in a document.
//!
//! | a literal filling | checked |
//! |---|---|
//! | an enum, directly or under wrappers | exactly: it must name a declared variant |
//! | anything that is `String` underneath | that the input exists, and nothing about the value |
//! | anything else | refused: text cannot be a `Money`, a `List` or an `Integer` |
//!
//! **Not checked**, for anything that is text underneath: whether the value satisfies the
//! invariants of the type it fills, and whether it names anything that exists outside the
//! specification. `invoice-created` being a template someone actually wrote is not a question this
//! model can answer, and [`MappingSource::Literal`] is a separate variant so that a reader can see
//! exactly which mappings the compiler verified.
//!
//! # `event.` is the only way to read the event
//!
//! [`MappingSource::parse`] reads anything without the prefix as a literal, so `evnt.customer_email`
//! would become the text `evnt.customer_email` and a generator would send it as the recipient. Two
//! narrow checks close that:
//!
//! * a literal whose first dotted segment is a near-miss of `event` is refused;
//! * a literal that exactly names a field of the triggering event is refused.
//!
//! Narrow on purpose. "Anything with a dot in it" would refuse `invoice.created`, which is a
//! perfectly good template name. The cost of the second check is that the text `customer_email`
//! cannot be written as a literal when the event has a field of that name; the alternative is a
//! specification that silently sends a field's name instead of its value.
//!
//! # One field, not a path
//!
//! `event.amount.currency` is refused as **unsupported**, not as a missing field. `Money` really does
//! have a `currency`, and a diagnostic saying otherwise sends an author hunting for a typo.
//!
//! A mapping's promise to a generator is that one field of the event fills one input of the command.
//! A path makes the generator emit a projection as well, and nothing in the model says a projection
//! is total: an `Optional` or a union part-way along one turns a value that must be present into one
//! that may be absent, and a mapping says nothing about that case. The repair is to map the whole
//! value, or to add a field to the event that carries it — both of which are statements someone
//! made, which a silently partial projection is not.

use std::collections::btree_map::Entry;
use std::collections::BTreeMap;

use aep_domain::error::{ParseError, ValidationCode, ValidationError, ValidationErrors};

use crate::command::{CommandSpec, EventSpec};
use crate::name::{Naming, QualifiedName};
use crate::types::{ConversionRegistry, Field, Primitive, TypeBody, TypeRef, TypeRegistry};

/// A binding, as a document says it.
#[derive(Debug, Clone, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RawBindingSpec {
    /// Its identifier, unique in the system.
    #[serde(alias = "id")]
    pub name: String,
    /// What it reacts to.
    pub when: RawTrigger,
    /// What it does.
    pub invoke: RawInvocation,
    /// How the event's fields become the command's input.
    #[serde(default)]
    pub mapping: MappingTable,
    /// How many times the command may run. Required.
    pub delivery: Delivery,
    /// What happens when it does not run. Required.
    pub on_failure: RawFailure,
    /// What it is called on the wire and shown as.
    #[serde(default)]
    pub naming: Naming,
    /// What it is for, in one line.
    #[serde(default)]
    pub summary: Option<String>,
}

/// What a binding reacts to.
#[derive(Debug, Clone, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RawTrigger {
    /// The event.
    pub event: QualifiedName,
}

/// What a binding does.
#[derive(Debug, Clone, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RawInvocation {
    /// The command.
    pub command: QualifiedName,
}

/// How many times the command may run.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Delivery {
    /// At least once, so the command must be idempotent.
    ///
    /// The only guarantee this build implements, and it is stated rather than assumed because
    /// "exactly once" is what everyone believes they have until a retry proves otherwise.
    AtLeastOnce,
}

/// What happens when the command does not run.
///
/// The word only. What an [`Escalate`](Self::Escalate) publishes is beside it — on
/// [`RawFailure::emits`] as a document writes it, on [`BindingSpec::escalation`] once validated —
/// so that this stays the one token every projection prints as "the word an author wrote".
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Failure {
    /// Try again, on whatever schedule the transport provides.
    ///
    /// Observable without anything further being declared: a retry is another invocation of the
    /// command, which [`Delivery::AtLeastOnce`] already obliges the handler to survive.
    Retry,
    /// Surface it to a person, and publish the declared event that says so.
    ///
    /// The event is required — see [`BindingSpec::escalation`]. Surfacing something to a person is
    /// an effect outside the system, and a specification that names one without saying how the
    /// system shows it happened has written a requirement no oracle can check.
    Escalate,
    /// Give up silently.
    ///
    /// Legal, and never a default: a system that loses work is a decision, and the decision has to
    /// be findable in the document that made it.
    ///
    /// Deliberately the one policy with nothing to observe. Publishing an event here would make it
    /// a notification, which is a different decision that already has a word.
    Drop,
}

impl Failure {
    /// The three words `on_failure:` accepts, in the order [`Failure`] declares them.
    ///
    /// Used to build the refusal a misspelt word gets, so the list a reader is offered cannot fall
    /// behind the variants.
    pub const WORDS: &'static [&'static str] = &["retry", "escalate", "drop"];

    /// The policy a word names, or `None` when it names none of them.
    pub fn parse(word: &str) -> Option<Self> {
        match word {
            "retry" => Some(Self::Retry),
            "escalate" => Some(Self::Escalate),
            "drop" => Some(Self::Drop),
            _ => None,
        }
    }

    /// The word, as a document writes it.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Retry => "retry",
            Self::Escalate => "escalate",
            Self::Drop => "drop",
        }
    }
}

impl std::fmt::Display for Failure {
    /// The word the author wrote, so a diagnostic quotes the document rather than the model.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What happens when the command does not run, as a document says it.
///
/// Two shapes under one key, and the word chooses the shape:
///
/// ```yaml
/// on_failure: retry
/// ```
///
/// ```yaml
/// on_failure:
///   escalate:
///     emits: billing.email.DeliveryEscalated
/// ```
///
/// # Why this parses without ambiguity
///
/// A scalar and a mapping are different YAML nodes, so nothing has to guess: the deserializer has
/// one reader for each shape and neither is a fallback for the other. That is the whole reason this
/// is a visitor rather than an untagged enum — `#[serde(untagged)]` tries each variant and reports
/// only that everything failed, which turns one misspelt word into a message naming no word at all.
///
/// Each of the four ways to get it wrong is refused with the key it is about:
///
/// | written | read as |
/// |---|---|
/// | `retry`, `escalate`, `drop` | that policy, with nothing published |
/// | `escalate:` with `emits:` under it | that policy, publishing that event |
/// | `retry:`/`drop:` with a block | refused while the document is read: only `escalate` publishes |
/// | two words under one `on_failure:` | refused while the document is read: a binding has one policy |
/// | a word that is none of the three | refused while the document is read, naming the three |
///
/// `escalate` written bare, `escalate:` with nothing under it, and `escalate:` with an empty block
/// are all read as "escalates, names no event". They are one author mistake in three spellings, so
/// they get one refusal — [`MissingDeclaration`](ValidationCode::MissingDeclaration), from
/// [`BindingSpec::validate`] — rather than a validation error and two different parse errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawFailure {
    /// The word.
    pub failure: Failure,
    /// The event an escalation publishes, when the document named one.
    pub emits: Option<QualifiedName>,
}

/// What an `escalate:` block says, as a document says it.
///
/// One key, so that adding a second way to observe an escalation later is a key beside `emits:`
/// rather than a change to the shape of `on_failure:`.
#[derive(Debug, Clone, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RawEscalation {
    /// The event the system publishes when it escalates.
    ///
    /// Optional here and required by [`BindingSpec::validate`], so that `escalate:` with an empty
    /// block is refused by the same rule as `escalate` written bare.
    #[serde(default)]
    pub emits: Option<QualifiedName>,
}

impl<'de> serde::Deserialize<'de> for RawFailure {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Policy;

        impl<'de> serde::de::Visitor<'de> for Policy {
            type Value = RawFailure;

            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("`retry`, `drop`, or `escalate:` with `emits: <event>` under it")
            }

            fn visit_str<E: serde::de::Error>(self, written: &str) -> Result<Self::Value, E> {
                Ok(RawFailure {
                    failure: policy_word(written)?,
                    emits: None,
                })
            }

            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut map: A,
            ) -> Result<Self::Value, A::Error> {
                let Some((written, block)) = map.next_entry::<String, Option<RawEscalation>>()?
                else {
                    return Err(serde::de::Error::custom(
                        "`on_failure` says nothing; write `retry`, `drop`, or `escalate:` with \
                         `emits: <event>` under it",
                    ));
                };
                if let Some((second, _)) = map.next_entry::<String, serde::de::IgnoredAny>()? {
                    return Err(serde::de::Error::custom(format!(
                        "`on_failure` says `{written}` and `{second}`; a binding has one policy for \
                         a command that does not run"
                    )));
                }
                let failure = policy_word(&written)?;
                if failure != Failure::Escalate {
                    return Err(serde::de::Error::custom(format!(
                        "`{written}` is written as a bare word — `on_failure: {written}`. Only \
                         `escalate` takes a block, because it is the only policy that publishes \
                         anything"
                    )));
                }
                Ok(RawFailure {
                    failure,
                    emits: block.and_then(|escalation| escalation.emits),
                })
            }
        }

        deserializer.deserialize_any(Policy)
    }
}

/// One of the three words, or a refusal that names all three.
fn policy_word<E: serde::de::Error>(written: &str) -> Result<Failure, E> {
    Failure::parse(written).ok_or_else(|| E::unknown_variant(written, Failure::WORDS))
}

impl schemars::JsonSchema for RawFailure {
    // Referenceable: `on_failure` is one construct with two spellings, and a reader following the
    // schema should land on the pair rather than on a `oneOf` inlined into the binding.
    fn schema_name() -> String {
        "BindingFailure".to_owned()
    }

    fn json_schema(generator: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        let mut block = schemars::schema::SchemaObject {
            instance_type: Some(schemars::schema::InstanceType::Object.into()),
            ..Default::default()
        };
        block.object().properties.insert(
            "escalate".to_owned(),
            generator.subschema_for::<RawEscalation>(),
        );
        block.object().required.insert("escalate".to_owned());
        // Only `escalate` takes a block; `retry:` and `drop:` with one are refused by the reader,
        // and the published schema has to say the same thing or the two disagree.
        block.object().additional_properties =
            Some(Box::new(schemars::schema::Schema::Bool(false)));

        let mut schema = schemars::schema::SchemaObject::default();
        schema.subschemas().one_of = Some(vec![
            generator.subschema_for::<Failure>(),
            schemars::schema::Schema::Object(block),
        ]);
        schema.metadata().description = Some(
            "What happens when the invoked command does not run: `retry`, `drop`, or an \
             `escalate:` block naming the event the escalation emits."
                .to_owned(),
        );
        schema.into()
    }
}

/// One field of the command's input, and where its value comes from.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Mapping {
    /// The command input field being filled.
    pub target: String,
    /// Where the value comes from.
    pub source: MappingSource,
}

/// A binding's mapping, as a document says it: one entry per line, in the order written.
///
/// A `BTreeMap` here would make one of this module's rules unenforceable. `serde_yaml` accepts a
/// repeated key and keeps the last, so
///
/// ```yaml
/// mapping:
///   recipient: event.customer_email
///   recipient: event.billing_email
/// ```
///
/// parses clean, drops the first line and leaves nothing downstream able to tell that the author
/// said two contradictory things. Keeping the entries in a list is what lets
/// [`TryFrom<RawBindingSpec>`] report it; [`BindingSpec::mapping`] is keyed by target, so past that
/// point a duplicate is unrepresentable.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MappingTable(pub Vec<Mapping>);

impl<'de> serde::Deserialize<'de> for MappingTable {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Entries;

        impl<'de> serde::de::Visitor<'de> for Entries {
            type Value = MappingTable;

            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(
                    "a mapping of command input to source, as in `recipient: event.customer_email`",
                )
            }

            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut map: A,
            ) -> Result<Self::Value, A::Error> {
                let mut entries = Vec::new();
                while let Some((target, source)) = map.next_entry::<String, String>()? {
                    entries.push(Mapping {
                        target,
                        source: MappingSource::parse(&source),
                    });
                }
                Ok(MappingTable(entries))
            }
        }

        deserializer.deserialize_map(Entries)
    }
}

impl serde::Serialize for MappingTable {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;

        let mut map = serializer.serialize_map(Some(self.0.len()))?;
        for entry in &self.0 {
            map.serialize_entry(&entry.target, &entry.source.to_string())?;
        }
        map.end()
    }
}

impl schemars::JsonSchema for MappingTable {
    // Inlined rather than referenced, so that the published schema says exactly what the
    // `BTreeMap<String, String>` this replaced said: the list is an implementation detail of
    // catching a repeated key, not something a document can see.
    fn is_referenceable() -> bool {
        false
    }

    fn schema_name() -> String {
        "MappingTable".to_owned()
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

/// Where a mapped value comes from.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum MappingSource {
    /// A field of the triggering event: `event.customer_email`.
    EventField {
        /// The field's name.
        field: String,
    },
    /// A value written in the binding: `template: invoice-created`.
    ///
    /// Its type cannot be checked against the target beyond "is it a string", which is exactly why
    /// it is a distinct variant: a reader can see which mappings the compiler has verified.
    Literal {
        /// The value, as written.
        value: String,
    },
}

impl MappingSource {
    /// The prefix that marks a field of the triggering event.
    pub const EVENT_PREFIX: &'static str = "event.";

    /// Reads `event.customer_email` as a field, anything else as a literal.
    pub fn parse(value: &str) -> Self {
        match value.strip_prefix(Self::EVENT_PREFIX) {
            Some(field) => Self::EventField {
                field: field.to_owned(),
            },
            None => Self::Literal {
                value: value.to_owned(),
            },
        }
    }
}

impl std::fmt::Display for MappingSource {
    /// As the document wrote it, so a diagnostic quotes the author rather than the model.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EventField { field } => write!(f, "{}{field}", Self::EVENT_PREFIX),
            Self::Literal { value } => f.write_str(value),
        }
    }
}

/// A binding's identifier.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
#[serde(transparent)]
pub struct BindingName(String);

impl BindingName {
    /// What a binding name looks like.
    pub const PATTERN: &'static str = "^[a-z][a-z0-9]*(-[a-z0-9]+)*$";

    /// Parses one.
    pub fn new(value: impl AsRef<str>) -> Result<Self, ParseError> {
        let value = value.as_ref();
        let valid = !value.is_empty()
            && value.starts_with(|c: char| c.is_ascii_lowercase())
            && !value.ends_with('-')
            && !value.contains("--")
            && value
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
        if !valid {
            return Err(ParseError::identifier(
                "binding name",
                value,
                "a binding name is lower-case words joined by single hyphens, such as \
                 `notify-on-invoice-created`"
                    .to_owned(),
            ));
        }
        Ok(Self(value.to_owned()))
    }

    /// The name as text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for BindingName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl schemars::JsonSchema for BindingName {
    fn schema_name() -> String {
        "BindingName".to_owned()
    }

    fn json_schema(_: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        let mut schema = schemars::schema::SchemaObject {
            instance_type: Some(schemars::schema::InstanceType::String.into()),
            ..Default::default()
        };
        schema.string().pattern = Some(Self::PATTERN.to_owned());
        schema.metadata().description =
            Some("A binding's identifier, such as `notify-on-invoice-created`.".to_owned());
        schema.into()
    }
}

/// A binding: one event causing one command.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct BindingSpec {
    /// Its identifier.
    pub name: BindingName,
    /// The event it reacts to.
    pub event: QualifiedName,
    /// The command it invokes.
    pub command: QualifiedName,
    /// How the event's fields become the command's input, keyed by target field.
    pub mapping: BTreeMap<String, MappingSource>,
    /// How many times the command may run.
    pub delivery: Delivery,
    /// What happens when it does not.
    pub failure: Failure,
    /// The event an escalation publishes.
    ///
    /// `Some` exactly when [`Self::failure`] is [`Failure::Escalate`], and both directions are
    /// checked: escalating without naming an event is
    /// [`MissingDeclaration`](ValidationCode::MissingDeclaration), and naming one without
    /// escalating is [`ConflictingDeclaration`](ValidationCode::ConflictingDeclaration). A document
    /// can only write the second by way of the first, because `retry:` and `drop:` take no block —
    /// but these fields are public, and a binding assembled in code has not been through the
    /// reader.
    ///
    /// This is what makes escalation provable. Every other consequence in the model is an event, a
    /// state change or an error; escalation named an effect outside the system and left the system
    /// with nothing to show for it, so a scenario could not assert that it happened.
    pub escalation: Option<QualifiedName>,
    /// What it is called on the wire, and what a person is shown.
    pub naming: Naming,
}

impl BindingSpec {
    /// Everything checkable without the rest of the specification.
    ///
    /// The shape of each mapping source: a path where the model takes a field, a prefix misspelt
    /// into a literal, a prefix with nothing after it. Plus the pairing of [`Self::failure`] with
    /// [`Self::escalation`], which needs nothing else declared to decide. Run by
    /// [`TryFrom<RawBindingSpec>`], and again by [`validate_bindings`] because the fields are public
    /// and a binding assembled in code has not been through the conversion.
    pub fn validate(&self) -> ValidationErrors {
        let mut errors = ValidationErrors::new();
        let prefix = MappingSource::EVENT_PREFIX;

        errors.extend(self.check_escalation());

        for (target, source) in &self.mapping {
            let at = format!("binding.{}.mapping.{target}", self.name);
            match source {
                MappingSource::EventField { field } if field.is_empty() => {
                    errors.push(
                        ValidationError::new(
                            ValidationCode::UnobservableFact,
                            at,
                            format!("`{prefix}` names no field of `{}`", self.event),
                        )
                        .with_hint("write the field after the dot, as in `event.customer_email`"),
                    );
                }
                MappingSource::EventField { field } if field.contains('.') => {
                    let root = field
                        .split_once('.')
                        .map_or(field.as_str(), |(root, _)| root);
                    errors.push(
                        ValidationError::new(
                            // Not `UnobservableFact`: `amount.currency` may well exist, and saying it
                            // does not sends an author hunting for a typo. See the module
                            // documentation for why a path is refused at all.
                            ValidationCode::UnsupportedConstruct,
                            at,
                            format!(
                                "`{prefix}{field}` reads a path, and this build maps one field of \
                                 an event onto one input of a command"
                            ),
                        )
                        .with_hint(format!(
                            "unsupported here rather than wrong: map `{prefix}{root}` whole, or add \
                             a field to `{}` that carries the value. An `Optional` or a union \
                             part-way along a path turns a value that must be present into one that \
                             may be absent, and a mapping says nothing about that case",
                            self.event
                        )),
                    );
                }
                MappingSource::EventField { .. } => {}
                MappingSource::Literal { value } => {
                    if let Some((written, rest)) = misspelt_event_prefix(value) {
                        errors.push(
                            ValidationError::new(
                                // Not `UndeclaredReference`: nothing was looked up, because this was
                                // not read as a reference at all. "Not declared" would be a false
                                // statement about a name nobody resolved.
                                ValidationCode::MisspelledReference,
                                at,
                                format!(
                                    "`{value}` is used as literal text, because `{written}` is not \
                                     `event`"
                                ),
                            )
                            .with_hint(format!(
                                "did you mean `{prefix}{rest}`? Anything without the `{prefix}` \
                                 prefix is a value rather than a reference, so a misspelt prefix \
                                 parses clean and sends the text"
                            )),
                        );
                    }
                }
            }
        }

        errors
    }

    /// [`Self::failure`] and [`Self::escalation`] agreeing about whether anything is published.
    ///
    /// Not folded into the loop above: it is about the binding's failure policy rather than about
    /// any one mapping entry, and a document with a broken mapping *and* an unobservable escalation
    /// has two things wrong with it and gets two errors.
    fn check_escalation(&self) -> ValidationErrors {
        let mut errors = ValidationErrors::new();
        match (self.failure, &self.escalation) {
            (Failure::Escalate, None) => {
                errors.push(
                    ValidationError::new(
                        // The same code an unmapped command input gets: a document did not write a
                        // key that what it did write makes required.
                        ValidationCode::MissingDeclaration,
                        format!("binding.{}.on_failure", self.name),
                        format!(
                            "binding `{}` escalates and does not say what that emits, so nothing \
                             can be asked to prove the escalation happened",
                            self.name
                        ),
                    )
                    .with_hint(
                        "write `on_failure:` with `escalate:` under it and `emits: <event>` under \
                         that. An escalation nobody can observe is the silent failure `on_failure` \
                         exists to prevent: `retry` is observable as another invocation, `drop` is \
                         unobservable on purpose, and `escalate` is neither",
                    ),
                );
            }
            (other, Some(event)) if other != Failure::Escalate => {
                errors.push(
                    ValidationError::new(
                        ValidationCode::ConflictingDeclaration,
                        format!("binding.{}.on_failure", self.name),
                        format!(
                            "binding `{}` fails with `{other}` and also emits `{event}` on \
                             escalation; only `escalate` publishes anything",
                            self.name
                        ),
                    )
                    .with_hint(
                        "a document cannot write this — `retry:` and `drop:` take no block — so \
                         this binding was assembled in code; drop the escalation event, or make \
                         the policy `escalate`",
                    ),
                );
            }
            _ => {}
        }
        errors
    }
}

impl TryFrom<RawBindingSpec> for BindingSpec {
    type Error = ValidationErrors;

    fn try_from(raw: RawBindingSpec) -> Result<Self, Self::Error> {
        let name = match BindingName::new(&raw.name) {
            Ok(name) => name,
            Err(error) => {
                return Err(ValidationErrors::new().with(ValidationError::new(
                    ValidationCode::TypeMismatch,
                    format!("binding {}", raw.name),
                    error.to_string(),
                )));
            }
        };

        let mut errors = ValidationErrors::new();
        let mut mapping: BTreeMap<String, MappingSource> = BTreeMap::new();
        for Mapping { target, source } in raw.mapping.0 {
            match mapping.entry(target) {
                Entry::Vacant(vacant) => {
                    vacant.insert(source);
                }
                Entry::Occupied(occupied) => {
                    errors.push(
                        ValidationError::new(
                            ValidationCode::DuplicateDeclaration,
                            format!("binding.{name}.mapping.{}", occupied.key()),
                            format!(
                                "`{}` is mapped from `{}` and again from `{source}`",
                                occupied.key(),
                                occupied.get()
                            ),
                        )
                        .with_hint(
                            "one input, one source; a repeated key parses clean and keeps the last, \
                             so nothing downstream could tell that two things were said",
                        ),
                    );
                }
            }
        }

        let binding = Self {
            name,
            event: raw.when.event,
            command: raw.invoke.command,
            mapping,
            delivery: raw.delivery,
            failure: raw.on_failure.failure,
            escalation: raw.on_failure.emits,
            naming: Naming {
                summary: raw.naming.summary.or(raw.summary),
                ..raw.naming
            },
        };

        errors.extend(binding.validate());
        errors.into_result(binding)
    }
}

/// Checks every binding against the events, commands and types the specification declares.
///
/// The cross-cutting pass: a binding is the one declaration that cannot be checked on its own,
/// because both of its ends are written somewhere else. Everything here comes from what
/// [`Specification::validate`](crate::spec::Specification::validate) already holds, and a missing
/// end stops only the checks that needed it — an event nobody declares is reported once, not once
/// per mapping entry underneath it.
pub fn validate_bindings(
    bindings: &BTreeMap<BindingName, BindingSpec>,
    events: &BTreeMap<QualifiedName, EventSpec>,
    commands: &BTreeMap<QualifiedName, CommandSpec>,
    types: &TypeRegistry,
    conversions: &ConversionRegistry,
) -> ValidationErrors {
    let mut errors = ValidationErrors::new();
    for binding in bindings.values() {
        errors.extend(binding.validate());
        errors.extend(
            Ends {
                binding,
                events,
                commands,
                types,
                conversions,
            }
            .check(),
        );
    }
    errors
}

/// One binding and everything its two ends are resolved against.
///
/// A struct rather than six arguments threaded through five functions, so that each rule reads as
/// what it checks rather than as what it had to be handed.
struct Ends<'a> {
    binding: &'a BindingSpec,
    events: &'a BTreeMap<QualifiedName, EventSpec>,
    commands: &'a BTreeMap<QualifiedName, CommandSpec>,
    types: &'a TypeRegistry,
    conversions: &'a ConversionRegistry,
}

impl Ends<'_> {
    /// The event this binding reacts to, when something declares it.
    fn event(&self) -> Option<&EventSpec> {
        self.events.get(&self.binding.event)
    }

    /// The command this binding invokes, when something declares it.
    fn command(&self) -> Option<&CommandSpec> {
        self.commands.get(&self.binding.command)
    }

    /// A location in the document form, which is what the compiler resolves to a line.
    fn at(&self, suffix: &str) -> String {
        format!("binding.{}.{suffix}", self.binding.name)
    }

    /// Both ends, every mapping entry, and every input nothing filled.
    fn check(&self) -> ValidationErrors {
        let mut errors = ValidationErrors::new();

        if self.event().is_none() {
            errors.push(
                ValidationError::new(
                    ValidationCode::UndeclaredReference,
                    self.at("when.event"),
                    format!("`{}` is not a declared event", self.binding.event),
                )
                .with_hint(available("event", self.events.keys())),
            );
        }
        if self.command().is_none() {
            errors.push(
                ValidationError::new(
                    ValidationCode::UndeclaredReference,
                    self.at("invoke.command"),
                    format!("`{}` is not a declared command", self.binding.command),
                )
                .with_hint(available("command", self.commands.keys())),
            );
        }
        if let Some(escalation) = &self.binding.escalation {
            if !self.events.contains_key(escalation) {
                errors.push(
                    ValidationError::new(
                        ValidationCode::UndeclaredReference,
                        self.at("on_failure.escalate.emits"),
                        format!("`{escalation}` is not a declared event"),
                    )
                    .with_hint(available("event", self.events.keys())),
                );
            }
        }

        for (target, source) in &self.binding.mapping {
            errors.extend(self.check_entry(target, source));
        }
        errors.extend(self.check_unfilled());

        errors
    }

    /// One entry: the input it fills, the value it takes, and whether the two agree.
    fn check_entry(&self, target: &str, source: &MappingSource) -> ValidationErrors {
        let mut errors = ValidationErrors::new();
        let at = self.at(&format!("mapping.{target}"));

        // A command nobody declares is reported once, above. Reporting every input it does not have
        // would bury the one error that can be repaired.
        let mut filled = None;
        if let Some(command) = self.command() {
            if let Some(input) = command.input_field(target) {
                filled = Some((command, input));
            } else {
                errors.push(
                    ValidationError::new(
                        ValidationCode::UndeclaredReference,
                        at.clone(),
                        format!("`{target}` is not an input of `{}`", command.name),
                    )
                    .with_hint(fillable(command)),
                );
            }
        }

        match source {
            MappingSource::EventField { field } => {
                // A shape `BindingSpec::validate` already refused is not resolved again: `event.`
                // and `event.amount.currency` each have one error, and it is not this one.
                if field.is_empty() || field.contains('.') {
                    return errors;
                }
                let Some(event) = self.event() else {
                    return errors;
                };
                let Some(read) = event.field(field) else {
                    errors.push(
                        ValidationError::new(
                            ValidationCode::UnobservableFact,
                            at,
                            format!(
                                "`{}{field}` is not a field of `{}`",
                                MappingSource::EVENT_PREFIX,
                                event.name
                            ),
                        )
                        .with_hint(readable(event)),
                    );
                    return errors;
                };
                if let Some((command, input)) = filled {
                    errors.extend(self.check_types(&at, event, read, command, input));
                }
            }
            MappingSource::Literal { value } => {
                errors.extend(self.check_literal(&at, value, filled));
            }
        }

        errors
    }

    /// Design §20: the two types must be compatible, or an explicit conversion must exist.
    ///
    /// [`ConversionRegistry::permits`] is both halves of that sentence, so this asks it once rather
    /// than deciding structural compatibility a second way.
    fn check_types(
        &self,
        at: &str,
        event: &EventSpec,
        read: &Field,
        command: &CommandSpec,
        input: &Field,
    ) -> ValidationErrors {
        let mut errors = ValidationErrors::new();
        if self.conversions.permits(&read.type_ref, &input.type_ref) {
            return errors;
        }

        // Design §29's diagnostic: both paths, both types, and the fact that nothing bridges them.
        errors.push(
            ValidationError::new(
                ValidationCode::TypeMismatch,
                at.to_owned(),
                format!(
                    "`{}.{}` has type `{}`, and `{}.{}` requires `{}`; no conversion is declared",
                    event.name,
                    read.name,
                    read.type_ref,
                    command.name,
                    input.name,
                    input.type_ref
                ),
            )
            .with_hint(format!(
                "declare the crossing — `conversions: [{{from: {}, to: {}, because: …}}]` — or make \
                 the two types agree. The reason is required, because a crossing nobody explained \
                 is the silent widening this refusal exists to catch",
                read.type_ref, input.type_ref
            )),
        );
        errors
    }

    /// A literal, against the representation of the input it fills.
    fn check_literal(
        &self,
        at: &str,
        value: &str,
        filled: Option<(&CommandSpec, &Field)>,
    ) -> ValidationErrors {
        let mut errors = ValidationErrors::new();
        let prefix = MappingSource::EVENT_PREFIX;

        // The prefix left off entirely. `recipient: customer_email` sends the field's name where its
        // value was meant, and nothing later in the pipeline can tell the difference.
        if let Some(event) = self.event() {
            if event.field(value).is_some() {
                errors.push(
                    ValidationError::new(
                        // As with a misspelt prefix: a reference was meant, and what was written
                        // was read as text instead.
                        ValidationCode::MisspelledReference,
                        at.to_owned(),
                        format!(
                            "`{value}` is a field of `{}` and is written here as literal text",
                            event.name
                        ),
                    )
                    .with_hint(format!(
                        "write `{prefix}{value}` to read the field; without the prefix the value is \
                         the text `{value}` itself"
                    )),
                );
                return errors;
            }
        }

        let Some((command, input)) = filled else {
            return errors;
        };
        let refuse = |reason: String| {
            ValidationError::new(ValidationCode::TypeMismatch, at.to_owned(), reason).with_hint(
                format!(
                    "only text and the variants of an enum can be written as a literal; take the \
                     value from a field of `{}` instead",
                    self.binding.event
                ),
            )
        };

        match representation(&input.type_ref, self.types) {
            // Text is as far as a literal can be checked — the module documentation says what that
            // leaves unsaid about the value — and a type that resolves to nothing is the type pass's
            // error, which reporting here would report twice and repair neither time.
            Some(Representation::Text) | None => {}
            Some(Representation::Variants(variants)) => {
                if !variants.iter().any(|variant| variant == value) {
                    errors.push(
                        ValidationError::new(
                            ValidationCode::TypeMismatch,
                            at.to_owned(),
                            format!(
                                "`{value}` is not a variant of `{}`, which is what `{}.{}` takes",
                                input.type_ref, command.name, input.name
                            ),
                        )
                        .with_hint(format!("variants: {}", list(variants))),
                    );
                }
            }
            Some(Representation::Primitive(primitive)) => {
                errors.push(refuse(format!(
                    "`{}.{}` is `{}`, which is `{primitive}` underneath, and a literal in a binding \
                     is text",
                    command.name, input.name, input.type_ref
                )));
            }
            Some(Representation::Structured) => {
                errors.push(refuse(format!(
                    "`{}.{}` is `{}`, which has structure, and a literal in a binding is one piece \
                     of text",
                    command.name, input.name, input.type_ref
                )));
            }
        }

        errors
    }

    /// Every input the command requires and the mapping does not fill.
    fn check_unfilled(&self) -> ValidationErrors {
        let mut errors = ValidationErrors::new();
        let Some(command) = self.command() else {
            return errors;
        };

        for input in &command.input {
            if input.type_ref.is_optional() || self.binding.mapping.contains_key(&input.name) {
                continue;
            }
            errors.push(
                ValidationError::new(
                    ValidationCode::MissingDeclaration,
                    self.at("mapping"),
                    format!(
                        "`{}.{}` requires a value and the mapping supplies none",
                        command.name, input.name
                    ),
                )
                .with_hint(format!(
                    "map it from a field of `{}`, give it a literal, or declare the input \
                     `Optional<{}>` if it may be absent",
                    self.binding.event, input.type_ref
                )),
            );
        }

        errors
    }
}

/// What a literal would have to be, once `Optional` and newtype wrappers are removed.
enum Representation<'a> {
    /// Text. A literal fills it, and the model can say nothing further about the value.
    Text,
    /// A closed set of names. A literal has to be one of them, which is checked exactly.
    Variants(&'a [String]),
    /// A primitive that is not text.
    Primitive(Primitive),
    /// Something with structure: a struct, a union, a list or a map.
    Structured,
}

/// How many wrappers deep the walk below will go before giving up.
///
/// Bounded rather than unbounded as defence in depth. `check_inhabitation` in
/// [`crate::system`] does refuse a newtype of itself, so this walk should never meet one — but the
/// two checks run in the same pass over the same document, and a validation pass that hangs is worse
/// than one that refuses a good document. A bound is cheaper than an ordering guarantee.
const WRAPPER_LIMIT: usize = 32;

/// The representation a literal would have to be spellable as, to fill `type_ref`.
///
/// `None` when the answer needs a type nothing declares, or when the wrappers run deeper than any
/// real specification: both are somebody else's error, already reported.
fn representation<'a>(
    type_ref: &'a TypeRef,
    types: &'a TypeRegistry,
) -> Option<Representation<'a>> {
    let mut current = type_ref;
    for _ in 0..WRAPPER_LIMIT {
        match current {
            TypeRef::Optional(inner) => current = inner,
            TypeRef::Primitive(Primitive::String) => return Some(Representation::Text),
            TypeRef::Primitive(primitive) => return Some(Representation::Primitive(*primitive)),
            TypeRef::List(_) | TypeRef::Map(_, _) => return Some(Representation::Structured),
            TypeRef::Named(name) => match types.get(name).map(|declared| &declared.body) {
                Some(TypeBody::Newtype { of, .. }) => current = of,
                Some(TypeBody::Enum { variants }) => {
                    return Some(Representation::Variants(variants));
                }
                Some(TypeBody::Struct { .. } | TypeBody::Union { .. }) => {
                    return Some(Representation::Structured);
                }
                None => return None,
            },
        }
    }
    None
}

/// The field a literal was probably meant to read, when its first segment is a near-miss of `event`.
///
/// Deliberately narrow — a near-miss of one word, not "anything with a dot in it" — because
/// `invoice.created` is a perfectly good template name and refusing it would be the worse mistake.
fn misspelt_event_prefix(value: &str) -> Option<(&str, &str)> {
    let (written, rest) = value.split_once('.')?;
    if written == MappingSource::EVENT_PREFIX.trim_end_matches('.') {
        return None;
    }
    // `noreply@example.com` splits into `noreply` and `example.com`, and neither half is a field: a
    // literal whose tail could not be a field name was not a reference to one.
    if !is_field_name(rest) {
        return None;
    }
    near_miss(written, MappingSource::EVENT_PREFIX.trim_end_matches('.')).then_some((written, rest))
}

/// `true` when `value` could be a field name — the same shape [`Field`] enforces.
fn is_field_name(value: &str) -> bool {
    value.starts_with(|c: char| c.is_ascii_alphabetic())
        && value.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// `true` when `value` is `word` misspelt: a different case, one edit away, or the right letters in
/// the wrong order.
fn near_miss(value: &str, word: &str) -> bool {
    if value.eq_ignore_ascii_case(word) {
        return true;
    }
    let lowered = value.to_ascii_lowercase();
    if edit_distance(&lowered, word) <= 1 {
        return true;
    }
    // A transposition is two edits by that measure and one slip of the fingers in practice. `evetn`
    // and `evnet` are both this, and both are the mistake the check is for.
    lowered.len() == word.len() && sorted_bytes(&lowered) == sorted_bytes(word)
}

/// Levenshtein distance over bytes. Both arguments are one word long.
fn edit_distance(left: &str, right: &str) -> usize {
    let mut previous: Vec<usize> = (0..=right.len()).collect();
    let mut current = vec![0; right.len() + 1];
    for (index, from) in left.bytes().enumerate() {
        current[0] = index + 1;
        for (position, into) in right.bytes().enumerate() {
            let substitute = previous[position] + usize::from(from != into);
            current[position + 1] = substitute
                .min(previous[position + 1] + 1)
                .min(current[position] + 1);
        }
        std::mem::swap(&mut previous, &mut current);
    }
    previous[right.len()]
}

/// The bytes of a word, sorted, so that two spellings of the same letters compare equal.
fn sorted_bytes(value: &str) -> Vec<u8> {
    let mut bytes = value.as_bytes().to_vec();
    bytes.sort_unstable();
    bytes
}

/// A hint saying what was declared, in the voice [`TypeRegistry::resolve`] uses.
fn available<T: std::fmt::Display>(kind: &str, names: impl IntoIterator<Item = T>) -> String {
    let listed = list(names);
    if listed.is_empty() {
        format!("nothing declares any {kind}")
    } else {
        format!("declared {kind}s: {listed}")
    }
}

/// What a mapping may read, for a hint.
fn readable(event: &EventSpec) -> String {
    if event.fields.is_empty() {
        return format!(
            "`{}` records nothing, so no mapping can read it",
            event.name
        );
    }
    format!(
        "readable here: {}",
        list(event.fields.iter().map(|field| &field.name))
    )
}

/// What a mapping may fill, for a hint.
fn fillable(command: &CommandSpec) -> String {
    if command.input.is_empty() {
        return format!(
            "`{}` takes no input, so there is nothing to map onto",
            command.name
        );
    }
    format!(
        "inputs of `{}`: {}",
        command.name,
        list(command.input.iter().map(|field| &field.name))
    )
}

/// Names joined for a hint, in whatever order they were declared or indexed.
fn list<T: std::fmt::Display>(names: impl IntoIterator<Item = T>) -> String {
    names
        .into_iter()
        .map(|name| name.to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Conversion, NamedType};

    const EVENT: &str = "billing.invoice.InvoiceCreated";
    const COMMAND: &str = "billing.email.SendEmail";
    /// The event `examples/billing/` escalates with.
    const ESCALATION: &str = "billing.email.DeliveryEscalated";
    /// The mapping `examples/billing/components.yaml` ships.
    const MAPPING: &str = "  recipient: event.customer_email\n  template: invoice-created\n";

    fn name(value: &str) -> QualifiedName {
        QualifiedName::new(value).expect("a valid name")
    }

    fn type_ref(value: &str) -> TypeRef {
        TypeRef::parse(value).expect("a valid type")
    }

    /// The example's types, plus the two only a test needs.
    fn types() -> TypeRegistry {
        let mut registry = TypeRegistry::new();
        for (declared, body) in [
            ("billing.invoice.Email", newtype("String")),
            ("billing.email.EmailAddress", newtype("String")),
            ("billing.email.VerifiedEmail", newtype("String")),
            ("billing.email.TemplateId", newtype("String")),
            ("billing.email.Priority", newtype("Integer")),
            (
                "billing.invoice.Money",
                TypeBody::Struct {
                    fields: vec![
                        Field::new("amount", type_ref("Decimal")),
                        Field::new("currency", type_ref("String")),
                    ],
                    invariants: Vec::new(),
                },
            ),
            (
                "billing.invoice.Channel",
                TypeBody::Enum {
                    variants: vec!["Email".to_owned(), "Post".to_owned(), "Portal".to_owned()],
                },
            ),
        ] {
            registry
                .insert(NamedType {
                    name: name(declared),
                    body,
                    naming: Naming::default(),
                })
                .expect("new");
        }
        registry
    }

    fn newtype(of: &str) -> TypeBody {
        TypeBody::Newtype {
            of: type_ref(of),
            invariants: Vec::new(),
        }
    }

    fn event(declared: &str, fields: &[(&str, &str)]) -> BTreeMap<QualifiedName, EventSpec> {
        let spec = EventSpec {
            name: name(declared),
            fields: fields
                .iter()
                .map(|(field, kind)| Field::new(*field, type_ref(kind)))
                .collect(),
            naming: Naming::default(),
        };
        [(spec.name.clone(), spec)].into()
    }

    /// A command with an input and nothing else.
    ///
    /// `outcomes` is empty on purpose: what a command results in is `command.rs`'s rule, and no
    /// binding reads it.
    fn command(declared: &str, input: &[(&str, &str)]) -> BTreeMap<QualifiedName, CommandSpec> {
        let spec = CommandSpec {
            name: name(declared),
            input: input
                .iter()
                .map(|(field, kind)| Field::new(*field, type_ref(kind)))
                .collect(),
            outcomes: Vec::new(),
            naming: Naming::default(),
        };
        [(spec.name.clone(), spec)].into()
    }

    /// The two events the example's binding touches: its trigger, and what its escalation emits.
    fn declared_events() -> BTreeMap<QualifiedName, EventSpec> {
        let mut events = event(
            EVENT,
            &[
                ("invoice_id", "billing.invoice.InvoiceId"),
                ("customer_email", "billing.invoice.Email"),
                ("amount", "billing.invoice.Money"),
            ],
        );
        events.extend(event(
            ESCALATION,
            &[("recipient", "billing.email.EmailAddress")],
        ));
        events
    }

    /// The command the example declares.
    fn send_email() -> BTreeMap<QualifiedName, CommandSpec> {
        command(
            COMMAND,
            &[
                ("recipient", "billing.email.EmailAddress"),
                ("template", "billing.email.TemplateId"),
            ],
        )
    }

    /// The conversion `examples/billing/components.yaml` declares, with the reason it gives.
    fn conversions() -> ConversionRegistry {
        let mut registry = ConversionRegistry::new();
        registry
            .insert(Conversion {
                from: type_ref("billing.invoice.Email"),
                to: type_ref("billing.email.EmailAddress"),
                because: "an invoice's customer email is a deliverable address".to_owned(),
            })
            .expect("new");
        registry
    }

    /// Reads a binding the way a document does.
    fn parse(event: &str, command: &str, mapping: &str) -> Result<BindingSpec, ValidationErrors> {
        let raw: RawBindingSpec = serde_yaml::from_str(&format!(
            "id: notify-on-invoice-created\nwhen:\n  event: {event}\ninvoke:\n  command: \
             {command}\nmapping:\n{mapping}delivery: at_least_once\non_failure:\n  escalate:\n    \
             emits: {ESCALATION}\n"
        ))
        .expect("well formed");
        BindingSpec::try_from(raw)
    }

    /// The example's binding, with its mapping replaced.
    fn binding(mapping: &str) -> BindingSpec {
        parse(EVENT, COMMAND, mapping).expect("the binding is well formed on its own")
    }

    /// The cross-cutting pass over one binding.
    fn check_against(
        binding: BindingSpec,
        events: &BTreeMap<QualifiedName, EventSpec>,
        commands: &BTreeMap<QualifiedName, CommandSpec>,
        conversions: &ConversionRegistry,
    ) -> ValidationErrors {
        let bindings = [(binding.name.clone(), binding)].into();
        validate_bindings(&bindings, events, commands, &types(), conversions)
    }

    /// The cross-cutting pass against what the example declares.
    fn check(binding: BindingSpec) -> ValidationErrors {
        check_against(binding, &declared_events(), &send_email(), &conversions())
    }

    /// The one error, when exactly one is expected.
    fn only(errors: &ValidationErrors) -> &ValidationError {
        assert_eq!(errors.len(), 1, "{errors}");
        &errors.as_slice()[0]
    }

    fn hint(error: &ValidationError) -> &str {
        error.hint.as_deref().unwrap_or_default()
    }

    #[test]
    fn the_binding_the_example_ships_typechecks() {
        // The other side of every rule below: a pass that refuses the specification this repository
        // ships as valid is worse than no pass.
        let errors = check(binding(MAPPING));
        assert!(errors.is_empty(), "{errors}");
    }

    #[test]
    fn a_binding_that_reacts_to_an_event_nothing_declares_is_refused() {
        let binding =
            parse("billing.invoice.InvoceCreated", COMMAND, MAPPING).expect("well formed");
        let errors = check(binding);

        let error = only(&errors);
        assert_eq!(error.code, ValidationCode::UndeclaredReference);
        assert_eq!(
            error.location,
            "binding.notify-on-invoice-created.when.event"
        );
        assert!(
            hint(error).contains(EVENT),
            "the hint says what is declared: {error}"
        );
    }

    #[test]
    fn a_binding_that_invokes_a_command_nothing_declares_is_refused() {
        let binding = parse(EVENT, "billing.email.SendMail", MAPPING).expect("well formed");
        let errors = check(binding);

        let error = only(&errors);
        assert_eq!(error.code, ValidationCode::UndeclaredReference);
        assert_eq!(
            error.location,
            "binding.notify-on-invoice-created.invoke.command"
        );
        assert!(hint(error).contains(COMMAND), "{error}");
    }

    #[test]
    fn a_mapping_that_reads_a_field_the_event_does_not_have_is_refused() {
        let errors = check(binding(
            "  recipient: event.customer_emial\n  template: invoice-created\n",
        ));

        let error = only(&errors);
        assert_eq!(error.code, ValidationCode::UnobservableFact);
        assert_eq!(
            error.location,
            "binding.notify-on-invoice-created.mapping.recipient"
        );
        assert!(error.message.contains("event.customer_emial"), "{error}");
        assert!(
            hint(error).contains("readable here: invoice_id, customer_email, amount"),
            "the hint lists what a mapping may read: {error}"
        );
    }

    #[test]
    fn a_mapping_that_writes_an_input_the_command_does_not_take_is_refused() {
        let errors = check(binding(
            "  recipent: event.customer_email\n  template: invoice-created\n",
        ));

        assert_eq!(
            errors.len(),
            2,
            "the typo, and the input it left unfilled: {errors}"
        );
        let error = errors
            .as_slice()
            .iter()
            .find(|error| error.code == ValidationCode::UndeclaredReference)
            .expect("the input nothing declares");
        assert_eq!(
            error.location,
            "binding.notify-on-invoice-created.mapping.recipent"
        );
        assert!(
            hint(error).contains("recipient, template"),
            "the hint lists what a mapping may fill: {error}"
        );
        assert!(
            errors.contains(ValidationCode::MissingDeclaration),
            "and `recipient` is still unfilled: {errors}"
        );
    }

    #[test]
    fn a_mapping_between_two_types_with_no_declared_conversion_is_refused() {
        // Design §29's case, and the diagnostic it drafts: both paths, both types, and the fact that
        // nothing bridges them.
        let commands = command(
            COMMAND,
            &[
                ("recipient", "billing.email.VerifiedEmail"),
                ("template", "billing.email.TemplateId"),
            ],
        );
        let errors = check_against(
            binding(MAPPING),
            &declared_events(),
            &commands,
            &conversions(),
        );

        let error = only(&errors);
        assert_eq!(error.code, ValidationCode::TypeMismatch);
        assert_eq!(
            error.location,
            "binding.notify-on-invoice-created.mapping.recipient"
        );
        for expected in [
            "billing.invoice.InvoiceCreated.customer_email",
            "billing.invoice.Email",
            "billing.email.SendEmail.recipient",
            "billing.email.VerifiedEmail",
            "no conversion is declared",
        ] {
            assert!(
                error.message.contains(expected),
                "a coding agent repairs this from the message: {expected:?} missing from {error}"
            );
        }
        assert!(
            hint(error).contains("because"),
            "and the repair says the reason is part of it: {error}"
        );
    }

    #[test]
    fn a_declared_conversion_is_what_lets_two_distinct_types_meet() {
        // `Email` and `EmailAddress` are both a `String` underneath. The entire value of naming them
        // apart is that this crossing has to be written down, so the refusal and the acceptance are
        // asserted together.
        let refused = check_against(
            binding(MAPPING),
            &declared_events(),
            &send_email(),
            &ConversionRegistry::new(),
        );
        assert!(refused.contains(ValidationCode::TypeMismatch), "{refused}");

        let permitted = check(binding(MAPPING));
        assert!(permitted.is_empty(), "{permitted}");
    }

    #[test]
    fn an_input_the_command_requires_and_the_mapping_omits_is_refused() {
        let errors = check(binding("  recipient: event.customer_email\n"));

        let error = only(&errors);
        assert_eq!(error.code, ValidationCode::MissingDeclaration);
        assert_eq!(error.location, "binding.notify-on-invoice-created.mapping");
        assert!(
            error.message.contains("billing.email.SendEmail.template"),
            "{error}"
        );
        assert!(
            hint(error).contains("Optional<billing.email.TemplateId>"),
            "the hint names the other way out: {error}"
        );
    }

    #[test]
    fn an_optional_input_the_mapping_omits_is_accepted() {
        // `Optional<T>` is the model's only way of saying a value may be absent, so leaving one
        // unmapped is a decision already stated rather than an omission.
        let commands = command(
            COMMAND,
            &[
                ("recipient", "billing.email.EmailAddress"),
                ("template", "Optional<billing.email.TemplateId>"),
            ],
        );
        let errors = check_against(
            binding("  recipient: event.customer_email\n"),
            &declared_events(),
            &commands,
            &conversions(),
        );
        assert!(errors.is_empty(), "{errors}");
    }

    #[test]
    fn an_input_mapped_twice_is_refused() {
        let errors = parse(
            EVENT,
            COMMAND,
            "  recipient: event.customer_email\n  recipient: event.invoice_id\n  template: \
             invoice-created\n",
        )
        .expect_err("`serde_yaml` keeps the last of a repeated key and says nothing");

        let error = only(&errors);
        assert_eq!(error.code, ValidationCode::DuplicateDeclaration);
        assert_eq!(
            error.location,
            "binding.notify-on-invoice-created.mapping.recipient"
        );
        assert!(
            error.message.contains("event.customer_email")
                && error.message.contains("event.invoice_id"),
            "both sources have to be named or neither can be chosen between: {error}"
        );
    }

    #[test]
    fn a_literal_that_is_not_a_variant_of_the_input_it_fills_is_refused() {
        let commands = command(
            COMMAND,
            &[
                ("recipient", "billing.email.EmailAddress"),
                ("template", "billing.email.TemplateId"),
                ("channel", "billing.invoice.Channel"),
            ],
        );
        let mapping = |channel: &str| format!("{MAPPING}  channel: {channel}\n");

        let errors = check_against(
            binding(&mapping("Postal")),
            &declared_events(),
            &commands,
            &conversions(),
        );
        let error = only(&errors);
        assert_eq!(error.code, ValidationCode::TypeMismatch);
        assert!(error.message.contains("not a variant"), "{error}");
        assert!(
            hint(error).contains("variants: Email, Post, Portal"),
            "{error}"
        );

        // An enum is the one literal the model checks exactly, so the accepted case is asserted too.
        let accepted = check_against(
            binding(&mapping("Post")),
            &declared_events(),
            &commands,
            &conversions(),
        );
        assert!(accepted.is_empty(), "{accepted}");
    }

    #[test]
    fn a_literal_cannot_fill_an_input_that_is_not_text() {
        let commands = command(
            COMMAND,
            &[
                ("recipient", "billing.email.EmailAddress"),
                ("template", "billing.email.TemplateId"),
                ("priority", "billing.email.Priority"),
            ],
        );
        let errors = check_against(
            binding(&format!("{MAPPING}  priority: \"3\"\n")),
            &declared_events(),
            &commands,
            &conversions(),
        );

        let error = only(&errors);
        assert_eq!(error.code, ValidationCode::TypeMismatch);
        assert!(
            error.message.contains("Integer") && error.message.contains("billing.email.Priority"),
            "the refusal names the representation it walked to: {error}"
        );
    }

    #[test]
    fn a_literal_that_has_structure_underneath_it_is_refused() {
        let commands = command(
            COMMAND,
            &[
                ("recipient", "billing.email.EmailAddress"),
                ("template", "billing.email.TemplateId"),
                ("amount", "billing.invoice.Money"),
            ],
        );
        let errors = check_against(
            binding(&format!("{MAPPING}  amount: 12.00\n")),
            &declared_events(),
            &commands,
            &conversions(),
        );

        let error = only(&errors);
        assert_eq!(error.code, ValidationCode::TypeMismatch);
        assert!(error.message.contains("has structure"), "{error}");
    }

    #[test]
    fn a_misspelt_event_prefix_is_refused_rather_than_sent_as_text() {
        for misspelling in [
            "evnt.customer_email",
            "Event.customer_email",
            "events.customer_email",
            "evetn.customer_email",
            "evnet.customer_email",
            "even.customer_email",
        ] {
            let errors = parse(
                EVENT,
                COMMAND,
                &format!("  recipient: {misspelling}\n  template: invoice-created\n"),
            )
            .expect_err(misspelling);

            let error = only(&errors);
            assert_eq!(
                error.code,
                ValidationCode::MisspelledReference,
                "{misspelling}: {error}"
            );
            assert!(
                hint(error).contains("event.customer_email"),
                "the hint says what was meant: {error}"
            );
        }
    }

    #[test]
    fn a_literal_that_merely_contains_a_dot_is_still_a_literal() {
        // The other side of the check above. `invoice.created` is a perfectly good template name,
        // and refusing it would be the worse mistake.
        for literal in ["invoice.created", "noreply@example.com", "v1.2.3"] {
            let errors = check(binding(&format!(
                "  recipient: event.customer_email\n  template: {literal}\n"
            )));
            assert!(errors.is_empty(), "{literal}: {errors}");
        }
    }

    #[test]
    fn a_literal_that_names_a_field_of_the_event_is_refused() {
        let errors = check(binding(
            "  recipient: customer_email\n  template: invoice-created\n",
        ));

        let error = only(&errors);
        assert_eq!(error.code, ValidationCode::MisspelledReference);
        assert_eq!(
            error.location,
            "binding.notify-on-invoice-created.mapping.recipient"
        );
        assert!(
            hint(error).contains("event.customer_email"),
            "the prefix left off entirely sends the field's name where its value was meant: {error}"
        );
    }

    #[test]
    fn a_nested_path_is_refused_as_unsupported_rather_than_as_a_missing_field() {
        let errors = parse(
            EVENT,
            COMMAND,
            "  recipient: event.amount.currency\n  template: invoice-created\n",
        )
        .expect_err("a path through a struct");

        let error = only(&errors);
        assert_eq!(error.code, ValidationCode::UnsupportedConstruct);
        assert!(error.message.contains("reads a path"), "{error}");
        assert!(
            !error.message.contains("is not a field"),
            "`amount.currency` exists; saying otherwise sends an author hunting a typo: {error}"
        );
        assert!(
            hint(error).contains("event.amount"),
            "the hint says what to map instead: {error}"
        );
    }

    #[test]
    fn a_source_that_names_no_field_after_the_prefix_is_refused() {
        let errors = parse(
            EVENT,
            COMMAND,
            "  recipient: event.\n  template: invoice-created\n",
        )
        .expect_err("a prefix with nothing after it");

        let error = only(&errors);
        assert_eq!(error.code, ValidationCode::UnobservableFact);
        assert!(error.message.contains("names no field"), "{error}");
    }

    #[test]
    fn every_problem_in_a_binding_is_reported_at_once() {
        // Accumulation is the point: an author fixing one error per run is an author running the
        // tool three times to learn what a single pass already knew.
        let errors = check(binding(
            "  recipient: event.customer_emial\n  templte: invoice-created\n",
        ));

        assert_eq!(errors.len(), 3, "{errors}");
        for code in [
            ValidationCode::UnobservableFact,
            ValidationCode::UndeclaredReference,
            ValidationCode::MissingDeclaration,
        ] {
            assert!(errors.contains(code), "{code}: {errors}");
        }
    }

    #[test]
    fn a_binding_assembled_in_code_is_checked_like_a_parsed_one() {
        // `BindingSpec`'s fields are public, so the shape checks `TryFrom` ran are not evidence
        // about a value that never went through it.
        let mut binding = binding(MAPPING);
        binding.mapping.insert(
            "recipient".to_owned(),
            MappingSource::EventField {
                field: "amount.currency".to_owned(),
            },
        );

        let errors = check(binding);
        assert!(
            errors.contains(ValidationCode::UnsupportedConstruct),
            "{errors}"
        );
    }

    #[test]
    fn a_binding_that_does_not_say_what_happens_when_it_fails_is_refused() {
        // Review F3: the words are required, and `drop` in particular has to be one someone typed.
        for (missing, document) in [
            (
                "delivery",
                "id: b\nwhen:\n  event: a.B\ninvoke:\n  command: a.C\non_failure: escalate\n",
            ),
            (
                "on_failure",
                "id: b\nwhen:\n  event: a.B\ninvoke:\n  command: a.C\ndelivery: at_least_once\n",
            ),
        ] {
            let error = serde_yaml::from_str::<RawBindingSpec>(document)
                .expect_err("a binding that can fail silently");
            assert!(error.to_string().contains(missing), "{missing}: {error}");
        }
    }

    /// The example's binding with `on_failure:` written however a test needs it.
    fn with_policy(policy: &str) -> Result<RawBindingSpec, serde_yaml::Error> {
        serde_yaml::from_str(&format!(
            "id: notify-on-invoice-created\nwhen:\n  event: {EVENT}\ninvoke:\n  command: \
             {COMMAND}\nmapping:\n{MAPPING}delivery: at_least_once\non_failure: {policy}\n"
        ))
    }

    #[test]
    fn an_escalation_that_names_no_event_is_refused_because_nothing_could_prove_it_happened() {
        // G2. `escalate` used to mean "surface it to a person" and nothing else, so a conformance
        // target could not be asked to prove escalation occurred. Three spellings of the same
        // omission, one refusal.
        for spelling in ["escalate", "\n  escalate:", "\n  escalate: {}"] {
            let raw = with_policy(spelling).expect("a document may still say this");
            let errors = BindingSpec::try_from(raw)
                .expect_err("an escalation nobody can observe: {spelling}");

            let error = only(&errors);
            assert_eq!(
                error.code,
                ValidationCode::MissingDeclaration,
                "{spelling:?}"
            );
            assert_eq!(
                error.location, "binding.notify-on-invoice-created.on_failure",
                "{spelling:?}"
            );
            assert!(
                hint(error).contains("emits:"),
                "the hint names the key to write: {error}"
            );
        }
    }

    #[test]
    fn an_escalation_names_the_event_it_emits_and_that_event_reaches_the_binding() {
        let binding = binding(MAPPING);
        assert_eq!(binding.failure, Failure::Escalate);
        assert_eq!(
            binding.escalation.as_ref().map(ToString::to_string),
            Some(ESCALATION.to_owned())
        );
        assert!(check(binding).is_empty());
    }

    #[test]
    fn a_binding_that_escalates_into_an_event_nothing_declares_is_refused() {
        // The same guarantee every other reference in the model has: a binding cannot escalate into
        // an event nobody declares, so the compiler always has a handle to resolve it to.
        let raw = with_policy("\n  escalate:\n    emits: billing.email.DeliveryEscalted")
            .expect("well formed");
        let binding = BindingSpec::try_from(raw).expect("a name is a name");
        let errors = check(binding);

        let error = only(&errors);
        assert_eq!(error.code, ValidationCode::UndeclaredReference);
        assert_eq!(
            error.location,
            "binding.notify-on-invoice-created.on_failure.escalate.emits"
        );
        assert!(
            hint(error).contains(ESCALATION),
            "the hint says what is declared: {error}"
        );
    }

    #[test]
    fn only_escalate_takes_a_block_because_it_is_the_only_policy_that_publishes_anything() {
        // `retry` is observable as another invocation and `drop` is unobservable on purpose, so
        // neither has anything to emit. Refused while the document is read: a scalar and a mapping
        // are different YAML nodes, so nothing has to guess which was meant.
        for word in ["retry", "drop"] {
            let error = with_policy(&format!("\n  {word}:\n    emits: {ESCALATION}"))
                .expect_err("only `escalate` publishes");
            assert!(error.to_string().contains(word), "{word}: {error}");
        }
    }

    #[test]
    fn one_on_failure_says_one_thing() {
        let error = with_policy("\n  escalate:\n    emits: billing.email.EmailSent\n  drop:")
            .expect_err("a binding has one policy");
        assert!(error.to_string().contains("one policy"), "{error}");
    }

    #[test]
    fn a_failure_policy_that_is_none_of_the_three_words_is_told_which_three_exist() {
        for spelling in ["dead_letter", "\n  dead_letter:\n    emits: a.B"] {
            let error = with_policy(spelling).expect_err("there are three");
            let message = error.to_string();
            for word in Failure::WORDS {
                assert!(message.contains(word), "{spelling:?}: {message}");
            }
        }
    }

    #[test]
    fn an_escalation_event_on_a_binding_that_does_not_escalate_is_refused() {
        // Unwritable in a document, because `retry:` and `drop:` take no block — but `BindingSpec`'s
        // fields are public, and a binding assembled in code has not been through the reader.
        let mut binding = binding(MAPPING);
        binding.failure = Failure::Drop;
        let errors = check(binding);

        let error = only(&errors);
        assert_eq!(error.code, ValidationCode::ConflictingDeclaration);
        assert_eq!(
            error.location,
            "binding.notify-on-invoice-created.on_failure"
        );
        assert!(error.message.contains("drop"), "{error}");
    }

    #[test]
    fn the_published_schema_offers_both_spellings_of_on_failure() {
        // A schema that only described the word would tell a document author that the block form is
        // invalid, and one that only described the block would say the reverse.
        let schema =
            serde_json::to_value(schemars::schema_for!(RawBindingSpec)).expect("serialises");
        let policy = &schema["definitions"]["BindingFailure"];
        let spellings = policy["oneOf"].as_array().expect("two spellings");
        assert_eq!(spellings.len(), 2, "{policy}");
        assert_eq!(
            spellings[0]["$ref"],
            serde_json::json!("#/definitions/Failure"),
            "{policy}"
        );
        assert_eq!(
            spellings[1]["required"],
            serde_json::json!(["escalate"]),
            "{policy}"
        );
        assert_eq!(
            spellings[1]["additionalProperties"],
            serde_json::json!(false),
            "only `escalate` takes a block: {policy}"
        );
    }

    #[test]
    fn a_delivery_guarantee_this_build_does_not_implement_is_refused_while_the_document_is_read() {
        let error = serde_yaml::from_str::<RawBindingSpec>(
            "id: b\nwhen:\n  event: a.B\ninvoke:\n  command: a.C\ndelivery: exactly_once\non_failure: escalate\n",
        )
        .expect_err("`exactly_once` is what everyone believes they have until a retry proves otherwise");
        assert!(error.to_string().contains("at_least_once"), "{error}");
    }

    #[test]
    fn a_binding_name_is_lower_kebab_because_it_becomes_a_generated_name() {
        for spelling in ["", "Notify", "notify_on_invoice", "notify--on", "notify-"] {
            let error = BindingName::new(spelling).expect_err(spelling);
            assert!(
                error.to_string().contains("binding name"),
                "{spelling:?}: {error}"
            );
        }
        assert_eq!(
            BindingName::new("notify-on-invoice-created")
                .expect("a binding name")
                .as_str(),
            "notify-on-invoice-created"
        );
    }

    #[test]
    fn a_mapping_source_renders_as_the_document_wrote_it() {
        // Diagnostics quote the author rather than the model, and the raw table serialises through
        // this too.
        assert_eq!(
            MappingSource::parse("event.customer_email").to_string(),
            "event.customer_email"
        );
        assert_eq!(
            MappingSource::parse("invoice-created").to_string(),
            "invoice-created"
        );
    }

    #[test]
    fn the_published_schema_still_describes_a_mapping_as_an_object_of_strings() {
        // The raw mapping is a list so that a repeated key can be reported. That is an internal
        // choice, and a document must not be able to see it.
        let schema =
            serde_json::to_value(schemars::schema_for!(RawBindingSpec)).expect("serialises");
        let mapping = &schema["properties"]["mapping"];
        assert_eq!(mapping["type"], serde_json::json!("object"), "{mapping}");
        assert_eq!(
            mapping["additionalProperties"],
            serde_json::json!({"type": "string"}),
            "{mapping}"
        );
        assert_eq!(mapping["default"], serde_json::json!({}), "{mapping}");
    }
}
