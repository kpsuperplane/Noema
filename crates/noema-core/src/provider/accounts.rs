use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use crate::memory::error::MemoryPersistenceError;

/// Supported provider account authentication methods.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum ProviderAuthMethod {
    /// OAuth device-code login.
    OauthDeviceCode,
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
}

/// Parse a stored provider auth method.
///
/// # Errors
///
/// Returns [`MemoryPersistenceError::InvalidEnum`] when the value is outside
/// the provider auth method vocabulary.
pub fn parse_auth_method(value: &str) -> Result<ProviderAuthMethod, MemoryPersistenceError> {
    match value {
        "oauth_device_code" => Ok(ProviderAuthMethod::OauthDeviceCode),
        "secret_input" => Ok(ProviderAuthMethod::SecretInput),
        "external_manual" => Ok(ProviderAuthMethod::ExternalManual),
        "none" => Ok(ProviderAuthMethod::None),
        other => Err(MemoryPersistenceError::InvalidEnum {
            kind: "provider_auth_method",
            value: other.to_string(),
        }),
    }
}

/// Parse a stored provider account status.
///
/// # Errors
///
/// Returns [`MemoryPersistenceError::InvalidEnum`] when the value is outside
/// the provider account status vocabulary.
pub fn parse_account_status(value: &str) -> Result<ProviderAccountStatus, MemoryPersistenceError> {
    match value {
        "unknown" => Ok(ProviderAccountStatus::Unknown),
        "checking" => Ok(ProviderAccountStatus::Checking),
        "authenticated" => Ok(ProviderAccountStatus::Authenticated),
        "unauthenticated" => Ok(ProviderAccountStatus::Unauthenticated),
        "unavailable" => Ok(ProviderAccountStatus::Unavailable),
        other => Err(MemoryPersistenceError::InvalidEnum {
            kind: "provider_account_status",
            value: other.to_string(),
        }),
    }
}
