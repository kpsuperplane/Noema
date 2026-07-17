//! Exact local-model lifecycle snapshot and retirement-CAS tests.

use noema_providers::{
    LOCAL_MODELS_PROVIDER_ACCOUNT_ID, LocalModelActivationPersistence, LocalModelBackend,
    LocalModelEventKind, LocalModelInstallationPersistence, LocalModelInstallationRecord,
    LocalModelInstallationStatus, LocalModelInstallationUpdate, LocalModelInstanceReferenceSource,
    LocalModelLifecyclePersistence, LocalModelRetirementClaimResult,
    LocalModelRuntimeRetirementResult, LocalModelSourceKind, NewLocalModelInstallation,
    ProviderInstanceKey, ProviderPersistenceError, ProviderReadySelection,
    ProviderSelectionSnapshot,
};
use rusqlite::params;

use crate::NoemaStore;

#[tokio::test]
async fn reconstruction_snapshot_contains_canonical_and_future_reference_owners() {
    let store = super::tests::test_store().await;
    let installation = installed(&store, "snapshot", "a").await;
    let ready_selection = ready_local_selection(&installation);
    LocalModelActivationPersistence::activate_local_model_as_system_default(
        &store,
        &installation.installation_id,
        &ready_selection,
    )
    .await
    .expect("activate installation");
    let installation = LocalModelInstallationPersistence::local_model_installation(
        &store,
        &installation.installation_id,
    )
    .await
    .expect("read active installation")
    .expect("active installation");
    seed_task_and_run_references(&store, &installation).await;

    let snapshot = LocalModelLifecyclePersistence::local_model_reconstruction_snapshot(&store)
        .await
        .expect("reconstruction snapshot");

    assert_eq!(snapshot.installations, vec![installation.clone()]);
    let sources = snapshot
        .references
        .iter()
        .filter(|reference| reference.provider_instance_key == installation.provider_instance_key)
        .map(|reference| reference.source.clone())
        .collect::<Vec<_>>();
    for expected in [
        LocalModelInstanceReferenceSource::ActiveInstallation,
        LocalModelInstanceReferenceSource::DefaultModelPreference,
        LocalModelInstanceReferenceSource::AgentRuntimePreference {
            agent_id: "agent:primary".to_string(),
        },
        LocalModelInstanceReferenceSource::AuxiliaryModelPreference {
            workload: "web_fetch_summarizer".to_string(),
        },
        LocalModelInstanceReferenceSource::MemoryService,
        LocalModelInstanceReferenceSource::TaskModelPool {
            pool_entry_id: "task_pool:setting:simple".to_string(),
        },
        LocalModelInstanceReferenceSource::TaskSnapshot {
            task_id: "task:lifecycle-future".to_string(),
        },
        LocalModelInstanceReferenceSource::AgentRunSnapshot {
            run_id: "run:lifecycle-future".to_string(),
        },
    ] {
        assert!(sources.contains(&expected), "missing source: {expected:?}");
    }
    assert!(
        !sources.contains(&LocalModelInstanceReferenceSource::TaskSnapshot {
            task_id: "task:lifecycle-terminal".to_string(),
        })
    );
    assert!(
        !sources.contains(&LocalModelInstanceReferenceSource::AgentRunSnapshot {
            run_id: "run:lifecycle-terminal".to_string(),
        })
    );
    assert_eq!(
        sources
            .iter()
            .filter(|source| {
                **source
                    == LocalModelInstanceReferenceSource::TaskSnapshot {
                        task_id: "task:lifecycle-future".to_string(),
                    }
            })
            .count(),
        1,
        "executor and reviewer roles collapse to their one task owner source"
    );
}

