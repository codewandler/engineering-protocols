//! Validated observation in, IR out — totally.
//!
//! [`compile`] cannot fail. Everything refusable was refused by `infra-domain` before an
//! [`Observation`] could exist, and what remains unresolvable in a *valid* observation — a
//! reference to something the cluster does not hold — is data the IR carries openly
//! ([`UnresolvedReference`]), because a degraded cluster is exactly what IW2's diagnosis is for.

use std::collections::BTreeMap;

use infra_domain::observation::Observation;
use infra_domain::workload::{EnvFromSource, EnvSource, VolumeSource, Workload, WorkloadKind};

use crate::ir::{
    ClaimHandle, ConfigMapHandle, InfraIr, InfraModel, NodeHandle, Provenance, Reference,
    ResolvedContainer, ResolvedEnvFrom, ResolvedEnvFromSource, ResolvedEnvSource, ResolvedEnvVar,
    ResolvedIngress, ResolvedIngressBackend, ResolvedIngressPath, ResolvedIngressRule, ResolvedPod,
    ResolvedVolume, ResolvedVolumeSource, ResolvedWorkload, SecretHandle, ServiceAccountHandle,
    ServiceHandle, UnresolvedReference, UnresolvedTarget,
};

/// The map key of a namespaced object: `namespace/name`.
fn scoped_key(namespace: Option<&str>, name: &str) -> String {
    match namespace {
        Some(namespace) => format!("{namespace}/{name}"),
        None => name.to_owned(),
    }
}

/// The map key of a workload: `namespace/kind/name`.
fn workload_key(namespace: Option<&str>, kind: WorkloadKind, name: &str) -> String {
    format!("{}/{}/{name}", namespace.unwrap_or_default(), kind.as_str())
}

