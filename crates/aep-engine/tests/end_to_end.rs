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
    ChangeSet, ContractResult, Evidence, Producer, PropertyTestResult, SpecDigest,
    SpecificationRecord, StaticAnalysisResult, TestResult, TestSuite,
};
use aep_domain::facts::FactSource;
use aep_domain::review::{ReviewDisposition, ReviewResult, Reviewer};
use aep_domain::task::Task;
use aep_domain::verification::{Seed, VerificationStatus, Verifier};
use aep_engine::engine::{EvidenceSubmission, ProtocolEngine, TransitionResult};
use aep_engine::{load_tree, Engine, FixedClock, Registry};

/// The digest of the resolved `billing/v3` model a conformance suite would be generated from.
///
/// A literal rather than a computed value, deliberately: `aep-engine` does not depend on `ess-gen`
/// and should not, because the engine consumes evidence and never produces it. A real runner reads
/// this from `ess_gen::Provenance::of` (`crates/ess-gen/src/provenance.rs:38`), which is the same
/// digest every generated artifact already carries in its header.
const BILLING_DIGEST: &str = "4e1d3f8a9b2c1d0e";

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
fn a_specification_governed_task_is_not_finished_until_something_else_says_it_conforms() {
    // The loop the vision describes, exercised end to end before any of ESS exists: a task whose
    // artifact graph holds an executable system specification owes conformance evidence, and the
    // protocol refuses to call it complete without a run by something other than the agent.
    use aep_domain::artifact::{
        Artifact, ArtifactId, ArtifactKind, ArtifactLocation, ArtifactStatus,
    };

    let engine = engine();
    let mut graph = artifacts();
    // The artifact, the label on the evidence and the digest name one specification. They used to
    // name two — an unversioned `ess:billing` in the graph against a `billing/v3` in the evidence,
    // with nothing checking either against the other. The graph now also pins the *revision*: an
    // ESS artifact with no `model_digest` is a specification no run can be checked against, and
    // the requirement refuses rather than assumes — which is what the last assertion below shows.
    graph.insert(
        Artifact::new(
            ArtifactId::new("ess:billing/v3").expect("id"),
            ArtifactKind::ExecutableSystemSpecification,
            ArtifactStatus::Approved,
            ArtifactLocation::Inline,
        )
        .with_model_digest(SpecDigest::new(BILLING_DIGEST).expect("a digest")),
    );

    let mut execution = engine
        .initialize_with_artifacts(task(Some("development.critical")), graph)
        .expect("initialises");

    let outstanding = |execution: &aep_engine::Execution| {
        engine
            .explain_completion(execution)
            .outstanding()
            .map(|item| item.requirement.clone())
            .collect::<Vec<_>>()
    };
    assert!(
        outstanding(&execution)
            .iter()
            .any(|item| item.contains("ess_conformance")),
        "with a specification in the graph, conformance to it is owed: {:?}",
        outstanding(&execution)
    );

    // An agent's own report that it conforms is not a conformance run.
    engine
        .submit_evidence(
            &mut execution,
            by_agent(Evidence::EssConformance(
                aep_domain::evidence::EssConformanceResult {
                    specification: "billing/v3".to_owned(),
                    spec_digest: SpecDigest::new(BILLING_DIGEST).expect("a digest"),
                    implementation: "invoice-service".to_owned(),
                    status: VerificationStatus::Passed,
                    scenarios_total: 24,
                    scenarios_failed: 0,
                    suite_version: None,
                    compiler_version: None,
                    generator_version: None,
                    failed_scenarios: Vec::new(),
                },
            )),
        )
        .expect("recorded");
    assert!(
        outstanding(&execution)
            .iter()
            .any(|item| item.contains("evidence ess_conformance")),
        "an agent asserting its own conformance leaves the requirement owed: {:?}",
        outstanding(&execution)
    );

    // A conformance runner's report satisfies it.
    engine
        .submit_evidence(
            &mut execution,
            by_verifier(
                Evidence::EssConformance(aep_domain::evidence::EssConformanceResult {
                    specification: "billing/v3".to_owned(),
                    spec_digest: SpecDigest::new(BILLING_DIGEST).expect("a digest"),
                    implementation: "invoice-service".to_owned(),
                    status: VerificationStatus::Passed,
                    scenarios_total: 24,
                    scenarios_failed: 0,
                    suite_version: Some("1".to_owned()),
                    compiler_version: Some("0.3.0".to_owned()),
                    generator_version: Some("0.3.0".to_owned()),
                    failed_scenarios: Vec::new(),
                }),
                Verifier::ConformanceRunner,
            ),
        )
        .expect("recorded");
    assert!(
        !outstanding(&execution)
            .iter()
            .any(|item| item.contains("ess_conformance")),
        "a conformance run by a runner closes it: {:?}",
        outstanding(&execution)
    );
}

