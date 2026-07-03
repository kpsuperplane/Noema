//! Provider-neutral generation contract.

use crate::memory::extraction::ExtractorMemoryProposal;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::future::Future;
use thiserror::Error;

/// Default model for small metadata classification tasks such as MCP tool calibration.
pub const DEFAULT_TOOL_CLASSIFICATION_MODEL: &str = "gpt-5.4-mini";

// The provider contract stays a native async trait and does not expose
// `dyn ModelProvider`, so the public future-bound tradeoff is intentional.
/// A model backend that can produce structured output from a generation request.
pub trait ModelProvider: Send + Sync {
    /// Generate a response for the given request.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError`] when the request is invalid, credentials are
    /// missing, the backend is unavailable, the backend returns an API error,
    /// or its response cannot be parsed.
    fn generate(
        &self,
        request: GenerateRequest,
    ) -> impl Future<Output = Result<GenerateResponse, ProviderError>> + Send;

    /// Return the provider's preferred model for metadata-only tool classification.
    fn default_tool_classification_model(&self) -> Option<String> {
        Some(DEFAULT_TOOL_CLASSIFICATION_MODEL.to_string())
    }

    /// Return provider/model context-window metadata used for prompt planning.
    fn context_metadata(&self, _model: Option<&str>) -> ProviderContextMetadata {
        ProviderContextMetadata::default()
    }

    /// Count request tokens when the provider has an authoritative tokenizer.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError`] when provider token counting fails.
    fn count_tokens(
        &self,
        _instructions: Option<&str>,
        _input: &str,
        _model: Option<&str>,
    ) -> impl Future<Output = Result<Option<u32>, ProviderError>> + Send {
        async { Ok(None) }
    }

    /// Generate a response while optionally emitting ephemeral stream events.
    fn generate_streaming<'a>(
        &'a self,
        request: GenerateRequest,
        on_event: &'a mut (dyn FnMut(GenerateStreamEvent) + Send),
    ) -> impl Future<Output = Result<GenerateResponse, ProviderError>> + Send + 'a {
        async move {
            let _ = on_event;
            self.generate(request).await
        }
    }
}

/// Provider/model context-window metadata for prompt planning.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ProviderContextMetadata {
    /// Maximum context window in tokens, if known.
    pub context_window_tokens: Option<u32>,
    /// Default output reserve for requests to this model.
    pub default_output_reserve_tokens: Option<u32>,
    /// Target summary size for compaction prompts.
    pub compact_summary_target_tokens: Option<u32>,
}

/// Input and options for a provider generation call.
#[derive(Debug, Clone, PartialEq)]
pub struct GenerateRequest {
    /// Noema conversation id when the request belongs to a durable conversation.
    pub conversation_id: Option<String>,
    /// Optional model override for this request.
    pub model: Option<String>,
    /// User-visible input to send to the provider.
    pub input: GenerateInput,
    /// Optional system or developer instructions.
    pub instructions: Option<String>,
    /// Provider-neutral generation controls.
    pub options: GenerateOptions,
}

impl GenerateRequest {
    /// Create a text-only request with default options.
    #[must_use]
    pub fn text(input: impl Into<String>) -> Self {
        Self {
            conversation_id: None,
            model: None,
            input: GenerateInput::Text(input.into()),
            instructions: None,
            options: GenerateOptions::default(),
        }
    }

    /// Return the request with a model override set.
    #[must_use]
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }
}

/// Provider-neutral generation input.
#[derive(Debug, Clone, PartialEq)]
pub enum GenerateInput {
    /// Plain text input.
    Text(String),
    /// Role-tagged conversation messages.
    Messages(Vec<GenerateMessage>),
}

