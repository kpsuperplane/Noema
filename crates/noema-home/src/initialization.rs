//! Noema home-directory initialization.

use std::{fs, path::PathBuf};

use thiserror::Error;

use crate::NoemaPaths;

/// Options for initializing or updating a Noema home directory.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NoemaHomeInitOptions {
    /// Rewrite `config.yaml` when initial configuration bytes are supplied and
    /// the file already exists.
    pub force: bool,
}

/// Result of preparing a Noema home directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoemaHomeInitResult {
    /// Noema root directory.
    pub root: PathBuf,
    /// Runtime directory under the root.
    pub run_dir: PathBuf,
    /// Config file path under the root.
    pub config_path: PathBuf,
    /// Whether the root directory did not exist before this call.
    pub created_root: bool,
    /// Whether the runtime directory did not exist before this call.
    pub created_run_dir: bool,
    /// Whether this call wrote `config.yaml`.
    pub wrote_config: bool,
}

/// Create the Noema root and runtime directory, optionally writing initial
/// configuration bytes.
///
/// The caller owns configuration semantics. Passing `None` prepares only the
/// filesystem layout; passing bytes writes them when `config.yaml` is absent or
/// [`NoemaHomeInitOptions::force`] is set.
///
/// # Errors
///
/// Returns [`NoemaHomeError`] when required directories cannot be created or
/// supplied configuration bytes cannot be written.
pub fn init_noema_home(
    paths: &NoemaPaths,
    initial_config: Option<&[u8]>,
    options: NoemaHomeInitOptions,
) -> Result<NoemaHomeInitResult, NoemaHomeError> {
    let root = paths.root().to_path_buf();
    let run_dir = paths.run_dir();
    let config_path = paths.config_path();

    let created_root = !root.exists();
    fs::create_dir_all(&root).map_err(|source| NoemaHomeError::CreateDirectory {
        path: root.clone(),
        source,
    })?;

    let created_run_dir = !run_dir.exists();
    fs::create_dir_all(&run_dir).map_err(|source| NoemaHomeError::CreateDirectory {
        path: run_dir.clone(),
        source,
    })?;

    let wrote_config = if let Some(initial_config) = initial_config
        && (options.force || !config_path.exists())
    {
        fs::write(&config_path, initial_config).map_err(|source| NoemaHomeError::WriteConfig {
            path: config_path.clone(),
            source,
        })?;
        true
    } else {
        false
    };

    Ok(NoemaHomeInitResult {
        root,
        run_dir,
        config_path,
        created_root,
        created_run_dir,
        wrote_config,
    })
}

/// Errors produced while preparing the Noema home directory.
#[derive(Debug, Error)]
pub enum NoemaHomeError {
    /// A required directory could not be created.
    #[error("failed to create directory {}: {source}", path.display())]
    CreateDirectory {
        /// Directory that could not be created.
        path: PathBuf,
        /// Underlying filesystem error.
        #[source]
        source: std::io::Error,
    },

    /// Initial configuration bytes could not be written.
    #[error("failed to write config file {}: {source}", path.display())]
    WriteConfig {
        /// Config file path.
        path: PathBuf,
        /// Underlying filesystem error.
        #[source]
        source: std::io::Error,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_CONFIG: &[u8] = b"provider: test\n";

    #[test]
    fn init_creates_home_run_dir_and_config() {
        let dir = tempfile::tempdir().expect("temp dir");
        let root = dir.path().join("noema");
        let paths = NoemaPaths::from_noema_home(&root).expect("paths");

        let result = init_noema_home(&paths, Some(TEST_CONFIG), NoemaHomeInitOptions::default())
            .expect("init");

        assert!(result.created_root);
        assert!(result.created_run_dir);
        assert!(result.wrote_config);
        assert!(root.is_dir());
        assert!(root.join("run").is_dir());
        assert_eq!(
            std::fs::read(root.join("config.yaml")).expect("config"),
            TEST_CONFIG
        );
    }

    #[test]
    fn init_preserves_existing_config_without_force() {
        let dir = tempfile::tempdir().expect("temp dir");
        let paths = NoemaPaths::from_noema_home(dir.path()).expect("paths");
        init_noema_home(&paths, Some(TEST_CONFIG), NoemaHomeInitOptions::default())
            .expect("initial init");
        std::fs::write(paths.config_path(), "provider: custom\n").expect("custom config");

        let result = init_noema_home(&paths, Some(TEST_CONFIG), NoemaHomeInitOptions::default())
            .expect("second init");

        assert!(!result.wrote_config);
        assert_eq!(
            std::fs::read_to_string(paths.config_path()).expect("config"),
            "provider: custom\n"
        );
    }

    #[test]
    fn force_rewrites_existing_config() {
        let dir = tempfile::tempdir().expect("temp dir");
        let paths = NoemaPaths::from_noema_home(dir.path()).expect("paths");
        std::fs::write(paths.config_path(), "provider: custom\n").expect("custom config");

        let result = init_noema_home(
            &paths,
            Some(TEST_CONFIG),
            NoemaHomeInitOptions { force: true },
        )
        .expect("force init");

        assert!(result.wrote_config);
        assert_eq!(
            std::fs::read(paths.config_path()).expect("config"),
            TEST_CONFIG
        );
    }

    #[test]
    fn can_prepare_home_without_writing_config() {
        let dir = tempfile::tempdir().expect("temp dir");
        let paths = NoemaPaths::from_noema_home(dir.path().join("noema")).expect("paths");

        let result = init_noema_home(&paths, None, NoemaHomeInitOptions::default()).expect("init");

        assert!(result.created_run_dir);
        assert!(!result.wrote_config);
        assert!(!paths.config_path().exists());
    }
}
