//! Atomic activation of one installed local model across current workloads.

use rusqlite::{OptionalExtension, Transaction, params};

use noema_providers::{
    LOCAL_MODELS_PROVIDER_ACCOUNT_ID, LocalModelEventKind, LocalModelInstallationRecord,
    LocalModelInstallationStatus, ProviderReadySelection,
};

use super::{
    NoemaStore, StoreError,
    local_model_rows::{INSTALLATION_SELECT, installation_from_raw, raw_installation_from_row},
    local_models::append_event,
    provider_selections::{
        CanonicalPreferenceOwner, write_preference_tx, write_task_pool_preference_tx,
    },
};

impl NoemaStore {
    /// Make one installed local model Noema's explicit system default.
    ///
    /// This is the only store operation that rewrites all current model
    /// workloads, so adding another provider later preserves these selections.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] unless the installation is complete, or when any
    /// write in the atomic activation transaction fails.
    pub async fn activate_local_model_as_system_default(
        &self,
        installation_id: &str,
        ready_selection: &ProviderReadySelection,
    ) -> Result<LocalModelInstallationRecord, StoreError> {
        self.with_immediate_transaction_retry(|transaction| {
            let selection = ready_selection.selection();
            if selection.provider_kind != "local_models"
                || selection.provider_account_id != LOCAL_MODELS_PROVIDER_ACCOUNT_ID
                || selection.model_profile.is_none()
                || selection.reasoning_effort.is_some()
                || selection.provider_instance_key.as_ref() != Some(ready_selection.key())
            {
                return Err(StoreError::InvariantViolation {
                    message: "local-model activation requires an exact ready local selection"
                        .to_string(),
                });
            }
            let (model_id, provider_instance_key, status, retirement_claimed_at): (
                String,
                String,
                String,
                Option<String>,
            ) = transaction
                .query_row(
                    "SELECT model_id, provider_instance_key, status, retirement_claimed_at \
                     FROM local_model_installations WHERE installation_id = ?1",
                    [installation_id],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                )
                .optional()?
                .ok_or_else(|| StoreError::LocalModelInstallationNotFound {
                    installation_id: installation_id.to_string(),
                })?;
            if provider_instance_key != ready_selection.key().as_str()
                || selection.model_profile.as_deref() != Some(model_id.as_str())
            {
                return Err(StoreError::ProviderInstanceKeyMismatch {
                    provider_instance_key: ready_selection.key().to_string(),
                });
            }
            if status != LocalModelInstallationStatus::Installed.as_str() {
                return Err(StoreError::LocalModelActivationNotReady {
                    installation_id: installation_id.to_string(),
                    status,
                });
            }
            if retirement_claimed_at.is_some() {
                return Err(StoreError::ProviderInstanceClaimed {
                    provider_instance_key,
                });
            }

            ensure_local_provider_account(transaction)?;
            ensure_builtin_agents(transaction)?;
            transaction.execute("UPDATE local_model_installations SET is_active = 0", [])?;
            let changed = transaction.execute(
                "UPDATE local_model_installations SET is_active = 1, \
                 runtime_retired_at = NULL, \
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
                 WHERE installation_id = ?1 AND retirement_claimed_at IS NULL",
                [installation_id],
            )?;
            if changed != 1 {
                return Err(StoreError::ProviderInstanceClaimed {
                    provider_instance_key,
                });
            }
            write_preference_tx(
                transaction,
                CanonicalPreferenceOwner::Default,
                selection,
                true,
            )?;
            save_agent_preferences(transaction, selection)?;
            save_task_pool_preferences(transaction, selection)?;
            save_auxiliary_preferences(transaction, selection)?;
            append_event(
                transaction,
                installation_id,
                LocalModelEventKind::Activated,
                None,
                None,
                None,
            )?;
            activated_installation(transaction, installation_id)
        })
        .await
    }
}

fn activated_installation(
    transaction: &Transaction<'_>,
    installation_id: &str,
) -> Result<LocalModelInstallationRecord, StoreError> {
    let raw = transaction
        .query_row(
            &format!("{INSTALLATION_SELECT} WHERE installation_id = ?1 LIMIT 1"),
            [installation_id],
            raw_installation_from_row,
        )
        .map_err(StoreError::Sqlite)?;
    installation_from_raw(raw)
}

fn ensure_local_provider_account(transaction: &Transaction<'_>) -> Result<(), StoreError> {
    transaction.execute(
        r#"
        INSERT INTO provider_accounts (
          provider_account_id, provider_kind, account_key, display_name,
          auth_method, is_active, is_default, status, metadata_json
        ) VALUES (?1, 'local_models', 'default', 'Local models', 'none', 1, 1, 'authenticated', '{}')
        ON CONFLICT(provider_account_id) DO UPDATE SET
          is_active = 1,
          is_default = 1,
          status = 'authenticated',
          last_authenticated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
          last_error_code = NULL,
          last_error_message = NULL,
          updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        "#,
        [LOCAL_MODELS_PROVIDER_ACCOUNT_ID],
    )?;
    Ok(())
}

fn ensure_builtin_agents(transaction: &Transaction<'_>) -> Result<(), StoreError> {
    for (agent_id, display_name, system_role) in [
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
    ] {
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

fn save_agent_preferences(
    transaction: &Transaction<'_>,
    selection: &noema_providers::ProviderSelectionSnapshot,
) -> Result<(), StoreError> {
    for agent_id in [
        "agent:primary",
        "agent:task-executor",
        "agent:task-reviewer",
    ] {
        write_preference_tx(
            transaction,
            CanonicalPreferenceOwner::Agent(agent_id),
            selection,
            true,
        )?;
    }
    Ok(())
}

fn save_task_pool_preferences(
    transaction: &Transaction<'_>,
    selection: &noema_providers::ProviderSelectionSnapshot,
) -> Result<(), StoreError> {
    for complexity in ["simple", "medium", "difficult"] {
        let pool_entry_id = format!("task_pool:setting:{complexity}");
        write_task_pool_preference_tx(transaction, &pool_entry_id, complexity, selection, true)?;
    }
    Ok(())
}

fn save_auxiliary_preferences(
    transaction: &Transaction<'_>,
    selection: &noema_providers::ProviderSelectionSnapshot,
) -> Result<(), StoreError> {
    for task_id in [
        "tool_progress_audit",
        "web_fetch_summarizer",
        "memory_extraction",
    ] {
        write_preference_tx(
            transaction,
            CanonicalPreferenceOwner::Auxiliary(task_id),
            selection,
            true,
        )?;
    }
    Ok(())
}
