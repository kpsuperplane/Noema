//! Exact local-model lifecycle snapshot and retirement-CAS tests.

use noema_providers::{
    LOCAL_MODELS_PROVIDER_ACCOUNT_ID, LocalModelActivationPersistence, LocalModelBackend,
    LocalModelEventKind, LocalModelInstallationPersistence, LocalModelInstallationRecord,
    LocalModelInstanceReferenceSource, LocalModelLifecyclePersistence,
    LocalModelRetirementClaimResult, LocalModelRuntimeRetirementResult, NewLocalModelInstallation,
    ProviderInstanceKey, ProviderPersistenceError,
};
use rusqlite::params;

use crate::NoemaStore;

#[tokio::test]
async fn reconstruction_snapshot_contains_canonical_and_future_run_reference_owners() {
    let store = super::tests::test_store().await;
    let installation = installed(&store, "snapshot", "a").await;
    let ready_selection = super::tests::ready_local_selection(&installation);
    LocalModelActivationPersistence::publish_local_model(
        &store,
        &installation.installation_id,
        &ready_selection,
        true,
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
        LocalModelInstanceReferenceSource::TaskModelPool {
            pool_entry_id: "task_pool:setting:simple".to_string(),
        },
        LocalModelInstanceReferenceSource::AgentRunSnapshot {
            run_id: "run:lifecycle-future".to_string(),
        },
    ] {
        assert!(sources.contains(&expected), "missing source: {expected:?}");
    }
    assert!(
        !sources.contains(&LocalModelInstanceReferenceSource::AgentRunSnapshot {
            run_id: "run:lifecycle-terminal".to_string(),
        })
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
    let ready_selection = super::tests::ready_local_selection(&active);
    LocalModelActivationPersistence::publish_local_model(
        &store,
        &active.installation_id,
        &ready_selection,
        true,
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
    let ready_selection = super::tests::ready_local_selection(&first);

    let error = LocalModelActivationPersistence::publish_local_model(
        &store,
        &first.installation_id,
        &ready_selection,
        true,
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

    let ready_selection = super::tests::ready_local_selection(&retired);
    let activated = LocalModelActivationPersistence::publish_local_model(
        &store,
        &installation.installation_id,
        &ready_selection,
        true,
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

async fn installed(
    store: &NoemaStore,
    suffix: &str,
    digest_char: &str,
) -> LocalModelInstallationRecord {
    let installation = store
        .upsert_local_model_installation(installation_input(suffix, digest_char))
        .await
        .expect("create installation");
    crate::tests::mark_local_model_installed(store, &installation).await;
    store
        .get_local_model_installation(&installation.installation_id)
        .await
        .expect("read installed model")
        .expect("installed model")
}

fn installation_input(suffix: &str, digest_char: &str) -> NewLocalModelInstallation {
    let mut input = crate::tests::local_model_installation(
        &format!("local_model_installation:{suffix}"),
        &format!("lifecycle-model-{suffix}"),
        LocalModelBackend::Cpu,
    );
    input.sha256 = Some(digest_char.repeat(64));
    input
}

async fn seed_task_and_run_references(
    store: &NoemaStore,
    installation: &LocalModelInstallationRecord,
) {
    store
        .with_connection(|conn| {
            for (task_id, stage_id) in [
                ("task:lifecycle-future", "stage:personal:queue"),
                ("task:lifecycle-terminal", "stage:personal:done"),
            ] {
                conn.execute(
                    r#"
                    INSERT INTO tasks (
                      task_id, workspace_id, workflow_id, stage_id, title,
                      description_markdown, source_kind, created_by_actor_id
                    ) VALUES (?1, 'workspace:personal', 'workflow:personal:default', ?2,
                              'Lifecycle', 'Exercise lifecycle references', 'system',
                              'actor:system')
                    "#,
                    params![task_id, stage_id],
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
                      run_id, instance_name, task_id, task_generation, run_kind, agent_id,
                      attempt_index, review_round,
                      provider_kind,
                      provider_account_id, provider_instance_key, selection_mode,
                      model_profile, max_provider_continuations, max_tool_calls,
                      max_active_minutes, progress_audit_interval,
                      max_automatic_retries, max_review_rounds, status
                    ) VALUES (?1, ?2, ?3, 1, 'executor', 'agent:task-executor',
                              0, 1, 'local_models', ?4, ?5, 'explicit_profile', ?6,
                              80, 400, 120, 20, 3, 3, ?7)
                    "#,
                    params![
                        run_id,
                        format!("Instance {run_id}"),
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
