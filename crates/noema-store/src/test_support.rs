//! Feature-gated store construction support for consumer tests.

use tempfile::TempDir;

use crate::{NoemaStore, StoreConfig, StoreError};

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
                "store-test-support",
                "store-test-support",
            ))
        })
    }
}

/// Open an initialized store under a fresh temporary directory.
///
/// The temporary directory is retained for the test process because
/// [`NoemaStore`] intentionally owns only the database connection and pathless
/// repository state.
///
/// # Errors
///
/// Returns [`StoreError`] when SQLite cannot be opened or bootstrapped.
pub async fn open_ephemeral_store() -> Result<NoemaStore, StoreError> {
    let directory = TempDir::new().map_err(StoreError::PreparePath)?;
    let config = StoreConfig::new(directory.path().join("db/noema.sqlite3"));
    let store = NoemaStore::open(&config).await?;
    let _retained_path = directory.keep();
    Ok(store)
}

/// Create a local conversation and return its stable id.
///
/// # Errors
///
/// Returns [`StoreError`] when the conversation cannot be persisted.
pub async fn create_local_conversation_id(store: &NoemaStore) -> Result<String, StoreError> {
    store
        .create_conversation(noema_conversations::NewConversation::local_chat(None, None))
        .await
        .map(|conversation| conversation.conversation_id)
}

/// Seed one simple task and return its stable id.
///
/// # Errors
///
/// Returns [`StoreError`] when canonical task prerequisites or the task cannot
/// be persisted.
pub async fn create_simple_task_id(store: &NoemaStore, title: &str) -> Result<String, StoreError> {
    store.ensure_default_actors().await?;
    store.ensure_default_provider_account().await?;
    store
        .update_provider_account_status(
            "provider_account:codex:default",
            noema_providers::ProviderAccountStatus::Authenticated,
            None,
            None,
        )
        .await?;
    let mut selection = noema_providers::ProviderSelectionSnapshot::explicit(
        "codex",
        "provider_account:codex:default",
        "gpt-5.6-luna",
        None,
        Some("store_test_support".to_string()),
    );
    let key = noema_providers::provider_account_instance_key(&selection.provider_account_id)
        .map_err(|error| StoreError::InvariantViolation {
            message: error.to_string(),
        })?;
    selection.provider_instance_key = Some(key.clone());
    let registry = noema_providers::ProviderRegistry::new();
    registry
        .register(key, std::sync::Arc::new(ReadyTestProvider))
        .map_err(|error| StoreError::InvariantViolation {
            message: error.to_string(),
        })?;
    let ready_selection = registry
        .prove_ready_selection(selection.clone())
        .map_err(|error| StoreError::InvariantViolation {
            message: error.to_string(),
        })?;
    store
        .initialize_missing_provider_selections(&selection, Some(&ready_selection))
        .await?;
    let pool = store
        .ensure_default_task_model_pool_settings("codex")
        .await?
        .into_iter()
        .find(|entry| entry.complexity == noema_tasks::TaskComplexity::Simple)
        .ok_or_else(|| StoreError::InvariantViolation {
            message: "default simple task model pool entry is missing".to_string(),
        })?;
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
        .map(|(task, _)| task.task_id)
}
