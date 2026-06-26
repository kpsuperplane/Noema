//! Database configuration and Postgres connection helpers.

use sqlx::{PgPool, postgres::PgPoolOptions};
use thiserror::Error;

/// Environment variable that provides the canonical Postgres connection URL.
pub const NOEMA_DATABASE_URL_ENV: &str = "NOEMA_DATABASE_URL";

/// Resolved canonical database configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatabaseConfig {
    /// Postgres connection URL.
    pub url: String,
}

impl DatabaseConfig {
    /// Build a database config from a URL string.
    ///
    /// # Errors
    ///
    /// Returns [`DatabaseConfigError::MissingUrl`] when the URL is empty.
    pub fn new(url: impl Into<String>) -> Result<Self, DatabaseConfigError> {
        let url = url.into();
        if url.trim().is_empty() {
            return Err(DatabaseConfigError::MissingUrl);
        }

        Ok(Self { url })
    }

    /// Connect to Postgres.
    ///
    /// # Errors
    ///
    /// Returns [`DatabaseConfigError::Connect`] when SQLx cannot connect.
    pub async fn connect(&self) -> Result<PgPool, DatabaseConfigError> {
        PgPoolOptions::new()
            .max_connections(10)
            .connect(&self.url)
            .await
            .map_err(DatabaseConfigError::Connect)
    }
}

/// Database configuration and connection errors.
#[derive(Debug, Error)]
pub enum DatabaseConfigError {
    /// Database URL was not configured.
    #[error("{NOEMA_DATABASE_URL_ENV} is required for the Noema server")]
    MissingUrl,
    /// Postgres connection failed.
    #[error("failed to connect to Postgres: {0}")]
    Connect(sqlx::Error),
}
