//! The type system.
//!
//! Semantic types stay distinct from their representations. `Email` and `InvoiceId` are both strings
//! on the wire and are not interchangeable in the model, because the whole value of a specification
//! is that it can refuse a binding which maps one into the other.
//!
//! ```text
//! primitive   String Boolean Integer Decimal Timestamp Duration Uuid Bytes
//! composite   Struct Enum Optional List Map Union(tagged)
//! named       Email  Money  InvoiceId       — distinct even when representations match
//! ```
//!
//! # Unions are tagged
//!
//! An untagged union cannot round-trip through JSON Schema, `OpenAPI` or Serde without ambiguity: two
//! variants with the same shape are indistinguishable on the way back. The model therefore carries
//! the tag field, and an untagged form is not offered rather than being offered with a warning.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use aep_domain::error::{ParseError, ValidationCode, ValidationError, ValidationErrors};

use crate::entity::{Invariant, RawInvariant};
use crate::name::{Naming, QualifiedName};

/// A type with no structure of its own.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Primitive {
    /// Text.
    String,
    /// True or false.
    Boolean,
    /// A whole number.
    Integer,
    /// An exact decimal. Never a float: money does not round the way a float does.
    Decimal,
    /// An instant.
    Timestamp,
    /// A length of time.
    Duration,
    /// A UUID.
    Uuid,
    /// Opaque bytes.
    Bytes,
}

impl Primitive {
    /// Every primitive.
    pub const ALL: &'static [Self] = &[
        Self::String,
        Self::Boolean,
        Self::Integer,
        Self::Decimal,
        Self::Timestamp,
        Self::Duration,
        Self::Uuid,
        Self::Bytes,
    ];

    /// The primitive as written in a specification.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::String => "String",
            Self::Boolean => "Boolean",
            Self::Integer => "Integer",
            Self::Decimal => "Decimal",
            Self::Timestamp => "Timestamp",
            Self::Duration => "Duration",
            Self::Uuid => "Uuid",
            Self::Bytes => "Bytes",
        }
    }

    /// Parses a primitive name.
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|kind| kind.as_str() == value)
    }
}

impl fmt::Display for Primitive {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A reference to a type, as written in a specification.
///
/// `Email`, `List<Money>`, `Optional<CustomerId>`, `Map<String, Money>`.
///
/// # Depth
///
/// Every value of this type that a document can produce is at most [`MAX_TYPE_DEPTH`] deep, because
/// [`TypeRef::parse`] is the only way a document reaches one — [`serde::Deserialize`] goes through
/// it, and the compiler's two conversions
/// (`Resolver::type_ref` and `spec_type_ref`) map one constructor to one constructor and so preserve
/// depth exactly. That is what lets the walkers below — [`Self::named_dependencies`],
/// [`Self::required`], [`Display`](fmt::Display), [`is_assignable`], and the ones in [`crate::view`],
/// [`crate::system`] and [`crate::binding`] — recurse without counting: at 32 levels the deepest of
/// them uses a few kilobytes of stack. It stops being true the moment something builds a `TypeRef`
/// by wrapping rather than by parsing, which is why the bound lives in the parser and this note
/// lives on the type.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TypeRef {
    /// A primitive.
    Primitive(Primitive),
    /// A named type declared elsewhere in the specification.
    Named(QualifiedName),
    /// A value that may be absent.
    Optional(Box<TypeRef>),
    /// An ordered sequence.
    List(Box<TypeRef>),
    /// A mapping. The key must be a primitive, because a structured key has no stable wire form.
    Map(Primitive, Box<TypeRef>),
}

/// How many generic wrappers a type reference may nest before it is refused.
///
/// A document chooses this number by writing it — `type: Optional<Optional<…>>` is one YAML scalar,
/// and nothing above this function bounds a scalar's length. Measured on this machine, a debug
/// build of [`TypeRef::parse`] overflows the 8 MiB main-thread stack between 3 000 and 4 000
/// wrappers, and the 2 MiB a spawned worker gets between 800 and 1 000; a stack overflow is an abort
/// with no diagnostic, which is the one failure mode this compiler does not otherwise have.
///
/// 32 because the deepest type a real specification writes is
/// `Optional<List<Map<String, Optional<Money>>>>`, which is five, and because
/// `WRAPPER_LIMIT` in [`crate::binding`] already chose 32 for the same reason in the same crate: one
/// number is easier to defend than two. That leaves a factor of twenty-five below the smallest
/// measured floor, and a factor of six above anything anybody has written. `serde_yaml`'s own
/// structural recursion cap is 128, so this refusal is reached first and carries our message rather
/// than the deserializer's.
pub const MAX_TYPE_DEPTH: usize = 32;

impl TypeRef {
    /// Parses a type reference.
    ///
    /// Refuses nesting beyond [`MAX_TYPE_DEPTH`] with [`ParseError::TooDeep`]. The check is made on
    /// the way down, before the `Box` for that level is allocated, so a refused document never
    /// builds the chain whose `Drop` would recurse just as deeply as the parse did.
    pub fn parse(value: &str) -> Result<Self, ParseError> {
        Self::parse_nested(value, 0)
    }

