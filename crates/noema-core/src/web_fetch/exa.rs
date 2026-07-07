//! Exa hosted web fetch provider.

use crate::web_fetch::{
    tool::sanitized_web_fetch_display_url,
    types::{FetchContentKind, FetchError, FetchRequest, FetchResponse, FetchSummaryStrategy},
    url_policy::validate_public_web_fetch_url,
};
use serde::Serialize;
use serde_json::Value;

pub const EXA_FETCH_PROVIDER_ID: &str = "exa";
pub const EXA_EXTRACTION: &str = "exa_contents";

#[derive(Debug, Clone)]
pub struct ExaFetchClient {
    pub base_url: String,
    pub api_key: String,
    pub http: reqwest::Client,
}

#[derive(Debug, Serialize)]
struct ExaContentsRequest<'a> {
    urls: [&'a str; 1],
    text: bool,
}

pub async fn fetch_exa(
    client: &ExaFetchClient,
    request: &FetchRequest,
) -> Result<FetchResponse, FetchError> {
    validate_public_web_fetch_url(&request.url).await?;
    let response = client
        .http
        .post(format!(
            "{}/contents",
            client.base_url.trim_end_matches('/')
        ))
        .header("x-api-key", &client.api_key)
        .json(&ExaContentsRequest {
            urls: [&request.url],
            text: true,
        })
        .send()
        .await
        .map_err(map_reqwest_error)?;
    if !response.status().is_success() {
        return Err(match response.status() {
            reqwest::StatusCode::UNAUTHORIZED | reqwest::StatusCode::FORBIDDEN => {
                FetchError::AuthFailed
            }
            reqwest::StatusCode::REQUEST_TIMEOUT | reqwest::StatusCode::GATEWAY_TIMEOUT => {
                FetchError::Timeout
            }
            _ => FetchError::Http,
        });
    }
    let value: Value = response.json().await.map_err(|_| FetchError::Extraction)?;
    normalize_exa_contents_response(&request.url, request.max_chars, &value)
}

pub fn normalize_exa_contents_response(
    requested_url: &str,
    max_chars: usize,
    value: &Value,
) -> Result<FetchResponse, FetchError> {
    let result = value
        .get("results")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .ok_or(FetchError::Extraction)?;
    let final_url = result
        .get("url")
        .and_then(Value::as_str)
        .unwrap_or(requested_url)
        .to_string();
    let title = result
        .get("title")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string);
    let raw_content = result
        .get("text")
        .and_then(Value::as_str)
        .ok_or(FetchError::Extraction)?;
    let raw_chars = raw_content.chars().count();
    let content = raw_content.chars().take(max_chars).collect::<String>();
    let returned_chars = content.chars().count();

    Ok(FetchResponse {
        provider: EXA_FETCH_PROVIDER_ID.to_string(),
        url: sanitized_web_fetch_display_url(requested_url),
        final_url,
        title,
        format: "markdown".to_string(),
        extraction: EXA_EXTRACTION.to_string(),
        content_kind: FetchContentKind::RawMarkdown,
        content,
        raw_excerpt: None,
        raw_chars,
        returned_chars,
        summary_model: None,
        summary_strategy: FetchSummaryStrategy::NotSummarized,
        truncated: raw_chars > returned_chars,
    })
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
    use crate::web_fetch::types::FetchRequest;
    use serde_json::json;
    use tokio::sync::mpsc;

    #[test]
    fn normalizes_exa_contents_response() {
        let value = json!({
            "results": [
                {
                    "title": "Rust",
                    "url": "https://www.rust-lang.org/",
                    "text": "# Rust\nFast and reliable."
                }
            ]
        });

        let response = normalize_exa_contents_response("https://www.rust-lang.org/", 30, &value)
            .expect("fetch");

        assert_eq!(response.provider, "exa");
        assert_eq!(response.url, "https://www.rust-lang.org/");
        assert_eq!(response.final_url, "https://www.rust-lang.org/");
        assert_eq!(response.title.as_deref(), Some("Rust"));
        assert_eq!(response.content, "# Rust\nFast and reliable.");
        assert_eq!(response.extraction, "exa_contents");
        assert!(!response.truncated);
    }

    #[tokio::test]
    async fn sends_contents_request_with_api_key() {
        let (base_url, mut request_rx) = spawn_server(
            200,
            r#"{"results":[{"title":"Rust","url":"https://www.rust-lang.org/","text":"Rust"}]}"#,
        )
        .await;
        let client = ExaFetchClient {
            base_url,
            api_key: "secret".to_string(),
            http: reqwest::Client::new(),
        };

        let response = fetch_exa(
            &client,
            &FetchRequest {
                url: "https://www.rust-lang.org/".to_string(),
                reason: None,
                max_chars: 8_000,
            },
        )
        .await
        .expect("fetch");

        let request = request_rx.recv().await.expect("request");
        assert_eq!(request.method, "POST");
        assert_eq!(request.path, "/contents");
        assert_eq!(request.x_api_key.as_deref(), Some("secret"));
        assert!(
            request
                .body
                .contains("\"urls\":[\"https://www.rust-lang.org/\"]")
        );
        assert!(request.body.contains("\"text\":true"));
        assert_eq!(response.content, "Rust");
    }

    #[derive(Debug)]
    struct CapturedRequest {
        method: String,
        path: String,
        x_api_key: Option<String>,
        body: String,
    }

    async fn spawn_server(
        status: u16,
        body: &'static str,
    ) -> (String, mpsc::Receiver<CapturedRequest>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind");
        let address = listener.local_addr().expect("local addr");
        let (request_tx, request_rx) = mpsc::channel(1);

        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("accept");
            let mut buffer = vec![0_u8; 8192];
            let read = tokio::io::AsyncReadExt::read(&mut stream, &mut buffer)
                .await
                .expect("read request");
            let request_text = String::from_utf8(buffer[..read].to_vec()).expect("utf8 request");
            let (head, body_text) = request_text
                .split_once("\r\n\r\n")
                .expect("request separator");
            let mut lines = head.lines();
            let request_line = lines.next().expect("request line");
            let mut request_parts = request_line.split_whitespace();
            let method = request_parts.next().unwrap_or_default().to_string();
            let path = request_parts.next().unwrap_or_default().to_string();
            let x_api_key = lines.find_map(|line| {
                line.split_once(':').and_then(|(name, value)| {
                    name.eq_ignore_ascii_case("x-api-key")
                        .then(|| value.trim().to_string())
                })
            });
            request_tx
                .send(CapturedRequest {
                    method,
                    path,
                    x_api_key,
                    body: body_text.to_string(),
                })
                .await
                .expect("send request");

            let response = format!(
                "HTTP/1.1 {status} OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            tokio::io::AsyncWriteExt::write_all(&mut stream, response.as_bytes())
                .await
                .expect("write response");
        });

        (format!("http://{}", address), request_rx)
    }
}
