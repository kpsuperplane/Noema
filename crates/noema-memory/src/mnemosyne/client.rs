//! HTTP client for Noema's private Mnemosyne sidecar.

use thiserror::Error;

use crate::model::{
    AddMemoryRequest, ListMemoriesRequest, ListMemoriesResponse, SearchMemoriesRequest,
    SearchMemoriesResponse,
};
use crate::operations::MemoryOperationError;

const MNEMOSYNE_REQUEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(180);
const MNEMOSYNE_READINESS_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);

/// Mnemosyne sidecar HTTP client.
#[derive(Debug, Clone)]
pub struct MnemosyneClient {
    base_url: String,
    api_key: Option<String>,
    http: reqwest::Client,
    request_timeout: std::time::Duration,
}

impl MnemosyneClient {
    /// Create a Mnemosyne sidecar client.
    #[must_use]
    pub fn new(base_url: String, api_key: Option<String>) -> Self {
        Self::new_with_request_timeout(base_url, api_key, MNEMOSYNE_REQUEST_TIMEOUT)
    }

    pub(crate) fn new_with_request_timeout(
        base_url: String,
        api_key: Option<String>,
        request_timeout: std::time::Duration,
    ) -> Self {
        let http = reqwest::Client::builder()
            .build()
            .expect("Mnemosyne HTTP client configuration must be valid");
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key,
            http,
            request_timeout,
        }
    }

    /// Check the sidecar readiness endpoint.
    ///
    /// # Errors
    ///
    /// Returns [`MnemosyneClientError`] when the request fails or Mnemosyne
    /// returns a non-success HTTP status.
    pub async fn check_readiness(&self) -> Result<(), MnemosyneClientError> {
        let mut builder = self
            .http
            .get(format!("{}/health", self.base_url))
            .timeout(MNEMOSYNE_READINESS_TIMEOUT);
        if let Some(api_key) = &self.api_key {
            builder = builder.bearer_auth(api_key);
        }

        let response = builder.send().await?;
        ensure_success(response.status())
    }

    /// Add one memory observation to Mnemosyne.
    ///
    /// # Errors
    ///
    /// Returns [`MnemosyneClientError`] when the request fails or Mnemosyne returns a
    /// non-success HTTP status.
    pub async fn add_memory(&self, request: AddMemoryRequest) -> Result<(), MnemosyneClientError> {
        let mut builder = self
            .http
            .post(format!("{}/v1/memories/add", self.base_url))
            .timeout(self.request_timeout)
            .json(&request);
        if let Some(api_key) = &self.api_key {
            builder = builder.bearer_auth(api_key);
        }

        let response = builder.send().await?;
        ensure_success(response.status())
    }

    /// Search Mnemosyne memories.
    ///
    /// # Errors
    ///
    /// Returns [`MnemosyneClientError`] when the request fails, Mnemosyne returns a
    /// non-success status, or the response body cannot be decoded.
    pub async fn search_memories(
        &self,
        request: SearchMemoriesRequest,
    ) -> Result<SearchMemoriesResponse, MnemosyneClientError> {
        let mut builder = self
            .http
            .post(format!("{}/v1/memories/search", self.base_url))
            .timeout(self.request_timeout)
            .json(&request);
        if let Some(api_key) = &self.api_key {
            builder = builder.bearer_auth(api_key);
        }

        let response = builder.send().await?;
        ensure_success(response.status())?;
        response
            .json::<SearchMemoriesResponse>()
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
        request: ListMemoriesRequest,
    ) -> Result<ListMemoriesResponse, MnemosyneClientError> {
        let query = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("user_id", &request.user_id)
            .append_pair("limit", &request.limit.to_string())
            .finish();
        let mut builder = self
            .http
            .get(format!("{}/v1/memories?{query}", self.base_url))
            .timeout(self.request_timeout);
        if let Some(api_key) = &self.api_key {
            builder = builder.bearer_auth(api_key);
        }

        let response = builder.send().await?;
        ensure_success(response.status())?;
        response
            .json::<ListMemoriesResponse>()
            .await
            .map_err(Into::into)
    }
}

fn ensure_success(status: reqwest::StatusCode) -> Result<(), MnemosyneClientError> {
    if status.is_success() {
        Ok(())
    } else {
        Err(MnemosyneClientError::Status(status.as_u16()))
    }
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

impl From<MnemosyneClientError> for MemoryOperationError {
    fn from(error: MnemosyneClientError) -> Self {
        match error {
            MnemosyneClientError::Request(error) if error.is_timeout() => Self::TimedOut,
            MnemosyneClientError::Request(error) if error.is_decode() => Self::UnreadableResponse,
            MnemosyneClientError::Request(_) => Self::RequestFailed,
            MnemosyneClientError::Status(401 | 403) => Self::AuthenticationRejected,
            MnemosyneClientError::Status(status) => Self::UnsuccessfulStatus(status),
        }
    }
}

impl MnemosyneClientError {
    /// Stable sanitized error code suitable for model-visible tool results.
    #[must_use]
    pub fn sanitized_code(&self) -> &'static str {
        match self {
            Self::Request(error) if error.is_timeout() => "timeout",
            Self::Request(error) if error.is_decode() => "decode_failed",
            Self::Request(_) => "request_failed",
            Self::Status(401 | 403) => "auth_error",
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
            Self::Status(401 | 403) => "memory service rejected authentication",
            Self::Status(_) => "memory service returned an unsuccessful status",
        }
    }
}
