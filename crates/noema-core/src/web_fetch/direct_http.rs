//! Direct Rust HTTP implementation for `web.fetch`.

use crate::web_fetch::{
    extraction::{extract_readable_content, normalize_plain_text},
    summarize::{SummaryDecision, raw_excerpt, summarize_markdown, summary_strategy_for_chars},
    types::{
        DIRECT_HTTP_PROVIDER_ID, EXTRACTION_READABILITYRS, FetchContentKind, FetchError,
        FetchRequest, FetchResponse, FetchRuntimeContext, FetchSummaryStrategy,
    },
    url_policy::{CheckedUrl, validate_public_web_fetch_url, validate_public_web_fetch_url_parsed},
};
use futures_util::StreamExt;
use reqwest::{Client, StatusCode, header};
#[cfg(test)]
use url::Url;

const MAX_REDIRECTS: usize = 3;
const MAX_RESPONSE_BYTES: usize = 750 * 1024;
const USER_AGENT: &str = "NoemaWebFetch/0.1 (+https://github.com/kpsuperplane/Noema)";

#[derive(Debug, Clone)]
pub struct DirectHttpClient {
    client: Client,
}

impl Default for DirectHttpClient {
    fn default() -> Self {
        Self {
            client: Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .expect("direct web fetch client"),
        }
    }
}

pub async fn fetch_direct_http(
    client: &DirectHttpClient,
    request: &FetchRequest,
    context: &FetchRuntimeContext,
) -> Result<FetchResponse, FetchError> {
    let checked = validate_public_web_fetch_url(&request.url).await?;
    fetch_direct_http_checked(client, request, context, checked).await
}

#[cfg(test)]
async fn fetch_direct_http_unchecked_initial_url(
    client: &DirectHttpClient,
    request: &FetchRequest,
    context: &FetchRuntimeContext,
) -> Result<FetchResponse, FetchError> {
    let url = Url::parse(&request.url).map_err(|_| FetchError::MalformedUrl)?;
    let checked = CheckedUrl {
        url,
        resolved_ips: Vec::new(),
    };
    fetch_direct_http_checked(client, request, context, checked).await
}

async fn fetch_direct_http_checked(
    client: &DirectHttpClient,
    request: &FetchRequest,
    context: &FetchRuntimeContext,
    mut checked: CheckedUrl,
) -> Result<FetchResponse, FetchError> {
    let original_url = checked.url.to_string();

    for redirect_count in 0..=MAX_REDIRECTS {
        let response = client
            .client
            .get(checked.url.clone())
            .header(header::USER_AGENT, USER_AGENT)
            .header(
                header::ACCEPT,
                "text/html,text/plain,text/markdown;q=0.9,*/*;q=0.1",
            )
            .header(header::ACCEPT_LANGUAGE, "en-US,en;q=0.9")
            .timeout(std::time::Duration::from_secs(30))
            .send()
            .await
            .map_err(map_reqwest_error)?;

        if response.status().is_redirection() {
            if redirect_count == MAX_REDIRECTS {
                return Err(FetchError::TooManyRedirects);
            }
            let location = response
                .headers()
                .get(header::LOCATION)
                .and_then(|value| value.to_str().ok())
                .ok_or(FetchError::Http)?;
            let next_url = checked
                .url
                .join(location)
                .map_err(|_| FetchError::MalformedUrl)?;
            checked = validate_public_web_fetch_url_parsed(next_url)
                .await
                .map_err(|error| match error {
                    FetchError::BlockedTarget | FetchError::UnsupportedScheme => {
                        FetchError::RedirectBlocked
                    }
                    other => other,
                })?;
            continue;
        }

        if !response.status().is_success() {
            return Err(match response.status() {
                StatusCode::REQUEST_TIMEOUT | StatusCode::GATEWAY_TIMEOUT => FetchError::Timeout,
                _ => FetchError::Http,
            });
        }

        let content_type = response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("")
            .to_ascii_lowercase();
        let is_html = content_type.contains("text/html") || content_type.is_empty();
        let is_text = content_type.contains("text/plain")
            || content_type.contains("text/markdown")
            || content_type.contains("text/x-markdown")
            || content_type.contains("application/markdown");
        if !is_html && !is_text {
            return Err(FetchError::UnsupportedContentType);
        }

        let final_url = checked.url.to_string();
        let body = read_limited_body(response).await?;
        let body_text = String::from_utf8_lossy(&body).to_string();
        let extracted = if is_html {
            extract_readable_content(&body_text, &final_url)?
        } else {
            normalize_plain_text(&body_text)
        };
        let raw_chars = extracted.markdown.chars().count();
        let decision = summary_strategy_for_chars(raw_chars);
        let (content_kind, content, raw_excerpt_value, summary_strategy, summary_model) =
            match decision {
                SummaryDecision::Raw => {
                    let content: String =
                        extracted.markdown.chars().take(request.max_chars).collect();
                    (
                        FetchContentKind::RawMarkdown,
                        content,
                        None,
                        FetchSummaryStrategy::NotSummarized,
                        None,
                    )
                }
                SummaryDecision::Summarize(strategy) => {
                    let summary = summarize_markdown(
                        context,
                        &final_url,
                        extracted.title.as_deref(),
                        &extracted.markdown,
                        request.max_chars,
                    )
                    .await?;
                    (
                        FetchContentKind::Summary,
                        summary,
                        Some(raw_excerpt(&extracted.markdown)),
                        strategy,
                        Some(context.summarizer_model.clone()),
                    )
                }
                SummaryDecision::Refuse => return Err(FetchError::PageTooLarge),
            };

        return Ok(FetchResponse {
            provider: DIRECT_HTTP_PROVIDER_ID.to_string(),
            url: original_url,
            final_url,
            title: extracted.title,
            format: "markdown".to_string(),
            extraction: EXTRACTION_READABILITYRS.to_string(),
            content_kind,
            returned_chars: content.chars().count(),
            content,
            raw_excerpt: raw_excerpt_value,
            raw_chars,
            summary_model,
            summary_strategy,
            truncated: false,
        });
    }

    Err(FetchError::TooManyRedirects)
}

