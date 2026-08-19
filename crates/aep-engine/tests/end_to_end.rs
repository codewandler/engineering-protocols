//! The wave's acceptance test: the documents, the engine and the example, together.
//!
//! Each test here corresponds to a claim the design specification makes. If one of these fails, the
//! project does not do the thing it exists to do — which is why they assert on the *reason* given,
//! not merely on a boolean.

use std::fs;
use std::path::{Path, PathBuf};

use aep_domain::action::{Action, ActionRequest, ProductionMutate};
use aep_domain::artifact::ArtifactGraph;
use aep_domain::capability::{CapabilityDecision, PolicySource};
use aep_domain::evidence::{
    ChangeSet, ContractResult, Evidence, Producer, PropertyTestResult, SpecificationRecord,
    StaticAnalysisResult, TestResult, TestSuite,
};
use aep_domain::facts::FactSource;
use aep_domain::review::{ReviewDisposition, ReviewResult, Reviewer};
use aep_domain::task::Task;
use aep_domain::verification::{VerificationStatus, Verifier};
use aep_engine::engine::{EvidenceSubmission, ProtocolEngine, TransitionResult};
use aep_engine::{load_tree, Engine, FixedClock, Registry};

/// The repository root.
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the workspace root exists")
}

/// The example directory.
fn example(file: &str) -> PathBuf {
    root().join("examples/development-passkeys").join(file)
}

/// The repository's document tree.
fn registry() -> Registry {
    load_tree(&root()).expect("the document tree is valid")
}

/// The example task, optionally with a different profile.
fn task(profile: Option<&str>) -> Task {
    let text = fs::read_to_string(example("task.yaml")).expect("the example task is readable");
    let text = match profile {
        Some(profile) => text.replace(
            "profile: development.standard",
            &format!("profile: {profile}"),
        ),
        None => text,
    };
    aep_schema::parse::task(&text, Some("examples/development-passkeys/task.yaml"))
        .expect("the example task is valid")
}

/// The example artifact graph.
fn artifacts() -> ArtifactGraph {
    let text = fs::read_to_string(example("artifacts.yaml")).expect("the manifest is readable");
    aep_schema::parse::artifact_manifest(
        &text,
        Some("examples/development-passkeys/artifacts.yaml"),
    )
    .expect("the example manifest is valid")
}

/// An engine with a fixed clock, so every run is reproducible.
fn engine() -> Engine<FixedClock> {
    Engine::with_clock(registry(), FixedClock::new(1_700_000_000_000))
}

fn by_runner(evidence: Evidence) -> EvidenceSubmission {
    EvidenceSubmission::new(
        evidence,
        Producer::Verifier {
            verifier: Verifier::TestRunner,
        },
    )
}

fn by_verifier(evidence: Evidence, verifier: Verifier) -> EvidenceSubmission {
    EvidenceSubmission::new(evidence, Producer::Verifier { verifier })
}

fn by_agent(evidence: Evidence) -> EvidenceSubmission {
    EvidenceSubmission::new(
        evidence,
        Producer::Agent {
            id: "opus-5".to_owned(),
        },
    )
}

fn failing_unit_test() -> Evidence {
    Evidence::TestResult(TestResult::failing(TestSuite::Unit, 0, 1))
}

fn passing_unit_test() -> Evidence {
    Evidence::TestResult(TestResult::passing(TestSuite::Unit, 34))
}

fn diff() -> Evidence {
    Evidence::Diff(ChangeSet {
        files_changed: 6,
        lines_added: 214,
        lines_removed: 18,
        revision_before: None,
        revision_after: None,
        paths: vec!["crates/auth/src/passkey.rs".to_owned()],
    })
}

fn contracts(failed: usize) -> Evidence {
    Evidence::ContractResult(ContractResult {
        checked: 4,
        failed,
        breaking_changes: 0,
        consumer: None,
        provider: None,
    })
}

fn static_analysis() -> Evidence {
    Evidence::StaticAnalysis(StaticAnalysisResult {
        tool: None,
        errors: 0,
        warnings: 3,
    })
}

