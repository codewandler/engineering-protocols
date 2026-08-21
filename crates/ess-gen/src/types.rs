//! The model's type system projected into JSON Schema, once.
//!
//! Three projections publish a schema for the same construct. [`schema`](crate::schema) writes one
//! self-contained JSON Schema document per message; [`openapi`](crate::openapi) embeds schemas under
//! `components.schemas`; [`asyncapi`](crate::asyncapi) does the same for event payloads. Each of them
//! carried its own copy of the mapping, and two of the copies disagreed — so this repository
//! published two contradictory answers to "what is a valid `billing.invoice.InvoiceCreated`". A
//! consumer reading the JSON Schema tree and a consumer reading the `AsyncAPI` document were told
//! different things about the same bytes, and nothing in the build noticed.
//!
//! This module is the single answer, and all three projections read it: `openapi` and `asyncapi`
//! build their `components.schemas` entries from these functions and retarget the pointer, rather
//! than carrying a copy. Everything below therefore describes what every projection emits, not what
//! one of them does.
//!
//! `tests/agreement.rs` is what keeps that true. It compares the fragment every projection publishes
//! for each named type and each message payload of the billing example — 11 constructs over 17
//! projection pairs — and requires them to be identical, classifying any difference as an assertion
//! (what a document *accepts*) or an annotation (a fact the model states about the construct). Both
//! classes fail, because an annotation that differs gives a code generator two answers to the same
//! question, and an unclassified keyword fails too. The one exemption is `$ref` spelling, which is
//! normalised: a pointer names where the definition sits in the document doing the pointing.
//!
//! # The mapping
//!
//! | model | schema | what a consumer that ignores `format` and `x-ess-*` still sees |
//! |---|---|---|
//! | `String` | `{"type": "string"}` | the same |
//! | `Boolean` | `{"type": "boolean"}` | the same |
//! | `Integer` | `{"type": "integer"}` | the same; no width, because the model states none |
//! | `Decimal` | `{"type": "string", "format": "decimal", "pattern": …}` | an exact decimal string, checked by the pattern |
//! | `Timestamp` | `{"type": "string", "format": "date-time"}` | any string |
//! | `Duration` | `{"type": "string", "format": "duration"}` | any string |
//! | `Uuid` | `{"type": "string", "format": "uuid", "pattern": …}` | the canonical hyphenated form, checked by the pattern |
//! | `Bytes` | `{"type": "string", "contentEncoding": "base64", "pattern": …}` | base64, checked by the pattern |
//! | `Named` | a `$ref` to a definition of its own | a named type, not its representation |
//! | `Optional<T>` in a field | `T`, and the field is left out of `required` | absent is valid, `null` is not |
//! | `Optional<T>` elsewhere | `{"anyOf": [T, {"type": "null"}]}` | `null` is valid, because a list element cannot be absent |
//! | `List<T>` | `{"type": "array", "items": T}` | the same |
//! | `Map<K, V>` | `{"type": "object", "propertyNames": K-as-text, "additionalProperties": V}` | the same |
//! | `Newtype` | a definition of its own, `$ref`d, `x-ess-kind: newtype` | the representation's assertions |
//! | `Struct` | `{"type": "object", "properties": …, "required": …, "additionalProperties": false}` | the same |
//! | `Enum` | `{"type": "string", "enum": […]}` | the same |
//! | `Union` | `{"oneOf": [{…"kind": {"const": "person"}, "value": …}]}` | the tag, so the branch is decidable |
//!
//! Where a pattern is emitted it is because the grammar is small enough to be certainly right. There
//! is deliberately none for `Timestamp` or `Duration`: a hand-written RFC 3339 or ISO 8601 regex that
//! is subtly too strict rejects data the specification permits, which is the exact failure
//! `aep-schema/tests/published.rs` exists to prevent, and a validator that ignores `format` is a
//! milder problem than a schema that lies.
//!
//! # The four disagreements, and the reading each was settled on
//!
//! | construct | it published | it now publishes | what that costs |
//! |---|---|---|---|
//! | `Optional` outside a field | `x-ess-optional: true` and no `null` branch (`asyncapi`) | `anyOf [T, null]` | the projection names `null` as the wire spelling of an absent list element, which the model does not state |
//! | tagged union | `anyOf` over the payloads plus `x-ess-union-tag` (`asyncapi`) | `oneOf` of adjacently tagged branches, plus `x-ess-union-tag` | the projection fixes a layout — `{"kind": "person", "value": …}` — that the model does not state |
//! | `Map<Integer, _>` key | `x-ess-map-key: Integer` only (`asyncapi`) | `propertyNames` constrained to integer text, **and** `x-ess-map-key` | a key spelt `007` is refused, and the model never said it was illegal |
//! | `Duration` | `x-ess-type: Duration`, no `format` (`asyncapi`) | `format: duration` | a validator that asserts formats applies RFC 3339 appendix A, a grammar the model never named |
//!
//! Each of those four went the same way, for one reason: **an extension is a note, and a keyword is
//! an assertion.** A consumer that ignores `x-ess-*` — which every conforming validator does, by
//! default and by specification — sees a *more permissive* schema than the model. That is not a
//! neutral trade. It means the artifact this repository publishes as a contract does not refuse the
//! thing the model refuses, and the one job of a published contract is refusing.
//!
//! The two rows that deserve their reasoning spelt out are the union and the map key, because both
//! are cases where the honest-looking option is the worse one.
//!
//! ## A union: claiming a layout beats refusing to
//!
//! The model permits only *tagged* unions, and says so in
//! [`types`](ess_domain::types)' own module documentation: an untagged union cannot be decoded
//! without guessing. An `anyOf` over the bare variant payloads publishes a schema in which **the tag
//! does not appear at all** — so `"someone@example.com"` validates as a `billing.invoice.Payee`, when
//! a bare string is not a `Payee` in this model at all. That projection has not declined to state a
//! layout; it has stated a *different* one, in which there is no tag, and handed the decoding guess
//! back to the consumer. It is also self-defeating: the objection to `oneOf` — that two variants
//! which are both a `String` underneath would match twice — is only true once the tag has been
//! dropped. With the tag pinned by a `const`, exactly one branch matches, which is what the model
//! bought by making the tag mandatory.
//!
//! So: adjacent tagging. The payload sits under `value`, or under [`CONTENT_WHEN_TAKEN`] when the tag
//! is itself called `value`. Adjacent rather than internal because a variant may be a `String` or a
//! `List` and neither can be merged into the tagged object — an internally tagged rendering would
//! work for some unions and silently fail for others. It is also the one encoding Serde can express
//! (`#[serde(tag = "…", content = "…")]`), so generated Rust and this schema agree by construction
//! rather than by review. The cost is real and is stated in the table: this file, not the
//! specification, is where the layout is decided, and a reviewer who wants a different one changes it
//! here and gets all three projections at once.
//!
//! ## A map key: the pattern is what makes the map a map
//!
//! `propertyNames` on an integer-keyed map is the one place in this mapping where a *stricter* schema
//! is the safer one, and `aep-schema/tests/published.rs` — whose whole subject is a schema that
//! refused its own normative example — is the reason it needs an argument rather than an assertion.
//!
//! The difference from `Timestamp` is not the strictness, it is the size of the grammar. RFC 3339 is
//! large: offsets, fractional seconds, case, `T` versus a space. A regex for it is *probably* wrong,
//! and there is no way to be sure by reading. A decimal integer has one canonical spelling per value,
//! and the whole grammar fits on this line. And the map needs it: a JSON object's keys are unique as
//! *text*, so if `7` and `007` are both legal spellings of one key, a single map can carry two entries
//! for one key — which destroys the property that made this projection choose an object over an array
//! of pairs. The pattern is not a guess about an external grammar; it is what makes the wire form
//! well defined. `x-ess-map-key` is kept beside it, because the *model's* key type is not recoverable
//! from `propertyNames` and a code generator wants it.
//!
//! # What every projection keeps
//!
//! * `additionalProperties: false` on every object, in both directions. A field the specification does
//!   not declare is one the receiver has no meaning for, and accepting it silently is how two systems
//!   drift into disagreeing about what they exchanged. The cost is that a publisher cannot add a field
//!   without a specification change — which is the intended cost, and it is why the `AsyncAPI`
//!   projection's event payloads are now closed too: the same event cannot be closed in one published
//!   file and open in another.
//! * A newtype never collapses into its representation. `billing.invoice.Email` and
//!   `billing.email.EmailAddress` are both a `String` underneath and get a definition each, carrying
//!   `title`, `x-ess-name` and `x-ess-kind: newtype`, referenced and never inlined. What that cannot
//!   do is make two *values* distinguishable: on the wire an `Email` is a bare JSON string, and a
//!   payload with the two fields' values swapped validates clean. JSON Schema constrains structure,
//!   and nominal identity is not structure.
//! * No invented `format` or `pattern` where the model states no grammar. `Email` wraps a `String`
//!   and says nothing about the shape of the string, so `"format": "email"` would publish a
//!   constraint the specification never stated and refuse addresses it permits.
//! * `Decimal` as an exact string. Money does not round the way a float does and a JSON number is
//!   read as a float by most of the world; the lossy rendering would have validated `0.1` as an exact
//!   decimal.
//! * `description` is the author's words and nothing else. An invariant is published verbatim under
//!   `x-ess-invariants` and is visibly an annotation rather than an assertion — `amount >= 0` is a
//!   predicate over a `Decimal`, which this mapping renders as a string, so `minimum` cannot express
//!   it. The `OpenAPI` projection used to fold both the invariants and a generated sentence about
//!   newtype distinctness into `description`; that moved into keywords, so that a reader can tell what
//!   the author said from what the generator said, and a drift check does not have to parse prose.
//!
//! # Determinism
//!
//! Every collection here is a [`BTreeMap`], a [`BTreeSet`] or a [`Vec`] built from the IR's own
//! declaration order. [`Node`] is a struct rather than a [`serde_json::Value`] for the same reason:
//! key order inside a `Value` object is whatever `serde_json`'s map type does, which is one Cargo
//! feature away from changing under this crate, and a fixed set of fields is a fixed set of keywords —
//! what a consumer has to implement in order to read this output is bounded by the declaration below,
//! and a keyword nobody decided to emit cannot appear by accident.

