use std::{
    io::{Read, Write},
    path::{Component, Path, PathBuf},
};

#[cfg(unix)]
use cap_std::fs::{MetadataExt as CapMetadataExt, OpenOptionsExt};
use cap_std::{
    ambient_authority,
    fs::{Dir, Metadata, OpenOptions},
};
use noema_artifacts::{
    ArtifactDomainError, ArtifactOwnerRef, artifact_version_dir, safe_artifact_filename,
};
use noema_home::NoemaPaths;
use thiserror::Error;

mod task_local;
pub use task_local::{
    NewTaskLocalFileArtifact, NewTaskLocalFileArtifactVersion,
    append_task_local_file_artifact_version, create_task_local_file_artifact,
};

/// Input for creating a conversation-owned local file artifact and first version.
#[derive(Debug, Clone, PartialEq)]
pub struct NewConversationLocalFileArtifact {
    /// Owning conversation id for the artifact.
    pub conversation_id: String,
    /// Human-readable artifact title.
    pub title: String,
    /// Optional artifact description.
    pub description: Option<String>,
    /// Product-defined artifact kind label.
    pub artifact_kind: String,
    /// Safe single-segment filename for the first local file version.
    pub filename: String,
    /// Artifact bytes to persist locally.
    pub bytes: Vec<u8>,
    /// Optional media type for the local payload.
    pub media_type: Option<String>,
    /// Actor responsible for the artifact and first version.
    pub created_by_actor_id: String,
    /// Optional transcript provenance for the artifact and version.
    pub source: noema_artifacts::ArtifactSource,
    /// Arbitrary artifact metadata stored with the canonical artifact row.
    pub metadata: serde_json::Value,
}

/// Input for appending a local file version to a conversation-owned artifact.
#[derive(Debug, Clone, PartialEq)]
pub struct NewConversationLocalFileArtifactVersion {
    /// Existing artifact id to append to.
    pub artifact_id: String,
    /// Optional version-specific title.
    pub title: Option<String>,
    /// Safe single-segment filename for this local file version.
    pub filename: String,
    /// Artifact bytes to persist locally.
    pub bytes: Vec<u8>,
    /// Optional media type for the local payload.
    pub media_type: Option<String>,
    /// Actor responsible for the appended version.
    pub created_by_actor_id: String,
    /// Optional transcript provenance for the version.
    pub source: noema_artifacts::ArtifactSource,
    /// Arbitrary version metadata stored with the immutable version row.
    pub metadata: serde_json::Value,
}

/// Errors produced while writing local artifact bytes and metadata.
#[derive(Debug, Error)]
pub enum ArtifactWriteError {
    /// The artifact filename or derived path was unsafe.
    #[error(transparent)]
    Path(#[from] ArtifactDomainError),

    /// The local artifact version directory could not be created.
    #[error("failed to create artifact directory {}: {source}", path.display())]
    CreateDirectory {
        /// Directory that failed to be created.
        path: PathBuf,
        /// Underlying filesystem error.
        source: std::io::Error,
    },

    /// The local artifact bytes could not be written.
    #[error("failed to write artifact file {}: {source}", path.display())]
    WriteFile {
        /// File path that failed to write.
        path: PathBuf,
        /// Underlying filesystem error.
        source: std::io::Error,
    },

    /// Artifact metadata persistence failed and the rollback file delete also failed.
    #[error(
        "failed to persist artifact metadata after writing {}: {store_error}; cleanup failed: {cleanup_error}",
        path.display()
    )]
    MetadataWriteRollback {
        /// File path whose metadata write and cleanup both failed.
        path: PathBuf,
        /// Canonical store error from the metadata write.
        store_error: crate::StoreError,
        /// Underlying filesystem error from rollback cleanup.
        cleanup_error: std::io::Error,
    },

    /// Artifact metadata persistence failed after bytes were written.
    #[error(transparent)]
    Store(#[from] crate::StoreError),
}

