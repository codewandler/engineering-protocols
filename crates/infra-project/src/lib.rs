//! The manifest projection: a simulation's gaps, turned into a diff somebody can review.
//!
//! The fifth infrastructure crate, and the last one on this side of the boundary. `infra-domain`
//! decides what a scanned bundle may claim, `infra-compiler` normalizes it, `infra-analyze`
//! interprets it, `infra-spec` measures it against what somebody declared — and this crate
//! answers the question that report leaves open: **so what would I change?**
//!
//! ```text
//! expected.yaml + observation.json
//!            |
//!         simulate            (infra-spec)
//!            |
//!         Simulation  --- every False carries a typed Gap
//!            |
//!         project             (this crate)
//!            |
//!    +-------+--------+-----------------+
//!    |                |                 |
//! patches/       objects/         OBLIGATIONS.md
//! (per object)   (new manifests)  (what nobody can patch)
//! ```
//!
//! | module | contents |
//! |---|---|
//! | [`patch`] | what gets written: merge and strategic patches, generated objects, the file names |
//! | [`project`](mod@project) | the decision: generated, obligation, refused — and the fixed point that makes it hold |
//! | [`render`] | `SUMMARY.md`, `OBLIGATIONS.md`, and the terminal form |
//!
//! # This is still not apply
//!
//! Nothing here reaches a cluster, opens a socket, or holds a credential — the boundary
//! `docs/VISION.md` draws and every infrastructure wave has kept. What this crate adds is not an
//! *acting* surface but a **reviewable** one: v1's entire output is a directory of files a person
//! reads, edits, commits and applies with their own hands and their own credentials. The verb
//! that applies them lives in the scanner's repository, on the other side of the line, and is not
//! scheduled here.
//!
//! That is also why the emitted files are patches against observed objects rather than whole
//! rewritten manifests. A whole manifest generated from a snapshot would carry every field the
//! observation model happens to keep and silently drop every field it does not — which is a
//! rewrite of somebody's deployment disguised as a fix. A patch names only what changes, and a
//! reviewer can see all of it.
//!
//! # Nothing here invents a value
//!
//! Every number, string and map that reaches a patch comes from one of two places: the gap
//! itself, or a [`Remedy`](infra_spec::Remedy) a human wrote in the specification. There is no
//! default table, no configuration file and no heuristic — a gap whose value neither of those two
//! decides becomes an [`Obligation`](project::Disposition::Obligation) naming the decision, and
//! `tests/projection.rs` holds every kind to that line.
//!
//! # Secrets
//!
//! A patch cannot contain a secret value, because nothing upstream of it holds one: the
//! observation model refuses an unsanitized secret (`INFRA-SECRET-001`) and carries only
//! `{sha256, length}` per key. `tests/secrets.rs` asserts it anyway, by scanning every emitted
//! byte for the fixture's own key names and digests — a property this cheap to check is one worth
//! checking rather than arguing about.
//!
//! # Determinism
//!
//! Same specification and snapshot in, byte-identical tree out (invariant 9).
//! `tests/determinism.rs` projects twice and compares every file, and its source scan keeps
//! unordered maps and clocks out of this crate.

pub mod patch;
pub mod project;
pub mod render;

pub use patch::{NewObject, ObjectPatch, ObjectRef, PatchType};
pub use project::{
    project, Artifact, Disposition, GapOrigin, GeneratedChange, ObligationReason, Projection,
    ProjectionDocument, ProjectionEntry, ProjectionObligation, ProjectionProvenance,
    ProjectionRefusal, ProjectionSummary, RefusalReason, PROJECTION_FORMAT,
};
pub use render::{obligations_markdown, projection_to_text, summary_markdown};