use std::collections::{BTreeMap, BTreeSet};

use ess_compiler::ir::{
    ResolvedBody, ResolvedCommand, ResolvedError, ResolvedEvent, ResolvedField, ResolvedType,
    ResolvedTypeRef, ResolvedView, TypeHandle,
};
use ess_compiler::EssIr;
use ess_domain::entity::Invariant;
use ess_domain::name::QualifiedName;
use ess_domain::types::Primitive;

use crate::provenance::Provenance;

/// An exact decimal, as text.
///
/// No exponent: `1e3` is a float's spelling of a number, and the point of writing money as a string
/// is that the digits are the value. A leading zero is refused for the same reason a trailing `.` is
/// — one value must have one spelling, or two systems agree on the schema and disagree on equality.
pub(crate) const DECIMAL_PATTERN: &str = r"^-?(0|[1-9][0-9]*)(\.[0-9]+)?$";

/// An integer, as the text a JSON object key is spelt with.
pub(crate) const INTEGER_TEXT_PATTERN: &str = r"^-?(0|[1-9][0-9]*)$";

/// A UUID in the canonical hyphenated form.
///
/// The `urn:uuid:` and brace-wrapped forms are refused. Nothing in this repository parses these
/// values, so this projection is where the wire form is decided, and one form is the decision.
pub(crate) const UUID_PATTERN: &str =
    "^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$";

