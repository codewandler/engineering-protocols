//! The audit trail.
//!
//! Audit is a cross-cutting concern rather than a feature of one subsystem, and it is not logging:
//! a log line is written for whoever is watching now, an audit record is written for whoever asks
//! six months from now. The question that reader has is never "what happened" — that is in the
//! event stream. It is:
//!
//! ```text
//! who authorised this
//! what actually ran
//! why was it allowed
//! what would have stopped it
//! ```
//!
//! # Denied actions are the interesting ones
//!
//! A refused action must not mutate domain state, and must still be observable. `agent attempted
//! production.write, policy denied it` is the record that matters during a security review, an
//! incident post-mortem or an access audit — and it is exactly the record a system that only
//! writes on success does not have. So a refusal produces a full [`AuditRecord`] carrying a
//! [`DecisionRecord`] and no [`ChangeRecord`], and [`AuditRecord::validate`] refuses a record that
//! claims both.
//!
//! # Actor is not executor
//!
//! ```text
//! actor      human:alice              on whose behalf, who bears responsibility
//! executor   agent:release-agent-17   what actually ran
//! approver   human:bob                who unblocked it
//! ```
//!
//! A trail that collapses these into one "user" field can answer neither question: it cannot say
//! whether a person or an agent performed a production change, and it cannot say who is
//! accountable when an agent did. [`AuditRecord`] keeps `actor` and `executor` apart; the approver
//! arrives as a separate `ApprovalGranted` record linked by correlation.
//!
//! # What each field is there to answer
//!
//! | question | field |
//! |---|---|
//! | who did it | `actor` |
//! | what executed it | `executor`, `execution_id` |
//! | what changed | `change` |
//! | when | `occurred_at` |
//! | why was it allowed | `decision` |
//! | which activity | `correlation_id`, `task` |
//! | what directly caused it | `causation` |
//! | which revisions | `change.before`, `change.after` |
//! | what justified it | `evidence` |
//! | was it rejected | `kind`, `decision.allowed` |
//!
//! # Why these types deserialize
//!
//! Validated *documents* in this crate deliberately do not implement
//! [`Deserialize`](serde::Deserialize), because a document authorises future behaviour and must be
//! validated to exist. An audit record authorises nothing: it is a report of something that
//! already happened, it crosses a process boundary on the way to storage and comes back out to be
//! read. [`AuditRecord::validate`] is therefore a producer-side conscience check, run before a
//! record is written, not a gate standing between the wire and the type.

use std::fmt;

use crate::capability::{Capability, CapabilityDecision, PolicySource};
use crate::entity::{ActorRef, EntityRef, EntityRevision};
use crate::error::{ValidationCode, ValidationError, ValidationErrors};
use crate::ids::{
    ApprovalId, AuditId, CommandId, CorrelationId, EventId, ExecutionId, RequestId, TaskId,
};
use crate::node::Node;
use crate::time::Timestamp;

/// What sort of thing an audit record reports.
///
/// The vocabulary is logical, not physical: an implementation may collapse or expand the records
/// it actually stores as long as every kind here stays queryable.
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
#[non_exhaustive]
pub enum AuditKind {
    /// A command was submitted.
    CommandAttempted,
    /// A command passed authorisation and validation.
    CommandAccepted,
    /// A command was refused; nothing changed.
    CommandRejected,
    /// An entity came into existence.
    EntityCreated,
    /// An entity changed.
    EntityUpdated,
    /// An entity was archived; it remains addressable.
    EntityArchived,
    /// An entity was replaced by a newer one.
    EntitySuperseded,
    /// A relation between two entities was created.
    RelationCreated,
    /// A relation between two entities was removed.
    RelationRemoved,
    /// The protocol engine decided whether something was permitted.
    ProtocolDecision,
    /// An approval was asked for.
    ApprovalRequested,
    /// An approval was given.
    ApprovalGranted,
    /// An approval was refused.
    ApprovalDenied,
    /// A workflow transition was taken.
    TransitionPerformed,
    /// A workflow transition was evaluated and not permitted.
    TransitionBlocked,
    /// Evidence was recorded against a subject.
    EvidenceRecorded,
    /// A verifier finished and reported an outcome.
    VerificationCompleted,
}