/// Walks as far as the evidence allows, returning the states visited.
fn advance(engine: &Engine<FixedClock>, execution: &mut aep_engine::Execution) -> Vec<String> {
    let mut visited = vec![execution.state_id().to_string()];
    for _ in 0..12 {
        match engine.transition(execution) {
            Ok(TransitionResult::Moved { to, .. }) => visited.push(to.to_string()),
            Ok(_) | Err(_) => break,
        }
    }
    visited
}

#[test]
fn the_example_walks_to_completion_on_its_own_evidence() {
    // The strongest statement this repository can make about itself: the documents, the engine and
    // the worked example agree, and a task with the right evidence actually finishes.
    let engine = engine();
    let mut execution = engine
        .initialize_with_artifacts(task(None), artifacts())
        .expect("initialises");

    let directory = example("evidence");
    let mut files: Vec<PathBuf> = fs::read_dir(&directory)
        .expect("the evidence directory is readable")
        .map(|entry| entry.expect("a readable entry").path())
        .collect();
    files.sort();
    assert!(
        files.len() >= 5,
        "the example carries a full evidence sequence"
    );

    for file in files {
        let text = fs::read_to_string(&file).expect("the evidence file is readable");
        let inputs = aep_schema::parse::evidence_list(&text, Some(&file.display().to_string()))
            .unwrap_or_else(|error| panic!("{} is not valid evidence: {error}", file.display()));
        for input in inputs {
            let mut submission = EvidenceSubmission::new(input.evidence, input.producer);
            submission.subject = input.about;
            if let Some(provenance) = input.provenance {
                submission.provenance = provenance;
            }
            engine
                .submit_evidence(&mut execution, submission)
                .unwrap_or_else(|error| panic!("{} was refused: {error}", file.display()));
        }
        // Advance as far as this evidence allows before reading the next file, which is what a
        // harness does: submit what it has, then ask whether anything can move.
        let mut seen = vec![execution.state_id().clone()];
        while let Ok(TransitionResult::Moved { to, .. }) = engine.transition(&mut execution) {
            if seen.contains(&to) {
                break;
            }
            seen.push(to);
        }
    }

    let evaluation = engine.evaluate(&execution);
    assert!(
        evaluation.is_complete,
        "the example must reach completion, or it teaches a workflow nobody can finish:\n{}",
        engine.explain_completion(&execution)
    );
    assert_eq!(execution.state_id().as_str(), "complete");
}

#[test]
fn a_conditional_requirement_that_does_not_apply_is_not_counted_as_missing() {
    // `evidence.missing` is read by completion conditions, so a rule that does not apply must not
    // hold the count above zero. The symptom this guards against is a report where every requirement
    // is ticked and the task still cannot finish.
    let engine = engine();
    let mut execution = engine
        .initialize_with_artifacts(task(None), artifacts())
        .expect("initialises");

    let approval_gate = execution
        .plan()
        .obligations
        .iter()
        .find(|obligation| obligation.principle.as_str() == "approval-gates")
        .expect("development.standard includes approval-gates");
    assert!(
        !approval_gate.requires.conditional.is_empty(),
        "the guard only means something while approval-gates is conditional"
    );

    for file in [
        "evidence/01-red-test.yaml",
        "evidence/02-implementation.yaml",
        "evidence/03-verification.yaml",
        "evidence/05-provenance.yaml",
    ] {
        let text = fs::read_to_string(example(file)).expect("readable");
        for input in aep_schema::parse::evidence_list(&text, None).expect("valid") {
            let mut submission = EvidenceSubmission::new(input.evidence, input.producer);
            submission.subject = input.about;
            engine
                .submit_evidence(&mut execution, submission)
                .expect("recorded");
        }
    }

    let missing = execution
        .fact_store()
        .fact(&"evidence.missing".parse().expect("path"))
        .expect("the engine always derives this fact");
    assert_eq!(
        missing,
        aep_domain::facts::FactValue::count(0),
        "this task touches no production, so the approval gate's evidence is not owed"
    );
}

