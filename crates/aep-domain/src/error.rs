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

/// Declares every validation code once.
///
/// The wire string and the [`ValidationCode::ALL`] list are generated from the same line as the
/// variant, because both were previously maintained by hand and both had silently fallen behind:
/// five codes existed, were emitted, and were absent from the list the tests iterate — so the guard
/// that was supposed to catch exactly that reported success.
macro_rules! validation_codes {
    ($( $(#[$attribute:meta])* $variant:ident => $wire:literal, )*) => {
        /// Stable machine-readable classification of a semantic validation failure.
        ///
        /// Codes are part of the public interface: harnesses and tests match on them rather than on
        /// message text.
        #[derive(
            Debug,
            Clone,
            Copy,
            PartialEq,
            Eq,
            PartialOrd,
            Ord,
            Hash,
            serde::Serialize,
            schemars::JsonSchema,
        )]
        #[serde(rename_all = "snake_case")]
        #[non_exhaustive]
        pub enum ValidationCode {
            $( $(#[$attribute])* $variant, )*
        }

        impl ValidationCode {
            /// Every code this build can produce, in declaration order.
            ///
            /// Generated, so it cannot fall behind the enum.
            pub const ALL: &'static [Self] = &[ $( Self::$variant, )* ];

            /// The code as it appears in output, such as `unreachable_state`.
            pub fn as_str(self) -> &'static str {
                match self {
                    $( Self::$variant => $wire, )*
                }
            }
        }
    };
}

validation_codes! {
    /// A workflow declares no states.
    EmptyWorkflow => "empty_workflow",

    /// The initial state does not exist.
    UnknownInitialState => "unknown_initial_state",

    /// A transition, obligation or override references a state that does not exist.
    UnknownState => "unknown_state",

    /// A non-terminal state has no outgoing transition, so execution would wedge.
    DeadEndState => "dead_end_state",

    /// A state cannot be reached from the initial state.
    UnreachableState => "unreachable_state",

    /// Two transitions share the same source and target.
    DuplicateTransition => "duplicate_transition",

    /// A referenced principle is not in the registry.
    UnknownPrinciple => "unknown_principle",

    /// The same principle is listed twice.
    DuplicatePrinciple => "duplicate_principle",

    /// A referenced profile is not in the registry.
    UnknownProfile => "unknown_profile",

    /// A referenced workflow is not in the registry.
    UnknownWorkflow => "unknown_workflow",

    /// A referenced protocol is not in the registry.
    UnknownProtocol => "unknown_protocol",

    /// A document requires a protocol major version this build does not implement.
    UnsupportedProtocolVersion => "unsupported_protocol_version",

    /// A document is written in a specification format version this build does not implement.
    ///
    /// Distinct from [`Self::UnsupportedProtocolVersion`], which is about the protocol a project
    /// executes: `ess/2` names how a document is written, not what governs the work, and a tool
    /// told the wrong one goes looking for the wrong upgrade.
    UnsupportedFormatVersion => "unsupported_format_version",

    /// A document uses a construct this build does not implement.
    ///
    /// Distinct from [`Self::UnsupportedFormatVersion`], which is about the version a document is
    /// written in: this one is a legal-looking thing the reader will implement later, and telling
    /// the two apart is the difference between "upgrade the tool" and "write it another way".
    UnsupportedConstruct => "unsupported_construct",

    /// A profile or task references a capability the protocol does not declare.
    UndeclaredCapability => "undeclared_capability",

    /// A requirement references an evidence kind the protocol does not declare.
    UndeclaredEvidenceKind => "undeclared_evidence_kind",

    /// Required evidence has no verifier that can establish it.
    NoVerifierForEvidence => "no_verifier_for_evidence",

    /// Rollback is required for a state marked irreversible.
    RollbackOnIrreversibleState => "rollback_on_irreversible_state",

    /// A capability is both needed and explicitly denied.
    CapabilityConflict => "capability_conflict",

    /// Production mutation is allowed without an approval requirement.
    ProductionWriteWithoutApproval => "production_write_without_approval",

    /// A condition reads something nothing makes available.
    ///
    /// In a protocol, a predicate references a fact the protocol does not declare observable. In a
    /// specification, a command's guard or a view's filter reads a field its subject does not have.
    /// Both are conditions nobody can decide.
    UnobservableFact => "unobservable_fact",

    /// An obligation is timed against a phase no state declares.
    UnknownPhase => "unknown_phase",

    /// A version mismatch between a pinned reference and the registry entry.
    VersionMismatch => "version_mismatch",

    /// A rollback failure policy is declared with no way to identify what to roll back to.
    IncompleteRollbackPolicy => "incomplete_rollback_policy",

    /// Something references itself where that cannot mean anything.
    SelfReference => "self_reference",

    /// Something that exists to change state changes nothing.
    ///
    /// A command whose acceptance would produce a revision nobody can explain; a specified outcome
    /// that neither emits an event nor names an error, so nothing about it is observable.
    EmptyChange => "empty_change",

    /// A refusal is recorded together with a change.
    ///
    /// An audit record that says an action was refused and also records a change; a specified
    /// outcome that reports an error and also emits. A refused command changes nothing, so either
    /// is two outcomes wearing one name.
    RefusalMutatedState => "refusal_mutated_state",

    /// An audit record says an entity changed without recording what changed.
    UnreconstructableChange => "unreconstructable_change",

    /// Something that settles an outcome does not say on what.
    ///
    /// A record of a decision that does not say what was decided; a specified outcome decided
    /// outside the input that names no cause, which leaves nobody able to reproduce it.
    UnexplainedDecision => "unexplained_decision",

    /// An audit record's redaction fields contradict each other.
    RedactionInconsistent => "redaction_inconsistent",

    /// An event's declared type does not match what its payload asserts.
    EventPayloadMismatch => "event_payload_mismatch",

    /// An event names a subject without the revision it describes, or the reverse.
    IncompleteEventSubject => "incomplete_event_subject",

    /// An event caused by a command does not name that command as its cause.
    MissingCausation => "missing_causation",

    /// Something written where a reference belongs is not one, and differs from a recognised form
    /// by a typo.
    ///
    /// Distinct from [`Self::UndeclaredReference`], where the reference is well formed and names
    /// nothing. Here the text was not read as a reference at all — `evnt.customer_email` becomes a
    /// literal string and is sent as one — so "not declared" would be a false statement about a
    /// name nobody looked up.
    MisspelledReference => "misspelled_reference",

    /// A reference names something nothing declares.
    ///
    /// Distinct from [`Self::UnknownState`], which is about workflow states specifically: a
    /// specification refers to types, events, errors, entities, fields and domains too, and a tool
    /// reading `unknown_state` off a missing event learns the wrong thing about what to fix.
    UndeclaredReference => "undeclared_reference",

    /// The same name is declared twice, and neither declaration can be said to win.
    DuplicateDeclaration => "duplicate_declaration",

    /// A declaration declares nothing that could have an effect: no fields, no outcomes, no
    /// states, nothing to enforce, no statement of what being finished means.
    EmptyDeclaration => "empty_declaration",

    /// A document does not make a declaration it is required to make: a required key is absent, or
    /// nothing declares the thing every other source contributes to.
    ///
    /// Distinct from [`Self::EmptyDeclaration`], which is about a declaration that is there and
    /// says nothing. Here there is nothing to read at all, and the repair is to write one.
    MissingDeclaration => "missing_declaration",

    /// Two declarations are each well formed and cannot both hold.
    ConflictingDeclaration => "conflicting_declaration",

    /// A declared value is not the kind of thing its position requires.
    ///
    /// A declared type disagreeing with the type it must match; a task naming something that is not
    /// an artifact reference; an absolute path where only a relative one has a meaning.
    TypeMismatch => "type_mismatch",

    /// A set of conditional branches leaves some input with no branch, so what happens to it is
    /// unspecified.
    ///
    /// Distinct from [`Self::DeadEndState`], which is about a state machine with no way onward: the
    /// branches here are each fine, and it is the gap between them that nobody has decided.
    NonExhaustiveBranches => "non_exhaustive_branches",

    /// Every branch of a declaration is decided outside its input, so no caller can reach one by
    /// choosing what to send.
    ///
    /// Distinct from [`Self::UnreachableState`], which is about a graph nothing walks into, and
    /// from [`Self::EmptyChange`], which is about a branch that does nothing: these branches do
    /// something, and nothing a caller can write selects between them.
    UnreachableBranch => "unreachable_branch",
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_code_has_a_distinct_stable_string() {
        let mut seen: Vec<&str> = Vec::new();
        for code in ValidationCode::ALL {
            let rendered = code.as_str();
            assert!(
                !rendered.is_empty()
                    && rendered.chars().all(|c| c.is_ascii_lowercase() || c == '_'),
                "{code:?} renders as {rendered:?}, which is not a snake_case code"
            );
            assert!(
                !seen.contains(&rendered),
                "two codes both render as {rendered:?}; a caller matching on it cannot tell them apart"
            );
            seen.push(rendered);
        }
    }

    #[test]
    fn the_serialised_form_matches_the_string_form() {
        for code in ValidationCode::ALL {
            let json = serde_json::to_string(code).expect("serialises");
            assert_eq!(
                json,
                format!("\"{}\"", code.as_str()),
                "a code's wire form and its display form must not drift apart"
            );
        }
    }

    #[test]
    fn errors_accumulate_and_report_every_problem() {
        let mut errors = ValidationErrors::new();
        assert!(errors.is_empty());
        errors.push(ValidationError::new(
            ValidationCode::UnknownState,
            "workflow.transitions[0].to",
            "`ghost` is not a declared state",
        ));
        errors.push(
            ValidationError::new(
                ValidationCode::DeadEndState,
                "workflow.states.a",
                "`a` has no outgoing transition",
            )
            .with_hint("add a transition, or mark it terminal"),
        );

        assert_eq!(errors.len(), 2);
        assert!(errors.contains(ValidationCode::DeadEndState));
        assert!(!errors.contains(ValidationCode::UnreachableState));

        let rendered = errors.to_string();
        assert!(rendered.contains("2 validation errors"), "{rendered}");
        assert!(rendered.contains("hint: add a transition"), "{rendered}");
    }
}
