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
//! 2. **Emission** — per target. [`rust`] was the first, [`go`] is the second, and both consume a
//!    finished plan through the same public surface. Everything language-shaped lives behind one
//!    of those two module boundaries; nothing about either reaches back into the planner.
//!
//! [`synthesize`] runs both stages for Rust and [`synthesize_for`] for a chosen [`Target`],
//! returning the whole output: the plan in its two canonical renderings beside the generated code,
//! because code without its plan is code that cannot say what it deliberately is not.
//!
//! # The seam, proved
//!
//! The plan did not change to admit the second target. Its two renderings are **byte-identical in
//! both trees** — the same `PLAN.md`, the same `plan.json` — and a test holds them to it. What one
//! target cannot carry is that target's to report, in a [`TargetReport`] beside the plan rather
//! than folded into it: a capability it cannot represent at all is a refusal marked
//! [`RefusalStage::Target`], and one it emits with a weaker guarantee is a [`TargetWeakening`]. A
//! silent downgrade is neither, and there is no disposition for it.
//!
//! # Where this sits among the crates
//!
//! `ess-gen` projects a specification into documents; `ess-conformance` derives the suite that
//! judges an implementation; this crate emits the part of an implementation that was never anyone's
//! to write. It reads only [`EssIr`] — the one input every projection reads, unforgeable by
//! construction — and reuses `ess-gen`'s [`Artifact`] and provenance conventions rather than
//! growing parallel ones.

pub mod go;
pub mod plan;
pub mod rust;

use std::collections::BTreeMap;
use std::fmt::Write as _;

use ess_compiler::ir::EssIr;
use ess_gen::{Artifact, Provenance};

pub use plan::{
    Capability, CapabilityKind, DispositionCounts, ImplementationObligation, ObligationReason,
    PlannedCapability, RefusalReason, RefusalStage, SynthesisDisposition, SynthesisPlan,
    SynthesisRefusal, REGENERATE,
};

/// The plan's Markdown rendering, at the root of the generated tree.
pub const PLAN_MARKDOWN: &str = "PLAN.md";

/// The plan's canonical JSON, beside it, for consumers that parse rather than read.
pub const PLAN_JSON: &str = "plan.json";

/// What the emission target could not carry across the plan, for a person.
///
/// Beside the plan and never inside it: a weakening or a target refusal is a fact about one
/// language, and folding it into `PLAN.md` would make the plan a different document per target —
/// which is the claim the seam exists to refute.
pub const TARGET_MARKDOWN: &str = "TARGET.md";

/// The same, for a tool.
pub const TARGET_JSON: &str = "target.json";

/// Which language a synthesis emits.
///
/// Two, and the enum is not an abstraction over "languages in general": each variant names a
/// sibling module that consumes the plan, and adding a third means writing a third emitter, not
/// registering one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// A standalone Cargo workspace — the first target.
    Rust,
    /// A standalone Go module — the second, chosen because it has no sum type.
    Go,
}

impl Target {
    /// The name a report carries.
    pub fn name(self) -> &'static str {
        match self {
            Self::Rust => "rust",
            Self::Go => go::TARGET,
        }
    }
}

/// Everything one synthesis produced: the plan, every file, and what the target could not carry.
pub struct Synthesis {
    /// The plan the files were emitted from.
    pub plan: SynthesisPlan,
    /// Every artifact — the plan's own renderings included — keyed by path relative to the
    /// generated root.
    pub artifacts: BTreeMap<String, Artifact>,
    /// What this target weakened or refused, or `None` where it carried the plan whole.
    pub target: Option<TargetReport>,
}

/// What one emission target could not carry across from the plan.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct TargetReport {
    /// The specification this report is about, and the build that emitted for it.
    ///
    /// Carried as data because JSON has no comments, exactly as the plan's is: an artifact that
    /// cannot say which specification produced it is an artifact nobody can audit, and W7.1's
    /// contract digest travels the same way.
    pub provenance: Provenance,
    /// The target that wrote it.
    pub target: &'static str,
    /// What it emits with a weaker guarantee than the plan's disposition implies.
    pub weakenings: Vec<TargetWeakening>,
    /// What it cannot represent at all, and therefore did not emit.
    pub refusals: Vec<TargetRefusal>,
}

/// One guarantee this target cannot hold, and what it provides instead.
///
/// Stated once per **rule** rather than once per capability, because "Go has no exhaustiveness
/// check" is one fact about a language and repeating it against forty capabilities buries the rows
/// a reader has to act on. Each names the capability kinds it touches, so the parity question is
/// still answerable from the table.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct TargetWeakening {
    /// The property the emitted code is expected to carry.
    pub guarantee: String,
    /// What this target provides instead, and why it cannot do better.
    pub instead: String,
    /// The capability kinds it touches.
    pub affects: Vec<CapabilityKind>,
}

