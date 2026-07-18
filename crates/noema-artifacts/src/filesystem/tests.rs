//! Local artifact filesystem service tests.

use std::sync::Barrier as ThreadBarrier;
use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use tempfile::TempDir;
use tokio::sync::Notify;

use crate::{
    AppendLocalArtifactVersionRequest, ArtifactAppendTarget, ArtifactDomainError, ArtifactFuture,
    ArtifactMetadataError, ArtifactMetadataStore, ArtifactOperationError, ArtifactOperations,
    ArtifactOwnerRef, ArtifactRecord, ArtifactSource, ArtifactVersionRecord, ArtifactWithVersions,
    CreateLocalArtifactRequest, NewArtifact, NewArtifactVersion, ReadLocalArtifactRequest,
};

use super::{LocalArtifactService, OperationIdSource, fs, storage};

const OPERATION_ONE: &str = "op-1111111111111111111111111111111111111111111111111111111111111111";
const OPERATION_TWO: &str = "op-2222222222222222222222222222222222222222222222222222222222222222";

#[derive(Debug, Default)]
struct FakeMetadataStore {
    next_id: AtomicU64,
    artifact: Mutex<Option<ArtifactWithVersions>>,
    write_behavior: Mutex<WriteBehavior>,
}

#[derive(Debug, Clone, Default)]
enum WriteBehavior {
    #[default]
    Immediate,
    Fail,
    Block(Arc<WriteGate>),
}

#[derive(Debug, Default)]
struct WriteGate {
    entered: Notify,
    release: Notify,
}

#[derive(Debug)]
struct BlockingFailPublishHook {
    entered: Arc<ThreadBarrier>,
    release: Arc<ThreadBarrier>,
    path: PathBuf,
}

impl storage::PublishHook for BlockingFailPublishHook {
    fn before_publish(&self) -> Result<(), ArtifactOperationError> {
        self.entered.wait();
        self.release.wait();
        Err(ArtifactOperationError::Filesystem {
            operation: "publish_test_hook",
            path: self.path.clone(),
            message: "injected pre-publication failure".to_string(),
        })
    }
}

impl FakeMetadataStore {
    fn set_write_behavior(&self, behavior: WriteBehavior) {
        *self.write_behavior.lock().expect("write behavior lock") = behavior;
    }

    async fn before_write(&self) -> Result<(), ArtifactMetadataError> {
        let behavior = self
            .write_behavior
            .lock()
            .expect("write behavior lock")
            .clone();
        match behavior {
            WriteBehavior::Immediate => Ok(()),
            WriteBehavior::Fail => Err(ArtifactMetadataError::Persistence {
                message: "injected metadata failure".to_string(),
            }),
            WriteBehavior::Block(gate) => {
                gate.entered.notify_one();
                gate.release.notified().await;
                Ok(())
            }
        }
    }

    fn artifact_count(&self) -> usize {
        usize::from(self.artifact.lock().expect("artifact lock").is_some())
    }
}

impl ArtifactMetadataStore for FakeMetadataStore {
    fn new_artifact_id(&self) -> String {
        format!("artifact-{}", self.next_id.fetch_add(1, Ordering::Relaxed))
    }

    fn new_artifact_version_id(&self) -> String {
        format!("version-{}", self.next_id.fetch_add(1, Ordering::Relaxed))
    }

