use std::{fs, path::PathBuf, sync::Arc};

use rusqlite::{Connection, OptionalExtension};
use tokio::sync::Mutex;

use super::{
    error::StoreError,
    schema::{
        STORE_SCHEMA_MARKER, STORE_SCHEMA_VERSION, bootstrap_schema_v2,
        expected_structural_fingerprint, structural_fingerprint,
    },
};

/// Configuration for the local Noema store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreConfig {
    /// Path to the SQLite database file.
    pub path: PathBuf,
    /// Root directory for Noema state.
    pub noema_home: PathBuf,
}

impl StoreConfig {
    /// Build store config from resolved Noema paths.
    #[must_use]
    pub fn from_paths(paths: &crate::NoemaPaths) -> Self {
        Self {
            path: paths.sqlite_db_path(),
            noema_home: paths.root().to_path_buf(),
        }
    }
}

/// Server-owned local canonical store.
#[derive(Debug, Clone)]
pub struct NoemaStore {
    pub(super) conn: Arc<Mutex<Connection>>,
    pub(super) noema_home: PathBuf,
    pub(super) append_item_lock: Arc<Mutex<()>>,
}

impl NoemaStore {
    /// Open and bootstrap the embedded store.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the database path cannot be prepared or
    /// SQLite cannot be opened or bootstrapped.
    pub async fn open(config: &StoreConfig) -> Result<Self, StoreError> {
        if let Some(parent) = config.path.parent() {
            fs::create_dir_all(parent).map_err(StoreError::PreparePath)?;
        }
        let expected_fingerprint = expected_structural_fingerprint();
        let database_is_empty = inspect_database(&config.path, expected_fingerprint)?;

        let mut conn = Connection::open(&config.path)?;
        conn.pragma_update(None, "foreign_keys", true)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        if database_is_empty {
            bootstrap_schema_v2(&mut conn)?;
        }
        validate_schema_v2(&conn, expected_fingerprint)?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
            noema_home: config.noema_home.clone(),
            append_item_lock: Arc::new(Mutex::new(())),
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

    /// Return the private home directory for one MCP server.
    #[must_use]
    pub(crate) fn mcp_server_home(&self, mcp_server_id: &str) -> PathBuf {
        self.noema_home
            .join("mcp")
            .join(crate::paths::sanitize_path_segment(mcp_server_id))
    }

    /// Return the private home directory for one provider account.
    #[must_use]
    pub(crate) fn provider_account_home(&self, provider_kind: &str, account_key: &str) -> PathBuf {
        self.noema_home
            .join("providers")
            .join(crate::paths::sanitize_path_segment(provider_kind))
            .join(crate::paths::sanitize_path_segment(account_key))
    }

    /// Reconstruct the resolved Noema paths for this store's home directory.
    ///
    /// # Errors
    ///
    /// Returns [`crate::NoemaPathError`] when the store was opened with an
    /// unusable Noema home path.
    pub(crate) fn noema_paths(&self) -> Result<crate::NoemaPaths, crate::NoemaPathError> {
        crate::NoemaPaths::from_noema_home(self.noema_home.clone())
    }

    /// Return a developer diagnostic logger rooted in this store's Noema home.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn system_error_logger(&self) -> crate::SystemErrorLogger {
        crate::SystemErrorLogger::new(self.noema_home.join("errors.log"))
    }

    /// Return the current schema marker version.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the schema marker cannot be queried.
    pub async fn schema_version(&self) -> Result<i64, StoreError> {
        self.with_connection(|conn| {
            conn.query_row(
                "SELECT version FROM schema_state WHERE name = ?1",
                [STORE_SCHEMA_MARKER],
                |row| row.get::<_, i64>(0),
            )
            .map_err(StoreError::Sqlite)
        })
        .await
    }

    /// Close the local store handle.
    ///
    /// # Errors
    ///
    /// This currently has no fallible SQLite shutdown work.
    pub async fn close(self) -> Result<(), StoreError> {
        Ok(())
    }
}

