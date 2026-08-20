//! Configmaps and secrets: keys and digests, never values.
//!
//! Two rules meet here, and they are the two this crate exists to hold:
//!
//! * **A secret value never enters the model.** The scanner already replaced every value with
//!   `{sha256, length}` before writing the bundle; this module *refuses* a bundle where that did
//!   not happen ([`InfraCode::UnsanitizedSecret`]), so the guarantee does not depend on which
//!   scanner produced the file. Defense in depth: two independent mechanisms, either sufficient.
//! * **A configmap value never enters the model either** — it is hashed at validation into the
//!   same `{sha256, length}` shape. Not because configuration is secret, but because the IR is a
//!   function of semantic cluster state: what IW2–IW4 ask is "which keys exist and did a value
//!   change", and a digest answers both without making the IR a copy of every `Corefile` in the
//!   cluster.

use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::Value;

use crate::code::{InfraCode, ValidationErrors};
use crate::observation::{identity, string_map, value_kind, Identity};
use crate::raw::{RawConfigMap, RawSecret};

/// A value's digest: enough to detect change, nothing to recover content from.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct ValueDigest {
    /// SHA-256 of the value, 64 lowercase hex characters.
    pub sha256: String,
    /// The value's length in bytes.
    pub length: u64,
}

/// A configmap: its keys, each with the digest of the value it held at scan time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConfigMap {
    /// Identity.
    pub identity: Identity,
    /// Labels.
    pub labels: BTreeMap<String, String>,
    /// Every key, from `data` and `binaryData` together. A `binaryData` value is digested over
    /// its base64 rendering as the bundle carries it, which is stable for an unchanged value —
    /// all a digest is asked to be.
    pub keys: BTreeMap<String, ValueDigest>,
}

/// A secret: its keys, each with the digest the scanner recorded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Secret {
    /// Identity.
    pub identity: Identity,
    /// Labels.
    pub labels: BTreeMap<String, String>,
    /// The secret's type, such as `Opaque`.
    pub secret_type: String,
    /// Every key, from `data` and `stringData` together.
    pub keys: BTreeMap<String, ValueDigest>,
}

impl ConfigMap {
    pub(crate) fn from_raw(
        raw: &RawConfigMap,
        location: &str,
        errors: &mut ValidationErrors,
    ) -> Option<Self> {
        let identity = identity(&raw.metadata, true, location, errors)?;
        let mut keys = BTreeMap::new();
        for (field, entries) in [("data", &raw.data), ("binaryData", &raw.binary_data)] {
            for (key, value) in entries {
                match value.as_str() {
                    Some(text) => {
                        keys.insert(key.clone(), digest_of(text));
                    }
                    None => errors.refuse(
                        InfraCode::MalformedObject,
                        format!("{location}.{field}.{key}"),
                        format!(
                            "a configmap value must be a string, found {}",
                            value_kind(value)
                        ),
                    ),
                }
            }
        }
        Some(Self {
            identity,
            labels: string_map(
                &raw.metadata.labels,
                &format!("{location}.metadata.labels"),
                errors,
            ),
            keys,
        })
    }
}

impl Secret {
    pub(crate) fn from_raw(
        raw: &RawSecret,
        location: &str,
        errors: &mut ValidationErrors,
    ) -> Option<Self> {
        let identity = identity(&raw.metadata, true, location, errors)?;
        let mut keys = BTreeMap::new();
        for (field, entries) in [("data", &raw.data), ("stringData", &raw.string_data)] {
            for (key, value) in entries {
                if let Some(digest) =
                    sanitized_digest(value, &format!("{location}.{field}.{key}"), errors)
                {
                    keys.insert(key.clone(), digest);
                }
            }
        }
        Some(Self {
            identity,
            labels: string_map(
                &raw.metadata.labels,
                &format!("{location}.metadata.labels"),
                errors,
            ),
            secret_type: raw
                .secret_type
                .clone()
                .unwrap_or_else(|| "Opaque".to_owned()),
            keys,
        })
    }
}

/// Checks that a secret's value is a well-formed `{sha256, length}` digest.
///
/// The messages here deliberately never echo the value: the plain-string branch is exactly the
/// case where the value *is* a secret, and a refusal that quotes it would put the secret in a
/// terminal, a CI log and possibly a committed report.
fn sanitized_digest(
    value: &Value,
    location: &str,
    errors: &mut ValidationErrors,
) -> Option<ValueDigest> {
    let Some(object) = value.as_object() else {
        if value.is_string() {
            errors.refuse(
                InfraCode::UnsanitizedSecret,
                location.to_owned(),
                "the value is a plain string; secret values must never appear in a bundle — \
                 re-scan with a sanitizing scout",
            );
        } else {
            errors.refuse(
                InfraCode::MalformedSecretDigest,
                location.to_owned(),
                format!(
                    "the value is {}, not a {{sha256, length}} digest object",
                    value_kind(value)
                ),
            );
        }
        return None;
    };

    let sha256 = object.get("sha256").and_then(Value::as_str);
    let length = object.get("length").and_then(Value::as_u64);
    match (sha256, length) {
        (Some(sha256), Some(length)) if is_lower_hex_64(sha256) => Some(ValueDigest {
            sha256: sha256.to_owned(),
            length,
        }),
        _ => {
            errors.refuse(
                InfraCode::MalformedSecretDigest,
                location.to_owned(),
                "expected `sha256` as 64 lowercase hex characters and `length` as a \
                 non-negative integer",
            );
            None
        }
    }
}

