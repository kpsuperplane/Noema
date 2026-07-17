//! Secret-bearing MCP control-plane input models.

use std::{collections::BTreeMap, fmt};

use serde::{Deserialize, Serialize};
use serde_json::Value;

const REDACTED: &str = "[REDACTED]";

/// Secret MCP setup material stored outside the structured repository.
#[derive(Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct McpSecretMaterial {
    /// Opaque generation mirrored in safe configuration to bind both stores.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secret_identity_revision: Option<String>,
    /// Secret environment variables for stdio MCP servers.
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    /// Secret headers for Streamable HTTP MCP servers.
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
    /// OAuth client-secret credentials for client-credentials authentication.
    #[serde(default)]
    pub oauth_client_credentials: Option<McpOAuthClientCredentials>,
    /// OAuth authorization-code credentials for hosted browser authentication.
    #[serde(default)]
    pub oauth_credentials: Option<McpOAuthStoredCredentials>,
}

impl McpSecretMaterial {
    /// Return whether any secret material is configured.
    #[must_use]
    pub fn has_secret_material(&self) -> bool {
        !self.env.is_empty()
            || !self.headers.is_empty()
            || self.oauth_client_credentials.is_some()
            || self.oauth_credentials.is_some()
    }
}

impl fmt::Debug for McpSecretMaterial {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("McpSecretMaterial")
            .field("env_entry_count", &self.env.len())
            .field("header_entry_count", &self.headers.len())
            .field(
                "oauth_client_credentials",
                &self.oauth_client_credentials.as_ref().map(|_| REDACTED),
            )
            .field(
                "oauth_credentials",
                &self.oauth_credentials.as_ref().map(|_| REDACTED),
            )
            .finish()
    }
}

/// OAuth 2.0 client-secret credentials supplied during MCP setup.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpOAuthClientCredentials {
    /// OAuth client identifier.
    pub client_id: String,
    /// OAuth client secret.
    pub client_secret: String,
    /// Requested OAuth scopes.
    #[serde(default)]
    pub scopes: Vec<String>,
}

impl fmt::Debug for McpOAuthClientCredentials {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("McpOAuthClientCredentials")
            .field("client_id", &REDACTED)
            .field("client_secret", &REDACTED)
            .field("scope_count", &self.scopes.len())
            .finish()
    }
}

/// OAuth 2.0 authorization-code credentials captured after browser setup.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct McpOAuthStoredCredentials {
    /// OAuth client identifier registered during the browser flow.
    pub client_id: String,
    /// Serialized token response returned by the OAuth server.
    pub token_response: Value,
    /// Unix timestamp when the token response was received.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_received_at: Option<u64>,
}

impl fmt::Debug for McpOAuthStoredCredentials {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("McpOAuthStoredCredentials")
            .field("client_id", &REDACTED)
            .field("token_response", &REDACTED)
            .field("has_token_received_at", &self.token_received_at.is_some())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn secret_models_redact_every_secret_value_from_debug_output() {
        let material = McpSecretMaterial {
            secret_identity_revision: Some("revision".to_string()),
            env: BTreeMap::from([("PRIVATE_TOKEN".to_string(), "env-secret".to_string())]),
            headers: BTreeMap::from([(
                "Authorization".to_string(),
                "Bearer header-secret".to_string(),
            )]),
            oauth_client_credentials: Some(McpOAuthClientCredentials {
                client_id: "private-client-id".to_string(),
                client_secret: "client-secret".to_string(),
                scopes: vec!["tools.read".to_string()],
            }),
            oauth_credentials: Some(McpOAuthStoredCredentials {
                client_id: "browser-client-id".to_string(),
                token_response: json!({
                    "access_token": "access-secret",
                    "refresh_token": "refresh-secret"
                }),
                token_received_at: Some(42),
            }),
        };

        let debug = format!("{material:?}");

        for secret in [
            "env-secret",
            "Bearer header-secret",
            "private-client-id",
            "client-secret",
            "browser-client-id",
            "access-secret",
            "refresh-secret",
        ] {
            assert!(!debug.contains(secret), "debug output leaked {secret:?}");
        }
        assert!(debug.contains(REDACTED));
        assert!(material.has_secret_material());
    }

    #[test]
    fn nested_credential_debug_implementations_are_redacted() {
        let client = McpOAuthClientCredentials {
            client_id: "client-id".to_string(),
            client_secret: "client-secret".to_string(),
            scopes: vec!["scope".to_string()],
        };
        let stored = McpOAuthStoredCredentials {
            client_id: "stored-client".to_string(),
            token_response: json!({"access_token": "stored-token"}),
            token_received_at: None,
        };

        let debug = format!("{client:?} {stored:?}");

        for secret in [
            "client-id",
            "client-secret",
            "stored-client",
            "stored-token",
        ] {
            assert!(!debug.contains(secret), "debug output leaked {secret:?}");
        }
    }
}
