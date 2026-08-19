//! Identifier newtypes.
//!
//! Every identifier in AEP is a validated newtype rather than a bare [`String`], so a
//! principle id cannot be passed where a state id is expected, and malformed ids are
//! rejected at the parser boundary.
//!
//! Three charset rules are used:
//!
//! | rule | shape | example |
//! |---|---|---|
//! | kebab | `[a-z][a-z0-9]*(-[a-z0-9]+)*` | `test-driven` |
//! | dotted | kebab segments separated by `.` or `/` | `development.standard`, `adp/default` |
//! | loose | alphanumeric segments separated by `.`, `-`, `_` or `/`, upper case allowed | `AUTH-142` |
//!
//! A trailing segment that is a bare integer is rejected for dotted ids, which keeps the
//! `<id>/<major>` reference syntax unambiguous (see [`crate::version`]).

use std::fmt;

use crate::error::ParseError;

/// Charset rule applied to an identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Charset {
    /// Lower-case kebab-case: `test-driven`.
    Kebab,
    /// Lower-case kebab-case segments separated by `.` or `/`: `development.standard`.
    Dotted,
    /// As [`Charset::Dotted`], additionally allowing `_` inside a segment: `adversarial_verify`.
    DottedSnake,
    /// Mixed-case alphanumeric segments separated by `.`, `/`, `-` or `_`: `AUTH-142`.
    Loose,
}

impl Charset {
    /// Characters that separate segments, for charset validation.
    fn separators(self) -> &'static [char] {
        match self {
            Self::Kebab => &['-'],
            Self::Dotted => &['.', '/', '-'],
            Self::DottedSnake | Self::Loose => &['.', '/', '-', '_'],
        }
    }

    /// Characters that separate namespace components, for the numeric-tail rule.
    fn path_separators(self) -> &'static [char] {
        match self {
            Self::Kebab | Self::Loose => &[],
            Self::Dotted | Self::DottedSnake => &['.', '/'],
        }
    }

    fn allows_upper(self) -> bool {
        self == Self::Loose
    }
}

/// Validates `value` against `charset`, returning a human-readable reason on failure.
fn validate(value: &str, charset: Charset, kind: &'static str) -> Result<(), ParseError> {
    let reject = |reason: String| Err(ParseError::identifier(kind, value, reason));

    if value.is_empty() {
        return reject("must not be empty".to_owned());
    }
    if value.len() > 200 {
        return reject(format!(
            "must be at most 200 characters, got {}",
            value.len()
        ));
    }

    let separators = charset.separators();
    let segments: Vec<&str> = value.split(|c| separators.contains(&c)).collect();

    for segment in &segments {
        if segment.is_empty() {
            return reject(format!(
                "has an empty segment; separators ({}) must not lead, trail or repeat",
                separators.iter().collect::<String>()
            ));
        }
        for ch in segment.chars() {
            let ok = ch.is_ascii_lowercase()
                || ch.is_ascii_digit()
                || (charset.allows_upper() && ch.is_ascii_uppercase());
            if !ok {
                return reject(format!("contains disallowed character {ch:?}"));
            }
        }
    }

    let first = value.chars().next().unwrap_or('_');
    if !(first.is_ascii_lowercase() || (charset.allows_upper() && first.is_ascii_alphanumeric())) {
        return reject(format!("must start with a letter, got {first:?}"));
    }

    let path_separators = charset.path_separators();
    if !path_separators.is_empty() {
        let components: Vec<&str> = value.split(|c| path_separators.contains(&c)).collect();
        if let Some(last) = components.last() {
            if last.chars().all(|c| c.is_ascii_digit()) {
                return reject(format!(
                    "must not end in a numeric segment ({last:?}); that form is reserved for \
                     version references such as `{value}/1`"
                ));
            }
        }
    }

    Ok(())
}

