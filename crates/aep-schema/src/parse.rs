//! Reading documents.
//!
//! Every document goes through the same two steps, and the error type keeps them apart:
//!
//! ```text
//! text ──serde_yaml──> Raw* ──TryFrom──> domain type
//!         Syntax                Invalid
//! ```
//!
//! The distinction matters to whoever has to fix the document: a syntax error is a typo on one
//! line, while a semantic error is a statement that parses fine and means something impossible.

use std::fmt;

use aep_domain::artifact::ArtifactGraph;
use aep_domain::error::ValidationErrors;
use aep_domain::principle::Principle;
use aep_domain::profile::Profile;
use aep_domain::protocol::Protocol;
use aep_domain::raw::{
    RawArtifactManifest, RawPrinciple, RawProfile, RawProtocol, RawTask, RawWorkflow,
};
use aep_domain::task::Task;
use aep_domain::workflow::Workflow;
use serde::de::DeserializeOwned;

/// Which sort of document is being read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DocumentKind {
    /// A protocol declaration.
    Protocol,
    /// A principle.
    Principle,
    /// A workflow.
    Workflow,
    /// A profile.
    Profile,
    /// A task.
    Task,
    /// An artifact manifest.
    ArtifactManifest,
    /// An artifact lifecycle.
    Lifecycle,
    /// A list of evidence submissions.
    Evidence,
}

impl DocumentKind {
    /// The kind's name, as used in messages and CLI arguments.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Protocol => "protocol",
            Self::Principle => "principle",
            Self::Workflow => "workflow",
            Self::Profile => "profile",
            Self::Task => "task",
            Self::ArtifactManifest => "artifact-manifest",
            Self::Lifecycle => "lifecycle",
            Self::Evidence => "evidence",
        }
    }

    /// Every document kind.
    pub const ALL: &'static [Self] = &[
        Self::Protocol,
        Self::Principle,
        Self::Workflow,
        Self::Profile,
        Self::Task,
        Self::ArtifactManifest,
        Self::Lifecycle,
        Self::Evidence,
    ];

    /// The repository subdirectory this kind is conventionally stored in.
    pub fn directory(self) -> &'static str {
        match self {
            Self::Protocol => "protocols",
            Self::Principle => "principles",
            Self::Workflow => "workflows",
            Self::Profile => "profiles",
            Self::Task => "tasks",
            Self::ArtifactManifest => ".engineering",
            Self::Lifecycle => "artifacts/lifecycles",
            Self::Evidence => "evidence",
        }
    }
}

impl fmt::Display for DocumentKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Why a document could not be read.
#[derive(Debug, thiserror::Error)]
pub enum DocumentError {
    /// The text is not well-formed YAML, or does not match the document's shape.
    #[error("{kind} document{}: {source}", context(origin.as_deref()))]
    Syntax {
        /// Which sort of document was expected.
        kind: DocumentKind,
        /// Where the text came from, when known.
        origin: Option<String>,
        /// The underlying parse error.
        source: serde_yaml::Error,
    },

    /// The document parses but is not semantically valid.
    #[error("{kind} document{} is not valid: {errors}", context(origin.as_deref()))]
    Invalid {
        /// Which sort of document it is.
        kind: DocumentKind,
        /// Where the text came from, when known.
        origin: Option<String>,
        /// What is wrong with it.
        errors: ValidationErrors,
    },
}

/// Renders an optional origin as ` (path)`.
fn context(origin: Option<&str>) -> String {
    match origin {
        Some(origin) => format!(" ({origin})"),
        None => String::new(),
    }
}

impl DocumentError {
    /// The validation errors, when this is a semantic failure.
    pub fn validation_errors(&self) -> Option<&ValidationErrors> {
        match self {
            Self::Invalid { errors, .. } => Some(errors),
            Self::Syntax { .. } => None,
        }
    }

    /// Where the document came from, when known.
    pub fn origin(&self) -> Option<&str> {
        match self {
            Self::Syntax { origin, .. } | Self::Invalid { origin, .. } => origin.as_deref(),
        }
    }
}

/// Reads one document: YAML or JSON text into a raw type, then into its validated counterpart.
///
/// `origin` is used only in error messages; pass the file path when there is one.
pub fn document<Raw, Domain>(
    kind: DocumentKind,
    text: &str,
    origin: Option<&str>,
) -> Result<Domain, DocumentError>
where
    Raw: DeserializeOwned,
    Domain: TryFrom<Raw, Error = ValidationErrors>,
{
    let raw: Raw = serde_yaml::from_str(text).map_err(|source| DocumentError::Syntax {
        kind,
        origin: origin.map(ToOwned::to_owned),
        source,
    })?;
    Domain::try_from(raw).map_err(|errors| DocumentError::Invalid {
        kind,
        origin: origin.map(ToOwned::to_owned),
        errors,
    })
}

/// Reads a protocol document.
pub fn protocol(text: &str, origin: Option<&str>) -> Result<Protocol, DocumentError> {
    document::<RawProtocol, Protocol>(DocumentKind::Protocol, text, origin)
}

/// Reads a principle document.
pub fn principle(text: &str, origin: Option<&str>) -> Result<Principle, DocumentError> {
    document::<RawPrinciple, Principle>(DocumentKind::Principle, text, origin)
}