#[test]
fn a_conformance_run_against_an_older_revision_leaves_the_requirement_owed() {
    // Gate G19. The evidence names the right specification, comes from a real conformance runner,
    // reports every scenario green — and was produced against a resolution of `billing/v3` that no
    // longer exists. Nothing about the record is malformed; it is a true statement about a model
    // nobody is building against any more, and the task must not close on it.
    //
    // The engine half of `conformance_evidence_from_an_older_revision_does_not_satisfy_a_current_requirement`
    // in `aep-domain`, and the same shape as an approval of version three not covering version
    // seven, one flavour of requirement down.
    use aep_domain::artifact::{
        Artifact, ArtifactId, ArtifactKind, ArtifactLocation, ArtifactStatus,
    };

    /// The resolution the suite in this test was generated from, before the model moved on.
    const YESTERDAY: &str = "0badc0ffee123456";

    let engine = engine();
    let mut graph = artifacts();
    graph.insert(
        Artifact::new(
            ArtifactId::new("ess:billing/v3").expect("id"),
            ArtifactKind::ExecutableSystemSpecification,
            ArtifactStatus::Approved,
            ArtifactLocation::Inline,
        )
        .with_model_digest(SpecDigest::new(BILLING_DIGEST).expect("a digest")),
    );

    let mut execution = engine
        .initialize_with_artifacts(task(Some("development.critical")), graph)
        .expect("initialises");

    let run = |digest: &str| {
        by_verifier(
            Evidence::EssConformance(aep_domain::evidence::EssConformanceResult {
                specification: "billing/v3".to_owned(),
                spec_digest: SpecDigest::new(digest).expect("a digest"),
                implementation: "invoice-service".to_owned(),
                status: VerificationStatus::Passed,
                scenarios_total: 24,
                scenarios_failed: 0,
                suite_version: Some("1".to_owned()),
                compiler_version: Some("0.3.0".to_owned()),
                generator_version: Some("0.3.0".to_owned()),
                failed_scenarios: Vec::new(),
            }),
            Verifier::ConformanceRunner,
        )
    };

    // The fixture has to reach the state the rule is about: the run is otherwise flawless, so if
    // the assertion below holds it can only be the revision that made it hold.
    assert_ne!(
        YESTERDAY, BILLING_DIGEST,
        "the fixture must name two different resolutions or it tests nothing"
    );

    engine
        .submit_evidence(&mut execution, run(YESTERDAY))
        .expect("recorded");
    let owed = engine.explain_completion(&execution);
    // Only the conformance lines: the completion explanation carries three dozen items here, and a
    // failure message that prints all of them is a failure message nobody reads.
    let conformance: Vec<&aep_engine::explain::ExplainedItem> = owed
        .items
        .iter()
        .filter(|item| item.requirement.contains("ess_conformance"))
        .collect();
    let stale = conformance
        .iter()
        .find(|item| !item.satisfied && item.requirement.contains("evidence ess_conformance"))
        .unwrap_or_else(|| {
            panic!(
                "a run against another revision must leave conformance owed, but every \
                 conformance requirement is met: {conformance:#?}"
            )
        });
    assert_eq!(
        stale.truth,
        aep_domain::predicate::Truth::False,
        "an older revision is contradicted, not merely unobserved: {stale:?}"
    );
    assert!(
        stale
            .detail
            .as_deref()
            .is_some_and(|detail| detail.contains(YESTERDAY) && detail.contains(BILLING_DIGEST)),
        "the refusal must name both revisions so a person knows what to re-run: {stale:?}"
    );

    // And the same suite re-run against the model that exists now closes it.
    engine
        .submit_evidence(&mut execution, run(BILLING_DIGEST))
        .expect("recorded");
    let settled = engine.explain_completion(&execution);
    let still_owed: Vec<&aep_engine::explain::ExplainedItem> = settled
        .outstanding()
        .filter(|item| item.requirement.contains("ess_conformance"))
        .collect();
    assert!(
        still_owed.is_empty(),
        "a run against the current revision satisfies it: {still_owed:#?}"
    );
}