    fn load_append_target<'a>(
        &'a self,
        artifact_id: &'a str,
    ) -> ArtifactFuture<'a, Option<ArtifactAppendTarget>> {
        Box::pin(async move {
            Ok(self
                .artifact
                .lock()
                .expect("artifact lock")
                .as_ref()
                .filter(|artifact| artifact.artifact.artifact_id == artifact_id)
                .map(|artifact| ArtifactAppendTarget {
                    artifact_id: artifact.artifact.artifact_id.clone(),
                    owner: artifact.artifact.owner.clone(),
                    storage_kind: artifact.artifact.storage_kind,
                    expected_next_version_index: artifact.versions.len() as i64 + 1,
                }))
        })
    }

    fn create_artifact_with_initial_version<'a>(
        &'a self,
        artifact: NewArtifact,
        initial_version: NewArtifactVersion,
    ) -> ArtifactFuture<'a, ArtifactWithVersions> {
        Box::pin(async move {
            self.before_write().await?;
            let artifact_id = artifact.artifact_id.expect("service supplies artifact id");
            let version = artifact_version_record(artifact_id.clone(), 1, initial_version);
            let artifact = ArtifactRecord {
                artifact_id: artifact_id.clone(),
                owner: artifact.owner,
                title: artifact.title,
                description: artifact.description,
                artifact_kind: artifact.artifact_kind,
                storage_kind: artifact.storage_kind,
                current_version_id: Some(version.artifact_version_id.clone()),
                created_by_actor_id: artifact.created_by_actor_id,
                source: artifact.source,
                metadata: artifact.metadata,
                created_at: "now".to_string(),
                updated_at: "now".to_string(),
            };
            let result = ArtifactWithVersions {
                artifact,
                current_version: version.clone(),
                versions: vec![version],
            };
            *self.artifact.lock().expect("artifact lock") = Some(result.clone());
            Ok(result)
        })
    }

    fn append_artifact_version<'a>(
        &'a self,
        artifact_id: &'a str,
        expected_next_version_index: i64,
        version: NewArtifactVersion,
    ) -> ArtifactFuture<'a, ArtifactVersionRecord> {
        Box::pin(async move {
            self.before_write().await?;
            let mut stored = self.artifact.lock().expect("artifact lock");
            let artifact = stored
                .as_mut()
                .filter(|stored| stored.artifact.artifact_id == artifact_id)
                .ok_or_else(|| ArtifactMetadataError::NotFound {
                    artifact_id: artifact_id.to_string(),
                })?;
            let actual_next_version_index = artifact.versions.len() as i64 + 1;
            if actual_next_version_index != expected_next_version_index {
                return Err(ArtifactMetadataError::AppendConflict {
                    artifact_id: artifact_id.to_string(),
                    expected_next_version_index,
                    actual_next_version_index,
                });
            }
            let record = artifact_version_record(
                artifact_id.to_string(),
                expected_next_version_index,
                version,
            );
            artifact.artifact.current_version_id = Some(record.artifact_version_id.clone());
            artifact.current_version = record.clone();
            artifact.versions.push(record.clone());
            Ok(record)
        })
    }
}

fn artifact_version_record(
    artifact_id: String,
    version_index: i64,
    version: NewArtifactVersion,
) -> ArtifactVersionRecord {
    ArtifactVersionRecord {
        artifact_version_id: version
            .artifact_version_id
            .expect("service supplies version id"),
        artifact_id,
        version_index,
        title: version.title,
        storage: version.storage,
        media_type: version.media_type,
        byte_size: version.byte_size,
        content_sha256: version.content_sha256,
        created_by_actor_id: version.created_by_actor_id,
        source: version.source,
        metadata: version.metadata,
        created_at: "now".to_string(),
    }
}

#[derive(Debug)]
struct FixedOperationIds {
    ids: Mutex<VecDeque<String>>,
}

impl FixedOperationIds {
    fn new(ids: impl IntoIterator<Item = &'static str>) -> Arc<Self> {
        Arc::new(Self {
            ids: Mutex::new(ids.into_iter().map(str::to_string).collect()),
        })
    }
}

impl OperationIdSource for FixedOperationIds {
    fn next_id(&self, root: &Path) -> Result<String, ArtifactOperationError> {
        self.ids
            .lock()
            .expect("operation id lock")
            .pop_front()
            .ok_or_else(|| ArtifactOperationError::Filesystem {
                operation: "allocate_operation_id",
                path: root.to_path_buf(),
                message: "test operation ids exhausted".to_string(),
            })
    }
}

