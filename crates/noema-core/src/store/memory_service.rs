//! Memory service store records and SQLite repository methods.

use rusqlite::{OptionalExtension, params};

use crate::provider::ReasoningEffort;

use super::{NoemaStore, StoreError};

/// Saved memory service mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryServiceMode {
    /// Noema manages a local Supermemory child process.
    Managed,
    /// Noema connects to an externally managed Supermemory service.
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
    /// Supermemory service mode.
    pub mode: MemoryServiceMode,
    /// External Supermemory service base URL.
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
    /// Supermemory service mode.
    pub mode: MemoryServiceMode,
    /// External Supermemory service base URL.
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
                    input.reasoning_effort.map(reasoning_effort_as_str),
                ],
            )?;
            Ok(())
        })
        .await?;
        self.memory_service_settings().await
    }
}

fn reasoning_effort_as_str(reasoning_effort: ReasoningEffort) -> &'static str {
    match reasoning_effort {
        ReasoningEffort::None => "none",
        ReasoningEffort::Minimal => "minimal",
        ReasoningEffort::Low => "low",
        ReasoningEffort::Medium => "medium",
        ReasoningEffort::High => "high",
        ReasoningEffort::XHigh => "xhigh",
    }
}

fn reasoning_effort_from_str(reasoning_effort: &str) -> Option<ReasoningEffort> {
    match reasoning_effort {
        "none" => Some(ReasoningEffort::None),
        "minimal" => Some(ReasoningEffort::Minimal),
        "low" => Some(ReasoningEffort::Low),
        "medium" => Some(ReasoningEffort::Medium),
        "high" => Some(ReasoningEffort::High),
        "xhigh" => Some(ReasoningEffort::XHigh),
        _ => None,
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
            .and_then(reasoning_effort_from_str),
    })
}

fn to_sqlite_from_sql_conversion_failure(error: StoreError) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error))
}
