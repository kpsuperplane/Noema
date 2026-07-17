use tempfile::TempDir;

use crate::StoreConfig;

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
    let pool = store
        .ensure_default_task_model_pool_settings("codex")
        .await
        .expect("task model settings")
        .into_iter()
        .find(|entry| entry.complexity == noema_tasks::TaskComplexity::Simple)
        .expect("simple task model");
    store
        .create_task_with_executor(noema_tasks::NewTask {
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
        })
        .await
        .expect("task")
}
