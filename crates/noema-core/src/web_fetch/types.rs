//! Provider-neutral web fetch types.

use crate::web_fetch::direct_http::DirectHttpClient;
use noema_capabilities::web::fetch::{FetchRequest, FetchResponse};
use noema_providers::ProviderHandle;
use thiserror::Error;

pub const DIRECT_HTTP_PROVIDER_ID: &str = "direct_http";
pub const EXTRACTION_READABILITYRS: &str = "readability_rs";
pub const RAW_MARKDOWN_LIMIT_CHARS: usize = 8_000;
pub const SINGLE_PASS_SUMMARY_LIMIT_CHARS: usize = 250_000;
pub const CHUNKED_SUMMARY_LIMIT_CHARS: usize = 1_000_000;
pub const RAW_EXCERPT_CHARS: usize = 2_000;

#[derive(Debug, Clone)]
pub struct FetchRuntimeContext {
    pub summarizer_provider_kind: String,
    pub summarizer_provider: ProviderHandle,
    pub summarizer_model: String,
    pub summarizer_reasoning_effort: Option<noema_providers::ReasoningEffort>,
    pub generation_priority: noema_providers::GenerationPriority,
}

#[derive(Debug, Error)]
pub enum FetchError {
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
    #[error("provider account unauthenticated")]
    AuthFailed,
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
pub enum WebFetchRuntimeProvider {
    DirectHttp {
        client: DirectHttpClient,
    },
    Exa {
        client: crate::web_fetch::exa::ExaFetchClient,
    },
    #[cfg(test)]
    Static {
        response: Box<FetchResponse>,
    },
}

impl Default for WebFetchRuntimeProvider {
    fn default() -> Self {
        Self::DirectHttp {
            client: DirectHttpClient::default(),
        }
    }
}

impl WebFetchRuntimeProvider {
    pub async fn fetch(
        &self,
        request: &FetchRequest,
        context: &FetchRuntimeContext,
    ) -> Result<FetchResponse, FetchError> {
        match self {
            Self::DirectHttp { client } => {
                crate::web_fetch::direct_http::fetch_direct_http(client, request, context).await
            }
            Self::Exa { client } => crate::web_fetch::exa::fetch_exa(client, request).await,
            #[cfg(test)]
            Self::Static { response } => {
                let mut response = response.as_ref().clone();
                response.url = request.url.clone();
                response.returned_chars = response.content.chars().count();
                Ok(response)
            }
        }
    }
}
