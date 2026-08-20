//! Live execution state.
//!
//! An [`Execution`] is a plan plus everything that has happened: which state it is in, which states
//! it has been through, every piece of evidence in submission order, the artifact graph, and the
//! event stream.
//!
//! # Derived facts
//!
//! Most facts come from evidence. A few can only be known by the engine, and it binds them **last**
//! so submitted evidence cannot overwrite them:
//!
//! ```text
//! state.current                the current state
//! state.<id>.entered           true once entered
//! workflow.terminal            whether the current state ends the workflow
//! evidence.count.<kind>        how many records of that kind
//! evidence.first_seq.<kind>    submission order, which is what makes red-before-green checkable
//! evidence.last_seq.<kind>
//! test.first_result            the first test outcome ever observed
//! evidence.missing             unmet evidence requirements, for `evidence.missing == 0`
//! approvals.granted            how many approvals have been granted
//! ```
//!
//! `evidence.first_seq.*` is the interesting one: it is what lets a document say "a test must have
//! failed before any code changed" as a checkable fact rather than as a comment.

use aep_domain::artifact::ArtifactGraph;
use aep_domain::entity::{ActorRef, EntityRef};
use aep_domain::event::{EventEnvelope, ProtocolEvent};
use aep_domain::evidence::{ApprovalDecision, Evidence, EvidenceKind, EvidenceRecord};
use aep_domain::facts::{FactPath, FactSource, FactStore, FactValue};
use aep_domain::ids::{EvidenceId, ExecutionId, StateId};
use aep_domain::plan::ExecutionPlan;
use aep_domain::predicate::Truth;
use aep_domain::requirement::{EvidenceRequirement, RequirementContext, RequirementSet};
use aep_domain::time::Timestamp;
use aep_domain::workflow::State;

use crate::error::ProtocolError;

/// One submitted piece of evidence, with the state it arrived in.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RecordedEvidence {
    /// The evidence.
    pub record: EvidenceRecord,
    /// Which state the execution was in when it was submitted.
    pub state: StateId,
}

/// A serialisable execution, for persistence across process boundaries.
///
/// The plan is not part of a snapshot: it is derived from the documents, and a snapshot that carried
/// its own copy could outlive a change to them without anyone noticing. Restoring re-resolves.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    /// Which execution this is.
    pub execution: ExecutionId,
    /// The task it belongs to, checked on restore.
    pub task: String,
    /// The current state.
    pub state: StateId,
    /// States already entered, in order.
    pub entered: Vec<StateId>,
    /// Evidence in submission order.
    pub evidence: Vec<RecordedEvidence>,
    /// The event stream.
    pub events: Vec<EventEnvelope>,
    /// The next event sequence number.
    pub next_seq: u64,
    /// On whose behalf the execution was running.
    #[serde(default = "default_actor")]
    pub actor: ActorRef,
}

/// Serde default for a snapshot taken before executions recorded an actor.
fn default_actor() -> ActorRef {
    ActorRef::System
}

/// A task being executed under a protocol.
#[derive(Debug, Clone)]
pub struct Execution {
    id: ExecutionId,
    plan: ExecutionPlan,
    actor: ActorRef,
    state: StateId,
    entered: Vec<StateId>,
    evidence: Vec<RecordedEvidence>,
    artifacts: ArtifactGraph,
    events: Vec<EventEnvelope>,
    next_seq: u64,
    facts: FactStore,
    records: Vec<EvidenceRecord>,
    evidence_entities: std::collections::BTreeMap<EvidenceId, EntityRef>,
}

impl Execution {
    /// Starts an execution at its workflow's initial state.
    pub fn new(id: ExecutionId, plan: ExecutionPlan, artifacts: ArtifactGraph) -> Self {
        let state = plan.workflow.initial.clone();
        let mut execution = Self {
            id,
            plan,
            actor: ActorRef::System,
            state: state.clone(),
            entered: vec![state],
            evidence: Vec::new(),
            artifacts,
            events: Vec::new(),
            next_seq: 1,
            facts: FactStore::new(),
            records: Vec::new(),
            evidence_entities: std::collections::BTreeMap::new(),
        };
        execution.refresh_facts();
        execution
    }

    /// Which execution this is.
    pub fn id(&self) -> &ExecutionId {
        &self.id
    }

    /// On whose behalf this execution is running.
    ///
    /// Defaults to [`ActorRef::System`]. A harness that knows who asked should say so, because every
    /// decision the engine records is attributed to this actor, and "the system decided" is not an
    /// answer anyone can act on.
    pub fn actor(&self) -> &ActorRef {
        &self.actor
    }

