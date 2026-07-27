//! Bounded, definition-declared extraction of transient OAuth client JSON.

use crate::json_limits::{parse_without_duplicate_keys, validate_json_shape};
use crate::{
    AdapterCredentialGenerationV1, AdapterCredentialMaterial, AuthenticationMode,
    CompiledAdapterDefinition, CredentialImportKind, CredentialImportSchema,
};
use serde_json::Value;
use thiserror::Error;

const MAX_IMPORT_BYTES: usize = 128 * 1024;
const MAX_POINTER_BYTES: usize = 512;
const MAX_ALTERNATIVES: usize = 16;
const MAX_SECRET_BYTES: usize = 16 * 1024;

/// Safe failure from the transient credential-import boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum AdapterCredentialImportError {
    /// The definition does not declare this import family.
    #[error("adapter credential import is unsupported")]
    Unsupported,
    /// The bounded upload or its selected fields are invalid.
    #[error("adapter credential import is invalid")]
    Invalid,
    /// The upload exceeds the transient input bound.
    #[error("adapter credential import is oversized")]
    Oversized,
}

/// Validate a definition-declared import schema without reading credential
/// bytes. The compiler maps the stable category to its manifest diagnostic.
pub(crate) fn validate_import_schema(schema: &CredentialImportSchema) -> Result<(), &'static str> {
    if schema.alternatives.is_empty() || schema.alternatives.len() > MAX_ALTERNATIVES {
        return Err("credential_import_alternatives");
    }
    let mut layouts = std::collections::BTreeSet::new();
    for layout in &schema.alternatives {
        validate_pointer(&layout.client_id_pointer).map_err(|_| "credential_import_pointer")?;
        if let Some(pointer) = &layout.client_secret_pointer {
            validate_pointer(pointer).map_err(|_| "credential_import_pointer")?;
        }
        if layout
            .client_secret_pointer
            .as_deref()
            .is_some_and(|pointer| pointer == layout.client_id_pointer)
            || !layouts.insert((
                layout.client_id_pointer.as_str(),
                layout.client_secret_pointer.as_deref(),
            ))
        {
            return Err("credential_import_duplicate");
        }
    }
    Ok(())
}

/// Extract exactly one reviewed OAuth client-metadata layout from transient
/// JSON. The input bytes are parsed and dropped inside this call; only the
/// selected fields are returned in the private credential generation.
///
/// # Errors
///
/// Returns a safe error when the definition has no matching schema, the
/// upload is malformed or ambiguous, or the selected values are invalid.
pub fn import_client_json(
    definition: &CompiledAdapterDefinition,
    bytes: &[u8],
    generation_id: String,
) -> Result<AdapterCredentialGenerationV1, AdapterCredentialImportError> {
    if !definition.reviewed
        || definition.authentication.mode != AuthenticationMode::Oauth2AuthorizationCodePkce
    {
        return Err(AdapterCredentialImportError::Unsupported);
    }
    let Some(schema) = definition.authentication.credential_import.as_ref() else {
        return Err(AdapterCredentialImportError::Unsupported);
    };
    if schema.kind != CredentialImportKind::OauthClientJson {
        return Err(AdapterCredentialImportError::Unsupported);
    }
    validate_import_schema(schema).map_err(|_| AdapterCredentialImportError::Invalid)?;
    if bytes.len() > MAX_IMPORT_BYTES {
        return Err(AdapterCredentialImportError::Oversized);
    }
    if !valid_generation_id(&generation_id) {
        return Err(AdapterCredentialImportError::Invalid);
    }
    let document: Value =
        parse_without_duplicate_keys(bytes).map_err(|_| AdapterCredentialImportError::Invalid)?;
    if !document.is_object() || !validate_json_shape(&document) {
        return Err(AdapterCredentialImportError::Invalid);
    }

    let mut match_value = None;
    for layout in &schema.alternatives {
        let Some(client_id_value) = document.pointer(&layout.client_id_pointer) else {
            continue;
        };
        let Some(client_id) = client_id_value.as_str() else {
            return Err(AdapterCredentialImportError::Invalid);
        };
        if !valid_secret(client_id) {
            return Err(AdapterCredentialImportError::Invalid);
        }
        let client_secret = match layout
            .client_secret_pointer
            .as_deref()
            .and_then(|pointer| document.pointer(pointer))
        {
            None | Some(Value::Null) => None,
            Some(value) => Some(
                value
                    .as_str()
                    .filter(|value| valid_secret(value))
                    .ok_or(AdapterCredentialImportError::Invalid)?
                    .to_string(),
            ),
        };
        if match_value.is_some() {
            return Err(AdapterCredentialImportError::Invalid);
        }
        match_value = Some((client_id.to_string(), client_secret));
    }
    let (client_id, client_secret) = match_value.ok_or(AdapterCredentialImportError::Invalid)?;
    Ok(AdapterCredentialGenerationV1 {
        schema_version: 1,
        generation_id,
        material: AdapterCredentialMaterial::Oauth2ClientMetadata {
            client_id,
            client_secret,
        },
    })
}

