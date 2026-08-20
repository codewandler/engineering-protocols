//! Capability decisions, with the rule that produced them.
//!
//! A refusal that cannot say why is not much better than a crash. Every decision carries the
//! document and rule responsible, in the shape the design document specifies:
//!
//! ```yaml
//! decision:
//!   allowed: false
//! reason:
//!   principle: least-privilege
//!   rule: production-write-requires-approval
//! missing:
//!   - approval for capability production.write
//! current_state: diagnose
//! ```
//!
//! # How an approval is satisfied
//!
//! A capability behind approval becomes exercisable when an approval **granting** it has been
//! recorded as evidence, matched either by subject — `subject: capability:<slug>` — or by the
//! approval's own id being that slug, where the slug is the capability in kebab-case:
//!
//! ```text
//! production.write                 → production-write
//! deployment.create:production     → deployment-create-production
//! ```
//!
//! The decision on the record is read, not merely its existence: an approval whose decision is
//! `denied` is a reviewer refusing the change, and treating it as a grant would mean the act of
//! refusing a production write is what permits it.
//!
//! The engine does not invent approvals; a harness submits one like any other evidence.

use aep_domain::action::ActionRequest;
use aep_domain::capability::{Capability, CapabilityDecision, CapabilityPolicy, PolicySource};
use aep_domain::evidence::{ApprovalDecision, Evidence};
use aep_domain::ids::StateId;

use crate::execution::Execution;

/// Which document and rule produced a decision.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct DecisionReason {
    /// The document responsible.
    #[serde(flatten)]
    pub source: PolicySource,
    /// A stable rule name, such as `production-write-requires-approval`.
    pub rule: String,
}

/// What the protocol says about one proposed action.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Decision {
    /// Whether the action may proceed now.
    pub allowed: bool,
    /// What the policy says about the capability.
    pub decision: CapabilityDecision,
    /// The action, in one line.
    pub operation: String,
    /// The capability it needs.
    pub capability: Capability,
    /// Which document and rule decided, when one did.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<DecisionReason>,
    /// What would have to exist for this to be allowed.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub missing: Vec<String>,
    /// Where the execution is.
    pub current_state: StateId,
}

impl Decision {
    /// `true` when the action may proceed.
    pub fn is_allowed(&self) -> bool {
        self.allowed
    }
}

/// The policy in force in the current state.
///
/// A state may grant capabilities the profile did not — a release workflow's promote state is the
/// obvious case — but it can never grant past a denial, because `deny` wins by set membership, nor
/// past the protocol's approval floor, which is re-checked here.
pub fn effective_policy(execution: &Execution) -> CapabilityPolicy {
    let mut policy = execution.plan().capability_policy.clone();
    if let Ok(state) = execution.state() {
        if !state.capabilities.is_empty() {
            policy.grant(&state.capabilities);
        }
    }
    policy
}

/// Decides whether `request` may proceed.
pub fn authorize(execution: &Execution, request: &ActionRequest) -> Decision {
    let capability = request.required_capability();
    let policy = effective_policy(execution);
    let mut decision = policy.decide(&capability);

    // The protocol's floor applies even to a state-level grant.
    if decision == CapabilityDecision::Allowed
        && execution.plan().protocol.needs_approval_floor(&capability)
    {
        decision = CapabilityDecision::RequiresApproval;
    }

    let mut missing = Vec::new();
    let mut allowed = decision == CapabilityDecision::Allowed;

    if decision == CapabilityDecision::RequiresApproval {
        if approval_recorded(execution, &capability) {
            allowed = true;
        } else {
            missing.push(format!("approval for capability {capability}"));
        }
    }
    if decision == CapabilityDecision::NotGranted {
        missing.push(format!(
            "a grant for {capability} in the profile's capabilities"
        ));
    }

    Decision {
        allowed,
        decision,
        operation: request.action.summary(),
        capability: capability.clone(),
        reason: reason_for(execution, &capability, decision),
        missing,
        current_state: execution.state_id().clone(),
    }
}

/// `true` when an approval covering `capability` has been recorded.
fn approval_recorded(execution: &Execution, capability: &Capability) -> bool {
    let slug = capability_slug(capability);
    let subject = format!("capability:{slug}");
    execution.recorded_evidence().iter().any(|recorded| {
        let Evidence::Approval(approval) = &recorded.record.value else {
            return false;
        };
        if approval.decision != ApprovalDecision::Granted {
            return false;
        }
        let by_subject = approval
            .subject
            .as_ref()
            .is_some_and(|reference| reference.to_string() == subject);
        by_subject || approval.approval.as_str() == slug
    })
}

