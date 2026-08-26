pub(in crate::local_models) mod fakes;

use std::{sync::Arc, time::Duration};

use futures_util::StreamExt;
use tokio::time::{sleep, timeout};

use crate::{
    LocalModelBackend, LocalModelEventKind, LocalModelInstallationStatus,
    LocalModelInstanceReferenceSource, ProviderInstanceKey, ProviderRegistry,
    ProviderRegistryError,
};

use super::{
    LocalModelManagerError, LocalModelManagerEvent, LocalModelManagerService,
    LocalModelRuntimeStatus,
};
use crate::LocalFileModelImport;
use fakes::{FakeProcessFactory, FakeRepository, installed_record};

type ManagerFixture = (
    tempfile::TempDir,
    Arc<FakeRepository>,
    Arc<FakeProcessFactory>,
    LocalModelManagerService,
);

fn manager_fixture(
    records: impl IntoIterator<Item = crate::LocalModelInstallationRecord>,
) -> ManagerFixture {
    let home = tempfile::tempdir().expect("home");
    let paths = noema_home::NoemaPaths::from_noema_home(home.path()).expect("paths");
    let repository = Arc::new(FakeRepository::default());
    records
        .into_iter()
        .for_each(|record| repository.insert(record));
    let factory = Arc::new(FakeProcessFactory::new(repository.log()));
    let manager =
        manager_with_diagnostics(Arc::clone(&repository), Arc::clone(&factory), &paths, None);
    (home, repository, factory, manager)
}

fn two_installation_fixture() -> ManagerFixture {
    manager_fixture([
        installed_record("first", "shared-model", 'a', false),
        installed_record("second", "shared-model", 'b', false),
    ])
}

async fn write_large_gguf(home: &std::path::Path, name: &str) -> std::path::PathBuf {
    let path = home.join(name);
    let mut bytes = vec![0_u8; 4 * 1024 * 1024];
    bytes[..4].copy_from_slice(b"GGUF");
    tokio::fs::write(&path, bytes).await.expect("model fixture");
    path
}

fn local_import(name: &str, model_id: &str, path: std::path::PathBuf) -> LocalFileModelImport {
    LocalFileModelImport {
        name: name.to_string(),
        model_id: model_id.to_string(),
        path,
        license: None,
        backend: LocalModelBackend::Cpu,
    }
}

