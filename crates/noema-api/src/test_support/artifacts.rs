use std::{
    ffi::OsStr,
    path::{Component, Path, PathBuf},
    sync::Arc,
};

use noema_artifacts::{
    AppendLocalArtifactVersionRequest, ArtifactDomainError, ArtifactFileContent,
    ArtifactMetadataError, ArtifactMetadataStoreHandle, ArtifactOperationError,
    ArtifactOperationFuture, ArtifactOperations, ArtifactOperationsHandle, ArtifactStorageKind,
    ArtifactVersionRecord, ArtifactVersionStorage, ArtifactWithVersions,
    CreateLocalArtifactRequest, NewArtifact, NewArtifactVersion, ReadLocalArtifactRequest,
    artifact_version_dir, safe_artifact_filename,
};

const TEST_OBJECT_DIRECTORY: &str = "api-test";

pub(crate) fn artifact_operations(
    store: &noema_store::NoemaStore,
) -> Result<ArtifactOperationsHandle, String> {
    let environment = super::test_environment();
    artifact_operations_for_environment(store, &environment)
}

pub(crate) fn artifact_operations_for_environment(
    store: &noema_store::NoemaStore,
    environment: &super::TestEnvironment,
) -> Result<ArtifactOperationsHandle, String> {
    std::fs::create_dir_all(environment.root()).map_err(|error| error.to_string())?;
    let metadata: ArtifactMetadataStoreHandle = Arc::new(store.clone());
    Ok(Arc::new(TestArtifactOperations {
        root: environment.root().to_path_buf(),
        metadata,
    }))
}

pub(crate) fn artifact_diagnostics_for_environment(
    environment: &super::TestEnvironment,
) -> noema_host::ArtifactDiagnosticHandle {
    Arc::new(TestArtifactDiagnostics {
        path: environment.errors_log_path(),
    })
}

#[derive(Debug)]
struct TestArtifactDiagnostics {
    path: PathBuf,
}

impl noema_host::ArtifactDiagnosticOperations for TestArtifactDiagnostics {
    fn record_download_failure(&self, operation: &'static str) {
        let Some(parent) = self.path.parent() else {
            return;
        };
        if std::fs::create_dir_all(parent).is_err() {
            return;
        }
        let event = serde_json::json!({
            "category": "artifact_download_failure",
            "message": "artifact download operation failed",
            "context": { "operation": operation },
        });
        let Ok(mut line) = serde_json::to_vec(&event) else {
            return;
        };
        line.push(b'\n');
        use std::io::Write as _;
        let _ = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .and_then(|mut file| file.write_all(&line));
    }
}

#[derive(Debug)]
struct TestArtifactOperations {
    root: PathBuf,
    metadata: ArtifactMetadataStoreHandle,
}

impl TestArtifactOperations {
    fn publish(
        &self,
        owner: &noema_artifacts::ArtifactOwnerRef,
        artifact_id: &str,
        version_index: i64,
        filename: &str,
        bytes: &[u8],
    ) -> Result<String, ArtifactOperationError> {
        let filename = safe_artifact_filename(filename)?;
        let version_dir = artifact_version_dir(&self.root, owner, artifact_id, version_index)?;
        let object_dir = version_dir.join("objects").join(TEST_OBJECT_DIRECTORY);
        std::fs::create_dir_all(&object_dir)
            .map_err(|error| filesystem_error("create_test_artifact_dir", &object_dir, error))?;
        let path = object_dir.join(filename);
        std::fs::write(&path, bytes)
            .map_err(|error| filesystem_error("write_test_artifact", &path, error))?;
        path.strip_prefix(&self.root)
            .ok()
            .and_then(Path::to_str)
            .map(str::to_string)
            .ok_or_else(|| ArtifactDomainError::UnsafeFilename {
                value: path.display().to_string(),
            })
            .map_err(Into::into)
    }

