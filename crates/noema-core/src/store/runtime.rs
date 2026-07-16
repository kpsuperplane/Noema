use std::{fs, path::PathBuf, sync::Arc};

use noema_home::{NoemaPathError, NoemaPaths, sanitize_path_segment};
use rusqlite::Connection;
use tokio::sync::Mutex;

use super::{
    error::StoreError,
    schema::{STORE_SCHEMA_SQL, STORE_SCHEMA_VERSION},
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
    pub fn from_paths(paths: &NoemaPaths) -> Self {
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
        let mut conn = Connection::open(&config.path)?;
        conn.pragma_update(None, "foreign_keys", true)?;
        conn.execute_batch(STORE_SCHEMA_SQL)?;
        super::schema_upgrade::upgrade_task_runtime_tables(&mut conn)?;
        // A compatibility rebuild drops the old table-owned indexes. Running
        // the idempotent bootstrap again restores every current index.
        conn.execute_batch(STORE_SCHEMA_SQL)?;
        debug_assert_eq!(STORE_SCHEMA_VERSION, 1);

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
            .join(sanitize_path_segment(mcp_server_id))
    }

    /// Return the private home directory for one provider account.
    #[must_use]
    pub(crate) fn provider_account_home(&self, provider_kind: &str, account_key: &str) -> PathBuf {
        self.noema_home
            .join("providers")
            .join(sanitize_path_segment(provider_kind))
            .join(sanitize_path_segment(account_key))
    }

    /// Reconstruct the resolved Noema paths for this store's home directory.
    ///
    /// # Errors
    ///
    /// Returns [`NoemaPathError`] when the store was opened with an
    /// unusable Noema home path.
    pub(crate) fn noema_paths(&self) -> Result<NoemaPaths, NoemaPathError> {
        NoemaPaths::from_noema_home(self.noema_home.clone())
    }

    /// Return a developer diagnostic logger rooted in this store's Noema home.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn system_error_logger(&self) -> noema_home::SystemErrorLogger {
        noema_home::SystemErrorLogger::new(self.noema_home.join("errors.log"))
    }

    /// Return the current schema marker version.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the schema marker cannot be queried.
    pub async fn schema_version(&self) -> Result<i64, StoreError> {
        self.with_connection(|conn| {
            conn.query_row(
                "SELECT version FROM schema_state WHERE name = 'sqlite_store_v1'",
                [],
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
