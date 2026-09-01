//! One-shot initialization of missing canonical provider selections.

use std::str::FromStr;

use noema_providers::{
    ModelPreferenceSelection, NoemaModelUseCase, ProviderKind, ProviderReadySelection,
    ProviderSelectionSnapshot, noema_model_recommendation,
};
use rusqlite::{Transaction, params};

use crate::{
    AuxiliaryModelTask, NoemaStore, StoreError,
    agents::BUILTIN_AGENTS,
    auxiliary_model_preferences::AuxiliaryModelDefault,
    provider_selections::{
        CanonicalPreferenceOwner, SelectionEligibility, explicit_model_preference,
        validate_provider_selection_tx, validate_ready_selection_proof, write_preference_tx,
        write_task_pool_preference_tx,
    },
};

const TASK_POOL_SETTINGS: [(&str, &str); 3] = [
    ("task_pool:setting:simple", "simple"),
    ("task_pool:setting:medium", "medium"),
    ("task_pool:setting:difficult", "difficult"),
];

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
        let provider_kind =
            ProviderKind::from_str(&configured_default.provider_kind).map_err(|provider_kind| {
                StoreError::ConfiguredDefaultUnresolvable {
                    reason: format!("unsupported configured model provider: {provider_kind}"),
                }
            })?;
        self.with_immediate_transaction_retry(|transaction| {
            if canonical_selections_complete(transaction, &provider_kind)? {
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
            let preference = if noema_model_recommendation(
                provider_kind.clone(),
                NoemaModelUseCase::Primary,
            )
            .is_some()
            {
                ModelPreferenceSelection::NoemaRecommended
            } else {
                explicit_model_preference(&selection)?
            };
            insert_missing_default(transaction, &selection, &preference)?;
            insert_missing_agent_preferences(transaction, &selection, &preference)?;
            insert_missing_task_pool(transaction, &selection, &preference)?;
            insert_missing_auxiliary_preferences(
                transaction,
                &selection,
                &preference,
                &provider_kind,
            )?;
            Ok(())
        })
        .await
    }
}

fn canonical_selections_complete(
    transaction: &Transaction<'_>,
    provider_kind: &ProviderKind,
) -> Result<bool, StoreError> {
    let mut required = vec![("default_model_preference", "preference_id", "default")];
    required.extend(
        BUILTIN_AGENTS
            .iter()
            .map(|(agent_id, _, _)| ("agent_runtime_preferences", "agent_id", *agent_id)),
    );
    required.extend(
        TASK_POOL_SETTINGS
            .iter()
            .map(|(pool_id, _)| ("task_model_pool_entries", "pool_entry_id", *pool_id)),
    );
    required.extend(
        AuxiliaryModelTask::ALL
            .iter()
            .copied()
            .filter(|task| {
                task.initial_default(provider_kind) == AuxiliaryModelDefault::ConfiguredProvider
            })
            .map(|task| ("auxiliary_model_preferences", "task_id", task.as_str())),
    );
    for (table, owner_column, owner_id) in required {
        let exists = transaction.query_row(
            &format!("SELECT EXISTS(SELECT 1 FROM {table} WHERE {owner_column} = ?1)"),
            [owner_id],
            |row| row.get::<_, bool>(0),
        )?;
        if !exists {
            return Ok(false);
        }
    }
    Ok(true)
}

pub(super) fn ensure_builtin_agents(transaction: &Transaction<'_>) -> Result<(), StoreError> {
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
    preference: &ModelPreferenceSelection,
) -> Result<(), StoreError> {
    write_preference_tx(
        transaction,
        CanonicalPreferenceOwner::Default,
        selection,
        preference,
        false,
    )
}

fn insert_missing_agent_preferences(
    transaction: &Transaction<'_>,
    selection: &ProviderSelectionSnapshot,
    preference: &ModelPreferenceSelection,
) -> Result<(), StoreError> {
    for (agent_id, _, _) in BUILTIN_AGENTS {
        write_preference_tx(
            transaction,
            CanonicalPreferenceOwner::Agent(agent_id),
            selection,
            preference,
            false,
        )?;
    }
    Ok(())
}

fn insert_missing_task_pool(
    transaction: &Transaction<'_>,
    selection: &ProviderSelectionSnapshot,
    preference: &ModelPreferenceSelection,
) -> Result<(), StoreError> {
    for (pool_entry_id, complexity) in TASK_POOL_SETTINGS {
        write_task_pool_preference_tx(
            transaction,
            pool_entry_id,
            complexity,
            selection,
            preference,
            false,
        )?;
    }
    Ok(())
}