impl AuditKind {
    /// Every kind, for vocabulary listing and exhaustive tests.
    pub const ALL: &'static [Self] = &[
        Self::CommandAttempted,
        Self::CommandAccepted,
        Self::CommandRejected,
        Self::EntityCreated,
        Self::EntityUpdated,
        Self::EntityArchived,
        Self::EntitySuperseded,
        Self::RelationCreated,
        Self::RelationRemoved,
        Self::ProtocolDecision,
        Self::ApprovalRequested,
        Self::ApprovalGranted,
        Self::ApprovalDenied,
        Self::TransitionPerformed,
        Self::TransitionBlocked,
        Self::EvidenceRecorded,
        Self::VerificationCompleted,
    ];

    /// The kind as it appears in output, such as `command_rejected`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CommandAttempted => "command_attempted",
            Self::CommandAccepted => "command_accepted",
            Self::CommandRejected => "command_rejected",
            Self::EntityCreated => "entity_created",
            Self::EntityUpdated => "entity_updated",
            Self::EntityArchived => "entity_archived",
            Self::EntitySuperseded => "entity_superseded",
            Self::RelationCreated => "relation_created",
            Self::RelationRemoved => "relation_removed",
            Self::ProtocolDecision => "protocol_decision",
            Self::ApprovalRequested => "approval_requested",
            Self::ApprovalGranted => "approval_granted",
            Self::ApprovalDenied => "approval_denied",
            Self::TransitionPerformed => "transition_performed",
            Self::TransitionBlocked => "transition_blocked",
            Self::EvidenceRecorded => "evidence_recorded",
            Self::VerificationCompleted => "verification_completed",
        }
    }

    /// `true` when this kind reports something the system refused to do.
    ///
    /// A refusal is the one shape where the absence of a change is part of the claim, so it is
    /// worth asking about by name rather than by inspecting fields.
    pub fn is_refusal(self) -> bool {
        matches!(
            self,
            Self::CommandRejected | Self::ApprovalDenied | Self::TransitionBlocked
        )
    }

    /// `true` when this kind asserts that domain state changed, and therefore owes a
    /// [`ChangeRecord`].
    ///
    /// A row claiming `entity_updated` without saying which revisions bracket the change cannot
    /// answer "what changed", which is the whole point of recording it.
    pub fn records_a_mutation(self) -> bool {
        matches!(
            self,
            Self::EntityCreated
                | Self::EntityUpdated
                | Self::EntityArchived
                | Self::EntitySuperseded
        )
    }
}

impl fmt::Display for AuditKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What directly caused this record.
///
/// Causation and correlation answer different questions and are both needed. Correlation says
/// *what belongs together* — one activity, one id, however many records. Causation says *what
/// produced this one*, and chains: a command attempt causes a capability decision, the decision
/// causes a denial, the denial causes an approval request. With only correlation the chain is a
/// bag; with only causation there is no way to ask for the whole activity.
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
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CausationRef {
    /// A command caused this.
    Command {
        /// Which command.
        command: CommandId,
    },
    /// An event caused this.
    Event {
        /// Which event.
        event: EventId,
    },
    /// An earlier decision caused this, named by the audit record that carries it.
    Decision {
        /// Which audit record.
        decision: AuditId,
    },
    /// An approval caused this.
    Approval {
        /// Which approval.
        approval: ApprovalId,
    },
}

impl CausationRef {
    /// The cause's kind as it appears in output, matching the `kind` tag.
    pub fn kind_name(&self) -> &'static str {
        match self {
            Self::Command { .. } => "command",
            Self::Event { .. } => "event",
            Self::Decision { .. } => "decision",
            Self::Approval { .. } => "approval",
        }
    }
}

impl fmt::Display for CausationRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Command { command } => write!(f, "command {command}"),
            Self::Event { event } => write!(f, "event {event}"),
            Self::Decision { decision } => write!(f, "decision {decision}"),
            Self::Approval { approval } => write!(f, "approval {approval}"),
        }
    }
}

/// Why the protocol allowed or refused something, in a form a person can read back.
///
/// This is the shape a refusal takes in the trail. "Denied" on its own is unusable six months
/// later: the reader needs the operation that was attempted, the rule that refused it, what was
/// missing, and the state the execution was in — because the same operation is often allowed one
/// state later, and the record has to make that visible.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
pub struct DecisionRecord {
    /// Whether the action was permitted.
    pub allowed: bool,
    /// What was attempted, such as `production.write`.
    pub operation: String,
    /// The capability it needed, when the decision was a capability decision.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capability: Option<Capability>,
    /// What the resolved policy said about that capability.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decision: Option<CapabilityDecision>,
    /// Which document decided it.
    ///
    /// This also carries "which principle refused this":
    /// [`PolicySource::Principle`] names it, so the
    /// record does not need a separate principle field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<PolicySource>,
    /// The named rule that decided it, such as `production-write-requires-approval`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule: Option<String>,
    /// What was missing, such as `approval:production-change`.
    ///
    /// This is the "what would have stopped it" half of the record: it tells a reader what to
    /// obtain in order to make the same attempt succeed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub missing: Vec<String>,
    /// The workflow state the execution was in when the decision was made.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
}

