//! Turning protocol decisions into audit records and command contexts.
//!
//! The engine decides; the interaction contract records. This module is the join, and it exists so
//! that a refusal by the protocol and a refusal by a backend end up in the *same* trail, queryable
//! the same way.
//!
//! # Two directions
//!
//! * [`audit_trail`] maps an execution's events into [`AuditRecord`]s — including the denials, which
//!   is the half most systems lose. "An agent tried to change production and was stopped, by this
//!   rule" is only useful if it is written down next to the changes that succeeded.
//! * [`command_context`] builds the context a command issued during an execution should carry, so
//!   everything the execution causes shares its correlation id and names it as the cause.

use aep_contract::command::{CausationRef as ContractCausation, CommandContext};
use aep_domain::audit::{AuditKind, AuditRecord, CausationRef, DecisionRecord};
use aep_domain::event::{EventEnvelope, ProtocolEvent};
use aep_domain::ids::{AuditId, CorrelationId, IdempotencyKey, RequestId};
use aep_domain::time::Timestamp;

use crate::execution::Execution;
use crate::policy::Decision;

/// The correlation id for everything one execution causes.
///
/// The execution id doubles as the correlation id: every command, event and audit record produced
/// while working one task belongs to that task's activity, and giving them a second identifier to
/// join on would only be a second thing to get wrong.
pub fn correlation_id(execution: &Execution) -> CorrelationId {
    CorrelationId::new(execution.id().as_str())
        .expect("an execution identifier is a valid correlation identifier")
}

/// The context a command issued during `execution` should carry.
///
/// The caller supplies the request id and idempotency key because only it knows whether this is a
/// first attempt or a retry — the engine cannot tell, and guessing would defeat the point of both.
pub fn command_context(
    execution: &Execution,
    request_id: RequestId,
    idempotency_key: IdempotencyKey,
    at: Timestamp,
) -> CommandContext {
    let mut context = CommandContext::new(
        request_id,
        idempotency_key,
        execution.actor().clone(),
        correlation_id(execution),
        at,
    )
    .during(execution.id().clone(), execution.plan().task.id.clone());

    // The state the execution is in is what caused this command to be issued at all.
    context.causation = Some(ContractCausation(format!("state {}", execution.state_id())));
    context
}

/// The audit record for one protocol decision about an action.
pub fn decision_record(execution: &Execution, decision: &Decision, at: Timestamp) -> AuditRecord {
    let audit_id = audit_id(execution, "decision", at.epoch_millis());
    let mut record = AuditRecord::new(
        audit_id,
        AuditKind::ProtocolDecision,
        at,
        execution.actor().clone(),
        correlation_id(execution),
    );
    record.decision = Some(DecisionRecord {
        allowed: decision.allowed,
        operation: decision.operation.clone(),
        capability: Some(decision.capability.clone()),
        decision: Some(decision.decision),
        source: decision.reason.as_ref().map(|reason| reason.source.clone()),
        rule: decision.reason.as_ref().map(|reason| reason.rule.clone()),
        missing: decision.missing.clone(),
        state: Some(decision.current_state.to_string()),
    });
    record.execution_id = Some(execution.id().clone());
    record.task = Some(execution.plan().task.id.clone());
    record
}

/// Every audit record implied by an execution's event stream, in order.
///
/// Events the protocol emits for its own bookkeeping — entering a state, resolving a profile — are
/// deliberately not audit records: nothing was decided and nothing changed, and a trail padded with
/// them is a trail nobody reads.
pub fn audit_trail(execution: &Execution) -> Vec<AuditRecord> {
    execution
        .events()
        .iter()
        .filter_map(|envelope| record_for(execution, envelope))
        .collect()
}

