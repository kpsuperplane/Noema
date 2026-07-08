//! Private OpenAI-compatible model proxy for managed Supermemory.

use std::{sync::Arc, time::SystemTime};

use serde::Deserialize;
use serde_json::{Value, json};
use thiserror::Error;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::oneshot,
    task::JoinHandle,
};

use crate::{
    SystemErrorLogger,
    daemon::RuntimeModelProvider,
    provider::{
        GenerateInput, GenerateInputItem, GenerateMessage, GenerateMessageRole, GenerateOptions,
        GenerateRequest, GenerateResponse, GenerateResponseStatus, GenerateStreamEvent,
        GenerateToolCall, GenerateToolCallInput, GenerateToolResultInput, NoemaToolChoice,
        NoemaToolExecution, NoemaToolSpec, ProviderError, ReasoningEffort,
    },
};

const MAX_REQUEST_BYTES: usize = 1024 * 1024;

/// Configuration for Noema's private Supermemory model proxy.
#[derive(Clone)]
pub struct SupermemoryModelProxyConfig {
    /// Provider selected by Settings > Memory.
    pub provider: Arc<dyn RuntimeModelProvider>,
    /// Bearer token accepted from the Supermemory child process.
    pub api_key: String,
    /// Model/profile selected by Settings > Memory.
    pub model_profile: String,
    /// Reasoning effort selected by Settings > Memory, when configured.
    pub reasoning_effort: Option<ReasoningEffort>,
    /// Developer diagnostics sink.
    pub system_errors: Option<SystemErrorLogger>,
}

/// Private loopback OpenAI-compatible proxy owned by managed Supermemory.
pub struct SupermemoryModelProxy {
    openai_base_url: String,
    api_key: String,
    model_profile: String,
    shutdown_tx: Option<oneshot::Sender<()>>,
    task: JoinHandle<()>,
}

impl SupermemoryModelProxy {
    /// Start the loopback model proxy.
    ///
    /// # Errors
    ///
    /// Returns [`SupermemoryModelProxyError`] when the listener cannot bind.
    pub async fn start(
        config: SupermemoryModelProxyConfig,
    ) -> Result<Self, SupermemoryModelProxyError> {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await?;
        let address = listener.local_addr()?;
        let openai_base_url = format!("http://{address}/v1");
        let api_key = config.api_key.clone();
        let model_profile = config.model_profile.clone();
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let task = tokio::spawn(run_proxy(listener, config, shutdown_rx));

        Ok(Self {
            openai_base_url,
            api_key,
            model_profile,
            shutdown_tx: Some(shutdown_tx),
            task,
        })
    }

    /// OpenAI-compatible base URL, including `/v1`.
    #[must_use]
    pub fn openai_base_url(&self) -> &str {
        &self.openai_base_url
    }

    /// Bearer token accepted by this proxy.
    #[must_use]
    pub fn api_key(&self) -> &str {
        &self.api_key
    }

    /// Model/profile to advertise to Supermemory.
    #[must_use]
    pub fn model_profile(&self) -> &str {
        &self.model_profile
    }

    /// Stop the proxy accept loop.
    pub async fn shutdown(mut self) {
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            let _ = shutdown_tx.send(());
        }
        let _ = self.task.await;
    }
}

impl std::fmt::Debug for SupermemoryModelProxy {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SupermemoryModelProxy")
            .field("openai_base_url", &self.openai_base_url)
            .field("model_profile", &self.model_profile)
            .finish_non_exhaustive()
    }
}

async fn run_proxy(
    listener: TcpListener,
    config: SupermemoryModelProxyConfig,
    mut shutdown_rx: oneshot::Receiver<()>,
) {
    loop {
        tokio::select! {
            biased;
            _ = &mut shutdown_rx => {
                break;
            }
            accepted = listener.accept() => {
                match accepted {
                    Ok((stream, _peer)) => {
                        let config = config.clone();
                        let system_errors = config.system_errors.clone();
                        tokio::spawn(async move {
                            if let Err(error) = handle_connection(stream, config).await {
                                log_proxy_error(
                                    system_errors.as_ref(),
                                    "supermemory_model_proxy_request_failed",
                                    &error.to_string(),
                                );
                            }
                        });
                    }
                    Err(error) => {
                        log_proxy_error(
                            config.system_errors.as_ref(),
                            "supermemory_model_proxy_accept_failed",
                            &error.to_string(),
                        );
                        break;
                    }
                }
            }
        }
    }
}

