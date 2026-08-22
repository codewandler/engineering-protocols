//! **F-W4.2-4**: a step map is checked against the plan it will drive, at launch.
//!
//! Run `W4-2/1` is the reason this file exists and is what the first test reproduces. The run walked
//! six states of `adp/default` under `development.driven` with the `development/checks` map, spent
//! ten model sessions, 76 minutes and $31.46, and stopped at `adversarial_verify -> review` with
//! `evidence.missing = 2` — because the plan wanted a `specification` record and an independent
//! `verification` record and no step of that map declares either kind. Both documents were on disk
//! before the run started.
//!
//! The tests split into two halves on purpose:
//!
//! * against the **real document tree**, because the finding is about two real documents and a
//!   fixture reproducing it would only prove the fixture was written to;
//! * against **synthetic documents**, for the three narrowings that keep the check from refusing
//!   runs it should not — an unreachable state, a person-shaped demand, and a conditional the
//!   task's facts rule out — none of which the shipped documents happen to exercise.

use std::path::{Path, PathBuf};

use aep_domain::evidence::EvidenceKind;
use aep_domain::plan::ExecutionPlan;
use aep_driver::coverage::{evidence_coverage, CoverageReport};
use aep_driver_spec::map::StepMap;
use aep_engine::registry::Registry;
use aep_engine::{load_tree_report, resolve};

/// The repository root, from this crate's manifest directory.
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the workspace root exists")
}

/// A `development.driven` feature against the real document tree, with these constraint facts.
fn driven_plan(facts: &str) -> ExecutionPlan {
    let registry = load_tree_report(&root())
        .into_result()
        .expect("the document tree is valid");
    let document = format!(
        "id: T-1\nkind: feature\nobjective: exercise map coverage\nprotocol: adp/1\n\
         profile: development.driven\n{facts}"
    );
    let task = aep_schema::parse::task(&document, None).expect("the fixture task parses");
    resolve(&task, &registry).expect("development.driven resolves")
}

/// One of the two shipped step maps, parsed from the tree.
fn shipped(name: &str) -> StepMap {
    let path = root().join("drivers/development").join(name);
    let text = std::fs::read_to_string(&path).expect("the shipped map is readable");
    aep_schema::parse::step_map(&text, Some(name)).expect("the shipped map validates")
}

/// The kinds a report says nothing can produce, as written in documents.
fn missing_kinds(report: &CoverageReport) -> Vec<&'static str> {
    report
        .missing
        .iter()
        .map(|entry| entry.kind.as_str())
        .collect()
}

/// The declared fact `W4-2/1`'s task carries after the applicability fix landed.
const NO_CODE_CHANGE: &str = "constraints:\n  facts:\n    change.code: false\n";

#[test]
fn the_gap_that_cost_thirty_one_dollars_is_named_before_a_single_step_runs() {
    let report = evidence_coverage(&driven_plan(NO_CODE_CHANGE), &shipped("checks.yaml"));

    assert!(
        !report.is_covered(),
        "`development/checks` cannot mint a `specification` or a `verification` record, which is \
         what `W4-2/1` found at a guard six states in: {report:#?}"
    );
    assert_eq!(
        missing_kinds(&report),
        ["verification", "specification"],
        "exactly the two kinds the run measured as `evidence.missing = 2` after the applicability \
         fix, and no others: {report:#?}"
    );

    let specification = report
        .missing
        .iter()
        .find(|entry| entry.kind == EvidenceKind::Specification)
        .expect("the `specification` gap is reported");
    assert_eq!(
        specification.demanded_by,
        ["principle spec-driven"],
        "the refusal names the document that asked, or nobody can act on it"
    );
    assert!(
        specification
            .blocks
            .contains(&"adversarial_verify -> review".to_owned()),
        "the guard `evidence.missing == 0` sits on that move, and naming it is what turns the \
         refusal into an instruction: {:?}",
        specification.blocks
    );

    let verification = report
        .missing
        .iter()
        .find(|entry| entry.kind == EvidenceKind::Verification)
        .expect("the `verification` gap is reported");
    assert!(
        verification.demanded_by[0].contains("provenance-tracking"),
        "{:?}",
        verification.demanded_by
    );
    assert!(
        verification.blocks.contains(&"completion".to_owned()),
        "`provenance-tracking` owes it before completion, so completion is shut too: {:?}",
        verification.blocks
    );
}