    /// [`Self::parse`], counting how many wrappers deep it already is.
    fn parse_nested(value: &str, depth: usize) -> Result<Self, ParseError> {
        let trimmed = value.trim();
        let reject = |reason: &str| ParseError::identifier("type", value, reason.to_owned());

        if depth > MAX_TYPE_DEPTH {
            return Err(ParseError::too_deep("type", trimmed, MAX_TYPE_DEPTH));
        }

        if let Some(inner) = generic_argument(trimmed, "Optional") {
            return Ok(Self::Optional(Box::new(Self::parse_nested(
                inner,
                depth + 1,
            )?)));
        }
        if let Some(inner) = generic_argument(trimmed, "List") {
            return Ok(Self::List(Box::new(Self::parse_nested(inner, depth + 1)?)));
        }
        if let Some(inner) = generic_argument(trimmed, "Map") {
            let (key, value_type) = inner.split_once(',').ok_or_else(|| {
                reject("a map needs a key and a value, as in `Map<String, Money>`")
            })?;
            let key = Primitive::parse(key.trim()).ok_or_else(|| {
                ParseError::identifier(
                    "type",
                    value,
                    format!(
                        "a map key must be a primitive, not {:?}; a structured key has no stable \
                         wire form",
                        key.trim()
                    ),
                )
            })?;
            return Ok(Self::Map(
                key,
                Box::new(Self::parse_nested(value_type, depth + 1)?),
            ));
        }
        if trimmed.contains(['<', '>']) {
            return Err(reject(
                "unknown generic; the model has `Optional<T>`, `List<T>` and `Map<K, V>`",
            ));
        }
        if let Some(primitive) = Primitive::parse(trimmed) {
            return Ok(Self::Primitive(primitive));
        }
        Ok(Self::Named(QualifiedName::new(trimmed)?))
    }

    /// Every named type this reference depends on.
    pub fn named_dependencies(&self) -> Vec<&QualifiedName> {
        match self {
            Self::Primitive(_) => Vec::new(),
            Self::Named(name) => vec![name],
            Self::Optional(inner) | Self::List(inner) | Self::Map(_, inner) => {
                inner.named_dependencies()
            }
        }
    }

    /// `true` when a value of this type may be absent.
    pub fn is_optional(&self) -> bool {
        matches!(self, Self::Optional(_))
    }

    /// This reference with any `Optional` wrapper removed.
    pub fn required(&self) -> &Self {
        match self {
            Self::Optional(inner) => inner.required(),
            other => other,
        }
    }
}

/// Extracts `T` from `Name<T>`.
fn generic_argument<'a>(value: &'a str, name: &str) -> Option<&'a str> {
    let rest = value.strip_prefix(name)?;
    let rest = rest.trim_start().strip_prefix('<')?;
    rest.trim_end().strip_suffix('>').map(str::trim)
}

impl fmt::Display for TypeRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Primitive(primitive) => write!(f, "{primitive}"),
            Self::Named(name) => write!(f, "{name}"),
            Self::Optional(inner) => write!(f, "Optional<{inner}>"),
            Self::List(inner) => write!(f, "List<{inner}>"),
            Self::Map(key, value) => write!(f, "Map<{key}, {value}>"),
        }
    }
}

impl std::str::FromStr for TypeRef {
    type Err = ParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

impl serde::Serialize for TypeRef {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> serde::Deserialize<'de> for TypeRef {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        Self::parse(&raw).map_err(serde::de::Error::custom)
    }
}

impl schemars::JsonSchema for TypeRef {
    fn schema_name() -> String {
        "TypeRef".to_owned()
    }

    fn json_schema(_: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        let mut schema = schemars::schema::SchemaObject {
            instance_type: Some(schemars::schema::InstanceType::String.into()),
            ..Default::default()
        };
        schema.metadata().description = Some(
            "A type: a primitive, a named type, or `Optional<T>`, `List<T>` or `Map<K, V>`."
                .to_owned(),
        );
        schema.metadata().examples = ["Email", "Money", "Optional<CustomerId>", "List<Money>"]
            .iter()
            .map(|value| serde_json::Value::String((*value).to_owned()))
            .collect();
        schema.into()
    }
}

/// One field of a struct or an event.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct Field {
    /// Its name.
    #[serde(deserialize_with = "deserialize_field_name")]
    #[schemars(regex(pattern = "^[A-Za-z][A-Za-z0-9_]*$"))]
    pub name: String,
    /// Its type.
    #[serde(rename = "type")]
    pub type_ref: TypeRef,
    /// What it is on the wire, and what a person is shown.
    #[serde(default, flatten, skip_serializing_if = "Naming::is_empty")]
    pub naming: Naming,
}

impl Field {
    /// The pattern published in generated JSON Schema.
    ///
    /// Kept beside the parser that enforces it, and a test asserts the published schema carries
    /// this one: a schema that accepts what the parser refuses is worse than no schema.
    pub const PATTERN: &'static str = "^[A-Za-z][A-Za-z0-9_]*$";

    /// A field with no naming overrides.
    pub fn new(name: impl Into<String>, type_ref: TypeRef) -> Self {
        Self {
            name: name.into(),
            type_ref,
            naming: Naming::default(),
        }
    }
}

/// Checks that `value` can be spelled as a field name.
///
/// A field name is not decoration: it becomes a struct field in generated code, a key on the wire
/// and a property in an `OpenAPI` document, so `""` and `not a field name!` each become three
/// things nobody can spell, in files where the specification that wrote them is no longer in view.
/// [`StateName`](crate::entity::StateName), [`OutcomeName`](crate::command::OutcomeName) and
/// [`QualifiedName`] check theirs for the same reason.
fn field_name(value: &str) -> Result<String, ParseError> {
    let reject = |reason: String| Err(ParseError::identifier("field name", value, reason));

    let Some(first) = value.chars().next() else {
        return reject("must not be empty".to_owned());
    };
    if !first.is_ascii_alphabetic() {
        return reject(format!(
            "must start with a letter, as in `invoice_id`, got {first:?}"
        ));
    }
    for character in value.chars() {
        if !(character.is_ascii_alphanumeric() || character == '_') {
            return reject(format!(
                "contains {character:?}; a field name has to survive into generated code as an \
                 identifier"
            ));
        }
    }
    Ok(value.to_owned())
}

