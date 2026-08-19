//! Wire representations.
//!
//! Every AEP document parses into a `Raw*` type and is then converted into its validated
//! counterpart with [`TryFrom`]. The raw types are what JSON Schema is generated from, and they
//! are re-exported here so that a consumer can find the whole wire surface in one place:
//!
//! | document | raw type | validated type |
//! |---|---|---|
//! | `protocols/*.yaml` | [`RawProtocol`] | [`Protocol`](crate::protocol::Protocol) |
//! | `principles/*.yaml` | [`RawPrinciple`] | [`Principle`](crate::principle::Principle) |
//! | `workflows/*.yaml` | [`RawWorkflow`] | [`Workflow`](crate::workflow::Workflow) |
//! | `profiles/*.yaml` | [`RawProfile`] | [`Profile`](crate::profile::Profile) |
//! | a task | [`RawTask`] | [`Task`](crate::task::Task) |
//! | `.engineering/artifacts.yaml` | [`RawArtifactManifest`] | [`ArtifactGraph`](crate::artifact::ArtifactGraph) |
//!
//! ```no_run
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # let input = "";
//! use aep_domain::protocol::Protocol;
//! use aep_domain::raw::RawProtocol;
//!
//! let raw: RawProtocol = serde_yaml::from_str(input)?;
//! let protocol = Protocol::try_from(raw)?;
//! // `protocol` is semantically valid from this point onward.
//! # let _ = protocol;
//! # Ok(())
//! # }
//! ```

pub use crate::artifact::{RawArtifactManifest, ARTIFACT_MANIFEST_VERSION};
pub use crate::principle::{RawObligation, RawPrinciple};
pub use crate::profile::RawProfile;
pub use crate::protocol::RawProtocol;
pub use crate::task::RawTask;
pub use crate::workflow::{RawState, RawTransition, RawWorkflow};
