//! Filesystem-backed MCP secret storage.
//!
//! Secret replacement is staged on the same filesystem and committed with a
//! rename. The returned commit can be rolled back until its backup is
//! finalized, which lets the service compensate a later repository failure.

mod filesystem;

use std::{
    fmt, io,
    path::PathBuf,
    sync::{Arc, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};

use noema_home::NoemaPaths;
use ring::rand::{SecureRandom, SystemRandom};
use thiserror::Error;

use crate::McpSecretMaterial;
use filesystem::SecretRoot;

const SECRET_FILE: &str = "secrets.json";
const STAGING_DIR: &str = ".staging";
const BACKUP_PREFIX: &str = ".secrets.json.backup-";
const RESTORE_PREFIX: &str = ".secrets.json.restore-";

/// Secret-store operation that failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpSecretStoreOperation {
    /// Reading existing secret material.
    Read,
    /// Writing a staged replacement.
    Stage,
    /// Committing a staged replacement.
    Commit,
    /// Rolling a committed replacement back.
    Rollback,
    /// Removing secret material.
    Remove,
    /// Cleaning abandoned staging state.
    Cleanup,
}

/// Internal secret-store error. It never contains serialized secret material.
#[derive(Debug, Error)]
#[error("MCP secret store {operation:?} failed: {source}")]
pub struct McpSecretStoreError {
    operation: McpSecretStoreOperation,
    #[source]
    source: io::Error,
}

impl McpSecretStoreError {
    fn new(operation: McpSecretStoreOperation, source: io::Error) -> Self {
        Self { operation, source }
    }

    /// Return the failed operation without exposing backend detail.
    #[must_use]
    pub const fn operation(&self) -> McpSecretStoreOperation {
        self.operation
    }
}

/// A private staged secret file that has not replaced active credentials.
pub struct McpSecretStage {
    root: Arc<SecretRoot>,
    path: PathBuf,
}

impl fmt::Debug for McpSecretStage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("McpSecretStage([REDACTED])")
    }
}

/// A committed replacement that can still be compensated.
pub struct McpSecretCommit {
    root: Arc<SecretRoot>,
    target: PathBuf,
    backup: Option<PathBuf>,
}

impl fmt::Debug for McpSecretCommit {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("McpSecretCommit([REDACTED])")
    }
}

/// Injectable secret-store boundary used by the local MCP service.
pub trait McpSecretStore: Send + Sync + fmt::Debug {
    /// Load one server's current material. A missing file is an empty secret set.
    ///
    /// # Errors
    ///
    /// Returns a redacted storage error when the active file cannot be read.
    fn load(&self, mcp_server_id: &str) -> Result<McpSecretMaterial, McpSecretStoreError>;

    /// Write a private staged replacement without changing active credentials.
    ///
    /// # Errors
    ///
    /// Returns a redacted storage error when private staging cannot complete.
    fn stage(&self, secrets: &McpSecretMaterial) -> Result<McpSecretStage, McpSecretStoreError>;

    /// Atomically replace one server's active secret file.
    ///
    /// # Errors
    ///
    /// Returns a redacted storage error when the replacement cannot commit.
    fn commit(
        &self,
        stage: McpSecretStage,
        mcp_server_id: &str,
    ) -> Result<McpSecretCommit, McpSecretStoreError>;

    /// Restore the previous secret file after a later operation fails.
    ///
    /// # Errors
    ///
    /// Returns a redacted storage error when the previous file cannot be restored.
    fn rollback(&self, commit: McpSecretCommit) -> Result<(), McpSecretStoreError>;

    /// Finalize a successful replacement by removing its backup.
    ///
    /// # Errors
    ///
    /// Returns a redacted storage error when the backup cannot be removed.
    fn finalize(&self, commit: McpSecretCommit) -> Result<(), McpSecretStoreError>;

    /// Discard a staged replacement that was never committed.
    ///
    /// # Errors
    ///
    /// Returns a redacted storage error when the staged file cannot be removed.
    fn discard(&self, stage: McpSecretStage) -> Result<(), McpSecretStoreError>;

