//! Document fixtures for the engine's own tests.
//!
//! Deliberately small: each test builds only the documents it needs, so a failure points at one
//! rule rather than at a large shared fixture. The real document tree is exercised separately by the
//! integration tests.

use aep_domain::task::Task;

use crate::registry::Registry;

/// A protocol declaring a generous vocabulary, so tests fail on the rule under test rather than on
/// an undeclared capability.
pub const PROTOCOL: &str = r"
id: aep
version: 1
title: Test protocol
capabilities:
  - repository.read
  - repository.write
  - tests.execute
  - command.execute
  - telemetry.read
  - production.read
  - production.write
  - deployment.create
  - deployment.rollback
  - secret.read
  - artifact.read
  - artifact.write
  - review.request
  - approval.request
approval_floor:
  - production.write
  - deployment.create:production
evidence_kinds:
  - test_result
  - static_analysis
  - contract_result
  - property_test_result
  - deployment_result
  - metric_observation
  - health_observation
  - approval
  - diff
  - artifact
  - review
  - verification
  - specification
verifiers:
  - compiler
  - test-runner
  - contract-runner
  - static-analyzer
  - property-tester
  - telemetry-query
  - policy-engine
  - human-approval
  - human-review
  - artifact-validator
artifact_kinds:
  - specification
  - design
  - architecture-design
  - architecture-decision-record
  - review-result
  - story
phases: [implementation, verification, completion]
observables:
  - 'task.**'
  - 'tests.**'
  - 'test.**'
  - 'static_analysis.**'
  - 'contracts.**'
  - 'property_test.**'
  - 'deployment.**'
  - 'metric.**'
  - 'service.**'
  - 'approval.**'
  - 'approvals.**'
  - 'diff.**'
  - 'artifact.**'
  - 'review.**'
  - 'verification.**'
  - 'specification.**'
  - 'evidence.**'
  - 'state.**'
  - 'obligations.**'
  - 'principle.**'
  - 'workflow.**'
  - 'change.**'
  - risk
  - recovery_verified
scales:
  risk: [low, medium, high, critical]
";

/// A three-state workflow with the phases principles time themselves against.
pub const WORKFLOW: &str = r"
id: test/linear
title: Linear
initial: implement
states:
  implement:
    title: Implement
    phases: [implementation]
  verify:
    title: Verify
    phases: [verification]
  complete:
    title: Complete
    terminal: true
    phases: [completion]
transitions:
  - from: implement
    to: verify
    when: diff.exists
  - from: verify
    to: complete
    when: tests.unit.failed == 0
";

/// A profile granting ordinary development capabilities.
pub const PROFILE: &str = r"
id: test.standard
title: Test standard
protocol: aep/1
workflow: test/linear
principles: [test-driven]
capabilities:
  allow: [repository.read, repository.write, tests.execute]
completion:
  - tests.unit.failed == 0
";

/// The test-driven principle, in the form the design document uses.
pub const TEST_DRIVEN: &str = r"
id: test-driven
version: 1
title: Test-driven development
applies_when:
  task.kind: {any_of: [feature, bugfix]}
requires:
  before_completion:
    - tests.unit.failed == 0
evidence:
  - kind: test_result
    independent: true
";

/// Builds a registry from document texts.
///
/// # Panics
///
/// Panics when a fixture document does not parse or does not validate, which is a broken fixture.
pub fn registry(
    protocols: &[&str],
    principles: &[&str],
    workflows: &[&str],
    profiles: &[&str],
) -> Registry {
    let mut registry = Registry::new();
    for text in protocols {
        registry
            .insert_protocol(
                aep_schema::parse::protocol(text, None).expect("fixture protocol parses"),
            )
            .expect("fixture protocol is unique");
    }
    for text in principles {
        registry
            .insert_principle(
                aep_schema::parse::principle(text, None).expect("fixture principle parses"),
            )
            .expect("fixture principle is unique");
    }
    for text in workflows {
        registry
            .insert_workflow(
                aep_schema::parse::workflow(text, None).expect("fixture workflow parses"),
            )
            .expect("fixture workflow is unique");
    }
    for text in profiles {
        registry
            .insert_profile(aep_schema::parse::profile(text, None).expect("fixture profile parses"))
            .expect("fixture profile is unique");
    }
    registry
}

/// A registry with the standard fixture documents.
pub fn standard_registry() -> Registry {
    registry(&[PROTOCOL], &[TEST_DRIVEN], &[WORKFLOW], &[PROFILE])
}

/// Parses a task document.
///
/// # Panics
///
/// Panics when the fixture does not parse or validate.
pub fn task(text: &str) -> Task {
    aep_schema::parse::task(text, None).expect("fixture task parses")
}

/// A feature task using the standard fixture profile.
pub fn standard_task() -> Task {
    task(
        r"
id: T-1
kind: feature
objective: add something
protocol: aep/1
profile: test.standard
",
    )
}
