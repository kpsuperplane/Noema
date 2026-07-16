//! Task-owned local artifact writers.

use noema_artifacts::{ArtifactOwnerRef, artifact_version_dir, safe_artifact_filename};
use noema_home::NoemaPaths;

use super::{ArtifactWriteError, artifact_relative_path, sha256_hex, write_local_artifact_bytes};

/// Input for creating a task-owned local file artifact and first version.
#[derive(Debug, Clone, PartialEq)]
pub struct NewTaskLocalFileArtifact {
    /// Owning task id.
    pub task_id: String,
    /// Human-readable title.
    pub title: String,
    /// Optional short description.
    pub description: Option<String>,
    /// Product-defined artifact kind.
    pub artifact_kind: String,
    /// Safe single-segment filename.
    pub filename: String,
    /// Initial version bytes.
    pub bytes: Vec<u8>,
    /// Optional media type.
    pub media_type: Option<String>,
    /// Actor creating the artifact.
    pub created_by_actor_id: String,
    /// Source provenance.
    pub source: noema_artifacts::ArtifactSource,
    /// Arbitrary artifact metadata.
    pub metadata: serde_json::Value,
}

/// Input for appending a local file version to a task-owned artifact.
#[derive(Debug, Clone, PartialEq)]
pub struct NewTaskLocalFileArtifactVersion {
    /// Existing artifact id.
    pub artifact_id: String,
    /// Optional version title.
    pub title: Option<String>,
    /// Safe single-segment filename.
    pub filename: String,
    /// Version bytes.
    pub bytes: Vec<u8>,
    /// Optional media type.
    pub media_type: Option<String>,
    /// Actor creating the version.
    pub created_by_actor_id: String,
    /// Source provenance.
    pub source: noema_artifacts::ArtifactSource,
    /// Arbitrary version metadata.
    pub metadata: serde_json::Value,
}

/// Create a task-owned local file artifact, writing bytes before metadata.
///
/// # Errors
///
/// Returns [`ArtifactWriteError`] when the path, file write, or metadata write
/// fails.
pub async fn create_task_local_file_artifact(
    store: &crate::NoemaStore,
    paths: &NoemaPaths,
    input: NewTaskLocalFileArtifact,
) -> Result<noema_artifacts::ArtifactWithVersions, ArtifactWriteError> {
    let artifact_id = store.new_artifact_id();
    let artifact_version_id = store.new_artifact_version_id();
    let filename = safe_artifact_filename(&input.filename)?;
    let version_dir = artifact_version_dir(
        paths.root(),
        &ArtifactOwnerRef::task(&input.task_id),
        &artifact_id,
        1,
    )?;
    let artifact_path = version_dir.join(filename);
    let relative_path = artifact_relative_path(paths.root(), &artifact_path)?;
    write_local_artifact_bytes(paths, &version_dir, &artifact_path, &input.bytes)?;

    let create_result = store
        .create_artifact_with_initial_version(
            noema_artifacts::NewArtifact {
                artifact_id: Some(artifact_id),
                owner: noema_artifacts::ArtifactOwnerRef::task(&input.task_id),
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
                content_sha256: Some(sha256_hex(&input.bytes)),
                created_by_actor_id: input.created_by_actor_id,
                source: input.source,
                metadata: serde_json::json!({}),
            },
        )
        .await;
    rollback_file_on_store_error(artifact_path, create_result).await
}

/// Append a new immutable version to a task-owned local file artifact.
///
/// # Errors
///
/// Returns [`ArtifactWriteError`] when the artifact is invalid or the path,
/// file write, or metadata write fails.
pub async fn append_task_local_file_artifact_version(
    store: &crate::NoemaStore,
    paths: &NoemaPaths,
    input: NewTaskLocalFileArtifactVersion,
) -> Result<noema_artifacts::ArtifactVersionRecord, ArtifactWriteError> {
    let artifact = store
        .get_artifact(&input.artifact_id)
        .await?
        .ok_or_else(|| crate::StoreError::ArtifactNotFound {
            artifact_id: input.artifact_id.clone(),
        })?;
    if artifact.artifact.owner.object_type != "task"
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

    let append_result = store
        .append_artifact_version(
            &artifact.artifact.artifact_id,
            noema_artifacts::NewArtifactVersion {
                artifact_version_id: None,
                title: input.title,
                storage: noema_artifacts::ArtifactVersionStorage::LocalFile { relative_path },
                media_type: input.media_type,
                byte_size: Some(input.bytes.len() as i64),
                content_sha256: Some(sha256_hex(&input.bytes)),
                created_by_actor_id: input.created_by_actor_id,
                source: input.source,
                metadata: input.metadata,
            },
        )
        .await;
    rollback_file_on_store_error(artifact_path, append_result).await
}

async fn rollback_file_on_store_error<T>(
    artifact_path: std::path::PathBuf,
    result: Result<T, crate::StoreError>,
) -> Result<T, ArtifactWriteError> {
    match result {
        Ok(value) => Ok(value),
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