fn insert_missing_auxiliary_preferences(
    transaction: &Transaction<'_>,
    selection: &ProviderSelectionSnapshot,
    preference: &ModelPreferenceSelection,
    provider_kind: &ProviderKind,
) -> Result<(), StoreError> {
    for task in AuxiliaryModelTask::ALL.iter().copied().filter(|task| {
        task.initial_default(provider_kind) == AuxiliaryModelDefault::ConfiguredProvider
    }) {
        write_preference_tx(
            transaction,
            CanonicalPreferenceOwner::Auxiliary(task.as_str()),
            selection,
            preference,
            false,
        )?;
    }
    Ok(())
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
        NewProviderAccount, ProviderAccountStatus, ProviderAuthMethod, ProviderModelProfile,
        ProviderSelectionSnapshot, provider_account_instance_key,
    };
    use serde_json::json;

    use super::*;
    use crate::{
        NewAgentRuntimePreference, ProviderSetupRole, ReadyProviderSetupSelection,
        tests::{exact_provider_selection, ready_provider_selection, test_store},
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
            Some(noema_providers::ReasoningEffort::Medium),
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

    fn codex_setup_assignments() -> Vec<ReadyProviderSetupSelection> {
        [
            ProviderSetupRole::Noema,
            ProviderSetupRole::SimpleTasks,
            ProviderSetupRole::MediumTasks,
            ProviderSetupRole::DifficultTasks,
            ProviderSetupRole::TaskReviewer,
            ProviderSetupRole::WebFetchSummarizer,
            ProviderSetupRole::ToolProgressAudit,
            ProviderSetupRole::ActionReviewer,
            ProviderSetupRole::MemoryConsolidation,
        ]
        .into_iter()
        .map(|role| {
            let ready = codex_default();
            ReadyProviderSetupSelection {
                role,
                selection: ready.selection().clone(),
                ready,
                preference: if role == ProviderSetupRole::DifficultTasks {
                    ModelPreferenceSelection::ExplicitProfile {
                        model_profile: "gpt-5.6-luna".to_string(),
                        reasoning_effort: Some(noema_providers::ReasoningEffort::Medium),
                    }
                } else {
                    ModelPreferenceSelection::NoemaRecommended
                },
            }
        })
        .collect()
    }

    #[tokio::test]
    async fn setup_confirmation_is_complete_atomic_and_first_commit_wins() {
        let store = ready_codex_store().await;
        let assignments = codex_setup_assignments();
        assert!(
            store
                .confirm_provider_setup_selections(&assignments)
                .await
                .expect("first setup confirmation")
        );
        let counts = store
            .with_connection(|connection| {
                Ok((
                    connection.query_row(
                        "SELECT COUNT(*) FROM default_model_preference",
                        [],
                        |row| row.get::<_, i64>(0),
                    )?,
                    connection.query_row(
                        "SELECT COUNT(*) FROM agent_runtime_preferences",
                        [],
                        |row| row.get::<_, i64>(0),
                    )?,
                    connection.query_row(
                        "SELECT COUNT(*) FROM task_model_pool_entries",
                        [],
                        |row| row.get::<_, i64>(0),
                    )?,
                    connection.query_row(
                        "SELECT COUNT(*) FROM auxiliary_model_preferences",
                        [],
                        |row| row.get::<_, i64>(0),
                    )?,
                ))
            })
            .await
            .expect("canonical selection counts");
        assert_eq!(counts, (1, 3, 3, 4));
        assert!(
            !store
                .confirm_provider_setup_selections(&codex_setup_assignments())
                .await
                .expect("later confirmation is ignored")
        );

        let invalid_store = ready_codex_store().await;
        let mut incomplete = codex_setup_assignments();
        incomplete.retain(|assignment| assignment.role != ProviderSetupRole::ActionReviewer);
        assert!(
            invalid_store
                .confirm_provider_setup_selections(&incomplete)
                .await
                .is_err()
        );
        assert!(
            invalid_store
                .get_default_model_preference()
                .await
                .expect("default preference read")
                .is_none()
        );
    }

    #[tokio::test]
    async fn explicit_initializer_can_fill_openrouter_canonical_selections() {
        let store = test_store().await;
        let mut metadata = json!({"credentialRevision": 1, "secretConfigured": true});
        ProviderModelProfile::write_account_metadata(
            &mut metadata,
            &[ProviderModelProfile {
                id: "openrouter/auto".to_string(),
                label: "OpenRouter Auto".to_string(),
                reasoning_efforts: Vec::new(),
                default_reasoning_effort: None,
                context_window_tokens: None,
            }],
        )
        .expect("profile metadata");
        store
            .create_provider_account(NewProviderAccount {
                provider_kind: "openrouter".to_string(),
                display_name: Some("OpenRouter".to_string()),
                auth_method: ProviderAuthMethod::OauthPkce,
                status: ProviderAccountStatus::Authenticated,
                metadata,
            })
            .await
            .expect("OpenRouter account");
        let mut selection = ProviderSelectionSnapshot::explicit(
            "openrouter",
            "provider_account:openrouter:default",
            "openrouter/auto",
            None,
            Some("provider_connection".to_string()),
        );
        selection.provider_instance_key = Some(
            provider_account_instance_key("provider_account:openrouter:default")
                .expect("OpenRouter instance key"),
        );
        let ready = ready_provider_selection(selection.clone());

        store
            .initialize_missing_provider_selections(&selection, Some(&ready))
            .await
            .expect("OpenRouter selections");
        assert_eq!(
            store
                .get_default_model_preference()
                .await
                .expect("default preference")
                .expect("initialized default")
                .provider_kind,
            "openrouter"
        );
    }

    async fn complete_runtime_retired_local_store() -> (NoemaStore, ProviderSelectionSnapshot) {
        let store = test_store().await;
        let installation_id = "local_model_installation:initializer-test";
        let model_id = "initializer-local-model";
        store
            .ensure_default_local_models_provider_account()
            .await
            .expect("local account");
        let installation = store
            .upsert_local_model_installation(crate::tests::local_model_installation(
                installation_id,
                model_id,
                noema_providers::LocalModelBackend::Metal,
            ))
            .await
            .expect("local installation");
        crate::tests::mark_local_model_installed(&store, &installation).await;
        let key = installation.provider_instance_key;
        store
            .with_connection(|connection| {
                connection.execute(
                    "UPDATE local_model_installations SET is_active = 1 WHERE provider_instance_key = ?1",
                    [key.as_str()],
                )?;
                Ok(())
            })
            .await
            .expect("activate local installation fixture");
        let selection = exact_provider_selection(
            "local_models",
            noema_providers::LOCAL_MODELS_PROVIDER_ACCOUNT_ID,
            model_id,
            key.clone(),
            "configured_default",
        );
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

        let default = store.default_provider_selection().await.expect("default");
        assert_eq!(default.provider_instance_key, Some(expected_key.clone()));
        assert_eq!(default.model_profile.as_deref(), Some("gpt-5.6-terra"));
        assert_eq!(
            default.reasoning_effort,
            Some(noema_providers::ReasoningEffort::Medium)
        );
        for agent_id in [
            "agent:primary",
            "agent:task-executor",
            "agent:task-reviewer",
        ] {
            let preference = store
                .get_agent_runtime_preference(agent_id)
                .await
                .expect("agent preference")
                .expect("initialized agent");
            assert_eq!(preference.provider_instance_key, expected_key);
            assert_eq!(
                preference.selection,
                ModelPreferenceSelection::NoemaRecommended
            );
        }
        for task in AuxiliaryModelTask::ALL {
            let preference = store
                .get_auxiliary_model_preference(*task)
                .await
                .expect("auxiliary preference");
            match task.initial_default(&ProviderKind::Codex) {
                AuxiliaryModelDefault::ConfiguredProvider => {
                    let preference = preference.as_ref().expect("configured preference");
                    assert_eq!(preference.provider_instance_key, expected_key);
                    assert_eq!(
                        preference.selection,
                        ModelPreferenceSelection::NoemaRecommended
                    );
                }
                AuxiliaryModelDefault::ExplicitSelectionRequired => assert!(preference.is_none()),
            }
        }
        let pool = store
            .list_task_model_pool_settings(None)
            .await
            .expect("pool settings");
        assert_eq!(pool.len(), 3);
        assert!(pool.iter().all(|entry| {
            entry.model.provider_instance_key.as_ref() == Some(&expected_key)
                && entry.preference == ModelPreferenceSelection::NoemaRecommended
        }));
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
            selection: ModelPreferenceSelection::ExplicitProfile {
                model_profile: noema_providers::DEFAULT_FOUNDATION_LOCAL_PROFILE.to_string(),
                reasoning_effort: None,
            },
            fast_mode: false,
        };
        let model_profile = preference
            .selection
            .model_profile()
            .expect("explicit foundation profile")
            .to_string();
        let ready_selection = ready_provider_selection(ProviderSelectionSnapshot::explicit(
            &preference.provider_kind,
            &preference.provider_account_id,
            model_profile,
            preference.selection.reasoning_effort(),
            Some("foundation_test_route".to_string()),
        ));
        let updated = store
            .upsert_agent_runtime_preference_with_ready_selection(preference, &ready_selection)
            .await
            .expect("update primary route");
        assert!(matches!(
            updated.selection,
            ModelPreferenceSelection::ExplicitProfile { .. }
        ));

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
                .get_auxiliary_model_preference(AuxiliaryModelTask::WebFetchSummarizer)
                .await
                .expect("auxiliary read")
                .is_none()
        );
    }
}
