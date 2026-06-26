//! Filesystem path resolution for Noema state.

use std::{
    env,
    ffi::OsString,
    path::{Path, PathBuf},
};
use thiserror::Error;

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

        Ok(Self::from_home_dir(home))
    }

    /// Resolve paths under a standard `.noema` directory inside `home`.
    #[must_use]
    pub fn from_home_dir(home: impl Into<PathBuf>) -> Self {
        Self {
            root: home.into().join(DEFAULT_NOEMA_DIR),
        }
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

    /// Path to the runtime directory.
    #[must_use]
    pub fn run_dir(&self) -> PathBuf {
        self.root.join("run")
    }

    /// Path to the canonical structured-state directory.
    #[must_use]
    pub fn db_dir(&self) -> PathBuf {
        self.root.join("db")
    }

    /// Path to the local Postgres data directory.
    #[must_use]
    pub fn postgres_data_dir(&self) -> PathBuf {
        self.db_dir().join("postgres")
    }

    /// Path to the daemon socket.
    #[must_use]
    pub fn socket_path(&self) -> PathBuf {
        self.run_dir().join("noema.sock")
    }

    /// Whether the Noema root exists.
    #[must_use]
    pub fn exists(&self) -> bool {
        self.root.exists()
    }

    /// Whether `config.yaml` exists.
    #[must_use]
    pub fn config_exists(&self) -> bool {
        self.config_path().exists()
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;

    #[test]
    fn noema_home_env_is_the_noema_root() {
        let paths = NoemaPaths::from_env_values(
            Some(OsString::from("/tmp/custom-noema")),
            Some(OsString::from("/tmp/home")),
        )
        .expect("paths");

        assert_eq!(paths.root(), Path::new("/tmp/custom-noema"));
        assert_eq!(
            paths.config_path(),
            PathBuf::from("/tmp/custom-noema/config.yaml")
        );
        assert_eq!(
            paths.socket_path(),
            PathBuf::from("/tmp/custom-noema/run/noema.sock")
        );
        assert_eq!(
            paths.postgres_data_dir(),
            PathBuf::from("/tmp/custom-noema/db/postgres")
        );
    }

    #[test]
    fn home_env_falls_back_to_dot_noema() {
        let paths =
            NoemaPaths::from_env_values(None, Some(OsString::from("/tmp/home"))).expect("paths");

        assert_eq!(paths.root(), Path::new("/tmp/home/.noema"));
        assert_eq!(
            paths.config_path(),
            PathBuf::from("/tmp/home/.noema/config.yaml")
        );
    }

    #[test]
    fn missing_home_is_an_error() {
        let error = NoemaPaths::from_env_values(None, None).unwrap_err();

        assert!(matches!(error, NoemaPathError::MissingHome));
    }
}
