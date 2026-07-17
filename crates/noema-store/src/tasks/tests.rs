use noema_providers::{
    ProviderAccountStatus, ProviderInstanceKey, ProviderSelectionSnapshot, ReasoningEffort,
    provider_account_instance_key,
};
use noema_tasks::{
    NewTask, NewTaskModelPoolEntry, NewTaskValidationCriterion, TaskComplexity, TaskSource,
};

use crate::{
    NewAgentRuntimePreference, StoreError,
    tests::{ready_provider_registry, ready_provider_selection, test_store},
};

async fn initialized_task_store() -> crate::NoemaStore {
    let store = test_store().await;
    store
        .ensure_default_provider_account()
        .await
        .expect("default provider account");
    store
        .update_provider_account_status(
            "provider_account:codex:default",
            ProviderAccountStatus::Authenticated,
            None,
            None,
        )
        .await
        .expect("authenticated provider account");
    let mut selection = ProviderSelectionSnapshot::explicit(
        "codex",
        "provider_account:codex:default",
        "gpt-5.6-luna",
        Some(ReasoningEffort::Medium),
        Some("test_default".to_string()),
    );
    selection.provider_instance_key = Some(
        provider_account_instance_key("provider_account:codex:default")
            .expect("hosted instance key"),
    );
    let ready_selection = ready_provider_selection(selection);
    store
        .initialize_missing_provider_selections(ready_selection.selection(), Some(&ready_selection))
        .await
        .expect("provider selection initialization");
    store
}