/// The audit record for one event, if it is one.
// One arm per event kind. The mapping is the contract between the two streams, and a reader needs to
// see which events become records and which deliberately do not.
#[allow(clippy::too_many_lines)]
fn record_for(execution: &Execution, envelope: &EventEnvelope) -> Option<AuditRecord> {
    let at = envelope.at;
    let actor = execution.actor().clone();
    let correlation = correlation_id(execution);
    let id = audit_id(execution, envelope.event.name(), envelope.seq);

    let mut record = match &envelope.event {
        ProtocolEvent::ActionAllowed {
            action, capability, ..
        } => {
            let mut record =
                AuditRecord::new(id, AuditKind::ProtocolDecision, at, actor, correlation);
            record.decision = Some(DecisionRecord {
                allowed: true,
                operation: action.clone(),
                capability: Some(capability.clone()),
                decision: None,
                source: None,
                rule: None,
                missing: Vec::new(),
                state: Some(execution.state_id().to_string()),
            });
            record
        }
        ProtocolEvent::ActionDenied {
            action,
            capability,
            decision,
            reason,
        } => {
            let mut record =
                AuditRecord::new(id, AuditKind::ProtocolDecision, at, actor, correlation);
            record.decision = Some(DecisionRecord {
                allowed: false,
                operation: action.clone(),
                capability: Some(capability.clone()),
                decision: Some(*decision),
                source: None,
                rule: Some(reason.clone()),
                missing: Vec::new(),
                state: Some(execution.state_id().to_string()),
            });
            record
        }
        ProtocolEvent::EvidenceProduced { evidence, .. } => {
            let mut record =
                AuditRecord::new(id, AuditKind::EvidenceRecorded, at, actor, correlation);
            record.decision = Some(informational(format!("evidence {evidence} recorded")));
            // When the evidence is stored as an entity, the trail points at the entity rather than
            // at the engine's copy of it.
            if let Some(entity) = execution.evidence_entity(evidence) {
                record.evidence = vec![entity.clone()];
            }
            record
        }
        ProtocolEvent::TransitionPerformed { from, to } => {
            let mut record =
                AuditRecord::new(id, AuditKind::TransitionPerformed, at, actor, correlation);
            record.decision = Some(informational(format!("{from} -> {to}")));
            record
        }
        ProtocolEvent::TransitionBlocked { from, to, unmet } => {
            let mut record =
                AuditRecord::new(id, AuditKind::TransitionBlocked, at, actor, correlation);
            let mut decision = DecisionRecord {
                allowed: false,
                operation: format!("{from} -> {to}"),
                capability: None,
                decision: None,
                source: None,
                rule: None,
                missing: unmet.clone(),
                state: Some(from.to_string()),
            };
            decision.rule = Some("transition-requirements-unmet".to_owned());
            record.decision = Some(decision);
            record
        }
        ProtocolEvent::ApprovalGranted { approval, .. } => {
            let mut record =
                AuditRecord::new(id, AuditKind::ApprovalGranted, at, actor, correlation);
            record.causation = Some(CausationRef::Approval {
                approval: approval.clone(),
            });
            record.decision = Some(informational(format!("approval {approval} granted")));
            record
        }
        ProtocolEvent::ApprovalDenied { approval, .. } => {
            let mut record =
                AuditRecord::new(id, AuditKind::ApprovalDenied, at, actor, correlation);
            record.causation = Some(CausationRef::Approval {
                approval: approval.clone(),
            });
            record.decision = Some(DecisionRecord {
                allowed: false,
                operation: format!("approval {approval}"),
                capability: None,
                decision: None,
                source: None,
                rule: Some("approval-denied".to_owned()),
                missing: Vec::new(),
                state: Some(execution.state_id().to_string()),
            });
            record
        }
        ProtocolEvent::VerificationPassed { verifier, claim }
        | ProtocolEvent::VerificationFailed {
            verifier, claim, ..
        } => {
            let passed = matches!(envelope.event, ProtocolEvent::VerificationPassed { .. });
            let mut record =
                AuditRecord::new(id, AuditKind::VerificationCompleted, at, actor, correlation);
            record.decision = Some(DecisionRecord {
                allowed: passed,
                operation: match claim {
                    Some(claim) => format!("{verifier} on {claim}"),
                    None => verifier.to_string(),
                },
                capability: None,
                decision: None,
                source: None,
                rule: (!passed).then(|| "verification-failed".to_owned()),
                missing: Vec::new(),
                state: Some(execution.state_id().to_string()),
            });
            record
        }
        _ => return None,
    };

    record.execution_id = Some(execution.id().clone());
    record.task = Some(execution.plan().task.id.clone());
    Some(record)
}

/// A decision record for something that happened rather than something that was decided.
fn informational(operation: String) -> DecisionRecord {
    DecisionRecord {
        allowed: true,
        operation,
        capability: None,
        decision: None,
        source: None,
        rule: None,
        missing: Vec::new(),
        state: None,
    }
}

