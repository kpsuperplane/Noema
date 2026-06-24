//! Provider adapter for the OpenAI Responses API.

use crate::provider::{
    GenerateInput, GenerateRequest, GenerateResponse, ModelProvider, ProviderError, TokenUsage,
};
use reqwest::{StatusCode, header::HeaderMap};
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Default request timeout for `OpenAI` calls.
pub const DEFAULT_OPENAI_TIMEOUT_SECONDS: u64 = 120;

/// Configuration for the `OpenAI` provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenAiProviderConfig {
    /// API key sent as a bearer token.
    pub api_key: String,
    /// Base URL for an OpenAI-compatible Responses API.
    pub base_url: String,
    /// Optional `OpenAI` organization id.
    pub organization_id: Option<String>,
    /// Optional `OpenAI` project id.
    pub project_id: Option<String>,
    /// Default model used when a request does not override it.
    pub default_model: String,
    /// Request timeout in seconds.
    pub timeout_seconds: u64,
}

/// Provider implementation backed by the `OpenAI` Responses API.
#[derive(Debug)]
pub struct OpenAiProvider {
    client: reqwest::Client,
    config: OpenAiProviderConfig,
}

impl OpenAiProvider {
    /// Build an `OpenAI` provider with a default reqwest client.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError`] when configuration is invalid or the HTTP
    /// client cannot be built.
    pub fn new(config: OpenAiProviderConfig) -> Result<Self, ProviderError> {
        let config = normalize_config(config)?;
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(config.timeout_seconds))
            .build()
            .map_err(|source| ProviderError::HttpFailure { source })?;

        Ok(Self { client, config })
    }

    /// Build an `OpenAI` provider with a caller-supplied reqwest client.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError::InvalidRequest`] or
    /// [`ProviderError::MissingCredentials`] when configuration validation
    /// fails.
    pub fn with_client(
        client: reqwest::Client,
        config: OpenAiProviderConfig,
    ) -> Result<Self, ProviderError> {
        let config = normalize_config(config)?;

        Ok(Self { client, config })
    }

    fn responses_url(&self) -> String {
        format!("{}/responses", self.config.base_url)
    }
}

fn normalize_config(
    mut config: OpenAiProviderConfig,
) -> Result<OpenAiProviderConfig, ProviderError> {
    if config.api_key.trim().is_empty() {
        return Err(ProviderError::MissingCredentials {
            provider: "openai".to_string(),
            credential: "api_key".to_string(),
        });
    }

    let base_url = config.base_url.trim().trim_end_matches('/').to_string();
    if base_url.is_empty() {
        return Err(ProviderError::InvalidRequest {
            message: "openai base URL cannot be empty".to_string(),
        });
    }

    if reqwest::Url::parse(&base_url).is_err() {
        return Err(ProviderError::InvalidRequest {
            message: "openai base URL must be an absolute URL".to_string(),
        });
    }

    let default_model = config.default_model.trim().to_string();
    if default_model.is_empty() {
        return Err(ProviderError::InvalidRequest {
            message: "openai default model cannot be empty".to_string(),
        });
    }

    if config.timeout_seconds == 0 {
        return Err(ProviderError::InvalidRequest {
            message: "openai timeout must be greater than zero seconds".to_string(),
        });
    }

    config.base_url = base_url;
    config.default_model = default_model;
    Ok(config)
}

impl ModelProvider for OpenAiProvider {
    async fn generate(&self, request: GenerateRequest) -> Result<GenerateResponse, ProviderError> {
        let GenerateInput::Text(input) = request.input;
        if input.trim().is_empty() {
            return Err(ProviderError::InvalidRequest {
                message: "input cannot be empty".to_string(),
            });
        }

        let model = request
            .model
            .filter(|model| !model.trim().is_empty())
            .unwrap_or_else(|| self.config.default_model.clone());

        if model.trim().is_empty() {
            return Err(ProviderError::InvalidRequest {
                message: "model cannot be empty".to_string(),
            });
        }

        let body = OpenAiResponsesRequest {
            model: model.clone(),
            input,
            instructions: request
                .instructions
                .filter(|instructions| !instructions.trim().is_empty()),
            max_output_tokens: request.options.max_output_tokens,
            temperature: request.options.temperature,
            store: false,
        };

        let mut builder = self
            .client
            .post(self.responses_url())
            .bearer_auth(&self.config.api_key)
            .json(&body);

        if let Some(organization_id) = self
            .config
            .organization_id
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            builder = builder.header("OpenAI-Organization", organization_id);
        }

