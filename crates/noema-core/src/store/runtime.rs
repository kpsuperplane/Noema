use std::{fs, path::PathBuf};

use surrealdb::{
    Surreal,
    engine::local::{Db, RocksDb},
};

use super::{
    error::StoreError,
    schema::{NOEMA_DATABASE, NOEMA_NAMESPACE, STORE_SCHEMA_SQL, STORE_SCHEMA_VERSION},
};

/// Configuration for the embedded Noema store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreConfig {
    /// Directory used by the embedded database.
    pub path: PathBuf,
}

impl StoreConfig {
    /// Build store config from resolved Noema paths.
    #[must_use]
    pub fn from_paths(paths: &crate::NoemaPaths) -> Self {
        Self {
            path: paths.db_dir(),
        }
    }
}

/// Server-owned embedded canonical store.
#[derive(Debug, Clone)]
pub struct NoemaStore {
    db: Surreal<Db>,
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
        Ok(Self { db })
    }

    /// Access the embedded SurrealDB client for repository modules.
    #[must_use]
    #[allow(dead_code)]
    pub(crate) fn db(&self) -> &Surreal<Db> {
        &self.db
    }

    /// Return the current schema marker version.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the schema marker cannot be queried.
    pub async fn schema_version(&self) -> Result<i64, StoreError> {
        #[derive(serde::Deserialize)]
        struct Row {
            version: i64,
        }

        let row: Option<Row> = self.db.select(("schema_state", "current")).await?;
        row.map(|row| row.version)
            .ok_or_else(|| StoreError::Schema("missing schema_state:current".to_string()))
    }
}
