use rusqlite::{OptionalExtension, params};
use serde_json::Value;

use crate::{
    ids::{allocate_id, invalid_enum, now_string},
    sqlite::{deserialize_json, serialize_json},
};
use noema_providers::{
    NewProviderAccount, ProviderAccountRecord, ProviderAccountStatus, ProviderAccountStatusUpdate,
    ProviderAuthMethod, capabilities_for_provider_account,
};

use super::{NoemaStore, StoreError};

#[derive(Clone, Copy)]
struct BuiltinProviderAccount {
    id: &'static str,
    kind: &'static str,
    display_name: &'static str,
    auth_method: &'static str,
    metadata_json: &'static str,
    refresh_on_conflict: bool,
}

impl BuiltinProviderAccount {
    const CODEX: Self = Self {
        id: "provider_account:codex:default",
        kind: "codex",
        display_name: "Codex",
        auth_method: "oauth_device_code",
        metadata_json: "{}",
        refresh_on_conflict: false,
    };
    const OPENAI: Self = Self {
        id: "provider_account:openai:default",
        kind: "openai",
        display_name: "OpenAI",
        auth_method: "external_manual",
        metadata_json: "{}",
        refresh_on_conflict: true,
    };
    const FOUNDATION_LOCAL: Self = Self {
        id: "provider_account:foundation_local:default",
        kind: "foundation_local",
        display_name: "Apple Foundation Models",
        auth_method: "none",
        metadata_json: r#"{"profiles":[{"id":"default","label":"Default on-device"}]}"#,
        refresh_on_conflict: false,
    };
    const LOCAL_MODELS: Self = Self {
        id: "provider_account:local_models:default",
        kind: "local_models",
        display_name: "Local models",
        auth_method: "none",
        metadata_json: "{}",
        refresh_on_conflict: true,
    };
}

impl NoemaStore {
    /// Create or return the default Codex provider account metadata.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the embedded store write or read fails.
    pub async fn ensure_default_provider_account(
        &self,
    ) -> Result<ProviderAccountRecord, StoreError> {
        self.ensure_builtin_provider_account(BuiltinProviderAccount::CODEX)
            .await
    }

    /// Create or return the configured OpenAI provider account metadata.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the embedded store write or read fails.
    pub async fn ensure_default_openai_provider_account(
        &self,
    ) -> Result<ProviderAccountRecord, StoreError> {
        self.ensure_builtin_provider_account(BuiltinProviderAccount::OPENAI)
            .await
    }

    /// Create or return the default Apple Foundation Models provider account metadata.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the embedded store write or read fails.
    pub async fn ensure_default_foundation_local_provider_account(
        &self,
    ) -> Result<ProviderAccountRecord, StoreError> {
        self.ensure_builtin_provider_account(BuiltinProviderAccount::FOUNDATION_LOCAL)
            .await
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
        self.ensure_builtin_provider_account(BuiltinProviderAccount::LOCAL_MODELS)
            .await
    }

