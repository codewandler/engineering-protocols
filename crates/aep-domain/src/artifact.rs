//! The engineering artifact graph.
//!
//! Work does not only move through workflow states; it also creates and consumes durable
//! artifacts — specifications, designs, ADRs, runbooks, postmortems — and the *relationships*
//! between them are what carry intent from "why" to "what changed".
//!
//! # What this buys a reader
//!
//! Someone picking up `AUTH-142` six months later can ask which specification governs it,
//! which design satisfies that specification, whether the approval on that design was given
//! against the revision that shipped, and which ADR recorded the decision — and get answers
//! from typed data rather than from archaeology in a chat log.
//!
//! # Location is not identity
//!
//! An artifact's [`ArtifactLocation`] is metadata. A design in `docs/designs/`, a PRD in
//! Linear and an architecture description generated from source are all the same kind of
//! thing to the protocol; only the graph is normative. This is why AEP can be adopted without
//! moving anybody's documents.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;
use std::str::FromStr;

use crate::error::{ParseError, ValidationCode, ValidationError, ValidationErrors};
use crate::evidence::Producer;
use crate::facts::{FactPath, FactStore, FactValue};
use crate::ids::{ProviderId, RepositoryRef};
use crate::node::Node;
use crate::time::Timestamp;

/// Identifier of an artifact, written `<namespace>:<name>`, such as `design:passkeys-auth`.
///
/// The namespace is a convention for humans reading the manifest; the artifact's [`ArtifactKind`]
/// is what the protocol reasons about, so `doc:passkeys` with `kind: design` is equally valid.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
#[serde(into = "String")]
pub struct ArtifactId {
    namespace: String,
    name: String,
}

impl ArtifactId {
    /// Parses an artifact id.
    pub fn new(value: impl AsRef<str>) -> Result<Self, ParseError> {
        let value = value.as_ref();
        let Some((namespace, name)) = value.split_once(':') else {
            return Err(ParseError::identifier(
                "artifact",
                value,
                "must be written `<namespace>:<name>`, for example `design:passkeys-auth`"
                    .to_owned(),
            ));
        };
        if namespace.is_empty() || name.is_empty() {
            return Err(ParseError::identifier(
                "artifact",
                value,
                "both the namespace and the name must be non-empty".to_owned(),
            ));
        }
        for (part, label) in [(namespace, "namespace"), (name, "name")] {
            for ch in part.chars() {
                if !(ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | '/')) {
                    return Err(ParseError::identifier(
                        "artifact",
                        value,
                        format!("{label} contains disallowed character {ch:?}"),
                    ));
                }
            }
        }
        Ok(Self {
            namespace: namespace.to_owned(),
            name: name.to_owned(),
        })
    }

    /// The namespace, such as `design`.
    pub fn namespace(&self) -> &str {
        &self.namespace
    }

    /// The name, such as `passkeys-auth`.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The pattern published in generated JSON Schema.
    pub const PATTERN: &'static str = "^[A-Za-z0-9][A-Za-z0-9._/-]*:[A-Za-z0-9][A-Za-z0-9._/-]*$";
}

impl fmt::Display for ArtifactId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.namespace, self.name)
    }
}

impl fmt::Debug for ArtifactId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ArtifactId({self})")
    }
}

impl FromStr for ArtifactId {
    type Err = ParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

impl From<ArtifactId> for String {
    fn from(value: ArtifactId) -> Self {
        value.to_string()
    }
}

impl<'de> serde::Deserialize<'de> for ArtifactId {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        Self::new(raw).map_err(serde::de::Error::custom)
    }
}

impl schemars::JsonSchema for ArtifactId {
    fn schema_name() -> String {
        "ArtifactId".to_owned()
    }

    fn json_schema(_: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        let mut schema = schemars::schema::SchemaObject {
            instance_type: Some(schemars::schema::InstanceType::String.into()),
            ..Default::default()
        };
        schema.string().pattern = Some(Self::PATTERN.to_owned());
        schema.metadata().description =
            Some("Artifact identifier, written `<namespace>:<name>`.".to_owned());
        schema.into()
    }
}

/// A label for one version of an artifact, such as `3` or a content digest.
#[derive(
    Debug,
    Clone,
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
pub struct ArtifactVersion(String);

impl ArtifactVersion {
    /// Builds an artifact version label.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// The label.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ArtifactVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A source-control revision, such as a commit SHA.
#[derive(
    Debug,
    Clone,
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
pub struct Revision(String);

impl Revision {
    /// Builds a revision.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// The revision string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Revision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A reference to an artifact, optionally pinned to one of its versions.
///
/// Written `design:passkeys-auth` or `design:passkeys-auth@3`. Pinning matters for review
/// freshness: an approval of version 3 must not silently approve version 7.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
#[serde(into = "String")]
pub struct ArtifactRef {
    id: ArtifactId,
    version: Option<ArtifactVersion>,
}

impl ArtifactRef {
    /// Builds a reference.
    pub fn new(id: ArtifactId, version: Option<ArtifactVersion>) -> Self {
        Self { id, version }
    }

    /// Builds an unpinned reference.
    pub fn unpinned(id: ArtifactId) -> Self {
        Self { id, version: None }
    }

    /// Parses `<id>` or `<id>@<version>`.
    pub fn parse(value: &str) -> Result<Self, ParseError> {
        match value.split_once('@') {
            Some((id, version)) => Ok(Self {
                id: ArtifactId::new(id)?,
                version: Some(ArtifactVersion::new(version)),
            }),
            None => Ok(Self::unpinned(ArtifactId::new(value)?)),
        }
    }

    /// The referenced artifact.
    pub fn id(&self) -> &ArtifactId {
        &self.id
    }

    /// The pinned version, if any.
    pub fn version(&self) -> Option<&ArtifactVersion> {
        self.version.as_ref()
    }

    /// The pattern published in generated JSON Schema.
    pub const PATTERN: &'static str =
        "^[A-Za-z0-9][A-Za-z0-9._/-]*:[A-Za-z0-9][A-Za-z0-9._/-]*(@[^@]+)?$";
}

impl fmt::Display for ArtifactRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.version {
            Some(version) => write!(f, "{}@{version}", self.id),
            None => write!(f, "{}", self.id),
        }
    }
}

impl fmt::Debug for ArtifactRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ArtifactRef({self})")
    }
}

impl FromStr for ArtifactRef {
    type Err = ParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

impl From<ArtifactRef> for String {
    fn from(value: ArtifactRef) -> Self {
        value.to_string()
    }
}

impl From<ArtifactId> for ArtifactRef {
    fn from(id: ArtifactId) -> Self {
        Self::unpinned(id)
    }
}

impl<'de> serde::Deserialize<'de> for ArtifactRef {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        Self::parse(&raw).map_err(serde::de::Error::custom)
    }
}

impl schemars::JsonSchema for ArtifactRef {
    fn schema_name() -> String {
        "ArtifactRef".to_owned()
    }

