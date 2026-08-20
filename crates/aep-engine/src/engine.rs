//! The protocol engine.
//!
//! The engine is not a harness and does no work of its own. It answers questions, and every answer
//! is a function of the validated documents plus the evidence submitted — never of anything it
//! observed for itself.
//!
//! # Determinism
//!
//! Two properties are enforced here rather than hoped for:
//!
//! * **Transitions are ordered.** When more than one transition is permitted, the first in document
//!   order is taken and the others are reported. A workflow author can therefore see that a choice
//!   existed, instead of watching a coin flip.
//! * **Time is injected.** With a [`FixedClock`](crate::clock::FixedClock), replaying an execution
//!   reproduces its event stream exactly, which is what makes an audit trail diffable.
//!
//! # Evidence the protocol does not declare is refused
//!
//! Storing it would let one document set grow a private vocabulary no other harness could interpret.
//! Refusing it early makes the missing declaration obvious.

use std::sync::atomic::{AtomicU64, Ordering};

use aep_domain::action::ActionRequest;
use aep_domain::artifact::ArtifactGraph;
use aep_domain::capability::CapabilityPolicy;
use aep_domain::entity::EntityRef;
use aep_domain::event::ProtocolEvent;
use aep_domain::evidence::{
    ApprovalDecision, Evidence, EvidenceKind, EvidenceRecord, Producer, Provenance,
};
use aep_domain::ids::{EvidenceId, ExecutionId, StateId, SubjectRef};
use aep_domain::task::Task;
use aep_domain::verification::VerificationStatus;

use crate::clock::{Clock, SystemClock};
use crate::error::ProtocolError;
use crate::evaluate::{evaluate, Evaluation, Requirement};
use crate::execution::Execution;
use crate::explain::{CompletionExplanation, DecisionExplanation};
use crate::policy::{authorize, effective_policy, Decision};
use crate::registry::Registry;
use crate::resolve::resolve;

/// Evidence as a harness submits it: the observation, who produced it, and how.
///
/// The engine assigns the identifier and the timestamp. A caller cannot backdate evidence or reuse
/// an id, which is the least that has to be true for the log to be worth reading.
#[derive(Debug, Clone)]
pub struct EvidenceSubmission {
    /// The observation.
    pub evidence: Evidence,
    /// What produced it.
    pub producer: Producer,
    /// What it is about.
    pub subject: Option<SubjectRef>,
    /// How it was obtained.
    pub provenance: Provenance,
    /// The entity this evidence is stored as, when a backend holds it.
    ///
    /// The specification's engine interface submits evidence *by reference* for exactly this case:
    /// a test run recorded as an entity has identity, revision and provenance of its own, and the
    /// audit trail should point at it rather than at a copy the engine happens to hold.
    pub entity: Option<EntityRef>,
}

impl EvidenceSubmission {
    /// A submission with no provenance beyond its producer.
    pub fn new(evidence: Evidence, producer: Producer) -> Self {
        Self {
            evidence,
            producer,
            subject: None,
            provenance: Provenance::default(),
            entity: None,
        }
    }

    /// Names the entity this evidence is stored as, builder-style.
    #[must_use]
    pub fn stored_as(mut self, entity: EntityRef) -> Self {
        self.entity = Some(entity);
        self
    }

    /// Attaches a subject, builder-style.
    #[must_use]
    pub fn with_subject(mut self, subject: SubjectRef) -> Self {
        self.subject = Some(subject);
        self
    }

    /// Attaches provenance, builder-style.
    #[must_use]
    pub fn with_provenance(mut self, provenance: Provenance) -> Self {
        self.provenance = provenance;
        self
    }
}

/// What happened when a transition was attempted.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum TransitionResult {
    /// The execution moved.
    Moved {
        /// Where from.
        from: StateId,
        /// Where to.
        to: StateId,
        /// Other transitions that were also permitted, in document order.
        also_permitted: Vec<StateId>,
    },
    /// The execution is finished.
    Completed {
        /// The terminal state it finished in.
        state: StateId,
    },
    /// Nothing may move yet.
    Blocked {
        /// Where the execution is.
        state: StateId,
        /// One line per unmet requirement.
        reasons: Vec<String>,
    },
}

impl TransitionResult {
    /// `true` when the execution moved.
    pub fn moved(&self) -> bool {
        matches!(self, Self::Moved { .. })
    }

    /// `true` when the execution is finished.
    pub fn is_complete(&self) -> bool {
        matches!(self, Self::Completed { .. })
    }
}

