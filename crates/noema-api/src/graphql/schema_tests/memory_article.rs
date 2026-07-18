    #[tokio::test]
    async fn memory_graph_lazily_generates_and_caches_article() {
        let server_base_url = spawn_memory_graph_mnemosyne_server().await;
        let store = crate::test_support::test_store().await;
        configure_external_memory(&store, server_base_url).await;
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
        let runtime = test_memory_article_runtime(store.clone(), &generated_article).await;
        let schema = build_schema(memory_graph_state_with_runtime(store.clone(), runtime));

        let response = schema
            .execute(async_graphql::Request::new(
                r#"{
                  memoryGraph(input: { page: 1, limit: 25 }) {
                    article { title markdown isGenerated generatedAt }
                  }
                }"#,
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
        configure_external_memory(&store, server_base_url).await;
        let runtime = test_memory_article_runtime(
            store.clone(),
            "# Kevin\n\nKevin prefers local-first tools.",
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
