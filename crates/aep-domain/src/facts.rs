//! Facts: the observable values predicates are evaluated against.
//!
//! A fact is a dotted path bound to a scalar value, such as `tests.unit.failed = 0`. Facts
//! are *projected* from evidence and from the artifact graph — the engine never invents one
//! — and a predicate can only reference paths the protocol declares observable, so a
//! completion condition cannot quietly depend on something nothing produces.
//!
//! # Ordered scales
//!
//! Some facts are ordered but not numeric (`risk`, `severity`). A protocol declares scales
//! so that `risk >= medium` has a defined meaning; without a declared scale, ordering
//! comparisons on strings evaluate to [`Unknown`](crate::predicate::Truth::Unknown) rather
//! than to an arbitrary lexicographic answer.

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;
use std::sync::OnceLock;

use crate::error::ParseError;

/// A number that is never NaN, so it can be totally ordered and compared for equality.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct Number(f64);

impl Number {
    /// Builds a number, rejecting NaN.
    pub fn new(value: f64) -> Result<Self, ParseError> {
        if value.is_nan() {
            return Err(ParseError::shape("number", "a real number", "NaN"));
        }
        Ok(Self(value))
    }

    /// The underlying value.
    pub const fn get(self) -> f64 {
        self.0
    }

    /// `true` when the value has no fractional part and fits an [`i64`].
    pub fn is_integral(self) -> bool {
        self.0.fract() == 0.0 && self.0.abs() < 9.007_199_254_740_992e15
    }
}

impl From<i64> for Number {
    fn from(value: i64) -> Self {
        #[allow(clippy::cast_precision_loss)]
        Self(value as f64)
    }
}

impl From<usize> for Number {
    fn from(value: usize) -> Self {
        #[allow(clippy::cast_precision_loss)]
        Self(value as f64)
    }
}

impl From<u32> for Number {
    fn from(value: u32) -> Self {
        Self(f64::from(value))
    }
}

impl PartialEq for Number {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl Eq for Number {}

impl PartialOrd for Number {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Number {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.total_cmp(&other.0)
    }
}

impl fmt::Display for Number {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_integral() {
            // `is_integral` has already established that the value fits an i64 exactly.
            #[allow(clippy::cast_possible_truncation)]
            let integral = self.0 as i64;
            write!(f, "{integral}")
        } else {
            write!(f, "{}", self.0)
        }
    }
}

impl schemars::JsonSchema for Number {
    fn schema_name() -> String {
        "Number".to_owned()
    }

    fn json_schema(_: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        let mut schema = schemars::schema::SchemaObject {
            instance_type: Some(schemars::schema::InstanceType::Number.into()),
            ..Default::default()
        };
        schema.metadata().description = Some("A number; NaN is not permitted.".to_owned());
        schema.into()
    }
}

/// The value a fact is bound to.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum FactValue {
    /// A boolean, such as `recovery_verified = true`.
    Bool(bool),
    /// A number, such as `tests.unit.failed = 0`.
    Number(Number),
    /// A string, such as `test.result = failed`.
    Text(String),
}

impl FactValue {
    /// A boolean fact value.
    pub const fn bool(value: bool) -> Self {
        Self::Bool(value)
    }

    /// A numeric fact value from a count.
    pub fn count(value: usize) -> Self {
        Self::Number(Number::from(value))
    }

    /// A numeric fact value.
    pub fn number(value: f64) -> Result<Self, ParseError> {
        Ok(Self::Number(Number::new(value)?))
    }

    /// A textual fact value.
    pub fn text(value: impl Into<String>) -> Self {
        Self::Text(value.into())
    }

