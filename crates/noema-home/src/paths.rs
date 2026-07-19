//! Filesystem path resolution for Noema state.

use std::{
    env,
    ffi::OsString,
    path::{Path, PathBuf},
};

use thiserror::Error;

use crate::sanitize_path_segment;

/// Environment variable that overrides the Noema home directory.
pub const NOEMA_HOME_ENV: &str = "NOEMA_HOME";
const DEFAULT_NOEMA_DIR: &str = ".noema";

/// Resolved filesystem paths for a Noema home directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoemaPaths {
    root: PathBuf,
}

impl NoemaPaths {
    /// Resolve paths from the current process environment.
    ///
    /// # Errors
    ///
    /// Returns [`NoemaPathError`] when neither `NOEMA_HOME` nor `HOME` can
    /// produce a valid Noema root.
    pub fn from_process_env() -> Result<Self, NoemaPathError> {
        Self::from_env_values(env::var_os(NOEMA_HOME_ENV), env::var_os("HOME"))
    }

    /// Resolve paths from explicit environment values.
    ///
    /// # Errors
    ///
    /// Returns [`NoemaPathError`] when `NOEMA_HOME` is empty or no usable
    /// fallback home directory is available.
    pub fn from_env_values(
        noema_home: Option<OsString>,
        home: Option<OsString>,
    ) -> Result<Self, NoemaPathError> {
        if let Some(noema_home) = noema_home {
            return Self::from_noema_home(noema_home);
        }

        let home = home.ok_or(NoemaPathError::MissingHome)?;
        if home.is_empty() {
            return Err(NoemaPathError::MissingHome);
        }

        Ok(Self {
            root: PathBuf::from(home).join(DEFAULT_NOEMA_DIR),
        })
    }

    /// Resolve paths from an explicit Noema home directory.
    ///
    /// # Errors
    ///
    /// Returns [`NoemaPathError::EmptyNoemaHome`] when the provided path is
    /// empty.
    pub fn from_noema_home(noema_home: impl Into<PathBuf>) -> Result<Self, NoemaPathError> {
        let root = noema_home.into();
        if root.as_os_str().is_empty() {
            return Err(NoemaPathError::EmptyNoemaHome);
        }

        Ok(Self { root })
    }

    /// Root directory for Noema state.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Path to `config.yaml`.
    #[must_use]
    pub fn config_path(&self) -> PathBuf {
        self.root.join("config.yaml")
    }

    /// Path to the developer diagnostic system error log.
    #[must_use]
    pub fn errors_log_path(&self) -> PathBuf {
        self.root.join("errors.log")
    }

    /// Path to the canonical SQLite database file.
    #[must_use]
    pub fn sqlite_db_path(&self) -> PathBuf {
        self.root.join("db/noema.sqlite3")
    }

    /// Root directory for downloaded local-model state.
    #[must_use]
    pub fn local_models_dir(&self) -> PathBuf {
        self.root.join("models")
    }

    /// Content-addressed GGUF blob directory.
    #[must_use]
    pub fn local_model_blobs_dir(&self) -> PathBuf {
        self.local_models_dir().join("blobs")
    }

    /// Directory for resumable, incomplete model downloads.
    #[must_use]
    pub fn local_model_downloads_dir(&self) -> PathBuf {
        self.local_models_dir().join("downloads")
    }

    /// Path to one verified content-addressed GGUF blob.
    ///
    /// # Errors
    ///
    /// Returns [`NoemaPathError::InvalidModelDigest`] unless `sha256` is a
    /// lowercase 64-character hexadecimal digest.
    pub fn local_model_blob_path(&self, sha256: &str) -> Result<PathBuf, NoemaPathError> {
        validate_model_digest(sha256)?;
        Ok(self.local_model_blobs_dir().join(format!("{sha256}.gguf")))
    }

    /// Path to one resumable model download.
    ///
    /// # Errors
    ///
    /// Returns [`NoemaPathError::InvalidModelDigest`] unless `sha256` is a
    /// lowercase 64-character hexadecimal digest.
    pub fn local_model_partial_path(&self, sha256: &str) -> Result<PathBuf, NoemaPathError> {
        validate_model_digest(sha256)?;
        Ok(self
            .local_model_downloads_dir()
            .join(format!("{sha256}.part")))
    }

    /// Path to an advanced import whose digest is not known yet.
    #[must_use]
    pub fn local_model_import_partial_path(&self, installation_id: &str) -> PathBuf {
        self.local_model_downloads_dir().join(format!(
            "import-{}.part",
            sanitize_path_segment(installation_id)
        ))
    }

