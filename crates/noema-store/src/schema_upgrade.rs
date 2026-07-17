//! Pre-stable compatibility repair for task runtime tables.

use std::collections::HashSet;

use rusqlite::{Connection, params};

use super::StoreError;

const CURRENT_RUN_COLUMNS: &[&str] = &[
    "resume_message",
    "max_provider_continuations",
    "max_tool_calls",
    "max_active_minutes",
    "progress_audit_interval",
    "provider_call_count",
    "tool_call_count",
    "cached_input_tokens",
    "active_milliseconds",
];

const CURRENT_ITEM_COLUMNS: &[&str] = &[
    "round_index",
    "status",
    "correlation_id",
    "parent_item_id",
    "updated_at",
];

const CURRENT_TASK_COLUMNS: &[(&str, &str)] =
    &[("blocked_question", "TEXT"), ("blocked_context", "TEXT")];

/// Repair the pre-v1 task runtime tables that shipped before their durable
/// supervision and transcript columns stabilized.
pub(super) fn upgrade_task_runtime_tables(conn: &mut Connection) -> Result<(), StoreError> {
    upgrade_tasks(conn)?;

    let run_columns = table_columns(conn, "agent_runs")?;
    let item_columns = table_columns(conn, "agent_run_items")?;
    let run_sql = table_sql(conn, "agent_runs")?;
    let item_sql = table_sql(conn, "agent_run_items")?;
    let rebuild_runs = CURRENT_RUN_COLUMNS
        .iter()
        .any(|column| !run_columns.contains(*column))
        || run_sql.contains("completion_delivery");
    let rebuild_items = CURRENT_ITEM_COLUMNS
        .iter()
        .any(|column| !item_columns.contains(*column))
        || !item_sql.contains("context_checkpoint");
    if !rebuild_runs && !rebuild_items {
        return Ok(());
    }

    let tx = conn.transaction()?;
    if rebuild_items {
        rebuild_agent_run_items(&tx, &item_columns)?;
    }
    if rebuild_runs {
        rebuild_agent_runs(&tx, &run_columns)?;
    }
    tx.commit()?;
    Ok(())
}

fn upgrade_tasks(conn: &mut Connection) -> Result<(), StoreError> {
    let columns = table_columns(conn, "tasks")?;
    let missing = CURRENT_TASK_COLUMNS
        .iter()
        .filter(|(name, _)| !columns.contains(*name))
        .collect::<Vec<_>>();
    if missing.is_empty() {
        return Ok(());
    }

    let tx = conn.transaction()?;
    for (name, definition) in missing {
        tx.execute(
            &format!("ALTER TABLE tasks ADD COLUMN {name} {definition}"),
            [],
        )?;
    }
    tx.commit()?;
    Ok(())
}