    /// The name of this value's type, for error messages.
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Bool(_) => "boolean",
            Self::Number(_) => "number",
            Self::Text(_) => "text",
        }
    }

    /// The string contents, when this is text.
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text(text) => Some(text),
            _ => None,
        }
    }

    /// The numeric contents, when this is a number.
    pub fn as_number(&self) -> Option<Number> {
        match self {
            Self::Number(number) => Some(*number),
            _ => None,
        }
    }

    /// The boolean contents, when this is a boolean.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(value) => Some(*value),
            _ => None,
        }
    }

    /// Interprets a bare path reference: booleans by their value, everything else as "the
    /// fact is present, therefore true".
    pub fn is_truthy(&self) -> bool {
        match self {
            Self::Bool(value) => *value,
            Self::Number(number) => number.get() != 0.0,
            Self::Text(text) => !text.is_empty() && text != "false",
        }
    }

    /// Parses a literal as written in a predicate expression.
    ///
    /// `true`/`false` become booleans, anything parsable as a number becomes a number, a
    /// quoted string becomes text verbatim, and any other bare word becomes text.
    pub fn parse_literal(raw: &str) -> Self {
        let trimmed = raw.trim();
        if let Some(quoted) = strip_quotes(trimmed) {
            return Self::Text(quoted.to_owned());
        }
        match trimmed {
            "true" => return Self::Bool(true),
            "false" => return Self::Bool(false),
            _ => {}
        }
        if let Ok(number) = trimmed.parse::<f64>() {
            if !number.is_nan() {
                return Self::Number(Number(number));
            }
        }
        Self::Text(trimmed.to_owned())
    }
}

/// Removes matching single or double quotes, returning the contents.
fn strip_quotes(value: &str) -> Option<&str> {
    for quote in ['"', '\''] {
        if value.len() >= 2 && value.starts_with(quote) && value.ends_with(quote) {
            return Some(&value[1..value.len() - 1]);
        }
    }
    None
}

impl fmt::Display for FactValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bool(value) => write!(f, "{value}"),
            Self::Number(value) => write!(f, "{value}"),
            Self::Text(value) => f.write_str(value),
        }
    }
}

impl schemars::JsonSchema for FactValue {
    fn schema_name() -> String {
        "FactValue".to_owned()
    }

    fn json_schema(generator: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        let mut schema = schemars::schema::SchemaObject::default();
        schema.subschemas().any_of = Some(vec![
            <bool>::json_schema(generator),
            <Number>::json_schema(generator),
            <String>::json_schema(generator),
        ]);
        schema.metadata().description = Some("A fact value: boolean, number or string.".to_owned());
        schema.into()
    }
}

impl From<bool> for FactValue {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

impl From<usize> for FactValue {
    fn from(value: usize) -> Self {
        Self::Number(Number::from(value))
    }
}

impl From<Number> for FactValue {
    fn from(value: Number) -> Self {
        Self::Number(value)
    }
}

impl From<&str> for FactValue {
    fn from(value: &str) -> Self {
        Self::Text(value.to_owned())
    }
}

/// A dotted path identifying an observable value, such as `tests.unit.failed`.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
#[serde(into = "String")]
pub struct FactPath {
    segments: Vec<String>,
}

impl FactPath {
    /// Parses a dotted fact path.
    pub fn new(value: impl AsRef<str>) -> Result<Self, ParseError> {
        let value = value.as_ref();
        let reject = |reason: String| Err(ParseError::identifier("fact path", value, reason));

        if value.is_empty() {
            return reject("must not be empty".to_owned());
        }
        let segments: Vec<String> = value.split('.').map(ToOwned::to_owned).collect();
        for segment in &segments {
            if segment.is_empty() {
                return reject(
                    "has an empty segment; dots must not lead, trail or repeat".to_owned(),
                );
            }
            for ch in segment.chars() {
                if !(ch.is_ascii_alphanumeric() || ch == '_' || ch == '-') {
                    return reject(format!("contains disallowed character {ch:?}"));
                }
            }
        }
        if !value.starts_with(|c: char| c.is_ascii_alphabetic()) {
            return reject("must start with a letter".to_owned());
        }
        Ok(Self { segments })
    }

    /// Builds a path from already-valid segments, joining with dots.
    ///
    /// # Panics
    ///
    /// Panics when the resulting path is not a valid fact path. Callers inside this workspace
    /// build paths from validated identifiers, so a panic here is a bug in fact projection,
    /// not bad input.
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
            .unwrap_or_else(|error| panic!("fact projection built an invalid path: {error}"))
    }

