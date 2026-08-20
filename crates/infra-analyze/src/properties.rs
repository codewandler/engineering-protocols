//! The observed invariant-like facts per workload — what IW3 will diff a desired state against.
//!
//! Deliberately minimal: replica count, images (parsed once, here, for every consumer), and
//! the resource envelope per container. Everything else a desired-state comparison might want
//! is already in the IR; this module exists so the three facts that read like invariants have
//! one extraction and one shape.

use std::collections::BTreeMap;

use infra_compiler::InfraIr;
use infra_domain::workload::WorkloadKind;
use serde::Serialize;

/// An image reference, split into what pins it and what does not.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ImageReference {
    /// Registry and repository — everything before the tag or digest.
    pub repository: String,
    /// The tag, when one is stated. `latest` is a tag like any other; judging it is the
    /// diagnosis's job (`INFRA-DIAG-006`), not the parser's.
    pub tag: Option<String>,
    /// The `sha256:…` digest, when the reference is pinned by one.
    pub digest: Option<String>,
}

/// Splits an image reference into repository, tag and digest.
///
/// One owner for the algorithm: the unpinned-image rule reads this, so the rule and the
/// properties cannot disagree about what an image's tag is. The subtlety is the registry port —
/// `registry:5000/app` has a colon and no tag — handled by only reading a tag out of the last
/// path segment.
#[must_use]
pub fn parse_image(image: &str) -> ImageReference {
    let (name, digest) = match image.split_once('@') {
        Some((name, digest)) => (name, Some(digest.to_owned())),
        None => (image, None),
    };
    let last_segment_at = name.rfind('/').map_or(0, |slash| slash + 1);
    let (repository, tag) = match name[last_segment_at..].find(':') {
        Some(colon) => (
            &name[..last_segment_at + colon],
            Some(name[last_segment_at + colon + 1..].to_owned()),
        ),
        None => (name, None),
    };
    ImageReference {
        repository: repository.to_owned(),
        tag,
        digest,
    }
}

/// One container's observed envelope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ContainerProperties {
    /// The container's name.
    pub container: String,
    /// The image, parsed.
    pub image: ImageReference,
    /// Requested quantities per resource, as the API stated them.
    pub requests: BTreeMap<String, String>,
    /// Limit quantities per resource, as the API stated them.
    pub limits: BTreeMap<String, String>,
}

/// One workload's observed invariant-like facts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WorkloadProperties {
    /// The workload's IR key, `namespace/kind/name`.
    pub workload: String,
    /// Which of the three kinds.
    pub kind: WorkloadKind,
    /// Desired replicas; absent on daemonsets.
    pub replicas: Option<u32>,
    /// Per container: image and resource envelope, in declared order.
    pub containers: Vec<ContainerProperties>,
}

/// Extracts every workload's properties, in key order.
#[must_use]
pub fn properties(ir: &InfraIr) -> Vec<WorkloadProperties> {
    ir.model
        .workloads
        .iter()
        .map(|(key, workload)| WorkloadProperties {
            workload: key.clone(),
            kind: workload.kind,
            replicas: workload.replicas,
            containers: workload
                .containers
                .iter()
                .map(|container| ContainerProperties {
                    container: container.name.clone(),
                    image: parse_image(&container.image),
                    requests: container.resources.requests.clone(),
                    limits: container.resources.limits.clone(),
                })
                .collect(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_image_with_a_registry_port_and_no_tag_is_untagged_not_tagged_5000() {
        let parsed = parse_image("registry.local:5000/team/app");
        assert_eq!(parsed.repository, "registry.local:5000/team/app");
        assert_eq!(parsed.tag, None, "the port is not a tag");
        assert_eq!(parsed.digest, None);
    }

    #[test]
    fn a_tagged_image_with_a_registry_port_keeps_both_apart() {
        let parsed = parse_image("registry.local:5000/team/app:1.2.3");
        assert_eq!(parsed.repository, "registry.local:5000/team/app");
        assert_eq!(parsed.tag.as_deref(), Some("1.2.3"));
    }

    #[test]
    fn a_digest_pinned_image_reports_the_digest_and_whatever_tag_rides_along() {
        let parsed = parse_image("app:1.0@sha256:abc123");
        assert_eq!(parsed.repository, "app");
        assert_eq!(parsed.tag.as_deref(), Some("1.0"));
        assert_eq!(parsed.digest.as_deref(), Some("sha256:abc123"));
    }

    #[test]
    fn a_bare_image_name_has_neither_tag_nor_digest() {
        let parsed = parse_image("redis");
        assert_eq!(parsed.repository, "redis");
        assert_eq!(parsed.tag, None);
        assert_eq!(parsed.digest, None);
    }
}