#[tokio::test]
async fn retirement_claim_is_monotonic_and_completion_appends_removal() {
    let store = super::tests::test_store().await;
    let installation = installed(&store, "claim", "b").await;

    let first = LocalModelLifecyclePersistence::claim_unreferenced_instance_for_retirement(
        &store,
        &installation.provider_instance_key,
    )
    .await
    .expect("claim installation");
    let LocalModelRetirementClaimResult::Claimed(first) = first else {
        panic!("expected first claim");
    };
    assert!(first.installation.retirement_claimed_at.is_some());

    let second = LocalModelLifecyclePersistence::claim_unreferenced_instance_for_retirement(
        &store,
        &installation.provider_instance_key,
    )
    .await
    .expect("repeat claim");
    let LocalModelRetirementClaimResult::AlreadyClaimed(second) = second else {
        panic!("expected idempotent existing claim");
    };
    assert_eq!(
        second.installation.retirement_claimed_at,
        first.installation.retirement_claimed_at
    );

    let removed = LocalModelLifecyclePersistence::complete_claimed_local_model_removal(
        &store,
        &installation.provider_instance_key,
    )
    .await
    .expect("complete removal");
    assert_eq!(removed.installation, second.installation);
    assert!(
        LocalModelInstallationPersistence::local_model_installation(
            &store,
            &installation.installation_id,
        )
        .await
        .expect("read removed installation")
        .is_none()
    );
    let events = LocalModelInstallationPersistence::local_model_events(&store, None, 20)
        .await
        .expect("events");
    assert_eq!(
        events.last().map(|event| event.kind),
        Some(LocalModelEventKind::Removed)
    );
}

#[tokio::test]
async fn retirement_claim_classifies_references_missing_and_unclaimed_completion() {
    let store = super::tests::test_store().await;
    let active = installed(&store, "active", "c").await;
    let ready_selection = ready_local_selection(&active);
    LocalModelActivationPersistence::activate_local_model_as_system_default(
        &store,
        &active.installation_id,
        &ready_selection,
    )
    .await
    .expect("activate installation");

    let referenced = LocalModelLifecyclePersistence::claim_unreferenced_instance_for_retirement(
        &store,
        &active.provider_instance_key,
    )
    .await
    .expect("classify active installation");
    let LocalModelRetirementClaimResult::Referenced { references, .. } = referenced else {
        panic!("active installation must remain referenced");
    };
    assert!(references.contains(&LocalModelInstanceReferenceSource::ActiveInstallation));

    let missing_key = ProviderInstanceKey::new("local-model:v1:missing").expect("missing key");
    assert_eq!(
        LocalModelLifecyclePersistence::claim_unreferenced_instance_for_retirement(
            &store,
            &missing_key,
        )
        .await
        .expect("classify missing installation"),
        LocalModelRetirementClaimResult::Missing
    );

    let inactive = installed(&store, "unclaimed", "d").await;
    let error = LocalModelLifecyclePersistence::complete_claimed_local_model_removal(
        &store,
        &inactive.provider_instance_key,
    )
    .await
    .expect_err("unclaimed row cannot be completed");
    assert_eq!(
        error,
        ProviderPersistenceError::Conflict {
            operation: "complete_unclaimed_local_model_removal"
        }
    );

    let queued = LocalModelInstallationPersistence::upsert_local_model_installation(
        &store,
        installation_input("queued-removal", "8"),
    )
    .await
    .expect("create queued installation");
    let error = LocalModelLifecyclePersistence::claim_unreferenced_instance_for_retirement(
        &store,
        &queued.provider_instance_key,
    )
    .await
    .expect_err("unfinished installation cannot enter installed-row removal flow");
    assert_eq!(
        error,
        ProviderPersistenceError::Conflict {
            operation: "claim_non_installed_local_model"
        }
    );
}

#[tokio::test]
async fn completion_failure_rolls_back_the_removed_event_and_retains_the_claim() {
    let store = super::tests::test_store().await;
    let installation = installed(&store, "rollback", "e").await;
    LocalModelLifecyclePersistence::claim_unreferenced_instance_for_retirement(
        &store,
        &installation.provider_instance_key,
    )
    .await
    .expect("claim installation");
    let events_before = LocalModelInstallationPersistence::local_model_events(&store, None, 20)
        .await
        .expect("events before")
        .len();
    {
        let connection = store.connection_for_tests();
        let connection = connection.lock().await;
        connection
            .execute_batch(
                r#"
                CREATE TEMP TRIGGER fail_claimed_local_model_delete
                BEFORE DELETE ON local_model_installations
                BEGIN
                  SELECT RAISE(ABORT, 'forced claimed removal failure');
                END;
                "#,
            )
            .expect("install failure trigger");
    }

    let error = LocalModelLifecyclePersistence::complete_claimed_local_model_removal(
        &store,
        &installation.provider_instance_key,
    )
    .await
    .expect_err("forced delete failure");
    assert_eq!(
        error,
        ProviderPersistenceError::Persistence {
            operation: "complete_claimed_local_model_removal"
        }
    );
    let retained = LocalModelInstallationPersistence::local_model_installation(
        &store,
        &installation.installation_id,
    )
    .await
    .expect("read retained claim")
    .expect("claimed installation remains");
    assert!(retained.retirement_claimed_at.is_some());
    assert_eq!(
        LocalModelInstallationPersistence::local_model_events(&store, None, 20)
            .await
            .expect("events after")
            .len(),
        events_before
    );
}