/// Declares an identifier newtype with a charset rule and a JSON Schema pattern.
macro_rules! identifier {
    ($(#[$meta:meta])* $name:ident, $charset:expr, $kind:literal, $pattern:literal) => {
        $(#[$meta])*
        #[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            /// Parses and validates an identifier.
            pub fn new(value: impl Into<String>) -> Result<Self, ParseError> {
                let value = value.into();
                validate(&value, $charset, $kind)?;
                Ok(Self(value))
            }

            /// The identifier as a string slice.
            pub fn as_str(&self) -> &str {
                &self.0
            }

            /// The regular expression this identifier is validated against.
            ///
            /// Published in generated JSON Schema so that non-Rust consumers can apply the
            /// same rule.
            pub const PATTERN: &'static str = $pattern;
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}({:?})", stringify!($name), self.0)
            }
        }

        impl std::str::FromStr for $name {
            type Err = ParseError;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Self::new(value)
            }
        }

        impl TryFrom<String> for $name {
            type Error = ParseError;

            fn try_from(value: String) -> Result<Self, Self::Error> {
                Self::new(value)
            }
        }

        impl From<$name> for String {
            fn from(value: $name) -> Self {
                value.0
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                &self.0
            }
        }

        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                let raw = String::deserialize(d)?;
                Self::new(raw).map_err(serde::de::Error::custom)
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
                schema.metadata().description = Some(format!("{} identifier.", $kind));
                schema.into()
            }
        }
    };
}

identifier!(
    /// Identifier of a protocol, such as `aep`, `adp` or `aop`.
    ProtocolId,
    Charset::Kebab,
    "protocol",
    "^[a-z][a-z0-9]*(-[a-z0-9]+)*$"
);

identifier!(
    /// Identifier of a principle, such as `test-driven`.
    PrincipleId,
    Charset::Kebab,
    "principle",
    "^[a-z][a-z0-9]*(-[a-z0-9]+)*$"
);

identifier!(
    /// Identifier of a profile, such as `development.standard`.
    ProfileId,
    Charset::Dotted,
    "profile",
    "^[a-z][a-z0-9]*(-[a-z0-9]+)*([./][a-z0-9]+(-[a-z0-9]+)*)*$"
);

identifier!(
    /// Identifier of a workflow, such as `adp/default`.
    WorkflowId,
    Charset::Dotted,
    "workflow",
    "^[a-z][a-z0-9]*(-[a-z0-9]+)*([./][a-z0-9]+(-[a-z0-9]+)*)*$"
);

identifier!(
    /// Identifier of a workflow state, such as `implement`.
    StateId,
    Charset::DottedSnake,
    "state",
    "^[a-z][a-z0-9]*([-_][a-z0-9]+)*([./][a-z0-9]+([-_][a-z0-9]+)*)*$"
);

identifier!(
    /// Identifier of a workflow phase, such as `implementation`.
    ///
    /// Phases are the join between principles and workflows: a principle's obligations are
    /// timed against phases (`before_implementation`), and states declare which phases they
    /// belong to.
    PhaseId,
    Charset::Kebab,
    "phase",
    "^[a-z][a-z0-9]*(-[a-z0-9]+)*$"
);

identifier!(
    /// Identifier of an obligation within a principle.
    ObligationId,
    Charset::DottedSnake,
    "obligation",
    "^[a-z][a-z0-9]*([-_][a-z0-9]+)*([./][a-z0-9]+([-_][a-z0-9]+)*)*$"
);

identifier!(
    /// Identifier of an approval, such as `security-review`.
    ApprovalId,
    Charset::Kebab,
    "approval",
    "^[a-z][a-z0-9]*(-[a-z0-9]+)*$"
);

identifier!(
    /// Identifier of a verified claim, such as `recovery` in `recovery_verified`.
    ClaimId,
    Charset::Kebab,
    "claim",
    "^[a-z][a-z0-9]*(-[a-z0-9]+)*$"
);

identifier!(
    /// Reference to an external tool, such as `cargo-nextest`.
    ToolRef,
    Charset::Dotted,
    "tool",
    "^[a-z][a-z0-9]*(-[a-z0-9]+)*([./][a-z0-9]+(-[a-z0-9]+)*)*$"
);

identifier!(
    /// Identifier of a service, such as `auth-api`.
    ServiceId,
    Charset::Dotted,
    "service",
    "^[a-z][a-z0-9-]*([./][a-z0-9-]+)*$"
);

identifier!(
    /// Identifier of an external provider holding artifacts, such as `linear` or `github`.
    ProviderId,
    Charset::Kebab,
    "provider",
    "^[a-z][a-z0-9]*(-[a-z0-9]+)*$"
);

identifier!(
    /// Reference to a repository, such as `acme/payments`.
    RepositoryRef,
    Charset::Loose,
    "repository",
    "^[A-Za-z0-9]([A-Za-z0-9]|[./_-][A-Za-z0-9])*$"
);

identifier!(
    /// Identifier of a task, such as `AUTH-142`.
    TaskId,
    Charset::Loose,
    "task",
    "^[A-Za-z0-9]([A-Za-z0-9]|[./_-][A-Za-z0-9])*$"
);

