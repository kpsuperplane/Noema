//! Capability-confined local artifact file operations.

use std::{
    io::{Read, Seek, SeekFrom, Write},
    path::{Component, Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};

#[cfg(unix)]
use cap_std::fs::MetadataExt;
use cap_std::fs::{Dir, OpenOptions};

use crate::{
    ArtifactDomainError, ArtifactFileContent, ArtifactMetadataError, ArtifactOperationError,
    ArtifactOwnerRef, ArtifactStorageKind, ArtifactVersionStorage, ReadLocalArtifactRequest,
    artifact_version_dir, safe_artifact_filename,
};

use super::{
    OperationLease,
    cleanup::CreatedDirectoryGuard,
    fs::{
        cleanup_empty_dirs, cleanup_empty_dirs_best_effort, duplicate_dir, ensure_verified_dir,
        filesystem_error, open_verified_cap_dir, read_verified_file, relative_path,
        relative_path_components_are_safe, same_cap_metadata, set_no_follow, sha256_hex,
    },
};

pub(super) use super::cleanup::cleanup_stale_staging;
pub(super) use super::fs::open_root;

const STAGING_DIR: &str = ".artifact-staging";
const OBJECTS_DIR: &str = "objects";
const OPERATION_ID_HEX_CHARS: usize = 64;

pub(super) struct PublishRequest {
    pub(super) root: PathBuf,
    pub(super) root_dir: Dir,
    pub(super) owner: ArtifactOwnerRef,
    pub(super) artifact_id: String,
    pub(super) version_index: i64,
    pub(super) filename: String,
    pub(super) bytes: Vec<u8>,
    pub(super) operation: OperationLease,
    pub(super) active_operations: Arc<Mutex<std::collections::HashSet<String>>>,
    pub(super) stale_staging_age: Duration,
    #[cfg(test)]
    pub(super) publish_hook: Option<Arc<dyn PublishHook>>,
}

#[cfg(test)]
pub(super) trait PublishHook: std::fmt::Debug + Send + Sync {
    fn before_publish(&self) -> Result<(), ArtifactOperationError>;
}

pub(super) fn duplicate_root(root_dir: &Dir, root: &Path) -> Result<Dir, ArtifactOperationError> {
    duplicate_dir(root_dir, root)
}

pub(super) fn operation_id_is_valid(operation_id: &str) -> bool {
    operation_id.len() == 3 + OPERATION_ID_HEX_CHARS
        && operation_id.starts_with("op-")
        && operation_id[3..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

pub(super) fn stage_and_publish(
    request: PublishRequest,
) -> Result<PublishedObject, ArtifactOperationError> {
    let filename = safe_artifact_filename(&request.filename)?.to_string();
    if !operation_id_is_valid(request.operation.id()) {
        return Err(ArtifactDomainError::UnsafeFilename {
            value: request.operation.id().to_string(),
        }
        .into());
    }
    let version_dir = artifact_version_dir(
        &request.root,
        &request.owner,
        &request.artifact_id,
        request.version_index,
    )?;
    let root_dir = request.root_dir;
    cleanup_stale_staging(
        &root_dir,
        &request.root,
        &request.active_operations,
        request.stale_staging_age,
    )?;

    let staging_root = request.root.join(STAGING_DIR);
    let staging_root_dir = ensure_verified_dir(&root_dir, &request.root, &staging_root)?;
    let staging_operation_dir = staging_root.join(request.operation.id());
    let staging_cleanup_parent = duplicate_dir(&staging_root_dir, &staging_root)?;
    staging_root_dir
        .create_dir(request.operation.id())
        .map_err(|_| {
            filesystem_error(
                "create_staging",
                &staging_operation_dir,
                "artifact staging operation directory already exists or is unavailable",
            )
        })?;
    let mut staging_directory_guard =
        CreatedDirectoryGuard::new(staging_cleanup_parent, request.operation.id());
    let staging_dir = open_verified_cap_dir(&root_dir, &request.root, &staging_operation_dir)
        .map_err(|_| {
            filesystem_error(
                "create_staging",
                &staging_operation_dir,
                "artifact staging operation directory could not be verified",
            )
        })?;
    let mut staged = StagedFile::new(
        duplicate_dir(&root_dir, &request.root)?,
        duplicate_dir(&staging_dir, &staging_operation_dir)?,
        filename.clone(),
        relative_path(&request.root, &staging_operation_dir)?,
    );
    staging_directory_guard.disarm();
    let staged_path = staging_operation_dir.join(&filename);
    let (byte_size, content_sha256) = staged.write_and_verify(&staged_path, &request.bytes)?;
    #[cfg(test)]
    if let Some(hook) = &request.publish_hook {
        hook.before_publish()?;
    }

    let objects_root = version_dir.join(OBJECTS_DIR);
    let objects_root_dir = ensure_verified_dir(&root_dir, &request.root, &objects_root)?;
    let object_operation_dir = objects_root.join(request.operation.id());
    let object_cleanup_parent = duplicate_dir(&objects_root_dir, &objects_root)?;
    objects_root_dir
        .create_dir(request.operation.id())
        .map_err(|_| {
            filesystem_error(
                "publish",
                &object_operation_dir,
                "artifact object operation directory already exists or is unavailable",
            )
        })?;
    let mut object_directory_guard =
        CreatedDirectoryGuard::new(object_cleanup_parent, request.operation.id());
    let object_dir = open_verified_cap_dir(&root_dir, &request.root, &object_operation_dir)
        .map_err(|_| {
            filesystem_error(
                "publish",
                &object_operation_dir,
                "artifact object operation directory could not be verified",
            )
        })?;
    let published_path = object_operation_dir.join(&filename);
    let mut published = PublishedObject::new(
        duplicate_dir(&root_dir, &request.root)?,
        object_dir,
        filename.clone(),
        relative_path(&request.root, &object_operation_dir)?,
        published_path,
        request.operation,
        byte_size,
        content_sha256,
    );
    object_directory_guard.disarm();
    hard_link_no_clobber(
        &staging_dir,
        Path::new(&filename),
        &published.object_dir,
        Path::new(&filename),
        published.path(),
    )?;
    published.mark_file_created();
    verify_linked_file(
        &staging_dir,
        &published.object_dir,
        Path::new(&filename),
        &staged_path,
        published.path(),
        byte_size,
        &published.content_sha256,
    )?;
    staged.remove_after_publish()?;
    published.relative_path = relative_path(&request.root, published.path())?
        .to_str()
        .ok_or_else(|| ArtifactDomainError::UnsafeFilename {
            value: published.path().display().to_string(),
        })?
        .to_string();
    Ok(published)
}

pub(super) fn read_local_file(
    root_dir: &Dir,
    root: &Path,
    request: &ReadLocalArtifactRequest,
) -> Result<ArtifactFileContent, ArtifactOperationError> {
    if request.version.artifact_id != request.artifact.artifact_id {
        return Err(ArtifactMetadataError::Invariant {
            message: "artifact version belongs to a different artifact".to_string(),
        }
        .into());
    }
    if request.artifact.storage_kind != ArtifactStorageKind::LocalFile {
        return Err(ArtifactDomainError::StorageKindMismatch.into());
    }
    let ArtifactVersionStorage::LocalFile { relative_path } = &request.version.storage else {
        return Err(ArtifactDomainError::StorageKindMismatch.into());
    };
    let relative = Path::new(relative_path);
    if !relative_path_components_are_safe(relative) {
        return Err(ArtifactDomainError::UnsafeFilename {
            value: relative_path.clone(),
        }
        .into());
    }
    let expected_version_dir = artifact_version_dir(
        root,
        &request.artifact.owner,
        &request.artifact.artifact_id,
        request.version.version_index,
    )?;
    let absolute = root.join(relative);
    let tail = absolute.strip_prefix(&expected_version_dir).map_err(|_| {
        ArtifactDomainError::UnsafeFilename {
            value: relative_path.clone(),
        }
    })?;
    let components = tail.components().collect::<Vec<_>>();
    let [
        Component::Normal(objects),
        Component::Normal(operation_id),
        Component::Normal(filename),
    ] = components.as_slice()
    else {
        return Err(ArtifactDomainError::UnsafeFilename {
            value: relative_path.clone(),
        }
        .into());
    };
    if *objects != OBJECTS_DIR {
        return Err(ArtifactDomainError::UnsafeFilename {
            value: relative_path.clone(),
        }
        .into());
    }
    let operation_id = operation_id
        .to_str()
        .filter(|value| operation_id_is_valid(value))
        .ok_or_else(|| ArtifactDomainError::UnsafeFilename {
            value: relative_path.clone(),
        })?;
    let filename = filename
        .to_str()
        .ok_or_else(|| ArtifactDomainError::UnsafeFilename {
            value: relative_path.clone(),
        })?;
    let filename = safe_artifact_filename(filename)?.to_string();
    let exact = expected_version_dir
        .join(OBJECTS_DIR)
        .join(operation_id)
        .join(&filename);
    if absolute != exact {
        return Err(ArtifactDomainError::UnsafeFilename {
            value: relative_path.clone(),
        }
        .into());
    }

    let parent = exact
        .parent()
        .ok_or_else(|| ArtifactDomainError::UnsafeFilename {
            value: relative_path.clone(),
        })?;
    let parent_dir = open_verified_cap_dir(root_dir, root, parent).map_err(|_| {
        filesystem_error("read", parent, "artifact object directory is unavailable")
    })?;
    let (bytes, metadata) = read_verified_file(&parent_dir, Path::new(&filename), &exact)?;
    let expected_size =
        request
            .version
            .byte_size
            .ok_or_else(|| ArtifactMetadataError::Invariant {
                message: "local artifact version has no byte size".to_string(),
            })?;
    let expected_sha256 = request.version.content_sha256.as_deref().ok_or_else(|| {
        ArtifactMetadataError::Invariant {
            message: "local artifact version has no content digest".to_string(),
        }
    })?;
    if expected_size != metadata.len().try_into().unwrap_or(i64::MAX)
        || expected_sha256 != sha256_hex(&bytes)
    {
        return Err(ArtifactOperationError::Integrity { path: exact });
    }
    Ok(ArtifactFileContent { filename, bytes })
}

pub(super) fn hard_link_no_clobber(
    staging_dir: &Dir,
    staging_name: &Path,
    object_dir: &Dir,
    object_name: &Path,
    published_path: &Path,
) -> Result<(), ArtifactOperationError> {
    staging_dir
        .hard_link(staging_name, object_dir, object_name)
        .map_err(|_| {
            filesystem_error(
                "publish",
                published_path,
                "artifact destination already exists or could not be linked",
            )
        })
}

fn verify_linked_file(
    staging_dir: &Dir,
    object_dir: &Dir,
    filename: &Path,
    staged_path: &Path,
    published_path: &Path,
    byte_size: i64,
    content_sha256: &str,
) -> Result<(), ArtifactOperationError> {
    let (staged_bytes, staged_metadata) = read_verified_file(staging_dir, filename, staged_path)?;
    let (published_bytes, published_metadata) =
        read_verified_file(object_dir, filename, published_path)?;
    let expected_size =
        u64::try_from(byte_size).map_err(|_| ArtifactOperationError::Integrity {
            path: published_path.to_path_buf(),
        })?;
    if !same_cap_metadata(&staged_metadata, &published_metadata)
        || staged_metadata.len() != expected_size
        || published_metadata.len() != expected_size
        || staged_bytes != published_bytes
        || sha256_hex(&published_bytes) != content_sha256
    {
        return Err(ArtifactOperationError::Integrity {
            path: published_path.to_path_buf(),
        });
    }
    #[cfg(unix)]
    if staged_metadata.nlink() < 2 || published_metadata.nlink() < 2 {
        return Err(ArtifactOperationError::Integrity {
            path: published_path.to_path_buf(),
        });
    }
    Ok(())
}

struct StagedFile {
    root_dir: Dir,
    staging_dir: Dir,
    filename: String,
    relative_operation_dir: PathBuf,
    file_created: bool,
}

impl StagedFile {
    fn new(
        root_dir: Dir,
        staging_dir: Dir,
        filename: String,
        relative_operation_dir: PathBuf,
    ) -> Self {
        Self {
            root_dir,
            staging_dir,
            filename,
            relative_operation_dir,
            file_created: false,
        }
    }

    fn write_and_verify(
        &mut self,
        path: &Path,
        bytes: &[u8],
    ) -> Result<(i64, String), ArtifactOperationError> {
        let mut options = OpenOptions::new();
        options.write(true).read(true).create_new(true);
        set_no_follow(&mut options);
        let mut file = self
            .staging_dir
            .open_with(Path::new(&self.filename), &options)
            .map_err(|_| {
                filesystem_error(
                    "write_staging",
                    path,
                    "staged artifact file already exists or could not be created",
                )
            })?;
        self.file_created = true;
        file.write_all(bytes).map_err(|_| {
            filesystem_error(
                "write_staging",
                path,
                "staged artifact bytes could not be written",
            )
        })?;
        file.flush().map_err(|_| {
            filesystem_error(
                "write_staging",
                path,
                "staged artifact bytes could not be flushed",
            )
        })?;
        file.sync_all().map_err(|_| {
            filesystem_error(
                "write_staging",
                path,
                "staged artifact bytes could not be synced",
            )
        })?;
        let metadata = file.metadata().map_err(|_| {
            filesystem_error(
                "verify_staging",
                path,
                "staged artifact metadata is unavailable",
            )
        })?;
        let byte_size =
            i64::try_from(bytes.len()).map_err(|_| ArtifactOperationError::Integrity {
                path: path.to_path_buf(),
            })?;
        if metadata.len() != u64::try_from(byte_size).unwrap_or(u64::MAX) {
            return Err(ArtifactOperationError::Integrity {
                path: path.to_path_buf(),
            });
        }
        file.seek(SeekFrom::Start(0)).map_err(|_| {
            filesystem_error(
                "verify_staging",
                path,
                "staged artifact could not be rewound",
            )
        })?;
        let mut verified = Vec::with_capacity(bytes.len());
        file.read_to_end(&mut verified).map_err(|_| {
            filesystem_error(
                "verify_staging",
                path,
                "staged artifact could not be verified",
            )
        })?;
        if verified != bytes {
            return Err(ArtifactOperationError::Integrity {
                path: path.to_path_buf(),
            });
        }
        Ok((byte_size, sha256_hex(&verified)))
    }

    fn remove_after_publish(&mut self) -> Result<(), ArtifactOperationError> {
        self.staging_dir
            .remove_file(Path::new(&self.filename))
            .map_err(|_| {
                filesystem_error(
                    "unlink_staging",
                    &self.relative_operation_dir.join(&self.filename),
                    "staged artifact link could not be removed",
                )
            })?;
        self.file_created = false;
        cleanup_empty_dirs(&self.root_dir, [&self.relative_operation_dir])?;
        Ok(())
    }

    fn cleanup(&mut self) {
        if self.file_created {
            let _ = self.staging_dir.remove_file(Path::new(&self.filename));
            self.file_created = false;
        }
        cleanup_empty_dirs_best_effort(&self.root_dir, [&self.relative_operation_dir]);
    }
}

impl Drop for StagedFile {
    fn drop(&mut self) {
        self.cleanup();
    }
}

pub(super) struct PublishedObject {
    root_dir: Dir,
    object_dir: Dir,
    filename: String,
    relative_operation_dir: PathBuf,
    published_path: PathBuf,
    relative_path: String,
    operation: Option<OperationLease>,
    byte_size: i64,
    content_sha256: String,
    file_created: bool,
}

impl PublishedObject {
    #[allow(clippy::too_many_arguments)]
    fn new(
        root_dir: Dir,
        object_dir: Dir,
        filename: String,
        relative_operation_dir: PathBuf,
        published_path: PathBuf,
        operation: OperationLease,
        byte_size: i64,
        content_sha256: String,
    ) -> Self {
        Self {
            root_dir,
            object_dir,
            filename,
            relative_operation_dir,
            published_path,
            relative_path: String::new(),
            operation: Some(operation),
            byte_size,
            content_sha256,
            file_created: false,
        }
    }

    pub(super) fn path(&self) -> &Path {
        &self.published_path
    }

    pub(super) fn relative_path(&self) -> &str {
        &self.relative_path
    }

    pub(super) const fn byte_size(&self) -> i64 {
        self.byte_size
    }

    pub(super) fn content_sha256(&self) -> &str {
        &self.content_sha256
    }

    fn mark_file_created(&mut self) {
        self.file_created = true;
    }

    pub(super) fn disarm(&mut self) {
        self.operation.take();
    }

    pub(super) fn metadata_failed(
        mut self,
        metadata_error: ArtifactMetadataError,
    ) -> ArtifactOperationError {
        match self.cleanup_reported() {
            Ok(()) => ArtifactOperationError::Metadata(metadata_error),
            Err(()) => ArtifactOperationError::MetadataRollback {
                path: self.published_path.clone(),
                metadata_error,
                cleanup_message: "private published artifact could not be removed".to_string(),
            },
        }
    }

    fn cleanup_reported(&mut self) -> Result<(), ()> {
        if self.file_created {
            self.object_dir
                .remove_file(Path::new(&self.filename))
                .map_err(|_| ())?;
            self.file_created = false;
        }
        cleanup_empty_dirs(&self.root_dir, [&self.relative_operation_dir]).map_err(|_| ())?;
        self.operation.take();
        Ok(())
    }

    fn cleanup_best_effort(&mut self) {
        if self.file_created {
            let _ = self.object_dir.remove_file(Path::new(&self.filename));
            self.file_created = false;
        }
        cleanup_empty_dirs_best_effort(&self.root_dir, [&self.relative_operation_dir]);
        self.operation.take();
    }
}

impl Drop for PublishedObject {
    fn drop(&mut self) {
        if self.operation.is_some() {
            self.cleanup_best_effort();
        }
    }
}
