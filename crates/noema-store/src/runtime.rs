#[cfg(any(test, feature = "test-support"))]
use std::sync::atomic::{AtomicUsize, Ordering};
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
    schema::{STORE_SCHEMA_MARKER, STORE_SCHEMA_SQL, STORE_SCHEMA_VERSION},
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
    #[cfg(any(test, feature = "test-support"))]
    operation_count: Arc<AtomicUsize>,
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
        if let Some(parent) = config.path.parent() {
            fs::create_dir_all(parent).map_err(StoreError::PreparePath)?;
        }

        let initial_state = inspect_existing_database(&config.path)?;
        if let SchemaCompatibility::Incompatible { kind, reason } = initial_state {
            return Err(StoreError::IncompatibleSchema { kind, reason });
        }

        let mut conn = Connection::open(&config.path)?;
        match initial_state {
            SchemaCompatibility::Empty => {
                bootstrap_empty_database(&mut conn, STORE_SCHEMA_SQL)?;
            }
            SchemaCompatibility::Current => {
                require_current_schema(classify_schema(&conn)?)?;
            }
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
            #[cfg(any(test, feature = "test-support"))]
            operation_count: Arc::new(AtomicUsize::new(0)),
        })
    }

    #[cfg(test)]
    #[must_use]
    #[allow(dead_code)]
    pub(crate) fn connection_for_tests(&self) -> Arc<Mutex<Connection>> {
        self.conn.clone()
    }

    /// Reset the test-only count of Store connection operations.
    #[cfg(any(test, feature = "test-support"))]
    pub fn reset_operation_count_for_tests(&self) {
        self.operation_count.store(0, Ordering::Relaxed);
    }

    /// Return the test-only count of Store connection operations.
    #[cfg(any(test, feature = "test-support"))]
    #[must_use]
    pub fn operation_count_for_tests(&self) -> usize {
        self.operation_count.load(Ordering::Relaxed)
    }

    pub(crate) async fn with_connection<T>(
        &self,
        work: impl FnOnce(&mut Connection) -> Result<T, StoreError>,
    ) -> Result<T, StoreError> {
        #[cfg(any(test, feature = "test-support"))]
        self.operation_count.fetch_add(1, Ordering::Relaxed);
        let mut conn = self.conn.lock().await;
        work(&mut conn)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum SchemaCompatibility {
    Empty,
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

fn bootstrap_empty_database(conn: &mut Connection, schema_sql: &str) -> Result<(), StoreError> {
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    match classify_schema(&tx)? {
        SchemaCompatibility::Empty => {}
        SchemaCompatibility::Current => {
            tx.commit()?;
            return Ok(());
        }
        SchemaCompatibility::Incompatible { kind, reason } => {
            return Err(StoreError::IncompatibleSchema { kind, reason });
        }
    }

    tx.execute_batch(schema_sql)?;
    match classify_schema(&tx)? {
        SchemaCompatibility::Current => tx.commit().map_err(StoreError::Sqlite),
        SchemaCompatibility::Empty => Err(StoreError::Schema(
            "bootstrap completed without creating schema objects".to_string(),
        )),
        SchemaCompatibility::Incompatible { reason, .. } => Err(StoreError::Schema(format!(
            "bootstrap produced an incompatible schema: {reason}"
        ))),
    }
}

fn classify_schema(conn: &Connection) -> Result<SchemaCompatibility, StoreError> {
    let actual_objects = schema_objects(conn)?;
    if actual_objects.is_empty() {
        return Ok(SchemaCompatibility::Empty);
    }

    let expected_objects = canonical_schema_objects()?;
    if actual_objects != expected_objects {
        return Ok(SchemaCompatibility::Incompatible {
            kind: SchemaIncompatibility::Shape {
                expected_object_count: expected_objects.len(),
                found_object_count: actual_objects.len(),
            },
            reason: schema_difference(&expected_objects, &actual_objects),
        });
    }

    let marker_rows = schema_marker_rows(conn)?;
    let expected_marker = vec![(STORE_SCHEMA_MARKER.to_string(), STORE_SCHEMA_VERSION)];
    if marker_rows != expected_marker {
        let found_names = marker_rows.iter().map(|(name, _)| name.clone()).collect();
        let found_version = (marker_rows.len() == 1).then(|| marker_rows[0].1);
        return Ok(SchemaCompatibility::Incompatible {
            kind: SchemaIncompatibility::Marker {
                expected_name: STORE_SCHEMA_MARKER,
                found_names,
                expected_version: STORE_SCHEMA_VERSION,
                found_version,
            },
            reason: format!("expected schema marker {expected_marker:?}, found {marker_rows:?}"),
        });
    }

    Ok(SchemaCompatibility::Current)
}

fn require_current_schema(state: SchemaCompatibility) -> Result<(), StoreError> {
    match state {
        SchemaCompatibility::Current => Ok(()),
        SchemaCompatibility::Empty => Err(StoreError::IncompatibleSchema {
            kind: SchemaIncompatibility::ChangedDuringOpen,
            reason: "database became empty during schema handshake".to_string(),
        }),
        SchemaCompatibility::Incompatible { kind, reason } => {
            Err(StoreError::IncompatibleSchema { kind, reason })
        }
    }
}

fn canonical_schema_objects() -> Result<Vec<SchemaObject>, StoreError> {
    let conn = Connection::open_in_memory()?;
    conn.execute_batch(STORE_SCHEMA_SQL)?;
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
        Ok(SchemaObject {
            object_type: row.get(0)?,
            name: row.get(1)?,
            table_name: row.get(2)?,
            sql: row.get(3)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StoreError::Sqlite)
}

fn schema_marker_rows(conn: &Connection) -> Result<Vec<(String, i64)>, StoreError> {
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
pub(crate) fn bootstrap_schema_for_test(
    conn: &mut Connection,
    schema_sql: &str,
) -> Result<(), StoreError> {
    bootstrap_empty_database(conn, schema_sql)
}

#[cfg(test)]
pub(crate) fn inspect_empty_schema_for_test(path: &Path) -> Result<bool, StoreError> {
    inspect_existing_database(path).map(|state| state == SchemaCompatibility::Empty)
}
