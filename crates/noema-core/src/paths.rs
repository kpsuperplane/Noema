//! Filesystem path resolution for Noema state.

use std::{
    env,
    ffi::OsString,
    path::{Component, Path, PathBuf},
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

    /// Path to the developer diagnostic system error log.
    #[must_use]
    pub fn errors_log_path(&self) -> PathBuf {
        self.root.join("errors.log")
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

    /// Path to the canonical SQLite database file.
    #[must_use]
    pub fn sqlite_db_path(&self) -> PathBuf {
        self.db_dir().join("noema.sqlite3")
    }

    /// Path to Mnemosyne-owned state.
    #[must_use]
    pub fn mnemosyne_dir(&self) -> PathBuf {
        self.root.join("mnemosyne")
    }

    /// Path to Mnemosyne managed data.
    #[must_use]
    pub fn mnemosyne_data_dir(&self) -> PathBuf {
        self.mnemosyne_dir().join("data")
    }

    /// Path to Mnemosyne runtime state.
    #[must_use]
    pub fn mnemosyne_runtime_dir(&self) -> PathBuf {
        self.mnemosyne_dir().join("run")
    }

    /// Path to the conversation filesystem root.
    #[must_use]
    pub fn conversations_dir(&self) -> PathBuf {
        self.root.join("conversations")
    }

    /// Path to one conversation's durable filesystem directory.
    #[must_use]
    pub fn conversation_dir(&self, conversation_id: &str) -> PathBuf {
        self.conversations_dir()
            .join(sanitize_path_segment(conversation_id))
    }

    /// Path to one conversation's artifact root.
    #[must_use]
    pub fn conversation_artifacts_dir(&self, conversation_id: &str) -> PathBuf {
        self.conversation_dir(conversation_id).join("artifacts")
    }

    /// Path to one conversation artifact version directory.
    #[must_use]
    pub fn conversation_artifact_version_dir(
        &self,
        conversation_id: &str,
        artifact_id: &str,
        version_index: i64,
    ) -> PathBuf {
        self.conversation_artifacts_dir(conversation_id)
            .join(sanitize_path_segment(artifact_id))
            .join("versions")
            .join(version_index.to_string())
    }

    /// Path to the task filesystem root.
    #[must_use]
    pub fn tasks_dir(&self) -> PathBuf {
        self.root.join("tasks")
    }

    /// Path to one task's durable filesystem directory.
    #[must_use]
    pub fn task_dir(&self, task_id: &str) -> PathBuf {
        self.tasks_dir().join(sanitize_path_segment(task_id))
    }

    /// Path to one task artifact version directory.
    #[must_use]
    pub fn task_artifact_version_dir(
        &self,
        task_id: &str,
        artifact_id: &str,
        version_index: i64,
    ) -> PathBuf {
        self.task_dir(task_id)
            .join("artifacts")
            .join(sanitize_path_segment(artifact_id))
            .join("versions")
            .join(version_index.to_string())
    }

    /// Path to the provider credential root.
    #[must_use]
    pub fn providers_dir(&self) -> PathBuf {
        self.root.join("providers")
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
        self.providers_dir()
            .join(sanitize_path_segment(provider_kind))
            .join(sanitize_path_segment(account_key))
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

    /// Artifact filename was empty or unsafe for local artifact storage.
    #[error("artifact filename must be a single safe path segment: {value}")]
    UnsafeArtifactFilename {
        /// Rejected raw filename value.
        value: String,
    },
}

pub(crate) fn sanitize_path_segment(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();
    if sanitized.is_empty() {
        "_".to_string()
    } else {
        sanitized
    }
}

pub(crate) fn safe_artifact_filename(value: &str) -> Result<&str, NoemaPathError> {
    if value.is_empty()
        || value.contains('\\')
        || value.contains('"')
        || value.chars().any(char::is_control)
    {
        return Err(NoemaPathError::UnsafeArtifactFilename {
            value: value.to_string(),
        });
    }

    let mut components = Path::new(value).components();
    match (components.next(), components.next()) {
        (Some(Component::Normal(_)), None) => Ok(value),
        _ => Err(NoemaPathError::UnsafeArtifactFilename {
            value: value.to_string(),
        }),
    }
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
    }

    #[test]
    fn db_dir_is_the_embedded_database_root() {
        let paths = NoemaPaths::from_noema_home("/tmp/custom-noema").expect("paths");

        assert_eq!(paths.db_dir(), PathBuf::from("/tmp/custom-noema/db"));
    }

    #[test]
    fn sqlite_db_path_lives_under_db_dir() {
        let paths = NoemaPaths::from_noema_home("/tmp/noema").expect("paths");

        assert_eq!(
            paths.sqlite_db_path(),
            PathBuf::from("/tmp/noema/db/noema.sqlite3")
        );
    }

    #[test]
    fn errors_log_path_lives_at_noema_root() {
        let paths = NoemaPaths::from_noema_home("/tmp/noema").expect("paths");

        assert_eq!(
            paths.errors_log_path(),
            PathBuf::from("/tmp/noema/errors.log")
        );
    }

    #[test]
    fn conversation_artifact_version_dir_lives_under_conversation_artifacts() {
        let paths = NoemaPaths::from_noema_home("/tmp/noema").expect("paths");

        assert_eq!(
            paths.conversation_artifact_version_dir("conversation:abc", "artifact:def", 2),
            PathBuf::from(
                "/tmp/noema/conversations/conversation_abc/artifacts/artifact_def/versions/2"
            )
        );
    }

    #[test]
    fn task_artifact_version_dir_lives_under_task_artifacts() {
        let paths = NoemaPaths::from_noema_home("/tmp/noema").expect("paths");

        assert_eq!(
            paths.task_artifact_version_dir("task:abc", "artifact:def", 2),
            PathBuf::from("/tmp/noema/tasks/task_abc/artifacts/artifact_def/versions/2")
        );
    }

    #[test]
    fn safe_artifact_filename_rejects_path_traversal() {
        assert!(safe_artifact_filename("report.md").is_ok());
        assert!(safe_artifact_filename("../report.md").is_err());
        assert!(safe_artifact_filename("nested/report.md").is_err());
        assert!(safe_artifact_filename("").is_err());
    }

    #[test]
    fn safe_artifact_filename_rejects_header_unsafe_characters() {
        assert!(safe_artifact_filename("report\".md").is_err());
        assert!(safe_artifact_filename("report\r.md").is_err());
        assert!(safe_artifact_filename("report\n.md").is_err());
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

    #[test]
    fn provider_account_home_is_under_noema_providers() {
        let paths = NoemaPaths::from_noema_home("/tmp/noema").expect("paths");

        assert_eq!(
            paths.provider_account_home("codex", "default"),
            PathBuf::from("/tmp/noema/providers/codex/default")
        );
    }

    #[test]
    fn mcp_server_home_is_under_noema_mcp_dir() {
        let paths = NoemaPaths::from_noema_home("/tmp/noema").expect("paths");

        assert_eq!(paths.mcp_dir(), PathBuf::from("/tmp/noema/mcp"));
        assert_eq!(
            paths.mcp_server_home("mcp:GitHub/Default"),
            PathBuf::from("/tmp/noema/mcp/mcp_GitHub_Default")
        );
    }

    #[test]
    fn provider_account_home_sanitizes_segments() {
        let paths = NoemaPaths::from_noema_home("/tmp/noema").expect("paths");

        assert_eq!(
            paths.provider_account_home("co/dex", "../default"),
            PathBuf::from("/tmp/noema/providers/co_dex/___default")
        );
    }

    #[test]
    fn provider_account_home_does_not_drop_empty_segments() {
        let paths = NoemaPaths::from_noema_home("/tmp/noema").expect("paths");

        assert_eq!(
            paths.provider_account_home("", ""),
            PathBuf::from("/tmp/noema/providers/_/_")
        );
    }
}
