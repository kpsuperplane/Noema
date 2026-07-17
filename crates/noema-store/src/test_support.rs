//! Feature-gated store construction support for consumer tests.

use tempfile::TempDir;

use crate::{NoemaStore, StoreConfig, StoreError};

/// Open an initialized store under a fresh temporary directory.
///
/// The temporary directory is retained for the test process because
/// [`NoemaStore`] intentionally owns only the database connection and pathless
/// repository state.
///
/// # Errors
///
/// Returns [`StoreError`] when SQLite cannot be opened or bootstrapped.
pub async fn open_ephemeral_store() -> Result<NoemaStore, StoreError> {
    let directory = TempDir::new().map_err(StoreError::PreparePath)?;
    let config = StoreConfig::new(directory.path().join("db/noema.sqlite3"));
    let store = NoemaStore::open(&config).await?;
    let _retained_path = directory.keep();
    Ok(store)
}
