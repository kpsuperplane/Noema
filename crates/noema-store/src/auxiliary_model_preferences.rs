use rusqlite::{OptionalExtension, params};

use noema_providers::ReasoningEffort;

use super::{NoemaStore, StoreError};

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
                SELECT task_id, provider_kind, provider_account_id, model_profile, reasoning_effort
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
        if !supported_auxiliary_model_task_id(&preference.task_id) {
            return Err(StoreError::InvalidEnum {
                kind: "auxiliary_model_preference_task_id",
                value: preference.task_id,
            });
        }
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
                kind: "auxiliary_model_preference_provider_kind",
                value: preference.provider_kind,
            });
        }
        let model_profile = preference.model_profile.trim();
        if model_profile.is_empty() {
            return Err(StoreError::InvalidEnum {
                kind: "auxiliary_model_preference_model_profile",
                value: preference.model_profile,
            });
        }

        self.with_connection(|conn| {
            conn.execute(
                r#"
                INSERT INTO auxiliary_model_preferences
                  (task_id, provider_kind, provider_account_id, model_profile, reasoning_effort, updated_at)
                VALUES (?1, ?2, ?3, ?4, ?5, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
                ON CONFLICT(task_id) DO UPDATE SET
                  provider_kind = excluded.provider_kind,
                  provider_account_id = excluded.provider_account_id,
                  model_profile = excluded.model_profile,
                  reasoning_effort = excluded.reasoning_effort,
                  updated_at = excluded.updated_at
                "#,
                params![
                    preference.task_id,
                    account.provider_kind,
                    account.provider_account_id,
                    model_profile,
                    preference.reasoning_effort.map(ReasoningEffort::as_persistence_str),
                ],
            )?;
            Ok(())
        })
        .await?;
        self.get_auxiliary_model_preference(&preference.task_id)
            .await?
            .ok_or(StoreError::InvalidEnum {
                kind: "auxiliary_model_preference_task_id",
                value: preference.task_id,
            })
    }
}

fn preference_from_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<AuxiliaryModelPreferenceRecord> {
    let reasoning_effort: Option<String> = row.get(4)?;
    Ok(AuxiliaryModelPreferenceRecord {
        task_id: row.get(0)?,
        provider_kind: row.get(1)?,
        provider_account_id: row.get(2)?,
        model_profile: row.get(3)?,
        reasoning_effort: reasoning_effort
            .as_deref()
            .and_then(ReasoningEffort::from_persistence_str),
    })
}
