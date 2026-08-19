//! Development-specific concepts: the **Agentic Development Protocol** (ADP).
//!
//! ADP is a *profile* of AEP, not a separate execution system. There is no development engine, no
//! development audit trail and no development command bus: `protocols/adp/1.yaml` extends `aep/1`
//! with development phases and observables, and everything in this crate travels the same
//! [`aep_contract::CommandService`] boundary, produces the same audit records and is authorised by
//! the same capability policy as a design approval. What ADP adds is vocabulary (§4.2):
//!
//! | it adds | it reuses from AEP |
//! |---|---|
//! | four entity types ([`body`]) | the entity envelope, identity, locators, revisions |
//! | four commands ([`command`]) | the command envelope, idempotency, optimistic concurrency |
//! | type discovery for those types ([`descriptor`]) | the registry contract itself |
//! | development phases and observables (`protocols/adp/1.yaml`) | workflow, evidence, capabilities |
//!
//! # What the vocabulary is for
//!
//! The development workflow (`workflows/development/default.yaml`) runs
//! `specify → decompose → establish_verifiers → implement → verify → adversarial_verify → review`,
//! and each of the added types is what one of those states produces or consumes:
//!
//! ```text
//! specify               adp.specification/v1        what must be true
//! specify               adp.acceptance-criteria/v1  when this story is finished
//! establish_verifiers   adp.test-plan/v1            what will decide it
//! implement             adp.change/v1               what was actually done
//! ```
//!
//! Every one of the development profiles (`development.fast`, `development.standard`,
//! `development.critical`) completes on `specification.satisfied` — not on "the tests pass", which
//! cannot distinguish a passing suite from a suite that covers the wrong thing. That condition is
//! only meaningful if a specification is an addressable entity with addressable requirements, and
//! if declaring it satisfied is a command that has to name its evidence. Both are here.
//!
//! # Capabilities
//!
//! ADP adds none. `adp/1` declares development phases and fact families and no new capability, so
//! every command in [`command`] requires one that already exists — `artifact.write`. A command
//! requiring a capability no protocol declares could never be granted by a profile, and so could
//! never be authorised at all.
//!
//! # Module map
//!
//! | module | contents |
//! |---|---|
//! | [`body`] | the four typed entity bodies and their `Node` conversions |
//! | [`command`] | the four development commands, their capabilities and their wire names |
//! | [`descriptor`] | type discovery, so a harness can ask what a test plan is |

pub mod body;
pub mod command;
pub mod descriptor;

pub use body::{
    AcceptanceCriteria, ChangeSet, Requirement, Specification, TestPlan, ACCEPTANCE_CRITERIA_TYPE,
    CHANGE_TYPE, SPECIFICATION_TYPE, TEST_PLAN_TYPE,
};
pub use command::{
    Command, CommandKind, CompleteStory, RecordTestPlan, SatisfySpecification, StartStory,
};
pub use descriptor::type_descriptors;
