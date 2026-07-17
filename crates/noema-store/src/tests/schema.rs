use std::fs;

use rusqlite::Connection;
use tempfile::TempDir;

use super::{schema_support::*, support::store_config, test_store};
use crate::{
    NoemaStore, SchemaIncompatibility, StoreConfig, StoreError,
    runtime::{bootstrap_schema_for_test, inspect_empty_schema_for_test},
    schema::{STORE_SCHEMA_MARKER, STORE_SCHEMA_SQL, STORE_SCHEMA_VERSION},
};

#[tokio::test]
async fn opens_sqlite_store_under_noema_db_dir() {
    let home = TempDir::new().expect("temp store root");
    let config = store_config(home.path());

    let store = NoemaStore::open(&config).await.expect("open store");

    assert!(home.path().join("db").exists());
    assert!(config.path.exists());
    assert_eq!(
        store.schema_version().await.expect("schema version"),
        STORE_SCHEMA_VERSION
    );
}

#[tokio::test]
async fn empty_database_bootstraps_the_exact_current_schema() {
    let home = TempDir::new().expect("temp store root");
    let config = store_config(home.path());

    let store = NoemaStore::open(&config).await.expect("bootstrap store");
    drop(store);

    let actual = database_snapshot(&config.path);
    let expected = canonical_schema_objects();
    assert_eq!(actual.schema_objects, expected);
    assert!(matches!(
        actual.schema_markers.as_deref(),
        Some([(name, STORE_SCHEMA_VERSION, _)]) if name == STORE_SCHEMA_MARKER
    ));
}