    /// Sets who this execution is running on behalf of.
    pub fn set_actor(&mut self, actor: ActorRef) {
        self.actor = actor;
    }

    /// The plan being executed.
    pub fn plan(&self) -> &ExecutionPlan {
        &self.plan
    }

    /// The current state's identifier.
    pub fn state_id(&self) -> &StateId {
        &self.state
    }

    /// The current state.
    pub fn state(&self) -> Result<&State, ProtocolError> {
        self.plan
            .workflow
            .state(&self.state)
            .ok_or_else(|| ProtocolError::UnknownState {
                state: self.state.clone(),
                workflow: self.plan.workflow.id.to_string(),
            })
    }

    /// States entered so far, in order.
    pub fn entered(&self) -> &[StateId] {
        &self.entered
    }

    /// Every submitted piece of evidence, with the state it arrived in.
    pub fn recorded_evidence(&self) -> &[RecordedEvidence] {
        &self.evidence
    }

    /// The artifact graph.
    pub fn artifact_graph(&self) -> &ArtifactGraph {
        &self.artifacts
    }

    /// Replaces the artifact graph, for a harness that reloads a manifest mid-execution.
    pub fn set_artifacts(&mut self, artifacts: ArtifactGraph) {
        self.artifacts = artifacts;
        self.refresh_facts();
    }

    /// The event stream.
    pub fn events(&self) -> &[EventEnvelope] {
        &self.events
    }

    /// The facts currently observable.
    pub fn fact_store(&self) -> &FactStore {
        &self.facts
    }

    /// Records that a piece of evidence is stored as an entity.
    pub fn link_evidence(&mut self, evidence: EvidenceId, entity: EntityRef) {
        self.evidence_entities.insert(evidence, entity);
    }

    /// The entity a piece of evidence is stored as, when a backend holds it.
    pub fn evidence_entity(&self, evidence: &EvidenceId) -> Option<&EntityRef> {
        self.evidence_entities.get(evidence)
    }

    /// Records evidence as arriving in the current state.
    pub fn record_evidence(&mut self, record: EvidenceRecord) {
        self.evidence.push(RecordedEvidence {
            record,
            state: self.state.clone(),
        });
        self.refresh_facts();
    }

    /// Moves to `state`, which must exist in the workflow.
    pub fn enter_state(&mut self, state: StateId) -> Result<(), ProtocolError> {
        if !self.plan.workflow.states.contains_key(&state) {
            return Err(ProtocolError::UnknownState {
                state,
                workflow: self.plan.workflow.id.to_string(),
            });
        }
        self.state = state.clone();
        self.entered.push(state);
        self.refresh_facts();
        Ok(())
    }

    /// Appends an event, assigning it the next sequence number.
    pub fn emit(&mut self, at: Timestamp, event: ProtocolEvent) -> &EventEnvelope {
        let envelope = EventEnvelope {
            seq: self.next_seq,
            at,
            execution: self.id.clone(),
            event,
        };
        self.next_seq += 1;
        self.events.push(envelope);
        self.events.last().expect("just pushed")
    }

    /// How many events have been emitted.
    pub fn event_count(&self) -> usize {
        self.events.len()
    }

