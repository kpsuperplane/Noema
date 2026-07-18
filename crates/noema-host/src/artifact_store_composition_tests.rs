//! Artifact-filesystem composition tests backed by the SQLite store.

use std::{future::Future, pin::Pin, sync::Arc, time::Duration};

use noema_artifacts::{
    AppendLocalArtifactVersionRequest, ArtifactAppendTarget, ArtifactFuture, ArtifactMetadataStore,
    ArtifactMetadataStoreHandle, ArtifactOperationError, ArtifactOperations, ArtifactOwnerRef,
    ArtifactSource, ArtifactVersionRecord, ArtifactVersionStorage, ArtifactWithVersions,
    CreateLocalArtifactRequest, LocalArtifactService, NewArtifact, NewArtifactVersion,
    ReadLocalArtifactRequest,
};
use tempfile::TempDir;
use tokio::sync::Barrier;

use noema_store::{NoemaStore, StoreConfig};

fn store_config(root: &std::path::Path) -> StoreConfig {
    StoreConfig::new(root.join("db/noema.sqlite3"))
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
            let conversation_id = noema_store::test_support::create_local_conversation_id(store)
                .await
                .expect("conversation");
            ArtifactOwnerRef::conversation(conversation_id)
        })
    })
    .await;
}

#[tokio::test]
async fn task_append_race_has_one_winner_and_cleans_loser() {
    run_append_race(|store| Box::pin(async move { task_owner(store).await })).await;
}

async fn task_owner(store: &NoemaStore) -> ArtifactOwnerRef {
    let (task, _) = noema_store::test_support::seed_task(store, "Artifact append race")
        .await
        .expect("task");
    ArtifactOwnerRef::task(task.task_id)
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
    let content = service_a
        .read_local_file(ReadLocalArtifactRequest {
            artifact: stored.artifact.clone(),
            version: stored.current_version.clone(),
        })
        .await
        .expect("read winning bytes");
    assert_eq!(content.bytes, winner_bytes);

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
    let mut expected_files = vec![original_path, winner_path.clone()];
    expected_files.sort();
    let artifact_root =
        noema_artifacts::owner_artifacts_dir(root, &stored.artifact.owner).expect("artifact root");
    assert_eq!(regular_files(&artifact_root), expected_files);
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
    assert_eq!(
        std::fs::read_dir(root.join(".artifact-staging"))
            .expect("staging root")
            .count(),
        0,
        "staging root should be empty"
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
