use tempfile::TempDir;

use crate::StoreConfig;

#[derive(Debug)]
struct ReadyTestProvider;

impl noema_providers::ProviderOperations for ReadyTestProvider {
    fn generate_streaming<'a>(
        &'a self,
        _request: noema_providers::GenerateRequest,
        _on_event: &'a mut (dyn FnMut(noema_providers::GenerateStreamEvent) + Send),
    ) -> noema_providers::ProviderOperationFuture<'a, noema_providers::GenerateResponse> {
        Box::pin(async {
            Ok(noema_providers::GenerateResponse::final_text(
                "ready",
                "store-test",
                "store-test",
            ))
        })
    }
}

pub(super) fn store_config(root: &std::path::Path) -> StoreConfig {
    StoreConfig::new(root.join("db/noema.sqlite3"))
}

pub(crate) async fn test_store() -> crate::NoemaStore {
    let home = TempDir::new().expect("temp store root");
    let store = crate::NoemaStore::open(&store_config(home.path()))
        .await
        .expect("open store");
    std::mem::forget(home);
    store
}

pub(crate) fn ready_provider_selection(
    selection: noema_providers::ProviderSelectionSnapshot,
) -> noema_providers::ProviderReadySelection {
    let registry = ready_provider_registry(&selection);
    let mut selection = selection;
    if selection.provider_instance_key.is_none() && selection.provider_kind != "local_models" {
        selection.provider_instance_key = Some(
            noema_providers::provider_account_instance_key(&selection.provider_account_id)
                .expect("hosted provider key"),
        );
    }
    registry
        .prove_ready_selection(selection)
        .expect("prove ready selection")
}

pub(crate) fn ready_provider_registry(
    selection: &noema_providers::ProviderSelectionSnapshot,
) -> noema_providers::ProviderRegistry {
    let key = selection
        .provider_instance_key
        .clone()
        .or_else(|| {
            (selection.provider_kind != "local_models")
                .then(|| {
                    noema_providers::provider_account_instance_key(&selection.provider_account_id)
                })
                .transpose()
                .expect("hosted provider key")
        })
        .expect("ready selection requires an exact key");
    let registry = noema_providers::ProviderRegistry::new();
    registry
        .register(key, std::sync::Arc::new(ReadyTestProvider))
        .expect("register test provider");
    registry
}

pub(crate) fn ready_codex_registry() -> noema_providers::ProviderRegistry {
    ready_provider_registry(&noema_providers::ProviderSelectionSnapshot::explicit(
        "codex",
        "provider_account:codex:default",
        "gpt-5.6-luna",
        None,
        Some("store_test_runtime".to_string()),
    ))
}

pub(crate) async fn seed_task(
    store: &crate::NoemaStore,
    title: &str,
) -> (noema_tasks::TaskRecord, noema_tasks::AgentRunRecord) {
    store.ensure_default_actors().await.expect("actors");
    store
        .ensure_default_provider_account()
        .await
        .expect("provider account");
    store
        .update_provider_account_status(
            "provider_account:codex:default",
            noema_providers::ProviderAccountStatus::Authenticated,
            None,
            None,
        )
        .await
        .expect("authenticated provider account");
    let ready_selection =
        ready_provider_selection(noema_providers::ProviderSelectionSnapshot::explicit(
            "codex",
            "provider_account:codex:default",
            "gpt-5.6-luna",
            None,
            Some("test_configured_default".to_string()),
        ));
    store
        .initialize_missing_provider_selections(ready_selection.selection(), Some(&ready_selection))
        .await
        .expect("initialized provider selections");
    let pool = store
        .ensure_default_task_model_pool_settings("codex")
        .await
        .expect("task model settings")
        .into_iter()
        .find(|entry| entry.complexity == noema_tasks::TaskComplexity::Simple)
        .expect("simple task model");
    let registry = ready_codex_registry();
    store
        .create_task_with_executor_with_readiness(
            noema_tasks::NewTask {
                task_id: None,
                title: title.to_string(),
                request_markdown: "Complete the task".to_string(),
                complexity: noema_tasks::TaskComplexity::Simple,
                owner_human_id: "human:local".to_string(),
                source: noema_tasks::TaskSource::default(),
                created_by_agent_id: "agent:primary".to_string(),
                creation_tool_call_id: None,
                pool_entry_id: pool.pool_entry_id,
                executor_model: pool.model.clone(),
                reviewer_model: pool.model,
                max_review_rounds: None,
                criteria: vec![noema_tasks::NewTaskValidationCriterion {
                    criterion_id: None,
                    ordinal: 1,
                    description: "Task is complete".to_string(),
                    expected_evidence: None,
                }],
            },
            &registry,
        )
        .await
        .expect("task")
}
