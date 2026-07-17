use rusqlite::{OptionalExtension, params};

use noema_providers::ReasoningEffort;

use super::{NoemaStore, StoreError};

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
                SELECT agent_id, provider_kind, provider_account_id, model_profile, reasoning_effort
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
        self.require_agent(&preference.agent_id).await?;
        let Some(account) = self
            .get_provider_account(&preference.provider_account_id)
            .await?
        else {
            return Err(StoreError::ProviderAccountNotFound {
                provider_account_id: preference.provider_account_id,
            });
        };
        if account.provider_kind != preference.provider_kind {
            return Err(StoreError::InvalidEnum {
                kind: "agent_runtime_provider_kind",
                value: preference.provider_kind,
            });
        }
        let model_profile = preference.model_profile.trim();
        if model_profile.is_empty() {
            return Err(StoreError::InvalidEnum {
                kind: "agent_runtime_model_profile",
                value: preference.model_profile,
            });
        }

        self.with_connection(|conn| {
            conn.execute(
                r#"
                INSERT INTO agent_runtime_preferences
                  (agent_id, provider_kind, provider_account_id, model_profile, reasoning_effort, updated_at)
                VALUES (?1, ?2, ?3, ?4, ?5, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
                ON CONFLICT(agent_id) DO UPDATE SET
                  provider_kind = excluded.provider_kind,
                  provider_account_id = excluded.provider_account_id,
                  model_profile = excluded.model_profile,
                  reasoning_effort = excluded.reasoning_effort,
                  updated_at = excluded.updated_at
                "#,
                params![
                    preference.agent_id,
                    account.provider_kind,
                    account.provider_account_id,
                    model_profile,
                    preference.reasoning_effort.map(ReasoningEffort::as_persistence_str),
                ],
            )?;
            Ok(())
        })
        .await?;
        self.get_agent_runtime_preference(&preference.agent_id)
            .await?
            .ok_or(StoreError::AgentNotFound {
                agent_id: preference.agent_id,
            })
    }
}

fn preference_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<AgentRuntimePreferenceRecord> {
    let reasoning_effort: Option<String> = row.get(4)?;
    Ok(AgentRuntimePreferenceRecord {
        agent_id: row.get(0)?,
        provider_kind: row.get(1)?,
        provider_account_id: row.get(2)?,
        model_profile: row.get(3)?,
        reasoning_effort: reasoning_effort
            .as_deref()
            .and_then(ReasoningEffort::from_persistence_str),
    })
}
