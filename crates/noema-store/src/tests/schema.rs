use tempfile::TempDir;

use super::{support::store_config, test_store};
use crate::{NoemaStore, StoreConfig};

#[tokio::test]
async fn opens_sqlite_store_under_noema_db_dir() {
    let home = TempDir::new().expect("temp store root");
    let config = store_config(home.path());

    let store = NoemaStore::open(&config).await.expect("open store");

    assert!(home.path().join("db").exists());
    assert!(config.path.exists());
    assert_eq!(store.schema_version().await.expect("schema version"), 1);
}

#[tokio::test]
async fn opening_pre_v1_task_runtime_tables_rebuilds_and_preserves_history() {
    let home = TempDir::new().expect("temp store root");
    let config = store_config(home.path());
    let store = NoemaStore::open(&config).await.expect("open store");
    store
        .with_connection(|conn| {
            conn.execute_batch(
                r#"
                DROP TABLE agent_run_items;
                DROP TABLE agent_runs;
                CREATE TABLE agent_runs (
                  run_id TEXT PRIMARY KEY NOT NULL,
                  task_id TEXT NOT NULL,
                  run_kind TEXT NOT NULL CHECK (run_kind IN ('executor', 'reviewer', 'completion_delivery')),
                  agent_id TEXT NOT NULL,
                  attempt_index INTEGER NOT NULL DEFAULT 0,
                  revision_index INTEGER NOT NULL DEFAULT 0,
                  parent_run_id TEXT,
                  triggering_submission_id TEXT,
                  triggering_review_id TEXT,
                  provider_kind TEXT NOT NULL,
                  provider_account_id TEXT NOT NULL,
                  selection_mode TEXT NOT NULL,
                  model_profile TEXT,
                  reasoning_effort TEXT,
                  selection_source TEXT,
                  actual_provider_kind TEXT,
                  actual_model_profile TEXT,
                  status TEXT NOT NULL,
                  priority INTEGER NOT NULL DEFAULT 0,
                  queued_at TEXT NOT NULL DEFAULT 'old',
                  lease_owner TEXT,
                  lease_token TEXT,
                  lease_expires_at TEXT,
                  heartbeat_at TEXT,
                  started_at TEXT,
                  ended_at TEXT,
                  cancellation_requested INTEGER NOT NULL DEFAULT 0,
                  retry_count INTEGER NOT NULL DEFAULT 0,
                  error_code TEXT,
                  error_message TEXT,
                  input_tokens INTEGER,
                  output_tokens INTEGER,
                  created_at TEXT NOT NULL DEFAULT 'old',
                  updated_at TEXT NOT NULL DEFAULT 'old'
                );
                CREATE TABLE agent_run_items (
                  item_id TEXT PRIMARY KEY NOT NULL,
                  run_id TEXT NOT NULL,
                  sequence_index INTEGER NOT NULL,
                  kind TEXT NOT NULL CHECK (kind IN ('model_input', 'assistant_output', 'tool_call', 'tool_result', 'progress_notice', 'task_submission', 'task_review', 'artifact_reference', 'failure', 'cancellation')),
                  content_text TEXT,
                  payload_json TEXT NOT NULL DEFAULT '{}',
                  created_at TEXT NOT NULL DEFAULT 'old',
                  UNIQUE(run_id, sequence_index)
                );
                INSERT INTO agent_runs (run_id, task_id, run_kind, agent_id, provider_kind, provider_account_id, selection_mode, status, input_tokens, output_tokens)
                VALUES ('run:legacy', 'task:legacy', 'executor', 'agent:task-executor', 'codex', 'provider_account:codex:default', 'explicit_profile', 'failed', NULL, NULL);
                INSERT INTO agent_run_items (item_id, run_id, sequence_index, kind, content_text)
                VALUES ('item:legacy', 'run:legacy', 1, 'assistant_output', 'preserved');
                "#,
            )?;
            Ok(())
        })
        .await
        .expect("install legacy schema");
    drop(store);

    let reopened = NoemaStore::open(&config)
        .await
        .expect("upgrade legacy store");
    let run = reopened
        .get_agent_run("run:legacy")
        .await
        .expect("read run")
        .expect("legacy run");
    assert_eq!(
        run.execution_policy,
        noema_tasks::TaskExecutionPolicy::default()
    );
    assert_eq!(run.input_tokens, 0);
    assert_eq!(run.output_tokens, 0);
    let items = reopened
        .list_agent_run_items("run:legacy")
        .await
        .expect("legacy items");
    assert_eq!(items[0].content_text.as_deref(), Some("preserved"));
    assert_eq!(items[0].status, noema_tasks::AgentRunItemStatus::Completed);
}

#[tokio::test]
async fn opening_legacy_tasks_adds_blocking_columns() {
    let home = TempDir::new().expect("temp store root");
    let config = store_config(home.path());
    let store = NoemaStore::open(&config).await.expect("open store");
    store
        .with_connection(|conn| {
            conn.execute_batch(
                "ALTER TABLE tasks DROP COLUMN blocked_question; ALTER TABLE tasks DROP COLUMN blocked_context;",
            )?;
            Ok(())
        })
        .await
        .expect("install legacy task schema");
    drop(store);

    let reopened = NoemaStore::open(&config)
        .await
        .expect("upgrade legacy task schema");
    let columns = reopened
        .with_connection(|conn| {
            let mut statement = conn.prepare("PRAGMA table_info(tasks)")?;
            let rows = statement.query_map([], |row| row.get::<_, String>(1))?;
            rows.collect::<Result<Vec<_>, _>>()
                .map_err(crate::StoreError::Sqlite)
        })
        .await
        .expect("read task columns");
    assert!(columns.iter().any(|column| column == "blocked_question"));
    assert!(columns.iter().any(|column| column == "blocked_context"));
}

#[tokio::test]
async fn sqlite_schema_does_not_create_memory_ingest_jobs() {
    let store = test_store().await;

    let table_count = store
        .with_connection(|conn| {
            conn.query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'memory_ingest_jobs'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map_err(crate::StoreError::Sqlite)
        })
        .await
        .expect("table lookup");

    assert_eq!(table_count, 0);
}

#[tokio::test]
async fn sqlite_schema_creates_artifact_tables() {
    let store = test_store().await;

    let tables = store
        .with_connection(|conn| {
            let mut statement = conn.prepare(
                "SELECT name FROM sqlite_master WHERE type = 'table' AND name IN ('artifacts', 'artifact_versions') ORDER BY name",
            )?;
            let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
            rows.collect::<Result<Vec<_>, _>>()
                .map_err(crate::StoreError::Sqlite)
        })
        .await
        .expect("table lookup");

    assert_eq!(tables, vec!["artifact_versions", "artifacts"]);
}

#[tokio::test]
async fn sqlite_store_config_is_stable_for_reopen() {
    let home = TempDir::new().expect("temp store root");
    let database_path = home.path().join("db/noema.sqlite3");
    let config = StoreConfig::new(database_path.clone());

    assert_eq!(config.path, database_path);
    assert_eq!(StoreConfig::new(config.path.clone()), config);
}
