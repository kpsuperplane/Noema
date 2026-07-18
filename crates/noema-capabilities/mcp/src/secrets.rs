//! Filesystem-backed MCP secret storage.
//!
//! Secret replacement is staged on the same filesystem and committed with a
//! rename. The returned commit retains its backup until the caller finalizes
//! or rolls the replacement back.

mod filesystem;

use std::{
    fmt, io,
    path::PathBuf,
    sync::{Arc, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};

use noema_home::NoemaPaths;
use thiserror::Error;

use crate::McpSecretMaterial;
use filesystem::SecretRoot;

const SECRET_FILE: &str = "secrets.json";
const STAGING_DIR: &str = ".staging";
const BACKUP_PREFIX: &str = ".secrets.json.backup-";
const RESTORE_PREFIX: &str = ".secrets.json.restore-";

/// Secret-store operation that failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum McpSecretStoreOperation {
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

/// A committed replacement whose backup has not yet been finalized.
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
    crate::identity::random_hex_id().map_err(|_| {
        McpSecretStoreError::new(
            operation,
            io::Error::other("secure random generation failed"),
        )
    })
}

#[cfg(all(test, unix))]
mod unix_symlink_tests;

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, fs};

    use super::*;
    use crate::test_fixture::secret_material;

    fn secret_store() -> (tempfile::TempDir, FilesystemMcpSecretStore) {
        let temp = tempfile::tempdir().expect("tempdir");
        let paths = NoemaPaths::from_noema_home(temp.path()).expect("paths");
        (temp, FilesystemMcpSecretStore::new(paths))
    }

    #[test]
    fn filesystem_secret_persistence_privacy_and_removal_contracts() {
        // Case: filesystem_secret_roundtrip_is_private_and_debug_redacted.
        let (temp, store) = secret_store();
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
        let debug = format!("{stage:?} {store:?}");
        let commit = store.commit(stage, "mcp:test").expect("commit");
        let target = NoemaPaths::from_noema_home(temp.path())
            .expect("paths")
            .mcp_server_home("mcp:test")
            .join(SECRET_FILE);
        store.finalize(commit).expect("finalize");

        assert_eq!(store.load("mcp:test").expect("load"), expected);
        let serialized: serde_json::Value =
            serde_json::from_slice(&fs::read(&target).expect("read serialized secret material"))
                .expect("parse serialized secret material");
        assert_eq!(serialized["env"]["GITHUB_TOKEN"], "env-secret");
        assert_eq!(
            serialized["headers"]["Authorization"],
            "Bearer header-secret"
        );
        assert!(serialized.get("safe_config").is_none());
        assert!(serialized.get("url").is_none());
        assert!(serialized.get("command").is_none());
        for secret in ["env-secret", "header-secret", ".staging"] {
            assert!(!debug.contains(secret));
        }
        assert!(debug.contains("[REDACTED]"));

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

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
        // Case: filesystem_secret_removal_is_idempotent_and_removes_the_existing_home.
        let (temp, store) = secret_store();
        let paths = NoemaPaths::from_noema_home(temp.path()).expect("paths");
        let home = paths.mcp_server_home("mcp:test");

        store.remove("mcp:test").expect("missing removal succeeds");
        let stage = store.stage(&secret_material("private")).expect("stage");
        let commit = store.commit(stage, "mcp:test").expect("commit");
        store.finalize(commit).expect("finalize");
        assert!(home.exists());

        store.remove("mcp:test").expect("remove existing home");
        assert!(!home.exists());
        store.remove("mcp:test").expect("repeated removal succeeds");
    }

    #[test]
    fn filesystem_secret_replacement_and_recovery_contracts() {
        // Case: replacement_is_atomic_and_can_be_rolled_back.
        let (_temp, store) = secret_store();
        let first = store.stage(&secret_material("first")).expect("first stage");
        let first = store.commit(first, "mcp:test").expect("first commit");
        store.finalize(first).expect("first finalize");

        let second = store
            .stage(&secret_material("second"))
            .expect("second stage");
        let second = store.commit(second, "mcp:test").expect("second commit");
        assert_eq!(
            store.load("mcp:test").expect("second"),
            secret_material("second")
        );

        store.rollback(second).expect("rollback");
        assert_eq!(
            store.load("mcp:test").expect("first"),
            secret_material("first")
        );
        // Case: cleanup_discards_abandoned_stages_and_recovers_the_last_valid_backup.
        let (temp, store) = secret_store();
        let first = store.stage(&secret_material("first")).expect("first stage");
        let first = store.commit(first, "mcp:test").expect("first commit");
        store.finalize(first).expect("first finalize");
        let abandoned = store
            .stage(&secret_material("abandoned"))
            .expect("abandoned");
        assert!(abandoned.path.exists());

        store.cleanup_abandoned_staging().expect("cleanup");

        assert!(!abandoned.path.exists());
        assert_eq!(
            store.load("mcp:test").expect("active"),
            secret_material("first")
        );

        let second = store
            .stage(&secret_material("second"))
            .expect("second stage");
        let interrupted = store.commit(second, "mcp:test").expect("second commit");
        let target = NoemaPaths::from_noema_home(temp.path())
            .expect("paths")
            .mcp_server_home("mcp:test")
            .join(SECRET_FILE);
        let backup = interrupted.backup.clone().expect("backup");
        fs::remove_file(&target).expect("simulate interrupted replacement");
        assert!(backup.exists());

        store.cleanup_abandoned_staging().expect("recover");

        assert_eq!(
            store.load("mcp:test").expect("restored"),
            secret_material("first")
        );
        assert!(!backup.exists());
    }
}