/// Base64 with padding, as `contentEncoding` describes but does not enforce.
pub(crate) const BASE64_PATTERN: &str =
    "^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$";

/// Where a union variant's value sits beside its tag.
pub(crate) const CONTENT: &str = "value";

/// Where a variant's value sits when the tag is already called `value`.
///
/// A union tagged `value` would otherwise need one property to be both the tag and the payload, and
/// the document would be nonsense rather than merely awkward.
pub(crate) const CONTENT_WHEN_TAKEN: &str = "content";

/// The pointer that reaches a named type's definition, inside the document making the reference.
///
/// The `$defs` spelling. [`schema`](crate::schema) writes self-contained documents that keep every
/// definition they reach under `$defs`, filed under the type's qualified name unmodified — so a
/// definition's key is `name.to_string()` and the pointer to it is that key with `#/$defs/` in front.
/// The `OpenAPI` and `AsyncAPI` projections publish the same fragments under `components.schemas`, so
/// they take what this module produces and retarget the pointer, in one shared helper
/// (`openapi::under_components`) rather than one copy each. That helper belongs beside this function;
/// it lives there because the retarget needs the document's own prefix, which is a fact about the
/// document and not about the type.
///
/// A qualified name's segments are `[A-Za-z][A-Za-z0-9_-]*` joined by dots, so no name can contain
/// the `/` or `~` a JSON Pointer would need escaped. The pointer is the name, unmodified.
pub(crate) fn pointer(name: &QualifiedName) -> String {
    format!("#/$defs/{name}")
}

