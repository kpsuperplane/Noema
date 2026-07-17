//! Artifact-filesystem composition tests backed by the SQLite store.

use std::{future::Future, pin::Pin, sync::Arc, time::Duration};

use noema_artifacts::{
    AppendLocalArtifactVersionRequest, ArtifactAppendTarget, ArtifactFuture, ArtifactMetadataStore,
    ArtifactMetadataStoreHandle, ArtifactOperationError, ArtifactOperations, ArtifactOwnerRef,
    ArtifactSource, ArtifactStorageKind, ArtifactVersionRecord, ArtifactVersionStorage,
    ArtifactWithVersions, CreateLocalArtifactRequest, LocalArtifactService, NewArtifact,
    NewArtifactVersion, ReadLocalArtifactRequest,
};
use tempfile::TempDir;
use tokio::sync::Barrier;

use noema_store::{NoemaStore, StoreConfig};

fn store_config(root: &std::path::Path) -> StoreConfig {
    StoreConfig::new(root.join("db/noema.sqlite3"))
}

#[tokio::test]
async fn conversation_local_file_artifact_writes_bytes_and_metadata() {
    let home = TempDir::new().expect("temp store root");
    let store = NoemaStore::open(&store_config(home.path()))
        .await
        .expect("open store");
    let conversation = store
        .create_conversation(noema_conversations::NewConversation::local_chat(None, None))
        .await
        .expect("conversation");
    let bytes = b"# report\n".to_vec();
    let artifact_metadata: ArtifactMetadataStoreHandle = Arc::new(store);
    let artifact_operations =
        LocalArtifactService::new(home.path(), artifact_metadata).expect("artifact operations");

    let artifact = artifact_operations
        .create_local_file(CreateLocalArtifactRequest {
            owner: ArtifactOwnerRef::conversation(&conversation.conversation_id),
            title: "Session report".to_string(),
            description: Some("Local markdown artifact".to_string()),
            artifact_kind: "document".to_string(),
            filename: "report.md".to_string(),
            bytes: bytes.clone(),
            media_type: Some("text/markdown".to_string()),
            created_by_actor_id: "agent:primary".to_string(),
            source: ArtifactSource {
                conversation_id: Some(conversation.conversation_id.clone()),
                turn_id: None,
                item_id: None,
            },
            metadata: serde_json::json!({"origin": "unit-test"}),
        })
        .await
        .expect("create local artifact");

    assert_eq!(
        artifact.artifact.storage_kind,
        ArtifactStorageKind::LocalFile
    );
    assert_eq!(artifact.current_version.version_index, 1);
    assert_eq!(
        artifact.current_version.media_type.as_deref(),
        Some("text/markdown")
    );
    assert_eq!(artifact.current_version.byte_size, Some(bytes.len() as i64));
    assert!(artifact.current_version.content_sha256.is_some());

    let ArtifactVersionStorage::LocalFile { relative_path } = &artifact.current_version.storage
    else {
        panic!("expected local file storage");
    };
    let absolute_path = home.path().join(relative_path);
    assert_eq!(
        tokio::fs::read(&absolute_path).await.expect("read bytes"),
        bytes
    );
    let owner = ArtifactOwnerRef::conversation(&conversation.conversation_id);
    assert!(absolute_path.starts_with(
        noema_artifacts::owner_artifacts_dir(home.path(), &owner).expect("artifact root")
    ));
    assert_eq!(
        artifact.artifact.metadata,
        serde_json::json!({"origin": "unit-test"})
    );
}

#[cfg(unix)]
#[tokio::test]
async fn conversation_local_file_artifact_rejects_symlinked_artifact_root() {
    let home = TempDir::new().expect("temp store root");
    let store = NoemaStore::open(&store_config(home.path()))
        .await
        .expect("open store");
    let conversation = store
        .create_conversation(noema_conversations::NewConversation::local_chat(None, None))
        .await
        .expect("conversation");
    let outside = home.path().join("outside-artifacts");
    tokio::fs::create_dir_all(&outside)
        .await
        .expect("outside dir");
    let owner = ArtifactOwnerRef::conversation(&conversation.conversation_id);
    let artifact_root =
        noema_artifacts::owner_artifacts_dir(home.path(), &owner).expect("artifact root");
    tokio::fs::create_dir_all(artifact_root.parent().expect("conversation directory"))
        .await
        .expect("conversation dir");
    std::os::unix::fs::symlink(&outside, artifact_root).expect("symlink artifact root");

    let artifact_metadata: ArtifactMetadataStoreHandle = Arc::new(store);
    let artifact_operations =
        LocalArtifactService::new(home.path(), artifact_metadata).expect("artifact operations");
    let error = artifact_operations
        .create_local_file(CreateLocalArtifactRequest {
            owner: ArtifactOwnerRef::conversation(&conversation.conversation_id),
            title: "Session report".to_string(),
            description: None,
            artifact_kind: "document".to_string(),
            filename: "report.md".to_string(),
            bytes: b"# report\n".to_vec(),
            media_type: Some("text/markdown".to_string()),
            created_by_actor_id: "agent:primary".to_string(),
            source: ArtifactSource::default(),
            metadata: serde_json::json!({}),
        })
        .await
        .expect_err("symlinked artifact root should be rejected");

    assert!(matches!(error, ArtifactOperationError::Filesystem { .. }));
}

