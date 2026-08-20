//! Analysis over the infrastructure IR: the graph, the diagnosis, the properties.
//!
//! The third infrastructure crate, and the first that *interprets*. `infra-domain` decides what
//! a scanned bundle may claim, `infra-compiler` normalizes a valid claim into a
//! content-addressed IR, and this crate answers the two questions the IR was built for: **what
//! depends on what** ([`graph`]) and **what is wrong** ([`diagnose`](mod@diagnose)) — plus the
//! small third one IW3 will consume, **what invariant-like facts each workload exhibits**
//! ([`properties`](mod@properties)).
//!
//! | module | contents |
//! |---|---|
//! | [`graph`] | the typed dependency graph: closed edge vocabulary, sites as evidence, derived pod ownership, Mermaid and JSON renderings |
//! | [`diagnose`](mod@diagnose) | fourteen diagnosis rules, each with a stable `INFRA-DIAG-*` code and a registered severity |
//! | [`code`] | the code registry and the severity taxonomy |
//! | [`properties`](mod@properties) | per-workload replicas, parsed images and resource envelopes |
//!
//! # Diagnosis refuses nothing
//!
//! The invariant carried over from IW1: observed infrastructure is allowed to be wrong. A
//! cluster full of findings diagnoses *successfully*; the findings are the product, not a
//! failure of the run. Refusal stays where it belongs — on bundles that lie about their shape
//! (`infra-domain`) and on persisted documents that lie about their content
//! (`infra-compiler`'s read-back).
//!
//! # Determinism
//!
//! Same IR in, byte-identical graph document, Mermaid text and finding list out (invariant 9).
//! `tests/determinism.rs` renders twice and compares bytes, and its source scan keeps unordered
//! maps and clocks out of this crate.

pub mod code;
pub mod diagnose;
pub mod graph;
pub mod properties;

pub use code::{DiagCode, Severity};
pub use diagnose::{diagnose, diagnose_with, Diagnosis, Finding, HIGH_RESTART_THRESHOLD};
pub use graph::{
    EdgeRelation, GraphDocument, GraphEdge, GraphNode, InfraGraph, NodeKind, UnderivedOwner,
    UnderivedReason, GRAPH_FORMAT,
};
pub use properties::{
    parse_image, properties, ContainerProperties, ImageReference, WorkloadProperties,
};
