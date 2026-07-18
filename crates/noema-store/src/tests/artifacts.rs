use super::test_store;

async fn assert_metadata_port_rejects_non_positive_index(store: &crate::NoemaStore) {
    use noema_artifacts::{
        ArtifactDomainError, ArtifactMetadataError, ArtifactMetadataStore, ArtifactSource,
        ArtifactVersionStorage, NewArtifactVersion,
    };

    let error = ArtifactMetadataStore::append_artifact_version(
        store,
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
    .expect_err("non-positive expected index");

    assert_eq!(
        error,
        ArtifactMetadataError::Domain(ArtifactDomainError::InvalidVersionIndex {
            version_index: 0
        })
    );
}

#[tokio::test]
async fn metadata_port_rejects_non_positive_expected_index_as_domain_error() {
    assert_metadata_port_rejects_non_positive_index(&test_store().await).await;
}

#[tokio::test]
async fn artifact_read_rejects_forged_non_http_external_url() {
    let store = test_store().await;
    let conversation = store
        .create_conversation(noema_conversations::NewConversation::local_chat(None, None))
        .await
        .expect("conversation");
    let created = seed_external_artifact(&store, &conversation.conversation_id).await;

    store
        .with_connection(|conn| {
            conn.execute(
                "UPDATE artifact_versions SET external_url = ?1 WHERE artifact_version_id = ?2",
                rusqlite::params![
                    "javascript:alert(1)",
                    created.current_version.artifact_version_id
                ],
            )
            .map(|_| ())
            .map_err(crate::StoreError::Sqlite)
        })
        .await
        .expect("forge external URL");

    let error = store
        .get_artifact(&created.artifact.artifact_id)
        .await
        .expect_err("forged external URL should be rejected on read");

    assert!(matches!(
        error,
        crate::StoreError::InvalidArtifactExternalUrl { .. }
    ));
}

#[tokio::test]
async fn artifact_owner_authorization_is_human_scoped_and_fail_closed() {
    let store = test_store().await;
    let local = store
        .create_conversation(noema_conversations::NewConversation::local_chat(None, None))
        .await
        .expect("local conversation");
    let mut foreign_input = noema_conversations::NewConversation::local_chat(None, None);
    foreign_input.owner = noema_conversations::ConversationOwnerRef::human("human:other")
        .expect("foreign human owner");
    foreign_input.primary_human_id = Some("human:other".to_string());
    let foreign = store
        .create_conversation(foreign_input)
        .await
        .expect("foreign conversation");

    assert!(
        store
            .artifact_owner_is_authorized_for_human(
                &noema_artifacts::ArtifactOwnerRef::conversation(local.conversation_id),
                "human:local",
            )
            .await
            .expect("local authorization")
    );
    assert!(
        !store
            .artifact_owner_is_authorized_for_human(
                &noema_artifacts::ArtifactOwnerRef::conversation(foreign.conversation_id),
                "human:local",
            )
            .await
            .expect("foreign authorization")
    );
    assert!(
        !store
            .artifact_owner_is_authorized_for_human(
                &noema_artifacts::ArtifactOwnerRef {
                    object_type: "unknown".to_string(),
                    object_id: "object:unknown".to_string(),
                },
                "human:local",
            )
            .await
            .expect("unknown owner authorization")
    );
}

async fn seed_external_artifact(
    store: &crate::NoemaStore,
    conversation_id: &str,
) -> noema_artifacts::ArtifactWithVersions {
    store
        .create_artifact_with_initial_version(
            noema_artifacts::NewArtifact {
                artifact_id: None,
                owner: noema_artifacts::ArtifactOwnerRef::conversation(conversation_id),
                title: "Sprint brief".to_string(),
                description: Some("Planning notes".to_string()),
                artifact_kind: "document".to_string(),
                storage_kind: noema_artifacts::ArtifactStorageKind::ExternalUrl,
                created_by_actor_id: "agent:primary".to_string(),
                source: noema_artifacts::ArtifactSource {
                    conversation_id: Some(conversation_id.to_string()),
                    turn_id: None,
                    item_id: None,
                },
                metadata: serde_json::json!({"provider": "notion"}),
            },
            noema_artifacts::NewArtifactVersion {
                artifact_version_id: None,
                title: Some("Initial".to_string()),
                storage: noema_artifacts::ArtifactVersionStorage::ExternalUrl {
                    url: "https://notion.so/noema-brief".to_string(),
                },
                media_type: Some("text/html".to_string()),
                byte_size: None,
                content_sha256: None,
                created_by_actor_id: "agent:primary".to_string(),
                source: noema_artifacts::ArtifactSource {
                    conversation_id: Some(conversation_id.to_string()),
                    turn_id: None,
                    item_id: None,
                },
                metadata: serde_json::json!({}),
            },
        )
        .await
        .expect("seed artifact")
}
