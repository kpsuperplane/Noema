use serde::Deserialize;
use surrealdb::types::SurrealValue;

use super::{NoemaStore, StoreError, agents::agent_record_fragment};

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
        let mut response = self
            .db
            .query(
                r#"
                SELECT agent_id, provider_kind, provider_account_id, model_profile
                FROM agent_runtime_preferences
                WHERE agent_id = $agent_id
                LIMIT 1;
                "#,
            )
            .bind(("agent_id", agent_id.to_string()))
            .await?;
        let rows: Vec<AgentRuntimePreferenceRow> = response.take(0)?;
        Ok(rows.into_iter().next().map(preference_from_row))
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

        self.db
            .query(
                r#"
                UPSERT type::record('agent_runtime_preferences', $record_id) SET
                  agent_id = $agent_id,
                  provider_kind = $provider_kind,
                  provider_account_id = $provider_account_id,
                  model_profile = $model_profile,
                  updated_at = time::now();
                "#,
            )
            .bind(("record_id", agent_record_fragment(&preference.agent_id)))
            .bind(("agent_id", preference.agent_id.clone()))
            .bind(("provider_kind", account.provider_kind))
            .bind(("provider_account_id", account.provider_account_id))
            .bind(("model_profile", model_profile.to_string()))
            .await?
            .check()?;
        self.get_agent_runtime_preference(&preference.agent_id)
            .await?
            .ok_or(StoreError::AgentNotFound {
                agent_id: preference.agent_id,
            })
    }
}

#[derive(Debug, Deserialize, SurrealValue)]
struct AgentRuntimePreferenceRow {
    agent_id: String,
    provider_kind: String,
    provider_account_id: String,
    model_profile: String,
}

fn preference_from_row(row: AgentRuntimePreferenceRow) -> AgentRuntimePreferenceRecord {
    AgentRuntimePreferenceRecord {
        agent_id: row.agent_id,
        provider_kind: row.provider_kind,
        provider_account_id: row.provider_account_id,
        model_profile: row.model_profile,
    }
}
