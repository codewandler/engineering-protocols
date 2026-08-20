//! Wire representations and generated JSON Schema for AEP.
//!
//! This crate is the boundary between documents on disk and the validated domain model. It does
//! two things:
//!
//! * [`parse`] — read a YAML or JSON document into its `Raw*` type and validate it into the
//!   domain type, reporting syntax errors and semantic errors distinguishably.
//! * [`schema`] — generate JSON Schema from those same Rust types.
//!
//! Schemas are **outputs**, never hand-maintained inputs. `cargo xtask schema` writes them to
//! `schemas/generated/`, and CI fails if the committed files differ from what the types produce,
//! which is what stops the published contract from drifting away from the implementation.
//!
//! Deriving a schema is not by itself enough to make that true: `#[serde(alias = "…")]` is
//! invisible to `schemars`, so a derived schema describes half the language the parser reads.
//! [`alias`] is where the other half is put back, and where the guard against it drifting again
//! lives.

pub mod alias;
pub mod format;
pub mod parse;
pub mod schema;

pub use alias::{WireAlias, WIRE_ALIASES};
pub use format::canonical_json;
pub use parse::{DocumentError, DocumentKind};
pub use schema::{generated_schemas, GeneratedSchema};
