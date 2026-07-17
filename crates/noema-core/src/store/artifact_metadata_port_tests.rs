//! SQLite artifact-metadata port tests.

use noema_artifacts::{
    ArtifactDomainError, ArtifactMetadataError, ArtifactMetadataStore, ArtifactSource,
    ArtifactVersionStorage, NewArtifactVersion,
};

#[tokio::test]
async fn metadata_port_rejects_non_positive_expected_index_as_domain_error() {
    let store = super::tests::test_store().await;
    let error = ArtifactMetadataStore::append_artifact_version(
        &store,
        "artifact:any",
        0,
        NewArtifactVersion {
            artifact_version_id: None,
            title: None,
            storage: ArtifactVersionStorage::LocalFile {
                relative_path: "unused".to_string(),
            },
            media_type: None,
            byte_size: None,
            content_sha256: None,
            created_by_actor_id: "agent:test".to_string(),
            source: ArtifactSource::default(),
            metadata: serde_json::json!({}),
        },
    )
    .await
    .expect_err("non-positive expected index must fail before persistence");

    assert_eq!(
        error,
        ArtifactMetadataError::Domain(ArtifactDomainError::InvalidVersionIndex {
            version_index: 0,
        })
    );
}
