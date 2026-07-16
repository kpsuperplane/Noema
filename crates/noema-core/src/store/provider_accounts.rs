use rusqlite::{OptionalExtension, params};
use serde_json::Value;

use crate::store::{
    ids::{allocate_id, invalid_enum, now_string},
    sqlite::{json_from_string, json_to_string},
};
use noema_providers::{
    NewProviderAccount, ProviderAccountCatalogEntry, ProviderAccountRecord, ProviderAccountStatus,
    ProviderAuthMethod, capabilities_for_provider_account,
};

use super::{NoemaStore, StoreError};

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

    /// Create or return the default Codex provider account metadata.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the embedded store write or read fails.
    pub async fn ensure_default_provider_account(
        &self,
    ) -> Result<ProviderAccountRecord, StoreError> {
        self.with_connection(|conn| {
            conn.execute(
                r#"
                INSERT INTO provider_accounts (
                  provider_account_id, provider_kind, account_key, display_name,
                  auth_method, is_active, is_default, status, metadata_json
                )
                VALUES (
                  'provider_account:codex:default', 'codex', 'default', 'Codex',
                  'oauth_device_code', 1, 1, 'unknown', '{}'
                )
                ON CONFLICT(provider_account_id) DO NOTHING
                "#,
                [],
            )?;
            Ok(())
        })
        .await?;
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
        self.with_connection(|conn| {
            conn.execute(
                r#"
                INSERT INTO provider_accounts (
                  provider_account_id, provider_kind, account_key, display_name,
                  auth_method, is_active, is_default, status, metadata_json
                )
                VALUES (
                  'provider_account:foundation_local:default',
                  'foundation_local',
                  'default',
                  'Apple Foundation Models',
                  'none',
                  1,
                  1,
                  'unknown',
                  '{"profiles":[{"id":"default","label":"Default on-device"}]}'
                )
                ON CONFLICT(provider_account_id) DO NOTHING
                "#,
                [],
            )?;
            Ok(())
        })
        .await?;
        self.get_provider_account(ACCOUNT_ID).await?.ok_or_else(|| {
            StoreError::ProviderAccountNotFound {
                provider_account_id: ACCOUNT_ID.to_string(),
            }
        })
    }

    /// Create or return the built-in local-model provider account metadata.
    ///
    /// The account itself requires no authentication. Its effective
    /// availability is derived from installed model state by the local runtime.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the embedded store write or read fails.
    pub async fn ensure_default_local_models_provider_account(
        &self,
    ) -> Result<ProviderAccountRecord, StoreError> {
        const ACCOUNT_ID: &str = "provider_account:local_models:default";
        self.with_connection(|conn| {
            conn.execute(
                r#"
                INSERT INTO provider_accounts (
                  provider_account_id, provider_kind, account_key, display_name,
                  auth_method, is_active, is_default, status, metadata_json
                )
                VALUES (
                  'provider_account:local_models:default',
                  'local_models',
                  'default',
                  'Local models',
                  'none',
                  1,
                  1,
                  'unknown',
                  '{}'
                )
                ON CONFLICT(provider_account_id) DO UPDATE SET
                  provider_kind = excluded.provider_kind,
                  account_key = excluded.account_key,
                  display_name = excluded.display_name,
                  auth_method = excluded.auth_method,
                  is_active = 1,
                  is_default = 1,
                  updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                "#,
                [],
            )?;
            Ok(())
        })
        .await?;
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
        let row = self
            .with_connection(|conn| {
                conn.query_row(
                    format!("{PROVIDER_ACCOUNT_SELECT} WHERE provider_kind = ?1 AND is_active = 1 AND is_default = 1 LIMIT 1").as_str(),
                    [provider_kind],
                    provider_account_row,
                )
                .optional()
                .map_err(StoreError::Sqlite)
            })
            .await?;
        row.map(provider_account_from_row).transpose()
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
        self.provider_account_rows("WHERE is_active = 1 AND is_default = 1 ORDER BY provider_kind")
            .await
    }

    /// Return all active created provider accounts in stable Settings order.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the embedded store read fails or a stored
    /// enum is invalid.
    pub async fn active_provider_accounts(&self) -> Result<Vec<ProviderAccountRecord>, StoreError> {
        self.provider_account_rows(
            "WHERE is_active = 1 ORDER BY provider_kind, display_name, account_key",
        )
        .await
    }

    /// Return all durable provider accounts in stable Settings order.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the embedded store read fails or a stored
    /// enum is invalid.
    pub async fn list_provider_accounts(&self) -> Result<Vec<ProviderAccountRecord>, StoreError> {
        self.provider_account_rows("ORDER BY provider_kind, display_name, account_key")
            .await
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
        let display_name = input
            .display_name
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("Exa")
            .to_string();
        let metadata_json = json_to_string(&input.metadata)?;

        self.with_connection(|conn| {
            conn.execute(
                r#"
                INSERT INTO provider_accounts (
                  provider_account_id, provider_kind, account_key, display_name,
                  auth_method, is_active, is_default, status, metadata_json
                )
                VALUES (?1, ?2, ?3, ?4, ?5, 1, 0, ?6, ?7)
                "#,
                params![
                    provider_account_id,
                    input.provider_kind,
                    account_key,
                    display_name,
                    input.auth_method.as_str(),
                    input.status.as_str(),
                    metadata_json,
                ],
            )?;
            Ok(())
        })
        .await?;

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
        let row = self
            .with_connection(|conn| {
                conn.query_row(
                    format!("{PROVIDER_ACCOUNT_SELECT} WHERE provider_account_id = ?1 LIMIT 1")
                        .as_str(),
                    [provider_account_id],
                    provider_account_row,
                )
                .optional()
                .map_err(StoreError::Sqlite)
            })
            .await?;
        row.map(provider_account_from_row).transpose()
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

        self.with_connection(|conn| {
            conn.execute(
                "DELETE FROM provider_capability_bindings WHERE provider_account_id = ?1",
                [provider_account_id],
            )?;
            conn.execute(
                "DELETE FROM provider_accounts WHERE provider_account_id = ?1",
                [provider_account_id],
            )?;
            Ok(())
        })
        .await?;
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
        self.with_connection(|conn| {
            conn.execute(
                r#"
                UPDATE provider_accounts
                SET status = ?2,
                    last_checked_at = ?3,
                    last_authenticated_at = ?4,
                    last_error_code = ?5,
                    last_error_message = ?6,
                    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                WHERE provider_account_id = ?1
                "#,
                params![
                    provider_account_id,
                    provider_status_str(status),
                    checked_at,
                    last_authenticated_at,
                    error_code,
                    error_message,
                ],
            )?;
            Ok(())
        })
        .await
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
        let metadata_json = json_to_string(&metadata)?;
        self.with_connection(|conn| {
            conn.execute(
                r#"
                UPDATE provider_accounts
                SET metadata_json = ?2,
                    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                WHERE provider_account_id = ?1
                "#,
                params![provider_account_id, metadata_json],
            )?;
            Ok(())
        })
        .await
    }

    async fn provider_account_rows(
        &self,
        clause: &str,
    ) -> Result<Vec<ProviderAccountRecord>, StoreError> {
        let rows = self
            .with_connection(|conn| {
                let mut statement =
                    conn.prepare(format!("{PROVIDER_ACCOUNT_SELECT} {clause}").as_str())?;
                let rows = statement.query_map([], provider_account_row)?;
                rows.collect::<Result<Vec<_>, _>>()
                    .map_err(StoreError::Sqlite)
            })
            .await?;
        rows.into_iter().map(provider_account_from_row).collect()
    }
}

const PROVIDER_ACCOUNT_SELECT: &str = r#"
SELECT provider_account_id, provider_kind, account_key, display_name,
  auth_method, is_active, is_default, status, last_checked_at,
  last_authenticated_at, last_error_code, last_error_message, metadata_json
FROM provider_accounts
"#;

#[derive(Debug)]
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
    metadata_json: String,
}

fn provider_account_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ProviderAccountRow> {
    Ok(ProviderAccountRow {
        provider_account_id: row.get(0)?,
        provider_kind: row.get(1)?,
        account_key: row.get(2)?,
        display_name: row.get(3)?,
        auth_method: row.get(4)?,
        is_active: row.get(5)?,
        is_default: row.get(6)?,
        status: row.get(7)?,
        last_checked_at: row.get(8)?,
        last_authenticated_at: row.get(9)?,
        last_error_code: row.get(10)?,
        last_error_message: row.get(11)?,
        metadata_json: row.get(12)?,
    })
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
        metadata: json_from_string(row.metadata_json)?,
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
