//! A green result from three weeks ago is not a fact.
//!
//! The story is `story:evidence-horizons` and the design is
//! `docs/design/evidence-horizons-design-v0.1.md`. What is asserted here is the whole of it as the
//! engine sees it:
//!
//! | § | property |
//! |---|---|
//! | 1 | past its horizon, the requirement reads `Unknown` — never `False` — and says which horizon and which date |
//! | 2 | the transition it used to permit is refused, **including** when the guard reads a fact rather than the requirement |
//! | 3 | re-submitting the identical record restores nothing; only a new observation time does |
//! | 4 | a snapshot re-decays against the clock it is restored under |
//! | 5 | an observation in the future is refused outright |
//! | 6 | `evidence.missing` and the requirement outcome agree about a lapsed record |
//!
//! Every test drives the engine on a [`FixedClock`], so *"three weeks later"* is a second engine
//! rather than a sleep, and the whole file is deterministic.

use aep_domain::evidence::{Evidence, Producer, TestResult, TestSuite};
use aep_domain::facts::{FactPath, FactSource};
use aep_domain::predicate::Truth;
use aep_domain::requirement::RequirementFlavour;
use aep_domain::time::{CivilDate, ObservedAt, Timestamp};
use aep_domain::verification::Verifier;
use aep_engine::clock::FixedClock;
use aep_engine::engine::{Engine, EvidenceSubmission, ProtocolEngine};
use aep_engine::execution::Execution;
use aep_engine::registry::Registry;

/// A profile whose completion needs a test run observed within three days.
///
/// The horizon is on the **requirement**, which is the whole of decision D4: there is no field on a
/// record for one, so no record can extend its own life.
const DECAYING_PRINCIPLE: &str = r"
id: test-driven
version: 1
title: Test-driven development
applies_when:
  task.kind: {any_of: [feature, bugfix]}
evidence:
  - kind: test_result
    horizon: 3d
";

/// The same workflow the other fixtures use, with a guard that reads a **fact** a test run feeds.
///
/// `verify -> complete` is guarded on `tests.unit.failed == 0`, which is a predicate over the fact
/// store and not a requirement — so it is the case that decides whether a lapse actually refuses a
/// transition or merely annotates one.
const WORKFLOW: &str = r"
id: test/linear
title: Linear
initial: verify
states:
  verify:
    title: Verify
    phases: [verification]
  complete:
    title: Complete
    terminal: true
    phases: [completion]
transitions:
  - from: verify
    to: complete
    when: tests.unit.failed == 0
    requires:
      evidence:
        - kind: test_result
          horizon: 3d
";

const PROFILE: &str = r"
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

/// The smallest protocol that admits a test run and the facts one projects.
const PROTOCOL: &str = r"
id: aep
version: 1
title: Horizon test protocol
capabilities: [repository.read, repository.write, tests.execute]
evidence_kinds: [test_result]
verifiers: [test-runner]
phases: [verification, completion]
observables:
  - 'task.**'
  - 'tests.**'
  - 'test.**'
  - 'evidence.**'
  - 'state.**'
  - 'workflow.**'
";

const TASK: &str = r"
id: T-1
kind: feature
objective: keep a fact from outliving its observation
protocol: aep/1
profile: test.standard
";

fn registry() -> Registry {
    let mut registry = Registry::new();
    registry
        .insert_protocol(aep_schema::parse::protocol(PROTOCOL, None).expect("the protocol parses"))
        .expect("the protocol is unique");
    registry
        .insert_principle(
            aep_schema::parse::principle(DECAYING_PRINCIPLE, None).expect("the principle parses"),
        )
        .expect("the principle is unique");
    registry
        .insert_workflow(aep_schema::parse::workflow(WORKFLOW, None).expect("the workflow parses"))
        .expect("the workflow is unique");
    registry
        .insert_profile(aep_schema::parse::profile(PROFILE, None).expect("the profile parses"))
        .expect("the profile is unique");
    registry
}

fn task() -> aep_domain::task::Task {
    aep_schema::parse::task(TASK, None).expect("the task parses")
}

/// An engine whose clock is stopped on `day`.
fn engine_on(day: &str) -> Engine<FixedClock> {
    Engine::with_clock(registry(), FixedClock::new(millis(day)))
}

/// Midnight UTC on a day, in epoch milliseconds.
fn millis(day: &str) -> u64 {
    CivilDate::parse(day)
        .expect("a valid date")
        .to_timestamp()
        .epoch_millis()
}

/// A green unit suite, observed on `day`.
fn green_on(day: &str) -> EvidenceSubmission {
    EvidenceSubmission::new(
        Evidence::TestResult(TestResult::passing(TestSuite::Unit, 12)),
        Producer::Verifier {
            verifier: Verifier::TestRunner,
        },
        ObservedAt::new(Timestamp::from_epoch_millis(millis(day))),
    )
}

