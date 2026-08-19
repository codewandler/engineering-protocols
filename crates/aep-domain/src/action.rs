//! Actions: the operations an agent may ask to perform.
//!
//! An action is what a harness asks the protocol about *before* doing it. Each action maps to
//! exactly one [`Capability`], so authorisation is a lookup rather than a judgement call.

use crate::capability::{Capability, Environment};
use crate::evidence::TestSuite;
use crate::ids::{ApprovalId, ToolRef};

/// The direction of a network request, which decides which capability it needs.
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
pub enum NetworkIntent {
    /// A request that only reads.
    Read,
    /// A request that changes remote state.
    Write,
}

/// Reading repository contents.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct RepositoryRead {
    /// Paths to be read, relative to the repository root.
    pub paths: Vec<String>,
}

/// Modifying repository contents.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct RepositoryWrite {
    /// Paths to be written, relative to the repository root.
    pub paths: Vec<String>,
    /// What the change is for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intent: Option<String>,
}

/// Running tests.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct TestExecute {
    /// The suite to run.
    pub suite: TestSuite,
    /// A selector narrowing the run, such as a test name filter.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selector: Option<String>,
}

/// Running a command in the execution sandbox.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct CommandExecute {
    /// The program.
    pub program: String,
    /// Its arguments.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
}

/// Making a network request.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct NetworkRequest {
    /// Where the request goes.
    pub url: String,
    /// Whether it changes remote state.
    pub intent: NetworkIntent,
}

/// Querying telemetry.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct TelemetryQuery {
    /// The query, in whatever language the telemetry system speaks.
    pub query: String,
    /// The service the query is about.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service: Option<String>,
}

/// Creating a deployment.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct Deploy {
    /// The target environment.
    pub environment: Environment,
    /// The revision being deployed.
    pub revision: String,
    /// The rollout strategy, such as `canary`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strategy: Option<String>,
}

/// Rolling a deployment back.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct Rollback {
    /// The target environment.
    pub environment: Environment,
    /// The revision to roll back to; absent means "the previous one".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to_revision: Option<String>,
}

/// Changing production state directly, outside a deployment.
///
/// Configuration flags, a database migration run by hand, a queue drained: the things that are not a
/// deployment but change what production does. They need their own action because `deployment.create`
/// is not an honest description of them, and a policy that only names deployments would let them
/// through.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct ProductionMutate {
    /// What is being changed, such as a flag name or a table.
    pub target: String,
    /// What the change is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub change: Option<String>,
}

/// Reading a secret.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct SecretRead {
    /// The secret's name.
    pub secret: String,
}

/// Creating or updating an engineering artifact.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct ArtifactWrite {
    /// The artifact being written.
    pub artifact: crate::artifact::ArtifactRef,
    /// What kind of artifact it is.
    pub kind: crate::artifact::ArtifactKind,
}

/// Asking for a review.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct ReviewRequest {
    /// What is to be reviewed.
    pub subject: crate::artifact::ArtifactRef,
    /// Who or which group is asked.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reviewer: Option<String>,
}

/// Asking a human for approval.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct ApprovalRequest {
    /// Which approval is being sought.
    pub approval: ApprovalId,
    /// Why it is needed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Running an external tool that is neither a test runner nor a shell command.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct ToolInvoke {
    /// The tool.
    pub tool: ToolRef,
    /// Its arguments.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
}

/// A meaningful operation an agent may request.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(tag = "action", rename_all = "snake_case")]
#[non_exhaustive]
pub enum Action {
    /// Read repository contents.
    RepositoryRead(RepositoryRead),
    /// Modify repository contents.
    RepositoryWrite(RepositoryWrite),
    /// Run tests.
    TestExecute(TestExecute),
    /// Run a command.
    CommandExecute(CommandExecute),
    /// Make a network request.
    NetworkRequest(NetworkRequest),
    /// Query telemetry.
    TelemetryQuery(TelemetryQuery),
    /// Create a deployment.
    Deploy(Deploy),
    /// Roll a deployment back.
    Rollback(Rollback),
    /// Change production state directly.
    ProductionMutate(ProductionMutate),
    /// Read a secret.
    SecretRead(SecretRead),
    /// Create or update an artifact.
    ArtifactWrite(ArtifactWrite),
    /// Ask for a review.
    ReviewRequest(ReviewRequest),
    /// Ask for approval.
    ApprovalRequest(ApprovalRequest),
    /// Run an external tool.
    ToolInvoke(ToolInvoke),
}