/// The interface a harness uses.
pub trait ProtocolEngine {
    /// Resolves a task and starts an execution at its workflow's initial state.
    fn initialize(&self, task: Task) -> Result<Execution, ProtocolError>;

    /// What must hold in the current state.
    fn requirements(&self, execution: &Execution) -> Vec<Requirement>;

    /// What may be done in the current state.
    fn capabilities(&self, execution: &Execution) -> CapabilityPolicy;

    /// Whether a proposed action may proceed, and why.
    fn authorize(&self, execution: &mut Execution, request: &ActionRequest) -> Decision;

    /// Records evidence, returning the identifier the engine assigned.
    fn submit_evidence(
        &self,
        execution: &mut Execution,
        submission: EvidenceSubmission,
    ) -> Result<EvidenceId, ProtocolError>;

    /// The whole picture: what is owed, which transitions are permitted, whether it is complete.
    fn evaluate(&self, execution: &Execution) -> Evaluation;

    /// Attempts to advance the execution.
    fn transition(&self, execution: &mut Execution) -> Result<TransitionResult, ProtocolError>;
}

/// The reference engine.
#[derive(Debug)]
pub struct Engine<C: Clock = SystemClock> {
    registry: Registry,
    clock: C,
    executions: AtomicU64,
}

impl Engine<SystemClock> {
    /// An engine over `registry`, using the wall clock.
    pub fn new(registry: Registry) -> Self {
        Self::with_clock(registry, SystemClock)
    }
}

impl<C: Clock> Engine<C> {
    /// An engine over `registry`, using `clock`.
    pub fn with_clock(registry: Registry, clock: C) -> Self {
        Self {
            registry,
            clock,
            executions: AtomicU64::new(0),
        }
    }

    /// The documents this engine resolves against.
    pub fn registry(&self) -> &Registry {
        &self.registry
    }

    /// Starts an execution with an artifact graph already loaded.
    ///
    /// The graph is an input, like the task: the engine reads it and never writes to it.
    pub fn initialize_with_artifacts(
        &self,
        task: Task,
        artifacts: ArtifactGraph,
    ) -> Result<Execution, ProtocolError> {
        // The plan takes ownership of the task, so a caller cannot mutate it out from under an
        // execution that is already being evaluated against it.
        let plan = resolve(&task, &self.registry)?;
        drop(task);
        let ordinal = self.executions.fetch_add(1, Ordering::Relaxed) + 1;
        let id = ExecutionId::new(format!("{}.{ordinal}", plan.task.id)).unwrap_or_else(|_| {
            ExecutionId::new(format!("execution.{ordinal}")).expect("valid id")
        });

        let mut execution = Execution::new(id, plan, artifacts);
        let now = self.clock.now();
        let task_id = execution.plan().task.id.clone();
        let kind = execution.plan().task.kind.clone();
        execution.emit(
            now,
            ProtocolEvent::TaskCreated {
                task: task_id,
                kind,
            },
        );

        let plan = execution.plan();
        let event = ProtocolEvent::ProtocolResolved {
            protocol: plan.protocol.reference(),
            profile: plan.profile.id.clone(),
            workflow: plan.workflow.id.clone(),
            principles: plan
                .principles
                .iter()
                .map(|principle| principle.id.clone())
                .collect(),
            dropped_principles: plan.dropped_principles.clone(),
        };
        execution.emit(self.clock.now(), event);

        let initial = execution.state_id().clone();
        execution.emit(
            self.clock.now(),
            ProtocolEvent::StateEntered { state: initial },
        );
        Ok(execution)
    }

    /// Rebuilds an execution from a snapshot, re-resolving the task against the current documents.
    pub fn restore(
        &self,
        task: Task,
        artifacts: ArtifactGraph,
        snapshot: crate::execution::Snapshot,
    ) -> Result<Execution, ProtocolError> {
        let plan = resolve(&task, &self.registry)?;
        drop(task);
        Execution::restore(plan, artifacts, snapshot)
    }

    /// Explains why an action was allowed or refused.
    pub fn explain_decision(decision: &Decision) -> DecisionExplanation {
        DecisionExplanation::from(decision)
    }

    /// Explains whether the task is complete, and what is outstanding.
    pub fn explain_completion(&self, execution: &Execution) -> CompletionExplanation {
        CompletionExplanation::from_evaluation(&evaluate(execution))
    }
}

