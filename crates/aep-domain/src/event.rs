//! The audit event vocabulary.
//!
//! Every meaningful decision an execution makes is representable as one append-only record.
//! AEP defines what the events mean and deliberately does not define where they are stored: a
//! harness may keep them in memory, write them to a file, or ship them to an event store.
//!
//! What the stream is for: reconstructing, months later, why an agent was allowed to do
//! something — or why it was stopped — without depending on anyone's memory of the session.

use crate::capability::{Capability, CapabilityDecision};
use crate::evidence::{EvidenceKind, Producer};
use crate::ids::{
    ApprovalId, EvidenceId, ExecutionId, ObligationId, PrincipleId, ProfileId, StateId, TaskId,
    WorkflowId,
};
use crate::task::TaskKind;
use crate::time::Timestamp;
use crate::verification::{VerificationStatus, Verifier};
use crate::version::ProtocolRef;

/// Something that happened during an execution.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(tag = "event", rename_all = "snake_case")]
#[non_exhaustive]
pub enum ProtocolEvent {
    /// A task was accepted.
    TaskCreated {
        /// Which task.
        task: TaskId,
        /// What sort of work it is.
        kind: TaskKind,
    },
    /// A task was resolved into an execution plan.
    ProtocolResolved {
        /// The protocol version.
        protocol: ProtocolRef,
        /// The profile.
        profile: ProfileId,
        /// The workflow.
        workflow: WorkflowId,
        /// The principles in force.
        principles: Vec<PrincipleId>,
        /// Principles dropped by the profile or the task.
        dropped_principles: Vec<PrincipleId>,
    },
    /// Execution entered a state.
    StateEntered {
        /// Which state.
        state: StateId,
    },
    /// An action was proposed.
    ActionRequested {
        /// What was proposed.
        action: String,
        /// The capability it needs.
        capability: Capability,
    },
    /// An action was authorised.
    ActionAllowed {
        /// What was authorised.
        action: String,
        /// The capability exercised.
        capability: Capability,
    },
    /// An action was refused.
    ActionDenied {
        /// What was refused.
        action: String,
        /// The capability it needed.
        capability: Capability,
        /// What the policy said.
        decision: CapabilityDecision,
        /// Which rule refused it.
        reason: String,
    },
    /// A harness reported carrying an action out.
    ActionExecuted {
        /// What was carried out.
        action: String,
        /// Whether it succeeded.
        succeeded: bool,
    },
    /// Evidence was submitted.
    EvidenceProduced {
        /// Its identifier.
        evidence: EvidenceId,
        /// Its kind.
        kind: EvidenceKind,
        /// What produced it.
        producer: Producer,
        /// The state it was submitted in.
        state: StateId,
    },
    /// A verifier established a claim.
    VerificationPassed {
        /// Which verifier.
        verifier: Verifier,
        /// What it established.
        claim: Option<String>,
    },
    /// A verifier refuted a claim, or could not establish it.
    VerificationFailed {
        /// Which verifier.
        verifier: Verifier,
        /// What it was asked about.
        claim: Option<String>,
        /// The outcome.
        status: VerificationStatus,
        /// How many counterexamples it produced.
        counterexamples: usize,
    },
    /// An approval was asked for.
    ApprovalRequested {
        /// Which approval.
        approval: ApprovalId,
        /// Why.
        reason: Option<String>,
    },
    /// An approval was given.
    ApprovalGranted {
        /// Which approval.
        approval: ApprovalId,
        /// Who gave it.
        approver: Producer,
    },
    /// An approval was refused.
    ApprovalDenied {
        /// Which approval.
        approval: ApprovalId,
        /// Who refused it.
        approver: Producer,
    },
    /// A transition was taken.
    TransitionPerformed {
        /// Where from.
        from: StateId,
        /// Where to.
        to: StateId,
    },
    /// A transition was evaluated and not permitted.
    TransitionBlocked {
        /// Where from.
        from: StateId,
        /// Where it would have gone.
        to: StateId,
        /// The requirements that were not met.
        unmet: Vec<String>,
    },
    /// An obligation was not met.
    ObligationUnmet {
        /// Which principle obliges it.
        principle: PrincipleId,
        /// Which obligation.
        obligation: ObligationId,
        /// What is missing.
        unmet: Vec<String>,
    },
    /// The task's completion condition was satisfied in a terminal state.
    TaskCompleted {
        /// Which task.
        task: TaskId,
        /// Where it finished.
        state: StateId,
    },
    /// The execution stopped without completing.
    TaskAbandoned {
        /// Which task.
        task: TaskId,
        /// Why.
        reason: String,
    },
}

impl ProtocolEvent {
    /// A stable name for this event, matching the `event` tag in serialised form.
    pub fn name(&self) -> &'static str {
        match self {
            Self::TaskCreated { .. } => "task_created",
            Self::ProtocolResolved { .. } => "protocol_resolved",
            Self::StateEntered { .. } => "state_entered",
            Self::ActionRequested { .. } => "action_requested",
            Self::ActionAllowed { .. } => "action_allowed",
            Self::ActionDenied { .. } => "action_denied",
            Self::ActionExecuted { .. } => "action_executed",
            Self::EvidenceProduced { .. } => "evidence_produced",
            Self::VerificationPassed { .. } => "verification_passed",
            Self::VerificationFailed { .. } => "verification_failed",
            Self::ApprovalRequested { .. } => "approval_requested",
            Self::ApprovalGranted { .. } => "approval_granted",
            Self::ApprovalDenied { .. } => "approval_denied",
            Self::TransitionPerformed { .. } => "transition_performed",
            Self::TransitionBlocked { .. } => "transition_blocked",
            Self::ObligationUnmet { .. } => "obligation_unmet",
            Self::TaskCompleted { .. } => "task_completed",
            Self::TaskAbandoned { .. } => "task_abandoned",
        }
    }
}

/// One event with its position in the stream.
///
/// The event's own fields are flattened into the envelope, so one record is one flat object:
/// `{seq, at, execution, event: "state_entered", state: "implement"}`. That keeps the stream
/// readable with ordinary tools, which matters for something whose job is being read during an
/// incident review.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
pub struct EventEnvelope {
    /// Its position, starting at 1 and never reused.
    pub seq: u64,
    /// When it happened.
    pub at: Timestamp,
    /// Which execution it belongs to.
    pub execution: ExecutionId,
    /// What happened.
    #[serde(flatten)]
    pub event: ProtocolEvent,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn events_serialise_with_a_stable_tag() {
        let envelope = EventEnvelope {
            seq: 1,
            at: Timestamp::from_epoch_millis(1_700_000_000_000),
            execution: ExecutionId::new("exec-1").expect("id"),
            event: ProtocolEvent::StateEntered {
                state: StateId::new("implement").expect("state"),
            },
        };
        let json = serde_json::to_value(&envelope).expect("serialises");
        assert_eq!(json["event"], "state_entered");
        assert_eq!(json["state"], "implement");
        assert_eq!(json["seq"], 1);
        assert_eq!(envelope.event.name(), "state_entered");

        let round_tripped: EventEnvelope = serde_json::from_value(json).expect("deserialises");
        assert_eq!(round_tripped, envelope);
    }
}
