use noema_providers::{
    LOCAL_MODELS_PROVIDER_ACCOUNT_ID, LocalModelBackend, LocalModelEventKind,
    LocalModelInstallationStatus, LocalModelInstallationUpdate, NewLocalModelInstallation,
    ProviderAccountStatus, ProviderSelectionSnapshot, local_model_provider_instance_key,
    provider_account_instance_key,
};

use crate::tests::{mark_local_model_installed, ready_local_selection};

fn installation() -> NewLocalModelInstallation {
    crate::tests::local_model_installation(
        "local_model_installation:bonsai",
        "ternary-bonsai-8b",
        LocalModelBackend::Metal,
    )
}

#[tokio::test]
async fn installation_events_are_ordered_and_cancellation_preserves_progress() {
    let store = crate::tests::test_store().await;
    let created = store
        .upsert_local_model_installation(installation())
        .await
        .expect("create installation");
    assert_eq!(created.status, LocalModelInstallationStatus::Queued);
    assert_eq!(
        created.provider_instance_key,
        local_model_provider_instance_key(
            LOCAL_MODELS_PROVIDER_ACCOUNT_ID,
            &created.installation_id,
            &created.model_id,
        )
        .expect("installation provider key")
    );
    assert_eq!(created.retirement_claimed_at, None);

    let updated = store
        .update_local_model_installation(
            &created.installation_id,
            LocalModelInstallationUpdate {
                status: LocalModelInstallationStatus::Downloading,
                downloaded_bytes: 50,
                expected_bytes: Some(100),
                sha256: None,
                blob_relative_path: None,
                error_code: None,
                error_message: None,
            },
        )
        .await
        .expect("progress");
    assert_eq!(updated.downloaded_bytes, 50);
    let events = store
        .list_local_model_events(None, 10)
        .await
        .expect("events");
    assert_eq!(events.len(), 2);
    assert!(events[1].cursor > events[0].cursor);
    assert_eq!(events[1].kind, LocalModelEventKind::Progress);
    store
        .update_local_model_installation(
            &created.installation_id,
            LocalModelInstallationUpdate {
                status: LocalModelInstallationStatus::Downloading,
                downloaded_bytes: 80,
                expected_bytes: Some(100),
                sha256: None,
                blob_relative_path: None,
                error_code: None,
                error_message: None,
            },
        )
        .await
        .expect("latest progress");

    let cancelled = store
        .cancel_local_model_installation(&created.installation_id)
        .await
        .expect("cancel");

    assert_eq!(cancelled.status, LocalModelInstallationStatus::Cancelled);
    assert_eq!(cancelled.downloaded_bytes, 80);
}

#[tokio::test]
async fn terminal_cleanup_removes_only_cancelled_installations() {
    let store = crate::tests::test_store().await;
    let created = store
        .upsert_local_model_installation(installation())
        .await
        .expect("create installation");
    let cancelled = store
        .cancel_local_model_installation(&created.installation_id)
        .await
        .expect("cancel installation");

    let removed = store
        .remove_terminal_local_model_installation(&created.installation_id)
        .await
        .expect("remove");

    assert_eq!(removed.installation, cancelled);
}

