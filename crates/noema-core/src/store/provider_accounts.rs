use serde::Deserialize;
use serde_json::Value;
use surrealdb::types::SurrealValue;

use crate::{
    ProviderAccountRecord, ProviderAccountStatus, ProviderAuthMethod, store::ids::now_string,
};

use super::{NoemaStore, StoreError};

impl NoemaStore {
    /// Create or refresh the built-in local human and primary agent.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the embedded store write fails.
    pub async fn ensure_default_actors(&self) -> Result<(), StoreError> {
        self.db
            .query(
                r#"
                UPSERT type::record('humans', 'human_local') SET
                  human_id = 'human:local',
                  display_name = 'Local Human',
                  updated_at = time::now();
                UPSERT type::record('agents', 'agent_primary') SET
                  agent_id = 'agent:primary',
                  updated_at = time::now();
                "#,
            )
            .await?
            .check()?;
        Ok(())
    }

    /// Create or return the default Codex provider account metadata.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the embedded store write or read fails.
    pub async fn ensure_default_provider_account(
        &self,
    ) -> Result<ProviderAccountRecord, StoreError> {
        if let Some(account) = self
            .get_provider_account("provider_account:codex:default")
            .await?
        {
            return Ok(account);
        }

        self.db
            .query(
                r#"
                UPSERT type::record('provider_accounts', 'codex_default') SET
                  provider_account_id = 'provider_account:codex:default',
                  provider_kind = 'codex',
                  account_key = 'default',
                  display_name = 'Codex',
                  auth_method = 'oauth_device_code',
                  is_active = true,
                  is_default = true,
                  status = 'unknown',
                  metadata = {},
                  updated_at = time::now();
                "#,
            )
            .await?
            .check()?;
        self.get_provider_account("provider_account:codex:default")
            .await?
            .ok_or_else(|| StoreError::ProviderAccountNotFound {
                provider_account_id: "provider_account:codex:default".to_string(),
            })
    }

    /// Return the active default account for one provider.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the embedded store read fails or a stored
    /// enum is invalid.
    pub async fn active_provider_account(
        &self,
        provider_kind: &str,
    ) -> Result<Option<ProviderAccountRecord>, StoreError> {
        let mut response = self
            .db
            .query(
                r#"
                SELECT provider_account_id, provider_kind, account_key, display_name,
                  auth_method, is_active, is_default, status, last_checked_at,
                  last_authenticated_at, last_error_code, last_error_message, metadata
                FROM provider_accounts
                WHERE provider_kind = $provider_kind
                  AND is_active = true
                  AND is_default = true
                LIMIT 1;
                "#,
            )
            .bind(("provider_kind", provider_kind.to_string()))
            .await?;
        let rows: Vec<ProviderAccountRow> = response.take(0)?;
        rows.into_iter()
            .next()
            .map(provider_account_from_row)
            .transpose()
    }

    /// Return one provider account by id.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the embedded store read fails or a stored
    /// enum is invalid.
    pub async fn get_provider_account(
        &self,
        provider_account_id: &str,
    ) -> Result<Option<ProviderAccountRecord>, StoreError> {
        let mut response = self
            .db
            .query(
                r#"
                SELECT provider_account_id, provider_kind, account_key, display_name,
                  auth_method, is_active, is_default, status, last_checked_at,
                  last_authenticated_at, last_error_code, last_error_message, metadata
                FROM provider_accounts
                WHERE provider_account_id = $provider_account_id
                LIMIT 1;
                "#,
            )
            .bind(("provider_account_id", provider_account_id.to_string()))
            .await?;
        let rows: Vec<ProviderAccountRow> = response.take(0)?;
        rows.into_iter()
            .next()
            .map(provider_account_from_row)
            .transpose()
    }

