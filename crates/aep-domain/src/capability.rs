//! Capabilities and capability policy.
//!
//! AEP does not reason about shell access. It reasons about *semantic capabilities*:
//! `repository.write`, `tests.execute`, `production.write`, `deployment.create:staging`. A
//! harness maps those onto whatever tools it actually exposes, which is what lets one policy
//! mean the same thing in a harness with a shell and in one with a fixed tool list.
//!
//! # Default deny
//!
//! A capability that appears in no list is **not granted**. Least privilege is the default
//! state of the system rather than a principle that has to remember to say so.
//!
//! # Precedence
//!
//! ```text
//! deny  >  require_approval  >  allow  >  (nothing) = not granted
//! ```
//!
//! Deny wins unconditionally: a profile cannot grant back something a principle denied, which
//! is what makes `deny` usable as a safety envelope.

use std::collections::BTreeSet;
use std::fmt;
use std::str::FromStr;

use crate::error::ParseError;
use crate::ids::{PrincipleId, ProfileId, ProtocolId, StateId, TaskId};

/// A deployment target.
///
/// [`Environment::Any`] is a wildcard: `deployment.create` with no environment grants the
/// capability for every environment, and a deny of `deployment.create` denies all of them.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Environment {
    /// Every environment.
    Any,
    /// A local or shared development environment.
    Development,
    /// An automated test environment.
    Test,
    /// A pre-production environment.
    Staging,
    /// The production environment.
    Production,
    /// An environment this vocabulary does not name.
    Named(String),
}

impl Environment {
    /// Parses an environment name.
    pub fn parse(value: &str) -> Result<Self, ParseError> {
        Ok(match value {
            "*" | "any" => Self::Any,
            "development" | "dev" => Self::Development,
            "test" => Self::Test,
            "staging" | "stage" => Self::Staging,
            "production" | "prod" => Self::Production,
            other => {
                let named = crate::ids::PhaseId::new(other).map_err(|_| {
                    ParseError::capability(
                        other,
                        "environment names must be lower-case kebab-case, such as `staging`",
                    )
                })?;
                Self::Named(named.as_str().to_owned())
            }
        })
    }

    /// The environment as written in a capability string.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Any => "*",
            Self::Development => "development",
            Self::Test => "test",
            Self::Staging => "staging",
            Self::Production => "production",
            Self::Named(name) => name,
        }
    }

    /// `true` when a grant for `self` covers `other`.
    pub fn covers(&self, other: &Self) -> bool {
        self == &Self::Any || self == other
    }

    /// `true` when this environment is production, which several principles treat specially.
    pub fn is_production(&self) -> bool {
        matches!(self, Self::Production)
    }
}

impl fmt::Display for Environment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for Environment {
    type Err = ParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

impl serde::Serialize for Environment {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> serde::Deserialize<'de> for Environment {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        Self::parse(&raw).map_err(serde::de::Error::custom)
    }
}

impl schemars::JsonSchema for Environment {
    fn schema_name() -> String {
        "Environment".to_owned()
    }

    fn json_schema(_: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        let mut schema = schemars::schema::SchemaObject {
            instance_type: Some(schemars::schema::InstanceType::String.into()),
            ..Default::default()
        };
        schema.string().pattern = Some("^(\\*|[a-z][a-z0-9-]*)$".to_owned());
        schema.metadata().description =
            Some("A deployment environment; `*` means every environment.".to_owned());
        schema.metadata().examples = ["*", "development", "test", "staging", "production"]
            .iter()
            .map(|value| serde_json::Value::String((*value).to_owned()))
            .collect();
        schema.into()
    }
}