#[tokio::test]
async fn activation_assigns_every_current_model_workload_atomically() {
    let store = crate::tests::test_store().await;
    let created = store
        .upsert_local_model_installation(installation())
        .await
        .expect("create installation");
    mark_local_model_installed(&store, &created).await;

    let ready_selection = ready_local_selection(&created);
    let activated = store
        .activate_local_model_as_system_default(&created.installation_id, &ready_selection)
        .await
        .expect("activate");
    assert_eq!(activated.installation_id, created.installation_id);
    assert_eq!(
        activated.provider_instance_key,
        created.provider_instance_key
    );
    assert!(activated.is_active);
    assert_eq!(activated.runtime_retired_at, None);

    for agent_id in [
        "agent:primary",
        "agent:task-executor",
        "agent:task-reviewer",
    ] {
        let agent = store
            .get_agent_runtime_preference(agent_id)
            .await
            .expect("agent preference")
            .expect("saved agent preference");
        assert_eq!(agent.provider_kind, "local_models");
        assert_eq!(agent.reasoning_effort, None);
    }
    let pools = store
        .list_task_model_pool_settings(None)
        .await
        .expect("task pools");
    assert_eq!(pools.len(), 3);
    assert!(
        pools
            .iter()
            .all(|entry| entry.model.provider_kind == "local_models")
    );
    for task in [
        crate::AuxiliaryModelTask::ToolProgressAudit,
        crate::AuxiliaryModelTask::WebFetchSummarizer,
        crate::AuxiliaryModelTask::MemoryConsolidation,
    ] {
        let auxiliary = store
            .get_auxiliary_model_preference(task)
            .await
            .expect("auxiliary preference")
            .expect("saved auxiliary preference");
        assert_eq!(auxiliary.provider_kind, "local_models");
    }

    let preference = crate::NewAgentRuntimePreference {
        agent_id: "agent:primary".to_string(),
        provider_kind: "local_models".to_string(),
        provider_account_id: LOCAL_MODELS_PROVIDER_ACCOUNT_ID.to_string(),
        model_profile: created.model_id.clone(),
        reasoning_effort: None,
    };
    let mut mismatched = ready_selection.selection().clone();
    mismatched.provider_instance_key = Some(
        noema_providers::ProviderInstanceKey::new("local-model:v1:mismatched")
            .expect("mismatched key"),
    );
    let mismatched = crate::tests::ready_provider_selection(mismatched);
    assert!(
        store
            .upsert_agent_runtime_preference_with_ready_selection(preference.clone(), &mismatched,)
            .await
            .is_err(),
        "local canonical change requires an exact ready proof"
    );
    let saved = store
        .upsert_agent_runtime_preference_with_ready_selection(preference, &ready_selection)
        .await
        .expect("proved local canonical write");
    assert_eq!(saved.provider_instance_key, created.provider_instance_key);

    let mut second_input = installation();
    second_input.installation_id = "local_model_installation:bonsai-second".to_string();
    let second = store
        .upsert_local_model_installation(second_input)
        .await
        .expect("second same-model installation");
    assert_eq!(created.model_id, second.model_id);
    assert_ne!(created.provider_instance_key, second.provider_instance_key);
}

#[tokio::test]
async fn provider_account_and_default_changes_preserve_explicit_workload_selections() {
    let store = crate::tests::test_store().await;
    let created = store
        .upsert_local_model_installation(installation())
        .await
        .expect("create installation");
    mark_local_model_installed(&store, &created).await;
    let ready_selection = ready_local_selection(&created);
    store
        .activate_local_model_as_system_default(&created.installation_id, &ready_selection)
        .await
        .expect("activate");
    store
        .ensure_default_provider_account()
        .await
        .expect("add Codex account");
    store
        .ensure_default_foundation_local_provider_account()
        .await
        .expect("add Foundation Models account");
    store
        .update_provider_account_status(
            "provider_account:codex:default",
            ProviderAccountStatus::Authenticated,
            None,
            None,
        )
        .await
        .expect("authenticate Codex account");

    let ready_selection =
        crate::tests::ready_provider_selection(ProviderSelectionSnapshot::explicit(
            "codex",
            "provider_account:codex:default",
            "gpt-5.6-luna",
            Some(noema_providers::ReasoningEffort::High),
            Some("default_preference_test".to_string()),
        ));
    let preference = store
        .save_default_model_preference_with_ready_selection(
            "codex",
            "provider_account:codex:default",
            "gpt-5.6-luna",
            Some("high"),
            &ready_selection,
        )
        .await
        .expect("save default");
    assert_eq!(preference.provider_kind, "codex");
    assert_eq!(
        preference.provider_instance_key,
        provider_account_instance_key("provider_account:codex:default")
            .expect("hosted provider key")
    );

    let primary = store
        .get_agent_runtime_preference("agent:primary")
        .await
        .expect("primary preference")
        .expect("saved primary preference");
    assert_eq!(primary.provider_kind, "local_models");
    assert_eq!(primary.model_profile, "ternary-bonsai-8b");
    assert!(
        store
            .list_task_model_pool_settings(None)
            .await
            .expect("task pools")
            .iter()
            .all(|entry| entry.model.provider_kind == "local_models")
    );
}
