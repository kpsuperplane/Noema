//! Provider account and authentication vocabulary.

use std::{fmt, path::PathBuf, time::Duration};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use crate::capabilities::capabilities_for_provider_account;
use crate::selection::{ProviderInstanceKey, ProviderSelectionError};
use crate::{ProviderCapability, config::CodexOAuthConfig};

/// Derive the immutable provider-instance identity for one hosted account.
///
/// # Errors
///
/// Returns [`ProviderSelectionError::EmptyField`] when the account id is blank.
pub fn provider_account_instance_key(
    provider_account_id: &str,
) -> Result<ProviderInstanceKey, ProviderSelectionError> {
    let provider_account_id = provider_account_id.trim();
    if provider_account_id.is_empty() {
        return Err(ProviderSelectionError::EmptyField("provider_account_id"));
    }
    ProviderInstanceKey::new(format!(
        "provider-account:v1:{}:{provider_account_id}",
        provider_account_id.len()
    ))
}

/// Supported provider account authentication methods.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum ProviderAuthMethod {
    /// OAuth device-code login.
    OauthDeviceCode,
    /// OAuth authorization-code login with PKCE.
    OauthPkce,
    /// Secret input such as an API key or access token.
    SecretInput,
    /// Manual terminal or externally managed login.
    ExternalManual,
    /// Provider does not require authentication.
    None,
}

impl ProviderAuthMethod {
    /// Return the stable storage string for this auth method.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OauthDeviceCode => "oauth_device_code",
            Self::OauthPkce => "oauth_pkce",
            Self::SecretInput => "secret_input",
            Self::ExternalManual => "external_manual",
            Self::None => "none",
        }
    }
}

/// Provider account readiness status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum ProviderAccountStatus {
    /// Status has not been checked.
    Unknown,
    /// Status check is in progress.
    Checking,
    /// Provider credentials are usable.
    Authenticated,
    /// Provider credentials are absent, expired, revoked, or invalid.
    Unauthenticated,
    /// Provider cannot be used because its binary, service, or config is unavailable.
    Unavailable,
}

impl ProviderAccountStatus {
    /// Return the stable storage string for this account status.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Checking => "checking",
            Self::Authenticated => "authenticated",
            Self::Unauthenticated => "unauthenticated",
            Self::Unavailable => "unavailable",
        }
    }
}

/// Durable non-secret provider account metadata.
#[derive(Debug, Clone, PartialEq)]
pub struct PersistedProviderAccountRecord {
    /// Stable provider account id.
    pub provider_account_id: String,
    /// Provider family, such as `codex`.
    pub provider_kind: String,
    /// Provider-local account key.
    pub account_key: String,
    /// Human-readable account name.
    pub display_name: String,
    /// Authentication method used for this account.
    pub auth_method: ProviderAuthMethod,
    /// Whether the account may be selected.
    pub is_active: bool,
    /// Whether the account is the default account for its provider.
    pub is_default: bool,
    /// Last known account readiness status.
    pub status: ProviderAccountStatus,
    /// Last time Noema checked the account status.
    pub last_checked_at: Option<String>,
    /// Last time Noema observed successful authentication.
    pub last_authenticated_at: Option<String>,
    /// Last non-secret provider error code.
    pub last_error_code: Option<String>,
    /// Last non-secret provider error message.
    pub last_error_message: Option<String>,
    /// Additional non-secret account metadata.
    pub metadata: Value,
}

/// Provider account metadata enriched with provider-derived capabilities.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderAccountRecord {
    /// Stable provider account id.
    pub provider_account_id: String,
    /// Provider family, such as `codex`.
    pub provider_kind: String,
    /// Provider-local account key.
    pub account_key: String,
    /// Human-readable account name.
    pub display_name: String,
    /// Authentication method used for this account.
    pub auth_method: ProviderAuthMethod,
    /// Whether the account may be selected.
    pub is_active: bool,
    /// Whether the account is the default account for its provider.
    pub is_default: bool,
    /// Last known account readiness status.
    pub status: ProviderAccountStatus,
    /// Last time Noema checked the account status.
    pub last_checked_at: Option<String>,
    /// Last time Noema observed successful authentication.
    pub last_authenticated_at: Option<String>,
    /// Last non-secret provider error code.
    pub last_error_code: Option<String>,
    /// Last non-secret provider error message.
    pub last_error_message: Option<String>,
    /// Additional non-secret account metadata.
    pub metadata: Value,
    /// Derived capabilities available through this provider account.
    #[serde(default)]
    pub capabilities: Vec<ProviderCapability>,
}

