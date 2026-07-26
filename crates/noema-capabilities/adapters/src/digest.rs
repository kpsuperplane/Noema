//! Domain-separated SHA-256 digest types.

use crate::{AdapterManifestV1, AdapterOperation};
use ring::digest::{SHA256, digest};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::fmt;
use thiserror::Error;

macro_rules! digest_type {
    ($name:ident, $label:literal) => {
        #[doc = $label]
        #[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub(crate) fn compute(bytes: &[u8]) -> Self {
                Self(hex_digest(bytes))
            }

            /// Parse a lowercase hexadecimal SHA-256 digest.
            ///
            /// # Errors
            ///
            /// Returns [`DigestError`] when the value is not exactly 64
            /// lowercase hexadecimal characters.
            pub fn parse(value: impl Into<String>) -> Result<Self, DigestError> {
                let value = value.into();
                if value.len() == 64
                    && value
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
                {
                    Ok(Self(value))
                } else {
                    Err(DigestError)
                }
            }

            /// Return the stable lowercase hexadecimal representation.
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(&self.0)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(&self.0)
            }
        }
    };
}

digest_type!(SourceDigest, "SHA-256 of exact imported source bytes.");
digest_type!(
    SemanticDigest,
    "SHA-256 of canonical execution and security semantics."
);
digest_type!(OperationDigest, "SHA-256 of one canonical operation plan.");

/// Invalid digest representation.
#[derive(Debug, Clone, Copy, Error, PartialEq, Eq)]
#[error("digest must be 64 lowercase hexadecimal characters")]
pub struct DigestError;

pub(crate) fn canonical_json_bytes(value: &Value) -> Result<Vec<u8>, serde_json::Error> {
    serde_json::to_vec(&canonicalize(value))
}

pub(crate) fn semantic_manifest_value(
    manifest: &AdapterManifestV1,
) -> Result<Value, serde_json::Error> {
    let mut value = serde_json::to_value(manifest)?;
    if let Some(object) = value.as_object_mut() {
        object.remove("display_name");
        if let Some(Value::Array(operations)) = object.get_mut("operations") {
            for operation in operations {
                if let Some(operation) = operation.as_object_mut() {
                    operation.remove("source_description");
                }
            }
        }
    }
    sort_semantic_sets(&mut value);
    Ok(value)
}

pub(crate) fn semantic_operation_value(
    operation: &AdapterOperation,
) -> Result<Value, serde_json::Error> {
    let mut value = serde_json::to_value(operation)?;
    if let Some(object) = value.as_object_mut() {
        object.remove("source_description");
    }
    sort_semantic_sets(&mut value);
    Ok(value)
}

fn canonicalize(value: &Value) -> Value {
    match value {
        Value::Object(object) => {
            let mut keys = object.keys().collect::<Vec<_>>();
            keys.sort_unstable();
            Value::Object(
                keys.into_iter()
                    .map(|key| (key.clone(), canonicalize(&object[key])))
                    .collect::<Map<_, _>>(),
            )
        }
        Value::Array(values) => Value::Array(values.iter().map(canonicalize).collect()),
        _ => value.clone(),
    }
}

fn sort_semantic_sets(value: &mut Value) {
    let Some(object) = value.as_object_mut() else {
        return;
    };
    for key in ["scopes", "gates", "enum_values"] {
        if let Some(Value::Array(values)) = object.get_mut(key) {
            values.sort_by_key(ToString::to_string);
        }
    }
    for (key, identity) in [("operations", "operation_id"), ("arguments", "name")] {
        if let Some(Value::Array(values)) = object.get_mut(key) {
            values.sort_by_key(|value| {
                value
                    .get(identity)
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string()
            });
            for value in values {
                sort_semantic_sets(value);
            }
        }
    }
    if let Some(Value::Object(authentication)) = object.get_mut("authentication")
        && let Some(Value::Array(scopes)) = authentication.get_mut("scopes")
    {
        scopes.sort_by_key(ToString::to_string);
    }
}

fn hex_digest(bytes: &[u8]) -> String {
    digest(&SHA256, bytes)
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
