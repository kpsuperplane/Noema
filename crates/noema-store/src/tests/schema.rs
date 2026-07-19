use std::fs;

use rusqlite::{Connection, params};
use tempfile::TempDir;

use super::{schema_support::*, support::store_config};
use crate::{
    NoemaStore, SchemaIncompatibility, StoreConfig, StoreError,
    runtime::{bootstrap_schema_for_test, inspect_empty_schema_for_test},
    schema::{STORE_SCHEMA_MARKER, STORE_SCHEMA_SQL, STORE_SCHEMA_VERSION},
};

#[tokio::test]
async fn work_v3_bootstrap_is_exact_idempotent_and_enforces_foreign_keys() {
    let home = TempDir::new().expect("temp store root");
    let config = store_config(home.path());
    let store = NoemaStore::open(&config).await.expect("bootstrap V3 store");

    store
        .with_connection(|conn| {
            assert_eq!(conn.query_row("PRAGMA foreign_keys", [], |row| row.get::<_, i64>(0))?, 1);
            assert_eq!(count_where(conn, "humans", "human_id = 'human:local'")?, 1);
            assert_eq!(count_where(conn, "workspaces", "workspace_id = 'workspace:personal' AND name = 'Personal' AND description = '' AND is_personal = 1 AND archived_at IS NULL AND revision = 1")?, 1);
            assert_eq!(count_where(conn, "workspace_memberships", "workspace_id = 'workspace:personal' AND human_id = 'human:local' AND role = 'owner'")?, 1);
            assert_eq!(count_where(conn, "workflow_definitions", "workflow_id = 'workflow:personal:default' AND workspace_id = 'workspace:personal' AND name = 'Personal workflow' AND is_default = 1 AND revision = 1")?, 1);
            assert_eq!(count_where(conn, "projects", "1 = 1")?, 0);

            let mut statement = conn.prepare(
                "SELECT stage_id, stable_key, display_name, ordinal, system_behavior, board_visible FROM workflow_stages WHERE workflow_id = 'workflow:personal:default' ORDER BY ordinal",
            )?;
            let stages = statement
                .query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, i64>(5)?,
                    ))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            assert_eq!(
                stages,
                vec![
                    stage("inbox", "Inbox", 10, "intake", 1),
                    stage("queue", "Queue", 20, "dispatch", 1),
                    stage("doing", "Doing", 30, "active", 1),
                    stage("waiting", "Waiting", 40, "human_gate", 1),
                    stage("review", "Review", 50, "acceptance", 1),
                    stage("completed", "Completed", 60, "terminal_success", 0),
                    stage("cancelled", "Cancelled", 70, "terminal_cancelled", 0),
                ]
            );

            let policy = conn.query_row(
                "SELECT max_provider_continuations, max_tool_calls, max_active_minutes, progress_audit_interval, max_automatic_retries, max_review_rounds FROM task_execution_policy WHERE policy_id = 'default'",
                [],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?, row.get::<_, i64>(2)?, row.get::<_, i64>(3)?, row.get::<_, i64>(4)?, row.get::<_, i64>(5)?)),
            )?;
            assert_eq!(policy, (80, 400, 120, 20, 3, 3));
            assert_eq!(count_where(conn, "schema_state", "name = 'sqlite_store_v3' AND version = 3")?, 1);
            Ok(())
        })
        .await
        .expect("inspect V3 bootstrap");

    drop(store);
    let reopened = NoemaStore::open(&config)
        .await
        .expect("second bootstrap/open is idempotent");
    reopened
        .with_connection(|conn| {
            for (table, predicate, expected) in [
                ("workspaces", "is_personal = 1", 1),
                (
                    "workspace_memberships",
                    "workspace_id = 'workspace:personal'",
                    1,
                ),
                (
                    "workflow_stages",
                    "workflow_id = 'workflow:personal:default'",
                    7,
                ),
                ("task_execution_policy", "policy_id = 'default'", 1),
            ] {
                assert_eq!(count_where(conn, table, predicate)?, expected);
            }
            Ok(())
        })
        .await
        .expect("inspect idempotent reopen");
}