impl DecisionRecord {
    /// A decision that permitted `operation`.
    pub fn allow(operation: impl Into<String>) -> Self {
        Self::new(true, operation)
    }

    /// A decision that refused `operation`.
    pub fn deny(operation: impl Into<String>) -> Self {
        Self::new(false, operation)
    }

    fn new(allowed: bool, operation: impl Into<String>) -> Self {
        Self {
            allowed,
            operation: operation.into(),
            capability: None,
            decision: None,
            source: None,
            rule: None,
            missing: Vec::new(),
            state: None,
        }
    }

    /// Names the capability that was evaluated and what the policy said about it.
    #[must_use]
    pub fn about(mut self, capability: Capability, decision: CapabilityDecision) -> Self {
        self.capability = Some(capability);
        self.decision = Some(decision);
        self
    }

    /// Names the document and the rule that decided it.
    #[must_use]
    pub fn by(mut self, source: PolicySource, rule: impl Into<String>) -> Self {
        self.source = Some(source);
        self.rule = Some(rule.into());
        self
    }

    /// Records what was missing.
    #[must_use]
    pub fn missing(mut self, missing: impl IntoIterator<Item = String>) -> Self {
        self.missing = missing.into_iter().collect();
        self
    }

    /// Records the workflow state the decision was made in.
    #[must_use]
    pub fn in_state(mut self, state: impl Into<String>) -> Self {
        self.state = Some(state.into());
        self
    }
}

impl fmt::Display for DecisionRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let verdict = if self.allowed { "allowed" } else { "denied" };
        write!(f, "{} {verdict}", self.operation)?;
        if let Some(rule) = &self.rule {
            write!(f, " by {rule}")?;
        }
        if !self.missing.is_empty() {
            write!(f, "; missing {}", self.missing.join(", "))?;
        }
        Ok(())
    }
}

/// What changed, in a form the mutation can be reconstructed from.
///
/// The logical contract is only that "what changed" be recoverable: a backend may hold full
/// revisions, patches, events or snapshots. What the record must carry is the entity, the
/// revisions on either side of the change, and the command that caused it, so that a reader can
/// go and fetch either side.
///
/// # Redaction
///
/// A payload may contain a secret, a customer's data or a credential, and auditability must not
/// require storing any of it. So the payload is droppable — but nothing else is. Attribution
/// (`actor`, `executor`), causality (`correlation_id`, `causation`), the entity and its revisions
/// and the reason for redaction all survive, because they are what the audit is *for*: after
/// redaction the trail can still say who did what to which entity and why the content is absent.
/// A redaction that took the actor with it would answer nothing, and would make redaction a way to
/// hide rather than a way to protect.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
pub struct ChangeRecord {
    /// Which entity changed.
    pub entity: EntityRef,
    /// The revision before the change; absent when the entity did not exist yet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<EntityRevision>,
    /// The revision after the change; absent when the entity no longer has a current revision.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<EntityRevision>,
    /// The name of the command that caused it, such as `design.approve`.
    ///
    /// Held as a name rather than a typed command so that the audit model does not depend on the
    /// command vocabulary, which changes far more often than the audit trail's readers do.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    /// The command payload, when it may be stored.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payload: Option<Node>,
    /// Whether the payload was withheld.
    ///
    /// Distinct from `payload: None`, which means there was nothing to record. This says there
    /// *was* a payload and it is deliberately not here.
    #[serde(default)]
    pub redacted: bool,
    /// Why the payload was withheld, such as `contains-credential`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub redaction_reason: Option<String>,
}

impl ChangeRecord {
    /// An entity came into existence at `after`.
    pub fn created(entity: EntityRef, after: EntityRevision) -> Self {
        Self::new(entity, None, Some(after))
    }

    /// An entity moved from `before` to `after`.
    pub fn updated(entity: EntityRef, before: EntityRevision, after: EntityRevision) -> Self {
        Self::new(entity, Some(before), Some(after))
    }

    /// An entity stopped having a current revision, having last been at `before`.
    ///
    /// Archived or superseded, never physically deleted: the entity remains addressable, and the
    /// revisions before this one remain fetchable.
    pub fn removed(entity: EntityRef, before: EntityRevision) -> Self {
        Self::new(entity, Some(before), None)
    }

    fn new(
        entity: EntityRef,
        before: Option<EntityRevision>,
        after: Option<EntityRevision>,
    ) -> Self {
        Self {
            entity,
            before,
            after,
            command: None,
            payload: None,
            redacted: false,
            redaction_reason: None,
        }
    }

