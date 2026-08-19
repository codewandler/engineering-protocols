//! Tasks: one unit of work, and the protocol it is executed under.
//!
//! ```yaml
//! id: AUTH-142
//! kind: feature
//! objective: add-passkey-support
//! protocol: aep/1
//! profile: development.standard
//! principle_overrides:
//!   add: [clean-room, differential-testing]
//!   remove: [mutation-testing]
//! derived_from:
//!   - story:AUTH-141
//! context:
//!   product_requirements: [prd:passkeys]
//! constraints:
//!   facts:
//!     change.architectural: true
//!     service.slo.error_threshold: 0.01
//! ```
//!
//! # Constraint facts
//!
//! `constraints.facts` is how a task states things nothing can observe for it: the SLO
//! threshold a completion condition compares against, whether the change is architectural,
//! what risk tier it is. They are inputs, so they are recorded as declared by the task and
//! distinguishable in the audit trail from anything a verifier established.

use std::collections::BTreeMap;
use std::fmt;

use crate::artifact::ArtifactRef;
use crate::capability::CapabilityPolicy;
use crate::error::ParseError;
use crate::facts::{FactPath, FactStore, FactValue};
use crate::ids::TaskId;
use crate::node::Node;
use crate::principle::PrincipleOverrides;
use crate::version::{ProfileVersionedRef, ProtocolRef};

/// What sort of work this is.
///
/// The kind is what most principles switch on: `test-driven` applies to a feature or a bugfix,
/// not to an incident mitigation at two in the morning.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum TaskKind {
    /// New behaviour.
    Feature,
    /// Fixing broken behaviour.
    Bugfix,
    /// Changing structure without changing behaviour.
    Refactor,
    /// Finding something out.
    Investigation,
    /// Responding to a live problem.
    Incident,
    /// Shipping.
    Release,
    /// Moving data or systems.
    Migration,
    /// Moving a dependency.
    DependencyUpgrade,
    /// Responding to a security issue.
    SecurityResponse,
    /// Changing infrastructure.
    InfrastructureChange,
    /// Writing things down.
    Documentation,
    /// A kind this vocabulary does not name.
    Other(String),
}

impl TaskKind {
    /// Every named kind.
    pub const NAMED: &'static [Self] = &[
        Self::Feature,
        Self::Bugfix,
        Self::Refactor,
        Self::Investigation,
        Self::Incident,
        Self::Release,
        Self::Migration,
        Self::DependencyUpgrade,
        Self::SecurityResponse,
        Self::InfrastructureChange,
        Self::Documentation,
    ];

    /// The kind as written in documents and facts.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Feature => "feature",
            Self::Bugfix => "bugfix",
            Self::Refactor => "refactor",
            Self::Investigation => "investigation",
            Self::Incident => "incident",
            Self::Release => "release",
            Self::Migration => "migration",
            Self::DependencyUpgrade => "dependency-upgrade",
            Self::SecurityResponse => "security-response",
            Self::InfrastructureChange => "infrastructure-change",
            Self::Documentation => "documentation",
            Self::Other(name) => name,
        }
    }

    /// Parses a kind name; anything kebab-case is accepted.
    pub fn parse(value: &str) -> Result<Self, ParseError> {
        if let Some(known) = Self::NAMED.iter().find(|kind| kind.as_str() == value) {
            return Ok(known.clone());
        }
        Ok(match value {
            "fix" | "bug" => Self::Bugfix,
            "feat" => Self::Feature,
            other => {
                let named = crate::ids::PrincipleId::new(other).map_err(|_| {
                    ParseError::identifier(
                        "task kind",
                        other,
                        "task kinds are lower-case kebab-case, such as `dependency-upgrade`"
                            .to_owned(),
                    )
                })?;
                Self::Other(named.as_str().to_owned())
            }
        })
    }
}

impl fmt::Display for TaskKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for TaskKind {
    type Err = ParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

impl serde::Serialize for TaskKind {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> serde::Deserialize<'de> for TaskKind {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        Self::parse(&raw).map_err(serde::de::Error::custom)
    }
}

impl schemars::JsonSchema for TaskKind {
    fn schema_name() -> String {
        "TaskKind".to_owned()
    }

    fn json_schema(_: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        let mut schema = schemars::schema::SchemaObject {
            instance_type: Some(schemars::schema::InstanceType::String.into()),
            ..Default::default()
        };
        schema.string().pattern = Some("^[a-z][a-z0-9-]*$".to_owned());
        schema.metadata().description = Some("What sort of work the task is.".to_owned());
        schema.metadata().examples = TaskKind::NAMED
            .iter()
            .map(|kind| serde_json::Value::String(kind.as_str().to_owned()))
            .collect();
        schema.into()
    }
}

/// What the task is for.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, schemars::JsonSchema)]
pub struct Objective {
    /// A one-line statement, such as `add-passkey-support`.
    pub summary: String,
    /// Anything longer.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
}