impl GenerateInput {
    /// Return whether this input has no model-visible text.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        match self {
            Self::Text(text) => text.trim().is_empty(),
            Self::Messages(messages) => messages
                .iter()
                .all(|message| message.content.trim().is_empty()),
        }
    }

    /// Render input to plain text for token counters that do not understand
    /// provider-neutral message structure.
    #[must_use]
    pub fn render_for_token_count(&self) -> String {
        match self {
            Self::Text(text) => text.clone(),
            Self::Messages(messages) => messages
                .iter()
                .filter(|message| !message.content.trim().is_empty())
                .map(|message| format!("{}: {}", message.role.as_str(), message.content))
                .collect::<Vec<_>>()
                .join("\n"),
        }
    }
}

/// One role-tagged message in provider-neutral generation input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenerateMessage {
    /// Role visible to the provider.
    pub role: GenerateMessageRole,
    /// Text content for the message.
    pub content: String,
}

/// Role for a provider-neutral generation message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GenerateMessageRole {
    /// Human/user message.
    User,
    /// Assistant/model message.
    Assistant,
}

impl GenerateMessageRole {
    /// Provider-independent lower-case role name.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Assistant => "assistant",
        }
    }
}

/// Prompt-cache retention request for providers that support configurable caching.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PromptCacheRetention {
    /// Keep eligible prompt prefixes cached for 24 hours.
    #[serde(rename = "24h")]
    TwentyFourHours,
}

/// Provider-neutral optional generation controls.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GenerateOptions {
    /// Optional maximum number of output tokens.
    pub max_output_tokens: Option<u32>,
    /// Optional sampling temperature.
    pub temperature: Option<f32>,
    /// Require a strict Noema response envelope with assistant text and memory proposals.
    pub require_noema_response: bool,
    /// Provider prompt-cache retention request when supported.
    pub prompt_cache_retention: Option<PromptCacheRetention>,
}

/// Structured response returned by a model provider.
#[derive(Debug, Clone, PartialEq)]
pub struct GenerateResponse {
    /// Ordered output items returned by the provider.
    pub output: Vec<GenerateOutputItem>,
    /// Provider identifier that produced the response.
    pub provider: String,
    /// Model identifier used by the provider.
    pub model: String,
    /// Provider response identifier when one is available.
    pub response_id: Option<String>,
    /// Token usage reported by the provider when available.
    pub usage: Option<TokenUsage>,
}

/// Ephemeral events emitted while a provider response is still generating.
#[derive(Debug, Clone, PartialEq)]
pub enum GenerateStreamEvent {
    /// Incremental human-visible assistant text.
    AssistantTextDelta {
        /// Text delta received from the provider.
        delta: String,
    },
    /// A non-empty memory proposal block has started streaming.
    MemoryProposalsStarted,
    /// A provider tool call output item has started streaming.
    ToolCallStarted {
        /// Zero-based index of the output item in the provider response.
        output_index: usize,
        /// Tool name reported by the provider.
        name: String,
    },
}

impl GenerateResponse {
    /// Return all assistant text output concatenated in order.
    #[must_use]
    pub fn assistant_text(&self) -> String {
        self.output
            .iter()
            .filter_map(|item| match item {
                GenerateOutputItem::AssistantText { text } => Some(text.as_str()),
                GenerateOutputItem::MemoryProposals { .. }
                | GenerateOutputItem::ToolCall { .. }
                | GenerateOutputItem::ToolResult { .. }
                | GenerateOutputItem::ApprovalRequest { .. }
                | GenerateOutputItem::ApprovalResult { .. }
                | GenerateOutputItem::Structured { .. } => None,
            })
            .collect()
    }

    /// Return all memory proposals emitted by the provider.
    #[must_use]
    pub fn memory_proposals(&self) -> Vec<ExtractorMemoryProposal> {
        self.output
            .iter()
            .flat_map(|item| match item {
                GenerateOutputItem::MemoryProposals { proposals } => proposals.as_slice(),
                GenerateOutputItem::AssistantText { .. }
                | GenerateOutputItem::ToolCall { .. }
                | GenerateOutputItem::ToolResult { .. }
                | GenerateOutputItem::ApprovalRequest { .. }
                | GenerateOutputItem::ApprovalResult { .. }
                | GenerateOutputItem::Structured { .. } => &[],
            })
            .cloned()
            .collect()
    }
}