/// Reads a workflow document.
pub fn workflow(text: &str, origin: Option<&str>) -> Result<Workflow, DocumentError> {
    document::<RawWorkflow, Workflow>(DocumentKind::Workflow, text, origin)
}

/// Reads a profile document.
pub fn profile(text: &str, origin: Option<&str>) -> Result<Profile, DocumentError> {
    document::<RawProfile, Profile>(DocumentKind::Profile, text, origin)
}

/// Reads a task document.
pub fn task(text: &str, origin: Option<&str>) -> Result<Task, DocumentError> {
    document::<RawTask, Task>(DocumentKind::Task, text, origin)
}

/// Reads an artifact manifest.
pub fn artifact_manifest(text: &str, origin: Option<&str>) -> Result<ArtifactGraph, DocumentError> {
    document::<RawArtifactManifest, ArtifactGraph>(DocumentKind::ArtifactManifest, text, origin)
}

/// Reads an artifact lifecycle.
///
/// Lifecycles have no separate raw form: the document *is* the validated shape, because there is
/// nothing to check beyond its structure.
pub fn lifecycle(
    text: &str,
    origin: Option<&str>,
) -> Result<aep_domain::artifact::ArtifactLifecycle, DocumentError> {
    serde_yaml::from_str(text).map_err(|source| DocumentError::Syntax {
        kind: DocumentKind::Lifecycle,
        origin: origin.map(ToOwned::to_owned),
        source,
    })
}

/// One evidence submission, as written in an evidence file.
///
/// The evidence's own fields are flattened, so a file reads as the observation plus who produced it:
///
/// ```yaml
/// - kind: test_result
///   suite: unit
///   passed: 12
///   producer: {producer: verifier, verifier: test-runner}
///   about: task:AUTH-142        # optional
/// ```
///
/// The envelope's subject is spelled `about`, not `subject`: several evidence kinds have a `subject`
/// of their own — a review's subject is the artifact reviewed — and one name for two things would
/// silently take the wrong one.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct EvidenceInput {
    /// The observation.
    #[serde(flatten)]
    pub evidence: aep_domain::evidence::Evidence,
    /// What produced it.
    pub producer: aep_domain::evidence::Producer,
    /// What the envelope is about.
    #[serde(default, alias = "envelope_subject")]
    pub about: Option<aep_domain::ids::SubjectRef>,
    /// How it was obtained.
    #[serde(default)]
    pub provenance: Option<aep_domain::evidence::Provenance>,
}

/// Reads a list of evidence submissions.
pub fn evidence_list(
    text: &str,
    origin: Option<&str>,
) -> Result<Vec<EvidenceInput>, DocumentError> {
    serde_yaml::from_str(text).map_err(|source| DocumentError::Syntax {
        kind: DocumentKind::Evidence,
        origin: origin.map(ToOwned::to_owned),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn separates_syntax_errors_from_semantic_errors() {
        let syntax = workflow("initial: [unclosed", Some("bad.yaml")).expect_err("syntax");
        assert!(matches!(syntax, DocumentError::Syntax { .. }));
        assert!(syntax.to_string().contains("bad.yaml"), "{syntax}");

        let semantic = workflow(
            r"
id: broken
title: Broken
initial: nowhere
states:
  a:
    title: A
    terminal: true
",
            Some("workflows/broken.yaml"),
        )
        .expect_err("semantic");
        assert!(matches!(semantic, DocumentError::Invalid { .. }));
        assert!(semantic
            .validation_errors()
            .expect("semantic failure carries errors")
            .contains(aep_domain::error::ValidationCode::UnknownInitialState));
        assert_eq!(semantic.origin(), Some("workflows/broken.yaml"));
    }

    #[test]
    fn reads_an_evidence_list_and_keeps_a_payloads_own_subject() {
        let inputs = evidence_list(
            r"
- kind: review
  subject: design:passkeys
  reviewer: {reviewer: human, id: ada}
  disposition: approved
  producer: {producer: human, id: ada}
  about: task:AUTH-142
",
            None,
        )
        .expect("parses");
        assert_eq!(inputs.len(), 1);
        assert_eq!(
            inputs[0].about.as_ref().map(ToString::to_string),
            Some("task:AUTH-142".to_owned())
        );
        let aep_domain::evidence::Evidence::Review(review) = &inputs[0].evidence else {
            panic!("expected a review");
        };
        assert_eq!(
            review.subject.to_string(),
            "design:passkeys",
            "the payload's own subject must not be taken by the envelope"
        );
    }

    #[test]
    fn reads_json_as_well_as_yaml() {
        let parsed = protocol(
            r#"{"id": "aep", "title": "AEP", "observables": ["task.**"]}"#,
            None,
        )
        .expect("json is a subset of yaml");
        assert_eq!(parsed.id.as_str(), "aep");
    }

    #[test]
    fn document_kinds_map_to_repository_directories() {
        assert_eq!(DocumentKind::Principle.directory(), "principles");
        assert_eq!(DocumentKind::Lifecycle.directory(), "artifacts/lifecycles");
        assert_eq!(DocumentKind::ALL.len(), 8);
    }
}
