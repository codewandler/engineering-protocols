//! Controllers between a workload and its pods: replicasets, jobs and cronjobs.
//!
//! IW2 derived a deployment pod's owner through the `pod-template-hash` label because the
//! scanner did not collect replicasets. It does now, so the chain the API actually declares —
//! pod → replicaset → deployment, pod → job → cronjob — is observable, and the hash heuristic
//! becomes a fallback for bundles older than these kinds. The model keeps only what ownership
//! and diagnosis read: identity, the controller owner reference, replica/completion counts and
//! a cronjob's schedule and suspension. Rollout mechanics (`backoffLimit`, concurrency policies,
//! history limits) stay excluded, per the exclusion classes in the crate doc.
//!
//! All three kinds are **optional in the bundle**: a scan that predates them still validates,
//! and their absence is carried as absence (`Option`), never rewritten into "none exist" — see
//! [`crate::observation::OPTIONAL_KINDS`].

use std::collections::BTreeMap;

use serde::Serialize;

use crate::code::{InfraCode, ValidationErrors};
use crate::observation::{identity, string_map, Identity, OwnerRef};
use crate::raw::{RawCronJob, RawJob, RawMeta, RawReplicaSet};

/// The managing controller declared in a `metadata.ownerReferences` list, when there is one.
fn controller_owner(meta: &RawMeta) -> Option<OwnerRef> {
    meta.owner_references
        .iter()
        .find(|reference| reference.controller)
        .map(|reference| OwnerRef {
            kind: reference.kind.clone(),
            name: reference.name.clone(),
        })
}

/// A replicaset: the rung the API puts between a deployment and its pods.
///
/// Its `pod-template-hash` label rides in [`Self::labels`]; nothing here re-derives from it,
/// because with the replicaset observed the owner reference *is* the evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReplicaSet {
    /// Identity.
    pub identity: Identity,
    /// Labels, the generated `pod-template-hash` among them.
    pub labels: BTreeMap<String, String>,
    /// The managing controller — the deployment, when the replicaset has one.
    pub owner: Option<OwnerRef>,
    /// Desired replicas.
    pub replicas: Option<u32>,
}

impl ReplicaSet {
    pub(crate) fn from_raw(
        raw: &RawReplicaSet,
        location: &str,
        errors: &mut ValidationErrors,
    ) -> Option<Self> {
        let identity = identity(&raw.metadata, true, location, errors)?;
        Some(Self {
            identity,
            labels: string_map(
                &raw.metadata.labels,
                &format!("{location}.metadata.labels"),
                errors,
            ),
            owner: controller_owner(&raw.metadata),
            replicas: raw.spec.replicas,
        })
    }
}

/// A job: a run-to-completion workload, reduced to its completion arithmetic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Job {
    /// Identity.
    pub identity: Identity,
    /// Labels.
    pub labels: BTreeMap<String, String>,
    /// The managing controller — a cronjob, when the job was spawned by one.
    pub owner: Option<OwnerRef>,
    /// How many completions the job wants; the API's absence means one, a resolution left to
    /// the consumer so the model records what the document said.
    pub completions: Option<u32>,
    /// Pods observed `Succeeded`.
    pub succeeded: u32,
    /// Pods observed `Failed`.
    pub failed: u32,
}

impl Job {
    pub(crate) fn from_raw(
        raw: &RawJob,
        location: &str,
        errors: &mut ValidationErrors,
    ) -> Option<Self> {
        let identity = identity(&raw.metadata, true, location, errors)?;
        Some(Self {
            identity,
            labels: string_map(
                &raw.metadata.labels,
                &format!("{location}.metadata.labels"),
                errors,
            ),
            owner: controller_owner(&raw.metadata),
            completions: raw.spec.completions,
            succeeded: raw.status.succeeded,
            failed: raw.status.failed,
        })
    }
}

/// A cronjob: a schedule that spawns jobs, and whether it is currently told not to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CronJob {
    /// Identity.
    pub identity: Identity,
    /// Labels.
    pub labels: BTreeMap<String, String>,
    /// The cron schedule expression, verbatim.
    pub schedule: String,
    /// Whether the controller is told not to start new jobs.
    pub suspend: bool,
}

impl CronJob {
    pub(crate) fn from_raw(
        raw: &RawCronJob,
        location: &str,
        errors: &mut ValidationErrors,
    ) -> Option<Self> {
        let identity = identity(&raw.metadata, true, location, errors)?;
        let Some(schedule) = raw
            .spec
            .schedule
            .clone()
            .filter(|schedule| !schedule.is_empty())
        else {
            errors.refuse(
                InfraCode::MalformedObject,
                format!("{location}.spec.schedule"),
                "a cronjob without a schedule cannot exist on a live API server",
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
            schedule,
            suspend: raw.spec.suspend,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_replicaset_keeps_its_controller_owner_and_ignores_non_controller_references() {
        let raw: RawReplicaSet = serde_json::from_value(serde_json::json!({
            "metadata": {
                "name": "web-abc", "namespace": "app", "uid": "rs-1",
                "labels": { "pod-template-hash": "abc" },
                "ownerReferences": [
                    { "kind": "HelmChart", "name": "web-chart", "controller": false },
                    { "kind": "Deployment", "name": "web", "controller": true }
                ]
            },
            "spec": { "replicas": 3 }
        }))
        .expect("the raw replicaset parses");
        let mut errors = ValidationErrors::new();
        let validated = ReplicaSet::from_raw(&raw, "rs", &mut errors).expect("validates");
        assert!(errors.is_empty(), "{errors}");
        assert_eq!(
            validated.owner,
            Some(OwnerRef {
                kind: "Deployment".to_owned(),
                name: "web".to_owned()
            }),
            "only the controller reference is the owner"
        );
        assert_eq!(validated.replicas, Some(3));
        assert_eq!(
            validated
                .labels
                .get("pod-template-hash")
                .map(String::as_str),
            Some("abc")
        );
    }

    #[test]
    fn a_job_carries_its_completion_arithmetic_and_defaults_absent_counts_to_zero() {
        let raw: RawJob = serde_json::from_value(serde_json::json!({
            "metadata": { "name": "reindex-1", "namespace": "app", "uid": "j-1",
                          "ownerReferences": [
                              { "kind": "CronJob", "name": "reindex", "controller": true } ] },
            "spec": { "completions": 1 },
            "status": { "failed": 3 }
        }))
        .expect("the raw job parses");
        let mut errors = ValidationErrors::new();
        let validated = Job::from_raw(&raw, "j", &mut errors).expect("validates");
        assert!(errors.is_empty(), "{errors}");
        assert_eq!(validated.completions, Some(1));
        assert_eq!(validated.succeeded, 0, "unreported means none observed");
        assert_eq!(validated.failed, 3);
        assert_eq!(
            validated.owner.as_ref().map(|o| o.kind.as_str()),
            Some("CronJob")
        );
    }

    #[test]
    fn a_cronjob_without_a_schedule_is_refused_because_the_api_cannot_serve_one() {
        let raw: RawCronJob = serde_json::from_value(serde_json::json!({
            "metadata": { "name": "broken", "namespace": "app", "uid": "cj-1" },
            "spec": { "suspend": true }
        }))
        .expect("the raw cronjob parses");
        let mut errors = ValidationErrors::new();
        assert!(CronJob::from_raw(&raw, "cj", &mut errors).is_none());
        assert!(
            errors.contains(InfraCode::MalformedObject),
            "expected INFRA-OBJECT-001, got: {errors}"
        );
    }
}
