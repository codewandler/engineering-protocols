//! Resolution: a task plus a document set becomes an execution plan.
//!
//! This is where a task stops being a reference to documents and becomes a concrete set of
//! obligations, capabilities and completion conditions. Everything downstream reads the plan and
//! decides nothing again.
//!
//! ```text
//! Task ─┐
//!       ├─▶ protocol (extends merged)
//!       ├─▶ profile  (extends merged)
//!       ├─▶ principles: profile's, plus task additions, minus task removals
//!       ├─▶ filtered by applicability against the task's facts
//!       ├─▶ capabilities: profile grants, principles restrict, task restricts
//!       ├─▶ obligations, from every principle in force
//!       └─▶ completion condition
//! ```
//!
//! Resolution is also the last point at which a whole configuration can be checked. A principle
//! timed against a phase the workflow does not declare, or a completion condition reading a fact
//! nothing can observe, is refused here rather than becoming a task that can never finish.

use aep_domain::capability::{CapabilityDecision, CapabilityPolicy, PolicySource};
use aep_domain::error::{ValidationCode, ValidationError, ValidationErrors};
use aep_domain::facts::FactStore;
use aep_domain::ids::PrincipleId;
use aep_domain::plan::{CapabilityGrant, ExecutionPlan, ResolvedObligation};
use aep_domain::principle::Principle;
use aep_domain::task::Task;
use aep_domain::version::PrincipleRef;

use crate::registry::{check_predicate, check_requirements, Registry};

