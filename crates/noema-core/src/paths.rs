use std::{
    env,
    ffi::OsString,
    path::{Path, PathBuf},
};
use thiserror::Error;

pub const NOEMA_HOME_ENV: &str = "NOEMA_HOME";
const DEFAULT_NOEMA_DIR: &str = ".noema";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoemaPaths {
    root: PathBuf,
}

impl NoemaPaths {
    pub fn from_process_env() -> Result<Self, NoemaPathError> {
        Self::from_env_values(env::var_os(NOEMA_HOME_ENV), env::var_os("HOME"))
    }

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

    pub fn from_home_dir(home: impl Into<PathBuf>) -> Self {
        Self {
            root: home.into().join(DEFAULT_NOEMA_DIR),
        }
    }

    pub fn from_noema_home(noema_home: impl Into<PathBuf>) -> Result<Self, NoemaPathError> {
        let root = noema_home.into();
        if root.as_os_str().is_empty() {
            return Err(NoemaPathError::EmptyNoemaHome);
        }

        Ok(Self { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn config_path(&self) -> PathBuf {
        self.root.join("config.yaml")
    }

    pub fn run_dir(&self) -> PathBuf {
        self.root.join("run")
    }

    pub fn socket_path(&self) -> PathBuf {
        self.run_dir().join("noema.sock")
    }

    pub fn exists(&self) -> bool {
        self.root.exists()
    }

    pub fn config_exists(&self) -> bool {
        self.config_path().exists()
    }
}

#[derive(Debug, Error)]
pub enum NoemaPathError {
    #[error("could not determine Noema directory; set NOEMA_HOME or HOME")]
    MissingHome,

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
