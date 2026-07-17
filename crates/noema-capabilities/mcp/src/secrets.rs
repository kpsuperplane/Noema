//! Filesystem-backed MCP secret storage.
//!
//! Secret replacement is staged on the same filesystem and committed with a
//! rename. The returned commit can be rolled back until its backup is
//! finalized, which lets the service compensate a later repository failure.

use std::{
    fmt, fs, io,
    path::{Path, PathBuf},
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use noema_home::NoemaPaths;
use ring::rand::{SecureRandom, SystemRandom};
use thiserror::Error;

use crate::McpSecretMaterial;

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
    path: PathBuf,
}

impl fmt::Debug for McpSecretStage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("McpSecretStage([REDACTED])")
    }
}

/// A committed replacement that can still be compensated.
pub struct McpSecretCommit {
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
    pub const fn new(paths: NoemaPaths) -> Self {
        Self { paths }
    }

    fn staging_dir(&self) -> PathBuf {
        self.paths.mcp_dir().join(STAGING_DIR)
    }

    fn target(&self, mcp_server_id: &str) -> PathBuf {
        self.paths.mcp_server_home(mcp_server_id).join(SECRET_FILE)
    }
}

impl McpSecretStore for FilesystemMcpSecretStore {
    fn load(&self, mcp_server_id: &str) -> Result<McpSecretMaterial, McpSecretStoreError> {
        let path = self.target(mcp_server_id);
        match fs::read(path) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map_err(io::Error::other)
                .map_err(|source| McpSecretStoreError::new(McpSecretStoreOperation::Read, source)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                Ok(McpSecretMaterial::default())
            }
            Err(source) => Err(McpSecretStoreError::new(
                McpSecretStoreOperation::Read,
                source,
            )),
        }
    }

    fn stage(&self, secrets: &McpSecretMaterial) -> Result<McpSecretStage, McpSecretStoreError> {
        let operation = McpSecretStoreOperation::Stage;
        let staging_dir = self.staging_dir();
        create_private_dir_all(&staging_dir)
            .map_err(|source| McpSecretStoreError::new(operation, source))?;
        let bytes = serde_json::to_vec_pretty(secrets)
            .map_err(io::Error::other)
            .map_err(|source| McpSecretStoreError::new(operation, source))?;
        for _ in 0..8 {
            let path = staging_dir.join(format!("{}.json", random_hex_id(operation)?));
            match write_new_private_file(&path, &bytes) {
                Ok(()) => return Ok(McpSecretStage { path }),
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
        let target = self.target(mcp_server_id);
        let parent = target.parent().ok_or_else(|| {
            McpSecretStoreError::new(
                operation,
                io::Error::new(io::ErrorKind::InvalidInput, "invalid MCP secret target"),
            )
        })?;
        create_private_dir_all(parent)
            .map_err(|source| McpSecretStoreError::new(operation, source))?;
        let backup = if target.exists() {
            let backup = parent.join(format!("{BACKUP_PREFIX}{}", random_hex_id(operation)?));
            copy_private_file(&target, &backup)
                .map_err(|source| McpSecretStoreError::new(operation, source))?;
            Some(backup)
        } else {
            None
        };
        if let Err(source) = replace_file(&stage.path, &target) {
            if !target.exists()
                && let Some(backup) = &backup
                && let Err(recovery) = restore_backup(backup, &target)
            {
                return Err(McpSecretStoreError::new(
                    operation,
                    combined_io_error(source, recovery),
                ));
            }
            return Err(McpSecretStoreError::new(operation, source));
        }
        if let Err(source) = set_private_file_permissions(&target) {
            let recovery = if let Some(backup) = &backup {
                restore_backup(backup, &target)
            } else {
                remove_file_if_present(&target)
            };
            if let Err(recovery) = recovery {
                return Err(McpSecretStoreError::new(
                    operation,
                    combined_io_error(source, recovery),
                ));
            }
            return Err(McpSecretStoreError::new(operation, source));
        }
        Ok(McpSecretCommit { target, backup })
    }

    fn rollback(&self, commit: McpSecretCommit) -> Result<(), McpSecretStoreError> {
        let operation = McpSecretStoreOperation::Rollback;
        if let Some(backup) = commit.backup {
            restore_backup(&backup, &commit.target)
                .map_err(|source| McpSecretStoreError::new(operation, source))?;
            remove_file_if_present(&backup)
                .map_err(|source| McpSecretStoreError::new(operation, source))?;
        } else {
            remove_file_if_present(&commit.target)
                .map_err(|source| McpSecretStoreError::new(operation, source))?;
        }
        Ok(())
    }

    fn finalize(&self, commit: McpSecretCommit) -> Result<(), McpSecretStoreError> {
        let Some(backup) = commit.backup else {
            return Ok(());
        };
        remove_file_if_present(&backup)
            .map_err(|source| McpSecretStoreError::new(McpSecretStoreOperation::Cleanup, source))
    }

    fn discard(&self, stage: McpSecretStage) -> Result<(), McpSecretStoreError> {
        remove_file_if_present(&stage.path)
            .map_err(|source| McpSecretStoreError::new(McpSecretStoreOperation::Cleanup, source))
    }

    fn remove(&self, mcp_server_id: &str) -> Result<(), McpSecretStoreError> {
        match fs::remove_dir_all(self.paths.mcp_server_home(mcp_server_id)) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(source) => Err(McpSecretStoreError::new(
                McpSecretStoreOperation::Remove,
                source,
            )),
        }
    }

    fn cleanup_abandoned_staging(&self) -> Result<(), McpSecretStoreError> {
        recover_abandoned_backups(&self.paths)
            .map_err(|source| McpSecretStoreError::new(McpSecretStoreOperation::Cleanup, source))?;
        let staging_dir = self.staging_dir();
        match fs::remove_dir_all(&staging_dir) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(source) => {
                return Err(McpSecretStoreError::new(
                    McpSecretStoreOperation::Cleanup,
                    source,
                ));
            }
        }
        create_private_dir_all(&staging_dir)
            .map_err(|source| McpSecretStoreError::new(McpSecretStoreOperation::Cleanup, source))
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

fn remove_file_if_present(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

fn copy_private_file(source: &Path, target: &Path) -> io::Result<()> {
    let bytes = fs::read(source)?;
    write_new_private_file(target, &bytes)
}

fn restore_backup(backup: &Path, target: &Path) -> io::Result<()> {
    let parent = target
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "invalid secret target"))?;
    let restore = parent.join(format!("{RESTORE_PREFIX}{}", random_hex_io()?));
    copy_private_file(backup, &restore)?;
    match replace_file(&restore, target) {
        Ok(()) => set_private_file_permissions(target),
        Err(error) => {
            let _ = remove_file_if_present(&restore);
            Err(error)
        }
    }
}

