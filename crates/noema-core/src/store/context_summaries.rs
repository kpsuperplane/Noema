use rusqlite::{OptionalExtension, params};

use crate::ConversationContextSummaryStatus;

use super::{
    NoemaStore, StoreError,
    ids::allocate_id,
    sqlite::{deserialize_json, serialize_json},
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
            if let Some(active) = self
                .latest_active_context_summary(
                    &summary.conversation_id,
                    &summary.provider_kind,
                    summary.model_profile.as_deref(),
                )
                .await?
                && active.covered_item_end_sequence > summary.covered_item_end_sequence
            {
                return Ok(active);
            }
            self.supersede_active_context_summaries(
                &summary.conversation_id,
                &summary.provider_kind,
                summary.model_profile.as_deref(),
            )
            .await?;
        }

        let summary_id = allocate_id("context-summary");
        let source_item_ids_json = serialize_json(&summary.source_item_ids)?;
        let input_token_estimate = estimate_to_i64(summary.input_token_estimate);
        let summary_token_estimate = estimate_to_i64(summary.summary_token_estimate);
        self.with_connection(|conn| {
            conn.execute(
                r#"
                INSERT INTO conversation_context_summaries (
                  summary_id, conversation_id, provider_kind, model_profile, summary_text,
                  covered_item_start_sequence, covered_item_end_sequence, source_item_ids_json,
                  input_token_estimate, summary_token_estimate, compaction_provider_kind,
                  compaction_model_profile, status, error_code, error_message
                )
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
                "#,
                params![
                    summary_id,
                    summary.conversation_id,
                    summary.provider_kind,
                    summary.model_profile,
                    summary.summary_text,
                    summary.covered_item_start_sequence,
                    summary.covered_item_end_sequence,
                    source_item_ids_json,
                    input_token_estimate,
                    summary_token_estimate,
                    summary.compaction_provider_kind,
                    summary.compaction_model_profile,
                    summary.status.as_str(),
                    summary.error_code,
                    summary.error_message,
                ],
            )?;
            Ok(())
        })
        .await?;

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
        let row = self
            .with_connection(|conn| {
                conn.query_row(
                    format!(
                        r#"
                        SELECT {CONTEXT_SUMMARY_SELECT}
                        FROM conversation_context_summaries
                        WHERE conversation_id = ?1
                          AND provider_kind = ?2
                          AND model_profile IS ?3
                          AND status = 'active'
                        ORDER BY covered_item_end_sequence DESC
                        LIMIT 1
                        "#
                    )
                    .as_str(),
                    params![conversation_id, provider_kind, model_profile],
                    context_summary_row,
                )
                .optional()
                .map_err(StoreError::Sqlite)
            })
            .await?;
        row.map(context_summary_from_row).transpose()
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
        let row = self
            .with_connection(|conn| {
                conn.query_row(
                    format!(
                        r#"
                        SELECT {CONTEXT_SUMMARY_SELECT}
                        FROM conversation_context_summaries
                        WHERE summary_id = ?1
                        LIMIT 1
                        "#
                    )
                    .as_str(),
                    [summary_id],
                    context_summary_row,
                )
                .optional()
                .map_err(StoreError::Sqlite)
            })
            .await?;
        row.map(context_summary_from_row).transpose()
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
        let rows = self
            .with_connection(|conn| {
                let mut statement = conn.prepare(
                    format!(
                        r#"
                        SELECT {CONTEXT_SUMMARY_SELECT}
                        FROM conversation_context_summaries
                        WHERE conversation_id = ?1
                        ORDER BY covered_item_end_sequence ASC
                        "#
                    )
                    .as_str(),
                )?;
                let rows = statement.query_map([conversation_id], context_summary_row)?;
                rows.collect::<Result<Vec<_>, _>>()
                    .map_err(StoreError::Sqlite)
            })
            .await?;
        rows.into_iter().map(context_summary_from_row).collect()
    }

    async fn supersede_active_context_summaries(
        &self,
        conversation_id: &str,
        provider_kind: &str,
        model_profile: Option<&str>,
    ) -> Result<(), StoreError> {
        self.with_connection(|conn| {
            conn.execute(
                r#"
                UPDATE conversation_context_summaries
                SET status = 'superseded',
                    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                WHERE conversation_id = ?1
                  AND provider_kind = ?2
                  AND model_profile IS ?3
                  AND status = 'active'
                "#,
                params![conversation_id, provider_kind, model_profile],
            )?;
            Ok(())
        })
        .await
    }
}

const CONTEXT_SUMMARY_SELECT: &str = r#"
summary_id, conversation_id, provider_kind, model_profile, summary_text,
covered_item_start_sequence, covered_item_end_sequence, source_item_ids_json,
input_token_estimate, summary_token_estimate,
compaction_provider_kind, compaction_model_profile,
status, error_code, error_message
"#;

#[derive(Debug)]
struct ContextSummaryRow {
    summary_id: String,
    conversation_id: String,
    provider_kind: String,
    model_profile: Option<String>,
    summary_text: String,
    covered_item_start_sequence: i64,
    covered_item_end_sequence: i64,
    source_item_ids_json: String,
    input_token_estimate: i64,
    summary_token_estimate: i64,
    compaction_provider_kind: String,
    compaction_model_profile: Option<String>,
    status: String,
    error_code: Option<String>,
    error_message: Option<String>,
}

fn context_summary_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ContextSummaryRow> {
    Ok(ContextSummaryRow {
        summary_id: row.get(0)?,
        conversation_id: row.get(1)?,
        provider_kind: row.get(2)?,
        model_profile: row.get(3)?,
        summary_text: row.get(4)?,
        covered_item_start_sequence: row.get(5)?,
        covered_item_end_sequence: row.get(6)?,
        source_item_ids_json: row.get(7)?,
        input_token_estimate: row.get(8)?,
        summary_token_estimate: row.get(9)?,
        compaction_provider_kind: row.get(10)?,
        compaction_model_profile: row.get(11)?,
        status: row.get(12)?,
        error_code: row.get(13)?,
        error_message: row.get(14)?,
    })
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
        source_item_ids: deserialize_json(row.source_item_ids_json)?,
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

fn estimate_to_i64(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}
