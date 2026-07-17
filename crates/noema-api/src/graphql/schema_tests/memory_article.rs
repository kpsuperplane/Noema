    #[tokio::test]
    async fn memory_graph_lazily_generates_and_caches_article() {
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
        store
            .save_memory_article_cache(noema_memory::SaveMemoryArticleCache {
                scope_id: "human:local".to_string(),
                fact_fingerprint: "legacy-uncited-format".to_string(),
                article_markdown: "# Legacy\n\nThis cached article has no footnotes.".to_string(),
                generated_at: "2099-01-01T00:00:00Z".to_string(),
            })
            .await
            .expect("legacy article cache");
        let first_citation = crate::graphql::memory::memory_citation_key("mem_1");
        let second_citation = crate::graphql::memory::memory_citation_key("mem_2");
        let generated_article = format!(
            "# Kevin\n\nKevin prefers local-first tools and tools that keep data local.[^{first_citation}][^{second_citation}]"
        );
        let (requests, runtime) = test_autofill_runtime_with_requests(
            store.clone(),
            &generated_article,
            Some("test-memory-writer"),
        )
        .await;
        let schema = build_schema(memory_graph_state_with_runtime(store.clone(), runtime));

        let response = schema
            .execute(async_graphql::Request::new(
                r#"
                {
                  memoryGraph(input: { page: 1, limit: 25 }) {
                    article {
                      title
                      markdown
                      isGenerated
                      generatedAt
                    }
                  }
                }
                "#,
            ))
            .await
            .into_result()
            .expect("query");
        let data = response.data.into_json().expect("json");

        assert_eq!(data["memoryGraph"]["article"]["title"], "Kevin");
        assert_eq!(
            data["memoryGraph"]["article"]["markdown"],
            generated_article
        );
        assert_eq!(data["memoryGraph"]["article"]["isGenerated"], true);
        assert!(data["memoryGraph"]["article"]["generatedAt"].is_string());
        {
            let requests = requests.lock().expect("requests");
            assert_eq!(requests.len(), 1);
            assert_eq!(requests[0].model.as_deref(), Some("gpt-5.6-luna"));
            match &requests[0].input {
                noema_providers::GenerateInput::Text(prompt) => {
                    assert!(prompt.contains("Kevin prefers local-first tools"));
                    assert!(prompt.contains("User-authored source observation"));
                    assert!(prompt.contains("do not turn a preference about another speaker"));
                    assert!(prompt.contains(&format!("cite as [^{first_citation}]")));
                    assert!(prompt.contains("Every factual sentence or clause"));
                    assert!(prompt.contains("Return Markdown only"));
                }
                other => panic!("unexpected memory article input: {other:?}"),
            }
        }
        let cached = store
            .memory_article_cache("human:local")
            .await
            .expect("article cache")
            .expect("article cache row");
        assert_eq!(cached.article_markdown, generated_article);
        assert!(cached.fact_fingerprint.starts_with("v2:"));
    }

    #[tokio::test]
    async fn memory_article_falls_back_when_model_omits_citations() {
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
        let (_requests, runtime) = test_autofill_runtime_with_requests(
            store.clone(),
            "# Kevin\n\nKevin prefers local-first tools.",
            Some("test-memory-writer"),
        )
        .await;
        let schema = build_schema(memory_graph_state_with_runtime(store.clone(), runtime));

        let response = schema
            .execute(async_graphql::Request::new(
                "{ memoryGraph(input: { page: 1, limit: 25 }) { article { markdown isGenerated } } }",
            ))
            .await
            .into_result()
            .expect("query");
        let data = response.data.into_json().expect("json");
        let first_citation = crate::graphql::memory::memory_citation_key("mem_1");

        assert_eq!(data["memoryGraph"]["article"]["isGenerated"], false);
        assert!(
            data["memoryGraph"]["article"]["markdown"]
                .as_str()
                .is_some_and(|markdown| markdown.contains(&format!("[^{first_citation}]")))
        );
        assert!(
            store
                .memory_article_cache("human:local")
                .await
                .expect("article cache")
                .is_none()
        );
    }

    #[tokio::test]
    async fn memory_article_uses_configured_memory_model_preference() {
        let server_base_url = spawn_memory_graph_mnemosyne_server().await;
        let store = crate::test_support::test_store().await;
        store
            .ensure_default_provider_account()
            .await
            .expect("provider account");
        store
            .update_provider_account_status(
                "provider_account:codex:default",
                noema_providers::ProviderAccountStatus::Authenticated,
                None,
                None,
            )
            .await
            .expect("authenticated provider account");
        let ready_selection = crate::test_support::ready_provider_selection(
            noema_providers::ProviderSelectionSnapshot::explicit(
                "codex",
                "provider_account:codex:default",
                "memory-writer",
                Some(noema_providers::ReasoningEffort::High),
                Some("memory_article_test".to_string()),
            ),
        );
        store
            .save_memory_service_settings_with_ready_selection(
                noema_memory::SaveMemoryServiceSettings {
                    mode: noema_memory::MemoryServiceMode::External,
                    base_url: Some(server_base_url),
                    port: None,
                    provider_account_id: Some("provider_account:codex:default".to_string()),
                    provider_kind: Some("codex".to_string()),
                    model_profile: Some("memory-writer".to_string()),
                    reasoning_effort: Some(noema_providers::ReasoningEffort::High),
                },
                &ready_selection,
            )
            .await
            .expect("settings");
        let citation = crate::graphql::memory::memory_citation_key("mem_1");
        let generated_article = format!("# Kevin\n\nKevin prefers local-first tools.[^{citation}]");
        let (requests, runtime) = test_autofill_runtime_with_requests(
            store.clone(),
            &generated_article,
            Some("wrong-tool-classifier"),
        )
        .await;
        let schema = build_schema(memory_graph_state_with_runtime(store.clone(), runtime));

        schema
            .execute(async_graphql::Request::new(
                "{ memoryGraph(input: { page: 1, limit: 25 }) { article { title } } }",
            ))
            .await
            .into_result()
            .expect("query");

        let requests = requests.lock().expect("requests");
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].model.as_deref(), Some("memory-writer"));
        assert_eq!(
            requests[0].options.reasoning_effort,
            Some(noema_providers::ReasoningEffort::High)
        );
    }

    #[tokio::test]
    async fn regenerate_memory_article_forces_generation() {
        let server_base_url = spawn_memory_graph_mnemosyne_server_with_limit(100).await;
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
        let citation = crate::graphql::memory::memory_citation_key("mem_1");
        let generated_article = format!("# Kevin\n\nKevin is freshly regenerated.[^{citation}]");
        let (requests, runtime) = test_autofill_runtime_with_requests(
            store.clone(),
            &generated_article,
            Some("test-memory-writer"),
        )
        .await;
        let schema = build_schema(memory_graph_state_with_runtime(store.clone(), runtime));

        let response = schema
            .execute(async_graphql::Request::new(
                r#"
                mutation {
                  regenerateMemoryArticle {
                    title
                    markdown
                    isGenerated
                  }
                }
                "#,
            ))
            .await
            .into_result()
            .expect("mutation");
        let data = response.data.into_json().expect("json");

        assert_eq!(data["regenerateMemoryArticle"]["title"], "Kevin");
        assert_eq!(
            data["regenerateMemoryArticle"]["markdown"],
            generated_article
        );
        assert_eq!(data["regenerateMemoryArticle"]["isGenerated"], true);
        assert_eq!(requests.lock().expect("requests").len(), 1);
    }

    #[tokio::test]
    async fn save_external_memory_service_settings_requires_base_url() {
        let store = crate::test_support::test_store().await;
        let schema = build_schema(GraphqlState::for_tests_with_store(store));

        let response = schema
            .execute(async_graphql::Request::new(
                r#"
                mutation {
                  saveMemoryServiceSettings(input: {
                    mode: EXTERNAL
                  }) {
                    mode
                  }
                }
                "#,
            ))
            .await;

        assert!(!response.errors.is_empty());
        assert!(response.errors[0].message.contains("base URL"));
    }

    #[tokio::test]
    async fn old_memory_graph_nodes_field_is_not_in_schema() {
        let schema = build_schema(GraphqlState::for_tests());
        let response = schema
            .execute(async_graphql::Request::new(
                "{ memoryGraph { nodes { nodeId } } }",
            ))
            .await;

        assert!(!response.errors.is_empty());
        assert!(response.errors[0].message.contains("nodes"));
    }

    #[tokio::test]
    async fn check_memory_service_times_out_when_socket_never_responds() {
        use tokio::{
            net::TcpListener,
            time::{Duration, timeout},
        };

        let listener = TcpListener::bind("127.0.0.1:0").await.expect("listener");
        let address = listener.local_addr().expect("listener address");
        let _server = tokio::spawn(async move {
            if let Ok((_socket, _peer)) = listener.accept().await {
                futures_util::future::pending::<()>().await;
            }
        });

        let store = crate::test_support::test_store().await;
        let schema = build_schema(memory_graph_state(store));
        let save_response = schema
            .execute(format!(
                r#"
                mutation {{
                  saveMemoryServiceSettings(input: {{
                    mode: EXTERNAL
                    baseUrl: "http://{address}"
                  }}) {{
                    baseUrl
                  }}
                }}
                "#
            ))
            .await;
        assert!(
            save_response.errors.is_empty(),
            "{:?}",
            save_response.errors
        );

        let response = timeout(
            Duration::from_secs(3),
            schema.execute(async_graphql::Request::new(
                r#"
                mutation {
                  checkMemoryService {
                    status
                    lastErrorCode
                  }
                }
                "#,
            )),
        )
        .await
        .expect("readiness check should not hang indefinitely");

        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let data = response.data.into_json().expect("json");
        assert_eq!(data["checkMemoryService"]["status"], "UNAVAILABLE");
        assert_eq!(
            data["checkMemoryService"]["lastErrorCode"],
            "request_failed"
        );
    }

    #[tokio::test]
    async fn save_memory_service_settings_rejects_reasoning_effort_without_model() {
        let store = crate::test_support::test_store().await;
        let schema = build_schema(GraphqlState::for_tests_with_store(store));

        let response = schema
            .execute(async_graphql::Request::new(
                r#"
                mutation {
                  saveMemoryServiceSettings(input: {
                    mode: MANAGED
                    reasoningEffort: HIGH
                  }) {
                    mode
                  }
                }
                "#,
            ))
            .await;

        assert!(!response.errors.is_empty());
        assert!(response.errors[0].message.contains("reasoning effort"));
    }
