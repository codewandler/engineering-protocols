//! Services and ingresses: how traffic reaches workloads.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::code::{InfraCode, ValidationErrors};
use crate::observation::{identity, port_string, string_map, Identity};
use crate::raw::{RawIngress, RawIngressBackend, RawService};

/// A service.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Service {
    /// Identity.
    pub identity: Identity,
    /// Labels.
    pub labels: BTreeMap<String, String>,
    /// The service type; `ClusterIP` when the document omitted it, which is the API's default.
    pub service_type: String,
    /// The pod selector. Empty means the service selects nothing by label — endpoints are
    /// managed by hand or by a controller, which is legal and carried as-is.
    pub selector: BTreeMap<String, String>,
    /// The declared ports, in declared order.
    pub ports: Vec<ServicePort>,
}

/// One declared service port.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ServicePort {
    /// The port's name, when declared.
    pub name: Option<String>,
    /// The exposed port.
    pub port: u16,
    /// The pod-side port: a number or a named container port, rendered as text. Equal to
    /// `port` when the document omitted it, which is the API's default.
    pub target_port: String,
    /// The protocol; `TCP` when the document omitted it, which is the API's default.
    pub protocol: String,
}

/// An ingress: rules from hosts and paths to backend services.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Ingress {
    /// Identity.
    pub identity: Identity,
    /// Labels.
    pub labels: BTreeMap<String, String>,
    /// The routing rules, in declared order.
    pub rules: Vec<IngressRule>,
    /// The backend used when no rule matches.
    pub default_backend: Option<IngressBackend>,
}

/// One ingress rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IngressRule {
    /// The host the rule applies to, when declared.
    pub host: Option<String>,
    /// The paths, in declared order.
    pub paths: Vec<IngressPath>,
}

/// One routed path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IngressPath {
    /// The URL path.
    pub path: Option<String>,
    /// How the path matches, such as `Prefix`.
    pub path_type: Option<String>,
    /// Where matching traffic goes.
    pub backend: IngressBackend,
}

/// A backend service reference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IngressBackend {
    /// The service's name, in the ingress's own namespace.
    pub service: String,
    /// The service port, by name or number, rendered as text.
    pub port: Option<String>,
}

impl Service {
    pub(crate) fn from_raw(
        raw: &RawService,
        location: &str,
        errors: &mut ValidationErrors,
    ) -> Option<Self> {
        let identity = identity(&raw.metadata, true, location, errors)?;
        let mut ports = Vec::with_capacity(raw.spec.ports.len());
        for (index, port) in raw.spec.ports.iter().enumerate() {
            let port_location = format!("{location}.spec.ports[{index}]");
            let Some(number) = port.port else {
                errors.refuse(
                    InfraCode::MalformedObject,
                    format!("{port_location}.port"),
                    "a service port without a port number exposes nothing",
                );
                continue;
            };
            let Ok(number) = u16::try_from(number) else {
                errors.refuse(
                    InfraCode::MalformedObject,
                    format!("{port_location}.port"),
                    format!("{number} is not a port; ports are 1–65535"),
                );
                continue;
            };
            let target_port = match &port.target_port {
                Some(value) => {
                    match port_string(value, &format!("{port_location}.targetPort"), errors) {
                        Some(target) => target,
                        None => continue,
                    }
                }
                None => number.to_string(),
            };
            ports.push(ServicePort {
                name: port.name.clone(),
                port: number,
                target_port,
                protocol: port.protocol.clone().unwrap_or_else(|| "TCP".to_owned()),
            });
        }
        Some(Self {
            identity,
            labels: string_map(
                &raw.metadata.labels,
                &format!("{location}.metadata.labels"),
                errors,
            ),
            service_type: raw
                .spec
                .service_type
                .clone()
                .unwrap_or_else(|| "ClusterIP".to_owned()),
            selector: string_map(
                &raw.spec.selector,
                &format!("{location}.spec.selector"),
                errors,
            ),
            ports,
        })
    }
}

