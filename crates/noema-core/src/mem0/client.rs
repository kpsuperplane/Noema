//! HTTP client for Noema's private Mem0 sidecar.

use serde::{Deserialize, Serialize};
use thiserror::Error;

const MEM0_REQUEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(3);

/// Mem0 sidecar HTTP client.
#[derive(Debug, Clone)]
pub struct Mem0Client {
    base_url: String,
    api_key: Option<String>,
    http: reqwest::Client,
}

impl Mem0Client {
    /// Create a Mem0 sidecar client.
    #[must_use]
    pub fn new(base_url: String, api_key: Option<String>) -> Self {
        let http = reqwest::Client::builder()
            .timeout(MEM0_REQUEST_TIMEOUT)
            .build()
            .expect("Mem0 HTTP client timeout configuration must be valid");
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key,
            http,
        }
    }

    /// Add one memory observation to Mem0.
    ///
    /// # Errors
    ///
    /// Returns [`Mem0ClientError`] when the request fails or Mem0 returns a
    /// non-success HTTP status.
    pub async fn add_memory(
        &self,
        request: Mem0AddMemoryRequest,
    ) -> Result<(), Mem0ClientError> {
        let mut builder = self
            .http
            .post(format!("{}/v1/memories/add", self.base_url))
            .json(&request);
        if let Some(api_key) = &self.api_key {
            builder = builder.bearer_auth(api_key);
        }

        let response = builder.send().await?;
        if !response.status().is_success() {
            return Err(Mem0ClientError::Status(response.status().as_u16()));
        }

        Ok(())
    }

    /// Search Mem0 memories.
    ///
    /// # Errors
    ///
    /// Returns [`Mem0ClientError`] when the request fails, Mem0 returns a
    /// non-success status, or the response body cannot be decoded.
    pub async fn search_memories(
        &self,
        request: Mem0SearchRequest,
    ) -> Result<Mem0SearchResponse, Mem0ClientError> {
        let mut builder = self
            .http
            .post(format!("{}/v1/memories/search", self.base_url))
            .json(&request);
        if let Some(api_key) = &self.api_key {
            builder = builder.bearer_auth(api_key);
        }

        let response = builder.send().await?;
        if !response.status().is_success() {
            return Err(Mem0ClientError::Status(response.status().as_u16()));
        }

        response
            .json::<Mem0SearchResponse>()
            .await
            .map_err(Into::into)
    }

    /// List Mem0 memories.
    ///
    /// # Errors
    ///
    /// Returns [`Mem0ClientError`] when the request fails, Mem0 returns a
    /// non-success status, or the response body cannot be decoded.
    pub async fn list_memories(
        &self,
        request: Mem0ListMemoriesRequest,
    ) -> Result<Mem0ListMemoriesResponse, Mem0ClientError> {
        let query = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("user_id", &request.user_id)
            .append_pair("limit", &request.limit.to_string())
            .finish();
        let mut builder = self
            .http
            .get(format!("{}/v1/memories?{query}", self.base_url));
        if let Some(api_key) = &self.api_key {
            builder = builder.bearer_auth(api_key);
        }

        let response = builder.send().await?;
        if !response.status().is_success() {
            return Err(Mem0ClientError::Status(response.status().as_u16()));
        }

        response
            .json::<Mem0ListMemoriesResponse>()
            .await
            .map_err(Into::into)
    }
}

/// Add-memory request.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Mem0AddMemoryRequest {
    /// Source messages for this observation.
    pub messages: Vec<Mem0Message>,
    /// Mem0 user scope.
    pub user_id: String,
    /// Optional Mem0 agent scope.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
    /// Optional Mem0 run/session scope.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
    /// Noema provenance metadata.
    pub metadata: serde_json::Value,
}

/// One source message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Mem0Message {
    /// Message role.
    pub role: String,
    /// Plain text content.
    pub content: String,
}

/// Search request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Mem0SearchRequest {
    /// Search query.
    pub query: String,
    /// Mem0 user scope.
    pub user_id: String,
    /// Optional Mem0 agent scope.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
    /// Optional Mem0 run/session scope.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
    /// Maximum result count.
    pub limit: u16,
}

/// Memory list request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Mem0ListMemoriesRequest {
    /// Mem0 user scope.
    pub user_id: String,
    /// Maximum result count.
    pub limit: u16,
}

/// Search response.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct Mem0SearchResponse {
    /// Matching memory results.
    #[serde(default)]
    pub results: Vec<Mem0Memory>,
}

/// Memory list response.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct Mem0ListMemoriesResponse {
    /// Memory results.
    #[serde(default, alias = "memories")]
    pub results: Vec<Mem0Memory>,
}

/// One Mem0 memory.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Mem0Memory {
    /// Mem0 memory id.
    pub id: String,
    /// Memory text.
    #[serde(default)]
    pub memory: Option<String>,
    /// Similarity score.
    #[serde(default)]
    pub score: Option<f64>,
    /// Noema provenance metadata when returned by Mem0.
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,
    /// Creation timestamp.
    #[serde(default)]
    pub created_at: Option<String>,
    /// Last update timestamp.
    #[serde(default)]
    pub updated_at: Option<String>,
}

/// Errors returned by the Mem0 client boundary.
#[derive(Debug, Error)]
pub enum Mem0ClientError {
    /// Mem0 request failed at the transport layer.
    #[error("mem0 request failed: {0}")]
    Request(#[from] reqwest::Error),
    /// Mem0 returned a non-success HTTP status.
    #[error("mem0 returned HTTP status {0}")]
    Status(u16),
}

impl Mem0ClientError {
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