impl<C: Clock> ProtocolEngine for Engine<C> {
    fn initialize(&self, task: Task) -> Result<Execution, ProtocolError> {
        self.initialize_with_artifacts(task, ArtifactGraph::new())
    }

    fn requirements(&self, execution: &Execution) -> Vec<Requirement> {
        evaluate(execution).requirements
    }

    fn capabilities(&self, execution: &Execution) -> CapabilityPolicy {
        effective_policy(execution)
    }

    fn authorize(&self, execution: &mut Execution, request: &ActionRequest) -> Decision {
        let decision = authorize(execution, request);
        let now = self.clock.now();
        execution.emit(
            now,
            ProtocolEvent::ActionRequested {
                action: decision.operation.clone(),
                capability: decision.capability.clone(),
            },
        );
        let event = if decision.allowed {
            ProtocolEvent::ActionAllowed {
                action: decision.operation.clone(),
                capability: decision.capability.clone(),
            }
        } else {
            ProtocolEvent::ActionDenied {
                action: decision.operation.clone(),
                capability: decision.capability.clone(),
                decision: decision.decision,
                reason: decision.reason.as_ref().map_or_else(
                    || "no policy grants this capability".to_owned(),
                    |reason| format!("{} rule {}", reason.source, reason.rule),
                ),
            }
        };
        execution.emit(self.clock.now(), event);
        decision
    }

    fn submit_evidence(
        &self,
        execution: &mut Execution,
        submission: EvidenceSubmission,
    ) -> Result<EvidenceId, ProtocolError> {
        let kind = submission.evidence.kind();
        if !execution.plan().protocol.declares_evidence(kind) {
            return Err(ProtocolError::EvidenceRejected {
                kind,
                declared: execution
                    .plan()
                    .protocol
                    .evidence_kinds
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", "),
            });
        }

        let ordinal = execution.recorded_evidence().len() + 1;
        let id = EvidenceId::new(format!("{}.evidence.{ordinal}", execution.id()))
            .unwrap_or_else(|_| EvidenceId::new(format!("evidence.{ordinal}")).expect("valid id"));
        let now = self.clock.now();

        let record = EvidenceRecord {
            id: id.clone(),
            produced_at: now,
            producer: submission.producer.clone(),
            subject: submission.subject,
            value: submission.evidence.clone(),
            provenance: submission.provenance,
        };
        let state = execution.state_id().clone();
        if let Some(entity) = submission.entity.clone() {
            execution.link_evidence(id.clone(), entity);
        }
        execution.record_evidence(record);

        execution.emit(
            now,
            ProtocolEvent::EvidenceProduced {
                evidence: id.clone(),
                kind,
                producer: submission.producer,
                state,
            },
        );
        for event in verification_events(&submission.evidence) {
            execution.emit(self.clock.now(), event);
        }

        Ok(id)
    }

    fn evaluate(&self, execution: &Execution) -> Evaluation {
        evaluate(execution)
    }

    fn transition(&self, execution: &mut Execution) -> Result<TransitionResult, ProtocolError> {
        let evaluation = evaluate(execution);

        if evaluation.is_complete {
            let state = evaluation.state.clone();
            let task = execution.plan().task.id.clone();
            let now = self.clock.now();
            execution.emit(
                now,
                ProtocolEvent::TaskCompleted {
                    task,
                    state: state.clone(),
                },
            );
            return Ok(TransitionResult::Completed { state });
        }

        let permitted: Vec<StateId> = evaluation
            .permitted_transitions()
            .iter()
            .map(|transition| transition.to.clone())
            .collect();

        let Some((first, others)) = permitted.split_first() else {
            // Report every candidate's unmet requirements, so the reason is in the audit trail and
            // not only in whatever the harness chose to print.
            for transition in &evaluation.transitions {
                let unmet = transition.unmet();
                let event = ProtocolEvent::TransitionBlocked {
                    from: evaluation.state.clone(),
                    to: transition.to.clone(),
                    unmet,
                };
                let now = self.clock.now();
                execution.emit(now, event);
            }
            let reasons = evaluation.blocking_reasons();
            return Ok(TransitionResult::Blocked {
                state: evaluation.state,
                reasons,
            });
        };

        let from = evaluation.state.clone();
        execution.enter_state(first.clone())?;
        let now = self.clock.now();
        execution.emit(
            now,
            ProtocolEvent::TransitionPerformed {
                from: from.clone(),
                to: first.clone(),
            },
        );
        execution.emit(
            self.clock.now(),
            ProtocolEvent::StateEntered {
                state: first.clone(),
            },
        );

        Ok(TransitionResult::Moved {
            from,
            to: first.clone(),
            also_permitted: others.to_vec(),
        })
    }
}

