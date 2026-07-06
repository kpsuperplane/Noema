use serde::Deserialize;
use surrealdb::types::SurrealValue;

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
        let mut response = self
            .db
            .query(
                r#"
                SELECT task_id, provider_kind, provider_account_id, model_profile
                FROM auxiliary_model_preferences
                WHERE task_id = $task_id
                LIMIT 1;
                "#,
            )
            .bind(("task_id", task_id.to_string()))
            .await?;
        let rows: Vec<AuxiliaryModelPreferenceRow> = response.take(0)?;
        Ok(rows.into_iter().next().map(preference_from_row))
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

        self.db
            .query(
                r#"
                UPSERT type::record('auxiliary_model_preferences', $record_id) SET
                  task_id = $task_id,
                  provider_kind = $provider_kind,
                  provider_account_id = $provider_account_id,
                  model_profile = $model_profile,
                  updated_at = time::now();
                "#,
            )
            .bind(("record_id", task_record_fragment(&preference.task_id)))
            .bind(("task_id", preference.task_id.clone()))
            .bind(("provider_kind", account.provider_kind))
            .bind(("provider_account_id", account.provider_account_id))
            .bind(("model_profile", model_profile.to_string()))
            .await?
            .check()?;
        self.get_auxiliary_model_preference(&preference.task_id)
            .await?
            .ok_or(StoreError::InvalidEnum {
                kind: "auxiliary_model_preference_task_id",
                value: preference.task_id,
            })
    }
}

#[derive(Debug, Deserialize, SurrealValue)]
struct AuxiliaryModelPreferenceRow {
    task_id: String,
    provider_kind: String,
    provider_account_id: String,
    model_profile: String,
}

fn preference_from_row(row: AuxiliaryModelPreferenceRow) -> AuxiliaryModelPreferenceRecord {
    AuxiliaryModelPreferenceRecord {
        task_id: row.task_id,
        provider_kind: row.provider_kind,
        provider_account_id: row.provider_account_id,
        model_profile: row.model_profile,
    }
}

fn task_record_fragment(task_id: &str) -> String {
    task_id
        .chars()
        .map(|character| match character {
            'a'..='z' | 'A'..='Z' | '0'..='9' => character,
            _ => '_',
        })
        .collect()
}
