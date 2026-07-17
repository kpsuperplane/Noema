use super::{ready_provider_selection, test_store};

#[tokio::test]
async fn sqlite_default_actors_round_trip() {
    let store = test_store().await;

    store.ensure_default_actors().await.expect("default actors");

    let human = store
        .get_human("human:local")
        .await
        .expect("get human")
        .expect("human exists");
    let agent = store
        .get_agent("agent:primary")
        .await
        .expect("get agent")
        .expect("agent exists");

    assert_eq!(human.human_id, "human:local");
    assert_eq!(agent.agent_id, "agent:primary");
    assert_eq!(agent.display_name, None);
}

#[tokio::test]
async fn sqlite_agent_display_name_updates() {
    let store = test_store().await;
    store.ensure_default_actors().await.expect("default actors");

    store
        .update_agent_display_name("agent:primary", "Noema")
        .await
        .expect("update agent");

    let agent = store
        .get_agent("agent:primary")
        .await
        .expect("get agent")
        .expect("agent exists");
    assert_eq!(agent.display_name.as_deref(), Some("Noema"));
}

#[tokio::test]
async fn sqlite_create_agent_rejects_duplicate_agent_id() {
    let store = test_store().await;

    store
        .create_agent(crate::NewAgent {
            agent_id: "agent:duplicate".to_string(),
            display_name: Some("Original".to_string()),
        })
        .await
        .expect("create agent");

    store
        .create_agent(crate::NewAgent {
            agent_id: "agent:duplicate".to_string(),
            display_name: Some("Replacement".to_string()),
        })
        .await
        .expect_err("duplicate agent id should fail");

    let agent = store
        .get_agent("agent:duplicate")
        .await
        .expect("get agent")
        .expect("agent exists");
    assert_eq!(agent.display_name.as_deref(), Some("Original"));
}

#[tokio::test]
async fn sqlite_provider_accounts_seed_and_list() {
    let store = test_store().await;

    store
        .ensure_default_provider_account()
        .await
        .expect("codex");
    store
        .ensure_default_foundation_local_provider_account()
        .await
        .expect("foundation");
    store
        .ensure_default_openai_provider_account()
        .await
        .expect("openai");

    let accounts = store
        .list_provider_accounts()
        .await
        .expect("provider accounts");
    let ids = accounts
        .iter()
        .map(|account| account.provider_account_id.as_str())
        .collect::<Vec<_>>();

    assert!(ids.contains(&"provider_account:codex:default"));
    assert!(ids.contains(&"provider_account:foundation_local:default"));
    assert!(ids.contains(&"provider_account:openai:default"));
}

#[tokio::test]
async fn sqlite_agent_model_preference_round_trip() {
    let store = test_store().await;
    store.ensure_default_actors().await.expect("actors");
    store
        .ensure_default_provider_account()
        .await
        .expect("provider");
    store
        .update_provider_account_status(
            "provider_account:codex:default",
            noema_providers::ProviderAccountStatus::Authenticated,
            None,
            None,
        )
        .await
        .expect("authenticated provider");

    let preference = crate::NewAgentRuntimePreference {
        agent_id: "agent:primary".to_string(),
        provider_kind: "codex".to_string(),
        provider_account_id: "provider_account:codex:default".to_string(),
        model_profile: "gpt-5.5".to_string(),
        reasoning_effort: Some(noema_providers::ReasoningEffort::Medium),
    };
    let ready_selection =
        ready_provider_selection(noema_providers::ProviderSelectionSnapshot::explicit(
            &preference.provider_kind,
            &preference.provider_account_id,
            &preference.model_profile,
            preference.reasoning_effort,
            Some("agent_preference_test".to_string()),
        ));
    store
        .upsert_agent_runtime_preference_with_ready_selection(preference, &ready_selection)
        .await
        .expect("save preference");

    let preference = store
        .get_agent_runtime_preference("agent:primary")
        .await
        .expect("get preference")
        .expect("preference exists");
    assert_eq!(preference.model_profile, "gpt-5.5");
    assert_eq!(
        preference.provider_instance_key,
        noema_providers::provider_account_instance_key("provider_account:codex:default")
            .expect("provider key")
    );
    assert_eq!(
        preference.reasoning_effort,
        Some(noema_providers::ReasoningEffort::Medium)
    );
}

#[tokio::test]
async fn direct_provider_account_delete_rejects_a_canonical_reference() {
    let store = test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let provider_account_id = "provider_account:codex:user-managed";
    {
        let connection = store.connection_for_tests();
        let connection = connection.lock().await;
        connection
            .execute(
                r#"
                INSERT INTO provider_accounts (
                  provider_account_id, provider_kind, account_key, display_name,
                  auth_method, is_active, is_default, status, metadata_json
                ) VALUES (?1, 'codex', 'user-managed', 'User managed Codex',
                          'external_manual', 1, 0, 'authenticated', '{}')
                "#,
                [provider_account_id],
            )
            .expect("user-managed provider account");
    }
    let preference = crate::NewAgentRuntimePreference {
        agent_id: "agent:primary".to_string(),
        provider_kind: "codex".to_string(),
        provider_account_id: provider_account_id.to_string(),
        model_profile: "gpt-test".to_string(),
        reasoning_effort: None,
    };
    let ready_selection =
        ready_provider_selection(noema_providers::ProviderSelectionSnapshot::explicit(
            &preference.provider_kind,
            &preference.provider_account_id,
            &preference.model_profile,
            preference.reasoning_effort,
            Some("account_delete_test".to_string()),
        ));
    store
        .upsert_agent_runtime_preference_with_ready_selection(preference, &ready_selection)
        .await
        .expect("canonical account reference");

    let error = store
        .delete_provider_account(provider_account_id)
        .await
        .expect_err("referenced account must be retained");

    assert!(matches!(
        error,
        crate::StoreError::ProviderAccountInUse { provider_account_id: id }
            if id == provider_account_id
    ));
    assert!(
        store
            .get_provider_account(provider_account_id)
            .await
            .expect("account read")
            .is_some()
    );
}

#[tokio::test]
async fn authenticated_hosted_account_without_a_registered_instance_cannot_become_canonical() {
    let store = test_store().await;
    store.ensure_default_actors().await.expect("actors");
    store
        .ensure_default_provider_account()
        .await
        .expect("provider");
    store
        .update_provider_account_status(
            "provider_account:codex:default",
            noema_providers::ProviderAccountStatus::Authenticated,
            None,
            None,
        )
        .await
        .expect("authenticated metadata");

    let error = store
        .upsert_agent_runtime_preference(crate::NewAgentRuntimePreference {
            agent_id: "agent:primary".to_string(),
            provider_kind: "codex".to_string(),
            provider_account_id: "provider_account:codex:default".to_string(),
            model_profile: "gpt-5.6-luna".to_string(),
            reasoning_effort: None,
        })
        .await
        .expect_err("authenticated metadata is not runtime readiness");

    assert!(matches!(
        error,
        crate::StoreError::ProviderInstanceUnavailable { .. }
    ));
    assert!(
        store
            .get_agent_runtime_preference("agent:primary")
            .await
            .expect("preference read")
            .is_none()
    );
}
