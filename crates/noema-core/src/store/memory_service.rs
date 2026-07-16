//! Memory service store records and SQLite repository methods.

use rusqlite::{OptionalExtension, params};

use noema_providers::ReasoningEffort;

use super::{NoemaStore, StoreError, sqlite};

/// Saved memory service mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryServiceMode {
    /// Noema manages a local memory service child process.
    Managed,
    /// Noema connects to an externally managed memory service.
    External,
}

impl MemoryServiceMode {
    /// Return the SQLite representation.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Managed => "managed",
            Self::External => "external",
        }
    }

    /// Parse the SQLite representation.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when `value` is not a known memory service mode.
    pub fn parse(value: &str) -> Result<Self, StoreError> {
        match value {
            "managed" => Ok(Self::Managed),
            "external" => Ok(Self::External),
            _ => Err(StoreError::InvalidEnum {
                kind: "memory service mode",
                value: value.to_string(),
            }),
        }
    }
}

/// Persisted memory service settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryServiceSettingsRecord {
    /// Stable singleton settings id.
    pub settings_id: String,
    /// Memory service mode.
    pub mode: MemoryServiceMode,
    /// External memory service base URL.
    pub base_url: Option<String>,
    /// External service port, when configured.
    pub port: Option<u16>,
    /// Provider account used for memory extraction.
    pub provider_account_id: Option<String>,
    /// Provider kind used for memory extraction.
    pub provider_kind: Option<String>,
    /// Model profile used for memory extraction.
    pub model_profile: Option<String>,
    /// Reasoning effort used for memory extraction.
    pub reasoning_effort: Option<ReasoningEffort>,
}

/// Input for saving memory service settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveMemoryServiceSettings {
    /// Memory service mode.
    pub mode: MemoryServiceMode,
    /// External memory service base URL.
    pub base_url: Option<String>,
    /// External service port, when configured.
    pub port: Option<u16>,
    /// Provider account used for memory extraction.
    pub provider_account_id: Option<String>,
    /// Provider kind used for memory extraction.
    pub provider_kind: Option<String>,
    /// Model profile used for memory extraction.
    pub model_profile: Option<String>,
    /// Reasoning effort used for memory extraction.
    pub reasoning_effort: Option<ReasoningEffort>,
}

/// Cached AI-written memory article.
#[derive(Debug, Clone, PartialEq)]
pub struct MemoryArticleCacheRecord {
    /// Memory scope this article describes.
    pub scope_id: String,
    /// Fingerprint of the facts used to generate the article.
    pub fact_fingerprint: String,
    /// Cached article Markdown.
    pub article_markdown: String,
    /// Timestamp when the article was generated.
    pub generated_at: String,
}

/// Input for saving a cached memory article.
#[derive(Debug, Clone, PartialEq)]
pub struct SaveMemoryArticleCache {
    /// Memory scope this article describes.
    pub scope_id: String,
    /// Fingerprint of the facts used to generate the article.
    pub fact_fingerprint: String,
    /// Cached article Markdown.
    pub article_markdown: String,
    /// Timestamp when the article was generated.
    pub generated_at: String,
}

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
                       model_profile, reasoning_effort
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
        self.with_connection(|conn| {
            conn.execute(
                r#"
                INSERT INTO memory_service_settings
                  (settings_id, mode, base_url, port, provider_account_id, provider_kind,
                   model_profile, reasoning_effort, updated_at)
                VALUES ('default', ?1, ?2, ?3, ?4, ?5, ?6, ?7, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
                ON CONFLICT(settings_id) DO UPDATE SET
                  mode = excluded.mode,
                  base_url = excluded.base_url,
                  port = excluded.port,
                  provider_account_id = excluded.provider_account_id,
                  provider_kind = excluded.provider_kind,
                  model_profile = excluded.model_profile,
                  reasoning_effort = excluded.reasoning_effort,
                  updated_at = excluded.updated_at
                "#,
                params![
                    input.mode.as_str(),
                    input.base_url,
                    input.port,
                    input.provider_account_id,
                    input.provider_kind,
                    input.model_profile,
                    input.reasoning_effort.map(ReasoningEffort::as_persistence_str),
                ],
            )?;
            Ok(())
        })
        .await?;
        self.memory_service_settings().await
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
    let reasoning_effort: Option<String> = row.get(7)?;
    Ok(MemoryServiceSettingsRecord {
        settings_id: row.get(0)?,
        mode: MemoryServiceMode::parse(&mode).map_err(to_sqlite_from_sql_conversion_failure)?,
        base_url: row.get(2)?,
        port,
        provider_account_id: row.get(4)?,
        provider_kind: row.get(5)?,
        model_profile: row.get(6)?,
        reasoning_effort: reasoning_effort
            .as_deref()
            .and_then(ReasoningEffort::from_persistence_str),
    })
}

fn to_sqlite_from_sql_conversion_failure(error: StoreError) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error))
}