    /// Captures the execution for persistence.
    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            execution: self.id.clone(),
            task: self.plan.task.id.to_string(),
            state: self.state.clone(),
            entered: self.entered.clone(),
            evidence: self.evidence.clone(),
            events: self.events.clone(),
            next_seq: self.next_seq,
            actor: self.actor.clone(),
        }
    }

    /// Rebuilds an execution from a freshly resolved plan and a snapshot.
    pub fn restore(
        plan: ExecutionPlan,
        artifacts: ArtifactGraph,
        snapshot: Snapshot,
    ) -> Result<Self, ProtocolError> {
        if snapshot.task != plan.task.id.to_string() {
            return Err(ProtocolError::UnknownState {
                state: snapshot.state,
                workflow: format!(
                    "snapshot belongs to task {} but the plan is for {}",
                    snapshot.task, plan.task.id
                ),
            });
        }
        if !plan.workflow.states.contains_key(&snapshot.state) {
            return Err(ProtocolError::UnknownState {
                state: snapshot.state,
                workflow: plan.workflow.id.to_string(),
            });
        }
        let mut execution = Self {
            id: snapshot.execution,
            plan,
            actor: snapshot.actor.clone(),
            state: snapshot.state,
            entered: snapshot.entered,
            evidence: snapshot.evidence,
            artifacts,
            events: snapshot.events,
            next_seq: snapshot.next_seq,
            facts: FactStore::new(),
            records: Vec::new(),
            evidence_entities: std::collections::BTreeMap::new(),
        };
        execution.refresh_facts();
        Ok(execution)
    }

    /// Rebuilds the fact store from the plan, the artifact graph, the evidence log and the engine's
    /// own derived facts, in that order.
    fn refresh_facts(&mut self) {
        self.records = self
            .evidence
            .iter()
            .map(|recorded| recorded.record.clone())
            .collect();

        // Two passes: what has been observed, then what the engine derives from it. The derived
        // facts are computed against the observed ones — never against themselves — which is what
        // keeps `evidence.missing` from depending on its own value.
        let mut observed = self.plan.facts.clone();
        observed.extend(self.artifacts.facts());
        for recorded in &self.evidence {
            observed.extend_facts(recorded.record.facts());
        }
        observed.set_scales(self.plan.protocol.scales.clone());

        let mut facts = observed.clone();
        for (path, value) in self.derived_facts(&observed) {
            facts.set(path, value);
        }
        self.facts = facts;
    }

    /// The facts only the engine can know, computed against the observed ones.
    fn derived_facts(&self, observed: &FactStore) -> Vec<(FactPath, FactValue)> {
        let mut facts: Vec<(FactPath, FactValue)> = Vec::new();
        let path = |segments: &[&str]| FactPath::from_segments(segments);

        facts.push((
            path(&["state", "current"]),
            FactValue::text(self.state.as_str()),
        ));
        for state in &self.entered {
            let mut segments = vec!["state".to_owned()];
            segments.extend(state.as_str().split(['.', '/']).map(ToOwned::to_owned));
            segments.push("entered".to_owned());
            facts.push((FactPath::from_segments(segments), FactValue::bool(true)));
        }
        let terminal = self
            .plan
            .workflow
            .state(&self.state)
            .is_some_and(State::is_terminal);
        facts.push((path(&["workflow", "terminal"]), FactValue::bool(terminal)));

        for kind in EvidenceKind::ALL {
            let matching: Vec<usize> = self
                .evidence
                .iter()
                .enumerate()
                .filter(|(_, recorded)| recorded.record.kind() == *kind)
                .map(|(index, _)| index + 1)
                .collect();
            if matching.is_empty() {
                continue;
            }
            let base = path(&["evidence", "count", kind.as_str()]);
            facts.push((base, FactValue::count(matching.len())));
            facts.push((
                path(&["evidence", "first_seq", kind.as_str()]),
                FactValue::count(matching[0]),
            ));
            facts.push((
                path(&["evidence", "last_seq", kind.as_str()]),
                FactValue::count(*matching.last().expect("non-empty")),
            ));
        }

        if let Some(first) =
            self.evidence
                .iter()
                .find_map(|recorded| match &recorded.record.value {
                    Evidence::TestResult(result) => Some(result.status()),
                    _ => None,
                })
        {
            facts.push((
                path(&["test", "first_result"]),
                FactValue::text(first.as_str()),
            ));
        }

        let granted = self
            .evidence
            .iter()
            .filter(|recorded| match &recorded.record.value {
                Evidence::Approval(approval) => approval.decision == ApprovalDecision::Granted,
                _ => false,
            })
            .count();
        facts.push((path(&["approvals", "granted"]), FactValue::count(granted)));

        let missing = self.count_missing_evidence(observed);
        facts.push((path(&["evidence", "missing"]), FactValue::count(missing)));
        facts.push((
            path(&["required_evidence", "missing"]),
            FactValue::count(missing),
        ));

        facts
    }

    /// Counts unmet evidence requirements across the plan.
    ///
    /// Two rules make this number trustworthy:
    ///
    /// * Only *evidence* requirements are counted, never predicates. `evidence.missing` is itself
    ///   read by predicates, and counting those would make the fact depend on its own value.
    /// * A requirement inside a conditional counts only when the condition **holds**. Otherwise a
    ///   rule that does not apply — an approval gate on a task that touches no production — would
    ///   keep `evidence.missing` above zero forever, and the report would say every requirement is
    ///   met while the count said otherwise.
    ///
    /// A conditional whose `when` cannot be evaluated counts as applying: an unknown rule is
    /// treated as in force, never as absent.
    fn count_missing_evidence(&self, observed: &FactStore) -> usize {
        let mut missing = 0;
        let mut count = |requirements: &RequirementSet| {
            for requirement in &requirements.evidence {
                if !self.satisfies_evidence(requirement) {
                    missing += 1;
                }
            }
            for conditional in &requirements.conditional {
                if conditional.when.evaluate(observed) == Truth::False {
                    continue;
                }
                for requirement in &conditional.require.evidence {
                    if !self.satisfies_evidence(requirement) {
                        missing += 1;
                    }
                }
            }
        };

        for obligation in &self.plan.obligations {
            count(&obligation.requires);
        }
        count(&self.plan.completion);
        for principle in &self.plan.principles {
            for requirement in &principle.evidence {
                if !self.satisfies_evidence(requirement) {
                    missing += 1;
                }
            }
        }
        missing
    }

    /// `true` when enough matching evidence has been submitted.
    fn satisfies_evidence(&self, requirement: &EvidenceRequirement) -> bool {
        let matching = self
            .records
            .iter()
            .filter(|record| requirement.matches(record))
            .count();
        matching >= requirement.at_least
    }
}