#[tokio::test]
async fn claimed_instances_reject_activation_and_shared_blobs_are_retained() {
    let store = super::tests::test_store().await;
    let first = installed(&store, "shared-first", "f").await;

    let claim = LocalModelLifecyclePersistence::claim_unreferenced_instance_for_retirement(
        &store,
        &first.provider_instance_key,
    )
    .await
    .expect("claim first installation");
    let LocalModelRetirementClaimResult::Claimed(claim) = claim else {
        panic!("first shared installation should be claimed");
    };
    assert!(claim.installation.retirement_claimed_at.is_some());

    // A same-digest installation may commit after the removal claim. Completion
    // therefore cannot use uniqueness observed at claim time to authorize an
    // out-of-transaction filesystem deletion.
    let second = installed(&store, "shared-second", "f").await;
    let ready_selection = ready_local_selection(&first);

    let error = LocalModelActivationPersistence::activate_local_model_as_system_default(
        &store,
        &first.installation_id,
        &ready_selection,
    )
    .await
    .expect_err("claimed installation cannot become canonical");
    assert_eq!(
        error,
        ProviderPersistenceError::ProviderInstanceRetiring {
            provider_instance_key: first.provider_instance_key.clone(),
        }
    );

    LocalModelLifecyclePersistence::complete_claimed_local_model_removal(
        &store,
        &first.provider_instance_key,
    )
    .await
    .expect("complete shared installation removal");
    assert_eq!(
        LocalModelInstallationPersistence::local_model_installation(
            &store,
            &second.installation_id,
        )
        .await
        .expect("read shared installation"),
        Some(second)
    );
}

#[tokio::test]
async fn runtime_retirement_is_reversible_and_distinct_from_removal_claiming() {
    let store = super::tests::test_store().await;
    let installation = installed(&store, "runtime-retire", "9").await;

    let result = LocalModelLifecyclePersistence::retire_unreferenced_instance_runtime(
        &store,
        &installation.provider_instance_key,
    )
    .await
    .expect("retire unreferenced runtime");
    let LocalModelRuntimeRetirementResult::Retired(retired) = result else {
        panic!("runtime should be newly retired");
    };
    assert!(retired.runtime_retired_at.is_some());
    assert_eq!(retired.retirement_claimed_at, None);

    let repeated = LocalModelLifecyclePersistence::retire_unreferenced_instance_runtime(
        &store,
        &installation.provider_instance_key,
    )
    .await
    .expect("repeat runtime retirement");
    assert!(matches!(
        repeated,
        LocalModelRuntimeRetirementResult::AlreadyRetired(_)
    ));

    let ready_selection = ready_local_selection(&retired);
    let activated = LocalModelActivationPersistence::activate_local_model_as_system_default(
        &store,
        &installation.installation_id,
        &ready_selection,
    )
    .await
    .expect("reactivate retired runtime");
    assert!(activated.is_active);
    assert_eq!(activated.runtime_retired_at, None);
    assert_eq!(activated.retirement_claimed_at, None);

    let referenced = LocalModelLifecyclePersistence::retire_unreferenced_instance_runtime(
        &store,
        &installation.provider_instance_key,
    )
    .await
    .expect("classify active runtime");
    assert!(matches!(
        referenced,
        LocalModelRuntimeRetirementResult::Referenced { .. }
    ));
}

fn ready_local_selection(installation: &LocalModelInstallationRecord) -> ProviderReadySelection {
    let mut selection = ProviderSelectionSnapshot::explicit(
        "local_models",
        LOCAL_MODELS_PROVIDER_ACCOUNT_ID,
        &installation.model_id,
        None,
        Some("local_model_lifecycle_test".to_string()),
    );
    selection.provider_instance_key = Some(installation.provider_instance_key.clone());
    crate::tests::ready_provider_selection(selection)
}

