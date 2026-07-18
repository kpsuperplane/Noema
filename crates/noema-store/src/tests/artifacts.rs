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

#[tokio::test]
async fn task_artifact_access_uses_v3_workspace_membership() {
    let store = test_store().await;
    store
        .with_connection(|connection| {
            connection.execute(
                "INSERT INTO humans (human_id, display_name)
                 VALUES ('human:foreign', 'Foreign')",
                [],
            )?;
            connection.execute(
                "INSERT INTO tasks (
                    task_id, workspace_id, workflow_id, stage_id, title,
                    source_kind, created_by_actor_id
                 ) VALUES (
                    'task:artifact-owner', 'workspace:personal',
                    'workflow:personal:default', 'stage:personal:inbox',
                    'Artifact owner', 'system', 'actor:system'
                 )",
                [],
            )?;
            Ok(())
        })
        .await
        .expect("seed task owner");
    let created = store
        .create_artifact_with_initial_version(
            noema_artifacts::NewArtifact {
                artifact_id: Some("artifact:task-access".to_string()),
                owner: noema_artifacts::ArtifactOwnerRef::task("task:artifact-owner"),
                title: "Task output".to_string(),
                description: None,
                artifact_kind: "document".to_string(),
                storage_kind: noema_artifacts::ArtifactStorageKind::LocalFile,
                created_by_actor_id: "actor:system".to_string(),
                source: noema_artifacts::ArtifactSource::default(),
                metadata: serde_json::json!({}),
            },
            noema_artifacts::NewArtifactVersion {
                artifact_version_id: Some("artifact_version:task-access".to_string()),
                title: None,
                storage: noema_artifacts::ArtifactVersionStorage::LocalFile {
                    relative_path: "tasks/output.md".to_string(),
                },
                media_type: Some("text/markdown".to_string()),
                byte_size: Some(10),
                content_sha256: None,
                created_by_actor_id: "actor:system".to_string(),
                source: noema_artifacts::ArtifactSource::default(),
                metadata: serde_json::json!({}),
            },
        )
        .await
        .expect("create task artifact");
    assert!(
        store
            .artifact_owner_is_authorized_for_human(
                &noema_artifacts::ArtifactOwnerRef::task("task:artifact-owner"),
                "human:local",
            )
            .await
            .expect("member authorization")
    );
    assert!(
        !store
            .artifact_owner_is_authorized_for_human(
                &noema_artifacts::ArtifactOwnerRef::task("task:artifact-owner"),
                "human:foreign",
            )
            .await
            .expect("foreign authorization")
    );
    assert!(
        store
            .get_local_artifact_version_for_human(
                &created.current_version.artifact_version_id,
                "human:local",
            )
            .await
            .expect("member download authorization")
            .is_some()
    );
    assert!(
        store
            .get_local_artifact_version_for_human(
                &created.current_version.artifact_version_id,
                "human:foreign",
            )
            .await
            .expect("foreign download authorization")
            .is_none()
    );
}

#[tokio::test]
async fn task_artifact_connection_is_owner_scoped_paginated_and_current_version_hydrated() {
    let store = test_store().await;
    store
        .with_connection(|connection| {
            connection
                .execute(
                    "INSERT INTO tasks (
                        task_id, workspace_id, workflow_id, stage_id, title,
                        source_kind, created_by_actor_id
                     ) VALUES (
                        'task:artifact-connection', 'workspace:personal',
                        'workflow:personal:default', 'stage:personal:inbox',
                        'Artifact connection owner', 'system', 'actor:system'
                     )",
                    [],
                )
                .map_err(crate::StoreError::Sqlite)
        })
        .await
        .expect("seed task owner");
    for suffix in ["a", "b"] {
        store
            .create_artifact_with_initial_version(
                noema_artifacts::NewArtifact {
                    artifact_id: Some(format!("artifact:task-connection-{suffix}")),
                    owner: noema_artifacts::ArtifactOwnerRef::task("task:artifact-connection"),
                    title: format!("Artifact {suffix}"),
                    description: None,
                    artifact_kind: "document".to_string(),
                    storage_kind: noema_artifacts::ArtifactStorageKind::LocalFile,
                    created_by_actor_id: "actor:system".to_string(),
                    source: noema_artifacts::ArtifactSource::default(),
                    metadata: serde_json::json!({}),
                },
                noema_artifacts::NewArtifactVersion {
                    artifact_version_id: Some(format!(
                        "artifact_version:task-connection-{suffix}-1"
                    )),
                    title: None,
                    storage: noema_artifacts::ArtifactVersionStorage::LocalFile {
                        relative_path: format!("tasks/{suffix}.md"),
                    },
                    media_type: Some("text/markdown".to_string()),
                    byte_size: Some(1),
                    content_sha256: None,
                    created_by_actor_id: "actor:system".to_string(),
                    source: noema_artifacts::ArtifactSource::default(),
                    metadata: serde_json::json!({}),
                },
            )
            .await
            .expect("create task artifact");
    }
    let appended = store
        .append_artifact_version(
            "artifact:task-connection-a",
            noema_artifacts::NewArtifactVersion {
                artifact_version_id: Some("artifact_version:task-connection-a-2".to_string()),
                title: Some("Current".to_string()),
                storage: noema_artifacts::ArtifactVersionStorage::LocalFile {
                    relative_path: "tasks/a-current.md".to_string(),
                },
                media_type: Some("text/markdown".to_string()),
                byte_size: Some(2),
                content_sha256: None,
                created_by_actor_id: "actor:system".to_string(),
                source: noema_artifacts::ArtifactSource::default(),
                metadata: serde_json::json!({}),
            },
        )
        .await
        .expect("append current version");
    let task_id = noema_tasks::TaskId::new("task:artifact-connection").expect("task id");
    let first = store
        .list_work_task_artifacts(crate::WorkTaskArtifactQuery {
            task_id: task_id.clone(),
            first: crate::WorkPageSize::new(1).expect("page size"),
            after: None,
        })
        .await
        .expect("first artifact page");
    assert_eq!(first.edges.len(), 1);
    assert!(first.page_info.has_next_page);
    assert_eq!(first.edges[0].node.artifact.owner.object_type, "task");
    assert_eq!(
        first.edges[0].node.artifact.owner.object_id,
        task_id.as_str()
    );
    let cursor = crate::WorkTaskArtifactCursor::decode(
        first
            .page_info
            .end_cursor
            .as_deref()
            .expect("first page cursor"),
    )
    .expect("decode artifact cursor");
    let second = store
        .list_work_task_artifacts(crate::WorkTaskArtifactQuery {
            task_id,
            first: crate::WorkPageSize::new(1).expect("page size"),
            after: Some(cursor),
        })
        .await
        .expect("second artifact page");
    assert_eq!(second.edges.len(), 1);
    assert!(!second.page_info.has_next_page);
    assert_ne!(
        second.edges[0].node.artifact.artifact_id,
        first.edges[0].node.artifact.artifact_id
    );
    let current_a = first
        .edges
        .iter()
        .chain(second.edges.iter())
        .find(|edge| edge.node.artifact.artifact_id == "artifact:task-connection-a")
        .expect("artifact a is present across both pages");
    assert_eq!(
        current_a.node.current_version.artifact_version_id,
        appended.artifact_version_id
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