async fn handle_connection(
    mut stream: TcpStream,
    config: SupermemoryModelProxyConfig,
) -> Result<(), SupermemoryModelProxyError> {
    let request = read_http_request(&mut stream).await?;
    let response = route_request(request, config).await;
    stream.write_all(&response.to_bytes()).await?;
    stream.shutdown().await?;
    Ok(())
}

async fn route_request(request: HttpRequest, config: SupermemoryModelProxyConfig) -> HttpResponse {
    if request.method != "GET" && request.method != "POST" {
        return json_error(http::StatusCode::METHOD_NOT_ALLOWED, "method_not_allowed");
    }
    if request.path == "/v1/models" && request.method == "GET" {
        return json_response(
            http::StatusCode::OK,
            json!({
                "object": "list",
                "data": [{
                    "id": config.model_profile,
                    "object": "model",
                    "created": unix_timestamp(),
                    "owned_by": "noema"
                }]
            }),
        );
    }
    if request.path != "/v1/chat/completions" || request.method != "POST" {
        return json_error(http::StatusCode::NOT_FOUND, "not_found");
    }
    let Some(authorization) = request.header("authorization") else {
        return json_error(http::StatusCode::UNAUTHORIZED, "missing_authorization");
    };
    if authorization != format!("Bearer {}", config.api_key) {
        return json_error(http::StatusCode::UNAUTHORIZED, "invalid_authorization");
    }
    let openai_request = match serde_json::from_slice::<OpenAiChatCompletionRequest>(&request.body)
    {
        Ok(request) => request,
        Err(error) => {
            return json_error_message(
                http::StatusCode::BAD_REQUEST,
                "invalid_json",
                error.to_string(),
            );
        }
    };
    if openai_request.stream.unwrap_or(false) {
        if let Some(system_errors) = &config.system_errors {
            system_errors.try_append(crate::SystemErrorEvent::new(
                "supermemory_model_proxy_streaming_unsupported",
                "Supermemory requested streaming from the private model proxy",
            ));
        }
        return json_error(http::StatusCode::NOT_IMPLEMENTED, "streaming_not_supported");
    }
    let generate_request = match openai_request.into_generate_request(&config) {
        Ok(request) => request,
        Err(error) => {
            return json_error_message(
                http::StatusCode::BAD_REQUEST,
                "invalid_request",
                error.to_string(),
            );
        }
    };
    let mut ignore_event = |_event: GenerateStreamEvent| {};
    match config
        .provider
        .generate_streaming(generate_request, &mut ignore_event)
        .await
    {
        Ok(response) => json_response(
            http::StatusCode::OK,
            openai_response_from_generate_response(response),
        ),
        Err(error) => {
            if let Some(system_errors) = &config.system_errors {
                system_errors.try_append(
                    crate::SystemErrorEvent::new(
                        "supermemory_model_proxy_provider_failed",
                        "Supermemory model proxy provider request failed",
                    )
                    .with_error_chain([error.to_string()]),
                );
            }
            json_error_message(
                http::StatusCode::BAD_GATEWAY,
                "provider_failed",
                error.to_string(),
            )
        }
    }
}

#[derive(Debug)]
struct HttpRequest {
    method: String,
    path: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl HttpRequest {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _value)| key.eq_ignore_ascii_case(name))
            .map(|(_key, value)| value.as_str())
    }
}

