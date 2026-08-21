//! The hand-written half of the synthesised `gatepass` workspace.
//!
//! `generated/rust/gatepass/` holds the types, the typestate lifecycle, the component port, the
//! system and the HTTP surface — everything the specification determines. It holds no behaviour:
//! every command's decision and every projection is an obligation the plan names, and a workspace
//! built on the generated stubs compiles and answers each of them with a typed refusal.
//!
//! This crate is the other half. [`visit`] implements the five obligations
//! `generated/rust/gatepass/PLAN.md` owes, [`linker`] resolves exactly one implementation per
//! obligation without ever choosing between two (gap register D-2), and `src/bin/gatepass-server.rs`
//! hands the assembled system to the generated `serve` function.
//!
//! # The arrow points one way
//!
//! Hand-written code satisfies generated interfaces by import, never the other way round. Nothing
//! here is imported by anything under `generated/`, and deleting `generated/rust/gatepass/` and
//! regenerating it changes nothing in this crate unless the specification moved.
//!
//! # Its Go twin
//!
//! `examples/gatepass-go-realization/` is the same five obligations, written against the Go module
//! synthesised from the same specification. The two are deliberately *not* generated from each
//! other: the demonstration is that two independently written realizations of one specification,
//! behind two synthesised surfaces, answer the same requests the same way — and
//! `cargo xtask synth --check` starts both and holds them to it.

pub mod linker;
pub mod visit;