    async fn create(
        &self,
        request: CreateLocalArtifactRequest,
    ) -> Result<ArtifactWithVersions, ArtifactOperationError> {
        request.owner.validate()?;
        let artifact_id = self.metadata.new_artifact_id();
        let artifact_version_id = self.metadata.new_artifact_version_id();
        let relative_path = self.publish(
            &request.owner,
            &artifact_id,
            1,
            &request.filename,
            &request.bytes,
        )?;
        let byte_size = i64::try_from(request.bytes.len()).unwrap_or(i64::MAX);
        self.metadata
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
                    storage: ArtifactVersionStorage::LocalFile { relative_path },
                    media_type: request.media_type,
                    byte_size: Some(byte_size),
                    content_sha256: None,
                    created_by_actor_id: request.created_by_actor_id,
                    source: request.source,
                    metadata: serde_json::json!({}),
                },
            )
            .await
            .map_err(Into::into)
    }

    async fn append(
        &self,
        request: AppendLocalArtifactVersionRequest,
    ) -> Result<ArtifactVersionRecord, ArtifactOperationError> {
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
        let artifact_version_id = self.metadata.new_artifact_version_id();
        let relative_path = self.publish(
            &target.owner,
            &target.artifact_id,
            target.expected_next_version_index,
            &request.filename,
            &request.bytes,
        )?;
        let byte_size = i64::try_from(request.bytes.len()).unwrap_or(i64::MAX);
        self.metadata
            .append_artifact_version(
                &request.artifact_id,
                target.expected_next_version_index,
                NewArtifactVersion {
                    artifact_version_id: Some(artifact_version_id),
                    title: request.title,
                    storage: ArtifactVersionStorage::LocalFile { relative_path },
                    media_type: request.media_type,
                    byte_size: Some(byte_size),
                    content_sha256: None,
                    created_by_actor_id: request.created_by_actor_id,
                    source: request.source,
                    metadata: request.metadata,
                },
            )
            .await
            .map_err(Into::into)
    }

    fn read(
        &self,
        request: ReadLocalArtifactRequest,
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
        if relative.is_absolute()
            || relative
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
        {
            return Err(unsafe_path(relative_path));
        }
        let expected_version_dir = artifact_version_dir(
            &self.root,
            &request.artifact.owner,
            &request.artifact.artifact_id,
            request.version.version_index,
        )?;
        let path = self.root.join(relative);
        let tail = path
            .strip_prefix(&expected_version_dir)
            .map_err(|_| unsafe_path(relative_path))?;
        let components = tail.components().collect::<Vec<_>>();
        let [
            Component::Normal(objects),
            Component::Normal(test_object),
            Component::Normal(filename),
        ] = components.as_slice()
        else {
            return Err(unsafe_path(relative_path));
        };
        if *objects != OsStr::new("objects") || *test_object != OsStr::new(TEST_OBJECT_DIRECTORY) {
            return Err(unsafe_path(relative_path));
        }
        let filename = filename
            .to_str()
            .ok_or_else(|| unsafe_path(relative_path))?;
        safe_artifact_filename(filename)?;
        reject_symlinks(&self.root, &path)?;
        let bytes = std::fs::read(&path)
            .map_err(|error| filesystem_error("read_test_artifact", &path, error))?;
        if request
            .version
            .byte_size
            .is_some_and(|expected| expected != i64::try_from(bytes.len()).unwrap_or(i64::MAX))
        {
            return Err(ArtifactOperationError::Integrity { path });
        }
        Ok(ArtifactFileContent {
            filename: filename.to_string(),
            bytes,
        })
    }
}

impl ArtifactOperations for TestArtifactOperations {
    fn create_local_file(
        &self,
        request: CreateLocalArtifactRequest,
    ) -> ArtifactOperationFuture<'_, ArtifactWithVersions> {
        Box::pin(async move { self.create(request).await })
    }

    fn append_local_file_version(
        &self,
        request: AppendLocalArtifactVersionRequest,
    ) -> ArtifactOperationFuture<'_, ArtifactVersionRecord> {
        Box::pin(async move { self.append(request).await })
    }

    fn read_local_file(
        &self,
        request: ReadLocalArtifactRequest,
    ) -> ArtifactOperationFuture<'_, ArtifactFileContent> {
        Box::pin(async move { self.read(request) })
    }
}

fn reject_symlinks(root: &Path, path: &Path) -> Result<(), ArtifactOperationError> {
    let relative = path
        .strip_prefix(root)
        .map_err(|_| unsafe_path(&path.display().to_string()))?;
    let mut current = root.to_path_buf();
    for component in relative.components() {
        let Component::Normal(component) = component else {
            return Err(unsafe_path(&path.display().to_string()));
        };
        current.push(component);
        let metadata = std::fs::symlink_metadata(&current)
            .map_err(|error| filesystem_error("inspect_test_artifact", &current, error))?;
        if metadata.file_type().is_symlink() {
            return Err(ArtifactOperationError::Integrity { path: current });
        }
    }
    Ok(())
}

fn unsafe_path(value: &str) -> ArtifactOperationError {
    ArtifactDomainError::UnsafeFilename {
        value: value.to_string(),
    }
    .into()
}

fn filesystem_error(
    operation: &'static str,
    path: &Path,
    error: std::io::Error,
) -> ArtifactOperationError {
    ArtifactOperationError::Filesystem {
        operation,
        path: path.to_path_buf(),
        message: error.to_string(),
    }
}