async fn read_http_request(
    stream: &mut TcpStream,
) -> Result<HttpRequest, SupermemoryModelProxyError> {
    let mut buffer = Vec::new();
    let header_end = loop {
        let mut chunk = [0_u8; 4096];
        let read = stream.read(&mut chunk).await?;
        if read == 0 {
            return Err(SupermemoryModelProxyError::Protocol(
                "connection closed before request headers".to_string(),
            ));
        }
        buffer.extend_from_slice(&chunk[..read]);
        if buffer.len() > MAX_REQUEST_BYTES {
            return Err(SupermemoryModelProxyError::Protocol(
                "request exceeded maximum size".to_string(),
            ));
        }
        if let Some(index) = find_header_end(&buffer) {
            break index;
        }
    };
    let header_text = std::str::from_utf8(&buffer[..header_end])
        .map_err(|_| SupermemoryModelProxyError::Protocol("headers are not utf-8".to_string()))?;
    let mut lines = header_text.split("\r\n");
    let request_line = lines
        .next()
        .ok_or_else(|| SupermemoryModelProxyError::Protocol("missing request line".to_string()))?;
    let mut request_parts = request_line.split_whitespace();
    let method = request_parts
        .next()
        .ok_or_else(|| SupermemoryModelProxyError::Protocol("missing method".to_string()))?
        .to_string();
    let path = request_parts
        .next()
        .ok_or_else(|| SupermemoryModelProxyError::Protocol("missing path".to_string()))?
        .to_string();
    let headers = lines
        .filter_map(|line| {
            let (name, value) = line.split_once(':')?;
            Some((name.trim().to_string(), value.trim().to_string()))
        })
        .collect::<Vec<_>>();
    let content_length = headers
        .iter()
        .find(|(name, _value)| name.eq_ignore_ascii_case("content-length"))
        .map(|(_name, value)| value.parse::<usize>())
        .transpose()
        .map_err(|_| SupermemoryModelProxyError::Protocol("invalid content-length".to_string()))?
        .unwrap_or(0);
    if content_length > MAX_REQUEST_BYTES {
        return Err(SupermemoryModelProxyError::Protocol(
            "request body exceeded maximum size".to_string(),
        ));
    }
    let body_start = header_end + 4;
    while buffer.len() < body_start + content_length {
        let mut chunk = [0_u8; 4096];
        let read = stream.read(&mut chunk).await?;
        if read == 0 {
            return Err(SupermemoryModelProxyError::Protocol(
                "connection closed before request body".to_string(),
            ));
        }
        buffer.extend_from_slice(&chunk[..read]);
        if buffer.len() > body_start + content_length {
            break;
        }
    }
    let body = buffer[body_start..body_start + content_length].to_vec();
    Ok(HttpRequest {
        method,
        path,
        headers,
        body,
    })
}

fn find_header_end(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|window| window == b"\r\n\r\n")
}

struct HttpResponse {
    status: http::StatusCode,
    body: Vec<u8>,
}

impl HttpResponse {
    fn to_bytes(&self) -> Vec<u8> {
        let reason = self.status.canonical_reason().unwrap_or("Unknown");
        let mut bytes = format!(
            "HTTP/1.1 {} {reason}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
            self.status.as_u16(),
            self.body.len()
        )
        .into_bytes();
        bytes.extend_from_slice(&self.body);
        bytes
    }
}

fn json_response(status: http::StatusCode, value: Value) -> HttpResponse {
    HttpResponse {
        status,
        body: serde_json::to_vec(&value).unwrap_or_else(|_| b"{}".to_vec()),
    }
}

fn json_error(status: http::StatusCode, code: &str) -> HttpResponse {
    json_error_message(status, code, code.to_string())
}

fn json_error_message(
    status: http::StatusCode,
    code: &str,
    message: impl Into<String>,
) -> HttpResponse {
    json_response(
        status,
        json!({
            "error": {
                "type": "noema_supermemory_model_proxy_error",
                "code": code,
                "message": message.into()
            }
        }),
    )
}

#[derive(Debug, Deserialize)]
struct OpenAiChatCompletionRequest {
    model: Option<String>,
    messages: Vec<OpenAiChatMessage>,
    #[serde(default)]
    tools: Vec<OpenAiChatTool>,
    #[serde(default)]
    tool_choice: Option<OpenAiToolChoice>,
    #[serde(default)]
    max_tokens: Option<u32>,
    #[serde(default)]
    max_completion_tokens: Option<u32>,
    #[serde(default)]
    temperature: Option<f32>,
    #[serde(default)]
    stream: Option<bool>,
}