fn inspect_database(
    path: &std::path::Path,
    expected_fingerprint: &str,
) -> Result<bool, StoreError> {
    if !path.exists() || fs::metadata(path).map_err(StoreError::PreparePath)?.len() == 0 {
        return Ok(true);
    }
    with_database_snapshot(path, |snapshot| {
        let count = snapshot.query_row(
            "SELECT COUNT(*) FROM sqlite_schema WHERE name NOT LIKE 'sqlite_%'",
            [],
            |row| row.get::<_, i64>(0),
        )?;
        if count == 0 {
            Ok(true)
        } else {
            snapshot.pragma_update(None, "foreign_keys", true)?;
            validate_schema_v2(snapshot, expected_fingerprint)?;
            Ok(false)
        }
    })
    .map_err(|error| match error {
        StoreError::Sqlite(error) => reset_required(
            format!("malformed or unreadable nonempty database: {error}"),
            expected_fingerprint,
        ),
        other => other,
    })
}
fn validate_schema_v2(conn: &Connection, expected_fingerprint: &str) -> Result<(), StoreError> {
    let marker = conn.query_row(
        "SELECT version, structural_fingerprint FROM schema_state WHERE name = ?1",
        [STORE_SCHEMA_MARKER],
        |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
    );
    let (version, persisted_fingerprint) = marker.map_err(|error| {
        reset_required(
            format!("missing or unreadable v2 marker: {error}"),
            expected_fingerprint,
        )
    })?;
    let actual_fingerprint = structural_fingerprint(conn).map_err(|error| {
        reset_required(
            format!("unreadable structural fingerprint: {error}"),
            expected_fingerprint,
        )
    })?;
    if version != STORE_SCHEMA_VERSION
        || persisted_fingerprint != expected_fingerprint
        || actual_fingerprint != expected_fingerprint
    {
        return Err(reset_required(
            format!(
                "version {version}, marker {persisted_fingerprint}, structure {actual_fingerprint}"
            ),
            expected_fingerprint,
        ));
    }

    let foreign_keys = conn.pragma_query_value(None, "foreign_keys", |row| row.get::<_, i64>(0))?;
    if foreign_keys != 1 {
        return Err(reset_required(
            format!("foreign_keys pragma is {foreign_keys}"),
            expected_fingerprint,
        ));
    }
    let journal_mode_is_wal = conn
        .pragma_query_value(None, "journal_mode", |row| row.get::<_, String>(0))?
        .eq_ignore_ascii_case("wal");
    if !journal_mode_is_wal {
        return Err(reset_required(
            "journal_mode pragma is not WAL".to_string(),
            expected_fingerprint,
        ));
    }
    let foreign_key_problem = conn
        .query_row("PRAGMA foreign_key_check", [], |_| Ok(()))
        .optional()?;
    if foreign_key_problem.is_some() {
        return Err(reset_required(
            "foreign_key_check reported violations".to_string(),
            expected_fingerprint,
        ));
    }
    Ok(())
}

fn with_database_snapshot<T>(
    path: &std::path::Path,
    work: impl FnOnce(&Connection) -> Result<T, StoreError>,
) -> Result<T, StoreError> {
    let snapshot_dir = create_snapshot_dir()?;
    let file_name = path.file_name().ok_or_else(|| {
        StoreError::Schema(format!(
            "database path has no file name: {}",
            path.display()
        ))
    })?;
    let snapshot_path = snapshot_dir.path.join(file_name);
    copy_database_files_stably(path, &snapshot_path)?;
    let conn = Connection::open(snapshot_path)?;
    work(&conn)
}

fn sidecar_path(path: &std::path::Path, suffix: &str) -> PathBuf {
    let mut value = path.as_os_str().to_os_string();
    value.push(suffix);
    PathBuf::from(value)
}