#[tokio::test]
async fn work_v3_schema_enforces_projection_history_and_ledger_invariants() {
    let home = TempDir::new().expect("temp store root");
    let store = NoemaStore::open(&store_config(home.path()))
        .await
        .expect("bootstrap V3 store");

    store
        .with_connection(|conn| {
            for table in [
                "workspaces", "workspace_memberships", "projects", "workflow_definitions",
                "workflow_stages", "tasks", "task_execution_contracts",
                "task_contract_criteria", "task_gates", "task_messages", "agent_runs",
                "task_submissions", "task_reviews", "work_events",
                "work_notification_outbox", "work_command_receipts",
            ] {
                assert!(schema_object_exists(conn, "table", table)?, "missing table {table}");
            }
            for index in [
                "task_gates_one_open_per_task", "agent_runs_one_runnable_per_task",
                "agent_runs_fifo_claim", "work_events_workspace_cursor",
                "work_notification_outbox_claim",
            ] {
                assert!(schema_object_exists(conn, "index", index)?, "missing index {index}");
            }
            assert!(!schema_object_exists(conn, "table", "task_events")?);
            assert!(!schema_object_exists(conn, "table", "run_events")?);
            assert!(!table_columns(conn, "tasks")?.iter().any(|column| column == "status"));
            let run_columns = table_columns(conn, "agent_runs")?;
            assert!(!run_columns.iter().any(|column| column == "priority"));
            assert!(!run_columns.iter().any(|column| column == "resume_message"));

            conn.execute_batch(
                r#"
                INSERT INTO workspaces (workspace_id, name) VALUES ('workspace:other', 'Other');
                INSERT INTO projects (project_id, workspace_id, name)
                VALUES ('project:other', 'workspace:other', 'Other project');
                INSERT INTO workflow_definitions (workflow_id, workspace_id, name)
                VALUES ('workflow:other', 'workspace:personal', 'Other workflow');
                "#,
            )?;
            assert!(conn.execute(
                "INSERT INTO tasks (task_id, workspace_id, project_id, workflow_id, stage_id, title, source_kind, created_by_actor_id) VALUES ('task:cross-project', 'workspace:personal', 'project:other', 'workflow:personal:default', 'stage:personal:inbox', 'Bad project', 'system', 'actor:system')",
                [],
            ).is_err());
            assert!(conn.execute(
                "INSERT INTO tasks (task_id, workspace_id, workflow_id, stage_id, title, source_kind, created_by_actor_id) VALUES ('task:cross-stage', 'workspace:personal', 'workflow:other', 'stage:personal:inbox', 'Bad stage', 'system', 'actor:system')",
                [],
            ).is_err());

            conn.execute(
                "INSERT INTO tasks (task_id, workspace_id, workflow_id, stage_id, title, source_kind, created_by_actor_id) VALUES ('task:valid', 'workspace:personal', 'workflow:personal:default', 'stage:personal:queue', 'Valid task', 'system', 'actor:system')",
                [],
            )?;
            conn.execute(
                "INSERT INTO task_gates (gate_id, task_id, task_generation, gate_kind, gate_state, prompt_markdown, opened_by_actor_id) VALUES ('gate:one', 'task:valid', 1, 'clarification', 'open', 'Question?', 'actor:system')",
                [],
            )?;
            assert!(conn.execute(
                "INSERT INTO task_gates (gate_id, task_id, task_generation, gate_kind, gate_state, prompt_markdown, opened_by_actor_id) VALUES ('gate:two', 'task:valid', 1, 'approval', 'open', 'Approve?', 'actor:system')",
                [],
            ).is_err());

            insert_planner_run(conn, "run:one")?;
            assert!(insert_planner_run(conn, "run:two").is_err());
            conn.execute("UPDATE agent_runs SET status = 'completed' WHERE run_id = 'run:one'", [])?;
            insert_planner_run(conn, "run:two")?;

            insert_delegated_contract(conn)?;
            conn.execute(
                "INSERT INTO task_contract_criteria (criterion_id, contract_id, ordinal, description) VALUES ('criterion:one', 'contract:one', 1, 'First')",
                [],
            )?;
            assert!(conn.execute(
                "INSERT INTO task_contract_criteria (criterion_id, contract_id, ordinal, description) VALUES ('criterion:two', 'contract:one', 1, 'Second')",
                [],
            ).is_err());

            for event_id in ["event:one", "event:two"] {
                conn.execute(
                    "INSERT INTO work_events (event_id, event_kind, workspace_id, actor_id, correlation_id) VALUES (?1, 'project.created', 'workspace:personal', 'actor:system', 'correlation:test')",
                    [event_id],
                )?;
            }
            let removed_sequence = conn.query_row(
                "SELECT event_sequence FROM work_events WHERE event_id = 'event:two'",
                [],
                |row| row.get::<_, i64>(0),
            )?;
            conn.execute("DELETE FROM work_events WHERE event_id = 'event:two'", [])?;
            conn.execute(
                "INSERT INTO work_events (event_id, event_kind, workspace_id, actor_id, correlation_id) VALUES ('event:three', 'project.created', 'workspace:personal', 'actor:system', 'correlation:test')",
                [],
            )?;
            let next_sequence = conn.query_row(
                "SELECT event_sequence FROM work_events WHERE event_id = 'event:three'",
                [],
                |row| row.get::<_, i64>(0),
            )?;
            assert!(next_sequence > removed_sequence, "AUTOINCREMENT must not recycle cursors");
            Ok(())
        })
        .await
        .expect("validate V3 integrity");
}

