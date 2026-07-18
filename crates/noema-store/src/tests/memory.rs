use super::{ready_provider_selection, test_store};

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
