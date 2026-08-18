use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

use rusqlite::{Connection, OpenFlags, TransactionBehavior};
use tempfile::{Builder as TempDirBuilder, TempDir};
use tokio::sync::Mutex;

use super::{
    error::{SchemaIncompatibility, StoreError},
    schema::{LEGACY_SCHEMA_MARKER, STORE_SCHEMA_VERSION, store_migrations},
};

const STORE_RUNTIME_PRAGMAS_SQL: &str = r#"
PRAGMA foreign_keys = ON;
PRAGMA journal_mode = WAL;
PRAGMA synchronous = NORMAL;
"#;

/// Configuration for the local Noema store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreConfig {
    /// Path to the SQLite database file.
    pub path: PathBuf,
}

impl StoreConfig {
    /// Build store config from an explicit SQLite database-file path.
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }
}

/// Server-owned local canonical store.
#[derive(Debug, Clone)]
pub struct NoemaStore {
    pub(super) conn: Arc<Mutex<Connection>>,
    pub(super) append_item_lock: Arc<Mutex<()>>,
    pub(super) client_revocations: Arc<tokio::sync::broadcast::Sender<String>>,
    pub(super) home_root: Arc<PathBuf>,
}

impl NoemaStore {
    /// Open and bootstrap the embedded store.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the database path cannot be prepared,
    /// SQLite cannot be opened, or an existing database is not the exact
    /// schema understood by this binary.
    pub async fn open(config: &StoreConfig) -> Result<Self, StoreError> {
        let home_root =
            std::path::absolute(infer_home_root(&config.path)).map_err(StoreError::PreparePath)?;
        if let Some(parent) = config.path.parent() {
            fs::create_dir_all(parent).map_err(StoreError::PreparePath)?;
        }

        let initial_state = inspect_existing_database(&config.path)?;
        if let SchemaCompatibility::Incompatible { kind, reason } = initial_state {
            return Err(StoreError::IncompatibleSchema { kind, reason });
        }

        let mut conn = Connection::open(&config.path)?;
        match initial_state {
            SchemaCompatibility::Empty
            | SchemaCompatibility::LegacyBaseline
            | SchemaCompatibility::Migratable { .. } => migrate_accepted_database(&mut conn)?,
            SchemaCompatibility::Current => require_current_schema(classify_schema(&conn)?)?,
            SchemaCompatibility::Incompatible { .. } => {
                unreachable!("incompatible schemas return before the writable connection is opened")
            }
        }

        // These pragmas are deliberately applied only after the strict schema
        // handshake accepts the file or the atomic bootstrap commits. In
        // particular, journal_mode=WAL must never touch a rejected database.
        conn.execute_batch(STORE_RUNTIME_PRAGMAS_SQL)?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
            append_item_lock: Arc::new(Mutex::new(())),
            client_revocations: Arc::new(tokio::sync::broadcast::channel(256).0),
            home_root: Arc::new(home_root),
        })
    }

    #[cfg(test)]
    #[must_use]
    #[allow(dead_code)]
    pub(crate) fn connection_for_tests(&self) -> Arc<Mutex<Connection>> {
        self.conn.clone()
    }

    pub(crate) async fn with_connection<T>(
        &self,
        work: impl FnOnce(&mut Connection) -> Result<T, StoreError>,
    ) -> Result<T, StoreError> {
        let mut conn = self.conn.lock().await;
        work(&mut conn)
    }

    #[cfg(test)]
    pub(crate) fn default_task_cwd(&self, task_id: &str) -> PathBuf {
        self.home_root.join("tasks").join(task_id)
    }
}

fn infer_home_root(database_path: &Path) -> PathBuf {
    let parent = database_path.parent().unwrap_or_else(|| Path::new("."));
    if parent.file_name().is_some_and(|name| name == "db") {
        parent.parent().unwrap_or(parent).to_path_buf()
    } else {
        parent.to_path_buf()
    }
}

#[cfg(test)]
mod path_tests {
    use super::*;

