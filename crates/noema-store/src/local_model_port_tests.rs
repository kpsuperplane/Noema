//! Local-model persistence port transaction and error-contract tests.

use noema_providers::{
    LocalModelActivationPersistence, LocalModelBackend, LocalModelInstallationPersistence,
    LocalModelInstallationStatus, LocalModelInstallationUpdate, LocalModelSourceKind,
    NewLocalModelInstallation, ProviderPersistenceError,
};

#[tokio::test]
async fn active_local_model_removal_has_a_typed_conflict() {
    let store = super::tests::test_store().await;
    let created =
        LocalModelInstallationPersistence::upsert_local_model_installation(&store, installation())
            .await
            .expect("create installation");
    for status in [
        LocalModelInstallationStatus::Downloading,
        LocalModelInstallationStatus::Verifying,
        LocalModelInstallationStatus::Installed,
    ] {
        LocalModelInstallationPersistence::update_local_model_installation(
            &store,
            &created.installation_id,
            LocalModelInstallationUpdate {
                status,
                downloaded_bytes: if status == LocalModelInstallationStatus::Downloading {
                    50
                } else {
                    100
                },
                expected_bytes: Some(100),
                sha256: None,
                blob_relative_path: (status == LocalModelInstallationStatus::Installed)
                    .then(|| "models/blobs/model.gguf".to_string()),
                error_code: None,
                error_message: None,
            },
        )
        .await
        .expect("advance installation");
    }
    LocalModelActivationPersistence::activate_local_model_as_system_default(
        &store,
        &created.installation_id,
    )
    .await
    .expect("activate installation");

    let error = LocalModelInstallationPersistence::remove_local_model_installation(
        &store,
        &created.installation_id,
    )
    .await
    .expect_err("active installation cannot be removed");
    assert_eq!(
        error,
        ProviderPersistenceError::ActiveInstallationConflict {
            installation_id: created.installation_id
        }
    );
}

#[tokio::test]
async fn local_model_port_returns_committed_state_and_classifies_invalid_input() {
    let store = super::tests::test_store().await;
    let created =
        LocalModelInstallationPersistence::upsert_local_model_installation(&store, installation())
            .await
            .expect("create installation");
    assert_eq!(
        LocalModelInstallationPersistence::local_model_installation(
            &store,
            &created.installation_id
        )
        .await
        .expect("read")
        .expect("installation"),
        created
    );
    let error = LocalModelInstallationPersistence::update_local_model_installation(
        &store,
        &created.installation_id,
        LocalModelInstallationUpdate {
            status: LocalModelInstallationStatus::Downloading,
            downloaded_bytes: 101,
            expected_bytes: Some(100),
            sha256: None,
            blob_relative_path: None,
            error_code: None,
            error_message: None,
        },
    )
    .await
    .expect_err("invalid byte count");
    assert_eq!(
        error,
        ProviderPersistenceError::InvalidRequest {
            kind: "downloaded_bytes_exceed_expected_bytes"
        }
    );
}

