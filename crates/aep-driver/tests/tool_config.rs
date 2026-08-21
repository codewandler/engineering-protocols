//! Review finding **F3**, as a test: the tool set is the *decision*, not one of its three inputs.
//!
//! The mutation this file exists to kill is one line long. The first draft of D3(a) derived an
//! `llm` step's tools from `CapabilityPolicy::allow` and called that invariant 6's ordering. It is
//! not: the ordering lives in `CapabilityPolicy::decide`, the three sets are independent — `grant`
//! extends all three — and membership is by `covers` rather than equality.

use aep_domain::capability::{Capability, CapabilityDecision, CapabilityPolicy, Environment};
use aep_driver::tool::{tool_config, TOOL_CANDIDATES};

/// The pairing the shipped profiles avoid **in a comment**, which is why it is a fixture here.
///
/// `allow` holds unscoped `deployment.create`, which parses to `Deploy(Environment::Any)`;
/// `approval_required` holds the scoped `deployment.create:production` that
/// `principles/governance/approval-gates.yaml` gates and the protocol floor re-checks.
fn policy_with_a_wide_grant_and_a_narrow_gate() -> CapabilityPolicy {
    CapabilityPolicy {
        allow: [
            Capability::Deploy(Environment::Any),
            Capability::RepositoryWrite,
        ]
        .into_iter()
        .collect(),
        approval_required: [Capability::Deploy(Environment::Production)]
            .into_iter()
            .collect(),
        deny: [Capability::ProductionWrite].into_iter().collect(),
    }
}

#[test]
fn a_wide_allow_entry_does_not_hand_out_a_narrowly_gated_deploy() {
    let policy = policy_with_a_wide_grant_and_a_narrow_gate();

    // The shape of the implementation this kills: `policy.allow.iter().cloned().collect()`. That
    // reads `deployment.create` out of `allow`, hands over the tool, and the model can then deploy
    // to production — the exact grant a principle put behind a human approval. It passes every
    // other test in this file.
    let tools = tool_config(&policy);
    let admitted: Vec<&Capability> = tools
        .capabilities()
        .iter()
        .filter(|capability| matches!(capability, Capability::Deploy(_) | Capability::Rollback(_)))
        .collect();
    assert!(
        admitted.is_empty(),
        "no deployment capability may be offered when one of them is gated: {admitted:?}"
    );

    // And the decision it rests on, asserted separately so a failure says which half moved.
    assert_eq!(
        policy.decide(&Capability::Deploy(Environment::Production)),
        CapabilityDecision::RequiresApproval,
        "`decide` is the one function that owns invariant 6's ordering"
    );
    assert!(
        policy.allow.contains(&Capability::Deploy(Environment::Any)),
        "the fixture's whole point is that `allow` does hold a deploy grant"
    );
}

#[test]
fn a_capability_that_was_never_granted_is_no_tool_and_a_denied_one_is_not_either() {
    let policy = policy_with_a_wide_grant_and_a_narrow_gate();
    let tools = tool_config(&policy);

    assert!(
        !tools.shell_offered(),
        "no development profile grants `command.execute`, so a development `llm` step holds no \
         shell: `cargo test` runs as a `command` step the driver executes, not as a tool the model \
         holds"
    );
    assert!(
        !tools.admits(&Capability::CommandExecution),
        "`NotGranted` maps to no tool"
    );
    assert!(
        !tools.admits(&Capability::ProductionWrite),
        "`Denied` maps to no tool"
    );
    assert!(
        tools.admits(&Capability::RepositoryWrite),
        "`Allowed` is the only decision that maps to a tool"
    );
}

#[test]
fn every_simple_capability_is_a_candidate_and_the_scoped_ones_ask_about_production() {
    for capability in Capability::SIMPLE {
        assert!(
            TOOL_CANDIDATES.contains(capability),
            "`{capability}` is a simple capability the tool table would never ask about; the two \
             lists have drifted, and a capability nobody asks about is one nobody can be offered"
        );
    }
    assert_eq!(
        TOOL_CANDIDATES.len(),
        Capability::SIMPLE.len() + 2,
        "the candidates are the simple capabilities plus the two that take an environment"
    );
    for scoped in [
        Capability::Deploy(Environment::Production),
        Capability::Rollback(Environment::Production),
    ] {
        assert!(
            TOOL_CANDIDATES.contains(&scoped),
            "`{scoped}` must be asked about at its production scope: `covers` widens from the \
             wildcard outwards, so asking about `Any` would step around an approval scoped to \
             production"
        );
    }
    assert!(
        !TOOL_CANDIDATES.contains(&Capability::Deploy(Environment::Any)),
        "asking about the wildcard is the question a narrow approval gate cannot answer"
    );
}

#[test]
fn an_empty_policy_offers_nothing_and_the_two_constant_answers_stand() {
    let tools = tool_config(&CapabilityPolicy::default());
    assert!(
        tools.is_empty(),
        "a policy that grants nothing offers nothing"
    );
    assert!(
        tools.skills_offered(),
        "a skill loader takes no action; everything it causes is a subsequent governed tool call"
    );
    assert!(
        !tools.subagents_offered(),
        "a subagent's tool set is derived by nothing here, so it would be a route around the \
         per-state allowlist"
    );
}