/// Resolves `task` against `registry`.
///
/// Returns every problem found rather than the first, so a misconfigured document set can be fixed
/// in one pass.
// Resolution is one pass with one accumulating error set. Splitting it would mean threading that set
// and a dozen intermediate values through private helpers, which makes the order of the steps — which
// is itself part of the semantics — harder to see, not easier.
#[allow(clippy::too_many_lines)]
pub fn resolve(task: &Task, registry: &Registry) -> Result<ExecutionPlan, ValidationErrors> {
    let mut errors = ValidationErrors::new();

    // Without a protocol and a profile there is nothing to check against, so these fail fast.
    registry.resolved_protocol(&task.protocol)?;
    let profile = registry.resolved_profile(&task.profile)?;

    if !protocol_satisfies(registry, &profile.protocol, &task.protocol) {
        errors.push(
            ValidationError::new(
                ValidationCode::VersionMismatch,
                format!("task {}.protocol", task.id),
                format!(
                    "the task targets `{}` but profile `{}` is written against `{}`, which does not \
                     extend it",
                    task.protocol, profile.id, profile.protocol
                ),
            )
            .with_hint(
                "a task may name the base protocol its profile refines — `aep/1` for a profile \
                 written against `adp/1` — but not an unrelated one",
            ),
        );
    }

    // The profile's protocol is the more specific one, so that is what the plan is checked against.
    let protocol = registry.resolved_protocol(&profile.protocol)?;

    let Some(workflow_reference) = profile.workflow.clone() else {
        errors.push(ValidationError::new(
            ValidationCode::UnknownWorkflow,
            format!("profile {}.workflow", profile.id),
            "no workflow is named, and none is inherited",
        ));
        return Err(errors);
    };
    let Some(workflow) = registry.workflow(&workflow_reference) else {
        errors.push(ValidationError::new(
            ValidationCode::UnknownWorkflow,
            format!("profile {}.workflow", profile.id),
            format!("no workflow document declares `{workflow_reference}`"),
        ));
        return Err(errors);
    };

    // Principles: the profile's, plus the task's additions, minus the task's removals.
    let mut references: Vec<PrincipleRef> = profile.principles.clone();
    for addition in &task.principle_overrides.add {
        if !references
            .iter()
            .any(|existing| existing.id() == addition.id())
        {
            references.push(addition.clone());
        }
    }

    let mut dropped: Vec<PrincipleId> = Vec::new();
    for removal in &task.principle_overrides.remove {
        let before = references.len();
        references.retain(|reference| reference.id() != removal);
        if references.len() == before {
            errors.push(
                ValidationError::new(
                    ValidationCode::UnknownPrinciple,
                    format!("task {}.principle_overrides.remove", task.id),
                    format!(
                        "`{removal}` is removed but profile `{}` does not include it",
                        profile.id
                    ),
                )
                .with_hint(
                    "a removal that does nothing usually means the principle id is misspelled, and \
                     the rule the author meant to drop is still in force",
                ),
            );
        } else {
            dropped.push(removal.clone());
        }
    }
    dropped.extend(profile.without_principles.iter().cloned());
    dropped.sort();
    dropped.dedup();

    let mut resolved: Vec<Principle> = Vec::new();
    for reference in &references {
        if let Some(principle) = registry.principle(reference) {
            resolved.push(principle.clone());
        } else {
            {
                let exists = registry
                    .principles()
                    .any(|candidate| &candidate.id == reference.id());
                let error = if exists {
                    ValidationError::new(
                        ValidationCode::VersionMismatch,
                        format!("profile {}.principles", profile.id),
                        format!("`{reference}` is pinned to a version the registry does not hold"),
                    )
                } else {
                    ValidationError::new(
                        ValidationCode::UnknownPrinciple,
                        format!("profile {}.principles", profile.id),
                        format!("no principle document declares `{}`", reference.id()),
                    )
                };
                errors.push(error);
            }
        }
    }

    // Facts known before anything is observed. Scales come from the protocol, so `risk >= medium`
    // has a defined meaning during applicability evaluation.
    let mut facts = FactStore::new();
    facts.extend(task.facts());
    for (path, value) in &profile.facts {
        facts.set_if_absent(path.clone(), value.clone());
    }
    facts.set_scales(protocol.scales.clone());

    let in_force: Vec<Principle> = resolved
        .into_iter()
        .filter(|principle| principle.applies(&facts))
        .collect();

    for principle in &in_force {
        facts.set_path(
            &format!("principle.{}.active", principle.id),
            aep_domain::FactValue::bool(true),
        );
    }

    // Capabilities: the profile grants, principles and the task may only take away.
    let mut policy = CapabilityPolicy::empty();
    let mut rationale: Vec<CapabilityGrant> = Vec::new();

    policy.grant(&profile.capabilities);
    record(
        &mut rationale,
        &profile.capabilities,
        &PolicySource::Profile {
            profile: profile.id.clone(),
        },
    );

    for principle in &in_force {
        if principle.capabilities.is_empty() {
            continue;
        }
        policy.restrict(&principle.capabilities);
        record_restrictions(
            &mut rationale,
            &principle.capabilities,
            &PolicySource::Principle {
                principle: principle.id.clone(),
            },
        );
    }

    if !task.constraints.capabilities.is_empty() {
        policy.restrict(&task.constraints.capabilities);
        record_restrictions(
            &mut rationale,
            &task.constraints.capabilities,
            &PolicySource::Task {
                task: task.id.clone(),
            },
        );
    }

    for capability in policy.mentioned() {
        if !protocol.declares_capability(capability) {
            errors.push(ValidationError::new(
                ValidationCode::UndeclaredCapability,
                format!("task {}.capabilities", task.id),
                format!(
                    "`{capability}` is not declared by protocol {}",
                    protocol.reference()
                ),
            ));
        }
    }

    // A task's `allow` list is read as "these are needed": if policy denies one, the task cannot run.
    for needed in &task.constraints.capabilities.allow {
        if policy.decide(needed) == CapabilityDecision::Denied {
            errors.push(
                ValidationError::new(
                    ValidationCode::CapabilityConflict,
                    format!("task {}.constraints.capabilities.allow", task.id),
                    format!("the task needs `{needed}`, which the resolved policy denies"),
                )
                .with_hint(
                    "a denial cannot be granted back; either the task needs a different profile, or \
                     the principle that denies it does not apply",
                ),
            );
        }
    }

    // The protocol's approval floor: some capabilities must never be granted outright.
    for capability in crate::registry::granted_outright(&policy) {
        if protocol.needs_approval_floor(capability) {
            errors.push(
                ValidationError::new(
                    ValidationCode::ProductionWriteWithoutApproval,
                    format!("profile {}.capabilities", profile.id),
                    format!(
                        "`{capability}` is granted outright, but protocol {} requires it to be \
                         behind approval or denied",
                        protocol.reference()
                    ),
                )
                .with_hint(
                    "move it to `require_approval`, or add it to `deny`; anyone acting under this \
                     profile could otherwise change production with no approval recorded",
                ),
            );
        }
    }

    let obligations: Vec<ResolvedObligation> = in_force
        .iter()
        .flat_map(|principle| {
            principle
                .obligations
                .iter()
                .map(|obligation| ResolvedObligation::new(&principle.id, obligation))
        })
        .collect();

    // Vocabulary checks for everything the plan will evaluate.
    for obligation in &obligations {
        errors.extend(check_requirements(
            &obligation.requires,
            &protocol,
            &format!("obligation {}", obligation.id),
        ));
    }
    errors.extend(check_requirements(
        &profile.completion,
        &protocol,
        &format!("profile {}.completion", profile.id),
    ));
    for principle in &in_force {
        errors.extend(check_predicate(
            &principle.applicability,
            &protocol,
            &format!("principle {}.applies_when", principle.id),
        ));
    }
    for transition in &workflow.transitions {
        errors.extend(check_predicate(
            &transition.when,
            &protocol,
            &format!("workflow {}.transitions[{transition}]", workflow.id),
        ));
        errors.extend(check_requirements(
            &transition.requires,
            &protocol,
            &format!("workflow {}.transitions[{transition}]", workflow.id),
        ));
    }
    for state in workflow.states.values() {
        errors.extend(check_requirements(
            &state.requires,
            &protocol,
            &format!("workflow {}.states.{}", workflow.id, state.id),
        ));
    }

    let plan = ExecutionPlan {
        task: task.clone(),
        protocol,
        profile,
        principles: in_force,
        workflow: workflow.clone(),
        capability_policy: policy,
        capability_rationale: rationale,
        obligations,
        completion: registry
            .resolved_profile(&task.profile)
            .map(|profile| profile.completion)
            .unwrap_or_default(),
        facts,
        dropped_principles: dropped,
    };

    // An obligation timed against something the workflow does not have can never be checked, which
    // would silently drop a rule.
    for phase in plan.unmatched_phases() {
        errors.push(
            ValidationError::new(
                ValidationCode::UnknownPhase,
                format!("workflow {}", plan.workflow.id),
                format!(
                    "an obligation is timed against phase `{phase}`, which no state declares"
                ),
            )
            .with_hint(format!(
                "declared phases: {}; tag a state with `phases: [{phase}]`, or retime the obligation",
                plan.workflow
                    .phases()
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            )),
        );
    }
    for state in plan.unmatched_states() {
        errors.push(ValidationError::new(
            ValidationCode::UnknownState,
            format!("workflow {}", plan.workflow.id),
            format!("an obligation is timed against state `{state}`, which the workflow does not declare"),
        ));
    }

    errors.into_result(plan)
}