    /// The path segments.
    pub fn segments(&self) -> &[String] {
        &self.segments
    }

    /// The first segment, which is the namespace the protocol declares as observable.
    pub fn namespace(&self) -> &str {
        &self.segments[0]
    }

    /// This path with `segment` appended.
    #[must_use]
    pub fn child(&self, segment: &str) -> Self {
        let mut segments = self.segments.clone();
        segments.push(segment.to_owned());
        Self { segments }
    }

    /// The pattern published in generated JSON Schema.
    pub const PATTERN: &'static str = "^[A-Za-z][A-Za-z0-9_-]*(\\.[A-Za-z0-9_-]+)*$";
}

impl fmt::Display for FactPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.segments.join("."))
    }
}

impl fmt::Debug for FactPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "FactPath({self})")
    }
}

impl FromStr for FactPath {
    type Err = ParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

impl From<FactPath> for String {
    fn from(value: FactPath) -> Self {
        value.to_string()
    }
}

impl<'de> serde::Deserialize<'de> for FactPath {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(d)?;
        Self::new(raw).map_err(serde::de::Error::custom)
    }
}

impl schemars::JsonSchema for FactPath {
    fn schema_name() -> String {
        "FactPath".to_owned()
    }

    fn json_schema(_: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        let mut schema = schemars::schema::SchemaObject {
            instance_type: Some(schemars::schema::InstanceType::String.into()),
            ..Default::default()
        };
        schema.string().pattern = Some(Self::PATTERN.to_owned());
        schema.metadata().description =
            Some("Dotted path to an observable value, such as `tests.unit.failed`.".to_owned());
        schema.into()
    }
}

/// A pattern matching a family of fact paths, such as `tests.**` or `artifact.*.status`.
///
/// `*` matches exactly one segment; a trailing `**` matches one or more remaining segments.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
#[serde(into = "String")]
pub struct FactPattern {
    segments: Vec<String>,
}

impl FactPattern {
    /// Parses a fact pattern.
    pub fn new(value: impl AsRef<str>) -> Result<Self, ParseError> {
        let value = value.as_ref();
        let reject = |reason: String| Err(ParseError::identifier("fact pattern", value, reason));

        if value.is_empty() {
            return reject("must not be empty".to_owned());
        }
        let segments: Vec<String> = value.split('.').map(ToOwned::to_owned).collect();
        let last = segments.len() - 1;
        for (index, segment) in segments.iter().enumerate() {
            if segment == "**" {
                if index != last {
                    return reject("`**` may only appear as the final segment".to_owned());
                }
                continue;
            }
            if segment == "*" {
                continue;
            }
            if segment.is_empty() {
                return reject("has an empty segment".to_owned());
            }
            for ch in segment.chars() {
                if !(ch.is_ascii_alphanumeric() || ch == '_' || ch == '-') {
                    return reject(format!("contains disallowed character {ch:?}"));
                }
            }
        }
        Ok(Self { segments })
    }

    /// `true` when `path` matches this pattern.
    pub fn matches(&self, path: &FactPath) -> bool {
        let actual = path.segments();
        for (index, pattern) in self.segments.iter().enumerate() {
            if pattern == "**" {
                return actual.len() > index;
            }
            let Some(segment) = actual.get(index) else {
                return false;
            };
            if pattern != "*" && pattern != segment {
                return false;
            }
        }
        actual.len() == self.segments.len()
    }

    /// The pattern published in generated JSON Schema.
    pub const PATTERN: &'static str = "^([A-Za-z0-9_*-]+)(\\.[A-Za-z0-9_*-]+)*$";
}

impl fmt::Display for FactPattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.segments.join("."))
    }
}

impl fmt::Debug for FactPattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "FactPattern({self})")
    }
}

impl FromStr for FactPattern {
    type Err = ParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

impl From<FactPattern> for String {
    fn from(value: FactPattern) -> Self {
        value.to_string()
    }
}

impl<'de> serde::Deserialize<'de> for FactPattern {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(d)?;
        Self::new(raw).map_err(serde::de::Error::custom)
    }
}

