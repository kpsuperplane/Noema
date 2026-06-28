use thiserror::Error;

/// Errors produced by the embedded canonical store.
#[derive(Debug, Error)]
pub enum StoreError {
    /// Store path could not be prepared.
    #[error("failed to prepare store path: {0}")]
    PreparePath(std::io::Error),
    /// Embedded SurrealDB operation failed.
    #[error("embedded store operation failed: {0}")]
    Surreal(#[from] surrealdb::Error),
    /// Schema bootstrap returned an invalid result.
    #[error("store schema bootstrap failed: {0}")]
    Schema(String),
}