impl Objective {
    /// An objective with only a summary.
    pub fn new(summary: impl Into<String>) -> Self {
        Self {
            summary: summary.into(),
            details: None,
        }
    }
}

impl fmt::Display for Objective {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.summary)
    }
}

impl<'de> serde::Deserialize<'de> for Objective {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let node = Node::deserialize(deserializer)?;
        match &node {
            Node::Text(summary) => Ok(Self::new(summary.clone())),
            Node::Map(entries) => {
                let summary = entries
                    .get("summary")
                    .or_else(|| entries.get("objective"))
                    .and_then(Node::as_text)
                    .ok_or_else(|| {
                        serde::de::Error::custom(ParseError::shape(
                            "objective",
                            "a `summary` field",
                            "no `summary`",
                        ))
                    })?;
                Ok(Self {
                    summary: summary.to_owned(),
                    details: entries
                        .get("details")
                        .and_then(Node::as_text)
                        .map(ToOwned::to_owned),
                })
            }
            other => Err(serde::de::Error::custom(ParseError::shape(
                "objective",
                "a string or a mapping",
                other.type_name(),
            ))),
        }
    }
}

/// Limits and inputs that apply to one task.
#[derive(
    Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct Constraints {
    /// Facts the task declares, which nothing else can observe for it.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub facts: BTreeMap<FactPath, FactValue>,
    /// Capability limits on top of the profile's.
    #[serde(default, skip_serializing_if = "CapabilityPolicy::is_empty")]
    pub capabilities: CapabilityPolicy,
    /// Anything a person needs to know that the protocol does not model.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
}

impl Constraints {
    /// `true` when nothing is constrained.
    pub fn is_empty(&self) -> bool {
        self.facts.is_empty() && self.capabilities.is_empty() && self.notes.is_empty()
    }
}

/// The artifacts a task comes from and depends on.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, schemars::JsonSchema)]
pub struct TaskArtifacts {
    /// What this task was decomposed from.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub derived_from: Vec<ArtifactRef>,
    /// Other artifacts that constrain it, grouped by a label of the author's choosing.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub context: BTreeMap<String, Vec<ArtifactRef>>,
    /// Where the artifact manifest lives, when the task carries one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manifest: Option<String>,
}

impl TaskArtifacts {
    /// `true` when the task references no artifacts.
    pub fn is_empty(&self) -> bool {
        self.derived_from.is_empty() && self.context.is_empty() && self.manifest.is_none()
    }

    /// Every referenced artifact.
    pub fn all(&self) -> Vec<&ArtifactRef> {
        self.derived_from
            .iter()
            .chain(self.context.values().flatten())
            .collect()
    }
}

/// One unit of work.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Task {
    /// Its identifier.
    pub id: TaskId,
    /// What sort of work it is.
    pub kind: TaskKind,
    /// What it is for.
    pub objective: Objective,
    /// Which protocol version governs it.
    pub protocol: ProtocolRef,
    /// Which profile it uses.
    pub profile: ProfileVersionedRef,
    /// Its limits and declared inputs.
    #[serde(skip_serializing_if = "Constraints::is_empty")]
    pub constraints: Constraints,
    /// Principles it adds or drops.
    #[serde(skip_serializing_if = "PrincipleOverrides::is_empty")]
    pub principle_overrides: PrincipleOverrides,
    /// The artifacts it comes from and depends on.
    #[serde(skip_serializing_if = "TaskArtifacts::is_empty")]
    pub artifacts: TaskArtifacts,
}

impl Task {
    /// The facts a task contributes before anything has been observed.
    ///
    /// These are what a principle's applicability condition reads: `task.kind`, `task.id`, plus
    /// whatever the task declared in `constraints.facts`.
    pub fn facts(&self) -> FactStore {
        let mut store = FactStore::new();
        store.set_path("task.id", FactValue::text(self.id.as_str()));
        store.set_path("task.kind", FactValue::text(self.kind.as_str()));
        store.set_path(
            "task.objective",
            FactValue::text(self.objective.summary.clone()),
        );
        store.set_path("task.profile", FactValue::text(self.profile.to_string()));
        for (path, value) in &self.constraints.facts {
            store.set(path.clone(), value.clone());
        }
        store
    }
}

/// A task document, as parsed.
#[derive(Debug, Clone, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RawTask {
    /// Its identifier.
    pub id: TaskId,
    /// What sort of work it is.
    #[serde(alias = "type")]
    pub kind: TaskKind,
    /// What it is for.
    pub objective: Objective,
    /// Which protocol version governs it.
    pub protocol: ProtocolRef,
    /// Which profile it uses.
    pub profile: ProfileVersionedRef,
    /// Its limits and declared inputs.
    #[serde(default)]
    pub constraints: Constraints,
    /// Principles it adds or drops.
    #[serde(default, alias = "principles")]
    pub principle_overrides: PrincipleOverrides,
    /// What this task was decomposed from.
    #[serde(default)]
    pub derived_from: Vec<Node>,
    /// Other artifacts that constrain it.
    #[serde(default)]
    pub context: BTreeMap<String, Vec<Node>>,
    /// Where the artifact manifest lives.
    #[serde(default, alias = "artifact_manifest")]
    pub manifest: Option<String>,
}