#[test]
fn the_example_documents_are_valid_and_resolve() {
    let plan = aep_engine::resolve(&task(None), &registry()).expect("the example task resolves");
    assert_eq!(plan.profile.id.as_str(), "development.standard");
    assert_eq!(plan.workflow.id.as_str(), "adp/default");
    assert!(
        plan.principles.len() >= 9,
        "development.standard inherits development.fast's principles: {:?}",
        plan.principles
            .iter()
            .map(|principle| principle.id.to_string())
            .collect::<Vec<_>>()
    );
    let graph = artifacts();
    assert!(graph
        .validate_lifecycles(registry().lifecycles())
        .is_empty());
}

#[test]
fn work_cannot_be_decomposed_before_a_specification_exists() {
    let engine = engine();

    // With no artifact graph, nothing has observed a specification.
    let mut without = engine.initialize(task(None)).expect("initialises");
    let visited = advance(&engine, &mut without);
    assert_eq!(
        visited,
        vec!["receive", "specify"],
        "intake proceeds, decomposition does not"
    );
    let blocked = engine
        .evaluate(&without)
        .transitions
        .iter()
        .flat_map(aep_engine::evaluate::TransitionEvaluation::unmet)
        .collect::<Vec<_>>();
    assert!(
        blocked
            .iter()
            .any(|reason| reason.contains("artifact.specification.exists")),
        "the refusal must name the missing specification: {blocked:?}"
    );

    // With the example manifest, the same execution moves on.
    let mut with = engine
        .initialize_with_artifacts(task(None), artifacts())
        .expect("initialises");
    let visited = advance(&engine, &mut with);
    assert!(
        visited.contains(&"establish_verifiers".to_owned()),
        "an approved specification unblocks decomposition: {visited:?}"
    );
}

#[test]
fn a_passing_test_submitted_before_any_code_fails_red_before_green() {
    let engine = engine();

    // A test that has never failed does not establish that it can fail.
    let mut green_first = engine
        .initialize_with_artifacts(task(None), artifacts())
        .expect("initialises");
    advance(&engine, &mut green_first);
    assert_eq!(green_first.state_id().as_str(), "establish_verifiers");
    engine
        .submit_evidence(&mut green_first, by_runner(passing_unit_test()))
        .expect("recorded");

    let evaluation = engine.evaluate(&green_first);
    let unmet: Vec<String> = evaluation
        .transitions
        .iter()
        .flat_map(aep_engine::evaluate::TransitionEvaluation::unmet)
        .collect();
    assert!(
        !evaluation
            .transitions
            .iter()
            .any(|transition| transition.permitted),
        "implementation must not begin on a test that was green from the start: {unmet:?}"
    );
    assert!(
        unmet
            .iter()
            .any(|reason| reason.contains("test.first_result == failed")),
        "the refusal must name the red-first rule: {unmet:?}"
    );

    // A test that failed first does.
    let mut red_first = engine
        .initialize_with_artifacts(task(None), artifacts())
        .expect("initialises");
    advance(&engine, &mut red_first);
    engine
        .submit_evidence(&mut red_first, by_runner(failing_unit_test()))
        .expect("recorded");
    assert!(
        engine
            .transition(&mut red_first)
            .expect("evaluates")
            .moved(),
        "a failing test is what licenses implementation"
    );
    assert_eq!(red_first.state_id().as_str(), "implement");
}

