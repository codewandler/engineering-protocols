//! Which construct rests on which — re-exported from the crate that owns the walk now.
//!
//! Wave 5 built the semantic dependency graph here, as the machinery under `ess impact`'s closure.
//! Wave 7 moved it into [`ess_compiler::graph`], one crate down, because generated artifacts now
//! record a digest of the model slice they derive from and `ess-gen` computes that slice by the
//! **same walk** this crate narrows by — two graphs would be two answers to "what rests on what",
//! and the drift between them would surface as an artifact claiming to stand still past a change
//! its own impact report names. The re-export keeps wave 5's paths alive: everything this module
//! ever exported is still spelled `ess_diff::graph::*`.

pub use ess_compiler::graph::{
    DependencyEdge, DependencyRelation, ImpactClass, Reach, SemanticDependencyGraph,
};
