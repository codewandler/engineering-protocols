//! What the billing specification left to a person, written by a person.
//!
//! The synthesised workspace under `generated/rust/billing/` carries a `PLAN.md` with eight
//! obligations — five command behaviours, two view projections, one binding escalation — each a
//! typed seam whose stub refuses with an [`UnmetObligation`](billing_types::obligation::UnmetObligation).
//! This crate is the other half of that bargain: one honest implementation per obligation, written
//! by reading `examples/billing/`, plus the [`linker`] that assembles generated components and
//! hand implementations into a runnable system.
//!
//! # The ownership boundary is absolute
//!
//! Nothing here is generated, and nothing here is ever written into `generated/`. Hand-written
//! code satisfies generated interfaces by import — that is the synthesis design's §17 boundary,
//! kept absolute so the generated tree stays fully disposable. If a change to the specification
//! moves a capability from obligation to generated, this crate loses code; it never gains any of
//! the generator's.
//!
//! # State is shared, obligations are separate
//!
//! The plan owes obligations one by one, and the linker (gap register D-2) resolves them one by
//! one — but four command behaviours and two projections speak about the *same* invoices. So the
//! store is a [`SharedInvoices`](invoice::SharedInvoices) handle every invoice obligation holds a
//! clone of, which is also what lets the deliberately [`corrupted`] variant replace exactly one
//! obligation while the other seven keep their state.
//!
//! # One deliberate lie lives here too
//!
//! [`corrupted::AcceptsAnyAmount`] is the falsifiability half of wave 6's acceptance criterion:
//! the same committed suite that passes the honest linkage must fail the corrupted one, at the one
//! scenario that exists to catch it. It follows `ess-conformance`'s `faulty` pattern — a named
//! fault, the scenario that catches it, a blast-radius claim a test holds.

pub mod corrupted;
pub mod email;
pub mod escalation;
pub mod invoice;
pub mod linker;