#[test]
fn failing_contracts_send_the_work_back_to_implementation() {
    let engine = engine();
    let mut execution = engine
        .initialize_with_artifacts(task(None), artifacts())
        .expect("initialises");
    advance(&engine, &mut execution);
    engine
        .submit_evidence(&mut execution, by_runner(failing_unit_test()))
        .expect("recorded");
    assert!(engine
        .transition(&mut execution)
        .expect("evaluates")
        .moved());
    assert_eq!(execution.state_id().as_str(), "implement");

    engine
        .submit_evidence(&mut execution, by_agent(diff()))
        .expect("recorded");
    // One transition at a time from here: the failing unit suite is still the most recent test
    // result, so walking greedily would take the back-edge straight away and hide what is being
    // tested.
    assert!(engine
        .transition(&mut execution)
        .expect("evaluates")
        .moved());
    assert_eq!(execution.state_id().as_str(), "verify");

    // Unit tests green, static analysis clean, but a consumer's contract is broken.
    engine
        .submit_evidence(&mut execution, by_runner(passing_unit_test()))
        .expect("recorded");
    engine
        .submit_evidence(
            &mut execution,
            by_verifier(static_analysis(), Verifier::StaticAnalyzer),
        )
        .expect("recorded");
    engine
        .submit_evidence(
            &mut execution,
            by_verifier(contracts(2), Verifier::ContractRunner),
        )
        .expect("recorded");

    let evaluation = engine.evaluate(&execution);
    let adversarial = evaluation
        .transitions
        .iter()
        .find(|transition| transition.to.as_str() == "adversarial_verify")
        .expect("the workflow offers adversarial verification");
    assert!(
        !adversarial.permitted,
        "a broken contract is not verified work"
    );
    assert!(
        adversarial
            .unmet()
            .iter()
            .any(|reason| reason.contains("tests.contract.failed")),
        "the refusal must name the contract failure: {:?}",
        adversarial.unmet()
    );

    let result = engine.transition(&mut execution).expect("evaluates");
    assert_eq!(
        result,
        TransitionResult::Moved {
            from: "verify".parse().expect("state"),
            to: "implement".parse().expect("state"),
            also_permitted: Vec::new(),
        },
        "the back-edge is taken: verification failing means more work, not a weaker check"
    );
}

#[test]
fn an_approval_of_design_version_three_does_not_cover_version_seven() {
    let engine = engine();
    let mut execution = engine
        .initialize_with_artifacts(task(Some("development.critical")), artifacts())
        .expect("initialises");

    // The example's review approved version 3; the manifest's design is at version 7.
    engine
        .submit_evidence(
            &mut execution,
            EvidenceSubmission::new(
                Evidence::Review(ReviewResult {
                    subject: "design:passkeys-auth".parse().expect("reference"),
                    subject_kind: Some(aep_domain::artifact::ArtifactKind::Design),
                    reviewer: Reviewer::Human {
                        id: "ada".to_owned(),
                    },
                    disposition: ReviewDisposition::Approved,
                    findings: Vec::new(),
                    reviewed_version: Some(aep_domain::artifact::ArtifactVersion::new("3")),
                    reviewed_revision: None,
                }),
                Producer::Human {
                    id: "ada".to_owned(),
                },
            ),
        )
        .expect("recorded");

    let explanation = engine.explain_completion(&execution);
    let stale = explanation
        .items
        .iter()
        .find(|item| item.requirement.contains("review of a design"))
        .expect("development.critical requires a human design review");
    assert!(
        !stale.satisfied,
        "an approval of version 3 must not authorise version 7"
    );
    assert!(
        stale
            .detail
            .as_deref()
            .unwrap_or_default()
            .contains("different version"),
        "the reason must say the approval was given against another version: {stale:?}"
    );

    // The same review against version 7 satisfies it.
    let mut current = engine
        .initialize_with_artifacts(task(Some("development.critical")), artifacts())
        .expect("initialises");
    engine
        .submit_evidence(
            &mut current,
            EvidenceSubmission::new(
                Evidence::Review(ReviewResult {
                    subject: "design:passkeys-auth".parse().expect("reference"),
                    subject_kind: Some(aep_domain::artifact::ArtifactKind::Design),
                    reviewer: Reviewer::Human {
                        id: "ada".to_owned(),
                    },
                    disposition: ReviewDisposition::Approved,
                    findings: Vec::new(),
                    reviewed_version: Some(aep_domain::artifact::ArtifactVersion::new("7")),
                    reviewed_revision: None,
                }),
                Producer::Human {
                    id: "ada".to_owned(),
                },
            ),
        )
        .expect("recorded");
    let fresh = engine.explain_completion(&current);
    assert!(
        fresh
            .items
            .iter()
            .find(|item| item.requirement.contains("review of a design"))
            .expect("the requirement is still listed")
            .satisfied,
        "{fresh}"
    );
}