#[tokio::test]
async fn installation_state_rolls_back_when_event_append_fails() {
    let store = super::tests::test_store().await;
    let created =
        LocalModelInstallationPersistence::upsert_local_model_installation(&store, installation())
            .await
            .expect("create installation");
    {
        let connection = store.connection_for_tests();
        let connection = connection.lock().await;
        connection
            .execute_batch(
                r#"
                CREATE TEMP TRIGGER fail_local_model_progress_event
                BEFORE INSERT ON local_model_events
                WHEN NEW.kind = 'progress'
                BEGIN
                  SELECT RAISE(ABORT, 'forced local-model event failure');
                END;
                "#,
            )
            .expect("install failure trigger");
    }

    let error = LocalModelInstallationPersistence::update_local_model_installation(
        &store,
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
    .expect_err("event failure must roll back state");
    assert_eq!(
        error,
        ProviderPersistenceError::Persistence {
            operation: "update_local_model_installation"
        }
    );

    let installation = LocalModelInstallationPersistence::local_model_installation(
        &store,
        &created.installation_id,
    )
    .await
    .expect("read installation")
    .expect("installation");
    assert_eq!(installation.status, LocalModelInstallationStatus::Queued);
    let events = LocalModelInstallationPersistence::local_model_events(&store, None, 10)
        .await
        .expect("read events");
    assert_eq!(events.len(), 1);
}

#[tokio::test]
async fn activation_failure_rolls_back_every_earlier_write() {
    let store = super::tests::test_store().await;
    let created =
        LocalModelInstallationPersistence::upsert_local_model_installation(&store, installation())
            .await
            .expect("create installation");
    for status in [
        LocalModelInstallationStatus::Downloading,
        LocalModelInstallationStatus::Verifying,
        LocalModelInstallationStatus::Installed,
    ] {
        LocalModelInstallationPersistence::update_local_model_installation(
            &store,
            &created.installation_id,
            LocalModelInstallationUpdate {
                status,
                downloaded_bytes: if status == LocalModelInstallationStatus::Downloading {
                    50
                } else {
                    100
                },
                expected_bytes: Some(100),
                sha256: None,
                blob_relative_path: (status == LocalModelInstallationStatus::Installed)
                    .then(|| "models/blobs/model.gguf".to_string()),
                error_code: None,
                error_message: None,
            },
        )
        .await
        .expect("advance installation");
    }
    let event_count_before =
        LocalModelInstallationPersistence::local_model_events(&store, None, 20)
            .await
            .expect("events")
            .len();
    {
        let connection = store.connection_for_tests();
        let connection = connection.lock().await;
        connection
            .execute_batch(
                r#"
                CREATE TEMP TRIGGER fail_local_model_activation
                BEFORE INSERT ON task_model_pool_entries
                BEGIN
                  SELECT RAISE(ABORT, 'forced local-model activation failure');
                END;
                "#,
            )
            .expect("install failure trigger");
    }

    let error = LocalModelActivationPersistence::activate_local_model_as_system_default(
        &store,
        &created.installation_id,
    )
    .await
    .expect_err("activation must fail");
    assert_eq!(
        error,
        ProviderPersistenceError::Persistence {
            operation: "activate_local_model"
        }
    );
    {
        let connection = store.connection_for_tests();
        let connection = connection.lock().await;
        connection
            .execute_batch("DROP TRIGGER fail_local_model_activation")
            .expect("drop failure trigger");
    }

    let installation = LocalModelInstallationPersistence::local_model_installation(
        &store,
        &created.installation_id,
    )
    .await
    .expect("read installation")
    .expect("installation");
    assert!(!installation.is_active);
    assert_eq!(
        store
            .get_default_model_preference()
            .await
            .expect("default preference"),
        None
    );
    assert!(
        store
            .get_provider_account(noema_providers::LOCAL_MODELS_PROVIDER_ACCOUNT_ID)
            .await
            .expect("provider account")
            .is_none()
    );
    assert!(
        store
            .get_agent_runtime_preference("agent:primary")
            .await
            .expect("agent preference")
            .is_none()
    );
    let events = LocalModelInstallationPersistence::local_model_events(&store, None, 20)
        .await
        .expect("events");
    assert_eq!(events.len(), event_count_before);
}

fn installation() -> NewLocalModelInstallation {
    NewLocalModelInstallation {
        installation_id: "local_model_installation:port-test".to_string(),
        model_id: "port-test-model".to_string(),
        display_name: "Port Test".to_string(),
        source_kind: LocalModelSourceKind::Catalog,
        source_repo: Some("example/model".to_string()),
        source_revision: Some("a".repeat(40)),
        source_file: Some("model.gguf".to_string()),
        sha256: Some("b".repeat(64)),
        download_gb: 1.0,
        expected_bytes: Some(100),
        license: Some("Apache-2.0".to_string()),
        backend: LocalModelBackend::Cpu,
    }
}
