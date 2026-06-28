//! Provider adapter for Codex direct Responses API calls.

use std::{path::PathBuf, time::Duration};

use reqwest::header::HeaderMap;
use serde::Serialize;

use crate::{
    provider::{
        GenerateInput, GenerateRequest, GenerateResponse, GenerateStreamEvent, ModelProvider,
        ProviderError, output_items_from_text, required_output_items_from_text,
    },
    providers::{
        codex_oauth::{
            CODEX_ACCESS_TOKEN_REFRESH_SKEW_SECONDS, CodexOAuthClient, CodexOAuthConfig,
            CodexTokenStore, DEFAULT_CODEX_BASE_URL,
        },
        responses::{ResponsesTransport, normalize_base_url},
    },
};

/// Default Codex Responses model used when no override is supplied.
pub const DEFAULT_CODEX_MODEL: &str = "gpt-5.5";
/// Default request timeout for Codex Responses calls.
pub const DEFAULT_CODEX_TIMEOUT_SECONDS: u64 = 300;

/// Configuration for the Codex direct Responses provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodexProviderConfig {
    /// Base URL for the Codex Responses API.
    pub base_url: String,
    /// Optional default model.
    pub default_model: Option<String>,
    /// Request timeout in seconds.
    pub timeout_seconds: u64,
    /// Provider account home containing Noema-owned token state.
    pub account_home: Option<PathBuf>,
    /// OAuth endpoint configuration used for token refresh and login.
    pub oauth: CodexOAuthConfig,
}

impl Default for CodexProviderConfig {
    fn default() -> Self {
        Self {
            base_url: DEFAULT_CODEX_BASE_URL.to_string(),
            default_model: Some(DEFAULT_CODEX_MODEL.to_string()),
            timeout_seconds: DEFAULT_CODEX_TIMEOUT_SECONDS,
            account_home: None,
            oauth: CodexOAuthConfig::default(),
        }
    }
}

/// Provider implementation backed by Codex OAuth and direct Responses calls.
#[derive(Debug, Clone)]
pub struct CodexResponsesProvider {
    transport: ResponsesTransport,
    token_store: CodexTokenStore,
    oauth_client: CodexOAuthClient,
    config: CodexProviderConfig,
}

impl CodexResponsesProvider {
    /// Build a Codex provider with a default reqwest client.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError`] when configuration is invalid or the HTTP
    /// client cannot be built.
    pub fn new(config: CodexProviderConfig) -> Result<Self, ProviderError> {
        let config = normalize_config(config)?;
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(config.timeout_seconds))
            .build()
            .map_err(|source| ProviderError::HttpFailure { source })?;
        Self::with_client(client, config)
    }

    /// Build a Codex provider with a caller-supplied reqwest client.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError`] when configuration is invalid.
    pub fn with_client(
        client: reqwest::Client,
        config: CodexProviderConfig,
    ) -> Result<Self, ProviderError> {
        let config = normalize_config(config)?;
        let account_home =
            config
                .account_home
                .clone()
                .ok_or_else(|| ProviderError::InvalidRequest {
                    message: "codex account home is required".to_string(),
                })?;
        let transport = ResponsesTransport::new(client, config.base_url.clone())?;
        let token_store = CodexTokenStore::new(account_home);
        let oauth_client = CodexOAuthClient::new(config.oauth.clone())?;
        Ok(Self {
            transport,
            token_store,
            oauth_client,
            config,
        })
    }

    /// Return the configured token store.
    #[must_use]
    pub fn token_store(&self) -> &CodexTokenStore {
        &self.token_store
    }

    fn model_for_request(&self, model: Option<String>) -> Result<String, ProviderError> {
        let model = model
            .filter(|model| !model.trim().is_empty())
            .or_else(|| self.config.default_model.clone())
            .unwrap_or_else(|| DEFAULT_CODEX_MODEL.to_string());
        let model = model.trim().to_string();
        if model.is_empty() {
            return Err(ProviderError::InvalidRequest {
                message: "codex model cannot be empty".to_string(),
            });
        }
        Ok(model)
    }
}

fn normalize_config(mut config: CodexProviderConfig) -> Result<CodexProviderConfig, ProviderError> {
    config.base_url = normalize_base_url(config.base_url, "codex base URL")?;
    if config.timeout_seconds == 0 {
        return Err(ProviderError::InvalidRequest {
            message: "codex timeout must be greater than zero seconds".to_string(),
        });
    }
    config.default_model = config.default_model.and_then(|model| {
        let model = model.trim().to_string();
        (!model.is_empty()).then_some(model)
    });
    Ok(config)
}

#[derive(Debug, Serialize)]
struct CodexResponsesRequest {
    model: String,
    input: Vec<CodexInputMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    instructions: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_output_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    store: bool,
    stream: bool,
}