#[test]
fn changing_production_is_refused_and_names_the_rule_that_refused_it() {
    let engine = engine();
    let mut execution = engine
        .initialize_with_artifacts(task(None), artifacts())
        .expect("initialises");

    let request = ActionRequest::new(Action::ProductionMutate(ProductionMutate {
        target: "auth.passkeys_enabled".to_owned(),
        change: Some("enable".to_owned()),
    }));
    let decision = engine.authorize(&mut execution, &request);

    assert!(!decision.is_allowed());
    assert_eq!(decision.decision, CapabilityDecision::RequiresApproval);
    let reason = decision.reason.clone().expect("the refusal is attributed");
    assert_eq!(reason.rule, "production-write-requires-approval");
    assert_eq!(
        reason.source,
        PolicySource::Principle {
            principle: "approval-gates".parse().expect("id")
        },
        "the refusal names the last principle to speak about the capability — the one whose rule \
         actually decided — so a person can go and read it"
    );
    assert_eq!(
        decision.missing,
        vec!["approval for capability production.write".to_owned()]
    );

    let rendered = Engine::<FixedClock>::explain_decision(&decision).to_string();
    assert!(rendered.contains("production.write denied"), "{rendered}");
    assert!(rendered.contains("approval-gates"), "{rendered}");
}

#[test]
fn completion_is_refused_with_the_missing_requirements_named() {
    let engine = engine();
    let mut execution = engine
        .initialize_with_artifacts(task(Some("development.critical")), artifacts())
        .expect("initialises");

    let explanation = engine.explain_completion(&execution);
    assert!(!explanation.complete);
    let outstanding: Vec<String> = explanation
        .outstanding()
        .map(|item| item.requirement.clone())
        .collect();

    for expected in [
        "tests.unit.failed == 0",
        "tests.mutation.failed == 0",
        "tests.differential.failed == 0",
        "verification.invariant.passed",
        "specification.satisfied",
        "contracts.failed == 0",
    ] {
        assert!(
            outstanding.iter().any(|item| item == expected),
            "`{expected}` must be listed as outstanding: {outstanding:?}"
        );
    }

    let rendered = explanation.to_string();
    assert!(rendered.contains("Task incomplete"), "{rendered}");
    assert!(
        rendered.contains('?'),
        "unobserved requirements are marked `?`, not `✗`: {rendered}"
    );

    // Evidence closes the specific requirements it speaks to, and nothing else.
    engine
        .submit_evidence(
            &mut execution,
            by_verifier(
                Evidence::Specification(SpecificationRecord {
                    artifact: Some("spec:passkeys-auth".parse().expect("reference")),
                    satisfied: true,
                    requirements_total: Some(4),
                    requirements_satisfied: Some(4),
                    unsatisfied: Vec::new(),
                }),
                Verifier::TestRunner,
            ),
        )
        .expect("recorded");
    engine
        .submit_evidence(
            &mut execution,
            by_verifier(
                Evidence::PropertyTestResult(PropertyTestResult {
                    property: "session-isolation".parse().expect("claim"),
                    cases: 10_000,
                    status: VerificationStatus::Passed,
                    counterexamples: Vec::new(),
                }),
                Verifier::PropertyTester,
            ),
        )
        .expect("recorded");

    let after = engine.explain_completion(&execution);
    let still_outstanding: Vec<String> = after
        .outstanding()
        .map(|item| item.requirement.clone())
        .collect();
    assert!(
        !still_outstanding.contains(&"specification.satisfied".to_owned()),
        "the specification requirement is satisfied by the record that speaks to it"
    );
    assert!(
        still_outstanding.contains(&"tests.mutation.failed == 0".to_owned()),
        "and nothing else is: {still_outstanding:?}"
    );
}