async fn installed(
    store: &NoemaStore,
    suffix: &str,
    digest_char: &str,
) -> LocalModelInstallationRecord {
    let installation = LocalModelInstallationPersistence::upsert_local_model_installation(
        store,
        installation_input(suffix, digest_char),
    )
    .await
    .expect("create installation");
    let mut current = installation;
    for status in [
        LocalModelInstallationStatus::Downloading,
        LocalModelInstallationStatus::Verifying,
        LocalModelInstallationStatus::Installed,
    ] {
        current = LocalModelInstallationPersistence::update_local_model_installation(
            store,
            &current.installation_id,
            LocalModelInstallationUpdate {
                status,
                downloaded_bytes: 100,
                expected_bytes: Some(100),
                sha256: None,
                blob_relative_path: (status == LocalModelInstallationStatus::Installed)
                    .then(|| format!("models/blobs/{}.gguf", digest_char.repeat(64))),
                error_code: None,
                error_message: None,
            },
        )
        .await
        .expect("advance installation");
    }
    current
}

fn installation_input(suffix: &str, digest_char: &str) -> NewLocalModelInstallation {
    NewLocalModelInstallation {
        installation_id: format!("local_model_installation:{suffix}"),
        model_id: format!("lifecycle-model-{suffix}"),
        display_name: format!("Lifecycle Model {suffix}"),
        source_kind: LocalModelSourceKind::Catalog,
        source_repo: Some("example/lifecycle".to_string()),
        source_revision: Some("f".repeat(40)),
        source_file: Some(format!("{suffix}.gguf")),
        sha256: Some(digest_char.repeat(64)),
        download_gb: 1.0,
        expected_bytes: Some(100),
        license: Some("Apache-2.0".to_string()),
        backend: LocalModelBackend::Cpu,
    }
}

async fn seed_task_and_run_references(
    store: &NoemaStore,
    installation: &LocalModelInstallationRecord,
) {
    store
        .with_connection(|conn| {
            for (task_id, status) in [
                ("task:lifecycle-future", "queued"),
                ("task:lifecycle-terminal", "failed"),
            ] {
                conn.execute(
                    r#"
                    INSERT INTO tasks (
                      task_id, title, request_markdown, complexity, status,
                      owner_human_id, created_by_agent_id, pool_entry_id,
                      executor_provider_kind, executor_provider_account_id,
                      executor_provider_instance_key, executor_selection_mode,
                      executor_model_profile, reviewer_provider_kind,
                      reviewer_provider_account_id, reviewer_provider_instance_key,
                      reviewer_selection_mode, reviewer_model_profile
                    ) VALUES (?1, 'Lifecycle', 'Exercise lifecycle references', 'simple', ?2,
                              'human:test', 'agent:primary', 'task_pool:setting:simple',
                              'local_models', ?3, ?4, 'explicit_profile', ?5,
                              'local_models', ?3, ?4, 'explicit_profile', ?5)
                    "#,
                    params![
                        task_id,
                        status,
                        LOCAL_MODELS_PROVIDER_ACCOUNT_ID,
                        installation.provider_instance_key.as_str(),
                        installation.model_id,
                    ],
                )?;
            }
            for (run_id, task_id, status) in [
                ("run:lifecycle-future", "task:lifecycle-future", "queued"),
                (
                    "run:lifecycle-terminal",
                    "task:lifecycle-terminal",
                    "failed",
                ),
            ] {
                conn.execute(
                    r#"
                    INSERT INTO agent_runs (
                      run_id, task_id, run_kind, agent_id, provider_kind,
                      provider_account_id, provider_instance_key, selection_mode,
                      model_profile, max_provider_continuations, max_tool_calls,
                      max_active_minutes, progress_audit_interval, status
                    ) VALUES (?1, ?2, 'executor', 'agent:task-executor', 'local_models',
                              ?3, ?4, 'explicit_profile', ?5, 80, 400, 120, 20, ?6)
                    "#,
                    params![
                        run_id,
                        task_id,
                        LOCAL_MODELS_PROVIDER_ACCOUNT_ID,
                        installation.provider_instance_key.as_str(),
                        installation.model_id,
                        status,
                    ],
                )?;
            }
            Ok(())
        })
        .await
        .expect("seed future and terminal references");
}