#[cfg(not(windows))]
fn replace_file(source: &Path, target: &Path) -> io::Result<()> {
    fs::rename(source, target)
}

#[cfg(windows)]
fn replace_file(source: &Path, target: &Path) -> io::Result<()> {
    match fs::rename(source, target) {
        Ok(()) => Ok(()),
        Err(error)
            if target.exists()
                && matches!(
                    error.kind(),
                    io::ErrorKind::AlreadyExists | io::ErrorKind::PermissionDenied
                ) =>
        {
            fs::remove_file(target)?;
            fs::rename(source, target)
        }
        Err(error) => Err(error),
    }
}

fn recover_abandoned_backups(paths: &NoemaPaths) -> io::Result<()> {
    let mcp_dir = paths.mcp_dir();
    let entries = match fs::read_dir(&mcp_dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    for entry in entries {
        let entry = entry?;
        if !entry.file_type()?.is_dir() || entry.file_name() == STAGING_DIR {
            continue;
        }
        let server_dir = entry.path();
        let target = server_dir.join(SECRET_FILE);
        let mut backups = fs::read_dir(&server_dir)?
            .filter_map(Result::ok)
            .filter(|candidate| {
                candidate
                    .file_name()
                    .to_str()
                    .is_some_and(|name| name.starts_with(BACKUP_PREFIX))
            })
            .map(|candidate| candidate.path())
            .collect::<Vec<_>>();
        backups.sort_by_key(|path| {
            fs::metadata(path)
                .and_then(|metadata| metadata.modified())
                .unwrap_or(UNIX_EPOCH)
        });
        if !target.exists()
            && let Some(backup) = backups.last()
        {
            restore_backup(backup, &target)?;
        }
        if target.exists() {
            for backup in backups {
                remove_file_if_present(&backup)?;
            }
        }
        for candidate in fs::read_dir(&server_dir)? {
            let candidate = candidate?;
            if candidate
                .file_name()
                .to_str()
                .is_some_and(|name| name.starts_with(RESTORE_PREFIX))
            {
                remove_file_if_present(&candidate.path())?;
            }
        }
    }
    Ok(())
}

fn combined_io_error(primary: io::Error, recovery: io::Error) -> io::Error {
    io::Error::new(
        primary.kind(),
        format!("{primary}; recovery also failed: {recovery}"),
    )
}

fn random_hex_io() -> io::Result<String> {
    let mut bytes = [0_u8; 16];
    SystemRandom::new()
        .fill(&mut bytes)
        .map_err(|_| io::Error::other("secure random generation failed"))?;
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use fmt::Write as _;
        let _ = write!(output, "{byte:02x}");
    }
    Ok(output)
}

#[cfg(unix)]
fn create_private_dir_all(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    fs::create_dir_all(path)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
}

#[cfg(not(unix))]
fn create_private_dir_all(path: &Path) -> io::Result<()> {
    fs::create_dir_all(path)
}

fn write_new_private_file(path: &Path, bytes: &[u8]) -> io::Result<()> {
    use std::io::Write as _;

    let mut options = fs::OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

#[cfg(unix)]
fn set_private_file_permissions(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
}

#[cfg(not(unix))]
fn set_private_file_permissions(_path: &Path) -> io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

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
        remove_file_if_present(&target).expect("simulate interrupted replacement");
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