/// Parses an artifact reference written either bare or as `{artifact: <ref>}`.
fn artifact_reference(node: &Node, location: &str) -> Result<ArtifactRef, ParseError> {
    match node {
        Node::Text(reference) => ArtifactRef::parse(reference),
        Node::Map(entries) => entries
            .get("artifact")
            .and_then(Node::as_text)
            .ok_or_else(|| ParseError::shape(location, "an `artifact` field", "no `artifact`"))
            .and_then(ArtifactRef::parse),
        other => Err(ParseError::shape(
            location,
            "an artifact reference or `{artifact: <ref>}`",
            other.type_name(),
        )),
    }
}

impl TryFrom<RawTask> for Task {
    type Error = crate::error::ValidationErrors;

    fn try_from(raw: RawTask) -> Result<Self, Self::Error> {
        let mut errors = crate::error::ValidationErrors::new();
        let mut derived_from = Vec::new();
        for (index, node) in raw.derived_from.iter().enumerate() {
            match artifact_reference(node, &format!("task {}.derived_from[{index}]", raw.id)) {
                Ok(reference) => derived_from.push(reference),
                Err(error) => errors.push(crate::error::ValidationError::new(
                    crate::error::ValidationCode::UnknownState,
                    format!("task {}.derived_from[{index}]", raw.id),
                    error.to_string(),
                )),
            }
        }

        let mut context = BTreeMap::new();
        for (label, nodes) in &raw.context {
            let mut references = Vec::new();
            for (index, node) in nodes.iter().enumerate() {
                match artifact_reference(node, &format!("task {}.context.{label}[{index}]", raw.id))
                {
                    Ok(reference) => references.push(reference),
                    Err(error) => errors.push(crate::error::ValidationError::new(
                        crate::error::ValidationCode::UnknownState,
                        format!("task {}.context.{label}[{index}]", raw.id),
                        error.to_string(),
                    )),
                }
            }
            context.insert(label.clone(), references);
        }

        let task = Self {
            id: raw.id,
            kind: raw.kind,
            objective: raw.objective,
            protocol: raw.protocol,
            profile: raw.profile,
            constraints: raw.constraints,
            principle_overrides: raw.principle_overrides,
            artifacts: TaskArtifacts {
                derived_from,
                context,
                manifest: raw.manifest,
            },
        };
        errors.into_result(task)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::facts::FactSource;

    fn task(yaml: &str) -> Task {
        let raw: RawTask = serde_yaml::from_str(yaml).expect("document parses");
        Task::try_from(raw).expect("document validates")
    }

    #[test]
    fn parses_the_documented_task_shape() {
        let parsed = task(
            r"
id: AUTH-142
type: feature
objective: add-passkey-support
protocol: aep/1
profile: development.standard
principles:
  add: [clean-room, differential-testing]
derived_from:
  - artifact: story:AUTH-141
context:
  product_requirements: [prd:passkeys]
constraints:
  facts:
    change.architectural: true
    service.slo.error_threshold: 0.01
",
        );

        assert_eq!(parsed.kind, TaskKind::Feature);
        assert_eq!(parsed.objective.to_string(), "add-passkey-support");
        assert_eq!(parsed.protocol.to_string(), "aep/1");
        assert_eq!(parsed.principle_overrides.add.len(), 2);
        assert_eq!(
            parsed.artifacts.derived_from[0].to_string(),
            "story:AUTH-141"
        );
        assert_eq!(parsed.artifacts.all().len(), 2);
    }

    #[test]
    fn task_facts_include_the_kind_and_declared_constraints() {
        let parsed = task(
            r"
id: OPS-7
kind: incident
objective:
  summary: restore checkout
  details: error rate above SLO since 09:12
protocol: aep/1
profile: incident.standard
constraints:
  facts:
    service.slo.error_threshold: 0.01
",
        );

        let facts = parsed.facts();
        let value = |path: &str| facts.fact(&FactPath::new(path).expect("path"));
        assert_eq!(value("task.kind"), Some(FactValue::text("incident")));
        assert_eq!(value("task.id"), Some(FactValue::text("OPS-7")));
        assert_eq!(
            value("service.slo.error_threshold"),
            Some(FactValue::number(0.01).expect("finite"))
        );
        assert_eq!(
            parsed.objective.details.as_deref(),
            Some("error rate above SLO since 09:12")
        );
    }

    #[test]
    fn rejects_an_unparsable_artifact_reference() {
        let raw: RawTask = serde_yaml::from_str(
            r"
id: T-1
kind: feature
objective: something
protocol: aep/1
profile: development.standard
derived_from:
  - not-an-artifact-reference
",
        )
        .expect("document parses");
        let errors = Task::try_from(raw).expect_err("bad reference");
        assert!(errors.to_string().contains("namespace"), "{errors}");
    }
}
