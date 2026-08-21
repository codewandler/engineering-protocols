//! D3(c): what approvals a run can **reach**, decided statically before the first step.
//!
//! The obvious first answer — *refuse a headless run whose `approval_required` set is non-empty* —
//! **refuses every run**, and it is worth naming because it is where intuition goes.
//! `principles/governance/least-privilege.yaml` has no `applies_when` and puts `production.write`,
//! `deployment.create` and `network.write` behind approval for every task under every profile. The
//! test therefore has to be about what a run can reach, not about a set being non-empty.
//!
//! # What is walked
//!
//! * `plan.completion`, every `plan.obligations[].requires`, every `workflow.states[].requires`,
//!   and — per review finding **F9** — every `workflow.transitions[].requires`. A transition's
//!   requirement set is first-class to the evaluator, which reads it beside the current and target
//!   states', so a `human: true` approval on a transition is genuinely owed. No shipped workflow
//!   uses one today, which is exactly why the omission would have been found by a user rather than
//!   by the gate.
//! * Inside each: an `approvals[]` entry with `human: true`, a `reviews[]` entry with
//!   `human: true`, or an `evidence[]` entry whose verifier satisfies `Verifier::is_human`.
//! * **Nested conditionals, at every level.** `ConditionalRequirement.require` is a boxed
//!   `RequirementSet`, so conditionals nest, and the `when == False` skip is applied at each level.
//!   The precedent this could have been borrowed from — `Execution::count_missing_evidence` —
//!   descends exactly one level *by design*, because it is **counting** rather than proving
//!   absence. A reachability scan that stops at one level under-reports, and under-reporting here
//!   means starting a headless run that will wedge (F9).
//! * Plus any capability a `command` step in the map would exercise for which the policy answers
//!   `RequiresApproval`. An `llm` step cannot reach one, by D3(a): a capability that is not
//!   `Allowed` is never a tool.
//!
//! An **unknown guard counts as in force**, never as absent: `Truth::Unknown` means nobody has
//! observed whether the conditional applies, and a pre-flight check that reads *unobserved* as
//! *does not apply* is a check that starts the run it exists to stop.
//!
//! # The policy this scan feeds is the caller's, and that split is deliberate
//!
//! `protocol drive` refuses to start headless when this returns anything, printing every entry with
//! the document that asked for it; `--pause-on-approval` converts the run to *run until the first
//! approval, then persist and exit 0*. That flag changes what a green exit **means** — without it
//! exit 0 is *finished*, with it *finished or waiting* — so a caller has to choose to be told, and
//! choosing is not this crate's business. What is this crate's business is that the scan is
//! decidable and complete.
//!
//! # Two results that surprise people, written down before they are met
//!
//! `development.standard` includes `approval-gates`, whose `before_completion` obligation carries a
//! `human: true` approval — read naively, that also refuses every standard run. It does not, but
//! **only** because the obligation is conditional on `defined(deployment.production.status)`, which
//! is `False` at pre-flight with no deployment fact. A future principle author who writes the bare
//! `deployment.production.status == succeeded` instead turns every headless development run into a
//! refusal, and nothing in the driver would explain why. Conversely `development.critical` carries
//! an **unconditional** human design review, so a headless run under it refuses to start unless the
//! review already exists — which is right for a profile chosen for work whose failure is silent.

use aep_domain::capability::{Capability, CapabilityDecision, CapabilityPolicy};
use aep_domain::evidence::EvidenceKind;
use aep_domain::facts::FactStore;
use aep_domain::plan::ExecutionPlan;
use aep_domain::predicate::Truth;
use aep_domain::requirement::RequirementSet;
use aep_driver_spec::map::{Step, StepMap};

/// One approval a run can reach, and the document that asked for it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ReachableApproval {
    /// Which document, state or transition asked — so the refusal can be navigated to.
    pub source: String,
    /// What is owed, in one line.
    pub detail: String,
}

