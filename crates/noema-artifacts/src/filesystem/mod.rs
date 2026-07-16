//! Root-bound governed local artifact service.

mod cleanup;
mod fs;
mod storage;

#[cfg(test)]
mod tests;

use std::{
    collections::HashSet,
    fmt,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, PoisonError},
    time::Duration,
};

use cap_std::fs::Dir;
use ring::rand::{SecureRandom, SystemRandom};

use crate::{
    AppendLocalArtifactVersionRequest, ArtifactDomainError, ArtifactMetadataError,
    ArtifactMetadataStoreHandle, ArtifactOperationError, ArtifactOperationFuture,
    ArtifactOperations, ArtifactStorageKind, ArtifactVersionStorage, ArtifactWithVersions,
    CreateLocalArtifactRequest, NewArtifact, NewArtifactVersion, ReadLocalArtifactRequest,
    safe_artifact_filename,
};

const STALE_STAGING_AGE: Duration = Duration::from_secs(24 * 60 * 60);
const OPERATION_ID_BYTES: usize = 32;
const MAX_OPERATION_ID_ATTEMPTS: usize = 8;

/// Root-bound implementation of governed local artifact operations.
pub struct LocalArtifactService {
    root: PathBuf,
    root_dir: Dir,
    metadata: ArtifactMetadataStoreHandle,
    operation_ids: Arc<dyn OperationIdSource>,
    active_operations: Arc<Mutex<HashSet<String>>>,
    stale_staging_age: Duration,
    #[cfg(test)]
    publish_hook: Option<Arc<dyn storage::PublishHook>>,
}

impl fmt::Debug for LocalArtifactService {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LocalArtifactService")
            .field("root", &self.root)
            .finish_non_exhaustive()
    }
}

impl LocalArtifactService {
    /// Bind governed artifact operations to one resolved Noema root.
    ///
    /// # Errors
    ///
    /// Returns [`ArtifactOperationError`] when the root is unavailable, is not
    /// a directory, or resolves through a symbolic link.
    pub fn new(
        root: impl Into<PathBuf>,
        metadata: ArtifactMetadataStoreHandle,
    ) -> Result<Self, ArtifactOperationError> {
        Self::with_operation_ids(
            root.into(),
            metadata,
            Arc::new(SecureOperationIdSource),
            STALE_STAGING_AGE,
        )
    }

    fn with_operation_ids(
        root: PathBuf,
        metadata: ArtifactMetadataStoreHandle,
        operation_ids: Arc<dyn OperationIdSource>,
        stale_staging_age: Duration,
    ) -> Result<Self, ArtifactOperationError> {
        let root_dir = storage::open_root(&root)?;
        let active_operations = Arc::new(Mutex::new(HashSet::new()));
        storage::cleanup_stale_staging(&root_dir, &root, &active_operations, stale_staging_age)?;
        Ok(Self {
            root,
            root_dir,
            metadata,
            operation_ids,
            active_operations,
            stale_staging_age,
            #[cfg(test)]
            publish_hook: None,
        })
    }

    fn begin_operation(&self) -> Result<OperationLease, ArtifactOperationError> {
        for _ in 0..MAX_OPERATION_ID_ATTEMPTS {
            let operation_id = self.operation_ids.next_id(&self.root)?;
            if !storage::operation_id_is_valid(&operation_id) {
                return Err(ArtifactDomainError::UnsafeFilename {
                    value: operation_id,
                }
                .into());
            }
            let mut active = self
                .active_operations
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            if active.insert(operation_id.clone()) {
                drop(active);
                return Ok(OperationLease {
                    operation_id,
                    active_operations: self.active_operations.clone(),
                });
            }
        }
        Err(ArtifactOperationError::Filesystem {
            operation: "allocate_operation_id",
            path: self.root.clone(),
            message: "could not allocate a unique artifact operation id".to_string(),
        })
    }

