//! Direct Rust HTTP implementation for `web.fetch`.

use crate::web_fetch::{
    extraction::{extract_readable_content, normalize_plain_text},
    summarize::{SummaryDecision, raw_excerpt, summarize_markdown, summary_strategy_for_chars},
    types::{DIRECT_HTTP_PROVIDER_ID, EXTRACTION_READABILITYRS, FetchError, FetchRuntimeContext},
    url_policy::{CheckedUrl, validate_public_web_fetch_url, validate_public_web_fetch_url_parsed},
};
use futures_util::StreamExt;
use noema_capabilities::web::fetch::{
    FetchContentKind, FetchRequest, FetchResponse, FetchSummaryStrategy,
};
use reqwest::{Client, StatusCode, header};
#[cfg(test)]
use std::net::SocketAddr;
use std::time::Duration;
#[cfg(not(test))]
use url::Host;
#[cfg(test)]
use url::{Host, Url};

const MAX_REDIRECTS: usize = 3;
const MAX_RESPONSE_BYTES: usize = 5 * 1024 * 1024;
const USER_AGENT: &str = "NoemaWebFetch/0.1 (+https://github.com/kpsuperplane/Noema)";

#[derive(Debug, Clone)]
pub struct DirectHttpClient {
    request_timeout: Duration,
}

impl Default for DirectHttpClient {
    fn default() -> Self {
        Self {
            request_timeout: Duration::from_secs(30),
        }
    }
}

impl DirectHttpClient {
    fn client_for_checked_url(&self, checked: &CheckedUrl) -> Result<Client, FetchError> {
        let mut builder = Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(self.request_timeout);
        if matches!(checked.url.host(), Some(Host::Domain(_))) {
            let host = checked.url.host_str().ok_or(FetchError::MalformedUrl)?;
            builder = builder.resolve_to_addrs(host, &checked.resolved_addrs);
        }
        builder.build().map_err(|_| FetchError::Http)
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
    let port = url
        .port_or_known_default()
        .ok_or(FetchError::MalformedUrl)?;
    let ip = url
        .host()
        .and_then(|host| match host {
            Host::Ipv4(ip) => Some(ip.into()),
            Host::Ipv6(ip) => Some(ip.into()),
            Host::Domain(host) => host.parse().ok(),
        })
        .ok_or(FetchError::MalformedUrl)?;
    let checked = CheckedUrl {
        url,
        resolved_addrs: vec![SocketAddr::new(ip, port)],
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
        let request_client = client.client_for_checked_url(&checked)?;
        let response = request_client
            .get(checked.url.clone())
            .header(header::USER_AGENT, USER_AGENT)
            .header(
                header::ACCEPT,
                "text/html,text/plain,text/markdown;q=0.9,*/*;q=0.1",
            )
            .header(header::ACCEPT_LANGUAGE, "en-US,en;q=0.9")
            .timeout(client.request_timeout)
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
        let (content_kind, content, raw_excerpt_value, summary_strategy, summary_model, truncated) =
            match decision {
                SummaryDecision::Raw => {
                    let content: String =
                        extracted.markdown.chars().take(request.max_chars).collect();
                    let truncated = raw_chars > content.chars().count();
                    (
                        FetchContentKind::RawMarkdown,
                        content,
                        None,
                        FetchSummaryStrategy::NotSummarized,
                        None,
                        truncated,
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
                        false,
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
            truncated,
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
    use crate::web_fetch::types::FetchRuntimeContext;
    use noema_capabilities::web::fetch::{FetchContentKind, FetchSummaryStrategy};
    use noema_providers::{GenerateRequest, GenerateResponse, GenerateStreamEvent, ProviderError};
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
    async fn direct_http_client_pins_domain_requests_to_checked_addresses() {
        let body = "<html><head><title>Pinned</title></head><body><article><h1>Pinned DNS</h1><p>Fetched through the pre-vetted socket address.</p></article></body></html>";
        let addr = serve_once_addr(&format!(
            "HTTP/1.1 200 OK\r\ncontent-type: text/html\r\ncontent-length: {}\r\n\r\n{}",
            body.len(),
            body
        ))
        .await;
        let request = FetchRequest {
            url: format!("http://example.com:{}/", addr.port()),
            reason: None,
            max_chars: 20_000,
        };
        let checked = CheckedUrl {
            url: Url::parse(&request.url).expect("url"),
            resolved_addrs: vec![addr],
        };
        let response = fetch_direct_http_checked(
            &DirectHttpClient::default(),
            &request,
            &test_context(),
            checked,
        )
        .await
        .expect("fetch through pinned address");

        assert_eq!(response.title.as_deref(), Some("Pinned"));
        assert_eq!(response.final_url, request.url);
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
    async fn extracts_readable_article_from_large_html_shell() {
        let shell = "var ignored = 1;\n".repeat(60_000);
        let body = format!(
            "<html><head><title>Readable</title><script>{shell}</script></head><body><article><h1>Readable Article</h1><p>This is the useful page text.</p></article></body></html>"
        );
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
        let response =
            fetch_direct_http_unchecked_initial_url(&test_client(), &request, &test_context())
                .await
                .expect("fetch");

        assert_eq!(response.content_kind, FetchContentKind::RawMarkdown);
        assert_eq!(response.title.as_deref(), Some("Readable"));
        assert!(response.content.contains("Readable Article"));
        assert!(response.content.contains("useful page text"));
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
    async fn marks_raw_markdown_truncated_when_limited_by_max_chars() {
        let body = "plain text content that is long enough to truncate";
        let url = serve_once(&format!(
            "HTTP/1.1 200 OK\r\ncontent-type: text/plain\r\ncontent-length: {}\r\n\r\n{}",
            body.len(),
            body
        ))
        .await;
        let request = FetchRequest {
            url,
            reason: None,
            max_chars: 10,
        };
        let response =
            fetch_direct_http_unchecked_initial_url(&test_client(), &request, &test_context())
                .await
                .expect("fetch");

        assert_eq!(response.content, "plain text");
        assert!(response.truncated);
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
        let addr = serve_once_addr(response).await;
        format!("http://{addr}/")
    }

    async fn serve_once_addr(response: &str) -> SocketAddr {
        let response = response.to_string();
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("accept");
            let mut buffer = [0_u8; 2048];
            let _ = stream.read(&mut buffer).await;
            stream.write_all(response.as_bytes()).await.expect("write");
        });
        addr
    }

    fn test_context() -> FetchRuntimeContext {
        FetchRuntimeContext {
            summarizer_provider_kind: "codex".to_string(),
            summarizer_route: crate::test_support::provider_route(
                noema_providers::ProviderSelectionSnapshot::explicit(
                    "codex",
                    "provider_account:codex:web-fetch-test",
                    "gpt-5.4-mini",
                    None,
                    Some("web_fetch_test".to_string()),
                ),
                Arc::new(StaticSummaryProvider),
            ),
            summarizer_model: "gpt-5.4-mini".to_string(),
            summarizer_reasoning_effort: None,
            generation_priority: noema_providers::GenerationPriority::Foreground,
        }
    }

    fn test_client() -> DirectHttpClient {
        DirectHttpClient::default()
    }

    #[derive(Debug)]
    struct StaticSummaryProvider;

    impl noema_providers::ProviderOperations for StaticSummaryProvider {
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
