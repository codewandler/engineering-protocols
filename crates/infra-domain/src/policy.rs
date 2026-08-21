//! Operational policy objects: pod disruption budgets and horizontal pod autoscalers.
//!
//! Neither runs anything; both make claims *about* workloads — how many pods may be down, how
//! many replicas the target may swing between — which is exactly the material IW2.5's
//! properties (coverage) and diagnosis (a budget guarding nothing, an autoscaler pinned or
//! aimed at nothing) read. Like the controllers, both kinds are **optional in the bundle**:
//! a scan that predates them still validates, and absence stays absence
//! ([`crate::observation::OPTIONAL_KINDS`]).

use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::Value;

use crate::code::{InfraCode, ValidationErrors};
use crate::observation::{identity, string_map, value_kind, Identity};
use crate::raw::{RawHorizontalPodAutoscaler, RawPodDisruptionBudget};

/// Renders an int-or-percentage budget bound (`1`, `"50%"`) as its string form.
///
/// Kept as the API's spelling rather than normalized, for the quantity-string reason
/// (`Resources`): `1` and `"1"` may be one amount, but which was declared is what the digest
/// covers.
fn budget_bound(value: &Value, location: &str, errors: &mut ValidationErrors) -> Option<String> {
    match value {
        Value::Number(number) => Some(number.to_string()),
        Value::String(text) => Some(text.clone()),
        other => {
            errors.refuse(
                InfraCode::MalformedObject,
                location.to_owned(),
                format!(
                    "a disruption budget bound must be a count or a percentage, found {}",
                    value_kind(other)
                ),
            );
            None
        }
    }
}

/// A pod disruption budget: a selector and the availability floor or ceiling it declares.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PodDisruptionBudget {
    /// Identity.
    pub identity: Identity,
    /// Labels.
    pub labels: BTreeMap<String, String>,
    /// The pod selector's `matchLabels`.
    pub selector: BTreeMap<String, String>,
    /// The availability floor, a count or a percentage, as declared.
    pub min_available: Option<String>,
    /// The disruption ceiling, a count or a percentage, as declared.
    pub max_unavailable: Option<String>,
}

impl PodDisruptionBudget {
    pub(crate) fn from_raw(
        raw: &RawPodDisruptionBudget,
        location: &str,
        errors: &mut ValidationErrors,
    ) -> Option<Self> {
        let identity = identity(&raw.metadata, true, location, errors)?;
        let selector = raw
            .spec
            .selector
            .as_ref()
            .map(|selector| {
                string_map(
                    &selector.match_labels,
                    &format!("{location}.spec.selector.matchLabels"),
                    errors,
                )
            })
            .unwrap_or_default();
        Some(Self {
            identity,
            labels: string_map(
                &raw.metadata.labels,
                &format!("{location}.metadata.labels"),
                errors,
            ),
            selector,
            min_available: raw.spec.min_available.as_ref().and_then(|value| {
                budget_bound(value, &format!("{location}.spec.minAvailable"), errors)
            }),
            max_unavailable: raw.spec.max_unavailable.as_ref().and_then(|value| {
                budget_bound(value, &format!("{location}.spec.maxUnavailable"), errors)
            }),
        })
    }
}

/// What an autoscaler scales: the target's kind and name, in the autoscaler's namespace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ScaleTarget {
    /// The target's kind, such as `Deployment`.
    pub kind: String,
    /// The target's name.
    pub name: String,
}

/// A horizontal pod autoscaler: its target and the replica range it may move within.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HorizontalPodAutoscaler {
    /// Identity.
    pub identity: Identity,
    /// Labels.
    pub labels: BTreeMap<String, String>,
    /// What it scales.
    pub target: ScaleTarget,
    /// The lower bound; the API's absence means one, a resolution left to the consumer.
    pub min_replicas: Option<u32>,
    /// The upper bound.
    pub max_replicas: u32,
}