    async fn ensure_builtin_provider_account(
        &self,
        account: BuiltinProviderAccount,
    ) -> Result<ProviderAccountRecord, StoreError> {
        self.with_connection(|conn| {
            conn.execute(
                r#"
                INSERT INTO provider_accounts (
                  provider_account_id, provider_kind, account_key, display_name,
                  auth_method, is_active, is_default, status, metadata_json
                ) VALUES (?1, ?2, 'default', ?3, ?4, 1, 1, 'unknown', ?5)
                ON CONFLICT(provider_account_id) DO UPDATE SET
                  provider_kind = excluded.provider_kind,
                  account_key = excluded.account_key,
                  display_name = excluded.display_name,
                  auth_method = excluded.auth_method,
                  is_active = 1,
                  is_default = 1,
                  updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                WHERE ?6
                "#,
                params![
                    account.id,
                    account.kind,
                    account.display_name,
                    account.auth_method,
                    account.metadata_json,
                    account.refresh_on_conflict,
                ],
            )?;
            Ok(())
        })
        .await?;
        self.get_provider_account(account.id).await?.ok_or_else(|| {
            StoreError::ProviderAccountNotFound {
                provider_account_id: account.id.to_string(),
            }
        })
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
        let (account_key, provider_account_id, default_name, is_default) =
            match input.provider_kind.as_str() {
                "exa" if input.auth_method == ProviderAuthMethod::SecretInput => {
                    let account_key = generated_account_key("exa");
                    (
                        account_key.clone(),
                        format!("provider_account:exa:{account_key}"),
                        "Exa",
                        false,
                    )
                }
                "codex" if input.auth_method == ProviderAuthMethod::OauthDeviceCode => (
                    "default".to_string(),
                    "provider_account:codex:default".to_string(),
                    "Codex",
                    true,
                ),
                "openrouter"
                    if matches!(
                        input.auth_method,
                        ProviderAuthMethod::OauthPkce | ProviderAuthMethod::SecretInput
                    ) =>
                {
                    (
                        "default".to_string(),
                        "provider_account:openrouter:default".to_string(),
                        "OpenRouter",
                        true,
                    )
                }
                _ => {
                    return Err(StoreError::InvalidEnum {
                        kind: "provider_kind_or_auth_method",
                        value: format!("{}:{}", input.provider_kind, input.auth_method.as_str()),
                    });
                }
            };
        if input.provider_kind == "exa" && input.auth_method != ProviderAuthMethod::SecretInput {
            return Err(StoreError::InvalidEnum {
                kind: "provider_auth_method",
                value: input.auth_method.as_str().to_string(),
            });
        }

        let display_name = input
            .display_name
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(default_name)
            .to_string();
        let metadata_json = serialize_json(&input.metadata)?;

        self.with_connection(|conn| {
            conn.execute(
                r#"
                INSERT INTO provider_accounts (
                  provider_account_id, provider_kind, account_key, display_name,
                  auth_method, is_active, is_default, status, metadata_json
                )
                VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6, ?7, ?8)
                "#,
                params![
                    provider_account_id,
                    input.provider_kind,
                    account_key,
                    display_name,
                    input.auth_method.as_str(),
                    i64::from(is_default),
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

    /// Hard-delete one unreferenced provider account and its capability bindings.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the account is still referenced,
    /// or when the embedded store delete fails. A missing account returns
    /// `Ok(false)`.
    pub async fn delete_provider_account(
        &self,
        provider_account_id: &str,
    ) -> Result<bool, StoreError> {
        self.with_immediate_transaction_retry(|transaction| {
            let provider_kind = transaction
                .query_row(
                    "SELECT provider_kind FROM provider_accounts WHERE provider_account_id = ?1",
                    [provider_account_id],
                    |row| row.get::<_, String>(0),
                )
                .optional()?;
            let Some(provider_kind) = provider_kind else {
                return Ok(false);
            };
            if matches!(
                provider_kind.as_str(),
                "duckduckgo_public" | "direct_http" | "obscura"
            ) {
                return Err(StoreError::ProviderAccountInUse {
                    provider_account_id: provider_account_id.to_string(),
                });
            }
            if provider_account_is_referenced(transaction, provider_account_id)? {
                return Err(StoreError::ProviderAccountInUse {
                    provider_account_id: provider_account_id.to_string(),
                });
            }

            transaction.execute(
                "DELETE FROM provider_capability_bindings WHERE provider_account_id = ?1",
                [provider_account_id],
            )?;
            let changed = transaction.execute(
                "DELETE FROM provider_accounts WHERE provider_account_id = ?1",
                [provider_account_id],
            )?;
            if changed != 1 {
                return Err(StoreError::InvariantViolation {
                    message: "guarded provider account delete changed an unexpected row count"
                        .to_string(),
                });
            }
            Ok(true)
        })
        .await
    }

    /// Delete one known startup-era placeholder only when no durable evidence
    /// shows that it was ever selected or used.
    ///
    /// # Errors
    ///
    /// Returns a store error when the guarded reconciliation transaction fails.
    pub async fn delete_unused_legacy_provider_account(
        &self,
        provider_account_id: &str,
        provider_kind: &str,
    ) -> Result<bool, StoreError> {
        if !matches!(
            (provider_account_id, provider_kind),
            ("provider_account:codex:default", "codex")
                | ("provider_account:openai:default", "openai")
                | (
                    "provider_account:foundation_local:default",
                    "foundation_local"
                )
                | ("provider_account:local_models:default", "local_models")
        ) {
            return Ok(false);
        }
        self.with_immediate_transaction_retry(|transaction| {
            let exists = transaction.query_row(
                "SELECT EXISTS (SELECT 1 FROM provider_accounts WHERE provider_account_id = ?1 AND provider_kind = ?2 AND account_key = 'default')",
                params![provider_account_id, provider_kind],
                |row| row.get::<_, bool>(0),
            )?;
            if !exists
                || legacy_provider_account_is_referenced(
                    transaction,
                    provider_account_id,
                    provider_kind,
                )?
            {
                return Ok(false);
            }
            Ok(transaction.execute(
                "DELETE FROM provider_accounts WHERE provider_account_id = ?1 AND provider_kind = ?2",
                params![provider_account_id, provider_kind],
            )? == 1)
        })
        .await
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
        self.update_provider_account_fields(
            provider_account_id,
            None,
            Some(&ProviderAccountStatusUpdate {
                status,
                error_code: error_code.map(str::to_string),
                error_message: error_message.map(str::to_string),
            }),
            None,
        )
        .await
        .map(|_| ())
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
        self.update_provider_account_fields(provider_account_id, None, None, Some(&metadata))
            .await
            .map(|_| ())
    }

    pub(super) async fn update_provider_account_fields(
        &self,
        provider_account_id: &str,
        auth_method: Option<ProviderAuthMethod>,
        status: Option<&ProviderAccountStatusUpdate>,
        metadata: Option<&Value>,
    ) -> Result<ProviderAccountRecord, StoreError> {
        let metadata_json = metadata.map(serialize_json).transpose()?;
        self.with_immediate_transaction_retry(|transaction| {
            let current = transaction
                .query_row(
                    format!("{PROVIDER_ACCOUNT_SELECT} WHERE provider_account_id = ?1 LIMIT 1")
                        .as_str(),
                    [provider_account_id],
                    provider_account_row,
                )
                .optional()?
                .ok_or_else(|| StoreError::ProviderAccountNotFound {
                    provider_account_id: provider_account_id.to_string(),
                })?;
            let current = provider_account_from_row(current)?;
            let checked_at = status.map(|_| now_string());
            let resulting_status = status.map_or(current.status, |update| update.status);
            let last_authenticated_at =
                if resulting_status == ProviderAccountStatus::Authenticated && status.is_some() {
                    checked_at.as_deref()
                } else {
                    current.last_authenticated_at.as_deref()
                };
            let error_code = status
                .map(|update| update.error_code.as_deref())
                .unwrap_or(current.last_error_code.as_deref());
            let error_message = status
                .map(|update| update.error_message.as_deref())
                .unwrap_or(current.last_error_message.as_deref());
            let changed = transaction.execute(
                r#"
                UPDATE provider_accounts
                SET status = ?2,
                    auth_method = COALESCE(?8, auth_method),
                    last_checked_at = COALESCE(?3, last_checked_at),
                    last_authenticated_at = ?4,
                    last_error_code = ?5,
                    last_error_message = ?6,
                    metadata_json = COALESCE(?7, metadata_json),
                    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                WHERE provider_account_id = ?1
                "#,
                params![
                    provider_account_id,
                    provider_status_str(resulting_status),
                    checked_at,
                    last_authenticated_at,
                    error_code,
                    error_message,
                    metadata_json,
                    auth_method.map(ProviderAuthMethod::as_str),
                ],
            )?;
            if changed != 1 {
                return Err(StoreError::InvariantViolation {
                    message: "guarded provider account update changed an unexpected row count"
                        .to_string(),
                });
            }
            let updated = transaction.query_row(
                format!("{PROVIDER_ACCOUNT_SELECT} WHERE provider_account_id = ?1 LIMIT 1")
                    .as_str(),
                [provider_account_id],
                provider_account_row,
            )?;
            provider_account_from_row(updated)
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

/// Return whether any canonical selection or future-lease-eligible workload
/// still names an account. This must be evaluated in the same immediate
/// transaction as deletion so reference writers serialize with the guard.
fn provider_account_is_referenced(
    transaction: &rusqlite::Transaction<'_>,
    provider_account_id: &str,
) -> Result<bool, StoreError> {
    transaction
        .query_row(
            r#"
            SELECT EXISTS (
              SELECT 1 FROM default_model_preference
              WHERE provider_account_id = ?1
              UNION ALL
              SELECT 1 FROM agent_runtime_preferences
              WHERE provider_account_id = ?1
              UNION ALL
              SELECT 1 FROM auxiliary_model_preferences
              WHERE provider_account_id = ?1
              UNION ALL
              SELECT 1 FROM task_model_pool_entries
              WHERE provider_account_id = ?1
              UNION ALL
              SELECT 1
              FROM task_execution_contracts AS contract
              JOIN tasks AS task ON task.task_id = contract.task_id
                                AND task.current_contract_id = contract.contract_id
              JOIN workflow_stages AS stage
                ON stage.workflow_id = task.workflow_id
               AND stage.stage_id = task.stage_id
              WHERE contract.executor_provider_account_id = ?1
                AND stage.system_behavior NOT IN ('terminal_success', 'terminal_cancelled')
              UNION ALL
              SELECT 1
              FROM task_execution_contracts AS contract
              JOIN tasks AS task ON task.task_id = contract.task_id
                                AND task.current_contract_id = contract.contract_id
              JOIN workflow_stages AS stage
                ON stage.workflow_id = task.workflow_id
               AND stage.stage_id = task.stage_id
              WHERE contract.reviewer_provider_account_id = ?1
                AND stage.system_behavior NOT IN ('terminal_success', 'terminal_cancelled')
              UNION ALL
              SELECT 1
              FROM agent_runs
              LEFT JOIN tasks ON tasks.task_id = agent_runs.task_id
              LEFT JOIN workflow_stages AS stage
                ON stage.workflow_id = tasks.workflow_id
               AND stage.stage_id = tasks.stage_id
              WHERE agent_runs.provider_account_id = ?1
                AND agent_runs.task_generation = tasks.generation
                AND stage.system_behavior NOT IN ('terminal_success', 'terminal_cancelled')
                AND (
                  agent_runs.status IN ('queued', 'leased', 'running', 'waiting_for_approval')
                  OR (
                    agent_runs.status = 'interrupted'
                    AND agent_runs.cancellation_requested = 0
                    AND tasks.generation = agent_runs.task_generation
                  )
                )
            )
            "#,
            [provider_account_id],
            |row| row.get::<_, bool>(0),
        )
        .map_err(StoreError::Sqlite)
}

fn legacy_provider_account_is_referenced(
    transaction: &rusqlite::Transaction<'_>,
    provider_account_id: &str,
    provider_kind: &str,
) -> Result<bool, StoreError> {
    transaction
        .query_row(
            r#"
            SELECT EXISTS (
              SELECT 1 FROM default_model_preference WHERE provider_account_id = ?1
              UNION ALL SELECT 1 FROM agent_runtime_preferences WHERE provider_account_id = ?1
              UNION ALL SELECT 1 FROM auxiliary_model_preferences WHERE provider_account_id = ?1
              UNION ALL SELECT 1 FROM task_model_pool_entries WHERE provider_account_id = ?1
              UNION ALL SELECT 1 FROM provider_capability_bindings WHERE provider_account_id = ?1
              UNION ALL SELECT 1 FROM task_execution_contracts
                WHERE executor_provider_account_id = ?1 OR reviewer_provider_account_id = ?1
              UNION ALL SELECT 1 FROM agent_runs WHERE provider_account_id = ?1
              UNION ALL SELECT 1 FROM conversations WHERE provider = ?2
              UNION ALL SELECT 1 FROM conversation_context_summaries
                WHERE provider_kind = ?2 OR compaction_provider_kind = ?2
              UNION ALL SELECT 1 FROM local_model_installations WHERE ?2 = 'local_models'
            )
            "#,
            params![provider_account_id, provider_kind],
            |row| row.get::<_, bool>(0),
        )
        .map_err(StoreError::Sqlite)
}

pub(super) const PROVIDER_ACCOUNT_SELECT: &str = r#"
SELECT provider_account_id, provider_kind, account_key, display_name,
  auth_method, is_active, is_default, status, last_checked_at,
  last_authenticated_at, last_error_code, last_error_message, metadata_json
FROM provider_accounts
"#;

#[derive(Debug)]
pub(super) struct ProviderAccountRow {
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

pub(super) fn provider_account_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<ProviderAccountRow> {
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

pub(super) fn provider_account_from_row(
    row: ProviderAccountRow,
) -> Result<ProviderAccountRecord, StoreError> {
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
        metadata: deserialize_json(row.metadata_json)?,
        capabilities,
    })
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
        "oauth_pkce" => Ok(ProviderAuthMethod::OauthPkce),
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

pub(super) const fn provider_status_str(status: ProviderAccountStatus) -> &'static str {
    match status {
        ProviderAccountStatus::Unknown => "unknown",
        ProviderAccountStatus::Checking => "checking",
        ProviderAccountStatus::Authenticated => "authenticated",
        ProviderAccountStatus::Unauthenticated => "unauthenticated",
        ProviderAccountStatus::Unavailable => "unavailable",
    }
}
