//! The rules that decide whether an agent may touch production, exercised from the state in which
//! each rule is load-bearing.
//!
//! Tests normally live beside the code they protect. These two do not, for one reason: they cover
//! rules in `protocol.rs` and `requirement.rs`, and this file was written while those modules were
//! being changed elsewhere. Everything they need is public, so nothing is lost by testing them
//! across the crate boundary — the fixtures are built the way a caller builds them, from a
//! document and from recorded evidence.
//!
//! Both are instances of one defect class a mutation review named
//! (`docs/reviews/2026-08-20-guard-efficacy-review.md`): the rule is correct, its doc comment
//! states it, and no fixture anywhere constructs the state in which the rule decides anything. So
//! each test asserts that its fixture reached that state before asserting the rule.

use aep_domain::evidence::{ApprovalDecision, ApprovalRecord};
use aep_domain::raw::RawProtocol;
use aep_domain::requirement::RequirementFlavour;
use aep_domain::{
    ApprovalId, ApprovalRequirement, ArtifactGraph, Capability, Environment, Evidence, EvidenceId,
    EvidenceRecord, FactPath, FactSource, FactStore, FactValue, Producer, Protocol,
    RequirementContext, RequirementSet, SubjectRef, Timestamp, Truth,
};

/// A protocol whose approval floor names production deployment at its most specific spelling.
///
/// This is the shape of the floor `protocols/aep/1.yaml` ships, written inline so the test states
/// its own premise rather than depending on a document that may be edited for other reasons.
const PROTOCOL: &str = r"
id: aep
version: 1
title: Agentic Engineering Protocol
capabilities: [repository.read, production.write, deployment.create]
approval_floor: [production.write, 'deployment.create:production']
evidence_kinds: [approval]
verifiers: [human-approval]
observables: ['tests.**', 'task.**']
";

fn protocol() -> Protocol {
    let raw: RawProtocol = serde_yaml::from_str(PROTOCOL).expect("the document parses");
    Protocol::try_from(raw).expect("the document validates")
}

#[test]
fn a_floor_on_production_deployment_catches_a_profile_that_grants_every_environment() {
    let protocol = protocol();

    assert!(
        protocol
            .approval_floor
            .contains(&Capability::Deploy(Environment::Production))
            && !protocol
                .approval_floor
                .contains(&Capability::Deploy(Environment::Any)),
        "the floor must name production specifically and not the wildcard, or the direction under \
         test is not the one being exercised: {:?}",
        protocol.approval_floor
    );

    assert!(
        protocol.needs_approval_floor(&Capability::Deploy(Environment::Any)),
        "`allow: [deployment.create]` is the lazy spelling of a grant that includes production; a \
         floor that only the careful spelling `deployment.create:production` trips protects \
         nobody, because the lazy spelling is the one people write"
    );
    assert!(
        protocol.needs_approval_floor(&Capability::Deploy(Environment::Production)),
        "the floor entry still catches the capability it names exactly"
    );
    assert!(
        protocol.needs_approval_floor(&Capability::ProductionWrite),
        "a floor entry that takes no environment still catches its own capability"
    );
    assert!(
        !protocol.needs_approval_floor(&Capability::Deploy(Environment::Staging)),
        "overlap is not the same as everything: staging deployment neither covers nor is covered \
         by a production floor, so it stays grantable outright"
    );
    assert!(
        !protocol.needs_approval_floor(&Capability::RepositoryRead),
        "a capability the floor says nothing about is unaffected by it"
    );
}

/// Everything the requirement layer reads, with only recorded evidence supplied.
struct Recorded {
    facts: FactStore,
    artifacts: ArtifactGraph,
    evidence: Vec<EvidenceRecord>,
}

impl Recorded {
    fn holding(evidence: EvidenceRecord) -> Self {
        let mut facts = FactStore::new();
        facts.extend_facts(evidence.facts());
        Self {
            facts,
            artifacts: ArtifactGraph::default(),
            evidence: vec![evidence],
        }
    }

    fn fact(&self, path: &str) -> Option<FactValue> {
        self.facts
            .fact(&FactPath::new(path).expect("a well-formed fact path"))
    }
}

impl RequirementContext for Recorded {
    fn facts(&self) -> &dyn FactSource {
        &self.facts
    }

    fn artifacts(&self) -> &ArtifactGraph {
        &self.artifacts
    }

    fn evidence(&self) -> &[EvidenceRecord] {
        &self.evidence
    }
}

/// A person read the change, decided against it, and recorded that refusal against the capability
/// it was about — `production.write`, spelled the way the engine slugs it into a subject.
fn refusal() -> EvidenceRecord {
    let alice = Producer::Human {
        id: "alice".to_owned(),
    };
    EvidenceRecord::new(
        EvidenceId::new("approval-4711").expect("an evidence id"),
        Timestamp::from_epoch_millis(1_700_000_000_000),
        alice.clone(),
        Evidence::Approval(ApprovalRecord {
            approval: ApprovalId::new("production-write").expect("an approval id"),
            approver: alice,
            decision: ApprovalDecision::Denied,
            subject: Some(SubjectRef::new("capability:production-write").expect("a subject")),
            note: Some("the blast radius is not bounded".to_owned()),
        }),
    )
}

#[test]
fn a_recorded_refusal_does_not_satisfy_the_approval_it_was_recorded_against() {
    let context = Recorded::holding(refusal());
    let Evidence::Approval(record) = &context.evidence[0].value else {
        panic!("the fixture is an approval");
    };
    assert_eq!(
        record.decision,
        ApprovalDecision::Denied,
        "the fixture must be a refusal, or the rule under test never applies: nothing else in \
         this workspace records an approval that was denied"
    );

    let required = RequirementSet {
        approvals: vec![ApprovalRequirement::new(
            ApprovalId::new("production-write").expect("an approval id"),
        )],
        ..RequirementSet::empty()
    };
    let report = required.evaluate(&context);

    let outcome = report.items.first().expect("one requirement was checked");
    assert_eq!(outcome.flavour, RequirementFlavour::Approval);
    assert_eq!(
        outcome.truth,
        Truth::False,
        "a reviewer who reads a change, refuses it and records the refusal has not thereby \
         approved it; recording a refusal must never be the thing that unlocks the action"
    );
    assert_eq!(
        outcome.detail.as_deref(),
        Some("the approval was refused"),
        "and the reason must say the approval was refused, not that none was found: waiting and \
         being told no are different situations for whoever reads this"
    );
    assert_eq!(report.truth, Truth::False);
}

#[test]
fn a_refused_approval_projects_a_grant_fact_that_is_false() {
    let context = Recorded::holding(refusal());

    assert_eq!(
        context.fact("approval.production-write.granted"),
        Some(FactValue::bool(false)),
        "the fact a predicate reads to decide whether an approval exists must be false for a \
         refusal; a guard written `approval.production-write.granted` would otherwise pass on the \
         record that refused it"
    );
    assert_eq!(
        context.fact("approval.production-write.decision"),
        Some(FactValue::text("denied")),
        "and the decision is reported as denied rather than absent"
    );
    assert_eq!(
        context.fact("approval.production-write.by_human"),
        Some(FactValue::bool(true)),
        "a refusal by a person is still attributed to a person"
    );
}
