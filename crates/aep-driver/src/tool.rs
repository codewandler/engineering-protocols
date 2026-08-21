//! Adapter point 2: capabilities become a tool description, and the decision is `decide`'s.
//!
//! ```rust
//! # use aep_domain::capability::CapabilityPolicy;
//! # use aep_driver::tool::tool_config;
//! # let policy = CapabilityPolicy::default();
//! let tools = tool_config(&policy);
//! assert!(!tools.shell_offered());
//! ```
//!
//! A pure function and deliberately **not** a trait. Making this point a trait method would let a
//! second harness quietly re-decide that `repository.write` admits a shell, and the protocol would
//! have no way to notice. A function every adapter calls, with the naming table as its only
//! per-harness input, is the narrower seam and the one that keeps a second harness honest.
//!
//! # The input is the decision, not one of the decision's three inputs (F3)
//!
//! `allow`, `approval_required` and `deny` are three independent sets; nothing removes a capability
//! from `allow` when a principle adds it to `deny`, and membership is by `covers` rather than
//! equality. An unscoped `allow: deployment.create` — which parses to `Deploy(Environment::Any)` —
//! therefore **covers** the scoped `deployment.create:production` that `approval-gates.yaml` puts
//! behind an approval, so iterating `.allow` hands out the tool the principle exists to gate. The
//! ordering lives in exactly one function, `CapabilityPolicy::decide`, and this one calls it.
//!
//! # A gap this function does not close, stated where somebody will meet it
//!
//! `aep_engine::policy::authorize` re-applies the **protocol's approval floor** on top of the
//! policy's answer: a capability the policy allows is still turned into `RequiresApproval` when
//! `Protocol::needs_approval_floor` says so. This function is specified as *admits iff
//! `decide(..) == Allowed`* (§ 4.9 point 2, D3(a)) and so does not consult the floor, which means a
//! floor-gated capability a profile allows outright — `production.write` under the shipped test
//! protocol — is offered as a tool and then refused at `authorize`. The refusal still happens, so
//! nothing ungoverned occurs; what the model sees is a tool it cannot successfully use. Closing it
//! means giving this function the `Protocol` as well, which changes the published adapter surface,
//! so it is recorded here rather than taken unilaterally.

use aep_domain::capability::{Capability, CapabilityDecision, CapabilityPolicy, Environment};
use aep_driver_spec::tool::ToolConfig;

/// The capabilities the tool table asks about, each at its **widest-reaching** scoped form.
///
/// `Capability::SIMPLE` plus `deployment.create:production` and `deployment.rollback:production`.
/// The scoped spelling is the load-bearing part: `decide` answers about the capability it is handed,
/// and `Deploy(Environment::Any)` is *not* covered by an `approval_required` entry for
/// `Deploy(Environment::Production)` — coverage widens from the wildcard outwards, never inwards.
/// Asking about the production form is therefore asking the strictest available question, and a
/// policy that gates production deployment answers `RequiresApproval` to it.
///
/// The sixteen simple capabilities are spelled out rather than spliced from `Capability::SIMPLE`
/// because a `const` slice cannot be concatenated. `tests/tool_config.rs` asserts the two lists
/// agree, so a seventeenth simple capability fails a test instead of silently never being offered.
pub const TOOL_CANDIDATES: &[Capability] = &[
    Capability::RepositoryRead,
    Capability::RepositoryWrite,
    Capability::TestExecution,
    Capability::CommandExecution,
    Capability::NetworkRead,
    Capability::NetworkWrite,
    Capability::TelemetryRead,
    Capability::ProductionRead,
    Capability::ProductionWrite,
    Capability::SecretRead,
    Capability::ArtifactRead,
    Capability::ArtifactWrite,
    Capability::PlanningRead,
    Capability::PlanningWrite,
    Capability::ReviewRequest,
    Capability::ApprovalRequest,
    Capability::Deploy(Environment::Production),
    Capability::Rollback(Environment::Production),
];

/// What a model may hold while a step runs, under `policy`.
///
/// A capability is admitted **iff `policy.decide(&capability) == CapabilityDecision::Allowed`**.
/// `RequiresApproval`, `Denied` and `NotGranted` all map to *no tool*, which is why the common case
/// needs no approval machinery at all: a development run that will never touch production cannot
/// reach the gated capability, because no step of it has a tool that would.
///
/// Total, clock-free and consumed by the executor. The result is harness-neutral — the adapter
/// renders it into its own tool names, and only the rendering is per-harness.
pub fn tool_config(policy: &CapabilityPolicy) -> ToolConfig {
    ToolConfig::new(
        TOOL_CANDIDATES
            .iter()
            .filter(|candidate| policy.decide(candidate) == CapabilityDecision::Allowed)
            .cloned()
            .collect(),
    )
}
