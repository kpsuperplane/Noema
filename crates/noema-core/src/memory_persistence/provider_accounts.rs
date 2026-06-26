use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{error::MemoryPersistenceError, repository::PostgresMemoryRepository};

type ProviderAccountRow = (
    String,
    String,
    String,
    String,
    String,
    bool,
    bool,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Value,
);

/// Supported provider account authentication methods.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
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
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::OauthDeviceCode => "oauth_device_code",
            Self::SecretInput => "secret_input",
            Self::ExternalManual => "external_manual",
            Self::None => "none",
        }
    }
}

/// Provider account readiness status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
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
    pub(super) const fn as_str(self) -> &'static str {
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

impl PostgresMemoryRepository {
    /// Create or return the default active provider account.
    ///
    /// # Errors
    ///
    /// Returns [`MemoryPersistenceError`] if Postgres writes or reads fail, if
    /// stored enum values are invalid, or if the default row cannot be found
    /// after the upsert.
    pub async fn ensure_default_provider_account(
        &self,
    ) -> Result<ProviderAccountRecord, MemoryPersistenceError> {
        sqlx::query(
            r#"
            INSERT INTO provider_accounts (
              provider_account_id, provider_kind, account_key, display_name,
              auth_method, is_active, is_default, status
            )
            VALUES (
              'provider_account:codex:default', 'codex', 'default', 'Codex',
              $1, true, true, 'unknown'
            )
            ON CONFLICT (provider_kind, account_key) DO UPDATE
              SET display_name = EXCLUDED.display_name,
                  auth_method = EXCLUDED.auth_method,
                  is_active = true,
                  is_default = true,
                  updated_at = now()
            "#,
        )
        .bind(ProviderAuthMethod::OauthDeviceCode.as_str())
        .execute(self.pool())
        .await
        .map_err(MemoryPersistenceError::Database)?;

        self.get_provider_account("provider_account:codex:default")
            .await?
            .ok_or_else(|| MemoryPersistenceError::ProviderAccountNotFound {
                provider_account_id: "provider_account:codex:default".to_string(),
            })
    }

    /// Return the active default provider account.
    ///
    /// # Errors
    ///
    /// Returns [`MemoryPersistenceError`] if Postgres reads fail or stored enum
    /// values are invalid.
    pub async fn active_provider_account(
        &self,
    ) -> Result<Option<ProviderAccountRecord>, MemoryPersistenceError> {
        let row = sqlx::query_as::<_, ProviderAccountRow>(
            r#"
            SELECT
              provider_account_id, provider_kind, account_key, display_name,
              auth_method, is_active, is_default, status,
              last_checked_at::text, last_authenticated_at::text,
              last_error_code, last_error_message, metadata
            FROM provider_accounts
            WHERE is_active = true
              AND is_default = true
              AND deleted_at IS NULL
            ORDER BY created_at
            LIMIT 1
            "#,
        )
        .fetch_optional(self.pool())
        .await
        .map_err(MemoryPersistenceError::Database)?;

        row.map(provider_account_from_row).transpose()
    }

    /// Return one provider account by id.
    ///
    /// # Errors
    ///
    /// Returns [`MemoryPersistenceError`] if Postgres reads fail or stored enum
    /// values are invalid.
    pub async fn get_provider_account(
        &self,
        provider_account_id: &str,
    ) -> Result<Option<ProviderAccountRecord>, MemoryPersistenceError> {
        let row = sqlx::query_as::<_, ProviderAccountRow>(
            r#"
            SELECT
              provider_account_id, provider_kind, account_key, display_name,
              auth_method, is_active, is_default, status,
              last_checked_at::text, last_authenticated_at::text,
              last_error_code, last_error_message, metadata
            FROM provider_accounts
            WHERE provider_account_id = $1
              AND deleted_at IS NULL
            "#,
        )
        .bind(provider_account_id)
        .fetch_optional(self.pool())
        .await
        .map_err(MemoryPersistenceError::Database)?;

        row.map(provider_account_from_row).transpose()
    }

    /// Update safe provider account status metadata.
    ///
    /// # Errors
    ///
    /// Returns [`MemoryPersistenceError`] if Postgres cannot update the row.
    pub async fn update_provider_account_status(
        &self,
        provider_account_id: &str,
        status: ProviderAccountStatus,
        error_code: Option<&str>,
        error_message: Option<&str>,
    ) -> Result<(), MemoryPersistenceError> {
        sqlx::query(
            r#"
            UPDATE provider_accounts
            SET status = $2,
                last_checked_at = now(),
                last_authenticated_at = CASE WHEN $2 = 'authenticated' THEN now() ELSE last_authenticated_at END,
                last_error_code = $3,
                last_error_message = $4,
                updated_at = now()
            WHERE provider_account_id = $1
            "#,
        )
        .bind(provider_account_id)
        .bind(status.as_str())
        .bind(error_code)
        .bind(error_message)
        .execute(self.pool())
        .await
        .map_err(MemoryPersistenceError::Database)?;
        Ok(())
    }
}

fn provider_account_from_row(
    row: ProviderAccountRow,
) -> Result<ProviderAccountRecord, MemoryPersistenceError> {
    let (
        provider_account_id,
        provider_kind,
        account_key,
        display_name,
        auth_method,
        is_active,
        is_default,
        status,
        last_checked_at,
        last_authenticated_at,
        last_error_code,
        last_error_message,
        metadata,
    ) = row;

    Ok(ProviderAccountRecord {
        provider_account_id,
        provider_kind,
        account_key,
        display_name,
        auth_method: parse_auth_method(&auth_method)?,
        is_active,
        is_default,
        status: parse_account_status(&status)?,
        last_checked_at,
        last_authenticated_at,
        last_error_code,
        last_error_message,
        metadata,
    })
}

fn parse_auth_method(value: &str) -> Result<ProviderAuthMethod, MemoryPersistenceError> {
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

fn parse_account_status(value: &str) -> Result<ProviderAccountStatus, MemoryPersistenceError> {
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
