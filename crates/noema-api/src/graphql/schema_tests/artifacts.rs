    #[tokio::test]
    async fn create_conversation_external_artifact_mutation_round_trips() {
        let store = crate::test_support::test_store().await;
        let conversation = store
            .create_conversation(noema_conversations::NewConversation::local_chat(None, None))
            .await
            .expect("conversation");
        let schema = build_schema(GraphqlState::for_tests_with_store(store));

        let response = schema
            .execute(async_graphql::Request::new(format!(
                r#"
                mutation {{
                  createConversationExternalArtifact(input: {{
                    conversationId: "{}"
                    title: "Noema notes"
                    artifactKind: "document"
                    externalUrl: "https://notion.so/noema-notes"
                    mediaType: "text/html"
                  }}) {{
                    artifactId
                    ownerObjectType
                    ownerObjectId
                    title
                    storageKind
                    currentVersion {{
                      versionIndex
                      externalUrl
                      downloadUrl
                    }}
                  }}
                }}
                "#,
                conversation.conversation_id
            )))
            .await;

        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let data = response.data.into_json().expect("json");
        let artifact = &data["createConversationExternalArtifact"];
        assert_eq!(artifact["ownerObjectType"], "conversation");
        assert_eq!(artifact["ownerObjectId"], conversation.conversation_id);
        assert_eq!(artifact["storageKind"], "EXTERNAL_URL");
        assert_eq!(artifact["currentVersion"]["versionIndex"], 1);
        assert_eq!(
            artifact["currentVersion"]["downloadUrl"],
            serde_json::Value::Null
        );
    }

    #[tokio::test]
    async fn create_conversation_external_artifact_rejects_non_http_url() {
        let store = crate::test_support::test_store().await;
        let conversation = store
            .create_conversation(noema_conversations::NewConversation::local_chat(None, None))
            .await
            .expect("conversation");
        let schema = build_schema(GraphqlState::for_tests_with_store(store));

        let response = schema
            .execute(async_graphql::Request::new(format!(
                r#"
                mutation {{
                  createConversationExternalArtifact(input: {{
                    conversationId: "{}"
                    title: "Bad"
                    artifactKind: "document"
                    externalUrl: "file:///tmp/secret.txt"
                  }}) {{
                    artifactId
                  }}
                }}
                "#,
                conversation.conversation_id
            )))
            .await;

        assert!(!response.errors.is_empty());
        assert!(response.errors[0].message.contains("HTTP"));
    }
    #[tokio::test]
    async fn artifacts_query_resolves_owner_scoped_artifacts_and_field_names() {
        let home = tempfile::TempDir::new().expect("home");
        let paths = TestEnvironment::from_root(home.path()).expect("paths");
        let store = crate::test_support::test_store_for_environment(&paths).await;
        let conversation = store
            .create_conversation(noema_conversations::NewConversation::local_chat(None, None))
            .await
            .expect("conversation");
        let other_conversation = store
            .create_conversation(noema_conversations::NewConversation::local_chat(None, None))
            .await
            .expect("other conversation");

        let expected = store
            .create_artifact_with_initial_version(
                noema_artifacts::NewArtifact {
                    artifact_id: None,
                    owner: noema_artifacts::ArtifactOwnerRef::conversation(
                        &conversation.conversation_id,
                    ),
                    title: "Sprint brief".to_string(),
                    description: Some("Planning notes".to_string()),
                    artifact_kind: "document".to_string(),
                    storage_kind: noema_artifacts::ArtifactStorageKind::ExternalUrl,
                    created_by_actor_id: "agent:primary".to_string(),
                    source: noema_artifacts::ArtifactSource {
                        conversation_id: Some(conversation.conversation_id.clone()),
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
                        conversation_id: Some(conversation.conversation_id.clone()),
                        turn_id: None,
                        item_id: None,
                    },
                    metadata: serde_json::json!({}),
                },
            )
            .await
            .expect("artifact");
        let other_artifact = store
            .create_artifact_with_initial_version(
                noema_artifacts::NewArtifact {
                    artifact_id: None,
                    owner: noema_artifacts::ArtifactOwnerRef::conversation(
                        &other_conversation.conversation_id,
                    ),
                    title: "Other brief".to_string(),
                    description: None,
                    artifact_kind: "document".to_string(),
                    storage_kind: noema_artifacts::ArtifactStorageKind::ExternalUrl,
                    created_by_actor_id: "agent:primary".to_string(),
                    source: noema_artifacts::ArtifactSource {
                        conversation_id: Some(other_conversation.conversation_id.clone()),
                        turn_id: None,
                        item_id: None,
                    },
                    metadata: serde_json::json!({}),
                },
                noema_artifacts::NewArtifactVersion {
                    artifact_version_id: None,
                    title: Some("Initial".to_string()),
                    storage: noema_artifacts::ArtifactVersionStorage::ExternalUrl {
                        url: "https://example.com/other".to_string(),
                    },
                    media_type: Some("text/html".to_string()),
                    byte_size: None,
                    content_sha256: None,
                    created_by_actor_id: "agent:primary".to_string(),
                    source: noema_artifacts::ArtifactSource {
                        conversation_id: Some(other_conversation.conversation_id.clone()),
                        turn_id: None,
                        item_id: None,
                    },
                    metadata: serde_json::json!({}),
                },
            )
            .await
            .expect("other artifact");
        let schema = build_schema(GraphqlState::for_tests_with_store_and_environment(store, paths));

        let response = schema
            .execute(async_graphql::Request::new(format!(
                r#"
                {{
                  artifacts(ownerObjectType: "conversation", ownerObjectId: "{}", limit: 10) {{
                    artifactId
                    ownerObjectType
                    ownerObjectId
                    title
                    description
                    artifactKind
                    storageKind
                    currentVersion {{
                      artifactVersionId
                      versionIndex
                      externalUrl
                      downloadUrl
                      mediaType
                    }}
                  }}
                }}
                "#,
                conversation.conversation_id
            )))
            .await;

        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let data = response.data.into_json().expect("json");
        let artifacts = data["artifacts"].as_array().expect("artifacts array");
        assert_eq!(artifacts.len(), 1);

        let artifact = &artifacts[0];
        assert_eq!(artifact["artifactId"], expected.artifact.artifact_id);
        assert_eq!(artifact["ownerObjectType"], "conversation");
        assert_eq!(artifact["ownerObjectId"], conversation.conversation_id);
        assert_eq!(artifact["title"], "Sprint brief");
        assert_eq!(artifact["description"], "Planning notes");
        assert_eq!(artifact["artifactKind"], "document");
        assert_eq!(artifact["storageKind"], "EXTERNAL_URL");
        assert_eq!(
            artifact["currentVersion"]["artifactVersionId"],
            expected.current_version.artifact_version_id
        );
        assert_eq!(artifact["currentVersion"]["versionIndex"], 1);
        assert_eq!(
            artifact["currentVersion"]["externalUrl"],
            "https://notion.so/noema-brief"
        );
        assert_eq!(
            artifact["currentVersion"]["downloadUrl"],
            serde_json::Value::Null
        );
        assert_eq!(artifact["currentVersion"]["mediaType"], "text/html");
        assert_ne!(artifact["artifactId"], other_artifact.artifact.artifact_id);
    }

    #[tokio::test]
    async fn artifact_query_resolves_one_artifact() {
        let store = crate::test_support::test_store().await;
        let conversation = store
            .create_conversation(noema_conversations::NewConversation::local_chat(None, None))
            .await
            .expect("conversation");
        let expected = store
            .create_artifact_with_initial_version(
                noema_artifacts::NewArtifact {
                    artifact_id: None,
                    owner: noema_artifacts::ArtifactOwnerRef::conversation(
                        &conversation.conversation_id,
                    ),
                    title: "Sprint brief".to_string(),
                    description: Some("Planning notes".to_string()),
                    artifact_kind: "document".to_string(),
                    storage_kind: noema_artifacts::ArtifactStorageKind::ExternalUrl,
                    created_by_actor_id: "agent:primary".to_string(),
                    source: noema_artifacts::ArtifactSource {
                        conversation_id: Some(conversation.conversation_id.clone()),
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
                        conversation_id: Some(conversation.conversation_id.clone()),
                        turn_id: None,
                        item_id: None,
                    },
                    metadata: serde_json::json!({}),
                },
            )
            .await
            .expect("artifact");
        let schema = build_schema(GraphqlState::for_tests_with_store(store));

        let response = schema
            .execute(async_graphql::Request::new(format!(
                r#"
                {{
                  artifact(artifactId: "{}") {{
                    artifactId
                    ownerObjectType
                    ownerObjectId
                    title
                    description
                    artifactKind
                    storageKind
                    currentVersion {{
                      artifactVersionId
                      versionIndex
                      externalUrl
                      downloadUrl
                      mediaType
                    }}
                  }}
                }}
                "#,
                expected.artifact.artifact_id
            )))
            .await;

        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let data = response.data.into_json().expect("json");
        let artifact = data["artifact"].as_object().expect("artifact object");
        assert_eq!(artifact["artifactId"], expected.artifact.artifact_id);
        assert_eq!(artifact["ownerObjectType"], "conversation");
        assert_eq!(artifact["ownerObjectId"], conversation.conversation_id);
        assert_eq!(artifact["title"], "Sprint brief");
        assert_eq!(artifact["description"], "Planning notes");
        assert_eq!(artifact["artifactKind"], "document");
        assert_eq!(artifact["storageKind"], "EXTERNAL_URL");
        assert_eq!(
            artifact["currentVersion"]["artifactVersionId"],
            expected.current_version.artifact_version_id
        );
        assert_eq!(artifact["currentVersion"]["versionIndex"], 1);
        assert_eq!(
            artifact["currentVersion"]["externalUrl"],
            "https://notion.so/noema-brief"
        );
        assert_eq!(
            artifact["currentVersion"]["downloadUrl"],
            serde_json::Value::Null
        );
        assert_eq!(artifact["currentVersion"]["mediaType"], "text/html");
    }

    #[tokio::test]
    async fn artifact_query_exposes_local_file_download_url() {
        let home = tempfile::TempDir::new().expect("home");
        let paths = TestEnvironment::from_root(home.path()).expect("paths");
        let store = crate::test_support::test_store_for_environment(&paths).await;
        let conversation = store
            .create_conversation(noema_conversations::NewConversation::local_chat(None, None))
            .await
            .expect("conversation");
        let artifact_operations =
            crate::test_support::artifact_operations_for_environment(&store, &paths)
                .expect("artifact operations");
        let artifact = artifact_operations
            .create_local_file(noema_artifacts::CreateLocalArtifactRequest {
                owner: noema_artifacts::ArtifactOwnerRef::conversation(
                    &conversation.conversation_id,
                ),
                title: "Session report".to_string(),
                description: Some("Local markdown artifact".to_string()),
                artifact_kind: "document".to_string(),
                filename: "report.md".to_string(),
                bytes: b"# report\n".to_vec(),
                media_type: Some("text/markdown".to_string()),
                created_by_actor_id: "agent:primary".to_string(),
                source: noema_artifacts::ArtifactSource {
                    conversation_id: Some(conversation.conversation_id.clone()),
                    turn_id: None,
                    item_id: None,
                },
                metadata: serde_json::json!({"origin": "unit-test"}),
            })
            .await
            .expect("artifact");
        let schema = build_schema(GraphqlState::for_tests_with_store_and_environment(store, paths));

        let response = schema
            .execute(async_graphql::Request::new(format!(
                r#"
                {{
                  artifact(artifactId: "{}") {{
                    artifactId
                    currentVersion {{
                      artifactVersionId
                      downloadUrl
                    }}
                  }}
                }}
                "#,
                artifact.artifact.artifact_id
            )))
            .await;

        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let data = response.data.into_json().expect("json");
        let queried_artifact = data["artifact"].as_object().expect("artifact object");
        assert_eq!(
            queried_artifact["artifactId"],
            artifact.artifact.artifact_id
        );
        assert_eq!(
            queried_artifact["currentVersion"]["artifactVersionId"],
            artifact.current_version.artifact_version_id
        );
        assert_eq!(
            queried_artifact["currentVersion"]["downloadUrl"],
            noema_artifacts::artifact_download_url(&artifact.current_version.artifact_version_id)
        );
    }

    #[tokio::test]
    async fn artifact_version_detail_reads_markdown_content() {
        let home = tempfile::TempDir::new().expect("home");
        let paths = TestEnvironment::from_root(home.path()).expect("paths");
        let store = crate::test_support::test_store_for_environment(&paths).await;
        let conversation = store
            .create_conversation(noema_conversations::NewConversation::local_chat(None, None))
            .await
            .expect("conversation");
        let artifact_operations =
            crate::test_support::artifact_operations_for_environment(&store, &paths)
                .expect("artifact operations");
        let artifact = artifact_operations
            .create_local_file(noema_artifacts::CreateLocalArtifactRequest {
                owner: noema_artifacts::ArtifactOwnerRef::conversation(
                    conversation.conversation_id,
                ),
                title: "Session report".to_string(),
                description: Some("Local markdown artifact".to_string()),
                artifact_kind: "document".to_string(),
                filename: "report.md".to_string(),
                bytes: b"# Report\n\nA useful note.\n".to_vec(),
                media_type: Some("text/markdown".to_string()),
                created_by_actor_id: "agent:primary".to_string(),
                source: noema_artifacts::ArtifactSource::default(),
                metadata: serde_json::json!({}),
            })
            .await
            .expect("artifact");
        let second_version = artifact_operations
            .append_local_file_version(noema_artifacts::AppendLocalArtifactVersionRequest {
                artifact_id: artifact.artifact.artifact_id.clone(),
                title: Some("Second draft".to_string()),
                filename: "report-v2.md".to_string(),
                bytes: b"# Report\n\nA sharper note.\n".to_vec(),
                media_type: Some("text/markdown".to_string()),
                created_by_actor_id: "agent:primary".to_string(),
                source: noema_artifacts::ArtifactSource::default(),
                metadata: serde_json::json!({}),
            })
            .await
            .expect("second version");
        let schema = build_schema(GraphqlState::for_tests_with_store_and_environment(store, paths));

        let response = schema
            .execute(async_graphql::Request::new(format!(
                r#"
                {{
                  artifactVersionDetail(artifactVersionId: "{}") {{
                    artifactVersionId
                    artifactId
                    versionIndex
                    title
                    artifactKind
                    storageKind
                    mediaType
                    previewKind
                    markdown
                    plainText
                    downloadUrl
                    externalUrl
                    versions {{
                      artifactVersionId
                      versionIndex
                      downloadUrl
                      mediaType
                    }}
                  }}
                }}
                "#,
                second_version.artifact_version_id
            )))
            .await;

        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let data = response.data.into_json().expect("json");
        let detail = data["artifactVersionDetail"]
            .as_object()
            .expect("detail object");
        assert_eq!(
            detail["artifactVersionId"],
            second_version.artifact_version_id
        );
        assert_eq!(detail["artifactId"], artifact.artifact.artifact_id);
        assert_eq!(detail["versionIndex"], 2);
        assert_eq!(detail["title"], "Second draft");
        assert_eq!(detail["artifactKind"], "document");
        assert_eq!(detail["storageKind"], "LOCAL_FILE");
        assert_eq!(detail["mediaType"], "text/markdown");
        assert_eq!(detail["previewKind"], "MARKDOWN");
        assert_eq!(detail["markdown"], "# Report\n\nA sharper note.\n");
        assert!(detail["plainText"].is_null());
        assert_eq!(
            detail["downloadUrl"],
            noema_artifacts::artifact_download_url(&second_version.artifact_version_id)
        );
        assert!(detail["externalUrl"].is_null());
        let versions = detail["versions"].as_array().expect("versions");
        assert_eq!(versions.len(), 2);
        assert_eq!(versions[0]["versionIndex"], 1);
        assert_eq!(versions[1]["versionIndex"], 2);
        assert_eq!(
            versions[0]["artifactVersionId"],
            artifact.current_version.artifact_version_id
        );
        assert_eq!(
            versions[1]["artifactVersionId"],
            second_version.artifact_version_id
        );
    }

    #[tokio::test]
    async fn artifact_version_detail_previews_plain_text_literally() {
        let home = tempfile::TempDir::new().expect("home");
        let paths = TestEnvironment::from_root(home.path()).expect("paths");
        let store = crate::test_support::test_store_for_environment(&paths).await;
        let conversation = store
            .create_conversation(noema_conversations::NewConversation::local_chat(None, None))
            .await
            .expect("conversation");
        let content = "# Literal heading\n\n* literal asterisk\n  indented\n";
        let artifact_operations =
            crate::test_support::artifact_operations_for_environment(&store, &paths)
                .expect("artifact operations");
        let artifact = artifact_operations
            .create_local_file(noema_artifacts::CreateLocalArtifactRequest {
                owner: noema_artifacts::ArtifactOwnerRef::conversation(
                    conversation.conversation_id,
                ),
                title: "Notes".to_string(),
                description: None,
                artifact_kind: "document".to_string(),
                filename: "notes.txt".to_string(),
                bytes: content.as_bytes().to_vec(),
                media_type: Some("text/plain; charset=utf-8".to_string()),
                created_by_actor_id: "agent:primary".to_string(),
                source: noema_artifacts::ArtifactSource::default(),
                metadata: serde_json::json!({}),
            })
            .await
            .expect("artifact");
        let schema = build_schema(GraphqlState::for_tests_with_store_and_environment(store, paths));

        let response = schema
            .execute(async_graphql::Request::new(format!(
                r#"
                {{
                  artifactVersionDetail(artifactVersionId: "{}") {{
                    previewKind
                    markdown
                    plainText
                    downloadUrl
                  }}
                }}
                "#,
                artifact.current_version.artifact_version_id
            )))
            .await;

        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let data = response.data.into_json().expect("json");
        let detail = data["artifactVersionDetail"]
            .as_object()
            .expect("detail object");
        assert_eq!(detail["previewKind"], "PLAIN_TEXT");
        assert!(detail["markdown"].is_null());
        assert_eq!(detail["plainText"], content);
        assert_eq!(
            detail["downloadUrl"],
            noema_artifacts::artifact_download_url(&artifact.current_version.artifact_version_id)
        );
    }
    #[tokio::test]
    async fn artifact_version_detail_marks_non_markdown_local_file_unsupported() {
        let home = tempfile::TempDir::new().expect("home");
        let paths = TestEnvironment::from_root(home.path()).expect("paths");
        let store = crate::test_support::test_store_for_environment(&paths).await;
        let conversation = store
            .create_conversation(noema_conversations::NewConversation::local_chat(None, None))
            .await
            .expect("conversation");
        let artifact_operations =
            crate::test_support::artifact_operations_for_environment(&store, &paths)
                .expect("artifact operations");
        let artifact = artifact_operations
            .create_local_file(noema_artifacts::CreateLocalArtifactRequest {
                owner: noema_artifacts::ArtifactOwnerRef::conversation(
                    conversation.conversation_id,
                ),
                title: "Data export".to_string(),
                description: None,
                artifact_kind: "table".to_string(),
                filename: "data.csv".to_string(),
                bytes: b"a,b\n1,2\n".to_vec(),
                media_type: Some("text/csv".to_string()),
                created_by_actor_id: "agent:primary".to_string(),
                source: noema_artifacts::ArtifactSource::default(),
                metadata: serde_json::json!({}),
            })
            .await
            .expect("artifact");
        let schema = build_schema(GraphqlState::for_tests_with_store_and_environment(store, paths));

        let response = schema
            .execute(async_graphql::Request::new(format!(
                r#"
                {{
                  artifactVersionDetail(artifactVersionId: "{}") {{
                    previewKind
                    markdown
                    plainText
                    downloadUrl
                    mediaType
                  }}
                }}
                "#,
                artifact.current_version.artifact_version_id
            )))
            .await;

        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let data = response.data.into_json().expect("json");
        let detail = data["artifactVersionDetail"]
            .as_object()
            .expect("detail object");
        assert_eq!(detail["previewKind"], "UNSUPPORTED");
        assert!(detail["markdown"].is_null());
        assert!(detail["plainText"].is_null());
        assert_eq!(detail["mediaType"], "text/csv");
        assert_eq!(
            detail["downloadUrl"],
            noema_artifacts::artifact_download_url(&artifact.current_version.artifact_version_id)
        );
    }

    #[tokio::test]
    async fn artifact_operations_hide_foreign_human_resources() {
        let home = tempfile::TempDir::new().expect("home");
        let paths = TestEnvironment::from_root(home.path()).expect("paths");
        let store = crate::test_support::test_store_for_environment(&paths).await;
        let conversation = store
            .create_conversation(conversation_for_human("human:other"))
            .await
            .expect("foreign conversation");
        let artifact_operations =
            crate::test_support::artifact_operations_for_environment(&store, &paths)
                .expect("artifact operations");
        let artifact = artifact_operations
            .create_local_file(noema_artifacts::CreateLocalArtifactRequest {
                owner: noema_artifacts::ArtifactOwnerRef::conversation(
                    &conversation.conversation_id,
                ),
                title: "Foreign notes".to_string(),
                description: None,
                artifact_kind: "document".to_string(),
                filename: "foreign.md".to_string(),
                bytes: b"# private".to_vec(),
                media_type: Some("text/markdown".to_string()),
                created_by_actor_id: "human:other".to_string(),
                source: noema_artifacts::ArtifactSource::default(),
                metadata: serde_json::json!({}),
            })
            .await
            .expect("foreign artifact");
        let schema = build_schema(GraphqlState::for_tests_with_store_and_environment(
            store, paths,
        ));

        let query_response = schema
            .execute(async_graphql::Request::new(format!(
                r#"
                query {{
                  artifacts(
                    ownerObjectType: "conversation"
                    ownerObjectId: "{}"
                  ) {{
                    artifactId
                  }}
                  artifact(artifactId: "{}") {{
                    artifactId
                  }}
                  artifactVersionDetail(artifactVersionId: "{}") {{
                    artifactVersionId
                    markdown
                  }}
                }}
                "#,
                conversation.conversation_id,
                artifact.artifact.artifact_id,
                artifact.current_version.artifact_version_id,
            )))
            .await;

        assert!(
            query_response.errors.is_empty(),
            "{:?}",
            query_response.errors
        );
        let data = query_response.data.into_json().expect("query json");
        assert_eq!(data["artifacts"], serde_json::json!([]));
        assert!(data["artifact"].is_null());
        assert!(data["artifactVersionDetail"].is_null());

        let mutation_response = schema
            .execute(async_graphql::Request::new(format!(
                r#"
                mutation {{
                  createConversationExternalArtifact(input: {{
                    conversationId: "{}"
                    title: "Injected"
                    artifactKind: "document"
                    externalUrl: "https://example.com/injected"
                  }}) {{
                    artifactId
                  }}
                }}
                "#,
                conversation.conversation_id,
            )))
            .await;
        assert_eq!(mutation_response.errors.len(), 1);
        assert!(
            mutation_response.errors[0]
                .message
                .contains("conversation is unavailable")
        );
    }