    #[tokio::test]
    async fn relative_store_path_yields_absolute_default_task_cwd() {
        let current_directory = std::env::current_dir().expect("current directory");
        let home = TempDirBuilder::new()
            .prefix("noema-relative-home-")
            .tempdir_in(&current_directory)
            .expect("relative Noema home");
        let relative_home = home
            .path()
            .strip_prefix(&current_directory)
            .expect("home under current directory");
        let store = NoemaStore::open(&StoreConfig::new(relative_home.join("db/noema.sqlite3")))
            .await
            .expect("open relative store");

        let task_cwd = store.default_task_cwd("task:relative-home");
        assert!(task_cwd.is_absolute());
        assert_eq!(
            task_cwd,
            home.path().join("tasks").join("task:relative-home")
        );
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum SchemaCompatibility {
    Empty,
    LegacyBaseline,
    Migratable {
        version: usize,
    },
    Current,
    Incompatible {
        kind: SchemaIncompatibility,
        reason: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SchemaObject {
    object_type: String,
    name: String,
    table_name: String,
    sql: Option<String>,
}

fn inspect_existing_database(path: &Path) -> Result<SchemaCompatibility, StoreError> {
    if !path.exists() {
        return Ok(SchemaCompatibility::Empty);
    }

    if fs::metadata(path).map_err(StoreError::PreparePath)?.len() == 0 {
        return Ok(SchemaCompatibility::Empty);
    }

    if has_nonempty_recovery_sidecar(path)? {
        return inspect_database_family_copy(path);
    }

    // `immutable=1` prevents even a read-only inspection from creating or
    // updating recovery sidecars. A database with pending WAL frames or a hot
    // rollback journal takes the copied-family path above so crash remnants
    // are recovered only in the private inspection directory.
    let uri = immutable_database_uri(path);
    let conn = Connection::open_with_flags(
        uri,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
    )?;
    Ok(classify_schema(&conn).unwrap_or_else(incompatible_inspection_error))
}

fn has_nonempty_recovery_sidecar(path: &Path) -> Result<bool, StoreError> {
    for suffix in ["-wal", "-journal"] {
        let sidecar = sibling_with_suffix(path, suffix);
        if sidecar.exists()
            && fs::metadata(sidecar)
                .map_err(StoreError::PreparePath)?
                .len()
                > 0
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn inspect_database_family_copy(path: &Path) -> Result<SchemaCompatibility, StoreError> {
    let copy = DatabaseFamilyCopy::create(path)?;
    let conn = Connection::open(&copy.database_path)?;
    let state = classify_schema(&conn).unwrap_or_else(incompatible_inspection_error);
    drop(conn);
    Ok(state)
}

fn incompatible_inspection_error(error: StoreError) -> SchemaCompatibility {
    SchemaCompatibility::Incompatible {
        kind: SchemaIncompatibility::Unreadable,
        reason: format!("schema metadata could not be read: {error}"),
    }
}

fn migrate_accepted_database(conn: &mut Connection) -> Result<(), StoreError> {
    // Table rebuilds need foreign-key checks disabled until all migrations in
    // the accepted sequence commit. Runtime enforcement starts after this
    // function completes.
    conn.pragma_update(None, "foreign_keys", "OFF")?;
    match classify_schema(conn)? {
        SchemaCompatibility::Empty | SchemaCompatibility::Migratable { .. } => {
            migrate_to_latest(conn)?;
        }
        SchemaCompatibility::LegacyBaseline => adopt_legacy_baseline(conn)?,
        SchemaCompatibility::Current => return Ok(()),
        SchemaCompatibility::Incompatible { kind, reason } => {
            return Err(StoreError::IncompatibleSchema { kind, reason });
        }
    }
    require_current_schema(classify_schema(conn)?)
}

fn adopt_legacy_baseline(conn: &mut Connection) -> Result<(), StoreError> {
    conn.pragma_update(None, "foreign_keys", "OFF")?;
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    if classify_schema(&tx)? != SchemaCompatibility::LegacyBaseline {
        return Err(StoreError::IncompatibleSchema {
            kind: SchemaIncompatibility::ChangedDuringOpen,
            reason: "database changed during legacy schema adoption".to_string(),
        });
    }
    // The exact legacy shape is migration 1. Mark it as such, then let the
    // migration authority apply adoption and every later schema change.
    tx.pragma_update(None, "user_version", 1)?;
    tx.commit().map_err(StoreError::Sqlite)?;
    migrate_to_latest(conn)
}

fn migrate_to_latest(conn: &mut Connection) -> Result<(), StoreError> {
    // Migration 52 temporarily narrows one rebuilt check constraint. Ignore
    // checks while later migrations restore its complete accepted values.
    conn.pragma_update(None, "ignore_check_constraints", "ON")?;
    let result = store_migrations().to_latest(conn);
    conn.pragma_update(None, "ignore_check_constraints", "OFF")?;
    result.map_err(StoreError::from)
}

fn classify_schema(conn: &Connection) -> Result<SchemaCompatibility, StoreError> {
    let actual_objects = schema_objects(conn)?;
    let found_version = schema_version(conn)?;
    if actual_objects.is_empty() {
        return if found_version == 0 {
            Ok(SchemaCompatibility::Empty)
        } else {
            Ok(incompatible_version(found_version))
        };
    }

    if found_version < 0 || found_version as usize > STORE_SCHEMA_VERSION {
        return Ok(incompatible_version(found_version));
    }

    if found_version == 0 {
        let expected_objects = canonical_schema_objects(1)?;
        if actual_objects != expected_objects {
            return Ok(incompatible_shape(&expected_objects, &actual_objects));
        }
        let marker_rows = legacy_schema_marker_rows(conn)?;
        let expected_marker = vec![(LEGACY_SCHEMA_MARKER.to_string(), 9)];
        return if marker_rows == expected_marker {
            Ok(SchemaCompatibility::LegacyBaseline)
        } else {
            Ok(SchemaCompatibility::Incompatible {
                kind: SchemaIncompatibility::LegacyMarker,
                reason: format!(
                    "expected legacy schema marker {expected_marker:?}, found {marker_rows:?}"
                ),
            })
        };
    }

    let found_version = found_version as usize;
    let expected_objects = canonical_schema_objects(found_version)?;
    if actual_objects != expected_objects {
        if found_version == 8
            && known_capability_auth_schema_drift(&expected_objects, &actual_objects)
        {
            return Ok(SchemaCompatibility::Migratable {
                version: found_version,
            });
        }
        if found_version == 15
            && known_adapter_v15_utf8_length_schema_drift(&expected_objects, &actual_objects)
        {
            return Ok(SchemaCompatibility::Migratable {
                version: found_version,
            });
        }
        if found_version == 20
            && known_task_pool_selection_index_drift(&expected_objects, &actual_objects)
        {
            return Ok(SchemaCompatibility::Migratable {
                version: found_version,
            });
        }
        if found_version == 30
            && known_recurrence_history_index_drift(&expected_objects, &actual_objects)
        {
            return Ok(SchemaCompatibility::Migratable {
                version: found_version,
            });
        }
        return Ok(incompatible_shape(&expected_objects, &actual_objects));
    }

    if found_version == STORE_SCHEMA_VERSION {
        Ok(SchemaCompatibility::Current)
    } else {
        Ok(SchemaCompatibility::Migratable {
            version: found_version,
        })
    }
}

fn known_recurrence_history_index_drift(
    expected: &[SchemaObject],
    actual: &[SchemaObject],
) -> bool {
    if expected.len() != actual.len() {
        return false;
    }
    let mut differences = expected
        .iter()
        .zip(actual)
        .filter(|(expected, actual)| expected != actual);
    let Some((expected_index, actual_index)) = differences.next() else {
        return false;
    };
    differences.next().is_none()
        && expected_index.object_type == "index"
        && expected_index.name == "task_recurrence_occurrences_history"
        && actual_index.name == expected_index.name
        && actual_index.table_name == expected_index.table_name
        && actual_index.sql.as_deref().is_some_and(|sql| {
            sql.contains("recurrence_id, scheduled_for DESC, occurrence_id DESC")
        })
}

fn known_task_pool_selection_index_drift(
    expected: &[SchemaObject],
    actual: &[SchemaObject],
) -> bool {
    if expected.len() != actual.len() {
        return false;
    }
    let mut differences = expected
        .iter()
        .zip(actual)
        .filter(|(expected, actual)| expected != actual);
    let Some((expected_index, actual_index)) = differences.next() else {
        return false;
    };
    differences.next().is_none()
        && expected_index.object_type == "index"
        && expected_index.name == "task_model_pool_entries_unique_selection"
        && actual_index.name == expected_index.name
        && actual_index.table_name == expected_index.table_name
        && expected_index.sql.as_deref().is_some_and(|sql| {
            actual_index.sql
                == Some(sql.replace(
                    "COALESCE(model_profile, pool_entry_id)",
                    "COALESCE(model_profile, '')",
                ))
        })
}

fn known_adapter_v15_utf8_length_schema_drift(
    expected: &[SchemaObject],
    actual: &[SchemaObject],
) -> bool {
    if expected.len() != actual.len() {
        return false;
    }
    let mut differences = expected
        .iter()
        .zip(actual)
        .filter(|(expected, actual)| expected != actual);
    let Some((expected_table, actual_table)) = differences.next() else {
        return false;
    };
    differences.next().is_none()
        && expected_table.name == "adapter_connections"
        && actual_table.name == "adapter_connections"
        && expected_table.sql.as_deref().is_some_and(|sql| {
            actual_table.sql
                == Some(
                    sql.replace("length(CAST(", "length(")
                        .replace(" AS BLOB))", ")"),
                )
        })
}

fn known_capability_auth_schema_drift(expected: &[SchemaObject], actual: &[SchemaObject]) -> bool {
    const DRIFT_TRIGGERS: [&str; 3] = [
        "capability_auth_requests_active_mcp_insert",
        "capability_auth_requests_active_mcp_update",
        "mcp_servers_active_capability_auth_delete",
    ];
    if actual.len() + DRIFT_TRIGGERS.len() != expected.len() {
        return false;
    }
    let actual_without_table = actual
        .iter()
        .filter(|object| object.name != "capability_auth_requests")
        .collect::<Vec<_>>();
    let expected_without_table = expected
        .iter()
        .filter(|object| {
            object.name != "capability_auth_requests"
                && !DRIFT_TRIGGERS.contains(&object.name.as_str())
        })
        .collect::<Vec<_>>();
    if actual_without_table != expected_without_table {
        return false;
    }
    let Some(table) = actual
        .iter()
        .find(|object| object.name == "capability_auth_requests")
    else {
        return false;
    };
    let Some(sql) = table.sql.as_deref() else {
        return false;
    };
    sql.contains("protected_arguments_ref")
        && sql.contains("result_context_json")
        && sql.contains("supersession_reason")
        && sql.contains("FOREIGN KEY (mcp_server_id) REFERENCES mcp_servers")
        && !sql.contains("origin_resumed_at")
        && !actual
            .iter()
            .any(|object| DRIFT_TRIGGERS.contains(&object.name.as_str()))
}

fn incompatible_version(found_version: i64) -> SchemaCompatibility {
    SchemaCompatibility::Incompatible {
        kind: SchemaIncompatibility::Version {
            expected_version: STORE_SCHEMA_VERSION,
            found_version,
        },
        reason: format!(
            "expected schema version at most {STORE_SCHEMA_VERSION}, found {found_version}"
        ),
    }
}

fn incompatible_shape(expected: &[SchemaObject], actual: &[SchemaObject]) -> SchemaCompatibility {
    SchemaCompatibility::Incompatible {
        kind: SchemaIncompatibility::Shape {
            expected_object_count: expected.len(),
            found_object_count: actual.len(),
        },
        reason: schema_difference(expected, actual),
    }
}

fn require_current_schema(state: SchemaCompatibility) -> Result<(), StoreError> {
    match state {
        SchemaCompatibility::Current => Ok(()),
        SchemaCompatibility::Empty
        | SchemaCompatibility::LegacyBaseline
        | SchemaCompatibility::Migratable { .. } => Err(StoreError::IncompatibleSchema {
            kind: SchemaIncompatibility::ChangedDuringOpen,
            reason: "database did not reach the current schema during migration".to_string(),
        }),
        SchemaCompatibility::Incompatible { kind, reason } => {
            Err(StoreError::IncompatibleSchema { kind, reason })
        }
    }
}

fn canonical_schema_objects(version: usize) -> Result<Vec<SchemaObject>, StoreError> {
    let mut conn = Connection::open_in_memory()?;
    conn.pragma_update(None, "foreign_keys", "OFF")?;
    store_migrations().to_version(&mut conn, version)?;
    schema_objects(&conn)
}

fn schema_objects(conn: &Connection) -> Result<Vec<SchemaObject>, StoreError> {
    let mut statement = conn.prepare(
        r#"
        SELECT type, name, tbl_name, sql
        FROM sqlite_schema
        WHERE name NOT LIKE 'sqlite_%'
        ORDER BY type, name, tbl_name
        "#,
    )?;
    let rows = statement.query_map([], |row| {
        let sql: Option<String> = row.get(3)?;
        Ok(SchemaObject {
            object_type: row.get(0)?,
            name: row.get(1)?,
            table_name: row.get(2)?,
            sql: sql.map(normalize_agent_run_reference_sql),
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StoreError::Sqlite)
}

fn normalize_agent_run_reference_sql(sql: String) -> String {
    sql.replace("REFERENCES \"agent_runs\"", "REFERENCES agent_runs")
}

fn schema_version(conn: &Connection) -> Result<i64, StoreError> {
    conn.query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(StoreError::Sqlite)
}

fn legacy_schema_marker_rows(conn: &Connection) -> Result<Vec<(String, i64)>, StoreError> {
    let mut statement = conn.prepare("SELECT name, version FROM schema_state ORDER BY name")?;
    let rows = statement.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StoreError::Sqlite)
}

fn schema_difference(expected: &[SchemaObject], actual: &[SchemaObject]) -> String {
    let first_difference = expected
        .iter()
        .zip(actual)
        .position(|(expected, actual)| expected != actual);
    match first_difference {
        Some(index) => format!(
            "schema object {index} differs: expected {:?}, found {:?}",
            expected[index], actual[index]
        ),
        None => format!(
            "schema object count differs: expected {}, found {}",
            expected.len(),
            actual.len()
        ),
    }
}

fn immutable_database_uri(path: &Path) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";

    let path = path_bytes(path);
    let mut uri = String::with_capacity(path.len() + 17);
    uri.push_str("file:");
    for byte in path {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b':' | b'-' | b'_' | b'.' | b'~') {
            uri.push(char::from(byte));
        } else {
            uri.push('%');
            uri.push(char::from(HEX[usize::from(byte >> 4)]));
            uri.push(char::from(HEX[usize::from(byte & 0x0f)]));
        }
    }
    uri.push_str("?immutable=1");
    uri
}

#[cfg(unix)]
fn path_bytes(path: &Path) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;

    path.as_os_str().as_bytes().to_vec()
}

#[cfg(not(unix))]
fn path_bytes(path: &Path) -> Vec<u8> {
    path.to_string_lossy().into_owned().into_bytes()
}

fn sibling_with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut value = path.as_os_str().to_os_string();
    value.push(suffix);
    PathBuf::from(value)
}

struct DatabaseFamilyCopy {
    _root: TempDir,
    database_path: PathBuf,
}

impl DatabaseFamilyCopy {
    fn create(source: &Path) -> Result<Self, StoreError> {
        let root = TempDirBuilder::new()
            .prefix("noema-schema-inspection-")
            .tempdir()
            .map_err(StoreError::PreparePath)?;
        let database_path = root.path().join("noema.sqlite3");
        fs::copy(source, &database_path).map_err(StoreError::PreparePath)?;
        for suffix in ["-wal", "-journal"] {
            let source_sidecar = sibling_with_suffix(source, suffix);
            if source_sidecar.exists() {
                let destination_sidecar = sibling_with_suffix(&database_path, suffix);
                fs::copy(source_sidecar, destination_sidecar).map_err(StoreError::PreparePath)?;
            }
        }
        Ok(Self {
            _root: root,
            database_path,
        })
    }
}

#[cfg(test)]
pub(crate) fn run_migration_for_test(
    conn: &mut Connection,
    schema_sql: &str,
) -> Result<(), StoreError> {
    rusqlite_migration::Migrations::new(vec![rusqlite_migration::M::up(schema_sql)])
        .to_latest(conn)
        .map_err(StoreError::Migration)
}

#[cfg(test)]
pub(crate) fn inspect_empty_schema_for_test(path: &Path) -> Result<bool, StoreError> {
    inspect_existing_database(path).map(|state| state == SchemaCompatibility::Empty)
}
