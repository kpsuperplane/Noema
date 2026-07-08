//! HTTP client for the local or external Supermemory service.

use serde::{Deserialize, Serialize};
use thiserror::Error;

const SUPERMEMORY_REQUEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(3);

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
        let http = reqwest::Client::builder()
            .timeout(SUPERMEMORY_REQUEST_TIMEOUT)
            .build()
            .expect("Supermemory HTTP client timeout configuration must be valid");
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key,
            http,
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

    /// Submit a conversation observation payload to Supermemory.
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

    /// List documents and memory entries for the Supermemory memory graph.
    ///
    /// # Errors
    ///
    /// Returns [`SupermemoryClientError`] when the request fails, Supermemory
    /// returns a non-success status, or the response body cannot be decoded.
    pub async fn list_memory_graph_documents(
        &self,
        request: SupermemoryGraphDocumentsRequest,
    ) -> Result<SupermemoryGraphDocumentsResponse, SupermemoryClientError> {
        let mut builder = self
            .http
            .post(format!("{}/v3/documents/documents", self.base_url))
            .json(&serde_json::json!({
                "containerTag": request.container_tag,
                "containerTags": [request.container_tag],
                "page": request.page,
                "limit": request.limit,
                "sort": "createdAt",
                "order": "desc"
            }));
        if let Some(api_key) = &self.api_key {
            builder = builder.bearer_auth(api_key);
        }

        let response = builder.send().await?;
        if !response.status().is_success() {
            return Err(SupermemoryClientError::Status(response.status().as_u16()));
        }

        response
            .json::<SupermemoryGraphDocumentsResponse>()
            .await
            .map_err(Into::into)
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

/// Memory graph document list request for one Supermemory container tag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SupermemoryGraphDocumentsRequest {
    /// Deterministic Noema-owned Supermemory container tag.
    pub container_tag: String,
    /// One-based Supermemory document page.
    pub page: u32,
    /// Maximum document count.
    pub limit: u32,
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
    #[serde(default)]
    pub memory: Option<String>,
    /// Supermemory metadata payload.
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,
    /// Last updated timestamp from Supermemory.
    #[serde(rename = "updatedAt")]
    pub updated_at: Option<String>,
    /// Similarity score from Supermemory.
    #[serde(default)]
    pub similarity: Option<f64>,
}

/// Supermemory memory graph document response.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct SupermemoryGraphDocumentsResponse {
    /// Documents and their extracted memory entries.
    #[serde(default)]
    pub documents: Vec<SupermemoryGraphDocument>,
    /// Supermemory pagination metadata.
    #[serde(default)]
    pub pagination: SupermemoryGraphPagination,
}

/// Supermemory graph pagination metadata.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SupermemoryGraphPagination {
    /// Current page when returned.
    #[serde(default, alias = "currentPage")]
    pub page: Option<u32>,
    /// Page size when returned.
    #[serde(default)]
    pub limit: Option<u32>,
    /// Whether more documents are available.
    #[serde(default)]
    pub has_more: Option<bool>,
    /// Total document count when returned.
    #[serde(default, alias = "totalCount", alias = "totalItems")]
    pub total: Option<u64>,
}

/// One Supermemory graph document.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SupermemoryGraphDocument {
    /// Supermemory document id.
    pub id: String,
    /// Caller supplied document id.
    #[serde(default)]
    pub custom_id: Option<String>,
    /// Document title.
    #[serde(default)]
    pub title: Option<String>,
    /// Document content.
    #[serde(default)]
    pub content: Option<String>,
    /// Document summary.
    #[serde(default)]
    pub summary: Option<String>,
    /// Source URL.
    #[serde(default)]
    pub url: Option<String>,
    /// Source label.
    #[serde(default)]
    pub source: Option<String>,
    /// Supermemory document type.
    #[serde(default, alias = "documentType")]
    pub r#type: Option<String>,
    /// Supermemory document status.
    pub status: String,
    /// Document metadata.
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,
    /// Creation timestamp.
    pub created_at: String,
    /// Update timestamp.
    pub updated_at: String,
    /// Memory entries extracted from the document.
    #[serde(default, alias = "memories")]
    pub memory_entries: Vec<SupermemoryGraphMemoryEntry>,
}