    fn json_schema(_: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        let mut schema = schemars::schema::SchemaObject {
            instance_type: Some(schemars::schema::InstanceType::String.into()),
            ..Default::default()
        };
        schema.string().pattern = Some(Self::PATTERN.to_owned());
        schema.metadata().description =
            Some("Reference to an artifact, optionally pinned with `@<version>`.".to_owned());
        schema.into()
    }
}

/// What kind of artifact this is.
///
/// The taxonomy is deliberately shallow and extensible: [`ArtifactKind::Other`] carries a kind
/// this vocabulary does not name, because no fixed ontology survives contact with a second
/// organisation.
///
/// Design kinds form a hierarchy — a requirement for a `design` is satisfied by an
/// `architecture-design` — see [`ArtifactKind::is_a`].
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum ArtifactKind {
    /// Long-range direction.
    Vision,
    /// What outcome should exist, for whom.
    ProductRequirements,
    /// A large body of work above an epic.
    Initiative,
    /// A major deliverable.
    Epic,
    /// An independently meaningful change.
    Story,
    /// A concrete unit of work.
    Task,
    /// Precisely what behaviour and constraints the implementation must satisfy.
    Specification,
    /// The conditions under which work is accepted.
    AcceptanceCriteria,
    /// A proposed solution.
    Design,
    /// A design scoped to a feature.
    FeatureDesign,
    /// A design scoped to a component.
    ComponentDesign,
    /// A design spanning systems, with stronger governance.
    ArchitectureDesign,
    /// A design of an interface.
    ApiDesign,
    /// A design of storage or data flow.
    DataDesign,
    /// A durable decision with its context and consequences.
    ArchitectureDecisionRecord,
    /// How the work will be tested.
    TestPlan,
    /// How the work will be evaluated where testing is not enough.
    EvaluationPlan,
    /// The outcome of verification.
    VerificationReport,
    /// The outcome of a review.
    ReviewResult,
    /// A recorded human approval.
    ApprovalRecord,
    /// How a change reaches production.
    ReleasePlan,
    /// How data or systems are migrated.
    MigrationPlan,
    /// Operational instructions.
    Runbook,
    /// What happened during an incident.
    IncidentReport,
    /// What was learned from an incident.
    Postmortem,
    /// A kind this vocabulary does not name.
    Other(String),
}

impl ArtifactKind {
    /// Every named kind, for vocabulary listing and schema examples.
    pub const NAMED: &'static [Self] = &[
        Self::Vision,
        Self::ProductRequirements,
        Self::Initiative,
        Self::Epic,
        Self::Story,
        Self::Task,
        Self::Specification,
        Self::AcceptanceCriteria,
        Self::Design,
        Self::FeatureDesign,
        Self::ComponentDesign,
        Self::ArchitectureDesign,
        Self::ApiDesign,
        Self::DataDesign,
        Self::ArchitectureDecisionRecord,
        Self::TestPlan,
        Self::EvaluationPlan,
        Self::VerificationReport,
        Self::ReviewResult,
        Self::ApprovalRecord,
        Self::ReleasePlan,
        Self::MigrationPlan,
        Self::Runbook,
        Self::IncidentReport,
        Self::Postmortem,
    ];

    /// The kind as written in documents, in kebab-case.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Vision => "vision",
            Self::ProductRequirements => "product-requirements",
            Self::Initiative => "initiative",
            Self::Epic => "epic",
            Self::Story => "story",
            Self::Task => "task",
            Self::Specification => "specification",
            Self::AcceptanceCriteria => "acceptance-criteria",
            Self::Design => "design",
            Self::FeatureDesign => "feature-design",
            Self::ComponentDesign => "component-design",
            Self::ArchitectureDesign => "architecture-design",
            Self::ApiDesign => "api-design",
            Self::DataDesign => "data-design",
            Self::ArchitectureDecisionRecord => "architecture-decision-record",
            Self::TestPlan => "test-plan",
            Self::EvaluationPlan => "evaluation-plan",
            Self::VerificationReport => "verification-report",
            Self::ReviewResult => "review-result",
            Self::ApprovalRecord => "approval-record",
            Self::ReleasePlan => "release-plan",
            Self::MigrationPlan => "migration-plan",
            Self::Runbook => "runbook",
            Self::IncidentReport => "incident-report",
            Self::Postmortem => "postmortem",
            Self::Other(name) => name,
        }
    }

    /// Parses a kind name, accepting `adr` and `prd` as aliases.
    pub fn parse(value: &str) -> Result<Self, ParseError> {
        if let Some(known) = Self::NAMED.iter().find(|kind| kind.as_str() == value) {
            return Ok(known.clone());
        }
        Ok(match value {
            "adr" => Self::ArchitectureDecisionRecord,
            "prd" => Self::ProductRequirements,
            "spec" => Self::Specification,
            "review" => Self::ReviewResult,
            other => {
                let named = crate::ids::PrincipleId::new(other).map_err(|_| {
                    ParseError::identifier(
                        "artifact kind",
                        other,
                        "artifact kinds are lower-case kebab-case, such as `architecture-design`"
                            .to_owned(),
                    )
                })?;
                Self::Other(named.as_str().to_owned())
            }
        })
    }

    /// The kind this one specialises, if any.
    pub fn parent(&self) -> Option<Self> {
        match self {
            Self::FeatureDesign
            | Self::ComponentDesign
            | Self::ArchitectureDesign
            | Self::ApiDesign
            | Self::DataDesign => Some(Self::Design),
            _ => None,
        }
    }

    /// `true` when this kind satisfies a requirement for `other`.
    ///
    /// Reflexive, and follows the design hierarchy upwards: an `architecture-design` *is a*
    /// `design`, so a principle requiring a design is satisfied by one.
    pub fn is_a(&self, other: &Self) -> bool {
        let mut current = Some(self.clone());
        while let Some(kind) = current {
            if &kind == other {
                return true;
            }
            current = kind.parent();
        }
        false
    }

    /// This kind and every kind it specialises, nearest first.
    pub fn lineage(&self) -> Vec<Self> {
        let mut lineage = vec![self.clone()];
        let mut current = self.parent();
        while let Some(kind) = current {
            current = kind.parent();
            lineage.push(kind);
        }
        lineage
    }

    /// `true` when this kind carries architectural weight, and therefore stronger governance.
    pub fn is_architectural(&self) -> bool {
        matches!(
            self,
            Self::ArchitectureDesign | Self::ArchitectureDecisionRecord
        )
    }

    /// `true` when this kind belongs to intent decomposition rather than to engineering output.
    ///
    /// AEP models these but does not own them: they usually live in a planning system.
    pub fn is_planning(&self) -> bool {
        matches!(
            self,
            Self::Vision
                | Self::ProductRequirements
                | Self::Initiative
                | Self::Epic
                | Self::Story
                | Self::Task
        )
    }
}

