//! The storage-independent interaction contract.
//!
//! **Status: not yet implemented.** Planned public surface, from
//! `docs/design/consolidated-design-v0.2.md` §34–47:
//!
//! | module | responsibility |
//! |---|---|
//! | `command` | `CommandService`, `CommandEnvelope`, `CommandContext`, `CommandResult` |
//! | `query` | `QueryService`, entity and relation queries, history, audit, paging |
//! | `consistency` | `ConsistencyToken` and `QueryConsistency`, giving read-your-writes across backends |
//! | `idempotency` | idempotency keys and replay semantics |
//! | `concurrency` | `expected_revision` and the machine-readable revision-conflict error |
//! | `registry` | `TypeDescriptor` and type discovery, so a harness need not hard-code domain types |
//!
//! The contract defines observable behaviour only: a backend may be a database, Git, files, a
//! remote API, or a composite of several.