/// A category of action an execution may be authorised to perform.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum Capability {
    /// Read repository contents.
    RepositoryRead,
    /// Modify repository contents.
    RepositoryWrite,
    /// Run tests.
    TestExecution,
    /// Run an arbitrary command in the execution sandbox.
    CommandExecution,
    /// Read from the network.
    NetworkRead,
    /// Send state-changing network requests.
    NetworkWrite,
    /// Query telemetry: metrics, logs, traces.
    TelemetryRead,
    /// Read production state.
    ProductionRead,
    /// Mutate production state.
    ProductionWrite,
    /// Create a deployment in an environment.
    Deploy(Environment),
    /// Roll a deployment back in an environment.
    Rollback(Environment),
    /// Read a secret.
    SecretRead,
    /// Read engineering artifacts: specifications, designs, ADRs.
    ArtifactRead,
    /// Create or update engineering artifacts.
    ArtifactWrite,
    /// Read planning systems such as an issue tracker.
    PlanningRead,
    /// Modify planning systems.
    PlanningWrite,
    /// Ask for a review.
    ReviewRequest,
    /// Ask a human for approval.
    ApprovalRequest,
}

impl Capability {
    /// Every capability that takes no environment, for vocabulary listing and diagnostics.
    pub const SIMPLE: &'static [Self] = &[
        Self::RepositoryRead,
        Self::RepositoryWrite,
        Self::TestExecution,
        Self::CommandExecution,
        Self::NetworkRead,
        Self::NetworkWrite,
        Self::TelemetryRead,
        Self::ProductionRead,
        Self::ProductionWrite,
        Self::SecretRead,
        Self::ArtifactRead,
        Self::ArtifactWrite,
        Self::PlanningRead,
        Self::PlanningWrite,
        Self::ReviewRequest,
        Self::ApprovalRequest,
    ];

    /// The capability name without any environment suffix.
    pub fn name(&self) -> &'static str {
        match self {
            Self::RepositoryRead => "repository.read",
            Self::RepositoryWrite => "repository.write",
            Self::TestExecution => "tests.execute",
            Self::CommandExecution => "command.execute",
            Self::NetworkRead => "network.read",
            Self::NetworkWrite => "network.write",
            Self::TelemetryRead => "telemetry.read",
            Self::ProductionRead => "production.read",
            Self::ProductionWrite => "production.write",
            Self::Deploy(_) => "deployment.create",
            Self::Rollback(_) => "deployment.rollback",
            Self::SecretRead => "secret.read",
            Self::ArtifactRead => "artifact.read",
            Self::ArtifactWrite => "artifact.write",
            Self::PlanningRead => "planning.read",
            Self::PlanningWrite => "planning.write",
            Self::ReviewRequest => "review.request",
            Self::ApprovalRequest => "approval.request",
        }
    }

    /// The environment this capability is scoped to, for the ones that take an environment.
    pub fn environment(&self) -> Option<&Environment> {
        match self {
            Self::Deploy(environment) | Self::Rollback(environment) => Some(environment),
            _ => None,
        }
    }

    /// `true` when holding `self` authorises `other`.
    ///
    /// Environment wildcards are the only widening: `deployment.create` covers
    /// `deployment.create:production`.
    pub fn covers(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Deploy(mine), Self::Deploy(theirs))
            | (Self::Rollback(mine), Self::Rollback(theirs)) => mine.covers(theirs),
            _ => self == other,
        }
    }

    /// `true` when exercising this capability mutates production.
    pub fn mutates_production(&self) -> bool {
        match self {
            Self::ProductionWrite => true,
            Self::Deploy(environment) | Self::Rollback(environment) => {
                environment.is_production() || environment == &Environment::Any
            }
            _ => false,
        }
    }

    /// Parses a capability string, such as `deployment.create:staging`.
    pub fn parse(value: &str) -> Result<Self, ParseError> {
        let (name, environment) = match value.split_once(':') {
            Some((name, environment)) => (name, Some(environment)),
            None => (value, None),
        };
        let with_environment = |constructor: fn(Environment) -> Self| -> Result<Self, ParseError> {
            let environment = match environment {
                Some(raw) => Environment::parse(raw)?,
                None => Environment::Any,
            };
            Ok(constructor(environment))
        };
        let simple = |capability: Self| -> Result<Self, ParseError> {
            if environment.is_some() {
                return Err(ParseError::capability(
                    value,
                    format!("`{name}` does not take an environment"),
                ));
            }
            Ok(capability)
        };

        match name {
            "repository.read" => simple(Self::RepositoryRead),
            "repository.write" => simple(Self::RepositoryWrite),
            "tests.execute" | "test.execute" => simple(Self::TestExecution),
            "command.execute" => simple(Self::CommandExecution),
            "network.read" => simple(Self::NetworkRead),
            "network.write" => simple(Self::NetworkWrite),
            "telemetry.read" | "telemetry.query" => simple(Self::TelemetryRead),
            "production.read" => simple(Self::ProductionRead),
            "production.write" => simple(Self::ProductionWrite),
            "secret.read" => simple(Self::SecretRead),
            "artifact.read" => simple(Self::ArtifactRead),
            "artifact.write" => simple(Self::ArtifactWrite),
            "planning.read" => simple(Self::PlanningRead),
            "planning.write" => simple(Self::PlanningWrite),
            "review.request" => simple(Self::ReviewRequest),
            "approval.request" => simple(Self::ApprovalRequest),
            "deployment.create" | "deploy" => with_environment(Self::Deploy),
            "deployment.rollback" | "rollback" => with_environment(Self::Rollback),
            unknown => Err(ParseError::capability(
                value,
                format!(
                    "{unknown:?} is not a capability; known capabilities are {}, \
                     deployment.create[:env] and deployment.rollback[:env]",
                    Self::SIMPLE
                        .iter()
                        .map(Self::name)
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            )),
        }
    }
}