/// Serde entry point for [`field_name`], so a name nothing could generate is refused while the
/// document is read rather than surviving into the model.
fn deserialize_field_name<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<String, D::Error> {
    let raw = <String as serde::Deserialize>::deserialize(deserializer)?;
    field_name(&raw).map_err(serde::de::Error::custom)
}

/// What a named type is made of.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TypeBody {
    /// A wrapper around one representation, distinct from it.
    ///
    /// This is what makes `Email` refusable where `String` is expected.
    Newtype {
        /// What it wraps.
        #[serde(rename = "of")]
        of: TypeRef,
        /// Conditions every value must satisfy, as predicates over `value`, what it wraps.
        #[serde(skip_serializing_if = "Vec::is_empty")]
        invariants: Vec<Invariant>,
    },
    /// Named fields.
    Struct {
        /// Its fields.
        fields: Vec<Field>,
        /// Conditions every value must satisfy, as predicates over those fields.
        #[serde(skip_serializing_if = "Vec::is_empty")]
        invariants: Vec<Invariant>,
    },
    /// One of a fixed set of names.
    Enum {
        /// The variants, in declaration order.
        variants: Vec<String>,
    },
    /// One of several shapes, distinguished by a tag field.
    ///
    /// Always tagged: an untagged union does not round-trip, because two variants with the same
    /// shape cannot be told apart on the way back.
    Union {
        /// The field carrying the variant's name.
        tag: String,
        /// The variants, by tag value.
        variants: BTreeMap<String, TypeRef>,
    },
}

/// A named type in a specification.
///
/// The body is flattened, so a declaration reads as one object: `{name, kind, fields}` rather than
/// `{name, body: {kind, fields}}`. A document becomes one of these through
/// [`RawNamedType`], which is where a misspelled key is refused.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct NamedType {
    /// Its stable identity.
    pub name: QualifiedName,
    /// What it is made of.
    #[serde(flatten)]
    pub body: TypeBody,
    /// What it is called on the wire and shown as.
    #[serde(skip_serializing_if = "Naming::is_empty")]
    pub naming: Naming,
}

impl NamedType {
    /// The pseudo-field a newtype's invariants read.
    ///
    /// A newtype has no fields — it is one representation under another name — so an invariant on
    /// it names what it wraps, the way an entity's invariant names
    /// [`state`](crate::entity::EntitySpec::STATE) for a lifecycle no field carries.
    pub const VALUE: &'static str = "value";

    /// Refuses a body that declares nothing.
    ///
    /// A type with no variants and a struct with no fields both parse, and both name something no
    /// value can be: every generator downstream would emit an enum with no cases or a struct with no
    /// members, and the first place anyone notices is a compiler error in generated code, where the
    /// mistake is furthest from the document that caused it.
    fn check_shape(&self) -> ValidationErrors {
        let mut errors = ValidationErrors::new();
        let at = |part: &str| format!("types.{}.{part}", self.name);

        let (part, empty) = match &self.body {
            TypeBody::Enum { variants } => ("variants", variants.is_empty()),
            TypeBody::Union { variants, .. } => ("variants", variants.is_empty()),
            TypeBody::Struct { fields, .. } => ("fields", fields.is_empty()),
            TypeBody::Newtype { .. } => ("of", false),
        };
        if empty {
            errors.push(
                ValidationError::new(
                    ValidationCode::EmptyDeclaration,
                    at(part),
                    format!("`{}` declares no {part}, so no value can be one", self.name),
                )
                .with_hint("give it at least one, or delete the declaration"),
            );
        }

        // A tagged union whose tag collides with a variant's own field is decodable only by luck.
        if let TypeBody::Union { tag, .. } = &self.body {
            if tag.is_empty() {
                errors.push(
                    ValidationError::new(
                        ValidationCode::EmptyDeclaration,
                        at("tag"),
                        format!("`{}` is a union with no tag field", self.name),
                    )
                    .with_hint(
                        "an untagged union cannot be decoded without guessing; name the field that \
                         carries the variant",
                    ),
                );
            }
        }

        errors
    }

    /// Every named type this one depends on.
    pub fn dependencies(&self) -> Vec<&QualifiedName> {
        match &self.body {
            TypeBody::Newtype { of, .. } => of.named_dependencies(),
            TypeBody::Struct { fields, .. } => fields
                .iter()
                .flat_map(|field| field.type_ref.named_dependencies())
                .collect(),
            TypeBody::Enum { .. } => Vec::new(),
            TypeBody::Union { variants, .. } => variants
                .values()
                .flat_map(TypeRef::named_dependencies)
                .collect(),
        }
    }

    /// The field with this name, for a struct.
    pub fn field(&self, name: &str) -> Option<&Field> {
        match &self.body {
            TypeBody::Struct { fields, .. } => fields.iter().find(|field| field.name == name),
            _ => None,
        }
    }