/// Enrich one durable account record with its provider-owned capabilities.
#[must_use]
pub fn provider_account_from_persisted(
    account: PersistedProviderAccountRecord,
) -> ProviderAccountRecord {
    let capabilities = capabilities_for_provider_account(
        &account.provider_kind,
        &account.account_key,
        account.status,
    );
    ProviderAccountRecord {
        provider_account_id: account.provider_account_id,
        provider_kind: account.provider_kind,
        account_key: account.account_key,
        display_name: account.display_name,
        auth_method: account.auth_method,
        is_active: account.is_active,
        is_default: account.is_default,
        status: account.status,
        last_checked_at: account.last_checked_at,
        last_authenticated_at: account.last_authenticated_at,
        last_error_code: account.last_error_code,
        last_error_message: account.last_error_message,
        metadata: account.metadata,
        capabilities,
    }
}

impl From<ProviderAccountRecord> for PersistedProviderAccountRecord {
    fn from(account: ProviderAccountRecord) -> Self {
        Self {
            provider_account_id: account.provider_account_id,
            provider_kind: account.provider_kind,
            account_key: account.account_key,
            display_name: account.display_name,
            auth_method: account.auth_method,
            is_active: account.is_active,
            is_default: account.is_default,
            status: account.status,
            last_checked_at: account.last_checked_at,
            last_authenticated_at: account.last_authenticated_at,
            last_error_code: account.last_error_code,
            last_error_message: account.last_error_message,
            metadata: account.metadata,
        }
    }
}

/// Provider type shown in the Settings add-account catalog.
#[derive(Debug, Clone, PartialEq)]
pub struct ProviderAccountCatalogEntry {
    /// Stable provider family identifier.
    pub provider_kind: String,
    /// Human-readable provider type name.
    pub display_name: String,
    /// Primary setup method presented to the user.
    pub preferred_auth_method: ProviderAuthMethod,
    /// Authentication methods supported for newly created accounts.
    pub supported_auth_methods: Vec<ProviderAuthMethod>,
    /// Capabilities this provider type can supply after account creation.
    pub capabilities: Vec<ProviderCapability>,
}

/// New user-created provider account metadata.
#[derive(Debug, Clone, PartialEq)]
pub struct NewProviderAccount {
    /// Stable provider family identifier.
    pub provider_kind: String,
    /// Optional user-facing account name.
    pub display_name: Option<String>,
    /// Authentication method for this account.
    pub auth_method: ProviderAuthMethod,
    /// Initial safe account status.
    pub status: ProviderAccountStatus,
    /// Non-secret provider account metadata.
    pub metadata: Value,
}

/// Return built-in system provider accounts exposed without durable rows.
#[must_use]
pub fn system_provider_accounts() -> Vec<ProviderAccountRecord> {
    vec![
        system_provider_account("duckduckgo_public", "DuckDuckGo public search"),
        system_provider_account("direct_http", "Direct HTTP web fetch"),
        system_provider_account("obscura", "Obscura interactive browser"),
    ]
}

/// Return provider types that can be added by the user.
#[must_use]
pub fn provider_account_catalog() -> Vec<ProviderAccountCatalogEntry> {
    [
        (
            "codex",
            "Codex",
            ProviderAuthMethod::OauthDeviceCode,
            vec![ProviderAuthMethod::OauthDeviceCode],
        ),
        (
            "openrouter",
            "OpenRouter",
            ProviderAuthMethod::OauthPkce,
            vec![
                ProviderAuthMethod::OauthPkce,
                ProviderAuthMethod::SecretInput,
            ],
        ),
        (
            "exa",
            "Exa",
            ProviderAuthMethod::SecretInput,
            vec![ProviderAuthMethod::SecretInput],
        ),
    ]
    .into_iter()
    .map(
        |(provider_kind, display_name, preferred_auth_method, supported_auth_methods)| {
            ProviderAccountCatalogEntry {
                provider_kind: provider_kind.to_string(),
                display_name: display_name.to_string(),
                preferred_auth_method,
                supported_auth_methods,
                capabilities: capabilities_for_provider_account(
                    provider_kind,
                    "catalog",
                    ProviderAccountStatus::Authenticated,
                ),
            }
        },
    )
    .collect()
}

fn system_provider_account(provider_kind: &str, display_name: &str) -> ProviderAccountRecord {
    let account_key = "system".to_string();
    let status = ProviderAccountStatus::Authenticated;
    ProviderAccountRecord {
        provider_account_id: format!("provider_account:{provider_kind}:system"),
        provider_kind: provider_kind.to_string(),
        account_key: account_key.clone(),
        display_name: display_name.to_string(),
        auth_method: ProviderAuthMethod::None,
        is_active: true,
        is_default: true,
        status,
        last_checked_at: None,
        last_authenticated_at: None,
        last_error_code: None,
        last_error_message: None,
        metadata: serde_json::json!({}),
        capabilities: capabilities_for_provider_account(provider_kind, &account_key, status),
    }
}

/// Short-lived provider auth attempt status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum ProviderAuthAttemptStatus {
    /// Attempt process is starting.
    Starting,
    /// Waiting for the user to complete an external auth step.
    WaitingForUser,
    /// Auth attempt completed successfully.
    Completed,
    /// Auth attempt failed.
    Failed,
    /// Auth attempt expired.
    Expired,
    /// Auth attempt was cancelled.
    Cancelled,
}

