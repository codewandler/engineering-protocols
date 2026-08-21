//! Desired state for an observed cluster: what somebody declared, what the snapshot decided, and
//! what moved between two snapshots.
//!
//! The fourth infrastructure crate, and the first that carries an *intention*. `infra-domain`
//! decides what a scanned bundle may claim, `infra-compiler` normalizes a valid claim into a
//! content-addressed IR, `infra-analyze` interprets one — and this crate is where a human's
//! sentence about how the cluster ought to be meets the sentence the cluster tells about itself.
//!
//! | module | contents |
//! |---|---|
//! | [`spec`] | the desired-state model: twelve expectation kinds, three scopes |
//! | [`raw`] | the permissive half, and the `TryFrom` that is the only way into an [`InfraSpec`] |
//! | [`facts`] | the workload fact sheet the predicate escape hatch reads, and why each withheld fact is withheld |
//! | [`simulate`](mod@simulate) | evaluation: three-valued verdicts with typed gaps and named reasons |
//! | [`drift`](mod@drift) | what moved between two compiled snapshots of one cluster |
//! | [`render`] | the text renderings |
//!
//! ```text
//! expected.yaml                      observation.json
//!      |                                    |
//!  validate                          validate + compile
//!      |                                    |
//!  InfraSpec  ------ simulate ------->  InfraIr ------ drift ------> InfraDrift
//!                        |                                (against another InfraIr)
//!                   Simulation
//! ```
//!
//! # Two verbs, and why not one
//!
//! `simulate` and `diff` answer two different questions and are two commands, not one with a
//! flag:
//!
//! | question | verb | compares |
//! |---|---|---|
//! | does the cluster do what I declared, and what would have to change | `protocol infra simulate --spec … --path …` | *unlike* things: a specification against a snapshot |
//! | what moved between these two scans | `protocol infra diff --from … --to …` | *like* things: a snapshot against a snapshot |
//!
//! The desired↔observed comparison is **inside `simulate`**, not a third verb: every `False`
//! already carries its [`Gap`], which is the have-versus-want a separate
//! `diff --spec` would have printed. Two producers of one answer is how a report and a gate come
//! to disagree, and `ess-diff`'s own split is the precedent in the other direction —
//! `diff` and `impact` are two verbs because they are two computations over one delta, where this
//! would be two renderings of one.
//!
//! The comparison is also **asymmetric on purpose**. A specification is not a snapshot: it says
//! nothing about most of the cluster, and a `False` means "this declared expectation is
//! contradicted", never "the cluster has something the specification does not mention". Nothing
//! here reports an object as *extra*, because a specification of eleven expectations is not a
//! description of a cluster and treating it as one would report a thousand phantom removals.
//!
//! # What is not here
//!
//! * **Apply.** Nothing in this workspace reaches a cluster, and a gap report is not a plan to
//!   change one. `docs/VISION.md` refuses the credential; IW3 refuses the verb.
//! * **Manifest projection.** Generating manifests *from* a desired-state model is IW4, and it
//!   needs a model that describes a whole workload rather than expectations about one.
//! * **Wall-clock anything.** Review finding I7: no expectation compares a timestamp, and there
//!   is no duration vocabulary to write one in. See [`spec`].
//!
//! # Determinism
//!
//! Same specification and snapshot in, byte-identical simulation out; same pair of snapshots in,
//! byte-identical drift out (invariant 9). `tests/determinism.rs` runs each twice and compares
//! bytes, and its source scan keeps unordered maps and clocks out of this crate.

pub mod drift;
pub mod facts;
pub mod raw;
pub mod render;
pub mod simulate;
pub mod spec;

pub use drift::{
    drift, DriftRefusal, DriftSideRef, InfraChange, InfraDrift, MemberKind, ServiceField,
    WorkloadField, DRIFT_FORMAT,
};
pub use facts::{workload_facts, WorkloadFacts, WORKLOAD_FACTS};
pub use raw::{read_spec, RawInfraSpec};
pub use render::{drift_counts, drift_to_text, simulation_to_text};
pub use simulate::{
    simulate, ExpectationReport, Gap, Outcome, Simulation, SnapshotRef, SubjectOutcome, Summary,
    UnknownReason, SIMULATION_FORMAT,
};
pub use spec::{Expectation, ExpectationKind, InfraSpec, Scope, SubjectClass, SPEC_FORMAT};
