//! The storage-independent interaction contract.
//!
//! Two surfaces, and the split is the whole design:
//!
//! ```text
//! CommandService   every state change, one boundary
//! QueryService     read-only, never mutates
//! ```
//!
//! A backend may be a database, a git repository, a directory of files, a remote API or several of
//! those at once. The contract defines **observable behaviour** only, which is what makes a
//! conformance suite possible: an implementation proves it conforms by passing tests it cannot see
//! the inside of.
//!
//! # What one mutation boundary buys
//!
//! Validation, authorisation, protocol enforcement, idempotency, optimistic concurrency, provenance,
//! events, audit, correlation and causation all attach to one place. A second write path is a second
//! place for every one of those to be forgotten.
//!
//! # Async without an executor
//!
//! The traits use `async fn`, and this crate depends on no runtime. A synchronous backend implements
//! them with futures that never yield; [`testing::block_on`] drives one to completion without
//! pulling an executor into a specification crate.
//!
//! | module | contents |
//! |---|---|
//! | [`command`] | `CommandEnvelope`, `CommandContext`, `CommandResult`, `CommandService` |
//! | [`query`] | `QueryService`, entity and relation queries, history, audit, paging |
//! | [`consistency`] | `ConsistencyToken` and `QueryConsistency` — read-your-writes across backends |
//! | [`error`] | the typed failure taxonomy both surfaces share |
//! | [`registry`] | `TypeDescriptor`, so a harness can ask what a design is |
//! | [`testing`] | the minimal future driver, for backends and conformance suites |

pub mod command;
pub mod consistency;
pub mod error;
pub mod query;
pub mod registry;
pub mod testing;

pub use command::{CommandContext, CommandEnvelope, CommandOutcome, CommandResult, CommandService};
pub use consistency::{ConsistencyToken, QueryConsistency};
pub use error::{CommandError, QueryError};
pub use query::{
    AuditQuery, Cursor, EntityEnvelope, EntityQuery, Page, QueryService, RelationQuery,
    RevisionRecord,
};
pub use registry::{CommandDescriptor, LifecycleDescriptor, RelationDescriptor, TypeDescriptor};
