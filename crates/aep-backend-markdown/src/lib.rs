//! A markdown planning store: one artifact per file, YAML frontmatter, git as the log.
//!
//! This is the first durable store in the repository. Until now the only implementation of the
//! interaction contract was `aep-backend-memory`, which forgets everything when the process
//! exits — good enough to prove the contract is implementable, useless for keeping a plan.
//!
//! # What it is for
//!
//! An agent that has to plan work needs somewhere to put epics, stories and tasks that survives
//! the session, that a human can read and edit in an ordinary editor, and whose history is already
//! reviewable. A directory of markdown files in the repository is all three: the diff of a status
//! move is one line, `git log` says who moved it and when, and nobody needs a database or a
//! credential to read the plan.
//!
//! ```text
//! .engineering/planning/
//! ├── initiative/passwordless.md
//! ├── epic/passkeys.md
//! ├── story/passkey-login.md
//! └── task/webauthn-ceremony.md
//! ```
//!
//! # Where this sits relative to the contract
//!
//! This crate is **the store**, not yet the backend. The plan is that it becomes the second
//! implementor of the `aep-contract` traits — `CommandService` and `QueryService` over a
//! journal beside the markdown, so a replayed command returns its original result and every
//! refusal leaves a record. That is a labelled later milestone and deliberately not attempted
//! here: the contract's guarantees are about idempotency, optimistic concurrency and audit, and
//! bolting them onto a file tree without a journal would produce a backend that passes by accident
//! and fails under a second writer. What exists today is the layer that implementor will sit on:
//! parse, render, validate a status move, read a whole tree and hand back an
//! [`ArtifactGraph`](aep_domain::artifact::ArtifactGraph).
//!
//! That gap is registered rather than left implicit: the CLI writes through this crate's
//! [`create`](store::MarkdownStore::create) and [`update`](store::MarkdownStore::update) instead of
//! through a command, which is **deviation D-P1** against invariant 14 in
//! `docs/plan/gap-register.md`. The mitigation is the shape above — every write in the workspace
//! funnels through those two functions, so the milestone that reroutes them reroutes all of it.
//!
//! Consequently there is **no delete**, on any type here. Removing a plan item is a status move to
//! `archived`, which is invariant 16 spelled for a file tree: an epic that was abandoned is a fact
//! about how the work went, and a store that can drop it silently is a store whose history lies.
//!
//! # The frontmatter is this backend's format, not the protocol's
//!
//! `aep-domain` gains nothing from this crate — no type, no field, no format constant. The
//! frontmatter key set, the `aep.planning-md/1` version, the `---` fences and the file layout are
//! all private to this backend, and the crate's job is to map them onto the domain types that
//! already exist: [`Artifact`](aep_domain::artifact::Artifact),
//! [`ArtifactId`](aep_domain::artifact::ArtifactId),
//! [`ArtifactKind`](aep_domain::artifact::ArtifactKind),
//! [`ArtifactStatus`](aep_domain::artifact::ArtifactStatus),
//! [`ArtifactRelation`](aep_domain::artifact::ArtifactRelation) and
//! [`ArtifactLifecycle`](aep_domain::artifact::ArtifactLifecycle).
//!
//! That boundary is the same one [`ArtifactLocation`](aep_domain::artifact::ArtifactLocation)
//! draws: location is metadata, the graph is normative. A second backend keeping the same
//! artifacts as rows in Postgres or as issues in Linear must be able to exist without teaching the
//! protocol anything about YAML fences — and it cannot if the protocol crate carries one storage
//! format's spelling.
//!
//! # No clock, no randomness
//!
//! Nothing here reads the wall clock or an RNG, so rendering a document twice produces the same
//! bytes and a test does not have to freeze time. In particular a document records **no
//! `created_at`**: git already knows who wrote the file and when, to the commit, and a timestamp
//! the tooling maintains beside it is a second answer that goes stale the first time somebody
//! rebases. The same reasoning fixes the CLI's seeding clock to `Timestamp::EPOCH`.

pub mod document;
pub mod frontmatter;
pub mod store;

pub use document::{MoveRefusal, PlanningDocument, PlanningDocumentError};
pub use frontmatter::{PlanningFrontmatter, RawPlanningFrontmatter, PLANNING_FORMAT};
pub use store::{MarkdownStore, StoreError, StoreFailure, StoreReport, StoredDocument};
