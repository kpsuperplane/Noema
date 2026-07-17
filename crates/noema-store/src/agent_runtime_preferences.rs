use rusqlite::{OptionalExtension, Transaction, params};

use noema_providers::{
    ProviderInstanceKey, ProviderReadySelection, ProviderSelectionSnapshot, ReasoningEffort,
};

use super::{NoemaStore, StoreError, provider_selections::resolve_new_canonical_selection_tx};

/// New or updated agent runtime preference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewAgentRuntimePreference {
    /// Durable agent id.
    pub agent_id: String,
    /// Provider kind selected for this agent.
    pub provider_kind: String,
    /// Provider account id selected for this agent.
    pub provider_account_id: String,
    /// Provider-specific model id or profile id.
    pub model_profile: String,
    /// Optional explicit reasoning effort for reasoning-capable model profiles.
    pub reasoning_effort: Option<ReasoningEffort>,
}

/// Persisted agent runtime preference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentRuntimePreferenceRecord {
    /// Durable agent id.
    pub agent_id: String,
    /// Provider kind selected for this agent.
    pub provider_kind: String,
    /// Provider account id selected for this agent.
    pub provider_account_id: String,
    /// Exact provider process selected for this agent.
    pub provider_instance_key: ProviderInstanceKey,
    /// Provider-specific model id or profile id.
    pub model_profile: String,
    /// Optional explicit reasoning effort for reasoning-capable model profiles.
    pub reasoning_effort: Option<ReasoningEffort>,
}

impl NoemaStore {
    /// Return the runtime preference for one agent.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the embedded store read fails.
    pub async fn get_agent_runtime_preference(
        &self,
        agent_id: &str,
    ) -> Result<Option<AgentRuntimePreferenceRecord>, StoreError> {
        self.with_connection(|conn| {
            conn.query_row(
                r#"
                SELECT agent_id, provider_kind, provider_account_id,
                       provider_instance_key, model_profile, reasoning_effort
                FROM agent_runtime_preferences
                WHERE agent_id = ?1
                LIMIT 1
                "#,
                [agent_id],
                preference_from_row,
            )
            .optional()
            .map_err(StoreError::Sqlite)
        })
        .await
    }

    /// Create or update one agent runtime preference.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the agent or provider account is missing, the
    /// model/profile is blank, or the embedded store write/read fails.
    pub async fn upsert_agent_runtime_preference(
        &self,
        preference: NewAgentRuntimePreference,
    ) -> Result<AgentRuntimePreferenceRecord, StoreError> {
        self.upsert_agent_runtime_preference_inner(preference, None)
            .await
    }

    /// Create or update one agent runtime preference using an opaque proof that
    /// its exact provider instance is ready. All new selections must use this
    /// API so the registry lease remains held through commit.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the agent or provider selection is invalid,
    /// the proof does not cover the exact route, or SQLite fails.
    pub async fn upsert_agent_runtime_preference_with_ready_selection(
        &self,
        preference: NewAgentRuntimePreference,
        ready_selection: &ProviderReadySelection,
    ) -> Result<AgentRuntimePreferenceRecord, StoreError> {
        self.upsert_agent_runtime_preference_inner(preference, Some(ready_selection))
            .await
    }

    async fn upsert_agent_runtime_preference_inner(
        &self,
        preference: NewAgentRuntimePreference,
        ready_selection: Option<&ProviderReadySelection>,
    ) -> Result<AgentRuntimePreferenceRecord, StoreError> {
        let selection = ProviderSelectionSnapshot::explicit(
            &preference.provider_kind,
            &preference.provider_account_id,
            &preference.model_profile,
            preference.reasoning_effort,
            Some(format!("agent_runtime_preference:{}", preference.agent_id)),
        );
        self.with_immediate_transaction_retry(|transaction| {
            let exists = transaction
                .query_row(
                    "SELECT 1 FROM agents WHERE agent_id = ?1 LIMIT 1",
                    [&preference.agent_id],
                    |_| Ok(()),
                )
                .optional()?
                .is_some();
            if !exists {
                return Err(StoreError::AgentNotFound {
                    agent_id: preference.agent_id.clone(),
                });
            }
            let selection =
                resolve_new_canonical_selection_tx(transaction, &selection, ready_selection)?;
            let key = selection
                .provider_instance_key
                .as_ref()
                .ok_or(StoreError::ProviderInstanceKeyMissing)?;
            transaction.execute(
                r#"
                INSERT INTO agent_runtime_preferences
                  (agent_id, provider_kind, provider_account_id, provider_instance_key,
                   model_profile, reasoning_effort, updated_at)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
                ON CONFLICT(agent_id) DO UPDATE SET
                  provider_kind = excluded.provider_kind,
                  provider_account_id = excluded.provider_account_id,
                  provider_instance_key = excluded.provider_instance_key,
                  model_profile = excluded.model_profile,
                  reasoning_effort = excluded.reasoning_effort,
                  updated_at = excluded.updated_at
                "#,
                params![
                    preference.agent_id,
                    selection.provider_kind,
                    selection.provider_account_id,
                    key.as_str(),
                    selection.model_profile,
                    preference
                        .reasoning_effort
                        .map(ReasoningEffort::as_persistence_str),
                ],
            )?;
            preference_in_transaction(transaction, &preference.agent_id)
        })
        .await
    }
}

fn preference_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<AgentRuntimePreferenceRecord> {
    let provider_instance_key =
        ProviderInstanceKey::new(row.get::<_, String>(3)?).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                3,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })?;
    let reasoning_effort: Option<String> = row.get(5)?;
    Ok(AgentRuntimePreferenceRecord {
        agent_id: row.get(0)?,
        provider_kind: row.get(1)?,
        provider_account_id: row.get(2)?,
        provider_instance_key,
        model_profile: row.get(4)?,
        reasoning_effort: reasoning_effort
            .as_deref()
            .and_then(ReasoningEffort::from_persistence_str),
    })
}

fn preference_in_transaction(
    transaction: &Transaction<'_>,
    agent_id: &str,
) -> Result<AgentRuntimePreferenceRecord, StoreError> {
    transaction
        .query_row(
            r#"
            SELECT agent_id, provider_kind, provider_account_id,
                   provider_instance_key, model_profile, reasoning_effort
            FROM agent_runtime_preferences
            WHERE agent_id = ?1
            LIMIT 1
            "#,
            [agent_id],
            preference_from_row,
        )
        .map_err(StoreError::Sqlite)
}
