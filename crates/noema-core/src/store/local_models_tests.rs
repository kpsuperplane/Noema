use noema_providers::{
    LocalModelBackend, LocalModelEventKind, LocalModelInstallationStatus,
    LocalModelInstallationUpdate, LocalModelSourceKind, NewLocalModelInstallation,
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
    let store = crate::store::tests::test_store().await;
    let created = store
        .upsert_local_model_installation(installation())
        .await
        .expect("create installation");
    assert_eq!(created.status, LocalModelInstallationStatus::Queued);

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
    let store = crate::store::tests::test_store().await;
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
async fn removal_returns_the_exact_deleted_blob_projection() {
    let store = crate::store::tests::test_store().await;
    let created = store
        .upsert_local_model_installation(installation())
        .await
        .expect("create installation");
    mark_installed(&store, &created).await;

    let removed = store
        .remove_local_model_installation(&created.installation_id)
        .await
        .expect("remove");

    assert_eq!(
        removed.installation.status,
        LocalModelInstallationStatus::Installed
    );
    assert_eq!(
        removed.installation.blob_relative_path.as_deref(),
        Some("models/blobs/bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb.gguf")
    );
    assert_eq!(
        removed.unreferenced_blob_relative_path,
        removed.installation.blob_relative_path
    );
}

#[tokio::test]
async fn activation_assigns_every_current_model_workload_atomically() {
    let store = crate::store::tests::test_store().await;
    let created = store
        .upsert_local_model_installation(installation())
        .await
        .expect("create installation");
    mark_installed(&store, &created).await;

    let preference = store
        .activate_local_model_as_system_default(&created.installation_id)
        .await
        .expect("activate");
    assert_eq!(preference.provider_kind, "local_models");
    assert_eq!(preference.model_profile, created.model_id);

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
async fn adding_other_providers_does_not_replace_local_workload_selections() {
    let store = crate::store::tests::test_store().await;
    let created = store
        .upsert_local_model_installation(installation())
        .await
        .expect("create installation");
    mark_installed(&store, &created).await;
    store
        .activate_local_model_as_system_default(&created.installation_id)
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
    let store = crate::store::tests::test_store().await;
    let created = store
        .upsert_local_model_installation(installation())
        .await
        .expect("create installation");
    mark_installed(&store, &created).await;
    store
        .activate_local_model_as_system_default(&created.installation_id)
        .await
        .expect("activate");
    store
        .ensure_default_provider_account()
        .await
        .expect("add Codex account");

    let preference = store
        .save_default_model_preference(
            "codex",
            "provider_account:codex:default",
            "gpt-5.6-luna",
            Some("high"),
        )
        .await
        .expect("save default");
    assert_eq!(preference.provider_kind, "codex");

    let primary = store
        .get_agent_runtime_preference("agent:primary")
        .await
        .expect("primary preference")
        .expect("saved primary preference");
    assert_eq!(primary.provider_kind, "local_models");
    assert_eq!(primary.model_profile, "ternary-bonsai-8b");
}

#[tokio::test]
async fn terminal_installation_state_cannot_be_regressed_by_a_worker() {
    let store = crate::store::tests::test_store().await;
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
