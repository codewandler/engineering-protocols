//! Major versions and versioned references.
//!
//! AEP versions four things: protocols, principles, workflows and profiles. References are
//! written `<id>/<major>`, optionally with a kind prefix:
//!
//! ```text
//! aep/1
//! protocol:aep/1
//! principle:test-driven/1
//! workflow:incident-standard/2
//! adp/default            <- no version pinned; `default` is part of the workflow id
//! ```
//!
//! Because workflow and profile ids may themselves contain `/`, the version suffix is
//! recognised only when the trailing segment is a bare integer. Identifiers are forbidden
//! from ending in a numeric segment (see [`crate::ids`]), so the rule is unambiguous.

use std::fmt;
use std::str::FromStr;

use crate::error::ParseError;
use crate::ids::{PrincipleId, ProfileId, ProtocolId, WorkflowId};

/// A protocol major version.
///
/// Only the major version is protocol-relevant: minor changes are, by definition, ones a
/// consumer written against the major version can ignore. An engine rejects a major version
/// it does not implement rather than guessing.
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
#[serde(transparent)]
pub struct MajorVersion(u32);

impl MajorVersion {
    /// Version 1.
    pub const V1: Self = Self(1);

    /// Builds a major version. Zero is rejected: `0` invites "unversioned" documents.
    pub fn new(value: u32) -> Result<Self, ParseError> {
        if value == 0 {
            return Err(ParseError::reference(
                "version",
                "0",
                "major version must be 1 or greater",
            ));
        }
        Ok(Self(value))
    }

    /// The numeric value.
    pub const fn get(self) -> u32 {
        self.0
    }
}

impl fmt::Display for MajorVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for MajorVersion {
    type Err = ParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let parsed = value
            .parse::<u32>()
            .map_err(|_| ParseError::reference("version", value, "expected an integer"))?;
        Self::new(parsed)
    }
}

/// Strips an optional `<kind>:` prefix.
fn strip_prefix<'a>(value: &'a str, prefix: &str) -> &'a str {
    value.strip_prefix(prefix).map_or(value, |rest| rest)
}

/// Splits a reference into its identifier and an optional pinned major version.
///
/// The trailing `/<n>` is a version only when `<n>` is a bare integer, which is why
/// `adp/default` keeps its second segment while `incident-standard/2` does not.
fn split_version(value: &str) -> (&str, Option<&str>) {
    match value.rsplit_once('/') {
        Some((head, tail))
            if !tail.is_empty() && !head.is_empty() && tail.chars().all(|c| c.is_ascii_digit()) =>
        {
            (head, Some(tail))
        }
        _ => (value, None),
    }
}

/// Reference to a protocol at a specific major version, such as `aep/1`.
///
/// The version is mandatory: a task that does not say which protocol version it targets
/// cannot be executed deterministically.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
#[serde(into = "String")]
pub struct ProtocolRef {
    protocol: ProtocolId,
    major: MajorVersion,
}

impl ProtocolRef {
    /// Builds a protocol reference.
    pub fn new(protocol: ProtocolId, major: MajorVersion) -> Self {
        Self { protocol, major }
    }

    /// The protocol id.
    pub fn protocol(&self) -> &ProtocolId {
        &self.protocol
    }

    /// The pinned major version.
    pub fn major(&self) -> MajorVersion {
        self.major
    }

    /// The pattern published in generated JSON Schema.
    pub const PATTERN: &'static str = "^(protocol:)?[a-z][a-z0-9-]*/[1-9][0-9]*$";
}

impl fmt::Display for ProtocolRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.protocol, self.major)
    }
}

impl fmt::Debug for ProtocolRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ProtocolRef({self})")
    }
}

impl FromStr for ProtocolRef {
    type Err = ParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let body = strip_prefix(value, "protocol:");
        let (id, version) = split_version(body);
        let Some(version) = version else {
            return Err(ParseError::reference(
                "protocol",
                value,
                "a protocol reference must pin a major version, for example `aep/1`",
            ));
        };
        Ok(Self {
            protocol: ProtocolId::new(id)?,
            major: version.parse()?,
        })
    }
}

impl From<ProtocolRef> for String {
    fn from(value: ProtocolRef) -> Self {
        value.to_string()
    }
}