    /// Checks that every invariant reads something this type has (§36.6).
    ///
    /// Only this type's own fields are resolvable here: the check runs while the declaration is
    /// being converted, before any [`TypeRegistry`] exists, so a path that leaves the type
    /// (`total.amount`) is checked as far as `total` and no further — which is also as far as
    /// [`EntitySpec::validate`](crate::entity::EntitySpec::validate) can check one without the
    /// registry.
    fn check_invariants(&self) -> ValidationErrors {
        let mut errors = ValidationErrors::new();
        let (invariants, readable, hint) = match &self.body {
            TypeBody::Newtype { invariants, .. } => (
                invariants,
                vec![Self::VALUE.to_owned()],
                format!(
                    "a newtype has no fields; its invariants read `{}`, the representation it wraps",
                    Self::VALUE
                ),
            ),
            TypeBody::Struct { fields, invariants } => {
                let names: Vec<String> =
                    fields.iter().map(|field| field.name.clone()).collect();
                let hint = format!("readable here: {}", names.join(", "));
                (invariants, names, hint)
            }
            TypeBody::Enum { .. } | TypeBody::Union { .. } => return errors,
        };

        for (index, invariant) in invariants.iter().enumerate() {
            let at = format!("types.{}.invariants[{index}]", self.name);
            for path in invariant.predicate.fact_paths() {
                let root = path.namespace();
                if readable.iter().any(|name| name == root) {
                    continue;
                }
                errors.push(
                    ValidationError::new(
                        ValidationCode::UnobservableFact,
                        at.clone(),
                        format!(
                            "`{invariant}` reads `{path}`, and `{root}` is not a field of `{}`",
                            self.name
                        ),
                    )
                    .with_hint(hint.clone()),
                );
            }
        }

        errors
    }
}

impl schemars::JsonSchema for NamedType {
    fn schema_name() -> String {
        <RawNamedType as schemars::JsonSchema>::schema_name()
    }

    // Delegated to the raw form, because the published schema has to describe what a document may
    // say, and the raw form is what a document is read into.
    fn json_schema(generator: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        <RawNamedType as schemars::JsonSchema>::json_schema(generator)
    }
}

/// What a named type is made of, as parsed.
///
/// `deny_unknown_fields` sits here rather than on [`RawNamedType`], which cannot carry it: serde
/// ignores the attribute on a struct with a flattened field. The body is what the keys nothing else
/// claimed are offered to, so refusing an unknown one here is what refuses `invarants:` and
/// `namin:` — each of which used to parse clean and silently drop what the author wrote.
#[derive(Debug, Clone, serde::Deserialize, schemars::JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum RawTypeBody {
    /// A wrapper around one representation, distinct from it.
    Newtype {
        /// What it wraps.
        #[serde(rename = "of")]
        of: TypeRef,
        /// Conditions every value must satisfy, as predicates over `value`, what it wraps.
        #[serde(default)]
        invariants: Vec<RawInvariant>,
    },
    /// Named fields.
    Struct {
        /// Its fields.
        fields: Vec<Field>,
        /// Conditions every value must satisfy, as predicates over those fields.
        #[serde(default)]
        invariants: Vec<RawInvariant>,
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
        variants: BTreeMap<String, TypeRef>,
    },
}

impl From<RawTypeBody> for TypeBody {
    fn from(raw: RawTypeBody) -> Self {
        match raw {
            RawTypeBody::Newtype { of, invariants } => Self::Newtype {
                of,
                invariants: invariants.into_iter().map(Invariant::from).collect(),
            },
            RawTypeBody::Struct { fields, invariants } => Self::Struct {
                fields,
                invariants: invariants.into_iter().map(Invariant::from).collect(),
            },
            RawTypeBody::Enum { variants } => Self::Enum { variants },
            RawTypeBody::Union { tag, variants } => Self::Union { tag, variants },
        }
    }
}

/// A named type, as parsed.
///
/// Everything a declaration can get wrong on its own is settled while it is read — an unspellable
/// name, an unparsable invariant, a key the model does not know — so the only question left when
/// it is converted is whether the invariants read fields the type has.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct RawNamedType {
    /// Its stable identity.
    pub name: QualifiedName,
    /// What it is made of.
    #[serde(flatten)]
    pub body: RawTypeBody,
    /// What it is called on the wire and shown as.
    #[serde(default)]
    pub naming: Naming,
}

impl schemars::JsonSchema for RawNamedType {
    fn schema_name() -> String {
        "NamedType".to_owned()
    }

    // Written by hand for the reason [`Version`](crate::name::Version)'s is: the derived schema
    // describes something no document says. `deny_unknown_fields` on the body renders as
    // `additionalProperties: false` in each branch of its `oneOf`, and a branch cannot see the keys
    // the flattened outer struct supplies — so the derived schema calls every real declaration
    // invalid, `examples/billing/` included. Putting `name` and `naming` where the branch can see
    // them is what lets the published schema refuse `invarants:` exactly where the parser does,
    // rather than refusing everything or nothing.
    fn json_schema(generator: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        let mut schema = match <RawTypeBody as schemars::JsonSchema>::json_schema(generator) {
            schemars::schema::Schema::Object(object) => object,
            boolean @ schemars::schema::Schema::Bool(_) => return boolean,
        };
        let name = described(
            generator.subschema_for::<QualifiedName>(),
            "Its stable identity.",
        );
        let naming = described(
            generator.subschema_for::<Naming>(),
            "What it is called on the wire and shown as.",
        );

        let object = schema.object();
        object.properties.insert("name".to_owned(), name.clone());
        object
            .properties
            .insert("naming".to_owned(), naming.clone());
        object.required.insert("name".to_owned());
        for branch in schema.subschemas().one_of.iter_mut().flatten() {
            let schemars::schema::Schema::Object(branch) = branch else {
                continue;
            };
            let object = branch.object();
            object.properties.insert("name".to_owned(), name.clone());
            object
                .properties
                .insert("naming".to_owned(), naming.clone());
            object.required.insert("name".to_owned());
        }

        schema.into()
    }
}