/// `true` for exactly 64 lowercase hex characters.
fn is_lower_hex_64(text: &str) -> bool {
    text.len() == 64
        && text
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

/// Digests a configmap value into the same shape the scanner writes for secrets.
fn digest_of(text: &str) -> ValueDigest {
    use sha2::{Digest, Sha256};
    use std::fmt::Write as _;

    let hash = Sha256::digest(text.as_bytes());
    let mut sha256 = String::with_capacity(64);
    for byte in &hash {
        let _ = write!(sha256, "{byte:02x}");
    }
    ValueDigest {
        sha256,
        length: text.len() as u64,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn secret(data: &serde_json::Value) -> Result<Secret, ValidationErrors> {
        let raw: RawSecret = serde_json::from_value(serde_json::json!({
            "metadata": { "name": "creds", "namespace": "app", "uid": "u1" },
            "type": "Opaque",
            "data": data,
        }))
        .expect("the raw secret parses");
        let mut errors = ValidationErrors::new();
        let validated = Secret::from_raw(&raw, "kinds.secrets.items[0]", &mut errors);
        if errors.is_empty() {
            Ok(validated.expect("no errors means a secret"))
        } else {
            Err(errors)
        }
    }

    const DIGEST: &str = "8a94462377096e0657f57b6e6bc0e29000464398727091d7863726ce50974968";

    #[test]
    fn a_sanitized_secret_keeps_its_keys_and_digests() {
        let secret = secret(&serde_json::json!({
            "token": { "sha256": DIGEST, "length": 42 }
        }))
        .expect("a digest object is what a sanitized bundle carries");
        assert_eq!(secret.keys["token"].length, 42);
        assert_eq!(secret.keys["token"].sha256, DIGEST);
    }

    #[test]
    fn a_plain_string_secret_value_is_refused_and_the_refusal_does_not_echo_it() {
        let errors = secret(&serde_json::json!({ "token": "hunter2-base64" }))
            .expect_err("an unsanitized bundle is refused");
        assert!(
            errors.contains(InfraCode::UnsanitizedSecret),
            "expected INFRA-SECRET-001, got: {errors}"
        );
        let rendered = errors.to_string();
        assert!(
            !rendered.contains("hunter2"),
            "the refusal must never echo the value: {rendered}"
        );
        assert!(
            rendered.contains("data.token"),
            "the refusal names the key so the holder can fix the scan: {rendered}"
        );
    }

    #[test]
    fn every_unsanitized_value_is_reported_not_just_the_first() {
        let errors = secret(&serde_json::json!({
            "a": "plain",
            "b": "also-plain",
            "c": { "sha256": DIGEST, "length": 1 }
        }))
        .expect_err("two plain values are refused");
        let count = errors
            .as_slice()
            .iter()
            .filter(|error| error.code == InfraCode::UnsanitizedSecret)
            .count();
        assert_eq!(count, 2, "both plain values are named: {errors}");
    }

    #[test]
    fn a_digest_with_uppercase_hex_or_short_hash_is_refused_as_malformed() {
        for bad in [
            serde_json::json!({ "sha256": "ABCD", "length": 4 }),
            serde_json::json!({ "sha256": DIGEST[..63].to_owned(), "length": 4 }),
            serde_json::json!({ "sha256": DIGEST }),
            serde_json::json!(42),
        ] {
            let errors = secret(&serde_json::json!({ "k": bad }))
                .expect_err("a malformed digest is refused");
            assert!(
                errors.contains(InfraCode::MalformedSecretDigest),
                "expected INFRA-SECRET-002, got: {errors}"
            );
        }
    }

    #[test]
    fn a_configmap_value_is_hashed_and_the_plaintext_does_not_survive_into_the_model() {
        let raw: RawConfigMap = serde_json::from_value(serde_json::json!({
            "metadata": { "name": "settings", "namespace": "app", "uid": "u1" },
            "data": { "mode": "verbose" }
        }))
        .expect("the raw configmap parses");
        let mut errors = ValidationErrors::new();
        let validated =
            ConfigMap::from_raw(&raw, "c", &mut errors).expect("a valid configmap validates");
        assert!(errors.is_empty(), "nothing to refuse: {errors}");
        assert_eq!(validated.keys["mode"].length, 7);
        let serialized = serde_json::to_string(&validated).expect("the model serializes");
        assert!(
            !serialized.contains("verbose"),
            "the value must not appear in the model: {serialized}"
        );
    }
}