#[tokio::test]
async fn bootstrap_rejection_and_explicit_recovery_schema_contracts() {
    let home = TempDir::new().expect("temp store root");
    let config = store_config(home.path());

    let store = NoemaStore::open(&config).await.expect("bootstrap store");
    store
        .with_connection(|conn| {
            for (table, column) in [
                ("agent_runtime_preferences", "provider_instance_key"),
                ("auxiliary_model_preferences", "provider_instance_key"),
                ("local_model_installations", "provider_instance_key"),
                ("local_model_installations", "retirement_claimed_at"),
                ("local_model_installations", "runtime_retired_at"),
                ("default_model_preference", "provider_instance_key"),
                ("memory_service_settings", "provider_instance_key"),
                ("task_model_pool_entries", "provider_instance_key"),
                ("task_execution_contracts", "executor_provider_instance_key"),
                ("task_execution_contracts", "reviewer_provider_instance_key"),
                ("agent_runs", "provider_instance_key"),
            ] {
                let mut statement = conn.prepare(&format!("PRAGMA table_info({table})"))?;
                let columns = statement
                    .query_map([], |row| row.get::<_, String>(1))?
                    .collect::<Result<Vec<_>, _>>()?;
                assert!(columns.iter().any(|actual| actual == column), "{table}.{column}");
            }
            conn.execute(
                "UPDATE schema_state SET applied_at = 'sentinel' WHERE name = ?1",
                [STORE_SCHEMA_MARKER],
            )?;
            conn.execute(
                "INSERT INTO humans (human_id, display_name) VALUES ('human:preserved', 'Preserved')",
                [],
            )?;
            Ok(())
        })
        .await
        .expect("exact identity schema and sentinel rows");
    drop(store);

    let actual = database_snapshot(&config.path);
    let expected = canonical_schema_objects();
    assert_eq!(actual.schema_objects, expected);
    assert!(matches!(
        actual.schema_markers.as_deref(),
        Some([(name, STORE_SCHEMA_VERSION, _)]) if name == STORE_SCHEMA_MARKER
    ));

    let reopened = NoemaStore::open(&config)
        .await
        .expect("reopen exact current store");
    reopened
        .with_connection(|conn| {
            let marker = conn.query_row(
                "SELECT applied_at FROM schema_state WHERE name = ?1",
                [STORE_SCHEMA_MARKER],
                |row| row.get::<_, String>(0),
            )?;
            let human = conn.query_row(
                "SELECT display_name FROM humans WHERE human_id = 'human:preserved'",
                [],
                |row| row.get::<_, String>(0),
            )?;
            assert_eq!((marker.as_str(), human.as_str()), ("sentinel", "Preserved"));
            Ok(())
        })
        .await
        .expect("marker and rows preserved");

    let zero = TempDir::new().expect("zero-byte root");
    let zero_config = store_config(zero.path());
    fs::create_dir_all(zero_config.path.parent().expect("database parent"))
        .expect("database parent");
    fs::File::create(&zero_config.path).expect("zero-byte database");
    drop(
        NoemaStore::open(&zero_config)
            .await
            .expect("zero-byte bootstrap"),
    );
    assert!(matches!(
        database_snapshot(&zero_config.path).schema_markers.as_deref(),
        Some([(name, STORE_SCHEMA_VERSION, _)]) if name == STORE_SCHEMA_MARKER
    ));

    #[derive(Clone, Copy, Debug)]
    enum Fixture {
        PreV1Runtime,
        LegacyWork,
        UnknownSchema,
        UnknownMarker,
    }

    for fixture in [
        Fixture::PreV1Runtime,
        Fixture::LegacyWork,
        Fixture::UnknownSchema,
        Fixture::UnknownMarker,
    ] {
        let home = TempDir::new().expect("fixture root");
        let config = store_config(home.path());
        create_current_database(&config.path);
        let conn = Connection::open(&config.path).expect("fixture database");
        match fixture {
            Fixture::PreV1Runtime => conn
                .execute_batch(
                    "DROP TABLE agent_run_items; CREATE TABLE agent_run_items (item_id TEXT PRIMARY KEY, content_text TEXT); INSERT INTO agent_run_items VALUES ('item:legacy', 'preserved');",
                )
                .expect("pre-v1 runtime fixture"),
            Fixture::LegacyWork => conn
                .execute_batch(
                    "ALTER TABLE tasks DROP COLUMN source_item_id;",
                )
                .expect("legacy work fixture"),
            Fixture::UnknownSchema => conn
                .execute_batch("CREATE TABLE unknown_owner_data (value TEXT NOT NULL);")
                .expect("unknown schema fixture"),
            Fixture::UnknownMarker => {
                conn.execute(
                    "UPDATE schema_state SET name = 'unknown_schema' WHERE name = ?1",
                    [STORE_SCHEMA_MARKER],
                )
                .expect("unknown marker fixture");
            }
        }
        drop(conn);
        let error = assert_rejected_without_mutation(&config).await;
        assert!(
            matches!(
                (fixture, error),
                (
                    Fixture::PreV1Runtime | Fixture::LegacyWork | Fixture::UnknownSchema,
                    StoreError::IncompatibleSchema {
                        kind: SchemaIncompatibility::Shape { .. },
                        ..
                    },
                ) | (
                    Fixture::UnknownMarker,
                    StoreError::IncompatibleSchema {
                        kind: SchemaIncompatibility::Marker { .. },
                        ..
                    },
                )
            ),
            "{fixture:?} classification"
        );
    }

    let home = TempDir::new().expect("fresh recovery root");
    let config = store_config(home.path());
    create_current_database(&config.path);
    Connection::open(&config.path)
        .expect("rejected fixture")
        .execute_batch("CREATE TABLE obsolete_pre_v1_data (value TEXT NOT NULL);")
        .expect("obsolete table");
    assert_rejected_without_mutation(&config).await;
    for path in [
        config.path.clone(),
        sidecar_path(&config.path, "-wal"),
        sidecar_path(&config.path, "-shm"),
    ] {
        if path.exists() {
            fs::remove_file(path).expect("explicitly remove rejected family");
        }
    }
    drop(NoemaStore::open(&config).await.expect("fresh recovery"));
    assert!(matches!(
        database_snapshot(&config.path).schema_markers.as_deref(),
        Some([(name, STORE_SCHEMA_VERSION, _)]) if name == STORE_SCHEMA_MARKER
    ));
}