/// One capability this target cannot represent, and why.
///
/// The counterpart of a planning refusal, and the reason [`RefusalStage::Target`] exists: a reader
/// can tell "no target can synthesise this" from "this language cannot", and switching targets
/// dissolves only the second.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct TargetRefusal {
    /// What was refused.
    pub capability: Capability,
    /// Why, naming the construct's own facts and the path that reaches the cause.
    pub detail: String,
}

impl TargetReport {
    /// `true` when the target carried the plan whole.
    pub fn is_empty(&self) -> bool {
        self.weakenings.is_empty() && self.refusals.is_empty()
    }

    /// The command that rewrites this document.
    pub fn regenerate(&self) -> String {
        format!("{REGENERATE} --target {}", self.target)
    }

    /// The report as canonical JSON: the same convention as the plan's, for the same reason — the
    /// committed copy is compared byte for byte.
    pub fn to_canonical_json(&self) -> String {
        let mut json = serde_json::to_string_pretty(self)
            .unwrap_or_else(|error| panic!("the target report serialises: {error}"));
        json.push('\n');
        json
    }

    /// The report as Markdown, for the person deciding whether this target's output is the one
    /// they want.
    pub fn to_markdown(&self) -> String {
        let regenerate = self.regenerate();
        let provenance = &self.provenance;
        let mut out = provenance.html_comment_for(&regenerate);
        let _ = write!(
            out,
            "# Target notes — {}\n\nFor {} {}. The `{PLAN_MARKDOWN}` beside this file is \
             language-neutral and **byte-identical in every target's tree**; this document is what \
             *this* target could not carry across it. Regenerate with `{regenerate}`.\n\n{} \
             weakening(s), {} target refusal(s). A weakening is emitted code that holds less than \
             the first target's; a target refusal is a capability the plan marks generated and \
             this language cannot represent — a fact about the language, never about the \
             specification.\n",
            self.target,
            provenance.system,
            provenance.specification_version,
            self.weakenings.len(),
            self.refusals.len(),
        );

        out.push_str(
            "\n## Weakened — emitted, with less than the first target holds\n\n| the guarantee | \
             what this target provides | capabilities affected |\n| --- | --- | --- |\n",
        );
        for weakening in &self.weakenings {
            let _ = writeln!(
                out,
                "| {} | {} | {} |",
                weakening.guarantee,
                weakening.instead,
                weakening
                    .affects
                    .iter()
                    .map(|kind| kind.describes().to_owned())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }

        out.push_str(
            "\n## Refused by this target — planned, not emitted\n\n| capability | source | why |\n\
             | --- | --- | --- |\n",
        );
        for refusal in &self.refusals {
            let _ = writeln!(
                out,
                "| {} | `{}` | {} |",
                refusal.capability.kind.describes(),
                refusal.capability.source,
                refusal.detail
            );
        }
        out
    }
}

/// Plans a specification and emits the Rust workspace the plan determines.
///
/// Deterministic: same IR, byte-identical artifacts, asserted by tests that run it twice. The plan
/// travels *inside* the output tree — `PLAN.md` for a person, `plan.json` for a tool — so the list
/// of what was deliberately not generated is committed and drift-checked with the code it is about.
pub fn synthesize(ir: &EssIr) -> Synthesis {
    synthesize_for(ir, Target::Rust)
}

/// Plans a specification and emits what the chosen target's emitter makes of the plan.
///
/// One plan, computed the same way whatever the target, and two renderings of it in every tree.
/// What differs between trees is the code and — for a target that could not carry everything — the
/// [`TargetReport`] beside it.
pub fn synthesize_for(ir: &EssIr, target: Target) -> Synthesis {
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
    let report = match target {
        Target::Rust => {
            for artifact in rust::workspace(ir, &plan) {
                insert(&mut artifacts, artifact);
            }
            None
        }
        Target::Go => {
            let emission = go::workspace(ir, &plan);
            for artifact in emission.artifacts {
                insert(&mut artifacts, artifact);
            }
            Some(emission.report)
        }
    };
    if let Some(report) = &report {
        insert(
            &mut artifacts,
            Artifact::new(TARGET_MARKDOWN, report.to_markdown()),
        );
        insert(
            &mut artifacts,
            Artifact::new(TARGET_JSON, report.to_canonical_json()),
        );
    }
    Synthesis {
        plan,
        artifacts,
        target: report,
    }
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