/// Every approval the resolved plan and the step map can reach, in document order.
///
/// Empty means a headless run cannot meet a person-shaped requirement on any path the documents
/// describe. It is not a promise that the run will finish — only that it will not stop for want of
/// somebody at a keyboard.
pub fn reachable_approvals(plan: &ExecutionPlan, map: &StepMap) -> Vec<ReachableApproval> {
    let mut found = Vec::new();

    scan(&plan.completion, "completion", &[], &plan.facts, &mut found);

    for obligation in &plan.obligations {
        let source = format!(
            "principle {} obligation {}",
            obligation.principle, obligation.id
        );
        scan(&obligation.requires, &source, &[], &plan.facts, &mut found);
    }

    for (id, state) in &plan.workflow.states {
        scan(
            &state.requires,
            &format!("state {id}"),
            &[],
            &plan.facts,
            &mut found,
        );
    }

    // F9: a transition's requirement set is not the target state's, and the evaluator reads both.
    for transition in &plan.workflow.transitions {
        scan(
            &transition.requires,
            &format!("transition {} -> {}", transition.from, transition.to),
            &[],
            &plan.facts,
            &mut found,
        );
    }

    found.extend(gated_capabilities(plan, map));
    found
}

/// Walks one requirement set, recursing through conditionals that are not ruled out.
///
/// `guards` is the chain of conditions this set sits under, so a nested finding says which branch
/// leads to it. A conditional whose `when` is `False` is skipped along with everything below it —
/// that is the only pruning, and `Truth::Unknown` is deliberately not pruned.
fn scan(
    set: &RequirementSet,
    source: &str,
    guards: &[String],
    facts: &FactStore,
    found: &mut Vec<ReachableApproval>,
) {
    let under = |detail: String| ReachableApproval {
        source: source.to_owned(),
        detail: if guards.is_empty() {
            detail
        } else {
            format!("{detail} (reached under {})", guards.join(", then "))
        },
    };

    for approval in &set.approvals {
        if approval.human {
            found.push(under(format!(
                "approval `{}` must be granted by a person",
                approval.approval
            )));
        }
    }

    for review in &set.reviews {
        if review.human {
            let subject = review
                .subject
                .as_ref()
                .map(ToString::to_string)
                .or_else(|| {
                    review
                        .subject_kind
                        .as_ref()
                        .map(|kind| format!("any {}", kind.as_str()))
                });
            found.push(under(format!(
                "a person must review {} to `{}`",
                subject.unwrap_or_else(|| "the work".to_owned()),
                review.result
            )));
        }
    }

    for evidence in &set.evidence {
        let Some(verifier) = &evidence.verifier else {
            continue;
        };
        if verifier.is_human() {
            found.push(under(format!(
                "`{}` evidence must come from `{verifier}`, which is a person",
                evidence.kind.as_str()
            )));
        }
    }

    for conditional in &set.conditional {
        if conditional.when.evaluate(facts) == Truth::False {
            continue;
        }
        let mut nested = guards.to_vec();
        nested.push(format!("`if {}`", conditional.when));
        scan(&conditional.require, source, &nested, facts, found);
    }
}

/// Approval-gated capabilities the map's `command` steps would exercise.
///
/// **The narrowing is the driver's approximation and is stated as one.** A program's reach is not
/// decidable from its argv — `cargo test` could deploy, and `sh -c` certainly could — so the step's
/// declared evidence is used as the best available signal: a step declaring `test_result` is treated
/// as exercising `tests.execute`, and every other command step as exercising `command.execute`.
/// That errs towards the wider capability for anything undeclared, which is the direction a
/// pre-flight refusal should err in.
///
/// The state's own grants are folded in, because `effective_policy` does the same at run time: a
/// state that grants `deployment.create` is a state whose steps can reach the approval that gates
/// it, and a scan reading only the plan's policy would miss exactly the state the grant exists for.
fn gated_capabilities(plan: &ExecutionPlan, map: &StepMap) -> Vec<ReachableApproval> {
    let mut found = Vec::new();
    for (state_id, entry) in &map.states {
        let mut policy: CapabilityPolicy = plan.capability_policy.clone();
        if let Some(state) = plan.workflow.states.get(state_id) {
            if !state.capabilities.is_empty() {
                policy.grant(&state.capabilities);
            }
        }
        for (index, step) in entry.steps.iter().enumerate() {
            let Step::Command(command) = step else {
                continue;
            };
            let capability = if command
                .evidence
                .as_ref()
                .is_some_and(|mapping| mapping.kind == EvidenceKind::TestResult)
            {
                Capability::TestExecution
            } else {
                Capability::CommandExecution
            };
            if policy.decide(&capability) == CapabilityDecision::RequiresApproval {
                found.push(ReachableApproval {
                    source: format!("step map {} state {state_id} step {index}", map.id),
                    detail: format!(
                        "`{}` needs an approval for capability `{capability}`",
                        command.program()
                    ),
                });
            }
        }
    }
    found
}