/// Declares an optionally-versioned reference type for one identifier kind.
macro_rules! versioned_ref {
    ($(#[$meta:meta])* $name:ident, $id:ty, $kind:literal, $prefix:literal, $pattern:literal) => {
        $(#[$meta])*
        #[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
        #[serde(into = "String")]
        pub struct $name {
            id: $id,
            major: Option<MajorVersion>,
        }

        impl $name {
            /// Builds a reference, optionally pinning a major version.
            pub fn new(id: $id, major: Option<MajorVersion>) -> Self {
                Self { id, major }
            }

            /// Builds an unpinned reference, which resolves to whatever the registry holds.
            pub fn unpinned(id: $id) -> Self {
                Self { id, major: None }
            }

            /// The referenced identifier.
            pub fn id(&self) -> &$id {
                &self.id
            }

            /// The pinned major version, if any.
            pub fn major(&self) -> Option<MajorVersion> {
                self.major
            }

            /// `true` when this reference accepts `candidate`.
            pub fn accepts(&self, candidate: MajorVersion) -> bool {
                self.major.map_or(true, |pinned| pinned == candidate)
            }

            /// The pattern published in generated JSON Schema.
            pub const PATTERN: &'static str = $pattern;
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                match self.major {
                    Some(major) => write!(f, "{}/{}", self.id, major),
                    None => write!(f, "{}", self.id),
                }
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}({})", stringify!($name), self)
            }
        }

        impl FromStr for $name {
            type Err = ParseError;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                let body = strip_prefix(value, $prefix);
                let (id, version) = split_version(body);
                Ok(Self {
                    id: <$id>::new(id)?,
                    major: version.map(str::parse).transpose()?,
                })
            }
        }

        impl From<$name> for String {
            fn from(value: $name) -> Self {
                value.to_string()
            }
        }

        impl From<$id> for $name {
            fn from(id: $id) -> Self {
                Self::unpinned(id)
            }
        }

        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                let raw = String::deserialize(d)?;
                raw.parse().map_err(serde::de::Error::custom)
            }
        }

        impl schemars::JsonSchema for $name {
            fn schema_name() -> String {
                stringify!($name).to_owned()
            }

            fn json_schema(_: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
                let mut schema = schemars::schema::SchemaObject {
                    instance_type: Some(schemars::schema::InstanceType::String.into()),
                    ..Default::default()
                };
                schema.string().pattern = Some($pattern.to_owned());
                schema.metadata().description = Some(concat!(
                    "Reference to a ", $kind, ", optionally pinned to a major version."
                ).to_owned());
                schema.into()
            }
        }
    };
}

versioned_ref!(
    /// Reference to a principle, such as `test-driven` or `principle:test-driven/1`.
    PrincipleRef,
    PrincipleId,
    "principle",
    "principle:",
    "^(principle:)?[a-z][a-z0-9-]*(/[1-9][0-9]*)?$"
);

versioned_ref!(
    /// Reference to a workflow, such as `adp/default` or `workflow:incident-standard/2`.
    WorkflowRef,
    WorkflowId,
    "workflow",
    "workflow:",
    "^(workflow:)?[a-z][a-z0-9-]*([./][a-z0-9-]+)*(/[1-9][0-9]*)?$"
);

versioned_ref!(
    /// Reference to a profile, such as `development.standard`.
    ProfileVersionedRef,
    ProfileId,
    "profile",
    "profile:",
    "^(profile:)?[a-z][a-z0-9-]*([./][a-z0-9-]+)*(/[1-9][0-9]*)?$"
);

impl<'de> serde::Deserialize<'de> for ProtocolRef {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(d)?;
        raw.parse().map_err(serde::de::Error::custom)
    }
}

impl schemars::JsonSchema for ProtocolRef {
    fn schema_name() -> String {
        "ProtocolRef".to_owned()
    }

    fn json_schema(_: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        let mut schema = schemars::schema::SchemaObject {
            instance_type: Some(schemars::schema::InstanceType::String.into()),
            ..Default::default()
        };
        schema.string().pattern = Some(Self::PATTERN.to_owned());
        schema.metadata().description =
            Some("Reference to a protocol at a major version, such as `aep/1`.".to_owned());
        schema.into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_protocol_refs() {
        let reference: ProtocolRef = "aep/1".parse().expect("parses");
        assert_eq!(reference.protocol().as_str(), "aep");
        assert_eq!(reference.major(), MajorVersion::V1);
        assert_eq!(reference.to_string(), "aep/1");
        assert_eq!(
            "protocol:aep/1".parse::<ProtocolRef>().expect("parses"),
            reference
        );
    }

    #[test]
    fn protocol_refs_require_a_version() {
        let err = "aep".parse::<ProtocolRef>().expect_err("no version");
        assert!(
            err.to_string().contains("must pin a major version"),
            "{err}"
        );
    }

    #[test]
    fn distinguishes_a_version_suffix_from_a_namespaced_id() {
        let versioned: WorkflowRef = "workflow:incident-standard/2".parse().expect("parses");
        assert_eq!(versioned.id().as_str(), "incident-standard");
        assert_eq!(versioned.major(), Some(MajorVersion(2)));

        let namespaced: WorkflowRef = "adp/default".parse().expect("parses");
        assert_eq!(namespaced.id().as_str(), "adp/default");
        assert_eq!(namespaced.major(), None);
    }

    #[test]
    fn unpinned_refs_accept_any_version() {
        let unpinned: PrincipleRef = "test-driven".parse().expect("parses");
        assert!(unpinned.accepts(MajorVersion::V1));
        assert!(unpinned.accepts(MajorVersion(7)));

        let pinned: PrincipleRef = "principle:test-driven/1".parse().expect("parses");
        assert!(pinned.accepts(MajorVersion::V1));
        assert!(!pinned.accepts(MajorVersion(2)));
    }

    #[test]
    fn rejects_zero_versions() {
        assert!("aep/0".parse::<ProtocolRef>().is_err());
    }
}
