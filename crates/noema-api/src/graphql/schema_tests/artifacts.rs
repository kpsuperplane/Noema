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
                  artifact(artifactId: "{}") {{
                    currentVersion {{ artifactVersionId downloadUrl }}
                  }}
                  artifactVersionDetail(artifactVersionId: "{}") {{
                    previewKind markdown plainText downloadUrl mediaType
                    versions {{ artifactVersionId versionIndex }}
                  }}
                }}"#,
                artifact.artifact.artifact_id, second.artifact_version_id,
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
        assert_eq!(data["artifact"]["currentVersion"]["artifactVersionId"], second.artifact_version_id);
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
        let artifact = store
            .create_artifact_with_initial_version(
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
                  artifact(artifactId: "{}") {{ artifactId }}
                }}"#,
                conversation.conversation_id, artifact.artifact.artifact_id,
            ))
            .await;
        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let data = response.data.into_json().expect("query json");
        assert_eq!(data["artifacts"], serde_json::json!([]));
        assert!(data["artifact"].is_null());

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