fn validate_pointer(pointer: &str) -> Result<(), ()> {
    if pointer.is_empty()
        || pointer.len() > MAX_POINTER_BYTES
        || !pointer.starts_with('/')
        || pointer.bytes().any(|byte| byte.is_ascii_control())
    {
        return Err(());
    }
    let mut bytes = pointer.bytes();
    while let Some(byte) = bytes.next() {
        if byte == b'~' && !matches!(bytes.next(), Some(b'0' | b'1')) {
            return Err(());
        }
    }
    Ok(())
}

fn valid_generation_id(value: &str) -> bool {
    value.len() == 32
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

fn valid_secret(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_SECRET_BYTES
        && !value.bytes().any(|byte| byte.is_ascii_control())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AdapterCompiler, AdapterManifestV1};
    use serde_json::json;

    fn definition() -> CompiledAdapterDefinition {
        let manifest: AdapterManifestV1 = serde_json::from_value(json!({
            "schema_version": 1,
            "definition_id": "definition:import",
            "adapter_id": "import",
            "definition_revision": "v1",
            "reviewed": true,
            "origin": "https://api.example.test/",
            "authentication": {
                "mode": "oauth2_authorization_code_pkce",
                "scopes": [],
                "credential_import": {
                    "kind": "oauth_client_json",
                    "alternatives": [
                        {"client_id_pointer": "/desktop/client_id", "client_secret_pointer": "/desktop/client_secret"},
                        {"client_id_pointer": "/browser/client_id", "client_secret_pointer": "/browser/client_secret"}
                    ]
                }
            },
            "provider_data_policy": {"retention_allowed": false, "deletion_supported": true},
            "quota": {"cost_class": "free"},
            "operations": [{
                "operation_id": "list",
                "method": "GET",
                "path": "/v1/items",
                "effect": "read_only",
                "admission": "direct",
                "result": {
                    "classification": "private",
                    "model_route": "local_only",
                    "model_payload": "full",
                    "provider_retention": "deny",
                    "persistence": "omit"
                },
                "retry": "transport_safe_read",
                "pagination": {"kind": "none"}
            }]
        }))
        .expect("manifest");
        assert_eq!(manifest.authentication.scopes.len(), 0);
        AdapterCompiler::compile(&manifest).expect("compile")
    }

    #[test]
    fn import_extracts_only_one_declared_layout() {
        let upload = br#"{"desktop":{"client_id":"client-marker","client_secret":"secret-marker","upload_marker":"discard-me"}}"#;
        let credential = import_client_json(&definition(), upload, "a".repeat(32)).expect("import");
        assert_eq!(credential.generation_id, "a".repeat(32));
        let stored = serde_json::to_string(&credential).expect("json");
        let AdapterCredentialMaterial::Oauth2ClientMetadata {
            client_id,
            client_secret,
        } = &credential.material
        else {
            panic!("metadata generation");
        };
        assert_eq!(client_id, "client-marker");
        assert_eq!(client_secret.as_deref(), Some("secret-marker"));
        assert!(!stored.contains("discard-me"));
    }

    #[test]
    fn import_rejects_ambiguous_or_malformed_uploads() {
        let definition = definition();
        assert!(
            import_client_json(
                &definition,
                br#"{"desktop":{"client_id":"one"},"browser":{"client_id":"two"}}"#,
                "b".repeat(32),
            )
            .is_err()
        );
        assert!(
            import_client_json(
                &definition,
                br#"{"desktop":{"client_id":42}}"#,
                "c".repeat(32),
            )
            .is_err()
        );
        assert!(import_client_json(&definition, b"not-json", "d".repeat(32)).is_err());
        assert!(import_client_json(&definition, br#"{"unrelated":true}"#, "e".repeat(32)).is_err());
        assert!(
            import_client_json(
                &definition,
                br#"{"desktop":{"client_id":"one","client_id":"two"}}"#,
                "g".repeat(32)
            )
            .is_err()
        );
        assert!(
            import_client_json(&definition, &vec![b'x'; 128 * 1024 + 1], "h".repeat(32)).is_err()
        );
        assert!(
            import_client_json(
                &definition,
                br#"{"desktop":{"client_id":"public"}}"#,
                "1".repeat(32)
            )
            .is_ok()
        );
        let mut unreviewed = definition.clone();
        unreviewed.reviewed = false;
        assert!(
            import_client_json(
                &unreviewed,
                br#"{"desktop":{"client_id":"one"}}"#,
                "f".repeat(32)
            )
            .is_err()
        );
    }

    #[test]
    fn schema_rejects_relative_or_duplicate_pointers() {
        let invalid = CredentialImportSchema {
            kind: CredentialImportKind::OauthClientJson,
            alternatives: vec![crate::definition::CredentialImportLayout {
                client_id_pointer: "desktop/client_id".to_string(),
                client_secret_pointer: None,
            }],
        };
        assert_eq!(
            validate_import_schema(&invalid),
            Err("credential_import_pointer")
        );
        let duplicate = CredentialImportSchema {
            kind: CredentialImportKind::OauthClientJson,
            alternatives: vec![crate::definition::CredentialImportLayout {
                client_id_pointer: "/client_id".to_string(),
                client_secret_pointer: Some("/client_id".to_string()),
            }],
        };
        assert_eq!(
            validate_import_schema(&duplicate),
            Err("credential_import_duplicate")
        );
    }
}
