//! Private, reviewed credential setup and Luau document normalization.

use crate::{
    AdapterCredentialGenerationV2, AdapterCredentialMaterial, AuthenticationSchemeV4,
    CompiledAdapterDefinition, CredentialInput, CredentialSetup, Oauth2CallbackMode,
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

const MAX_IMPORT_BYTES: usize = 128 * 1024;
const MAX_SECRET_BYTES: usize = 16 * 1024;

/// Safe failure from the transient credential-setup boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum AdapterCredentialImportError {
    /// The definition does not accept this setup form or callback mode.
    #[error("adapter credential setup is unsupported")]
    Unsupported,
    /// Submitted fields, document, or normalized output are invalid.
    #[error("adapter credential setup is invalid")]
    Invalid,
    /// The transient document exceeds the input bound.
    #[error("adapter credential setup is oversized")]
    Oversized,
}

/// Normalize one write-only submission into a private credential generation.
///
/// # Errors
///
/// Returns a safe category when the definition, serving callback mode, submitted
/// input shape, transform output, or generation identity is invalid.
pub fn setup_credential(
    definition: &CompiledAdapterDefinition,
    callback_mode: Option<Oauth2CallbackMode>,
    field_values: BTreeMap<String, String>,
    document: Option<&[u8]>,
    generation_id: String,
) -> Result<AdapterCredentialGenerationV2, AdapterCredentialImportError> {
    if !definition.reviewed || !valid_generation_id(&generation_id) {
        return Err(AdapterCredentialImportError::Invalid);
    }
    let (setup, mode) = match &definition.authentication {
        AuthenticationSchemeV4::Credential(config) if callback_mode.is_none() => {
            (&config.setup, None)
        }
        AuthenticationSchemeV4::None
        | AuthenticationSchemeV4::Credential(_)
        | AuthenticationSchemeV4::Oauth2AuthorizationCodePkce(_) => {
            return Err(AdapterCredentialImportError::Unsupported);
        }
    };
    let fields = normalize(setup, field_values, document)?;
    let material = match mode {
        None => AdapterCredentialMaterial::Credential { fields },
        Some(callback_mode) => AdapterCredentialMaterial::Oauth2ClientMetadata {
            callback_mode,
            client_id: fields
                .get("client_id")
                .cloned()
                .ok_or(AdapterCredentialImportError::Invalid)?,
            client_secret: fields.get("client_secret").cloned(),
        },
    };
    Ok(AdapterCredentialGenerationV2 {
        schema_version: 2,
        generation_id,
        material,
    })
}

fn normalize(
    setup: &CredentialSetup,
    field_values: BTreeMap<String, String>,
    document: Option<&[u8]>,
) -> Result<BTreeMap<String, String>, AdapterCredentialImportError> {
    let expected = setup
        .input
        .fields()
        .iter()
        .map(|field| field.id.as_str())
        .collect::<BTreeSet<_>>();
    let values = match &setup.input {
        CredentialInput::Fields { .. } => {
            if document.is_some()
                || field_values
                    .keys()
                    .map(String::as_str)
                    .collect::<BTreeSet<_>>()
                    != expected
            {
                return Err(AdapterCredentialImportError::Invalid);
            }
            field_values
        }
        CredentialInput::Document { normalize, .. } => {
            if !field_values.is_empty() {
                return Err(AdapterCredentialImportError::Invalid);
            }
            let document = document.ok_or(AdapterCredentialImportError::Invalid)?;
            if document.len() > MAX_IMPORT_BYTES {
                return Err(AdapterCredentialImportError::Oversized);
            }
            let document =
                std::str::from_utf8(document).map_err(|_| AdapterCredentialImportError::Invalid)?;
            let output = crate::luau::normalize_credential(
                normalize.source(),
                &json!({"document": document}),
            )
            .map_err(|_| AdapterCredentialImportError::Invalid)?;
            private_string_map(output)?
        }
    };
    if values.keys().map(String::as_str).collect::<BTreeSet<_>>() != expected
        || values.values().any(|value| !valid_secret(value))
    {
        return Err(AdapterCredentialImportError::Invalid);
    }
    Ok(values)
}

fn private_string_map(
    value: Value,
) -> Result<BTreeMap<String, String>, AdapterCredentialImportError> {
    let Value::Object(values) = value else {
        return Err(AdapterCredentialImportError::Invalid);
    };
    values
        .into_iter()
        .map(|(key, value)| {
            value
                .as_str()
                .map(|value| (key, value.to_string()))
                .ok_or(AdapterCredentialImportError::Invalid)
        })
        .collect()
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
    use crate::{AdapterCompiler, AdapterManifest};
    use serde_json::json;

    fn oauth_definition() -> CompiledAdapterDefinition {
        let manifest: AdapterManifest = serde_json::from_value(json!({
            "schema_version": 9,
            "definition_id": "definition:google_web",
            "adapter_id": "google_web",
            "definition_revision": "v1",
            "reviewed": true,
            "origin": "https://api.example.test/",
            "authentication": {
                "kind": "oauth2_authorization_code_pkce",
                "profile_digest": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
            },
            "operations": [{
                "operation_id": "list", "description": "List items.", "method": "GET", "path": "/v1/items",
                "authorization": {"kind": "oauth_scopes", "accepted_scope_sets": [["calendar.read"]]},
                "behavior": {"readOnly": {"value": true, "source": "model"}, "idempotent": {"value": true, "source": "model"}, "destructive": {"value": false, "source": "model"}, "openWorld": {"value": true, "source": "model"}},
                "retry": "transport_safe_read", "pagination": {"kind": "none"},
                "response": {"accepted_content_types": ["application/json"], "transform": {"language": "luau", "source": "return function(response) return nil end"}, "output_schema": {"type": "null"}}
            }]
        })).expect("manifest");
        AdapterCompiler::compile(&manifest).expect("definition")
    }

    #[test]
    fn oauth_documents_are_not_imported_as_connection_credentials() {
        let definition = oauth_definition();
        assert_eq!(
            setup_credential(
                &definition,
                Some(Oauth2CallbackMode::Hosted),
                BTreeMap::new(),
                Some(br#"{"web":{"client_id":"client-marker","client_secret":"secret-marker"}}"#),
                "a".repeat(32),
            ),
            Err(AdapterCredentialImportError::Unsupported)
        );
    }
}
