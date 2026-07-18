//! Memory service store records and SQLite repository methods.

use rusqlite::{OptionalExtension, params};

use noema_memory::{
    MemoryArticleCacheRecord, MemoryServiceMode, MemoryServiceSettingsRecord,
    SaveMemoryArticleCache, SaveMemoryServiceSettings,
};
use noema_providers::{ProviderReadySelection, ProviderSelectionSnapshot, ReasoningEffort};

use super::{
    NoemaStore, StoreError, provider_selections::resolve_new_canonical_selection_tx, sqlite,
};

impl NoemaStore {
    /// Return the singleton memory service settings record.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the embedded store read fails or the row is missing.
    pub async fn memory_service_settings(&self) -> Result<MemoryServiceSettingsRecord, StoreError> {
        self.with_connection(|conn| {
            conn.query_row(
                r#"
                SELECT settings_id, mode, base_url, port, provider_account_id, provider_kind,
                       provider_instance_key, model_profile, reasoning_effort
                FROM memory_service_settings
                WHERE settings_id = 'default'
                LIMIT 1
                "#,
                [],
                settings_from_row,
            )
            .optional()?
            .ok_or_else(|| StoreError::InvariantViolation {
                message: "default memory service settings row is missing".to_string(),
            })
        })
        .await
    }

    /// Create or update the singleton memory service settings record.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the embedded store write/read fails.
    pub async fn save_memory_service_settings(
        &self,
        input: SaveMemoryServiceSettings,
    ) -> Result<MemoryServiceSettingsRecord, StoreError> {
        self.save_memory_service_settings_inner(input, None).await
    }

    /// Save memory settings while retaining a registry readiness lease through
    /// commit. Every explicit provider selection must use this API.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the settings or provider selection is
    /// invalid, the proof does not cover the exact route, or SQLite fails.
    pub async fn save_memory_service_settings_with_ready_selection(
        &self,
        input: SaveMemoryServiceSettings,
        ready_selection: &ProviderReadySelection,
    ) -> Result<MemoryServiceSettingsRecord, StoreError> {
        self.save_memory_service_settings_inner(input, Some(ready_selection))
            .await
    }