/// A `$ref` with a description beside it — the shape a derived schema gives a documented field.
fn described(schema: schemars::schema::Schema, description: &str) -> schemars::schema::Schema {
    let mut wrapper = schemars::schema::SchemaObject::default();
    wrapper.subschemas().all_of = Some(vec![schema]);
    wrapper.metadata().description = Some(description.to_owned());
    wrapper.into()
}

impl TryFrom<RawNamedType> for NamedType {
    type Error = ValidationErrors;

    fn try_from(raw: RawNamedType) -> Result<Self, Self::Error> {
        let declared = Self {
            name: raw.name,
            body: raw.body.into(),
            naming: raw.naming,
        };
        let mut errors = declared.check_shape();
        errors.extend(declared.check_invariants());
        errors.into_result(declared)
    }
}

/// A conversion someone decided to allow.
///
/// Design §20 requires that a mapping's two types "be compatible, or an explicit conversion must
/// exist". This is that conversion, and it is a declaration rather than an inference on purpose:
/// `Email` and `VerifiedEmail` are both a `String` underneath, and the entire value of naming them
/// apart is that the model refuses to treat one as the other. Someone has to write down that this
/// particular crossing is intended, so that the next reader can find who decided it.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct Conversion {
    /// The type a value has.
    pub from: TypeRef,
    /// The type it may be used as.
    pub to: TypeRef,
    /// Why this crossing is allowed.
    ///
    /// Required. A conversion with no reason is the thing this declaration exists to prevent: a
    /// silent widening that someone added to make a build pass.
    pub because: String,
}

/// Every conversion a specification declares.
///
/// Directional: declaring `Email → EmailAddress` does not permit the reverse, because the reverse is
/// usually the unsafe direction and nobody would notice it being granted.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
#[serde(transparent)]
pub struct ConversionRegistry {
    declared: BTreeSet<Conversion>,
}

impl ConversionRegistry {
    /// An empty registry: nothing crosses.
    pub fn new() -> Self {
        Self::default()
    }

    /// Records one, reporting a second declaration of the same crossing.
    pub fn insert(&mut self, conversion: Conversion) -> Result<(), ValidationError> {
        if let Some(existing) = self
            .declared
            .iter()
            .find(|candidate| candidate.from == conversion.from && candidate.to == conversion.to)
        {
            return Err(ValidationError::new(
                ValidationCode::DuplicateDeclaration,
                format!("conversions.{} -> {}", conversion.from, conversion.to),
                format!(
                    "this crossing is already declared, because `{}`",
                    existing.because
                ),
            )
            .with_hint(
                "one crossing, one reason; two reasons means the decision was not settled",
            ));
        }
        self.declared.insert(conversion);
        Ok(())
    }

    /// `true` when a value of `source` may be used where `target` is expected.
    ///
    /// Structural compatibility first, then a declared crossing. The order matters only for
    /// diagnostics: a declared conversion between identical types is redundant, not wrong.
    pub fn permits(&self, source: &TypeRef, target: &TypeRef) -> bool {
        is_assignable(source, target)
            || self
                .declared
                .iter()
                .any(|conversion| &conversion.from == source && &conversion.to == target)
    }

    /// Every conversion, in a stable order.
    pub fn iter(&self) -> impl Iterator<Item = &Conversion> {
        self.declared.iter()
    }

    /// How many.
    pub fn len(&self) -> usize {
        self.declared.len()
    }

    /// `true` when nothing is declared.
    pub fn is_empty(&self) -> bool {
        self.declared.is_empty()
    }
}

/// Every named type in a specification, indexed by identity.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
#[serde(transparent)]
pub struct TypeRegistry {
    types: BTreeMap<QualifiedName, NamedType>,
}

impl TypeRegistry {
    /// An empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a type, refusing a second declaration of the same name.
    pub fn insert(&mut self, declared: NamedType) -> Result<(), ValidationError> {
        if self.types.contains_key(&declared.name) {
            return Err(ValidationError::new(
                ValidationCode::DuplicateDeclaration,
                format!("types.{}", declared.name),
                format!("`{}` is declared more than once", declared.name),
            ));
        }
        self.types.insert(declared.name.clone(), declared);
        Ok(())
    }

    /// The type with this name.
    pub fn get(&self, name: &QualifiedName) -> Option<&NamedType> {
        self.types.get(name)
    }

    /// Every type, in name order.
    pub fn iter(&self) -> impl Iterator<Item = &NamedType> {
        self.types.values()
    }

    /// How many types are declared.
    pub fn len(&self) -> usize {
        self.types.len()
    }

    /// `true` when nothing is declared.
    pub fn is_empty(&self) -> bool {
        self.types.is_empty()
    }

    /// Checks that every named type a reference mentions exists.
    pub fn resolve(&self, reference: &TypeRef, location: &str) -> ValidationErrors {
        let mut errors = ValidationErrors::new();
        for name in reference.named_dependencies() {
            if !self.types.contains_key(name) {
                errors.push(
                    ValidationError::new(
                        ValidationCode::UndeclaredReference,
                        location.to_owned(),
                        format!("`{name}` is not a declared type"),
                    )
                    .with_hint(format!(
                        "declared types: {}",
                        self.types
                            .keys()
                            .map(ToString::to_string)
                            .collect::<Vec<_>>()
                            .join(", ")
                    )),
                );
            }
        }
        errors
    }
}

