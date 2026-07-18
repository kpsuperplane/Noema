use rusqlite::OptionalExtension;

use noema_providers::{
    ProviderInstanceKey, ProviderReadySelection, ProviderSelectionSnapshot, ReasoningEffort,
};

use super::{
    NoemaStore, StoreError,
    provider_selections::{
        CanonicalPreferenceOwner, resolve_new_canonical_selection_tx, write_preference_tx,
    },
    sqlite::{parse_column, reasoning_column},
};

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
                resolve_new_canonical_selection_tx(transaction, &selection, Some(ready_selection))?;
            write_preference_tx(
                transaction,
                CanonicalPreferenceOwner::Agent(&preference.agent_id),
                &selection,
                true,
            )?;
            Ok(AgentRuntimePreferenceRecord {
                agent_id: preference.agent_id.clone(),
                provider_kind: selection.provider_kind.clone(),
                provider_account_id: selection.provider_account_id.clone(),
                provider_instance_key: selection
                    .provider_instance_key
                    .clone()
                    .ok_or(StoreError::ProviderInstanceKeyMissing)?,
                model_profile: selection.model_profile.clone().ok_or_else(|| {
                    StoreError::InvariantViolation {
                        message: "agent runtime preference lost its model profile".to_string(),
                    }
                })?,
                reasoning_effort: selection.reasoning_effort,
            })
        })
        .await
    }
}

fn preference_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<AgentRuntimePreferenceRecord> {
    Ok(AgentRuntimePreferenceRecord {
        agent_id: row.get(0)?,
        provider_kind: row.get(1)?,
        provider_account_id: row.get(2)?,
        provider_instance_key: parse_column(row, 3)?,
        model_profile: row.get(4)?,
        reasoning_effort: reasoning_column(row, 5)?,
    })
}