/// Create a conversation-owned local file artifact, writing bytes first and metadata second.
///
/// # Errors
///
/// Returns [`ArtifactWriteError`] when the filename is unsafe, local file I/O
/// fails, or the canonical artifact metadata write fails.
pub async fn create_conversation_local_file_artifact(
    store: &crate::NoemaStore,
    paths: &NoemaPaths,
    input: NewConversationLocalFileArtifact,
) -> Result<noema_artifacts::ArtifactWithVersions, ArtifactWriteError> {
    let artifact_id = store.new_artifact_id();
    let artifact_version_id = store.new_artifact_version_id();
    let filename = safe_artifact_filename(&input.filename)?;
    let version_dir = artifact_version_dir(
        paths.root(),
        &ArtifactOwnerRef::conversation(&input.conversation_id),
        &artifact_id,
        1,
    )?;
    let artifact_path = version_dir.join(filename);
    let relative_path = artifact_relative_path(paths.root(), &artifact_path)?;
    write_local_artifact_bytes(paths, &version_dir, &artifact_path, &input.bytes)?;

    let content_sha256 = sha256_hex(&input.bytes);
    let create_result = store
        .create_artifact_with_initial_version(
            noema_artifacts::NewArtifact {
                artifact_id: Some(artifact_id),
                owner: noema_artifacts::ArtifactOwnerRef::conversation(&input.conversation_id),
                title: input.title,
                description: input.description,
                artifact_kind: input.artifact_kind,
                storage_kind: noema_artifacts::ArtifactStorageKind::LocalFile,
                created_by_actor_id: input.created_by_actor_id.clone(),
                source: input.source.clone(),
                metadata: input.metadata,
            },
            noema_artifacts::NewArtifactVersion {
                artifact_version_id: Some(artifact_version_id),
                title: None,
                storage: noema_artifacts::ArtifactVersionStorage::LocalFile { relative_path },
                media_type: input.media_type,
                byte_size: Some(input.bytes.len() as i64),
                content_sha256: Some(content_sha256),
                created_by_actor_id: input.created_by_actor_id,
                source: input.source,
                metadata: serde_json::json!({}),
            },
        )
        .await;

    match create_result {
        Ok(artifact) => Ok(artifact),
        Err(store_error) => match tokio::fs::remove_file(&artifact_path).await {
            Ok(()) => Err(ArtifactWriteError::Store(store_error)),
            Err(cleanup_error) => Err(ArtifactWriteError::MetadataWriteRollback {
                path: artifact_path,
                store_error,
                cleanup_error,
            }),
        },
    }
}

/// Append a new local file version to an existing conversation-owned artifact.
///
/// # Errors
///
/// Returns [`ArtifactWriteError`] when the artifact is missing, not a
/// conversation-owned local file artifact, the filename is unsafe, local file
/// I/O fails, or the canonical version metadata write fails.
pub async fn append_conversation_local_file_artifact_version(
    store: &crate::NoemaStore,
    paths: &NoemaPaths,
    input: NewConversationLocalFileArtifactVersion,
) -> Result<noema_artifacts::ArtifactVersionRecord, ArtifactWriteError> {
    let artifact = store
        .get_artifact(&input.artifact_id)
        .await?
        .ok_or_else(|| crate::StoreError::ArtifactNotFound {
            artifact_id: input.artifact_id.clone(),
        })?;
    if artifact.artifact.owner.object_type != "conversation"
        || artifact.artifact.storage_kind != noema_artifacts::ArtifactStorageKind::LocalFile
    {
        return Err(ArtifactWriteError::Store(
            crate::StoreError::ArtifactStorageKindMismatch,
        ));
    }

    let next_version_index = artifact
        .versions
        .last()
        .map_or(1, |version| version.version_index + 1);
    let filename = safe_artifact_filename(&input.filename)?;
    let version_dir = artifact_version_dir(
        paths.root(),
        &artifact.artifact.owner,
        &artifact.artifact.artifact_id,
        next_version_index,
    )?;
    let artifact_path = version_dir.join(filename);
    let relative_path = artifact_relative_path(paths.root(), &artifact_path)?;
    write_local_artifact_bytes(paths, &version_dir, &artifact_path, &input.bytes)?;

    let content_sha256 = sha256_hex(&input.bytes);
    let append_result = store
        .append_artifact_version(
            &artifact.artifact.artifact_id,
            noema_artifacts::NewArtifactVersion {
                artifact_version_id: None,
                title: input.title,
                storage: noema_artifacts::ArtifactVersionStorage::LocalFile { relative_path },
                media_type: input.media_type,
                byte_size: Some(input.bytes.len() as i64),
                content_sha256: Some(content_sha256),
                created_by_actor_id: input.created_by_actor_id,
                source: input.source,
                metadata: input.metadata,
            },
        )
        .await;

    match append_result {
        Ok(version) => Ok(version),
        Err(store_error) => match tokio::fs::remove_file(&artifact_path).await {
            Ok(()) => Err(ArtifactWriteError::Store(store_error)),
            Err(cleanup_error) => Err(ArtifactWriteError::MetadataWriteRollback {
                path: artifact_path,
                store_error,
                cleanup_error,
            }),
        },
    }
}

