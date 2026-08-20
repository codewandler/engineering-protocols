//! Validated observation → `infra-ir/1`: normalized, content-addressed, deterministic.
//!
//! The second half of the infrastructure pipeline. `infra-domain` is the boundary a scanned
//! bundle crosses — parse permissively, validate strictly, refuse loudly; this crate is what the
//! toolchain *keeps*: an IR whose maps are keyed by identity, whose references are checked
//! handles or openly carried unresolved facts, and whose digest is a function of semantic
//! cluster state alone.
//!
//! | module | contents |
//! |---|---|
//! | [`ir`] | the IR: handles, resolved shapes, the model, provenance, the digest |
//! | [`compile`](mod@compile) | the total compilation from a validated observation |
//!
//! # Determinism
//!
//! Same observation in, byte-identical canonical JSON out — and the same bytes again when the
//! bundle's `kinds` or any item list arrives in a different order, because every map is keyed by
//! identity and the unresolved facts are sorted. `tests/determinism.rs` compiles twice, shuffles,
//! and compares bytes; the source scan in the same file keeps unordered maps and clocks out of
//! this crate (invariant 9).

pub mod compile;
pub mod ir;

pub use compile::compile;
pub use ir::{
    digest_of_canonical, ClaimHandle, ConfigMapHandle, InfraIr, InfraIrDocument, InfraModel,
    NodeHandle, Provenance, Reference, ResolvedContainer, ResolvedEnvFrom, ResolvedEnvFromSource,
    ResolvedEnvSource, ResolvedEnvVar, ResolvedIngress, ResolvedIngressBackend,
    ResolvedIngressPath, ResolvedIngressRule, ResolvedPod, ResolvedVolume, ResolvedVolumeSource,
    ResolvedWorkload, SecretHandle, ServiceAccountHandle, ServiceHandle, UnresolvedReference,
    UnresolvedTarget, IR_FORMAT,
};
