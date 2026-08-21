//! The one hash construction this crate uses, in one place.
//!
//! SHA-256, rendered as 64 lowercase hex characters — the same construction and the same full
//! width as `ess-gen`'s specification digest and `infra-compiler`'s IR digest, which gap register
//! D-4 settled: the moment a digest becomes an acceptance criterion, 64 bits is fine against
//! drift and weak against construction.
//!
//! Two digests exist in this family and they are digests of different things, deliberately:
//!
//! | digest | over | why that and not the other |
//! |---|---|---|
//! | the **transcript** digest | the raw bytes of the transcript file, exactly as read | an adapter upgrade that starts understanding an event must not change the name of the run it judged (design § 2.9) |
//! | the **specification** digest | the canonical JSON of the *validated* [`TraceSpec`](crate::spec::TraceSpec) | a comment or a reordered key is not a different specification, and a report that said so would be noise in every diff |

use std::fmt::Write as _;

/// The digest of raw bytes: the full SHA-256, 64 lowercase hex characters.
///
/// What the transcript digest is computed over. Anyone holding the file can recompute it with
/// `sha256sum`, which is the property that makes a report checkable by a person who does not run
/// this code.
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
/// Through [`serde_json::Value`] first, deliberately, for the reason `infra-compiler` gives:
/// `Value` maps are ordered by key, so the canonical form is *key-sorted* compact JSON —
/// reproducible by anyone who parses the document, where direct struct serialization would bake
/// this crate's field declaration order into the digest and no reader could recompute it.
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
        assert_eq!(digest.len(), 64);
    }

    #[test]
    fn canonical_json_sorts_keys_so_two_spellings_of_one_value_digest_the_same() {
        // The same map written in two field orders. A digest that depended on declaration order
        // could not be recomputed by a reader holding only the document.
        let first: serde_json::Value = serde_json::json!({ "b": 1, "a": 2 });
        let second: serde_json::Value = serde_json::json!({ "a": 2, "b": 1 });
        assert_eq!(digest_of_canonical(&first), digest_of_canonical(&second));
    }
}