impl CodexResponsesRequest {
    fn text(
        model: String,
        input: String,
        instructions: Option<String>,
        max_output_tokens: Option<u32>,
        temperature: Option<f32>,
    ) -> Self {
        Self {
            model,
            input: vec![CodexInputMessage {
                role: "user",
                content: input,
            }],
            instructions,
            max_output_tokens,
            temperature,
            store: false,
            stream: true,
        }
    }
}

#[derive(Debug, Serialize)]
struct CodexInputMessage {
    role: &'static str,
    content: String,
}

impl CodexResponsesProvider {
    async fn generate_with_events(
        &self,
        request: GenerateRequest,
        on_event: &mut (dyn FnMut(GenerateStreamEvent) + Send),
    ) -> Result<GenerateResponse, ProviderError> {
        let GenerateInput::Text(input) = request.input;
        if input.trim().is_empty() {
            return Err(ProviderError::InvalidRequest {
                message: "input cannot be empty".to_string(),
            });
        }

        let model = self.model_for_request(request.model)?;
        let instructions = request
            .instructions
            .clone()
            .filter(|instructions| !instructions.trim().is_empty());
        let max_output_tokens = request.options.max_output_tokens;
        let temperature = request.options.temperature;
        let body = CodexResponsesRequest::text(
            model.clone(),
            input.clone(),
            instructions.clone(),
            max_output_tokens,
            temperature,
        );

        let access_token = self
            .token_store
            .access_token(&self.oauth_client, CODEX_ACCESS_TOKEN_REFRESH_SKEW_SECONDS)
            .await?;
        let response = match self
            .transport
            .send_streaming(&access_token, body, HeaderMap::new(), on_event)
            .await
        {
            Ok(response) => response,
            Err(ProviderError::AuthenticationFailure { .. }) => {
                let refreshed = self
                    .token_store
                    .refresh_access_token(&self.oauth_client)
                    .await?;
                let retry_body = CodexResponsesRequest::text(
                    model.clone(),
                    input,
                    instructions,
                    max_output_tokens,
                    temperature,
                );
                self.transport
                    .send_streaming(&refreshed, retry_body, HeaderMap::new(), on_event)
                    .await?
            }
            Err(error) => return Err(error),
        };
        let text = response.output_text()?;

        let output = if request.options.require_noema_response {
            required_output_items_from_text(text)?
        } else {
            output_items_from_text(text)?
        };

        Ok(GenerateResponse {
            output,
            provider: "codex".to_string(),
            model: response.model.unwrap_or(model),
            response_id: response.id,
            usage: response.usage.map(Into::into),
        })
    }
}

impl ModelProvider for CodexResponsesProvider {
    async fn generate(&self, request: GenerateRequest) -> Result<GenerateResponse, ProviderError> {
        self.generate_with_events(request, &mut |_| {}).await
    }

    async fn generate_streaming(
        &self,
        request: GenerateRequest,
        on_event: &mut (dyn FnMut(GenerateStreamEvent) + Send),
    ) -> Result<GenerateResponse, ProviderError> {
        self.generate_with_events(request, on_event).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::codex_oauth::CodexOAuthTokens;
    use serde_json::Value;
    use std::collections::HashMap;
    use tempfile::TempDir;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
        sync::oneshot,
    };

    #[test]
    fn rejects_missing_account_home() {
        let error = CodexResponsesProvider::new(CodexProviderConfig {
            account_home: None,
            ..CodexProviderConfig::default()
        })
        .unwrap_err();

        assert!(matches!(error, ProviderError::InvalidRequest { .. }));
    }

    #[test]
    fn accepts_noema_owned_token_store() {
        let dir = TempDir::new().expect("temp dir");
        let account_home = dir.path().join("providers/codex/default");
        let provider = CodexResponsesProvider::new(CodexProviderConfig {
            account_home: Some(account_home.clone()),
            ..CodexProviderConfig::default()
        })
        .expect("provider");

        provider
            .token_store()
            .write(&CodexOAuthTokens {
                access_token: "access".to_string(),
                refresh_token: "refresh".to_string(),
                last_refresh: 123,
            })
            .expect("write token");

        assert!(provider.token_store().has_usable_tokens());
    }