fn rebuild_agent_runs(conn: &Connection, columns: &HashSet<String>) -> Result<(), StoreError> {
    conn.execute_batch(
        r#"
        DROP TABLE IF EXISTS agent_runs_upgrade;
        CREATE TABLE agent_runs_upgrade (
          run_id TEXT PRIMARY KEY NOT NULL,
          task_id TEXT NOT NULL,
          run_kind TEXT NOT NULL CHECK (run_kind IN ('executor', 'reviewer')),
          agent_id TEXT NOT NULL,
          attempt_index INTEGER NOT NULL DEFAULT 0 CHECK (attempt_index >= 0),
          revision_index INTEGER NOT NULL DEFAULT 0 CHECK (revision_index >= 0),
          parent_run_id TEXT,
          triggering_submission_id TEXT,
          triggering_review_id TEXT,
          resume_message TEXT,
          provider_kind TEXT NOT NULL CHECK (provider_kind IN ('codex', 'openai', 'foundation_local', 'local_models')),
          provider_account_id TEXT NOT NULL,
          selection_mode TEXT NOT NULL CHECK (selection_mode IN ('explicit_profile', 'provider_default')),
          model_profile TEXT,
          reasoning_effort TEXT CHECK (reasoning_effort IS NULL OR reasoning_effort IN ('none', 'minimal', 'low', 'medium', 'high', 'xhigh')),
          selection_source TEXT,
          actual_provider_kind TEXT,
          actual_model_profile TEXT,
          max_provider_continuations INTEGER NOT NULL CHECK (max_provider_continuations BETWEEN 1 AND 1000),
          max_tool_calls INTEGER NOT NULL CHECK (max_tool_calls BETWEEN 1 AND 10000),
          max_active_minutes INTEGER NOT NULL CHECK (max_active_minutes BETWEEN 1 AND 10080),
          progress_audit_interval INTEGER NOT NULL CHECK (progress_audit_interval > 0 AND progress_audit_interval <= max_provider_continuations),
          status TEXT NOT NULL CHECK (status IN ('queued', 'leased', 'running', 'completed', 'waiting_for_approval', 'interrupted', 'failed', 'cancelled')),
          priority INTEGER NOT NULL DEFAULT 0,
          queued_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
          lease_owner TEXT,
          lease_token TEXT,
          lease_expires_at TEXT,
          heartbeat_at TEXT,
          started_at TEXT,
          ended_at TEXT,
          cancellation_requested INTEGER NOT NULL DEFAULT 0 CHECK (cancellation_requested IN (0, 1)),
          retry_count INTEGER NOT NULL DEFAULT 0 CHECK (retry_count >= 0),
          error_code TEXT,
          error_message TEXT,
          provider_call_count INTEGER NOT NULL DEFAULT 0 CHECK (provider_call_count >= 0),
          tool_call_count INTEGER NOT NULL DEFAULT 0 CHECK (tool_call_count >= 0),
          input_tokens INTEGER NOT NULL DEFAULT 0 CHECK (input_tokens >= 0),
          cached_input_tokens INTEGER NOT NULL DEFAULT 0 CHECK (cached_input_tokens >= 0),
          output_tokens INTEGER NOT NULL DEFAULT 0 CHECK (output_tokens >= 0),
          active_milliseconds INTEGER NOT NULL DEFAULT 0 CHECK (active_milliseconds >= 0),
          created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
          updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
        );
        "#,
    )?;
    let select = [
        column(columns, "run_id", "''"),
        column(columns, "task_id", "''"),
        column(columns, "run_kind", "'executor'"),
        column(columns, "agent_id", "''"),
        column(columns, "attempt_index", "0"),
        column(columns, "revision_index", "0"),
        column(columns, "parent_run_id", "NULL"),
        column(columns, "triggering_submission_id", "NULL"),
        column(columns, "triggering_review_id", "NULL"),
        column(columns, "resume_message", "NULL"),
        column(columns, "provider_kind", "'codex'"),
        column(columns, "provider_account_id", "''"),
        column(columns, "selection_mode", "'provider_default'"),
        column(columns, "model_profile", "NULL"),
        column(columns, "reasoning_effort", "NULL"),
        column(columns, "selection_source", "NULL"),
        column(columns, "actual_provider_kind", "NULL"),
        column(columns, "actual_model_profile", "NULL"),
        column(columns, "max_provider_continuations", "80"),
        column(columns, "max_tool_calls", "400"),
        column(columns, "max_active_minutes", "120"),
        column(columns, "progress_audit_interval", "20"),
        column(columns, "status", "'queued'"),
        column(columns, "priority", "0"),
        column(
            columns,
            "queued_at",
            "strftime('%Y-%m-%dT%H:%M:%fZ', 'now')",
        ),
        column(columns, "lease_owner", "NULL"),
        column(columns, "lease_token", "NULL"),
        column(columns, "lease_expires_at", "NULL"),
        column(columns, "heartbeat_at", "NULL"),
        column(columns, "started_at", "NULL"),
        column(columns, "ended_at", "NULL"),
        column(columns, "cancellation_requested", "0"),
        column(columns, "retry_count", "0"),
        column(columns, "error_code", "NULL"),
        column(columns, "error_message", "NULL"),
        column(columns, "provider_call_count", "0"),
        column(columns, "tool_call_count", "0"),
        coalesced_column(columns, "input_tokens", "0"),
        column(columns, "cached_input_tokens", "0"),
        coalesced_column(columns, "output_tokens", "0"),
        column(columns, "active_milliseconds", "0"),
        column(
            columns,
            "created_at",
            "strftime('%Y-%m-%dT%H:%M:%fZ', 'now')",
        ),
        column(
            columns,
            "updated_at",
            "strftime('%Y-%m-%dT%H:%M:%fZ', 'now')",
        ),
    ]
    .join(", ");
    conn.execute_batch(&format!(
        "INSERT INTO agent_runs_upgrade SELECT {select} FROM agent_runs WHERE run_kind IN ('executor', 'reviewer'); DROP TABLE agent_runs; ALTER TABLE agent_runs_upgrade RENAME TO agent_runs;"
    ))?;
    conn.execute(
        "UPDATE tasks SET latest_run_id = (SELECT run_id FROM agent_runs WHERE agent_runs.task_id = tasks.task_id ORDER BY created_at DESC, run_id DESC LIMIT 1) WHERE latest_run_id IS NOT NULL AND NOT EXISTS (SELECT 1 FROM agent_runs WHERE run_id = tasks.latest_run_id)",
        [],
    )?;
    Ok(())
}