/// Request to start Codex device-code login.
#[derive(Clone, PartialEq, Eq)]
pub struct CodexDeviceAuthRequest {
    /// Stable provider account id.
    pub provider_account_id: String,
    /// Account-specific directory for Noema-owned Codex credentials.
    pub account_home: PathBuf,
    /// OAuth endpoint configuration.
    pub oauth: CodexOAuthConfig,
    /// Optional attempt timeout override. Defaults to five minutes.
    pub attempt_timeout: Option<Duration>,
}

impl fmt::Debug for CodexDeviceAuthRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CodexDeviceAuthRequest")
            .field("provider_account_id", &self.provider_account_id)
            .field("account_home", &self.account_home)
            .field("oauth", &self.oauth)
            .field("attempt_timeout", &self.attempt_timeout)
            .finish()
    }
}

/// Safe provider auth attempt state returned to the UI.
///
/// This type intentionally contains only typed state suitable for display. It
/// must not store raw provider stdout, stderr, credential paths, or provider
/// output.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct ProviderAuthAttemptView {
    /// Short-lived auth attempt id.
    pub attempt_id: String,
    /// Provider family, such as `codex`.
    pub provider_kind: String,
    /// Stable provider account id.
    pub provider_account_id: String,
    /// Provider account auth method.
    pub method: ProviderAuthMethod,
    /// Current attempt status.
    pub status: ProviderAuthAttemptStatus,
    /// Typed verification URL parsed from provider output.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub verification_url: Option<String>,
    /// Typed user code parsed from provider output.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub user_code: Option<String>,
    /// Static UI-safe instruction text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub instructions: Option<String>,
    /// Stable non-secret error code.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub error_code: Option<String>,
    /// Fixed non-secret error message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub error_message: Option<String>,
}

impl fmt::Debug for ProviderAuthAttemptView {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProviderAuthAttemptView")
            .field("attempt_id", &self.attempt_id)
            .field("provider_kind", &self.provider_kind)
            .field("provider_account_id", &self.provider_account_id)
            .field("method", &self.method)
            .field("status", &self.status)
            .field("verification_url", &self.verification_url)
            .field("user_code", &self.user_code.as_ref().map(|_| "[REDACTED]"))
            .field("instructions", &self.instructions)
            .field("error_code", &self.error_code)
            .field("error_message", &self.error_message)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hosted_instance_keys_are_stable_and_unambiguous_per_account() {
        let first = provider_account_instance_key("provider_account:openai:a:b")
            .expect("first account key");
        let second =
            provider_account_instance_key("provider_account:openai:a").expect("second account key");

        assert_ne!(first, second);
        assert_eq!(
            first,
            provider_account_instance_key(" provider_account:openai:a:b ")
                .expect("stable account key")
        );
        assert_eq!(
            provider_account_instance_key("   "),
            Err(ProviderSelectionError::EmptyField("provider_account_id"))
        );
    }

    #[test]
    fn auth_attempt_debug_redacts_user_code() {
        let view = ProviderAuthAttemptView {
            attempt_id: "attempt-1".to_string(),
            provider_kind: "codex".to_string(),
            provider_account_id: "provider_account:codex:default".to_string(),
            method: ProviderAuthMethod::OauthDeviceCode,
            status: ProviderAuthAttemptStatus::WaitingForUser,
            verification_url: Some("https://example.test/verify".to_string()),
            user_code: Some("SECRET-CODE".to_string()),
            instructions: None,
            error_code: None,
            error_message: None,
        };

        let debug = format!("{view:?}");
        assert!(!debug.contains("SECRET-CODE"));
        assert!(debug.contains("[REDACTED]"));
    }

    #[test]
    fn device_auth_request_debug_preserves_ordinary_configuration() {
        let request = CodexDeviceAuthRequest {
            provider_account_id: "provider_account:codex:default".to_string(),
            account_home: PathBuf::from("/private/device-auth-account"),
            oauth: CodexOAuthConfig {
                issuer: "https://issuer-user:issuer-secret@issuer.example.test/oauth".to_string(),
                client_id: "device-client-public".to_string(),
                token_url:
                    "https://issuer.example.test/oauth/token?access_token=token-secret&audience=noema"
                        .to_string(),
                timeout_seconds: 20,
            },
            attempt_timeout: None,
        };

        let debug = format!("{request:?}");
        for ordinary in [
            "provider_account:codex:default",
            "/private/device-auth-account",
            "https://issuer.example.test/oauth",
            "device-client-public",
            "https://issuer.example.test/oauth/token?audience=noema",
        ] {
            assert!(debug.contains(ordinary), "debug omitted {ordinary}");
        }
        assert!(!debug.contains("issuer-secret"));
        assert!(!debug.contains("token-secret"));
    }
}
