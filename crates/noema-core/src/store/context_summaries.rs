use serde::Deserialize;
use surrealdb::types::SurrealValue;

use crate::ConversationContextSummaryStatus;

use super::{
    NoemaStore, StoreError,
    ids::{allocate_id, record_fragment},
};

/// Input for creating a derived conversation context summary checkpoint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewConversationContextSummary {
    /// Conversation that owns the derived summary.
    pub conversation_id: String,
    /// Runtime provider this summary profile targets.
    pub provider_kind: String,
    /// Provider model/profile this summary profile targets.
    pub model_profile: Option<String>,
    /// Model-visible compacted context text.
    pub summary_text: String,
    /// First transcript item sequence covered by this summary.
    pub covered_item_start_sequence: i64,
    /// Last transcript item sequence covered by this summary.
    pub covered_item_end_sequence: i64,
    /// Covered source item ids, or a bounded sample for large ranges.
    pub source_item_ids: Vec<String>,
    /// Estimated tokens in the compaction input.
    pub input_token_estimate: u64,
    /// Estimated tokens in the summary output.
    pub summary_token_estimate: u64,
    /// Provider used to generate the summary.
    pub compaction_provider_kind: String,
    /// Model/profile used to generate the summary.
    pub compaction_model_profile: Option<String>,
    /// Summary lifecycle status.
    pub status: ConversationContextSummaryStatus,
    /// Stable failure code, when status is failed.
    pub error_code: Option<String>,
    /// Safe failure message, when status is failed.
    pub error_message: Option<String>,
}

/// Persisted derived conversation context summary checkpoint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConversationContextSummaryRecord {
    /// Durable summary id.
    pub summary_id: String,
    /// Conversation that owns the derived summary.
    pub conversation_id: String,
    /// Runtime provider this summary profile targets.
    pub provider_kind: String,
    /// Provider model/profile this summary profile targets.
    pub model_profile: Option<String>,
    /// Model-visible compacted context text.
    pub summary_text: String,
    /// First transcript item sequence covered by this summary.
    pub covered_item_start_sequence: i64,
    /// Last transcript item sequence covered by this summary.
    pub covered_item_end_sequence: i64,
    /// Covered source item ids, or a bounded sample for large ranges.
    pub source_item_ids: Vec<String>,
    /// Estimated tokens in the compaction input.
    pub input_token_estimate: u64,
    /// Estimated tokens in the summary output.
    pub summary_token_estimate: u64,
    /// Provider used to generate the summary.
    pub compaction_provider_kind: String,
    /// Model/profile used to generate the summary.
    pub compaction_model_profile: Option<String>,
    /// Summary lifecycle status.
    pub status: ConversationContextSummaryStatus,
    /// Stable failure code, when status is failed.
    pub error_code: Option<String>,
    /// Safe failure message, when status is failed.
    pub error_message: Option<String>,
}

