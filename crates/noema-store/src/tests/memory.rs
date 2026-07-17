use super::{ready_provider_selection, test_store};

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

    let input = noema_memory::SaveMemoryServiceSettings {
        mode: noema_memory::MemoryServiceMode::External,
        base_url: Some("http://127.0.0.1:7777".to_string()),
        port: None,
        provider_account_id: Some("provider_account:codex:default".to_string()),
        provider_kind: Some("codex".to_string()),
        model_profile: Some("gpt-5.1".to_string()),
        reasoning_effort: Some(noema_providers::ReasoningEffort::Low),
    };
    let ready_selection =
        ready_provider_selection(noema_providers::ProviderSelectionSnapshot::explicit(
            "codex",
            "provider_account:codex:default",
            "gpt-5.1",
            Some(noema_providers::ReasoningEffort::Low),
            Some("memory_settings_test".to_string()),
        ));
    store
        .save_memory_service_settings_with_ready_selection(input, &ready_selection)
        .await
        .expect("save settings");

    let settings = store.memory_service_settings().await.expect("settings");
    assert_eq!(settings.mode, noema_memory::MemoryServiceMode::External);
    assert_eq!(settings.base_url.as_deref(), Some("http://127.0.0.1:7777"));
    assert_eq!(
        settings.reasoning_effort,
        Some(noema_providers::ReasoningEffort::Low)
    );
    assert_eq!(
        settings.provider_instance_key,
        Some(
            noema_providers::provider_account_instance_key("provider_account:codex:default")
                .expect("provider key")
        )
    );
}

#[tokio::test]
async fn managed_settings_without_route_fields_preserve_the_initialized_exact_selection() {
    let store = test_store().await;
    store
        .ensure_default_provider_account()
        .await
        .expect("provider account");
    let mut configured_default = noema_providers::ProviderSelectionSnapshot::explicit(
        "codex",
        "provider_account:codex:default",
        "gpt-5.6-luna",
        Some(noema_providers::ReasoningEffort::Low),
        Some("test_configured_default".to_string()),
    );
    let expected_key =
        noema_providers::provider_account_instance_key("provider_account:codex:default")
            .expect("provider key");
    configured_default.provider_instance_key = Some(expected_key.clone());
    let ready_selection = ready_provider_selection(configured_default);
    store
        .initialize_missing_provider_selections(ready_selection.selection(), Some(&ready_selection))
        .await
        .expect("initialize selections");

    let saved = store
        .save_memory_service_settings(noema_memory::SaveMemoryServiceSettings {
            mode: noema_memory::MemoryServiceMode::Managed,
            base_url: None,
            port: None,
            provider_account_id: None,
            provider_kind: None,
            model_profile: None,
            reasoning_effort: None,
        })
        .await
        .expect("save managed settings");

    assert_eq!(saved.provider_kind.as_deref(), Some("codex"));
    assert_eq!(
        saved.provider_account_id.as_deref(),
        Some("provider_account:codex:default")
    );
    assert_eq!(saved.provider_instance_key, Some(expected_key));
    assert_eq!(saved.model_profile.as_deref(), Some("gpt-5.6-luna"));
    assert_eq!(
        saved.reasoning_effort,
        Some(noema_providers::ReasoningEffort::Low)
    );
}

#[tokio::test]
async fn managed_settings_cannot_erase_an_uninitialized_route() {
    let store = test_store().await;

    let error = store
        .save_memory_service_settings(noema_memory::SaveMemoryServiceSettings {
            mode: noema_memory::MemoryServiceMode::Managed,
            base_url: None,
            port: None,
            provider_account_id: None,
            provider_kind: None,
            model_profile: None,
            reasoning_effort: None,
        })
        .await
        .expect_err("uninitialized managed route");

    assert!(matches!(
        error,
        crate::StoreError::InvariantViolation { .. }
    ));
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
