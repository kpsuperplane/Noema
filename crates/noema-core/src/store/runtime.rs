use std::{fs, path::PathBuf, sync::Arc};

use surrealdb::{
    Surreal,
    engine::local::{Db, RocksDb},
    types::SurrealValue,
};
use tokio::sync::Mutex;

use super::{
    error::StoreError,
    schema::{NOEMA_DATABASE, NOEMA_NAMESPACE, STORE_SCHEMA_SQL, STORE_SCHEMA_VERSION},
};

/// Configuration for the embedded Noema store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreConfig {
    /// Directory used by the embedded database.
    pub path: PathBuf,
    /// Root directory for Noema state.
    pub noema_home: PathBuf,
}

impl StoreConfig {
    /// Build store config from resolved Noema paths.
    #[must_use]
    pub fn from_paths(paths: &crate::NoemaPaths) -> Self {
        Self {
            path: paths.db_dir(),
            noema_home: paths.root().to_path_buf(),
        }
    }
}

/// Server-owned embedded canonical store.
#[derive(Debug, Clone)]
pub struct NoemaStore {
    pub(super) db: Surreal<Db>,
    pub(super) noema_home: PathBuf,
    pub(super) append_item_lock: Arc<Mutex<()>>,
    pub(super) claim_write_lock: Arc<Mutex<()>>,
}

impl NoemaStore {
    /// Open and bootstrap the embedded store.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the database directory cannot be prepared or
    /// SurrealDB cannot be opened or bootstrapped.
    pub async fn open(config: &StoreConfig) -> Result<Self, StoreError> {
        fs::create_dir_all(&config.path).map_err(StoreError::PreparePath)?;
        let db = Surreal::new::<RocksDb>(config.path.as_path()).await?;
        db.use_ns(NOEMA_NAMESPACE).use_db(NOEMA_DATABASE).await?;
        debug_assert_eq!(STORE_SCHEMA_VERSION, 1);
        db.query(STORE_SCHEMA_SQL).await?.check()?;
        Ok(Self {
            db,
            noema_home: config.noema_home.clone(),
            append_item_lock: Arc::new(Mutex::new(())),
            claim_write_lock: Arc::new(Mutex::new(())),
        })
    }

    /// Access the embedded SurrealDB client for repository modules.
    #[must_use]
    #[allow(dead_code)]
    pub(crate) fn db(&self) -> &Surreal<Db> {
        &self.db
    }

    /// Return the private home directory for one MCP server.
    #[must_use]
    pub(crate) fn mcp_server_home(&self, mcp_server_id: &str) -> PathBuf {
        self.noema_home
            .join("mcp")
            .join(crate::paths::sanitize_path_segment(mcp_server_id))
    }

    /// Return a developer diagnostic logger rooted in this store's Noema home.
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
        #[derive(serde::Deserialize, SurrealValue)]
        struct Row {
            version: i64,
        }

        let row: Option<Row> = self.db.select(("schema_state", "current")).await?;
        row.map(|row| row.version)
            .ok_or_else(|| StoreError::Schema("missing schema_state:current".to_string()))
    }

    /// Close the embedded store handle.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when SurrealDB fails to invalidate the local
    /// connection.
    pub async fn close(self) -> Result<(), StoreError> {
        self.db.invalidate().await?;
        Ok(())
    }
}
