//! D3(c) and review finding **F9**: what a headless run can reach, walked statically.
//!
//! Three claims are asserted here, and the third is the one that keeps the other two useful:
//!
//! 1. a `human: true` approval on a **transition's** `requires` is found — a transition's
//!    requirement set is first-class to the evaluator, and the scan that read only states and
//!    obligations would have missed it;
//! 2. an approval nested **two levels** inside conditionals is found — the counting function this
//!    could have been modelled on descends exactly one level by design, and a reachability scan
//!    that stops there under-reports, which means starting a headless run that will wedge;
//! 3. a conditional whose guard is **`False`** contributes nothing — otherwise the scan refuses
//!    every run, which is the failure D3 opens by naming.

use aep_domain::plan::ExecutionPlan;
use aep_driver::approval::reachable_approvals;
use aep_driver_spec::map::StepMap;
use aep_engine::registry::Registry;
use aep_engine::resolve::resolve;

const PROTOCOL: &str = r"
id: aep
version: 1
title: Test protocol
capabilities:
  - repository.read
  - repository.write
  - tests.execute
  - command.execute
  - deployment.create
evidence_kinds: [test_result, static_analysis, approval, diff, review]
verifiers: [test-runner, static-analyzer, human-approval, human-review, compiler]
artifact_kinds: [specification, design, story]
phases: [implementation, verification, completion]
observables:
  - 'task.**'
  - 'tests.**'
  - 'static_analysis.**'
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

/// `implement -> verify` carries a human approval on the **transition**, not on either state.
const WORKFLOW: &str = r"
id: test/linear
version: 1
title: Linear
initial: implement
states:
  implement:
    title: Implement
    phases: [implementation]
  verify:
    title: Verify
    phases: [verification]
    requires:
      conditional:
        # Unobserved, so `Unknown`: in force, because a check that reads *unobserved* as *does not
        # apply* is a check that starts the run it exists to stop.
        - when: tests.unit.failed == 0
          require:
            conditional:
              - when: static_analysis.errors == 0
                require:
                  approvals:
                    - approval: nested-sign-off
                      human: true
  complete:
    title: Complete
    terminal: true
    phases: [completion]
transitions:
  - from: implement
    to: verify
    when: diff.exists
    requires:
      approvals:
        - approval: leave-implement
          human: true
  - from: verify
    to: complete
    when: tests.unit.failed == 0
";

const PROFILE: &str = r"
id: test.standard
title: Test standard
protocol: aep/1
workflow: test/linear
capabilities:
  allow: [repository.read, repository.write, tests.execute]
completion:
  predicates:
    - tests.unit.failed == 0
  conditional:
    # `defined(...)` is two-valued on purpose: with no deployment fact this is `False`, the
    # conditional is skipped, and the run starts. A future author writing the bare comparison
    # instead would silently turn every headless run into a refusal.
    - when: defined(deployment.production.status)
      require:
        approvals:
          - approval: production-change
            human: true
";

/// The same profile with `command.execute` behind an approval, so a `command` step reaches one.
const GATED_PROFILE: &str = r"
id: test.gated
title: Test gated
protocol: aep/1
workflow: test/linear
capabilities:
  allow: [repository.read, repository.write, tests.execute]
  approval_required: [command.execute]
completion:
  predicates:
    - tests.unit.failed == 0
";

const MAP: &str = r"
format: aep.driver-steps/1
id: test/approvals
workflow: test/linear/1
states:
  implement:
    steps:
      - kind: llm
        prompt: write the code
      - kind: command
        run: [cargo, test]
        evidence:
          kind: test_result
          verifier: test-runner
          suite: unit
      - kind: command
        run: [scripts/publish.sh]
";

fn plan(profile: &str) -> ExecutionPlan {
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
            r"
id: T-1
kind: feature
objective: reach an approval
protocol: aep/1
profile: {}
",
            profile
                .lines()
                .find_map(|line| line.strip_prefix("id: "))
                .expect("the fixture profile declares an id")
        ),
        None,
    )
    .expect("the task parses");
    resolve(&task, &registry).expect("the fixture plan resolves")
}

fn map() -> StepMap {
    aep_schema::parse::step_map(MAP, Some("test/approvals.yaml"))
        .expect("the fixture map validates")
}

#[test]
fn a_human_approval_on_a_transition_and_one_nested_two_levels_deep_are_both_reachable() {
    let found = reachable_approvals(&plan(PROFILE), &map());

    let transition: Vec<&aep_driver::ReachableApproval> = found
        .iter()
        .filter(|entry| entry.detail.contains("leave-implement"))
        .collect();
    assert_eq!(
        transition.len(),
        1,
        "F9: a transition's `requires` is read beside the two states', so an approval on it is \
         genuinely owed; found {found:#?}"
    );
    assert_eq!(
        transition[0].source, "transition implement -> verify",
        "the source names the document that asked, so the refusal can be navigated to"
    );

    let nested: Vec<&aep_driver::ReachableApproval> = found
        .iter()
        .filter(|entry| entry.detail.contains("nested-sign-off"))
        .collect();
    assert_eq!(
        nested.len(),
        1,
        "a conditional inside a conditional is still reachable; found {found:#?}"
    );
    assert_eq!(nested[0].source, "state verify");
    assert!(
        nested[0].detail.contains("tests.unit.failed")
            && nested[0].detail.contains("static_analysis.errors"),
        "the guard chain travels with the finding, or nobody can tell which branch leads to it: \
         {}",
        nested[0].detail
    );
}

#[test]
fn a_conditional_whose_guard_is_false_contributes_nothing() {
    let found = reachable_approvals(&plan(PROFILE), &map());
    assert!(
        !found
            .iter()
            .any(|entry| entry.detail.contains("production-change")),
        "with no deployment fact `defined(deployment.production.status)` is `False`, the \
         conditional is skipped, and the run starts. This test is green for a reason outside D3, \
         and that is worth knowing: {found:#?}"
    );
    assert_eq!(
        found.len(),
        2,
        "exactly the transition's approval and the nested one, and nothing else: {found:#?}"
    );
}

#[test]
fn a_command_step_reaching_a_gated_capability_is_reported_with_the_step_that_reaches_it() {
    let found = reachable_approvals(&plan(GATED_PROFILE), &map());
    let gated: Vec<&aep_driver::ReachableApproval> = found
        .iter()
        .filter(|entry| entry.detail.contains("command.execute"))
        .collect();

    assert_eq!(
        gated.len(),
        1,
        "the step declaring `test_result` exercises `tests.execute`, which is allowed; the one \
         that declares no evidence is treated as `command.execute`, which is gated: {found:#?}"
    );
    assert_eq!(
        gated[0].source, "step map test/approvals state implement step 2",
        "the step is named, because a capability nobody can point at is one nobody can remove"
    );
    assert!(
        gated[0].detail.contains("publish.sh"),
        "the program is named too: {}",
        gated[0].detail
    );
}
