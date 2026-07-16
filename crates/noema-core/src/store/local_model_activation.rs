//! Atomic activation of one installed local model across current workloads.

use rusqlite::{OptionalExtension, Transaction, params};

use noema_providers::{
    DefaultModelPreferenceRecord, LOCAL_MODELS_PROVIDER_ACCOUNT_ID, LocalModelEventKind,
    LocalModelInstallationStatus,
};

use super::{NoemaStore, StoreError, local_models::append_event};

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
    ) -> Result<DefaultModelPreferenceRecord, StoreError> {
        self.with_connection(|conn| {
            let transaction = conn.transaction()?;
            let (model_id, status): (String, String) = transaction
                .query_row(
                    "SELECT model_id, status FROM local_model_installations WHERE installation_id = ?1",
                    [installation_id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()?
                .ok_or_else(|| StoreError::LocalModelInstallationNotFound {
                    installation_id: installation_id.to_string(),
                })?;
            if status != LocalModelInstallationStatus::Installed.as_str() {
                return Err(StoreError::LocalModelActivationNotReady {
                    installation_id: installation_id.to_string(),
                    status,
                });
            }

            ensure_local_provider_account(&transaction)?;
            ensure_builtin_agents(&transaction)?;
            transaction.execute("UPDATE local_model_installations SET is_active = 0", [])?;
            transaction.execute(
                "UPDATE local_model_installations SET is_active = 1, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE installation_id = ?1",
                [installation_id],
            )?;
            save_default_preference(&transaction, &model_id)?;
            save_agent_preferences(&transaction, &model_id)?;
            save_task_pool_preferences(&transaction, &model_id)?;
            save_memory_preference(&transaction, &model_id)?;
            save_auxiliary_preferences(&transaction, &model_id)?;
            append_event(
                &transaction,
                installation_id,
                LocalModelEventKind::Activated,
                None,
                None,
                None,
            )?;
            let preference = default_model_preference(&transaction)?;
            transaction.commit()?;
            Ok(preference)
        })
        .await
    }
}

fn default_model_preference(
    transaction: &Transaction<'_>,
) -> Result<DefaultModelPreferenceRecord, StoreError> {
    transaction
        .query_row(
            r#"
            SELECT provider_kind, provider_account_id, model_profile,
                   reasoning_effort, updated_at
            FROM default_model_preference
            WHERE preference_id = 'default'
            "#,
            [],
            |row| {
                Ok(DefaultModelPreferenceRecord {
                    provider_kind: row.get(0)?,
                    provider_account_id: row.get(1)?,
                    model_profile: row.get(2)?,
                    reasoning_effort: row.get(3)?,
                    updated_at: row.get(4)?,
                })
            },
        )
        .map_err(StoreError::Sqlite)
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

fn save_default_preference(
    transaction: &Transaction<'_>,
    model_id: &str,
) -> Result<(), StoreError> {
    transaction.execute(
        r#"
        INSERT INTO default_model_preference (
          preference_id, provider_kind, provider_account_id, model_profile,
          reasoning_effort, updated_at
        ) VALUES ('default', 'local_models', ?1, ?2, NULL, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
        ON CONFLICT(preference_id) DO UPDATE SET
          provider_kind = excluded.provider_kind,
          provider_account_id = excluded.provider_account_id,
          model_profile = excluded.model_profile,
          reasoning_effort = NULL,
          updated_at = excluded.updated_at
        "#,
        params![LOCAL_MODELS_PROVIDER_ACCOUNT_ID, model_id],
    )?;
    Ok(())
}

fn save_agent_preferences(transaction: &Transaction<'_>, model_id: &str) -> Result<(), StoreError> {
    for agent_id in [
        "agent:primary",
        "agent:task-executor",
        "agent:task-reviewer",
    ] {
        transaction.execute(
            r#"
            INSERT INTO agent_runtime_preferences (
              agent_id, provider_kind, provider_account_id, model_profile,
              reasoning_effort, updated_at
            ) VALUES (?1, 'local_models', ?2, ?3, NULL, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
            ON CONFLICT(agent_id) DO UPDATE SET
              provider_kind = excluded.provider_kind,
              provider_account_id = excluded.provider_account_id,
              model_profile = excluded.model_profile,
              reasoning_effort = NULL,
              updated_at = excluded.updated_at
            "#,
            params![agent_id, LOCAL_MODELS_PROVIDER_ACCOUNT_ID, model_id],
        )?;
    }
    Ok(())
}

fn save_task_pool_preferences(
    transaction: &Transaction<'_>,
    model_id: &str,
) -> Result<(), StoreError> {
    for complexity in ["simple", "medium", "difficult"] {
        let pool_entry_id = format!("task_pool:setting:{complexity}");
        transaction.execute(
            r#"
            INSERT INTO task_model_pool_entries (
              pool_entry_id, complexity, label, provider_kind,
              provider_account_id, model_profile, reasoning_effort, enabled,
              sort_order, updated_at
            ) VALUES (?1, ?2, NULL, 'local_models', ?3, ?4, NULL, 1, 0,
                      strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
            ON CONFLICT(pool_entry_id) DO UPDATE SET
              provider_kind = excluded.provider_kind,
              provider_account_id = excluded.provider_account_id,
              model_profile = excluded.model_profile,
              reasoning_effort = NULL,
              enabled = 1,
              sort_order = 0,
              updated_at = excluded.updated_at
            "#,
            params![
                pool_entry_id,
                complexity,
                LOCAL_MODELS_PROVIDER_ACCOUNT_ID,
                model_id
            ],
        )?;
    }
    Ok(())
}

fn save_memory_preference(transaction: &Transaction<'_>, model_id: &str) -> Result<(), StoreError> {
    transaction.execute(
        r#"
        UPDATE memory_service_settings
        SET provider_kind = 'local_models',
            provider_account_id = ?1,
            model_profile = ?2,
            reasoning_effort = NULL,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE settings_id = 'default'
        "#,
        params![LOCAL_MODELS_PROVIDER_ACCOUNT_ID, model_id],
    )?;
    Ok(())
}

fn save_auxiliary_preferences(
    transaction: &Transaction<'_>,
    model_id: &str,
) -> Result<(), StoreError> {
    for task_id in ["tool_progress_audit", "web_fetch_summarizer"] {
        transaction.execute(
            r#"
            INSERT INTO auxiliary_model_preferences (
              task_id, provider_kind, provider_account_id, model_profile,
              reasoning_effort, updated_at
            ) VALUES (?1, 'local_models', ?2, ?3, NULL, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
            ON CONFLICT(task_id) DO UPDATE SET
              provider_kind = excluded.provider_kind,
              provider_account_id = excluded.provider_account_id,
              model_profile = excluded.model_profile,
              reasoning_effort = NULL,
              updated_at = excluded.updated_at
            "#,
            params![task_id, LOCAL_MODELS_PROVIDER_ACCOUNT_ID, model_id],
        )?;
    }
    Ok(())
}