impl RequirementContext for Execution {
    fn facts(&self) -> &dyn FactSource {
        &self.facts
    }

    fn artifacts(&self) -> &ArtifactGraph {
        &self.artifacts
    }

    fn evidence(&self) -> &[EvidenceRecord] {
        &self.records
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures;
    use crate::resolve::resolve;
    use aep_domain::evidence::{
        ApprovalDecision, ApprovalRecord, ChangeSet, Producer, TestResult, TestSuite,
    };
    use aep_domain::facts::FactPath;
    use aep_domain::ids::EvidenceId;
    use aep_domain::verification::Verifier;

    fn execution() -> Execution {
        let registry = fixtures::standard_registry();
        let plan = resolve(&fixtures::standard_task(), &registry).expect("resolves");
        Execution::new(
            ExecutionId::new("exec.1").expect("id"),
            plan,
            ArtifactGraph::new(),
        )
    }

    fn record(ordinal: usize, producer: Producer, evidence: Evidence) -> EvidenceRecord {
        EvidenceRecord::new(
            EvidenceId::new(format!("e{ordinal}")).expect("id"),
            Timestamp::from_epoch_millis(ordinal as u64),
            producer,
            evidence,
        )
    }

    fn fact(execution: &Execution, path: &str) -> Option<FactValue> {
        execution
            .fact_store()
            .fact(&FactPath::new(path).expect("path"))
    }

    #[test]
    fn starts_in_the_workflows_initial_state() {
        let execution = execution();
        assert_eq!(execution.state_id().as_str(), "implement");
        assert_eq!(
            fact(&execution, "state.current"),
            Some(FactValue::text("implement"))
        );
        assert_eq!(
            fact(&execution, "state.implement.entered"),
            Some(FactValue::bool(true))
        );
        assert_eq!(
            fact(&execution, "workflow.terminal"),
            Some(FactValue::bool(false))
        );
        assert_eq!(
            fact(&execution, "task.kind"),
            Some(FactValue::text("feature")),
            "the task's own facts are part of the plan"
        );
    }

    #[test]
    fn submission_order_is_observable_so_ordering_rules_are_checkable() {
        let mut execution = execution();
        execution.record_evidence(record(
            1,
            Producer::Verifier {
                verifier: Verifier::TestRunner,
            },
            Evidence::TestResult(TestResult::failing(TestSuite::Unit, 0, 1)),
        ));
        execution.record_evidence(record(
            2,
            Producer::Agent {
                id: "opus".to_owned(),
            },
            Evidence::Diff(ChangeSet {
                files_changed: 3,
                lines_added: 40,
                lines_removed: 2,
                revision_before: None,
                revision_after: None,
                paths: Vec::new(),
            }),
        ));

        assert_eq!(
            fact(&execution, "evidence.first_seq.test_result"),
            Some(FactValue::count(1))
        );
        assert_eq!(
            fact(&execution, "evidence.first_seq.diff"),
            Some(FactValue::count(2)),
            "the test ran before the code changed, and the facts say so"
        );
        assert_eq!(
            fact(&execution, "evidence.count.test_result"),
            Some(FactValue::count(1))
        );
    }

    #[test]
    fn the_first_test_result_is_remembered_after_a_later_pass() {
        let mut execution = execution();
        execution.record_evidence(record(
            1,
            Producer::Verifier {
                verifier: Verifier::TestRunner,
            },
            Evidence::TestResult(TestResult::failing(TestSuite::Unit, 0, 1)),
        ));
        execution.record_evidence(record(
            2,
            Producer::Verifier {
                verifier: Verifier::TestRunner,
            },
            Evidence::TestResult(TestResult::passing(TestSuite::Unit, 4)),
        ));

        assert_eq!(
            fact(&execution, "test.result"),
            Some(FactValue::text("passed")),
            "the current result is the latest"
        );
        assert_eq!(
            fact(&execution, "test.first_result"),
            Some(FactValue::text("failed")),
            "the first result cannot be rewritten by a later green run"
        );
    }

    #[test]
    fn counts_unmet_evidence_requirements_without_consulting_predicates() {
        let mut execution = execution();
        assert_eq!(
            fact(&execution, "evidence.missing"),
            Some(FactValue::count(1)),
            "the test-driven principle requires independent test evidence"
        );

        execution.record_evidence(record(
            1,
            Producer::Agent {
                id: "opus".to_owned(),
            },
            Evidence::TestResult(TestResult::passing(TestSuite::Unit, 4)),
        ));
        assert_eq!(
            fact(&execution, "evidence.missing"),
            Some(FactValue::count(1)),
            "an agent's own report is not independent evidence"
        );

        execution.record_evidence(record(
            2,
            Producer::Verifier {
                verifier: Verifier::TestRunner,
            },
            Evidence::TestResult(TestResult::passing(TestSuite::Unit, 4)),
        ));
        assert_eq!(
            fact(&execution, "evidence.missing"),
            Some(FactValue::count(0))
        );
    }

    #[test]
    fn a_snapshot_round_trips_through_a_freshly_resolved_plan() {
        let registry = fixtures::standard_registry();
        let task = fixtures::standard_task();
        let mut execution = execution();
        execution.record_evidence(record(
            1,
            Producer::Verifier {
                verifier: Verifier::TestRunner,
            },
            Evidence::TestResult(TestResult::passing(TestSuite::Unit, 4)),
        ));
        execution.emit(
            Timestamp::from_epoch_millis(5),
            ProtocolEvent::StateEntered {
                state: execution.state_id().clone(),
            },
        );
        let snapshot = execution.snapshot();

        let serialised = serde_yaml::to_string(&snapshot).expect("snapshot serialises");
        let parsed: Snapshot = serde_yaml::from_str(&serialised).expect("snapshot parses");

        let plan = resolve(&task, &registry).expect("resolves");
        let restored = Execution::restore(plan, ArtifactGraph::new(), parsed).expect("restores");

        assert_eq!(restored.state_id(), execution.state_id());
        assert_eq!(restored.recorded_evidence().len(), 1);
        assert_eq!(restored.event_count(), 1);
        assert_eq!(
            fact(&restored, "tests.unit.result"),
            Some(FactValue::text("passed")),
            "facts are rebuilt from the evidence, not carried in the snapshot"
        );
    }

    #[test]
    fn a_refused_approval_is_not_counted_among_the_granted_ones() {
        // `approvals.granted` is a fact completion conditions read — `approvals.granted >= 1` is
        // how a gate is written. Counting the approval that said *no* would let the refusal
        // satisfy the gate it refused, which is the same defect as in `policy::approval_recorded`
        // and a second place it can be introduced.
        let approval = |ordinal: usize, decision| {
            record(
                ordinal,
                Producer::Human {
                    id: "ada".to_owned(),
                },
                Evidence::Approval(ApprovalRecord {
                    approval: "production-change".parse().expect("approval id"),
                    approver: Producer::Human {
                        id: "ada".to_owned(),
                    },
                    decision,
                    subject: Some("capability:production-write".parse().expect("subject")),
                    note: None,
                }),
            )
        };

        let mut execution = execution();
        execution.record_evidence(approval(1, ApprovalDecision::Denied));
        assert_eq!(
            fact(&execution, "approvals.granted"),
            Some(FactValue::count(0)),
            "a reviewer refusing a change has not granted it"
        );

        execution.record_evidence(approval(2, ApprovalDecision::Granted));
        assert_eq!(
            fact(&execution, "approvals.granted"),
            Some(FactValue::count(1)),
            "and the grant that follows counts once, not twice"
        );
    }

    #[test]
    fn a_snapshot_from_another_task_is_refused() {
        let execution = execution();
        let mut snapshot = execution.snapshot();
        snapshot.task = "OTHER-9".to_owned();

        let registry = fixtures::standard_registry();
        let plan = resolve(&fixtures::standard_task(), &registry).expect("resolves");
        let error =
            Execution::restore(plan, ArtifactGraph::new(), snapshot).expect_err("wrong task");
        assert!(error.to_string().contains("OTHER-9"), "{error}");
    }
}