/// `true` when `declared` is `required`, or extends it transitively.
fn protocol_satisfies(
    registry: &Registry,
    declared: &aep_domain::version::ProtocolRef,
    required: &aep_domain::version::ProtocolRef,
) -> bool {
    let mut current = Some(declared.clone());
    let mut depth = 0;
    while let Some(reference) = current {
        if &reference == required {
            return true;
        }
        depth += 1;
        if depth > 8 {
            return false;
        }
        current = registry
            .protocol(&reference)
            .and_then(|protocol| protocol.extends.clone());
    }
    false
}

/// Records every entry of a granting policy.
fn record(rationale: &mut Vec<CapabilityGrant>, policy: &CapabilityPolicy, source: &PolicySource) {
    for capability in &policy.allow {
        rationale.push(CapabilityGrant {
            capability: capability.clone(),
            effect: CapabilityDecision::Allowed,
            source: source.clone(),
        });
    }
    record_restrictions(rationale, policy, source);
}

/// Records only the restricting entries of a policy.
fn record_restrictions(
    rationale: &mut Vec<CapabilityGrant>,
    policy: &CapabilityPolicy,
    source: &PolicySource,
) {
    for capability in &policy.approval_required {
        rationale.push(CapabilityGrant {
            capability: capability.clone(),
            effect: CapabilityDecision::RequiresApproval,
            source: source.clone(),
        });
    }
    for capability in &policy.deny {
        rationale.push(CapabilityGrant {
            capability: capability.clone(),
            effect: CapabilityDecision::Denied,
            source: source.clone(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures;
    use aep_domain::capability::Capability;

    #[test]
    fn resolves_a_task_into_a_plan() {
        let registry = fixtures::standard_registry();
        let task = fixtures::standard_task();
        let plan = resolve(&task, &registry).expect("resolves");

        assert_eq!(plan.principles.len(), 1);
        assert_eq!(plan.workflow.id.as_str(), "test/linear");
        assert_eq!(plan.obligations.len(), 1);
        assert!(plan
            .capability_policy
            .decide(&Capability::TestExecution)
            .is_allowed());
        assert_eq!(
            plan.capability_policy.decide(&Capability::ProductionWrite),
            CapabilityDecision::NotGranted,
            "capabilities nobody granted are not granted"
        );
    }

    #[test]
    fn a_principle_whose_condition_excludes_the_task_is_not_in_force() {
        let registry = fixtures::standard_registry();
        let task = fixtures::task(
            r"
id: OPS-1
kind: incident
objective: restore service
protocol: aep/1
profile: test.standard
",
        );
        let plan = resolve(&task, &registry).expect("resolves");
        assert!(
            plan.principles.is_empty(),
            "test-driven applies to features and bugfixes, not incidents"
        );
        assert!(plan.obligations.is_empty());
    }

    #[test]
    fn records_every_dropped_principle() {
        let registry = fixtures::standard_registry();
        let task = fixtures::task(
            r"
id: T-2
kind: feature
objective: something
protocol: aep/1
profile: test.standard
principles:
  remove: [test-driven]
",
        );
        let plan = resolve(&task, &registry).expect("resolves");
        assert!(plan.principles.is_empty());
        assert_eq!(
            plan.dropped_principles
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>(),
            vec!["test-driven"],
            "dropping a rule must stay visible in the plan"
        );
    }

    #[test]
    fn a_removal_that_does_nothing_is_an_error() {
        let registry = fixtures::standard_registry();
        let task = fixtures::task(
            r"
id: T-3
kind: feature
objective: something
protocol: aep/1
profile: test.standard
principles:
  remove: [contract-testing]
",
        );
        let errors = resolve(&task, &registry).expect_err("removal does nothing");
        assert!(
            errors.contains(ValidationCode::UnknownPrinciple),
            "{errors}"
        );
        assert!(
            errors.to_string().contains("does not include it"),
            "{errors}"
        );
    }

    #[test]
    fn an_added_principle_joins_the_obligations() {
        let extra = r"
id: static-analysis
version: 1
title: Static analysis
requires:
  before_completion:
    - static_analysis.errors == 0
evidence: [static_analysis]
";
        let registry = fixtures::registry(
            &[fixtures::PROTOCOL],
            &[fixtures::TEST_DRIVEN, extra],
            &[fixtures::WORKFLOW],
            &[fixtures::PROFILE],
        );
        let task = fixtures::task(
            r"
id: T-4
kind: feature
objective: something
protocol: aep/1
profile: test.standard
principles:
  add: [static-analysis]
",
        );
        let plan = resolve(&task, &registry).expect("resolves");
        assert_eq!(plan.principles.len(), 2);
        assert_eq!(plan.obligations.len(), 2);
    }

    #[test]
    fn rejects_an_unknown_principle() {
        let profile = r"
id: test.standard
title: Test standard
protocol: aep/1
workflow: test/linear
principles: [test-driven, does-not-exist]
completion:
  - tests.unit.failed == 0
";
        let registry = fixtures::registry(
            &[fixtures::PROTOCOL],
            &[fixtures::TEST_DRIVEN],
            &[fixtures::WORKFLOW],
            &[profile],
        );
        let errors = resolve(&fixtures::standard_task(), &registry).expect_err("unknown principle");
        assert!(
            errors.contains(ValidationCode::UnknownPrinciple),
            "{errors}"
        );
    }

    #[test]
    fn rejects_an_obligation_timed_against_a_phase_no_state_declares() {
        let principle = r"
id: spec-driven
version: 1
title: Specification first
requires:
  before_specification:
    - specification.exists
evidence: [specification]
";
        let profile = r"
id: test.standard
title: Test standard
protocol: aep/1
workflow: test/linear
principles: [spec-driven]
completion:
  - tests.unit.failed == 0
";
        let registry = fixtures::registry(
            &[fixtures::PROTOCOL],
            &[principle],
            &[fixtures::WORKFLOW],
            &[profile],
        );
        let errors = resolve(&fixtures::standard_task(), &registry).expect_err("unknown phase");
        assert!(errors.contains(ValidationCode::UnknownPhase), "{errors}");
        assert!(
            errors.to_string().contains("no state declares"),
            "the message must say why the rule could never be checked: {errors}"
        );
    }

    #[test]
    fn rejects_a_completion_condition_reading_an_unobservable_fact() {
        let profile = r"
id: test.standard
title: Test standard
protocol: aep/1
workflow: test/linear
principles: [test-driven]
completion:
  - vibes.good == true
";
        let registry = fixtures::registry(
            &[fixtures::PROTOCOL],
            &[fixtures::TEST_DRIVEN],
            &[fixtures::WORKFLOW],
            &[profile],
        );
        let errors = resolve(&fixtures::standard_task(), &registry).expect_err("unobservable fact");
        assert!(
            errors.contains(ValidationCode::UnobservableFact),
            "{errors}"
        );
    }

    #[test]
    fn refuses_to_grant_production_write_outright() {
        let profile = r"
id: test.reckless
title: Reckless
protocol: aep/1
workflow: test/linear
principles: []
capabilities:
  allow: [repository.read, production.write]
completion:
  - tests.unit.failed == 0
";
        let registry = fixtures::registry(
            &[fixtures::PROTOCOL],
            &[fixtures::TEST_DRIVEN],
            &[fixtures::WORKFLOW],
            &[profile],
        );
        let task = fixtures::task(
            r"
id: T-5
kind: feature
objective: something
protocol: aep/1
profile: test.reckless
",
        );
        let errors = resolve(&task, &registry).expect_err("approval floor");
        assert!(
            errors.contains(ValidationCode::ProductionWriteWithoutApproval),
            "{errors}"
        );

        let guarded = profile.replace(
            "  allow: [repository.read, production.write]",
            "  allow: [repository.read]\n  require_approval: [production.write]",
        );
        let registry = fixtures::registry(
            &[fixtures::PROTOCOL],
            &[fixtures::TEST_DRIVEN],
            &[fixtures::WORKFLOW],
            &[&guarded],
        );
        let plan = resolve(&task, &registry).expect("approval satisfies the floor");
        assert_eq!(
            plan.capability_policy.decide(&Capability::ProductionWrite),
            CapabilityDecision::RequiresApproval
        );
    }

    #[test]
    fn a_principle_can_take_a_capability_away_and_the_reason_is_recorded() {
        let principle = r"
id: least-privilege
version: 1
title: Least privilege
requires:
  always:
    - task.id
capabilities:
  deny: [secret.read]
  require_approval: [repository.write]
";
        let profile = r"
id: test.standard
title: Test standard
protocol: aep/1
workflow: test/linear
principles: [least-privilege]
capabilities:
  allow: [repository.read, repository.write, secret.read, tests.execute]
completion:
  - tests.unit.failed == 0
";
        let registry = fixtures::registry(
            &[fixtures::PROTOCOL],
            &[principle],
            &[fixtures::WORKFLOW],
            &[profile],
        );
        let plan = resolve(&fixtures::standard_task(), &registry).expect("resolves");

        assert_eq!(
            plan.capability_policy.decide(&Capability::SecretRead),
            CapabilityDecision::Denied
        );
        assert_eq!(
            plan.capability_policy.decide(&Capability::RepositoryWrite),
            CapabilityDecision::RequiresApproval
        );

        let denial = plan
            .capability_rationale
            .iter()
            .find(|grant| {
                grant.capability == Capability::SecretRead
                    && grant.effect == CapabilityDecision::Denied
            })
            .expect("the denial is attributed");
        assert_eq!(
            denial.source,
            PolicySource::Principle {
                principle: "least-privilege".parse().expect("id")
            }
        );
    }

    #[test]
    fn a_task_cannot_need_a_capability_the_policy_denies() {
        let principle = r"
id: least-privilege
version: 1
title: Least privilege
requires:
  always:
    - task.id
capabilities:
  deny: [secret.read]
";
        let profile = r"
id: test.standard
title: Test standard
protocol: aep/1
workflow: test/linear
principles: [least-privilege]
capabilities:
  allow: [repository.read]
completion:
  - tests.unit.failed == 0
";
        let registry = fixtures::registry(
            &[fixtures::PROTOCOL],
            &[principle],
            &[fixtures::WORKFLOW],
            &[profile],
        );
        let task = fixtures::task(
            r"
id: T-6
kind: feature
objective: something
protocol: aep/1
profile: test.standard
constraints:
  capabilities:
    allow: [secret.read]
",
        );
        let errors = resolve(&task, &registry).expect_err("conflict");
        assert!(
            errors.contains(ValidationCode::CapabilityConflict),
            "{errors}"
        );
    }

    #[test]
    fn a_profile_extending_another_inherits_its_workflow_and_tightens_completion() {
        let critical = r"
id: test.critical
title: Test critical
protocol: aep/1
extends: test.standard
principles: [static-analysis]
completion:
  - static_analysis.errors == 0
";
        let static_analysis = r"
id: static-analysis
version: 1
title: Static analysis
requires:
  before_completion:
    - static_analysis.errors == 0
evidence: [static_analysis]
";
        let registry = fixtures::registry(
            &[fixtures::PROTOCOL],
            &[fixtures::TEST_DRIVEN, static_analysis],
            &[fixtures::WORKFLOW],
            &[fixtures::PROFILE, critical],
        );
        let task = fixtures::task(
            r"
id: T-7
kind: feature
objective: something
protocol: aep/1
profile: test.critical
",
        );
        let plan = resolve(&task, &registry).expect("resolves");
        assert_eq!(
            plan.workflow.id.as_str(),
            "test/linear",
            "workflow is inherited"
        );
        assert_eq!(plan.principles.len(), 2);
        assert_eq!(
            plan.completion.predicates.len(),
            2,
            "extending a profile can only make completion harder"
        );
    }

    #[test]
    fn rejects_a_task_whose_protocol_the_profile_does_not_refine() {
        let unrelated = fixtures::PROTOCOL.replace("id: aep", "id: aop");
        let registry = fixtures::registry(
            &[fixtures::PROTOCOL, &unrelated],
            &[fixtures::TEST_DRIVEN],
            &[fixtures::WORKFLOW],
            &[fixtures::PROFILE],
        );
        let task = fixtures::task(
            r"
id: T-8
kind: feature
objective: something
protocol: aop/1
profile: test.standard
",
        );
        let errors = resolve(&task, &registry).expect_err("unrelated protocol");
        assert!(errors.contains(ValidationCode::VersionMismatch), "{errors}");
    }

    #[test]
    fn a_task_may_name_the_base_protocol_its_profile_refines() {
        // This is the shape the design documents use: the task says `aep/1`, the profile is written
        // against the development protocol that extends it.
        let derived = r"
id: adp
version: 1
title: Development protocol
extends: aep/1
observables:
  - 'build.**'
";
        let profile = fixtures::PROFILE.replace("protocol: aep/1", "protocol: adp/1");
        let registry = fixtures::registry(
            &[fixtures::PROTOCOL, derived],
            &[fixtures::TEST_DRIVEN],
            &[fixtures::WORKFLOW],
            &[&profile],
        );
        let plan = resolve(&fixtures::standard_task(), &registry).expect("resolves");
        assert_eq!(
            plan.protocol.id.as_str(),
            "adp",
            "the plan is checked against the profile's more specific protocol"
        );
        assert!(
            plan.protocol
                .is_observable(&"tests.unit.failed".parse().expect("path")),
            "the base protocol's observables are inherited"
        );
        assert!(plan
            .protocol
            .is_observable(&"build.status".parse().expect("path")));
    }
}
