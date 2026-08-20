//! Resolution, the normalized IR, and diagnostics.
//!
//! [`ess_domain`] answers "is this document well formed, and locally consistent". This crate answers
//! the question a generator actually has: **does every reference in it resolve, and to what**.
//!
//! # An unresolved reference is unrepresentable
//!
//! A [`Specification`](ess_domain::spec::Specification) holds names. `CreateInvoice` emits
//! `billing.invoice.InvoiceCreated`, and that is a [`QualifiedName`](ess_domain::name::QualifiedName)
//! which *probably* names a declared event. Anything downstream either re-checks it or trusts that
//! someone else did, and both of those are how a generator emits code for a type that does not
//! exist.
//!
//! [`ir::EssIr`] holds resolved handles instead. Getting one requires [`resolve::compile`], and
//! compiling is what runs the checks — so a projection reading the IR cannot ask a question the IR
//! cannot answer. That is the same two-stage discipline as `Raw*` → validated, one level up:
//!
//! | representation | guarantee |
//! |---|---|
//! | `Raw*` | what a document may say |
//! | `Specification` | what it means, each declaration locally consistent |
//! | [`ir::EssIr`] | what it means, every reference resolved |
//!
//! # Diagnostics are repair instructions
//!
//! A coding agent consumes a diagnostic and edits a file. That works when the diagnostic carries the
//! two types and the two document paths as *fields* (design §29); it does not work when the agent has
//! to parse them back out of a sentence. [`diagnostic::Diagnostic`] is the structured form, and its
//! rendering is a projection of it rather than the other way round.
//!
//! # Determinism
//!
//! No clock, no RNG, `BTreeMap`/`BTreeSet` only, and canonical serialisation with a trailing
//! newline. Asserting determinism is what review F8 called out as insufficient; a test that compiles
//! the same source twice and compares bytes is what makes it true.

pub mod diagnostic;
pub mod ir;
pub mod resolve;
pub mod source;

pub use diagnostic::{Diagnostic, Diagnostics, Severity};
pub use ir::EssIr;
pub use resolve::compile;