#[tokio::test]
async fn publication_lifecycle_and_cancellation_contracts() {
    // Case: create_append_and_read_verify_immutable_bytes.
    let temp = TempDir::new().expect("tempdir");
    let metadata = Arc::new(FakeMetadataStore::default());
    let service = test_service(
        temp.path(),
        metadata,
        FixedOperationIds::new([OPERATION_ONE, OPERATION_TWO]),
    );

    let created = service
        .create_local_file(create_request(b"first"))
        .await
        .expect("create artifact");
    let appended = service
        .append_local_file_version(AppendLocalArtifactVersionRequest {
            artifact_id: created.artifact.artifact_id.clone(),
            title: Some("second".to_string()),
            filename: "report.txt".to_string(),
            bytes: b"second".to_vec(),
            media_type: Some("text/plain".to_string()),
            created_by_actor_id: "actor-1".to_string(),
            source: ArtifactSource::default(),
            metadata: serde_json::json!({}),
        })
        .await
        .expect("append artifact");

    let first_content = service
        .read_local_file(ReadLocalArtifactRequest {
            artifact: created.artifact.clone(),
            version: created.current_version.clone(),
        })
        .await
        .expect("read first artifact version");
    let content = service
        .read_local_file(ReadLocalArtifactRequest {
            artifact: created.artifact.clone(),
            version: appended.clone(),
        })
        .await
        .expect("read artifact");
    assert_eq!(first_content.bytes, b"first");
    assert_eq!(content.filename, "report.txt");
    assert_eq!(content.bytes, b"second");
    assert_ne!(created.current_version.storage, appended.storage);

    let mut incomplete_version = created.current_version;
    incomplete_version.byte_size = None;
    let error = service
        .read_local_file(ReadLocalArtifactRequest {
            artifact: created.artifact,
            version: incomplete_version,
        })
        .await
        .expect_err("incomplete local metadata must fail closed");
    assert!(matches!(
        error,
        ArtifactOperationError::Metadata(ArtifactMetadataError::Invariant { .. })
    ));
    assert_staging_empty(temp.path());

    // Case: metadata_failure_removes_only_operation_private_publication.
    let temp = TempDir::new().expect("tempdir");
    let metadata = Arc::new(FakeMetadataStore::default());
    metadata.set_write_behavior(WriteBehavior::Fail);
    let unrelated = temp.path().join("unrelated.txt");
    std::fs::write(&unrelated, b"keep").expect("unrelated file");
    let service = test_service(
        temp.path(),
        metadata.clone(),
        FixedOperationIds::new([OPERATION_ONE]),
    );

    let error = service
        .create_local_file(create_request(b"discard"))
        .await
        .expect_err("metadata failure");

    assert!(matches!(error, ArtifactOperationError::Metadata(_)));
    assert_eq!(metadata.artifact_count(), 0);
    assert_eq!(std::fs::read(unrelated).expect("unrelated bytes"), b"keep");
    let version_dir = crate::artifact_version_dir(
        temp.path(),
        &ArtifactOwnerRef::conversation("conversation-1"),
        "artifact-0",
        1,
    )
    .expect("version directory");
    let objects_dir = version_dir.join("objects");
    assert_eq!(
        std::fs::read_dir(objects_dir)
            .expect("objects directory")
            .count(),
        0,
        "failed publication should leave no private object directory"
    );
    assert_staging_empty(temp.path());

    // Case: cancelling_blocked_metadata_future_commits_nothing_and_removes_bytes.
    let temp = TempDir::new().expect("tempdir");
    let metadata = Arc::new(FakeMetadataStore::default());
    let gate = Arc::new(WriteGate::default());
    metadata.set_write_behavior(WriteBehavior::Block(gate.clone()));
    let service = Arc::new(test_service(
        temp.path(),
        metadata.clone(),
        FixedOperationIds::new([OPERATION_ONE]),
    ));
    let task = {
        let service = service.clone();
        tokio::spawn(async move { service.create_local_file(create_request(b"cancel")).await })
    };
    gate.entered.notified().await;
    assert_eq!(regular_files(temp.path()).len(), 1);

    task.abort();
    let _ = task.await;

    assert_eq!(metadata.artifact_count(), 0);
    assert!(regular_files(temp.path()).is_empty());
    gate.release.notify_one();

    // Case: cancelling_before_publication_cleans_detached_staging_worker.
    let temp = TempDir::new().expect("tempdir");
    let metadata = Arc::new(FakeMetadataStore::default());
    let entered = Arc::new(ThreadBarrier::new(2));
    let release = Arc::new(ThreadBarrier::new(2));
    let mut service = test_service(
        temp.path(),
        metadata.clone(),
        FixedOperationIds::new([OPERATION_ONE]),
    );
    service.publish_hook = Some(Arc::new(BlockingFailPublishHook {
        entered: entered.clone(),
        release: release.clone(),
        path: temp.path().to_path_buf(),
    }));
    let service = Arc::new(service);
    let task = {
        let service = service.clone();
        tokio::spawn(async move { service.create_local_file(create_request(b"cancel")).await })
    };
    tokio::task::spawn_blocking(move || entered.wait())
        .await
        .expect("wait for staging hook");
    assert_eq!(regular_files(temp.path()).len(), 1);

    task.abort();
    tokio::task::spawn_blocking(move || release.wait())
        .await
        .expect("release staging hook");
    let _ = task.await;
    for _ in 0..100 {
        if regular_files(temp.path()).is_empty() && staging_is_empty(temp.path()) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }

    assert_eq!(metadata.artifact_count(), 0);
    assert!(regular_files(temp.path()).is_empty());
    assert_staging_empty(temp.path());
}