impl fmt::Display for Capability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.environment() {
            Some(Environment::Any) | None => f.write_str(self.name()),
            Some(environment) => write!(f, "{}:{}", self.name(), environment),
        }
    }
}

impl FromStr for Capability {
    type Err = ParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

impl serde::Serialize for Capability {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> serde::Deserialize<'de> for Capability {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        Self::parse(&raw).map_err(serde::de::Error::custom)
    }
}

impl schemars::JsonSchema for Capability {
    fn schema_name() -> String {
        "Capability".to_owned()
    }

    fn json_schema(_: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        let mut schema = schemars::schema::SchemaObject {
            instance_type: Some(schemars::schema::InstanceType::String.into()),
            ..Default::default()
        };
        let mut examples: Vec<serde_json::Value> = Capability::SIMPLE
            .iter()
            .map(|capability| serde_json::Value::String(capability.name().to_owned()))
            .collect();
        examples.push(serde_json::Value::String(
            "deployment.create:staging".to_owned(),
        ));
        examples.push(serde_json::Value::String(
            "deployment.rollback:production".to_owned(),
        ));
        schema.metadata().description = Some(
            "A semantic capability, such as `repository.write` or `deployment.create:staging`."
                .to_owned(),
        );
        schema.metadata().examples = examples;
        schema.into()
    }
}

/// What a policy says about one capability.
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
pub enum CapabilityDecision {
    /// May be exercised.
    Allowed,
    /// May be exercised once a matching approval has been recorded.
    RequiresApproval,
    /// Explicitly forbidden; no grant can override this.
    Denied,
    /// Not mentioned by the policy, and therefore not granted.
    NotGranted,
}

impl CapabilityDecision {
    /// `true` when the action may proceed without further authorisation.
    pub fn is_allowed(self) -> bool {
        self == Self::Allowed
    }

    /// The decision as it appears in output.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Allowed => "allowed",
            Self::RequiresApproval => "requires_approval",
            Self::Denied => "denied",
            Self::NotGranted => "not_granted",
        }
    }
}

impl fmt::Display for CapabilityDecision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Which document contributed a policy entry, so a denial can be explained.
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
#[serde(tag = "source", rename_all = "snake_case")]
pub enum PolicySource {
    /// The protocol's own vocabulary declaration.
    Protocol {
        /// The protocol.
        protocol: ProtocolId,
    },
    /// A profile.
    Profile {
        /// The profile.
        profile: ProfileId,
    },
    /// A principle.
    Principle {
        /// The principle.
        principle: PrincipleId,
    },
    /// The task's own constraints.
    Task {
        /// The task.
        task: TaskId,
    },
    /// A workflow state's override.
    State {
        /// The state.
        state: StateId,
    },
}

