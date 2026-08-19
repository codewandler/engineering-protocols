//! Operations-specific concepts: the **Agentic Operations Protocol** (AOP).
//!
//! AOP is a *profile* of AEP, not a separate execution system. There is no operations engine, no
//! operations audit trail and no operations command bus: `protocols/aop/1.yaml` extends `aep/1`
//! with operational phases and observables, and everything in this crate travels the same
//! [`aep_contract::CommandService`] boundary, produces the same audit records and is authorised by
//! the same capability policy as a design approval. What AOP adds is vocabulary (§4.3).
//!
//! # What the vocabulary is for
//!
//! Operational work has the same shape as development work — observe, assess, plan, change,
//! verify, record — under a clock and with production on the other end of the change. The two
//! profiles the repository ships put that shape into documents:
//!
//! | profile | workflow | what this crate contributes |
//! |---|---|---|
//! | `incident.standard` | `incident/standard` | [`Incident`], [`Runbook`], the three incident commands |
//! | `release.progressive` | `release/progressive` | [`Release`], the two release commands |
//!
//! # Why the statuses are not the workflow states
//!
//! A workflow state says what is *being done* (`mitigate`); a status says what has *been done*
//! (`mitigated`). The workflow is the plan the engine holds an execution to and lives in
//! `workflows/`; the status lives on the entity, survives the execution that set it, and is what a
//! reader finds six months later. [`IncidentStatus`] and [`ReleaseStatus`] are therefore ladders in
//! the past tense, and each refuses the shortcuts that would leave the record empty.
//!
//! # Module map
//!
//! | module | contents |
//! |---|---|
//! | [`body`] | the three entity bodies and the two status ladders |
//! | [`command`] | the five operational commands, their capabilities and their wire names |
//! | [`descriptor`] | type discovery for a harness that has never seen an incident |

pub mod body;
pub mod command;
pub mod descriptor;

pub use body::{Incident, IncidentStatus, Release, ReleaseStatus, Runbook};
pub use command::{
    AcknowledgeIncident, Command, CommandKind, MitigateIncident, PromoteRelease, ResolveIncident,
    RollbackRelease,
};
pub use descriptor::type_descriptors;