#[tokio::test]
async fn opening_partial_schema_is_rejected_without_mutation() {
    let home = TempDir::new().expect("temp store root");
    let config = store_config(home.path());
    create_current_database(&config.path);
    let conn = Connection::open(&config.path).expect("open partial fixture");
    conn.execute_batch("DROP INDEX work_events_run_cursor;")
        .expect("remove one schema object");
    drop(conn);

    let error = assert_rejected_without_mutation(&config).await;
    assert!(matches!(
        error,
        StoreError::IncompatibleSchema {
            kind: SchemaIncompatibility::Shape { .. },
            ..
        }
    ));
}

#[tokio::test]
async fn opening_future_schema_marker_is_rejected_without_mutation() {
    let home = TempDir::new().expect("temp store root");
    let config = store_config(home.path());
    create_current_database(&config.path);
    let conn = Connection::open(&config.path).expect("open future fixture");
    conn.execute(
        "UPDATE schema_state SET version = ?1 WHERE name = ?2",
        (STORE_SCHEMA_VERSION + 1, STORE_SCHEMA_MARKER),
    )
    .expect("install future marker");
    drop(conn);

    let error = assert_rejected_without_mutation(&config).await;
    assert!(matches!(
        error,
        StoreError::IncompatibleSchema {
            kind: SchemaIncompatibility::Marker {
                expected_version: STORE_SCHEMA_VERSION,
                found_version: Some(found_version),
                ..
            },
            ..
        } if found_version == STORE_SCHEMA_VERSION + 1
    ));
}

