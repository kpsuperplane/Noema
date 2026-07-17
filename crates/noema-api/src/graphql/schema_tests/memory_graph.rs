    async fn spawn_memory_graph_mnemosyne_server() -> String {
        spawn_memory_graph_mnemosyne_server_with_limit(25).await
    }

    fn memory_graph_state(store: noema_store::NoemaStore) -> GraphqlState {
        let repository: noema_memory::MemoryRepositoryHandle = Arc::new(store.clone());
        let access = crate::test_support::memory_service_access(repository);
        GraphqlState::for_tests_with_store(store).with_memory_service_access(access)
    }

    fn memory_graph_state_with_runtime(
        store: noema_store::NoemaStore,
        runtime: noema_runtime::RuntimeHandle,
    ) -> GraphqlState {
        let repository: noema_memory::MemoryRepositoryHandle = Arc::new(store.clone());
        let access = crate::test_support::memory_service_access(repository);
        GraphqlState::for_tests_with_store_and_runtime(store, runtime)
            .with_memory_service_access(access)
    }

    async fn spawn_memory_graph_mnemosyne_server_with_limit(limit: u16) -> String {
        let results = json!([
            {
                "id": "mem_1",
                "memory": "Kevin prefers local-first tools",
                "metadata": {"sourceKind": "user_message"},
                "created_at": "2026-07-08T00:00:30.000Z",
                "updated_at": "2026-07-08T00:01:00.000Z"
            },
            {
                "id": "mem_2",
                "memory": "Kevin likes tools that keep data local",
                "metadata": {
                    "noemaConversationId": "abc",
                    "sourceObservation": "I prefer local-first tools."
                },
                "created_at": "2026-07-08T00:00:40.000Z",
                "updated_at": "2026-07-08T00:01:10.000Z"
            }
        ]);
        spawn_memory_graph_mnemosyne_server_with_results(limit, results).await
    }

    async fn spawn_memory_graph_mnemosyne_server_with_results(
        limit: u16,
        results: serde_json::Value,
    ) -> String {
        use tokio::{
            io::{AsyncReadExt, AsyncWriteExt},
            net::TcpListener,
        };

        let listener = TcpListener::bind("127.0.0.1:0").await.expect("listener");
        let base_url = format!(
            "http://{}",
            listener.local_addr().expect("listener address")
        );
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("accept");
            let mut buffer = vec![0_u8; 8192];
            let read = stream.read(&mut buffer).await.expect("read");
            let request = String::from_utf8_lossy(&buffer[..read]);
            let (head, _body) = request.split_once("\r\n\r\n").expect("request head");
            let mut lines = head.lines();
            let request_line = lines.next().expect("request line");
            assert_eq!(
                request_line,
                format!("GET /v1/memories?user_id=human%3Alocal&limit={limit} HTTP/1.1")
            );

            let response = json!({ "results": results });
            let response_body = serde_json::to_vec(&response).expect("response JSON");
            let response_head = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\r\n",
                response_body.len()
            );
            stream
                .write_all(response_head.as_bytes())
                .await
                .expect("write head");
            stream.write_all(&response_body).await.expect("write body");
        });

        base_url
    }

    #[tokio::test]
    async fn memory_settings_query_returns_defaults() {
        let store = crate::test_support::test_store().await;
        let schema = build_schema(GraphqlState::for_tests_with_store(store));

        let response = schema
            .execute(async_graphql::Request::new(
                "{ memorySettings { mode baseUrl port status { status } } }",
            ))
            .await
            .into_result()
            .expect("query");

        assert_eq!(
            response.data,
            async_graphql::Value::from_json(serde_json::json!({
                "memorySettings": {
                    "mode": "MANAGED",
                    "baseUrl": null,
                    "port": null,
                    "status": {"status": "UNAVAILABLE"}
                }
            }))
            .expect("json")
        );
    }

    #[tokio::test]
    async fn memory_settings_query_reports_managed_mnemosyne_unavailable() {
        let home = tempfile::TempDir::new().expect("home");
        let paths = TestEnvironment::from_root(home.path()).expect("paths");
        let store = crate::test_support::test_store_for_environment(&paths).await;
        let schema = build_schema(GraphqlState::for_tests_with_store_and_environment(store, paths));

        let response = schema
            .execute(async_graphql::Request::new(
                r#"
                {
                  memorySettings {
                    status {
                      status
                      lastErrorCode
                      lastErrorMessage
                    }
                  }
                }
                "#,
            ))
            .await
            .into_result()
            .expect("query");
        let data = response.data.into_json().expect("json");

        assert_eq!(data["memorySettings"]["status"]["status"], "UNAVAILABLE");
        assert_eq!(
            data["memorySettings"]["status"]["lastErrorCode"],
            "mnemosyne_unavailable"
        );
        assert_eq!(
            data["memorySettings"]["status"]["lastErrorMessage"],
            "Managed Mnemosyne is not running"
        );
    }

    #[tokio::test]
    async fn memory_graph_returns_unavailable_without_memory_connection() {
        let store = crate::test_support::test_store().await;
        let schema = build_schema(GraphqlState::for_tests_with_store(store));

        let response = schema
            .execute(async_graphql::Request::new(
                r#"
                {
                  memoryGraph(input: { page: 1, limit: 25 }) {
                    status {
                      status
                      lastErrorCode
                    }
                    documents {
                      id
                    }
                    article {
                      title
                      markdown
                      isGenerated
                    }
                    pageInfo {
                      page
                      limit
                      hasMore
                    }
                  }
                }
                "#,
            ))
            .await
            .into_result()
            .expect("query");
        let data = response.data.into_json().expect("json");

        assert_eq!(data["memoryGraph"]["status"]["status"], "UNAVAILABLE");
        assert_eq!(
            data["memoryGraph"]["status"]["lastErrorCode"],
            "mnemosyne_unavailable"
        );
        assert_eq!(data["memoryGraph"]["documents"], json!([]));
        assert_eq!(data["memoryGraph"]["article"]["title"], "Local human");
        assert_eq!(
            data["memoryGraph"]["article"]["markdown"],
            "# Local human\n\nLittle is currently known about Local human."
        );
        assert_eq!(data["memoryGraph"]["article"]["isGenerated"], false);
        assert_eq!(data["memoryGraph"]["pageInfo"]["page"], 1);
        assert_eq!(data["memoryGraph"]["pageInfo"]["limit"], 25);
        assert_eq!(data["memoryGraph"]["pageInfo"]["hasMore"], false);
    }

    #[tokio::test]
    async fn memory_graph_lists_external_mnemosyne_memories() {
        let server_base_url = spawn_memory_graph_mnemosyne_server().await;
        let store = crate::test_support::test_store().await;
        store
            .save_memory_service_settings(noema_memory::SaveMemoryServiceSettings {
                mode: noema_memory::MemoryServiceMode::External,
                base_url: Some(server_base_url),
                port: None,
                provider_account_id: None,
                provider_kind: None,
                model_profile: None,
                reasoning_effort: None,
            })
            .await
            .expect("settings");
        let schema = build_schema(memory_graph_state(store));

        let response = schema
            .execute(async_graphql::Request::new(
                r#"
                {
                  memoryGraph(input: { page: 1, limit: 25 }) {
                    status {
                      status
                      lastErrorCode
                    }
                    documents {
                      id
                      title
                      memoryEntries {
                        id
                        citationKey
                        documentId
                        content
                        source {
                          kind
                          conversationId
                          turnId
                          itemId
                          messageText
                        }
                        metadata
                        spaceContainerTag
                        parentMemoryId
                        rootMemoryId
                        memoryRelations
                      }
                    }
                    article {
                      title
                      subtitle
                      markdown
                      isGenerated
                    }
                    pageInfo {
                      page
                      limit
                      hasMore
                      total
                    }
                  }
                }
                "#,
            ))
            .await
            .into_result()
            .expect("query");
        let data = response.data.into_json().expect("json");

        assert_eq!(data["memoryGraph"]["status"]["status"], "READY");
        assert_eq!(data["memoryGraph"]["article"]["title"], "Local human");
        let first_citation = crate::graphql::memory::memory_citation_key("mem_1");
        let second_citation = crate::graphql::memory::memory_citation_key("mem_2");
        assert_eq!(
            data["memoryGraph"]["article"]["markdown"],
            format!(
                "# Local human\n\nLocal human is described by the currently available biographical facts.\n\nKevin prefers local-first tools [^{first_citation}]\n\nKevin likes tools that keep data local [^{second_citation}]"
            )
        );
        assert_eq!(data["memoryGraph"]["article"]["isGenerated"], false);
        assert_eq!(
            data["memoryGraph"]["documents"][0]["id"],
            "conversation:abc"
        );
        assert_eq!(
            data["memoryGraph"]["documents"][0]["memoryEntries"][0]["id"],
            "mem_2"
        );
        assert_eq!(
            data["memoryGraph"]["documents"][0]["memoryEntries"][0]["citationKey"],
            second_citation
        );
        assert_eq!(
            data["memoryGraph"]["documents"][0]["memoryEntries"][0]["documentId"],
            "conversation:abc"
        );
        assert_eq!(
            data["memoryGraph"]["documents"][0]["memoryEntries"][0]["spaceContainerTag"],
            "human:local"
        );
        assert_eq!(
            data["memoryGraph"]["documents"][0]["memoryEntries"][0]["metadata"]["sourceObservation"],
            "I prefer local-first tools."
        );
        assert_eq!(
            data["memoryGraph"]["documents"][0]["memoryEntries"][0]["source"]["conversationId"],
            "abc"
        );
        assert_eq!(
            data["memoryGraph"]["documents"][0]["memoryEntries"][0]["source"]["messageText"],
            "I prefer local-first tools."
        );
        assert_eq!(
            data["memoryGraph"]["documents"][1]["id"],
            "mnemosyne:human:local"
        );
        assert_eq!(data["memoryGraph"]["documents"][1]["title"], "Human memory");
        assert_eq!(data["memoryGraph"]["pageInfo"]["hasMore"], false);
        assert_eq!(data["memoryGraph"]["pageInfo"]["total"], 2);
    }

    #[tokio::test]
    async fn memory_graph_source_resolves_exact_persisted_user_message() {
        let store = crate::test_support::test_store().await;
        store.ensure_default_actors().await.expect("actors");
        let conversation = store
            .get_or_create_primary_conversation_for_provider(
                "human:local",
                "codex",
                Some("gpt-test".to_string()),
                None,
            )
            .await
            .expect("conversation");
        let turn = store
            .create_conversation_turn(noema_conversations::NewConversationTurn {
                conversation_id: conversation.conversation_id.clone(),
                trigger_item_id: None,
                metadata: json!({}),
            })
            .await
            .expect("turn");
        let user_item = store
            .append_conversation_item(noema_conversations::NewConversationItem {
                conversation_id: conversation.conversation_id.clone(),
                turn_id: Some(turn.turn_id.clone()),
                parent_item_id: None,
                kind: noema_conversations::ConversationItemKind::UserText,
                status: noema_conversations::ConversationItemStatus::Completed,
                author: noema_conversations::ActorRef::human("human:local")
                    .expect("static local human actor id must be valid"),
                content_text: Some("I like airplanes and local-first tools.".to_string()),
                payload_json: json!({}),
                metadata: json!({}),
            })
            .await
            .expect("user item");
        let server_base_url = spawn_memory_graph_mnemosyne_server_with_results(
            25,
            json!([
                {
                    "id": "mem_exact",
                    "memory": "Kevin likes airplanes and local-first tools",
                    "metadata": {
                        "noemaConversationId": conversation.conversation_id,
                        "turnId": turn.turn_id,
                        "userItemId": user_item.item_id,
                        "sourceKind": "user_message",
                        "sourceObservation": "stale copied text"
                    },
                    "created_at": "2026-07-08T00:00:40.000Z",
                    "updated_at": "2026-07-08T00:01:10.000Z"
                }
            ]),
        )
        .await;
        store
            .save_memory_service_settings(noema_memory::SaveMemoryServiceSettings {
                mode: noema_memory::MemoryServiceMode::External,
                base_url: Some(server_base_url),
                port: None,
                provider_account_id: None,
                provider_kind: None,
                model_profile: None,
                reasoning_effort: None,
            })
            .await
            .expect("settings");
        let schema = build_schema(memory_graph_state(store));

        let response = schema
            .execute(async_graphql::Request::new(
                r#"
                {
                  memoryGraph(input: { page: 1, limit: 25 }) {
                    documents {
                      memoryEntries {
                        source {
                          kind
                          conversationId
                          turnId
                          itemId
                          messageText
                        }
                      }
                    }
                  }
                }
                "#,
            ))
            .await
            .into_result()
            .expect("query");
        let data = response.data.into_json().expect("json");
        let source = &data["memoryGraph"]["documents"][0]["memoryEntries"][0]["source"];

        assert_eq!(source["kind"], "user_message");
        assert_eq!(
            source["messageText"],
            "I like airplanes and local-first tools."
        );
        assert_ne!(source["messageText"], "stale copied text");
    }
