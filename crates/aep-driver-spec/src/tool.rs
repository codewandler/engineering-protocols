//! What a model may hold while a step runs, described without naming a single harness's tools.
//!
//! Adapter point 2 of § 4.9 is two layers, and the split is the point: the **decision** about which
//! capabilities admit which actions is shared and lives in the protocol's own table, where every
//! `Action` maps to exactly one `Capability`; only the **rendering** into `Bash`, `Edit`, `Read`
//! and the rest is harness-specific. This type is the boundary between them — the value a
//! harness-neutral function produces and a harness adapter renders.
//!
//! # Three entries are not functions of a capability
//!
//! | tool | decision |
//! |---|---|
//! | a shell | offered only when `command.execute` is admitted; granting it grants a superset of the shell's reach, and narrower gating by command pattern is best-effort. **No development profile grants it**, so a development `llm` step holds no shell — `cargo test` runs as a `command` step the driver executes, not as a tool the model holds |
//! | a skill loader | a **named exemption**. It loads instructions and takes no action; everything it causes is a subsequent, governed tool call |
//! | a subagent spawner | **never offered**. A subagent's tool set is derived by nothing in these decisions, so it would be a route around the per-state allowlist |
//!
//! Each is a decision recorded here rather than an implementer's judgement, which is what
//! [`ToolConfig::skills_offered`] and [`ToolConfig::subagents_offered`] exist to say out loud.

use std::collections::BTreeSet;

use aep_domain::capability::Capability;

/// The capabilities a step may exercise, decided.
///
/// Built only from `CapabilityPolicy::decide`, never from membership of `allow`: the three sets are
/// independent, membership is by `covers` rather than equality, and an unscoped `deployment.create`
/// in `allow` covers the scoped `deployment.create:production` that a principle put behind an
/// approval. Review finding **F3**, and the reason this type carries no constructor that takes a
/// set of *granted* capabilities.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct ToolConfig {
    admitted: BTreeSet<Capability>,
}

impl ToolConfig {
    /// Builds a configuration from capabilities that were **decided** admissible.
    ///
    /// The caller is `aep_driver::tool_config`, which is the one place the decision is made.
    pub fn new(admitted: BTreeSet<Capability>) -> Self {
        Self { admitted }
    }

    /// Every admitted capability, in a stable order.
    pub fn capabilities(&self) -> &BTreeSet<Capability> {
        &self.admitted
    }

    /// `true` when `capability` is admitted.
    ///
    /// Exact membership, deliberately: the set holds the capabilities a decision was actually taken
    /// about, and answering by `covers` here would re-introduce the widening this type exists to
    /// prevent.
    pub fn admits(&self, capability: &Capability) -> bool {
        self.admitted.contains(capability)
    }

    /// `true` when a shell may be offered.
    pub fn shell_offered(&self) -> bool {
        self.admits(&Capability::CommandExecution)
    }

    /// `true` when a skill loader may be offered, which it always may.
    ///
    /// A constant, not a computation: it takes no action, and the guide's rule that a tool with no
    /// `Action` cannot be governed is answered by everything it causes being a governed tool call.
    pub fn skills_offered(&self) -> bool {
        true
    }

    /// `true` when a subagent spawner may be offered, which it never may.
    pub fn subagents_offered(&self) -> bool {
        false
    }

    /// `true` when nothing at all is admitted.
    pub fn is_empty(&self) -> bool {
        self.admitted.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_two_constant_answers_are_stated_rather_than_computed() {
        let config = ToolConfig::default();
        assert!(config.is_empty());
        assert!(config.skills_offered());
        assert!(
            !config.subagents_offered(),
            "a subagent's tool set is derived by nothing here, so it is a route around the \
             per-state allowlist"
        );
        assert!(!config.shell_offered());
    }

    #[test]
    fn membership_is_exact_so_a_wide_entry_does_not_admit_a_narrow_one() {
        let config = ToolConfig::new(
            [Capability::Deploy(aep_domain::capability::Environment::Any)]
                .into_iter()
                .collect(),
        );
        assert!(config.admits(&Capability::Deploy(
            aep_domain::capability::Environment::Any
        )));
        assert!(
            !config.admits(&Capability::Deploy(
                aep_domain::capability::Environment::Production
            )),
            "answering by `covers` here would re-introduce exactly the widening F3 found"
        );
    }
}
