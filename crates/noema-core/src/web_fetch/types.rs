//! Provider-neutral web fetch types.

use crate::daemon::RuntimeModelProvider;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use thiserror::Error;

pub(crate) const DIRECT_HTTP_PROVIDER_ID: &str = "direct_http";
pub(crate) const EXTRACTION_READABILITYRS: &str = "readability_rs";
pub(crate) const DEFAULT_MAX_CHARS: usize = 20_000;
pub(crate) const HARD_MAX_CHARS: usize = 20_000;
pub(crate) const MAX_URL_CHARS: usize = 2048;
pub(crate) const MAX_REASON_CHARS: usize = 500;
pub(crate) const RAW_MARKDOWN_LIMIT_CHARS: usize = 8_000;
pub(crate) const SINGLE_PASS_SUMMARY_LIMIT_CHARS: usize = 250_000;
pub(crate) const CHUNKED_SUMMARY_LIMIT_CHARS: usize = 1_000_000;
pub(crate) const RAW_EXCERPT_CHARS: usize = 2_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FetchRequest {
    pub url: String,
    pub reason: Option<String>,
    pub max_chars: usize,
}

#[derive(Debug, Clone)]
pub(crate) struct FetchRuntimeContext {
    pub summarizer_provider_kind: String,
    pub summarizer_provider: Arc<dyn RuntimeModelProvider>,
    pub summarizer_model: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum FetchContentKind {
    RawMarkdown,
    Summary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum FetchSummaryStrategy {
    NotSummarized,
    SinglePass,
    Chunked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct FetchResponse {
    pub provider: String,
    pub url: String,
    pub final_url: String,
    pub title: Option<String>,
    pub format: String,
    pub extraction: String,
    pub content_kind: FetchContentKind,
    pub content: String,
    pub raw_excerpt: Option<String>,
    pub raw_chars: usize,
    pub returned_chars: usize,
    pub summary_model: Option<String>,
    pub summary_strategy: FetchSummaryStrategy,
    pub truncated: bool,
}

#[derive(Debug, Error)]
pub(crate) enum FetchError {
    #[error("{0}")]
    InvalidArguments(String),
    #[error("unsupported URL scheme")]
    UnsupportedScheme,
    #[error("malformed URL")]
    MalformedUrl,
    #[error("blocked private or local target")]
    BlockedTarget,
    #[error("DNS lookup failed")]
    Dns,
    #[error("redirect target is blocked")]
    RedirectBlocked,
    #[error("too many redirects")]
    TooManyRedirects,
    #[error("fetch request timed out")]
    Timeout,
    #[error("fetch request failed")]
    Http,
    #[error("unsupported content type")]
    UnsupportedContentType,
    #[error("response too large")]
    ResponseTooLarge,
    #[error("page content could not be extracted")]
    Extraction,
    #[error("summarization failed")]
    Summarization,
    #[error("page too large to summarize")]
    PageTooLarge,
}

#[derive(Debug, Clone)]
pub(crate) enum WebFetchRuntimeProvider {
    DirectHttp {
        client: Client,
    },
    #[cfg(test)]
    Static {
        response: FetchResponse,
    },
}

impl Default for WebFetchRuntimeProvider {
    fn default() -> Self {
        Self::DirectHttp {
            client: Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .expect("direct web fetch client"),
        }
    }
}

impl WebFetchRuntimeProvider {
    pub(crate) async fn fetch(
        &self,
        request: &FetchRequest,
        context: &FetchRuntimeContext,
    ) -> Result<FetchResponse, FetchError> {
        match self {
            Self::DirectHttp { client } => {
                crate::web_fetch::direct_http::fetch_direct_http(client, request, context).await
            }
            #[cfg(test)]
            Self::Static { response } => {
                let mut response = response.clone();
                response.url = request.url.clone();
                response.returned_chars = response.content.chars().count();
                Ok(response)
            }
        }
    }
}
