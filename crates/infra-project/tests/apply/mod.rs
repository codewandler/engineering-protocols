//! A merge-patch applier, written here and nowhere else.
//!
//! # Why this lives in the tests
//!
//! Applying a patch is the *acting* half, and nothing in this workspace acts on a cluster. What
//! the library owes is that its patches are correct; proving that needs an applier, and an
//! applier that shipped would be the first half of the verb this repository refuses. So it is
//! here, it is small, and it takes no dependency — the workspace's rule is to prefer no crate and
//! record the refusal, and a JSON merge patch is thirty lines.
//!
//! # What it implements
//!
//! RFC 7386 in full — every key replaces what it names, `null` deletes — plus the **one** rule
//! Kubernetes' strategic merge adds that this crate's patches rely on: a list at a declared path
//! merges by its `name` key instead of replacing. The path list is a constant, not a heuristic:
//! a general strategic-merge implementation would need the API server's own struct tags, and one
//! that guessed would prove less than this one does.
//!
//! Being narrower than the real thing is what makes it a test: a patch this applier gets wrong is
//! a patch `kubectl` would also get wrong, because the only paths it treats specially are the ones
//! the projection declares strategic.

#![allow(dead_code)]

use serde_json::Value;

/// The one list this crate's patches reach into, and the key the API server merges it by.
pub const KEYED_LISTS: &[(&str, &str)] = &[("spec.template.spec.containers", "name")];

/// Applies a patch document to a target document.
///
/// `keyed` is empty for an RFC 7386 merge patch and [`KEYED_LISTS`] for a strategic one — the
/// distinction the emitted filename carries, honoured here rather than assumed away.
pub fn apply(target: &mut Value, patch: &Value, keyed: &[(&str, &str)]) {
    merge(target, patch, "", keyed);
}

/// The recursion, tracking the dotted path so a keyed list is recognised by where it is.
fn merge(target: &mut Value, patch: &Value, at: &str, keyed: &[(&str, &str)]) {
    let Some(fields) = patch.as_object() else {
        *target = patch.clone();
        return;
    };
    if !target.is_object() {
        *target = Value::Object(serde_json::Map::new());
    }
    for (key, value) in fields {
        let here = if at.is_empty() {
            key.clone()
        } else {
            format!("{at}.{key}")
        };
        let map = target.as_object_mut().expect("just made an object");
        if value.is_null() {
            map.remove(key);
            continue;
        }
        if let Some((_, merge_key)) = keyed.iter().find(|(at, _)| *at == here) {
            if let (Some(existing), Some(incoming)) = (
                map.get_mut(key).and_then(Value::as_array_mut),
                value.as_array(),
            ) {
                merge_keyed_list(existing, incoming, merge_key, &here, keyed);
                continue;
            }
        }
        let slot = map.entry(key.clone()).or_insert(Value::Null);
        merge(slot, value, &here, keyed);
    }
}

/// Merges a list by its declared key: an entry whose key matches is merged into, a new one is
/// appended, and nothing is ever dropped.
fn merge_keyed_list(
    existing: &mut Vec<Value>,
    incoming: &[Value],
    merge_key: &str,
    at: &str,
    keyed: &[(&str, &str)],
) {
    for entry in incoming {
        let name = entry.get(merge_key).cloned().unwrap_or(Value::Null);
        match existing
            .iter_mut()
            .find(|found| found.get(merge_key) == Some(&name))
        {
            Some(found) => merge(found, entry, at, keyed),
            None => existing.push(entry.clone()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_plain_merge_patch_replaces_a_list_whole_which_is_why_a_container_change_is_not_one() {
        let mut target = serde_json::json!({"containers": [{"name": "a"}, {"name": "b"}]});
        apply(
            &mut target,
            &serde_json::json!({"containers": [{"name": "a", "image": "x"}]}),
            &[],
        );
        assert_eq!(
            target["containers"].as_array().expect("a list").len(),
            1,
            "RFC 7386 replaces; `b` is gone, which is exactly the deletion the strategic type \
             exists to avoid"
        );
    }

    #[test]
    fn a_keyed_list_merges_the_entry_it_names_and_leaves_the_rest_alone() {
        let mut target = serde_json::json!({
            "spec": {"template": {"spec": {"containers": [
                {"name": "a", "image": "one"}, {"name": "b", "image": "two"}
            ]}}}
        });
        apply(
            &mut target,
            &serde_json::json!({
                "spec": {"template": {"spec": {"containers": [{"name": "b", "image": "three"}]}}}
            }),
            KEYED_LISTS,
        );
        let containers = target["spec"]["template"]["spec"]["containers"]
            .as_array()
            .expect("a list");
        assert_eq!(containers.len(), 2, "nothing is dropped by a keyed merge");
        assert_eq!(containers[0]["image"], "one");
        assert_eq!(containers[1]["image"], "three");
    }

    #[test]
    fn a_null_deletes_the_key_it_names_as_rfc_7386_says() {
        let mut target = serde_json::json!({"a": 1, "b": 2});
        apply(&mut target, &serde_json::json!({"b": null}), &[]);
        assert_eq!(target, serde_json::json!({"a": 1}));
    }
}