/// Finds the policy entry responsible for a decision.
fn reason_for(
    execution: &Execution,
    capability: &Capability,
    decision: CapabilityDecision,
) -> Option<DecisionReason> {
    let plan = execution.plan();
    // The last matching entry wins: rationale is recorded in composition order, and the narrowest
    // document to speak about a capability is the one that decided it.
    let entry = plan
        .capability_rationale
        .iter()
        .rev()
        .find(|grant| grant.capability.covers(capability) && grant.effect == decision);

    match entry {
        Some(grant) => Some(DecisionReason {
            source: grant.source.clone(),
            rule: rule_name(capability, decision),
        }),
        None if decision == CapabilityDecision::RequiresApproval => Some(DecisionReason {
            // No document listed it: the protocol's floor put it behind approval.
            source: PolicySource::Protocol {
                protocol: plan.protocol.id.clone(),
            },
            rule: rule_name(capability, decision),
        }),
        None => None,
    }
}

/// The stable rule name for a decision, such as `production-write-requires-approval`.
fn rule_name(capability: &Capability, decision: CapabilityDecision) -> String {
    let slug = capability_slug(capability);
    match decision {
        CapabilityDecision::Allowed => format!("{slug}-allowed"),
        CapabilityDecision::RequiresApproval => format!("{slug}-requires-approval"),
        CapabilityDecision::Denied => format!("{slug}-denied"),
        CapabilityDecision::NotGranted => format!("{slug}-not-granted"),
    }
}