fn rebuild_agent_run_items(conn: &Connection, columns: &HashSet<String>) -> Result<(), StoreError> {
    conn.execute_batch(
        r#"
        DROP TABLE IF EXISTS agent_run_items_upgrade;
        CREATE TABLE agent_run_items_upgrade (
          item_id TEXT PRIMARY KEY NOT NULL,
          run_id TEXT NOT NULL,
          sequence_index INTEGER NOT NULL CHECK (sequence_index >= 1),
          round_index INTEGER NOT NULL DEFAULT 0 CHECK (round_index >= 0),
          kind TEXT NOT NULL CHECK (kind IN ('model_input', 'assistant_output', 'tool_call', 'tool_result', 'progress_notice', 'context_checkpoint', 'task_submission', 'task_review', 'artifact_reference', 'failure', 'cancellation')),
          status TEXT NOT NULL DEFAULT 'completed' CHECK (status IN ('pending', 'running', 'completed', 'failed', 'cancelled', 'skipped')),
          correlation_id TEXT,
          parent_item_id TEXT,
          content_text TEXT,
          payload_json TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(payload_json)),
          created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
          updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
          UNIQUE(run_id, sequence_index)
        );
        "#,
    )?;
    let select = [
        column(columns, "item_id", "''"),
        column(columns, "run_id", "''"),
        column(columns, "sequence_index", "1"),
        column(columns, "round_index", "0"),
        column(columns, "kind", "'assistant_output'"),
        column(columns, "status", "'completed'"),
        column(columns, "correlation_id", "NULL"),
        column(columns, "parent_item_id", "NULL"),
        column(columns, "content_text", "NULL"),
        column(columns, "payload_json", "'{}'"),
        column(
            columns,
            "created_at",
            "strftime('%Y-%m-%dT%H:%M:%fZ', 'now')",
        ),
        column(columns, "updated_at", "created_at"),
    ]
    .join(", ");
    conn.execute_batch(&format!(
        "INSERT INTO agent_run_items_upgrade SELECT {select} FROM agent_run_items; DROP TABLE agent_run_items; ALTER TABLE agent_run_items_upgrade RENAME TO agent_run_items;"
    ))?;
    Ok(())
}

fn table_columns(conn: &Connection, table: &str) -> Result<HashSet<String>, StoreError> {
    let mut statement = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let rows = statement.query_map([], |row| row.get::<_, String>(1))?;
    rows.collect::<Result<HashSet<_>, _>>()
        .map_err(StoreError::Sqlite)
}

fn table_sql(conn: &Connection, table: &str) -> Result<String, StoreError> {
    conn.query_row(
        "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = ?1",
        params![table],
        |row| row.get(0),
    )
    .map_err(StoreError::Sqlite)
}

fn column(columns: &HashSet<String>, name: &str, fallback: &str) -> String {
    if columns.contains(name) {
        name.to_string()
    } else {
        fallback.to_string()
    }
}

fn coalesced_column(columns: &HashSet<String>, name: &str, fallback: &str) -> String {
    if columns.contains(name) {
        format!("COALESCE({name}, {fallback})")
    } else {
        fallback.to_string()
    }
}