#[test]
fn a_task_that_declares_no_code_change_is_not_refused_for_contract_or_property_evidence() {
    // W4.2's applicability fix, seen from the launch check. Neither map declares a
    // `contract_result` or a `property_test_result` and neither ever will — no contract runner and
    // no property tester can observe a document — so if applicability were ignored here, every
    // documentation task in this repository would be refused for two rules that do not apply to it.
    for name in ["default.yaml", "checks.yaml"] {
        let scoped = evidence_coverage(&driven_plan(NO_CODE_CHANGE), &shipped(name));
        for absent in ["contract_result", "property_test_result"] {
            assert!(
                !missing_kinds(&scoped).contains(&absent),
                "`change.code: false` removes the principle that wants `{absent}`, so {name} must \
                 not be refused for it: {scoped:#?}"
            );
        }

        // The other half, and the reason the assertion above is a test of the rule rather than of
        // the map: with the fact *undeclared* the principles stay in force — Unknown is not False —
        // and the same map is refused for exactly those two kinds. Without this, commenting the
        // applicability walk out entirely would leave the test above green.
        let silent = evidence_coverage(&driven_plan(""), &shipped(name));
        for owed in ["contract_result", "property_test_result"] {
            assert!(
                missing_kinds(&silent).contains(&owed),
                "a task declaring nothing still owes `{owed}`, and {name} cannot produce it: \
                 {silent:#?}"
            );
        }
    }
}

#[test]
fn both_shipped_maps_report_the_gap_the_governed_run_measured_and_nothing_more() {
    // Recorded as a fact about the tree at this commit, not as an aspiration. Neither shipped map
    // can finish a run under `adp/default`, and the two kinds are the same for both: `W4-2/1`'s
    // `evidence.missing = 2` with the fact declared, and `= 4` without it. Closing the gap means
    // adding steps to the maps, which is a documents change and a decision, not this check's job.
    for name in ["default.yaml", "checks.yaml"] {
        assert_eq!(
            missing_kinds(&evidence_coverage(
                &driven_plan(NO_CODE_CHANGE),
                &shipped(name)
            )),
            ["verification", "specification"],
            "{name}, with `change.code: false`"
        );
        assert_eq!(
            missing_kinds(&evidence_coverage(&driven_plan(""), &shipped(name))),
            [
                "contract_result",
                "property_test_result",
                "verification",
                "specification"
            ],
            "{name}, with nothing declared"
        );
    }
}

/// A map for `adp/default` that closes the gap the shipped ones leave.
///
/// `record:` is the route: `specification` and `verification` are outside
/// `EvidenceMapping::MINTABLE`, because neither can be built from an exit status, so a step that
/// declares one has to name the document its verifier wrote. Both verifiers here are ones
/// `default_verifiers` accepts for the kind, so the map also passes `cross_validate`.
const COMPLETE_MAP: &str = r"
format: aep.driver-steps/1
id: fixture/complete
workflow: adp/default/1
states:
  establish_verifiers:
    steps:
      - kind: command
        run: [sh, -c, 'exit 1']
        evidence:
          kind: test_result
          suite: unit
          verifier: test-runner
  implement:
    steps:
      - kind: command
        run: [git, diff]
        evidence:
          kind: diff
          verifier: compiler
  verify:
    steps:
      - kind: command
        run: [sh, -c, 'exit 0']
        evidence:
          kind: static_analysis
          verifier: static-analyzer
      - kind: command
        run: [sh, -c, 'exit 0']
        evidence:
          kind: specification
          verifier: test-runner
          record: '{run_directory}/specification.yaml'
      - kind: command
        run: [sh, -c, 'exit 0']
        evidence:
          kind: verification
          verifier: policy-engine
          record: '{run_directory}/verification.yaml'
  review:
    steps:
      - kind: operator
        prompt: judge the change as a whole
";

#[test]
fn a_map_that_declares_every_demanded_kind_launches() {
    // The check has to be passable, or it is a check nobody can act on. This is the same real plan
    // the two tests above refuse, against a map with the two missing steps added.
    let map = aep_schema::parse::step_map(COMPLETE_MAP, Some("fixture/complete.yaml"))
        .expect("the fixture map validates");
    let report = evidence_coverage(&driven_plan(NO_CODE_CHANGE), &map);

    assert!(
        report.is_covered(),
        "with a producer for every demanded kind there is nothing to refuse: {report:#?}"
    );
    assert!(
        report.mintable.contains(&EvidenceKind::Approval),
        "the `operator` step is what makes a person's record reachable at all: {:?}",
        report.mintable
    );
}

// ---------------------------------------------------------------------------------------------
// The three narrowings, on documents written to reach them. None of the shipped documents does.
// ---------------------------------------------------------------------------------------------

