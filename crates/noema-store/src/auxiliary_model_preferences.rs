use rusqlite::{OptionalExtension, Transaction, params};

use noema_providers::{
    ProviderInstanceKey, ProviderReadySelection, ProviderSelectionSnapshot, ReasoningEffort,
};

use super::{NoemaStore, StoreError, provider_selections::resolve_new_canonical_selection_tx};

/// Auxiliary model preference task id for `web.fetch` summarization.
pub const WEB_FETCH_SUMMARIZER_TASK_ID: &str = "web_fetch_summarizer";

/// Auxiliary model preference task id for provider tool-continuation progress audits.
pub const TOOL_PROGRESS_AUDIT_TASK_ID: &str = "tool_progress_audit";

fn supported_auxiliary_model_task_id(task_id: &str) -> bool {
    matches!(
        task_id,
        WEB_FETCH_SUMMARIZER_TASK_ID | TOOL_PROGRESS_AUDIT_TASK_ID
    )
}

/// New or updated auxiliary model preference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewAuxiliaryModelPreference {
    /// Durable auxiliary task id.
    pub task_id: String,
    /// Provider kind selected for this auxiliary task.
    pub provider_kind: String,
    /// Provider account id selected for this auxiliary task.
    pub provider_account_id: String,
    /// Provider-specific model id or profile id.
    pub model_profile: String,
    /// Optional explicit reasoning effort for reasoning-capable model profiles.
    pub reasoning_effort: Option<ReasoningEffort>,
}

/// Persisted auxiliary model preference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuxiliaryModelPreferenceRecord {
    /// Durable auxiliary task id.
    pub task_id: String,
    /// Provider kind selected for this auxiliary task.
    pub provider_kind: String,
    /// Provider account id selected for this auxiliary task.
    pub provider_account_id: String,
    /// Exact provider process selected for this auxiliary task.
    pub provider_instance_key: ProviderInstanceKey,
    /// Provider-specific model id or profile id.
    pub model_profile: String,
    /// Optional explicit reasoning effort for reasoning-capable model profiles.
    pub reasoning_effort: Option<ReasoningEffort>,
}

impl NoemaStore {
    /// Return one auxiliary model preference by task id.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the embedded store read fails.
    pub async fn get_auxiliary_model_preference(
        &self,
        task_id: &str,
    ) -> Result<Option<AuxiliaryModelPreferenceRecord>, StoreError> {
        self.with_connection(|conn| {
            conn.query_row(
                r#"
                SELECT task_id, provider_kind, provider_account_id,
                       provider_instance_key, model_profile, reasoning_effort
                FROM auxiliary_model_preferences
                WHERE task_id = ?1
                LIMIT 1
                "#,
                [task_id],
                preference_from_row,
            )
            .optional()
            .map_err(StoreError::Sqlite)
        })
        .await
    }

    /// Create or update one auxiliary model preference.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the task id is unsupported, provider account
    /// is missing or mismatched, model/profile is blank, or the embedded store
    /// write/read fails.
    pub async fn upsert_auxiliary_model_preference(
        &self,
        preference: NewAuxiliaryModelPreference,
    ) -> Result<AuxiliaryModelPreferenceRecord, StoreError> {
        self.upsert_auxiliary_model_preference_inner(preference, None)
            .await
    }

    /// Create or update one auxiliary preference while retaining a registry
    /// readiness lease through commit. All new selections must use this API.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the task or provider selection is invalid,
    /// the proof does not cover the exact route, or SQLite fails.
    pub async fn upsert_auxiliary_model_preference_with_ready_selection(
        &self,
        preference: NewAuxiliaryModelPreference,
        ready_selection: &ProviderReadySelection,
    ) -> Result<AuxiliaryModelPreferenceRecord, StoreError> {
        self.upsert_auxiliary_model_preference_inner(preference, Some(ready_selection))
            .await
    }

    async fn upsert_auxiliary_model_preference_inner(
        &self,
        preference: NewAuxiliaryModelPreference,
        ready_selection: Option<&ProviderReadySelection>,
    ) -> Result<AuxiliaryModelPreferenceRecord, StoreError> {
        if !supported_auxiliary_model_task_id(&preference.task_id) {
            return Err(StoreError::InvalidEnum {
                kind: "auxiliary_model_preference_task_id",
                value: preference.task_id,
            });
        }
        let selection = ProviderSelectionSnapshot::explicit(
            &preference.provider_kind,
            &preference.provider_account_id,
            &preference.model_profile,
            preference.reasoning_effort,
            Some(format!("auxiliary_model_preference:{}", preference.task_id)),
        );
        self.with_immediate_transaction_retry(|transaction| {
            let selection =
                resolve_new_canonical_selection_tx(transaction, &selection, ready_selection)?;
            let key = selection
                .provider_instance_key
                .as_ref()
                .ok_or(StoreError::ProviderInstanceKeyMissing)?;
            transaction.execute(
                r#"
                INSERT INTO auxiliary_model_preferences
                  (task_id, provider_kind, provider_account_id, provider_instance_key,
                   model_profile, reasoning_effort, updated_at)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
                ON CONFLICT(task_id) DO UPDATE SET
                  provider_kind = excluded.provider_kind,
                  provider_account_id = excluded.provider_account_id,
                  provider_instance_key = excluded.provider_instance_key,
                  model_profile = excluded.model_profile,
                  reasoning_effort = excluded.reasoning_effort,
                  updated_at = excluded.updated_at
                "#,
                params![
                    preference.task_id,
                    selection.provider_kind,
                    selection.provider_account_id,
                    key.as_str(),
                    selection.model_profile,
                    preference
                        .reasoning_effort
                        .map(ReasoningEffort::as_persistence_str),
                ],
            )?;
            preference_in_transaction(transaction, &preference.task_id)
        })
        .await
    }
}

fn preference_from_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<AuxiliaryModelPreferenceRecord> {
    let provider_instance_key =
        ProviderInstanceKey::new(row.get::<_, String>(3)?).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                3,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })?;
    let reasoning_effort: Option<String> = row.get(5)?;
    Ok(AuxiliaryModelPreferenceRecord {
        task_id: row.get(0)?,
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
    task_id: &str,
) -> Result<AuxiliaryModelPreferenceRecord, StoreError> {
    transaction
        .query_row(
            r#"
            SELECT task_id, provider_kind, provider_account_id,
                   provider_instance_key, model_profile, reasoning_effort
            FROM auxiliary_model_preferences
            WHERE task_id = ?1
            LIMIT 1
            "#,
            [task_id],
            preference_from_row,
        )
        .map_err(StoreError::Sqlite)
}
