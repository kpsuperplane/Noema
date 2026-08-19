//! Feature-gated store construction support for consumer tests.
#[cfg(any(test, feature = "test-support"))]
use std::sync::Arc;

#[cfg(any(test, feature = "test-support"))]
use noema_providers::{
    GenerateRequest, GenerateResponse, GenerateStreamEvent, ProviderInstanceKey,
    ProviderOperationFuture, ProviderOperations, ProviderReadySelection, ProviderRegistry,
    ProviderRegistryHandle, ProviderSelectionSnapshot, provider_account_instance_key,
};
use noema_tasks::{
    CaptureTask, CommandMeta, TaskProvenance, TaskRecord, TaskSourceKind, WorkCommand,
};
use noema_workspaces::{PERSONAL_WORKSPACE_ID, WorkspaceId};
use tempfile::TempDir;

use crate::{NoemaStore, StoreConfig, StoreError, WorkCommandService};

#[cfg(any(test, feature = "test-support"))]
#[derive(Debug, Default)]
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
///
/// # Errors
///
/// Returns [`StoreError`] when the provider key is invalid or already
/// registered.
#[cfg(any(test, feature = "test-support"))]
pub fn register_ready_provider(
    registry: &ProviderRegistry,
    key: ProviderInstanceKey,
) -> Result<(), StoreError> {
    registry
        .register(key, Arc::new(ReadyTestProvider::default()))
        .map(|_| ())
        .map_err(invariant)
}

#[cfg(any(test, feature = "test-support"))]
/// Build a registry whose route for `selection` is immediately ready.
///
/// # Errors
///
/// Returns [`StoreError`] when the selection cannot resolve to an exact
/// provider instance key or the test provider cannot be registered.
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
///
/// # Errors
///
/// Returns [`StoreError`] when the selection cannot resolve to an exact
/// provider route or readiness cannot be proven.
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
///
/// # Errors
///
/// Returns [`StoreError`] when an account id cannot form a provider key or a
/// provider cannot be registered.
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

/// Insert the minimum MCP server authority required by consumer runtime tests.
///
/// # Errors
///
/// Returns [`StoreError`] when the test authority cannot be persisted.
#[cfg(any(test, feature = "test-support"))]
pub async fn insert_mcp_server(store: &NoemaStore, mcp_server_id: &str) -> Result<(), StoreError> {
    let mcp_server_id = mcp_server_id.to_string();
    store
        .with_connection(|connection| {
            connection.execute(
                "INSERT INTO mcp_definitions (mcp_definition_id, display_name, transport_kind, safe_config_json, definition_revision) VALUES (?1, 'Test MCP', 'streamable_http', '{}', ?2)",
                rusqlite::params![
                    format!("mcp_definition:{mcp_server_id}"),
                    format!("mcp_definition_revision:{mcp_server_id}"),
                ],
            )?;
            connection.execute(
                "INSERT INTO mcp_servers (mcp_server_id, mcp_definition_id, connection_config_json, auth_status, health_status, enabled) VALUES (?1, ?2, '{}', 'needs_auth', 'unavailable', 1)",
                rusqlite::params![mcp_server_id, format!("mcp_definition:{mcp_server_id}")],
            )?;
            Ok(())
        })
        .await
}

/// Remove one saved conversation item from visible replay for a consumer test.
///
/// # Errors
///
/// Returns [`StoreError`] when the exact item cannot be updated.
#[cfg(any(test, feature = "test-support"))]
pub async fn hide_conversation_item_from_visible_replay(
    store: &NoemaStore,
    item_id: &str,
) -> Result<(), StoreError> {
    let item_id = item_id.to_string();
    store
        .with_connection(|connection| {
            let changed = connection.execute(
                "UPDATE conversation_items SET deleted_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE item_id = ?1 AND deleted_at IS NULL",
                [item_id],
            )?;
            if changed != 1 {
                return Err(invariant("test conversation item was not visible"));
            }
            Ok(())
        })
        .await
}

/// Initialize the canonical Codex selections used by contract tests.
///
/// # Errors
///
/// Returns [`StoreError`] when provider defaults, account state, or canonical
/// selections cannot be persisted and proven ready.
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

