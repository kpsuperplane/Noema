use std::path::{Component, Path, PathBuf};

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

    tokio::fs::create_dir_all(&version_dir)
        .await
        .map_err(|source| ArtifactWriteError::CreateDirectory {
            path: version_dir.clone(),
            source,
        })?;
    tokio::fs::write(&artifact_path, &input.bytes)
        .await
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

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = ring::digest::digest(&ring::digest::SHA256, bytes);
    digest
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