#[tokio::test]
async fn opening_extra_schema_marker_is_rejected_without_mutation() {
    let home = TempDir::new().expect("temp store root");
    let config = store_config(home.path());
    create_current_database(&config.path);
    let conn = Connection::open(&config.path).expect("open extra marker fixture");
    conn.execute(
        "INSERT INTO schema_state (name, version) VALUES ('unexpected_schema', ?1)",
        [STORE_SCHEMA_VERSION],
    )
    .expect("install extra marker");
    drop(conn);

    let error = assert_rejected_without_mutation(&config).await;
    assert!(matches!(
        error,
        StoreError::IncompatibleSchema {
            kind: SchemaIncompatibility::Marker {
                found_names,
                found_version: None,
                ..
            },
            ..
        } if found_names == [STORE_SCHEMA_MARKER, "unexpected_schema"]
    ));
}

#[tokio::test]
async fn rejected_pending_wal_schema_preserves_main_wal_and_shm_bytes() {
    let home = TempDir::new().expect("temp store root");
    let config = store_config(home.path());
    create_current_wal_database(&config.path);
    let conn = Connection::open(&config.path).expect("open pending WAL fixture");
    conn.execute(
        "UPDATE schema_state SET version = ?1 WHERE name = ?2",
        (STORE_SCHEMA_VERSION + 1, STORE_SCHEMA_MARKER),
    )
    .expect("write future marker into WAL");
    assert!(sidecar_path(&config.path, "-wal").exists());
    assert!(sidecar_path(&config.path, "-shm").exists());
    let before = database_snapshot(&config.path);

    let error = NoemaStore::open(&config)
        .await
        .expect_err("pending future schema must be rejected");

    assert!(matches!(error, StoreError::IncompatibleSchema { .. }));
    assert_eq!(database_snapshot(&config.path), before);
    drop(conn);
}

#[tokio::test]
async fn current_schema_with_pending_wal_rows_opens_and_preserves_data() {
    let home = TempDir::new().expect("temp store root");
    let config = store_config(home.path());
    create_current_wal_database(&config.path);
    let writer = Connection::open(&config.path).expect("open pending WAL fixture");
    writer
        .execute(
            "INSERT INTO humans (human_id, display_name) VALUES ('human:pending', 'Pending')",
            [],
        )
        .expect("write application row into WAL");
    assert!(sidecar_path(&config.path, "-wal").exists());

    let store = NoemaStore::open(&config)
        .await
        .expect("current pending-WAL schema must open");
    let display_name = store
        .with_connection(|conn| {
            conn.query_row(
                "SELECT display_name FROM humans WHERE human_id = 'human:pending'",
                [],
                |row| row.get::<_, String>(0),
            )
            .map_err(StoreError::Sqlite)
        })
        .await
        .expect("read pending WAL row");

    assert_eq!(display_name, "Pending");
    drop(store);
    drop(writer);
}