/// One JSON Schema node: every keyword this repository has decided to publish, and no other.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub(crate) struct Node {
    /// The dialect, at a document root only.
    #[serde(rename = "$schema", skip_serializing_if = "Option::is_none")]
    pub(crate) dialect: Option<&'static str>,
    /// A pointer to a definition in this same document.
    #[serde(rename = "$ref", skip_serializing_if = "Option::is_none")]
    pub(crate) reference: Option<String>,
    /// What a person is shown.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) title: Option<String>,
    /// The author's own words, and never the generator's.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) description: Option<String>,
    /// The qualified name of the type or message this describes.
    #[serde(rename = "x-ess-name", skip_serializing_if = "Option::is_none")]
    pub(crate) ess_name: Option<String>,
    /// Which construct it is: `newtype`, `struct`, `enum`, `union`, or a message kind.
    #[serde(rename = "x-ess-kind", skip_serializing_if = "Option::is_none")]
    pub(crate) ess_kind: Option<&'static str>,
    /// The field's declared name, when the wire name differs from it.
    #[serde(rename = "x-ess-field", skip_serializing_if = "Option::is_none")]
    pub(crate) ess_field: Option<String>,
    /// The JSON type.
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub(crate) kind: Option<&'static str>,
    /// A registered format, where the model's primitive has one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) format: Option<&'static str>,
    /// The grammar, where it is small enough to be certainly right.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) pattern: Option<&'static str>,
    /// How bytes are carried.
    #[serde(rename = "contentEncoding", skip_serializing_if = "Option::is_none")]
    pub(crate) content_encoding: Option<&'static str>,
    /// The one value this node accepts — a union's tag, an outcome's name.
    #[serde(rename = "const", skip_serializing_if = "Option::is_none")]
    pub(crate) constant: Option<String>,
    /// An enum's variants.
    #[serde(rename = "enum", skip_serializing_if = "Vec::is_empty")]
    pub(crate) choices: Vec<String>,
    /// An object's declared properties, in the order their author wrote them.
    #[serde(skip_serializing_if = "Properties::is_empty")]
    pub(crate) properties: Properties,
    /// Which of them a value has to carry.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) required: Vec<String>,
    /// What is permitted beyond the declared properties.
    #[serde(
        rename = "additionalProperties",
        skip_serializing_if = "Option::is_none"
    )]
    pub(crate) additional: Option<Additional>,
    /// How a map's keys are spelt.
    #[serde(rename = "propertyNames", skip_serializing_if = "Option::is_none")]
    pub(crate) property_names: Option<Box<Node>>,
    /// The model's key type, which a JSON object cannot express.
    #[serde(rename = "x-ess-map-key", skip_serializing_if = "Option::is_none")]
    pub(crate) ess_map_key: Option<&'static str>,
    /// What an array holds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) items: Option<Box<Node>>,
    /// A choice where more than one branch may match: the nullable idiom.
    #[serde(rename = "anyOf", skip_serializing_if = "Vec::is_empty")]
    pub(crate) any_of: Vec<Node>,
    /// A choice where exactly one branch matches: a tagged union.
    #[serde(rename = "oneOf", skip_serializing_if = "Vec::is_empty")]
    pub(crate) one_of: Vec<Node>,
    /// The field a union's variant is named in.
    #[serde(rename = "x-ess-union-tag", skip_serializing_if = "Option::is_none")]
    pub(crate) ess_union_tag: Option<String>,
    /// Conditions every value satisfies, as the author wrote them. An annotation, not an assertion.
    #[serde(rename = "x-ess-invariants", skip_serializing_if = "Vec::is_empty")]
    pub(crate) invariants: Vec<String>,
    /// Where this artifact came from, at a document root only.
    #[serde(rename = "x-ess-provenance", skip_serializing_if = "Option::is_none")]
    pub(crate) provenance: Option<Attribution>,
    /// The named types this document reaches.
    #[serde(rename = "$defs", skip_serializing_if = "BTreeMap::is_empty")]
    pub(crate) defs: BTreeMap<String, Node>,
}

impl Node {
    /// The node as canonical JSON, with a trailing newline.
    ///
    /// Canonical means: key order comes from the declaration above and from [`BTreeMap`], so it is
    /// the same on every machine and in every run; the indentation is `serde_json`'s two spaces; and
    /// the last byte is a newline, because a file without one shows up as modified in the next diff.
    ///
    /// Serialisation cannot fail: every map key is a string and no float is involved.
    pub(crate) fn to_canonical_json(&self) -> String {
        let mut json = serde_json::to_string_pretty(self)
            .unwrap_or_else(|error| panic!("a schema node serialises: {error}"));
        json.push('\n');
        json
    }

    /// A reference to a named type's definition, elsewhere in this same document.
    pub(crate) fn referring_to(name: &QualifiedName) -> Self {
        Self {
            reference: Some(pointer(name)),
            ..Self::default()
        }
    }

    /// The absence of a value, as JSON spells it.
    fn null() -> Self {
        Self {
            kind: Some("null"),
            ..Self::default()
        }
    }

    /// The same node carrying the words a reader needs.
    ///
    /// Both are legal beside a `$ref` in 2020-12 — and in `OpenAPI` 3.1, whose dialect *is* 2020-12 —
    /// where `$ref` is a keyword rather than a whole-object replacement. In draft-07 they would have
    /// been silently discarded, which is one reason nothing here emits draft-07.
    pub(crate) fn annotated(mut self, title: Option<&str>, description: Option<String>) -> Self {
        if let Some(text) = title {
            self.title = Some(text.to_owned());
        }
        self.description = description;
        self
    }
}

/// An object's properties, in the order the specification declares them.
///
/// Declaration order rather than a [`BTreeMap`]'s alphabetical order: a reader comparing this
/// document with the specification reads the fields in the order they were written, and the property
/// order carries no meaning to a validator either way. A [`Vec`] of pairs also makes the ordering a
/// property of this type rather than of a Cargo feature.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Properties(Vec<(String, Node)>);

impl Properties {
    /// Records one property.
    pub(crate) fn insert(&mut self, name: impl Into<String>, schema: Node) {
        self.0.push((name.into(), schema));
    }

    /// `true` when the object declares no property, so the keyword is left out entirely.
    pub(crate) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl serde::Serialize for Properties {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap as _;

        let mut map = serializer.serialize_map(Some(self.0.len()))?;
        for (name, schema) in &self.0 {
            map.serialize_entry(name, schema)?;
        }
        map.end()
    }
}

/// What an object permits beyond the properties it declares.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Additional {
    /// Nothing. A property the schema does not declare is a rejection, not a key silently ignored —
    /// which is the whole reason to publish a wire contract rather than describe one.
    Refused,
    /// Every undeclared property matches this. How a `Map` says what its values are.
    Matching(Box<Node>),
}

impl serde::Serialize for Additional {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Refused => serializer.serialize_bool(false),
            Self::Matching(schema) => schema.serialize(serializer),
        }
    }
}

