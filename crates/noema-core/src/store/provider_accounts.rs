use serde::Deserialize;
use serde_json::Value;
use surrealdb::types::SurrealValue;

use crate::{
    ProviderAccountRecord, ProviderAccountStatus, ProviderAuthMethod,
    provider::capabilities_for_provider_account,
    store::ids::{allocate_id, invalid_enum, now_string},
};

use super::{NoemaStore, StoreError, agents::agent_record_fragment};

/// Provider type shown in the Settings add-account catalog.
#[derive(Debug, Clone, PartialEq)]
pub struct ProviderAccountCatalogEntry {
    /// Stable provider family identifier.
    pub provider_kind: String,
    /// Human-readable provider type name.
    pub display_name: String,
    /// Authentication method used for newly created accounts.
    pub auth_method: ProviderAuthMethod,
    /// Capabilities this provider type can supply after account creation.
    pub capabilities: Vec<crate::provider::ProviderCapability>,
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

impl NoemaStore {
    /// Return built-in system provider accounts exposed without durable rows.
    #[must_use]
    pub fn system_provider_accounts(&self) -> Vec<ProviderAccountRecord> {
        vec![
            system_provider_account("duckduckgo_public", "DuckDuckGo public search"),
            system_provider_account("direct_http", "Direct HTTP web fetch"),
        ]
    }

    /// Return provider types that can be added by the user.
    #[must_use]
    pub fn provider_account_catalog(&self) -> Vec<ProviderAccountCatalogEntry> {
        vec![ProviderAccountCatalogEntry {
            provider_kind: "exa".to_string(),
            display_name: "Exa".to_string(),
            auth_method: ProviderAuthMethod::SecretInput,
            capabilities: capabilities_for_provider_account(
                "exa",
                "catalog",
                ProviderAccountStatus::Authenticated,
            ),
        }]
    }

