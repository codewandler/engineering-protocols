//! Protocol-neutral domain model for the **Agentic Engineering Protocol** (AEP).
//!
//! This crate is the source of truth for AEP: every wire format (JSON Schema, YAML documents) is
//! generated from the types defined here, never maintained separately.
//!
//! # Two-stage document model
//!
//! ```text
//! YAML ──serde──> RawWorkflow ──TryFrom──> Workflow ──> execution
//!        structure           semantics
//! ```
//!
//! Validated types deliberately do **not** implement [`Deserialize`](serde::Deserialize): the
//! only way to obtain one is to validate a raw document, so an invalid state cannot enter the
//! runtime through a parser. See [`raw`] for the wire surface.
//!
//! # Module map
//!
//! | module | contents |
//! |---|---|
//! | [`ids`] | identifier newtypes with charset rules |
//! | [`version`] | major versions and versioned references (`aep/1`, `principle:test-driven/1`) |
//! | [`node`] | format-neutral dynamic value for untyped document fragments |
//! | [`facts`] | fact paths, values, stores, patterns and ordered scales |
//! | [`predicate`] | the predicate language and its three-valued evaluation |
//! | [`requirement`] | evidence, artifact, review, approval and conditional requirements |
//! | [`capability`] | capabilities, environments and capability policy |
//! | [`action`] | actions an agent may request, and the capability each requires |
//! | [`evidence`] | evidence kinds, envelopes, provenance and fact projection |
//! | [`verification`] | verifier classes, verification results and counterexamples |
//! | [`artifact`] | the engineering artifact graph, its kinds, statuses and lifecycles |
//! | [`review`] | review dispositions, findings and approval freshness |
//! | [`principle`] | principles, obligations, timing and failure policy |
//! | [`workflow`] | workflow state machines |
//! | [`task`] | tasks, objectives, constraints and principle overrides |
//! | [`protocol`] | protocol declarations: the vocabulary a profile may use |
//! | [`profile`] | profiles: protocol, workflow, principles and completion |
//! | [`plan`] | resolved execution plans |
//! | [`event`] | the audit event vocabulary |
//! | [`error`] | parse and validation errors |

pub mod action;
pub mod artifact;
pub mod capability;
pub mod error;
pub mod event;
pub mod evidence;
pub mod facts;
pub mod ids;
pub mod node;
pub mod plan;
pub mod predicate;
pub mod principle;
pub mod profile;
pub mod protocol;
pub mod raw;
pub mod requirement;
pub mod review;
pub mod task;
pub mod time;
pub mod verification;
pub mod version;
pub mod workflow;

pub use action::{Action, ActionRequest};
pub use artifact::{
    Artifact, ArtifactGraph, ArtifactId, ArtifactKind, ArtifactLifecycle, ArtifactLocation,
    ArtifactRef, ArtifactRelation, ArtifactStatus, ArtifactVersion, LifecycleRegistry,
    RelationKind, Revision,
};
pub use capability::{Capability, CapabilityDecision, CapabilityPolicy, Environment, PolicySource};
pub use error::{ParseError, ValidationCode, ValidationError, ValidationErrors};
pub use event::{EventEnvelope, ProtocolEvent};
pub use evidence::{
    Evidence, EvidenceEnvelope, EvidenceKind, EvidenceRecord, Producer, Provenance, TestSuite,
};
pub use facts::{FactPath, FactPattern, FactSource, FactStore, FactValue, Number, Scales};
pub use ids::{
    ApprovalId, ClaimId, EvidenceId, ExecutionId, ObligationId, PhaseId, PrincipleId, ProfileId,
    ProtocolId, ProviderId, RepositoryRef, ServiceId, StateId, SubjectRef, TaskId, ToolRef,
    WorkflowId,
};
pub use node::Node;
pub use plan::{CapabilityGrant, ExecutionPlan, ResolvedObligation};
pub use predicate::{CompareOp, LeafOutcome, Operand, Predicate, PredicateOutcome, Truth};
pub use principle::{
    FailurePolicy, Obligation, ObligationTiming, PhaseRef, Principle, PrincipleOverrides,
    VerificationRequirement,
};
pub use profile::Profile;
pub use protocol::Protocol;
pub use requirement::{
    ApprovalRequirement, ArtifactRequirement, ConditionalRequirement, EvidenceRequirement,
    RequirementContext, RequirementOutcome, RequirementReport, RequirementSet, ReviewRequirement,
};
pub use review::{Finding, ReviewDisposition, ReviewResult, Reviewer, Severity};
pub use task::{Constraints, Objective, Task, TaskKind};
pub use time::Timestamp;
pub use verification::{Counterexample, VerificationResult, VerificationStatus, Verifier};
pub use version::{MajorVersion, PrincipleRef, ProfileVersionedRef, ProtocolRef, WorkflowRef};
pub use workflow::{State, StateKind, Transition, Workflow};