#[tokio::test]
async fn hot_rollback_journal_is_recovered_only_in_private_inspection_copy() {
    let home = TempDir::new().expect("temp store root");
    let source_path = home.path().join("source.sqlite3");
    let target_path = home.path().join("db/noema.sqlite3");
    fs::create_dir_all(target_path.parent().expect("database parent"))
        .expect("create database parent");
    let mut source = Connection::open(&source_path).expect("open crash source");
    source
        .execute_batch(
            r#"
            PRAGMA journal_mode = DELETE;
            PRAGMA synchronous = FULL;
            CREATE TABLE seed (value TEXT);
            DROP TABLE seed;
            VACUUM;
            PRAGMA cache_size = 1;
            PRAGMA cache_spill = ON;
            "#,
        )
        .expect("prepare empty rollback-journal database");
    let tx = source
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .expect("begin interrupted bootstrap");
    tx.execute_batch(
        r#"
        CREATE TABLE schema_state (
          name TEXT PRIMARY KEY NOT NULL,
          version INTEGER NOT NULL,
          applied_at TEXT NOT NULL
        );
        CREATE TABLE interrupted_bootstrap (payload BLOB NOT NULL);
        INSERT INTO interrupted_bootstrap (payload) VALUES (zeroblob(2097152));
        "#,
    )
    .expect("write partial bootstrap pages");
    let source_journal = sidecar_path(&source_path, "-journal");
    assert!(source_journal.exists());
    assert!(
        fs::metadata(&source_journal)
            .expect("journal metadata")
            .len()
            > 512
    );
    fs::copy(&source_path, &target_path).expect("copy interrupted main database");
    let target_journal = sidecar_path(&target_path, "-journal");
    fs::copy(&source_journal, &target_journal).expect("copy hot rollback journal");
    drop(tx);
    drop(source);

    let main_before = fs::read(&target_path).expect("snapshot interrupted main database");
    let journal_before = fs::read(&target_journal).expect("snapshot hot rollback journal");
    assert!(
        inspect_empty_schema_for_test(&target_path).expect("inspect hot-journal family"),
        "private-copy recovery should reveal the pre-bootstrap empty schema"
    );
    assert_eq!(
        fs::read(&target_path).expect("reread interrupted main database"),
        main_before
    );
    assert_eq!(
        fs::read(&target_journal).expect("reread hot rollback journal"),
        journal_before
    );

    NoemaStore::open(&StoreConfig::new(&target_path))
        .await
        .expect("recover hot journal and bootstrap current schema");
}

#[tokio::test]
async fn non_sqlite_file_is_typed_incompatible_and_unchanged() {
    let home = TempDir::new().expect("temp store root");
    let config = store_config(home.path());
    fs::create_dir_all(config.path.parent().expect("database parent"))
        .expect("create database parent");
    fs::write(&config.path, b"not a sqlite database").expect("write invalid database");
    let before = fs::read(&config.path).expect("snapshot invalid database");

    let error = NoemaStore::open(&config)
        .await
        .expect_err("non-SQLite file must be rejected");

    assert!(matches!(
        error,
        StoreError::IncompatibleSchema {
            kind: SchemaIncompatibility::Unreadable,
            ..
        }
    ));
    assert_eq!(
        fs::read(&config.path).expect("reread invalid database"),
        before
    );
    assert!(!sidecar_path(&config.path, "-wal").exists());
    assert!(!sidecar_path(&config.path, "-shm").exists());
}

#[test]
fn injected_mid_bootstrap_failure_rolls_back_every_schema_object() {
    let home = TempDir::new().expect("temp store root");
    let path = home.path().join("db/noema.sqlite3");
    fs::create_dir_all(path.parent().expect("database parent")).expect("create database parent");
    fs::File::create(&path).expect("create empty database file");
    let mut conn = Connection::open(&path).expect("open empty fixture");
    let before = database_snapshot(&path);
    let faulty_schema = STORE_SCHEMA_SQL.replacen(
        "INSERT INTO schema_state (name, version, applied_at)",
        "SELECT noema_injected_bootstrap_failure();\n\nINSERT INTO schema_state (name, version, applied_at)",
        1,
    );

    bootstrap_schema_for_test(&mut conn, &faulty_schema)
        .expect_err("injected bootstrap failure must abort");
    drop(conn);

    let after = database_snapshot(&path);
    assert_eq!(after, before);
    assert!(after.schema_objects.is_empty());
    assert!(after.schema_markers.is_none());
}

