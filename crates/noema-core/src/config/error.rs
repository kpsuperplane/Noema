use crate::NoemaPathError;
use std::path::PathBuf;
use thiserror::Error;

/// Errors produced while resolving configuration.
#[derive(Debug, Error)]
pub enum ConfigError {
    /// An explicitly requested config file does not exist.
    #[error("config file not found: {}", path.display())]
    ConfigFileNotFound {
        /// Requested config path.
        path: PathBuf,
    },

    /// A config file exists but does not match the supported schema.
    #[error("failed to parse config file {}: {source}", path.display())]
    ParseConfig {
        /// Config path that failed to parse.
        path: PathBuf,
        /// Parser error.
        source: Box<figment::Error>,
    },

    /// Layered configuration could not be extracted.
    #[error("failed to load configuration: {0}")]
    Load(Box<figment::Error>),

    /// The configured provider is not supported.
    #[error("unsupported provider: {provider}")]
    UnsupportedProvider {
        /// Unsupported provider value.
        provider: String,
    },

    /// Required credentials were missing.
    #[error("missing credentials for {provider}: {credential}")]
    MissingCredential {
        /// Provider that needs the credential.
        provider: String,
        /// Missing credential name.
        credential: String,
    },

    /// An integer setting failed validation.
    #[error("invalid integer value for {name}: {value}")]
    InvalidInteger {
        /// Setting name.
        name: String,
        /// Invalid value.
        value: String,
    },

    /// Path resolution failed.
    #[error(transparent)]
    Path(#[from] NoemaPathError),
}

impl From<figment::Error> for ConfigError {
    fn from(source: figment::Error) -> Self {
        Self::Load(Box::new(source))
    }
}
