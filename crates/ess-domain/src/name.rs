//! Identity, naming and versions.
//!
//! Three different things, kept apart from the first version rather than merged and separated later
//! (the review's F5). They differ in what a change to them costs:
//!
//! | | changes when | breaks |
//! |---|---|---|
//! | **qualified name** — `billing.invoice.CreateInvoice` | the concept is renamed | every reference in the model |
//! | **wire name** — `CreateInvoice`, `invoices.created.v1` | a transport is reshaped | every consumer already deployed |
//! | **display name** — "Create invoice" | somebody improves the wording | nothing |
//!
//! Merging them is why a cosmetic rename becomes a breaking change: an author fixes a capitalisation
//! and a Kafka topic moves.

use std::fmt;
use std::str::FromStr;

use aep_domain::error::ParseError;

/// One segment of a qualified name.
///
/// Segments are lower-case for namespaces and `UpperCamelCase` for concepts; both are accepted here
/// and distinguished by position, because `billing.invoice.CreateInvoice` reads correctly and
/// `billing.invoice.create_invoice` does not.
fn validate_segment(value: &str, whole: &str) -> Result<(), ParseError> {
    if value.is_empty() {
        return Err(ParseError::identifier(
            "qualified name",
            whole,
            "has an empty segment; dots must not lead, trail or repeat".to_owned(),
        ));
    }
    let first = value.chars().next().unwrap_or('_');
    if !first.is_ascii_alphabetic() {
        return Err(ParseError::identifier(
            "qualified name",
            whole,
            format!("segment {value:?} must start with a letter"),
        ));
    }
    for character in value.chars() {
        if !(character.is_ascii_alphanumeric() || character == '-' || character == '_') {
            return Err(ParseError::identifier(
                "qualified name",
                whole,
                format!("segment {value:?} contains {character:?}"),
            ));
        }
    }
    Ok(())
}

/// The stable logical identity of anything in a specification.
///
/// `billing.invoice.CreateInvoice`. This is what references resolve against, and what a diff
/// compares; it is not what appears on the wire.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
#[serde(into = "String")]
pub struct QualifiedName {
    segments: Vec<String>,
}

impl QualifiedName {
    /// Parses a dotted qualified name.
    pub fn new(value: impl AsRef<str>) -> Result<Self, ParseError> {
        let value = value.as_ref();
        if value.is_empty() {
            return Err(ParseError::identifier(
                "qualified name",
                value,
                "must not be empty".to_owned(),
            ));
        }
        let segments: Vec<String> = value.split('.').map(ToOwned::to_owned).collect();
        for segment in &segments {
            validate_segment(segment, value)?;
        }
        Ok(Self { segments })
    }

    /// Builds a name from segments.
    ///
    /// # Panics
    ///
    /// Panics when the result is not a valid qualified name; callers inside this crate build them
    /// from already-validated parts.
    pub fn from_segments<I, S>(segments: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let joined = segments
            .into_iter()
            .map(|segment| segment.as_ref().to_owned())
            .collect::<Vec<_>>()
            .join(".");
        Self::new(&joined)
            .unwrap_or_else(|error| panic!("built an invalid qualified name: {error}"))
    }

    /// The segments.
    pub fn segments(&self) -> &[String] {
        &self.segments
    }

    /// The last segment — the concept's own name.
    pub fn local(&self) -> &str {
        self.segments.last().map_or("", String::as_str)
    }

    /// Everything but the last segment.
    pub fn namespace(&self) -> Option<Self> {
        if self.segments.len() < 2 {
            return None;
        }
        Some(Self {
            segments: self.segments[..self.segments.len() - 1].to_vec(),
        })
    }

    /// This name with `segment` appended.
    #[must_use]
    pub fn child(&self, segment: &str) -> Self {
        let mut segments = self.segments.clone();
        segments.push(segment.to_owned());
        Self { segments }
    }

