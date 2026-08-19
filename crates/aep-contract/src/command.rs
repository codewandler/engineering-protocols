//! The command side: one boundary for every state change.
//!
//! # The context is the point
//!
//! A command carries six identifiers that look redundant and are not:
//!
//! | field | answers |
//! |---|---|
//! | `request_id` | which transport attempt was this? |
//! | `command_id` | which *logical* command? a retry reuses it |
//! | `idempotency_key` | is this the same intended mutation as one already applied? |
//! | `actor` | on whose behalf? |
//! | `executor` | what actually ran? |
//! | `correlation_id` | what wider activity does this belong to? |
//! | `causation` | what directly caused *this* one thing? |
//!
//! The pair that earns its keep is actor and executor. `actor: human:alice, executor:
//! agent:release-agent-17` is the ordinary case for agentic work, and a trail that collapses them
//! can answer neither "who authorised this?" nor "what did it?".

use aep_domain::entity::{ActorRef, EntityRef, EntityRevision, VersionedEntityRef};
use aep_domain::ids::{
    AuditId, CommandId, CorrelationId, EventId, ExecutionId, IdempotencyKey, RequestId, TaskId,
};
use aep_domain::time::Timestamp;

use crate::consistency::ConsistencyToken;
use crate::error::CommandError;

/// What directly caused a command.
///
/// Typed as a string here rather than pulling in the audit vocabulary: the contract has to be
/// implementable by a backend that keeps no audit records of its own.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(transparent)]
pub struct CausationRef(pub String);

/// Attribution and causal tracing for one command.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct CommandContext {
    /// One transport attempt. A retry gets a new one.
    pub request_id: RequestId,
    /// Makes the mutation safe to retry.
    pub idempotency_key: IdempotencyKey,
    /// On whose behalf the action occurs.
    pub actor: ActorRef,
    /// What is actually performing it, when that differs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub executor: Option<ActorRef>,
    /// What wider activity this belongs to.
    pub correlation_id: CorrelationId,
    /// What directly caused it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub causation: Option<CausationRef>,
    /// The protocol execution this belongs to, when there is one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution_id: Option<ExecutionId>,
    /// The task being worked on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<TaskId>,
    /// When the caller issued it.
    pub issued_at: Timestamp,
}

impl CommandContext {
    /// A context for a first attempt by `actor`.
    pub fn new(
        request_id: RequestId,
        idempotency_key: IdempotencyKey,
        actor: ActorRef,
        correlation_id: CorrelationId,
        issued_at: Timestamp,
    ) -> Self {
        Self {
            request_id,
            idempotency_key,
            actor,
            executor: None,
            correlation_id,
            causation: None,
            execution_id: None,
            task: None,
            issued_at,
        }
    }

    /// Names what actually ran, builder-style.
    #[must_use]
    pub fn executed_by(mut self, executor: ActorRef) -> Self {
        self.executor = Some(executor);
        self
    }

    /// Names the immediate cause, builder-style.
    #[must_use]
    pub fn caused_by(mut self, causation: CausationRef) -> Self {
        self.causation = Some(causation);
        self
    }

    /// Attaches the protocol execution, builder-style.
    #[must_use]
    pub fn during(mut self, execution: ExecutionId, task: TaskId) -> Self {
        self.execution_id = Some(execution);
        self.task = Some(task);
        self
    }

    /// What ran, falling back to who authorised when nothing else is named.
    pub fn effective_executor(&self) -> &ActorRef {
        self.executor.as_ref().unwrap_or(&self.actor)
    }
}

/// A command, ready to execute.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct CommandEnvelope<C> {
    /// The logical command. A retry reuses this.
    pub command_id: CommandId,
    /// Its versioned type name, such as `aep.design.approve/v1`.
    pub command_type: String,
    /// What it targets.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<EntityRef>,
    /// The revision it asserts. A mismatch is a conflict, never a silent overwrite.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_revision: Option<EntityRevision>,
    /// The command itself.
    pub payload: C,
    /// Attribution and causal tracing.
    pub context: CommandContext,
}

impl<C> CommandEnvelope<C> {
    /// Wraps a payload.
    pub fn new(
        command_id: CommandId,
        command_type: impl Into<String>,
        payload: C,
        context: CommandContext,
    ) -> Self {
        Self {
            command_id,
            command_type: command_type.into(),
            target: None,
            expected_revision: None,
            payload,
            context,
        }
    }

    /// Targets an entity, builder-style.
    #[must_use]
    pub fn targeting(mut self, target: EntityRef) -> Self {
        self.target = Some(target);
        self
    }

    /// Asserts the revision the caller believes it is changing, builder-style.
    #[must_use]
    pub fn expecting(mut self, revision: EntityRevision) -> Self {
        self.expected_revision = Some(revision);
        self
    }

    /// Targets one exact revision, which is both the target and the assertion.
    #[must_use]
    pub fn targeting_revision(mut self, reference: &VersionedEntityRef) -> Self {
        self.target = Some(reference.unversioned());
        self.expected_revision = Some(reference.revision);
        self
    }

    /// `true` when this command asserts a revision, and so cannot silently overwrite newer state.
    pub fn is_revision_guarded(&self) -> bool {
        self.expected_revision.is_some()
    }
}

/// How a command was applied.
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
pub enum CommandOutcome {
    /// Applied for the first time.
    Accepted,
    /// Recognised as a replay of a command already applied; the original result is returned.
    Replayed,
    /// Accepted, and nothing changed — the state already matched.
    NoOp,
}

