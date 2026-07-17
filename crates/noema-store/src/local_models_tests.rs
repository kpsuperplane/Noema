use noema_providers::{
    LOCAL_MODELS_PROVIDER_ACCOUNT_ID, LocalModelBackend, LocalModelEventKind,
    LocalModelInstallationStatus, LocalModelInstallationUpdate, LocalModelSourceKind,
    NewLocalModelInstallation, ProviderAccountStatus, ProviderReadySelection,
    ProviderSelectionSnapshot, local_model_provider_instance_key, provider_account_instance_key,
};

fn installation() -> NewLocalModelInstallation {
    NewLocalModelInstallation {
        installation_id: "local_model_installation:bonsai".to_string(),
        model_id: "ternary-bonsai-8b".to_string(),
        display_name: "Ternary Bonsai 8B".to_string(),
        source_kind: LocalModelSourceKind::Catalog,
        source_repo: Some("vinpix/Bonsai-8B-llama.cpp".to_string()),
        source_revision: Some("a".repeat(40)),
        source_file: Some("Bonsai-8B-Q2_KT.gguf".to_string()),
        sha256: Some("b".repeat(64)),
        download_gb: 3.0,
        expected_bytes: Some(100),
        license: Some("Apache-2.0".to_string()),
        backend: LocalModelBackend::Metal,
    }
}

async fn mark_installed(
    store: &crate::NoemaStore,
    installation: &noema_providers::LocalModelInstallationRecord,
) {
    for status in [
        LocalModelInstallationStatus::Downloading,
        LocalModelInstallationStatus::Verifying,
        LocalModelInstallationStatus::Installed,
    ] {
        store
            .update_local_model_installation(
                &installation.installation_id,
                LocalModelInstallationUpdate {
                    status,
                    downloaded_bytes: if status == LocalModelInstallationStatus::Downloading {
                        50
                    } else {
                        100
                    },
                    expected_bytes: Some(100),
                    sha256: None,
                    blob_relative_path: (status == LocalModelInstallationStatus::Installed).then(
                        || {
                            format!(
                                "models/blobs/{}.gguf",
                                installation.sha256.as_deref().expect("catalog digest")
                            )
                        },
                    ),
                    error_code: None,
                    error_message: None,
                },
            )
            .await
            .expect("installation transition");
    }
}

#[tokio::test]
async fn installation_updates_append_cursor_events() {
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
}

#[tokio::test]
async fn cancellation_preserves_the_latest_durable_progress() {
    let store = crate::tests::test_store().await;
    let created = store
        .upsert_local_model_installation(installation())
        .await
        .expect("create installation");
    for downloaded_bytes in [40, 80] {
        store
            .update_local_model_installation(
                &created.installation_id,
                LocalModelInstallationUpdate {
                    status: LocalModelInstallationStatus::Downloading,
                    downloaded_bytes,
                    expected_bytes: Some(100),
                    sha256: None,
                    blob_relative_path: None,
                    error_code: None,
                    error_message: None,
                },
            )
            .await
            .expect("progress");
    }

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
    mark_installed(&store, &created).await;

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
    let memory = store
        .memory_service_settings()
        .await
        .expect("memory settings");
    assert_eq!(memory.provider_kind.as_deref(), Some("local_models"));
    for task_id in ["tool_progress_audit", "web_fetch_summarizer"] {
        let auxiliary = store
            .get_auxiliary_model_preference(task_id)
            .await
            .expect("auxiliary preference")
            .expect("saved auxiliary preference");
        assert_eq!(auxiliary.provider_kind, "local_models");
    }
}

