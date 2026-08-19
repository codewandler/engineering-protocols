//! Parse and validation errors.
//!
//! The two error types mirror the two-stage document model:
//!
//! * [`ParseError`] — a value is not well formed (bad identifier, unparsable predicate).
//!   Raised while deserializing, and therefore reported by [`serde`] with document context.
//! * [`ValidationError`] — a document is well formed but not semantically valid (a
//!   transition points at a state that does not exist). Raised by `TryFrom` conversions and
//!   collected into [`ValidationErrors`] so that one run reports every problem, not the
//!   first.

use std::fmt;

/// A value that is not well formed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ParseError {
    /// An identifier violates its charset rule.
    #[error("invalid {kind} identifier {value:?}: {reason}")]
    Identifier {
        /// What kind of identifier was expected, such as `principle`.
        kind: &'static str,
        /// The offending value.
        value: String,
        /// Why it was rejected.
        reason: String,
    },

    /// A version reference could not be parsed.
    #[error("invalid {kind} reference {value:?}: {reason}")]
    Reference {
        /// What kind of reference was expected, such as `protocol`.
        kind: &'static str,
        /// The offending value.
        value: String,
        /// Why it was rejected.
        reason: String,
    },

    /// A predicate expression could not be parsed.
    #[error("cannot parse predicate {expression:?}: {reason}")]
    Predicate {
        /// The offending expression.
        expression: String,
        /// Why it was rejected.
        reason: String,
    },

    /// A capability string is not a known capability.
    #[error("unknown capability {value:?}: {reason}")]
    Capability {
        /// The offending value.
        value: String,
        /// Why it was rejected, including the accepted names where the list is short enough.
        reason: String,
    },

    /// A document fragment has the wrong shape.
    #[error("{location}: expected {expected}, found {found}")]
    Shape {
        /// Where in the document the problem is, in dotted form.
        location: String,
        /// What was expected.
        expected: String,
        /// What was found.
        found: String,
    },
}

impl ParseError {
    /// Builds a [`ParseError::Identifier`].
    pub fn identifier(kind: &'static str, value: &str, reason: String) -> Self {
        Self::Identifier {
            kind,
            value: value.to_owned(),
            reason,
        }
    }

    /// Builds a [`ParseError::Reference`].
    pub fn reference(kind: &'static str, value: &str, reason: impl Into<String>) -> Self {
        Self::Reference {
            kind,
            value: value.to_owned(),
            reason: reason.into(),
        }
    }

    /// Builds a [`ParseError::Predicate`].
    pub fn predicate(expression: &str, reason: impl Into<String>) -> Self {
        Self::Predicate {
            expression: expression.to_owned(),
            reason: reason.into(),
        }
    }

    /// Builds a [`ParseError::Capability`].
    pub fn capability(value: &str, reason: impl Into<String>) -> Self {
        Self::Capability {
            value: value.to_owned(),
            reason: reason.into(),
        }
    }

    /// Builds a [`ParseError::Shape`].
    pub fn shape(
        location: impl Into<String>,
        expected: impl Into<String>,
        found: impl Into<String>,
    ) -> Self {
        Self::Shape {
            location: location.into(),
            expected: expected.into(),
            found: found.into(),
        }
    }
}

/// Stable machine-readable classification of a semantic validation failure.
///
/// Codes are part of the public interface: harnesses and tests match on them rather than on
/// message text.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ValidationCode {
    /// A workflow declares no states.
    EmptyWorkflow,
    /// The initial state does not exist.
    UnknownInitialState,
    /// A transition, obligation or override references a state that does not exist.
    UnknownState,
    /// A non-terminal state has no outgoing transition, so execution would wedge.
    DeadEndState,
    /// A state cannot be reached from the initial state.
    UnreachableState,
    /// Two transitions share the same source and target.
    DuplicateTransition,
    /// A referenced principle is not in the registry.
    UnknownPrinciple,
    /// The same principle is listed twice.
    DuplicatePrinciple,
    /// A referenced profile is not in the registry.
    UnknownProfile,
    /// A referenced workflow is not in the registry.
    UnknownWorkflow,
    /// A referenced protocol is not in the registry.
    UnknownProtocol,
    /// A document requires a protocol major version this build does not implement.
    UnsupportedProtocolVersion,
    /// A profile or task references a capability the protocol does not declare.
    UndeclaredCapability,
    /// A requirement references an evidence kind the protocol does not declare.
    UndeclaredEvidenceKind,
    /// Required evidence has no verifier that can establish it.
    NoVerifierForEvidence,
    /// Rollback is required for a state marked irreversible.
    RollbackOnIrreversibleState,
    /// A capability is both needed and explicitly denied.
    CapabilityConflict,
    /// Production mutation is allowed without an approval requirement.
    ProductionWriteWithoutApproval,
    /// A predicate references a fact the protocol does not declare observable.
    UnobservableFact,
    /// An obligation is timed against a phase no state declares.
    UnknownPhase,
    /// A version mismatch between a pinned reference and the registry entry.
    VersionMismatch,
    /// A rollback failure policy is declared with no way to identify what to roll back to.
    IncompleteRollbackPolicy,
}

