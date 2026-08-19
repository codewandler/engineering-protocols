//! Black-box conformance suites for AEP backends.
//!
//! **Status: not yet implemented.** Planned suites, from
//! `docs/design/consolidated-design-v0.2.md` §78 and §104:
//!
//! identity · command execution · idempotency · optimistic concurrency · query · consistency ·
//! relations · history · immutability · audit · rejected-action audit · correlation · causation ·
//! provenance · events · type registry
//!
//! The suites test observable behaviour through the [`aep-contract`] traits, never a backend's
//! internals, and must not depend on sleeps: ordering is established with consistency tokens.
