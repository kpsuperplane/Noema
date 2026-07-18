//! HTTP client for Noema's private Mnemosyne sidecar.

use thiserror::Error;

use crate::model::{
    AddMemoryRequest, ListMemoriesRequest, ListMemoriesResponse, SearchMemoriesRequest,
    SearchMemoriesResponse,
};
use crate::operations::MemoryOperationError;

const MNEMOSYNE_REQUEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(180);
const MNEMOSYNE_READINESS_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);

#[derive(Debug, Clone)]
pub(super) struct MnemosyneClient {
    base_url: String,
    api_key: Option<String>,
    http: reqwest::Client,
    request_timeout: std::time::Duration,
}

impl MnemosyneClient {
    #[must_use]
    pub(super) fn new(base_url: String, api_key: Option<String>) -> Self {
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

    pub(super) async fn check_readiness(&self) -> Result<(), MnemosyneClientError> {
        self.send(
            self.http.get(format!("{}/health", self.base_url)),
            MNEMOSYNE_READINESS_TIMEOUT,
        )
        .await
        .map(drop)
    }

    pub(super) async fn add_memory(
        &self,
        request: AddMemoryRequest,
    ) -> Result<(), MnemosyneClientError> {
        self.send(
            self.http
                .post(format!("{}/v1/memories/add", self.base_url))
                .json(&request),
            self.request_timeout,
        )
        .await
        .map(drop)
    }

    pub(super) async fn search_memories(
        &self,
        request: SearchMemoriesRequest,
    ) -> Result<SearchMemoriesResponse, MnemosyneClientError> {
        let response = self
            .send(
                self.http
                    .post(format!("{}/v1/memories/search", self.base_url))
                    .json(&request),
                self.request_timeout,
            )
            .await?;
        response
            .json::<SearchMemoriesResponse>()
            .await
            .map_err(Into::into)
    }

    pub(super) async fn list_memories(
        &self,
        request: ListMemoriesRequest,
    ) -> Result<ListMemoriesResponse, MnemosyneClientError> {
        let query = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("user_id", &request.user_id)
            .append_pair("limit", &request.limit.to_string())
            .finish();
        let response = self
            .send(
                self.http
                    .get(format!("{}/v1/memories?{query}", self.base_url)),
                self.request_timeout,
            )
            .await?;
        response
            .json::<ListMemoriesResponse>()
            .await
            .map_err(Into::into)
    }

    async fn send(
        &self,
        mut request: reqwest::RequestBuilder,
        timeout: std::time::Duration,
    ) -> Result<reqwest::Response, MnemosyneClientError> {
        request = request.timeout(timeout);
        if let Some(api_key) = &self.api_key {
            request = request.bearer_auth(api_key);
        }
        let response = request.send().await?;
        ensure_success(response.status())?;
        Ok(response)
    }
}

fn ensure_success(status: reqwest::StatusCode) -> Result<(), MnemosyneClientError> {
    if status.is_success() {
        Ok(())
    } else {
        Err(MnemosyneClientError::Status(status.as_u16()))
    }
}

#[derive(Debug, Error)]
pub(super) enum MnemosyneClientError {
    #[error("mnemosyne request failed: {0}")]
    Request(#[from] reqwest::Error),
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