    /// Names the command that caused the change.
    #[must_use]
    pub fn by_command(mut self, command: impl Into<String>) -> Self {
        self.command = Some(command.into());
        self
    }

    /// Attaches the command payload.
    #[must_use]
    pub fn with_payload(mut self, payload: Node) -> Self {
        self.payload = Some(payload);
        self
    }

    /// Withholds the payload, recording that it existed and why it is absent.
    ///
    /// Everything that attributes the change survives this call; only the payload is dropped.
    #[must_use]
    pub fn redact(mut self, reason: impl Into<String>) -> Self {
        self.payload = None;
        self.redacted = true;
        self.redaction_reason = Some(reason.into());
        self
    }

    /// `true` when this change brought the entity into existence.
    pub fn is_creation(&self) -> bool {
        self.before.is_none() && self.after.is_some()
    }

    /// `true` when this change left the entity without a current revision.
    ///
    /// Archival or supersession, not a physical delete: there is no universal physical delete in
    /// the protocol.
    pub fn is_removal(&self) -> bool {
        self.before.is_some() && self.after.is_none()
    }
}

/// One immutable record in the audit trail.
///
/// Records are never updated in place. A correction is a new record that names the one it
/// corrects through its [`CausationRef`], which is what makes the trail evidence rather than a
/// mutable summary of someone's current opinion of the past.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
pub struct AuditRecord {
    /// This record's identity.
    pub audit_id: AuditId,
    /// What sort of thing it reports.
    pub kind: AuditKind,
    /// When it happened.
    pub occurred_at: Timestamp,
    /// On whose behalf it happened, and who bears responsibility.
    pub actor: ActorRef,
    /// What actually ran, when that differs from who authorised it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub executor: Option<ActorRef>,
    /// What it was about.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject: Option<EntityRef>,
    /// The transport attempt it arrived on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_id: Option<RequestId>,
    /// The logical command it belongs to; retries of one intent share it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command_id: Option<CommandId>,
    /// The domain event it belongs to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub event_id: Option<EventId>,
    /// Which broader activity it belongs to.
    ///
    /// Mandatory, unlike everything else that links records together: a record nobody can reach
    /// from the activity it belongs to is a record nobody will ever read.
    pub correlation_id: CorrelationId,
    /// What directly caused it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub causation: Option<CausationRef>,
    /// The protocol execution that was active.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution_id: Option<ExecutionId>,
    /// The task the work belongs to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<TaskId>,
    /// Why it was allowed or refused.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decision: Option<DecisionRecord>,
    /// What changed. Absent whenever nothing did — in particular on every refusal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub change: Option<ChangeRecord>,
    /// The evidence or approvals that justified it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence: Vec<EntityRef>,
}

impl AuditRecord {
    /// A record with only the fields every record must have.
    pub fn new(
        audit_id: AuditId,
        kind: AuditKind,
        occurred_at: Timestamp,
        actor: ActorRef,
        correlation_id: CorrelationId,
    ) -> Self {
        Self {
            audit_id,
            kind,
            occurred_at,
            actor,
            executor: None,
            subject: None,
            request_id: None,
            command_id: None,
            event_id: None,
            correlation_id,
            causation: None,
            execution_id: None,
            task: None,
            decision: None,
            change: None,
            evidence: Vec::new(),
        }
    }

    /// A command was refused: the attempt, who made it, and why it was refused.
    ///
    /// There is deliberately no way to attach a [`ChangeRecord`] while building one of these, and
    /// [`Self::validate`] refuses one that acquired a change later.
    pub fn command_rejected(
        audit_id: AuditId,
        occurred_at: Timestamp,
        actor: ActorRef,
        correlation_id: CorrelationId,
        command_id: CommandId,
        decision: DecisionRecord,
    ) -> Self {
        let mut record = Self::new(
            audit_id,
            AuditKind::CommandRejected,
            occurred_at,
            actor,
            correlation_id,
        );
        record.causation = Some(CausationRef::Command {
            command: command_id.clone(),
        });
        record.command_id = Some(command_id);
        record.decision = Some(decision);
        record
    }

    /// The protocol engine decided something, allowed or refused.
    pub fn protocol_decision(
        audit_id: AuditId,
        occurred_at: Timestamp,
        actor: ActorRef,
        correlation_id: CorrelationId,
        decision: DecisionRecord,
    ) -> Self {
        let mut record = Self::new(
            audit_id,
            AuditKind::ProtocolDecision,
            occurred_at,
            actor,
            correlation_id,
        );
        record.decision = Some(decision);
        record
    }