    async fn save_memory_service_settings_inner(
        &self,
        input: SaveMemoryServiceSettings,
        ready_selection: Option<&ProviderReadySelection>,
    ) -> Result<MemoryServiceSettingsRecord, StoreError> {
        self.with_immediate_transaction_retry(|transaction| {
            let explicit_selection = input.provider_kind.is_some()
                || input.provider_account_id.is_some()
                || input.model_profile.is_some();
            let selection = memory_selection_from_input(transaction, &input)?;
            let selection = if explicit_selection {
                selection
                    .as_ref()
                    .map(|selection| {
                        resolve_new_canonical_selection_tx(transaction, selection, ready_selection)
                    })
                    .transpose()?
            } else {
                selection
            };
            transaction.execute(
                r#"
                INSERT INTO memory_service_settings
                  (settings_id, mode, base_url, port, provider_account_id, provider_kind,
                   provider_instance_key, model_profile, reasoning_effort, updated_at)
                VALUES ('default', ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8,
                        strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
                ON CONFLICT(settings_id) DO UPDATE SET
                  mode = excluded.mode,
                  base_url = excluded.base_url,
                  port = excluded.port,
                  provider_account_id = excluded.provider_account_id,
                  provider_kind = excluded.provider_kind,
                  provider_instance_key = excluded.provider_instance_key,
                  model_profile = excluded.model_profile,
                  reasoning_effort = excluded.reasoning_effort,
                  updated_at = excluded.updated_at
                "#,
                params![
                    input.mode.as_str(),
                    &input.base_url,
                    input.port,
                    selection
                        .as_ref()
                        .map(|selection| &selection.provider_account_id),
                    selection.as_ref().map(|selection| &selection.provider_kind),
                    selection
                        .as_ref()
                        .and_then(|selection| selection.provider_instance_key.as_ref())
                        .map(noema_providers::ProviderInstanceKey::as_str),
                    selection
                        .as_ref()
                        .and_then(|selection| selection.model_profile.as_deref()),
                    selection
                        .as_ref()
                        .and_then(|selection| selection.reasoning_effort)
                        .map(ReasoningEffort::as_persistence_str),
                ],
            )?;
            transaction
                .query_row(
                    r#"
                    SELECT settings_id, mode, base_url, port, provider_account_id,
                           provider_kind, provider_instance_key, model_profile, reasoning_effort
                    FROM memory_service_settings
                    WHERE settings_id = 'default'
                    LIMIT 1
                    "#,
                    [],
                    settings_from_row,
                )
                .map_err(StoreError::Sqlite)
        })
        .await
    }

    /// Return the cached AI-written memory article for a scope.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the embedded store read fails.
    pub async fn memory_article_cache(
        &self,
        scope_id: &str,
    ) -> Result<Option<MemoryArticleCacheRecord>, StoreError> {
        self.with_connection(|conn| {
            sqlite::optional_row(
                conn,
                r#"
                SELECT scope_id, fact_fingerprint, article_markdown, generated_at
                FROM memory_article_cache
                WHERE scope_id = ?1
                LIMIT 1
                "#,
                [scope_id],
                |row| {
                    Ok(MemoryArticleCacheRecord {
                        scope_id: row.get(0)?,
                        fact_fingerprint: row.get(1)?,
                        article_markdown: row.get(2)?,
                        generated_at: row.get(3)?,
                    })
                },
            )
        })
        .await
    }

    /// Save the cached AI-written memory article for a scope.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the embedded store write fails.
    pub async fn save_memory_article_cache(
        &self,
        input: SaveMemoryArticleCache,
    ) -> Result<(), StoreError> {
        self.with_connection(|conn| {
            conn.execute(
                r#"
                INSERT INTO memory_article_cache
                  (scope_id, fact_fingerprint, article_markdown, generated_at, updated_at)
                VALUES (?1, ?2, ?3, ?4, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
                ON CONFLICT(scope_id) DO UPDATE SET
                  fact_fingerprint = excluded.fact_fingerprint,
                  article_markdown = excluded.article_markdown,
                  generated_at = excluded.generated_at,
                  updated_at = excluded.updated_at
                "#,
                params![
                    input.scope_id,
                    input.fact_fingerprint,
                    input.article_markdown,
                    input.generated_at,
                ],
            )?;
            Ok(())
        })
        .await
    }
}

fn settings_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<MemoryServiceSettingsRecord> {
    let mode: String = row.get(1)?;
    let port: Option<u16> = row.get(3)?;
    let provider_instance_key = row
        .get::<_, Option<String>>(6)?
        .map(noema_providers::ProviderInstanceKey::new)
        .transpose()
        .map_err(to_sqlite_from_provider_selection_failure)?;
    let reasoning_effort: Option<String> = row.get(8)?;
    Ok(MemoryServiceSettingsRecord {
        settings_id: row.get(0)?,
        mode: MemoryServiceMode::parse(&mode).map_err(to_sqlite_from_sql_conversion_failure)?,
        base_url: row.get(2)?,
        port,
        provider_account_id: row.get(4)?,
        provider_kind: row.get(5)?,
        provider_instance_key,
        model_profile: row.get(7)?,
        reasoning_effort: reasoning_effort
            .as_deref()
            .and_then(ReasoningEffort::from_persistence_str),
    })
}

fn memory_selection_from_input(
    transaction: &rusqlite::Transaction<'_>,
    input: &SaveMemoryServiceSettings,
) -> Result<Option<ProviderSelectionSnapshot>, StoreError> {
    match (
        input.provider_kind.as_deref(),
        input.provider_account_id.as_deref(),
        input.model_profile.as_deref(),
    ) {
        (None, None, None) => {
            let current = transaction
                .query_row(
                    r#"
                    SELECT settings_id, mode, base_url, port, provider_account_id,
                           provider_kind, provider_instance_key, model_profile, reasoning_effort
                    FROM memory_service_settings
                    WHERE settings_id = 'default'
                    LIMIT 1
                    "#,
                    [],
                    settings_from_row,
                )
                .optional()?;
            let Some(current) = current else {
                return Ok(None);
            };
            match noema_memory::memory_provider_selection(&current) {
                Ok(selection) => Ok(Some(selection)),
                Err(_) if input.mode == MemoryServiceMode::External => Ok(None),
                Err(_) => Err(StoreError::InvariantViolation {
                    message: "managed memory provider selection is not initialized".to_string(),
                }),
            }
        }
        (Some(provider_kind), Some(provider_account_id), Some(model_profile)) => {
            Ok(Some(ProviderSelectionSnapshot::explicit(
                provider_kind,
                provider_account_id,
                model_profile,
                input.reasoning_effort,
                Some("memory_service_settings".to_string()),
            )))
        }
        _ => Err(StoreError::InvariantViolation {
            message: "memory provider kind, account, and model must be saved together".to_string(),
        }),
    }
}

fn to_sqlite_from_provider_selection_failure(
    error: noema_providers::ProviderSelectionError,
) -> rusqlite::Error {
    sqlite::conversion_failure(6, rusqlite::types::Type::Text, error)
}

fn to_sqlite_from_sql_conversion_failure(
    error: noema_memory::MemorySettingsError,
) -> rusqlite::Error {
    sqlite::conversion_failure(0, rusqlite::types::Type::Text, error)
}