        if let Some(project_id) = self
            .config
            .project_id
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            builder = builder.header("OpenAI-Project", project_id);
        }

        let response = builder
            .send()
            .await
            .map_err(|source| ProviderError::HttpFailure { source })?;
        let status = response.status();
        let request_id = request_id(response.headers());
        let body_text = response
            .text()
            .await
            .map_err(|source| ProviderError::HttpFailure { source })?;

        if !status.is_success() {
            return Err(error_from_status(status, request_id, &body_text));
        }

        let response: OpenAiResponsesResponse =
            serde_json::from_str(&body_text).map_err(|source| {
                ProviderError::MalformedResponse {
                    message: format!("failed to parse JSON: {source}"),
                }
            })?;

        let text = collect_output_text(&response)?;

        Ok(GenerateResponse {
            text,
            provider: "openai".to_string(),
            model: response.model.unwrap_or(model),
            response_id: response.id,
            usage: response.usage.map(Into::into),
        })
    }
}

#[derive(Debug, Serialize)]
struct OpenAiResponsesRequest {
    model: String,
    input: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    instructions: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_output_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    store: bool,
}

#[derive(Debug, Deserialize)]
struct OpenAiResponsesResponse {
    id: Option<String>,
    model: Option<String>,
    #[serde(default)]
    output: Vec<OpenAiOutputItem>,
    usage: Option<OpenAiUsage>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum OpenAiOutputItem {
    #[serde(rename = "message")]
    Message { content: Vec<OpenAiContent> },
    #[serde(other)]
    Other,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum OpenAiContent {
    #[serde(rename = "output_text")]
    OutputText { text: String },
    #[serde(rename = "refusal")]
    Refusal { refusal: String },
    #[serde(other)]
    Other,
}

#[derive(Debug, Deserialize)]
struct OpenAiUsage {
    #[serde(default, rename = "input_tokens")]
    input: u64,
    #[serde(default, rename = "output_tokens")]
    output: u64,
    #[serde(default, rename = "total_tokens")]
    total: u64,
}

impl From<OpenAiUsage> for TokenUsage {
    fn from(value: OpenAiUsage) -> Self {
        Self {
            input_tokens: value.input,
            output_tokens: value.output,
            total_tokens: value.total,
        }
    }
}

#[derive(Debug, Deserialize)]
struct OpenAiErrorResponse {
    error: Option<OpenAiErrorBody>,
}

#[derive(Debug, Deserialize)]
struct OpenAiErrorBody {
    message: Option<String>,
}

fn collect_output_text(response: &OpenAiResponsesResponse) -> Result<String, ProviderError> {
    let mut output = String::new();
    let mut refusals = Vec::new();

    for item in &response.output {
        let OpenAiOutputItem::Message { content } = item else {
            continue;
        };

        for content_item in content {
            match content_item {
                OpenAiContent::OutputText { text } => output.push_str(text),
                OpenAiContent::Refusal { refusal } => refusals.push(refusal.as_str()),
                OpenAiContent::Other => {}
            }
        }
    }

    if !output.is_empty() {
        return Ok(output);
    }

    if !refusals.is_empty() {
        return Err(ProviderError::ApiError {
            status: 200,
            message: refusals.join("\n"),
            request_id: response.id.clone(),
        });
    }

    Err(ProviderError::MalformedResponse {
        message: "response did not contain output_text".to_string(),
    })
}

fn error_from_status(
    status: StatusCode,
    request_id: Option<String>,
    body_text: &str,
) -> ProviderError {
    let message = serde_json::from_str::<OpenAiErrorResponse>(body_text)
        .ok()
        .and_then(|body| body.error)
        .and_then(|error| error.message)
        .filter(|message| !message.trim().is_empty())
        .unwrap_or_else(|| body_text.trim().to_string())
        .if_empty_then(|| status.canonical_reason().unwrap_or("API error").to_string());

    match status {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => ProviderError::AuthenticationFailure {
            message,
            request_id,
        },
        StatusCode::TOO_MANY_REQUESTS => ProviderError::RateLimit {
            message,
            request_id,
        },
        _ => ProviderError::ApiError {
            status: status.as_u16(),
            message,
            request_id,
        },
    }
}

fn request_id(headers: &HeaderMap) -> Option<String> {
    headers
        .get("x-request-id")
        .and_then(|value| value.to_str().ok())
        .map(ToString::to_string)
}

trait EmptyStringExt {
    fn if_empty_then(self, fallback: impl FnOnce() -> String) -> String;
}

impl EmptyStringExt for String {
    fn if_empty_then(self, fallback: impl FnOnce() -> String) -> String {
        if self.is_empty() { fallback() } else { self }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use std::collections::HashMap;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
        sync::oneshot,
    };

    #[tokio::test]
    async fn sends_expected_request_and_extracts_text() {
        let (base_url, request_rx) = spawn_server(
            200,
            r#"{
              "id": "resp_test",
              "model": "gpt-test",
              "output": [{
                "type": "message",
                "content": [
                  {"type": "output_text", "text": "Hello"},
                  {"type": "output_text", "text": ", world"}
                ]
              }],
              "usage": {
                "input_tokens": 2,
                "output_tokens": 3,
                "total_tokens": 5
              }
            }"#,
        )
        .await;

        let provider = OpenAiProvider::new(OpenAiProviderConfig {
            api_key: "secret".to_string(),
            base_url,
            organization_id: Some("org_test".to_string()),
            project_id: Some("proj_test".to_string()),
            default_model: "default-model".to_string(),
            timeout_seconds: DEFAULT_OPENAI_TIMEOUT_SECONDS,
        })
        .expect("provider");

        let response = provider
            .generate(GenerateRequest {
                model: Some("gpt-test".to_string()),
                input: GenerateInput::Text("Hello?".to_string()),
                instructions: Some("Be brief.".to_string()),
                options: crate::provider::GenerateOptions {
                    max_output_tokens: Some(32),
                    temperature: Some(0.4),
                },
            })
            .await
            .expect("response");

        let captured = request_rx.await.expect("captured request");
        assert_eq!(captured.method, "POST");
        assert_eq!(captured.path, "/responses");
        assert_eq!(
            captured.headers.get("authorization").map(String::as_str),
            Some("Bearer secret")
        );
        assert_eq!(
            captured
                .headers
                .get("openai-organization")
                .map(String::as_str),
            Some("org_test")
        );
        assert_eq!(
            captured.headers.get("openai-project").map(String::as_str),
            Some("proj_test")
        );

        let body: Value = serde_json::from_str(&captured.body).expect("json body");
        assert_eq!(body["model"], "gpt-test");
        assert_eq!(body["input"], "Hello?");
        assert_eq!(body["instructions"], "Be brief.");
        assert_eq!(body["store"], false);
        assert_eq!(body["max_output_tokens"], 32);
        assert_eq!(body["temperature"], 0.4);

        assert_eq!(response.text, "Hello, world");
        assert_eq!(response.provider, "openai");
        assert_eq!(response.model, "gpt-test");
        assert_eq!(response.response_id.as_deref(), Some("resp_test"));
        assert_eq!(
            response.usage,
            Some(TokenUsage {
                input_tokens: 2,
                output_tokens: 3,
                total_tokens: 5,
            })
        );
    }

    #[tokio::test]
    async fn maps_authentication_errors() {
        let (base_url, _request_rx) = spawn_server(
            401,
            r#"{"error":{"message":"bad key","type":"invalid_request_error"}}"#,
        )
        .await;

        let provider = test_provider(base_url);
        let error = provider
            .generate(GenerateRequest::text("hello"))
            .await
            .unwrap_err();

        assert!(matches!(
            error,
            ProviderError::AuthenticationFailure { message, .. } if message == "bad key"
        ));
    }

    #[tokio::test]
    async fn maps_rate_limit_errors() {
        let (base_url, _request_rx) =
            spawn_server(429, r#"{"error":{"message":"slow down"}}"#).await;

        let provider = test_provider(base_url);
        let error = provider
            .generate(GenerateRequest::text("hello"))
            .await
            .unwrap_err();

        assert!(matches!(
            error,
            ProviderError::RateLimit { message, .. } if message == "slow down"
        ));
    }

    #[tokio::test]
    async fn maps_api_errors() {
        let (base_url, _request_rx) =
            spawn_server(500, r#"{"error":{"message":"upstream broke"}}"#).await;

        let provider = test_provider(base_url);
        let error = provider
            .generate(GenerateRequest::text("hello"))
            .await
            .unwrap_err();

        assert!(matches!(
            error,
            ProviderError::ApiError { status: 500, message, .. } if message == "upstream broke"
        ));
    }

    #[tokio::test]
    async fn malformed_when_success_response_has_no_output_text() {
        let (base_url, _request_rx) = spawn_server(
            200,
            r#"{
              "id": "resp_test",
              "model": "gpt-test",
              "output": [{"type": "message", "content": []}]
            }"#,
        )
        .await;

        let provider = test_provider(base_url);
        let error = provider
            .generate(GenerateRequest::text("hello"))
            .await
            .unwrap_err();

        assert!(matches!(error, ProviderError::MalformedResponse { .. }));
    }

    #[tokio::test]
    async fn rejects_empty_input_before_http_call() {
        let provider = test_provider("http://127.0.0.1:1".to_string());
        let error = provider
            .generate(GenerateRequest::text("   "))
            .await
            .unwrap_err();

        assert!(matches!(error, ProviderError::InvalidRequest { .. }));
    }

    #[test]
    fn rejects_missing_api_key() {
        let error = OpenAiProvider::new(OpenAiProviderConfig {
            api_key: " ".to_string(),
            base_url: "http://127.0.0.1:1".to_string(),
            organization_id: None,
            project_id: None,
            default_model: "default-model".to_string(),
            timeout_seconds: DEFAULT_OPENAI_TIMEOUT_SECONDS,
        })
        .unwrap_err();

        assert!(matches!(error, ProviderError::MissingCredentials { .. }));
    }

    #[test]
    fn rejects_zero_timeout() {
        let error = OpenAiProvider::new(OpenAiProviderConfig {
            api_key: "secret".to_string(),
            base_url: "http://127.0.0.1:1".to_string(),
            organization_id: None,
            project_id: None,
            default_model: "default-model".to_string(),
            timeout_seconds: 0,
        })
        .unwrap_err();

        assert!(matches!(error, ProviderError::InvalidRequest { .. }));
    }

    fn test_provider(base_url: String) -> OpenAiProvider {
        OpenAiProvider::new(OpenAiProviderConfig {
            api_key: "secret".to_string(),
            base_url,
            organization_id: None,
            project_id: None,
            default_model: "default-model".to_string(),
            timeout_seconds: DEFAULT_OPENAI_TIMEOUT_SECONDS,
        })
        .expect("provider")
    }

    #[derive(Debug)]
    struct CapturedRequest {
        method: String,
        path: String,
        headers: HashMap<String, String>,
        body: String,
    }

    async fn spawn_server(
        status: u16,
        response_body: &'static str,
    ) -> (String, oneshot::Receiver<CapturedRequest>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("local addr");
        let (request_tx, request_rx) = oneshot::channel();

        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.expect("accept");
            let request = read_request(&mut socket).await;
            let _ = request_tx.send(request);

            let reason = match status {
                200 => "OK",
                401 => "Unauthorized",
                429 => "Too Many Requests",
                _ => "Error",
            };
            let response = format!(
                "HTTP/1.1 {status} {reason}\r\ncontent-type: application/json\r\nx-request-id: req_test\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                response_body.len(),
                response_body
            );
            socket
                .write_all(response.as_bytes())
                .await
                .expect("write response");
        });

        (format!("http://{addr}"), request_rx)
    }