identifier!(
    /// Identifier of a single unit of evidence.
    EvidenceId,
    Charset::Loose,
    "evidence",
    "^[A-Za-z0-9]([A-Za-z0-9]|[./_-][A-Za-z0-9])*$"
);

identifier!(
    /// Identifier of a protocol execution.
    ExecutionId,
    Charset::Loose,
    "execution",
    "^[A-Za-z0-9]([A-Za-z0-9]|[./_-][A-Za-z0-9])*$"
);

/// What a piece of evidence or an approval is about, written `<kind>:<id>`.
///
/// Examples: `task:AUTH-142`, `service:auth-api`, `suite:unit`, `deployment:rev-4711`.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
#[serde(into = "String")]
pub struct SubjectRef {
    kind: String,
    id: String,
}

impl SubjectRef {
    /// Parses a `<kind>:<id>` subject reference.
    pub fn new(value: impl Into<String>) -> Result<Self, ParseError> {
        let value: String = value.into();
        let Some((kind, id)) = value.split_once(':') else {
            return Err(ParseError::identifier(
                "subject",
                &value,
                "must be written `<kind>:<id>`, for example `service:auth-api`".to_owned(),
            ));
        };
        validate(kind, Charset::Kebab, "subject kind")?;
        validate(id, Charset::Loose, "subject id")?;
        Ok(Self {
            kind: kind.to_owned(),
            id: id.to_owned(),
        })
    }

    /// Builds a subject reference for a task.
    pub fn task(task: &TaskId) -> Self {
        Self {
            kind: "task".to_owned(),
            id: task.as_str().to_owned(),
        }
    }

    /// The subject kind, such as `service`.
    pub fn kind(&self) -> &str {
        &self.kind
    }

    /// The subject id, such as `auth-api`.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// The pattern published in generated JSON Schema.
    pub const PATTERN: &'static str = "^[a-z][a-z0-9-]*:[A-Za-z0-9][A-Za-z0-9._/-]*$";
}

impl fmt::Display for SubjectRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.kind, self.id)
    }
}

impl fmt::Debug for SubjectRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SubjectRef({}:{})", self.kind, self.id)
    }
}

impl std::str::FromStr for SubjectRef {
    type Err = ParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

impl From<SubjectRef> for String {
    fn from(value: SubjectRef) -> Self {
        value.to_string()
    }
}

impl<'de> serde::Deserialize<'de> for SubjectRef {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(d)?;
        Self::new(raw).map_err(serde::de::Error::custom)
    }
}

impl schemars::JsonSchema for SubjectRef {
    fn schema_name() -> String {
        "SubjectRef".to_owned()
    }

    fn json_schema(_: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        let mut schema = schemars::schema::SchemaObject {
            instance_type: Some(schemars::schema::InstanceType::String.into()),
            ..Default::default()
        };
        schema.string().pattern = Some(Self::PATTERN.to_owned());
        schema.metadata().description =
            Some("Subject of evidence, written `<kind>:<id>`.".to_owned());
        schema.into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_well_formed_identifiers() {
        assert!(PrincipleId::new("test-driven").is_ok());
        assert!(ProfileId::new("development.standard").is_ok());
        assert!(WorkflowId::new("adp/default").is_ok());
        assert!(StateId::new("adversarial_verify").is_ok());
        assert!(TaskId::new("AUTH-142").is_ok());
    }

    #[test]
    fn rejects_malformed_identifiers() {
        assert!(PrincipleId::new("Test-Driven").is_err(), "upper case");
        assert!(
            PrincipleId::new("test--driven").is_err(),
            "repeated separator"
        );
        assert!(PrincipleId::new("-test").is_err(), "leading separator");
        assert!(PrincipleId::new("test.driven").is_err(), "dot in kebab id");
        assert!(PrincipleId::new("").is_err(), "empty");
        assert!(TaskId::new("AUTH 142").is_err(), "space");
    }

    #[test]
    fn rejects_numeric_tail_on_dotted_ids_to_keep_version_refs_unambiguous() {
        let err = WorkflowId::new("incident-standard/2").expect_err("numeric tail");
        assert!(err.to_string().contains("version references"), "{err}");
        assert!(WorkflowId::new("adp/default").is_ok());
    }

    #[test]
    fn subject_refs_round_trip() {
        let subject: SubjectRef = "service:auth-api".parse().expect("parses");
        assert_eq!(subject.kind(), "service");
        assert_eq!(subject.id(), "auth-api");
        assert_eq!(subject.to_string(), "service:auth-api");
        assert!(SubjectRef::new("auth-api").is_err(), "missing kind");
    }
}
