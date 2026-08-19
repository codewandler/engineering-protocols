//! Canonical serialisation.
//!
//! Generated files are compared byte-for-byte in CI, so they must be produced deterministically:
//! two-space indentation, object keys in a stable order (`serde_json` preserves the order
//! `schemars` emits, which is itself deterministic), and exactly one trailing newline.

use serde::Serialize;

/// Serialises `value` as pretty-printed JSON with a trailing newline.
pub fn canonical_json<T: Serialize>(value: &T) -> Result<String, serde_json::Error> {
    let mut rendered = serde_json::to_string_pretty(value)?;
    rendered.push('\n');
    Ok(rendered)
}