    #[tokio::test]
    async fn sends_codex_input_as_response_message_list() {
        let (base_url, request_rx) = spawn_server(
            200,
            "event: response.output_text.delta\n\
             data: {\"type\":\"response.output_text.delta\",\"delta\":\"Hel\"}\n\
             \n\
             event: response.output_text.delta\n\
             data: {\"type\":\"response.output_text.delta\",\"delta\":\"lo\"}\n\
             \n\
             event: response.completed\n\
             data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_test\",\"model\":\"gpt-test\",\"status\":\"completed\",\"usage\":{\"input_tokens\":2,\"output_tokens\":1,\"total_tokens\":3}}}\n\
             \n",
        )
        .await;
        let dir = TempDir::new().expect("temp dir");
        let account_home = dir.path().join("providers/codex/default");
        let provider = CodexResponsesProvider::new(CodexProviderConfig {
            base_url,
            default_model: Some("gpt-test".to_string()),
            account_home: Some(account_home.clone()),
            ..CodexProviderConfig::default()
        })
        .expect("provider");
        provider
            .token_store()
            .write(&CodexOAuthTokens {
                access_token: "access".to_string(),
                refresh_token: "refresh".to_string(),
                last_refresh: 123,
            })
            .expect("write token");

        let response = provider
            .generate(GenerateRequest::text("Hello?"))
            .await
            .expect("response");

        let captured = request_rx.await.expect("captured request");
        assert_eq!(captured.method, "POST");
        assert_eq!(captured.path, "/responses");
        assert_eq!(
            captured.headers.get("authorization").map(String::as_str),
            Some("Bearer access")
        );
        let body: Value = serde_json::from_str(&captured.body).expect("json body");
        assert_eq!(body["model"], "gpt-test");
        assert_eq!(body["input"][0]["role"], "user");
        assert_eq!(body["input"][0]["content"], "Hello?");
        assert_eq!(body["stream"], true);
        assert_eq!(body["store"], false);

        assert_eq!(response.assistant_text(), "Hello");
    }

    #[tokio::test]
    async fn forwards_codex_streaming_text_deltas() {
        let (base_url, request_rx) = spawn_server(
            200,
            "event: response.output_text.delta\n\
             data: {\"type\":\"response.output_text.delta\",\"delta\":\"Hel\"}\n\
             \n\
             event: response.output_text.delta\n\
             data: {\"type\":\"response.output_text.delta\",\"delta\":\"lo\"}\n\
             \n\
             event: response.completed\n\
             data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_test\",\"model\":\"gpt-test\",\"status\":\"completed\"}}\n\
             \n",
        )
        .await;
        let dir = TempDir::new().expect("temp dir");
        let account_home = dir.path().join("providers/codex/default");
        let provider = CodexResponsesProvider::new(CodexProviderConfig {
            base_url,
            default_model: Some("gpt-test".to_string()),
            account_home: Some(account_home.clone()),
            ..CodexProviderConfig::default()
        })
        .expect("provider");
        provider
            .token_store()
            .write(&CodexOAuthTokens {
                access_token: "access".to_string(),
                refresh_token: "refresh".to_string(),
                last_refresh: 123,
            })
            .expect("write token");

        let mut events = Vec::new();
        let response = provider
            .generate_streaming(GenerateRequest::text("Hello?"), &mut |event| {
                events.push(event);
            })
            .await
            .expect("response");

        let _captured = request_rx.await.expect("captured request");
        assert_eq!(response.assistant_text(), "Hello");
        assert_eq!(
            events,
            vec![
                GenerateStreamEvent::AssistantTextDelta {
                    delta: "Hel".to_string()
                },
                GenerateStreamEvent::AssistantTextDelta {
                    delta: "lo".to_string()
                }
            ]
        );
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
                "HTTP/1.1 {status} {reason}\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\r\n{}",
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
        let mut bytes = Vec::new();
        let mut buffer = [0_u8; 1024];
        loop {
            let read = socket.read(&mut buffer).await.expect("read request");
            assert_ne!(read, 0, "client closed before complete request");
            bytes.extend_from_slice(&buffer[..read]);
            if bytes.windows(4).any(|window| window == b"\r\n\r\n") {
                let text = String::from_utf8_lossy(&bytes);
                if let Some(content_length) = parse_content_length(&text) {
                    let header_end = bytes
                        .windows(4)
                        .position(|window| window == b"\r\n\r\n")
                        .expect("header end")
                        + 4;
                    if bytes.len() >= header_end + content_length {
                        break;
                    }
                }
            }
        }

        parse_request(&bytes)
    }

    fn parse_request(bytes: &[u8]) -> CapturedRequest {
        let header_end = bytes
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .expect("header end");
        let headers_text = String::from_utf8(bytes[..header_end].to_vec()).expect("headers utf8");
        let body = String::from_utf8(bytes[header_end + 4..].to_vec()).expect("body utf8");
        let mut lines = headers_text.lines();
        let request_line = lines.next().expect("request line");
        let mut request_parts = request_line.split_whitespace();
        let method = request_parts.next().expect("method").to_string();
        let path = request_parts.next().expect("path").to_string();
        let mut headers = HashMap::new();
        for line in lines {
            let Some((name, value)) = line.split_once(':') else {
                continue;
            };
            headers.insert(name.to_ascii_lowercase(), value.trim().to_string());
        }

        CapturedRequest {
            method,
            path,
            headers,
            body,
        }
    }

    fn parse_content_length(text: &str) -> Option<usize> {
        text.lines().find_map(|line| {
            let (name, value) = line.split_once(':')?;
            (name.eq_ignore_ascii_case("content-length"))
                .then(|| value.trim().parse().ok())
                .flatten()
        })
    }
}