/// Where an artifact came from, as a keyword rather than as prose.
///
/// JSON has no comments; `$comment` is the nearest thing and it is the wrong thing twice over —
/// implementations are explicitly permitted to strip it, and a drift check comparing digests would
/// have to re-parse prose out of it. An unknown keyword is preserved by every tool that round-trips a
/// schema, and 2020-12 requires validators to ignore it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub(crate) struct Attribution {
    /// Design §10's four facts.
    #[serde(flatten)]
    pub(crate) provenance: Provenance,
    /// How to reproduce the artifact carrying it.
    pub(crate) regenerate: &'static str,
}

impl Attribution {
    /// Records provenance, and how to reproduce the artifact carrying it.
    pub(crate) fn new(provenance: &Provenance) -> Self {
        Self {
            provenance: provenance.clone(),
            regenerate: "protocol ess generate",
        }
    }
}

/// The schema for a primitive value.
pub(crate) fn primitive(primitive: Primitive) -> Node {
    let string = |format: Option<&'static str>, pattern: Option<&'static str>| Node {
        kind: Some("string"),
        format,
        pattern,
        ..Node::default()
    };

    match primitive {
        Primitive::String => string(None, None),
        Primitive::Boolean => Node {
            kind: Some("boolean"),
            ..Node::default()
        },
        Primitive::Integer => Node {
            kind: Some("integer"),
            ..Node::default()
        },
        // A string, not a number. Money does not round the way a float does, and a JSON number is
        // read as a float by most of the world — `JSON.parse` has no other option. The lossy
        // rendering would have validated `0.1` as an exact decimal, which is the failure the
        // model's own comment on `Primitive::Decimal` names.
        Primitive::Decimal => string(Some("decimal"), Some(DECIMAL_PATTERN)),
        // Two registered 2020-12 formats, and no pattern either could be wrong about. The model says
        // only "an instant" and "a length of time", so `format` is where the wire grammar is named:
        // it is a standard keyword whose meaning the dialect defines rather than this file, and it is
        // an annotation by default, so a validator that does not assert formats sees any string —
        // which is a milder failure than an extension no validator reads at all.
        Primitive::Timestamp => string(Some("date-time"), None),
        Primitive::Duration => string(Some("duration"), None),
        Primitive::Uuid => string(Some("uuid"), Some(UUID_PATTERN)),
        // `contentEncoding` is an annotation in 2020-12, so it describes the encoding without
        // checking it. The pattern is what makes a non-base64 string a rejection.
        Primitive::Bytes => Node {
            kind: Some("string"),
            pattern: Some(BASE64_PATTERN),
            content_encoding: Some("base64"),
            ..Node::default()
        },
    }
}

/// The schema for a primitive used as an object key.
///
/// A JSON object's keys are always strings, so this is the *spelling* of a key rather than the key:
/// `Map<Integer, _>` becomes an object whose property names are the decimal text of an integer.
/// Returns `None` for `String`, where a `propertyNames` rule would check nothing and invite a reader
/// to believe something was checked.
pub(crate) fn key(primitive_key: Primitive) -> Option<Node> {
    match primitive_key {
        Primitive::String => None,
        Primitive::Boolean => Some(Node {
            kind: Some("string"),
            choices: vec!["false".to_owned(), "true".to_owned()],
            ..Node::default()
        }),
        Primitive::Integer => Some(Node {
            kind: Some("string"),
            pattern: Some(INTEGER_TEXT_PATTERN),
            ..Node::default()
        }),
        other => Some(primitive(other)),
    }
}

/// The schema for a type reference, in a position where absence cannot be expressed.
///
/// "Cannot be expressed" is the whole subtlety of `Optional`. A missing object property is a thing
/// JSON can say; a missing array element is not. So an `Optional` inside a `List` or a `Map` value
/// becomes a `null` branch, and an `Optional` in a field position is handled by [`object`], which
/// leaves the field out of `required` instead. Sending both would give one fact two spellings, which
/// is the ambiguity the model refuses everywhere else.
///
/// Recurses as deep as the reference nests and does not count, because it cannot meet a deep one: a
/// [`ResolvedTypeRef`] is at most [`MAX_TYPE_DEPTH`](ess_domain::types::MAX_TYPE_DEPTH) deep, and a
/// named leaf becomes a `$ref` rather than being inlined, so a type graph that refers to itself
/// produces a wide definitions table and not a deep node. [`reachable`] is what keeps that table
/// finite, with a worklist and a visited set.
pub(crate) fn type_ref(reference: &ResolvedTypeRef) -> Node {
    match reference {
        ResolvedTypeRef::Primitive { name } => primitive(*name),
        ResolvedTypeRef::Declared { name } => Node::referring_to(name.name()),
        ResolvedTypeRef::Optional { of } => Node {
            any_of: vec![type_ref(of), Node::null()],
            ..Node::default()
        },
        ResolvedTypeRef::List { of } => Node {
            kind: Some("array"),
            items: Some(Box::new(type_ref(of))),
            ..Node::default()
        },
        // An object, not an array of pairs. The model restricts a key to a primitive precisely so
        // the map has a stable wire form, and a JSON object is that form: an array of pairs would
        // carry a non-string key faithfully but throw away key uniqueness, since nothing stops the
        // same key appearing twice in an array, and it would make every consumer walk pairs to read
        // what JSON already has a shape for.
        ResolvedTypeRef::Map { key: by, value } => Node {
            kind: Some("object"),
            additional: Some(Additional::Matching(Box::new(type_ref(value)))),
            property_names: key(*by).map(Box::new),
            ess_map_key: Some(by.as_str()),
            ..Node::default()
        },
    }
}