#[tokio::test]
async fn exact_current_database_reopens_without_changing_marker_or_rows() {
    let home = TempDir::new().expect("temp store root");
    let config = store_config(home.path());
    let store = NoemaStore::open(&config).await.expect("open store");
    store
        .with_connection(|conn| {
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
        .expect("seed exact current database");
    drop(store);

    let reopened = NoemaStore::open(&config).await.expect("reopen exact store");
    let (applied_at, display_name) = reopened
        .with_connection(|conn| {
            let applied_at = conn.query_row(
                "SELECT applied_at FROM schema_state WHERE name = ?1",
                [STORE_SCHEMA_MARKER],
                |row| row.get::<_, String>(0),
            )?;
            let display_name = conn.query_row(
                "SELECT display_name FROM humans WHERE human_id = 'human:preserved'",
                [],
                |row| row.get::<_, String>(0),
            )?;
            Ok((applied_at, display_name))
        })
        .await
        .expect("read preserved schema marker and data");

    assert_eq!(applied_at, "sentinel");
    assert_eq!(display_name, "Preserved");
}

#[tokio::test]
async fn opening_pre_v1_task_runtime_tables_is_rejected_without_mutation() {
    let home = TempDir::new().expect("temp store root");
    let config = store_config(home.path());
    create_current_wal_database(&config.path);
    let conn = Connection::open(&config.path).expect("open legacy fixture");
    conn.execute_batch(
        r#"
        DROP TABLE agent_run_items;
        DROP TABLE agent_runs;
        CREATE TABLE agent_runs (
          run_id TEXT PRIMARY KEY NOT NULL,
          task_id TEXT NOT NULL,
          run_kind TEXT NOT NULL CHECK (run_kind IN ('executor', 'reviewer', 'completion_delivery')),
          agent_id TEXT NOT NULL,
          provider_kind TEXT NOT NULL,
          provider_account_id TEXT NOT NULL,
          selection_mode TEXT NOT NULL,
          status TEXT NOT NULL,
          input_tokens INTEGER,
          output_tokens INTEGER
        );
        CREATE TABLE agent_run_items (
          item_id TEXT PRIMARY KEY NOT NULL,
          run_id TEXT NOT NULL,
          sequence_index INTEGER NOT NULL,
          kind TEXT NOT NULL,
          content_text TEXT,
          payload_json TEXT NOT NULL DEFAULT '{}',
          created_at TEXT NOT NULL DEFAULT 'old',
          UNIQUE(run_id, sequence_index)
        );
        INSERT INTO agent_runs (
          run_id, task_id, run_kind, agent_id, provider_kind,
          provider_account_id, selection_mode, status
        ) VALUES (
          'run:legacy', 'task:legacy', 'executor', 'agent:task-executor',
          'codex', 'provider_account:codex:default', 'explicit_profile', 'failed'
        );
        INSERT INTO agent_run_items (
          item_id, run_id, sequence_index, kind, content_text
        ) VALUES (
          'item:legacy', 'run:legacy', 1, 'assistant_output', 'preserved'
        );
        "#,
    )
    .expect("install pre-v1 runtime schema");
    checkpoint_and_remove_sidecars(conn, &config.path);
    assert_eq!(sqlite_header_versions(&config.path), [2, 2]);
    assert!(!sidecar_path(&config.path, "-wal").exists());
    assert!(!sidecar_path(&config.path, "-shm").exists());

    let error = assert_rejected_without_mutation(&config).await;
    assert!(matches!(
        error,
        StoreError::IncompatibleSchema {
            kind: SchemaIncompatibility::Shape { .. },
            ..
        }
    ));

    let conn = immutable_connection(&config.path);
    let text = conn
        .query_row(
            "SELECT content_text FROM agent_run_items WHERE item_id = 'item:legacy'",
            [],
            |row| row.get::<_, String>(0),
        )
        .expect("legacy row remains readable");
    assert_eq!(text, "preserved");
}

#[tokio::test]
async fn opening_legacy_tasks_without_blocking_columns_is_rejected_without_mutation() {
    let home = TempDir::new().expect("temp store root");
    let config = store_config(home.path());
    create_current_wal_database(&config.path);
    let conn = Connection::open(&config.path).expect("open legacy fixture");
    conn.execute_batch(
        "ALTER TABLE tasks DROP COLUMN blocked_question; ALTER TABLE tasks DROP COLUMN blocked_context;",
    )
    .expect("install legacy task schema");
    checkpoint_and_remove_sidecars(conn, &config.path);
    assert_eq!(sqlite_header_versions(&config.path), [2, 2]);
    assert!(!sidecar_path(&config.path, "-wal").exists());
    assert!(!sidecar_path(&config.path, "-shm").exists());

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
async fn opening_partial_schema_is_rejected_without_mutation() {
    let home = TempDir::new().expect("temp store root");
    let config = store_config(home.path());
    create_current_database(&config.path);
    let conn = Connection::open(&config.path).expect("open partial fixture");
    conn.execute_batch("DROP INDEX run_events_run_sequence;")
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
async fn opening_unknown_schema_is_rejected_without_mutation() {
    let home = TempDir::new().expect("temp store root");
    let config = store_config(home.path());
    create_current_database(&config.path);
    let conn = Connection::open(&config.path).expect("open unknown fixture");
    conn.execute_batch("CREATE TABLE unknown_owner_data (value TEXT NOT NULL);")
        .expect("install unknown schema object");
    drop(conn);

    assert_rejected_without_mutation(&config).await;
}

#[tokio::test]
async fn opening_unknown_schema_marker_is_rejected_without_mutation() {
    let home = TempDir::new().expect("temp store root");
    let config = store_config(home.path());
    create_current_database(&config.path);
    let conn = Connection::open(&config.path).expect("open unknown marker fixture");
    conn.execute(
        "UPDATE schema_state SET name = 'unknown_schema' WHERE name = ?1",
        [STORE_SCHEMA_MARKER],
    )
    .expect("install unknown marker");
    drop(conn);

    let error = assert_rejected_without_mutation(&config).await;
    assert!(matches!(
        error,
        StoreError::IncompatibleSchema {
            kind: SchemaIncompatibility::Marker {
                found_names,
                found_version: Some(STORE_SCHEMA_VERSION),
                ..
            },
            ..
        } if found_names == ["unknown_schema"]
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

    let recovered = NoemaStore::open(&StoreConfig::new(&target_path))
        .await
        .expect("recover hot journal and bootstrap current schema");
    assert_eq!(
        recovered.schema_version().await.expect("schema version"),
        STORE_SCHEMA_VERSION
    );
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

#[tokio::test]
async fn explicit_fresh_path_recovers_after_non_mutating_rejection() {
    let home = TempDir::new().expect("temp store root");
    let config = store_config(home.path());
    create_current_database(&config.path);
    let conn = Connection::open(&config.path).expect("open incompatible fixture");
    conn.execute_batch("CREATE TABLE obsolete_pre_v1_data (value TEXT NOT NULL);")
        .expect("install incompatible schema object");
    drop(conn);
    let rejected_bytes = fs::read(&config.path).expect("snapshot rejected database");

    NoemaStore::open(&config)
        .await
        .expect_err("ordinary startup must not reset an incompatible database");
    assert_eq!(
        fs::read(&config.path).expect("reread rejected database"),
        rejected_bytes
    );

    // Destructive recovery is deliberately caller-owned and explicit.
    for path in [
        config.path.clone(),
        sidecar_path(&config.path, "-wal"),
        sidecar_path(&config.path, "-shm"),
    ] {
        if path.exists() {
            fs::remove_file(path).expect("remove rejected database family");
        }
    }
    let recovered = NoemaStore::open(&config)
        .await
        .expect("open explicitly fresh path");

    assert_eq!(
        recovered.schema_version().await.expect("schema version"),
        STORE_SCHEMA_VERSION
    );
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
async fn existing_zero_byte_database_bootstraps_successfully() {
    let home = TempDir::new().expect("temp store root");
    let config = store_config(home.path());
    fs::create_dir_all(config.path.parent().expect("database parent"))
        .expect("create database parent");
    fs::File::create(&config.path).expect("create zero-byte database");

    let store = NoemaStore::open(&config)
        .await
        .expect("bootstrap zero-byte database");

    assert_eq!(
        store.schema_version().await.expect("schema version"),
        STORE_SCHEMA_VERSION
    );
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

    let reopened = NoemaStore::open(&config)
        .await
        .expect("inspect reserved-character path");

    assert_eq!(
        reopened.schema_version().await.expect("schema version"),
        STORE_SCHEMA_VERSION
    );
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
            .map_err(StoreError::Sqlite)
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
                .map_err(StoreError::Sqlite)
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
