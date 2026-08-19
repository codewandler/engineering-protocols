//! Explanations.
//!
//! Two shapes, both taken from the design specification, both built from the same data the engine
//! decides on — so the human view and the machine view cannot disagree.
//!
//! A refused action:
//!
//! ```yaml
//! decision:
//!   allowed: false
//!   operation: deploy rev-4711 to production
//! reason:
//!   principle: least-privilege
//!   rule: production-write-requires-approval
//! missing:
//!   - approval for capability production.write
//! current_state: diagnose
//! ```
//!
//! An incomplete task:
//!
//! ```text
//! Task incomplete in `verify`:
//!   ✓ tests.unit.failed == 0                       [completion]
//!   ✓ static_analysis.errors == 0                  [principle static-analysis]
//!   ✗ property_test.session_isolation.passed       [principle property-based-testing]
//!   ? approval security-review                     [principle approval-gates]
//! ```
//!
//! `✗` means something observed contradicts the requirement; `?` means nothing has observed it yet.
//! The distinction is the whole point: one says go and fix the code, the other says go and run the
//! verifier.

use std::fmt;

use aep_domain::ids::StateId;
use aep_domain::predicate::Truth;

use crate::evaluate::{Evaluation, Requirement};
use crate::policy::{Decision, DecisionReason};

/// The `decision:` block of a refusal.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct DecisionOutcome {
    /// Whether the action may proceed.
    pub allowed: bool,
    /// The action, in one line.
    pub operation: String,
    /// The capability it needed.
    pub capability: String,
}

/// Why an action was allowed or refused, in the specification's shape.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct DecisionExplanation {
    /// What was decided.
    pub decision: DecisionOutcome,
    /// Which document and rule decided.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<DecisionReason>,
    /// What would have to exist for this to be allowed.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub missing: Vec<String>,
    /// Where the execution is.
    pub current_state: StateId,
}

impl From<&Decision> for DecisionExplanation {
    fn from(decision: &Decision) -> Self {
        Self {
            decision: DecisionOutcome {
                allowed: decision.allowed,
                operation: decision.operation.clone(),
                capability: decision.capability.to_string(),
            },
            reason: decision.reason.clone(),
            missing: decision.missing.clone(),
            current_state: decision.current_state.clone(),
        }
    }
}

impl fmt::Display for DecisionExplanation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.decision.allowed {
            writeln!(f, "{} is allowed", self.decision.capability)?;
        } else {
            writeln!(f, "{} denied", self.decision.capability)?;
        }
        writeln!(f, "  operation: {}", self.decision.operation)?;
        if let Some(reason) = &self.reason {
            writeln!(f, "  reason:    {} rule {}", reason.source, reason.rule)?;
        }
        for missing in &self.missing {
            writeln!(f, "  missing:   {missing}")?;
        }
        write!(f, "  state:     {}", self.current_state)
    }
}

/// Whether a task is complete, and what is outstanding.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct CompletionExplanation {
    /// Whether the task is finished.
    pub complete: bool,
    /// Where the execution is.
    pub state: StateId,
    /// Whether the current state ends the workflow.
    pub terminal: bool,
    /// Every completion requirement, satisfied or not.
    pub items: Vec<ExplainedItem>,
}

/// One line of an explanation.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ExplainedItem {
    /// Whether it holds.
    pub satisfied: bool,
    /// `true`, `false` or `unknown`.
    pub truth: Truth,
    /// The requirement, in one line.
    pub requirement: String,
    /// What was observed, or what is missing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    /// Which document asked for it.
    pub source: String,
}

impl From<&Requirement> for ExplainedItem {
    fn from(requirement: &Requirement) -> Self {
        Self {
            satisfied: requirement.is_satisfied(),
            truth: requirement.outcome.truth,
            requirement: requirement.outcome.requirement.clone(),
            detail: requirement.outcome.detail.clone(),
            source: requirement.source.label(),
        }
    }
}

impl CompletionExplanation {
    /// Builds an explanation from an evaluation.
    pub fn from_evaluation(evaluation: &Evaluation) -> Self {
        Self {
            complete: evaluation.is_complete,
            state: evaluation.state.clone(),
            terminal: evaluation.terminal,
            items: evaluation
                .completion
                .iter()
                .map(ExplainedItem::from)
                .collect(),
        }
    }

    /// The requirements that do not hold.
    pub fn outstanding(&self) -> impl Iterator<Item = &ExplainedItem> {
        self.items.iter().filter(|item| !item.satisfied)
    }
}

impl fmt::Display for CompletionExplanation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.complete {
            writeln!(f, "Task complete in `{}`:", self.state)?;
        } else if self.terminal {
            writeln!(
                f,
                "Task incomplete in `{}` (a terminal state, so it cannot progress):",
                self.state
            )?;
        } else {
            writeln!(f, "Task incomplete in `{}`:", self.state)?;
        }
        if self.items.is_empty() {
            return write!(f, "  (the profile states no completion condition)");
        }
        let width = self
            .items
            .iter()
            .map(|item| item.requirement.chars().count())
            .max()
            .unwrap_or(0)
            .min(60);
        for item in &self.items {
            let mark = match item.truth {
                Truth::True => '\u{2713}',
                Truth::False => '\u{2717}',
                Truth::Unknown => '?',
            };
            writeln!(
                f,
                "  {mark} {:<width$}  [{}]",
                item.requirement,
                item.source,
                width = width
            )?;
            if let Some(detail) = &item.detail {
                writeln!(f, "      {detail}")?;
            }
        }
        Ok(())
    }
}
