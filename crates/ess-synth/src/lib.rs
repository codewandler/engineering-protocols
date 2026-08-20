//! Structural synthesis: from a resolved specification to a plan, and from the plan to code.
//!
//! # The hinge, and the seam
//!
//! Two stages, deliberately, and the boundary between them is the design's whole argument
//! (§2, §5):
//!
//! 1. **Planning** ([`plan`]) — language-neutral. Every semantic capability of the specification
//!    gets exactly one disposition: generated, an obligation with its contract, or a refusal with
//!    its reason. Zero guessed business logic; the plan is where "what was not generated" becomes
//!    a document instead of an absence.
//! 2. **Emission** ([`rust`]) — per target. Rust is the first target, not the only intended one,
//!    so everything Rust-shaped lives behind that module's one entry point, and a later target is
//!    a sibling module consuming the same plan — not a framework built in advance of it.
//!
//! [`synthesize`] runs both and returns the whole output: the plan in its two canonical renderings
//! beside the generated workspace, because a workspace without its plan is code that cannot say
//! what it deliberately is not.
//!
//! # Where this sits among the crates
//!
//! `ess-gen` projects a specification into documents; `ess-conformance` derives the suite that
//! judges an implementation; this crate emits the part of an implementation that was never anyone's
//! to write. It reads only [`EssIr`] — the one input every projection reads, unforgeable by
//! construction — and reuses `ess-gen`'s [`Artifact`] and provenance conventions rather than
//! growing parallel ones.

pub mod plan;
pub mod rust;

use std::collections::BTreeMap;

use ess_compiler::ir::EssIr;
use ess_gen::Artifact;

pub use plan::{
    Capability, CapabilityKind, DispositionCounts, ImplementationObligation, ObligationReason,
    PlannedCapability, RefusalReason, RefusalStage, SynthesisDisposition, SynthesisPlan,
    SynthesisRefusal, REGENERATE,
};

/// The plan's Markdown rendering, at the root of the generated workspace.
pub const PLAN_MARKDOWN: &str = "PLAN.md";

/// The plan's canonical JSON, beside it, for consumers that parse rather than read.
pub const PLAN_JSON: &str = "plan.json";

/// Everything one synthesis produced: the plan, and every file, keyed by path.
pub struct Synthesis {
    /// The plan the files were emitted from.
    pub plan: SynthesisPlan,
    /// Every artifact — the plan's own renderings included — keyed by path relative to the
    /// generated workspace root.
    pub artifacts: BTreeMap<String, Artifact>,
}

/// Plans a specification and emits the Rust workspace the plan determines.
///
/// Deterministic: same IR, byte-identical artifacts, asserted by tests that run it twice. The plan
/// travels *inside* the output tree — `PLAN.md` for a person, `plan.json` for a tool — so the list
/// of what was deliberately not generated is committed and drift-checked with the code it is about.
pub fn synthesize(ir: &EssIr) -> Synthesis {
    let plan = SynthesisPlan::of(ir);
    let mut artifacts = BTreeMap::new();
    insert(
        &mut artifacts,
        Artifact::new(PLAN_MARKDOWN, plan.to_markdown()),
    );
    insert(
        &mut artifacts,
        Artifact::new(PLAN_JSON, plan.to_canonical_json()),
    );
    for artifact in rust::workspace(ir, &plan) {
        insert(&mut artifacts, artifact);
    }
    Synthesis { plan, artifacts }
}

/// Keyed insertion that refuses a duplicate path: two artifacts claiming one file means the second
/// silently overwrites the first and the tree looks complete while missing one — the same rule
/// `ess-gen` enforces, held here as an internal invariant because every path is produced by this
/// crate itself.
fn insert(artifacts: &mut BTreeMap<String, Artifact>, artifact: Artifact) {
    let path = artifact.path.clone();
    assert!(
        artifacts.insert(path.clone(), artifact).is_none(),
        "two artifacts claimed `{path}`; that is a defect in ess-synth"
    );
}