impl OpenAiChatCompletionRequest {
    fn into_generate_request(
        self,
        config: &SupermemoryModelProxyConfig,
    ) -> Result<GenerateRequest, SupermemoryModelProxyError> {
        let mut instructions = Vec::new();
        let mut items = Vec::new();
        for message in self.messages {
            match message.role.as_str() {
                "system" | "developer" => {
                    if let Some(content) = message.content_text() {
                        instructions.push(content);
                    }
                }
                "user" => {
                    if let Some(content) = message.content_text() {
                        items.push(GenerateInputItem::Message(GenerateMessage {
                            role: GenerateMessageRole::User,
                            content,
                        }));
                    }
                }
                "assistant" => {
                    if let Some(content) = message.content_text() {
                        items.push(GenerateInputItem::Message(GenerateMessage {
                            role: GenerateMessageRole::Assistant,
                            content,
                        }));
                    }
                    for tool_call in message.tool_calls {
                        items.push(GenerateInputItem::ToolCall(GenerateToolCallInput {
                            id: Some(tool_call.id.clone()),
                            call_id: tool_call.id,
                            name: tool_call.function.name.clone(),
                            provider_name: Some(tool_call.function.name),
                            arguments: parse_arguments_json(&tool_call.function.arguments)?,
                        }));
                    }
                }
                "tool" => {
                    let content = message.content_text();
                    let Some(tool_call_id) = message.tool_call_id else {
                        return Err(SupermemoryModelProxyError::Protocol(
                            "tool message is missing tool_call_id".to_string(),
                        ));
                    };
                    let name = message.name.unwrap_or_else(|| "tool".to_string());
                    items.push(GenerateInputItem::ToolResult(GenerateToolResultInput {
                        id: None,
                        call_id: tool_call_id,
                        name: name.clone(),
                        provider_name: Some(name),
                        arguments: Value::Null,
                        success: true,
                        payload: content.map_or(Value::Null, |content| json!({"content": content})),
                    }));
                }
                role => {
                    return Err(SupermemoryModelProxyError::Protocol(format!(
                        "unsupported message role: {role}"
                    )));
                }
            }
        }

        let tools = self
            .tools
            .into_iter()
            .map(OpenAiChatTool::into_noema_tool)
            .collect::<Result<Vec<_>, _>>()?;

        Ok(GenerateRequest {
            conversation_id: None,
            model: Some(
                self.model
                    .filter(|model| !model.trim().is_empty())
                    .unwrap_or_else(|| config.model_profile.clone()),
            ),
            input: GenerateInput::Items(items),
            instructions: nonempty_join(instructions, "\n\n"),
            options: GenerateOptions {
                max_output_tokens: self.max_completion_tokens.or(self.max_tokens),
                temperature: self.temperature,
                reasoning_effort: config.reasoning_effort,
                require_noema_response: false,
                prompt_cache_retention: None,
            },
            tools,
            tool_choice: self.tool_choice.map_or(NoemaToolChoice::Auto, Into::into),
            parallel_tool_calls: true,
        })
    }
}

#[derive(Debug, Deserialize)]
struct OpenAiChatMessage {
    role: String,
    #[serde(default)]
    content: Option<OpenAiMessageContent>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    tool_call_id: Option<String>,
    #[serde(default)]
    tool_calls: Vec<OpenAiToolCall>,
}