impl fmt::Display for ArtifactKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for ArtifactKind {
    type Err = ParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

impl serde::Serialize for ArtifactKind {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> serde::Deserialize<'de> for ArtifactKind {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        Self::parse(&raw).map_err(serde::de::Error::custom)
    }
}

impl schemars::JsonSchema for ArtifactKind {
    fn schema_name() -> String {
        "ArtifactKind".to_owned()
    }

    fn json_schema(_: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        let mut schema = schemars::schema::SchemaObject {
            instance_type: Some(schemars::schema::InstanceType::String.into()),
            ..Default::default()
        };
        schema.string().pattern = Some("^[a-z][a-z0-9-]*$".to_owned());
        schema.metadata().description = Some(
            "Artifact kind in kebab-case; the named vocabulary is listed in `examples`, and any \
             other kebab-case name is accepted as an organisation-specific kind."
                .to_owned(),
        );
        schema.metadata().examples = ArtifactKind::NAMED
            .iter()
            .map(|kind| serde_json::Value::String(kind.as_str().to_owned()))
            .collect();
        schema.into()
    }
}

/// Where an artifact's lifecycle has got to.
///
/// This is independent of workflow state: a design can be `approved` while the task that
/// consumes it is still in `implement`.
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
pub enum ArtifactStatus {
    /// Being written.
    Draft,
    /// Put forward for a decision.
    Proposed,
    /// Under review.
    InReview,
    /// Reviewed and agreed.
    Approved,
    /// Decided and in force; the ADR spelling of `approved`.
    Accepted,
    /// Reviewed and turned down.
    Rejected,
    /// Current and in use.
    Active,
    /// Realised in the system.
    Implemented,
    /// Replaced by a later artifact.
    Superseded,
    /// Kept for the record only.
    Archived,
}

impl ArtifactStatus {
    /// Every status.
    pub const ALL: &'static [Self] = &[
        Self::Draft,
        Self::Proposed,
        Self::InReview,
        Self::Approved,
        Self::Accepted,
        Self::Rejected,
        Self::Active,
        Self::Implemented,
        Self::Superseded,
        Self::Archived,
    ];

    /// The status as written in documents.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Proposed => "proposed",
            Self::InReview => "in_review",
            Self::Approved => "approved",
            Self::Accepted => "accepted",
            Self::Rejected => "rejected",
            Self::Active => "active",
            Self::Implemented => "implemented",
            Self::Superseded => "superseded",
            Self::Archived => "archived",
        }
    }

    /// `true` when the artifact has been agreed and may be relied on.
    ///
    /// Requirements written as `status: approved` accept any of these, because
    /// `accepted`/`active`/`implemented` are all downstream of approval.
    pub fn is_approved(self) -> bool {
        matches!(
            self,
            Self::Approved | Self::Accepted | Self::Active | Self::Implemented
        )
    }

    /// `true` when the artifact no longer describes the current state of the world.
    pub fn is_retired(self) -> bool {
        matches!(self, Self::Superseded | Self::Archived | Self::Rejected)
    }

    /// `true` when an artifact in this status satisfies a requirement for `required`.
    pub fn satisfies(self, required: Self) -> bool {
        if self == required {
            return true;
        }
        required == Self::Approved && self.is_approved()
    }
}

impl fmt::Display for ArtifactStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Where an artifact lives.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ArtifactLocation {
    /// A path inside a repository.
    RepositoryPath {
        /// The repository, when the artifact lives outside the current one.
        repository: Option<RepositoryRef>,
        /// The path, relative to the repository root.
        path: String,
    },
    /// A URL.
    Url(String),
    /// An object in an external system, resolved by a connector rather than by AEP.
    External {
        /// The system holding it, such as `linear`.
        provider: ProviderId,
        /// Its identifier in that system.
        reference: String,
    },
    /// Carried in the manifest itself, with no external body.
    Inline,
}

impl ArtifactLocation {
    /// Parses a location from document form.
    ///
    /// Accepts `inline`, a bare repository path, a URL string, or a mapping with `path`
    /// (optionally `repository`), `url`, or `provider` and `reference`.
    pub fn from_node(node: &Node) -> Result<Self, ParseError> {
        match node {
            Node::Text(text) if text == "inline" => Ok(Self::Inline),
            Node::Text(text) if text.starts_with("http://") || text.starts_with("https://") => {
                Ok(Self::Url(text.clone()))
            }
            Node::Text(text) => Ok(Self::RepositoryPath {
                repository: None,
                path: text.clone(),
            }),
            Node::Map(entries) => {
                if let Some(url) = entries.get("url").and_then(Node::as_text) {
                    return Ok(Self::Url(url.to_owned()));
                }
                if let Some(path) = entries.get("path").and_then(Node::as_text) {
                    let repository = match entries.get("repository").and_then(Node::as_text) {
                        Some(repository) => Some(RepositoryRef::new(repository)?),
                        None => None,
                    };
                    return Ok(Self::RepositoryPath {
                        repository,
                        path: path.to_owned(),
                    });
                }
                match (
                    entries.get("provider").and_then(Node::as_text),
                    entries.get("reference").and_then(Node::as_text),
                ) {
                    (Some(provider), Some(reference)) => Ok(Self::External {
                        provider: ProviderId::new(provider)?,
                        reference: reference.to_owned(),
                    }),
                    (Some(_), None) => Err(ParseError::shape(
                        "artifact.location",
                        "`reference` alongside `provider`",
                        "only `provider`",
                    )),
                    _ => Err(ParseError::shape(
                        "artifact.location",
                        "one of `path`, `url`, or `provider` with `reference`",
                        format!("keys {:?}", entries.keys().collect::<Vec<_>>()),
                    )),
                }
            }
            other => Err(ParseError::shape(
                "artifact.location",
                "a string or a mapping",
                other.type_name(),
            )),
        }
    }

    /// Renders this location back into document form.
    pub fn to_node(&self) -> Node {
        match self {
            Self::Inline => Node::Text("inline".to_owned()),
            Self::Url(url) => Node::Map([("url".to_owned(), Node::Text(url.clone()))].into()),
            Self::RepositoryPath { repository, path } => {
                let mut entries = BTreeMap::new();
                entries.insert("path".to_owned(), Node::Text(path.clone()));
                if let Some(repository) = repository {
                    entries.insert(
                        "repository".to_owned(),
                        Node::Text(repository.as_str().to_owned()),
                    );
                }
                Node::Map(entries)
            }
            Self::External {
                provider,
                reference,
            } => Node::Map(
                [
                    (
                        "provider".to_owned(),
                        Node::Text(provider.as_str().to_owned()),
                    ),
                    ("reference".to_owned(), Node::Text(reference.clone())),
                ]
                .into(),
            ),
        }
    }