/// A protocol, workflow and task for a task governed by one named specification.
///
/// Local to this test rather than reusing the repository tree, because the point is a completion
/// condition that *pins a digest* — and no shipped profile does that yet. See the report on gate
/// G11: making `principles/verification/ess-conformance.yaml` pin it is the remaining step.
const GOVERNED_PROTOCOL: &str = r"
id: aep
version: 1
title: Test protocol
capabilities: [repository.read]
evidence_kinds: [ess_conformance]
verifiers: [conformance-runner]
phases: [completion]
observables:
  - 'task.**'
  - 'ess_conformance.**'
  - 'evidence.**'
";

const GOVERNED_WORKFLOW: &str = r"
id: test/linear
title: Linear
initial: implement
states:
  implement:
    title: Implement
  complete:
    title: Complete
    terminal: true
    phases: [completion]
transitions:
  - from: implement
    to: complete
    when: ess_conformance.passed
";

const GOVERNED_TASK: &str = r"
id: T-1
kind: feature
objective: implement the billing specification
protocol: aep/1
profile: test.governed
";

/// An engine whose only profile completes when the run names `digest`.
fn engine_pinned_to(digest: &str) -> Engine<FixedClock> {
    let profile = format!(
        r#"
id: test.governed
title: Governed by a specification
protocol: aep/1
workflow: test/linear
completion:
  - ess_conformance.passed
  - ess_conformance.spec_digest == "{digest}"
"#
    );
    let mut registry = Registry::new();
    registry
        .insert_protocol(aep_schema::parse::protocol(GOVERNED_PROTOCOL, None).expect("protocol"))
        .expect("unique");
    registry
        .insert_workflow(aep_schema::parse::workflow(GOVERNED_WORKFLOW, None).expect("workflow"))
        .expect("unique");
    registry
        .insert_profile(aep_schema::parse::profile(&profile, None).expect("profile"))
        .expect("unique");
    Engine::with_clock(registry, FixedClock::new(1_700_000_000_000))
}

#[test]
fn conformance_evidence_for_one_specification_does_not_satisfy_a_requirement_about_another() {
    // The engine half of `conformance_evidence_for_one_specification_does_not_attest_another`.
    // A profile pins the specification its work is governed by; a conformance run against a
    // different resolution of `billing/v3` is a true report about a different model, and it must
    // leave the requirement owed rather than close it. Same shape as the revision-bound approval:
    // an approval of version three does not cover version seven.
    let engine = engine_pinned_to(BILLING_DIGEST);
    let mut execution = engine
        .initialize(aep_schema::parse::task(GOVERNED_TASK, None).expect("task"))
        .expect("initialises");

    let run = |digest: &str| {
        by_verifier(
            Evidence::EssConformance(aep_domain::evidence::EssConformanceResult {
                specification: "billing/v3".to_owned(),
                spec_digest: SpecDigest::new(digest).expect("a digest"),
                implementation: "invoice-service".to_owned(),
                status: VerificationStatus::Passed,
                scenarios_total: 24,
                scenarios_failed: 0,
                suite_version: Some("1".to_owned()),
                compiler_version: Some("0.3.0".to_owned()),
                generator_version: Some("0.3.0".to_owned()),
                failed_scenarios: Vec::new(),
            }),
            Verifier::ConformanceRunner,
        )
    };

    let outstanding = |execution: &aep_engine::Execution| {
        engine
            .explain_completion(execution)
            .outstanding()
            .map(|item| item.requirement.clone())
            .collect::<Vec<_>>()
    };

    engine
        .submit_evidence(&mut execution, run("0badc0ffee123456"))
        .expect("recorded");
    let owed = outstanding(&execution);
    assert!(
        owed.iter().any(|item| item.contains("spec_digest")),
        "a green run against another specification must leave this one owed: {owed:?}"
    );
    assert!(
        !owed.iter().any(|item| item.contains("passed")),
        "and it must be the specification that is owed, not the passing: {owed:?}"
    );

    engine
        .submit_evidence(&mut execution, run(BILLING_DIGEST))
        .expect("recorded");
    assert!(
        outstanding(&execution).is_empty(),
        "the run against the specification in front of us closes it:\n{}",
        engine.explain_completion(&execution)
    );
}

