//! Black-box conformance suites for AEP backends.
//!
//! **Status: not yet implemented — wave 3.** The plan is `docs/plan/wave-3-conformance.md`; the
//! reference scenario it generalises already runs as a test in `aep-backend-memory`.
//!
//! Planned suites, from `docs/design/consolidated-design-v0.2.md` §78 and §104:
//!
//! identity · command execution · idempotency · optimistic concurrency · query · consistency ·
//! relations · history · immutability · audit · rejected-action audit · correlation · causation ·
//! provenance · events · type registry
//!
//! The suites test observable behaviour through the `aep-contract` traits, never a backend's
//! internals, and must not depend on sleeps: ordering is established with consistency tokens.
//!
//! They ship with a deliberately broken backend that injects one fault at a time, so the suite's own
//! tests can prove each suite catches the thing it exists to catch. A conformance suite that passes
//! everything is not a suite.
