use std::fs;

use rusqlite::Connection;
use tempfile::TempDir;

use super::{schema_support::*, support::store_config};
use crate::{
    NoemaStore, SchemaIncompatibility, StoreConfig, StoreError,
    runtime::{bootstrap_schema_for_test, inspect_empty_schema_for_test},
    schema::{STORE_SCHEMA_MARKER, STORE_SCHEMA_SQL, STORE_SCHEMA_VERSION},
};

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
                ("tasks", "executor_provider_instance_key"),
                ("tasks", "reviewer_provider_instance_key"),
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
        LegacyTasks,
        UnknownSchema,
        UnknownMarker,
    }

    for fixture in [
        Fixture::PreV1Runtime,
        Fixture::LegacyTasks,
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
            Fixture::LegacyTasks => conn
                .execute_batch(
                    "ALTER TABLE tasks DROP COLUMN blocked_question; ALTER TABLE tasks DROP COLUMN blocked_context;",
                )
                .expect("legacy tasks fixture"),
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
                    Fixture::PreV1Runtime | Fixture::LegacyTasks | Fixture::UnknownSchema,
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