/// The schema for one field's value, in a position where absence *is* expressible.
///
/// The `Optional` wrapper is stripped, because at a field position absence is spelt by leaving the
/// name out of `required` — so this describes the value that is there when it is there. Takes the
/// whole [`ResolvedField`] rather than only its type, which is a deliberate change from the requested
/// signature: a field's `title`, its `description` and its declared name are exactly what two of the
/// three projections disagreed about, and a function handed only a [`ResolvedTypeRef`] cannot carry
/// them.
pub(crate) fn field(declared: &ResolvedField) -> Node {
    let mut node = type_ref(declared.type_ref.required()).annotated(
        declared.naming.display.as_deref(),
        declared.naming.summary.clone(),
    );
    if wire_name(declared) != declared.name {
        node.ess_field = Some(declared.name.clone());
    }
    node
}

/// What a field is called on the wire.
///
/// From the IR, so that no projection re-reads the source for it: a schema keyed on the model's field
/// name where the specification renames it would refuse every message a producer actually sends.
pub(crate) fn wire_name(declared: &ResolvedField) -> &str {
    declared.naming.wire.as_deref().unwrap_or(&declared.name)
}

/// A closed object over these fields, with the non-`Optional` ones required.
///
/// A field's property key is its wire name. Two fields with the same wire name would collapse into
/// one property; the model validates field *names* for duplication and does not constrain wire names,
/// so that is a gap in the model rather than something to paper over here.
pub(crate) fn object(fields: &[ResolvedField]) -> Node {
    let mut properties = Properties::default();
    let mut required = Vec::new();

    for declared in fields {
        let wire = wire_name(declared);
        if !declared.type_ref.is_optional() {
            required.push(wire.to_owned());
        }
        properties.insert(wire, field(declared));
    }

    Node {
        kind: Some("object"),
        properties,
        required,
        additional: Some(Additional::Refused),
        ..Node::default()
    }
}

/// The schema for one named type, as it appears in a document's definitions.
pub(crate) fn body(declared: &ResolvedType) -> Node {
    let mut node = match &declared.body {
        // Referenced, never inlined, even though a newtype over `String` has the same assertions as
        // a `String`. The reference is what keeps `Email` and `EmailAddress` two types in the
        // document and in anything generated from it.
        ResolvedBody::Newtype { of, invariants } => Node {
            invariants: statements(invariants),
            ..type_ref(of)
        },
        ResolvedBody::Struct { fields, invariants } => Node {
            invariants: statements(invariants),
            ..object(fields)
        },
        ResolvedBody::Enum { variants } => Node {
            kind: Some("string"),
            choices: variants.clone(),
            ..Node::default()
        },
        ResolvedBody::Union { tag, variants } => Node {
            one_of: variants
                .iter()
                .map(|(label, payload)| variant(tag, label, payload))
                .collect(),
            ess_union_tag: Some(tag.clone()),
            ..Node::default()
        },
    };

    node.title = Some(declared.naming.display_or(&declared.name).to_owned());
    node.description.clone_from(&declared.naming.summary);
    node.ess_name = Some(declared.name.to_string());
    node.ess_kind = Some(body_kind(&declared.body));
    node
}

/// One branch of a tagged union.
///
/// The tag is a `const`, so exactly one branch can match and a decoder never has to guess which
/// shape it is looking at. That is the property the model exists to guarantee — it offers no untagged
/// form at all — and a choice without the `const` would have thrown it away here.
fn variant(tag: &str, label: &str, payload: &ResolvedTypeRef) -> Node {
    let content = content_key(tag);
    let mut properties = Properties::default();
    properties.insert(
        tag,
        Node {
            kind: Some("string"),
            constant: Some(label.to_owned()),
            ..Node::default()
        },
    );
    properties.insert(content, type_ref(payload.required()));

    // The content key is a property of an object, so absence is spelt the way it is spelt at every
    // other field position: by leaving the name out of `required`, not by a `null` branch.
    let mut required = vec![tag.to_owned()];
    if !payload.is_optional() {
        required.push(content.to_owned());
    }

    Node {
        title: Some(label.to_owned()),
        kind: Some("object"),
        properties,
        required,
        additional: Some(Additional::Refused),
        ..Node::default()
    }
}

/// Where a variant's value sits beside its tag.
pub(crate) fn content_key(tag: &str) -> &'static str {
    if tag == CONTENT {
        CONTENT_WHEN_TAKEN
    } else {
        CONTENT
    }
}

/// What an author wrote, for each invariant.
fn statements(invariants: &[Invariant]) -> Vec<String> {
    invariants
        .iter()
        .map(|invariant| invariant.statement.clone())
        .collect()
}

