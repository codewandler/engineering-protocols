//! The failure taxonomy.
//!
//! Every variant exists so a caller can branch on *what went wrong* rather than parse a message. The
//! two that matter most are [`CommandError::RevisionConflict`] — machine-readable by design, because
//! a client has to be able to refetch and retry — and [`CommandError::Unauthorised`], which is how a
//! protocol refusal reaches the caller.

use aep_domain::capability::Capability;
use aep_domain::entity::{EntityRef, EntityRevision};
use aep_domain::error::ValidationErrors;
use aep_domain::ids::{CommandId, IdempotencyKey};

/// Why a command was not accepted.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum CommandError {
    /// The entity has moved on since the revision the command expected.
    ///
    /// This is the check that stops two agents from silently overwriting each other. The caller is
    /// expected to refetch, decide whether its intent still holds, and reissue.
    #[error("`{entity}` is at revision {actual}, but the command expected {expected}")]
    RevisionConflict {
        /// Which entity.
        entity: EntityRef,
        /// What the command asserted.
        expected: EntityRevision,
        /// What the backend holds.
        actual: EntityRevision,
    },

    /// The target does not exist.
    #[error("`{entity}` does not exist")]
    NotFound {
        /// What was addressed.
        entity: EntityRef,
    },

    /// The actor may not do this.
    #[error("not permitted{}: {reason}", capability.as_ref().map(|capability| format!(" ({capability})")).unwrap_or_default())]
    Unauthorised {
        /// The capability that was needed, when the refusal was capability-based.
        capability: Option<Capability>,
        /// Which rule refused, in one line.
        reason: String,
    },

    /// The command is not well formed, or contradicts itself.
    #[error("the command is not valid: {errors}")]
    Invalid {
        /// What is wrong with it.
        errors: ValidationErrors,
    },

    /// The idempotency key was used before, for a different command.
    ///
    /// Replaying the *same* logical command is expected and returns the original result. Reusing a
    /// key for a different one is a client bug, and silently accepting it would make the key useless.
    #[error("idempotency key `{key}` was already used by command `{original}`")]
    IdempotencyMismatch {
        /// The reused key.
        key: IdempotencyKey,
        /// The command that used it first.
        original: CommandId,
    },

    /// The command contradicts the current state in a way revisions do not describe.
    #[error("conflict: {reason}")]
    Conflict {
        /// What conflicts.
        reason: String,
    },

    /// This backend does not implement the command.
    ///
    /// Distinct from a refusal: the command is legitimate and another backend may accept it. Saying
    /// so plainly is better than an error that reads as "you may not", which sends a caller looking
    /// for a permission problem that does not exist.
    #[error("this backend does not implement `{command_type}`")]
    Unsupported {
        /// The command's versioned type name.
        command_type: String,
    },

    /// The backend could not answer.
    #[error("the backend is unavailable: {reason}")]
    Unavailable {
        /// Why.
        reason: String,
    },
}

impl CommandError {
    /// A stable machine-readable code.
    pub fn code(&self) -> &'static str {
        match self {
            Self::RevisionConflict { .. } => "revision_conflict",
            Self::NotFound { .. } => "not_found",
            Self::Unauthorised { .. } => "unauthorised",
            Self::Invalid { .. } => "invalid",
            Self::IdempotencyMismatch { .. } => "idempotency_mismatch",
            Self::Conflict { .. } => "conflict",
            Self::Unsupported { .. } => "unsupported",
            Self::Unavailable { .. } => "unavailable",
        }
    }

    /// `true` when reissuing the same command unchanged could succeed later.
    ///
    /// A revision conflict is *not* retryable in this sense: the command asserted a revision that no
    /// longer exists, so reissuing it unchanged asserts something false.
    pub fn is_retryable(&self) -> bool {
        matches!(self, Self::Unavailable { .. })
    }
}

/// Why a query could not be answered.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum QueryError {
    /// The entity does not exist, or the locator resolves to nothing.
    #[error("{what} does not exist")]
    NotFound {
        /// What was addressed.
        what: String,
    },

    /// The query itself is malformed.
    #[error("the query is not valid: {reason}")]
    Invalid {
        /// What is wrong with it.
        reason: String,
    },

    /// The caller may not read this.
    #[error("not permitted: {reason}")]
    Unauthorised {
        /// Which rule refused.
        reason: String,
    },

    /// The backend could not reach the requested consistency in time.
    #[error("the backend did not reach consistency token `{token}` in time")]
    ConsistencyTimeout {
        /// The token that was waited for.
        token: String,
    },

    /// The backend could not answer.
    #[error("the backend is unavailable: {reason}")]
    Unavailable {
        /// Why.
        reason: String,
    },
}

impl QueryError {
    /// A stable machine-readable code.
    pub fn code(&self) -> &'static str {
        match self {
            Self::NotFound { .. } => "not_found",
            Self::Invalid { .. } => "invalid",
            Self::Unauthorised { .. } => "unauthorised",
            Self::ConsistencyTimeout { .. } => "consistency_timeout",
            Self::Unavailable { .. } => "unavailable",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_revision_conflict_reports_both_revisions() {
        let error = CommandError::RevisionConflict {
            entity: EntityRef::new("01K2R8JD3ZJME72AJGQY67E5F8".parse().expect("id")),
            expected: EntityRevision::new(7).expect("revision"),
            actual: EntityRevision::new(8).expect("revision"),
        };
        assert_eq!(error.code(), "revision_conflict");
        let message = error.to_string();
        assert!(message.contains("revision 8"), "{message}");
        assert!(message.contains("expected 7"), "{message}");
        assert!(
            !error.is_retryable(),
            "reissuing a command that asserts a revision which no longer exists asserts something \
             false"
        );
    }

    #[test]
    fn reusing_an_idempotency_key_for_another_command_is_its_own_failure() {
        let error = CommandError::IdempotencyMismatch {
            key: "retry-42".parse().expect("key"),
            original: "cmd-1".parse().expect("id"),
        };
        assert_eq!(error.code(), "idempotency_mismatch");
        assert!(error.to_string().contains("cmd-1"));
    }

    #[test]
    fn only_unavailability_is_retryable_unchanged() {
        assert!(CommandError::Unavailable {
            reason: "the store is restarting".to_owned()
        }
        .is_retryable());
        assert!(!CommandError::Conflict {
            reason: "the design is already accepted".to_owned()
        }
        .is_retryable());
    }
}