async fn initialized_local_task_store() -> (
    crate::NoemaStore,
    noema_providers::ProviderRegistry,
    ProviderSelectionSnapshot,
) {
    let store = test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let installation_id = "local_model_installation:task-readiness";
    let model_id = "task-readiness-model";
    let key = noema_providers::local_model_provider_instance_key(
        noema_providers::LOCAL_MODELS_PROVIDER_ACCOUNT_ID,
        installation_id,
        model_id,
    )
    .expect("local key");
    {
        let connection = store.connection_for_tests();
        let connection = connection.lock().await;
        connection
            .execute(
                r#"
                INSERT INTO provider_accounts (
                  provider_account_id, provider_kind, account_key, display_name,
                  auth_method, is_active, is_default, status, metadata_json
                ) VALUES (?1, 'local_models', 'default', 'Local models', 'none',
                          1, 1, 'authenticated', '{}')
                "#,
                [noema_providers::LOCAL_MODELS_PROVIDER_ACCOUNT_ID],
            )
            .expect("local account");
        connection
            .execute(
                r#"
                INSERT INTO local_model_installations (
                  installation_id, provider_instance_key, model_id, display_name,
                  source_kind, source_file, sha256, download_gb, expected_bytes,
                  downloaded_bytes, backend, status, blob_relative_path, is_active,
                  installed_at
                ) VALUES (?1, ?2, ?3, 'Task readiness model', 'local_file',
                          'task-readiness.gguf', ?4, 1.0, 1, 1, 'metal', 'installed',
                          'models/blobs/task-readiness.gguf', 1,
                          strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
                "#,
                rusqlite::params![installation_id, key.as_str(), model_id, "b".repeat(64)],
            )
            .expect("local installation");
    }
    let mut selection = ProviderSelectionSnapshot::explicit(
        "local_models",
        noema_providers::LOCAL_MODELS_PROVIDER_ACCOUNT_ID,
        model_id,
        None,
        Some("test_local_default".to_string()),
    );
    selection.provider_instance_key = Some(key);
    let registry = ready_provider_registry(&selection);
    let ready_selection = registry
        .prove_ready_selection(selection.clone())
        .expect("ready local selection");
    store
        .initialize_missing_provider_selections(&selection, Some(&ready_selection))
        .await
        .expect("initialize local selections");
    (store, registry, selection)
}

#[tokio::test]
async fn task_creation_requires_and_retains_local_readiness_proofs() {
    let (store, registry, selection) = initialized_local_task_store().await;
    let input = NewTask {
        task_id: Some("task:local-readiness".to_string()),
        title: "Local readiness".to_string(),
        request_markdown: "Prove local readiness before queuing.".to_string(),
        complexity: TaskComplexity::Simple,
        owner_human_id: "human:local".to_string(),
        source: TaskSource::default(),
        created_by_agent_id: "agent:primary".to_string(),
        creation_tool_call_id: None,
        pool_entry_id: "task_pool:setting:simple".to_string(),
        executor_model: selection.clone(),
        reviewer_model: selection,
        max_review_rounds: None,
        criteria: vec![NewTaskValidationCriterion {
            criterion_id: None,
            ordinal: 1,
            description: "The route was proved ready".to_string(),
            expected_evidence: None,
        }],
    };

    let unproved = store
        .create_task_with_executor(input.clone())
        .await
        .expect_err("unproved local task route");
    assert!(matches!(
        unproved,
        StoreError::ProviderInstanceUnavailable { .. }
    ));

    let (task, run) = store
        .create_task_with_executor_with_readiness(input, &registry)
        .await
        .expect("proved local task route");
    assert_eq!(task.latest_run_id.as_deref(), Some(run.run_id.as_str()));
}

#[tokio::test]
async fn task_creation_captures_pool_and_reviewer_inside_writer_transaction() {
    let store = initialized_task_store().await;
    let stale_pool = store
        .get_task_model_pool_entry("task_pool:setting:simple")
        .await
        .expect("pool read")
        .expect("simple pool");
    let stale_reviewer = store
        .get_agent_runtime_preference(noema_tasks::TASK_REVIEWER_AGENT_ID)
        .await
        .expect("reviewer read")
        .expect("reviewer preference");

    let pool_input = NewTaskModelPoolEntry {
        pool_entry_id: Some(stale_pool.pool_entry_id.clone()),
        complexity: TaskComplexity::Simple,
        label: Some("Current executor".to_string()),
        provider_kind: "codex".to_string(),
        provider_account_id: "provider_account:codex:default".to_string(),
        model_profile: "gpt-executor-current".to_string(),
        reasoning_effort: Some(ReasoningEffort::High),
        enabled: true,
        sort_order: 0,
    };
    let pool_ready = ready_provider_selection(ProviderSelectionSnapshot::explicit(
        &pool_input.provider_kind,
        &pool_input.provider_account_id,
        &pool_input.model_profile,
        pool_input.reasoning_effort,
        Some("current_pool_test".to_string()),
    ));
    let current_pool = store
        .update_task_model_pool_entry_with_ready_selection(
            &stale_pool.pool_entry_id,
            pool_input,
            &pool_ready,
        )
        .await
        .expect("update pool");
    let reviewer_input = NewAgentRuntimePreference {
        agent_id: noema_tasks::TASK_REVIEWER_AGENT_ID.to_string(),
        provider_kind: "codex".to_string(),
        provider_account_id: "provider_account:codex:default".to_string(),
        model_profile: "gpt-reviewer-current".to_string(),
        reasoning_effort: Some(ReasoningEffort::XHigh),
    };
    let reviewer_ready = ready_provider_selection(ProviderSelectionSnapshot::explicit(
        &reviewer_input.provider_kind,
        &reviewer_input.provider_account_id,
        &reviewer_input.model_profile,
        reviewer_input.reasoning_effort,
        Some("current_reviewer_test".to_string()),
    ));
    let current_reviewer = store
        .upsert_agent_runtime_preference_with_ready_selection(reviewer_input, &reviewer_ready)
        .await
        .expect("update reviewer");

    let mut unresolved_executor_hint = stale_pool.model;
    unresolved_executor_hint.provider_instance_key = None;
    let stale_reviewer_hint = ProviderSelectionSnapshot::explicit(
        stale_reviewer.provider_kind,
        stale_reviewer.provider_account_id,
        stale_reviewer.model_profile,
        stale_reviewer.reasoning_effort,
        Some("stale_reviewer".to_string()),
    );
    let registry = ready_provider_registry(&current_pool.model);
    let (task, run) = store
        .create_task_with_executor_with_readiness(
            NewTask {
                task_id: Some("task:transactional-selection".to_string()),
                title: "Capture exact selections".to_string(),
                request_markdown: "Prove the store owns selection capture.".to_string(),
                complexity: TaskComplexity::Simple,
                owner_human_id: "human:local".to_string(),
                source: TaskSource::default(),
                created_by_agent_id: "agent:primary".to_string(),
                creation_tool_call_id: None,
                pool_entry_id: stale_pool.pool_entry_id,
                executor_model: unresolved_executor_hint,
                reviewer_model: stale_reviewer_hint,
                max_review_rounds: None,
                criteria: vec![NewTaskValidationCriterion {
                    criterion_id: None,
                    ordinal: 1,
                    description: "Exact selections were captured".to_string(),
                    expected_evidence: None,
                }],
            },
            &registry,
        )
        .await
        .expect("create task");

    assert_eq!(task.executor_model, current_pool.model);
    assert_eq!(
        task.reviewer_model.provider_instance_key.as_ref(),
        Some(&current_reviewer.provider_instance_key)
    );
    assert_eq!(
        task.reviewer_model.model_profile.as_deref(),
        Some(current_reviewer.model_profile.as_str())
    );
    assert_eq!(run.model, task.executor_model);
}

#[tokio::test]
async fn authenticated_hosted_metadata_cannot_create_future_work_without_registration() {
    let store = initialized_task_store().await;
    let pool = store
        .get_task_model_pool_entry("task_pool:setting:simple")
        .await
        .expect("pool read")
        .expect("simple pool");
    let input = NewTask {
        task_id: Some("task:hosted-readiness".to_string()),
        title: "Hosted readiness".to_string(),
        request_markdown: "Require a registered hosted runtime.".to_string(),
        complexity: TaskComplexity::Simple,
        owner_human_id: "human:local".to_string(),
        source: TaskSource::default(),
        created_by_agent_id: "agent:primary".to_string(),
        creation_tool_call_id: None,
        pool_entry_id: pool.pool_entry_id,
        executor_model: pool.model.clone(),
        reviewer_model: pool.model,
        max_review_rounds: None,
        criteria: vec![NewTaskValidationCriterion {
            criterion_id: None,
            ordinal: 1,
            description: "The hosted route is registered".to_string(),
            expected_evidence: None,
        }],
    };

    let error = store
        .create_task_with_executor(input)
        .await
        .expect_err("authenticated metadata is not runtime readiness");

    assert!(matches!(
        error,
        StoreError::ProviderInstanceUnavailable { .. }
    ));
    assert!(
        store
            .get_task("task:hosted-readiness")
            .await
            .expect("task read")
            .is_none()
    );
}

#[tokio::test]
async fn durable_run_creation_rejects_a_mismatched_exact_key() {
    let store = initialized_task_store().await;
    let mut model = ProviderSelectionSnapshot::explicit(
        "codex",
        "provider_account:codex:default",
        "gpt-5.6-luna",
        None,
        Some("mismatched_test".to_string()),
    );
    model.provider_instance_key = Some(
        ProviderInstanceKey::new("provider-account:v1:5:wrong")
            .expect("syntactically valid wrong key"),
    );

    let error = store
        .create_agent_run(noema_tasks::NewAgentRun {
            run_id: Some("run:mismatched-key".to_string()),
            task_id: "task:not-created".to_string(),
            run_kind: noema_tasks::RunKind::Executor,
            agent_id: noema_tasks::TASK_EXECUTOR_AGENT_ID.to_string(),
            revision_index: 0,
            attempt_index: 0,
            parent_run_id: None,
            triggering_submission_id: None,
            triggering_review_id: None,
            model,
            execution_policy: noema_tasks::TaskExecutionPolicy::default(),
            priority: 0,
        })
        .await
        .expect_err("mismatched key must be rejected before queue insertion");

    assert!(matches!(
        error,
        StoreError::ProviderInstanceKeyMismatch { .. }
    ));
}

#[tokio::test]
async fn preserved_run_creation_rejects_a_claimed_local_instance() {
    let store = test_store().await;
    store
        .ensure_default_local_models_provider_account()
        .await
        .expect("local provider account");
    store
        .update_provider_account_status(
            noema_providers::LOCAL_MODELS_PROVIDER_ACCOUNT_ID,
            ProviderAccountStatus::Authenticated,
            None,
            None,
        )
        .await
        .expect("authenticated local provider account");
    let key = noema_providers::local_model_provider_instance_key(
        noema_providers::LOCAL_MODELS_PROVIDER_ACCOUNT_ID,
        "installation:claimed",
        "model:claimed",
    )
    .expect("local instance key");
    store
        .with_connection(|connection| {
            connection.execute(
                r#"
                INSERT INTO local_model_installations (
                  installation_id, provider_instance_key, model_id, display_name,
                  source_kind, sha256, download_gb, expected_bytes, downloaded_bytes,
                  backend, status, blob_relative_path, is_active,
                  retirement_claimed_at, installed_at
                ) VALUES (
                  'installation:claimed', ?1, 'model:claimed', 'Claimed model',
                  'local_file', ?2, 1.0, 1, 1, 'cpu', 'installed',
                  'models/claimed.gguf', 0, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                  strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                )
                "#,
                rusqlite::params![key.as_str(), "0".repeat(64)],
            )?;
            Ok(())
        })
        .await
        .expect("claimed installation");
    let mut model = ProviderSelectionSnapshot::explicit(
        "local_models",
        noema_providers::LOCAL_MODELS_PROVIDER_ACCOUNT_ID,
        "model:claimed",
        None,
        Some("preserved_retry".to_string()),
    );
    model.provider_instance_key = Some(key.clone());

    let error = store
        .create_agent_run(noema_tasks::NewAgentRun {
            run_id: Some("run:claimed-key".to_string()),
            task_id: "task:not-created".to_string(),
            run_kind: noema_tasks::RunKind::Executor,
            agent_id: noema_tasks::TASK_EXECUTOR_AGENT_ID.to_string(),
            revision_index: 0,
            attempt_index: 1,
            parent_run_id: Some("run:parent".to_string()),
            triggering_submission_id: None,
            triggering_review_id: None,
            model,
            execution_policy: noema_tasks::TaskExecutionPolicy::default(),
            priority: 0,
        })
        .await
        .expect_err("claimed route must not gain a future retry reference");

    assert!(matches!(
        error,
        StoreError::ProviderInstanceClaimed {
            provider_instance_key,
        } if provider_instance_key == key.as_str()
    ));
}
