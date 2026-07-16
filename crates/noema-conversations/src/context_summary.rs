use crate::ConversationContextSummaryStatus;

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
