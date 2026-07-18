//! Feature-gated store construction support for consumer tests.
#![allow(clippy::missing_errors_doc)]

#[cfg(any(test, feature = "test-support"))]
use std::sync::Arc;

#[cfg(any(test, feature = "test-support"))]
use noema_providers::{
    GenerateRequest, GenerateResponse, GenerateStreamEvent, ProviderInstanceKey,
    ProviderOperationFuture, ProviderOperations, ProviderReadySelection, ProviderRegistry,
    ProviderRegistryHandle, ProviderSelectionSnapshot, provider_account_instance_key,
};
#[cfg(any(test, feature = "test-support"))]
use noema_tasks::{
    AgentRunRecord, NewTask, NewTaskValidationCriterion, TaskComplexity, TaskRecord, TaskSource,
};
use tempfile::TempDir;

use crate::{NoemaStore, StoreConfig, StoreError};

#[cfg(any(test, feature = "test-support"))]
#[derive(Debug)]
struct ReadyTestProvider;

#[cfg(any(test, feature = "test-support"))]
fn invariant(error: impl std::fmt::Display) -> StoreError {
    StoreError::InvariantViolation {
        message: error.to_string(),
    }
}

#[cfg(any(test, feature = "test-support"))]
impl ProviderOperations for ReadyTestProvider {
    fn generate_streaming<'a>(
        &'a self,
        _request: GenerateRequest,
        _on_event: &'a mut (dyn FnMut(GenerateStreamEvent) + Send),
    ) -> ProviderOperationFuture<'a, GenerateResponse> {
        Box::pin(async {
            Ok(GenerateResponse::final_text(
                "ready",
                "store-test-support",
                "store-test-support",
            ))
        })
    }
}

/// Register one inert ready provider under an exact test key.
#[cfg(any(test, feature = "test-support"))]
pub fn register_ready_provider(
    registry: &ProviderRegistry,
    key: ProviderInstanceKey,
) -> Result<(), StoreError> {
    registry
        .register(key, Arc::new(ReadyTestProvider))
        .map(|_| ())
        .map_err(invariant)
}

#[cfg(any(test, feature = "test-support"))]
/// Build a registry whose route for `selection` is immediately ready.
pub fn ready_provider_registry(
    selection: &ProviderSelectionSnapshot,
) -> Result<ProviderRegistry, StoreError> {
    let key = match &selection.provider_instance_key {
        Some(key) => key.clone(),
        None if selection.provider_kind != "local_models" => {
            provider_account_instance_key(&selection.provider_account_id).map_err(invariant)?
        }
        None => {
            return Err(StoreError::InvariantViolation {
                message: "ready selection requires an exact provider instance key".to_string(),
            });
        }
    };
    let registry = ProviderRegistry::new();
    register_ready_provider(&registry, key)?;
    Ok(registry)
}

#[cfg(any(test, feature = "test-support"))]
/// Prove an exact ready selection against an inert test provider.
pub fn ready_provider_selection(
    mut selection: ProviderSelectionSnapshot,
) -> Result<ProviderReadySelection, StoreError> {
    let registry = ready_provider_registry(&selection)?;
    if selection.provider_instance_key.is_none() && selection.provider_kind != "local_models" {
        selection.provider_instance_key =
            Some(provider_account_instance_key(&selection.provider_account_id).map_err(invariant)?);
    }
    registry.prove_ready_selection(selection).map_err(invariant)
}

/// Build one ready registry from hosted provider account ids.
#[cfg(any(test, feature = "test-support"))]
pub fn ready_hosted_provider_registry(
    account_ids: impl IntoIterator<Item = impl AsRef<str>>,
) -> Result<ProviderRegistryHandle, StoreError> {
    let registry = Arc::new(ProviderRegistry::new());
    for account_id in account_ids {
        let key = provider_account_instance_key(account_id.as_ref()).map_err(invariant)?;
        register_ready_provider(&registry, key)?;
    }
    Ok(registry)
}

/// Initialize the canonical Codex selections used by contract tests.
#[cfg(any(test, feature = "test-support"))]
pub async fn initialize_codex_provider_selections(store: &NoemaStore) -> Result<(), StoreError> {
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
    let selection = ProviderSelectionSnapshot::explicit(
        "codex",
        "provider_account:codex:default",
        "gpt-5.6-luna",
        None,
        Some("test_configured_default".to_string()),
    );
    let ready = ready_provider_selection(selection)?;
    store
        .initialize_missing_provider_selections(ready.selection(), Some(&ready))
        .await
}

/// Seed one simple task and its executor run for a contract test.
#[cfg(any(test, feature = "test-support"))]
pub async fn seed_task(
    store: &NoemaStore,
    title: &str,
) -> Result<(TaskRecord, AgentRunRecord), StoreError> {
    initialize_codex_provider_selections(store).await?;
    let pool = store
        .ensure_default_task_model_pool_settings("codex")
        .await?
        .into_iter()
        .find(|entry| entry.complexity == TaskComplexity::Simple)
        .ok_or_else(|| StoreError::InvariantViolation {
            message: "default simple task model is missing".to_string(),
        })?;
    let registry = ready_hosted_provider_registry(["provider_account:codex:default"])?;
    store
        .create_task_with_executor_with_readiness(
            NewTask {
                task_id: None,
                title: title.to_string(),
                request_markdown: "Complete the task".to_string(),
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
                    description: "Task is complete".to_string(),
                    expected_evidence: None,
                }],
            },
            registry.as_ref(),
        )
        .await
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

/// Return whether a task was persisted for one delegation call.
///
/// # Errors
///
/// Returns [`StoreError`] when the test-owned store query fails.
pub async fn task_created_by_call(
    store: &NoemaStore,
    conversation_id: &str,
    call_id: &str,
) -> Result<bool, StoreError> {
    store
        .with_connection(|conn| {
            conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM tasks WHERE source_conversation_id = ?1 AND creation_tool_call_id = ?2)",
                [conversation_id, call_id],
                |row| row.get(0),
            )
            .map_err(StoreError::Sqlite)
        })
        .await
}