async fn read_limited_body(response: reqwest::Response) -> Result<Vec<u8>, FetchError> {
    let mut stream = response.bytes_stream();
    let mut body = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(map_reqwest_error)?;
        if body.len() + chunk.len() > MAX_RESPONSE_BYTES {
            return Err(FetchError::ResponseTooLarge);
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

fn map_reqwest_error(error: reqwest::Error) -> FetchError {
    if error.is_timeout() {
        FetchError::Timeout
    } else {
        FetchError::Http
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        provider::{GenerateRequest, GenerateResponse, GenerateStreamEvent, ProviderError},
        web_fetch::types::{FetchContentKind, FetchRuntimeContext, FetchSummaryStrategy},
    };
    use std::{future::Future, pin::Pin, sync::Arc};
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };

    #[tokio::test]
    async fn fetches_html_and_extracts_markdown() {
        let body = "<html><head><title>Rust</title></head><body><article><h1>Rust</h1><p>Fast and reliable systems programming for everyone.</p><p>It helps teams build dependable software with confidence.</p></article></body></html>";
        let url = serve_once(&format!(
            "HTTP/1.1 200 OK\r\ncontent-type: text/html\r\ncontent-length: {}\r\n\r\n{}",
            body.len(),
            body
        ))
        .await;
        let request = FetchRequest {
            url,
            reason: None,
            max_chars: 20_000,
        };
        let context = test_context();
        let response = fetch_direct_http_unchecked_initial_url(&test_client(), &request, &context)
            .await
            .expect("fetch");

        assert_eq!(response.provider, DIRECT_HTTP_PROVIDER_ID);
        assert_eq!(response.content_kind, FetchContentKind::RawMarkdown);
        assert_eq!(
            response.summary_strategy,
            FetchSummaryStrategy::NotSummarized
        );
        assert_eq!(response.title.as_deref(), Some("Rust"));
        assert!(response.raw_chars > 0);
        assert!(!response.content.trim().is_empty());
    }

    #[tokio::test]
    async fn rejects_redirect_to_blocked_target() {
        let url = serve_once(
            "HTTP/1.1 302 Found\r\nlocation: http://127.0.0.1/private\r\ncontent-length: 0\r\n\r\n",
        )
        .await;
        let request = FetchRequest {
            url,
            reason: None,
            max_chars: 20_000,
        };
        let error =
            fetch_direct_http_unchecked_initial_url(&test_client(), &request, &test_context())
                .await
                .expect_err("blocked redirect");

        assert!(matches!(error, FetchError::RedirectBlocked));
    }

    #[tokio::test]
    async fn direct_http_client_rejects_blocked_redirect_without_auto_following() {
        let url = serve_once(
            "HTTP/1.1 302 Found\r\nlocation: http://127.0.0.1/private\r\ncontent-length: 0\r\n\r\n",
        )
        .await;
        let request = FetchRequest {
            url,
            reason: None,
            max_chars: 20_000,
        };
        let error = fetch_direct_http_unchecked_initial_url(
            &DirectHttpClient::default(),
            &request,
            &test_context(),
        )
        .await
        .expect_err("blocked redirect");

        assert!(matches!(error, FetchError::RedirectBlocked));
    }

    #[tokio::test]
    async fn rejects_response_body_over_byte_cap() {
        let body = "x".repeat(MAX_RESPONSE_BYTES + 1);
        let url = serve_once(&format!(
            "HTTP/1.1 200 OK\r\ncontent-type: text/plain\r\ncontent-length: {}\r\n\r\n{}",
            body.len(),
            body
        ))
        .await;
        let request = FetchRequest {
            url,
            reason: None,
            max_chars: 20_000,
        };
        let error =
            fetch_direct_http_unchecked_initial_url(&test_client(), &request, &test_context())
                .await
                .expect_err("size cap");

        assert!(matches!(error, FetchError::ResponseTooLarge));
    }

    #[tokio::test]
    async fn summarizes_large_plain_text() {
        let body = "large page sentence.\n".repeat(450);
        let url = serve_once(&format!(
            "HTTP/1.1 200 OK\r\ncontent-type: text/plain\r\ncontent-length: {}\r\n\r\n{}",
            body.len(),
            body
        ))
        .await;
        let request = FetchRequest {
            url,
            reason: None,
            max_chars: 20_000,
        };
        let response =
            fetch_direct_http_unchecked_initial_url(&test_client(), &request, &test_context())
                .await
                .expect("summary");

        assert_eq!(response.content_kind, FetchContentKind::Summary);
        assert_eq!(response.summary_strategy, FetchSummaryStrategy::SinglePass);
        assert_eq!(response.content, "summary");
        assert!(response.raw_excerpt.is_some());
    }

    #[tokio::test]
    async fn rejects_unsupported_content_type() {
        let url = serve_once(
            "HTTP/1.1 200 OK\r\ncontent-type: application/octet-stream\r\ncontent-length: 4\r\n\r\nnope",
        )
        .await;
        let request = FetchRequest {
            url,
            reason: None,
            max_chars: 20_000,
        };
        let error =
            fetch_direct_http_unchecked_initial_url(&test_client(), &request, &test_context())
                .await
                .expect_err("content type rejected");

        assert!(matches!(error, FetchError::UnsupportedContentType));
    }

    async fn serve_once(response: &str) -> String {
        let response = response.to_string();
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("accept");
            let mut buffer = [0_u8; 2048];
            let _ = stream.read(&mut buffer).await;
            stream.write_all(response.as_bytes()).await.expect("write");
        });
        format!("http://{addr}/")
    }

    fn test_context() -> FetchRuntimeContext {
        FetchRuntimeContext {
            summarizer_provider_kind: "codex".to_string(),
            summarizer_provider: Arc::new(StaticSummaryProvider),
            summarizer_model: "gpt-5.4-mini".to_string(),
        }
    }

    fn test_client() -> DirectHttpClient {
        DirectHttpClient::default()
    }

    #[derive(Debug)]
    struct StaticSummaryProvider;

    impl crate::daemon::RuntimeModelProvider for StaticSummaryProvider {
        fn generate_streaming<'a>(
            &'a self,
            request: GenerateRequest,
            on_event: &'a mut (dyn FnMut(GenerateStreamEvent) + Send),
        ) -> Pin<Box<dyn Future<Output = Result<GenerateResponse, ProviderError>> + Send + 'a>>
        {
            let _ = (request, on_event);
            Box::pin(async {
                Ok(GenerateResponse::final_text(
                    "summary",
                    "codex",
                    "gpt-5.4-mini",
                ))
            })
        }
    }
}
