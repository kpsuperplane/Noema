//! HTTP client for the local or external Supermemory service.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Supermemory HTTP client.
#[derive(Debug, Clone)]
pub struct SupermemoryClient {
    base_url: String,
    api_key: Option<String>,
    http: reqwest::Client,
}

impl SupermemoryClient {
    /// Create a Supermemory client.
    #[must_use]
    pub fn new(base_url: String, api_key: Option<String>) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key,
            http: reqwest::Client::new(),
        }
    }

    /// Search Supermemory memories in one container tag.
    ///
    /// # Errors
    ///
    /// Returns [`SupermemoryClientError`] when the request fails, Supermemory
    /// returns a non-success status, or the response body cannot be decoded.
    pub async fn search_memories(
        &self,
        request: SupermemorySearchRequest,
    ) -> Result<SupermemorySearchResponse, SupermemoryClientError> {
        let mut builder =
            self.http
                .post(format!("{}/v4/search", self.base_url))
                .json(&serde_json::json!({
                    "q": request.query,
                    "containerTag": request.container_tag,
                    "limit": request.limit,
                    "searchMode": "memories",
                    "include": {
                        "documents": false,
                        "summaries": false,
                        "relatedMemories": false,
                        "forgottenMemories": false,
                        "chunks": false
                    }
                }));
        if let Some(api_key) = &self.api_key {
            builder = builder.bearer_auth(api_key);
        }

        let response = builder.send().await?;
        if !response.status().is_success() {
            return Err(SupermemoryClientError::Status(response.status().as_u16()));
        }

        response
            .json::<SupermemorySearchResponse>()
            .await
            .map_err(Into::into)
    }

    /// Submit a completed conversation payload to Supermemory.
    ///
    /// # Errors
    ///
    /// Returns [`SupermemoryClientError`] when the request fails or Supermemory
    /// returns a non-success status.
    pub async fn ingest_conversation(
        &self,
        request: SupermemoryConversationIngestRequest,
    ) -> Result<(), SupermemoryClientError> {
        let mut builder = self
            .http
            .post(format!("{}/v4/conversations", self.base_url))
            .json(&request);
        if let Some(api_key) = &self.api_key {
            builder = builder.bearer_auth(api_key);
        }

        let response = builder.send().await?;
        if !response.status().is_success() {
            return Err(SupermemoryClientError::Status(response.status().as_u16()));
        }

        Ok(())
    }
}

/// Search request for one Supermemory container tag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SupermemorySearchRequest {
    /// User/model search query.
    pub query: String,
    /// Deterministic Noema-owned Supermemory container tag.
    pub container_tag: String,
    /// Maximum result count.
    pub limit: u16,
}

/// Supermemory search response.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct SupermemorySearchResponse {
    /// Matching memory results.
    pub results: Vec<SupermemorySearchResult>,
    /// Supermemory timing value when returned.
    #[serde(default)]
    pub timing: Option<u64>,
    /// Total matching result count when returned.
    #[serde(default)]
    pub total: Option<u64>,
}

/// One Supermemory search result.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct SupermemorySearchResult {
    /// Supermemory memory id.
    pub id: String,
    /// Memory text.
    pub memory: String,
    /// Supermemory metadata payload.
    #[serde(default)]
    pub metadata: serde_json::Value,
    /// Last updated timestamp from Supermemory.
    #[serde(rename = "updatedAt")]
    pub updated_at: Option<String>,
    /// Similarity score from Supermemory.
    #[serde(default)]
    pub similarity: Option<f64>,
}

/// Conversation ingest request.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SupermemoryConversationIngestRequest {
    /// Noema conversation id.
    pub conversation_id: String,
    /// Deterministic Noema-owned Supermemory container tag.
    pub container_tag: String,
    /// Serialized conversation payload owned by the caller.
    pub payload: serde_json::Value,
}

/// Errors returned by the Supermemory client boundary.
#[derive(Debug, Error)]
pub enum SupermemoryClientError {
    /// Supermemory request failed at the transport layer.
    #[error("supermemory request failed: {0}")]
    Request(#[from] reqwest::Error),
    /// Supermemory returned a non-success HTTP status.
    #[error("supermemory returned HTTP status {0}")]
    Status(u16),
}