impl IngressBackend {
    /// Validates one backend, refusing a backend that names no service.
    fn from_raw(
        raw: &RawIngressBackend,
        location: &str,
        errors: &mut ValidationErrors,
    ) -> Option<Self> {
        let Some(service) = raw
            .service
            .as_ref()
            .and_then(|service| service.name.clone())
            .filter(|name| !name.is_empty())
        else {
            errors.refuse(
                InfraCode::IncompleteBackend,
                location.to_owned(),
                "the backend names no service; traffic routed nowhere is a rule that means nothing",
            );
            return None;
        };
        let port = raw.service.as_ref().and_then(|backend| {
            backend.port.as_ref().and_then(|port| {
                port.name
                    .clone()
                    .or_else(|| port.number.map(|number| number.to_string()))
            })
        });
        Some(Self { service, port })
    }
}

impl Ingress {
    pub(crate) fn from_raw(
        raw: &RawIngress,
        location: &str,
        errors: &mut ValidationErrors,
    ) -> Option<Self> {
        let identity = identity(&raw.metadata, true, location, errors)?;
        let mut rules = Vec::with_capacity(raw.spec.rules.len());
        for (rule_index, rule) in raw.spec.rules.iter().enumerate() {
            let mut paths = Vec::new();
            if let Some(http) = &rule.http {
                for (path_index, path) in http.paths.iter().enumerate() {
                    let path_location = format!(
                        "{location}.spec.rules[{rule_index}].http.paths[{path_index}].backend"
                    );
                    let backend = if let Some(backend) = &path.backend {
                        IngressBackend::from_raw(backend, &path_location, errors)
                    } else {
                        errors.refuse(
                            InfraCode::IncompleteBackend,
                            path_location,
                            "the path declares no backend at all",
                        );
                        None
                    };
                    if let Some(backend) = backend {
                        paths.push(IngressPath {
                            path: path.path.clone(),
                            path_type: path.path_type.clone(),
                            backend,
                        });
                    }
                }
            }
            rules.push(IngressRule {
                host: rule.host.clone(),
                paths,
            });
        }
        let default_backend = raw.spec.default_backend.as_ref().and_then(|backend| {
            IngressBackend::from_raw(backend, &format!("{location}.spec.defaultBackend"), errors)
        });
        Some(Self {
            identity,
            labels: string_map(
                &raw.metadata.labels,
                &format!("{location}.metadata.labels"),
                errors,
            ),
            rules,
            default_backend,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_service_port_out_of_range_is_refused_and_the_rest_of_the_service_survives_the_pass() {
        let raw: RawService = serde_json::from_value(serde_json::json!({
            "metadata": { "name": "web", "namespace": "app", "uid": "u1" },
            "spec": { "ports": [
                { "port": 70000 },
                { "port": 443, "targetPort": "https" }
            ] }
        }))
        .expect("the raw service parses");
        let mut errors = ValidationErrors::new();
        let service = Service::from_raw(&raw, "s", &mut errors).expect("identity is intact");
        assert!(
            errors.contains(InfraCode::MalformedObject),
            "70000 is refused: {errors}"
        );
        assert_eq!(service.ports.len(), 1, "the valid port is still collected");
        assert_eq!(service.ports[0].target_port, "https");
    }

    #[test]
    fn an_ingress_path_without_a_service_name_is_refused_as_an_incomplete_backend() {
        let raw: RawIngress = serde_json::from_value(serde_json::json!({
            "metadata": { "name": "edge", "namespace": "app", "uid": "u1" },
            "spec": { "rules": [ { "host": "x.test", "http": { "paths": [
                { "path": "/", "backend": { "service": {} } }
            ] } } ] }
        }))
        .expect("the raw ingress parses");
        let mut errors = ValidationErrors::new();
        Ingress::from_raw(&raw, "i", &mut errors);
        assert!(
            errors.contains(InfraCode::IncompleteBackend),
            "expected INFRA-INGRESS-001, got: {errors}"
        );
    }

    #[test]
    fn defaults_fill_in_what_the_api_would_have_cluster_ip_and_tcp_and_port_equal_target() {
        let raw: RawService = serde_json::from_value(serde_json::json!({
            "metadata": { "name": "web", "namespace": "app", "uid": "u1" },
            "spec": { "ports": [ { "port": 80 } ] }
        }))
        .expect("the raw service parses");
        let mut errors = ValidationErrors::new();
        let service = Service::from_raw(&raw, "s", &mut errors).expect("valid");
        assert!(errors.is_empty(), "nothing to refuse: {errors}");
        assert_eq!(service.service_type, "ClusterIP");
        assert_eq!(service.ports[0].protocol, "TCP");
        assert_eq!(service.ports[0].target_port, "80");
    }
}
