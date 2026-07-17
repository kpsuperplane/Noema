use super::test_store;

#[tokio::test]
async fn sqlite_memory_service_defaults_to_managed() {
    let store = test_store().await;

    let settings = store.memory_service_settings().await.expect("settings");

    assert_eq!(settings.mode, noema_memory::MemoryServiceMode::Managed);
    assert_eq!(settings.base_url, None);
    assert_eq!(settings.port, None);
}

#[tokio::test]
async fn sqlite_memory_service_settings_round_trip_external() {
    let store = test_store().await;

    store
        .save_memory_service_settings(noema_memory::SaveMemoryServiceSettings {
            mode: noema_memory::MemoryServiceMode::External,
            base_url: Some("http://127.0.0.1:7777".to_string()),
            port: None,
            provider_account_id: Some("provider_account:openai:default".to_string()),
            provider_kind: Some("openai".to_string()),
            model_profile: Some("gpt-5.1".to_string()),
            reasoning_effort: Some(noema_providers::ReasoningEffort::Low),
        })
        .await
        .expect("save settings");

    let settings = store.memory_service_settings().await.expect("settings");
    assert_eq!(settings.mode, noema_memory::MemoryServiceMode::External);
    assert_eq!(settings.base_url.as_deref(), Some("http://127.0.0.1:7777"));
    assert_eq!(
        settings.reasoning_effort,
        Some(noema_providers::ReasoningEffort::Low)
    );
}

#[tokio::test]
async fn sqlite_memory_article_cache_round_trip() {
    let store = test_store().await;

    store
        .save_memory_article_cache(noema_memory::SaveMemoryArticleCache {
            scope_id: "human:local".to_string(),
            fact_fingerprint: "facts-v1".to_string(),
            article_markdown: "# Kevin\n\nLittle is currently known about Kevin.".to_string(),
            generated_at: "2026-07-08T20:00:00Z".to_string(),
        })
        .await
        .expect("save article cache");

    let cached = store
        .memory_article_cache("human:local")
        .await
        .expect("cache")
        .expect("cache row");
    assert_eq!(cached.fact_fingerprint, "facts-v1");
    assert_eq!(
        cached.article_markdown,
        "# Kevin\n\nLittle is currently known about Kevin."
    );
}