    async fn create_local(
        &self,
        request: CreateLocalArtifactRequest,
    ) -> Result<ArtifactWithVersions, ArtifactOperationError> {
        request.owner.validate()?;
        let filename = safe_artifact_filename(&request.filename)?.to_string();
        let artifact_id = self.metadata.new_artifact_id();
        let artifact_version_id = self.metadata.new_artifact_version_id();
        let operation = self.begin_operation()?;
        let root = self.root.clone();
        let root_dir = storage::duplicate_root(&self.root_dir, &self.root)?;
        let owner = request.owner.clone();
        let publish_artifact_id = artifact_id.clone();
        let bytes = request.bytes;
        let stale_staging_age = self.stale_staging_age;
        let active_operations = self.active_operations.clone();
        #[cfg(test)]
        let publish_hook = self.publish_hook.clone();
        let publication = tokio::task::spawn_blocking(move || {
            storage::stage_and_publish(storage::PublishRequest {
                root,
                root_dir,
                owner,
                artifact_id: publish_artifact_id,
                version_index: 1,
                filename,
                bytes,
                operation,
                active_operations,
                stale_staging_age,
                #[cfg(test)]
                publish_hook,
            })
        })
        .await
        .map_err(|_| ArtifactOperationError::Filesystem {
            operation: "publish",
            path: self.root.clone(),
            message: "artifact filesystem worker stopped".to_string(),
        })??;

        let metadata_result = self
            .metadata
            .create_artifact_with_initial_version(
                NewArtifact {
                    artifact_id: Some(artifact_id),
                    owner: request.owner,
                    title: request.title,
                    description: request.description,
                    artifact_kind: request.artifact_kind,
                    storage_kind: ArtifactStorageKind::LocalFile,
                    created_by_actor_id: request.created_by_actor_id.clone(),
                    source: request.source.clone(),
                    metadata: request.metadata,
                },
                NewArtifactVersion {
                    artifact_version_id: Some(artifact_version_id),
                    title: None,
                    storage: ArtifactVersionStorage::LocalFile {
                        relative_path: publication.relative_path().to_string(),
                    },
                    media_type: request.media_type,
                    byte_size: Some(publication.byte_size()),
                    content_sha256: Some(publication.content_sha256().to_string()),
                    created_by_actor_id: request.created_by_actor_id,
                    source: request.source,
                    metadata: serde_json::json!({}),
                },
            )
            .await;
        finish_metadata(publication, metadata_result)
    }

    async fn append_local(
        &self,
        request: AppendLocalArtifactVersionRequest,
    ) -> Result<crate::ArtifactVersionRecord, ArtifactOperationError> {
        let target = self
            .metadata
            .load_append_target(&request.artifact_id)
            .await?
            .ok_or_else(|| ArtifactMetadataError::NotFound {
                artifact_id: request.artifact_id.clone(),
            })?;
        if target.storage_kind != ArtifactStorageKind::LocalFile {
            return Err(ArtifactDomainError::StorageKindMismatch.into());
        }
        if target.artifact_id != request.artifact_id {
            return Err(ArtifactMetadataError::Invariant {
                message: "append target belongs to a different artifact".to_string(),
            }
            .into());
        }
        target.owner.validate()?;
        let filename = safe_artifact_filename(&request.filename)?.to_string();
        let artifact_version_id = self.metadata.new_artifact_version_id();
        let operation = self.begin_operation()?;
        let root = self.root.clone();
        let root_dir = storage::duplicate_root(&self.root_dir, &self.root)?;
        let artifact_id = request.artifact_id.clone();
        let publish_artifact_id = artifact_id.clone();
        let owner = target.owner;
        let expected_next_version_index = target.expected_next_version_index;
        let bytes = request.bytes;
        let stale_staging_age = self.stale_staging_age;
        let active_operations = self.active_operations.clone();
        #[cfg(test)]
        let publish_hook = self.publish_hook.clone();
        let publication = tokio::task::spawn_blocking(move || {
            storage::stage_and_publish(storage::PublishRequest {
                root,
                root_dir,
                owner,
                artifact_id: publish_artifact_id,
                version_index: expected_next_version_index,
                filename,
                bytes,
                operation,
                active_operations,
                stale_staging_age,
                #[cfg(test)]
                publish_hook,
            })
        })
        .await
        .map_err(|_| ArtifactOperationError::Filesystem {
            operation: "publish",
            path: self.root.clone(),
            message: "artifact filesystem worker stopped".to_string(),
        })??;

        let metadata_result = self
            .metadata
            .append_artifact_version(
                &artifact_id,
                expected_next_version_index,
                NewArtifactVersion {
                    artifact_version_id: Some(artifact_version_id),
                    title: request.title,
                    storage: ArtifactVersionStorage::LocalFile {
                        relative_path: publication.relative_path().to_string(),
                    },
                    media_type: request.media_type,
                    byte_size: Some(publication.byte_size()),
                    content_sha256: Some(publication.content_sha256().to_string()),
                    created_by_actor_id: request.created_by_actor_id,
                    source: request.source,
                    metadata: request.metadata,
                },
            )
            .await;
        finish_metadata(publication, metadata_result)
    }
}

