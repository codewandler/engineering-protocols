//! What a projection writes: a patch against an object the scan observed, or a manifest for an
//! object it did not.
//!
//! # Two patch types, decided by the field and not by taste
//!
//! A merge patch (RFC 7386) replaces whatever it names. That is exactly right for
//! `spec.replicas`, and exactly wrong for `spec.template.spec.containers`: a merge patch naming
//! one container **replaces the whole list**, deleting every container it does not mention. So a
//! change that reaches inside a container is emitted as a *strategic* merge patch, whose
//! `containers` list merges by the `name` key the way the API server merges it.
//!
//! | change | type | why |
//! |---|---|---|
//! | `spec.replicas` | [`Merge`](PatchType::Merge) | a scalar; nothing to lose |
//! | a container's `resources` | [`Strategic`](PatchType::Strategic) | the list is keyed by `name` |
//! | a container's probes | [`Strategic`](PatchType::Strategic) | the same list |
//!
//! A file's type is the **join** over the changes in it: one object with a replica change and a
//! container change is one strategic patch, because strategic merge does everything merge does
//! for the fields merge would have handled. There is never a merge patch and a strategic patch
//! for the same object — two files against one object is two things to apply in an order nobody
//! wrote down.
//!
//! # The file is the patch, and nothing else
//!
//! A patch file's bytes are the patch document: no header, no provenance comment, no wrapper.
//! `kubectl patch <kind> <name> -n <namespace> --type=<type> --patch-file <file>` is meant to
//! work on the file as committed, and a wrapper would make every consumer unwrap it first. What
//! the file *is* — which object, which type, which gap it closes — is in `SUMMARY.md` and in the
//! projection document, where a reviewer reads it once for the whole tree.

use std::collections::BTreeMap;

use infra_domain::workload::WorkloadKind;
use serde::Serialize;
use serde_json::{Map, Value};

/// How a patch document is meant to be applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PatchType {
    /// RFC 7386: every key replaces what it names.
    Merge,
    /// Kubernetes' strategic merge: lists with a declared merge key merge by it.
    Strategic,
}

impl PatchType {
    /// The wire discriminant, which is also the filename's type marker.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Merge => "merge",
            Self::Strategic => "strategic",
        }
    }

    /// The spelling `kubectl patch --type` takes.
    pub fn kubectl_type(self) -> &'static str {
        match self {
            Self::Merge => "merge",
            Self::Strategic => "strategic",
        }
    }

    /// The weaker of the two, which is the type a file holding both kinds of change needs.
    #[must_use]
    pub fn join(self, other: Self) -> Self {
        match (self, other) {
            (Self::Merge, Self::Merge) => Self::Merge,
            _ => Self::Strategic,
        }
    }
}

impl std::fmt::Display for PatchType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Which object a patch or a manifest is about, in the API's own spellings.
///
/// The IR keys objects as `namespace/kind/name` with a lowercase kind; a manifest spells the same
/// object `apps/v1` + `Deployment`. Both are here because both are needed — the first names the
/// file, the second is what a reader pastes into a command.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct ObjectRef {
    /// The API group and version, such as `apps/v1`.
    pub api_version: String,
    /// The API kind, such as `Deployment`.
    pub kind: String,
    /// The namespace.
    pub namespace: String,
    /// The name.
    pub name: String,
}

impl ObjectRef {
    /// The reference to a workload the IR holds.
    pub fn workload(kind: WorkloadKind, namespace: &str, name: &str) -> Self {
        Self {
            api_version: "apps/v1".to_owned(),
            kind: match kind {
                WorkloadKind::Deployment => "Deployment",
                WorkloadKind::StatefulSet => "StatefulSet",
                WorkloadKind::DaemonSet => "DaemonSet",
            }
            .to_owned(),
            namespace: namespace.to_owned(),
            name: name.to_owned(),
        }
    }

    /// The reference to a disruption budget.
    pub fn disruption_budget(namespace: &str, name: &str) -> Self {
        Self {
            api_version: "policy/v1".to_owned(),
            kind: "PodDisruptionBudget".to_owned(),
            namespace: namespace.to_owned(),
            name: name.to_owned(),
        }
    }

    /// The filename stem this object's artifact is written under: `shop.deployment.storefront`.
    ///
    /// Injective over the objects a projection can reach, which is what makes it safe as a
    /// filename. A namespace is a DNS-1123 *label* and cannot contain a dot, and the kind comes
    /// from a closed set this crate writes — so the first dot ends the namespace, the second ends
    /// the kind, and everything after it is the name, dots and all.
    pub fn slug(&self) -> String {
        format!(
            "{}.{}.{}",
            self.namespace,
            self.kind.to_lowercase(),
            self.name
        )
    }

    /// The command a reader would run to apply a patch of this type to this object.
    ///
    /// A sentence in a document, never something this workspace runs: nothing here reaches a
    /// cluster, and naming the command is the opposite of taking it — a reviewer has to be able
    /// to see what the patch would do before anybody does it.
    pub fn kubectl_patch(&self, patch_type: PatchType, file: &str) -> String {
        format!(
            "kubectl patch {} {} -n {} --type={} --patch-file {file}",
            self.kind.to_lowercase(),
            self.name,
            self.namespace,
            patch_type.kubectl_type()
        )
    }
}

impl std::fmt::Display for ObjectRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}/{}", self.namespace, self.kind, self.name)
    }
}

/// One object's patch: everything this projection would change about it, in one document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ObjectPatch {
    /// Which object.
    pub target: ObjectRef,
    /// How it is meant to be applied.
    pub patch_type: PatchType,
    /// The path the tree writes it to, relative to the projection root.
    pub path: String,
    /// The patch document itself.
    pub patch: Value,
}