/// The completion requirement's outcome, which is the one the horizon governs.
fn test_requirement(engine: &Engine<FixedClock>, execution: &Execution) -> (Truth, String) {
    let evaluation = engine.evaluate(execution);
    let requirement = evaluation
        .completion
        .iter()
        .find(|requirement| {
            requirement.outcome.flavour == RequirementFlavour::Evidence
                && requirement.outcome.requirement.contains("test_result")
        })
        .expect("the profile's principle asks for a test run");
    (
        requirement.outcome.truth,
        requirement.outcome.detail.clone().unwrap_or_default(),
    )
}

fn fact(execution: &Execution, path: &str) -> Option<String> {
    execution
        .fact_store()
        .fact(&FactPath::new(path).expect("a well-formed fact path"))
        .map(|value| value.to_string())
}

#[test]
fn inside_its_horizon_a_test_run_satisfies_the_requirement_and_permits_the_transition() {
    // The premise, asserted before the rule it is the premise of: without this, every assertion
    // below would pass on a fixture that never worked.
    let engine = engine_on("2026-09-01");
    let mut execution = engine.initialize(task()).expect("the task resolves");
    engine
        .submit_evidence(&mut execution, green_on("2026-08-30"))
        .expect("a two-day-old observation is accepted");

    let (truth, _) = test_requirement(&engine, &execution);
    assert_eq!(truth, Truth::True, "two days is inside a three-day horizon");
    assert_eq!(
        fact(&execution, "tests.unit.failed").as_deref(),
        Some("0"),
        "a live record's facts are in the store"
    );
    assert_eq!(fact(&execution, "evidence.lapsed").as_deref(), Some("0"));
    assert!(
        engine.evaluate(&execution).transitions[0].permitted,
        "the guard reads the fact and the transition is open"
    );
}

#[test]
fn past_its_horizon_the_requirement_reads_unknown_and_names_the_horizon_and_the_observation() {
    // Same evidence, same documents, a clock four days further on. Nothing else differs, which is
    // what makes the horizon the cause.
    let engine = engine_on("2026-09-05");
    let mut execution = engine.initialize(task()).expect("the task resolves");
    engine
        .submit_evidence(&mut execution, green_on("2026-08-30"))
        .expect("an old observation is still admissible; it is simply old");

    let (truth, detail) = test_requirement(&engine, &execution);
    assert_eq!(
        truth,
        Truth::Unknown,
        "`Unknown`, never `False`: a lapsed suite has not failed, nobody has run it"
    );
    assert!(
        detail.contains("2026-08-30"),
        "the observation date: {detail}"
    );
    assert!(detail.contains("3d"), "the horizon: {detail}");
    assert!(detail.contains("2026-09-02"), "when it lapsed: {detail}");
    assert_ne!(truth, Truth::False, "invariant 5, spelled out");
}

#[test]
fn the_transition_a_lapsed_record_used_to_permit_is_refused_rather_than_taken() {
    // The acceptance bullet that a requirement-only implementation quietly fails. This workflow's
    // guard is `tests.unit.failed == 0` — a predicate over the fact store, which the requirement
    // outcome never touches. If a lapsed record's facts stayed in the store, the requirement would
    // read `?` and the guard would wave the transition through anyway.
    let engine = engine_on("2026-09-05");
    let mut execution = engine.initialize(task()).expect("the task resolves");
    engine
        .submit_evidence(&mut execution, green_on("2026-08-30"))
        .expect("submitted");

    assert_eq!(
        fact(&execution, "tests.unit.failed"),
        None,
        "a lapsed record's facts are withheld, so the fact is absent rather than wrong"
    );
    assert_eq!(fact(&execution, "evidence.lapsed").as_deref(), Some("1"));

    let evaluation = engine.evaluate(&execution);
    let transition = &evaluation.transitions[0];
    assert!(!transition.permitted, "the transition is refused");
    assert_eq!(
        transition.guard.truth,
        Truth::Unknown,
        "an absent fact is unknown, not false — the guard does not claim the tests failed"
    );

    let result = engine
        .transition(&mut execution)
        .expect("a blocked execution is reported, not an error");
    let aep_engine::TransitionResult::Blocked { reasons, .. } = &result else {
        panic!("expected a blocked execution, got {result:?}");
    };
    assert!(
        reasons
            .iter()
            .any(|reason| reason.contains("2026-08-30") && reason.contains("3d")),
        "the refusal names the horizon and the observation nobody has repeated: {reasons:?}"
    );
    assert!(
        !reasons
            .iter()
            .any(|reason| reason.contains("failed") && reason.contains("false")),
        "and it never claims the tests failed: {reasons:?}"
    );
}