    /// An entity changed. The subject is taken from the change, so the two cannot disagree.
    pub fn entity_changed(
        audit_id: AuditId,
        kind: AuditKind,
        occurred_at: Timestamp,
        actor: ActorRef,
        correlation_id: CorrelationId,
        change: ChangeRecord,
    ) -> Self {
        let mut record = Self::new(audit_id, kind, occurred_at, actor, correlation_id);
        record.subject = Some(change.entity.clone());
        record.change = Some(change);
        record
    }

    /// Records what actually ran, when that is not the actor.
    #[must_use]
    pub fn with_executor(mut self, executor: ActorRef) -> Self {
        self.executor = Some(executor);
        self
    }

    /// Records what this was about, when it is not already implied by a change.
    #[must_use]
    pub fn with_subject(mut self, subject: EntityRef) -> Self {
        self.subject = Some(subject);
        self
    }

    /// Records the transport attempt this arrived on.
    #[must_use]
    pub fn with_request(mut self, request_id: RequestId) -> Self {
        self.request_id = Some(request_id);
        self
    }

    /// Records what directly caused this.
    #[must_use]
    pub fn caused_by(mut self, causation: CausationRef) -> Self {
        self.causation = Some(causation);
        self
    }

    /// Records the protocol execution and task that were active.
    #[must_use]
    pub fn during(mut self, execution_id: ExecutionId, task: TaskId) -> Self {
        self.execution_id = Some(execution_id);
        self.task = Some(task);
        self
    }

    /// Adds a piece of evidence or an approval that justified this.
    #[must_use]
    pub fn with_evidence(mut self, evidence: EntityRef) -> Self {
        self.evidence.push(evidence);
        self
    }

    /// `true` when this record reports something the system refused to do.
    ///
    /// True either because the kind says so or because the decision does; a record whose kind and
    /// decision disagree is rejected by [`Self::validate`] rather than silently resolved here.
    pub fn is_rejection(&self) -> bool {
        self.kind.is_refusal()
            || self
                .decision
                .as_ref()
                .is_some_and(|decision| !decision.allowed)
    }

    /// `true` when domain state actually changed.
    pub fn mutated(&self) -> bool {
        self.change.is_some()
    }

    /// Checks the record against the audit model's own rules.
    ///
    /// The rule worth naming: **a refused action must not mutate state, and must still be
    /// recorded**. Everything else here follows from records having to remain readable long after
    /// the code that wrote them was replaced.
    ///
    /// Validation accumulates — a record with three problems reports three errors. Every failure
    /// carries the code that names the specific inconsistency, so a caller can branch on
    /// their own, and inventing one here would fork the shared code vocabulary.
    pub fn validate(&self) -> Result<(), ValidationErrors> {
        let mut errors = ValidationErrors::new();

        if self.is_rejection() {
            if let Some(change) = &self.change {
                errors.push(
                    ValidationError::new(
                        ValidationCode::RefusalMutatedState,
                        "audit.change",
                        format!(
                            "a {} record carries a change to entity {}, but a refused action must \
                             not mutate state",
                            self.kind, change.entity
                        ),
                    )
                    .with_hint("record the attempt and its decision, and leave `change` unset"),
                );
            }
        }

        if let Some(decision) = &self.decision {
            if self.kind.is_refusal() && decision.allowed {
                errors.push(ValidationError::new(
                    ValidationCode::RefusalMutatedState,
                    "audit.decision.allowed",
                    format!(
                        "a {} record carries a decision that says {} was allowed",
                        self.kind, decision.operation
                    ),
                ));
            }
        } else if self.kind == AuditKind::ProtocolDecision {
            errors.push(
                ValidationError::new(
                    ValidationCode::UnexplainedDecision,
                    "audit.decision",
                    "a protocol_decision record has no decision, so it explains nothing".to_owned(),
                )
                .with_hint("attach the DecisionRecord the engine produced"),
            );
        }

        match &self.change {
            Some(change) => validate_change(change, &mut errors),
            None if self.kind.records_a_mutation() => {
                errors.push(ValidationError::new(
                    ValidationCode::UnreconstructableChange,
                    "audit.change",
                    format!(
                        "a {} record has no change, so `what changed` cannot be reconstructed",
                        self.kind
                    ),
                ));
            }
            None => {}
        }

        errors.into_result(())
    }
}