#[tokio::test]
async fn confinement_identity_and_cleanup_contracts() {
    // Case: replacing_root_after_construction_fails_closed.
    let temp = TempDir::new().expect("tempdir");
    let root = temp.path().join("root");
    let retained = temp.path().join("retained");
    std::fs::create_dir(&root).expect("root");
    let metadata = Arc::new(FakeMetadataStore::default());
    let service = test_service(
        &root,
        metadata.clone(),
        FixedOperationIds::new([OPERATION_ONE]),
    );
    std::fs::rename(&root, &retained).expect("move retained root");
    std::fs::create_dir(&root).expect("replacement root");

    let error = service
        .create_local_file(create_request(b"confined"))
        .await
        .expect_err("root replacement rejected");

    assert!(matches!(error, ArtifactOperationError::Filesystem { .. }));
    assert_eq!(metadata.artifact_count(), 0);
    assert!(regular_files(&root).is_empty());
    assert!(regular_files(&retained).is_empty());

    // Case: conversation_local_file_artifact_rejects_symlinked_artifact_root.
    #[cfg(unix)]
    {
        let temp = TempDir::new().expect("tempdir");
        let service = test_service(
            temp.path(),
            Arc::new(FakeMetadataStore::default()),
            FixedOperationIds::new([OPERATION_ONE]),
        );
        let owner = ArtifactOwnerRef::conversation("conversation-1");
        let artifact_root = crate::owner_artifacts_dir(temp.path(), &owner).expect("artifact root");
        std::fs::create_dir_all(artifact_root.parent().expect("conversation directory"))
            .expect("conversation directory");
        let outside = temp.path().join("outside");
        std::fs::create_dir(&outside).expect("outside directory");
        std::os::unix::fs::symlink(outside, artifact_root).expect("symlink artifact root");

        let error = service
            .create_local_file(create_request(b"report"))
            .await
            .expect_err("symlinked artifact root rejected");
        assert!(matches!(error, ArtifactOperationError::Filesystem { .. }));
    }

    // Case: publication_rejects_clobber_and_malformed_operation_ids.
    let temp = TempDir::new().expect("tempdir");
    let staging_path = temp.path().join("staging");
    let object_path = temp.path().join("object");
    std::fs::create_dir(&staging_path).expect("staging dir");
    std::fs::create_dir(&object_path).expect("object dir");
    std::fs::write(staging_path.join("report.txt"), b"new").expect("staged file");
    std::fs::write(object_path.join("report.txt"), b"existing").expect("existing object");
    let root = fs::open_root(temp.path()).expect("cap root");
    let staging =
        fs::open_verified_cap_dir(&root, temp.path(), &staging_path).expect("open staging dir");
    let object =
        fs::open_verified_cap_dir(&root, temp.path(), &object_path).expect("open object dir");

    let result = storage::hard_link_no_clobber(
        &staging,
        Path::new("report.txt"),
        &object,
        Path::new("report.txt"),
        &object_path.join("report.txt"),
    );

    assert!(result.is_err());
    assert_eq!(
        std::fs::read(object_path.join("report.txt")).expect("destination bytes"),
        b"existing"
    );

    let invalid = TempDir::new().expect("tempdir");
    let service = test_service(
        invalid.path(),
        Arc::new(FakeMetadataStore::default()),
        FixedOperationIds::new(["op-not-random"]),
    );
    let error = service
        .create_local_file(create_request(b"never-written"))
        .await
        .expect_err("malformed id rejected");
    assert!(matches!(
        error,
        ArtifactOperationError::Domain(ArtifactDomainError::UnsafeFilename { .. })
    ));
    assert!(regular_files(invalid.path()).is_empty());

    // Case: startup_cleanup_only_removes_recognized_real_private_entries.
    let temp = TempDir::new().expect("tempdir");
    let staging = temp.path().join(".artifact-staging");
    let stale = staging.join(OPERATION_ONE);
    let invalid = staging.join("not-governed");
    std::fs::create_dir_all(&stale).expect("stale op");
    std::fs::write(stale.join("report.txt"), b"stale").expect("stale file");
    std::fs::create_dir(&invalid).expect("invalid entry");
    std::fs::write(invalid.join("keep.txt"), b"keep").expect("invalid file");
    #[cfg(unix)]
    let outside = {
        use std::os::unix::fs::symlink;

        let outside = temp.path().join("outside");
        std::fs::create_dir(&outside).expect("outside dir");
        std::fs::write(outside.join("victim.txt"), b"keep").expect("outside file");
        symlink(&outside, staging.join(OPERATION_TWO)).expect("staging symlink");
        outside
    };

    let _service = LocalArtifactService::with_operation_ids(
        temp.path().to_path_buf(),
        Arc::new(FakeMetadataStore::default()),
        FixedOperationIds::new([]),
        Duration::ZERO,
    )
    .expect("service");

    assert!(!stale.exists());
    assert_eq!(
        std::fs::read(invalid.join("keep.txt")).expect("invalid entry preserved"),
        b"keep"
    );
    #[cfg(unix)]
    assert_eq!(
        std::fs::read(outside.join("victim.txt")).expect("outside file preserved"),
        b"keep"
    );
    #[cfg(unix)]
    assert!(staging.join(OPERATION_TWO).exists());
}