impl HorizontalPodAutoscaler {
    pub(crate) fn from_raw(
        raw: &RawHorizontalPodAutoscaler,
        location: &str,
        errors: &mut ValidationErrors,
    ) -> Option<Self> {
        let identity = identity(&raw.metadata, true, location, errors)?;
        let target = raw.spec.scale_target_ref.as_ref().and_then(|reference| {
            match (
                reference.kind.clone().filter(|kind| !kind.is_empty()),
                reference.name.clone().filter(|name| !name.is_empty()),
            ) {
                (Some(kind), Some(name)) => Some(ScaleTarget { kind, name }),
                _ => None,
            }
        });
        let Some(target) = target else {
            errors.refuse(
                InfraCode::MalformedObject,
                format!("{location}.spec.scaleTargetRef"),
                "an autoscaler without a target kind and name scales nothing the API would accept",
            );
            return None;
        };
        let Some(max_replicas) = raw.spec.max_replicas else {
            errors.refuse(
                InfraCode::MalformedObject,
                format!("{location}.spec.maxReplicas"),
                "an autoscaler without maxReplicas cannot exist on a live API server",
            );
            return None;
        };
        Some(Self {
            identity,
            labels: string_map(
                &raw.metadata.labels,
                &format!("{location}.metadata.labels"),
                errors,
            ),
            target,
            min_replicas: raw.spec.min_replicas,
            max_replicas,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_budget_keeps_a_numeric_and_a_percentage_bound_as_their_declared_spellings() {
        let raw: RawPodDisruptionBudget = serde_json::from_value(serde_json::json!({
            "metadata": { "name": "nats", "namespace": "app", "uid": "pdb-1" },
            "spec": {
                "selector": { "matchLabels": { "app": "nats" } },
                "minAvailable": "50%",
                "maxUnavailable": 1
            }
        }))
        .expect("the raw budget parses");
        let mut errors = ValidationErrors::new();
        let validated = PodDisruptionBudget::from_raw(&raw, "pdb", &mut errors).expect("validates");
        assert!(errors.is_empty(), "{errors}");
        assert_eq!(validated.min_available.as_deref(), Some("50%"));
        assert_eq!(validated.max_unavailable.as_deref(), Some("1"));
        assert_eq!(
            validated.selector.get("app").map(String::as_str),
            Some("nats")
        );
    }

    #[test]
    fn a_budget_bound_that_is_neither_count_nor_percentage_is_refused_with_its_location() {
        let raw: RawPodDisruptionBudget = serde_json::from_value(serde_json::json!({
            "metadata": { "name": "odd", "namespace": "app", "uid": "pdb-2" },
            "spec": { "minAvailable": { "count": 1 } }
        }))
        .expect("the raw budget parses");
        let mut errors = ValidationErrors::new();
        let validated = PodDisruptionBudget::from_raw(&raw, "pdb", &mut errors);
        assert!(
            validated.is_some_and(|budget| budget.min_available.is_none()),
            "the budget survives; the bound does not"
        );
        assert!(
            errors.contains(InfraCode::MalformedObject),
            "expected INFRA-OBJECT-001, got: {errors}"
        );
    }

    #[test]
    fn an_autoscaler_without_a_target_or_without_max_replicas_is_refused() {
        for spec in [
            serde_json::json!({ "maxReplicas": 4 }),
            serde_json::json!({ "scaleTargetRef": { "kind": "Deployment", "name": "web" } }),
        ] {
            let raw: RawHorizontalPodAutoscaler = serde_json::from_value(serde_json::json!({
                "metadata": { "name": "broken", "namespace": "app", "uid": "hpa-1" },
                "spec": spec
            }))
            .expect("the raw autoscaler parses");
            let mut errors = ValidationErrors::new();
            assert!(HorizontalPodAutoscaler::from_raw(&raw, "hpa", &mut errors).is_none());
            assert!(
                errors.contains(InfraCode::MalformedObject),
                "expected INFRA-OBJECT-001, got: {errors}"
            );
        }
    }

    #[test]
    fn a_complete_autoscaler_validates_with_its_range_and_target() {
        let raw: RawHorizontalPodAutoscaler = serde_json::from_value(serde_json::json!({
            "metadata": { "name": "web", "namespace": "app", "uid": "hpa-2" },
            "spec": {
                "scaleTargetRef": { "kind": "Deployment", "name": "web" },
                "minReplicas": 2, "maxReplicas": 6
            }
        }))
        .expect("the raw autoscaler parses");
        let mut errors = ValidationErrors::new();
        let validated =
            HorizontalPodAutoscaler::from_raw(&raw, "hpa", &mut errors).expect("validates");
        assert!(errors.is_empty(), "{errors}");
        assert_eq!(validated.target.kind, "Deployment");
        assert_eq!(validated.min_replicas, Some(2));
        assert_eq!(validated.max_replicas, 6);
    }
}