#[test]
fn a_task_without_a_specification_owes_no_conformance() {
    let engine = engine();
    let execution = engine
        .initialize_with_artifacts(task(Some("development.critical")), artifacts())
        .expect("initialises");

    let outstanding: Vec<String> = engine
        .explain_completion(&execution)
        .outstanding()
        .map(|item| item.requirement.clone())
        .collect();
    assert!(
        !outstanding
            .iter()
            .any(|item| item.contains("ess_conformance")),
        "a rule that does not apply must not be owed: {outstanding:?}"
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
                    seed: Some(Seed::new("17650292319862362387").expect("a seed")),
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

// ---- the closed loop -------------------------------------------------------------------------
//
// Design §33 and §49 step 10, at the level the engine sees it. The tests above build conformance
// records in Rust, which is enough to check the engine's rules and not enough to check the claim
// the project actually makes — that a *real run* of a specification's own suite is what decides a
// task. These two replay `examples/billing-conformance/`, whose two conformance records are
// produced by `protocol ess conform evidence` and drift-checked against the runner in
// `crates/protocol-cli/tests/cli.rs`. The engine only ever reads them, which is invariant 7: the
// conversion happened in the crate that ran the suite, and nothing here could have written it.

/// The worked example that requires conformance.
fn conformance_example(file: &str) -> PathBuf {
    root().join("examples/billing-conformance").join(file)
}

/// The task and artifact graph of that example.
fn conformance_task() -> (Task, ArtifactGraph) {
    let origin = "examples/billing-conformance/task.yaml";
    let text = fs::read_to_string(conformance_example("task.yaml")).expect("the task is readable");
    let task = aep_schema::parse::task(&text, Some(origin)).expect("the task is valid");

    let origin = "examples/billing-conformance/artifacts.yaml";
    let text = fs::read_to_string(conformance_example("artifacts.yaml"))
        .expect("the manifest is readable");
    let graph =
        aep_schema::parse::artifact_manifest(&text, Some(origin)).expect("the manifest is valid");
    (task, graph)
}

/// Submits one evidence file of the example, exactly as the CLI would.
fn submit_file(engine: &Engine<FixedClock>, execution: &mut aep_engine::Execution, file: &str) {
    let path = conformance_example(&format!("evidence/{file}"));
    let text = fs::read_to_string(&path).expect("the evidence file is readable");
    let inputs = aep_schema::parse::evidence_list(&text, Some(&path.display().to_string()))
        .unwrap_or_else(|error| panic!("{} is not valid evidence: {error}", path.display()));
    for input in inputs {
        let mut submission = EvidenceSubmission::new(input.evidence, input.producer);
        submission.subject = input.about;
        if let Some(provenance) = input.provenance {
            submission.provenance = provenance;
        }
        engine
            .submit_evidence(execution, submission)
            .unwrap_or_else(|error| panic!("{} was refused: {error}", path.display()));
    }
}

/// The example's evidence, up to but not including the conformance run.
const EVERYTHING_BUT_CONFORMANCE: [&str; 5] = [
    "01-red-test.yaml",
    "02-implementation.yaml",
    "03-verification.yaml",
    "04-review.yaml",
    "05-verifications.yaml",
];

/// Walks the example with `record` as its conformance evidence, if any.
fn walk_conformance_example(record: Option<&str>) -> (Engine<FixedClock>, aep_engine::Execution) {
    let engine = engine();
    let (task, graph) = conformance_task();
    let mut execution = engine
        .initialize_with_artifacts(task, graph)
        .expect("initialises");
    for file in EVERYTHING_BUT_CONFORMANCE {
        submit_file(&engine, &mut execution, file);
    }
    if let Some(record) = record {
        submit_file(&engine, &mut execution, record);
    }
    advance(&engine, &mut execution);
    (engine, execution)
}

#[test]
fn a_task_governed_by_a_specification_finishes_only_on_a_conformance_run_it_did_not_produce() {
    // The fixture reaches the state the rule is load-bearing in first: with everything else
    // submitted the task is still short, and the only thing it is short of is the run. Asserting
    // that before submitting it is what makes the second half attributable.
    let (engine, execution) = walk_conformance_example(None);
    let owed = engine.explain_completion(&execution);
    let outstanding: Vec<&str> = owed
        .outstanding()
        .map(|item| item.requirement.as_str())
        .collect();
    assert!(!owed.complete, "not without the run: {outstanding:#?}");
    assert!(
        outstanding.iter().all(|requirement| {
            // The profile's own completion line is the one exception, and only because
            // `evidence.missing` counts the record that is not there — checked below rather than
            // waved through, so a second unrelated gap could not hide behind it.
            requirement.contains("ess_conformance")
                || requirement.contains("conformance-runner")
                || requirement.contains("evidence.missing")
        }),
        "everything except conformance must already hold, or the next assertion proves nothing: \
         {outstanding:#?}"
    );
    assert!(
        owed.outstanding().any(|item| {
            item.requirement.contains("evidence.missing")
                && item.detail.as_deref() == Some("evidence.missing = 1")
        }),
        "and the one record missing is the conformance run: {outstanding:#?}"
    );

    let (engine, execution) = walk_conformance_example(Some("06-conformance.yaml"));
    let done = engine.explain_completion(&execution);
    assert!(
        done.complete,
        "the run is the only thing that changed: {}",
        done.outstanding()
            .map(|item| format!("{} — {:?}", item.requirement, item.detail))
            .collect::<Vec<_>>()
            .join("\n")
    );
    assert_eq!(execution.state_id().to_string(), "complete");
}

#[test]
fn a_run_that_found_the_implementation_wrong_leaves_the_task_open_and_says_which_scenario() {
    // Direction two. The record is present, independent, produced by the conformance runner and
    // about the right revision of the model — every structural check passes. What refuses it is
    // what it *says*, which is the only kind of refusal worth having.
    let (engine, execution) = walk_conformance_example(Some("06-conformance-faulty.yaml"));
    let owed = engine.explain_completion(&execution);
    assert!(
        !owed.complete,
        "a contradicted specification does not finish"
    );
    assert_ne!(
        execution.state_id().to_string(),
        "complete",
        "and the workflow does not reach the terminal state either"
    );

    let conformance: Vec<&aep_engine::explain::ExplainedItem> = owed
        .outstanding()
        .filter(|item| item.requirement.contains("ess_conformance"))
        .collect();
    assert_eq!(
        conformance.len(),
        2,
        "both predicates the shipped rule names must refuse it: {conformance:#?}"
    );
    for item in &conformance {
        assert_eq!(
            item.truth,
            aep_domain::predicate::Truth::False,
            "a failing run contradicts the requirement; it does not leave it unobserved: {item:?}"
        );
        assert!(
            item.source.contains("ess-conformance"),
            "the refusal names the rule a person can read: {item:?}"
        );
    }
    assert!(
        conformance
            .iter()
            .any(|item| item.detail.as_deref() == Some("ess_conformance.scenarios.failed = 1")),
        "and it says how many scenarios the specification lost: {conformance:#?}"
    );

    // The record itself still names the scenario, so the repair is one lookup away rather than a
    // re-run. This is §48's counterexample-as-feedback, arriving through the protocol.
    assert_eq!(
        aep_domain::requirement::RequirementContext::facts(&execution)
            .fact(&aep_domain::facts::FactPath::from_segments([
                "ess_conformance",
                "spec_digest"
            ]))
            .map(|value| value.to_string()),
        Some("e19d384dac86219a38b673f7ac5a9775eba834643b4e19ddbdc61767fb8a46f5".to_owned()),
        "a failing run still attests which revision it was run against"
    );
}