impl schemars::JsonSchema for FactPattern {
    fn schema_name() -> String {
        "FactPattern".to_owned()
    }

    fn json_schema(_: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        let mut schema = schemars::schema::SchemaObject {
            instance_type: Some(schemars::schema::InstanceType::String.into()),
            ..Default::default()
        };
        schema.string().pattern = Some(Self::PATTERN.to_owned());
        schema.metadata().description = Some(
            "Pattern over fact paths; `*` matches one segment, a trailing `**` matches the rest."
                .to_owned(),
        );
        schema.into()
    }
}

/// Named ordered scales, so that non-numeric facts can still be compared with `<` and `>=`.
#[derive(
    Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(transparent)]
pub struct Scales {
    /// Scale name to its values, lowest rank first.
    scales: BTreeMap<String, Vec<String>>,
}

impl Scales {
    /// A shared empty scale set, used as the default for fact sources that declare none.
    pub fn empty() -> &'static Self {
        static EMPTY: OnceLock<Scales> = OnceLock::new();
        EMPTY.get_or_init(Self::default)
    }

    /// `true` when no scales are declared.
    pub fn is_empty(&self) -> bool {
        self.scales.is_empty()
    }

    /// Absorbs another scale set, keeping this set's definitions on conflict.
    pub fn extend(&mut self, other: &Self) {
        for (name, values) in &other.scales {
            self.scales
                .entry(name.clone())
                .or_insert_with(|| values.clone());
        }
    }

    /// Declares a scale, lowest value first.
    pub fn insert(&mut self, name: impl Into<String>, values: Vec<String>) {
        self.scales.insert(name.into(), values);
    }

    /// The declared scales.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &Vec<String>)> {
        self.scales.iter()
    }

    /// Compares two values using the unique scale that contains both.
    ///
    /// Returns `None` when no scale contains both values, or when more than one does and they
    /// disagree — an ambiguous comparison is reported as unknown rather than guessed.
    pub fn compare(&self, left: &str, right: &str) -> Option<Ordering> {
        let mut result: Option<Ordering> = None;
        for values in self.scales.values() {
            let left_rank = values.iter().position(|value| value == left);
            let right_rank = values.iter().position(|value| value == right);
            if let (Some(left_rank), Some(right_rank)) = (left_rank, right_rank) {
                let ordering = left_rank.cmp(&right_rank);
                match result {
                    None => result = Some(ordering),
                    Some(previous) if previous == ordering => {}
                    Some(_) => return None,
                }
            }
        }
        result
    }
}

/// A source of facts a predicate can be evaluated against.
pub trait FactSource {
    /// The value bound to `path`, or `None` when nothing has observed it.
    fn fact(&self, path: &FactPath) -> Option<FactValue>;

    /// The ordered scales available for non-numeric comparison.
    fn scales(&self) -> &Scales {
        Scales::empty()
    }
}

/// An in-memory set of facts.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct FactStore {
    facts: BTreeMap<FactPath, FactValue>,
    #[serde(skip_serializing_if = "is_empty_scales")]
    scales: Scales,
}

/// Whether a scale set is empty, for output suppression.
fn is_empty_scales(scales: &Scales) -> bool {
    scales.is_empty()
}

impl FactStore {
    /// An empty store.
    pub fn new() -> Self {
        Self::default()
    }

    /// Binds `path` to `value`, replacing any previous binding.
    pub fn set(&mut self, path: FactPath, value: impl Into<FactValue>) {
        self.facts.insert(path, value.into());
    }

    /// Binds a path given as a string.
    ///
    /// # Panics
    ///
    /// Panics when `path` is not a valid fact path; intended for statically-known paths in
    /// fact projection and tests.
    pub fn set_path(&mut self, path: &str, value: impl Into<FactValue>) {
        let path = FactPath::new(path).unwrap_or_else(|error| panic!("invalid fact path: {error}"));
        self.set(path, value);
    }

    /// Binds `path` to `value` only when it is not already bound.
    pub fn set_if_absent(&mut self, path: FactPath, value: impl Into<FactValue>) {
        self.facts.entry(path).or_insert_with(|| value.into());
    }