    /// The repository path, when the artifact is a file in a repository.
    ///
    /// This is the only case a local validator can open, which is why it is called out.
    pub fn local_path(&self) -> Option<&str> {
        match self {
            Self::RepositoryPath {
                repository: None,
                path,
            } => Some(path),
            _ => None,
        }
    }
}

impl fmt::Display for ArtifactLocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Inline => f.write_str("inline"),
            Self::Url(url) => f.write_str(url),
            Self::RepositoryPath {
                repository: Some(repository),
                path,
            } => write!(f, "{repository}:{path}"),
            Self::RepositoryPath {
                repository: None,
                path,
            } => f.write_str(path),
            Self::External {
                provider,
                reference,
            } => write!(f, "{provider}/{reference}"),
        }
    }
}

impl serde::Serialize for ArtifactLocation {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.to_node().serialize(serializer)
    }
}

impl<'de> serde::Deserialize<'de> for ArtifactLocation {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let node = Node::deserialize(deserializer)?;
        Self::from_node(&node).map_err(serde::de::Error::custom)
    }
}

impl schemars::JsonSchema for ArtifactLocation {
    fn schema_name() -> String {
        "ArtifactLocation".to_owned()
    }

    fn json_schema(generator: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        let mut schema = schemars::schema::SchemaObject::default();
        schema.subschemas().any_of = Some(vec![
            <String>::json_schema(generator),
            <BTreeMap<String, String>>::json_schema(generator),
        ]);
        schema.metadata().description = Some(
            "Where the artifact lives: `inline`, a repository path, a URL, or `{provider, \
             reference}` for an object in an external system."
                .to_owned(),
        );
        schema.into()
    }
}

/// The meaning of an edge in the artifact graph.
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
pub enum RelationKind {
    /// Shaped by, without being derived from.
    InformedBy,
    /// Produced from a higher-level artifact.
    DerivedFrom,
    /// Breaks a larger artifact into smaller work.
    Decomposes,
    /// States the required behaviour of something.
    Specifies,
    /// Proposes how to satisfy something.
    Designs,
    /// Realises something in the system.
    Implements,
    /// Records a decision taken within something.
    Decides,
    /// Assesses something.
    Reviews,
    /// Establishes that something holds.
    Verifies,
    /// Prevents progress on something.
    Blocks,
    /// Needs something else first.
    DependsOn,
    /// Replaces something.
    Supersedes,
}

impl RelationKind {
    /// Every relation kind.
    pub const ALL: &'static [Self] = &[
        Self::InformedBy,
        Self::DerivedFrom,
        Self::Decomposes,
        Self::Specifies,
        Self::Designs,
        Self::Implements,
        Self::Decides,
        Self::Reviews,
        Self::Verifies,
        Self::Blocks,
        Self::DependsOn,
        Self::Supersedes,
    ];

    /// The relation as written in documents.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InformedBy => "informed_by",
            Self::DerivedFrom => "derived_from",
            Self::Decomposes => "decomposes",
            Self::Specifies => "specifies",
            Self::Designs => "designs",
            Self::Implements => "implements",
            Self::Decides => "decides",
            Self::Reviews => "reviews",
            Self::Verifies => "verifies",
            Self::Blocks => "blocks",
            Self::DependsOn => "depends_on",
            Self::Supersedes => "supersedes",
        }
    }

    /// Parses a relation name.
    pub fn parse(value: &str) -> Result<Self, ParseError> {
        Self::ALL
            .iter()
            .copied()
            .find(|relation| relation.as_str() == value)
            .ok_or_else(|| {
                ParseError::identifier(
                    "artifact relation",
                    value,
                    format!(
                        "expected one of {}",
                        Self::ALL
                            .iter()
                            .map(|relation| relation.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                )
            })
    }

    /// How the edge reads in the other direction, for explanations.
    pub fn inverse_label(self) -> &'static str {
        match self {
            Self::InformedBy => "informs",
            Self::DerivedFrom => "derived into",
            Self::Decomposes => "decomposed by",
            Self::Specifies => "specified by",
            Self::Designs => "designed by",
            Self::Implements => "implemented by",
            Self::Decides => "decided by",
            Self::Reviews => "reviewed by",
            Self::Verifies => "verified by",
            Self::Blocks => "blocked by",
            Self::DependsOn => "depended on by",
            Self::Supersedes => "superseded by",
        }
    }
}

impl fmt::Display for RelationKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One edge of the artifact graph.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ArtifactRelation {
    /// What the edge means.
    pub kind: RelationKind,
    /// What it points at.
    pub target: ArtifactRef,
}

impl ArtifactRelation {
    /// Builds a relation.
    pub fn new(kind: RelationKind, target: ArtifactRef) -> Self {
        Self { kind, target }
    }

    /// Parses the document form `{<relation>: <artifact-ref>}`.
    pub fn from_node(node: &Node) -> Result<Self, ParseError> {
        let Some((key, value)) = node.as_single_entry() else {
            return Err(ParseError::shape(
                "artifact.relations[]",
                "a single-entry mapping such as `{designs: spec:passkeys-auth}`",
                node.type_name(),
            ));
        };
        let Some(target) = value.as_text() else {
            return Err(ParseError::shape(
                format!("artifact.relations[].{key}"),
                "an artifact reference",
                value.type_name(),
            ));
        };
        Ok(Self {
            kind: RelationKind::parse(key)?,
            target: ArtifactRef::parse(target)?,
        })
    }

    /// Renders this relation back into document form.
    pub fn to_node(&self) -> Node {
        Node::Map(
            [(
                self.kind.as_str().to_owned(),
                Node::Text(self.target.to_string()),
            )]
            .into(),
        )
    }
}

impl fmt::Display for ArtifactRelation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.kind, self.target)
    }
}

impl serde::Serialize for ArtifactRelation {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.to_node().serialize(serializer)
    }
}

impl<'de> serde::Deserialize<'de> for ArtifactRelation {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let node = Node::deserialize(deserializer)?;
        Self::from_node(&node).map_err(serde::de::Error::custom)
    }
}

impl schemars::JsonSchema for ArtifactRelation {
    fn schema_name() -> String {
        "ArtifactRelation".to_owned()
    }

    fn json_schema(_: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        let mut schema = schemars::schema::SchemaObject {
            instance_type: Some(schemars::schema::InstanceType::Object.into()),
            ..Default::default()
        };
        schema.object().max_properties = Some(1);
        schema.object().min_properties = Some(1);
        schema.metadata().description = Some(
            "A single-entry mapping from a relation name to an artifact reference, such as \
             `{designs: spec:passkeys-auth}`."
                .to_owned(),
        );
        schema.metadata().examples = RelationKind::ALL
            .iter()
            .map(|relation| serde_json::json!({ relation.as_str(): "design:passkeys-auth" }))
            .collect();
        schema.into()
    }
}