    /// `true` when this name sits inside `namespace`.
    pub fn is_within(&self, namespace: &Self) -> bool {
        self.segments.len() > namespace.segments.len()
            && self.segments[..namespace.segments.len()] == namespace.segments[..]
    }

    /// The pattern published in generated JSON Schema.
    pub const PATTERN: &'static str = "^[A-Za-z][A-Za-z0-9_-]*(\\.[A-Za-z][A-Za-z0-9_-]*)*$";
}

impl fmt::Display for QualifiedName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.segments.join("."))
    }
}

impl fmt::Debug for QualifiedName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "QualifiedName({self})")
    }
}

impl FromStr for QualifiedName {
    type Err = ParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

impl From<QualifiedName> for String {
    fn from(value: QualifiedName) -> Self {
        value.to_string()
    }
}

impl<'de> serde::Deserialize<'de> for QualifiedName {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        Self::new(raw).map_err(serde::de::Error::custom)
    }
}

impl schemars::JsonSchema for QualifiedName {
    fn schema_name() -> String {
        "QualifiedName".to_owned()
    }

    fn json_schema(_: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        let mut schema = schemars::schema::SchemaObject {
            instance_type: Some(schemars::schema::InstanceType::String.into()),
            ..Default::default()
        };
        schema.string().pattern = Some(Self::PATTERN.to_owned());
        schema.metadata().description = Some(
            "Stable logical identity, such as `billing.invoice.CreateInvoice`. Not a wire name."
                .to_owned(),
        );
        schema.into()
    }
}

/// What a concept is called on the wire, and what a person is shown.
///
/// Both are optional and both default to the qualified name's last segment. The point of separating
/// them is that changing either is a different kind of event: a display change is free, a wire
/// change breaks deployed consumers, and a qualified-name change breaks the model.
#[derive(
    Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct Naming {
    /// What this is called on the wire — an HTTP path segment, a topic name, a JSON field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wire: Option<String>,
    /// What a person is shown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display: Option<String>,
    /// One line about what it is, for generated documentation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
}

impl Naming {
    /// The wire name, falling back to the concept's own name.
    pub fn wire_or<'a>(&'a self, name: &'a QualifiedName) -> &'a str {
        self.wire.as_deref().unwrap_or_else(|| name.local())
    }

    /// The display name, falling back to the concept's own name.
    pub fn display_or<'a>(&'a self, name: &'a QualifiedName) -> &'a str {
        self.display.as_deref().unwrap_or_else(|| name.local())
    }

    /// `true` when nothing is overridden.
    pub fn is_empty(&self) -> bool {
        self.wire.is_none() && self.display.is_none() && self.summary.is_none()
    }
}

/// A major version of something in a specification, written `v1`.
///
/// Only the major part exists on purpose: a minor version that consumers are expected to ignore is
/// not something the model should carry, and one they are not expected to ignore is a major version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
#[serde(transparent)]
pub struct Version(u32);

impl Version {
    /// Version 1.
    pub const V1: Self = Self(1);

    /// What a written version looks like, for the generated schema.
    ///
    /// Kept beside [`Version::parse`] so the two are read together; a test asserts they agree,
    /// because a schema that accepts what the parser refuses is worse than no schema.
    pub const PATTERN: &'static str = "^v[1-9][0-9]*$";

    /// Builds a version. Zero is rejected: it invites "unversioned".
    pub fn new(value: u32) -> Result<Self, ParseError> {
        if value == 0 {
            return Err(ParseError::reference(
                "version",
                "v0",
                "versions start at 1",
            ));
        }
        Ok(Self(value))
    }

    /// The numeric value.
    pub const fn get(self) -> u32 {
        self.0
    }

    /// Parses `v1`.
    ///
    /// Stricter than `u32::from_str`, which accepts `01` and `+1`. Two spellings of one version is
    /// two documents that disagree textually and agree semantically — and the published schema
    /// cannot express "any leading zeros" without accepting `v007`, so the parser is the side that
    /// moves.
    pub fn parse(value: &str) -> Result<Self, ParseError> {
        let digits = value
            .strip_prefix('v')
            .ok_or_else(|| ParseError::reference("version", value, "versions are written `v1`"))?;
        let parsed = parse_major(digits).ok_or_else(|| {
            ParseError::reference(
                "version",
                value,
                "expected a whole number after `v`, without a leading zero",
            )
        })?;
        Self::new(parsed)
    }