const PROTOCOL: &str = r"
id: aep
version: 1
title: Test protocol
capabilities: [repository.read, repository.write, tests.execute]
evidence_kinds: [test_result, static_analysis, contract_result, approval, diff, review]
verifiers: [test-runner, contract-runner, static-analyzer, human-approval, human-review, compiler]
artifact_kinds: [specification]
phases: [implementation, verification, completion]
observables:
  - 'task.**'
  - 'change.**'
  - 'tests.**'
  - 'static_analysis.**'
  - 'contracts.**'
  - 'deployment.**'
  - 'approval.**'
  - 'approvals.**'
  - 'diff.**'
  - 'artifact.**'
  - 'review.**'
  - 'evidence.**'
  - 'state.**'
  - 'workflow.**'
";

/// `orphan` is declared, demands a `contract_result`, and nothing transitions into it.
const WORKFLOW: &str = r"
id: test/linear
version: 1
title: Linear
initial: implement
allow_unreachable_states: true
states:
  implement:
    title: Implement
    phases: [implementation]
  verify:
    title: Verify
    phases: [verification]
  orphan:
    title: Orphan
    phases: [verification]
    requires:
      evidence:
        - kind: diff
          verifier: compiler
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
    when: evidence.missing == 0
  # `orphan` needs a way out or the workflow is refused as a dead end. It still has no way *in*,
  # which is the property the test is about.
  - from: orphan
    to: complete
";

/// Two conditionals over completion: one the task's facts rule out, one nobody has observed.
const PROFILE: &str = r"
id: test.standard
title: Test standard
protocol: aep/1
workflow: test/linear
capabilities:
  allow: [repository.read, repository.write, tests.execute]
completion:
  conditional:
    - when: change.code
      require:
        evidence:
          - kind: static_analysis
            verifier: static-analyzer
    - when: tests.unit.failed == 0
      require:
        evidence:
          - kind: contract_result
            verifier: contract-runner
";

/// A profile whose completion wants a record only a person can produce.
const HUMAN_PROFILE: &str = r"
id: test.human
title: Test human
protocol: aep/1
workflow: test/linear
capabilities:
  allow: [repository.read, repository.write, tests.execute]
completion:
  evidence:
    - kind: test_result
      verifier: human-approval
";

/// A profile whose completion wants a kind the map declares, from a verifier it does not name.
const MISMATCH_PROFILE: &str = r"
id: test.mismatch
title: Test mismatch
protocol: aep/1
workflow: test/linear
capabilities:
  allow: [repository.read, repository.write, tests.execute]
completion:
  evidence:
    - kind: test_result
      verifier: compiler
";

/// Declares `test_result` from a `test-runner`, and nothing else.
const MAP: &str = r"
format: aep.driver-steps/1
id: test/coverage
workflow: test/linear/1
states:
  implement:
    steps:
      - kind: command
        run: [cargo, test]
        evidence:
          kind: test_result
          suite: unit
          verifier: test-runner
";

/// The same map with a person to ask.
const MAP_WITH_OPERATOR: &str = r"
format: aep.driver-steps/1
id: test/coverage-operator
workflow: test/linear/1
states:
  implement:
    steps:
      - kind: command
        run: [cargo, test]
        evidence:
          kind: test_result
          suite: unit
          verifier: test-runner
  verify:
    steps:
      - kind: operator
        prompt: hand over the record this plan needs
";

/// Resolves the synthetic fixture with `profile` and the task's declared `facts`.
fn fixture_plan(profile: &str, profile_id: &str, facts: &str) -> ExecutionPlan {
    let mut registry = Registry::new();
    registry
        .insert_protocol(aep_schema::parse::protocol(PROTOCOL, None).expect("the protocol parses"))
        .expect("the protocol is unique");
    registry
        .insert_workflow(aep_schema::parse::workflow(WORKFLOW, None).expect("the workflow parses"))
        .expect("the workflow is unique");
    registry
        .insert_profile(aep_schema::parse::profile(profile, None).expect("the profile parses"))
        .expect("the profile is unique");
    let task = aep_schema::parse::task(
        &format!(
            "id: T-1\nkind: feature\nobjective: reach a demand\nprotocol: aep/1\n\
             profile: {profile_id}\n{facts}"
        ),
        None,
    )
    .expect("the fixture task parses");
    resolve(&task, &registry).expect("the fixture plan resolves")
}

/// Parses one of the synthetic maps.
fn fixture_map(text: &str) -> StepMap {
    aep_schema::parse::step_map(text, Some("fixture.yaml")).expect("the fixture map validates")
}