/// How an artifact came to exist.
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
pub enum ArtifactSource {
    /// Written by a person.
    HumanAuthored,
    /// Written by an agent.
    AgentAuthored,
    /// Produced by a tool from something else.
    ToolGenerated,
    /// Derived from another artifact.
    Derived,
    /// Brought in from another system.
    Imported,
}

/// Who made an artifact, when, and from what.
///
/// Authorship is recorded, not scored: an agent-authored design is not thereby less
/// trustworthy than a human-authored one. What it changes is what evidence a protocol may
/// reasonably ask for.
#[derive(
    Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct ArtifactProvenance {
    /// Who created it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_by: Option<Producer>,
    /// When it was created.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<Timestamp>,
    /// How it came to exist.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<ArtifactSource>,
    /// The source revision it was generated from, for generated artifacts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision: Option<Revision>,
}

/// How long an artifact stays valid.
#[derive(
    Debug,
    Clone,
    Copy,
    Default,
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
pub enum FreshnessPolicy {
    /// Never goes stale.
    AlwaysValid,
    /// Valid until something supersedes it. The default.
    #[default]
    UntilSuperseded,
    /// Valid only for the revision it was approved against.
    BoundToRevision,
    /// Valid only while its dependencies are unchanged.
    BoundToDependencySet,
}

/// Human-facing description of an artifact.
#[derive(
    Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct ArtifactMetadata {
    /// Its title.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// One-line summary.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    /// Who owns it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    /// Free-form labels.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub tags: BTreeSet<String>,
    /// Anything else the organisation wants to carry.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty", flatten)]
    pub extra: BTreeMap<String, Node>,
}

/// A durable engineering artifact.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct Artifact {
    /// Its identifier.
    pub id: ArtifactId,
    /// What kind of artifact it is.
    pub kind: ArtifactKind,
    /// Which version this record describes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<ArtifactVersion>,
    /// Where its lifecycle has got to.
    pub status: ArtifactStatus,
    /// Where it lives.
    pub location: ArtifactLocation,
    /// Its outgoing edges.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub relations: Vec<ArtifactRelation>,
    /// Human-facing description.
    #[serde(default, skip_serializing_if = "is_default_metadata")]
    pub metadata: ArtifactMetadata,
    /// Who made it and from what.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance: Option<ArtifactProvenance>,
    /// How long it stays valid.
    #[serde(default, skip_serializing_if = "is_default_freshness")]
    pub freshness: FreshnessPolicy,
}

/// Whether metadata is empty, for output suppression.
fn is_default_metadata(metadata: &ArtifactMetadata) -> bool {
    metadata == &ArtifactMetadata::default()
}

/// Whether a freshness policy is the default, for output suppression.
///
/// Takes a reference because that is the signature `skip_serializing_if` requires.
#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_default_freshness(freshness: &FreshnessPolicy) -> bool {
    *freshness == FreshnessPolicy::default()
}

impl Artifact {
    /// A minimal artifact.
    pub fn new(
        id: ArtifactId,
        kind: ArtifactKind,
        status: ArtifactStatus,
        location: ArtifactLocation,
    ) -> Self {
        Self {
            id,
            kind,
            version: None,
            status,
            location,
            relations: Vec::new(),
            metadata: ArtifactMetadata::default(),
            provenance: None,
            freshness: FreshnessPolicy::default(),
        }
    }

    /// Adds a relation, builder-style.
    #[must_use]
    pub fn with_relation(mut self, kind: RelationKind, target: ArtifactRef) -> Self {
        self.relations.push(ArtifactRelation::new(kind, target));
        self
    }

    /// Every target of `kind`.
    pub fn targets(&self, kind: RelationKind) -> impl Iterator<Item = &ArtifactRef> {
        self.relations
            .iter()
            .filter(move |relation| relation.kind == kind)
            .map(|relation| &relation.target)
    }

    /// `true` when this artifact satisfies a requirement for `kind`, following the hierarchy.
    pub fn is_kind(&self, kind: &ArtifactKind) -> bool {
        self.kind.is_a(kind)
    }
}

/// The legal statuses and status transitions for one artifact kind.
///
/// Lifecycles differ by kind — an ADR goes `proposed → accepted → superseded`, a design goes
/// `draft → in_review → approved → implemented` — and validation rejects a status a kind does
/// not have, which catches the copy-paste error that would otherwise make a requirement
/// silently unsatisfiable.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct ArtifactLifecycle {
    /// The kind this lifecycle governs; absent for the fallback lifecycle.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<ArtifactKind>,
    /// The status a new artifact starts in.
    pub initial: ArtifactStatus,
    /// For each status, the statuses it may move to.
    #[serde(default)]
    pub transitions: BTreeMap<ArtifactStatus, BTreeSet<ArtifactStatus>>,
}

impl ArtifactLifecycle {
    /// The permissive lifecycle used for kinds that declare none: every status is legal.
    pub fn permissive() -> Self {
        Self {
            kind: None,
            initial: ArtifactStatus::Draft,
            transitions: ArtifactStatus::ALL
                .iter()
                .map(|status| (*status, ArtifactStatus::ALL.iter().copied().collect()))
                .collect(),
        }
    }

    /// Every status this lifecycle mentions.
    pub fn statuses(&self) -> BTreeSet<ArtifactStatus> {
        let mut statuses: BTreeSet<ArtifactStatus> = [self.initial].into();
        for (from, to) in &self.transitions {
            statuses.insert(*from);
            statuses.extend(to.iter().copied());
        }
        statuses
    }

    /// `true` when `status` is legal for this kind.
    pub fn permits(&self, status: ArtifactStatus) -> bool {
        self.statuses().contains(&status)
    }

    /// `true` when an artifact may move from `from` to `to`.
    pub fn permits_transition(&self, from: ArtifactStatus, to: ArtifactStatus) -> bool {
        self.transitions
            .get(&from)
            .is_some_and(|targets| targets.contains(&to))
    }
}

/// Lifecycles by artifact kind, with hierarchy fallback.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct LifecycleRegistry {
    lifecycles: BTreeMap<ArtifactKind, ArtifactLifecycle>,
}

impl LifecycleRegistry {
    /// An empty registry, which treats every status as legal.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a lifecycle for a kind.
    pub fn insert(&mut self, kind: ArtifactKind, lifecycle: ArtifactLifecycle) {
        self.lifecycles.insert(kind, lifecycle);
    }

    /// The lifecycle governing `kind`, falling back along the kind hierarchy.
    pub fn for_kind(&self, kind: &ArtifactKind) -> Option<&ArtifactLifecycle> {
        kind.lineage()
            .iter()
            .find_map(|candidate| self.lifecycles.get(candidate))
    }

    /// `true` when nothing is registered.
    pub fn is_empty(&self) -> bool {
        self.lifecycles.is_empty()
    }

    /// Every registered lifecycle.
    pub fn iter(&self) -> impl Iterator<Item = (&ArtifactKind, &ArtifactLifecycle)> {
        self.lifecycles.iter()
    }
}

