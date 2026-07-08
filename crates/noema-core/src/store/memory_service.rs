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

/// Saved memory service readiness status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryServiceStatus {
    /// Service configuration has not been checked.
    NotConfigured,
    /// Managed service startup is in progress.
    Starting,
    /// Service is reachable and ready.
    Ready,
    /// Service is not reachable.
    Unavailable,
    /// Service authentication failed.
    AuthError,
}

impl MemoryServiceStatus {
    /// Return the SQLite representation.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotConfigured => "not_configured",
            Self::Starting => "starting",
            Self::Ready => "ready",
            Self::Unavailable => "unavailable",
            Self::AuthError => "auth_error",
        }
    }

    /// Parse the SQLite representation.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when `value` is not a known memory service status.
    pub fn parse(value: &str) -> Result<Self, StoreError> {
        match value {
            "not_configured" => Ok(Self::NotConfigured),
            "starting" => Ok(Self::Starting),
            "ready" => Ok(Self::Ready),
            "unavailable" => Ok(Self::Unavailable),
            "auth_error" => Ok(Self::AuthError),
            _ => Err(StoreError::InvalidEnum {
                kind: "memory service status",
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
    /// Base URL for the Supermemory service.
    pub base_url: String,
    /// Managed service port, when configured.
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
    /// Base URL for the Supermemory service.
    pub base_url: String,
    /// Managed service port, when configured.
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

/// Persisted memory service readiness status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryServiceStatusRecord {
    /// Stable singleton status id.
    pub status_id: String,
    /// Current readiness status.
    pub status: MemoryServiceStatus,
    /// Last readiness check timestamp.
    pub checked_at: Option<String>,
    /// Sanitized last error code.
    pub last_error_code: Option<String>,
    /// Sanitized last error message.
    pub last_error_message: Option<String>,
}

/// Input for creating a memory ingest job.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewMemoryIngestJob {
    /// Durable ingest job id.
    pub job_id: String,
    /// Conversation submitted to Supermemory.
    pub conversation_id: String,
    /// Completed turn submitted to Supermemory.
    pub turn_id: String,
    /// Supermemory conversation/container id.
    pub supermemory_conversation_id: String,
}

/// Persisted memory ingest job.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryIngestJobRecord {
    /// Durable ingest job id.
    pub job_id: String,
    /// Conversation submitted to Supermemory.
    pub conversation_id: String,
    /// Completed turn submitted to Supermemory.
    pub turn_id: String,
    /// Current submission status.
    pub status: String,
    /// Supermemory conversation/container id.
    pub supermemory_conversation_id: String,
    /// Sanitized error code.
    pub error_code: Option<String>,
    /// Sanitized error message.
    pub error_message: Option<String>,
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

    /// Return the singleton memory service readiness status.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the embedded store read fails or the row is missing.
    pub async fn memory_service_status(&self) -> Result<MemoryServiceStatusRecord, StoreError> {
        self.with_connection(|conn| {
            conn.query_row(
                r#"
                SELECT status_id, status, checked_at, last_error_code, last_error_message
                FROM memory_service_status
                WHERE status_id = 'default'
                LIMIT 1
                "#,
                [],
                status_from_row,
            )
            .optional()?
            .ok_or_else(|| StoreError::InvariantViolation {
                message: "default memory service status row is missing".to_string(),
            })
        })
        .await
    }

    /// Create or update the singleton memory service readiness status.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the embedded store write/read fails.
    pub async fn save_memory_service_status(
        &self,
        status: MemoryServiceStatusRecord,
    ) -> Result<MemoryServiceStatusRecord, StoreError> {
        self.with_connection(|conn| {
            conn.execute(
                r#"
                INSERT INTO memory_service_status
                  (status_id, status, checked_at, last_error_code, last_error_message, updated_at)
                VALUES ('default', ?1, ?2, ?3, ?4, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
                ON CONFLICT(status_id) DO UPDATE SET
                  status = excluded.status,
                  checked_at = excluded.checked_at,
                  last_error_code = excluded.last_error_code,
                  last_error_message = excluded.last_error_message,
                  updated_at = excluded.updated_at
                "#,
                params![
                    status.status.as_str(),
                    status.checked_at,
                    status.last_error_code,
                    status.last_error_message,
                ],
            )?;
            Ok(())
        })
        .await?;
        self.memory_service_status().await
    }

    /// Insert a queued completed-turn ingest job.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the embedded store write/read fails.
    pub async fn insert_memory_ingest_job(
        &self,
        input: NewMemoryIngestJob,
    ) -> Result<MemoryIngestJobRecord, StoreError> {
        self.with_connection(|conn| {
            conn.execute(
                r#"
                INSERT INTO memory_ingest_jobs
                  (job_id, conversation_id, turn_id, status, supermemory_conversation_id, updated_at)
                VALUES (?1, ?2, ?3, 'queued', ?4, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
                "#,
                params![
                    input.job_id,
                    input.conversation_id,
                    input.turn_id,
                    input.supermemory_conversation_id,
                ],
            )?;
            Ok(())
        })
        .await?;
        self.memory_ingest_job(&input.job_id).await
    }

    /// Mark an ingest job as submitted to Supermemory.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the job is missing or the embedded store write/read fails.
    pub async fn mark_memory_ingest_job_submitted(
        &self,
        job_id: &str,
    ) -> Result<MemoryIngestJobRecord, StoreError> {
        self.with_connection(|conn| {
            conn.execute(
                r#"
                UPDATE memory_ingest_jobs
                SET status = 'submitted',
                    error_code = NULL,
                    error_message = NULL,
                    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                WHERE job_id = ?1
                "#,
                [job_id],
            )
            .map(|_| ())
            .map_err(StoreError::Sqlite)
        })
        .await?;
        self.memory_ingest_job(job_id).await
    }

    /// Mark an ingest job as failed with sanitized error details.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the job is missing or the embedded store write/read fails.
    pub async fn mark_memory_ingest_job_failed(
        &self,
        job_id: &str,
        code: &str,
        message: &str,
    ) -> Result<MemoryIngestJobRecord, StoreError> {
        self.with_connection(|conn| {
            conn.execute(
                r#"
                UPDATE memory_ingest_jobs
                SET status = 'failed',
                    error_code = ?2,
                    error_message = ?3,
                    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                WHERE job_id = ?1
                "#,
                params![job_id, code, message],
            )
            .map(|_| ())
            .map_err(StoreError::Sqlite)
        })
        .await?;
        self.memory_ingest_job(job_id).await
    }

    /// Return one memory ingest job by id.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the job is missing or the embedded store read fails.
    pub async fn memory_ingest_job(
        &self,
        job_id: &str,
    ) -> Result<MemoryIngestJobRecord, StoreError> {
        self.with_connection(|conn| {
            conn.query_row(
                r#"
                SELECT job_id, conversation_id, turn_id, status, supermemory_conversation_id,
                       error_code, error_message
                FROM memory_ingest_jobs
                WHERE job_id = ?1
                LIMIT 1
                "#,
                [job_id],
                ingest_job_from_row,
            )
            .optional()?
            .ok_or_else(|| StoreError::InvariantViolation {
                message: format!("memory ingest job not found: {job_id}"),
            })
        })
        .await
    }

    /// Return memory ingest jobs for one completed turn.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the embedded store read fails.
    pub async fn memory_ingest_jobs_for_turn(
        &self,
        turn_id: &str,
    ) -> Result<Vec<MemoryIngestJobRecord>, StoreError> {
        self.with_connection(|conn| {
            let mut statement = conn.prepare(
                r#"
                SELECT job_id, conversation_id, turn_id, status, supermemory_conversation_id,
                       error_code, error_message
                FROM memory_ingest_jobs
                WHERE turn_id = ?1
                ORDER BY job_id ASC
                "#,
            )?;
            let rows = statement.query_map([turn_id], ingest_job_from_row)?;
            Ok(rows.collect::<Result<Vec<_>, _>>()?)
        })
        .await
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

fn status_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<MemoryServiceStatusRecord> {
    let status: String = row.get(1)?;
    Ok(MemoryServiceStatusRecord {
        status_id: row.get(0)?,
        status: MemoryServiceStatus::parse(&status)
            .map_err(to_sqlite_from_sql_conversion_failure)?,
        checked_at: row.get(2)?,
        last_error_code: row.get(3)?,
        last_error_message: row.get(4)?,
    })
}

fn ingest_job_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<MemoryIngestJobRecord> {
    Ok(MemoryIngestJobRecord {
        job_id: row.get(0)?,
        conversation_id: row.get(1)?,
        turn_id: row.get(2)?,
        status: row.get(3)?,
        supermemory_conversation_id: row.get(4)?,
        error_code: row.get(5)?,
        error_message: row.get(6)?,
    })
}

fn to_sqlite_from_sql_conversion_failure(error: StoreError) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error))
}