#[test]
fn a_requirement_on_a_state_nothing_can_walk_into_refuses_nothing() {
    // `orphan` wants a `contract_result` and the map declares none. It is also unreachable, so no
    // run can ever be asked for it, and refusing would be refusing for a rule that never fires.
    // The fixture has to reach the state where this is load-bearing, so first: the state exists and
    // really does demand the kind.
    let plan = fixture_plan(PROFILE, "test.standard", NO_CODE_CHANGE);
    assert!(
        plan.workflow
            .states
            .get(&"orphan".parse().expect("a state id"))
            .is_some_and(|state| state
                .requires
                .evidence
                .iter()
                .any(|requirement| requirement.kind == EvidenceKind::Diff)),
        "the fixture's unreachable state must actually demand the kind, or this proves nothing"
    );
    assert!(
        !fixture_map(MAP)
            .declared_evidence_kinds()
            .contains(&EvidenceKind::Diff),
        "and the map must not declare it, or the absence below would be for the wrong reason"
    );

    let report = evidence_coverage(&plan, &fixture_map(MAP));
    assert!(
        !missing_kinds(&report).contains(&"diff"),
        "a demand written on an unreachable state blocks nothing: {report:#?}"
    );
}

#[test]
fn a_conditional_the_task_ruled_out_demands_nothing_and_an_unobserved_one_still_does() {
    // Both conditionals sit in the same completion block, so the two answers differ only in what
    // the task said. `change.code: false` is written down, so its branch is `False` and pruned.
    // `tests.unit.failed == 0` has never been observed, so it is `Unknown` — in force, invariant 5
    // — and the `contract_result` under it is demanded and refused.
    let report = evidence_coverage(
        &fixture_plan(PROFILE, "test.standard", NO_CODE_CHANGE),
        &fixture_map(MAP),
    );

    assert!(
        !missing_kinds(&report).contains(&"static_analysis"),
        "the branch under `change.code: false` is ruled out by a fact the task wrote: {report:#?}"
    );
    assert!(
        missing_kinds(&report).contains(&"contract_result"),
        "nobody has observed `tests.unit.failed`, and a check that read unobserved as `does not \
         apply` would start the run it exists to stop: {report:#?}"
    );

    let entry = report
        .missing
        .iter()
        .find(|entry| entry.kind == EvidenceKind::ContractResult)
        .expect("the conditional demand is reported");
    assert!(
        entry.demanded_by[0].contains("under if tests.unit.failed == 0"),
        "the branch that reaches a demand travels with it, or nobody can tell which one to look \
         at: {:?}",
        entry.demanded_by
    );
    assert!(
        entry.blocks.contains(&"verify -> complete".to_owned()),
        "`evidence.missing == 0` guards that move, and a completion demand raises that count: {:?}",
        entry.blocks
    );
}

#[test]
fn a_record_only_a_person_can_produce_is_a_warning_and_never_a_refusal() {
    // The driver mints no approval and signs as no person (invariant 7, `tests/evidence_scan.rs`),
    // so `the map cannot produce this` is true of every map ever written and says nothing about
    // this one. With no `operator` step it is worth saying; with one it is not even that.
    let plan = fixture_plan(HUMAN_PROFILE, "test.human", "");

    let unasked = evidence_coverage(&plan, &fixture_map(MAP));
    assert!(
        unasked.is_covered(),
        "a person's record is never grounds for refusing a map: {unasked:#?}"
    );
    assert!(
        unasked
            .warnings
            .iter()
            .any(|warning| warning.contains("human-approval") && warning.contains("operator")),
        "with nobody to ask, the run is told: {:?}",
        unasked.warnings
    );

    let asked = evidence_coverage(&plan, &fixture_map(MAP_WITH_OPERATOR));
    assert!(
        asked.warnings.is_empty(),
        "an `operator` step is where the person is asked, so there is nothing left to warn about: \
         {:?}",
        asked.warnings
    );
}

#[test]
fn a_demand_pinning_a_verifier_no_step_names_warns_rather_than_refusing() {
    // The kind is produced; whether the *producer* will match is fixed when the step runs, not when
    // the map is written. Undecidable at launch, so it prints and does not block.
    let plan = fixture_plan(MISMATCH_PROFILE, "test.mismatch", "");
    let report = evidence_coverage(&plan, &fixture_map(MAP));

    assert!(
        report.is_covered(),
        "the kind is declared by a step, so it is not a gap: {report:#?}"
    );
    assert_eq!(
        report.warnings.len(),
        1,
        "one line, about the one demand whose verifier no step names: {:?}",
        report.warnings
    );
    assert!(
        report.warnings[0].contains("`compiler`") && report.warnings[0].contains("test_result"),
        "the warning says which verifier and which kind: {:?}",
        report.warnings
    );
}