/// The artifact graph for one execution.
///
/// Construction validates the graph, so a [`ArtifactGraph`] in hand is one whose edges all
/// resolve — the same raw-versus-validated split the rest of the domain uses.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
#[serde(transparent)]
pub struct ArtifactGraph {
    artifacts: BTreeMap<ArtifactId, Artifact>,
}

impl ArtifactGraph {
    /// An empty graph.
    pub fn new() -> Self {
        Self::default()
    }

    /// Builds and validates a graph.
    ///
    /// Rejects duplicate ids, edges pointing at artifacts that do not exist, self-supersession
    /// and cycles within one relation kind. Lifecycle checks need a
    /// [`LifecycleRegistry`]; pass one to [`ArtifactGraph::validate_lifecycles`].
    pub fn build<I: IntoIterator<Item = Artifact>>(artifacts: I) -> Result<Self, ValidationErrors> {
        let mut graph = Self::new();
        let mut errors = ValidationErrors::new();

        for artifact in artifacts {
            let id = artifact.id.clone();
            if graph.artifacts.insert(id.clone(), artifact).is_some() {
                errors.push(
                    ValidationError::new(
                        ValidationCode::UnknownState,
                        format!("artifacts.{id}"),
                        format!("artifact {id} is declared more than once"),
                    )
                    .with_hint("artifact identifiers must be unique within a manifest"),
                );
            }
        }

        errors.extend(graph.validate_edges());
        errors.into_result(graph)
    }

    /// Adds an artifact, replacing any previous record with the same id.
    pub fn insert(&mut self, artifact: Artifact) {
        self.artifacts.insert(artifact.id.clone(), artifact);
    }

    /// The artifact with this id.
    pub fn get(&self, id: &ArtifactId) -> Option<&Artifact> {
        self.artifacts.get(id)
    }

    /// The artifact a reference points at.
    pub fn resolve(&self, reference: &ArtifactRef) -> Option<&Artifact> {
        self.get(reference.id())
    }

    /// Every artifact, in id order.
    pub fn artifacts(&self) -> impl Iterator<Item = &Artifact> {
        self.artifacts.values()
    }

    /// The number of artifacts.
    pub fn len(&self) -> usize {
        self.artifacts.len()
    }

    /// `true` when the graph holds nothing.
    pub fn is_empty(&self) -> bool {
        self.artifacts.is_empty()
    }

    /// Every artifact of `kind`, including kinds that specialise it.
    pub fn of_kind<'a>(&'a self, kind: &'a ArtifactKind) -> impl Iterator<Item = &'a Artifact> {
        self.artifacts
            .values()
            .filter(move |artifact| artifact.is_kind(kind))
    }

    /// Artifacts `id` points at with `relation`.
    pub fn related(&self, id: &ArtifactId, relation: RelationKind) -> Vec<&Artifact> {
        let Some(artifact) = self.get(id) else {
            return Vec::new();
        };
        artifact
            .targets(relation)
            .filter_map(|target| self.get(target.id()))
            .collect()
    }

    /// Artifacts that point at `id` with `relation`.
    ///
    /// This is how "which artifact supersedes this ADR?" and "which review approved this
    /// design?" are answered, since both edges are stored on the newer artifact.
    pub fn inverse(&self, id: &ArtifactId, relation: RelationKind) -> Vec<&Artifact> {
        self.artifacts
            .values()
            .filter(|artifact| artifact.targets(relation).any(|target| target.id() == id))
            .collect()
    }

    /// Walks `relation` edges from `id` transitively, nearest first, excluding `id` itself.
    pub fn ancestors(&self, id: &ArtifactId, relation: RelationKind) -> Vec<&Artifact> {
        let mut seen: BTreeSet<&ArtifactId> = [id].into();
        let mut queue: VecDeque<&ArtifactId> = [id].into();
        let mut found = Vec::new();
        while let Some(current) = queue.pop_front() {
            for artifact in self.related(current, relation) {
                if seen.insert(&artifact.id) {
                    found.push(artifact);
                    queue.push_back(&artifact.id);
                }
            }
        }
        found
    }

    /// Checks every edge resolves, nothing supersedes itself, and no relation kind cycles.
    fn validate_edges(&self) -> ValidationErrors {
        let mut errors = ValidationErrors::new();

        for artifact in self.artifacts.values() {
            for (index, relation) in artifact.relations.iter().enumerate() {
                let location = format!("artifacts.{}.relations[{index}]", artifact.id);
                if !self.artifacts.contains_key(relation.target.id()) {
                    errors.push(
                        ValidationError::new(
                            ValidationCode::UnknownState,
                            location.clone(),
                            format!(
                                "{} {} points at {}, which the manifest does not declare",
                                artifact.id, relation.kind, relation.target
                            ),
                        )
                        .with_hint(
                            "declare the target artifact, or drop the relation; a dangling edge \
                             cannot be checked later",
                        ),
                    );
                }
                if relation.target.id() == &artifact.id {
                    errors.push(ValidationError::new(
                        ValidationCode::UnknownState,
                        location,
                        format!("{} {} itself", artifact.id, relation.kind),
                    ));
                }
            }
        }

        for relation in RelationKind::ALL {
            if let Some(cycle) = self.find_cycle(*relation) {
                errors.push(
                    ValidationError::new(
                        ValidationCode::UnknownState,
                        "artifacts",
                        format!(
                            "`{relation}` edges form a cycle: {}",
                            cycle
                                .iter()
                                .map(ToString::to_string)
                                .collect::<Vec<_>>()
                                .join(" -> ")
                        ),
                    )
                    .with_hint("relations describe derivation, which cannot be circular"),
                );
            }
        }

        errors
    }