impl NoemaStore {
    /// Insert a derived conversation context summary checkpoint.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the conversation is missing or the embedded
    /// store write/read fails.
    pub async fn insert_conversation_context_summary(
        &self,
        summary: NewConversationContextSummary,
    ) -> Result<ConversationContextSummaryRecord, StoreError> {
        self.require_conversation(&summary.conversation_id).await?;
        if summary.status == ConversationContextSummaryStatus::Active {
            self.supersede_active_context_summaries(
                &summary.conversation_id,
                &summary.provider_kind,
                summary.model_profile.as_deref(),
            )
            .await?;
        }

        let summary_id = allocate_id("context-summary");
        self.db
            .query(
                r#"
                CREATE type::record('conversation_context_summaries', $record_id) SET
                  summary_id = $summary_id,
                  conversation_id = $conversation_id,
                  provider_kind = $provider_kind,
                  model_profile = $model_profile,
                  summary_text = $summary_text,
                  covered_item_start_sequence = $covered_item_start_sequence,
                  covered_item_end_sequence = $covered_item_end_sequence,
                  source_item_ids = $source_item_ids,
                  input_token_estimate = $input_token_estimate,
                  summary_token_estimate = $summary_token_estimate,
                  compaction_provider_kind = $compaction_provider_kind,
                  compaction_model_profile = $compaction_model_profile,
                  status = $status,
                  error_code = $error_code,
                  error_message = $error_message,
                  updated_at = time::now();
                "#,
            )
            .bind(("record_id", record_fragment(&summary_id)))
            .bind(("summary_id", summary_id.clone()))
            .bind(("conversation_id", summary.conversation_id.clone()))
            .bind(("provider_kind", summary.provider_kind.clone()))
            .bind(("model_profile", summary.model_profile.clone()))
            .bind(("summary_text", summary.summary_text.clone()))
            .bind((
                "covered_item_start_sequence",
                summary.covered_item_start_sequence,
            ))
            .bind((
                "covered_item_end_sequence",
                summary.covered_item_end_sequence,
            ))
            .bind(("source_item_ids", summary.source_item_ids.clone()))
            .bind(("input_token_estimate", summary.input_token_estimate as i64))
            .bind((
                "summary_token_estimate",
                summary.summary_token_estimate as i64,
            ))
            .bind((
                "compaction_provider_kind",
                summary.compaction_provider_kind.clone(),
            ))
            .bind((
                "compaction_model_profile",
                summary.compaction_model_profile.clone(),
            ))
            .bind(("status", summary.status.as_str().to_string()))
            .bind(("error_code", summary.error_code.clone()))
            .bind(("error_message", summary.error_message.clone()))
            .await?
            .check()?;

        Ok(ConversationContextSummaryRecord {
            summary_id,
            conversation_id: summary.conversation_id,
            provider_kind: summary.provider_kind,
            model_profile: summary.model_profile,
            summary_text: summary.summary_text,
            covered_item_start_sequence: summary.covered_item_start_sequence,
            covered_item_end_sequence: summary.covered_item_end_sequence,
            source_item_ids: summary.source_item_ids,
            input_token_estimate: summary.input_token_estimate,
            summary_token_estimate: summary.summary_token_estimate,
            compaction_provider_kind: summary.compaction_provider_kind,
            compaction_model_profile: summary.compaction_model_profile,
            status: summary.status,
            error_code: summary.error_code,
            error_message: summary.error_message,
        })
    }

    /// Return the latest active context summary for a conversation context profile.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the conversation is missing, the embedded
    /// store read fails, or stored enums are invalid.
    pub async fn latest_active_context_summary(
        &self,
        conversation_id: &str,
        provider_kind: &str,
        model_profile: Option<&str>,
    ) -> Result<Option<ConversationContextSummaryRecord>, StoreError> {
        self.require_conversation(conversation_id).await?;
        let mut response = self
            .db
            .query(format!(
                r#"
                SELECT {CONTEXT_SUMMARY_SELECT}
                FROM conversation_context_summaries
                WHERE conversation_id = $conversation_id
                  AND provider_kind = $provider_kind
                  AND model_profile = $model_profile
                  AND status = 'active'
                ORDER BY covered_item_end_sequence DESC
                LIMIT 1;
                "#
            ))
            .bind(("conversation_id", conversation_id.to_string()))
            .bind(("provider_kind", provider_kind.to_string()))
            .bind(("model_profile", model_profile.map(str::to_string)))
            .await?;
        let rows: Vec<ContextSummaryRow> = response.take(0)?;
        rows.into_iter()
            .next()
            .map(context_summary_from_row)
            .transpose()
    }