/// Builds a deterministic audit identifier, so a replayed execution produces the same trail.
fn audit_id(execution: &Execution, label: &str, sequence: u64) -> AuditId {
    AuditId::new(format!("{}.{label}.{sequence}", execution.id()))
        .unwrap_or_else(|_| AuditId::new(format!("audit.{sequence}")).expect("well formed"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{Engine, EvidenceSubmission, ProtocolEngine};
    use crate::fixtures;
    use crate::FixedClock;
    use aep_domain::action::ActionRequest;
    use aep_domain::action::{Action, SecretRead};
    use aep_domain::evidence::{Evidence, Producer, TestResult, TestSuite};
    use aep_domain::verification::Verifier;

    fn execution() -> (Engine<FixedClock>, crate::Execution) {
        let engine = Engine::with_clock(fixtures::standard_registry(), FixedClock::new(1_000));
        let mut execution = engine
            .initialize(fixtures::standard_task())
            .expect("initialises");
        execution.set_actor("human:alice".parse().expect("actor"));
        (engine, execution)
    }

    #[test]
    fn a_refused_action_becomes_an_audit_record_naming_the_rule() {
        let (engine, mut execution) = execution();
        let request = ActionRequest::new(Action::SecretRead(SecretRead {
            secret: "database-password".to_owned(),
        }));
        engine.authorize(&mut execution, &request);

        let trail = audit_trail(&execution);
        let refusal = trail
            .iter()
            .find(|record| {
                record
                    .decision
                    .as_ref()
                    .is_some_and(|decision| !decision.allowed)
            })
            .expect("the refusal is in the trail");

        assert_eq!(refusal.kind, AuditKind::ProtocolDecision);
        assert_eq!(refusal.actor.to_string(), "human:alice");
        assert!(refusal.change.is_none(), "a refusal changed nothing");
        assert!(
            refusal.validate().is_ok(),
            "and the record says so consistently"
        );
        let decision = refusal.decision.as_ref().expect("a decision");
        assert_eq!(
            decision.capability.as_ref().map(ToString::to_string),
            Some("secret.read".to_owned())
        );
    }

    #[test]
    fn bookkeeping_events_do_not_become_audit_records() {
        let (_, execution) = execution();
        // Initialising emits task-created, protocol-resolved and state-entered. None of them is a
        // decision, and a trail padded with them is a trail nobody reads.
        assert!(execution.event_count() >= 3);
        assert!(audit_trail(&execution).is_empty());
    }

    #[test]
    fn evidence_and_transitions_are_recorded_with_the_execution_and_task() {
        let (engine, mut execution) = execution();
        engine
            .submit_evidence(
                &mut execution,
                EvidenceSubmission::new(
                    Evidence::TestResult(TestResult::passing(TestSuite::Unit, 4)),
                    Producer::Verifier {
                        verifier: Verifier::TestRunner,
                    },
                ),
            )
            .expect("recorded");

        let trail = audit_trail(&execution);
        let record = trail
            .iter()
            .find(|record| record.kind == AuditKind::EvidenceRecorded)
            .expect("evidence is audited");
        assert_eq!(record.execution_id.as_ref(), Some(execution.id()));
        assert_eq!(
            record.task.as_ref().map(ToString::to_string),
            Some("T-1".to_owned())
        );
        assert_eq!(record.correlation_id, correlation_id(&execution));
    }

    #[test]
    fn a_blocked_transition_is_audited_with_what_was_missing() {
        let (engine, mut execution) = execution();
        engine.transition(&mut execution).expect("evaluates");

        let trail = audit_trail(&execution);
        let blocked = trail
            .iter()
            .find(|record| record.kind == AuditKind::TransitionBlocked)
            .expect("the block is audited");
        let decision = blocked.decision.as_ref().expect("a decision");
        assert!(!decision.allowed);
        assert!(
            decision
                .missing
                .iter()
                .any(|reason| reason.contains("diff.exists")),
            "{:?}",
            decision.missing
        );
    }

    #[test]
    fn a_command_issued_during_an_execution_inherits_its_activity() {
        let (_, execution) = execution();
        let context = command_context(
            &execution,
            "req-1".parse().expect("id"),
            "key-1".parse().expect("key"),
            Timestamp::from_epoch_millis(2_000),
        );

        assert_eq!(context.correlation_id, correlation_id(&execution));
        assert_eq!(context.execution_id.as_ref(), Some(execution.id()));
        assert_eq!(context.actor.to_string(), "human:alice");
        assert!(
            context.causation.is_some(),
            "the state the execution is in is what caused the command"
        );
    }

    #[test]
    fn evidence_stored_as_an_entity_is_pointed_at_rather_than_copied() {
        let (engine, mut execution) = execution();
        let entity = aep_domain::entity::EntityRef::new("01MEM0000000000042".parse().expect("id"));
        engine
            .submit_evidence(
                &mut execution,
                EvidenceSubmission::new(
                    Evidence::TestResult(TestResult::passing(TestSuite::Unit, 4)),
                    Producer::Verifier {
                        verifier: Verifier::TestRunner,
                    },
                )
                .stored_as(entity.clone()),
            )
            .expect("recorded");

        let trail = audit_trail(&execution);
        let record = trail
            .iter()
            .find(|record| record.kind == AuditKind::EvidenceRecorded)
            .expect("evidence is audited");
        assert_eq!(record.evidence, vec![entity]);
    }

    #[test]
    fn the_trail_is_identical_across_replays() {
        let first = {
            let (engine, mut execution) = execution();
            engine.transition(&mut execution).expect("evaluates");
            serde_json::to_string(&audit_trail(&execution)).expect("serialises")
        };
        let second = {
            let (engine, mut execution) = execution();
            engine.transition(&mut execution).expect("evaluates");
            serde_json::to_string(&audit_trail(&execution)).expect("serialises")
        };
        assert_eq!(
            first, second,
            "an audit trail that cannot be diffed cannot be reviewed"
        );
    }
}