/// One Supermemory graph memory entry.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SupermemoryGraphMemoryEntry {
    /// Supermemory memory entry id.
    pub id: String,
    /// Source document id.
    #[serde(default)]
    pub document_id: Option<String>,
    /// Memory content.
    #[serde(default, alias = "memory")]
    pub content: Option<String>,
    /// Memory summary.
    #[serde(default)]
    pub summary: Option<String>,
    /// Memory title.
    #[serde(default)]
    pub title: Option<String>,
    /// Memory type.
    #[serde(default)]
    pub r#type: Option<String>,
    /// Memory metadata.
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,
    /// Creation timestamp.
    pub created_at: String,
    /// Update timestamp.
    pub updated_at: String,
    /// Container tag used by Supermemory spaces.
    #[serde(default)]
    pub space_container_tag: Option<String>,
    /// Relationship to another memory.
    #[serde(default)]
    pub relation: Option<String>,
    /// Parent memory id when Supermemory links memories into its graph.
    #[serde(default)]
    pub parent_memory_id: Option<String>,
    /// Root memory id for the memory graph branch.
    #[serde(default)]
    pub root_memory_id: Option<String>,
    /// Supermemory graph relation map keyed by target memory id.
    #[serde(default)]
    pub memory_relations: Option<serde_json::Value>,
    /// Whether this is the latest memory.
    #[serde(default)]
    pub is_latest: Option<bool>,
    /// Supermemory space id.
    #[serde(default)]
    pub space_id: Option<String>,
}

/// Conversation ingest request.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SupermemoryConversationIngestRequest {
    /// Supermemory conversation/source document identity.
    pub conversation_id: String,
    /// Deterministic Noema-owned Supermemory container tag.
    pub container_tag: String,
    /// Compatibility tag list for the bundled self-hosted Supermemory build.
    pub container_tags: Vec<String>,
    /// Flat Noema provenance metadata for this observation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
    /// Conversation messages submitted for Supermemory extraction.
    pub messages: Vec<SupermemoryConversationMessage>,
}

impl SupermemoryConversationIngestRequest {
    /// Build an observation ingest request for one Noema-owned container tag.
    #[must_use]
    pub fn new(
        conversation_id: impl Into<String>,
        container_tag: impl Into<String>,
        messages: Vec<SupermemoryConversationMessage>,
    ) -> Self {
        let container_tag = container_tag.into();
        Self {
            conversation_id: conversation_id.into(),
            container_tag: container_tag.clone(),
            container_tags: vec![container_tag],
            metadata: None,
            messages,
        }
    }

    /// Attach provenance metadata to the ingest request.
    #[must_use]
    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = Some(metadata);
        self
    }
}

/// One Supermemory conversation message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SupermemoryConversationMessage {
    /// Message role expected by Supermemory.
    pub role: String,
    /// Plain message content.
    pub content: String,
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

impl SupermemoryClientError {
    /// Stable sanitized error code suitable for model-visible tool results.
    #[must_use]
    pub fn sanitized_code(&self) -> &'static str {
        match self {
            Self::Request(error) if error.is_timeout() => "timeout",
            Self::Request(error) if error.is_decode() => "decode_failed",
            Self::Request(_) => "request_failed",
            Self::Status(status) if *status == 401 || *status == 403 => "auth_error",
            Self::Status(_) => "http_error",
        }
    }

    /// Stable sanitized error message suitable for model-visible tool results.
    #[must_use]
    pub fn sanitized_message(&self) -> &'static str {
        match self {
            Self::Request(error) if error.is_timeout() => "memory service request timed out",
            Self::Request(error) if error.is_decode() => {
                "memory service returned an unreadable response"
            }
            Self::Request(_) => "memory service request failed",
            Self::Status(status) if *status == 401 || *status == 403 => {
                "memory service rejected authentication"
            }
            Self::Status(_) => "memory service returned an unsuccessful status",
        }
    }
}