/// An object no scan observed, written in full because there is nothing to patch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NewObject {
    /// Which object.
    pub target: ObjectRef,
    /// The path the tree writes it to, relative to the projection root.
    pub path: String,
    /// The manifest.
    ///
    /// Carries **no `uid`, no `resourceVersion` and no status** — a manifest is what a person
    /// commits, and every one of those is something the API server assigns. That is the one place
    /// this document deliberately differs from the observed objects it sits beside, which all
    /// carry a uid because they were observed.
    pub manifest: Value,
}

/// A patch under construction: the top-level `spec` fields, and the per-container ones.
///
/// Containers accumulate in a [`BTreeMap`] keyed by name rather than in a list: two changes to one
/// container must land in one list entry, and the map's order is the emitted order (invariant 9).
/// Strategic merge does not care about list order, so sorting by name costs nothing and buys
/// byte-stability.
#[derive(Debug, Default)]
pub(crate) struct PatchDraft {
    /// Fields under `spec` that are not inside the pod template.
    spec: Map<String, Value>,
    /// Per-container patches, keyed by container name.
    containers: BTreeMap<String, Map<String, Value>>,
}

impl PatchDraft {
    /// Sets a field directly under `spec`.
    pub(crate) fn set_spec(&mut self, field: &str, value: Value) {
        self.spec.insert(field.to_owned(), value);
    }

    /// Sets a field on one container of the pod template.
    pub(crate) fn set_container(&mut self, container: &str, field: &str, value: Value) {
        self.containers
            .entry(container.to_owned())
            .or_default()
            .insert(field.to_owned(), value);
    }

    /// The type this draft needs: the [`join`](PatchType::join) over the changes in it.
    ///
    /// A field under `spec` is carried by a merge patch; a field inside a container is not, and
    /// the file needs whichever of the two carries both.
    pub(crate) fn patch_type(&self) -> PatchType {
        let mut joined = PatchType::Merge;
        if !self.spec.is_empty() {
            joined = joined.join(PatchType::Merge);
        }
        if !self.containers.is_empty() {
            joined = joined.join(PatchType::Strategic);
        }
        joined
    }

    /// `true` when nothing has been set, which is a draft that must not become a file.
    pub(crate) fn is_empty(&self) -> bool {
        self.spec.is_empty() && self.containers.is_empty()
    }

    /// The patch document.
    pub(crate) fn document(&self) -> Value {
        let mut spec = self.spec.clone();
        if !self.containers.is_empty() {
            let containers: Vec<Value> = self
                .containers
                .iter()
                .map(|(name, fields)| {
                    let mut entry = fields.clone();
                    // The merge key, and the reason this is a strategic patch at all: without it
                    // the API server has nothing to match the list entry against.
                    entry.insert("name".to_owned(), Value::String(name.clone()));
                    Value::Object(entry)
                })
                .collect();
            spec.insert(
                "template".to_owned(),
                serde_json::json!({"spec": {"containers": containers}}),
            );
        }
        serde_json::json!({ "spec": Value::Object(spec) })
    }
}

/// A JSON document as this crate commits one: key-sorted, indented, one trailing newline.
///
/// Through [`serde_json::Value`] for the reason every other document in the infrastructure family
/// goes through one — `serde_json::Map` is a `BTreeMap` unless the `preserve_order` feature is on,
/// which nothing here enables, so the bytes are a function of the content and not of any struct's
/// field order.
pub(crate) fn canonical_json(value: &Value) -> String {
    let mut rendered = serde_json::to_string_pretty(value).expect("a JSON value renders as JSON");
    rendered.push('\n');
    rendered
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_holding_a_container_change_is_strategic_however_it_was_reached() {
        let mut draft = PatchDraft::default();
        draft.set_spec("replicas", Value::from(2));
        assert_eq!(
            draft.patch_type(),
            PatchType::Merge,
            "a scalar field needs nothing stronger"
        );
        draft.set_container("agent", "resources", serde_json::json!({"limits": {}}));
        assert_eq!(
            draft.patch_type(),
            PatchType::Strategic,
            "a merge patch naming one container would delete every container it does not name"
        );
    }

    #[test]
    fn the_container_list_carries_the_merge_key_it_is_matched_by() {
        let mut draft = PatchDraft::default();
        draft.set_container("redis", "livenessProbe", serde_json::json!({"exec": null}));
        let document = draft.document();
        let containers = &document["spec"]["template"]["spec"]["containers"];
        assert_eq!(
            containers[0]["name"], "redis",
            "without `name` the API server has nothing to merge the entry against: {document}"
        );
    }

    #[test]
    fn the_join_of_two_types_is_the_one_that_can_carry_both() {
        assert_eq!(
            PatchType::Merge.join(PatchType::Merge),
            PatchType::Merge,
            "two scalar changes need no list semantics"
        );
        for pair in [
            (PatchType::Merge, PatchType::Strategic),
            (PatchType::Strategic, PatchType::Merge),
            (PatchType::Strategic, PatchType::Strategic),
        ] {
            assert_eq!(pair.0.join(pair.1), PatchType::Strategic);
        }
    }

    #[test]
    fn a_slug_is_read_back_as_namespace_kind_and_name_even_when_the_name_holds_dots() {
        let reference = ObjectRef::workload(WorkloadKind::StatefulSet, "shop", "switch.board.8");
        let slug = reference.slug();
        assert_eq!(slug, "shop.statefulset.switch.board.8");
        let mut parts = slug.splitn(3, '.');
        assert_eq!(parts.next(), Some("shop"));
        assert_eq!(parts.next(), Some("statefulset"));
        assert_eq!(
            parts.next(),
            Some("switch.board.8"),
            "a namespace cannot hold a dot and the kind is a closed set, so the first two dots \
             are the separators"
        );
    }
}
