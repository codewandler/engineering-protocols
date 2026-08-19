//! Engine errors.
//!
//! Resolution failures are [`ValidationErrors`] — a document set that cannot be executed. Everything
//! else here is a runtime refusal, and each variant exists so a harness can branch on *why* rather
//! than parse a message.

use aep_domain::error::ValidationErrors;
use aep_domain::evidence::EvidenceKind;
use aep_domain::ids::StateId;

/// Why the engine refused.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ProtocolError {
    /// The documents cannot be resolved into an executable plan.
    #[error("the task cannot be resolved: {0}")]
    Resolution(#[from] ValidationErrors),

    /// The execution refers to a state its workflow does not declare.
    ///
    /// Only reachable by restoring a snapshot taken against a different workflow version.
    #[error("state `{state}` is not part of workflow `{workflow}`")]
    UnknownState {
        /// The state that is missing.
        state: StateId,
        /// The workflow it was expected in.
        workflow: String,
    },

    /// Evidence was submitted that the protocol does not declare.
    ///
    /// Accepting it would let a document set grow a private vocabulary that no other harness can
    /// interpret, so it is refused rather than stored.
    #[error(
        "the protocol does not declare evidence of kind `{kind}`; declared kinds are {declared}"
    )]
    EvidenceRejected {
        /// The kind that was refused.
        kind: EvidenceKind,
        /// What the protocol does declare.
        declared: String,
    },

    /// No transition out of the current state is permitted.
    #[error("no transition out of `{state}` is permitted: {}", reasons.join("; "))]
    NoTransitionPermitted {
        /// Where the execution is stuck.
        state: StateId,
        /// One line per unmet requirement.
        reasons: Vec<String>,
    },

    /// The execution is already in a terminal state.
    #[error("the execution is already complete in `{state}`")]
    AlreadyComplete {
        /// The terminal state.
        state: StateId,
    },
}

impl ProtocolError {
    /// A stable machine-readable code, for a harness that reports rather than branches.
    pub fn code(&self) -> &'static str {
        match self {
            Self::Resolution(_) => "resolution_failed",
            Self::UnknownState { .. } => "unknown_state",
            Self::EvidenceRejected { .. } => "evidence_rejected",
            Self::NoTransitionPermitted { .. } => "no_transition_permitted",
            Self::AlreadyComplete { .. } => "already_complete",
        }
    }
}