    /// Create or refresh the built-in local human and primary agent.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the embedded store write fails.
    pub async fn ensure_default_actors(&self) -> Result<(), StoreError> {
        self.db()
            .query(
                r#"
                UPSERT type::record('humans', 'human_local') SET
                  human_id = 'human:local',
                  display_name = 'Local Human',
                  updated_at = time::now();
                UPSERT type::record('agents', $agent_record_id) SET
                  agent_id = 'agent:primary',
                  updated_at = time::now();
                "#,
            )
            .bind(("agent_record_id", agent_record_fragment("agent:primary")))
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

        self.db()
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

    /// Create or return the default Apple Foundation Models provider account metadata.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the embedded store write or read fails.
    pub async fn ensure_default_foundation_local_provider_account(
        &self,
    ) -> Result<ProviderAccountRecord, StoreError> {
        const ACCOUNT_ID: &str = "provider_account:foundation_local:default";
        if let Some(account) = self.get_provider_account(ACCOUNT_ID).await? {
            return Ok(account);
        }

        self.db()
            .query(
                r#"
                UPSERT type::record('provider_accounts', 'foundation_local_default') SET
                  provider_account_id = 'provider_account:foundation_local:default',
                  provider_kind = 'foundation_local',
                  account_key = 'default',
                  display_name = 'Apple Foundation Models',
                  auth_method = 'none',
                  is_active = true,
                  is_default = true,
                  status = 'unknown',
                  metadata = { profiles: [{ id: 'default', label: 'Default on-device' }] },
                  updated_at = time::now();
                "#,
            )
            .await?
            .check()?;
        self.get_provider_account(ACCOUNT_ID).await?.ok_or_else(|| {
            StoreError::ProviderAccountNotFound {
                provider_account_id: ACCOUNT_ID.to_string(),
            }
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
            .db()
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

    /// Return all active default provider accounts in stable Settings order.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the embedded store read fails or a stored
    /// enum is invalid.
    pub async fn active_default_provider_accounts(
        &self,
    ) -> Result<Vec<ProviderAccountRecord>, StoreError> {
        let mut response = self
            .db()
            .query(
                r#"
                SELECT provider_account_id, provider_kind, account_key, display_name,
                  auth_method, is_active, is_default, status, last_checked_at,
                  last_authenticated_at, last_error_code, last_error_message, metadata
                FROM provider_accounts
                WHERE is_active = true
                  AND is_default = true;
                "#,
            )
            .await?;
        let rows: Vec<ProviderAccountRow> = response.take(0)?;
        let mut accounts = rows
            .into_iter()
            .map(provider_account_from_row)
            .collect::<Result<Vec<_>, _>>()?;
        accounts.sort_by(|left, right| left.provider_kind.cmp(&right.provider_kind));
        Ok(accounts)
    }

    /// Return all active created provider accounts in stable Settings order.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the embedded store read fails or a stored
    /// enum is invalid.
    pub async fn active_provider_accounts(&self) -> Result<Vec<ProviderAccountRecord>, StoreError> {
        let mut response = self
            .db()
            .query(
                r#"
                SELECT provider_account_id, provider_kind, account_key, display_name,
                  auth_method, is_active, is_default, status, last_checked_at,
                  last_authenticated_at, last_error_code, last_error_message, metadata
                FROM provider_accounts
                WHERE is_active = true;
                "#,
            )
            .await?;
        let rows: Vec<ProviderAccountRow> = response.take(0)?;
        let mut accounts = rows
            .into_iter()
            .map(provider_account_from_row)
            .collect::<Result<Vec<_>, _>>()?;
        accounts.sort_by(|left, right| {
            left.provider_kind
                .cmp(&right.provider_kind)
                .then(left.display_name.cmp(&right.display_name))
                .then(left.account_key.cmp(&right.account_key))
        });
        Ok(accounts)
    }

    /// Create a user-managed provider account.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the provider kind or auth method is not
    /// supported, or when the embedded store write/read fails.
    pub async fn create_provider_account(
        &self,
        input: NewProviderAccount,
    ) -> Result<ProviderAccountRecord, StoreError> {
        if input.provider_kind != "exa" {
            return Err(StoreError::InvalidEnum {
                kind: "provider_kind",
                value: input.provider_kind,
            });
        }
        if input.auth_method != ProviderAuthMethod::SecretInput {
            return Err(StoreError::InvalidEnum {
                kind: "provider_auth_method",
                value: input.auth_method.as_str().to_string(),
            });
        }

        let account_key = generated_account_key(&input.provider_kind);
        let provider_account_id = format!("provider_account:{}:{account_key}", input.provider_kind);
        let record_id = format!("{}_{}", input.provider_kind, account_key);
        let display_name = input
            .display_name
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("Exa")
            .to_string();

        self.db()
            .query(
                r#"
                CREATE type::record('provider_accounts', $record_id) SET
                  provider_account_id = $provider_account_id,
                  provider_kind = $provider_kind,
                  account_key = $account_key,
                  display_name = $display_name,
                  auth_method = $auth_method,
                  is_active = true,
                  is_default = false,
                  status = $status,
                  metadata = $metadata,
                  updated_at = time::now();
                "#,
            )
            .bind(("record_id", record_id))
            .bind(("provider_account_id", provider_account_id.clone()))
            .bind(("provider_kind", input.provider_kind))
            .bind(("account_key", account_key))
            .bind(("display_name", display_name))
            .bind(("auth_method", input.auth_method.as_str().to_string()))
            .bind(("status", input.status.as_str().to_string()))
            .bind(("metadata", input.metadata))
            .await?
            .check()?;

        self.get_provider_account(&provider_account_id)
            .await?
            .ok_or(StoreError::ProviderAccountNotFound {
                provider_account_id,
            })
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
            .db()
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

    /// Hard-delete one user-managed provider account and its capability bindings.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the account is missing, is a default account,
    /// or the embedded store delete fails.
    pub async fn delete_provider_account(
        &self,
        provider_account_id: &str,
    ) -> Result<bool, StoreError> {
        let Some(account) = self.get_provider_account(provider_account_id).await? else {
            return Ok(false);
        };
        if account.is_default {
            return Err(StoreError::InvalidEnum {
                kind: "provider_account_delete_target",
                value: provider_account_id.to_string(),
            });
        }

        self.db()
            .query(
                r#"
                DELETE provider_capability_bindings
                WHERE provider_account_id = $provider_account_id;
                DELETE provider_accounts
                WHERE provider_account_id = $provider_account_id;
                "#,
            )
            .bind(("provider_account_id", provider_account_id.to_string()))
            .await?
            .check()?;
        Ok(true)
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
        self.db()
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

    /// Replace safe non-secret provider account metadata.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the account is missing or the embedded store
    /// write fails.
    pub async fn update_provider_account_metadata(
        &self,
        provider_account_id: &str,
        metadata: Value,
    ) -> Result<(), StoreError> {
        if self
            .get_provider_account(provider_account_id)
            .await?
            .is_none()
        {
            return Err(StoreError::ProviderAccountNotFound {
                provider_account_id: provider_account_id.to_string(),
            });
        }
        self.db()
            .query(
                r#"
                UPDATE provider_accounts SET
                  metadata = $metadata,
                  updated_at = time::now()
                WHERE provider_account_id = $provider_account_id;
                "#,
            )
            .bind(("provider_account_id", provider_account_id.to_string()))
            .bind(("metadata", metadata))
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
    let status = parse_provider_status(&row.status)?;
    let capabilities =
        capabilities_for_provider_account(&row.provider_kind, &row.account_key, status);
    Ok(ProviderAccountRecord {
        provider_account_id: row.provider_account_id,
        provider_kind: row.provider_kind,
        account_key: row.account_key,
        display_name: row.display_name,
        auth_method: parse_provider_auth_method(&row.auth_method)?,
        is_active: row.is_active,
        is_default: row.is_default,
        status,
        last_checked_at: row.last_checked_at,
        last_authenticated_at: row.last_authenticated_at,
        last_error_code: row.last_error_code,
        last_error_message: row.last_error_message,
        metadata: row.metadata,
        capabilities,
    })
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

fn generated_account_key(provider_kind: &str) -> String {
    let allocated = allocate_id("provider_account");
    let suffix = allocated
        .rsplit(':')
        .next()
        .unwrap_or("account")
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();
    format!("acct_{provider_kind}_{suffix}")
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

fn parse_provider_status(value: &str) -> Result<ProviderAccountStatus, StoreError> {
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
