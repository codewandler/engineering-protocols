//! A format-neutral dynamic value.
//!
//! Some document fragments are genuinely untyped: a counterexample's input, an artifact's
//! metadata, the parameters of an external verifier. [`Node`] carries those without pulling a
//! specific serialization format into the domain crate, and it is the intermediate
//! representation the predicate parser works on, which is what lets one parser accept both
//! the string form (`tests.unit.failed == 0`) and the map form (`{all: [...]}`).

use std::collections::BTreeMap;
use std::fmt;

use crate::facts::Number;

/// A dynamic document value: null, boolean, number, string, sequence or mapping.
#[derive(
    Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
#[serde(untagged)]
pub enum Node {
    /// An absent value.
    #[default]
    Null,
    /// A boolean.
    Bool(bool),
    /// A number.
    Number(Number),
    /// A string.
    Text(String),
    /// An ordered sequence.
    Seq(Vec<Node>),
    /// A mapping with string keys, ordered for deterministic output.
    Map(BTreeMap<String, Node>),
}

impl Node {
    /// The name of this node's shape, for error messages.
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Null => "null",
            Self::Bool(_) => "a boolean",
            Self::Number(_) => "a number",
            Self::Text(_) => "a string",
            Self::Seq(_) => "a sequence",
            Self::Map(_) => "a mapping",
        }
    }

    /// The string contents, when this is a string.
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text(text) => Some(text),
            _ => None,
        }
    }

    /// The sequence contents, when this is a sequence.
    pub fn as_seq(&self) -> Option<&[Node]> {
        match self {
            Self::Seq(items) => Some(items),
            _ => None,
        }
    }

    /// The mapping contents, when this is a mapping.
    pub fn as_map(&self) -> Option<&BTreeMap<String, Node>> {
        match self {
            Self::Map(entries) => Some(entries),
            _ => None,
        }
    }

    /// A single-entry mapping's key and value, when this is a mapping of exactly one entry.
    ///
    /// This shape carries most of AEP's document syntax: `{all: [...]}`, `{derived_from: x}`,
    /// `{path: docs/designs/passkeys.md}`.
    pub fn as_single_entry(&self) -> Option<(&str, &Node)> {
        let entries = self.as_map()?;
        if entries.len() == 1 {
            entries
                .iter()
                .next()
                .map(|(key, value)| (key.as_str(), value))
        } else {
            None
        }
    }

    /// Interprets this node as a sequence, treating a single scalar as a one-element sequence.
    ///
    /// Documents are written by humans, who reasonably write `add: clean-room` where the
    /// schema says a list.
    pub fn as_seq_or_single(&self) -> Vec<&Node> {
        match self {
            Self::Seq(items) => items.iter().collect(),
            Self::Null => Vec::new(),
            other => vec![other],
        }
    }
}

impl fmt::Display for Node {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Null => f.write_str("null"),
            Self::Bool(value) => write!(f, "{value}"),
            Self::Number(value) => write!(f, "{value}"),
            Self::Text(value) => f.write_str(value),
            Self::Seq(items) => {
                f.write_str("[")?;
                for (index, item) in items.iter().enumerate() {
                    if index > 0 {
                        f.write_str(", ")?;
                    }
                    write!(f, "{item}")?;
                }
                f.write_str("]")
            }
            Self::Map(entries) => {
                f.write_str("{")?;
                for (index, (key, value)) in entries.iter().enumerate() {
                    if index > 0 {
                        f.write_str(", ")?;
                    }
                    write!(f, "{key}: {value}")?;
                }
                f.write_str("}")
            }
        }
    }
}

impl schemars::JsonSchema for Node {
    fn schema_name() -> String {
        "Node".to_owned()
    }

    fn json_schema(_: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        let mut schema = schemars::schema::SchemaObject::default();
        schema.metadata().description =
            Some("Any JSON/YAML value: null, boolean, number, string, array or object.".to_owned());
        schema.into()
    }
}

impl From<bool> for Node {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

impl From<&str> for Node {
    fn from(value: &str) -> Self {
        Self::Text(value.to_owned())
    }
}

impl From<String> for Node {
    fn from(value: String) -> Self {
        Self::Text(value)
    }
}

impl From<Number> for Node {
    fn from(value: Number) -> Self {
        Self::Number(value)
    }
}
