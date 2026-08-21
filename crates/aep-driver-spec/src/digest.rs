//! The one hash construction this crate uses, in one place.
//!
//! SHA-256, rendered as 64 lowercase hex characters — the same construction, the same full width
//! and the same reasoning as `trace-domain`'s two digests and `infra-compiler`'s IR digest, which
//! gap register D-4 settled.
//!
//! There is one digest here and it is over the **validated** map's canonical JSON, not over the
//! file's bytes. A comment or a reordered key is not a different step map, and a cursor that
//! refused a resumed run because somebody rewrapped a description would be a refusal nobody could
//! act on.

use std::fmt::Write as _;

/// The digest of raw bytes: the full SHA-256, 64 lowercase hex characters.
#[must_use]
pub fn digest_of_bytes(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};

    let hash = Sha256::digest(bytes);
    let mut rendered = String::with_capacity(64);
    for byte in &hash {
        let _ = write!(rendered, "{byte:02x}");
    }
    rendered
}

/// The digest of a serializable value's canonical JSON.
///
/// Through [`serde_json::Value`] first, deliberately, for the reason `trace-domain` gives: `Value`
/// maps are ordered by key, so the canonical form is key-sorted compact JSON — reproducible by
/// anyone who parses the document, where direct struct serialization would bake this crate's field
/// declaration order into the digest.
///
/// # Panics
///
/// If the value cannot be serialized, which for the types in this crate cannot happen: none holds
/// a non-string map key or a non-finite float.
#[must_use]
pub fn digest_of_canonical<T: serde::Serialize>(value: &T) -> String {
    let value = serde_json::to_value(value).expect("the value has no non-serializable state");
    let canonical = serde_json::to_vec(&value).expect("a value serializes");
    digest_of_bytes(&canonical)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_digest_is_sixty_four_lowercase_hex_characters() {
        let digest = digest_of_bytes(b"");
        assert_eq!(
            digest, "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            "the empty input's SHA-256 is a published constant, so a changed construction shows up \
             here rather than as an unexplained digest change downstream"
        );
    }

    #[test]
    fn canonical_json_sorts_keys_so_field_order_cannot_reach_the_digest() {
        let first: serde_json::Value = serde_json::json!({"a": 1, "b": 2});
        let second: serde_json::Value = serde_json::json!({"b": 2, "a": 1});
        assert_eq!(digest_of_canonical(&first), digest_of_canonical(&second));
    }
}