impl ValidationCode {
    /// The code as it appears in output, such as `unreachable_state`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::EmptyWorkflow => "empty_workflow",
            Self::UnknownInitialState => "unknown_initial_state",
            Self::UnknownState => "unknown_state",
            Self::DeadEndState => "dead_end_state",
            Self::UnreachableState => "unreachable_state",
            Self::DuplicateTransition => "duplicate_transition",
            Self::UnknownPrinciple => "unknown_principle",
            Self::DuplicatePrinciple => "duplicate_principle",
            Self::UnknownProfile => "unknown_profile",
            Self::UnknownWorkflow => "unknown_workflow",
            Self::UnknownProtocol => "unknown_protocol",
            Self::UnsupportedProtocolVersion => "unsupported_protocol_version",
            Self::UndeclaredCapability => "undeclared_capability",
            Self::UndeclaredEvidenceKind => "undeclared_evidence_kind",
            Self::NoVerifierForEvidence => "no_verifier_for_evidence",
            Self::RollbackOnIrreversibleState => "rollback_on_irreversible_state",
            Self::CapabilityConflict => "capability_conflict",
            Self::ProductionWriteWithoutApproval => "production_write_without_approval",
            Self::UnobservableFact => "unobservable_fact",
            Self::UnknownPhase => "unknown_phase",
            Self::VersionMismatch => "version_mismatch",
            Self::IncompleteRollbackPolicy => "incomplete_rollback_policy",
        }
    }
}

impl fmt::Display for ValidationCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One semantic validation failure.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, schemars::JsonSchema)]
pub struct ValidationError {
    /// Stable classification.
    pub code: ValidationCode,
    /// Where the problem is, in dotted document form, such as `workflow.transitions[3].to`.
    pub location: String,
    /// What is wrong.
    pub message: String,
    /// How to fix it, when there is an obvious remedy.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
}

impl ValidationError {
    /// Builds a validation error.
    pub fn new(
        code: ValidationCode,
        location: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            code,
            location: location.into(),
            message: message.into(),
            hint: None,
        }
    }

    /// Attaches a remediation hint.
    #[must_use]
    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}: {}", self.code, self.location, self.message)?;
        if let Some(hint) = &self.hint {
            write!(f, " (hint: {hint})")?;
        }
        Ok(())
    }
}

/// Every semantic validation failure found in one document or resolution.
///
/// Validation accumulates: a document with four broken references reports four errors, so a
/// caller does not have to fix and re-run four times.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, schemars::JsonSchema)]
#[serde(transparent)]
pub struct ValidationErrors(Vec<ValidationError>);

impl ValidationErrors {
    /// An empty error set.
    pub fn new() -> Self {
        Self(Vec::new())
    }

    /// Records a failure.
    pub fn push(&mut self, error: ValidationError) {
        self.0.push(error);
    }

    /// Records a failure and returns `self`, for builder-style accumulation.
    #[must_use]
    pub fn with(mut self, error: ValidationError) -> Self {
        self.push(error);
        self
    }

    /// Absorbs another error set.
    pub fn extend(&mut self, other: Self) {
        self.0.extend(other.0);
    }

    /// `true` when nothing failed.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The number of failures.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// The failures, in discovery order.
    pub fn as_slice(&self) -> &[ValidationError] {
        &self.0
    }

    /// `Ok(value)` when nothing failed, otherwise the error set.
    pub fn into_result<T>(self, value: T) -> Result<T, Self> {
        if self.is_empty() {
            Ok(value)
        } else {
            Err(self)
        }
    }

    /// `true` when any failure carries `code`.
    pub fn contains(&self, code: ValidationCode) -> bool {
        self.0.iter().any(|error| error.code == code)
    }
}

impl Default for ValidationErrors {
    fn default() -> Self {
        Self::new()
    }
}

impl From<ValidationError> for ValidationErrors {
    fn from(error: ValidationError) -> Self {
        Self(vec![error])
    }
}

impl IntoIterator for ValidationErrors {
    type Item = ValidationError;
    type IntoIter = std::vec::IntoIter<ValidationError>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl fmt::Display for ValidationErrors {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0.len() {
            0 => f.write_str("no validation errors"),
            1 => write!(f, "{}", self.0[0]),
            n => {
                writeln!(f, "{n} validation errors:")?;
                for error in &self.0 {
                    writeln!(f, "  - {error}")?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for ValidationErrors {}
