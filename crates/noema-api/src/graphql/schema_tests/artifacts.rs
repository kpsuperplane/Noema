    use noema_artifacts::ArtifactMetadataStore;

    #[tokio::test]
    async fn local_artifact_queries_expose_versions_downloads_and_markdown() {
        let home = tempfile::TempDir::new().expect("home");
        let environment = TestEnvironment::from_root(home.path()).expect("environment");
        let store = crate::test_support::test_store_for_environment(&environment).await;
        let conversation = store
            .create_conversation(noema_conversations::NewConversation::local_chat(None, None))
            .await
            .expect("conversation");
        let operations =
            crate::test_support::artifact_operations_for_environment(&store, &environment)
                .expect("artifact operations");
        let artifact = operations
            .create_local_file(noema_artifacts::CreateLocalArtifactRequest {
                owner: noema_artifacts::ArtifactOwnerRef::conversation(
                    &conversation.conversation_id,
                ),
                title: "Session report".to_string(),
                description: None,
                artifact_kind: "document".to_string(),
                filename: "report.md".to_string(),
                bytes: b"# first\n".to_vec(),
                media_type: Some("text/markdown".to_string()),
                created_by_actor_id: "agent:primary".to_string(),
                source: noema_artifacts::ArtifactSource::default(),
                metadata: serde_json::json!({}),
            })
            .await
            .expect("artifact");
        let second = operations
            .append_local_file_version(noema_artifacts::AppendLocalArtifactVersionRequest {
                artifact_id: artifact.artifact.artifact_id.clone(),
                title: Some("Revision".to_string()),
                filename: "report.md".to_string(),
                bytes: b"# second\n".to_vec(),
                media_type: Some("text/markdown".to_string()),
                created_by_actor_id: "agent:primary".to_string(),
                source: noema_artifacts::ArtifactSource::default(),
                metadata: serde_json::json!({}),
            })
            .await
            .expect("second version");
        let schema = build_schema(GraphqlState::for_tests_with_store_and_environment(
            store,
            environment,
        ));

        let response = schema
            .execute(format!(
                r#"{{
                  artifactVersionDetail(artifactVersionId: "{}") {{
                    previewKind markdown plainText downloadUrl mediaType
                    versions {{ artifactVersionId versionIndex }}
                  }}
                }}"#,
                second.artifact_version_id,
            ))
            .await;
        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let data = response.data.into_json().expect("json");
        assert_eq!(
            data["artifactVersionDetail"]["versions"]
                .as_array()
                .map(Vec::len),
            Some(2)
        );
        assert_eq!(data["artifactVersionDetail"]["previewKind"], "MARKDOWN");
        assert_eq!(data["artifactVersionDetail"]["markdown"], "# second\n");
        assert!(data["artifactVersionDetail"]["plainText"].is_null());
        assert_eq!(
            data["artifactVersionDetail"]["downloadUrl"],
            noema_artifacts::artifact_download_url(&second.artifact_version_id)
        );
    }

    #[tokio::test]
    async fn artifact_operations_hide_foreign_human_resources() {
        let store = crate::test_support::test_store().await;
        let conversation = store
            .create_conversation(conversation_for_human("human:other"))
            .await
            .expect("foreign conversation");
        ArtifactMetadataStore::create_artifact_with_initial_version(
            &store,
            noema_artifacts::NewArtifact {
                artifact_id: None,
                owner: noema_artifacts::ArtifactOwnerRef::conversation(
                    &conversation.conversation_id,
                ),
                title: "Foreign notes".to_string(),
                description: None,
                artifact_kind: "document".to_string(),
                storage_kind: noema_artifacts::ArtifactStorageKind::ExternalUrl,
                created_by_actor_id: "human:other".to_string(),
                source: noema_artifacts::ArtifactSource::default(),
                metadata: serde_json::json!({}),
            },
            noema_artifacts::NewArtifactVersion {
                artifact_version_id: None,
                title: None,
                storage: noema_artifacts::ArtifactVersionStorage::ExternalUrl {
                    url: "https://example.com/foreign".to_string(),
                },
                media_type: Some("text/html".to_string()),
                byte_size: None,
                content_sha256: None,
                created_by_actor_id: "human:other".to_string(),
                source: noema_artifacts::ArtifactSource::default(),
                metadata: serde_json::json!({}),
            },
        )
        .await
        .expect("foreign artifact");
        let schema = build_schema(GraphqlState::for_tests_with_store(store));

        let response = schema
            .execute(format!(
                r#"query {{
                  artifacts(ownerObjectType: "conversation", ownerObjectId: "{}") {{ artifactId }}
                }}"#,
                conversation.conversation_id,
            ))
            .await;
        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let data = response.data.into_json().expect("query json");
        assert_eq!(data["artifacts"], serde_json::json!([]));

        let response = schema
            .execute(format!(
                r#"mutation {{
                  createConversationExternalArtifact(input: {{
                    conversationId: "{}", title: "Injected", artifactKind: "document",
                    externalUrl: "https://example.com/injected"
                  }}) {{ artifactId }}
                }}"#,
                conversation.conversation_id,
            ))
            .await;
        assert_single_graphql_error(&response, "conversation is unavailable");
    }

    #[tokio::test]
    async fn inbox_task_upload_preserves_private_source_receipt() {
        use base64::{Engine as _, engine::general_purpose::STANDARD};

        let home = tempfile::TempDir::new().expect("home");
        let environment = TestEnvironment::from_root(home.path()).expect("environment");
        let store = crate::test_support::test_store_for_environment(&environment).await;
        let schema = build_schema(GraphqlState::for_tests_with_store_and_environment(
            store.clone(),
            environment,
        ));
        let capture = schema
            .execute(
                r#"mutation {
                  captureTask(input: {
                    workspaceId: "workspace:personal"
                    title: "Private packet"
                    clientMutationId: "capture-private-packet"
                  }) { task { taskId revision generation } }
                }"#,
            )
            .await;
        assert!(capture.errors.is_empty(), "{:?}", capture.errors);
        let capture = capture.data.into_json().expect("capture JSON");
        let task = &capture["captureTask"]["task"];
        let task_id = task["taskId"].as_str().expect("task ID");
        let content = STANDARD.encode(b"item,amount\nTransit,12.50\n");
        let upload = schema
            .execute(format!(
                r#"mutation {{
                  createTaskLocalArtifact(input: {{
                    taskId: "{task_id}"
                    expectedRevision: {}
                    expectedGeneration: {}
                    title: "August statement"
                    filename: "statement.csv"
                    mediaType: "text/csv"
                    contentBase64: "{content}"
                    sourceId: "statement-2026-08"
                    sourceVersion: "download-1"
                    sourceOwner: "Kevin"
                    disclosureScope: "private to Kevin and this Task"
                  }}) {{
                    artifact {{ artifactId ownerObjectType ownerObjectId currentVersion {{ contentSha256 }} }}
                    sourceId sourceVersion sourceOwner disclosureScope contentSha256 byteSize
                  }}
                }}"#,
                task["revision"].as_i64().expect("revision"),
                task["generation"].as_i64().expect("generation"),
            ))
            .await;
        assert!(upload.errors.is_empty(), "{:?}", upload.errors);
        let upload = upload.data.into_json().expect("upload JSON");
        let receipt = &upload["createTaskLocalArtifact"];
        assert_eq!(receipt["artifact"]["ownerObjectType"], "task");
        assert_eq!(receipt["artifact"]["ownerObjectId"], task_id);
        assert_eq!(receipt["sourceVersion"], "download-1");
        assert_eq!(receipt["disclosureScope"], "private to Kevin and this Task");
        assert_eq!(receipt["byteSize"], 26);
        assert_eq!(
            receipt["contentSha256"],
            receipt["artifact"]["currentVersion"]["contentSha256"]
        );
        let artifact_id = receipt["artifact"]["artifactId"]
            .as_str()
            .expect("artifact ID");
        let stored = store
            .get_artifact(artifact_id)
            .await
            .expect("artifact read")
            .expect("artifact");
        assert_eq!(stored.artifact.metadata["source_id"], "statement-2026-08");
        assert_eq!(stored.artifact.metadata["source_owner"], "Kevin");
    }
