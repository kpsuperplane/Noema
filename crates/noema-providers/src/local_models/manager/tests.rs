mod fakes;

use std::{sync::Arc, time::Duration};

use futures_util::StreamExt;
use tokio::time::{sleep, timeout};

use crate::{
    LOCAL_MODELS_PROVIDER_ACCOUNT_ID, LocalModelBackend, LocalModelEventKind,
    LocalModelInstallationStatus, ProviderSelectionSnapshot,
};

use super::{
    LocalModelManager, LocalModelManagerConfig, LocalModelManagerError, LocalModelManagerEvent,
    LocalModelRuntimeStatus,
};
use crate::LocalFileModelImport;
use fakes::{FakeProcessFactory, FakeRepository, installed_record};

fn manager(
    repository: Arc<FakeRepository>,
    factory: Arc<FakeProcessFactory>,
    paths: &noema_home::NoemaPaths,
) -> LocalModelManager {
    manager_with_diagnostics(repository, factory, paths, None)
}

fn manager_with_diagnostics(
    repository: Arc<FakeRepository>,
    factory: Arc<FakeProcessFactory>,
    paths: &noema_home::NoemaPaths,
    system_errors: Option<noema_home::SystemErrorLogger>,
) -> LocalModelManager {
    let installations: Arc<dyn crate::LocalModelInstallationPersistence> = repository.clone();
    let activation: Arc<dyn crate::LocalModelActivationPersistence> = repository;
    LocalModelManager::new_with_factory(
        installations,
        activation,
        paths.clone(),
        factory,
        system_errors,
    )
    .expect("manager")
}

fn local_selection(model_id: &str) -> ProviderSelectionSnapshot {
    ProviderSelectionSnapshot::explicit(
        "local_models",
        LOCAL_MODELS_PROVIDER_ACCOUNT_ID,
        model_id,
        None,
        Some("manager-test".to_string()),
    )
}