/// Compiles a validated observation into the IR. Total: see the module doc.
///
/// Long because a cluster has many kinds, not because any step is deep: each block below is one
/// kind's normalization, and splitting them into functions would only scatter the one invariant
/// they share — every map keyed by identity, every dangling reference pushed as a fact.
#[allow(clippy::too_many_lines)]
#[must_use]
pub fn compile(observation: &Observation) -> InfraIr {
    let mut facts: Vec<UnresolvedReference> = Vec::new();

    // The plain kinds first: they are what references resolve against.
    let namespaces: BTreeMap<String, _> = observation
        .namespaces
        .iter()
        .map(|namespace| (namespace.identity.name.clone(), namespace.clone()))
        .collect();
    let nodes: BTreeMap<String, _> = observation
        .nodes
        .iter()
        .map(|node| (node.identity.name.clone(), node.clone()))
        .collect();
    let services: BTreeMap<String, _> = observation
        .services
        .iter()
        .map(|service| {
            (
                scoped_key(
                    service.identity.namespace.as_deref(),
                    &service.identity.name,
                ),
                service.clone(),
            )
        })
        .collect();
    let config_maps: BTreeMap<String, _> = observation
        .config_maps
        .iter()
        .map(|config_map| {
            (
                scoped_key(
                    config_map.identity.namespace.as_deref(),
                    &config_map.identity.name,
                ),
                config_map.clone(),
            )
        })
        .collect();
    let secrets: BTreeMap<String, _> = observation
        .secrets
        .iter()
        .map(|secret| {
            (
                scoped_key(secret.identity.namespace.as_deref(), &secret.identity.name),
                secret.clone(),
            )
        })
        .collect();
    let service_accounts: BTreeMap<String, _> = observation
        .service_accounts
        .iter()
        .map(|account| {
            (
                scoped_key(
                    account.identity.namespace.as_deref(),
                    &account.identity.name,
                ),
                account.clone(),
            )
        })
        .collect();
    let claims: BTreeMap<String, _> = observation
        .claims
        .iter()
        .map(|claim| {
            (
                scoped_key(claim.identity.namespace.as_deref(), &claim.identity.name),
                claim.clone(),
            )
        })
        .collect();

    // Pods next: services' selectors are checked against them.
    let mut pods = BTreeMap::new();
    for pod in &observation.pods {
        let key = scoped_key(pod.identity.namespace.as_deref(), &pod.identity.name);
        let from = format!("pods/{key}");
        let node = pod.node.as_ref().map(|name| {
            if nodes.contains_key(name) {
                Reference::Resolved {
                    key: NodeHandle::new(name.clone()),
                }
            } else {
                facts.push(UnresolvedReference {
                    from: from.clone(),
                    site: "nodeName".to_owned(),
                    target: UnresolvedTarget::Node { name: name.clone() },
                });
                Reference::Unresolved { name: name.clone() }
            }
        });
        pods.insert(
            key,
            ResolvedPod {
                identity: pod.identity.clone(),
                labels: pod.labels.clone(),
                phase: pod.phase,
                ready: pod.ready,
                node,
                owner: pod.owner.clone(),
                containers: pod.containers.clone(),
            },
        );
    }

    // Workloads, with every configuration reference checked.
    let mut workloads = BTreeMap::new();
    for workload in &observation.workloads {
        let key = workload_key(
            workload.identity.namespace.as_deref(),
            workload.kind,
            &workload.identity.name,
        );
        let resolved = resolve_workload(
            workload,
            &key,
            &services,
            &config_maps,
            &secrets,
            &service_accounts,
            &claims,
            &mut facts,
        );
        workloads.insert(key, resolved);
    }

    // Ingresses: each backend names a service in the ingress's own namespace.
    let mut ingresses = BTreeMap::new();
    for ingress in &observation.ingresses {
        let namespace = ingress.identity.namespace.as_deref();
        let key = scoped_key(namespace, &ingress.identity.name);
        let from = format!("ingresses/{key}");
        let resolve_backend = |backend: &infra_domain::network::IngressBackend,
                               site: String,
                               facts: &mut Vec<UnresolvedReference>| {
            let service_key = scoped_key(namespace, &backend.service);
            let service = if services.contains_key(&service_key) {
                Reference::Resolved {
                    key: ServiceHandle::new(service_key),
                }
            } else {
                facts.push(UnresolvedReference {
                    from: from.clone(),
                    site,
                    target: UnresolvedTarget::Service {
                        name: backend.service.clone(),
                    },
                });
                Reference::Unresolved {
                    name: backend.service.clone(),
                }
            };
            ResolvedIngressBackend {
                service,
                port: backend.port.clone(),
            }
        };
        let rules = ingress
            .rules
            .iter()
            .enumerate()
            .map(|(rule_index, rule)| ResolvedIngressRule {
                host: rule.host.clone(),
                paths: rule
                    .paths
                    .iter()
                    .enumerate()
                    .map(|(path_index, path)| ResolvedIngressPath {
                        path: path.path.clone(),
                        path_type: path.path_type.clone(),
                        backend: resolve_backend(
                            &path.backend,
                            format!("rules[{rule_index}].paths[{path_index}]"),
                            &mut facts,
                        ),
                    })
                    .collect(),
            })
            .collect();
        let default_backend = ingress
            .default_backend
            .as_ref()
            .map(|backend| resolve_backend(backend, "defaultBackend".to_owned(), &mut facts));
        ingresses.insert(
            key,
            ResolvedIngress {
                identity: ingress.identity.clone(),
                labels: ingress.labels.clone(),
                rules,
                default_backend,
            },
        );
    }

    // A selector that matches nothing is a fact, not an error: the cluster really is in that
    // state, and diagnosis (IW2) is the one to say whether it is a defect.
    for (key, service) in &services {
        if service.selector.is_empty() {
            continue;
        }
        let selects_something = observation.pods.iter().any(|pod| {
            pod.identity.namespace == service.identity.namespace
                && service
                    .selector
                    .iter()
                    .all(|(label, value)| pod.labels.get(label) == Some(value))
        });
        if !selects_something {
            facts.push(UnresolvedReference {
                from: format!("services/{key}"),
                site: "selector".to_owned(),
                target: UnresolvedTarget::PodsMatchingSelector {
                    selector: service.selector.clone(),
                },
            });
        }
    }

    // Every namespaced object sits in a namespace the scan should have seen.
    for (map, keys_and_namespaces) in [
        (
            "workloads",
            workloads
                .iter()
                .map(|(key, entry)| (key.clone(), entry.identity.namespace.clone()))
                .collect::<Vec<_>>(),
        ),
        (
            "services",
            services
                .iter()
                .map(|(key, entry)| (key.clone(), entry.identity.namespace.clone()))
                .collect(),
        ),
        (
            "ingresses",
            ingresses
                .iter()
                .map(|(key, entry)| (key.clone(), entry.identity.namespace.clone()))
                .collect(),
        ),
        (
            "config_maps",
            config_maps
                .iter()
                .map(|(key, entry)| (key.clone(), entry.identity.namespace.clone()))
                .collect(),
        ),
        (
            "secrets",
            secrets
                .iter()
                .map(|(key, entry)| (key.clone(), entry.identity.namespace.clone()))
                .collect(),
        ),
        (
            "service_accounts",
            service_accounts
                .iter()
                .map(|(key, entry)| (key.clone(), entry.identity.namespace.clone()))
                .collect(),
        ),
        (
            "claims",
            claims
                .iter()
                .map(|(key, entry)| (key.clone(), entry.identity.namespace.clone()))
                .collect(),
        ),
        (
            "pods",
            pods.iter()
                .map(|(key, entry)| (key.clone(), entry.identity.namespace.clone()))
                .collect(),
        ),
    ] {
        for (key, namespace) in keys_and_namespaces {
            if let Some(namespace) = namespace {
                if !namespaces.contains_key(&namespace) {
                    facts.push(UnresolvedReference {
                        from: format!("{map}/{key}"),
                        site: "metadata.namespace".to_owned(),
                        target: UnresolvedTarget::Namespace { name: namespace },
                    });
                }
            }
        }
    }

    // Sorted and deduplicated: the aggregate must not leak the order the bundle presented
    // anything in, or the digest stops being a function of the cluster.
    facts.sort();
    facts.dedup();

    InfraIr {
        provenance: Provenance {
            context: observation.context.clone(),
            scanned_at: observation.scanned_at.clone(),
            scout_version: observation.scout_version.clone(),
            compiler_version: env!("CARGO_PKG_VERSION").to_owned(),
        },
        model: InfraModel {
            namespaces,
            nodes,
            workloads,
            services,
            ingresses,
            config_maps,
            secrets,
            service_accounts,
            claims,
            pods,
            unresolved: facts,
        },
    }
}