#[derive(Debug)]
struct BarrierMetadataStore {
    store: NoemaStore,
    append_loaded: Arc<Barrier>,
}

impl ArtifactMetadataStore for BarrierMetadataStore {
    fn new_artifact_id(&self) -> String {
        self.store.new_artifact_id()
    }

    fn new_artifact_version_id(&self) -> String {
        self.store.new_artifact_version_id()
    }

    fn load_append_target<'a>(
        &'a self,
        artifact_id: &'a str,
    ) -> ArtifactFuture<'a, Option<ArtifactAppendTarget>> {
        Box::pin(async move {
            let target =
                ArtifactMetadataStore::load_append_target(&self.store, artifact_id).await?;
            self.append_loaded.wait().await;
            Ok(target)
        })
    }

    fn create_artifact_with_initial_version<'a>(
        &'a self,
        artifact: NewArtifact,
        initial_version: NewArtifactVersion,
    ) -> ArtifactFuture<'a, ArtifactWithVersions> {
        ArtifactMetadataStore::create_artifact_with_initial_version(
            &self.store,
            artifact,
            initial_version,
        )
    }

    fn append_artifact_version<'a>(
        &'a self,
        artifact_id: &'a str,
        expected_next_version_index: i64,
        version: NewArtifactVersion,
    ) -> ArtifactFuture<'a, ArtifactVersionRecord> {
        ArtifactMetadataStore::append_artifact_version(
            &self.store,
            artifact_id,
            expected_next_version_index,
            version,
        )
    }
}

#[tokio::test]
async fn conversation_append_race_has_one_winner_and_cleans_loser() {
    run_append_race(|store| {
        Box::pin(async move {
            let conversation = store
                .create_conversation(noema_conversations::NewConversation::local_chat(None, None))
                .await
                .expect("conversation");
            ArtifactOwnerRef::conversation(conversation.conversation_id)
        })
    })
    .await;
}

#[tokio::test]
async fn task_append_race_has_one_winner_and_cleans_loser() {
    run_append_race(|store| {
        Box::pin(async move {
            let (task, _) = crate::test_support::seed_task(store, "Artifact append race").await;
            ArtifactOwnerRef::task(task.task_id)
        })
    })
    .await;
}