    /// Remove one server's secret directory. Missing state is successful.
    ///
    /// # Errors
    ///
    /// Returns a redacted storage error when the directory cannot be removed.
    fn remove(&self, mcp_server_id: &str) -> Result<(), McpSecretStoreError>;

    /// Remove abandoned staged files left by an interrupted process.
    ///
    /// # Errors
    ///
    /// Returns a redacted storage error when abandoned staging cannot be cleaned.
    fn cleanup_abandoned_staging(&self) -> Result<(), McpSecretStoreError>;
}

/// Shared MCP secret-store handle.
pub type McpSecretStoreHandle = Arc<dyn McpSecretStore>;

/// Atomic filesystem implementation rooted in [`NoemaPaths`].
#[derive(Clone)]
pub struct FilesystemMcpSecretStore {
    paths: NoemaPaths,
    root: Arc<OnceLock<Arc<SecretRoot>>>,
}

impl fmt::Debug for FilesystemMcpSecretStore {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FilesystemMcpSecretStore")
            .finish_non_exhaustive()
    }
}

impl FilesystemMcpSecretStore {
    /// Construct a filesystem secret store.
    #[must_use]
    pub fn new(paths: NoemaPaths) -> Self {
        Self {
            paths,
            root: Arc::new(OnceLock::new()),
        }
    }

    fn root(
        &self,
        operation: McpSecretStoreOperation,
    ) -> Result<Arc<SecretRoot>, McpSecretStoreError> {
        if let Some(root) = self.root.get() {
            return Ok(Arc::clone(root));
        }
        let opened = Arc::new(
            SecretRoot::open(self.paths.root())
                .map_err(|source| McpSecretStoreError::new(operation, source))?,
        );
        let _ = self.root.set(opened);
        Ok(Arc::clone(
            self.root
                .get()
                .expect("MCP secret root is initialized before access"),
        ))
    }
}

impl McpSecretStore for FilesystemMcpSecretStore {
    fn load(&self, mcp_server_id: &str) -> Result<McpSecretMaterial, McpSecretStoreError> {
        let operation = McpSecretStoreOperation::Read;
        let root = self.root(operation)?;
        match root.read_secret(mcp_server_id) {
            Ok(Some(bytes)) => serde_json::from_slice(&bytes)
                .map_err(io::Error::other)
                .map_err(|source| McpSecretStoreError::new(operation, source)),
            Ok(None) => Ok(McpSecretMaterial::default()),
            Err(source) => Err(McpSecretStoreError::new(operation, source)),
        }
    }

    fn stage(&self, secrets: &McpSecretMaterial) -> Result<McpSecretStage, McpSecretStoreError> {
        let operation = McpSecretStoreOperation::Stage;
        let root = self.root(operation)?;
        let bytes = serde_json::to_vec_pretty(secrets)
            .map_err(io::Error::other)
            .map_err(|source| McpSecretStoreError::new(operation, source))?;
        for _ in 0..8 {
            let filename = format!("{}.json", random_hex_id(operation)?);
            match root.write_stage(&filename, &bytes) {
                Ok(path) => {
                    return Ok(McpSecretStage {
                        root: Arc::clone(&root),
                        path,
                    });
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(source) => return Err(McpSecretStoreError::new(operation, source)),
            }
        }
        Err(McpSecretStoreError::new(
            operation,
            io::Error::new(
                io::ErrorKind::AlreadyExists,
                "could not allocate a unique MCP secret staging file",
            ),
        ))
    }

    fn commit(
        &self,
        stage: McpSecretStage,
        mcp_server_id: &str,
    ) -> Result<McpSecretCommit, McpSecretStoreError> {
        let operation = McpSecretStoreOperation::Commit;
        if stage.root.path() != self.paths.root() {
            return Err(McpSecretStoreError::new(
                operation,
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "staged MCP secret belongs to another root",
                ),
            ));
        }
        let backup_filename = format!("{BACKUP_PREFIX}{}", random_hex_id(operation)?);
        let (target, backup) = stage
            .root
            .commit_stage(&stage.path, mcp_server_id, &backup_filename)
            .map_err(|source| McpSecretStoreError::new(operation, source))?;
        Ok(McpSecretCommit {
            root: stage.root,
            target,
            backup,
        })
    }