/// Rules a [`ChangeRecord`] must satisfy, pushed into `errors` rather than returned.
fn validate_change(change: &ChangeRecord, errors: &mut ValidationErrors) {
    if change.before.is_none() && change.after.is_none() {
        errors.push(
            ValidationError::new(
                ValidationCode::UnreconstructableChange,
                "audit.change.after",
                format!(
                    "the change to entity {} names neither a before nor an after revision, so \
                     nothing about it is reconstructable",
                    change.entity
                ),
            )
            .with_hint("a creation names `after`, a removal names `before`, an update names both"),
        );
    }

    if change.redacted && change.payload.is_some() {
        errors.push(ValidationError::new(
            ValidationCode::RedactionInconsistent,
            "audit.change.payload",
            "the change is marked redacted but still carries its payload".to_owned(),
        ));
    }

    if !change.redacted && change.redaction_reason.is_some() {
        errors.push(ValidationError::new(
            ValidationCode::RedactionInconsistent,
            "audit.change.redaction_reason",
            "the change carries a redaction reason but is not marked redacted".to_owned(),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capability::Environment;
    use crate::entity::EntityId;
    use crate::ids::ProfileId;

    fn audit_id() -> AuditId {
        AuditId::new("audit-1").expect("audit id")
    }

    fn correlation() -> CorrelationId {
        CorrelationId::new("corr-42").expect("correlation id")
    }

    fn alice() -> ActorRef {
        ActorRef::parse("human:alice").expect("actor")
    }

    fn release_agent() -> ActorRef {
        ActorRef::parse("agent:release-agent-17").expect("executor")
    }

    fn entity() -> EntityRef {
        EntityRef::new(EntityId::new("01K2R8JD3ZJME72AJGQY67E5F8").expect("entity id"))
    }

    fn at() -> Timestamp {
        Timestamp::from_epoch_millis(1_700_000_000_000)
    }

    fn revision(value: u64) -> EntityRevision {
        EntityRevision::new(value).expect("revision")
    }

    fn denial() -> DecisionRecord {
        DecisionRecord::deny("production.write")
            .about(Capability::ProductionWrite, CapabilityDecision::Denied)
            .by(
                PolicySource::Profile {
                    profile: ProfileId::new("release.standard").expect("profile"),
                },
                "production-write-requires-approval",
            )
            .missing(["approval:production-change".to_owned()])
            .in_state("incident.diagnose")
    }

    fn rejection() -> AuditRecord {
        AuditRecord::command_rejected(
            audit_id(),
            at(),
            alice(),
            correlation(),
            CommandId::new("cmd-9").expect("command id"),
            denial(),
        )
    }

    #[test]
    fn a_refused_command_records_why_and_changes_nothing() {
        let record = rejection();

        assert!(record.is_rejection());
        assert!(!record.mutated());
        assert!(record.change.is_none());
        let decision = record.decision.as_ref().expect("a refusal explains itself");
        assert!(!decision.allowed);
        assert_eq!(decision.operation, "production.write");
        assert_eq!(decision.missing, ["approval:production-change"]);
        assert_eq!(decision.state.as_deref(), Some("incident.diagnose"));
        record.validate().expect("a plain refusal is valid");
    }

    #[test]
    fn a_refused_command_that_carries_a_change_is_rejected() {
        let mut record = rejection();
        record.change = Some(ChangeRecord::updated(entity(), revision(3), revision(4)));

        let errors = record.validate().expect_err("a denial must not mutate");
        assert_eq!(errors.len(), 1);
        let error = &errors.as_slice()[0];
        assert_eq!(error.code, ValidationCode::RefusalMutatedState);
        assert_eq!(error.location, "audit.change");
        assert!(
            error.message.contains("must not mutate state"),
            "unexpected message: {}",
            error.message
        );
    }

    #[test]
    fn a_record_whose_decision_refused_the_action_may_not_carry_a_change() {
        // `is_rejection` is true either because the kind says so or because the decision does,
        // and every other fixture here is built from `rejection()`, whose kind is already a
        // refusal. This is the other disjunct: a kind that claims a mutation, carrying a decision
        // that says the action was refused. It is the record a security reviewer must never find
        // — "the policy refused this" beside the rows it changed.
        let mut record = AuditRecord::entity_changed(
            audit_id(),
            AuditKind::EntityUpdated,
            at(),
            alice(),
            correlation(),
            ChangeRecord::updated(entity(), revision(3), revision(4)),
        );
        record.decision = Some(denial());

        assert!(
            !record.kind.is_refusal(),
            "the fixture's kind must not be a refusal on its own, or the decision-based half of \
             the rule is not what is being tested"
        );
        assert!(
            record.is_rejection(),
            "a record carrying a decision that was not allowed is a refusal"
        );

        let errors = record.validate().expect_err(
            "a refused action must not mutate state, whichever field says it was refused",
        );
        assert_eq!(errors.len(), 1, "{errors}");
        let error = &errors.as_slice()[0];
        assert_eq!(error.code, ValidationCode::RefusalMutatedState);
        assert_eq!(error.location, "audit.change");
    }

    #[test]
    fn a_refusal_whose_decision_says_allowed_is_rejected() {
        let mut record = rejection();
        record.decision = Some(DecisionRecord::allow("production.write"));

        let errors = record.validate().expect_err("kind and decision disagree");
        assert_eq!(errors.len(), 1);
        assert_eq!(errors.as_slice()[0].location, "audit.decision.allowed");
        assert!(errors.contains(ValidationCode::RefusalMutatedState));
    }

    #[test]
    fn a_change_record_round_trips_with_both_revisions() {
        let change = ChangeRecord::updated(entity(), revision(6), revision(7))
            .by_command("design.approve")
            .with_payload(Node::Text("approved".to_owned()));

        let json = serde_json::to_value(&change).expect("serialises");
        assert_eq!(json["before"], 6);
        assert_eq!(json["after"], 7);
        assert_eq!(json["command"], "design.approve");
        assert_eq!(json["redacted"], false);

        let restored: ChangeRecord = serde_json::from_value(json).expect("deserialises");
        assert_eq!(restored, change);
        assert!(!restored.is_creation());
        assert!(!restored.is_removal());
    }

    #[test]
    fn a_redacted_payload_keeps_the_actor_correlation_and_reason() {
        let change = ChangeRecord::updated(entity(), revision(1), revision(2))
            .by_command("service.rotate-credential")
            .with_payload(Node::Text("hunter2".to_owned()))
            .redact("contains-credential");
        let record = AuditRecord::entity_changed(
            audit_id(),
            AuditKind::EntityUpdated,
            at(),
            alice(),
            correlation(),
            change,
        )
        .with_executor(release_agent());

        record.validate().expect("redaction is valid");
        let json = serde_json::to_value(&record).expect("serialises");
        assert!(json["change"].get("payload").is_none());
        assert_eq!(json["change"]["redacted"], true);
        assert_eq!(json["change"]["redaction_reason"], "contains-credential");
        assert_eq!(json["change"]["command"], "service.rotate-credential");
        assert_eq!(json["change"]["before"], 1);
        assert_eq!(json["actor"], "human:alice");
        assert_eq!(json["executor"], "agent:release-agent-17");
        assert_eq!(json["correlation_id"], "corr-42");
        assert!(
            !serde_json::to_string(&record)
                .expect("serialises")
                .contains("hunter2"),
            "a redacted payload must not survive anywhere in the record"
        );
    }

    #[test]
    fn a_redacted_change_that_kept_its_payload_is_rejected() {
        let mut change = ChangeRecord::updated(entity(), revision(1), revision(2));
        change.redacted = true;
        change.payload = Some(Node::Text("hunter2".to_owned()));
        let record = AuditRecord::entity_changed(
            audit_id(),
            AuditKind::EntityUpdated,
            at(),
            alice(),
            correlation(),
            change,
        );

        let errors = record
            .validate()
            .expect_err("redaction must remove payload");
        assert_eq!(errors.len(), 1);
        assert_eq!(errors.as_slice()[0].location, "audit.change.payload");
        assert_eq!(
            errors.as_slice()[0].code,
            ValidationCode::RedactionInconsistent
        );
    }

    #[test]
    fn causation_serialises_with_its_kind_tag() {
        let causation = CausationRef::Decision {
            decision: AuditId::new("audit-0").expect("audit id"),
        };
        let json = serde_json::to_value(&causation).expect("serialises");
        assert_eq!(json["kind"], "decision");
        assert_eq!(json["decision"], "audit-0");
        assert_eq!(causation.kind_name(), "decision");
        assert_eq!(causation.to_string(), "decision audit-0");

        let restored: CausationRef = serde_json::from_value(json).expect("deserialises");
        assert_eq!(restored, causation);
    }

    #[test]
    fn a_change_without_a_before_revision_is_a_creation() {
        let change = ChangeRecord::created(entity(), EntityRevision::INITIAL);
        assert!(change.is_creation());
        assert!(!change.is_removal());
    }

    #[test]
    fn a_change_without_an_after_revision_is_a_removal() {
        let change = ChangeRecord::removed(entity(), revision(4));
        assert!(change.is_removal());
        assert!(!change.is_creation());
    }

    #[test]
    fn a_change_that_names_neither_revision_is_rejected() {
        let mut change = ChangeRecord::updated(entity(), revision(1), revision(2));
        change.before = None;
        change.after = None;
        let record = AuditRecord::entity_changed(
            audit_id(),
            AuditKind::EntityArchived,
            at(),
            alice(),
            correlation(),
            change,
        );

        let errors = record.validate().expect_err("nothing is reconstructable");
        assert_eq!(errors.len(), 1);
        assert_eq!(errors.as_slice()[0].location, "audit.change.after");
        assert!(errors.as_slice()[0].message.contains("neither a before"));
    }

    #[test]
    fn correlation_is_mandatory_and_causation_is_optional() {
        let record = AuditRecord::new(
            audit_id(),
            AuditKind::EvidenceRecorded,
            at(),
            alice(),
            correlation(),
        );

        assert!(record.causation.is_none());
        record
            .validate()
            .expect("a record without causation is fine");
        let json = serde_json::to_value(&record).expect("serialises");
        assert_eq!(json["correlation_id"], "corr-42");
        assert!(json.get("causation").is_none());
        assert!(json.get("executor").is_none());

        let caused = record.caused_by(CausationRef::Event {
            event: EventId::new("evt-3").expect("event id"),
        });
        assert_eq!(
            serde_json::to_value(&caused).expect("serialises")["causation"]["kind"],
            "event"
        );
    }

    #[test]
    fn an_entity_change_record_without_a_change_is_rejected() {
        let record = AuditRecord::new(
            audit_id(),
            AuditKind::EntityUpdated,
            at(),
            alice(),
            correlation(),
        );

        let errors = record.validate().expect_err("what changed is missing");
        assert_eq!(errors.len(), 1);
        assert_eq!(errors.as_slice()[0].location, "audit.change");
        assert!(errors.as_slice()[0]
            .message
            .contains("cannot be reconstructed"));
    }

    #[test]
    fn a_protocol_decision_without_a_decision_is_rejected() {
        let record = AuditRecord::new(
            audit_id(),
            AuditKind::ProtocolDecision,
            at(),
            alice(),
            correlation(),
        );

        let errors = record.validate().expect_err("it explains nothing");
        assert_eq!(errors.len(), 1);
        assert_eq!(errors.as_slice()[0].location, "audit.decision");
        assert_eq!(
            errors.as_slice()[0].hint.as_deref(),
            Some("attach the DecisionRecord the engine produced")
        );
    }

    #[test]
    fn validation_reports_every_problem_at_once() {
        let mut record = rejection();
        let mut change = ChangeRecord::updated(entity(), revision(1), revision(2));
        change.before = None;
        change.after = None;
        change.redaction_reason = Some("contains-credential".to_owned());
        record.change = Some(change);

        let errors = record.validate().expect_err("three problems");
        let locations: Vec<&str> = errors
            .as_slice()
            .iter()
            .map(|error| error.location.as_str())
            .collect();
        assert_eq!(
            locations,
            [
                "audit.change",
                "audit.change.after",
                "audit.change.redaction_reason"
            ]
        );
    }

    #[test]
    fn the_actor_and_the_executor_stay_distinct() {
        let record = AuditRecord::protocol_decision(
            audit_id(),
            at(),
            alice(),
            correlation(),
            DecisionRecord::allow("deployment.create")
                .about(
                    Capability::Deploy(Environment::Production),
                    CapabilityDecision::Allowed,
                )
                .in_state("release.deploy"),
        )
        .with_executor(release_agent())
        .during(
            ExecutionId::new("exec-7").expect("execution id"),
            TaskId::new("AUTH-142").expect("task id"),
        )
        .with_evidence(entity());

        record.validate().expect("an allowed decision is valid");
        assert!(!record.is_rejection());
        assert!(record.actor.is_human());
        assert!(record.executor.as_ref().expect("executor").is_agent());
        assert_eq!(
            record.executor.as_ref().expect("executor").name(),
            "release-agent-17"
        );
        assert_eq!(record.evidence.len(), 1);

        let restored: AuditRecord =
            serde_json::from_value(serde_json::to_value(&record).expect("serialises"))
                .expect("deserialises");
        assert_eq!(restored, record);
    }

    #[test]
    fn every_audit_kind_round_trips_through_its_snake_case_name() {
        for kind in AuditKind::ALL {
            let json = serde_json::to_value(kind).expect("serialises");
            assert_eq!(json, serde_json::Value::String(kind.as_str().to_owned()));
            assert_eq!(kind.to_string(), kind.as_str());
            let restored: AuditKind = serde_json::from_value(json).expect("deserialises");
            assert_eq!(restored, *kind);
        }
        assert_eq!(AuditKind::ALL.len(), 17);
        assert!(AuditKind::TransitionBlocked.is_refusal());
        assert!(!AuditKind::TransitionPerformed.is_refusal());
        assert!(AuditKind::EntitySuperseded.records_a_mutation());
        assert!(!AuditKind::EvidenceRecorded.records_a_mutation());
    }
}
