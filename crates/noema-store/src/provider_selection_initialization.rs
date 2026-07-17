//! One-shot initialization of missing canonical provider selections.

use noema_providers::{ProviderReadySelection, ProviderSelectionSnapshot, ReasoningEffort};
use rusqlite::{Transaction, params};

use crate::{
    NoemaStore, StoreError,
    provider_selections::{
        SelectionEligibility, validate_provider_selection_tx, validate_ready_selection_proof,
    },
};

const BUILTIN_AGENTS: [(&str, Option<&str>, &str); 3] = [
    ("agent:primary", None, "primary"),
    (
        "agent:task-executor",
        Some("Task Executor"),
        "task_executor",
    ),
    (
        "agent:task-reviewer",
        Some("Task Reviewer"),
        "task_reviewer",
    ),
];

const TASK_POOL_SETTINGS: [(&str, &str); 3] = [
    ("task_pool:setting:simple", "simple"),
    ("task_pool:setting:medium", "medium"),
    ("task_pool:setting:difficult", "difficult"),
];

const AUXILIARY_TASKS: [&str; 2] = ["tool_progress_audit", "web_fetch_summarizer"];

impl NoemaStore {
    /// Fill every absent canonical selection from one ready configured default.
    ///
    /// Existing rows are never rewritten, even when their exact instances are
    /// temporarily unavailable. Consumers report that availability failure at
    /// use time instead of silently changing the user's route.
    ///
    /// # Errors
    ///
    /// Returns a typed configured-default error when the supplied selection
    /// cannot be resolved and validated in the same writer transaction.
    pub async fn initialize_missing_provider_selections(
        &self,
        configured_default: &ProviderSelectionSnapshot,
        ready_selection: Option<&ProviderReadySelection>,
    ) -> Result<(), StoreError> {
        let configured_default =
            configured_default
                .normalized_for_persistence()
                .map_err(|error| StoreError::ConfiguredDefaultUnresolvable {
                    reason: error.to_string(),
                })?;
        self.with_immediate_transaction_retry(|transaction| {
            if canonical_selections_complete(transaction)? {
                return Ok(());
            }
            let ready_selection = ready_selection.ok_or_else(|| {
                StoreError::ConfiguredDefaultUnresolvable {
                    reason: "canonical provider selections are incomplete and the configured instance is not ready"
                        .to_string(),
                }
            })?;
            validate_ready_selection_proof(&configured_default, ready_selection)
                .map_err(configured_default_error)?;
            let selection = validate_provider_selection_tx(
                transaction,
                &configured_default,
                SelectionEligibility::ConfiguredDefault,
            )
            .map_err(configured_default_error)?;
            if selection.model_profile.is_none() {
                return Err(StoreError::ConfiguredDefaultUnresolvable {
                    reason: "the resolved configured default has no concrete model profile"
                        .to_string(),
                });
            }
            ensure_builtin_agents(transaction)?;
            insert_missing_default(transaction, &selection)?;
            insert_missing_agent_preferences(transaction, &selection)?;
            insert_missing_task_pool(transaction, &selection)?;
            initialize_missing_memory_selection(transaction, &selection)?;
            insert_missing_auxiliary_preferences(transaction, &selection)?;
            Ok(())
        })
        .await
    }
}

fn canonical_selections_complete(transaction: &Transaction<'_>) -> Result<bool, StoreError> {
    transaction
        .query_row(
            r#"
            SELECT
              EXISTS(SELECT 1 FROM default_model_preference WHERE preference_id = 'default')
              AND (SELECT COUNT(*) FROM agent_runtime_preferences
                   WHERE agent_id IN ('agent:primary', 'agent:task-executor', 'agent:task-reviewer')) = 3
              AND (SELECT COUNT(*) FROM task_model_pool_entries
                   WHERE pool_entry_id IN ('task_pool:setting:simple', 'task_pool:setting:medium',
                                           'task_pool:setting:difficult')) = 3
              AND EXISTS(
                SELECT 1 FROM memory_service_settings
                WHERE settings_id = 'default'
                  AND provider_kind IS NOT NULL
                  AND provider_account_id IS NOT NULL
                  AND provider_instance_key IS NOT NULL
                  AND model_profile IS NOT NULL
              )
              AND (SELECT COUNT(*) FROM auxiliary_model_preferences
                   WHERE task_id IN ('tool_progress_audit', 'web_fetch_summarizer')) = 2
            "#,
            [],
            |row| row.get::<_, bool>(0),
        )
        .map_err(StoreError::Sqlite)
}