    /// Path to the MCP server configuration root.
    #[must_use]
    pub fn mcp_dir(&self) -> PathBuf {
        self.root.join("mcp")
    }

    /// Path to one MCP server's private configuration home.
    #[must_use]
    pub fn mcp_server_home(&self, mcp_server_id: &str) -> PathBuf {
        self.mcp_dir().join(sanitize_path_segment(mcp_server_id))
    }

    /// Path to one provider account's credential home.
    #[must_use]
    pub fn provider_account_home(&self, provider_kind: &str, account_key: &str) -> PathBuf {
        self.root
            .join("providers")
            .join(sanitize_path_segment(provider_kind))
            .join(sanitize_path_segment(account_key))
    }
}

/// Errors produced while resolving Noema paths.
#[derive(Debug, Error)]
pub enum NoemaPathError {
    /// Neither `NOEMA_HOME` nor `HOME` could determine a root directory.
    #[error("could not determine Noema directory; set NOEMA_HOME or HOME")]
    MissingHome,

    /// The `NOEMA_HOME` value was present but empty.
    #[error("{NOEMA_HOME_ENV} cannot be empty")]
    EmptyNoemaHome,

    /// A content-addressed model digest was malformed.
    #[error("model SHA-256 digest must be 64 lowercase hexadecimal characters: {value}")]
    InvalidModelDigest {
        /// Rejected digest value.
        value: String,
    },
}

fn validate_model_digest(value: &str) -> Result<(), NoemaPathError> {
    if value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        Ok(())
    } else {
        Err(NoemaPathError::InvalidModelDigest {
            value: value.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    use super::*;

    #[test]
    fn noema_paths_preserve_environment_layout_digest_and_confinement_contracts() {
        let override_paths = NoemaPaths::from_env_values(
            Some(OsString::from("/tmp/custom-noema")),
            Some(OsString::from("/tmp/home")),
        )
        .expect("paths");

        assert_eq!(override_paths.root(), Path::new("/tmp/custom-noema"));
        assert_eq!(
            override_paths.config_path(),
            PathBuf::from("/tmp/custom-noema/config.yaml")
        );
        assert_eq!(
            override_paths.local_model_import_partial_path("install:public/model"),
            PathBuf::from("/tmp/custom-noema/models/downloads/import-install_public_model.part")
        );
        let fallback_paths =
            NoemaPaths::from_env_values(None, Some(OsString::from("/tmp/home"))).expect("paths");
        assert_eq!(fallback_paths.root(), Path::new("/tmp/home/.noema"));
        assert_eq!(
            fallback_paths.config_path(),
            PathBuf::from("/tmp/home/.noema/config.yaml")
        );
        assert!(matches!(
            NoemaPaths::from_env_values(None, None),
            Err(NoemaPathError::MissingHome)
        ));
        assert_eq!(
            override_paths.sqlite_db_path(),
            PathBuf::from("/tmp/custom-noema/db/noema.sqlite3")
        );
        assert_eq!(
            override_paths.errors_log_path(),
            PathBuf::from("/tmp/custom-noema/errors.log")
        );

        let paths = NoemaPaths::from_noema_home("/tmp/noema").expect("paths");
        let digest = "a".repeat(64);

        assert_eq!(
            paths.local_model_blob_path(&digest).expect("blob path"),
            PathBuf::from(format!("/tmp/noema/models/blobs/{digest}.gguf"))
        );
        assert_eq!(
            paths
                .local_model_partial_path(&digest)
                .expect("partial path"),
            PathBuf::from(format!("/tmp/noema/models/downloads/{digest}.part"))
        );
        for digest in ["../model", &"A".repeat(64), &"g".repeat(64)] {
            assert!(paths.local_model_blob_path(digest).is_err(), "{digest}");
        }

        assert_eq!(
            paths.provider_account_home("codex", "default"),
            PathBuf::from("/tmp/noema/providers/codex/default")
        );
        assert_eq!(
            paths.provider_account_home("co/dex", "../default"),
            PathBuf::from("/tmp/noema/providers/co_dex/___default")
        );
        assert_eq!(
            paths.provider_account_home("", ""),
            PathBuf::from("/tmp/noema/providers/_/_")
        );
        assert_eq!(paths.mcp_dir(), PathBuf::from("/tmp/noema/mcp"));
        assert_eq!(
            paths.mcp_server_home("mcp:GitHub/Default"),
            PathBuf::from("/tmp/noema/mcp/mcp_GitHub_Default")
        );
    }
}