    /// Return one context summary by id.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the embedded store read fails or stored
    /// enums are invalid.
    pub async fn get_conversation_context_summary(
        &self,
        summary_id: &str,
    ) -> Result<Option<ConversationContextSummaryRecord>, StoreError> {
        let mut response = self
            .db
            .query(format!(
                r#"
                SELECT {CONTEXT_SUMMARY_SELECT}
                FROM conversation_context_summaries
                WHERE summary_id = $summary_id
                LIMIT 1;
                "#
            ))
            .bind(("summary_id", summary_id.to_string()))
            .await?;
        let rows: Vec<ContextSummaryRow> = response.take(0)?;
        rows.into_iter()
            .next()
            .map(context_summary_from_row)
            .transpose()
    }

    /// List all context summaries for a conversation in creation order.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the conversation is missing, the embedded
    /// store read fails, or stored enums are invalid.
    pub async fn list_context_summaries_for_conversation(
        &self,
        conversation_id: &str,
    ) -> Result<Vec<ConversationContextSummaryRecord>, StoreError> {
        self.require_conversation(conversation_id).await?;
        let mut response = self
            .db
            .query(format!(
                r#"
                SELECT {CONTEXT_SUMMARY_SELECT}
                FROM conversation_context_summaries
                WHERE conversation_id = $conversation_id
                ORDER BY created_at ASC;
                "#
            ))
            .bind(("conversation_id", conversation_id.to_string()))
            .await?;
        let rows: Vec<ContextSummaryRow> = response.take(0)?;
        rows.into_iter().map(context_summary_from_row).collect()
    }

    async fn supersede_active_context_summaries(
        &self,
        conversation_id: &str,
        provider_kind: &str,
        model_profile: Option<&str>,
    ) -> Result<(), StoreError> {
        self.db
            .query(
                r#"
                UPDATE conversation_context_summaries SET
                  status = 'superseded',
                  updated_at = time::now()
                WHERE conversation_id = $conversation_id
                  AND provider_kind = $provider_kind
                  AND model_profile = $model_profile
                  AND status = 'active';
                "#,
            )
            .bind(("conversation_id", conversation_id.to_string()))
            .bind(("provider_kind", provider_kind.to_string()))
            .bind(("model_profile", model_profile.map(str::to_string)))
            .await?
            .check()?;
        Ok(())
    }
}

const CONTEXT_SUMMARY_SELECT: &str = r#"
summary_id, conversation_id, provider_kind, model_profile, summary_text,
covered_item_start_sequence, covered_item_end_sequence, source_item_ids,
input_token_estimate, summary_token_estimate,
compaction_provider_kind, compaction_model_profile,
status, error_code, error_message
"#;

#[derive(Debug, Deserialize, SurrealValue)]
struct ContextSummaryRow {
    summary_id: String,
    conversation_id: String,
    provider_kind: String,
    model_profile: Option<String>,
    summary_text: String,
    covered_item_start_sequence: i64,
    covered_item_end_sequence: i64,
    source_item_ids: Vec<String>,
    input_token_estimate: i64,
    summary_token_estimate: i64,
    compaction_provider_kind: String,
    compaction_model_profile: Option<String>,
    status: String,
    error_code: Option<String>,
    error_message: Option<String>,
}

fn context_summary_from_row(
    row: ContextSummaryRow,
) -> Result<ConversationContextSummaryRecord, StoreError> {
    Ok(ConversationContextSummaryRecord {
        summary_id: row.summary_id,
        conversation_id: row.conversation_id,
        provider_kind: row.provider_kind,
        model_profile: row.model_profile,
        summary_text: row.summary_text,
        covered_item_start_sequence: row.covered_item_start_sequence,
        covered_item_end_sequence: row.covered_item_end_sequence,
        source_item_ids: row.source_item_ids,
        input_token_estimate: row.input_token_estimate.max(0) as u64,
        summary_token_estimate: row.summary_token_estimate.max(0) as u64,
        compaction_provider_kind: row.compaction_provider_kind,
        compaction_model_profile: row.compaction_model_profile,
        status: ConversationContextSummaryStatus::parse(&row.status)
            .map_err(|error| StoreError::Schema(error.to_string()))?,
        error_code: row.error_code,
        error_message: row.error_message,
    })
}