async fn run_append_race<F>(owner: F)
where
    F: for<'a> FnOnce(
        &'a NoemaStore,
    ) -> Pin<Box<dyn Future<Output = ArtifactOwnerRef> + Send + 'a>>,
{
    let home = TempDir::new().expect("temp store root");
    let root = home.path();
    let config = store_config(root);
    let store_a = NoemaStore::open(&config).await.expect("first store");
    let store_b = NoemaStore::open(&config).await.expect("second store");
    let owner = owner(&store_a).await;
    let initial_metadata: ArtifactMetadataStoreHandle = Arc::new(store_a.clone());
    let initial_service =
        LocalArtifactService::new(root, initial_metadata).expect("initial artifact service");
    let artifact = initial_service
        .create_local_file(create_request(owner, b"initial"))
        .await
        .expect("initial artifact");

    let barrier = Arc::new(Barrier::new(2));
    let service_a = LocalArtifactService::new(
        root,
        Arc::new(BarrierMetadataStore {
            store: store_a.clone(),
            append_loaded: barrier.clone(),
        }),
    )
    .expect("first racing service");
    let service_b = LocalArtifactService::new(
        root,
        Arc::new(BarrierMetadataStore {
            store: store_b,
            append_loaded: barrier,
        }),
    )
    .expect("second racing service");
    let first_bytes = b"first contender".to_vec();
    let second_bytes = b"second contender".to_vec();
    let first = service_a.append_local_file_version(append_request(
        &artifact.artifact.artifact_id,
        first_bytes.clone(),
    ));
    let second = service_b.append_local_file_version(append_request(
        &artifact.artifact.artifact_id,
        second_bytes.clone(),
    ));

    let (first_result, second_result) = tokio::time::timeout(Duration::from_secs(5), async {
        tokio::join!(first, second)
    })
    .await
    .expect("append race completed");
    let (winner, winner_bytes, loser) = match (first_result, second_result) {
        (Ok(winner), Err(loser)) => (winner, first_bytes, loser),
        (Err(loser), Ok(winner)) => (winner, second_bytes, loser),
        results => panic!("expected one append winner and one loser: {results:?}"),
    };
    assert!(
        matches!(
            &loser,
            ArtifactOperationError::Metadata(
                noema_artifacts::ArtifactMetadataError::AppendConflict {
                    expected_next_version_index: 2,
                    actual_next_version_index: 3,
                    ..
                }
            )
        ),
        "unexpected loser error: {loser:?}"
    );

    let stored = store_a
        .get_artifact(&artifact.artifact.artifact_id)
        .await
        .expect("artifact query")
        .expect("artifact exists");
    assert_eq!(stored.versions.len(), 2);
    assert_eq!(
        stored.current_version.artifact_version_id,
        winner.artifact_version_id
    );
    assert_eq!(stored.current_version.content_sha256, winner.content_sha256);
    let content = service_a
        .read_local_file(ReadLocalArtifactRequest {
            artifact: stored.artifact.clone(),
            version: stored.current_version.clone(),
        })
        .await
        .expect("read winning bytes");
    assert_eq!(content.filename, "report.txt");
    assert_eq!(content.bytes, winner_bytes);

    let original_content = service_a
        .read_local_file(ReadLocalArtifactRequest {
            artifact: stored.artifact.clone(),
            version: stored.versions[0].clone(),
        })
        .await
        .expect("read original bytes");
    assert_eq!(original_content.filename, "report.txt");
    assert_eq!(original_content.bytes, b"initial");

    let ArtifactVersionStorage::LocalFile { relative_path } = &winner.storage else {
        panic!("winner must be local");
    };
    let winner_path = root.join(relative_path);
    let ArtifactVersionStorage::LocalFile {
        relative_path: original_relative_path,
    } = &stored.versions[0].storage
    else {
        panic!("original must be local");
    };
    let original_path = root.join(original_relative_path);
    assert_ne!(original_path, winner_path);
    let artifact_dir = noema_artifacts::artifact_version_dir(
        root,
        &stored.artifact.owner,
        &stored.artifact.artifact_id,
        1,
    )
    .expect("first version directory")
    .parent()
    .and_then(std::path::Path::parent)
    .expect("artifact directory")
    .to_path_buf();
    let mut expected_files = vec![original_path, winner_path.clone()];
    expected_files.sort();
    assert_eq!(regular_files(&artifact_dir), expected_files);

    let winner_operation_dir = winner_path.parent().expect("winner operation directory");
    let objects_dir = winner_operation_dir.parent().expect("objects directory");
    let object_entries = std::fs::read_dir(objects_dir)
        .expect("objects entries")
        .map(|entry| entry.expect("object entry").path())
        .collect::<Vec<_>>();
    assert_eq!(object_entries, vec![winner_operation_dir.to_path_buf()]);
    assert_staging_empty(root);
}

fn create_request(owner: ArtifactOwnerRef, bytes: &[u8]) -> CreateLocalArtifactRequest {
    CreateLocalArtifactRequest {
        owner,
        title: "Race report".to_string(),
        description: None,
        artifact_kind: "document".to_string(),
        filename: "report.txt".to_string(),
        bytes: bytes.to_vec(),
        media_type: Some("text/plain".to_string()),
        created_by_actor_id: "agent:test".to_string(),
        source: ArtifactSource::default(),
        metadata: serde_json::json!({}),
    }
}

fn append_request(artifact_id: &str, bytes: Vec<u8>) -> AppendLocalArtifactVersionRequest {
    AppendLocalArtifactVersionRequest {
        artifact_id: artifact_id.to_string(),
        title: None,
        filename: "report.txt".to_string(),
        bytes,
        media_type: Some("text/plain".to_string()),
        created_by_actor_id: "agent:test".to_string(),
        source: ArtifactSource::default(),
        metadata: serde_json::json!({}),
    }
}

fn regular_files(root: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();
    collect_regular_files(root, &mut files);
    files.sort();
    files
}

fn assert_staging_empty(root: &std::path::Path) {
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

fn collect_regular_files(directory: &std::path::Path, files: &mut Vec<std::path::PathBuf>) {
    for entry in std::fs::read_dir(directory)
        .expect("artifact directory")
        .flatten()
    {
        let path = entry.path();
        let metadata = std::fs::symlink_metadata(&path).expect("artifact entry metadata");
        if metadata.is_file() {
            files.push(path);
        } else if metadata.is_dir() && !metadata.file_type().is_symlink() {
            collect_regular_files(&path, files);
        }
    }
}
