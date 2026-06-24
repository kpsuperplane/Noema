use crate::paths::NoemaPaths;
use std::{fs, path::PathBuf};
use thiserror::Error;

pub const DEFAULT_NOEMA_CONFIG_YAML: &str = r#"# Noema configuration
provider: codex

codex:
  command: codex
  sandbox: read-only
  ephemeral: true
  ignore_rules: true
  ignore_user_config: false
  startup_timeout_seconds: 60
  turn_timeout_seconds: 300
"#;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoemaHomeInitOptions {
    pub force: bool,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoemaHomeInitResult {
    pub root: PathBuf,
    pub run_dir: PathBuf,
    pub config_path: PathBuf,
    pub created_root: bool,
    pub created_run_dir: bool,
    pub wrote_config: bool,
}

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

#[derive(Debug, Error)]
pub enum NoemaHomeError {
    #[error("failed to create directory {}: {source}", path.display())]
    CreateDirectory {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("failed to write config file {}: {source}", path.display())]
    WriteConfig {
        path: PathBuf,
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