/// Which of the four bodies a named type has, for `x-ess-kind`.
fn body_kind(declared: &ResolvedBody) -> &'static str {
    match declared {
        ResolvedBody::Newtype { .. } => "newtype",
        ResolvedBody::Struct { .. } => "struct",
        ResolvedBody::Enum { .. } => "enum",
        ResolvedBody::Union { .. } => "union",
    }
}

/// A command's input, as a message.
pub(crate) const COMMAND_INPUT: &str = "command-input";

/// An event's payload, as a message.
pub(crate) const EVENT_PAYLOAD: &str = "event-payload";

/// An error's payload, as a message.
pub(crate) const ERROR_PAYLOAD: &str = "error-payload";

/// One row of a view, as a message.
pub(crate) const VIEW_ROW: &str = "view-row";

/// One message that crosses this system's boundary.
///
/// Shared rather than per projection because the *payload* is the thing the three projections
/// disagreed about: `billing.invoice.InvoiceCreated` is published by two of them, and
/// `billing.invoice.CreateInvoice`'s input by two others, so the annotations around the object have to
/// come from one place too or the fragments differ by a title.
pub(crate) struct Message<'a> {
    /// What kind of message it is, for `x-ess-kind`.
    pub(crate) kind: &'static str,
    /// Its identity.
    pub(crate) name: &'a QualifiedName,
    /// What a person is shown.
    pub(crate) title: String,
    /// One line, when the specification has one.
    pub(crate) description: Option<String>,
    /// What it carries.
    pub(crate) fields: &'a [ResolvedField],
}

impl<'a> Message<'a> {
    /// A command's input.
    pub(crate) fn of_command(command: &'a ResolvedCommand) -> Self {
        Self {
            kind: COMMAND_INPUT,
            name: &command.name,
            title: format!("{} input", command.naming.display_or(&command.name)),
            description: command.naming.summary.clone(),
            fields: &command.input,
        }
    }

    /// An event's payload.
    pub(crate) fn of_event(event: &'a ResolvedEvent) -> Self {
        Self {
            kind: EVENT_PAYLOAD,
            name: &event.name,
            title: format!("{} payload", event.naming.display_or(&event.name)),
            description: event.naming.summary.clone(),
            fields: &event.fields,
        }
    }

    /// An error's payload.
    ///
    /// An error carries no [`Naming`](ess_domain::name::Naming) in the IR, so its title is its own
    /// name and its description is the summary the author wrote for whoever receives it.
    pub(crate) fn of_error(error: &'a ResolvedError) -> Self {
        Self {
            kind: ERROR_PAYLOAD,
            name: &error.name,
            title: format!("{} payload", error.name.local()),
            description: error.summary.clone(),
            fields: &error.fields,
        }
    }

    /// One row of a view.
    ///
    /// A message like the other three, and for the same reason: a row crosses the system's boundary
    /// the moment a component declares it is reached from outside, and a projection that described
    /// it its own way would be the fourth copy of the type mapping this crate exists to prevent.
    pub(crate) fn of_view(view: &'a ResolvedView) -> Self {
        Self {
            kind: VIEW_ROW,
            name: &view.name,
            title: format!("{} row", view.naming.display_or(&view.name)),
            description: view.naming.summary.clone(),
            fields: &view.fields,
        }
    }

    /// Which subdirectory a document describing this message belongs in.
    ///
    /// Derived from the kind rather than stored beside it, so the two cannot disagree.
    pub(crate) fn directory(&self) -> &'static str {
        match self.kind {
            COMMAND_INPUT => "commands",
            EVENT_PAYLOAD => "events",
            VIEW_ROW => "views",
            _ => "errors",
        }
    }
}

/// The schema for one message's payload.
pub(crate) fn message(carried: &Message<'_>) -> Node {
    Node {
        title: Some(carried.title.clone()),
        description: carried.description.clone(),
        ess_name: Some(carried.name.to_string()),
        ess_kind: Some(carried.kind),
        ..object(carried.fields)
    }
}

/// Every named type reachable from `roots`, including the roots.
///
/// Transitive, because a document whose definitions held only what a message mentions directly would
/// contain a `$ref` pointing at nothing — a document that parses as JSON, passes a "has the required
/// keywords" check, and fails the moment anyone validates against it. The visited set is what
/// terminates it: a type graph that refers to itself is walked once.
pub(crate) fn reachable<'a>(
    ir: &'a EssIr,
    roots: impl IntoIterator<Item = &'a TypeHandle>,
) -> BTreeSet<&'a TypeHandle> {
    let mut found: BTreeSet<&'a TypeHandle> = BTreeSet::new();
    let mut pending: Vec<&'a TypeHandle> = roots.into_iter().collect();

    while let Some(handle) = pending.pop() {
        if !found.insert(handle) {
            continue;
        }
        pending.extend(body_leaves(&ir.named_type(handle).body));
    }

    found
}

