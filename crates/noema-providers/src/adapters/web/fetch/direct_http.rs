//! Direct Rust HTTP implementation for `web.fetch`.

use super::{
    extraction::extract_readable_content,
    summarize::{SummaryDecision, raw_excerpt, summarize_markdown, summary_strategy_for_chars},
    url_policy::{CheckedUrl, validate_public_web_fetch_url, validate_public_web_fetch_url_parsed},
};
use crate::{
    DIRECT_HTTP_PROVIDER_ID, EXTRACTION_READABILITYRS, WebFetchBackend, WebFetchBackendHandle,
    WebFetchContext, WebFetchError, WebOperationFuture,
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

/// Checked direct-HTTP web fetch backend.
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
    fn client_for_checked_url(&self, checked: &CheckedUrl) -> Result<Client, WebFetchError> {
        let mut builder = Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(self.request_timeout);
        if matches!(checked.url.host(), Some(Host::Domain(_))) {
            let host = checked.url.host_str().ok_or(WebFetchError::MalformedUrl)?;
            builder = builder.resolve_to_addrs(host, &checked.resolved_addrs);
        }
        builder.build().map_err(|_| WebFetchError::Http)
    }
}

impl WebFetchBackend for DirectHttpClient {
    fn backend_id(&self) -> &str {
        DIRECT_HTTP_PROVIDER_ID
    }

    fn fetch<'a>(
        &'a self,
        request: &'a FetchRequest,
        context: &'a WebFetchContext,
    ) -> WebOperationFuture<'a, FetchResponse, WebFetchError> {
        Box::pin(fetch_direct_http(self, request, context))
    }
}

/// Build the default checked direct-HTTP backend handle.
#[must_use]
pub fn default_runtime_provider() -> WebFetchBackendHandle {
    WebFetchBackendHandle::new(DirectHttpClient::default())
}

async fn fetch_direct_http(
    client: &DirectHttpClient,
    request: &FetchRequest,
    context: &WebFetchContext,
) -> Result<FetchResponse, WebFetchError> {
    let checked = validate_public_web_fetch_url(&request.url).await?;
    fetch_direct_http_checked(client, request, context, checked).await
}

#[cfg(test)]
async fn fetch_direct_http_unchecked_initial_url(
    client: &DirectHttpClient,
    request: &FetchRequest,
    context: &WebFetchContext,
) -> Result<FetchResponse, WebFetchError> {
    let url = Url::parse(&request.url).map_err(|_| WebFetchError::MalformedUrl)?;
    let port = url
        .port_or_known_default()
        .ok_or(WebFetchError::MalformedUrl)?;
    let ip = url
        .host()
        .and_then(|host| match host {
            Host::Ipv4(ip) => Some(ip.into()),
            Host::Ipv6(ip) => Some(ip.into()),
            Host::Domain(host) => host.parse().ok(),
        })
        .ok_or(WebFetchError::MalformedUrl)?;
    let checked = CheckedUrl {
        url,
        resolved_addrs: vec![SocketAddr::new(ip, port)],
    };
    fetch_direct_http_checked(client, request, context, checked).await
}