impl CommandOutcome {
    /// `true` when this call is what changed the state.
    pub fn changed_state(self) -> bool {
        self == Self::Accepted
    }
}

/// What a successful command produced.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct CommandResult {
    /// The logical command this answers.
    pub command_id: CommandId,
    /// How it was applied.
    pub outcome: CommandOutcome,
    /// What changed, at the revisions it changed to.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub affected: Vec<VersionedEntityRef>,
    /// Events it emitted.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub events: Vec<EventId>,
    /// Audit records it produced.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub audit: Vec<AuditId>,
    /// A token a later read can demand, to see this write.
    pub consistency: ConsistencyToken,
}

impl CommandResult {
    /// A result for a command that changed `affected`.
    pub fn accepted(
        command_id: CommandId,
        affected: Vec<VersionedEntityRef>,
        consistency: ConsistencyToken,
    ) -> Self {
        Self {
            command_id,
            outcome: CommandOutcome::Accepted,
            affected,
            events: Vec::new(),
            audit: Vec::new(),
            consistency,
        }
    }

    /// The revision `entity` ended at, if this command touched it.
    pub fn revision_of(&self, entity: &EntityRef) -> Option<EntityRevision> {
        self.affected
            .iter()
            .find(|reference| reference.id == entity.id)
            .map(|reference| reference.revision)
    }
}

/// The one boundary through which state changes.
pub trait CommandService {
    /// The command payload this service accepts.
    type Command;

    /// Executes a command.
    ///
    /// Implementations must:
    ///
    /// * return the **original** result for a replay of an already-applied logical command, rather
    ///   than applying it twice;
    /// * refuse with [`CommandError::RevisionConflict`] when `expected_revision` does not match,
    ///   rather than overwriting;
    /// * leave state unchanged when they return an error — including a refusal, which is still
    ///   expected to be recorded.
    fn execute(
        &self,
        command: CommandEnvelope<Self::Command>,
    ) -> impl std::future::Future<Output = Result<CommandResult, CommandError>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context() -> CommandContext {
        CommandContext::new(
            "req-1".parse().expect("id"),
            "retry-1".parse().expect("key"),
            "human:alice".parse().expect("actor"),
            "corr-42".parse().expect("id"),
            Timestamp::from_epoch_millis(1_700_000_000_000),
        )
    }

    fn entity() -> EntityRef {
        EntityRef::new("01K2R8JD3ZJME72AJGQY67E5F8".parse().expect("id"))
    }

    #[test]
    fn the_executor_defaults_to_the_actor_and_is_recorded_when_it_differs() {
        let plain = context();
        assert_eq!(plain.effective_executor().to_string(), "human:alice");

        let delegated = context().executed_by("agent:release-agent-17".parse().expect("actor"));
        assert_eq!(delegated.actor.to_string(), "human:alice");
        assert_eq!(
            delegated.effective_executor().to_string(),
            "agent:release-agent-17"
        );
    }

    #[test]
    fn targeting_a_revision_sets_both_the_target_and_the_assertion() {
        let pinned = entity().at(EntityRevision::new(7).expect("revision"));
        let envelope = CommandEnvelope::new(
            "cmd-1".parse().expect("id"),
            "aep.design.approve/v1",
            (),
            context(),
        )
        .targeting_revision(&pinned);

        assert_eq!(envelope.target, Some(entity()));
        assert_eq!(
            envelope.expected_revision,
            Some(EntityRevision::new(7).expect("revision"))
        );
        assert!(envelope.is_revision_guarded());
    }

    #[test]
    fn an_unguarded_command_is_visibly_unguarded() {
        let envelope = CommandEnvelope::new(
            "cmd-2".parse().expect("id"),
            "aep.entity.create/v1",
            (),
            context(),
        );
        assert!(
            !envelope.is_revision_guarded(),
            "a create has nothing to overwrite, and says so rather than implying a guard"
        );
    }

    #[test]
    fn a_result_reports_the_revision_each_entity_ended_at() {
        let result = CommandResult::accepted(
            "cmd-1".parse().expect("id"),
            vec![entity().at(EntityRevision::new(8).expect("revision"))],
            ConsistencyToken::new("seq:12").expect("token"),
        );
        assert_eq!(
            result.revision_of(&entity()),
            Some(EntityRevision::new(8).expect("revision"))
        );
        assert!(result.outcome.changed_state());
    }

    #[test]
    fn a_replay_is_not_a_state_change() {
        assert!(!CommandOutcome::Replayed.changed_state());
        assert!(!CommandOutcome::NoOp.changed_state());
        assert!(CommandOutcome::Accepted.changed_state());
    }

    #[test]
    fn a_context_round_trips_through_serde_with_its_causal_fields() {
        let context = context()
            .executed_by("agent:opus-5".parse().expect("actor"))
            .caused_by(CausationRef("cmd-0".to_owned()))
            .during(
                "exec-1".parse().expect("id"),
                "AUTH-142".parse().expect("task"),
            );
        let json = serde_json::to_value(&context).expect("serialises");
        assert_eq!(json["actor"], "human:alice");
        assert_eq!(json["executor"], "agent:opus-5");
        assert_eq!(json["correlation_id"], "corr-42");
        assert_eq!(json["task"], "AUTH-142");

        let parsed: CommandContext = serde_json::from_value(json).expect("deserialises");
        assert_eq!(parsed, context);
    }
}