impl fmt::Display for PolicySource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Protocol { protocol } => write!(f, "protocol {protocol}"),
            Self::Profile { profile } => write!(f, "profile {profile}"),
            Self::Principle { principle } => write!(f, "principle {principle}"),
            Self::Task { task } => write!(f, "task {task}"),
            Self::State { state } => write!(f, "state {state}"),
        }
    }
}

/// The capabilities an execution may exercise.
#[derive(
    Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct CapabilityPolicy {
    /// Capabilities that may be exercised.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub allow: BTreeSet<Capability>,

    /// Capabilities that may be exercised only with a recorded approval.
    #[serde(
        default,
        alias = "require_approval",
        skip_serializing_if = "BTreeSet::is_empty"
    )]
    pub approval_required: BTreeSet<Capability>,

    /// Capabilities that must never be exercised.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub deny: BTreeSet<Capability>,
}

impl CapabilityPolicy {
    /// A policy that grants nothing.
    pub fn empty() -> Self {
        Self::default()
    }

    /// A policy allowing exactly `capabilities`.
    pub fn allowing<I: IntoIterator<Item = Capability>>(capabilities: I) -> Self {
        Self {
            allow: capabilities.into_iter().collect(),
            ..Self::default()
        }
    }

    /// A policy denying exactly `capabilities`.
    pub fn denying<I: IntoIterator<Item = Capability>>(capabilities: I) -> Self {
        Self {
            deny: capabilities.into_iter().collect(),
            ..Self::default()
        }
    }

    /// `true` when the policy mentions nothing at all.
    pub fn is_empty(&self) -> bool {
        self.allow.is_empty() && self.deny.is_empty() && self.approval_required.is_empty()
    }

    /// What this policy says about `capability`.
    pub fn decide(&self, capability: &Capability) -> CapabilityDecision {
        if Self::covered_by(&self.deny, capability) {
            return CapabilityDecision::Denied;
        }
        if Self::covered_by(&self.approval_required, capability) {
            return CapabilityDecision::RequiresApproval;
        }
        if Self::covered_by(&self.allow, capability) {
            return CapabilityDecision::Allowed;
        }
        CapabilityDecision::NotGranted
    }

    /// The policy entry responsible for a decision, for explanation.
    pub fn matching_entry(&self, capability: &Capability) -> Option<&Capability> {
        Self::find(&self.deny, capability)
            .or_else(|| Self::find(&self.approval_required, capability))
            .or_else(|| Self::find(&self.allow, capability))
    }

    fn covered_by(set: &BTreeSet<Capability>, capability: &Capability) -> bool {
        Self::find(set, capability).is_some()
    }