async fn fetch_direct_http_checked(
    client: &DirectHttpClient,
    request: &FetchRequest,
    context: &WebFetchContext,
    mut checked: CheckedUrl,
) -> Result<FetchResponse, WebFetchError> {
    let original_url = checked.url.to_string();

    for redirect_count in 0..=MAX_REDIRECTS {
        let request_client = client.client_for_checked_url(&checked)?;
        let response = request_client
            .get(checked.url.clone())
            .header(header::USER_AGENT, USER_AGENT)
            .header(
                header::ACCEPT,
                "text/html,text/*;q=0.9,application/json;q=0.8,application/xml;q=0.8,*/*;q=0.1",
            )
            .header(header::ACCEPT_LANGUAGE, "en-US,en;q=0.9")
            .timeout(client.request_timeout)
            .send()
            .await
            .map_err(map_reqwest_error)?;

        if response.status().is_redirection() {
            if redirect_count == MAX_REDIRECTS {
                return Err(WebFetchError::TooManyRedirects);
            }
            let location = response
                .headers()
                .get(header::LOCATION)
                .and_then(|value| value.to_str().ok())
                .ok_or(WebFetchError::Http)?;
            let next_url = checked
                .url
                .join(location)
                .map_err(|_| WebFetchError::MalformedUrl)?;
            checked = validate_public_web_fetch_url_parsed(next_url)
                .await
                .map_err(|error| match error {
                    WebFetchError::BlockedTarget | WebFetchError::UnsupportedScheme => {
                        WebFetchError::RedirectBlocked
                    }
                    other => other,
                })?;
            continue;
        }

        if !response.status().is_success() {
            return Err(match response.status() {
                StatusCode::REQUEST_TIMEOUT | StatusCode::GATEWAY_TIMEOUT => WebFetchError::Timeout,
                _ => WebFetchError::Http,
            });
        }

        let content_type = response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("")
            .to_ascii_lowercase();
        let media_type = content_type.split(';').next().unwrap_or_default().trim();
        let is_html = matches!(media_type, "" | "text/html" | "application/xhtml+xml");
        let is_text = !is_html && is_supported_text_media_type(media_type);
        if !is_html && !is_text {
            return Err(WebFetchError::UnsupportedContentType);
        }

        let final_url = checked.url.to_string();
        if is_text {
            let (content, raw_chars, truncated) =
                read_text_prefix(response, request.max_chars).await?;
            return Ok(FetchResponse {
                provider: DIRECT_HTTP_PROVIDER_ID.to_string(),
                url: original_url,
                final_url,
                title: None,
                links: Vec::new(),
                format: media_type.to_string(),
                extraction: "none".to_string(),
                content_kind: FetchContentKind::RawText,
                returned_chars: content.chars().count(),
                content,
                raw_excerpt: None,
                raw_chars,
                summary_model: None,
                summary_strategy: FetchSummaryStrategy::NotSummarized,
                truncated,
            });
        }
        let body = read_limited_body(response).await?;
        let body_text = String::from_utf8_lossy(&body).to_string();
        let extracted = extract_readable_content(&body_text, &final_url)?;
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
                SummaryDecision::Refuse => return Err(WebFetchError::PageTooLarge),
            };

        return Ok(FetchResponse {
            provider: DIRECT_HTTP_PROVIDER_ID.to_string(),
            url: original_url,
            final_url,
            title: extracted.title,
            links: extracted.links,
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

    Err(WebFetchError::TooManyRedirects)
}

fn is_supported_text_media_type(media_type: &str) -> bool {
    media_type.starts_with("text/")
        || matches!(
            media_type,
            "application/markdown" | "application/json" | "application/xml"
        )
        || media_type.ends_with("+json")
        || media_type.ends_with("+xml")
}

async fn read_text_prefix(
    response: reqwest::Response,
    max_chars: usize,
) -> Result<(String, usize, bool), WebFetchError> {
    let byte_limit = max_chars
        .saturating_add(1)
        .saturating_mul(4)
        .saturating_add(3);
    let mut stream = response.bytes_stream();
    let mut body = Vec::with_capacity(byte_limit.min(64 * 1024));
    let mut source_has_more_bytes = false;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(map_reqwest_error)?;
        let remaining = byte_limit.saturating_sub(body.len());
        if chunk.len() > remaining {
            body.extend_from_slice(&chunk[..remaining]);
            source_has_more_bytes = true;
            break;
        }
        body.extend_from_slice(&chunk);
        if std::str::from_utf8(&body).is_ok_and(|text| text.chars().count() > max_chars) {
            break;
        }
    }

    let text = match std::str::from_utf8(&body) {
        Ok(text) => text,
        Err(error) if error.error_len().is_none() && source_has_more_bytes => {
            std::str::from_utf8(&body[..error.valid_up_to()])
                .map_err(|_| WebFetchError::InvalidTextEncoding)?
        }
        Err(_) => return Err(WebFetchError::InvalidTextEncoding),
    };
    let raw_chars = text.chars().count();
    let content = text.chars().take(max_chars).collect();
    Ok((
        content,
        raw_chars,
        source_has_more_bytes || raw_chars > max_chars,
    ))
}

async fn read_limited_body(response: reqwest::Response) -> Result<Vec<u8>, WebFetchError> {
    let mut stream = response.bytes_stream();
    let mut body = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(map_reqwest_error)?;
        if body.len() + chunk.len() > MAX_RESPONSE_BYTES {
            return Err(WebFetchError::ResponseTooLarge);
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

fn map_reqwest_error(error: reqwest::Error) -> WebFetchError {
    if error.is_timeout() {
        WebFetchError::Timeout
    } else {
        WebFetchError::Http
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{GenerateRequest, GenerateResponse, GenerateStreamEvent, ProviderError};
    use noema_capabilities::web::fetch::{FetchContentKind, FetchSummaryStrategy};
    use std::{env, future::Future, pin::Pin, sync::Arc};
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
        process::Command,
    };

    const PROXY_BYPASS_CHILD_ENV: &str = "NOEMA_DIRECT_HTTP_PROXY_BYPASS_CHILD";

    #[tokio::test]
    async fn fetches_html_and_extracts_markdown() {
        let body = "<html><head><title>Rust</title></head><body><article><h1>Rust</h1><p>Fast and reliable systems programming for everyone.</p><p>It helps teams build dependable software with confidence.</p><a href='https://example.com/docs#part'>Documentation</a></article></body></html>";
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
        assert_eq!(response.links, ["https://example.com/docs"]);
        assert!(response.raw_chars > 0);
        assert!(!response.content.trim().is_empty());
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

        assert!(matches!(error, WebFetchError::RedirectBlocked));
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
    async fn direct_http_client_bypasses_environment_proxies_for_pinned_requests() {
        if env::var_os(PROXY_BYPASS_CHILD_ENV).is_some() {
            assert_pinned_request_bypasses_environment_proxy().await;
            return;
        }

        let proxy_body = "<html><head><title>Proxy</title></head><body>proxy</body></html>";
        let proxy_addr = serve_once_addr(&format!(
            "HTTP/1.1 200 OK\r\ncontent-type: text/html\r\ncontent-length: {}\r\n\r\n{}",
            proxy_body.len(),
            proxy_body
        ))
        .await;
        let proxy_url = format!("http://{proxy_addr}");
        let output = Command::new(env::current_exe().expect("current test executable"))
            .arg("direct_http_client_bypasses_environment_proxies_for_pinned_requests")
            .arg("--nocapture")
            .arg("--test-threads=1")
            .env(PROXY_BYPASS_CHILD_ENV, "1")
            .env("HTTP_PROXY", &proxy_url)
            .env("http_proxy", &proxy_url)
            .env("HTTPS_PROXY", &proxy_url)
            .env("https_proxy", &proxy_url)
            .env("ALL_PROXY", &proxy_url)
            .env("all_proxy", &proxy_url)
            .env("NO_PROXY", "")
            .env("no_proxy", "")
            .output()
            .await
            .expect("run isolated proxy regression");

        assert!(
            output.status.success(),
            "isolated proxy regression failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[tokio::test]
    async fn rejects_html_response_body_over_byte_cap() {
        let body = "x".repeat(MAX_RESPONSE_BYTES + 1);
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
        let error =
            fetch_direct_http_unchecked_initial_url(&test_client(), &request, &test_context())
                .await
                .expect_err("size cap");

        assert!(matches!(error, WebFetchError::ResponseTooLarge));
    }

    #[tokio::test]
    async fn summarizes_large_html() {
        let body = format!(
            "<html><head><title>Large article</title></head><body><article><h1>Large article</h1><p>{}</p></article></body></html>",
            "large page sentence with useful source detail. ".repeat(450)
        );
        let url = serve_once(&format!(
            "HTTP/1.1 200 OK\r\ncontent-type: text/html\r\ncontent-length: {}\r\n\r\n{}",
            body.len(),
            body
        ))
        .await;
        let response = fetch_direct_http_unchecked_initial_url(
            &test_client(),
            &FetchRequest {
                url,
                reason: None,
                max_chars: 20_000,
            },
            &test_context(),
        )
        .await
        .expect("summary");

        assert_eq!(response.content_kind, FetchContentKind::Summary);
        assert_eq!(response.summary_strategy, FetchSummaryStrategy::SinglePass);
        assert_eq!(response.content, "summary");
        assert!(response.raw_excerpt.is_some());
    }

    #[tokio::test]
    async fn returns_bounded_csv_without_reading_the_complete_resource() {
        let body = "1,example.com\n2,example.org\n".repeat(10_000);
        let url = serve_once(&format!(
            "HTTP/1.1 200 OK\r\ncontent-type: text/csv\r\ncontent-length: {}\r\n\r\n{}",
            body.len(),
            body
        ))
        .await;
        let request = FetchRequest {
            url,
            reason: None,
            max_chars: 2_000,
        };
        let response =
            fetch_direct_http_unchecked_initial_url(&test_client(), &request, &test_context())
                .await
                .expect("fetch");

        assert_eq!(response.content_kind, FetchContentKind::RawText);
        assert_eq!(response.format, "text/csv");
        assert_eq!(response.extraction, "none");
        assert_eq!(response.returned_chars, 2_000);
        assert!(response.truncated);
    }

    #[tokio::test]
    async fn rejects_invalid_utf8_text() {
        let mut response =
            b"HTTP/1.1 200 OK\r\ncontent-type: text/csv\r\ncontent-length: 2\r\n\r\n".to_vec();
        response.extend_from_slice(&[0xff, 0xfe]);
        let url = serve_once_bytes(response).await;
        let error = fetch_direct_http_unchecked_initial_url(
            &test_client(),
            &FetchRequest {
                url,
                reason: None,
                max_chars: 2_000,
            },
            &test_context(),
        )
        .await
        .expect_err("invalid UTF-8");

        assert_eq!(error, WebFetchError::InvalidTextEncoding);
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

        assert!(matches!(error, WebFetchError::UnsupportedContentType));
    }

    async fn serve_once(response: &str) -> String {
        let addr = serve_once_addr(response).await;
        format!("http://{addr}/")
    }

    async fn serve_once_bytes(response: Vec<u8>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("accept");
            let mut buffer = [0_u8; 2048];
            let _ = stream.read(&mut buffer).await;
            stream.write_all(&response).await.expect("write");
        });
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

    async fn assert_pinned_request_bypasses_environment_proxy() {
        let target_body = "response from the DNS-pinned target";
        let target_addr = serve_once_addr(&format!(
            "HTTP/1.1 200 OK\r\ncontent-type: text/plain\r\ncontent-length: {}\r\n\r\n{}",
            target_body.len(),
            target_body
        ))
        .await;
        let request = FetchRequest {
            url: format!("http://proxy-bypass.test:{}/", target_addr.port()),
            reason: None,
            max_chars: 20_000,
        };
        let checked = CheckedUrl {
            url: Url::parse(&request.url).expect("url"),
            resolved_addrs: vec![target_addr],
        };

        let response = fetch_direct_http_checked(
            &DirectHttpClient::default(),
            &request,
            &test_context(),
            checked,
        )
        .await
        .expect("fetch pinned target without environment proxy");

        assert_eq!(response.content, target_body);
    }

    fn test_context() -> WebFetchContext {
        WebFetchContext {
            summarizer_route: super::super::summarize::test_provider_route(
                crate::ProviderSelectionSnapshot::explicit(
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
            summarizer_fast_mode: false,
            generation_priority: crate::GenerationPriority::Foreground,
        }
    }

    fn test_client() -> DirectHttpClient {
        DirectHttpClient::default()
    }

    #[derive(Debug)]
    struct StaticSummaryProvider;

    impl crate::ProviderOperations for StaticSummaryProvider {
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