#[tokio::test]
async fn changing_a_local_canonical_selection_requires_a_ready_registry_proof() {
    let store = crate::tests::test_store().await;
    let created = store
        .upsert_local_model_installation(installation())
        .await
        .expect("create installation");
    mark_installed(&store, &created).await;
    let ready_selection = ready_local_selection(&created);
    store
        .activate_local_model_as_system_default(&created.installation_id, &ready_selection)
        .await
        .expect("activate");
    let preference = crate::NewAgentRuntimePreference {
        agent_id: "agent:primary".to_string(),
        provider_kind: "local_models".to_string(),
        provider_account_id: LOCAL_MODELS_PROVIDER_ACCOUNT_ID.to_string(),
        model_profile: created.model_id.clone(),
        reasoning_effort: None,
    };

    let unproved_error = store
        .upsert_agent_runtime_preference(preference.clone())
        .await
        .expect_err("local selection must prove registry readiness");
    assert!(matches!(
        unproved_error,
        crate::StoreError::ProviderInstanceUnavailable { .. }
    ));

    let mut exact_selection = ProviderSelectionSnapshot::explicit(
        "local_models",
        LOCAL_MODELS_PROVIDER_ACCOUNT_ID,
        &created.model_id,
        None,
        Some("test_ready_selection".to_string()),
    );
    exact_selection.provider_instance_key = Some(created.provider_instance_key.clone());
    let ready_selection = crate::tests::ready_provider_selection(exact_selection);
    let saved = store
        .upsert_agent_runtime_preference_with_ready_selection(preference, &ready_selection)
        .await
        .expect("proved local selection");

    assert_eq!(saved.provider_instance_key, created.provider_instance_key);
}

#[tokio::test]
async fn adding_other_providers_does_not_replace_local_workload_selections() {
    let store = crate::tests::test_store().await;
    let created = store
        .upsert_local_model_installation(installation())
        .await
        .expect("create installation");
    mark_installed(&store, &created).await;
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

#[tokio::test]
async fn saving_a_default_does_not_rewrite_explicit_workload_selections() {
    let store = crate::tests::test_store().await;
    let created = store
        .upsert_local_model_installation(installation())
        .await
        .expect("create installation");
    mark_installed(&store, &created).await;
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
        .update_provider_account_status(
            "provider_account:codex:default",
            ProviderAccountStatus::Authenticated,
            None,
            None,
        )
        .await
        .expect("authenticate Codex account");

    let error = store
        .save_default_model_preference(
            "codex",
            "provider_account:codex:default",
            "gpt-5.6-luna",
            Some("high"),
        )
        .await
        .expect_err("authenticated metadata is not runtime readiness");
    assert!(matches!(
        error,
        crate::StoreError::ProviderInstanceUnavailable { .. }
    ));

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
}

fn ready_local_selection(
    installation: &noema_providers::LocalModelInstallationRecord,
) -> ProviderReadySelection {
    let mut selection = ProviderSelectionSnapshot::explicit(
        "local_models",
        LOCAL_MODELS_PROVIDER_ACCOUNT_ID,
        &installation.model_id,
        None,
        Some("local_model_activation_test".to_string()),
    );
    selection.provider_instance_key = Some(installation.provider_instance_key.clone());
    crate::tests::ready_provider_selection(selection)
}

#[tokio::test]
async fn same_model_installations_receive_distinct_exact_instance_keys() {
    let store = crate::tests::test_store().await;
    let first = store
        .upsert_local_model_installation(installation())
        .await
        .expect("first installation");
    let mut second_input = installation();
    second_input.installation_id = "local_model_installation:bonsai-second".to_string();
    let second = store
        .upsert_local_model_installation(second_input)
        .await
        .expect("second installation");

    assert_eq!(first.model_id, second.model_id);
    assert_ne!(first.installation_id, second.installation_id);
    assert_ne!(first.provider_instance_key, second.provider_instance_key);
}

#[tokio::test]
async fn terminal_installation_state_cannot_be_regressed_by_a_worker() {
    let store = crate::tests::test_store().await;
    let created = store
        .upsert_local_model_installation(installation())
        .await
        .expect("create installation");
    store
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
        .expect("start download");
    store
        .cancel_local_model_installation(&created.installation_id)
        .await
        .expect("cancel installation");

    let error = store
        .update_local_model_installation(
            &created.installation_id,
            LocalModelInstallationUpdate {
                status: LocalModelInstallationStatus::Installed,
                downloaded_bytes: 100,
                expected_bytes: Some(100),
                sha256: None,
                blob_relative_path: Some("models/blobs/cancelled.gguf".to_string()),
                error_code: None,
                error_message: None,
            },
        )
        .await
        .expect_err("cancelled installation must be terminal");
    assert!(
        error
            .to_string()
            .contains("cannot transition from cancelled")
    );
}
