//! Durable, privacy-bounded runtime profiling records.
#![allow(
    missing_docs,
    reason = "runtime debug types are an internal cross-crate store projection"
)]

use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};

use crate::{
    NoemaStore, StoreError,
    ids::allocate_id,
    sqlite::{deserialize_json, serialize_json},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RuntimeDebugScope {
    ConversationTurn(String),
    AgentRun(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuntimeDebugSpanCategory {
    Provider,
    Tool,
    Runtime,
    Persistence,
}

impl RuntimeDebugSpanCategory {
    fn as_str(self) -> &'static str {
        match self {
            Self::Provider => "provider",
            Self::Tool => "tool",
            Self::Runtime => "runtime",
            Self::Persistence => "persistence",
        }
    }

    fn parse(value: &str) -> Result<Self, StoreError> {
        match value {
            "provider" => Ok(Self::Provider),
            "tool" => Ok(Self::Tool),
            "runtime" => Ok(Self::Runtime),
            "persistence" => Ok(Self::Persistence),
            _ => Err(invariant(format!(
                "unknown runtime debug category: {value}"
            ))),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuntimeDebugSpanStatus {
    Running,
    Completed,
    Failed,
    Cancelled,
    Interrupted,
}

impl RuntimeDebugSpanStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::Interrupted => "interrupted",
        }
    }

    fn parse(value: &str) -> Result<Self, StoreError> {
        match value {
            "running" => Ok(Self::Running),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            "cancelled" => Ok(Self::Cancelled),
            "interrupted" => Ok(Self::Interrupted),
            _ => Err(invariant(format!("unknown runtime debug status: {value}"))),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RuntimeDebugMetadata {
    pub provider: Option<String>,
    pub model: Option<String>,
    pub phase: Option<String>,
    pub error: Option<String>,
    pub response_index: Option<u64>,
    pub round_index: Option<u64>,
    pub tool_name: Option<String>,
    pub correlation_id: Option<String>,
    pub input_tokens: Option<u64>,
    pub cached_input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub total_tokens: Option<u64>,
    pub transport: Option<String>,
    pub input_mode: Option<String>,
    pub fallback_reason: Option<String>,
    pub used_response_id: Option<bool>,
}

#[derive(Clone, Debug)]
pub struct NewRuntimeDebugSpan {
    pub scope: RuntimeDebugScope,
    pub category: RuntimeDebugSpanCategory,
    pub name: String,
    pub metadata: RuntimeDebugMetadata,
}

#[derive(Clone, Debug)]
pub struct RuntimeDebugChildSpan {
    pub name: String,
    pub start_offset_milliseconds: u64,
    pub duration_milliseconds: u64,
    pub metadata: RuntimeDebugMetadata,
}

#[derive(Clone, Debug)]
pub struct RuntimeDebugSpanRecord {
    pub span_id: String,
    pub category: RuntimeDebugSpanCategory,
    pub name: String,
    pub status: RuntimeDebugSpanStatus,
    pub duration_milliseconds: Option<u64>,
    pub metadata: RuntimeDebugMetadata,
    pub started_at: String,
    pub ended_at: Option<String>,
}

#[derive(Clone, Debug)]
pub struct RuntimeDebugProfileRecord {
    pub scope: RuntimeDebugScope,
    pub container_id: String,
    pub status: String,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub spans: Vec<RuntimeDebugSpanRecord>,
}

impl NoemaStore {
    /// Start one durable runtime profiling interval.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the owner is missing, the span is invalid,
    /// or SQLite cannot persist it.
    pub async fn begin_runtime_debug_span(
        &self,
        span: NewRuntimeDebugSpan,
    ) -> Result<String, StoreError> {
        if span.name.trim().is_empty() {
            return Err(invariant("runtime debug span name cannot be empty"));
        }
        let span_id = allocate_id("debug_span");
        let metadata = serialize_json(&span.metadata)?;
        let (turn_id, run_id) = match span.scope {
            RuntimeDebugScope::ConversationTurn(id) => (Some(id), None),
            RuntimeDebugScope::AgentRun(id) => (None, Some(id)),
        };
        self.with_connection(|conn| {
            conn.execute(
                "INSERT INTO runtime_debug_spans (span_id, conversation_turn_id, agent_run_id, category, name, metadata_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![span_id, turn_id, run_id, span.category.as_str(), span.name, metadata],
            )?;
            Ok(())
        })
        .await?;
        Ok(span_id)
    }

    /// Close one running runtime profiling interval.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the span is missing, already closed, or
    /// SQLite cannot update it.
    pub async fn finish_runtime_debug_span(
        &self,
        span_id: &str,
        status: RuntimeDebugSpanStatus,
        duration_milliseconds: u64,
        metadata: RuntimeDebugMetadata,
    ) -> Result<(), StoreError> {
        self.finish_runtime_debug_span_with_children(
            span_id,
            status,
            duration_milliseconds,
            metadata,
            &[],
        )
        .await
    }

    /// Close one interval and add its completed child intervals atomically.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when an interval is invalid or SQLite cannot
    /// update the stored profile.
    pub async fn finish_runtime_debug_span_with_children(
        &self,
        span_id: &str,
        status: RuntimeDebugSpanStatus,
        duration_milliseconds: u64,
        metadata: RuntimeDebugMetadata,
        children: &[RuntimeDebugChildSpan],
    ) -> Result<(), StoreError> {
        let duration = i64::try_from(duration_milliseconds)
            .map_err(|_| invariant("runtime debug span duration exceeds SQLite range"))?;
        let metadata = serialize_json(&metadata)?;
        let mut prepared_children = Vec::with_capacity(children.len());
        for child in children {
            if child.name.trim().is_empty() {
                return Err(invariant("runtime debug child span name cannot be empty"));
            }
            let start_offset = i64::try_from(child.start_offset_milliseconds)
                .map_err(|_| invariant("runtime debug child offset exceeds SQLite range"))?;
            let child_duration = i64::try_from(child.duration_milliseconds)
                .map_err(|_| invariant("runtime debug child duration exceeds SQLite range"))?;
            if start_offset.saturating_add(child_duration) > duration {
                return Err(invariant("runtime debug child span exceeds its parent"));
            }
            prepared_children.push((
                allocate_id("debug_span"),
                child.name.clone(),
                start_offset,
                child_duration,
                serialize_json(&child.metadata)?,
            ));
        }
        self.with_immediate_transaction_retry(|transaction| {
            let changed = transaction.execute(
                "UPDATE runtime_debug_spans SET status = ?2, duration_milliseconds = ?3, metadata_json = ?4, ended_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE span_id = ?1 AND status = 'running'",
                params![span_id, status.as_str(), duration, metadata],
            )?;
            if changed != 1 {
                return Err(invariant(format!(
                    "runtime debug span is unavailable or already closed: {span_id}"
                )));
            }
            for (child_id, name, start_offset, child_duration, child_metadata) in
                &prepared_children
            {
                let inserted = transaction.execute(
                    r#"
                    INSERT INTO runtime_debug_spans (
                      span_id, conversation_turn_id, agent_run_id, category, name,
                      status, duration_milliseconds, metadata_json, started_at, ended_at
                    )
                    SELECT ?2, conversation_turn_id, agent_run_id, category, ?3,
                      'completed', ?5, ?6,
                      strftime('%Y-%m-%dT%H:%M:%fZ', julianday(started_at) + (?4 / 86400000.0)),
                      strftime('%Y-%m-%dT%H:%M:%fZ', julianday(started_at) + ((?4 + ?5) / 86400000.0))
                    FROM runtime_debug_spans WHERE span_id = ?1
                    "#,
                    params![
                        span_id,
                        child_id,
                        name,
                        start_offset,
                        child_duration,
                        child_metadata
                    ],
                )?;
                if inserted != 1 {
                    return Err(invariant(format!(
                        "runtime debug parent span is unavailable: {span_id}"
                    )));
                }
            }
            Ok(())
        })
        .await
    }

    /// Read the owner and ordered spans for one turn or agent run.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when stored data is invalid or SQLite cannot
    /// read the requested scope.
    pub async fn runtime_debug_profile(
        &self,
        scope: RuntimeDebugScope,
    ) -> Result<Option<RuntimeDebugProfileRecord>, StoreError> {
        self.with_connection(|conn| {
            let owner = match &scope {
                RuntimeDebugScope::ConversationTurn(turn_id) => conn
                    .query_row(
                        "SELECT conversation_id, status, started_at, completed_at FROM conversation_turns WHERE turn_id = ?1",
                        [turn_id],
                        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                    )
                    .optional()?,
                RuntimeDebugScope::AgentRun(run_id) => conn
                    .query_row(
                        "SELECT task_id, status, COALESCE(started_at, queued_at), ended_at FROM agent_runs WHERE run_id = ?1",
                        [run_id],
                        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                    )
                    .optional()?,
            };
            let Some((container_id, status, started_at, ended_at)) = owner else {
                return Ok(None);
            };
            let (column, owner_id) = match &scope {
                RuntimeDebugScope::ConversationTurn(id) => ("conversation_turn_id", id),
                RuntimeDebugScope::AgentRun(id) => ("agent_run_id", id),
            };
            let sql = format!(
                "SELECT span_id, category, name, status, duration_milliseconds, metadata_json, started_at, ended_at FROM runtime_debug_spans WHERE {column} = ?1 ORDER BY started_at, span_id"
            );
            let mut statement = conn.prepare(&sql)?;
            let rows = statement.query_map([owner_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, Option<i64>>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, Option<String>>(7)?,
                ))
            })?;
            let mut spans = Vec::new();
            for row in rows {
                let (span_id, category, name, span_status, duration, metadata, span_started, span_ended) = row?;
                spans.push(RuntimeDebugSpanRecord {
                    span_id,
                    category: RuntimeDebugSpanCategory::parse(&category)?,
                    name,
                    status: RuntimeDebugSpanStatus::parse(&span_status)?,
                    duration_milliseconds: duration
                        .map(|value| u64::try_from(value).map_err(|_| invariant("negative runtime debug duration")))
                        .transpose()?,
                    metadata: deserialize_json(metadata)?,
                    started_at: span_started,
                    ended_at: span_ended,
                });
            }
            Ok(Some(RuntimeDebugProfileRecord {
                scope,
                container_id,
                status,
                started_at,
                ended_at,
                spans,
            }))
        })
        .await
    }
}

fn invariant(message: impl Into<String>) -> StoreError {
    StoreError::InvariantViolation {
        message: message.into(),
    }
}
