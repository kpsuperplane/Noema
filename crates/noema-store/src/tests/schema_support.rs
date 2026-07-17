use std::{fs, path::Path};

use rusqlite::{Connection, OpenFlags};

use crate::{NoemaStore, StoreConfig, StoreError, schema::STORE_SCHEMA_SQL};

#[derive(Debug, PartialEq, Eq)]
pub(super) struct DatabaseSnapshot {
    pub(super) main: Option<Vec<u8>>,
    pub(super) wal: Option<Vec<u8>>,
    pub(super) shm: Option<Vec<u8>>,
    pub(super) journal: Option<Vec<u8>>,
    pub(super) schema_objects: Vec<(String, String, String, Option<String>)>,
    pub(super) schema_markers: Option<Vec<(String, i64, String)>>,
    pub(super) table_row_counts: Vec<(String, i64)>,
}

pub(super) async fn assert_rejected_without_mutation(config: &StoreConfig) -> StoreError {
    let before = database_snapshot(&config.path);
    let error = NoemaStore::open(config)
        .await
        .expect_err("incompatible schema must be rejected");
    let after = database_snapshot(&config.path);

    assert!(matches!(error, StoreError::IncompatibleSchema { .. }));
    assert_eq!(after, before);
    error
}

pub(super) fn create_current_database(path: &Path) {
    fs::create_dir_all(path.parent().expect("database parent")).expect("create database parent");
    let mut conn = Connection::open(path).expect("create current fixture");
    let tx = conn
        .transaction()
        .expect("begin schema fixture transaction");
    tx.execute_batch(STORE_SCHEMA_SQL)
        .expect("execute current schema");
    tx.commit().expect("commit current schema");
}

pub(super) fn create_current_wal_database(path: &Path) {
    create_current_database(path);
    let conn = Connection::open(path).expect("open WAL fixture");
    conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL;")
        .expect("persist WAL mode");
    checkpoint_and_remove_sidecars(conn, path);
}

pub(super) fn checkpoint_and_remove_sidecars(conn: Connection, path: &Path) {
    conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
        .expect("checkpoint fixture WAL");
    drop(conn);
    for suffix in ["-wal", "-shm"] {
        let sidecar = sidecar_path(path, suffix);
        if sidecar.exists() {
            fs::remove_file(sidecar).expect("remove checkpointed fixture sidecar");
        }
    }
}

pub(super) fn canonical_schema_objects() -> Vec<(String, String, String, Option<String>)> {
    let conn = Connection::open_in_memory().expect("open canonical schema database");
    conn.execute_batch(STORE_SCHEMA_SQL)
        .expect("execute canonical schema");
    read_schema_objects(&conn)
}

pub(super) fn database_snapshot(path: &Path) -> DatabaseSnapshot {
    let main = file_bytes(path);
    let wal = file_bytes(&sidecar_path(path, "-wal"));
    let shm = file_bytes(&sidecar_path(path, "-shm"));
    let journal = file_bytes(&sidecar_path(path, "-journal"));
    if main.as_ref().is_none_or(Vec::is_empty) {
        return DatabaseSnapshot {
            main,
            wal,
            shm,
            journal,
            schema_objects: Vec::new(),
            schema_markers: None,
            table_row_counts: Vec::new(),
        };
    }

    let conn = immutable_connection(path);
    let schema_objects = read_schema_objects(&conn);
    let table_names = schema_objects
        .iter()
        .filter(|(object_type, name, _, _)| object_type == "table" && name != "schema_state")
        .map(|(_, name, _, _)| name.clone())
        .collect::<Vec<_>>();
    let table_row_counts = table_names
        .into_iter()
        .map(|table| {
            let quoted = table.replace('"', "\"\"");
            let count = conn
                .query_row(&format!("SELECT COUNT(*) FROM \"{quoted}\""), [], |row| {
                    row.get::<_, i64>(0)
                })
                .expect("count table rows");
            (table, count)
        })
        .collect();
    let has_schema_state = schema_objects
        .iter()
        .any(|(object_type, name, _, _)| object_type == "table" && name == "schema_state");
    let schema_markers = has_schema_state.then(|| {
        let mut statement = conn
            .prepare("SELECT name, version, applied_at FROM schema_state ORDER BY name")
            .expect("prepare schema marker snapshot");
        let rows = statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
            .expect("query schema markers");
        rows.collect::<Result<Vec<_>, _>>()
            .expect("collect schema markers")
    });
    DatabaseSnapshot {
        main,
        wal,
        shm,
        journal,
        schema_objects,
        schema_markers,
        table_row_counts,
    }
}

fn read_schema_objects(conn: &Connection) -> Vec<(String, String, String, Option<String>)> {
    let mut statement = conn
        .prepare(
            r#"
            SELECT type, name, tbl_name, sql
            FROM sqlite_schema
            WHERE name NOT LIKE 'sqlite_%'
            ORDER BY type, name, tbl_name
            "#,
        )
        .expect("prepare schema snapshot");
    let rows = statement
        .query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })
        .expect("query schema snapshot");
    rows.collect::<Result<Vec<_>, _>>()
        .expect("collect schema snapshot")
}

pub(super) fn immutable_connection(path: &Path) -> Connection {
    let encoded_path = path
        .to_string_lossy()
        .bytes()
        .fold(String::new(), |mut encoded, byte| {
            if byte.is_ascii_alphanumeric()
                || matches!(byte, b'/' | b':' | b'-' | b'_' | b'.' | b'~')
            {
                encoded.push(char::from(byte));
            } else {
                use std::fmt::Write as _;
                write!(encoded, "%{byte:02X}").expect("encode path byte");
            }
            encoded
        });
    Connection::open_with_flags(
        format!("file:{encoded_path}?immutable=1"),
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
    )
    .expect("open immutable database snapshot")
}

pub(super) fn sqlite_header_versions(path: &Path) -> [u8; 2] {
    let bytes = fs::read(path).expect("read SQLite header");
    [bytes[18], bytes[19]]
}

pub(super) fn sidecar_path(path: &Path, suffix: &str) -> std::path::PathBuf {
    let mut value = path.as_os_str().to_os_string();
    value.push(suffix);
    value.into()
}

fn file_bytes(path: &Path) -> Option<Vec<u8>> {
    path.exists()
        .then(|| fs::read(path).expect("read database-family file"))
}