impl OpenAiChatMessage {
    fn content_text(&self) -> Option<String> {
        match self.content.as_ref()? {
            OpenAiMessageContent::Text(text) => nonempty_string(text),
            OpenAiMessageContent::Parts(parts) => nonempty_join(
                parts
                    .iter()
                    .filter_map(|part| match part {
                        OpenAiContentPart::Text { text } => nonempty_string(text),
                    })
                    .collect::<Vec<_>>(),
                "\n",
            ),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum OpenAiMessageContent {
    Text(String),
    Parts(Vec<OpenAiContentPart>),
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum OpenAiContentPart {
    #[serde(rename = "text")]
    Text { text: String },
}

#[derive(Debug, Deserialize)]
struct OpenAiChatTool {
    #[serde(rename = "type")]
    kind: String,
    function: OpenAiFunctionSpec,
}

impl OpenAiChatTool {
    fn into_noema_tool(self) -> Result<NoemaToolSpec, SupermemoryModelProxyError> {
        if self.kind != "function" {
            return Err(SupermemoryModelProxyError::Protocol(format!(
                "unsupported tool type: {}",
                self.kind
            )));
        }
        NoemaToolSpec::new(
            self.function.name,
            self.function.description,
            self.function.parameters,
            NoemaToolExecution::LocalBuiltin,
        )
        .map_err(|error| SupermemoryModelProxyError::Protocol(error.to_string()))
    }
}

#[derive(Debug, Deserialize)]
struct OpenAiFunctionSpec {
    name: String,
    description: String,
    #[serde(default = "default_object_schema")]
    parameters: Value,
}

#[derive(Debug, Deserialize)]
struct OpenAiToolCall {
    id: String,
    function: OpenAiToolCallFunction,
}

#[derive(Debug, Deserialize)]
struct OpenAiToolCallFunction {
    name: String,
    arguments: String,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum OpenAiToolChoice {
    Mode(String),
    Function {
        #[serde(rename = "type")]
        kind: String,
        function: OpenAiToolChoiceFunction,
    },
}

impl From<OpenAiToolChoice> for NoemaToolChoice {
    fn from(choice: OpenAiToolChoice) -> Self {
        match choice {
            OpenAiToolChoice::Mode(mode) if mode == "none" => Self::None,
            OpenAiToolChoice::Mode(mode) if mode == "required" => Self::Required,
            OpenAiToolChoice::Mode(_) => Self::Auto,
            OpenAiToolChoice::Function { kind, function } => {
                let _ = (kind, function.name);
                Self::Required
            }
        }
    }
}

#[derive(Debug, Deserialize)]
struct OpenAiToolChoiceFunction {
    name: String,
}

fn openai_response_from_generate_response(response: GenerateResponse) -> Value {
    let has_tools =
        response.response_status == GenerateResponseStatus::NeedsTools || response.has_tool_calls();
    let finish_reason = if has_tools { "tool_calls" } else { "stop" };
    let id = response
        .response_id
        .clone()
        .unwrap_or_else(|| format!("chatcmpl_{}", unix_timestamp()));
    let content = response.assistant_text();
    let model = response.model.clone();
    let tool_calls = openai_tool_calls(response.tool_calls);
    let usage = response.usage.map(|usage| {
        json!({
            "prompt_tokens": usage.input_tokens,
            "completion_tokens": usage.output_tokens,
            "total_tokens": usage.total_tokens,
        })
    });
    json!({
        "id": id,
        "object": "chat.completion",
        "created": unix_timestamp(),
        "model": model,
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": content,
                "tool_calls": tool_calls,
            },
            "finish_reason": finish_reason
        }],
        "usage": usage.unwrap_or(Value::Null)
    })
}

fn openai_tool_calls(calls: Vec<GenerateToolCall>) -> Value {
    if calls.is_empty() {
        return Value::Null;
    }
    Value::Array(
        calls
            .into_iter()
            .enumerate()
            .map(|(index, call)| {
                let id = call
                    .provider_call_id
                    .clone()
                    .or(call.id.clone())
                    .unwrap_or_else(|| format!("call_{index}"));
                let name = call.provider_name.unwrap_or(call.name);
                json!({
                    "id": id,
                    "type": "function",
                    "function": {
                        "name": name,
                        "arguments": call.payload.to_string(),
                    }
                })
            })
            .collect(),
    )
}

fn parse_arguments_json(arguments: &str) -> Result<Value, SupermemoryModelProxyError> {
    if arguments.trim().is_empty() {
        return Ok(json!({}));
    }
    serde_json::from_str(arguments).map_err(|error| {
        SupermemoryModelProxyError::Protocol(format!("invalid tool call arguments: {error}"))
    })
}

fn default_object_schema() -> Value {
    json!({"type": "object", "properties": {}})
}

fn nonempty_string(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn nonempty_join(values: Vec<String>, separator: &str) -> Option<String> {
    let value = values
        .into_iter()
        .filter_map(|value| nonempty_string(&value))
        .collect::<Vec<_>>()
        .join(separator);
    nonempty_string(&value)
}

fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}

fn log_proxy_error(system_errors: Option<&SystemErrorLogger>, code: &'static str, message: &str) {
    if let Some(system_errors) = system_errors {
        system_errors.try_append(
            crate::SystemErrorEvent::new(code, "Supermemory model proxy request failed")
                .with_error_chain([message.to_string()]),
        );
    }
}

/// Errors returned by the private model proxy.
#[derive(Debug, Error)]
pub enum SupermemoryModelProxyError {
    /// Network I/O failed.
    #[error("supermemory model proxy I/O failed: {0}")]
    Io(#[from] std::io::Error),
    /// Request or protocol validation failed.
    #[error("supermemory model proxy protocol error: {0}")]
    Protocol(String),
    /// Provider generation failed.
    #[error("supermemory model proxy provider failed: {0}")]
    Provider(#[from] ProviderError),
}

#[cfg(test)]
mod tests {
    use std::{
        future::Future,
        pin::Pin,
        sync::{Arc, Mutex},
    };

    use crate::{
        daemon::RuntimeModelProvider,
        provider::{
            GenerateInput, GenerateRequest, GenerateResponse, GenerateResponseStatus,
            GenerateStreamEvent, GenerateToolCall, NoemaToolChoice, ProviderError,
        },
    };
    use serde_json::{Value, json};

    #[derive(Debug)]
    struct CapturingProvider {
        response: Mutex<GenerateResponse>,
        requests: Mutex<Vec<GenerateRequest>>,
    }

    impl CapturingProvider {
        fn new(response: GenerateResponse) -> Self {
            Self {
                response: Mutex::new(response),
                requests: Mutex::new(Vec::new()),
            }
        }

        fn requests(&self) -> Vec<GenerateRequest> {
            self.requests.lock().expect("requests lock").clone()
        }
    }

    impl RuntimeModelProvider for CapturingProvider {
        fn generate_streaming<'a>(
            &'a self,
            request: GenerateRequest,
            _on_event: &'a mut (dyn FnMut(GenerateStreamEvent) + Send),
        ) -> Pin<Box<dyn Future<Output = Result<GenerateResponse, ProviderError>> + Send + 'a>>
        {
            Box::pin(async move {
                self.requests.lock().expect("requests lock").push(request);
                Ok(self.response.lock().expect("response lock").clone())
            })
        }
    }

    #[tokio::test]
    async fn chat_completions_route_to_configured_memory_model() {
        let provider = Arc::new(CapturingProvider::new(GenerateResponse::final_text(
            "stored",
            "fake",
            "memory-model",
        )));
        let proxy = super::SupermemoryModelProxy::start(super::SupermemoryModelProxyConfig {
            provider: provider.clone(),
            api_key: "secret".to_string(),
            model_profile: "memory-model".to_string(),
            reasoning_effort: None,
            system_errors: None,
        })
        .await
        .expect("start proxy");

        let response = reqwest::Client::new()
            .post(format!("{}/chat/completions", proxy.openai_base_url()))
            .bearer_auth(proxy.api_key())
            .json(&json!({
                "model": "memory-model",
                "messages": [
                    {"role": "system", "content": "extract useful memories"},
                    {"role": "user", "content": "Kevin likes local-first tools"}
                ],
                "max_tokens": 123,
                "temperature": 0.2,
                "tool_choice": "required",
                "tools": [{
                    "type": "function",
                    "function": {
                        "name": "CreateMemory",
                        "description": "Create a memory",
                        "parameters": {
                            "type": "object",
                            "properties": {"text": {"type": "string"}},
                            "required": ["text"],
                            "additionalProperties": false
                        }
                    }
                }]
            }))
            .send()
            .await
            .expect("request");

        assert!(response.status().is_success());
        let body: Value = response.json().await.expect("json response");
        assert_eq!(body["choices"][0]["message"]["content"], "stored");

        let requests = provider.requests();
        assert_eq!(requests.len(), 1);
        let request = &requests[0];
        assert_eq!(request.model.as_deref(), Some("memory-model"));
        assert_eq!(
            request.instructions.as_deref(),
            Some("extract useful memories")
        );
        assert_eq!(request.options.max_output_tokens, Some(123));
        assert_eq!(request.options.temperature, Some(0.2));
        assert!(matches!(request.tool_choice, NoemaToolChoice::Required));
        assert_eq!(request.tools[0].name.as_str(), "CreateMemory");
        assert_eq!(request.tools[0].description, "Create a memory");
        let GenerateInput::Items(items) = &request.input else {
            panic!("expected itemized input");
        };
        assert_eq!(items.len(), 1);
        assert!(
            request
                .input
                .render_for_token_count()
                .contains("Kevin likes")
        );
    }

    #[tokio::test]
    async fn chat_completions_return_provider_tool_calls() {
        let provider = Arc::new(CapturingProvider::new(GenerateResponse {
            responses: Vec::new(),
            tool_calls: vec![GenerateToolCall {
                id: Some("item_1".to_string()),
                provider_call_id: Some("call_1".to_string()),
                provider_name: Some("CreateMemory".to_string()),
                name: "CreateMemory".to_string(),
                payload: json!({"text": "Kevin likes local-first tools"}),
            }],
            reasoning_items: Vec::new(),
            response_status: GenerateResponseStatus::NeedsTools,
            provider: "fake".to_string(),
            model: "memory-model".to_string(),
            response_id: Some("resp_1".to_string()),
            usage: None,
        }));
        let proxy = super::SupermemoryModelProxy::start(super::SupermemoryModelProxyConfig {
            provider: provider.clone(),
            api_key: "secret".to_string(),
            model_profile: "memory-model".to_string(),
            reasoning_effort: None,
            system_errors: None,
        })
        .await
        .expect("start proxy");

        let body: Value = reqwest::Client::new()
            .post(format!("{}/chat/completions", proxy.openai_base_url()))
            .bearer_auth(proxy.api_key())
            .json(&json!({
                "model": "memory-model",
                "messages": [{"role": "user", "content": "remember this"}]
            }))
            .send()
            .await
            .expect("request")
            .json()
            .await
            .expect("json");

        assert_eq!(body["id"], "resp_1");
        assert_eq!(body["choices"][0]["finish_reason"], "tool_calls");
        assert_eq!(
            body["choices"][0]["message"]["tool_calls"][0]["id"],
            "call_1"
        );
        assert_eq!(
            body["choices"][0]["message"]["tool_calls"][0]["function"]["name"],
            "CreateMemory"
        );
        assert_eq!(
            body["choices"][0]["message"]["tool_calls"][0]["function"]["arguments"],
            "{\"text\":\"Kevin likes local-first tools\"}"
        );
    }

    #[tokio::test]
    async fn chat_completions_reject_streaming_requests() {
        let provider = Arc::new(CapturingProvider::new(GenerateResponse::final_text(
            "unused",
            "fake",
            "memory-model",
        )));
        let proxy = super::SupermemoryModelProxy::start(super::SupermemoryModelProxyConfig {
            provider: provider.clone(),
            api_key: "secret".to_string(),
            model_profile: "memory-model".to_string(),
            reasoning_effort: None,
            system_errors: None,
        })
        .await
        .expect("start proxy");

        let response = reqwest::Client::new()
            .post(format!("{}/chat/completions", proxy.openai_base_url()))
            .bearer_auth(proxy.api_key())
            .json(&json!({
                "model": "memory-model",
                "stream": true,
                "messages": [{"role": "user", "content": "hello"}]
            }))
            .send()
            .await
            .expect("request");

        assert_eq!(response.status(), reqwest::StatusCode::NOT_IMPLEMENTED);
        assert!(provider.requests().is_empty());
    }
}