#[tokio::test]
async fn immutable_schema_inspection_handles_uri_reserved_path_characters() {
    let home = TempDir::new().expect("temp store root");
    let config = StoreConfig::new(
        home.path()
            .join("db with space")
            .join("noema?#schema.sqlite3"),
    );
    drop(NoemaStore::open(&config).await.expect("bootstrap store"));

    NoemaStore::open(&config)
        .await
        .expect("inspect reserved-character path");
}

fn stage(
    stable_key: &str,
    display_name: &str,
    ordinal: i64,
    behavior: &str,
    board_visible: i64,
) -> (String, String, String, i64, String, i64) {
    (
        format!("stage:personal:{stable_key}"),
        stable_key.to_string(),
        display_name.to_string(),
        ordinal,
        behavior.to_string(),
        board_visible,
    )
}

fn count_where(conn: &Connection, table: &str, predicate: &str) -> rusqlite::Result<i64> {
    conn.query_row(
        &format!("SELECT COUNT(*) FROM {table} WHERE {predicate}"),
        [],
        |row| row.get(0),
    )
}

fn schema_object_exists(
    conn: &Connection,
    object_type: &str,
    name: &str,
) -> rusqlite::Result<bool> {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type = ?1 AND name = ?2)",
        params![object_type, name],
        |row| row.get(0),
    )
}

fn table_columns(conn: &Connection, table: &str) -> rusqlite::Result<Vec<String>> {
    let mut statement = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    statement
        .query_map([], |row| row.get(1))?
        .collect::<Result<Vec<_>, _>>()
}

fn insert_planner_run(conn: &Connection, run_id: &str) -> rusqlite::Result<usize> {
    conn.execute(
        r#"
        INSERT INTO agent_runs (
          run_id, task_id, task_generation, contract_id, run_kind, agent_id,
          attempt_index, review_round, provider_kind, provider_account_id,
          provider_instance_key, selection_mode, model_profile,
          max_provider_continuations, max_tool_calls, max_active_minutes,
          progress_audit_interval, max_automatic_retries, max_review_rounds, status
        ) VALUES (
          ?1, 'task:valid', 1, NULL, 'planner', 'agent:task-executor',
          0, 0, 'codex', 'provider_account:codex:default',
          'provider_account:codex:default', 'explicit_profile', 'gpt-5.6-luna',
          80, 400, 120, 20, 3, 3, 'queued'
        )
        "#,
        [run_id],
    )
}

fn insert_delegated_contract(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        r#"
        INSERT INTO task_execution_contracts (
          contract_id, task_id, version, task_generation, origin,
          request_markdown, complexity,
          executor_provider_kind, executor_provider_account_id,
          executor_provider_instance_key, executor_selection_mode,
          executor_model_profile,
          reviewer_provider_kind, reviewer_provider_account_id,
          reviewer_provider_instance_key, reviewer_selection_mode,
          reviewer_model_profile,
          max_provider_continuations, max_tool_calls, max_active_minutes,
          progress_audit_interval, max_automatic_retries, max_review_rounds,
          workspace_id_snapshot, workspace_name_snapshot,
          workspace_description_snapshot, created_by_actor_id
        ) VALUES (
          'contract:one', 'task:valid', 1, 1, 'delegated',
          'Complete the task', 'simple',
          'codex', 'provider_account:codex:default',
          'provider_account:codex:default', 'explicit_profile', 'gpt-5.6-luna',
          'codex', 'provider_account:codex:default',
          'provider_account:codex:default', 'explicit_profile', 'gpt-5.6-luna',
          80, 400, 120, 20, 3, 3,
          'workspace:personal', 'Personal', '', 'actor:system'
        );
        "#,
    )
}
