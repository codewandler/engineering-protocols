// generated from gatepass v1
// model digest f2e0f8ff51c077fa1c713d8151544379bafac36a5a927e71c685042d53ab6e61
// contract digest e6e58e055d24f8f494dcff274f55e723d967f9d1f9aea16641bb8dacbb71171e
// compiler 0.1.0 · generator 0.1.0
// do not edit: regenerate with `protocol ess synthesize`

//! Semantic types synthesised from the `gatepass` specification, v1.
//!
//! Visitor passes for a building: who is expected, who is inside, and who has left. Three commands, two of which can be refused for two different reasons, one lifecycle, two projections and no binding — because the one transport this system has is the one its component's own words require, and nothing here reacts to an event.
//!
//! Generated, not written: the specification is the source of truth, and the door to changing
//! anything here is `protocol ess synthesize`. What is deliberately absent — behaviour, queries,
//! escalations — is listed with reasons in the `PLAN.md` beside this workspace, and every entry
//! there is owed through a typed seam in an `obligations` module here.

// `deny`, not the source workspace's lint set: this crate must hold on its own, and an undocumented
// public item here is an emitter defect worth failing the gate over.
#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod obligation;
pub mod primitives;
pub mod visit;