    /// Reads the numeric spelling, `3` rather than `v3`.
    ///
    /// Refuses exactly what [`Version::parse`] refuses, which is the whole reason it is a function
    /// and not a cast. `4294967296 as u32` saturates to `4294967295` and `1e300` lands on the same
    /// value, so a document naming a version this type cannot hold used to become a *different*
    /// version silently — and two documents that disagree about which version they mean then
    /// compared equal. The published [`Version::PATTERN`] refuses both spellings of that, so a
    /// numeric path that accepted them was a parser looser than its own schema.
    fn from_number(number: aep_domain::facts::Number) -> Result<Self, ParseError> {
        let value = number.get();
        if !value.is_finite() || value.fract() != 0.0 {
            return Err(ParseError::reference(
                "version",
                &number.to_string(),
                "expected a whole number, without a fractional part",
            ));
        }
        if !(0.0..=f64::from(u32::MAX)).contains(&value) {
            return Err(ParseError::reference(
                "version",
                &number.to_string(),
                format!("versions run from 1 to {}", u32::MAX),
            ));
        }
        // Guarded directly above: whole, not negative, and no larger than `u32::MAX`, so the cast
        // is exact rather than saturating.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let major = value as u32;
        Self::new(major)
    }
}

/// Reads the digits of a major version: one or more, no sign, no leading zero.
///
/// Shared so a version and a format version cannot drift into accepting different spellings; both
/// publish a `^…[1-9][0-9]*$` pattern, and a parser looser than its own published pattern is a
/// document the schema calls invalid and the tool accepts.
pub(crate) fn parse_major(digits: &str) -> Option<u32> {
    let mut characters = digits.chars();
    let first = characters.next()?;
    if !first.is_ascii_digit() || first == '0' || !characters.all(|c| c.is_ascii_digit()) {
        return None;
    }
    digits.parse::<u32>().ok()
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "v{}", self.0)
    }
}

impl FromStr for Version {
    type Err = ParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

impl schemars::JsonSchema for Version {
    fn schema_name() -> String {
        "Version".to_owned()
    }

