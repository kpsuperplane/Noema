//! Stable memory settings, records, and service-neutral request models.

use noema_providers::{ProviderInstanceKey, ReasoningEffort};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Stable human-memory scope used by the current single-human product slice.
pub const HUMAN_MEMORY_SCOPE_ID: &str = "human:local";

/// Normalize one durable conversation id into its governed memory scope id.
#[must_use]
pub fn conversation_scope_id(conversation_id: &str) -> String {
    if conversation_id.starts_with("conversation:") {
        conversation_id.to_string()
    } else {
        format!("conversation:{conversation_id}")
    }
}

/// Saved memory service mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryServiceMode {
    /// Noema manages a local memory service child process.
    Managed,
    /// Noema connects to an externally managed memory service.
    External,
}

impl MemoryServiceMode {
    /// Return the persistence representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Managed => "managed",
            Self::External => "external",
        }
    }

    /// Parse the persistence representation.
    ///
    /// # Errors
    ///
    /// Returns [`MemorySettingsError::InvalidServiceMode`] for an unknown value.
    pub fn parse(value: &str) -> Result<Self, MemorySettingsError> {
        match value {
            "managed" => Ok(Self::Managed),
            "external" => Ok(Self::External),
            _ => Err(MemorySettingsError::InvalidServiceMode {
                value: value.to_string(),
            }),
        }
    }
}

/// Memory settings vocabulary failure.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum MemorySettingsError {
    /// A persisted service mode is outside the closed vocabulary.
    #[error("invalid memory service mode: {value}")]
    InvalidServiceMode {
        /// Rejected persistence value.
        value: String,
    },
}

/// Persisted memory service settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryServiceSettingsRecord {
    /// Stable singleton settings id.
    pub settings_id: String,
    /// Memory service mode.
    pub mode: MemoryServiceMode,
    /// External memory service base URL.
    pub base_url: Option<String>,
    /// External service port, when configured.
    pub port: Option<u16>,
    /// Provider account used for memory extraction.
    pub provider_account_id: Option<String>,
    /// Provider kind used for memory extraction.
    pub provider_kind: Option<String>,
    /// Exact provider process used for memory model work.
    pub provider_instance_key: Option<ProviderInstanceKey>,
    /// Model profile used for memory extraction.
    pub model_profile: Option<String>,
    /// Reasoning effort used for memory extraction.
    pub reasoning_effort: Option<ReasoningEffort>,
}

/// Input for saving memory service settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveMemoryServiceSettings {
    /// Memory service mode.
    pub mode: MemoryServiceMode,
    /// External memory service base URL.
    pub base_url: Option<String>,
    /// External service port, when configured.
    pub port: Option<u16>,
    /// Provider account used for memory extraction.
    pub provider_account_id: Option<String>,
    /// Provider kind used for memory extraction.
    pub provider_kind: Option<String>,
    /// Model profile used for memory extraction.
    pub model_profile: Option<String>,
    /// Reasoning effort used for memory extraction.
    pub reasoning_effort: Option<ReasoningEffort>,
}

/// Cached AI-written memory article.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryArticleCacheRecord {
    /// Memory scope this article describes.
    pub scope_id: String,
    /// Fingerprint of the facts used to generate the article.
    pub fact_fingerprint: String,
    /// Cached article Markdown.
    pub article_markdown: String,
    /// Timestamp when the article was generated.
    pub generated_at: String,
}

/// Input for saving a cached memory article.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveMemoryArticleCache {
    /// Memory scope this article describes.
    pub scope_id: String,
    /// Fingerprint of the facts used to generate the article.
    pub fact_fingerprint: String,
    /// Cached article Markdown.
    pub article_markdown: String,
    /// Timestamp when the article was generated.
    pub generated_at: String,
}

/// One source message submitted as a memory observation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryMessage {
    /// Message role.
    pub role: String,
    /// Plain text content.
    pub content: String,
}

/// Request to submit one memory observation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AddMemoryRequest {
    /// Source messages for this observation.
    pub messages: Vec<MemoryMessage>,
    /// Memory user scope.
    pub user_id: String,
    /// Optional memory agent scope.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
    /// Optional memory run/session scope.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
    /// Noema provenance metadata.
    pub metadata: serde_json::Value,
}

/// Request to search memory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchMemoriesRequest {
    /// Search query.
    pub query: String,
    /// Memory user scope.
    pub user_id: String,
    /// Optional memory agent scope.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
    /// Optional memory run/session scope.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
    /// Maximum result count.
    pub limit: u16,
}

/// Request to list memory records.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListMemoriesRequest {
    /// Memory user scope.
    pub user_id: String,
    /// Maximum result count.
    pub limit: u16,
}

/// One memory record returned by the service.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct MemoryRecord {
    /// Service-owned memory id.
    pub id: String,
    /// Memory text.
    #[serde(default)]
    pub memory: Option<String>,
    /// Similarity score.
    #[serde(default)]
    pub score: Option<f64>,
    /// Noema provenance metadata when returned by the service.
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,
    /// Creation timestamp.
    #[serde(default)]
    pub created_at: Option<String>,
    /// Last update timestamp.
    #[serde(default)]
    pub updated_at: Option<String>,
}

/// Search results returned by memory operations.
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
pub struct SearchMemoriesResponse {
    /// Matching memory records.
    #[serde(default)]
    pub results: Vec<MemoryRecord>,
}

/// Listed memory records returned by memory operations.
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
pub struct ListMemoriesResponse {
    /// Memory records.
    #[serde(default, alias = "memories")]
    pub results: Vec<MemoryRecord>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_service_mode_round_trips_persistence_values() {
        for mode in [MemoryServiceMode::Managed, MemoryServiceMode::External] {
            assert_eq!(MemoryServiceMode::parse(mode.as_str()), Ok(mode));
        }
    }

    #[test]
    fn conversation_scope_id_preserves_canonical_ids() {
        assert_eq!(conversation_scope_id("conv_1"), "conversation:conv_1");
        assert_eq!(
            conversation_scope_id("conversation:conv_1"),
            "conversation:conv_1"
        );
    }

    #[test]
    fn list_response_accepts_legacy_memories_key() {
        let response: ListMemoriesResponse = serde_json::from_value(serde_json::json!({
            "memories": [{"id": "memory_1", "memory": "likes tea"}]
        }))
        .expect("list response");

        assert_eq!(response.results[0].id, "memory_1");
    }
}