    fn rollback(&self, commit: McpSecretCommit) -> Result<(), McpSecretStoreError> {
        let operation = McpSecretStoreOperation::Rollback;
        commit
            .root
            .rollback(&commit.target, commit.backup.as_deref())
            .map_err(|source| McpSecretStoreError::new(operation, source))
    }

    fn finalize(&self, commit: McpSecretCommit) -> Result<(), McpSecretStoreError> {
        let Some(backup) = commit.backup else {
            return Ok(());
        };
        commit
            .root
            .finalize(&backup)
            .map_err(|source| McpSecretStoreError::new(McpSecretStoreOperation::Cleanup, source))
    }

    fn discard(&self, stage: McpSecretStage) -> Result<(), McpSecretStoreError> {
        stage
            .root
            .discard(&stage.path)
            .map_err(|source| McpSecretStoreError::new(McpSecretStoreOperation::Cleanup, source))
    }

    fn remove(&self, mcp_server_id: &str) -> Result<(), McpSecretStoreError> {
        let operation = McpSecretStoreOperation::Remove;
        self.root(operation)?
            .remove_server(mcp_server_id)
            .map_err(|source| McpSecretStoreError::new(operation, source))
    }

    fn cleanup_abandoned_staging(&self) -> Result<(), McpSecretStoreError> {
        let operation = McpSecretStoreOperation::Cleanup;
        self.root(operation)?
            .cleanup_abandoned_staging()
            .map_err(|source| McpSecretStoreError::new(operation, source))
    }
}

/// Current Unix timestamp used for OAuth token receipt bookkeeping.
#[must_use]
pub(crate) fn now_epoch_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn random_hex_id(operation: McpSecretStoreOperation) -> Result<String, McpSecretStoreError> {
    let mut bytes = [0_u8; 16];
    SystemRandom::new().fill(&mut bytes).map_err(|_| {
        McpSecretStoreError::new(
            operation,
            io::Error::other("secure random generation failed"),
        )
    })?;
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use fmt::Write as _;
        let _ = write!(output, "{byte:02x}");
    }
    Ok(output)
}

