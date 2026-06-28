//! Provider-neutral generation contract.

use crate::memory_extraction::ExtractorMemoryProposal;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::future::Future;
use thiserror::Error;

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

/// Input and options for a provider generation call.
#[derive(Debug, Clone, PartialEq)]
pub struct GenerateRequest {
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

    if let Ok(value) = serde_json::from_str::<Value>(trimmed)
        && value.get("type").and_then(Value::as_str) == Some("noema_response")
    {
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
        return Ok(envelope.output);
    }

    if require_noema_response {
        return Err(ProviderError::MalformedResponse {
            message: "provider did not return a Noema structured response envelope".to_string(),
        });
    }

    Ok(vec![GenerateOutputItem::AssistantText { text }])
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

    /// The provider does not support a requested option.
    #[error("unsupported provider feature: {feature}")]
    UnsupportedFeature {
        /// Unsupported feature name.
        feature: String,
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
            let GenerateInput::Text(text) = request.input;

            Ok(GenerateResponse {
                output: vec![GenerateOutputItem::AssistantText { text }],
                provider: "mock".to_string(),
                model: request.model.unwrap_or_else(|| "mock-model".to_string()),
                response_id: Some("mock-response".to_string()),
                usage: Some(TokenUsage {
                    input_tokens: 1,
                    output_tokens: 1,
                    total_tokens: 2,
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
    fn required_noema_response_rejects_plain_text() {
        let error = required_output_items_from_text("Hello".to_string()).unwrap_err();

        assert!(matches!(
            error,
            ProviderError::MalformedResponse { message }
                if message == "provider did not return a Noema structured response envelope"
        ));
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
}