/// Provider output item for rich responses.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum GenerateOutputItem {
    /// Human-visible assistant text.
    AssistantText {
        /// Text to show in the transcript.
        text: String,
    },
    /// Memory proposals emitted in the same provider call.
    MemoryProposals {
        /// Proposed memories. The daemon still validates and policy-gates them.
        proposals: Vec<ExtractorMemoryProposal>,
    },
    /// Provider-reported tool invocation.
    ToolCall {
        /// Provider item id or tool-call id, when available.
        id: Option<String>,
        /// Tool or operation name.
        name: String,
        /// Provider payload for audit and replay.
        payload: Value,
    },
    /// Provider-reported tool result.
    ToolResult {
        /// Provider tool-call id, when available.
        call_id: Option<String>,
        /// Tool or operation name, when available.
        name: Option<String>,
        /// Whether the result succeeded, when known.
        success: Option<bool>,
        /// Provider payload for audit and replay.
        payload: Value,
    },
    /// Provider request for approval or elicitation.
    ApprovalRequest {
        /// Provider request id, when available.
        id: Option<String>,
        /// Provider method that requested approval.
        method: String,
        /// Provider payload for audit and replay.
        payload: Value,
    },
    /// Recorded approval or elicitation decision.
    ApprovalResult {
        /// Provider request id, when available.
        request_id: Option<String>,
        /// Decision returned to the provider.
        decision: String,
        /// Provider response payload for audit and replay.
        payload: Value,
    },
    /// Future rich structured output payload.
    Structured {
        /// Stable schema identifier for the payload.
        schema: String,
        /// Provider-produced payload for that schema.
        payload: Value,
    },
}

/// Parse a provider text payload into structured Noema output items.
///
/// Providers that can only return text may emit a strict envelope:
///
/// ```json
/// {"type":"noema_response","output":[{"kind":"assistant_text","text":"Hello"}]}
/// ```
///
/// Text without this envelope is treated as one assistant text item.
///
/// # Errors
///
/// Returns [`ProviderError::MalformedResponse`] when a Noema envelope is
/// present but does not match the structured output contract.
pub fn output_items_from_text(text: String) -> Result<Vec<GenerateOutputItem>, ProviderError> {
    output_items_from_text_with_mode(text, false)
}

/// Parse a provider text payload that must be a Noema structured response.
///
/// # Errors
///
/// Returns [`ProviderError::MalformedResponse`] when the payload is not a
/// strict `noema_response` envelope containing assistant text and memory
/// proposals.
pub fn required_output_items_from_text(
    text: String,
) -> Result<Vec<GenerateOutputItem>, ProviderError> {
    output_items_from_text_with_mode(text, true)
}

fn output_items_from_text_with_mode(
    text: String,
    require_noema_response: bool,
) -> Result<Vec<GenerateOutputItem>, ProviderError> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err(ProviderError::MalformedResponse {
            message: "provider produced empty output".to_string(),
        });
    }

    if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
        if let Some(output) = output_items_from_structured_value(value, require_noema_response)? {
            return Ok(output);
        }
        if require_noema_response {
            return Err(ProviderError::MalformedResponse {
                message: "provider did not return a Noema structured response envelope".to_string(),
            });
        }
    }

    if require_noema_response {
        if let Some(output) = embedded_required_noema_response_output(trimmed)? {
            return Ok(output);
        }

        return Ok(vec![
            GenerateOutputItem::AssistantText { text },
            GenerateOutputItem::MemoryProposals {
                proposals: Vec::new(),
            },
        ]);
    }

    Ok(vec![GenerateOutputItem::AssistantText { text }])
}