#[tokio::test]
async fn two_same_model_installations_keep_distinct_keys_and_old_leases() {
    let home = tempfile::tempdir().expect("home");
    let paths = noema_home::NoemaPaths::from_noema_home(home.path()).expect("paths");
    let repository = Arc::new(FakeRepository::default());
    repository.insert(installed_record("first", "shared-model", 'a', false));
    repository.insert(installed_record("second", "shared-model", 'b', false));
    let factory = Arc::new(FakeProcessFactory::new(repository.log()));
    let manager = manager(repository, Arc::clone(&factory), &paths);

    manager.activate("first").await.expect("first activation");
    let old_lease = manager
        .route_handle()
        .resolve_snapshot(local_selection("shared-model"))
        .await
        .expect("old lease");
    manager.activate("second").await.expect("second activation");
    let new_lease = manager
        .route_handle()
        .resolve_snapshot(local_selection("shared-model"))
        .await
        .expect("new lease");

    assert_ne!(old_lease.key(), new_lease.key());
    assert!(old_lease.key().as_str().contains("first"));
    assert!(new_lease.key().as_str().contains("second"));
    assert_eq!(manager.managed_instances().await.len(), 2);
    assert_eq!(factory.process("first").shutdowns(), 0);

    drop(old_lease);
    drop(new_lease);
    manager.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn failed_activation_stops_only_new_process_and_preserves_old_route() {
    let home = tempfile::tempdir().expect("home");
    let paths = noema_home::NoemaPaths::from_noema_home(home.path()).expect("paths");
    let repository = Arc::new(FakeRepository::default());
    repository.insert(installed_record("first", "shared-model", 'a', false));
    repository.insert(installed_record("second", "shared-model", 'b', false));
    let factory = Arc::new(FakeProcessFactory::new(repository.log()));
    let manager = manager(Arc::clone(&repository), Arc::clone(&factory), &paths);

    manager.activate("first").await.expect("first activation");
    let old_key = manager
        .route_handle()
        .active_key()
        .await
        .expect("old active key");
    repository.fail_next_activation();
    let error = manager
        .activate("second")
        .await
        .expect_err("injected persistence failure");

    assert!(matches!(error, LocalModelManagerError::Persistence(_)));
    assert_eq!(manager.route_handle().active_key().await, Some(old_key));
    assert_eq!(factory.process("first").shutdowns(), 0);
    assert_eq!(factory.process("second").shutdowns(), 1);
    assert_eq!(manager.managed_instances().await.len(), 1);

    manager.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn aborted_activation_remains_manager_owned_and_shutdown_stops_it() {
    let home = tempfile::tempdir().expect("home");
    let paths = noema_home::NoemaPaths::from_noema_home(home.path()).expect("paths");
    let repository = Arc::new(FakeRepository::default());
    repository.insert(installed_record("first", "shared-model", 'a', false));
    repository.insert(installed_record("second", "shared-model", 'b', false));
    let factory = Arc::new(FakeProcessFactory::new(repository.log()));
    let manager = manager(repository, Arc::clone(&factory), &paths);

    manager.activate("first").await.expect("first activation");
    let route = manager.route_handle();
    let route_read = route.read().await;
    let activating_manager = manager.clone();
    let activation = tokio::spawn(async move { activating_manager.activate("second").await });
    timeout(Duration::from_secs(1), async {
        while !factory.started_ids().iter().any(|id| id == "second") {
            sleep(Duration::from_millis(1)).await;
        }
    })
    .await
    .expect("second process registered");

    activation.abort();
    let _ = activation.await;
    let shutdown_manager = manager.clone();
    let shutdown = tokio::spawn(async move { shutdown_manager.shutdown().await });
    sleep(Duration::from_millis(20)).await;
    assert!(!shutdown.is_finished());
    drop(route_read);
    shutdown
        .await
        .expect("shutdown task")
        .expect("shutdown result");

    assert_eq!(factory.process("first").shutdowns(), 1);
    assert_eq!(factory.process("second").shutdowns(), 1);
}

#[tokio::test]
async fn activation_that_commits_during_shutdown_finishes_before_retirement() {
    let home = tempfile::tempdir().expect("home");
    let paths = noema_home::NoemaPaths::from_noema_home(home.path()).expect("paths");
    let repository = Arc::new(FakeRepository::default());
    repository.insert(installed_record("first", "shared-model", 'a', false));
    repository.insert(installed_record("second", "shared-model", 'b', false));
    let factory = Arc::new(FakeProcessFactory::new(repository.log()));
    let manager = manager(Arc::clone(&repository), Arc::clone(&factory), &paths);

    manager.activate("first").await.expect("first activation");
    repository.pause_next_activation();
    let activating_manager = manager.clone();
    let activation = tokio::spawn(async move { activating_manager.activate("second").await });
    repository.wait_for_activation().await;
    let shutdown_manager = manager.clone();
    let shutdown = tokio::spawn(async move { shutdown_manager.shutdown().await });
    sleep(Duration::from_millis(20)).await;
    repository.release_activation();

    let activated = activation
        .await
        .expect("activation task")
        .expect("committed activation");
    assert_eq!(activated.installation_id, "second");
    assert!(activated.is_active);
    shutdown
        .await
        .expect("shutdown task")
        .expect("shutdown result");
    assert!(
        repository
            .record("second")
            .expect("second record")
            .is_active
    );
    assert_eq!(factory.process("first").shutdowns(), 1);
    assert_eq!(factory.process("second").shutdowns(), 1);
}

#[tokio::test]
async fn replacement_status_cannot_be_overwritten_by_retired_forwarder() {
    let home = tempfile::tempdir().expect("home");
    let paths = noema_home::NoemaPaths::from_noema_home(home.path()).expect("paths");
    let repository = Arc::new(FakeRepository::default());
    repository.insert(installed_record("first", "shared-model", 'a', false));
    repository.insert(installed_record("second", "shared-model", 'b', false));
    let factory = Arc::new(FakeProcessFactory::new(repository.log()));
    let manager = manager(repository, Arc::clone(&factory), &paths);

    manager.activate("first").await.expect("first activation");
    let first = factory.process("first");
    first.set_status(LocalModelRuntimeStatus::Failed {
        message: "retired process failure".to_string(),
    });
    manager.activate("second").await.expect("second activation");
    first.set_status(LocalModelRuntimeStatus::Stopped);
    sleep(Duration::from_millis(20)).await;

    assert!(matches!(
        manager.runtime_status(),
        LocalModelRuntimeStatus::Ready { endpoint, .. }
            if endpoint == "http://second.invalid/"
    ));
    manager.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn inactive_remove_waits_for_lease_then_stops_before_persistence_delete() {
    let home = tempfile::tempdir().expect("home");
    let paths = noema_home::NoemaPaths::from_noema_home(home.path()).expect("paths");
    let repository = Arc::new(FakeRepository::default());
    repository.insert(installed_record("first", "shared-model", 'a', false));
    repository.insert(installed_record("second", "shared-model", 'b', false));
    let factory = Arc::new(FakeProcessFactory::new(repository.log()));
    let manager = manager(Arc::clone(&repository), Arc::clone(&factory), &paths);

    manager.activate("first").await.expect("first activation");
    let old_lease = manager
        .route_handle()
        .resolve_snapshot(local_selection("shared-model"))
        .await
        .expect("old lease");
    manager.activate("second").await.expect("second activation");
    let removing_manager = manager.clone();
    let removal = tokio::spawn(async move { removing_manager.remove("first").await });

    sleep(Duration::from_millis(30)).await;
    assert!(!removal.is_finished());
    assert!(repository.log().lock().expect("log lock").is_empty());
    drop(old_lease);
    removal.await.expect("removal task").expect("remove first");

    {
        let log = repository.log();
        let log = log.lock().expect("log lock");
        let stop = log
            .iter()
            .position(|entry| entry == "stop:first")
            .expect("stop log");
        let remove = log
            .iter()
            .position(|entry| entry == "remove:first")
            .expect("remove log");
        assert!(stop < remove);
    }
    assert!(repository.record("first").is_none());
    assert!(matches!(
        manager.remove("second").await,
        Err(LocalModelManagerError::ActiveInstallation { .. })
    ));

    manager.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn stop_failure_preserves_inactive_row_and_blob_deletion_intent() {
    let home = tempfile::tempdir().expect("home");
    let paths = noema_home::NoemaPaths::from_noema_home(home.path()).expect("paths");
    let repository = Arc::new(FakeRepository::default());
    repository.insert(installed_record("first", "shared-model", 'a', false));
    repository.insert(installed_record("second", "shared-model", 'b', false));
    let factory = Arc::new(FakeProcessFactory::new(repository.log()));
    let manager = manager(Arc::clone(&repository), Arc::clone(&factory), &paths);

    manager.activate("first").await.expect("first activation");
    manager.activate("second").await.expect("second activation");
    factory.process("first").fail_shutdown();
    let error = manager
        .remove("first")
        .await
        .expect_err("injected stop failure");

    assert!(matches!(error, LocalModelManagerError::Runtime { .. }));
    assert!(repository.record("first").is_some());
    assert!(
        !repository
            .log()
            .lock()
            .expect("log lock")
            .iter()
            .any(|entry| entry == "remove:first")
    );
}

#[tokio::test]
async fn restart_reconstructs_only_active_and_transient_failure_keeps_manager_usable() {
    let home = tempfile::tempdir().expect("home");
    let paths = noema_home::NoemaPaths::from_noema_home(home.path()).expect("paths");
    let repository = Arc::new(FakeRepository::default());
    repository.insert(installed_record("active", "shared-model", 'a', true));
    repository.insert(installed_record("inactive", "shared-model", 'b', false));
    let factory = Arc::new(FakeProcessFactory::new(repository.log()));
    factory.fail_start("active");
    let diagnostics_path = home.path().join("errors.log");
    let manager = manager_with_diagnostics(
        repository,
        Arc::clone(&factory),
        &paths,
        Some(noema_home::SystemErrorLogger::new(diagnostics_path.clone())),
    );

    manager
        .start_active_installation()
        .await
        .expect("transient start failure remains usable");
    assert!(matches!(
        manager.runtime_status(),
        LocalModelRuntimeStatus::Failed { .. }
    ));
    assert!(factory.started_ids().is_empty());
    let diagnostics = std::fs::read_to_string(diagnostics_path).expect("runtime diagnostic");
    assert!(diagnostics.contains("\"category\":\"local_model_runtime_unavailable\""));
    assert!(diagnostics.contains("start_fake_process"));
    assert!(diagnostics.contains("injected start failure"));

    factory.allow_start("active");
    manager
        .retry_active_installation()
        .await
        .expect("retry active installation");
    assert_eq!(factory.started_ids(), vec!["active"]);
    assert_eq!(manager.managed_instances().await.len(), 1);

    manager.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn restart_with_missing_blob_is_nonfatal_and_diagnostic() {
    let home = tempfile::tempdir().expect("home");
    let paths = noema_home::NoemaPaths::from_noema_home(home.path()).expect("paths");
    let diagnostics_path = home.path().join("errors.log");
    let repository = Arc::new(FakeRepository::default());
    repository.insert(installed_record("active", "shared-model", 'a', true));
    let installations: Arc<dyn crate::LocalModelInstallationPersistence> = repository.clone();
    let activation: Arc<dyn crate::LocalModelActivationPersistence> = repository;
    let manager = LocalModelManager::new(
        installations,
        activation,
        paths,
        LocalModelManagerConfig {
            runtime_root: None,
            context_window_tokens: 8_192,
            timeout_seconds: 30,
            startup_timeout_seconds: 30,
            system_errors: Some(noema_home::SystemErrorLogger::new(diagnostics_path.clone())),
        },
    )
    .expect("manager");

    manager
        .start_active_installation()
        .await
        .expect("missing blob remains recoverable");
    assert!(matches!(
        manager.runtime_status(),
        LocalModelRuntimeStatus::Failed { .. }
    ));
    let diagnostics = std::fs::read_to_string(diagnostics_path).expect("runtime diagnostic");
    assert!(diagnostics.contains("\"category\":\"local_model_runtime_unavailable\""));
    assert!(diagnostics.contains("installed model blob is unavailable"));
    manager.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn retry_replaces_a_failed_retained_process_and_preserves_the_exact_route() {
    let home = tempfile::tempdir().expect("home");
    let paths = noema_home::NoemaPaths::from_noema_home(home.path()).expect("paths");
    let repository = Arc::new(FakeRepository::default());
    repository.insert(installed_record("active", "shared-model", 'a', true));
    let factory = Arc::new(FakeProcessFactory::new(repository.log()));
    let manager = manager(repository, Arc::clone(&factory), &paths);

    manager
        .start_active_installation()
        .await
        .expect("start active installation");
    let failed = factory.process("active");
    failed.set_status(LocalModelRuntimeStatus::Failed {
        message: "injected runtime failure".to_string(),
    });

    let status = manager
        .retry_active_installation()
        .await
        .expect("retry failed process");
    let replacement = factory.process("active");

    assert!(matches!(status, LocalModelRuntimeStatus::Ready { .. }));
    assert!(!Arc::ptr_eq(&failed, &replacement));
    assert_eq!(failed.shutdowns(), 1);
    assert_eq!(replacement.shutdowns(), 0);
    let lease = manager
        .route_handle()
        .resolve_snapshot(local_selection("shared-model"))
        .await
        .expect("replacement route");
    assert!(lease.key().as_str().contains("active"));
    drop(lease);

    manager.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn shutdown_rejects_new_work_drains_lease_and_stops_every_process() {
    let home = tempfile::tempdir().expect("home");
    let paths = noema_home::NoemaPaths::from_noema_home(home.path()).expect("paths");
    let repository = Arc::new(FakeRepository::default());
    repository.insert(installed_record("first", "shared-model", 'a', false));
    repository.insert(installed_record("second", "shared-model", 'b', false));
    let factory = Arc::new(FakeProcessFactory::new(repository.log()));
    let manager = manager(repository, Arc::clone(&factory), &paths);

    manager.activate("first").await.expect("first activation");
    let old_lease = manager
        .route_handle()
        .resolve_snapshot(local_selection("shared-model"))
        .await
        .expect("old lease");
    manager.activate("second").await.expect("second activation");
    let shutdown_manager = manager.clone();
    let shutdown = tokio::spawn(async move { shutdown_manager.shutdown().await });
    sleep(Duration::from_millis(30)).await;

    assert!(!shutdown.is_finished());
    assert!(matches!(
        manager.activate("second").await,
        Err(LocalModelManagerError::ShuttingDown)
    ));
    assert!(
        manager
            .route_handle()
            .resolve_snapshot(local_selection("shared-model"))
            .await
            .is_err()
    );
    drop(old_lease);
    shutdown
        .await
        .expect("shutdown task")
        .expect("shutdown result");

    assert_eq!(factory.process("first").shutdowns(), 1);
    assert_eq!(factory.process("second").shutdowns(), 1);
    assert_eq!(manager.runtime_status(), LocalModelRuntimeStatus::Stopped);
}

#[tokio::test]
async fn distinct_install_workers_never_overlap_global_artifact_mutation() {
    let home = tempfile::tempdir().expect("home");
    let paths = noema_home::NoemaPaths::from_noema_home(home.path()).expect("paths");
    let repository = Arc::new(FakeRepository::default());
    repository.delay_mutations();
    let factory = Arc::new(FakeProcessFactory::new(repository.log()));
    let manager = manager(Arc::clone(&repository), factory, &paths);
    let first_path = home.path().join("first.gguf");
    let second_path = home.path().join("second.gguf");
    tokio::fs::write(&first_path, b"GGUF first")
        .await
        .expect("first model");
    tokio::fs::write(&second_path, b"GGUF second")
        .await
        .expect("second model");

    let (first, second) = tokio::join!(
        manager.import_local_file(LocalFileModelImport {
            name: "First".to_string(),
            model_id: "first".to_string(),
            path: first_path,
            license: None,
            backend: LocalModelBackend::Cpu,
        }),
        manager.import_local_file(LocalFileModelImport {
            name: "Second".to_string(),
            model_id: "second".to_string(),
            path: second_path,
            license: None,
            backend: LocalModelBackend::Cpu,
        })
    );
    first.expect("first queued");
    second.expect("second queued");

    timeout(Duration::from_secs(3), async {
        loop {
            let records = manager.installations().await.expect("installations");
            if records.len() == 2
                && records
                    .iter()
                    .all(|record| record.status == LocalModelInstallationStatus::Installed)
            {
                break;
            }
            sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("workers completed");

    assert_eq!(repository.max_active_mutations(), 1);
    manager.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn one_local_model_operation_is_retained_per_installation() {
    let home = tempfile::tempdir().expect("home");
    let paths = noema_home::NoemaPaths::from_noema_home(home.path()).expect("paths");
    let repository = Arc::new(FakeRepository::default());
    repository.pause_next_upsert();
    let factory = Arc::new(FakeProcessFactory::new(repository.log()));
    let manager = manager(Arc::clone(&repository), factory, &paths);
    let model_path = home.path().join("duplicate.gguf");
    let mut model_bytes = vec![0_u8; 4 * 1024 * 1024];
    model_bytes[..4].copy_from_slice(b"GGUF");
    tokio::fs::write(&model_path, model_bytes)
        .await
        .expect("model fixture");
    let input = LocalFileModelImport {
        name: "Duplicate".to_string(),
        model_id: "duplicate".to_string(),
        path: model_path,
        license: None,
        backend: LocalModelBackend::Cpu,
    };

    let first_manager = manager.clone();
    let first_input = input.clone();
    let first = tokio::spawn(async move { first_manager.import_local_file(first_input).await });
    repository.wait_for_upsert().await;
    let second_manager = manager.clone();
    let second = tokio::spawn(async move { second_manager.import_local_file(input).await });
    tokio::task::yield_now().await;
    repository.release_upsert();
    let first = first.await.expect("first task").expect("first import");
    let second = second
        .await
        .expect("second task")
        .expect("duplicate import");

    assert_eq!(first.installation_id, second.installation_id);
    timeout(Duration::from_secs(3), async {
        loop {
            if manager
                .installations()
                .await
                .expect("installations")
                .iter()
                .any(|record| record.status == LocalModelInstallationStatus::Installed)
            {
                break;
            }
            sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("worker completed");
    manager.shutdown().await.expect("shutdown");
    assert_eq!(repository.copying_transitions(), 1);
}

#[tokio::test]
async fn begin_shutdown_cancels_and_drains_an_in_flight_import_worker() {
    let home = tempfile::tempdir().expect("home");
    let paths = noema_home::NoemaPaths::from_noema_home(home.path()).expect("paths");
    let repository = Arc::new(FakeRepository::default());
    repository.delay_mutations();
    let factory = Arc::new(FakeProcessFactory::new(repository.log()));
    let manager = manager(Arc::clone(&repository), factory, &paths);
    let model_path = home.path().join("cancellable.gguf");
    let mut model_bytes = vec![0_u8; 4 * 1024 * 1024];
    model_bytes[..4].copy_from_slice(b"GGUF");
    tokio::fs::write(&model_path, model_bytes)
        .await
        .expect("model fixture");

    let queued = manager
        .import_local_file(LocalFileModelImport {
            name: "Cancellable".to_string(),
            model_id: "cancellable".to_string(),
            path: model_path,
            license: None,
            backend: LocalModelBackend::Cpu,
        })
        .await
        .expect("queue import");
    timeout(Duration::from_secs(1), async {
        while repository.active_mutations() == 0 {
            sleep(Duration::from_millis(1)).await;
        }
    })
    .await
    .expect("worker entered persistence mutation");

    manager.begin_shutdown().await;

    assert_eq!(repository.active_mutations(), 0);
    assert!(manager.inner.workers.lock().await.is_empty());
    assert_eq!(
        repository
            .record(&queued.installation_id)
            .expect("cancelled record")
            .status,
        LocalModelInstallationStatus::Cancelled
    );
    manager.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn merged_events_backfill_before_runtime_and_reconnect_from_runtime_cursor() {
    let home = tempfile::tempdir().expect("home");
    let paths = noema_home::NoemaPaths::from_noema_home(home.path()).expect("paths");
    let repository = Arc::new(FakeRepository::default());
    repository.insert(installed_record("active", "shared-model", 'a', true));
    repository.append_event("active", LocalModelEventKind::Installed);
    let factory = Arc::new(FakeProcessFactory::new(repository.log()));
    let manager = manager(Arc::clone(&repository), Arc::clone(&factory), &paths);
    manager
        .start_active_installation()
        .await
        .expect("start active installation");
    let mut events = manager
        .subscribe_events(Some("0"))
        .await
        .expect("subscribe from origin");
    factory
        .process("active")
        .set_status(LocalModelRuntimeStatus::Failed {
            message: "injected runtime failure".to_string(),
        });

    let durable = timeout(Duration::from_secs(1), events.next())
        .await
        .expect("durable event timeout")
        .expect("durable event")
        .expect("durable result");
    assert_eq!(durable.cursor, "1");
    assert!(matches!(
        durable.payload,
        LocalModelManagerEvent::Durable { .. }
    ));

    let runtime = timeout(Duration::from_secs(1), events.next())
        .await
        .expect("runtime event timeout")
        .expect("runtime event")
        .expect("runtime result");
    assert_eq!(runtime.cursor, "1:runtime:1");
    assert!(matches!(
        runtime.payload,
        LocalModelManagerEvent::RuntimeChanged {
            status: LocalModelRuntimeStatus::Failed { .. }
        }
    ));

    let mut reconnected = manager
        .subscribe_events(Some(&runtime.cursor))
        .await
        .expect("reconnect from runtime cursor");
    repository.append_event("active", LocalModelEventKind::Activated);
    let next = timeout(Duration::from_secs(1), reconnected.next())
        .await
        .expect("reconnected event timeout")
        .expect("reconnected event")
        .expect("reconnected result");
    assert_eq!(next.cursor, "2");
    assert!(matches!(
        next.payload,
        LocalModelManagerEvent::Durable { .. }
    ));

    drop(events);
    drop(reconnected);
    manager.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn event_stream_drains_final_runtime_status_and_closes_on_shutdown() {
    let home = tempfile::tempdir().expect("home");
    let paths = noema_home::NoemaPaths::from_noema_home(home.path()).expect("paths");
    let repository = Arc::new(FakeRepository::default());
    repository.insert(installed_record("active", "shared-model", 'a', false));
    let factory = Arc::new(FakeProcessFactory::new(repository.log()));
    let manager = manager(repository, factory, &paths);
    let mut events = manager
        .subscribe_events(None)
        .await
        .expect("event subscription");

    manager.activate("active").await.expect("activation");
    manager.shutdown().await.expect("shutdown");
    let drained = timeout(Duration::from_secs(1), async {
        let mut drained = Vec::new();
        while let Some(event) = events.next().await {
            drained.push(event.expect("event"));
        }
        drained
    })
    .await
    .expect("event stream terminated");

    assert!(drained.iter().any(|event| {
        matches!(
            &event.payload,
            LocalModelManagerEvent::RuntimeChanged {
                status: LocalModelRuntimeStatus::Stopped
            }
        )
    }));
}
