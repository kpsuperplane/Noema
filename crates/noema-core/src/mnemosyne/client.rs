//! HTTP client for Noema's private Mnemosyne sidecar.

use serde::{Deserialize, Serialize};
use thiserror::Error;

const MNEMOSYNE_REQUEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(3);

/// Mnemosyne sidecar HTTP client.
#[derive(Debug, Clone)]
pub struct MnemosyneClient {
    base_url: String,
    api_key: Option<String>,
    http: reqwest::Client,
}

impl MnemosyneClient {
    /// Create a Mnemosyne sidecar client.
    #[must_use]
    pub fn new(base_url: String, api_key: Option<String>) -> Self {
        let http = reqwest::Client::builder()
            .timeout(MNEMOSYNE_REQUEST_TIMEOUT)
            .build()
            .expect("Mnemosyne HTTP client timeout configuration must be valid");
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key,
            http,
        }
    }

    /// Add one memory observation to Mnemosyne.
    ///
    /// # Errors
    ///
    /// Returns [`MnemosyneClientError`] when the request fails or Mnemosyne returns a
    /// non-success HTTP status.
    pub async fn add_memory(
        &self,
        request: MnemosyneAddMemoryRequest,
    ) -> Result<(), MnemosyneClientError> {
        let mut builder = self
            .http
            .post(format!("{}/v1/memories/add", self.base_url))
            .json(&request);
        if let Some(api_key) = &self.api_key {
            builder = builder.bearer_auth(api_key);
        }

        let response = builder.send().await?;
        if !response.status().is_success() {
            return Err(MnemosyneClientError::Status(response.status().as_u16()));
        }

        Ok(())
    }

    /// Search Mnemosyne memories.
    ///
    /// # Errors
    ///
    /// Returns [`MnemosyneClientError`] when the request fails, Mnemosyne returns a
    /// non-success status, or the response body cannot be decoded.
    pub async fn search_memories(
        &self,
        request: MnemosyneSearchRequest,
    ) -> Result<MnemosyneSearchResponse, MnemosyneClientError> {
        let mut builder = self
            .http
            .post(format!("{}/v1/memories/search", self.base_url))
            .json(&request);
        if let Some(api_key) = &self.api_key {
            builder = builder.bearer_auth(api_key);
        }

        let response = builder.send().await?;
        if !response.status().is_success() {
            return Err(MnemosyneClientError::Status(response.status().as_u16()));
        }

        response
            .json::<MnemosyneSearchResponse>()
            .await
            .map_err(Into::into)
    }

    /// List Mnemosyne memories.
    ///
    /// # Errors
    ///
    /// Returns [`MnemosyneClientError`] when the request fails, Mnemosyne returns a
    /// non-success status, or the response body cannot be decoded.
    pub async fn list_memories(
        &self,
        request: MnemosyneListMemoriesRequest,
    ) -> Result<MnemosyneListMemoriesResponse, MnemosyneClientError> {
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
            return Err(MnemosyneClientError::Status(response.status().as_u16()));
        }

        response
            .json::<MnemosyneListMemoriesResponse>()
            .await
            .map_err(Into::into)
    }
}

/// Add-memory request.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MnemosyneAddMemoryRequest {
    /// Source messages for this observation.
    pub messages: Vec<MnemosyneMessage>,
    /// Mnemosyne user scope.
    pub user_id: String,
    /// Optional Mnemosyne agent scope.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
    /// Optional Mnemosyne run/session scope.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
    /// Noema provenance metadata.
    pub metadata: serde_json::Value,
}

/// One source message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MnemosyneMessage {
    /// Message role.
    pub role: String,
    /// Plain text content.
    pub content: String,
}

/// Search request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MnemosyneSearchRequest {
    /// Search query.
    pub query: String,
    /// Mnemosyne user scope.
    pub user_id: String,
    /// Optional Mnemosyne agent scope.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
    /// Optional Mnemosyne run/session scope.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
    /// Maximum result count.
    pub limit: u16,
}

/// Memory list request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MnemosyneListMemoriesRequest {
    /// Mnemosyne user scope.
    pub user_id: String,
    /// Maximum result count.
    pub limit: u16,
}

/// Search response.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct MnemosyneSearchResponse {
    /// Matching memory results.
    #[serde(default)]
    pub results: Vec<MnemosyneMemory>,
}

/// Memory list response.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct MnemosyneListMemoriesResponse {
    /// Memory results.
    #[serde(default, alias = "memories")]
    pub results: Vec<MnemosyneMemory>,
}

/// One Mnemosyne memory.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct MnemosyneMemory {
    /// Mnemosyne memory id.
    pub id: String,
    /// Memory text.
    #[serde(default)]
    pub memory: Option<String>,
    /// Similarity score.
    #[serde(default)]
    pub score: Option<f64>,
    /// Noema provenance metadata when returned by Mnemosyne.
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,
    /// Creation timestamp.
    #[serde(default)]
    pub created_at: Option<String>,
    /// Last update timestamp.
    #[serde(default)]
    pub updated_at: Option<String>,
}

/// Errors returned by the Mnemosyne client boundary.
#[derive(Debug, Error)]
pub enum MnemosyneClientError {
    /// Mnemosyne request failed at the transport layer.
    #[error("mnemosyne request failed: {0}")]
    Request(#[from] reqwest::Error),
    /// Mnemosyne returned a non-success HTTP status.
    #[error("mnemosyne returned HTTP status {0}")]
    Status(u16),
}

impl MnemosyneClientError {
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