pub(crate) fn local_artifact_absolute_path(
    paths: &NoemaPaths,
    relative_path: &str,
) -> Result<PathBuf, ArtifactDomainError> {
    if !relative_path_components_are_safe(Path::new(relative_path)) {
        return Err(ArtifactDomainError::UnsafeFilename {
            value: relative_path.to_string(),
        });
    }
    Ok(paths.root().join(relative_path))
}

pub(crate) fn validated_local_artifact_absolute_path(
    paths: &NoemaPaths,
    artifact: &noema_artifacts::ArtifactRecord,
    version: &noema_artifacts::ArtifactVersionRecord,
) -> Result<PathBuf, ArtifactDomainError> {
    let noema_artifacts::ArtifactVersionStorage::LocalFile { relative_path } = &version.storage
    else {
        return Err(ArtifactDomainError::UnsafeFilename {
            value: version.artifact_version_id.clone(),
        });
    };

    let absolute_path = local_artifact_absolute_path(paths, relative_path)?;
    let filename = absolute_path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| ArtifactDomainError::UnsafeFilename {
            value: relative_path.clone(),
        })?;
    let filename = safe_artifact_filename(filename)?;
    let expected_dir = artifact_version_dir(
        paths.root(),
        &artifact.owner,
        &artifact.artifact_id,
        version.version_index,
    )?;
    let expected_path = expected_dir.join(filename);
    if absolute_path != expected_path {
        return Err(ArtifactDomainError::UnsafeFilename {
            value: relative_path.clone(),
        });
    }

    Ok(absolute_path)
}

pub(crate) fn read_validated_local_artifact_file(
    paths: &NoemaPaths,
    artifact: &noema_artifacts::ArtifactRecord,
    version: &noema_artifacts::ArtifactVersionRecord,
) -> Result<(PathBuf, Vec<u8>), ArtifactDomainError> {
    let absolute_path = validated_local_artifact_absolute_path(paths, artifact, version)?;
    let parent = absolute_path
        .parent()
        .ok_or_else(|| ArtifactDomainError::UnsafeFilename {
            value: absolute_path.display().to_string(),
        })?;
    let filename =
        absolute_path
            .file_name()
            .ok_or_else(|| ArtifactDomainError::UnsafeFilename {
                value: absolute_path.display().to_string(),
            })?;
    let root_dir =
        open_cap_root(paths.root()).map_err(|_| ArtifactDomainError::UnsafeFilename {
            value: paths.root().display().to_string(),
        })?;
    let parent_dir = open_verified_cap_dir(&root_dir, paths.root(), parent).map_err(|_| {
        ArtifactDomainError::UnsafeFilename {
            value: parent.display().to_string(),
        }
    })?;
    let filename_path = Path::new(filename);
    let before_metadata = parent_dir.symlink_metadata(filename_path).map_err(|_| {
        ArtifactDomainError::UnsafeFilename {
            value: absolute_path.display().to_string(),
        }
    })?;
    if before_metadata.file_type().is_symlink() || !before_metadata.is_file() {
        return Err(ArtifactDomainError::UnsafeFilename {
            value: absolute_path.display().to_string(),
        });
    }

    let mut options = OpenOptions::new();
    options.read(true);
    set_no_follow(&mut options);
    let mut file = parent_dir.open_with(filename_path, &options).map_err(|_| {
        ArtifactDomainError::UnsafeFilename {
            value: absolute_path.display().to_string(),
        }
    })?;
    let file_metadata = file
        .metadata()
        .map_err(|_| ArtifactDomainError::UnsafeFilename {
            value: absolute_path.display().to_string(),
        })?;
    let after_metadata = parent_dir.symlink_metadata(filename_path).map_err(|_| {
        ArtifactDomainError::UnsafeFilename {
            value: absolute_path.display().to_string(),
        }
    })?;
    if !same_cap_metadata(&file_metadata, &after_metadata) {
        return Err(ArtifactDomainError::UnsafeFilename {
            value: absolute_path.display().to_string(),
        });
    }

    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .map_err(|_| ArtifactDomainError::UnsafeFilename {
            value: absolute_path.display().to_string(),
        })?;
    Ok((absolute_path, bytes))
}