#[cfg(all(test, unix))]
mod unix_symlink_tests;

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, fs};

    use super::*;

    fn store() -> (tempfile::TempDir, FilesystemMcpSecretStore) {
        let temp = tempfile::tempdir().expect("tempdir");
        let paths = NoemaPaths::from_noema_home(temp.path()).expect("paths");
        (temp, FilesystemMcpSecretStore::new(paths))
    }

    fn material(value: &str) -> McpSecretMaterial {
        McpSecretMaterial {
            env: BTreeMap::from([("TOKEN".to_string(), value.to_string())]),
            ..McpSecretMaterial::default()
        }
    }

    #[test]
    fn filesystem_secret_file_round_trips_env_and_headers_without_safe_configuration() {
        let (_temp, store) = store();
        let expected = McpSecretMaterial {
            secret_identity_revision: Some("revision:test".to_string()),
            env: BTreeMap::from([("GITHUB_TOKEN".to_string(), "env-secret".to_string())]),
            headers: BTreeMap::from([(
                "Authorization".to_string(),
                "Bearer header-secret".to_string(),
            )]),
            ..McpSecretMaterial::default()
        };

        let stage = store.stage(&expected).expect("stage");
        let commit = store.commit(stage, "mcp:test").expect("commit");
        let target = commit.target.clone();
        store.finalize(commit).expect("finalize");

        assert_eq!(store.load("mcp:test").expect("load"), expected);
        let serialized: serde_json::Value =
            serde_json::from_slice(&fs::read(target).expect("read serialized secret material"))
                .expect("parse serialized secret material");
        assert_eq!(serialized["env"]["GITHUB_TOKEN"], "env-secret");
        assert_eq!(
            serialized["headers"]["Authorization"],
            "Bearer header-secret"
        );
        assert!(serialized.get("safe_config").is_none());
        assert!(serialized.get("url").is_none());
        assert!(serialized.get("command").is_none());
    }

    #[test]
    fn filesystem_secret_removal_is_idempotent_and_removes_the_existing_home() {
        let (temp, store) = store();
        let paths = NoemaPaths::from_noema_home(temp.path()).expect("paths");
        let home = paths.mcp_server_home("mcp:test");

        store.remove("mcp:test").expect("missing removal succeeds");
        let stage = store.stage(&material("private")).expect("stage");
        let commit = store.commit(stage, "mcp:test").expect("commit");
        store.finalize(commit).expect("finalize");
        assert!(home.exists());

        store.remove("mcp:test").expect("remove existing home");
        assert!(!home.exists());
        store.remove("mcp:test").expect("repeated removal succeeds");
    }

    #[test]
    fn replacement_is_atomic_and_can_be_rolled_back() {
        let (_temp, store) = store();
        let first = store.stage(&material("first")).expect("first stage");
        let first = store.commit(first, "mcp:test").expect("first commit");
        store.finalize(first).expect("first finalize");

        let second = store.stage(&material("second")).expect("second stage");
        let second = store.commit(second, "mcp:test").expect("second commit");
        assert_eq!(store.load("mcp:test").expect("second"), material("second"));

        store.rollback(second).expect("rollback");
        assert_eq!(store.load("mcp:test").expect("first"), material("first"));
    }

    #[test]
    fn abandoned_staging_cleanup_does_not_touch_active_secret_files() {
        let (_temp, store) = store();
        let active = store.stage(&material("active")).expect("active stage");
        let active = store.commit(active, "mcp:test").expect("active commit");
        store.finalize(active).expect("active finalize");
        let abandoned = store.stage(&material("abandoned")).expect("abandoned");
        assert!(abandoned.path.exists());

        store.cleanup_abandoned_staging().expect("cleanup");

        assert!(!abandoned.path.exists());
        assert_eq!(store.load("mcp:test").expect("active"), material("active"));
    }

    #[test]
    fn abandoned_backup_restores_the_last_valid_credentials() {
        let (_temp, store) = store();
        let first = store.stage(&material("first")).expect("first stage");
        let first = store.commit(first, "mcp:test").expect("first commit");
        store.finalize(first).expect("first finalize");
        let second = store.stage(&material("second")).expect("second stage");
        let interrupted = store.commit(second, "mcp:test").expect("second commit");
        let target = interrupted.target.clone();
        let backup = interrupted.backup.clone().expect("backup");
        fs::remove_file(&target).expect("simulate interrupted replacement");
        assert!(backup.exists());

        store.cleanup_abandoned_staging().expect("recover");

        assert_eq!(store.load("mcp:test").expect("restored"), material("first"));
        assert!(!backup.exists());
    }

    #[test]
    fn secret_debug_output_redacts_paths_and_material() {
        let (_temp, store) = store();
        let stage = store.stage(&material("private-value")).expect("stage");
        let debug = format!("{stage:?} {store:?}");
        assert!(!debug.contains("private-value"));
        assert!(!debug.contains(".staging"));
        assert!(debug.contains("[REDACTED]"));
    }

    #[cfg(unix)]
    #[test]
    fn committed_secret_directory_and_file_are_private() {
        use std::os::unix::fs::PermissionsExt;

        let (_temp, store) = store();
        let stage = store.stage(&material("private")).expect("stage");
        let commit = store.commit(stage, "mcp:test").expect("commit");
        let target = commit.target.clone();
        store.finalize(commit).expect("finalize");

        assert_eq!(
            fs::metadata(target.parent().expect("parent"))
                .expect("dir metadata")
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
        assert_eq!(
            fs::metadata(target)
                .expect("file metadata")
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }
}