/// The definitions table for everything `roots` reaches, keyed by qualified name.
///
/// The key is what [`pointer`] points at, so the two are written to agree: a table keyed any other
/// way would publish a document whose own `$ref`s resolve to nothing.
pub(crate) fn definitions<'a>(
    ir: &'a EssIr,
    roots: impl IntoIterator<Item = &'a TypeHandle>,
) -> BTreeMap<String, Node> {
    reachable(ir, roots)
        .into_iter()
        .map(|handle| {
            let declared = ir.named_type(handle);
            (declared.name.to_string(), body(declared))
        })
        .collect()
}

/// Every named type a body reaches directly.
pub(crate) fn body_leaves(declared: &ResolvedBody) -> Vec<&TypeHandle> {
    match declared {
        ResolvedBody::Newtype { of, .. } => of.named_leaves(),
        ResolvedBody::Struct { fields, .. } => field_leaves(fields),
        ResolvedBody::Enum { .. } => Vec::new(),
        ResolvedBody::Union { variants, .. } => variants
            .values()
            .flat_map(ResolvedTypeRef::named_leaves)
            .collect(),
    }
}

/// Every named type these fields reach.
pub(crate) fn field_leaves(fields: &[ResolvedField]) -> Vec<&TypeHandle> {
    fields
        .iter()
        .flat_map(|declared| declared.type_ref.named_leaves())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_decimal_is_written_as_an_exact_string_because_a_json_number_is_read_as_a_float() {
        let node = primitive(Primitive::Decimal);
        assert_eq!(node.kind, Some("string"));
        assert_eq!(node.pattern, Some(DECIMAL_PATTERN));
    }

    #[test]
    fn a_timestamp_and_a_duration_publish_a_format_and_no_pattern_they_could_be_wrong_about() {
        for (which, format) in [
            (Primitive::Timestamp, "date-time"),
            (Primitive::Duration, "duration"),
        ] {
            let node = primitive(which);
            assert_eq!(node.format, Some(format));
            assert_eq!(
                node.pattern, None,
                "a hand-written RFC 3339 or ISO 8601 regex refuses values the model permits"
            );
        }
    }

    #[test]
    fn an_integer_key_is_constrained_to_the_text_an_integer_is_spelt_with() {
        let node = key(Primitive::Integer).expect("an integer key is constrained");
        assert_eq!(node.kind, Some("string"));
        assert_eq!(node.pattern, Some(INTEGER_TEXT_PATTERN));
    }

    #[test]
    fn a_string_keyed_map_publishes_no_property_name_rule_that_checks_nothing() {
        let map = ResolvedTypeRef::Map {
            key: Primitive::String,
            value: Box::new(ResolvedTypeRef::Primitive {
                name: Primitive::String,
            }),
        };
        let node = type_ref(&map);
        assert_eq!(node.kind, Some("object"));
        assert!(node.property_names.is_none());
        assert_eq!(
            node.ess_map_key,
            Some("String"),
            "the model's key type is not recoverable from `propertyNames`, so it is recorded"
        );
    }

    #[test]
    fn an_optional_outside_a_field_gains_a_null_branch_because_a_list_element_cannot_be_absent() {
        let list = ResolvedTypeRef::List {
            of: Box::new(ResolvedTypeRef::Optional {
                of: Box::new(ResolvedTypeRef::Primitive {
                    name: Primitive::String,
                }),
            }),
        };
        let node = type_ref(&list);
        let items = node.items.expect("a list says what it holds");
        assert_eq!(items.any_of.len(), 2);
        assert_eq!(items.any_of[1].kind, Some("null"));
    }

    #[test]
    fn a_union_tagged_value_moves_its_payload_aside_rather_than_colliding_with_the_tag() {
        assert_eq!(content_key("kind"), "value");
        assert_eq!(content_key("value"), "content");
    }

    #[test]
    fn a_union_branch_pins_its_tag_so_exactly_one_branch_can_match() {
        // The reading this module settled the `anyOf`-versus-`oneOf` disagreement on: without the
        // `const`, two variants that are both a `String` underneath are indistinguishable, which is
        // the guess the model made the tag mandatory to remove.
        let node = variant(
            "kind",
            "person",
            &ResolvedTypeRef::Primitive {
                name: Primitive::String,
            },
        );
        assert_eq!(node.required, vec!["kind".to_owned(), "value".to_owned()]);
        assert_eq!(node.additional, Some(Additional::Refused));
        assert_eq!(node.properties.0[0].1.constant, Some("person".to_owned()));
    }

    #[test]
    fn a_reference_is_a_pointer_into_the_defs_of_the_document_holding_it() {
        let name = QualifiedName::new("billing.invoice.Money").expect("a qualified name");
        assert_eq!(pointer(&name), "#/$defs/billing.invoice.Money");
        assert_eq!(
            Node::referring_to(&name).reference.as_deref(),
            Some("#/$defs/billing.invoice.Money"),
            "a nested reference and a document root spell the same pointer"
        );
        // The key and the pointer are one decision, not two: a definition is filed under the bare
        // qualified name, so a pointer spelt any other way resolves to nothing in its own document.
        let key = name.to_string();
        assert_eq!(pointer(&name), format!("#/$defs/{key}"));
    }
}
