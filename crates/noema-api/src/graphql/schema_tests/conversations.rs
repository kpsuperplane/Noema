    #[tokio::test]
    async fn start_primary_conversation_uses_saved_agent_provider_preference() {
        use crate::test_support::test_store;

        let store = test_store().await;
        store.ensure_default_actors().await.expect("actors");
        store
            .update_agent_display_name("agent:primary", "Noema")
            .await
            .expect("name primary");
        let foundation = store
            .ensure_default_foundation_local_provider_account()
            .await
            .expect("foundation account");
        store
            .update_provider_account_status(
                &foundation.provider_account_id,
                noema_providers::ProviderAccountStatus::Authenticated,
                None,
                None,
            )
            .await
            .expect("authenticated foundation account");
        let ready_selection = crate::test_support::ready_provider_selection(
            noema_providers::ProviderSelectionSnapshot::explicit(
                "foundation_local",
                &foundation.provider_account_id,
                "default",
                None,
                Some("primary_conversation_test".to_string()),
            ),
        );
        store
            .upsert_agent_runtime_preference_with_ready_selection(
                noema_store::NewAgentRuntimePreference {
                    agent_id: "agent:primary".to_string(),
                    provider_kind: "foundation_local".to_string(),
                    provider_account_id: foundation.provider_account_id,
                    model_profile: "default".to_string(),
                    reasoning_effort: None,
                },
                &ready_selection,
            )
            .await
            .expect("preference");

        let codex_provider = noema_providers::erase_model_provider(AutofillTestProvider {
            text: "codex".to_string(),
            tool_classification_model: None,
            requests: Arc::new(Mutex::new(Vec::new())),
        });
        let foundation_provider = noema_providers::erase_model_provider(AutofillTestProvider {
            text: "foundation".to_string(),
            tool_classification_model: None,
            requests: Arc::new(Mutex::new(Vec::new())),
        });
        let runtime = crate::test_support::spawn_runtime_with_provider_map(
            "codex",
            vec![
                ("codex".to_string(), codex_provider),
                ("foundation_local".to_string(), foundation_provider),
            ],
            store.clone(),
        )
        .await
        .expect("runtime");
        let schema = build_schema(GraphqlState::for_tests_with_store_and_runtime(
            store.clone(),
            runtime,
        ));

        let response = schema
            .execute(async_graphql::Request::new(
                r#"
                mutation {
                  ensurePrimaryConversation {
                    provider
                    conversationId
                  }
                }
                "#,
            ))
            .await;

        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let data = response.data.into_json().expect("json");
        assert_eq!(
            data["ensurePrimaryConversation"]["provider"],
            "foundation_local"
        );
    }

    #[tokio::test]
    async fn primary_conversation_returns_identity_without_transcript() {
        use crate::test_support::test_store;

        let store = test_store().await;
        let conversation = store
            .get_or_create_primary_conversation_for_provider(
                "human:local",
                "codex",
                Some("gpt-test".to_string()),
                None,
            )
            .await
            .expect("primary conversation");
        let runtime = test_autofill_runtime(store.clone(), "ok").await;
        let schema = build_schema(GraphqlState::for_tests_with_store_and_runtime(
            store, runtime,
        ));

        let response = schema
            .execute(async_graphql::Request::new(
                r#"
                query {
                  primaryConversation {
                    provider
                    conversationId
                  }
                }
                "#,
            ))
            .await;

        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let data = response.data.into_json().expect("json");
        assert_eq!(
            data["primaryConversation"]["conversationId"],
            conversation.conversation_id
        );
        assert_eq!(data["primaryConversation"]["provider"], "codex");
        assert!(data["primaryConversation"].get("replay").is_none());
    }

    #[tokio::test]
    async fn primary_conversation_returns_latest_transcript_page() {
        use crate::test_support::test_store;

        let store = test_store().await;
        store.ensure_default_actors().await.expect("actors");
        let conversation = store
            .get_or_create_primary_conversation_for_provider(
                "human:local",
                "codex",
                Some("gpt-test".to_string()),
                None,
            )
            .await
            .expect("primary conversation");
        let turn = store
            .create_conversation_turn(noema_conversations::NewConversationTurn {
                conversation_id: conversation.conversation_id.clone(),
                trigger_item_id: None,
                metadata: serde_json::json!({ "turn_index": 1 }),
            })
            .await
            .expect("turn");

        for label in ["one", "two", "three"] {
            store
                .append_conversation_item(noema_conversations::NewConversationItem {
                    conversation_id: conversation.conversation_id.clone(),
                    turn_id: Some(turn.turn_id.clone()),
                    parent_item_id: None,
                    kind: noema_conversations::ConversationItemKind::UserText,
                    status: noema_conversations::ConversationItemStatus::Completed,
                    author: noema_conversations::ActorRef::human("human:local")
                        .expect("static local human actor id must be valid"),
                    content_text: Some(label.to_string()),
                    payload_json: serde_json::json!({}),
                    metadata: serde_json::json!({ "turn_index": 1 }),
                })
                .await
                .expect("item");
        }

        let runtime = test_autofill_runtime(store.clone(), "ok").await;
        let schema = build_schema(GraphqlState::for_tests_with_store_and_runtime(
            store, runtime,
        ));

        let response = schema
            .execute(async_graphql::Request::new(
                r#"
                query {
                  primaryConversation {
                    provider
                    conversationId
                    latestTranscriptPage(limit: 2) {
                      items {
                        itemId
                        cursor
                        item { __typename ... on UserText { text } }
                      }
                      pageInfo { beforeCursor hasMoreBefore limit }
                    }
                  }
                }
                "#,
            ))
            .await;

        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let data = response.data.into_json().expect("json");
        let primary = &data["primaryConversation"];
        assert_eq!(primary["conversationId"], conversation.conversation_id);
        assert_eq!(primary["provider"], "codex");
        let page = &primary["latestTranscriptPage"];
        assert_eq!(page["items"][0]["item"]["text"], "two");
        assert_eq!(page["items"][1]["item"]["text"], "three");
        assert_eq!(page["pageInfo"]["hasMoreBefore"], true);
        assert_eq!(page["pageInfo"]["limit"], 2);
        assert!(page["pageInfo"]["beforeCursor"].as_str().is_some());
    }

    #[tokio::test]
    async fn conversation_transcript_page_supports_latest_and_cursor_reads() {
        use crate::test_support::test_store;

        let store = test_store().await;
        store.ensure_default_actors().await.expect("actors");
        let conversation = store
            .create_conversation(noema_conversations::NewConversation::local_chat(None, None))
            .await
            .expect("conversation");
        let turn = store
            .create_conversation_turn(noema_conversations::NewConversationTurn {
                conversation_id: conversation.conversation_id.clone(),
                trigger_item_id: None,
                metadata: serde_json::json!({ "turn_index": 1 }),
            })
            .await
            .expect("turn");

        for label in ["one", "two", "three"] {
            store
                .append_conversation_item(noema_conversations::NewConversationItem {
                    conversation_id: conversation.conversation_id.clone(),
                    turn_id: Some(turn.turn_id.clone()),
                    parent_item_id: None,
                    kind: noema_conversations::ConversationItemKind::UserText,
                    status: noema_conversations::ConversationItemStatus::Completed,
                    author: noema_conversations::ActorRef::human("human:local")
                        .expect("static local human actor id must be valid"),
                    content_text: Some(label.to_string()),
                    payload_json: serde_json::json!({}),
                    metadata: serde_json::json!({ "turn_index": 1 }),
                })
                .await
                .expect("item");
        }

        let schema = build_schema(GraphqlState::for_tests_with_store(store));
        let latest = schema
            .execute(async_graphql::Request::new(format!(
                r#"
                query {{
                  conversationTranscriptPage(input: {{ conversationId: "{}", limit: 2 }}) {{
                    items {{
                      itemId
                      cursor
                      item {{ __typename ... on UserText {{ text }} }}
                    }}
                    pageInfo {{ beforeCursor hasMoreBefore limit }}
                  }}
                }}
                "#,
                conversation.conversation_id
            )))
            .await;

        assert!(latest.errors.is_empty(), "{:?}", latest.errors);
        let latest_data = latest.data.into_json().expect("latest json");
        let page = &latest_data["conversationTranscriptPage"];
        assert_eq!(page["items"][0]["item"]["text"], "two");
        assert_eq!(page["items"][1]["item"]["text"], "three");
        assert_eq!(page["pageInfo"]["hasMoreBefore"], true);
        let before_cursor = page["pageInfo"]["beforeCursor"].as_str().expect("cursor");

        let older = schema
            .execute(async_graphql::Request::new(format!(
                r#"
                query {{
                  conversationTranscriptPage(input: {{ conversationId: "{}", cursor: "{}", limit: 2 }}) {{
                    items {{ item {{ __typename ... on UserText {{ text }} }} }}
                    pageInfo {{ hasMoreBefore }}
                  }}
                }}
                "#,
                conversation.conversation_id, before_cursor
            )))
            .await;

        assert!(older.errors.is_empty(), "{:?}", older.errors);
        let older_data = older.data.into_json().expect("older json");
        assert_eq!(
            older_data["conversationTranscriptPage"]["items"][0]["item"]["text"],
            "one"
        );
        assert_eq!(
            older_data["conversationTranscriptPage"]["pageInfo"]["hasMoreBefore"],
            false
        );
    }

    #[tokio::test]
    async fn conversation_transcript_page_returns_item_metadata() {
        use crate::test_support::test_store;

        let store = test_store().await;
        store.ensure_default_actors().await.expect("actors");
        let conversation = store
            .create_conversation(noema_conversations::NewConversation::local_chat(None, None))
            .await
            .expect("conversation");
        let turn = store
            .create_conversation_turn(noema_conversations::NewConversationTurn {
                conversation_id: conversation.conversation_id.clone(),
                trigger_item_id: None,
                metadata: serde_json::json!({ "turn_index": 1 }),
            })
            .await
            .expect("turn");
        store
            .append_conversation_item(noema_conversations::NewConversationItem {
                conversation_id: conversation.conversation_id.clone(),
                turn_id: Some(turn.turn_id.clone()),
                parent_item_id: None,
                kind: noema_conversations::ConversationItemKind::AssistantText,
                status: noema_conversations::ConversationItemStatus::Completed,
                author: noema_conversations::ActorRef::agent("agent:primary")
                    .expect("static primary agent id must be valid"),
                content_text: Some("hello".to_string()),
                payload_json: serde_json::json!({}),
                metadata: serde_json::json!({
                    "provider_usage": {
                        "provider": "codex",
                        "model": "gpt-test",
                        "phase": "initial",
                        "response_index": 0,
                        "input_tokens": 100,
                        "cached_input_tokens": 25,
                        "cache_hit_ratio": 0.25,
                        "output_tokens": 5,
                        "total_tokens": 105
                    }
                }),
            })
            .await
            .expect("item");

        let schema = build_schema(GraphqlState::for_tests_with_store(store));
        let response = schema
            .execute(async_graphql::Request::new(format!(
                r#"
                query {{
                  conversationTranscriptPage(input: {{ conversationId: "{}", limit: 10 }}) {{
                    items {{
                      metadata
                      item {{ __typename ... on AssistantText {{ text }} }}
                    }}
                  }}
                }}
                "#,
                conversation.conversation_id
            )))
            .await;

        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let data = response.data.into_json().expect("json");
        let item = &data["conversationTranscriptPage"]["items"][0];
        assert_eq!(item["item"]["text"], "hello");
        assert_eq!(
            item["metadata"]["provider_usage"]["cached_input_tokens"],
            25
        );
        assert_eq!(item["metadata"]["provider_usage"]["cache_hit_ratio"], 0.25);
    }

    #[tokio::test]
    async fn conversation_transcript_page_exposes_artifact_reference_item() {
        let store = crate::test_support::test_store().await;
        let conversation = store
            .create_conversation(noema_conversations::NewConversation::local_chat(None, None))
            .await
            .expect("conversation");
        let turn = store
            .create_conversation_turn(noema_conversations::NewConversationTurn {
                conversation_id: conversation.conversation_id.clone(),
                trigger_item_id: None,
                metadata: serde_json::json!({ "turn_index": 1 }),
            })
            .await
            .expect("turn");
        let artifact = store
            .create_artifact_with_initial_version(
                noema_artifacts::NewArtifact {
                    artifact_id: None,
                    owner: noema_artifacts::ArtifactOwnerRef::conversation(
                        &conversation.conversation_id,
                    ),
                    title: "Noema notes".to_string(),
                    description: Some("Shared notes".to_string()),
                    artifact_kind: "document".to_string(),
                    storage_kind: noema_artifacts::ArtifactStorageKind::ExternalUrl,
                    created_by_actor_id: "agent:primary".to_string(),
                    source: noema_artifacts::ArtifactSource {
                        conversation_id: Some(conversation.conversation_id.clone()),
                        turn_id: Some(turn.turn_id.clone()),
                        item_id: None,
                    },
                    metadata: serde_json::json!({}),
                },
                noema_artifacts::NewArtifactVersion {
                    artifact_version_id: None,
                    title: None,
                    storage: noema_artifacts::ArtifactVersionStorage::ExternalUrl {
                        url: "https://notion.so/noema-notes".to_string(),
                    },
                    media_type: Some("text/html".to_string()),
                    byte_size: None,
                    content_sha256: None,
                    created_by_actor_id: "agent:primary".to_string(),
                    source: noema_artifacts::ArtifactSource {
                        conversation_id: Some(conversation.conversation_id.clone()),
                        turn_id: Some(turn.turn_id.clone()),
                        item_id: None,
                    },
                    metadata: serde_json::json!({}),
                },
            )
            .await
            .expect("artifact");
        store
            .append_conversation_item(noema_conversations::NewConversationItem {
                conversation_id: conversation.conversation_id.clone(),
                turn_id: Some(turn.turn_id.clone()),
                parent_item_id: None,
                kind: noema_conversations::ConversationItemKind::ArtifactReference,
                status: noema_conversations::ConversationItemStatus::Completed,
                author: noema_conversations::ActorRef::agent("agent:primary")
                    .expect("static primary agent id must be valid"),
                content_text: None,
                payload_json: serde_json::json!({
                    "artifact_id": artifact.artifact.artifact_id,
                    "artifact_version_id": artifact.current_version.artifact_version_id,
                    "title": "Noema notes",
                    "artifact_kind": "document",
                    "storage_kind": "external_url",
                    "external_url": "https://notion.so/noema-notes",
                    "download_url": null,
                    "media_type": "text/html"
                }),
                metadata: serde_json::json!({}),
            })
            .await
            .expect("item");

        let schema = build_schema(GraphqlState::for_tests_with_store(store));
        let response = schema
            .execute(async_graphql::Request::new(format!(
                r#"
                query {{
                  conversationTranscriptPage(input: {{ conversationId: "{}", limit: 10 }}) {{
                    items {{
                      item {{
                        __typename
                        ... on ArtifactReference {{
                          artifactId
                          artifactVersionId
                          title
                          artifactKind
                          storageKind
                          externalUrl
                          downloadUrl
                          mediaType
                        }}
                      }}
                    }}
                  }}
                }}
                "#,
                conversation.conversation_id
            )))
            .await;

        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let data = response.data.into_json().expect("json");
        let item = &data["conversationTranscriptPage"]["items"][0]["item"];
        assert_eq!(item["__typename"], "ArtifactReference");
        assert_eq!(item["artifactId"], artifact.artifact.artifact_id);
        assert_eq!(
            item["artifactVersionId"],
            artifact.current_version.artifact_version_id
        );
        assert_eq!(item["title"], "Noema notes");
        assert_eq!(item["artifactKind"], "document");
        assert_eq!(item["storageKind"], "external_url");
        assert_eq!(item["externalUrl"], "https://notion.so/noema-notes");
        assert!(item["downloadUrl"].is_null());
        assert_eq!(item["mediaType"], "text/html");
    }