impl ArtifactOperations for LocalArtifactService {
    fn create_local_file(
        &self,
        request: CreateLocalArtifactRequest,
    ) -> ArtifactOperationFuture<'_, ArtifactWithVersions> {
        Box::pin(async move { self.create_local(request).await })
    }

    fn append_local_file_version(
        &self,
        request: AppendLocalArtifactVersionRequest,
    ) -> ArtifactOperationFuture<'_, crate::ArtifactVersionRecord> {
        Box::pin(async move { self.append_local(request).await })
    }

    fn read_local_file(
        &self,
        request: ReadLocalArtifactRequest,
    ) -> ArtifactOperationFuture<'_, crate::ArtifactFileContent> {
        Box::pin(async move {
            let root = self.root.clone();
            let root_dir = storage::duplicate_root(&self.root_dir, &self.root)?;
            tokio::task::spawn_blocking(move || {
                storage::read_local_file(&root_dir, &root, &request)
            })
            .await
            .map_err(|_| ArtifactOperationError::Filesystem {
                operation: "read",
                path: self.root.clone(),
                message: "artifact filesystem worker stopped".to_string(),
            })?
        })
    }
}

fn finish_metadata<T>(
    mut publication: storage::PublishedObject,
    result: Result<T, ArtifactMetadataError>,
) -> Result<T, ArtifactOperationError> {
    match result {
        Ok(value) => {
            publication.disarm();
            Ok(value)
        }
        Err(metadata_error) => Err(publication.metadata_failed(metadata_error)),
    }
}

trait OperationIdSource: fmt::Debug + Send + Sync {
    fn next_id(&self, root: &Path) -> Result<String, ArtifactOperationError>;
}

#[derive(Debug)]
struct SecureOperationIdSource;

impl OperationIdSource for SecureOperationIdSource {
    fn next_id(&self, root: &Path) -> Result<String, ArtifactOperationError> {
        let mut bytes = [0_u8; OPERATION_ID_BYTES];
        SystemRandom::new()
            .fill(&mut bytes)
            .map_err(|_| ArtifactOperationError::Filesystem {
                operation: "allocate_operation_id",
                path: root.to_path_buf(),
                message: "secure randomness is unavailable".to_string(),
            })?;
        let mut operation_id = String::with_capacity(3 + (OPERATION_ID_BYTES * 2));
        operation_id.push_str("op-");
        for byte in bytes {
            use std::fmt::Write as _;
            write!(&mut operation_id, "{byte:02x}").map_err(|_| {
                ArtifactOperationError::Filesystem {
                    operation: "allocate_operation_id",
                    path: root.to_path_buf(),
                    message: "could not encode artifact operation id".to_string(),
                }
            })?;
        }
        Ok(operation_id)
    }
}

pub(super) struct OperationLease {
    operation_id: String,
    active_operations: Arc<Mutex<HashSet<String>>>,
}

impl OperationLease {
    fn id(&self) -> &str {
        &self.operation_id
    }
}

impl Drop for OperationLease {
    fn drop(&mut self) {
        self.active_operations
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&self.operation_id);
    }
}