    /// Update safe provider account status metadata.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the account is missing or the embedded store
    /// write fails.
    pub async fn update_provider_account_status(
        &self,
        provider_account_id: &str,
        status: ProviderAccountStatus,
        error_code: Option<&str>,
        error_message: Option<&str>,
    ) -> Result<(), StoreError> {
        let Some(account) = self.get_provider_account(provider_account_id).await? else {
            return Err(StoreError::ProviderAccountNotFound {
                provider_account_id: provider_account_id.to_string(),
            });
        };
        let checked_at = now_string();
        let last_authenticated_at = if status == ProviderAccountStatus::Authenticated {
            Some(checked_at.clone())
        } else {
            account.last_authenticated_at
        };
        self.db
            .query(
                r#"
                UPDATE provider_accounts SET
                  status = $status,
                  last_checked_at = $last_checked_at,
                  last_authenticated_at = $last_authenticated_at,
                  last_error_code = $error_code,
                  last_error_message = $error_message,
                  updated_at = time::now()
                WHERE provider_account_id = $provider_account_id;
                "#,
            )
            .bind(("provider_account_id", provider_account_id.to_string()))
            .bind(("status", provider_status_str(status).to_string()))
            .bind(("last_checked_at", Some(checked_at)))
            .bind(("last_authenticated_at", last_authenticated_at))
            .bind(("error_code", error_code.map(ToString::to_string)))
            .bind(("error_message", error_message.map(ToString::to_string)))
            .await?
            .check()?;
        Ok(())
    }
}

#[derive(Debug, Deserialize, SurrealValue)]
struct ProviderAccountRow {
    provider_account_id: String,
    provider_kind: String,
    account_key: String,
    display_name: String,
    auth_method: String,
    is_active: bool,
    is_default: bool,
    status: String,
    last_checked_at: Option<String>,
    last_authenticated_at: Option<String>,
    last_error_code: Option<String>,
    last_error_message: Option<String>,
    metadata: Value,
}

fn provider_account_from_row(row: ProviderAccountRow) -> Result<ProviderAccountRecord, StoreError> {
    Ok(ProviderAccountRecord {
        provider_account_id: row.provider_account_id,
        provider_kind: row.provider_kind,
        account_key: row.account_key,
        display_name: row.display_name,
        auth_method: parse_provider_auth_method(&row.auth_method)?,
        is_active: row.is_active,
        is_default: row.is_default,
        status: parse_provider_account_status(&row.status)?,
        last_checked_at: row.last_checked_at,
        last_authenticated_at: row.last_authenticated_at,
        last_error_code: row.last_error_code,
        last_error_message: row.last_error_message,
        metadata: row.metadata,
    })
}

fn parse_provider_auth_method(value: &str) -> Result<ProviderAuthMethod, StoreError> {
    match value {
        "oauth_device_code" => Ok(ProviderAuthMethod::OauthDeviceCode),
        "secret_input" => Ok(ProviderAuthMethod::SecretInput),
        "external_manual" => Ok(ProviderAuthMethod::ExternalManual),
        "none" => Ok(ProviderAuthMethod::None),
        _ => invalid_enum("provider_auth_method", value),
    }
}

fn parse_provider_account_status(value: &str) -> Result<ProviderAccountStatus, StoreError> {
    match value {
        "unknown" => Ok(ProviderAccountStatus::Unknown),
        "checking" => Ok(ProviderAccountStatus::Checking),
        "authenticated" => Ok(ProviderAccountStatus::Authenticated),
        "unauthenticated" => Ok(ProviderAccountStatus::Unauthenticated),
        "unavailable" => Ok(ProviderAccountStatus::Unavailable),
        _ => invalid_enum("provider_account_status", value),
    }
}

const fn provider_status_str(status: ProviderAccountStatus) -> &'static str {
    match status {
        ProviderAccountStatus::Unknown => "unknown",
        ProviderAccountStatus::Checking => "checking",
        ProviderAccountStatus::Authenticated => "authenticated",
        ProviderAccountStatus::Unauthenticated => "unauthenticated",
        ProviderAccountStatus::Unavailable => "unavailable",
    }
}

fn invalid_enum<T>(kind: &'static str, value: &str) -> Result<T, StoreError> {
    Err(StoreError::InvalidEnum {
        kind,
        value: value.to_string(),
    })
}