fn copy_database_files_stably(
    source_path: &std::path::Path,
    snapshot_path: &std::path::Path,
) -> Result<(), StoreError> {
    let sources = ["", "-wal"]
        .into_iter()
        .filter_map(|suffix| {
            let path = sidecar_path(source_path, suffix);
            path.exists().then_some((suffix, path))
        })
        .collect::<Vec<_>>();
    let mut copied_digests = Vec::with_capacity(sources.len());
    for (suffix, source) in &sources {
        let destination = sidecar_path(snapshot_path, suffix);
        copied_digests.push(copy_private_with_digest(source, &destination)?);
    }
    let source_set_after = ["", "-wal"]
        .into_iter()
        .filter(|suffix| sidecar_path(source_path, suffix).exists())
        .collect::<Vec<_>>();
    if source_set_after
        != sources
            .iter()
            .map(|(suffix, _)| *suffix)
            .collect::<Vec<_>>()
        || sources
            .iter()
            .zip(copied_digests)
            .any(|((_, source), copied)| match file_digest(source) {
                Ok(actual) => actual != copied,
                Err(_) => true,
            })
    {
        return Err(StoreError::Schema(
            "database changed during non-mutating schema inspection; stop all writers and retry"
                .to_string(),
        ));
    }
    Ok(())
}

fn copy_private_with_digest(
    source: &std::path::Path,
    destination: &std::path::Path,
) -> Result<Vec<u8>, StoreError> {
    use std::io::{Read, Write};

    let mut source = fs::File::open(source).map_err(StoreError::PreparePath)?;
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut destination = options.open(destination).map_err(StoreError::PreparePath)?;
    let mut context = ring::digest::Context::new(&ring::digest::SHA256);
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = source.read(&mut buffer).map_err(StoreError::PreparePath)?;
        if read == 0 {
            break;
        }
        context.update(&buffer[..read]);
        destination
            .write_all(&buffer[..read])
            .map_err(StoreError::PreparePath)?;
    }
    destination.sync_all().map_err(StoreError::PreparePath)?;
    Ok(context.finish().as_ref().to_vec())
}

fn file_digest(path: &std::path::Path) -> Result<Vec<u8>, StoreError> {
    use std::io::Read;

    let mut file = fs::File::open(path).map_err(StoreError::PreparePath)?;
    let mut context = ring::digest::Context::new(&ring::digest::SHA256);
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).map_err(StoreError::PreparePath)?;
        if read == 0 {
            break;
        }
        context.update(&buffer[..read]);
    }
    Ok(context.finish().as_ref().to_vec())
}

fn create_snapshot_dir() -> Result<SnapshotDir, StoreError> {
    for _ in 0..16 {
        let name = super::ids::allocate_id("schema-inspection")?.replace(':', "-");
        let path = std::env::temp_dir().join(name);
        let mut builder = fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        match builder.create(&path) {
            Ok(()) => return Ok(SnapshotDir { path }),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(StoreError::PreparePath(error)),
        }
    }
    Err(StoreError::Schema(
        "could not allocate a private schema inspection directory".to_string(),
    ))
}

struct SnapshotDir {
    path: PathBuf,
}

impl Drop for SnapshotDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[cfg(all(test, unix))]
mod tests {
    use std::os::unix::fs::PermissionsExt;

    use super::*;

    #[test]
    fn schema_inspection_snapshot_uses_private_permissions() {
        let directory = create_snapshot_dir().expect("snapshot directory");
        let file = directory.path.join("store.sqlite");
        let source = directory.path.join("source.sqlite");
        fs::write(&source, b"private").expect("source");
        copy_private_with_digest(&source, &file).expect("snapshot file");

        assert_eq!(
            fs::metadata(&directory.path).unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert_eq!(
            fs::metadata(file).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
}

fn reset_required(found: String, expected_fingerprint: &str) -> StoreError {
    StoreError::ResetRequired {
        found,
        expected: format!("schema v{STORE_SCHEMA_VERSION} fingerprint {expected_fingerprint}"),
    }
}