    fn find<'a>(set: &'a BTreeSet<Capability>, capability: &Capability) -> Option<&'a Capability> {
        set.iter().find(|entry| entry.covers(capability))
    }

    /// Adds `other`'s grants, approvals and denials to this policy.
    ///
    /// Used when a broader document hands capabilities to a narrower one.
    pub fn grant(&mut self, other: &Self) {
        self.allow.extend(other.allow.iter().cloned());
        self.approval_required
            .extend(other.approval_required.iter().cloned());
        self.deny.extend(other.deny.iter().cloned());
    }

    /// Adds `other`'s approvals and denials only, never its grants.
    ///
    /// This is how principles compose: a principle can take capabilities away or put them
    /// behind approval, but a principle cannot hand out access a profile did not grant.
    pub fn restrict(&mut self, other: &Self) {
        self.approval_required
            .extend(other.approval_required.iter().cloned());
        self.deny.extend(other.deny.iter().cloned());
    }

    /// Every capability the policy mentions.
    pub fn mentioned(&self) -> BTreeSet<&Capability> {
        self.allow
            .iter()
            .chain(&self.approval_required)
            .chain(&self.deny)
            .collect()
    }

    /// Capabilities that are granted (directly or behind approval) and mutate production.
    pub fn production_mutations(&self) -> Vec<&Capability> {
        self.allow
            .iter()
            .chain(&self.approval_required)
            .filter(|capability| capability.mutates_production())
            .filter(|capability| self.decide(capability) != CapabilityDecision::Denied)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn capability(value: &str) -> Capability {
        Capability::parse(value).expect("parses")
    }

    #[test]
    fn parses_and_renders_capability_strings() {
        assert_eq!(capability("repository.write"), Capability::RepositoryWrite);
        assert_eq!(
            capability("deployment.create:staging"),
            Capability::Deploy(Environment::Staging)
        );
        assert_eq!(
            capability("deployment.create"),
            Capability::Deploy(Environment::Any)
        );
        assert_eq!(
            capability("deployment.create:staging").to_string(),
            "deployment.create:staging"
        );
        assert_eq!(
            capability("deployment.create").to_string(),
            "deployment.create"
        );
    }

    #[test]
    fn rejects_unknown_capabilities_and_misplaced_environments() {
        let error = Capability::parse("kubernetes.exec").expect_err("unknown");
        assert!(error.to_string().contains("is not a capability"), "{error}");
        assert!(Capability::parse("repository.read:production").is_err());
    }

    #[test]
    fn unmentioned_capabilities_are_not_granted() {
        let policy = CapabilityPolicy::allowing([Capability::RepositoryRead]);
        assert_eq!(
            policy.decide(&Capability::RepositoryWrite),
            CapabilityDecision::NotGranted
        );
    }

    #[test]
    fn deny_beats_approval_which_beats_allow() {
        let policy = CapabilityPolicy {
            allow: [Capability::ProductionWrite, Capability::SecretRead].into(),
            approval_required: [Capability::ProductionWrite].into(),
            deny: [Capability::SecretRead].into(),
        };
        assert_eq!(
            policy.decide(&Capability::ProductionWrite),
            CapabilityDecision::RequiresApproval
        );
        assert_eq!(
            policy.decide(&Capability::SecretRead),
            CapabilityDecision::Denied
        );
    }

    #[test]
    fn an_environment_wildcard_covers_named_environments() {
        let policy = CapabilityPolicy::allowing([Capability::Deploy(Environment::Any)]);
        assert!(policy
            .decide(&Capability::Deploy(Environment::Production))
            .is_allowed());

        let narrow = CapabilityPolicy::allowing([Capability::Deploy(Environment::Staging)]);
        assert_eq!(
            narrow.decide(&Capability::Deploy(Environment::Production)),
            CapabilityDecision::NotGranted
        );
    }

    #[test]
    fn restrict_cannot_grant_but_grant_can() {
        let mut policy = CapabilityPolicy::allowing([Capability::RepositoryRead]);
        policy.restrict(&CapabilityPolicy::allowing([Capability::ProductionWrite]));
        assert_eq!(
            policy.decide(&Capability::ProductionWrite),
            CapabilityDecision::NotGranted,
            "a principle must not be able to hand out access"
        );

        policy.restrict(&CapabilityPolicy::denying([Capability::RepositoryRead]));
        assert_eq!(
            policy.decide(&Capability::RepositoryRead),
            CapabilityDecision::Denied
        );

        let mut granted = CapabilityPolicy::empty();
        granted.grant(&CapabilityPolicy::allowing([Capability::TestExecution]));
        assert!(granted.decide(&Capability::TestExecution).is_allowed());
    }

    #[test]
    fn identifies_production_mutations() {
        assert!(Capability::ProductionWrite.mutates_production());
        assert!(Capability::Deploy(Environment::Production).mutates_production());
        assert!(!Capability::Deploy(Environment::Staging).mutates_production());
        assert!(!Capability::RepositoryWrite.mutates_production());
    }
}