    async fn read_request(socket: &mut tokio::net::TcpStream) -> CapturedRequest {
        let mut buffer = Vec::new();
        let header_end;

        loop {
            let mut chunk = [0_u8; 1024];
            let bytes_read = socket.read(&mut chunk).await.expect("read request");
            assert!(bytes_read > 0, "connection closed before headers");
            buffer.extend_from_slice(&chunk[..bytes_read]);

            if let Some(position) = find_header_end(&buffer) {
                header_end = position;
                break;
            }
        }

        let header_text = String::from_utf8(buffer[..header_end].to_vec()).expect("utf8 headers");
        let mut lines = header_text.split("\r\n");
        let request_line = lines.next().expect("request line");
        let mut request_parts = request_line.split_whitespace();
        let method = request_parts.next().expect("method").to_string();
        let path = request_parts.next().expect("path").to_string();

        let mut headers = HashMap::new();
        for line in lines {
            if let Some((name, value)) = line.split_once(':') {
                headers.insert(name.trim().to_ascii_lowercase(), value.trim().to_string());
            }
        }

        let content_length = headers
            .get("content-length")
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(0);
        let body_start = header_end + 4;
        let mut body = buffer[body_start..].to_vec();

        while body.len() < content_length {
            let mut chunk = vec![0_u8; content_length - body.len()];
            let bytes_read = socket.read(&mut chunk).await.expect("read body");
            assert!(bytes_read > 0, "connection closed before body");
            body.extend_from_slice(&chunk[..bytes_read]);
        }

        CapturedRequest {
            method,
            path,
            headers,
            body: String::from_utf8(body).expect("utf8 body"),
        }
    }

    fn find_header_end(buffer: &[u8]) -> Option<usize> {
        buffer.windows(4).position(|window| window == b"\r\n\r\n")
    }
}