    /// Finds one cycle among `relation` edges, if any.
    fn find_cycle(&self, relation: RelationKind) -> Option<Vec<&ArtifactId>> {
        #[derive(Clone, Copy, PartialEq)]
        enum Mark {
            Open,
            Done,
        }

        fn walk<'a>(
            graph: &'a ArtifactGraph,
            relation: RelationKind,
            id: &'a ArtifactId,
            marks: &mut BTreeMap<&'a ArtifactId, Mark>,
            stack: &mut Vec<&'a ArtifactId>,
        ) -> Option<Vec<&'a ArtifactId>> {
            marks.insert(id, Mark::Open);
            stack.push(id);
            for next in graph.related(id, relation) {
                match marks.get(&next.id) {
                    Some(Mark::Open) => {
                        let start = stack
                            .iter()
                            .position(|entry| *entry == &next.id)
                            .unwrap_or(0);
                        let mut cycle: Vec<&ArtifactId> = stack[start..].to_vec();
                        cycle.push(&next.id);
                        return Some(cycle);
                    }
                    Some(Mark::Done) => {}
                    None => {
                        if let Some(cycle) = walk(graph, relation, &next.id, marks, stack) {
                            return Some(cycle);
                        }
                    }
                }
            }
            stack.pop();
            marks.insert(id, Mark::Done);
            None
        }

        let mut marks = BTreeMap::new();
        for id in self.artifacts.keys() {
            if marks.contains_key(id) {
                continue;
            }
            let mut stack = Vec::new();
            if let Some(cycle) = walk(self, relation, id, &mut marks, &mut stack) {
                return Some(cycle);
            }
        }
        None
    }

    /// Checks every artifact's status against its kind's lifecycle, and that a superseded
    /// artifact names its successor.
    pub fn validate_lifecycles(&self, lifecycles: &LifecycleRegistry) -> ValidationErrors {
        let mut errors = ValidationErrors::new();

        for artifact in self.artifacts.values() {
            if let Some(lifecycle) = lifecycles.for_kind(&artifact.kind) {
                if !lifecycle.permits(artifact.status) {
                    errors.push(
                        ValidationError::new(
                            ValidationCode::UnknownState,
                            format!("artifacts.{}.status", artifact.id),
                            format!(
                                "status `{}` is not part of the {} lifecycle ({})",
                                artifact.status,
                                artifact.kind,
                                lifecycle
                                    .statuses()
                                    .iter()
                                    .map(|status| status.as_str())
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            ),
                        )
                        .with_hint("use a status the kind's lifecycle declares"),
                    );
                }
            }

            if artifact.status == ArtifactStatus::Superseded
                && self
                    .inverse(&artifact.id, RelationKind::Supersedes)
                    .is_empty()
            {
                errors.push(
                    ValidationError::new(
                        ValidationCode::UnknownState,
                        format!("artifacts.{}.status", artifact.id),
                        format!(
                            "{} is marked superseded but nothing declares `supersedes: {}`",
                            artifact.id, artifact.id
                        ),
                    )
                    .with_hint(
                        "add the successor artifact with a `supersedes` relation, so the chain of \
                         decisions stays followable",
                    ),
                );
            }

            if artifact.kind == ArtifactKind::ReviewResult
                && artifact.targets(RelationKind::Reviews).next().is_none()
            {
                errors.push(
                    ValidationError::new(
                        ValidationCode::UnknownState,
                        format!("artifacts.{}.relations", artifact.id),
                        format!("review {} does not say what it reviews", artifact.id),
                    )
                    .with_hint("add a `reviews` relation naming the subject"),
                );
            }
        }

        errors
    }

    /// Projects the graph into facts, so predicates can be written against it.
    ///
    /// For every kind present (and every kind it specialises) this emits:
    ///
    /// ```text
    /// artifact.<kind>.exists            bool
    /// artifact.<kind>.count             number
    /// artifact.<kind>.approved          bool    at least one approved, not retired
    /// artifact.<kind>.approved.count    number
    /// artifact.<kind>.<status>.count    number
    /// artifact.total                    number
    /// ```
    pub fn facts(&self) -> FactStore {
        let mut store = FactStore::new();
        let mut counts: BTreeMap<String, usize> = BTreeMap::new();
        let mut approved: BTreeMap<String, usize> = BTreeMap::new();
        let mut by_status: BTreeMap<(String, ArtifactStatus), usize> = BTreeMap::new();

        for artifact in self.artifacts.values() {
            for kind in artifact.kind.lineage() {
                let key = kind.as_str().to_owned();
                *counts.entry(key.clone()).or_default() += 1;
                *by_status.entry((key.clone(), artifact.status)).or_default() += 1;
                if artifact.status.is_approved() {
                    *approved.entry(key).or_default() += 1;
                }
            }
        }

        store.set(
            FactPath::from_segments(["artifact", "total"]),
            FactValue::count(self.len()),
        );

        for (kind, count) in &counts {
            let base = FactPath::from_segments(["artifact", kind]);
            store.set(base.child("exists"), FactValue::bool(true));
            store.set(base.child("count"), FactValue::count(*count));
            let approved_count = approved.get(kind).copied().unwrap_or_default();
            store.set(base.child("approved"), FactValue::bool(approved_count > 0));
            store.set(
                base.child("approved").child("count"),
                FactValue::count(approved_count),
            );
        }

        for ((kind, status), count) in &by_status {
            let base = FactPath::from_segments(["artifact", kind]);
            store.set(
                base.child(status.as_str()).child("count"),
                FactValue::count(*count),
            );
        }

        store
    }
}

/// An artifact manifest document, as parsed.
///
/// This is the file a project keeps — conventionally `.engineering/artifacts.yaml` — that points
/// at its artifacts rather than duplicating them:
///
/// ```yaml
/// version: aep.artifacts/1
/// artifacts:
///   - id: spec:passkeys-auth
///     kind: specification
///     status: approved
///     location:
///       path: docs/specs/passkeys-auth.md
///     relations:
///       - specifies: story:AUTH-142
/// ```
#[derive(Debug, Clone, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RawArtifactManifest {
    /// The manifest format version.
    #[serde(default = "default_manifest_version")]
    pub version: String,
    /// The artifacts it declares.
    #[serde(default)]
    pub artifacts: Vec<Artifact>,
}

/// The manifest format version this build writes.
pub const ARTIFACT_MANIFEST_VERSION: &str = "aep.artifacts/1";

/// Serde default for the manifest version.
fn default_manifest_version() -> String {
    ARTIFACT_MANIFEST_VERSION.to_owned()
}

impl TryFrom<RawArtifactManifest> for ArtifactGraph {
    type Error = ValidationErrors;

