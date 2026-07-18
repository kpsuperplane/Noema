//! Local-model persistence port transaction and error-contract tests.

use noema_providers::{
    LOCAL_MODELS_PROVIDER_ACCOUNT_ID, LocalModelActivationPersistence, LocalModelBackend,
    LocalModelInstallationPersistence, LocalModelInstallationStatus, LocalModelInstallationUpdate,
    NewLocalModelInstallation, ProviderInstanceKey, ProviderPersistenceError,
    ProviderReadySelection,
};

use crate::tests::mark_local_model_installed;

#[tokio::test]
async fn installation_state_rolls_back_when_event_append_fails() {
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
        .expect("committed read"),
        Some(created.clone())
    );
    assert_eq!(
        LocalModelInstallationPersistence::update_local_model_installation(
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
        .expect_err("invalid byte count"),
        ProviderPersistenceError::InvalidRequest {
            kind: "downloaded_bytes_exceed_expected_bytes"
        }
    );
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

    let persisted = LocalModelInstallationPersistence::local_model_installation(
        &store,
        &created.installation_id,
    )
    .await
    .expect("read installation")
    .expect("installation");
    assert_eq!(persisted.status, LocalModelInstallationStatus::Queued);
    let events = LocalModelInstallationPersistence::local_model_events(&store, None, 10)
        .await
        .expect("read events");
    assert_eq!(events.len(), 1);

    let active_store = super::tests::test_store().await;
    let active = LocalModelInstallationPersistence::upsert_local_model_installation(
        &active_store,
        installation(),
    )
    .await
    .expect("active installation");
    mark_local_model_installed(&active_store, &active).await;
    LocalModelActivationPersistence::activate_local_model_as_system_default(
        &active_store,
        &active.installation_id,
        &ready_local_selection(&active.model_id, &active.provider_instance_key),
    )
    .await
    .expect("activate");
    assert_eq!(
        LocalModelInstallationPersistence::remove_terminal_local_model_installation(
            &active_store,
            &active.installation_id,
        )
        .await
        .expect_err("active removal conflict"),
        ProviderPersistenceError::ActiveInstallationConflict {
            installation_id: active.installation_id
        }
    );
}

#[tokio::test]
async fn activation_failure_rolls_back_every_earlier_write() {
    let store = super::tests::test_store().await;
    let created =
        LocalModelInstallationPersistence::upsert_local_model_installation(&store, installation())
            .await
            .expect("create installation");
    mark_local_model_installed(&store, &created).await;
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

    let ready_selection = ready_local_selection(&created.model_id, &created.provider_instance_key);
    let error = LocalModelActivationPersistence::activate_local_model_as_system_default(
        &store,
        &created.installation_id,
        &ready_selection,
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

#[tokio::test]
async fn activation_rejects_an_instance_key_that_does_not_own_the_installation() {
    let store = super::tests::test_store().await;
    let created =
        LocalModelInstallationPersistence::upsert_local_model_installation(&store, installation())
            .await
            .expect("create installation");
    let wrong_key = noema_providers::ProviderInstanceKey::new("local-model:v1:wrong")
        .expect("wrong instance key");
    let ready_selection = ready_local_selection(&created.model_id, &wrong_key);

    let error = LocalModelActivationPersistence::activate_local_model_as_system_default(
        &store,
        &created.installation_id,
        &ready_selection,
    )
    .await
    .expect_err("mismatched key must fail closed");

    assert_eq!(
        error,
        ProviderPersistenceError::Invariant {
            operation: "activate_local_model"
        }
    );
}

fn ready_local_selection(model_id: &str, key: &ProviderInstanceKey) -> ProviderReadySelection {
    crate::tests::ready_provider_selection(crate::tests::exact_provider_selection(
        "local_models",
        LOCAL_MODELS_PROVIDER_ACCOUNT_ID,
        model_id,
        key.clone(),
        "local_model_activation_port_test",
    ))
}

fn installation() -> NewLocalModelInstallation {
    crate::tests::local_model_installation(
        "local_model_installation:port-test",
        "port-test-model",
        LocalModelBackend::Cpu,
    )
}