/// Events that follow from a piece of evidence being a verifier's verdict.
fn verification_events(evidence: &Evidence) -> Vec<ProtocolEvent> {
    match evidence {
        Evidence::Verification(record) => vec![if record.status.is_pass() {
            ProtocolEvent::VerificationPassed {
                verifier: record.verifier.clone(),
                claim: Some(record.claim.to_string()),
            }
        } else {
            ProtocolEvent::VerificationFailed {
                verifier: record.verifier.clone(),
                claim: Some(record.claim.to_string()),
                status: record.status,
                counterexamples: record.counterexamples.len(),
            }
        }],
        Evidence::PropertyTestResult(result) => vec![if result.status.is_pass() {
            ProtocolEvent::VerificationPassed {
                verifier: aep_domain::verification::Verifier::PropertyTester,
                claim: Some(result.property.to_string()),
            }
        } else {
            ProtocolEvent::VerificationFailed {
                verifier: aep_domain::verification::Verifier::PropertyTester,
                claim: Some(result.property.to_string()),
                status: result.status,
                counterexamples: result.counterexamples.len(),
            }
        }],
        Evidence::TestResult(result) if result.status() == VerificationStatus::Failed => {
            vec![ProtocolEvent::VerificationFailed {
                verifier: aep_domain::verification::Verifier::TestRunner,
                claim: Some(format!("{} tests", result.suite)),
                status: VerificationStatus::Failed,
                counterexamples: 0,
            }]
        }
        Evidence::Approval(approval) => vec![if approval.decision == ApprovalDecision::Granted {
            ProtocolEvent::ApprovalGranted {
                approval: approval.approval.clone(),
                approver: approval.approver.clone(),
            }
        } else {
            ProtocolEvent::ApprovalDenied {
                approval: approval.approval.clone(),
                approver: approval.approver.clone(),
            }
        }],
        _ => Vec::new(),
    }
}

