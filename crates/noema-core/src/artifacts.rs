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
use thiserror::Error;

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
    pub source: crate::ArtifactSource,
    /// Arbitrary artifact metadata stored with the canonical artifact row.
    pub metadata: serde_json::Value,
}

/// Errors produced while writing local artifact bytes and metadata.
#[derive(Debug, Error)]
pub enum ArtifactWriteError {
    /// The artifact filename or derived path was unsafe.
    #[error(transparent)]
    Path(#[from] crate::NoemaPathError),

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

/// Build the local download route for an artifact version.
#[must_use]
pub fn artifact_download_url(artifact_version_id: &str) -> String {
    format!("/artifacts/{artifact_version_id}/download")
}

/// Create a conversation-owned local file artifact, writing bytes first and metadata second.
///
/// # Errors
///
/// Returns [`ArtifactWriteError`] when the filename is unsafe, local file I/O
/// fails, or the canonical artifact metadata write fails.
pub async fn create_conversation_local_file_artifact(
    store: &crate::NoemaStore,
    paths: &crate::NoemaPaths,
    input: NewConversationLocalFileArtifact,
) -> Result<crate::ArtifactWithVersions, ArtifactWriteError> {
    let artifact_id = store.new_artifact_id();
    let artifact_version_id = store.new_artifact_version_id();
    let filename = crate::paths::safe_artifact_filename(&input.filename)?;
    let version_dir =
        paths.conversation_artifact_version_dir(&input.conversation_id, &artifact_id, 1);
    let artifact_path = version_dir.join(filename);
    let relative_path = artifact_relative_path(paths.root(), &artifact_path)?;

    let root_dir =
        open_cap_root(paths.root()).map_err(|source| ArtifactWriteError::CreateDirectory {
            path: paths.root().to_path_buf(),
            source,
        })?;
    let relative_version_dir = relative_path_for_cap_operation(paths.root(), &version_dir)
        .map_err(ArtifactWriteError::Path)?;
    root_dir
        .create_dir_all(&relative_version_dir)
        .map_err(|source| ArtifactWriteError::CreateDirectory {
            path: version_dir.clone(),
            source,
        })?;
    let version_dir_handle =
        open_verified_cap_dir(&root_dir, paths.root(), &version_dir).map_err(|source| {
            ArtifactWriteError::WriteFile {
                path: version_dir.clone(),
                source,
            }
        })?;

    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    set_no_follow(&mut options);
    let mut file = version_dir_handle
        .open_with(Path::new(filename), &options)
        .map_err(|source| ArtifactWriteError::WriteFile {
            path: artifact_path.clone(),
            source,
        })?;
    file.write_all(&input.bytes)
        .map_err(|source| ArtifactWriteError::WriteFile {
            path: artifact_path.clone(),
            source,
        })?;
    file.flush()
        .map_err(|source| ArtifactWriteError::WriteFile {
            path: artifact_path.clone(),
            source,
        })?;

    let content_sha256 = sha256_hex(&input.bytes);
    let create_result = store
        .create_artifact_with_initial_version(
            crate::NewArtifact {
                artifact_id: Some(artifact_id),
                owner: crate::ArtifactOwnerRef::conversation(&input.conversation_id),
                title: input.title,
                description: input.description,
                artifact_kind: input.artifact_kind,
                storage_kind: crate::ArtifactStorageKind::LocalFile,
                created_by_actor_id: input.created_by_actor_id.clone(),
                source: input.source.clone(),
                metadata: input.metadata,
            },
            crate::NewArtifactVersion {
                artifact_version_id: Some(artifact_version_id),
                title: None,
                storage: crate::ArtifactVersionStorage::LocalFile { relative_path },
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

pub(crate) fn local_artifact_absolute_path(
    paths: &crate::NoemaPaths,
    relative_path: &str,
) -> Result<PathBuf, crate::NoemaPathError> {
    if !relative_path_components_are_safe(Path::new(relative_path)) {
        return Err(crate::NoemaPathError::UnsafeArtifactFilename {
            value: relative_path.to_string(),
        });
    }
    Ok(paths.root().join(relative_path))
}

pub(crate) fn validated_local_artifact_absolute_path(
    paths: &crate::NoemaPaths,
    artifact: &crate::ArtifactRecord,
    version: &crate::ArtifactVersionRecord,
) -> Result<PathBuf, crate::NoemaPathError> {
    let crate::ArtifactVersionStorage::LocalFile { relative_path } = &version.storage else {
        return Err(crate::NoemaPathError::UnsafeArtifactFilename {
            value: version.artifact_version_id.clone(),
        });
    };

    if artifact.owner.object_type != "conversation" {
        return Err(crate::NoemaPathError::UnsafeArtifactFilename {
            value: artifact.owner.object_type.clone(),
        });
    }

    let absolute_path = local_artifact_absolute_path(paths, relative_path)?;
    let filename = absolute_path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| crate::NoemaPathError::UnsafeArtifactFilename {
            value: relative_path.clone(),
        })?;
    let filename = crate::paths::safe_artifact_filename(filename)?;
    let expected_path = paths
        .conversation_artifact_version_dir(
            &artifact.owner.object_id,
            &artifact.artifact_id,
            version.version_index,
        )
        .join(filename);
    if absolute_path != expected_path {
        return Err(crate::NoemaPathError::UnsafeArtifactFilename {
            value: relative_path.clone(),
        });
    }

    Ok(absolute_path)
}

pub(crate) fn read_validated_local_artifact_file(
    paths: &crate::NoemaPaths,
    artifact: &crate::ArtifactRecord,
    version: &crate::ArtifactVersionRecord,
) -> Result<(PathBuf, Vec<u8>), crate::NoemaPathError> {
    let absolute_path = validated_local_artifact_absolute_path(paths, artifact, version)?;
    let parent =
        absolute_path
            .parent()
            .ok_or_else(|| crate::NoemaPathError::UnsafeArtifactFilename {
                value: absolute_path.display().to_string(),
            })?;
    let filename =
        absolute_path
            .file_name()
            .ok_or_else(|| crate::NoemaPathError::UnsafeArtifactFilename {
                value: absolute_path.display().to_string(),
            })?;
    let root_dir =
        open_cap_root(paths.root()).map_err(|_| crate::NoemaPathError::UnsafeArtifactFilename {
            value: paths.root().display().to_string(),
        })?;
    let parent_dir = open_verified_cap_dir(&root_dir, paths.root(), parent).map_err(|_| {
        crate::NoemaPathError::UnsafeArtifactFilename {
            value: parent.display().to_string(),
        }
    })?;
    let filename_path = Path::new(filename);
    let before_metadata = parent_dir.symlink_metadata(filename_path).map_err(|_| {
        crate::NoemaPathError::UnsafeArtifactFilename {
            value: absolute_path.display().to_string(),
        }
    })?;
    if before_metadata.file_type().is_symlink() || !before_metadata.is_file() {
        return Err(crate::NoemaPathError::UnsafeArtifactFilename {
            value: absolute_path.display().to_string(),
        });
    }

    let mut options = OpenOptions::new();
    options.read(true);
    set_no_follow(&mut options);
    let mut file = parent_dir.open_with(filename_path, &options).map_err(|_| {
        crate::NoemaPathError::UnsafeArtifactFilename {
            value: absolute_path.display().to_string(),
        }
    })?;
    let file_metadata =
        file.metadata()
            .map_err(|_| crate::NoemaPathError::UnsafeArtifactFilename {
                value: absolute_path.display().to_string(),
            })?;
    let after_metadata = parent_dir.symlink_metadata(filename_path).map_err(|_| {
        crate::NoemaPathError::UnsafeArtifactFilename {
            value: absolute_path.display().to_string(),
        }
    })?;
    if !same_cap_metadata(&file_metadata, &after_metadata) {
        return Err(crate::NoemaPathError::UnsafeArtifactFilename {
            value: absolute_path.display().to_string(),
        });
    }

    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .map_err(|_| crate::NoemaPathError::UnsafeArtifactFilename {
            value: absolute_path.display().to_string(),
        })?;
    Ok((absolute_path, bytes))
}

fn artifact_relative_path(
    root: &Path,
    artifact_path: &Path,
) -> Result<String, crate::NoemaPathError> {
    let relative = artifact_path.strip_prefix(root).map_err(|_| {
        crate::NoemaPathError::UnsafeArtifactFilename {
            value: artifact_path.display().to_string(),
        }
    })?;
    if !relative_path_components_are_safe(relative) {
        return Err(crate::NoemaPathError::UnsafeArtifactFilename {
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
) -> Result<PathBuf, crate::NoemaPathError> {
    let relative =
        path.strip_prefix(root)
            .map_err(|_| crate::NoemaPathError::UnsafeArtifactFilename {
                value: path.display().to_string(),
            })?;
    if !relative_path_components_are_safe(relative) {
        return Err(crate::NoemaPathError::UnsafeArtifactFilename {
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