/// Renders a capability as a kebab-case slug.
fn capability_slug(capability: &Capability) -> String {
    capability
        .to_string()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '-'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures;
    use crate::resolve::resolve;
    use aep_domain::action::{Action, Deploy, RepositoryWrite, TestExecute};
    use aep_domain::artifact::ArtifactGraph;
    use aep_domain::capability::Environment;
    use aep_domain::evidence::{ApprovalRecord, Evidence, EvidenceRecord, Producer, TestSuite};
    use aep_domain::ids::{EvidenceId, ExecutionId};
    use aep_domain::time::Timestamp;

    const GUARDED_PROFILE: &str = r"
id: test.guarded
title: Guarded
protocol: aep/1
workflow: test/linear
principles: [least-privilege]
capabilities:
  allow: [repository.read, tests.execute]
  require_approval: [production.write, repository.write]
  deny: [secret.read]
completion:
  - tests.unit.failed == 0
";

    const LEAST_PRIVILEGE: &str = r"
id: least-privilege
version: 1
title: Least privilege
requires:
  always:
    - task.id
capabilities:
  deny: [secret.read]
  require_approval: [production.write]
";

    fn execution(profile: &str) -> crate::execution::Execution {
        let registry = fixtures::registry(
            &[fixtures::PROTOCOL],
            &[LEAST_PRIVILEGE],
            &[fixtures::WORKFLOW],
            &[profile],
        );
        let task = fixtures::task(
            r"
id: T-9
kind: feature
objective: something
protocol: aep/1
profile: test.guarded
",
        );
        let plan = resolve(&task, &registry).expect("resolves");
        crate::execution::Execution::new(
            ExecutionId::new("exec.1").expect("id"),
            plan,
            ArtifactGraph::new(),
        )
    }

    fn approval(subject: Option<&str>, id: &str) -> EvidenceRecord {
        decided(subject, id, ApprovalDecision::Granted)
    }

    /// An approval record with the decision spelled out.
    ///
    /// Separate from [`approval`] because a fixture that can only say `Granted` cannot reach the
    /// state where the decision is load-bearing — which is exactly why a rule that ignored the
    /// decision survived every test in this module.
    fn decided(subject: Option<&str>, id: &str, decision: ApprovalDecision) -> EvidenceRecord {
        let record = ApprovalRecord {
            approval: id.parse().expect("approval id"),
            approver: Producer::Human {
                id: "ada".to_owned(),
            },
            decision,
            subject: subject.map(|value| value.parse().expect("subject")),
            note: None,
        };
        EvidenceRecord::new(
            EvidenceId::new("a1").expect("id"),
            Timestamp::from_epoch_millis(1),
            Producer::Human {
                id: "ada".to_owned(),
            },
            Evidence::Approval(record),
        )
    }

    #[test]
    fn an_allowed_action_is_allowed_and_says_which_document_granted_it() {
        let execution = execution(GUARDED_PROFILE);
        let request = ActionRequest::new(Action::TestExecute(TestExecute {
            suite: TestSuite::Unit,
            selector: None,
        }));
        let decision = authorize(&execution, &request);
        assert!(decision.is_allowed());
        assert_eq!(decision.decision, CapabilityDecision::Allowed);
        let reason = decision.reason.expect("attributed");
        assert_eq!(reason.rule, "tests-execute-allowed");
    }

    #[test]
    fn production_write_is_refused_and_names_the_rule_that_refused_it() {
        let execution = execution(GUARDED_PROFILE);
        let request = ActionRequest::new(Action::ProductionMutate(
            aep_domain::action::ProductionMutate {
                target: "checkout.feature_flag".to_owned(),
                change: Some("disable".to_owned()),
            },
        ));
        let decision = authorize(&execution, &request);

        assert!(!decision.is_allowed());
        assert_eq!(decision.decision, CapabilityDecision::RequiresApproval);
        assert_eq!(
            decision.missing,
            vec!["approval for capability production.write".to_owned()],
            "the refusal says exactly what would unlock it"
        );
        let reason = decision.reason.expect("attributed");
        assert_eq!(reason.rule, "production-write-requires-approval");
        assert_eq!(
            reason.source,
            PolicySource::Principle {
                principle: "least-privilege".parse().expect("id")
            },
            "the principle that required approval is what the refusal names"
        );
    }

    #[test]
    fn deploying_to_an_environment_nobody_granted_is_not_granted() {
        let execution = execution(GUARDED_PROFILE);
        let request = ActionRequest::new(Action::Deploy(Deploy {
            environment: Environment::Production,
            revision: "rev-4711".to_owned(),
            strategy: None,
        }));
        let decision = authorize(&execution, &request);
        assert_eq!(decision.decision, CapabilityDecision::NotGranted);
    }

    #[test]
    fn a_state_grant_cannot_get_past_the_protocols_approval_floor() {
        // A release workflow's promote state legitimately grants production deployment. The floor
        // still applies: the grant becomes "allowed once an approval is recorded", not "allowed".
        let workflow = r"
id: test/release
title: Release
initial: promote
states:
  promote:
    title: Promote
    phases: [implementation]
    capabilities:
      allow: [deployment.create:production]
  done:
    title: Done
    terminal: true
    phases: [completion]
transitions:
  - from: promote
    to: done
    when: deployment.succeeded
";
        let profile = r"
id: test.release
title: Release
protocol: aep/1
workflow: test/release
principles: []
capabilities:
  allow: [telemetry.read]
completion:
  - deployment.succeeded
";
        let registry = fixtures::registry(
            &[fixtures::PROTOCOL],
            &[LEAST_PRIVILEGE],
            &[workflow],
            &[profile],
        );
        let task = fixtures::task(
            r"
id: REL-1
kind: release
objective: ship it
protocol: aep/1
profile: test.release
",
        );
        let plan = resolve(&task, &registry).expect("resolves");
        let mut execution = crate::execution::Execution::new(
            ExecutionId::new("exec.2").expect("id"),
            plan,
            ArtifactGraph::new(),
        );

        let request = ActionRequest::new(Action::Deploy(Deploy {
            environment: Environment::Production,
            revision: "rev-4711".to_owned(),
            strategy: Some("canary".to_owned()),
        }));
        let decision = authorize(&execution, &request);
        assert_eq!(
            decision.decision,
            CapabilityDecision::RequiresApproval,
            "the state granted it, but the protocol floor still requires an approval"
        );
        assert!(!decision.is_allowed());

        execution.record_evidence(approval(
            Some("capability:deployment-create-production"),
            "production-change",
        ));
        assert!(
            authorize(&execution, &request).is_allowed(),
            "with the approval recorded, the state's grant takes effect"
        );
    }

    #[test]
    fn a_denied_capability_cannot_be_unlocked_by_an_approval() {
        let mut execution = execution(GUARDED_PROFILE);
        execution.record_evidence(approval(Some("capability:secret-read"), "secret-read"));
        let request = ActionRequest::new(Action::SecretRead(aep_domain::action::SecretRead {
            secret: "database-password".to_owned(),
        }));
        let decision = authorize(&execution, &request);
        assert_eq!(decision.decision, CapabilityDecision::Denied);
        assert!(!decision.is_allowed(), "a denial is not negotiable");
    }

    #[test]
    fn an_approval_recorded_as_evidence_unlocks_the_capability() {
        let mut execution = execution(GUARDED_PROFILE);
        let request = ActionRequest::new(Action::RepositoryWrite(RepositoryWrite {
            paths: vec!["src/lib.rs".to_owned()],
            intent: None,
        }));
        assert!(!authorize(&execution, &request).is_allowed());

        execution.record_evidence(approval(
            Some("capability:repository-write"),
            "change-review",
        ));
        let decision = authorize(&execution, &request);
        assert!(
            decision.is_allowed(),
            "the approval is evidence like any other; the engine does not invent it"
        );
        assert_eq!(decision.decision, CapabilityDecision::RequiresApproval);
    }

    #[test]
    fn an_approval_may_also_be_matched_by_its_identifier() {
        let mut execution = execution(GUARDED_PROFILE);
        let request = ActionRequest::new(Action::RepositoryWrite(RepositoryWrite {
            paths: vec!["src/lib.rs".to_owned()],
            intent: None,
        }));
        execution.record_evidence(approval(None, "repository-write"));
        assert!(authorize(&execution, &request).is_allowed());
    }

    #[test]
    fn a_reviewer_who_refuses_a_production_change_has_not_thereby_permitted_it() {
        // Delete the `decision != Granted` guard in `approval_recorded` and this is what happens:
        // a reviewer reads the change, refuses it, records the refusal — and the refusal is the
        // evidence that unlocks the production write. Every other test here records `Granted`, so
        // none of them can reach the state where that check does any work.
        let mut execution = execution(GUARDED_PROFILE);
        execution.record_evidence(decided(
            Some("capability:production-write"),
            "production-change",
            ApprovalDecision::Denied,
        ));
        let request = ActionRequest::new(Action::ProductionMutate(
            aep_domain::action::ProductionMutate {
                target: "checkout.feature_flag".to_owned(),
                change: Some("disable".to_owned()),
            },
        ));
        let decision = authorize(&execution, &request);

        assert!(!decision.is_allowed(), "a refusal is not a permission");
        assert_eq!(decision.decision, CapabilityDecision::RequiresApproval);
        assert_eq!(
            decision.missing,
            vec!["approval for capability production.write".to_owned()],
            "the approval is still owed; the refusal did not pay it"
        );
        let reason = decision.reason.expect("attributed");
        assert_eq!(reason.rule, "production-write-requires-approval");
        assert_eq!(
            reason.source,
            PolicySource::Principle {
                principle: "least-privilege".parse().expect("id")
            }
        );
    }

    #[test]
    fn a_refused_approval_matched_by_its_identifier_unlocks_nothing_either() {
        // `approval_recorded` matches on a subject *or* on the approval's own id. Two ways in is
        // two places the decision could go unread, so the refusal is checked through both.
        let mut execution = execution(GUARDED_PROFILE);
        execution.record_evidence(decided(None, "repository-write", ApprovalDecision::Denied));
        let request = ActionRequest::new(Action::RepositoryWrite(RepositoryWrite {
            paths: vec!["src/lib.rs".to_owned()],
            intent: None,
        }));
        let refused = authorize(&execution, &request);
        assert!(!refused.is_allowed(), "a refusal is not a permission");
        assert_eq!(
            refused.missing,
            vec!["approval for capability repository.write".to_owned()]
        );

        execution.record_evidence(approval(None, "repository-write"));
        assert!(
            authorize(&execution, &request).is_allowed(),
            "and a real grant still unlocks it, so the guard is not just refusing everything"
        );
    }

    #[test]
    fn a_capability_nobody_granted_is_refused_with_what_is_missing() {
        let execution = execution(GUARDED_PROFILE);
        let request =
            ActionRequest::new(Action::TelemetryQuery(aep_domain::action::TelemetryQuery {
                query: "rate(errors[5m])".to_owned(),
                service: None,
            }));
        let decision = authorize(&execution, &request);
        assert_eq!(decision.decision, CapabilityDecision::NotGranted);
        assert_eq!(
            decision.missing,
            vec!["a grant for telemetry.read in the profile's capabilities".to_owned()]
        );
    }
}