async fn wait_for_installed(manager: &LocalModelManagerService, expected: usize) {
    timeout(Duration::from_secs(3), async {
        loop {
            let records = manager.installations().await.expect("installations");
            if records.len() == expected
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
}

fn manager(
    repository: Arc<FakeRepository>,
    factory: Arc<FakeProcessFactory>,
    paths: &noema_home::NoemaPaths,
) -> LocalModelManagerService {
    manager_with_diagnostics(repository, factory, paths, None)
}

fn manager_with_diagnostics(
    repository: Arc<FakeRepository>,
    factory: Arc<FakeProcessFactory>,
    paths: &noema_home::NoemaPaths,
    system_errors: Option<noema_home::SystemErrorLogger>,
) -> LocalModelManagerService {
    let installations: Arc<dyn crate::LocalModelInstallationPersistence> = repository.clone();
    let activation: Arc<dyn crate::LocalModelActivationPersistence> = repository.clone();
    let lifecycle: Arc<dyn crate::LocalModelLifecyclePersistence> = repository;
    LocalModelManagerService::new_with_factory(
        installations,
        activation,
        lifecycle,
        Arc::new(ProviderRegistry::new()),
        paths.clone(),
        factory,
        system_errors,
    )
    .expect("manager")
}

#[tokio::test]
async fn activation_and_route_replacement_contracts() {
    eprintln!("case: two_same_model_installations_keep_distinct_keys_and_old_leases");
    let (_home, repository, factory, manager) = two_installation_fixture();
    repository.allow_installation_reads(2);
    let first_key = repository.record_key("first");
    let second_key = repository.record_key("second");
    assert!(
        manager
            .activate("first")
            .await
            .expect("first activation")
            .is_active
    );
    let old_lease = manager.registry().lease(&first_key).expect("old lease");
    manager.activate("second").await.expect("second activation");
    let new_lease = manager.registry().lease(&second_key).expect("new lease");

    assert_ne!(old_lease.key(), new_lease.key());
    assert!(old_lease.key().as_str().contains("first"));
    assert!(new_lease.key().as_str().contains("second"));
    assert_eq!(manager.managed_instance_count(), 2);
    assert_eq!(factory.process("first").shutdowns(), 0);

    drop(old_lease);
    drop(new_lease);
    manager.shutdown().await.expect("shutdown");
    eprintln!("case: failed_activation_stops_only_new_process_and_preserves_old_route");
    let (_home, repository, factory, manager) = two_installation_fixture();

    manager.activate("first").await.expect("first activation");
    let old_key = repository.record_key("first");
    repository.fail_next_activation();
    let error = manager
        .activate("second")
        .await
        .expect_err("injected persistence failure");

    assert!(matches!(error, LocalModelManagerError::Persistence(_)));
    assert!(manager.registry().lease(&old_key).is_ok());
    assert_eq!(factory.process("first").shutdowns(), 0);
    assert_eq!(factory.process("second").shutdowns(), 1);
    assert_eq!(manager.managed_instance_count(), 1);

    manager.shutdown().await.expect("shutdown");
    eprintln!("case: aborted_activation_remains_manager_owned_and_shutdown_stops_it");
    let (_home, repository, factory, manager) = two_installation_fixture();

    manager.activate("first").await.expect("first activation");
    repository.pause_next_activation();
    let activating_manager = manager.clone();
    let activation = tokio::spawn(async move { activating_manager.activate("second").await });
    repository.wait_for_activation().await;

    activation.abort();
    let _ = activation.await;
    let shutdown_manager = manager.clone();
    let shutdown = tokio::spawn(async move { shutdown_manager.shutdown().await });
    sleep(Duration::from_millis(20)).await;
    assert!(!shutdown.is_finished());
    repository.release_activation();
    shutdown
        .await
        .expect("shutdown task")
        .expect("shutdown result");

    assert_eq!(factory.process("first").shutdowns(), 1);
    assert_eq!(factory.process("second").shutdowns(), 1);
    eprintln!("case: activation_that_commits_during_shutdown_finishes_before_retirement");
    let (_home, repository, factory, manager) = two_installation_fixture();

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
    eprintln!("case: replacement_status_cannot_be_overwritten_by_retired_forwarder");
    let (_home, _repository, factory, manager) = two_installation_fixture();

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
async fn removal_retirement_and_cleanup_contracts() {
    eprintln!("case: inactive_remove_drains_before_delete_and_same_key_can_be_reinstalled");
    let (_home, repository, _factory, retirement_manager) = two_installation_fixture();

    retirement_manager
        .activate("first")
        .await
        .expect("first activation");
    let first_key = repository.record_key("first");
    let old_lease = retirement_manager
        .registry()
        .lease(&first_key)
        .expect("old lease");
    let old_generation = old_lease.generation();
    repository.deactivate_all_and_clear_references();
    repository.set_active("second");
    let removing_manager = retirement_manager.clone();
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
        retirement_manager.remove("second").await,
        Err(LocalModelManagerError::ActiveInstallation { .. })
    ));

    repository.insert(installed_record("first", "shared-model", 'a', false));
    retirement_manager
        .activate("first")
        .await
        .expect("reinstall and reactivate deterministic identity");
    let replacement = retirement_manager
        .registry()
        .lease(&first_key)
        .expect("replacement generation is ready");
    assert!(replacement.generation() > old_generation);
    drop(replacement);

    retirement_manager.shutdown().await.expect("shutdown");
    eprintln!("case: stop_failure_preserves_claimed_inactive_row");
    let (_home, repository, factory, failure_manager) = two_installation_fixture();

    failure_manager
        .activate("first")
        .await
        .expect("first activation");
    failure_manager
        .activate("second")
        .await
        .expect("second activation");
    factory.process("first").fail_shutdown();
    let error = failure_manager
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
    eprintln!(
        "case: explicit_removal_deletes_row_before_best_effort_partial_cleanup_and_retains_blob"
    );
    let home = tempfile::tempdir().expect("home");
    let paths = noema_home::NoemaPaths::from_noema_home(home.path()).expect("paths");
    let repository = Arc::new(FakeRepository::default());
    let installation = installed_record("cleanup", "cleanup-model", '6', false);
    let digest = installation.sha256.clone().expect("verified digest");
    repository.insert(installation);
    let blob_path = paths
        .local_model_blob_path(&digest)
        .expect("verified blob path");
    let partial_path = paths
        .local_model_partial_path(&digest)
        .expect("partial path");
    tokio::fs::create_dir_all(blob_path.parent().expect("blob parent"))
        .await
        .expect("create blob parent");
    tokio::fs::write(&blob_path, b"verified blob")
        .await
        .expect("write verified blob");
    tokio::fs::create_dir_all(&partial_path)
        .await
        .expect("create directory that cannot be removed as a file");
    let factory = Arc::new(FakeProcessFactory::new(repository.log()));
    let manager = manager(Arc::clone(&repository), factory, &paths);

    manager
        .remove("cleanup")
        .await
        .expect("durable removal succeeds before optional cleanup");

    assert!(repository.record("cleanup").is_none());
    assert!(
        blob_path.is_file(),
        "verified blobs are deferred to digest GC"
    );
    assert!(
        partial_path.is_dir(),
        "best-effort cleanup failure is retained"
    );
    manager.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn reconstruction_reaping_and_retry_contracts() {
    eprintln!("case: restart_reconstructs_only_active_and_transient_failure_keeps_manager_usable");
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
        .reconstruct_persisted_instances()
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
    assert_eq!(manager.managed_instance_count(), 1);

    manager.shutdown().await.expect("shutdown");

    let home = tempfile::tempdir().expect("home");
    let paths = noema_home::NoemaPaths::from_noema_home(home.path()).expect("paths");
    let diagnostics_path = home.path().join("missing-blob-errors.log");
    let repository = Arc::new(FakeRepository::default());
    repository.insert(installed_record("active", "shared-model", 'a', true));
    let installations: Arc<dyn crate::LocalModelInstallationPersistence> = repository.clone();
    let activation: Arc<dyn crate::LocalModelActivationPersistence> = repository.clone();
    let lifecycle: Arc<dyn crate::LocalModelLifecyclePersistence> = repository;
    let manager = LocalModelManagerService::new(
        installations,
        activation,
        lifecycle,
        Arc::new(ProviderRegistry::new()),
        paths,
        crate::LocalModelManagerConfig {
            runtime_root: None,
            context_window_tokens: 8_192,
            timeout_seconds: 30,
            startup_timeout_seconds: 30,
            system_errors: Some(noema_home::SystemErrorLogger::new(diagnostics_path.clone())),
        },
    )
    .expect("manager");

    manager
        .reconstruct_persisted_instances()
        .await
        .expect("missing blob remains recoverable");
    assert!(matches!(
        manager.runtime_status(),
        LocalModelRuntimeStatus::Failed { .. }
    ));
    let diagnostics = std::fs::read_to_string(diagnostics_path).expect("diagnostic");
    assert!(diagnostics.contains("\"category\":\"local_model_runtime_unavailable\""));
    assert!(diagnostics.contains("installed model blob is unavailable"));
    manager.shutdown().await.expect("shutdown");
    eprintln!(
        "case: reconstruction_starts_every_referenced_instance_and_keeps_transient_failures_degraded"
    );
    let first = installed_record("first", "first-model", 'a', false);
    let first_key = first.provider_instance_key.clone();
    let second = installed_record("second", "second-model", 'b', false);
    let second_key = second.provider_instance_key.clone();
    let (_home, repository, factory, manager) = manager_fixture([first, second]);
    repository.reference(
        first_key.clone(),
        LocalModelInstanceReferenceSource::DefaultModelPreference,
    );
    repository.reference(
        second_key.clone(),
        LocalModelInstanceReferenceSource::TaskSnapshot {
            task_id: "task:1".to_string(),
        },
    );
    factory.fail_start("second");

    let report = manager
        .reconstruct_persisted_instances()
        .await
        .expect("reconstruct references");

    assert_eq!(report.ready, vec![first_key.clone()]);
    assert_eq!(report.degraded.len(), 1);
    assert_eq!(report.degraded[0].key, second_key.clone());
    assert!(manager.registry().lease(&first_key).is_ok());
    assert_eq!(
        manager.registry().lease(&second_key).expect_err("unready"),
        ProviderRegistryError::Unready { key: second_key }
    );
    manager.shutdown().await.expect("shutdown");
    eprintln!(
        "case: reconstruction_rejects_missing_and_claimed_referenced_instances_before_starting_any"
    );
    let mut claimed = installed_record("claimed", "claimed-model", 'a', false);
    claimed.retirement_claimed_at = Some("2026-07-16T00:00:00Z".to_string());
    let claimed_key = claimed.provider_instance_key.clone();
    let (_home, repository, factory, manager) = manager_fixture([claimed]);
    repository.reference(
        claimed_key.clone(),
        LocalModelInstanceReferenceSource::AgentRunSnapshot {
            run_id: "run:1".to_string(),
        },
    );
    assert!(matches!(
        manager.reconstruct_persisted_instances().await,
        Err(LocalModelManagerError::ReferencedInstallationClaimed {
            provider_instance_key
        }) if provider_instance_key == claimed_key
    ));
    assert!(factory.started_ids().is_empty());

    repository.deactivate_all_and_clear_references();
    let mut runtime_retired = installed_record("runtime-retired", "retired-model", 'c', false);
    runtime_retired.runtime_retired_at = Some("2026-07-16T00:00:00Z".to_string());
    let runtime_retired_key = runtime_retired.provider_instance_key.clone();
    repository.insert(runtime_retired);
    repository.reference(
        runtime_retired_key.clone(),
        LocalModelInstanceReferenceSource::TaskSnapshot {
            task_id: "task:runtime-retired".to_string(),
        },
    );
    assert!(matches!(
        manager.reconstruct_persisted_instances().await,
        Err(LocalModelManagerError::ReferencedInstallationRuntimeRetired {
            provider_instance_key
        }) if provider_instance_key == runtime_retired_key
    ));
    assert!(factory.started_ids().is_empty());

    repository.deactivate_all_and_clear_references();
    let missing_key = ProviderInstanceKey::new("local-model:missing").expect("missing key");
    repository.reference(
        missing_key.clone(),
        LocalModelInstanceReferenceSource::TaskSnapshot {
            task_id: "task:missing".to_string(),
        },
    );
    assert!(matches!(
        manager.reconstruct_persisted_instances().await,
        Err(LocalModelManagerError::ReferencedInstallationMissing {
            provider_instance_key
        }) if provider_instance_key == missing_key
    ));
    assert!(factory.started_ids().is_empty());
    manager.shutdown().await.expect("shutdown");
    eprintln!("case: startup_reaper_never_claims_an_in_progress_installation");
    let mut downloading = installed_record("downloading", "future-model", 'd', false);
    downloading.status = LocalModelInstallationStatus::Downloading;
    downloading.retirement_claimed_at = None;
    let (_home, repository, _factory, manager) = manager_fixture([downloading]);

    manager
        .reconstruct_persisted_instances()
        .await
        .expect("startup reconstruction");

    let retained = repository
        .record("downloading")
        .expect("in-progress installation retained");
    assert_eq!(retained.status, LocalModelInstallationStatus::Downloading);
    assert!(retained.retirement_claimed_at.is_none());
    manager.shutdown().await.expect("shutdown");
    eprintln!("case: reaper_stops_runtime_but_preserves_installed_row_and_blob");
    let home = tempfile::tempdir().expect("home");
    let paths = noema_home::NoemaPaths::from_noema_home(home.path()).expect("paths");
    let repository = Arc::new(FakeRepository::default());
    repository.insert(installed_record("retire", "retire-model", 'e', true));
    let factory = Arc::new(FakeProcessFactory::new(repository.log()));
    let manager =
        manager_with_diagnostics(Arc::clone(&repository), Arc::clone(&factory), &paths, None);
    manager
        .reconstruct_persisted_instances()
        .await
        .expect("startup reconstruction");
    repository.deactivate_all_and_clear_references();
    let process = factory.process("retire");
    assert_eq!(process.shutdowns(), 0);

    manager.trigger_reaper().await;
    timeout(Duration::from_secs(1), async {
        while process.shutdowns() == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("periodic retirement");
    let retained = repository
        .record("retire")
        .expect("automatic retirement preserves installation row");
    assert_eq!(
        retained.blob_relative_path.as_deref(),
        Some("models/blobs/eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee.gguf")
    );
    assert!(retained.runtime_retired_at.is_some());
    assert_eq!(retained.retirement_claimed_at, None);
    assert!(matches!(
        manager.registry().lease(&retained.provider_instance_key),
        Err(ProviderRegistryError::Retiring { .. })
    ));

    manager.shutdown().await.expect("shutdown");
    eprintln!("case: retry_replaces_a_failed_retained_process_and_preserves_the_exact_route");
    let (_home, repository, factory, manager) =
        manager_fixture([installed_record("active", "shared-model", 'a', true)]);

    manager
        .reconstruct_persisted_instances()
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
    let active_key = repository.record_key("active");
    let lease = manager
        .registry()
        .lease(&active_key)
        .expect("replacement route");
    assert!(lease.key().as_str().contains("active"));
    drop(lease);

    manager.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn worker_serialization_cancellation_and_shutdown_contracts() {
    eprintln!("case: shutdown_rejects_new_work_drains_lease_and_stops_every_process");
    let (_home, repository, factory, manager) = two_installation_fixture();
    let first_key = repository.record_key("first");
    manager.activate("first").await.expect("first activation");
    let old_lease = manager.registry().lease(&first_key).expect("old lease");
    manager.activate("second").await.expect("second activation");
    let shutdown_manager = manager.clone();
    let shutdown = tokio::spawn(async move { shutdown_manager.shutdown().await });
    sleep(Duration::from_millis(30)).await;

    assert!(!shutdown.is_finished());
    assert!(matches!(
        manager.activate("second").await,
        Err(LocalModelManagerError::ShuttingDown)
    ));
    assert!(manager.registry().lease(&first_key).is_err());
    drop(old_lease);
    shutdown
        .await
        .expect("shutdown task")
        .expect("shutdown result");

    assert_eq!(factory.process("first").shutdowns(), 1);
    assert_eq!(factory.process("second").shutdowns(), 1);
    assert_eq!(manager.runtime_status(), LocalModelRuntimeStatus::Stopped);
    eprintln!("case: distinct_install_workers_never_overlap_global_artifact_mutation");
    let (home, repository, _factory, manager) = manager_fixture([]);
    repository.delay_mutations();
    let first_path = home.path().join("first.gguf");
    let second_path = home.path().join("second.gguf");
    tokio::fs::write(&first_path, b"GGUF first")
        .await
        .expect("first model");
    tokio::fs::write(&second_path, b"GGUF second")
        .await
        .expect("second model");

    let (first, second) = tokio::join!(
        manager.import_local_file(local_import("First", "first", first_path)),
        manager.import_local_file(local_import("Second", "second", second_path))
    );
    first.expect("first queued");
    second.expect("second queued");

    wait_for_installed(&manager, 2).await;

    assert_eq!(repository.max_active_mutations(), 1);
    manager.shutdown().await.expect("shutdown");
    eprintln!("case: one_local_model_operation_is_retained_per_installation");
    let (home, repository, _factory, manager) = manager_fixture([]);
    repository.pause_next_upsert();
    let model_path = write_large_gguf(home.path(), "duplicate.gguf").await;
    let input = local_import("Duplicate", "duplicate", model_path);

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
    wait_for_installed(&manager, 1).await;
    manager.shutdown().await.expect("shutdown");
    assert_eq!(repository.copying_transitions(), 1);
    eprintln!("case: remove_cancels_and_drains_owned_worker_before_terminal_row_deletion");
    let (home, repository, _factory, manager) = manager_fixture([]);
    repository.delay_mutations();
    let model_path = write_large_gguf(home.path(), "remove-owned-worker.gguf").await;
    let queued = manager
        .import_local_file(local_import(
            "Remove owned worker",
            "remove-owned-worker",
            model_path,
        ))
        .await
        .expect("queue import");

    let removed = manager
        .remove(&queued.installation_id)
        .await
        .expect("cancel, drain, and remove");

    assert!(matches!(
        removed.installation.status,
        LocalModelInstallationStatus::Cancelled | LocalModelInstallationStatus::Failed
    ));
    assert!(repository.record(&queued.installation_id).is_none());
    sleep(Duration::from_millis(50)).await;
    assert!(
        repository.record(&queued.installation_id).is_none(),
        "completed worker must not recreate the deleted row"
    );
    manager.shutdown().await.expect("shutdown");
    eprintln!("case: begin_shutdown_cancels_and_drains_an_in_flight_import_worker");
    let (home, repository, _factory, manager) = manager_fixture([]);
    repository.delay_mutations();
    let model_path = write_large_gguf(home.path(), "cancellable.gguf").await;

    let queued = manager
        .import_local_file(local_import("Cancellable", "cancellable", model_path))
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
async fn event_subscription_lifecycle_contracts() {
    eprintln!("case: merged_events_backfill_before_runtime_and_reconnect_from_runtime_cursor");
    let (_home, repository, factory, manager) =
        manager_fixture([installed_record("active", "shared-model", 'a', true)]);
    repository.append_event("active", LocalModelEventKind::Installed);
    manager
        .reconstruct_persisted_instances()
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
    eprintln!("case: event_stream_drains_final_runtime_status_and_closes_on_shutdown");
    let (_home, _repository, _factory, manager) =
        manager_fixture([installed_record("active", "shared-model", 'a', false)]);
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