fn embedded_required_noema_response_output(
    text: &str,
) -> Result<Option<Vec<GenerateOutputItem>>, ProviderError> {
    let mut output = None;
    for candidate in balanced_json_object_candidates(text) {
        let Ok(value) = serde_json::from_str::<Value>(candidate) else {
            continue;
        };
        let Some(candidate_output) = output_items_from_structured_value(value, true)? else {
            continue;
        };
        if output.is_some() {
            return Err(ProviderError::MalformedResponse {
                message: "provider returned multiple Noema structured response envelopes"
                    .to_string(),
            });
        }
        output = Some(candidate_output);
    }
    Ok(output)
}

fn balanced_json_object_candidates(text: &str) -> Vec<&str> {
    let mut candidates = Vec::new();
    let mut start = None;
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaping = false;

    for (index, ch) in text.char_indices() {
        if in_string {
            if escaping {
                escaping = false;
            } else if ch == '\\' {
                escaping = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }

        match ch {
            '"' if depth > 0 => in_string = true,
            '{' => {
                if depth == 0 {
                    start = Some(index);
                }
                depth += 1;
            }
            '}' if depth > 0 => {
                depth -= 1;
                if depth == 0
                    && let Some(start_index) = start.take()
                {
                    candidates.push(&text[start_index..index + ch.len_utf8()]);
                }
            }
            _ => {}
        }
    }

    candidates
}

fn output_items_from_structured_value(
    value: Value,
    require_noema_response: bool,
) -> Result<Option<Vec<GenerateOutputItem>>, ProviderError> {
    let is_explicit_envelope = value.get("type").and_then(Value::as_str) == Some("noema_response");
    if !is_explicit_envelope {
        return Ok(None);
    }

    let envelope: GenerateOutputEnvelope =
        serde_json::from_value(value).map_err(|source| ProviderError::MalformedResponse {
            message: format!("invalid Noema structured response: {source}"),
        })?;
    if envelope.output.is_empty() {
        return Err(ProviderError::MalformedResponse {
            message: "Noema structured response contained no output items".to_string(),
        });
    }
    if require_noema_response {
        validate_required_noema_response_output(&envelope.output)?;
    }
    Ok(Some(envelope.output))
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct GenerateOutputEnvelope {
    #[serde(rename = "type")]
    _envelope_type: String,
    output: Vec<GenerateOutputItem>,
}

fn validate_required_noema_response_output(
    output: &[GenerateOutputItem],
) -> Result<(), ProviderError> {
    let has_assistant_text = output.iter().any(|item| match item {
        GenerateOutputItem::AssistantText { text } => !text.trim().is_empty(),
        GenerateOutputItem::MemoryProposals { .. }
        | GenerateOutputItem::ToolCall { .. }
        | GenerateOutputItem::ToolResult { .. }
        | GenerateOutputItem::ApprovalRequest { .. }
        | GenerateOutputItem::ApprovalResult { .. }
        | GenerateOutputItem::Structured { .. } => false,
    });
    if !has_assistant_text {
        return Err(ProviderError::MalformedResponse {
            message: "Noema structured response did not include assistant_text".to_string(),
        });
    }

    let has_memory_proposals = output
        .iter()
        .any(|item| matches!(item, GenerateOutputItem::MemoryProposals { .. }));
    if !has_memory_proposals {
        return Err(ProviderError::MalformedResponse {
            message: "Noema structured response did not include memory_proposals".to_string(),
        });
    }

    Ok(())
}

/// Provider-reported token counts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenUsage {
    /// Number of input tokens consumed.
    pub input_tokens: u64,
    /// Number of output tokens produced.
    pub output_tokens: u64,
    /// Total tokens reported by the provider.
    pub total_tokens: u64,
    /// Input tokens served from provider prompt cache when reported.
    pub cached_input_tokens: Option<u64>,
}

/// Errors produced by model providers.
#[derive(Debug, Error)]
pub enum ProviderError {
    /// Required provider credentials were missing.
    #[error("missing credentials for {provider}: {credential}")]
    MissingCredentials {
        /// Provider name.
        provider: String,
        /// Missing credential name.
        credential: String,
    },

    /// The caller supplied an invalid request.
    #[error("invalid request: {message}")]
    InvalidRequest {
        /// Human-readable validation failure.
        message: String,
    },

    /// A transport-level HTTP request failed.
    #[error("http request failed: {source}")]
    HttpFailure {
        /// Underlying HTTP client error.
        #[from]
        source: reqwest::Error,
    },

    /// The provider returned a non-success API response.
    #[error("provider API error ({status}): {message}")]
    ApiError {
        /// HTTP or provider status code.
        status: u16,
        /// Provider error message.
        message: String,
        /// Provider request id when available.
        request_id: Option<String>,
    },

    /// The provider rejected the request because of rate limits.
    #[error("provider rate limit: {message}")]
    RateLimit {
        /// Provider rate-limit message.
        message: String,
        /// Provider request id when available.
        request_id: Option<String>,
    },

    /// The provider rejected credentials or authorization.
    #[error("provider authentication failed: {message}")]
    AuthenticationFailure {
        /// Provider authentication message.
        message: String,
        /// Provider request id when available.
        request_id: Option<String>,
    },

    /// The provider returned an invalid or unsupported response shape.
    #[error("malformed provider response: {message}")]
    MalformedResponse {
        /// Parse or validation failure.
        message: String,
    },

    /// The provider failed after completing some durable output items.
    #[error("{provider} provider returned partial output: {message}")]
    PartialResponse {
        /// Provider name.
        provider: String,
        /// Model identifier used by the provider.
        model: String,
        /// Failure message.
        message: String,
        /// Completed output items that should still be persisted for audit.
        output: Vec<GenerateOutputItem>,
    },

    /// A provider-specific protocol failed.
    #[error("{provider} provider protocol error: {message}")]
    ProtocolError {
        /// Provider name.
        provider: String,
        /// Protocol failure message.
        message: String,
    },

    /// A provider operation timed out.
    #[error("{provider} provider timed out during {operation} after {seconds} seconds")]
    Timeout {
        /// Provider name.
        provider: String,
        /// Operation that timed out.
        operation: String,
        /// Timeout in seconds.
        seconds: u64,
    },

    /// The provider cannot currently be used.
    #[error("{provider} provider is unavailable: {message}")]
    ProviderUnavailable {
        /// Provider name.
        provider: String,
        /// Availability failure message.
        message: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    struct EchoProvider;

    impl ModelProvider for EchoProvider {
        async fn generate(
            &self,
            request: GenerateRequest,
        ) -> Result<GenerateResponse, ProviderError> {
            let text = request.input.render_for_token_count();

            Ok(GenerateResponse {
                output: vec![GenerateOutputItem::AssistantText { text }],
                provider: "mock".to_string(),
                model: request.model.unwrap_or_else(|| "mock-model".to_string()),
                response_id: Some("mock-response".to_string()),
                usage: Some(TokenUsage {
                    input_tokens: 1,
                    output_tokens: 1,
                    total_tokens: 2,
                    cached_input_tokens: None,
                }),
            })
        }
    }

    #[tokio::test]
    async fn model_provider_contract_can_be_implemented_by_a_mock() {
        let provider = EchoProvider;
        let response = provider
            .generate(GenerateRequest::text("hello").with_model("mock-1"))
            .await
            .expect("mock provider should return a response");

        assert_eq!(response.assistant_text(), "hello");
        assert_eq!(response.provider, "mock");
        assert_eq!(response.model, "mock-1");
    }

    #[tokio::test]
    async fn model_provider_default_streaming_delegates_to_generate_without_events() {
        let provider = EchoProvider;
        let mut events = Vec::new();
        let response = {
            let mut on_event = |event| events.push(event);
            provider
                .generate_streaming(GenerateRequest::text("hello stream"), &mut on_event)
                .await
                .expect("mock provider should return a streaming response")
        };

        assert_eq!(response.assistant_text(), "hello stream");
        assert_eq!(events, Vec::<GenerateStreamEvent>::new());
    }

    #[test]
    fn default_provider_context_metadata_is_unknown() {
        let provider = EchoProvider;

        assert_eq!(provider.context_metadata(None).context_window_tokens, None);
        assert_eq!(
            provider
                .context_metadata(Some("mock"))
                .default_output_reserve_tokens,
            None
        );
    }

    #[test]
    fn parses_noema_structured_response_envelope() {
        let output = output_items_from_text(
            r#"{"type":"noema_response","output":[{"kind":"assistant_text","text":"Hello"}]}"#
                .to_string(),
        )
        .expect("structured output");

        assert_eq!(
            output,
            vec![GenerateOutputItem::AssistantText {
                text: "Hello".to_string()
            }]
        );
    }

    #[test]
    fn treats_implicit_output_array_as_plain_assistant_text_when_envelope_is_optional() {
        let output = output_items_from_text(
            r#"{"output":[{"kind":"tool_call","id":"call_1","name":"search_memory","payload":{"query":"trains"}}]}"#
                .to_string(),
        )
        .expect("plain assistant text");

        assert_eq!(
            output,
            vec![GenerateOutputItem::AssistantText {
                text: r#"{"output":[{"kind":"tool_call","id":"call_1","name":"search_memory","payload":{"query":"trains"}}]}"#
                    .to_string()
            }]
        );
    }

    #[test]
    fn required_noema_response_rejects_implicit_output_array() {
        let error = required_output_items_from_text(
            r#"{"output":[{"kind":"assistant_text","text":"Hello"},{"kind":"memory_proposals","proposals":[]}]}"#
                .to_string(),
        )
        .unwrap_err();

        assert!(matches!(
            error,
            ProviderError::MalformedResponse { message }
                if message == "provider did not return a Noema structured response envelope"
        ));
    }

    #[test]
    fn parses_provider_action_output_items() {
        let output = output_items_from_text(
            r#"{"type":"noema_response","output":[{"kind":"assistant_text","text":"Done"},{"kind":"tool_call","id":"call_1","name":"search_memory","payload":{"query":"trains"}},{"kind":"approval_result","request_id":"approval_1","decision":"decline","payload":{"reason":"test"}}]}"#
                .to_string(),
        )
        .expect("structured output");

        assert!(matches!(
            &output[1],
            GenerateOutputItem::ToolCall { id: Some(id), name, .. }
                if id == "call_1" && name == "search_memory"
        ));
        assert!(matches!(
            &output[2],
            GenerateOutputItem::ApprovalResult {
                request_id: Some(id),
                decision,
                ..
            } if id == "approval_1" && decision == "decline"
        ));
    }

    #[test]
    fn required_noema_response_accepts_assistant_text_and_memory_proposals() {
        let output = required_output_items_from_text(
            r#"{"type":"noema_response","output":[{"kind":"assistant_text","text":"Hello"},{"kind":"memory_proposals","proposals":[]}]}"#
                .to_string(),
        )
        .expect("required structured output");

        assert_eq!(output.len(), 2);
        assert_eq!(
            output[0],
            GenerateOutputItem::AssistantText {
                text: "Hello".to_string()
            }
        );
        assert!(matches!(
            output[1],
            GenerateOutputItem::MemoryProposals { ref proposals } if proposals.is_empty()
        ));
    }

    #[test]
    fn required_noema_response_keeps_answer_when_memory_proposal_omits_evidence() {
        let output = required_output_items_from_text(
            r#"{"type":"noema_response","output":[{"kind":"assistant_text","text":"Hello"},{"kind":"memory_proposals","proposals":[{"content":"Kevin is debugging Foundation Local.","memory_type":"note","title":"Foundation Local debugging","confidence":0.9,"sensitivity":"normal","subjects":[{"id":"human:local","kind":"human","name":"Kevin","role":"about"}],"retrieval_hints":{"topics":["foundation local"],"keywords":["debugging"],"summary":"Kevin is debugging Foundation Local."},"risk_flags":[]}]}]}"#
                .to_string(),
        )
        .expect("required structured output should tolerate invalid proposal fields");

        assert_eq!(output.len(), 2);
        assert_eq!(
            output[0],
            GenerateOutputItem::AssistantText {
                text: "Hello".to_string()
            }
        );
        assert!(matches!(
            &output[1],
            GenerateOutputItem::MemoryProposals { proposals }
                if proposals.len() == 1 && proposals[0].evidence_excerpt.is_empty()
        ));
    }

    #[test]
    fn required_noema_response_accepts_tool_calls_with_memory_proposals() {
        let output = required_output_items_from_text(
            r#"{"type":"noema_response","output":[{"kind":"assistant_text","text":"I will check memory."},{"kind":"tool_call","id":"call_1","name":"search_memory","payload":{"query":"trains"}},{"kind":"memory_proposals","proposals":[]}]}"#
                .to_string(),
        )
        .expect("required structured output");

        assert!(matches!(
            &output[1],
            GenerateOutputItem::ToolCall { id: Some(id), name, .. }
                if id == "call_1" && name == "search_memory"
        ));
    }

    #[test]
    fn required_noema_response_recovers_envelope_after_leading_prose() {
        let output = required_output_items_from_text(
            "Searching Dex now.{\"type\":\"noema_response\",\"output\":[{\"kind\":\"tool_call\",\"id\":\"call_1\",\"name\":\"mcp.dex.search\",\"payload\":{\"query\":\"Gautam\"}},{\"kind\":\"assistant_text\",\"text\":\"Searching Dex now.\"},{\"kind\":\"memory_proposals\",\"proposals\":[]}]}"
                .to_string(),
        )
        .expect("embedded envelope");

        assert!(matches!(
            &output[0],
            GenerateOutputItem::ToolCall { id: Some(id), name, .. }
                if id == "call_1" && name == "mcp.dex.search"
        ));
        assert!(matches!(
            &output[1],
            GenerateOutputItem::AssistantText { text } if text == "Searching Dex now."
        ));
    }

    #[test]
    fn required_noema_response_treats_plain_text_as_assistant_text_with_empty_memory_proposals() {
        let output = required_output_items_from_text(
            "No. I didn't actually call a Notion write tool.".to_string(),
        )
        .expect("plain text fallback");

        assert_eq!(
            output,
            vec![
                GenerateOutputItem::AssistantText {
                    text: "No. I didn't actually call a Notion write tool.".to_string(),
                },
                GenerateOutputItem::MemoryProposals { proposals: vec![] },
            ]
        );
    }

    #[test]
    fn required_noema_response_recovers_envelope_before_trailing_prose() {
        let output = required_output_items_from_text(
            r#"{"type":"noema_response","output":[{"kind":"assistant_text","text":"Hello"},{"kind":"memory_proposals","proposals":[]}]} trailing prose"#
                .to_string(),
        )
        .expect("embedded envelope");

        assert_eq!(
            output[0],
            GenerateOutputItem::AssistantText {
                text: "Hello".to_string()
            }
        );
    }

    #[test]
    fn required_noema_response_requires_memory_proposals_item() {
        let error = required_output_items_from_text(
            r#"{"type":"noema_response","output":[{"kind":"assistant_text","text":"Hello"}]}"#
                .to_string(),
        )
        .unwrap_err();

        assert!(matches!(
            error,
            ProviderError::MalformedResponse { message }
                if message == "Noema structured response did not include memory_proposals"
        ));
    }

    #[test]
    fn required_noema_response_rejects_concatenated_stream_duplicate() {
        let duplicate = r#"{"type":"noema_response","output":[{"kind":"assistant_text","text":"Searching memory."},{"kind":"tool_call","id":"call_1","name":"search_memory","payload":{"scope_ids":["human:local"],"query":"","purpose":"answer_human_question","limit":8}},{"kind":"memory_proposals","proposals":[]}]}"#;
        let error = required_output_items_from_text(format!("{duplicate}{duplicate}")).unwrap_err();

        assert!(matches!(
            error,
            ProviderError::MalformedResponse { message }
                if message == "provider returned multiple Noema structured response envelopes"
        ));
    }
}