fn write_local_artifact_bytes(
    paths: &NoemaPaths,
    version_dir: &Path,
    artifact_path: &Path,
    bytes: &[u8],
) -> Result<(), ArtifactWriteError> {
    let root_dir =
        open_cap_root(paths.root()).map_err(|source| ArtifactWriteError::CreateDirectory {
            path: paths.root().to_path_buf(),
            source,
        })?;
    let relative_version_dir = relative_path_for_cap_operation(paths.root(), version_dir)
        .map_err(ArtifactWriteError::Path)?;
    root_dir
        .create_dir_all(&relative_version_dir)
        .map_err(|source| ArtifactWriteError::CreateDirectory {
            path: version_dir.to_path_buf(),
            source,
        })?;
    let version_dir_handle =
        open_verified_cap_dir(&root_dir, paths.root(), version_dir).map_err(|source| {
            ArtifactWriteError::WriteFile {
                path: version_dir.to_path_buf(),
                source,
            }
        })?;
    let filename = artifact_path.file_name().ok_or_else(|| {
        ArtifactWriteError::Path(ArtifactDomainError::UnsafeFilename {
            value: artifact_path.display().to_string(),
        })
    })?;

    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    set_no_follow(&mut options);
    let mut file = version_dir_handle
        .open_with(Path::new(filename), &options)
        .map_err(|source| ArtifactWriteError::WriteFile {
            path: artifact_path.to_path_buf(),
            source,
        })?;
    file.write_all(bytes)
        .map_err(|source| ArtifactWriteError::WriteFile {
            path: artifact_path.to_path_buf(),
            source,
        })?;
    file.flush()
        .map_err(|source| ArtifactWriteError::WriteFile {
            path: artifact_path.to_path_buf(),
            source,
        })?;
    Ok(())
}

fn artifact_relative_path(
    root: &Path,
    artifact_path: &Path,
) -> Result<String, ArtifactDomainError> {
    let relative =
        artifact_path
            .strip_prefix(root)
            .map_err(|_| ArtifactDomainError::UnsafeFilename {
                value: artifact_path.display().to_string(),
            })?;
    if !relative_path_components_are_safe(relative) {
        return Err(ArtifactDomainError::UnsafeFilename {
            value: relative.display().to_string(),
        });
    }
    Ok(relative.to_string_lossy().into_owned())
}

fn relative_path_components_are_safe(path: &Path) -> bool {
    let mut saw_component = false;
    for component in path.components() {
        match component {
            Component::Normal(_) => saw_component = true,
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return false,
        }
    }
    saw_component
}

fn open_cap_root(root: &Path) -> Result<Dir, std::io::Error> {
    Dir::open_ambient_dir(root, ambient_authority())
}

fn open_verified_cap_dir(
    root_dir: &Dir,
    root: &Path,
    directory: &Path,
) -> Result<Dir, std::io::Error> {
    reject_symlink_path_components(root, directory)?;
    let relative = relative_path_for_cap_operation(root, directory).map_err(|error| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, error.to_string())
    })?;
    let dir = root_dir.open_dir(&relative)?;
    reject_symlink_path_components(root, directory)?;
    let path_metadata = root_dir.symlink_metadata(&relative)?;
    if !path_metadata.is_dir() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "artifact directory path is not a directory",
        ));
    }
    let dir_metadata = dir.dir_metadata()?;
    if !same_cap_metadata(&dir_metadata, &path_metadata) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "artifact directory changed while opening",
        ));
    }
    Ok(dir)
}

fn reject_symlink_path_components(root: &Path, path: &Path) -> Result<(), std::io::Error> {
    let relative = path.strip_prefix(root).map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "artifact path is outside Noema root",
        )
    })?;
    let mut current = root.to_path_buf();
    reject_symlink_component(&current)?;

    for component in relative.components() {
        let Component::Normal(segment) = component else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "artifact path contains unsafe components",
            ));
        };
        current.push(segment);
        reject_symlink_component(&current)?;
    }

    Ok(())
}

fn reject_symlink_component(path: &Path) -> Result<(), std::io::Error> {
    let metadata = std::fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "artifact path contains a symlink",
        ));
    }
    Ok(())
}

fn relative_path_for_cap_operation(
    root: &Path,
    path: &Path,
) -> Result<PathBuf, ArtifactDomainError> {
    let relative = path
        .strip_prefix(root)
        .map_err(|_| ArtifactDomainError::UnsafeFilename {
            value: path.display().to_string(),
        })?;
    if !relative_path_components_are_safe(relative) {
        return Err(ArtifactDomainError::UnsafeFilename {
            value: relative.display().to_string(),
        });
    }
    Ok(relative.to_path_buf())
}

fn set_no_follow(options: &mut OpenOptions) {
    #[cfg(unix)]
    options.custom_flags(libc::O_NOFOLLOW);
}

#[cfg(unix)]
fn same_cap_metadata(left: &Metadata, right: &Metadata) -> bool {
    left.dev() == right.dev() && left.ino() == right.ino()
}

#[cfg(not(unix))]
fn same_cap_metadata(left: &Metadata, right: &Metadata) -> bool {
    left.len() == right.len()
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = ring::digest::digest(&ring::digest::SHA256, bytes);
    digest
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
