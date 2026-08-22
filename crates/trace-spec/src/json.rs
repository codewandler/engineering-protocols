//! The JSON shapes both adapters read, in one place.
//!
//! Two adapters now lift a transcript into `trace-ir/1` — the Claude Code `stream-json` reader in
//! [`crate::adapter`] and the metaharness event-stream reader in [`crate::event_stream`] — and
//! they meet the same shapes: a string field that may be recorded as `null`, a list of names whose
//! entries may be bare strings or objects, a plugin list, an MCP server list.
//!
//! They live here rather than once per adapter because the alternative is two definitions of *what
//! a name list is*, drifting apart silently: a fix applied to one reader and not the other would
//! show up as two adapters disagreeing about the same run, which is the one thing a
//! harness-neutral IR exists to prevent. What stays in an adapter is the part that is genuinely
//! harness-specific — which key carries which fact, and what an absent key means for **that**
//! harness.
//!
//! Nothing here refuses anything. A shape these functions cannot read yields [`None`], which
//! becomes an `unk` verdict rather than a wrong answer.

use serde_json::Value;
use trace_domain::ir::{LoadedPlugin, McpServer};

/// A borrowed string field, where the object records one as a string.
pub(crate) fn str_at<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key)?.as_str()
}

/// An owned string field. A recorded `null` is absence, which is what `unk` is made of.
///
/// Load-bearing for the event stream in particular: metaharness writes every absent payload field
/// as an explicit `null` rather than omitting the key, so *every* optional field arrives as a
/// present key with a null value and only this rule keeps absence distinguishable from a value.
pub(crate) fn text_at(value: &Value, key: &str) -> Option<String> {
    str_at(value, key).map(ToOwned::to_owned)
}

/// An unsigned integer field.
pub(crate) fn u64_at(value: &Value, key: &str) -> Option<u64> {
    value.get(key)?.as_u64()
}

/// A signed integer field. Deltas move both ways.
pub(crate) fn i64_at(value: &Value, key: &str) -> Option<i64> {
    value.get(key)?.as_i64()
}

/// The length of a recorded list, where the list is there at all.
///
/// An absent list is [`None`] — the harness did not say — while an empty list is `Some(0)`, which
/// is the harness saying *none*. Absence is not zero, and the two must stay distinguishable all
/// the way to the verdict.
pub(crate) fn count_at(value: &Value, key: &str) -> Option<u64> {
    let entries = value.get(key)?.as_array()?;
    u64::try_from(entries.len()).ok()
}

/// A list of names, where each entry may be a bare name or an object carrying one.
///
/// Claude Code at `2.1.238` writes `tools`, `skills` and `agents` as arrays of strings and
/// `plugins` as an array of objects; metaharness writes `offered_tools`, `skills` and `agents` as
/// arrays of strings. Both are read, because the difference between them is one release: an entry
/// that is neither keeps its compact JSON as its name rather than disappearing, so a list whose
/// *length* somebody asserts on cannot silently shrink.
pub(crate) fn names_at(value: &Value, key: &str) -> Option<Vec<String>> {
    let entries = value.get(key)?.as_array()?;
    Some(entries.iter().map(name_of).collect())
}

/// One entry's name, by the rule [`names_at`] documents.
pub(crate) fn name_of(entry: &Value) -> String {
    if let Some(name) = entry.as_str() {
        return name.to_owned();
    }
    if let Some(name) = entry.get("name").and_then(Value::as_str) {
        return name.to_owned();
    }
    compact(entry)
}

/// The plugins the harness loaded, each with whatever it recorded about it.
///
/// A field the wire does not carry stays [`None`] rather than becoming a default: metaharness's
/// plugin record has no `path`, and an empty string there would answer a question nobody observed.
pub(crate) fn plugins_at(value: &Value) -> Option<Vec<LoadedPlugin>> {
    let entries = value.get("plugins")?.as_array()?;
    Some(
        entries
            .iter()
            .map(|entry| LoadedPlugin {
                name: name_of(entry),
                version: text_at(entry, "version"),
                source: text_at(entry, "source"),
                path: text_at(entry, "path"),
            })
            .collect(),
    )
}

/// The MCP servers the session was given, each with the status the harness reported.
///
/// Absent and empty stay different all the way down: no `mcp_servers` key yields [`None`] and an
/// empty array yields `Some(vec![])`, because `env.mcp_servers` reads the first as *undecidable*
/// and the second as *hermetic*. An entry that is a bare string keeps its name and answers nothing
/// about status, which is [`name_of`]'s rule applied one level out.
pub(crate) fn mcp_servers_at(value: &Value) -> Option<Vec<McpServer>> {
    let entries = value.get("mcp_servers")?.as_array()?;
    Some(
        entries
            .iter()
            .map(|entry| McpServer {
                name: name_of(entry),
                status: text_at(entry, "status"),
            })
            .collect(),
    )
}

/// Compact JSON of a value, which is what both byte measures are taken over.
pub(crate) fn compact(value: &Value) -> String {
    serde_json::to_string(value).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_recorded_null_is_absence_rather_than_a_value() {
        // metaharness writes every absent field as an explicit `null`, so this is the rule the
        // whole event-stream reader rests on: a present key with a null value must read the same
        // as a key that is not there.
        let written = serde_json::json!({"model": null, "num_turns": null, "mcp_servers": null});
        assert_eq!(text_at(&written, "model"), None);
        assert_eq!(u64_at(&written, "num_turns"), None);
        assert_eq!(mcp_servers_at(&written), None);
    }

    #[test]
    fn an_empty_list_is_zero_and_an_absent_list_is_unknown() {
        let stated = serde_json::json!({"mcp_servers": [], "permission_denials": []});
        assert_eq!(mcp_servers_at(&stated), Some(Vec::new()), "it says none");
        assert_eq!(count_at(&stated, "permission_denials"), Some(0));
        let silent = serde_json::json!({});
        assert_eq!(mcp_servers_at(&silent), None, "it does not say");
        assert_eq!(count_at(&silent, "permission_denials"), None);
    }

    #[test]
    fn an_entry_no_reader_can_name_keeps_its_json_rather_than_disappearing() {
        let listed = serde_json::json!({"skills": ["a", {"name": "b"}, {"unnamed": true}]});
        assert_eq!(
            names_at(&listed, "skills"),
            Some(vec![
                "a".to_owned(),
                "b".to_owned(),
                r#"{"unnamed":true}"#.to_owned()
            ]),
            "a list whose length somebody asserts on must not silently shrink"
        );
    }
}
