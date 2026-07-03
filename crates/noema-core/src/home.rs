//! Noema home-directory initialization.

use crate::paths::NoemaPaths;
use std::{fs, path::PathBuf};
use thiserror::Error;

/// Default config written during Noema home initialization.
pub const DEFAULT_NOEMA_CONFIG_YAML: &str = r"# Noema configuration
provider: codex

codex:
  base_url: https://chatgpt.com/backend-api/codex
  model: gpt-5.5
  # tool_classification_model defaults to gpt-5.4-mini when unset.
  # tool_classification_model: gpt-5.4-mini
  timeout_seconds: 300

# The daemon opens the embedded Noema store under this home directory.

web:
  host: 127.0.0.1
  port: 3737
";

/// Options for initializing or updating a Noema home directory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoemaHomeInitOptions {
    /// Rewrite `config.yaml` even when it already exists.
    pub force: bool,
    /// Write `config.yaml` as part of initialization.
    pub write_config: bool,
}

impl Default for NoemaHomeInitOptions {
    fn default() -> Self {
        Self {
            force: false,
            write_config: true,
        }
    }
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

/// Create the Noema root, runtime directory, and optional default config.
///
/// # Errors
///
/// Returns [`NoemaHomeError`] when required directories cannot be created or
/// the default config cannot be written.
pub fn init_noema_home(
    paths: &NoemaPaths,
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

    let wrote_config = if options.write_config && (options.force || !config_path.exists()) {
        fs::write(&config_path, DEFAULT_NOEMA_CONFIG_YAML).map_err(|source| {
            NoemaHomeError::WriteConfig {
                path: config_path.clone(),
                source,
            }
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
        source: std::io::Error,
    },

    /// The default config file could not be written.
    #[error("failed to write config file {}: {source}", path.display())]
    WriteConfig {
        /// Config file path.
        path: PathBuf,
        /// Underlying filesystem error.
        source: std::io::Error,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_creates_home_run_dir_and_config() {
        let dir = tempfile::tempdir().expect("temp dir");
        let root = dir.path().join("noema");
        let paths = NoemaPaths::from_noema_home(&root).expect("paths");

        let result = init_noema_home(&paths, NoemaHomeInitOptions::default()).expect("init");

        assert!(result.created_root);
        assert!(result.created_run_dir);
        assert!(result.wrote_config);
        assert!(root.is_dir());
        assert!(root.join("run").is_dir());
        assert_eq!(
            std::fs::read_to_string(root.join("config.yaml")).expect("config"),
            DEFAULT_NOEMA_CONFIG_YAML
        );
    }

    #[test]
    fn init_preserves_existing_config_without_force() {
        let dir = tempfile::tempdir().expect("temp dir");
        let paths = NoemaPaths::from_noema_home(dir.path()).expect("paths");
        init_noema_home(&paths, NoemaHomeInitOptions::default()).expect("initial init");
        std::fs::write(paths.config_path(), "provider: openai\n").expect("custom config");

        let result = init_noema_home(&paths, NoemaHomeInitOptions::default()).expect("second init");

        assert!(!result.wrote_config);
        assert_eq!(
            std::fs::read_to_string(paths.config_path()).expect("config"),
            "provider: openai\n"
        );
    }

    #[test]
    fn force_rewrites_existing_config() {
        let dir = tempfile::tempdir().expect("temp dir");
        let paths = NoemaPaths::from_noema_home(dir.path()).expect("paths");
        init_noema_home(&paths, NoemaHomeInitOptions::default()).expect("initial init");
        std::fs::write(paths.config_path(), "provider: openai\n").expect("custom config");

        let result = init_noema_home(
            &paths,
            NoemaHomeInitOptions {
                force: true,
                write_config: true,
            },
        )
        .expect("force init");

        assert!(result.wrote_config);
        assert_eq!(
            std::fs::read_to_string(paths.config_path()).expect("config"),
            DEFAULT_NOEMA_CONFIG_YAML
        );
    }

    #[test]
    fn can_prepare_home_without_writing_config() {
        let dir = tempfile::tempdir().expect("temp dir");
        let paths = NoemaPaths::from_noema_home(dir.path()).expect("paths");

        let result = init_noema_home(
            &paths,
            NoemaHomeInitOptions {
                force: false,
                write_config: false,
            },
        )
        .expect("init");

        assert!(result.created_run_dir);
        assert!(!result.wrote_config);
        assert!(!paths.config_path().exists());
    }
}
