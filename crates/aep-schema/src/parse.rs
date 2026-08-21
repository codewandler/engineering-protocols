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
//!
//! Reading the text is itself two steps, for the reason `read_yaml` gives: a mapping key written
//! twice is refused before anything is deserialized, because otherwise it is not refused at all.

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
use aep_driver_spec::map::{RawStepMap, StepMap};
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
    /// A project's own configuration.
    Project,
    /// A driver's step map: what a harness does in each state of a workflow.
    StepMap,
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
            Self::Project => "project",
            Self::StepMap => "step-map",
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
        Self::Project,
        Self::StepMap,
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
            Self::Project => "project",
            // Beside `workflows/`, `principles/` and `profiles/`: a step map is a validated,
            // versioned, schema-generated document exactly like the four already in the tree, and
            // anywhere else would be a fifth kind loaded by a second mechanism.
            Self::StepMap => "drivers",
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

/// Reads YAML or JSON text, refusing a mapping key that is written twice.
///
/// The first parse is the guard, and it is the whole reason there are two. Deserializing straight
/// into the target type lets `serde_yaml` **silently keep the last of two identical keys**: a
/// profile that writes `capabilities:` twice keeps one block and drops the other, so the profile
/// that grants capabilities grants a different set than the file appears to, and nothing anywhere
/// says so. Parsing to [`serde_yaml::Value`] first refuses it, naming the key and the line.
///
/// This is a **syntax** failure, not a semantic one, on the definition this module's own header
/// gives: a duplicated key is a defect in the text that can be seen without knowing what the
/// document means, and there is no well-defined mapping for a validator to disagree with. It is
/// also the only classification available to every kind alike — [`lifecycle`] and [`evidence_list`]
/// have no raw stage and so can only fail as [`DocumentError::Syntax`] — and one class of defect
/// that reports differently depending on the document kind is worse than either choice.
///
/// The typed parse then reads the *text* again rather than the parsed `Value`. `serde_yaml::from_value`
/// carries no spans, so staging through it would buy the duplicate-key diagnostic at the price of
/// the line and column on every shape error, and pointing at one line is what a `Syntax` error is
/// for. Parsing twice costs nothing at document sizes.
///
/// The same guard is written once more, in `ess_domain::spec::RawSpecFile::parse`. The two cannot
/// share one implementation today: `aep-schema` depends on `ess-domain`, not the other way round,
/// and `aep-domain` — the crate both depend on — deliberately holds no serialization format.
fn read_yaml<T: DeserializeOwned>(text: &str) -> Result<T, serde_yaml::Error> {
    let _: serde_yaml::Value = serde_yaml::from_str(text)?;
    serde_yaml::from_str(text)
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
    let raw: Raw = read_yaml(text).map_err(|source| DocumentError::Syntax {
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

/// Reads a project's own configuration.
pub fn project(
    text: &str,
    origin: Option<&str>,
) -> Result<aep_domain::project::ProjectConfig, DocumentError> {
    document::<aep_domain::project::RawProjectConfig, aep_domain::project::ProjectConfig>(
        DocumentKind::Project,
        text,
        origin,
    )
}

/// Reads a driver's step map.
///
/// Structural validation only: the format version, the mandatory workflow pin and each step. The
/// two cross-document phases are elsewhere and cannot be here — the first needs the registry the
/// document tree is still being read into, and the second needs the protocol the **task** names,
/// which no document loader has seen.
pub fn step_map(text: &str, origin: Option<&str>) -> Result<StepMap, DocumentError> {
    document::<RawStepMap, StepMap>(DocumentKind::StepMap, text, origin)
}

/// Reads an artifact lifecycle.
///
/// Lifecycles have no separate raw form: the document *is* the validated shape, because there is
/// nothing to check beyond its structure.
pub fn lifecycle(
    text: &str,
    origin: Option<&str>,
) -> Result<aep_domain::artifact::ArtifactLifecycle, DocumentError> {
    read_yaml(text).map_err(|source| DocumentError::Syntax {
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
///   observed_at: 2026-08-30      # when the suite was run, not when this file was written
///   producer: {producer: verifier, verifier: test-runner}
///   about: task:AUTH-142         # optional
/// ```
///
/// The envelope's subject is spelled `about`, not `subject`: several evidence kinds have a `subject`
/// of their own — a review's subject is the artifact reviewed — and one name for two things would
/// silently take the wrong one.
///
/// `observed_at` is **required**, and it is the one field in this shape with no reasonable default.
/// Inferring it from when the document was read is the single-field convention that classifies a
/// three-week-old reading as this morning's; a date a person has to write is a date a person has to
/// know. It accepts either spelling
/// [`ObservedAt`](aep_domain::time::ObservedAt) accepts — a calendar date, or epoch milliseconds.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct EvidenceInput {
    /// The observation.
    #[serde(flatten)]
    pub evidence: aep_domain::evidence::Evidence,
    /// When the observation was made.
    pub observed_at: aep_domain::time::ObservedAt,
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
    read_yaml(text).map_err(|source| DocumentError::Syntax {
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
  observed_at: 2026-08-30
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
        assert_eq!(
            inputs[0].observed_at,
            aep_domain::time::ObservedAt::new(
                aep_domain::time::CivilDate::parse("2026-08-30")
                    .expect("a date")
                    .to_timestamp()
            ),
            "an observation time written as a date reaches the submission as an instant"
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
    fn a_profile_that_writes_one_key_twice_is_refused_with_the_key_named() {
        // A profile is the document that *grants* capabilities. Keeping the last of two
        // `capabilities:` blocks means this file grants `production.write` and denies nothing that
        // it appears to deny — a silent widening of what an agent may do, from a typo.
        let error = profile(
            r"
id: test.standard
title: Test standard
protocol: aep/1
workflow: test/linear
capabilities:
  allow: [repository.read]
capabilities:
  allow: [production.write]
",
            Some("profiles/test.yaml"),
        )
        .expect_err("the key is written twice");
        assert!(
            matches!(error, DocumentError::Syntax { .. }),
            "a key written twice is a defect in the text, not a statement that means something \
             impossible: {error}"
        );
        let rendered = error.to_string();
        assert!(
            rendered.contains("capabilities"),
            "the diagnostic has to name the key: {rendered}"
        );
        assert!(
            rendered.contains("duplicate"),
            "the reader has to be told which fault this is: {rendered}"
        );
        assert_eq!(error.origin(), Some("profiles/test.yaml"));
    }

    #[test]
    fn a_duplicated_key_is_refused_in_a_document_with_no_raw_stage_too() {
        // `lifecycle` and `evidence_list` deserialize straight into their final shape, so `Syntax`
        // is the only failure they can report. That is the argument for classifying a duplicated
        // key as syntax everywhere rather than only where a raw stage happens to exist.
        let error = lifecycle(
            r"
kind: design
initial: draft
transitions:
  draft: [in_review]
  draft: [archived]
",
            None,
        )
        .expect_err("the state is written twice");
        let rendered = error.to_string();
        assert!(rendered.contains("draft"), "{rendered}");
        assert!(rendered.contains("duplicate"), "{rendered}");

        let error = evidence_list(
            r"
- kind: review
  subject: design:passkeys
  subject: design:something-else
  reviewer: {reviewer: human, id: ada}
  disposition: approved
  producer: {producer: human, id: ada}
",
            None,
        )
        .expect_err("the key is written twice");
        let rendered = error.to_string();
        assert!(rendered.contains("subject"), "{rendered}");
        assert!(rendered.contains("duplicate"), "{rendered}");
    }

    #[test]
    fn a_shape_error_still_says_which_line_it_is_on() {
        // The duplicate-key guard parses the text twice rather than deserializing from the parsed
        // `serde_yaml::Value`, because a `Value` carries no spans. If that ever changes, this is
        // the test that notices: a syntax error whose whole job is to point at one line stops
        // pointing at anything.
        let error = profile(
            r"
id: test.standard
title: [not, a, string]
protocol: aep/1
workflow: test/linear
",
            None,
        )
        .expect_err("a title is not a list");
        let rendered = error.to_string();
        assert!(
            rendered.contains("line 3"),
            "a shape error must keep its location: {rendered}"
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
        // Beside the four document kinds the tree already held, because a step map is a validated,
        // versioned, schema-generated document exactly like them.
        assert_eq!(DocumentKind::StepMap.directory(), "drivers");
        assert_eq!(DocumentKind::ALL.len(), 10);
    }
}