/// Capture a test-owned task through the semantic Work command writer.
///
/// The task remains in the personal Inbox and carries Work UI provenance. Each
/// call creates a distinct task because test fixtures do not use an
/// idempotency key.
///
/// # Errors
///
/// Returns [`StoreError`] when the test provider route cannot be initialized,
/// the capture command is invalid, SQLite fails, or the command result omits
/// its captured task projection.
pub async fn capture_work_task(store: &NoemaStore, title: &str) -> Result<TaskRecord, StoreError> {
    initialize_codex_provider_selections(store).await?;
    let registry = ready_hosted_provider_registry(["provider_account:codex:default"])?;
    let result = WorkCommandService::new(store.clone(), registry)
        .execute(WorkCommand::CaptureTask(CaptureTask {
            meta: CommandMeta {
                actor_id: "actor:human:local".to_string(),
                causation_id: None,
                correlation_id: "correlation:test-support:capture-work-task".to_string(),
                idempotency_key: None,
            },
            workspace_id: WorkspaceId::new(PERSONAL_WORKSPACE_ID).map_err(invariant)?,
            title: title.to_string(),
            task_document_markdown: String::new(),
            project_id: None,
            provenance: TaskProvenance {
                source_kind: TaskSourceKind::WorkUi,
                created_by_actor_id: "actor:human:local".to_string(),
                ..TaskProvenance::default()
            },
            schedule: None,
            executor_agent_id: None,
            cwd_override: None,
        }))
        .await?;
    result.task.ok_or_else(|| StoreError::InvariantViolation {
        message: "capture command omitted its task projection".to_string(),
    })
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
                "SELECT EXISTS(SELECT 1 FROM tasks WHERE source_conversation_id = ?1 AND source_tool_call_id = ?2)",
                [conversation_id, call_id],
                |row| row.get(0),
            )
            .map_err(StoreError::Sqlite)
        })
        .await
}

/// Force one current queued/active Work run into a deliberate missing-run fixture.
///
/// This helper exists only for consumer reconciliation tests. It atomically
/// cancels the exact current run, clears the task's current-run pointer, and
/// moves the task to a deterministic old pagination timestamp.
///
/// # Errors
///
/// Returns [`StoreError`] and rolls back both writes unless the exact run and
/// its exact Queue/Doing task pointer are each changed once.
pub async fn force_work_task_missing_current_run(
    store: &NoemaStore,
    task_id: &noema_tasks::TaskId,
    run_id: &str,
) -> Result<(), StoreError> {
    if run_id.trim().is_empty() {
        return Err(StoreError::Work(
            noema_tasks::WorkDomainError::InvalidInput {
                field: "test_support.run_id",
                message: "run id cannot be blank".to_string(),
            },
        ));
    }
    store
        .with_immediate_transaction_retry(|transaction| {
            let run_changed = transaction.execute(
                "UPDATE agent_runs
                 SET status = 'cancelled', cancellation_requested = 1,
                     ended_at = COALESCE(ended_at, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
                     lease_owner = NULL, lease_token = NULL, lease_expires_at = NULL,
                     updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                 WHERE run_id = ?1 AND task_id = ?2
                   AND status IN ('queued', 'leased', 'running')
                   AND EXISTS (
                     SELECT 1 FROM tasks task
                     WHERE task.task_id = ?2 AND task.latest_run_id = ?1
                       AND task.stage_id IN ('stage:personal:queue', 'stage:personal:doing')
                   )",
                rusqlite::params![run_id, task_id.as_str()],
            )?;
            if run_changed != 1 {
                return Err(StoreError::InvariantViolation {
                    message: "test fixture did not match one current queued/active run".to_string(),
                });
            }
            let task_changed = transaction.execute(
                "UPDATE tasks
                 SET latest_run_id = NULL, updated_at = '2000-01-01T00:00:00.000Z'
                 WHERE task_id = ?1 AND latest_run_id = ?2
                   AND stage_id IN ('stage:personal:queue', 'stage:personal:doing')",
                rusqlite::params![task_id.as_str(), run_id],
            )?;
            if task_changed != 1 {
                return Err(StoreError::InvariantViolation {
                    message: "test fixture did not clear one exact current-run pointer".to_string(),
                });
            }
            Ok(())
        })
        .await
}