/// Resolves one workload's references. Its own function for the reason `run_ess` is one in the
/// CLI: the body is long because a workload has many reference sites, not because it is complex.
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn resolve_workload(
    workload: &Workload,
    key: &str,
    services: &BTreeMap<String, infra_domain::network::Service>,
    config_maps: &BTreeMap<String, infra_domain::config::ConfigMap>,
    secrets: &BTreeMap<String, infra_domain::config::Secret>,
    service_accounts: &BTreeMap<String, infra_domain::observation::ServiceAccount>,
    claims: &BTreeMap<String, infra_domain::observation::PersistentVolumeClaim>,
    facts: &mut Vec<UnresolvedReference>,
) -> ResolvedWorkload {
    let namespace = workload.identity.namespace.as_deref();
    let from = format!("workloads/{key}");

    let resolve_config_map = |name: &str,
                              needed_key: Option<&str>,
                              optional: bool,
                              site: String,
                              facts: &mut Vec<UnresolvedReference>| {
        let map_key = scoped_key(namespace, name);
        if let Some(config_map) = config_maps.get(&map_key) {
            if let Some(needed_key) = needed_key {
                if !config_map.keys.contains_key(needed_key) {
                    facts.push(UnresolvedReference {
                        from: from.clone(),
                        site,
                        target: UnresolvedTarget::ConfigMapKey {
                            name: name.to_owned(),
                            key: needed_key.to_owned(),
                            optional,
                        },
                    });
                }
            }
            Reference::Resolved {
                key: ConfigMapHandle::new(map_key),
            }
        } else {
            facts.push(UnresolvedReference {
                from: from.clone(),
                site,
                target: UnresolvedTarget::ConfigMap {
                    name: name.to_owned(),
                    optional,
                },
            });
            Reference::Unresolved {
                name: name.to_owned(),
            }
        }
    };
    let resolve_secret = |name: &str,
                          needed_key: Option<&str>,
                          optional: bool,
                          site: String,
                          facts: &mut Vec<UnresolvedReference>| {
        let map_key = scoped_key(namespace, name);
        if let Some(secret) = secrets.get(&map_key) {
            if let Some(needed_key) = needed_key {
                if !secret.keys.contains_key(needed_key) {
                    facts.push(UnresolvedReference {
                        from: from.clone(),
                        site,
                        target: UnresolvedTarget::SecretKey {
                            name: name.to_owned(),
                            key: needed_key.to_owned(),
                            optional,
                        },
                    });
                }
            }
            Reference::Resolved {
                key: SecretHandle::new(map_key),
            }
        } else {
            facts.push(UnresolvedReference {
                from: from.clone(),
                site,
                target: UnresolvedTarget::Secret {
                    name: name.to_owned(),
                    optional,
                },
            });
            Reference::Unresolved {
                name: name.to_owned(),
            }
        }
    };

    let containers = workload
        .template
        .containers
        .iter()
        .map(|container| {
            let env = container
                .env
                .iter()
                .map(|variable| {
                    let site = format!("containers[{}].env[{}]", container.name, variable.name);
                    let source = match &variable.source {
                        EnvSource::Literal { value } => ResolvedEnvSource::Literal {
                            value: value.clone(),
                        },
                        EnvSource::ConfigMapKey {
                            name,
                            key: entry,
                            optional,
                        } => ResolvedEnvSource::ConfigMapKey {
                            config_map: resolve_config_map(
                                name,
                                Some(entry),
                                *optional,
                                site,
                                facts,
                            ),
                            key: entry.clone(),
                            optional: *optional,
                        },
                        EnvSource::SecretKey {
                            name,
                            key: entry,
                            optional,
                        } => ResolvedEnvSource::SecretKey {
                            secret: resolve_secret(name, Some(entry), *optional, site, facts),
                            key: entry.clone(),
                            optional: *optional,
                        },
                        EnvSource::FieldRef { path } => {
                            ResolvedEnvSource::FieldRef { path: path.clone() }
                        }
                        EnvSource::ResourceFieldRef => ResolvedEnvSource::ResourceFieldRef,
                    };
                    ResolvedEnvVar {
                        name: variable.name.clone(),
                        source,
                    }
                })
                .collect();
            let env_from = container
                .env_from
                .iter()
                .enumerate()
                .map(|(index, entry)| {
                    let site = format!("containers[{}].envFrom[{index}]", container.name);
                    let source = match &entry.source {
                        EnvFromSource::ConfigMap { name, optional } => {
                            ResolvedEnvFromSource::ConfigMap {
                                config_map: resolve_config_map(name, None, *optional, site, facts),
                                optional: *optional,
                            }
                        }
                        EnvFromSource::Secret { name, optional } => ResolvedEnvFromSource::Secret {
                            secret: resolve_secret(name, None, *optional, site, facts),
                            optional: *optional,
                        },
                    };
                    ResolvedEnvFrom {
                        prefix: entry.prefix.clone(),
                        source,
                    }
                })
                .collect();
            ResolvedContainer {
                name: container.name.clone(),
                image: container.image.clone(),
                env,
                env_from,
                volume_mounts: container.volume_mounts.clone(),
                probes: container.probes.clone(),
                resources: container.resources.clone(),
            }
        })
        .collect();

    let volumes = workload
        .template
        .volumes
        .iter()
        .map(|volume| {
            let site = format!("volumes[{}]", volume.name);
            let source = match &volume.source {
                VolumeSource::ConfigMap { name, optional } => ResolvedVolumeSource::ConfigMap {
                    config_map: resolve_config_map(name, None, *optional, site, facts),
                    optional: *optional,
                },
                VolumeSource::Secret { name, optional } => ResolvedVolumeSource::Secret {
                    secret: resolve_secret(name, None, *optional, site, facts),
                    optional: *optional,
                },
                VolumeSource::Claim { claim } => {
                    let map_key = scoped_key(namespace, claim);
                    let reference = if claims.contains_key(&map_key) {
                        Reference::Resolved {
                            key: ClaimHandle::new(map_key),
                        }
                    } else {
                        facts.push(UnresolvedReference {
                            from: from.clone(),
                            site,
                            target: UnresolvedTarget::Claim {
                                name: claim.clone(),
                            },
                        });
                        Reference::Unresolved {
                            name: claim.clone(),
                        }
                    };
                    ResolvedVolumeSource::Claim { claim: reference }
                }
                VolumeSource::EmptyDir => ResolvedVolumeSource::EmptyDir,
                VolumeSource::HostPath { path } => {
                    ResolvedVolumeSource::HostPath { path: path.clone() }
                }
                VolumeSource::Other => ResolvedVolumeSource::Other,
            };
            ResolvedVolume {
                name: volume.name.clone(),
                source,
            }
        })
        .collect();

    // The document's absence of `serviceAccountName` means `default` — a resolution the
    // compiler performs, so the IR always states which account the pods run as.
    let account_name = workload
        .template
        .service_account
        .clone()
        .unwrap_or_else(|| "default".to_owned());
    let account_key = scoped_key(namespace, &account_name);
    let service_account = if service_accounts.contains_key(&account_key) {
        Reference::Resolved {
            key: ServiceAccountHandle::new(account_key),
        }
    } else {
        facts.push(UnresolvedReference {
            from: from.clone(),
            site: "serviceAccountName".to_owned(),
            target: UnresolvedTarget::ServiceAccount {
                name: account_name.clone(),
            },
        });
        Reference::Unresolved { name: account_name }
    };

    let governing_service = workload.service_name.as_ref().map(|name| {
        let service_key = scoped_key(namespace, name);
        if services.contains_key(&service_key) {
            Reference::Resolved {
                key: ServiceHandle::new(service_key),
            }
        } else {
            facts.push(UnresolvedReference {
                from: from.clone(),
                site: "serviceName".to_owned(),
                target: UnresolvedTarget::Service { name: name.clone() },
            });
            Reference::Unresolved { name: name.clone() }
        }
    });

    ResolvedWorkload {
        kind: workload.kind,
        identity: workload.identity.clone(),
        labels: workload.labels.clone(),
        replicas: workload.replicas,
        selector: workload.selector.clone(),
        governing_service,
        service_account,
        template_labels: workload.template.labels.clone(),
        containers,
        volumes,
    }
}