/// `true` when a value of `source` can be used where `target` is expected.
///
/// Deliberately strict: identical, or an optional target accepting a required source. Nothing else,
/// because the value of naming `Email` separately from `String` is entirely in the conversions this
/// refuses.
///
/// A free function rather than a method on [`TypeRegistry`], because it never consults one: two
/// named types are assignable exactly when they are the same name, so there is nothing to resolve.
/// Taking a registry it ignores would suggest the answer could depend on what is declared.
pub fn is_assignable(source: &TypeRef, target: &TypeRef) -> bool {
    if source == target {
        return true;
    }
    if let TypeRef::Optional(inner) = target {
        return is_assignable(source, inner);
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn name(value: &str) -> QualifiedName {
        QualifiedName::new(value).expect("a valid name")
    }

    fn declared(yaml: &str) -> Result<NamedType, ValidationErrors> {
        let raw: RawNamedType = serde_yaml::from_str(yaml).expect("the document is well formed");
        NamedType::try_from(raw)
    }

    fn registry() -> TypeRegistry {
        let mut registry = TypeRegistry::new();
        registry
            .insert(NamedType {
                name: name("billing.Email"),
                body: TypeBody::Newtype {
                    of: TypeRef::Primitive(Primitive::String),
                    invariants: Vec::new(),
                },
                naming: Naming::default(),
            })
            .expect("new");
        registry
            .insert(NamedType {
                name: name("billing.Money"),
                body: TypeBody::Struct {
                    fields: vec![
                        Field::new("amount", TypeRef::Primitive(Primitive::Decimal)),
                        Field::new("currency", TypeRef::Primitive(Primitive::String)),
                    ],
                    invariants: vec![Invariant::parse("amount >= 0").expect("a predicate")],
                },
                naming: Naming::default(),
            })
            .expect("new");
        registry
    }

    #[test]
    fn type_references_parse_and_round_trip() {
        for spelling in [
            "Email",
            "String",
            "Optional<billing.CustomerId>",
            "List<billing.Money>",
            "Map<String, billing.Money>",
        ] {
            let parsed = TypeRef::parse(spelling).expect("parses");
            assert_eq!(parsed.to_string(), spelling, "round trip");
        }
    }

    #[test]
    fn the_deepest_type_a_real_specification_writes_is_still_accepted() {
        // The failure mode of a depth bound is refusing a good document, so the bound is asserted
        // from the accepting side first: this is the deepest thing anybody writes, and it is five.
        let spelling = "Optional<List<Map<String, Optional<billing.Money>>>>";
        let parsed = TypeRef::parse(spelling).expect("a real specification's deepest type");
        assert_eq!(parsed.to_string(), spelling, "round trip");

        // And the whole budget, exactly, one wrapper short of the refusal.
        let at_limit = format!(
            "{}String{}",
            "Optional<".repeat(MAX_TYPE_DEPTH),
            ">".repeat(MAX_TYPE_DEPTH)
        );
        assert!(
            TypeRef::parse(&at_limit).is_ok(),
            "{MAX_TYPE_DEPTH} wrappers is the limit, not one past it"
        );
    }

    #[test]
    fn a_type_nested_past_the_limit_is_refused_rather_than_overflowing_the_stack() {
        // 10 000 wrappers overflowed an 8 MiB stack before this bound existed; the point of the
        // test is that the answer is a refusal a caller can read, not an abort.
        let deep = format!("{}String{}", "Optional<".repeat(10_000), ">".repeat(10_000));
        let error = TypeRef::parse(&deep).expect_err("nesting past the limit");
        assert!(
            matches!(
                error,
                ParseError::TooDeep {
                    kind: "type",
                    limit: MAX_TYPE_DEPTH,
                    ..
                }
            ),
            "the refusal names the construct and the limit: {error:?}"
        );
    }

    #[test]
    fn a_refused_type_is_not_built_so_dropping_it_cannot_overflow_either() {
        // `TypeRef` is `Box`-recursive, so a chain deep enough to overflow the parser would also
        // overflow its own `Drop`, and refusing late would not save the stack. The bound is checked
        // on the way down: 10 000 wrappers refuse without ever allocating the tenth box.
        let deep = format!("{}String{}", "Optional<".repeat(10_000), ">".repeat(10_000));
        let refused = TypeRef::parse(&deep);
        assert!(refused.is_err());
        drop(refused);

        // The deepest value the parser will build, dropped explicitly.
        let accepted = TypeRef::parse(&format!(
            "{}String{}",
            "Optional<".repeat(MAX_TYPE_DEPTH),
            ">".repeat(MAX_TYPE_DEPTH)
        ))
        .expect("at the limit");
        drop(accepted);
    }

    #[test]
    fn a_structured_map_key_is_refused() {
        let error = TypeRef::parse("Map<billing.Money, String>").expect_err("structured key");
        assert!(
            error.to_string().contains("no stable wire form"),
            "the refusal must say why: {error}"
        );
    }

    #[test]
    fn an_unknown_generic_is_refused_rather_than_read_as_a_name() {
        let error = TypeRef::parse("Set<Email>").expect_err("unknown generic");
        assert!(error.to_string().contains("Optional<T>"), "{error}");
    }

    #[test]
    fn a_named_type_is_not_its_representation() {
        let email = TypeRef::Named(name("billing.Email"));
        let text = TypeRef::Primitive(Primitive::String);

        assert!(!is_assignable(&email, &text));
        assert!(
            !is_assignable(&text, &email),
            "the entire value of naming `Email` is in this refusal"
        );
        assert!(is_assignable(&email, &email));
    }

    #[test]
    fn a_required_value_satisfies_an_optional_target_and_not_the_reverse() {
        let email = TypeRef::Named(name("billing.Email"));
        let maybe_email = TypeRef::Optional(Box::new(email.clone()));

        assert!(is_assignable(&email, &maybe_email));
        assert!(
            !is_assignable(&maybe_email, &email),
            "a value that may be absent cannot fill a slot that must be present"
        );
        assert!(maybe_email.is_optional());
        assert_eq!(maybe_email.required(), &email);
    }

    #[test]
    fn an_unresolved_type_is_reported_with_what_is_available() {
        let registry = registry();
        let errors = registry.resolve(
            &TypeRef::parse("List<billing.Invoice>").expect("parses"),
            "command.CreateInvoice.input",
        );
        assert_eq!(errors.len(), 1);
        let rendered = errors.to_string();
        assert!(rendered.contains("billing.Invoice"), "{rendered}");
        assert!(
            rendered.contains("billing.Email"),
            "and what was available: {rendered}"
        );
    }

    #[test]
    fn a_type_declared_twice_is_refused() {
        let mut registry = registry();
        let error = registry
            .insert(NamedType {
                name: name("billing.Email"),
                body: TypeBody::Enum {
                    variants: vec!["Work".to_owned(), "Personal".to_owned()],
                },
                naming: Naming::default(),
            })
            .expect_err("already declared");
        assert!(error.to_string().contains("more than once"), "{error}");
    }

    #[test]
    fn dependencies_are_reported_through_composites() {
        let declared = NamedType {
            name: name("billing.Basket"),
            body: TypeBody::Struct {
                fields: vec![
                    Field::new(
                        "lines",
                        TypeRef::parse("List<billing.Money>").expect("parses"),
                    ),
                    Field::new(
                        "owner",
                        TypeRef::parse("Optional<billing.Email>").expect("parses"),
                    ),
                ],
                invariants: Vec::new(),
            },
            naming: Naming::default(),
        };
        let dependencies: Vec<String> = declared
            .dependencies()
            .iter()
            .map(ToString::to_string)
            .collect();
        assert_eq!(dependencies, vec!["billing.Money", "billing.Email"]);
    }

    #[test]
    fn a_type_declaration_reads_as_one_object() {
        let money = declared(
            "name: billing.invoice.Money\nkind: struct\nfields:\n  - name: amount\n    type: Decimal\ninvariants: [amount >= 0]\n",
        )
        .expect("the flattened form parses");
        assert_eq!(money.name.to_string(), "billing.invoice.Money");
        let TypeBody::Struct { fields, invariants } = &money.body else {
            panic!("expected a struct");
        };
        assert_eq!(fields.len(), 1);
        assert_eq!(
            invariants[0].statement, "amount >= 0",
            "the author's own spelling is kept"
        );

        let email = declared("name: billing.invoice.Email\nkind: newtype\nof: String\n")
            .expect("the newtype form parses");
        assert!(matches!(email.body, TypeBody::Newtype { .. }));
    }

    #[test]
    fn a_type_invariant_that_is_not_a_predicate_is_refused() {
        let error = serde_yaml::from_str::<RawNamedType>(
            "name: billing.invoice.Money\nkind: struct\nfields:\n  - name: amount\n    type: Decimal\ninvariants: [\"))) this is not a predicate\"]\n",
        )
        .expect_err("a value object's invariants are predicates, exactly like an entity's");
        assert!(
            error
                .to_string()
                .contains("a predicate is either a comparison"),
            "the refusal says what an invariant is: {error}"
        );
    }

    #[test]
    fn a_type_invariant_that_reads_a_field_the_type_does_not_have_is_refused() {
        let errors = declared(
            "name: billing.invoice.Money\nkind: struct\nfields:\n  - name: amount\n    type: Decimal\ninvariants: [nonexistent_field >= 0]\n",
        )
        .expect_err("`nonexistent_field` is not a field of `Money`");
        assert_eq!(errors.len(), 1, "{errors}");
        let error = &errors.as_slice()[0];
        assert_eq!(error.code, ValidationCode::UnobservableFact);
        assert_eq!(error.location, "types.billing.invoice.Money.invariants[0]");
        assert!(
            error
                .message
                .contains("`nonexistent_field` is not a field of"),
            "{error}"
        );
        assert!(
            error
                .hint
                .as_deref()
                .unwrap_or_default()
                .contains("readable here: amount"),
            "the hint lists what an invariant may name: {error}"
        );
    }

    #[test]
    fn a_newtype_invariant_reads_the_value_it_wraps() {
        declared("name: billing.Positive\nkind: newtype\nof: Decimal\ninvariants: [value > 0]\n")
            .expect("`value` is how an invariant names what a newtype wraps");

        let errors = declared(
            "name: billing.Positive\nkind: newtype\nof: Decimal\ninvariants: [amount > 0]\n",
        )
        .expect_err("a newtype has no field `amount`");
        assert!(
            errors.contains(ValidationCode::UnobservableFact),
            "{errors}"
        );
        assert!(
            errors.to_string().contains("the representation it wraps"),
            "the hint says what a newtype's invariant may read: {errors}"
        );
    }

    #[test]
    fn a_key_the_model_does_not_know_is_refused_in_a_type_declaration() {
        for (misspelling, dropped) in [
            ("invarants: [value >= 0]", "invarants"),
            ("namin:\n  wire: money", "namin"),
        ] {
            let error = serde_yaml::from_str::<RawNamedType>(&format!(
                "name: billing.Money\nkind: newtype\nof: Decimal\n{misspelling}\n"
            ))
            .expect_err("a misspelt key used to be dropped in silence");
            assert!(
                error.to_string().contains(dropped),
                "the refusal names the key: {error}"
            );
        }

        let money =
            declared("name: billing.Money\nkind: newtype\nof: Decimal\ninvariants: [value >= 0]\n")
                .expect("the spelling the model does know");
        assert!(
            matches!(&money.body, TypeBody::Newtype { invariants, .. } if invariants.len() == 1)
        );
    }

    #[test]
    fn the_published_schema_accepts_what_the_parser_accepts() {
        // The schema is loaded by an author's editor. Refusing a declaration this repository ships
        // as valid is the one thing it must never do, and `deny_unknown_fields` on the body is what
        // makes a derived one do exactly that.
        let schema = serde_json::to_value(schemars::schema_for!(RawNamedType)).expect("serialises");
        let branches = schema["oneOf"].as_array().expect("one branch per kind");
        assert_eq!(branches.len(), 4);
        for branch in branches {
            assert_eq!(
                branch["additionalProperties"],
                serde_json::json!(false),
                "a misspelt key has to be a red squiggle: {branch}"
            );
            for key in ["name", "naming"] {
                assert!(
                    branch["properties"][key].is_object(),
                    "the flattened outer keys have to be visible where that is enforced, or the \
                     schema refuses every real declaration: {branch}"
                );
            }
        }
    }

    /// Reads a type declaration the way a document does.
    fn declaration(yaml: &str) -> Result<NamedType, ValidationErrors> {
        NamedType::try_from(serde_yaml::from_str::<RawNamedType>(yaml).expect("well formed"))
    }

    #[test]
    fn a_type_no_value_can_be_is_refused() {
        for (label, yaml) in [
            (
                "an enum",
                "name: billing.Channel\nkind: enum\nvariants: []\n",
            ),
            (
                "a union",
                "name: billing.Payee\nkind: union\ntag: kind\nvariants: {}\n",
            ),
            (
                "a struct",
                "name: billing.Money\nkind: struct\nfields: []\n",
            ),
        ] {
            let errors = declaration(yaml).expect_err(label);
            assert!(
                errors.contains(ValidationCode::EmptyDeclaration),
                "{label}: {errors}"
            );
        }
    }

    #[test]
    fn a_union_that_names_no_tag_field_is_refused() {
        // An untagged union is decodable only by guessing which branch a payload is, and the guess
        // is wrong at run time rather than at build time.
        let errors = declaration(
            "name: billing.Payee\nkind: union\ntag: \"\"\nvariants: {person: String}\n",
        )
        .expect_err("a union with no tag");
        assert!(
            errors.contains(ValidationCode::EmptyDeclaration),
            "{errors}"
        );
    }

    #[test]
    fn a_type_that_declares_something_is_accepted() {
        // The other side of the rule: the check must not refuse the shapes the example uses.
        for yaml in [
            "name: billing.Channel\nkind: enum\nvariants: [Email, Post]\n",
            "name: billing.Payee\nkind: union\ntag: kind\nvariants: {person: String}\n",
            "name: billing.Money\nkind: struct\nfields: [{name: amount, type: Decimal}]\n",
            "name: billing.Email\nkind: newtype\nof: String\n",
        ] {
            declaration(yaml).unwrap_or_else(|errors| panic!("{yaml} is valid: {errors}"));
        }
    }

    #[test]
    fn a_declaration_reaches_the_model_only_through_the_conversion() {
        // The two-stage rule, checked rather than trusted: `NamedType` has no `Deserialize`, so a
        // document cannot become one without `TryFrom` running every rule on it.
        let raw = serde_yaml::from_str::<RawNamedType>(
            "name: billing.Money\nkind: newtype\nof: Decimal\ninvariants: [amount > 0]\n",
        )
        .expect("well formed");
        let errors = NamedType::try_from(raw).expect_err("a newtype has no field `amount`");
        assert!(
            errors.contains(ValidationCode::UnobservableFact),
            "{errors}"
        );
    }

    #[test]
    fn a_field_name_must_be_spellable_as_an_identifier() {
        for spelling in ["", "not a field name!", "1st", "total-amount"] {
            let error =
                serde_yaml::from_str::<Field>(&format!("name: {spelling:?}\ntype: Decimal\n"))
                    .expect_err(spelling);
            assert!(
                error.to_string().contains("field name"),
                "{spelling:?}: {error}"
            );
        }
        serde_yaml::from_str::<Field>("name: invoice_id\ntype: Decimal\n")
            .expect("`invoice_id` is a field name");

        let schema = serde_json::to_value(schemars::schema_for!(Field)).expect("serialises");
        assert_eq!(
            schema["properties"]["name"]["pattern"],
            serde_json::json!(Field::PATTERN),
            "a schema that accepts what the parser refuses is worse than no schema"
        );
    }

    #[test]
    fn a_union_carries_its_tag() {
        let declared = NamedType {
            name: name("billing.PaymentMethod"),
            body: TypeBody::Union {
                tag: "method".to_owned(),
                variants: [
                    ("card".to_owned(), TypeRef::Named(name("billing.Card"))),
                    (
                        "transfer".to_owned(),
                        TypeRef::Named(name("billing.Transfer")),
                    ),
                ]
                .into(),
            },
            naming: Naming::default(),
        };
        let TypeBody::Union { tag, variants } = &declared.body else {
            panic!("expected a union");
        };
        assert_eq!(tag, "method");
        assert_eq!(variants.len(), 2);
        assert_eq!(declared.dependencies().len(), 2);
    }
}