fn test_service(
    root: &Path,
    metadata: Arc<FakeMetadataStore>,
    ids: Arc<FixedOperationIds>,
) -> LocalArtifactService {
    LocalArtifactService::with_operation_ids(
        root.to_path_buf(),
        metadata,
        ids,
        Duration::from_secs(24 * 60 * 60),
    )
    .expect("local artifact service")
}

fn create_request(bytes: &[u8]) -> CreateLocalArtifactRequest {
    CreateLocalArtifactRequest {
        owner: ArtifactOwnerRef::conversation("conversation-1"),
        title: "Report".to_string(),
        description: None,
        artifact_kind: "report".to_string(),
        filename: "report.txt".to_string(),
        bytes: bytes.to_vec(),
        media_type: Some("text/plain".to_string()),
        created_by_actor_id: "actor-1".to_string(),
        source: ArtifactSource::default(),
        metadata: serde_json::json!({}),
    }
}

fn regular_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut directories = vec![root.to_path_buf()];
    while let Some(directory) = directories.pop() {
        for entry in std::fs::read_dir(directory).into_iter().flatten().flatten() {
            let path = entry.path();
            let Ok(metadata) = std::fs::symlink_metadata(&path) else {
                continue;
            };
            if metadata.is_file() {
                files.push(path);
            } else if metadata.is_dir() && !metadata.file_type().is_symlink() {
                directories.push(path);
            }
        }
    }
    files
}

fn assert_staging_empty(root: &Path) {
    let staging = root.join(".artifact-staging");
    assert!(
        staging.is_dir(),
        "staging root should remain as shared scaffolding"
    );
    assert_eq!(
        std::fs::read_dir(staging).expect("staging root").count(),
        0,
        "staging root should contain no operation-private entries"
    );
}

fn staging_is_empty(root: &Path) -> bool {
    std::fs::read_dir(root.join(".artifact-staging"))
        .is_ok_and(|mut entries| entries.next().is_none())
}