/// Evidence kinds a harness is expected to be able to produce for a given verifier class.
///
/// Exposed for a harness deciding which tools to expose: if the protocol requires a
/// `contract_result` and the harness has no contract runner, that is worth knowing before the task
/// starts rather than at the transition that needs it.
pub fn kinds_for_verifier(verifier: &aep_domain::verification::Verifier) -> Vec<EvidenceKind> {
    EvidenceKind::ALL
        .iter()
        .copied()
        .filter(|kind| kind.default_verifiers().contains(verifier))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clock::{FixedClock, SteppingClock};
    use crate::fixtures;
    use aep_domain::action::{Action, RepositoryWrite};
    use aep_domain::evidence::{ChangeSet, HealthObservation, HealthStatus, TestResult, TestSuite};
    use aep_domain::verification::Verifier;

    fn engine() -> Engine<SteppingClock> {
        Engine::with_clock(fixtures::standard_registry(), SteppingClock::new(1_000, 10))
    }

    fn diff() -> EvidenceSubmission {
        EvidenceSubmission::new(
            Evidence::Diff(ChangeSet {
                files_changed: 2,
                lines_added: 30,
                lines_removed: 4,
                revision_before: None,
                revision_after: None,
                paths: vec!["src/lib.rs".to_owned()],
            }),
            Producer::Agent {
                id: "opus".to_owned(),
            },
        )
    }

    fn passing_tests() -> EvidenceSubmission {
        EvidenceSubmission::new(
            Evidence::TestResult(TestResult::passing(TestSuite::Unit, 12)),
            Producer::Verifier {
                verifier: Verifier::TestRunner,
            },
        )
    }

    #[test]
    fn walks_a_workflow_from_start_to_completion() {
        let engine = engine();
        let mut execution = engine
            .initialize(fixtures::standard_task())
            .expect("initialises");
        assert_eq!(execution.state_id().as_str(), "implement");

        // Nothing has been observed yet, so nothing may move.
        let blocked = engine.transition(&mut execution).expect("evaluates");
        assert!(
            matches!(blocked, TransitionResult::Blocked { .. }),
            "{blocked:?}"
        );

        engine
            .submit_evidence(&mut execution, diff())
            .expect("diff is declared");
        let moved = engine.transition(&mut execution).expect("evaluates");
        assert_eq!(
            moved,
            TransitionResult::Moved {
                from: "implement".parse().expect("state"),
                to: "verify".parse().expect("state"),
                also_permitted: Vec::new(),
            }
        );

        let blocked = engine.transition(&mut execution).expect("evaluates");
        assert!(
            matches!(blocked, TransitionResult::Blocked { .. }),
            "{blocked:?}"
        );

        engine
            .submit_evidence(&mut execution, passing_tests())
            .expect("test results are declared");
        assert!(engine
            .transition(&mut execution)
            .expect("evaluates")
            .moved());
        assert_eq!(execution.state_id().as_str(), "complete");

        let completed = engine.transition(&mut execution).expect("evaluates");
        assert!(completed.is_complete(), "{completed:?}");
        assert!(engine.evaluate(&execution).is_complete);
    }

    #[test]
    fn a_blocked_transition_says_what_is_missing_and_records_it() {
        let engine = engine();
        let mut execution = engine
            .initialize(fixtures::standard_task())
            .expect("initialises");

        let TransitionResult::Blocked { reasons, .. } =
            engine.transition(&mut execution).expect("evaluates")
        else {
            panic!("expected to be blocked");
        };
        assert!(
            reasons.iter().any(|reason| reason.contains("diff.exists")),
            "{reasons:?}"
        );
        assert!(
            execution
                .events()
                .iter()
                .any(|event| event.event.name() == "transition_blocked"),
            "a refusal belongs in the audit trail, not only in whatever the harness printed"
        );
    }

    #[test]
    fn rejects_evidence_the_protocol_does_not_declare() {
        let profile = fixtures::PROFILE;
        let narrow_protocol = fixtures::PROTOCOL.replace("  - health_observation\n", "");
        let registry = fixtures::registry(
            &[&narrow_protocol],
            &[fixtures::TEST_DRIVEN],
            &[fixtures::WORKFLOW],
            &[profile],
        );
        let engine = Engine::with_clock(registry, FixedClock::new(1));
        let mut execution = engine
            .initialize(fixtures::standard_task())
            .expect("initialises");

        let error = engine
            .submit_evidence(
                &mut execution,
                EvidenceSubmission::new(
                    Evidence::HealthObservation(HealthObservation {
                        service: None,
                        status: HealthStatus::Healthy,
                        detail: None,
                    }),
                    Producer::Verifier {
                        verifier: Verifier::TelemetryQuery,
                    },
                ),
            )
            .expect_err("undeclared kind");

        assert_eq!(error.code(), "evidence_rejected");
        assert!(error.to_string().contains("health_observation"), "{error}");
        assert!(
            execution.recorded_evidence().is_empty(),
            "refused evidence must not be stored: a private vocabulary is worse than a missing one"
        );
    }

    #[test]
    fn replays_identically_under_a_fixed_clock() {
        let run = || {
            let engine = Engine::with_clock(fixtures::standard_registry(), FixedClock::new(7));
            let mut execution = engine
                .initialize(fixtures::standard_task())
                .expect("initialises");
            engine
                .submit_evidence(&mut execution, diff())
                .expect("recorded");
            engine.transition(&mut execution).expect("evaluates");
            engine
                .submit_evidence(&mut execution, passing_tests())
                .expect("recorded");
            engine.transition(&mut execution).expect("evaluates");
            engine.transition(&mut execution).expect("evaluates");
            serde_json::to_string(execution.events()).expect("events serialise")
        };
        assert_eq!(
            run(),
            run(),
            "the same inputs must produce the same audit trail"
        );
    }

    #[test]
    fn reports_other_permitted_transitions_rather_than_hiding_the_choice() {
        let workflow = r"
id: test/forked
title: Forked
initial: start
states:
  start:
    title: Start
    phases: [implementation]
  left:
    title: Left
    terminal: true
    phases: [completion]
  right:
    title: Right
    terminal: true
    phases: [completion]
transitions:
  - from: start
    to: left
    when: diff.exists
  - from: start
    to: right
    when: diff.exists
";
        let profile = fixtures::PROFILE.replace("workflow: test/linear", "workflow: test/forked");
        let registry = fixtures::registry(
            &[fixtures::PROTOCOL],
            &[fixtures::TEST_DRIVEN],
            &[workflow],
            &[&profile],
        );
        let engine = Engine::with_clock(registry, FixedClock::new(1));
        let mut execution = engine
            .initialize(fixtures::standard_task())
            .expect("initialises");
        engine
            .submit_evidence(&mut execution, diff())
            .expect("recorded");
        // Both targets are terminal, so the obligations owed before completion apply to each.
        engine
            .submit_evidence(&mut execution, passing_tests())
            .expect("recorded");

        let TransitionResult::Moved {
            to, also_permitted, ..
        } = engine.transition(&mut execution).expect("evaluates")
        else {
            panic!("expected to move");
        };
        assert_eq!(
            to.as_str(),
            "left",
            "the first transition in document order wins"
        );
        assert_eq!(
            also_permitted
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>(),
            vec!["right"],
            "the choice that existed is reported rather than silently dropped"
        );
    }

    #[test]
    fn a_refused_action_is_recorded_with_the_rule_that_refused_it() {
        let engine = engine();
        let mut execution = engine
            .initialize(fixtures::standard_task())
            .expect("initialises");

        let request = ActionRequest::new(Action::SecretRead(aep_domain::action::SecretRead {
            secret: "database-password".to_owned(),
        }));
        let decision = engine.authorize(&mut execution, &request);
        assert!(!decision.is_allowed());

        let denied = execution
            .events()
            .iter()
            .find(|event| event.event.name() == "action_denied")
            .expect("the denial is in the event stream");
        let json = serde_json::to_value(denied).expect("serialises");
        assert_eq!(json["capability"], "secret.read");
        assert_eq!(json["decision"], "not_granted");

        // An allowed action is recorded too, so the trail shows what was done, not only what was not.
        let allowed = ActionRequest::new(Action::RepositoryWrite(RepositoryWrite {
            paths: vec!["src/lib.rs".to_owned()],
            intent: Some("implement the feature".to_owned()),
        }));
        assert!(engine.authorize(&mut execution, &allowed).is_allowed());
        assert!(execution
            .events()
            .iter()
            .any(|event| event.event.name() == "action_allowed"));
    }

    #[test]
    fn explains_completion_as_a_checklist() {
        let engine = engine();
        let mut execution = engine
            .initialize(fixtures::standard_task())
            .expect("initialises");
        let explanation = engine.explain_completion(&execution);
        assert!(!explanation.complete);
        assert!(explanation.outstanding().count() > 0);
        let rendered = explanation.to_string();
        assert!(rendered.contains("Task incomplete"), "{rendered}");
        assert!(rendered.contains("tests.unit.failed == 0"), "{rendered}");

        engine
            .submit_evidence(&mut execution, diff())
            .expect("recorded");
        engine
            .submit_evidence(&mut execution, passing_tests())
            .expect("recorded");
        let explanation = engine.explain_completion(&execution);
        assert_eq!(
            explanation.outstanding().count(),
            0,
            "every completion requirement is met once the evidence exists: {explanation}"
        );
    }

    #[test]
    fn a_refused_approval_enters_the_audit_trail_as_a_refusal() {
        // The third place the same rule lives. The event stream is what an audit reads back, so
        // emitting `approval_granted` for a decision of `denied` would put a grant in the
        // permanent record of a refusal — and the record is the only thing left once everyone has
        // forgotten the conversation.
        let engine = engine();
        let mut execution = engine
            .initialize(fixtures::standard_task())
            .expect("initialises");

        let submit = |execution: &mut Execution, decision| {
            engine
                .submit_evidence(
                    execution,
                    EvidenceSubmission::new(
                        Evidence::Approval(aep_domain::evidence::ApprovalRecord {
                            approval: "production-change".parse().expect("approval id"),
                            approver: Producer::Human {
                                id: "ada".to_owned(),
                            },
                            decision,
                            subject: Some("capability:production-write".parse().expect("subject")),
                            note: None,
                        }),
                        Producer::Human {
                            id: "ada".to_owned(),
                        },
                    ),
                )
                .expect("recorded")
        };

        submit(&mut execution, ApprovalDecision::Denied);
        let names = |execution: &Execution| {
            execution
                .events()
                .iter()
                .map(|event| event.event.name().to_owned())
                .collect::<Vec<_>>()
        };
        assert!(
            names(&execution).contains(&"approval_denied".to_owned()),
            "the refusal has to be findable: {:?}",
            names(&execution)
        );
        assert!(
            !names(&execution).contains(&"approval_granted".to_owned()),
            "and it must not be recorded as its opposite: {:?}",
            names(&execution)
        );

        submit(&mut execution, ApprovalDecision::Granted);
        assert!(
            names(&execution).contains(&"approval_granted".to_owned()),
            "a real grant is still recorded as one: {:?}",
            names(&execution)
        );
    }
}