#[test]
fn re_submitting_the_identical_record_restores_nothing_and_a_new_observation_does() {
    // The trap the corpus calls "re-check versus extend", asserted rather than documented. The
    // engine stamps a fresh id and a fresh `produced_at` on the second submission, and neither is
    // read by the horizon: the observation time is the identity of the fact.
    let engine = engine_on("2026-09-05");
    let mut execution = engine.initialize(task()).expect("the task resolves");
    engine
        .submit_evidence(&mut execution, green_on("2026-08-30"))
        .expect("submitted");
    assert_eq!(test_requirement(&engine, &execution).0, Truth::Unknown);

    engine
        .submit_evidence(&mut execution, green_on("2026-08-30"))
        .expect("the identical record is accepted");
    assert_eq!(
        execution.recorded_evidence().len(),
        2,
        "the fixture reaches the state the rule is about: two records, one observation time"
    );
    assert_ne!(
        execution.recorded_evidence()[0].record.id,
        execution.recorded_evidence()[1].record.id,
        "the engine did stamp a new record, so this is not a de-duplication test"
    );
    assert_eq!(
        test_requirement(&engine, &execution).0,
        Truth::Unknown,
        "submitting it again is not looking again"
    );

    engine
        .submit_evidence(&mut execution, green_on("2026-09-04"))
        .expect("a fresh observation is accepted");
    assert_eq!(
        test_requirement(&engine, &execution).0,
        Truth::True,
        "only a new observation time restores it"
    );
}

#[test]
fn a_snapshot_restored_after_its_horizon_re_decays_from_the_same_bytes() {
    // A snapshot carries the observation time and no verdict, so restoring is a re-decision rather
    // than a replay of one. The same bytes, two clocks, two answers — and the later answer is `?`.
    let taken = engine_on("2026-09-01");
    let mut execution = taken.initialize(task()).expect("the task resolves");
    taken
        .submit_evidence(&mut execution, green_on("2026-08-30"))
        .expect("submitted");
    assert_eq!(test_requirement(&taken, &execution).0, Truth::True);

    let snapshot = execution.snapshot();
    let serialised = serde_yaml::to_string(&snapshot).expect("a snapshot serialises");
    assert!(
        serialised.contains("observed_at"),
        "the observation time is what survives; the verdict is not: {serialised}"
    );
    assert!(
        !serialised.contains("evaluated_at"),
        "a snapshot that carried the instant it was taken at would carry its verdict too"
    );

    let restored: aep_engine::execution::Snapshot =
        serde_yaml::from_str(&serialised).expect("a snapshot round-trips");
    let later = engine_on("2026-09-06");
    let execution = later
        .restore(task(), aep_domain::artifact::ArtifactGraph::new(), restored)
        .expect("the snapshot restores");

    assert_eq!(
        test_requirement(&later, &execution).0,
        Truth::Unknown,
        "six days later the same bytes say nobody knows"
    );
}

#[test]
fn an_observation_in_the_future_is_refused_and_never_stored() {
    // One comparison, and it is the cheapest guard in the engine. A scheduled check stored as a
    // performed one is the *freshest* record in the log, which is how a corpus comes to report
    // that everything has been checked recently.
    let engine = engine_on("2026-09-01");
    let mut execution = engine.initialize(task()).expect("the task resolves");

    let refusal = engine
        .submit_evidence(&mut execution, green_on("2026-09-10"))
        .expect_err("a check that has not happened yet is not evidence");
    assert_eq!(refusal.code(), "observation_in_future", "{refusal}");
    assert!(
        execution.recorded_evidence().is_empty(),
        "a refused submission leaves nothing behind"
    );

    // The boundary: an observation stamped at the current instant is not in the future.
    engine
        .submit_evidence(&mut execution, green_on("2026-09-01"))
        .expect("`now` is not the future");
    assert_eq!(execution.recorded_evidence().len(), 1);
}

#[test]
fn the_missing_count_and_the_requirement_outcome_agree_about_a_lapsed_record() {
    // Two implementations of one question — `evidence.missing` is derived by the execution, the
    // outcome by the requirement — and a document guarded on `evidence.missing == 0` must not pass
    // while the evaluation beside it reads `?`. They are consistent, not equal: one is a count and
    // the other a truth value, and `evidence.lapsed` is what tells a stale gate from an empty one.
    let engine = engine_on("2026-09-05");
    let mut execution = engine.initialize(task()).expect("the task resolves");
    engine
        .submit_evidence(&mut execution, green_on("2026-08-30"))
        .expect("submitted");

    assert_eq!(test_requirement(&engine, &execution).0, Truth::Unknown);
    assert_eq!(
        fact(&execution, "evidence.missing").as_deref(),
        Some("1"),
        "the count agrees that the requirement is not met"
    );
    assert_eq!(
        fact(&execution, "evidence.lapsed").as_deref(),
        Some("1"),
        "and says why: somebody looked, and nobody has looked since"
    );

    engine
        .submit_evidence(&mut execution, green_on("2026-09-04"))
        .expect("submitted");
    assert_eq!(test_requirement(&engine, &execution).0, Truth::True);
    assert_eq!(fact(&execution, "evidence.missing").as_deref(), Some("0"));
    assert_eq!(
        fact(&execution, "evidence.lapsed").as_deref(),
        Some("1"),
        "the old record is still lapsed; the requirement is met by the new one"
    );
}
