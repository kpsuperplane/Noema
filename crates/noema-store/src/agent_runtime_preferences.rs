use rusqlite::OptionalExtension;

use noema_providers::{
    ModelPreferenceSelection, ProviderInstanceKey, ProviderReadySelection,
    ProviderSelectionSnapshot,
};

use super::{
    NoemaStore, StoreError,
    provider_selections::{
        CanonicalPreferenceOwner, resolve_new_canonical_selection_tx, write_preference_tx,
    },
    sqlite::{model_preference_selection_column, parse_column},
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
    /// Whether Noema or the human chooses the concrete model.
    pub selection: ModelPreferenceSelection,
    /// Whether this preference requests faster service.
    pub fast_mode: bool,
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
    /// Whether Noema or the human chooses the concrete model.
    pub selection: ModelPreferenceSelection,
    /// Whether this preference requests faster service.
    pub fast_mode: bool,
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
                       provider_instance_key, selection_mode, model_profile, reasoning_effort,
                       fast_mode
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
        let mut explicit = match &preference.selection {
            ModelPreferenceSelection::ExplicitProfile {
                model_profile,
                reasoning_effort,
            } => ProviderSelectionSnapshot::explicit(
                &preference.provider_kind,
                &preference.provider_account_id,
                model_profile,
                *reasoning_effort,
                Some(format!("agent_runtime_preference:{}", preference.agent_id)),
            ),
            ModelPreferenceSelection::NoemaRecommended => ready_selection.selection().clone(),
        };
        explicit.fast_mode = preference.fast_mode;
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
                resolve_new_canonical_selection_tx(transaction, &explicit, Some(ready_selection))?;
            write_preference_tx(
                transaction,
                CanonicalPreferenceOwner::Agent(&preference.agent_id),
                &selection,
                &preference.selection,
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
                selection: preference.selection.clone(),
                fast_mode: preference.fast_mode,
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
        selection: model_preference_selection_column(row, 4, 5, 6)?,
        fast_mode: row.get::<_, i64>(7)? != 0,
    })
}