    fn try_from(raw: RawArtifactManifest) -> Result<Self, Self::Error> {
        if raw.version != ARTIFACT_MANIFEST_VERSION {
            return Err(ValidationError::new(
                ValidationCode::UnsupportedProtocolVersion,
                "artifacts.version",
                format!(
                    "this build reads manifest version `{ARTIFACT_MANIFEST_VERSION}`, not `{}`",
                    raw.version
                ),
            )
            .into());
        }
        Self::build(raw.artifacts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn artifact(id: &str, kind: &str, status: ArtifactStatus) -> Artifact {
        Artifact::new(
            ArtifactId::new(id).expect("id"),
            ArtifactKind::parse(kind).expect("kind"),
            status,
            ArtifactLocation::Inline,
        )
    }

    fn reference(id: &str) -> ArtifactRef {
        ArtifactRef::parse(id).expect("reference")
    }

    #[test]
    fn design_subkinds_satisfy_a_design_requirement() {
        assert!(ArtifactKind::ArchitectureDesign.is_a(&ArtifactKind::Design));
        assert!(ArtifactKind::Design.is_a(&ArtifactKind::Design));
        assert!(!ArtifactKind::Design.is_a(&ArtifactKind::ArchitectureDesign));
        assert!(!ArtifactKind::Runbook.is_a(&ArtifactKind::Design));
    }

    #[test]
    fn parses_kind_aliases_and_unknown_kinds() {
        assert_eq!(
            ArtifactKind::parse("adr").expect("alias"),
            ArtifactKind::ArchitectureDecisionRecord
        );
        assert_eq!(
            ArtifactKind::parse("threat-model").expect("extension"),
            ArtifactKind::Other("threat-model".to_owned())
        );
        assert!(ArtifactKind::parse("Threat Model").is_err());
    }

    #[test]
    fn parses_locations_in_every_documented_form() {
        let path = ArtifactLocation::from_node(&Node::Map(
            [("path".to_owned(), Node::from("docs/designs/passkeys.md"))].into(),
        ))
        .expect("path form");
        assert_eq!(path.local_path(), Some("docs/designs/passkeys.md"));

        let external = ArtifactLocation::from_node(&Node::Map(
            [
                ("provider".to_owned(), Node::from("linear")),
                ("reference".to_owned(), Node::from("AUTH-142")),
            ]
            .into(),
        ))
        .expect("external form");
        assert!(matches!(external, ArtifactLocation::External { .. }));
        assert_eq!(external.local_path(), None);

        let inline = ArtifactLocation::from_node(&Node::from("inline")).expect("inline form");
        assert_eq!(inline, ArtifactLocation::Inline);

        let error = ArtifactLocation::from_node(&Node::Map(
            [("provider".to_owned(), Node::from("linear"))].into(),
        ))
        .expect_err("provider without reference");
        assert!(error.to_string().contains("reference"), "{error}");
    }

    #[test]
    fn rejects_dangling_edges_and_cycles() {
        let dangling =
            ArtifactGraph::build([artifact("design:x", "design", ArtifactStatus::Draft)
                .with_relation(RelationKind::Designs, reference("spec:missing"))])
            .expect_err("dangling edge");
        assert!(
            dangling.to_string().contains("does not declare"),
            "{dangling}"
        );

        let cyclic = ArtifactGraph::build([
            artifact("design:a", "design", ArtifactStatus::Draft)
                .with_relation(RelationKind::DerivedFrom, reference("design:b")),
            artifact("design:b", "design", ArtifactStatus::Draft)
                .with_relation(RelationKind::DerivedFrom, reference("design:a")),
        ])
        .expect_err("cycle");
        assert!(cyclic.to_string().contains("cycle"), "{cyclic}");
    }

    #[test]
    fn walks_relations_forwards_and_backwards() {
        let graph = ArtifactGraph::build([
            artifact(
                "prd:passkeys",
                "product-requirements",
                ArtifactStatus::Active,
            ),
            artifact("story:auth-142", "story", ArtifactStatus::Active)
                .with_relation(RelationKind::DerivedFrom, reference("prd:passkeys")),
            artifact("spec:passkeys", "specification", ArtifactStatus::Approved)
                .with_relation(RelationKind::Specifies, reference("story:auth-142")),
            artifact("design:passkeys", "design", ArtifactStatus::Approved)
                .with_relation(RelationKind::Designs, reference("spec:passkeys")),
            artifact("adr:0042", "adr", ArtifactStatus::Accepted)
                .with_relation(RelationKind::Decides, reference("design:passkeys")),
        ])
        .expect("valid graph");

        let story = ArtifactId::new("story:auth-142").expect("id");
        assert_eq!(
            graph.related(&story, RelationKind::DerivedFrom)[0]
                .id
                .to_string(),
            "prd:passkeys"
        );
        let design = ArtifactId::new("design:passkeys").expect("id");
        assert_eq!(
            graph.inverse(&design, RelationKind::Decides)[0]
                .id
                .to_string(),
            "adr:0042"
        );
        let chain = graph.ancestors(
            &ArtifactId::new("design:passkeys").expect("id"),
            RelationKind::Designs,
        );
        assert_eq!(chain.len(), 1, "design designs exactly one spec");
    }

    #[test]
    fn superseded_artifacts_must_name_a_successor() {
        let graph = ArtifactGraph::build([artifact("adr:0001", "adr", ArtifactStatus::Superseded)])
            .expect("edges are fine");
        let errors = graph.validate_lifecycles(&LifecycleRegistry::new());
        assert_eq!(errors.len(), 1, "{errors}");
        assert!(errors.to_string().contains("supersedes"), "{errors}");

        let chained = ArtifactGraph::build([
            artifact("adr:0001", "adr", ArtifactStatus::Superseded),
            artifact("adr:0002", "adr", ArtifactStatus::Accepted)
                .with_relation(RelationKind::Supersedes, reference("adr:0001")),
        ])
        .expect("valid");
        assert!(chained
            .validate_lifecycles(&LifecycleRegistry::new())
            .is_empty());
    }

    #[test]
    fn lifecycle_rejects_a_status_the_kind_does_not_have() {
        let mut registry = LifecycleRegistry::new();
        registry.insert(
            ArtifactKind::ArchitectureDecisionRecord,
            ArtifactLifecycle {
                kind: Some(ArtifactKind::ArchitectureDecisionRecord),
                initial: ArtifactStatus::Proposed,
                transitions: [
                    (
                        ArtifactStatus::Proposed,
                        [ArtifactStatus::Accepted, ArtifactStatus::Rejected].into(),
                    ),
                    (
                        ArtifactStatus::Accepted,
                        [ArtifactStatus::Superseded].into(),
                    ),
                ]
                .into(),
            },
        );

        let graph =
            ArtifactGraph::build([artifact("adr:0007", "adr", ArtifactStatus::Implemented)])
                .expect("edges are fine");
        let errors = graph.validate_lifecycles(&registry);
        assert!(errors.to_string().contains("not part of the"), "{errors}");
        assert!(registry
            .for_kind(&ArtifactKind::ArchitectureDecisionRecord)
            .expect("registered")
            .permits_transition(ArtifactStatus::Proposed, ArtifactStatus::Accepted));
    }

    #[test]
    fn projects_facts_including_the_kind_hierarchy() {
        let graph = ArtifactGraph::build([
            artifact("design:a", "architecture-design", ArtifactStatus::Approved),
            artifact("design:b", "design", ArtifactStatus::Draft),
        ])
        .expect("valid");

        let facts = graph.facts();
        let value = |path: &str| {
            crate::facts::FactSource::fact(&facts, &FactPath::new(path).expect("path"))
        };

        assert_eq!(value("artifact.design.count"), Some(FactValue::count(2)));
        assert_eq!(
            value("artifact.architecture-design.count"),
            Some(FactValue::count(1))
        );
        assert_eq!(
            value("artifact.design.approved"),
            Some(FactValue::bool(true))
        );
        assert_eq!(
            value("artifact.design.approved.count"),
            Some(FactValue::count(1))
        );
        assert_eq!(
            value("artifact.design.draft.count"),
            Some(FactValue::count(1))
        );
        assert_eq!(
            value("artifact.runbook.exists"),
            None,
            "absent kinds stay unknown"
        );
    }
}
