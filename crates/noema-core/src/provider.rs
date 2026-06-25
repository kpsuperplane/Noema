//! Provider-neutral generation contract.

use crate::memory_extraction::ExtractorMemoryProposal;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

// V1 keeps the provider contract as a native async trait and does not expose
// `dyn ModelProvider`, so the public future-bound tradeoff is intentional.
#[allow(async_fn_in_trait)]
/// A model backend that can produce structured output from a generation request.
pub trait ModelProvider: Send + Sync {
    /// Generate a response for the given request.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError`] when the request is invalid, credentials are
    /// missing, the backend is unavailable, the backend returns an API error,
    /// or its response cannot be parsed.
    async fn generate(&self, request: GenerateRequest) -> Result<GenerateResponse, ProviderError>;
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

impl GenerateResponse {
    /// Return all assistant text output concatenated in order.
    #[must_use]
    pub fn assistant_text(&self) -> String {
        self.output
            .iter()
            .filter_map(|item| match item {
                GenerateOutputItem::AssistantText { text } => Some(text.as_str()),
                GenerateOutputItem::MemoryProposals { .. }
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
        return Ok(envelope.output);
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
}