    /// Absorbs every fact from `other`, overwriting on conflict.
    pub fn extend(&mut self, other: Self) {
        self.facts.extend(other.facts);
    }

    /// Absorbs facts from an iterator.
    pub fn extend_facts<I: IntoIterator<Item = (FactPath, FactValue)>>(&mut self, facts: I) {
        self.facts.extend(facts);
    }

    /// Declares the ordered scales used for non-numeric comparisons.
    pub fn set_scales(&mut self, scales: Scales) {
        self.scales = scales;
    }

    /// The number of bound facts.
    pub fn len(&self) -> usize {
        self.facts.len()
    }

    /// `true` when no facts are bound.
    pub fn is_empty(&self) -> bool {
        self.facts.is_empty()
    }

    /// Every bound fact, in path order.
    pub fn iter(&self) -> impl Iterator<Item = (&FactPath, &FactValue)> {
        self.facts.iter()
    }

    /// Every bound path.
    pub fn paths(&self) -> impl Iterator<Item = &FactPath> {
        self.facts.keys()
    }
}

impl FactSource for FactStore {
    fn fact(&self, path: &FactPath) -> Option<FactValue> {
        self.facts.get(path).cloned()
    }

    fn scales(&self) -> &Scales {
        &self.scales
    }
}

impl FromIterator<(FactPath, FactValue)> for FactStore {
    fn from_iter<I: IntoIterator<Item = (FactPath, FactValue)>>(iter: I) -> Self {
        Self {
            facts: iter.into_iter().collect(),
            scales: Scales::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_rejects_fact_paths() {
        assert_eq!(
            FactPath::new("tests.unit.failed")
                .expect("parses")
                .segments()
                .len(),
            3
        );
        assert!(FactPath::new("artifact.architecture-design.status").is_ok());
        assert!(FactPath::new("tests..failed").is_err());
        assert!(FactPath::new(".tests").is_err());
        assert!(FactPath::new("1tests").is_err());
        assert!(FactPath::new("tests unit").is_err());
    }

    #[test]
    fn patterns_match_segment_wise() {
        let exact = FactPattern::new("tests.unit.failed").expect("parses");
        let one = FactPattern::new("artifact.*.status").expect("parses");
        let rest = FactPattern::new("tests.**").expect("parses");

        let path = |value: &str| FactPath::new(value).expect("parses");

        assert!(exact.matches(&path("tests.unit.failed")));
        assert!(!exact.matches(&path("tests.unit")));

        assert!(one.matches(&path("artifact.design.status")));
        assert!(!one.matches(&path("artifact.design.review.status")));

        assert!(rest.matches(&path("tests.unit")));
        assert!(rest.matches(&path("tests.unit.failed")));
        assert!(
            !rest.matches(&path("tests")),
            "`**` requires at least one segment"
        );
    }

    #[test]
    fn parses_literals_by_shape() {
        assert_eq!(FactValue::parse_literal("true"), FactValue::Bool(true));
        assert_eq!(FactValue::parse_literal("0"), FactValue::count(0));
        assert_eq!(
            FactValue::parse_literal("0.01"),
            FactValue::number(0.01).expect("finite")
        );
        assert_eq!(
            FactValue::parse_literal("failed"),
            FactValue::text("failed")
        );
        assert_eq!(
            FactValue::parse_literal("\"1.2.3\""),
            FactValue::text("1.2.3")
        );
    }

    #[test]
    fn scales_order_non_numeric_values() {
        let mut scales = Scales::default();
        scales.insert(
            "risk",
            ["low", "medium", "high", "critical"]
                .map(ToOwned::to_owned)
                .to_vec(),
        );

        assert_eq!(scales.compare("high", "medium"), Some(Ordering::Greater));
        assert_eq!(scales.compare("low", "low"), Some(Ordering::Equal));
        assert_eq!(scales.compare("high", "unknown-value"), None);
    }

    #[test]
    fn rejects_nan_numbers() {
        assert!(Number::new(f64::NAN).is_err());
        assert!(Number::new(1.5).is_ok());
    }
}