fn ensure_builtin_agents(transaction: &Transaction<'_>) -> Result<(), StoreError> {
    for (agent_id, display_name, system_role) in BUILTIN_AGENTS {
        transaction.execute(
            r#"
            INSERT INTO agents (agent_id, display_name, system_role)
            VALUES (?1, ?2, ?3)
            ON CONFLICT(agent_id) DO UPDATE SET
              system_role = excluded.system_role,
              updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            "#,
            params![agent_id, display_name, system_role],
        )?;
    }
    Ok(())
}

fn insert_missing_default(
    transaction: &Transaction<'_>,
    selection: &ProviderSelectionSnapshot,
) -> Result<(), StoreError> {
    let key = selection_key(selection)?;
    transaction.execute(
        r#"
        INSERT INTO default_model_preference (
          preference_id, provider_kind, provider_account_id,
          provider_instance_key, model_profile, reasoning_effort
        ) VALUES ('default', ?1, ?2, ?3, ?4, ?5)
        ON CONFLICT(preference_id) DO NOTHING
        "#,
        params![
            selection.provider_kind,
            selection.provider_account_id,
            key,
            selection.model_profile,
            reasoning_effort(selection),
        ],
    )?;
    Ok(())
}

fn insert_missing_agent_preferences(
    transaction: &Transaction<'_>,
    selection: &ProviderSelectionSnapshot,
) -> Result<(), StoreError> {
    let key = selection_key(selection)?;
    for (agent_id, _, _) in BUILTIN_AGENTS {
        transaction.execute(
            r#"
            INSERT INTO agent_runtime_preferences (
              agent_id, provider_kind, provider_account_id,
              provider_instance_key, model_profile, reasoning_effort
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ON CONFLICT(agent_id) DO NOTHING
            "#,
            params![
                agent_id,
                selection.provider_kind,
                selection.provider_account_id,
                key,
                selection.model_profile,
                reasoning_effort(selection),
            ],
        )?;
    }
    Ok(())
}

fn insert_missing_task_pool(
    transaction: &Transaction<'_>,
    selection: &ProviderSelectionSnapshot,
) -> Result<(), StoreError> {
    let key = selection_key(selection)?;
    for (pool_entry_id, complexity) in TASK_POOL_SETTINGS {
        transaction.execute(
            r#"
            INSERT INTO task_model_pool_entries (
              pool_entry_id, complexity, label, provider_kind,
              provider_account_id, provider_instance_key, model_profile,
              reasoning_effort, enabled, sort_order
            ) VALUES (?1, ?2, NULL, ?3, ?4, ?5, ?6, ?7, 1, 0)
            ON CONFLICT(pool_entry_id) DO NOTHING
            "#,
            params![
                pool_entry_id,
                complexity,
                selection.provider_kind,
                selection.provider_account_id,
                key,
                selection.model_profile,
                reasoning_effort(selection),
            ],
        )?;
    }
    Ok(())
}

fn initialize_missing_memory_selection(
    transaction: &Transaction<'_>,
    selection: &ProviderSelectionSnapshot,
) -> Result<(), StoreError> {
    let key = selection_key(selection)?;
    transaction.execute(
        r#"
        UPDATE memory_service_settings
        SET provider_kind = ?1,
            provider_account_id = ?2,
            provider_instance_key = ?3,
            model_profile = ?4,
            reasoning_effort = ?5,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE settings_id = 'default'
          AND provider_kind IS NULL
          AND provider_account_id IS NULL
          AND provider_instance_key IS NULL
          AND model_profile IS NULL
        "#,
        params![
            selection.provider_kind,
            selection.provider_account_id,
            key,
            selection.model_profile,
            reasoning_effort(selection),
        ],
    )?;
    Ok(())
}

fn insert_missing_auxiliary_preferences(
    transaction: &Transaction<'_>,
    selection: &ProviderSelectionSnapshot,
) -> Result<(), StoreError> {
    let key = selection_key(selection)?;
    for task_id in AUXILIARY_TASKS {
        transaction.execute(
            r#"
            INSERT INTO auxiliary_model_preferences (
              task_id, provider_kind, provider_account_id,
              provider_instance_key, model_profile, reasoning_effort
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ON CONFLICT(task_id) DO NOTHING
            "#,
            params![
                task_id,
                selection.provider_kind,
                selection.provider_account_id,
                key,
                selection.model_profile,
                reasoning_effort(selection),
            ],
        )?;
    }
    Ok(())
}

fn selection_key(selection: &ProviderSelectionSnapshot) -> Result<&str, StoreError> {
    selection
        .provider_instance_key
        .as_ref()
        .map(noema_providers::ProviderInstanceKey::as_str)
        .ok_or(StoreError::ProviderInstanceKeyMissing)
}

fn reasoning_effort(selection: &ProviderSelectionSnapshot) -> Option<&'static str> {
    selection
        .reasoning_effort
        .map(ReasoningEffort::as_persistence_str)
}

fn configured_default_error(error: StoreError) -> StoreError {
    match error {
        StoreError::Sqlite(_) | StoreError::Json(_) => error,
        other => StoreError::ConfiguredDefaultUnresolvable {
            reason: other.to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use noema_providers::{
        ProviderAccountStatus, ProviderSelectionSnapshot, provider_account_instance_key,
    };

    use super::*;
    use crate::{
        NewAgentRuntimePreference, WEB_FETCH_SUMMARIZER_TASK_ID,
        tests::{ready_provider_selection, test_store},
    };

    async fn ready_codex_store() -> NoemaStore {
        let store = test_store().await;
        store
            .ensure_default_provider_account()
            .await
            .expect("default account");
        store
            .update_provider_account_status(
                "provider_account:codex:default",
                ProviderAccountStatus::Authenticated,
                None,
                None,
            )
            .await
            .expect("ready account");
        store
    }

    fn codex_default() -> ProviderReadySelection {
        ready_provider_selection(ProviderSelectionSnapshot::explicit(
            "codex",
            "provider_account:codex:default",
            "gpt-5.6-luna",
            None,
            Some("configured_default".to_string()),
        ))
    }

    async fn initialize_codex(store: &NoemaStore) -> Result<(), StoreError> {
        let ready_selection = codex_default();
        store
            .initialize_missing_provider_selections(
                ready_selection.selection(),
                Some(&ready_selection),
            )
            .await
    }

    async fn complete_runtime_retired_local_store() -> (NoemaStore, ProviderSelectionSnapshot) {
        let store = test_store().await;
        let installation_id = "local_model_installation:initializer-test";
        let model_id = "initializer-local-model";
        let key = noema_providers::local_model_provider_instance_key(
            noema_providers::LOCAL_MODELS_PROVIDER_ACCOUNT_ID,
            installation_id,
            model_id,
        )
        .expect("local instance key");
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
                    ) VALUES (?1, ?2, ?3, 'Initializer local model', 'local_file',
                              'initializer.gguf', ?4, 1.0, 1, 1, 'metal', 'installed',
                              'models/blobs/initializer.gguf', 1,
                              strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
                    "#,
                    params![installation_id, key.as_str(), model_id, "a".repeat(64)],
                )
                .expect("installed local model");
        }
        let mut selection = ProviderSelectionSnapshot::explicit(
            "local_models",
            noema_providers::LOCAL_MODELS_PROVIDER_ACCOUNT_ID,
            model_id,
            None,
            Some("configured_default".to_string()),
        );
        selection.provider_instance_key = Some(key.clone());
        let ready_selection = ready_provider_selection(selection.clone());
        store
            .initialize_missing_provider_selections(&selection, Some(&ready_selection))
            .await
            .expect("initial local selection seed");
        {
            let connection = store.connection_for_tests();
            let connection = connection.lock().await;
            connection
                .execute(
                    "UPDATE local_model_installations SET is_active = 0, runtime_retired_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE provider_instance_key = ?1",
                    [key.as_str()],
                )
                .expect("runtime retire local model");
        }
        (store, selection)
    }

    #[tokio::test]
    async fn initializer_fills_every_missing_canonical_selection_with_one_exact_key() {
        let store = ready_codex_store().await;
        initialize_codex(&store)
            .await
            .expect("initialize selections");
        let expected_key =
            provider_account_instance_key("provider_account:codex:default").expect("hosted key");

        assert_eq!(
            store
                .default_provider_selection()
                .await
                .expect("default")
                .provider_instance_key,
            Some(expected_key.clone())
        );
        for agent_id in [
            "agent:primary",
            "agent:task-executor",
            "agent:task-reviewer",
        ] {
            assert_eq!(
                store
                    .get_agent_runtime_preference(agent_id)
                    .await
                    .expect("agent preference")
                    .expect("initialized agent")
                    .provider_instance_key,
                expected_key
            );
        }
        assert_eq!(
            store
                .get_auxiliary_model_preference(WEB_FETCH_SUMMARIZER_TASK_ID)
                .await
                .expect("auxiliary preference")
                .expect("initialized auxiliary")
                .provider_instance_key,
            expected_key
        );
        assert_eq!(
            store
                .memory_service_settings()
                .await
                .expect("memory settings")
                .provider_instance_key,
            Some(expected_key.clone())
        );
        let pool = store
            .list_task_model_pool_settings(None)
            .await
            .expect("pool settings");
        assert_eq!(pool.len(), 3);
        assert!(
            pool.iter()
                .all(|entry| { entry.model.provider_instance_key.as_ref() == Some(&expected_key) })
        );
    }

    #[tokio::test]
    async fn initializer_never_rewrites_an_existing_exact_selection() {
        let store = ready_codex_store().await;
        initialize_codex(&store)
            .await
            .expect("initialize selections");
        store
            .ensure_default_foundation_local_provider_account()
            .await
            .expect("foundation account");
        store
            .update_provider_account_status(
                "provider_account:foundation_local:default",
                ProviderAccountStatus::Authenticated,
                None,
                None,
            )
            .await
            .expect("ready foundation account");
        let preference = NewAgentRuntimePreference {
            agent_id: "agent:primary".to_string(),
            provider_kind: "foundation_local".to_string(),
            provider_account_id: "provider_account:foundation_local:default".to_string(),
            model_profile: noema_providers::DEFAULT_FOUNDATION_LOCAL_PROFILE.to_string(),
            reasoning_effort: None,
        };
        let ready_selection = ready_provider_selection(ProviderSelectionSnapshot::explicit(
            &preference.provider_kind,
            &preference.provider_account_id,
            &preference.model_profile,
            preference.reasoning_effort,
            Some("foundation_test_route".to_string()),
        ));
        let updated = store
            .upsert_agent_runtime_preference_with_ready_selection(preference, &ready_selection)
            .await
            .expect("update primary route");

        initialize_codex(&store)
            .await
            .expect("repeat initialization");

        assert_eq!(
            store
                .get_agent_runtime_preference("agent:primary")
                .await
                .expect("primary preference")
                .expect("existing primary")
                .provider_instance_key,
            updated.provider_instance_key
        );
    }

    #[tokio::test]
    async fn unresolvable_configured_default_is_typed_and_writes_nothing() {
        let store = test_store().await;
        let ready_selection = codex_default();
        let error = store
            .initialize_missing_provider_selections(
                ready_selection.selection(),
                Some(&ready_selection),
            )
            .await
            .expect_err("missing configured account");

        assert!(matches!(
            error,
            StoreError::ConfiguredDefaultUnresolvable { .. }
        ));
        assert!(
            store
                .get_default_model_preference()
                .await
                .expect("default read")
                .is_none()
        );
    }

    #[tokio::test]
    async fn complete_local_initialization_needs_no_ready_proof_and_writes_nothing() {
        let (store, selection) = complete_runtime_retired_local_store().await;
        let before_changes = {
            let connection = store.connection_for_tests();
            let connection = connection.lock().await;
            connection.total_changes()
        };

        store
            .initialize_missing_provider_selections(&selection, None)
            .await
            .expect("complete initialization is preserved while runtime is unready");

        let after_changes = {
            let connection = store.connection_for_tests();
            let connection = connection.lock().await;
            connection.total_changes()
        };
        assert_eq!(after_changes, before_changes);
    }

    #[tokio::test]
    async fn incomplete_local_initialization_without_a_ready_proof_fails_without_filling() {
        let (store, selection) = complete_runtime_retired_local_store().await;
        {
            let connection = store.connection_for_tests();
            let connection = connection.lock().await;
            connection
                .execute(
                    "DELETE FROM auxiliary_model_preferences WHERE task_id = 'web_fetch_summarizer'",
                    [],
                )
                .expect("remove one canonical selection");
        }

        let error = store
            .initialize_missing_provider_selections(&selection, None)
            .await
            .expect_err("missing selection requires ready proof");

        assert!(matches!(
            error,
            StoreError::ConfiguredDefaultUnresolvable { .. }
        ));
        assert!(
            store
                .get_auxiliary_model_preference("web_fetch_summarizer")
                .await
                .expect("auxiliary read")
                .is_none()
        );
    }
}
