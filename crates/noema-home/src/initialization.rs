//! Noema home-directory initialization.

use std::path::PathBuf;

use thiserror::Error;

use crate::{NoemaPaths, atomic_write_private, ensure_private_dir, ensure_private_file};

/// Create the Noema root and runtime directory, optionally writing initial
/// configuration bytes.
///
/// The caller owns configuration semantics. Passing `None` prepares only the
/// filesystem layout; passing bytes writes them when `config.yaml` is absent.
///
/// # Errors
///
/// Returns [`NoemaHomeError`] when required directories cannot be created or
/// supplied configuration bytes cannot be written.
pub fn init_noema_home(
    paths: &NoemaPaths,
    initial_config: Option<&[u8]>,
) -> Result<(), NoemaHomeError> {
    let root = paths.root().to_path_buf();
    let run_dir = root.join("run");
    let config_path = paths.config_path();

    ensure_private_dir(&root).map_err(|source| NoemaHomeError::CreateDirectory {
        path: root.clone(),
        source,
    })?;

    ensure_private_dir(&run_dir).map_err(|source| NoemaHomeError::CreateDirectory {
        path: run_dir.clone(),
        source,
    })?;

    if let Some(initial_config) = initial_config
        && !config_path.exists()
    {
        atomic_write_private(&config_path, initial_config).map_err(|source| {
            NoemaHomeError::WriteConfig {
                path: config_path.clone(),
                source,
            }
        })?;
    } else if config_path.exists() {
        ensure_private_file(&config_path).map_err(|source| NoemaHomeError::WriteConfig {
            path: config_path.clone(),
            source,
        })?;
    }
    Ok(())
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
    fn initialization_creates_layout_and_honors_config_write_policy() {
        let dir = tempfile::tempdir().expect("temp dir");
        let root = dir.path().join("noema");
        let paths = NoemaPaths::from_noema_home(&root).expect("paths");

        init_noema_home(&paths, Some(TEST_CONFIG)).expect("init");

        assert!(root.is_dir());
        assert!(root.join("run").is_dir());
        assert_eq!(
            std::fs::read(root.join("config.yaml")).expect("config"),
            TEST_CONFIG
        );

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            for (path, expected) in [(&root, 0o700), (&root.join("run"), 0o700)] {
                assert_eq!(
                    std::fs::metadata(path)
                        .expect("directory mode")
                        .permissions()
                        .mode()
                        & 0o777,
                    expected
                );
            }
            assert_eq!(
                std::fs::metadata(paths.config_path())
                    .expect("config mode")
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
        std::fs::write(paths.config_path(), "provider: custom\n").expect("custom config");

        init_noema_home(&paths, Some(TEST_CONFIG)).expect("preserving init");

        assert_eq!(
            std::fs::read_to_string(paths.config_path()).expect("config"),
            "provider: custom\n"
        );

        let no_config_paths =
            NoemaPaths::from_noema_home(dir.path().join("no-config")).expect("paths");
        init_noema_home(&no_config_paths, None).expect("init");
        assert!(no_config_paths.root().is_dir());
        assert!(no_config_paths.root().join("run").is_dir());
        assert!(!no_config_paths.config_path().exists());
    }
}
