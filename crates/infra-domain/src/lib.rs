//! The observation model for infrastructure: the Kubernetes subset, v1.
//!
//! An external scanner (`infra-scout`, a separate repository) reads a cluster and writes an
//! `infra-observation/1` bundle — raw API objects keyed by kind, with every secret value already
//! replaced by `{sha256, length}`. This crate is the boundary that bundle crosses into the
//! toolchain: it parses permissively, validates strictly and accumulates every refusal, exactly
//! as `ess-domain` does for specifications. **Nothing in this workspace reaches a cluster, holds
//! a credential or opens a network connection**; the scanner is the actor, this is the model.
//!
//! | module | contents |
//! |---|---|
//! | [`code`] | the `INFRA-` refusal codes and the accumulating error type |
//! | [`raw`] | the permissive half: what a bundle deserializes into, nothing trusted |
//! | [`observation`] | the validated observation and the `TryFrom` that is the only way in |
//! | [`workload`] | deployments, statefulsets, daemonsets: pod-template essentials |
//! | [`network`] | services and ingresses |
//! | [`config`] | configmaps and secrets: keys and digests, never values |
//!
//! # Two stages, same discipline as everywhere else
//!
//! Bundles deserialize into `Raw*` types and become validated ones through [`TryFrom`]; validated
//! types do not implement [`Deserialize`](serde::Deserialize), so the only way to obtain an
//! [`observation::Observation`] is to have run every rule. Validation pushes into
//! [`code::ValidationErrors`] and keeps going: one run reports every problem.
//!
//! # What the model deliberately excludes
//!
//! The IR downstream is a function of **semantic cluster state** — what someone declared plus the
//! runtime facts diagnosis reads — not of API bookkeeping. Whole classes of fields are therefore
//! absent from the validated model, by class:
//!
//! * **Write-tracking bookkeeping** — `managedFields`, `resourceVersion`, `generation`,
//!   `observedGeneration`. These change on every write, including writes that change nothing
//!   semantic, and any of them in the model would make the digest a counter of API traffic.
//! * **Timestamps** — `creationTimestamp`, condition transition times, `startTime`. A cluster
//!   redeployed identically differs in all of them; the IR of the two must not.
//! * **Assigned runtime addresses** — `clusterIP`, pod and host IPs, `nodePort` allocations.
//!   Assigned by the cluster, not declared by anyone; two identical deployments of one manifest
//!   set differ in every one of them.
//! * **Status beyond the modelled essentials** — conditions, per-replica counters, revision
//!   hashes, image lists on nodes. The essentials that *are* kept (pod phase, readiness, restart
//!   counts, a waiting container's reason, node assignment, owner, claim phase, node capacity
//!   and info) are the runtime facts IW2's diagnosis reads; the rest is reconstruction detail.
//! * **Rollout mechanics** — update strategies, `revisionHistoryLimit`,
//!   `progressDeadlineSeconds`, pod management policies. How a change rolls out is not part of
//!   what the cluster *is*.
//! * **Values of configuration and secrets** — a secret's value never entered the bundle
//!   (refused if it did: `INFRA-SECRET-001`), and a configmap's value is reduced to
//!   `{sha256, length}` at validation. Keys and change-detection survive; content does not.
//!
//! Exclusion happens by construction: the `Raw*` types tolerate every unknown field, and the
//! validated types simply have nowhere to put the noise.

pub mod code;
pub mod config;
pub mod network;
pub mod observation;
pub mod raw;
pub mod workload;

pub use code::{InfraCode, ValidationError, ValidationErrors};
pub use config::{ConfigMap, Secret, ValueDigest};
pub use network::{Ingress, IngressBackend, IngressPath, IngressRule, Service, ServicePort};
pub use observation::{
    ClaimPhase, ContainerStatus, Identity, Namespace, Node, NodeInfo, Observation, OwnerRef,
    PersistentVolumeClaim, Pod, PodPhase, ServiceAccount, KINDS, OBSERVATION_FORMAT,
};
pub use raw::RawBundle;
pub use workload::{
    Container, EnvFrom, EnvFromSource, EnvSource, EnvVar, PodTemplate, Probe, ProbeHandler, Probes,
    Resources, Volume, VolumeMount, VolumeSource, Workload, WorkloadKind,
};