    // Written by hand because [`Version`]'s `Deserialize` is: a derived schema describes the
    // *representation* (a `u32`), and the representation is not what anyone writes. Every document
    // and every example says `v3`, so a derived schema tells an author that the normative example
    // is invalid — which is the one thing a schema published for editors must never do.
    fn json_schema(_: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        let text = schemars::schema::SchemaObject {
            instance_type: Some(schemars::schema::InstanceType::String.into()),
            string: Some(Box::new(schemars::schema::StringValidation {
                pattern: Some(Self::PATTERN.to_owned()),
                ..Default::default()
            })),
            ..Default::default()
        };
        // Bounded at both ends, because the parser is: an unbounded `minimum: 1` would tell an
        // author that `4294967296` is a valid version and the tool would then refuse the file.
        let number = schemars::schema::SchemaObject {
            instance_type: Some(schemars::schema::InstanceType::Integer.into()),
            number: Some(Box::new(schemars::schema::NumberValidation {
                minimum: Some(1.0),
                maximum: Some(f64::from(u32::MAX)),
                ..Default::default()
            })),
            ..Default::default()
        };
        let mut schema = schemars::schema::SchemaObject {
            subschemas: Some(Box::new(schemars::schema::SubschemaValidation {
                one_of: Some(vec![text.into(), number.into()]),
                ..Default::default()
            })),
            ..Default::default()
        };
        schema.metadata().description =
            Some("A major version, written `v1`. A bare `1` is accepted too.".to_owned());
        schema.metadata().examples = ["v1", "v3"]
            .iter()
            .map(|value| serde_json::Value::String((*value).to_owned()))
            .collect();
        schema.into()
    }
}

impl<'de> serde::Deserialize<'de> for Version {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        // Accepts `v1` and a bare `1`, because both read naturally in a document and neither is
        // ambiguous — and the two spellings agree, which is what makes that true. Whatever `v…`
        // refuses, a bare number refuses as well; see [`Version::from_number`].
        let node = aep_domain::node::Node::deserialize(deserializer)?;
        match &node {
            aep_domain::node::Node::Text(text) => {
                Self::parse(text).map_err(serde::de::Error::custom)
            }
            aep_domain::node::Node::Number(number) => {
                Self::from_number(*number).map_err(serde::de::Error::custom)
            }
            other => Err(serde::de::Error::custom(format!(
                "expected a version such as `v1`, found {}",
                other.type_name()
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_version_pattern_accepts_exactly_what_the_parser_accepts() {
        // The pattern is published; the parser is what runs. A reader who trusts the schema and is
        // then refused by the tool has been lied to by a file this repository generated.
        let pattern = regex_lite_match;
        for accepted in ["v1", "v2", "v10", "v99"] {
            assert!(Version::parse(accepted).is_ok(), "{accepted}");
            assert!(
                pattern(accepted),
                "the schema rejects `{accepted}`, which parses"
            );
        }
        for refused in ["v0", "1", "", "v", "vv1", "v1.0", "v-1", "v01"] {
            assert!(
                Version::parse(refused).is_err(),
                "{refused} should not parse"
            );
            assert!(
                !pattern(refused),
                "the schema accepts `{refused}`, which the parser refuses"
            );
        }
    }

    /// `^v[1-9][0-9]*$`, by hand — this crate has no regex dependency and does not need one for a
    /// pattern this shape.
    fn regex_lite_match(value: &str) -> bool {
        let Some(digits) = value.strip_prefix('v') else {
            return false;
        };
        let mut characters = digits.chars();
        let Some(first) = characters.next() else {
            return false;
        };
        first.is_ascii_digit() && first != '0' && characters.all(|c| c.is_ascii_digit())
    }

    #[test]
    fn the_two_spellings_of_a_version_mean_the_same_thing() {
        // `version: v3` and `version: 3` are one version written two ways. If they ever disagree,
        // two documents naming different versions compare equal and nothing says so.
        for major in [1_u32, 2, 42, u32::MAX] {
            let written: Version = serde_yaml::from_str(&format!("v{major}")).expect("written");
            let numeric: Version = serde_yaml::from_str(&major.to_string()).expect("numeric");
            assert_eq!(written, numeric, "the spellings of v{major} disagree");
            assert_eq!(written.get(), major);
            assert_eq!(
                Version::parse(&written.to_string()).expect("round trips"),
                written,
                "`parse(display(v))` is not `v`"
            );
        }
    }

    #[test]
    fn a_numeric_version_out_of_range_is_refused_rather_than_saturated() {
        // Every one of these used to become `v4294967295` or `v1`, silently: the numeric path cast
        // with `as u32`, which saturates. The written spelling refused all of them already.
        for refused in ["4294967296", "1e300", "-1", "1.5", "0"] {
            let error = serde_yaml::from_str::<Version>(refused)
                .expect_err(&format!("`{refused}` is not a version"));
            assert!(
                Version::parse(&format!("v{refused}")).is_err(),
                "the written spelling of `{refused}` must be refused too"
            );
            let rendered = error.to_string();
            assert!(
                rendered.contains("versions run from 1 to 4294967295")
                    || rendered.contains("versions start at 1")
                    || rendered.contains("whole number"),
                "the reader has to be told which bound was missed: {rendered}"
            );
        }
    }

    #[test]
    fn a_version_too_wide_for_the_document_model_is_refused_before_it_reaches_the_parser() {
        // A number is carried as an `f64`, so a literal wider than that is refused one layer out
        // and the diagnostic is the document model's rather than this type's — asserted separately
        // for exactly that reason, so a change to either message is visible here.
        let error = serde_yaml::from_str::<Version>("18446744073709551616").expect_err("too wide");
        assert!(error.to_string().contains("invalid type"), "{error}");
    }

    #[test]
    fn the_schema_bounds_the_numeric_version_where_the_parser_does() {
        // The published schema is what an editor validates against. A schema that accepts
        // `4294967296` while the parser refuses it is the same defect in the other direction.
        let mut generator = schemars::gen::SchemaGenerator::default();
        let schema = serde_json::to_value(<Version as schemars::JsonSchema>::json_schema(
            &mut generator,
        ))
        .expect("the schema serialises");
        let branches = schema["oneOf"].as_array().expect("two spellings");
        let numeric = branches
            .iter()
            .find(|branch| branch["type"] == "integer")
            .expect("a numeric branch");
        assert_eq!(numeric["minimum"], serde_json::json!(1.0));
        assert_eq!(numeric["maximum"], serde_json::json!(4_294_967_295.0));
    }

    #[test]
    fn a_qualified_name_knows_its_namespace_and_its_own_name() {
        let name: QualifiedName = "billing.invoice.CreateInvoice".parse().expect("parses");
        assert_eq!(name.local(), "CreateInvoice");
        assert_eq!(
            name.namespace().map(|namespace| namespace.to_string()),
            Some("billing.invoice".to_owned())
        );
        assert_eq!(name.to_string(), "billing.invoice.CreateInvoice");

        let root: QualifiedName = "billing".parse().expect("parses");
        assert_eq!(root.local(), "billing");
        assert!(root.namespace().is_none());
        assert!(name.is_within(&root));
        assert!(!root.is_within(&name));
    }

    #[test]
    fn malformed_names_are_refused() {
        assert!(
            QualifiedName::new("billing..invoice").is_err(),
            "empty segment"
        );
        assert!(QualifiedName::new(".billing").is_err(), "leading dot");
        assert!(QualifiedName::new("billing.").is_err(), "trailing dot");
        assert!(QualifiedName::new("1billing").is_err(), "leading digit");
        assert!(QualifiedName::new("billing invoice").is_err(), "space");
        assert!(QualifiedName::new("").is_err(), "empty");
    }

    #[test]
    fn a_wire_name_can_change_without_the_identity_changing() {
        let name: QualifiedName = "billing.invoice.InvoiceCreated".parse().expect("parses");
        let naming = Naming {
            wire: Some("invoices.created.v1".to_owned()),
            display: Some("Invoice created".to_owned()),
            summary: None,
        };

        assert_eq!(naming.wire_or(&name), "invoices.created.v1");
        assert_eq!(naming.display_or(&name), "Invoice created");
        assert_eq!(
            name.to_string(),
            "billing.invoice.InvoiceCreated",
            "renaming the topic must not move the concept"
        );
    }

    #[test]
    fn naming_defaults_to_the_concepts_own_name() {
        let name: QualifiedName = "billing.invoice.InvoiceCreated".parse().expect("parses");
        let naming = Naming::default();
        assert!(naming.is_empty());
        assert_eq!(naming.wire_or(&name), "InvoiceCreated");
        assert_eq!(naming.display_or(&name), "InvoiceCreated");
    }

    #[test]
    fn versions_start_at_one_and_read_both_ways() {
        assert_eq!(Version::parse("v3").expect("parses").get(), 3);
        assert!(
            Version::parse("3").is_err(),
            "the `v` is part of the spelling"
        );
        assert!(Version::parse("v0").is_err(), "zero invites `unversioned`");

        let from_text: Version = serde_yaml::from_str("v2").expect("parses");
        let from_number: Version = serde_yaml::from_str("2").expect("parses");
        assert_eq!(
            from_text, from_number,
            "both spellings read naturally in a document"
        );
        assert_eq!(from_text.to_string(), "v2");
    }
}