impl Action {
    /// The single capability this action requires.
    pub fn required_capability(&self) -> Capability {
        match self {
            Self::RepositoryRead(_) => Capability::RepositoryRead,
            Self::RepositoryWrite(_) => Capability::RepositoryWrite,
            Self::TestExecute(_) => Capability::TestExecution,
            Self::CommandExecute(_) | Self::ToolInvoke(_) => Capability::CommandExecution,
            Self::NetworkRequest(request) => match request.intent {
                NetworkIntent::Read => Capability::NetworkRead,
                NetworkIntent::Write => Capability::NetworkWrite,
            },
            Self::TelemetryQuery(_) => Capability::TelemetryRead,
            Self::Deploy(deploy) => Capability::Deploy(deploy.environment.clone()),
            Self::Rollback(rollback) => Capability::Rollback(rollback.environment.clone()),
            Self::ProductionMutate(_) => Capability::ProductionWrite,
            Self::SecretRead(_) => Capability::SecretRead,
            Self::ArtifactWrite(_) => Capability::ArtifactWrite,
            Self::ReviewRequest(_) => Capability::ReviewRequest,
            Self::ApprovalRequest(_) => Capability::ApprovalRequest,
        }
    }

    /// A one-line description, for audit records and explanations.
    pub fn summary(&self) -> String {
        match self {
            Self::RepositoryRead(read) => format!("read {}", read.paths.join(", ")),
            Self::RepositoryWrite(write) => format!("write {}", write.paths.join(", ")),
            Self::TestExecute(test) => match &test.selector {
                Some(selector) => format!("run {} tests matching {selector}", test.suite),
                None => format!("run {} tests", test.suite),
            },
            Self::CommandExecute(command) => {
                format!("run `{} {}`", command.program, command.args.join(" "))
                    .trim_end()
                    .to_owned()
            }
            Self::NetworkRequest(request) => format!("{:?} {}", request.intent, request.url),
            Self::TelemetryQuery(query) => format!("query telemetry: {}", query.query),
            Self::Deploy(deploy) => {
                format!("deploy {} to {}", deploy.revision, deploy.environment)
            }
            Self::Rollback(rollback) => match &rollback.to_revision {
                Some(revision) => format!("roll {} back to {revision}", rollback.environment),
                None => format!("roll {} back", rollback.environment),
            },
            Self::ProductionMutate(mutation) => match &mutation.change {
                Some(change) => format!("change production {}: {change}", mutation.target),
                None => format!("change production {}", mutation.target),
            },
            Self::SecretRead(secret) => format!("read secret {}", secret.secret),
            Self::ArtifactWrite(write) => format!("write {} {}", write.kind, write.artifact),
            Self::ReviewRequest(review) => format!("request review of {}", review.subject),
            Self::ApprovalRequest(approval) => format!("request approval {}", approval.approval),
            Self::ToolInvoke(tool) => format!("invoke tool {}", tool.tool),
        }
    }

    /// `true` when this action changes state outside the execution sandbox.
    ///
    /// Used by the reversible-changes and blast-radius principles, which care about mutation
    /// rather than about which capability was used.
    pub fn is_mutating(&self) -> bool {
        matches!(
            self,
            Self::RepositoryWrite(_)
                | Self::ProductionMutate(_)
                | Self::Deploy(_)
                | Self::Rollback(_)
                | Self::ArtifactWrite(_)
                | Self::NetworkRequest(NetworkRequest {
                    intent: NetworkIntent::Write,
                    ..
                })
        )
    }
}

/// An action together with who is asking, as submitted to the engine for authorisation.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct ActionRequest {
    /// The action.
    pub action: Action,
    /// Who is asking.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requested_by: Option<crate::evidence::Producer>,
    /// Free-form rationale, recorded in the audit trail.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rationale: Option<String>,
}

impl ActionRequest {
    /// Wraps an action with no attribution.
    pub fn new(action: Action) -> Self {
        Self {
            action,
            requested_by: None,
            rationale: None,
        }
    }

    /// The capability the wrapped action requires.
    pub fn required_capability(&self) -> Capability {
        self.action.required_capability()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_action_maps_to_one_capability() {
        let deploy = Action::Deploy(Deploy {
            environment: Environment::Production,
            revision: "rev-4711".to_owned(),
            strategy: Some("canary".to_owned()),
        });
        assert_eq!(
            deploy.required_capability(),
            Capability::Deploy(Environment::Production)
        );
        assert!(deploy.is_mutating());

        let read = Action::RepositoryRead(RepositoryRead {
            paths: vec!["src/lib.rs".to_owned()],
        });
        assert_eq!(read.required_capability(), Capability::RepositoryRead);
        assert!(!read.is_mutating());
    }

    #[test]
    fn network_intent_selects_the_capability() {
        let read = Action::NetworkRequest(NetworkRequest {
            url: "https://example.test/health".to_owned(),
            intent: NetworkIntent::Read,
        });
        let write = Action::NetworkRequest(NetworkRequest {
            url: "https://example.test/deploy".to_owned(),
            intent: NetworkIntent::Write,
        });
        assert_eq!(read.required_capability(), Capability::NetworkRead);
        assert_eq!(write.required_capability(), Capability::NetworkWrite);
    }
}
