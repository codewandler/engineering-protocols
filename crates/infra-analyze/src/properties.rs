//! The observed invariant-like facts per workload — what IW3 will diff a desired state against.
//!
//! IW2 shipped the minimal three (replicas, parsed images, resource envelopes); IW2.5 widens
//! the sheet to what the refined operator story asks per workload: declared **and observed**
//! replicas, the image reference split down to its registry, and which disruption budgets and
//! autoscalers cover the workload. Everything else a desired-state comparison might want is
//! already in the IR; this module exists so the facts that read like invariants have one
//! extraction and one shape.

use std::collections::BTreeMap;

use infra_compiler::InfraIr;
use infra_domain::workload::WorkloadKind;
use serde::Serialize;

use crate::diagnose::pdb_covers;
use crate::graph::InfraGraph;

/// An image reference, split into what pins it and what does not.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ImageReference {
    /// Registry and repository — everything before the tag or digest.
    pub repository: String,
    /// The registry host, when the reference names one — the first path segment, read as a
    /// registry exactly when it could not be a repository name: it contains a dot or a colon,
    /// or is `localhost` (the container runtimes' own disambiguation rule). `None` means the
    /// default registry resolves the image.
    pub registry: Option<String>,
    /// The tag, when one is stated. `latest` is a tag like any other; judging it is the
    /// diagnosis's job (`INFRA-DIAG-006`), not the parser's.
    pub tag: Option<String>,
    /// The `sha256:…` digest, when the reference is pinned by one.
    pub digest: Option<String>,
}

/// Splits an image reference into repository, registry, tag and digest.
///
/// One owner for the algorithm: the unpinned-image rule and the registry-uniformity candidate
/// both read this, so a rule and the properties cannot disagree about what an image's tag or
/// registry is. The subtleties: a registry port — `registry:5000/app` has a colon and no
/// tag — is handled by only reading a tag out of the last path segment, and a first segment is
/// a registry only under the runtimes' dot/colon/`localhost` rule, so `team/app` stays a plain
/// repository.
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
    let registry = repository.split_once('/').and_then(|(first, _)| {
        if first.contains('.') || first.contains(':') || first == "localhost" {
            Some(first.to_owned())
        } else {
            None
        }
    });
    ImageReference {
        repository: repository.to_owned(),
        registry,
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
    /// Pods whose derived controller is this workload — the observed side of `replicas`.
    pub observed_pods: u32,
    /// How many of those pods currently pass readiness.
    pub ready_pods: u32,
    /// The disruption budgets whose selectors cover this workload's template, by IR key;
    /// `None` when the bundle did not scan budgets — unobserved is not uncovered.
    pub pod_disruption_budgets: Option<Vec<String>>,
    /// The autoscalers targeting this workload, by IR key; `None` when the bundle did not
    /// scan autoscalers.
    pub horizontal_pod_autoscalers: Option<Vec<String>>,
    /// Per container: image and resource envelope, in declared order.
    pub containers: Vec<ContainerProperties>,
}

/// Extracts every workload's properties, in key order.
///
/// Builds the ownership graph internally because the observed-replica count reads it; a caller
/// that already has one saves the walk with [`properties_with`].
#[must_use]
pub fn properties(ir: &InfraIr) -> Vec<WorkloadProperties> {
    properties_with(ir, &InfraGraph::of(ir))
}

/// [`properties`] over a graph the caller already built.
#[must_use]
pub fn properties_with(ir: &InfraIr, graph: &InfraGraph) -> Vec<WorkloadProperties> {
    // One pass over the pods, not one per workload: the derived owner is on the graph.
    let mut observed: BTreeMap<&str, (u32, u32)> = BTreeMap::new();
    for (pod_key, pod) in &ir.model.pods {
        let Some(workload) = graph.owner_of(pod_key) else {
            continue;
        };
        let entry = observed.entry(workload).or_default();
        entry.0 += 1;
        if pod.ready {
            entry.1 += 1;
        }
    }

    ir.model
        .workloads
        .iter()
        .map(|(key, workload)| {
            let (observed_pods, ready_pods) = observed.get(key.as_str()).copied().unwrap_or((0, 0));
            let pod_disruption_budgets = ir.model.pod_disruption_budgets.as_ref().map(|budgets| {
                budgets
                    .iter()
                    .filter(|(_, budget)| {
                        budget.identity.namespace == workload.identity.namespace
                            && pdb_covers(&budget.selector, &workload.template_labels)
                    })
                    .map(|(budget_key, _)| budget_key.clone())
                    .collect()
            });
            let horizontal_pod_autoscalers =
                ir.model
                    .horizontal_pod_autoscalers
                    .as_ref()
                    .map(|autoscalers| {
                        autoscalers
                            .iter()
                            .filter(|(_, autoscaler)| {
                                autoscaler.identity.namespace == workload.identity.namespace
                                    && autoscaler.target.name == workload.identity.name
                                    && matches!(
                                        (autoscaler.target.kind.as_str(), workload.kind),
                                        ("Deployment", WorkloadKind::Deployment)
                                            | ("StatefulSet", WorkloadKind::StatefulSet)
                                    )
                            })
                            .map(|(autoscaler_key, _)| autoscaler_key.clone())
                            .collect()
                    });
            WorkloadProperties {
                workload: key.clone(),
                kind: workload.kind,
                replicas: workload.replicas,
                observed_pods,
                ready_pods,
                pod_disruption_budgets,
                horizontal_pod_autoscalers,
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
            }
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
        assert_eq!(parsed.registry.as_deref(), Some("registry.local:5000"));
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
    fn a_bare_image_name_has_neither_registry_nor_tag_nor_digest() {
        let parsed = parse_image("redis");
        assert_eq!(parsed.repository, "redis");
        assert_eq!(parsed.registry, None);
        assert_eq!(parsed.tag, None);
        assert_eq!(parsed.digest, None);
    }

    #[test]
    fn a_namespaced_hub_image_has_no_registry_because_team_is_not_a_host() {
        // The runtimes' disambiguation rule: `team/app` pulls `docker.io/team/app`, and calling
        // `team` a registry would invent a host nobody named.
        assert_eq!(parse_image("team/app:1").registry, None);
        assert_eq!(
            parse_image("localhost/app:1").registry.as_deref(),
            Some("localhost"),
            "`localhost` is the rule's one hostname without a dot"
        );
        assert_eq!(
            parse_image("111122223333.dkr.ecr.eu-central-1.amazonaws.com/team/app:latest")
                .registry
                .as_deref(),
            Some("111122223333.dkr.ecr.eu-central-1.amazonaws.com")
        );
    }
}
