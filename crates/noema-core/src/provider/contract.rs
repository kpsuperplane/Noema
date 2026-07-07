//! Provider-neutral generation contract.

use super::tools::{NoemaToolChoice, NoemaToolSpec, ProviderToolCapabilities};
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

    /// Return native tool-calling capabilities for this provider/model.
    fn tool_capabilities(&self, _model: Option<&str>) -> ProviderToolCapabilities {
        ProviderToolCapabilities::default()
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
    /// Provider-neutral model-visible tools for this request.
    pub tools: Vec<NoemaToolSpec>,
    /// Tool selection policy requested by Noema.
    pub tool_choice: NoemaToolChoice,
    /// Whether Noema allows the provider to emit independent tool calls in parallel.
    pub parallel_tool_calls: bool,
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
            tools: Vec::new(),
            tool_choice: NoemaToolChoice::default(),
            parallel_tool_calls: false,
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
    /// Ordered provider-neutral input items.
    Items(Vec<GenerateInputItem>),
    /// Provider-native tool result items for same-turn continuations.
    NativeToolResults(Vec<GenerateToolResultInput>),
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
            Self::Items(items) => items.iter().all(GenerateInputItem::is_empty),
            Self::NativeToolResults(results) => results.is_empty(),
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
            Self::Items(items) => items
                .iter()
                .filter(|item| !item.is_empty())
                .map(GenerateInputItem::render_for_token_count)
                .collect::<Vec<_>>()
                .join("\n"),
            Self::NativeToolResults(results) => {
                serde_json::to_string(results).unwrap_or_else(|_| "[]".to_string())
            }
        }
    }
}

/// One provider-neutral input item in durable model-visible history.
#[derive(Debug, Clone, PartialEq)]
pub enum GenerateInputItem {
    /// Role-tagged text message.
    Message(GenerateMessage),
    /// Provider-encrypted reasoning context for stateless replay.
    Reasoning(GenerateReasoningInput),
    /// Historical provider/model tool call.
    ToolCall(GenerateToolCallInput),
    /// Historical local tool result.
    ToolResult(GenerateToolResultInput),
}

impl GenerateInputItem {
    /// Return whether this item has no model-visible content.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        match self {
            Self::Message(message) => message.content.trim().is_empty(),
            Self::Reasoning(reasoning) => reasoning.encrypted_content.trim().is_empty(),
            Self::ToolCall(call) => call.call_id.trim().is_empty() || call.name.trim().is_empty(),
            Self::ToolResult(result) => {
                result.call_id.trim().is_empty() || result.name.trim().is_empty()
            }
        }
    }

    /// Render this item for providers or token counters that need text.
    #[must_use]
    pub fn render_for_token_count(&self) -> String {
        match self {
            Self::Message(message) => format!("{}: {}", message.role.as_str(), message.content),
            Self::Reasoning(reasoning) => serde_json::json!({
                "type": "reasoning",
                "id": reasoning.id,
                "encrypted_content": reasoning.encrypted_content,
            })
            .to_string(),
            Self::ToolCall(call) => serde_json::json!({
                "type": "function_call",
                "id": call.id,
                "call_id": call.call_id,
                "name": call.provider_name.as_ref().unwrap_or(&call.name),
                "canonical_name": call.name,
                "arguments": call.arguments,
            })
            .to_string(),
            Self::ToolResult(result) => serde_json::json!({
                "type": "function_call_output",
                "call_id": result.call_id,
                "name": result.name,
                "provider_name": result.provider_name,
                "success": result.success,
                "payload": result.payload,
            })
            .to_string(),
        }
    }
}

/// Provider-neutral encrypted reasoning item for stateless replay.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GenerateReasoningInput {
    /// Provider reasoning item id, when available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Opaque provider-encrypted reasoning payload.
    pub encrypted_content: String,
}

/// Provider-neutral native tool-call input for durable history replay.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GenerateToolCallInput {
    /// Provider item id for the original function-call item, when available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Provider-native tool call id used by the provider to correlate results.
    pub call_id: String,
    /// Canonical Noema tool or operation name.
    pub name: String,
    /// Provider-visible tool or operation name, when different from canonical.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_name: Option<String>,
    /// Original provider tool arguments.
    #[serde(default)]
    pub arguments: Value,
}

/// Provider-neutral native tool result input for same-turn continuation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GenerateToolResultInput {
    /// Provider item id for the original function-call item, when available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Provider-native tool call id used by the provider to correlate results.
    pub call_id: String,
    /// Canonical Noema tool or operation name.
    pub name: String,
    /// Provider-visible tool or operation name, when different from canonical.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_name: Option<String>,
    /// Original provider tool arguments.
    #[serde(default)]
    pub arguments: Value,
    /// Whether the local execution succeeded.
    pub success: bool,
    /// Runtime payload returned by Noema.
    #[serde(default)]
    pub payload: Value,
}

impl GenerateToolResultInput {
    /// Render the result body expected inside provider-native function output.
    #[must_use]
    pub fn output_json_string(&self) -> String {
        serde_json::json!({
            "call_id": self.call_id,
            "name": self.name,
            "provider_name": self.provider_name,
            "success": self.success,
            "payload": self.payload,
        })
        .to_string()
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

/// Provider-neutral reasoning effort for providers that expose explicit reasoning controls.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReasoningEffort {
    /// Disable explicit reasoning where the provider supports it.
    #[serde(rename = "none")]
    None,
    /// Minimal reasoning effort.
    #[serde(rename = "minimal")]
    Minimal,
    /// Low reasoning effort.
    #[serde(rename = "low")]
    Low,
    /// Medium reasoning effort.
    #[serde(rename = "medium")]
    Medium,
    /// High reasoning effort.
    #[serde(rename = "high")]
    High,
    /// Extra-high reasoning effort.
    #[serde(rename = "xhigh")]
    XHigh,
}

/// Provider-neutral optional generation controls.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GenerateOptions {
    /// Optional maximum number of output tokens.
    pub max_output_tokens: Option<u32>,
    /// Optional sampling temperature.
    pub temperature: Option<f32>,
    /// Optional explicit reasoning effort for reasoning-capable providers/models.
    pub reasoning_effort: Option<ReasoningEffort>,
    /// Require a strict Noema response object with response fields and memory proposals.
    pub require_noema_response: bool,
    /// Provider prompt-cache retention request when supported.
    pub prompt_cache_retention: Option<PromptCacheRetention>,
}

/// Structured response returned by a model provider.
#[derive(Debug, Clone, PartialEq)]
pub struct GenerateResponse {
    /// User-visible response items returned by the provider.
    pub responses: Vec<GenerateResponseItem>,
    /// Tool calls requested by the provider.
    pub tool_calls: Vec<GenerateToolCall>,
    /// Memory proposals emitted by the provider.
    pub memory_proposals: Vec<ExtractorMemoryProposal>,
    /// Opaque encrypted reasoning items returned by the provider for replay.
    pub reasoning_items: Vec<GenerateReasoningItem>,
    /// Whether this response needs tool execution or completes the turn.
    pub response_status: GenerateResponseStatus,
    /// Provider identifier that produced the response.
    pub provider: String,
    /// Model identifier used by the provider.
    pub model: String,
    /// Provider response identifier when one is available.
    pub response_id: Option<String>,
    /// Token usage reported by the provider when available.
    pub usage: Option<TokenUsage>,
}

/// Encrypted reasoning item returned by a provider response.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GenerateReasoningItem {
    /// Provider reasoning item id, when available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Opaque provider-encrypted reasoning payload.
    pub encrypted_content: Option<String>,
}

/// Ephemeral events emitted while a provider response is still generating.
#[derive(Debug, Clone, PartialEq)]
pub enum GenerateStreamEvent {
    /// Incremental human-visible assistant text.
    AssistantTextDelta {
        /// Zero-based index of the response item being streamed.
        response_index: usize,
        /// Text delta received from the provider.
        delta: String,
    },
    /// A non-empty memory proposal block has started streaming.
    MemoryProposalsStarted,
    /// A provider tool call item has started streaming.
    ToolCallStarted {
        /// Zero-based index of the tool call in the provider response.
        output_index: usize,
        /// Tool name reported by the provider.
        name: String,
    },
}

impl GenerateResponse {
    /// Build a provider response from a parsed Noema response object.
    #[must_use]
    pub fn from_parsed(
        parsed: ParsedNoemaResponse,
        provider: impl Into<String>,
        model: impl Into<String>,
        response_id: Option<String>,
        usage: Option<TokenUsage>,
    ) -> Self {
        Self {
            responses: parsed.responses,
            tool_calls: parsed.tool_calls,
            memory_proposals: parsed.memory_proposals,
            reasoning_items: Vec::new(),
            response_status: parsed.response_status,
            provider: provider.into(),
            model: model.into(),
            response_id,
            usage,
        }
    }

    /// Build a final text response for provider adapters and tests.
    #[must_use]
    pub fn final_text(
        text: impl Into<String>,
        provider: impl Into<String>,
        model: impl Into<String>,
    ) -> Self {
        Self {
            responses: vec![GenerateResponseItem::Text {
                phase: None,
                text: text.into(),
            }],
            tool_calls: Vec::new(),
            memory_proposals: Vec::new(),
            reasoning_items: Vec::new(),
            response_status: GenerateResponseStatus::Final,
            provider: provider.into(),
            model: model.into(),
            response_id: None,
            usage: None,
        }
    }

    /// Attach provider-encrypted reasoning items to this response.
    #[must_use]
    pub fn with_reasoning_items(mut self, reasoning_items: Vec<GenerateReasoningItem>) -> Self {
        self.reasoning_items = reasoning_items;
        self
    }

    /// Return all text response items concatenated in order.
    #[must_use]
    pub fn assistant_text(&self) -> String {
        self.responses
            .iter()
            .filter_map(|item| match item {
                GenerateResponseItem::Text { text, .. } => Some(text.as_str()),
                GenerateResponseItem::Structured { .. } => None,
            })
            .collect()
    }

    /// Return whether the response asks Noema to execute any tools.
    #[must_use]
    pub fn has_tool_calls(&self) -> bool {
        !self.tool_calls.is_empty()
    }
}

/// Provider-declared status for a Noema response.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GenerateResponseStatus {
    /// The response contains tool calls that Noema should execute.
    NeedsTools,
    /// The response is the terminal assistant response for this turn.
    Final,
}

/// User-visible phase for assistant text within one provider turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssistantTextPhase {
    /// Mid-turn assistant text such as preamble, status, or progress narration.
    Commentary,
    /// Terminal answer text for the current user-visible turn.
    FinalAnswer,
}

impl AssistantTextPhase {
    /// Return a stable storage string.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Commentary => "commentary",
            Self::FinalAnswer => "final_answer",
        }
    }

    /// Infer a display phase for provider text that omitted phase.
    #[must_use]
    pub fn effective_for_response_item(
        item: &GenerateResponseItem,
        provider_phase_has_tools: bool,
    ) -> Self {
        match item {
            GenerateResponseItem::Text {
                phase: Some(phase), ..
            } => *phase,
            GenerateResponseItem::Text { phase: None, .. } if provider_phase_has_tools => {
                Self::Commentary
            }
            GenerateResponseItem::Text { phase: None, .. } => Self::FinalAnswer,
            GenerateResponseItem::Structured { .. } => Self::FinalAnswer,
        }
    }
}

/// One user-visible response item.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum GenerateResponseItem {
    /// Human-visible assistant text.
    Text {
        /// Whether the text is mid-turn commentary or the final answer.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        phase: Option<AssistantTextPhase>,
        /// Text to show in the transcript.
        text: String,
    },
    /// Future rich structured output payload.
    Structured {
        /// Stable schema identifier for the payload.
        schema: String,
        /// Provider-produced payload for that schema.
        payload: Value,
    },
}

/// One provider-requested tool call.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GenerateToolCall {
    /// Provider item id or tool-call id, when available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Provider-native call id used to correlate native tool results.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_call_id: Option<String>,
    /// Provider-visible tool or operation name, when different from canonical.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_name: Option<String>,
    /// Canonical Noema tool or operation name.
    pub name: String,
    /// Provider payload for audit and replay.
    #[serde(default)]
    pub payload: Value,
}

/// Runtime action item persisted after provider output is interpreted.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum GenerateActionItem {
    /// Runtime or provider-reported tool invocation.
    ToolCall {
        /// Provider item id or tool-call id, when available.
        id: Option<String>,
        /// Provider-native call id for result correlation, when distinct.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        provider_call_id: Option<String>,
        /// Provider-visible tool or operation name, when distinct.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        provider_name: Option<String>,
        /// Tool or operation name.
        name: String,
        /// Provider payload for audit and replay.
        payload: Value,
    },
    /// Runtime or provider-reported tool result.
    ToolResult {
        /// Provider tool-call id, when available.
        call_id: Option<String>,
        /// Provider-native call id used for result correlation, when distinct.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        provider_call_id: Option<String>,
        /// Provider-visible tool or operation name, when distinct.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        provider_name: Option<String>,
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
}

#[cfg(test)]
mod reasoning_effort_tests {
    use super::*;

    #[test]
    fn reasoning_effort_serializes_lowercase_api_values() {
        assert_eq!(
            serde_json::to_string(&ReasoningEffort::None).unwrap(),
            "\"none\""
        );
        assert_eq!(
            serde_json::to_string(&ReasoningEffort::Minimal).unwrap(),
            "\"minimal\""
        );
        assert_eq!(
            serde_json::to_string(&ReasoningEffort::Low).unwrap(),
            "\"low\""
        );
        assert_eq!(
            serde_json::to_string(&ReasoningEffort::Medium).unwrap(),
            "\"medium\""
        );
        assert_eq!(
            serde_json::to_string(&ReasoningEffort::High).unwrap(),
            "\"high\""
        );
        assert_eq!(
            serde_json::to_string(&ReasoningEffort::XHigh).unwrap(),
            "\"xhigh\""
        );
    }

    #[test]
    fn reasoning_effort_deserializes_lowercase_api_values() {
        assert_eq!(
            serde_json::from_str::<ReasoningEffort>("\"xhigh\"").unwrap(),
            ReasoningEffort::XHigh
        );
        assert!(serde_json::from_str::<ReasoningEffort>("\"extreme\"").is_err());
    }
}

/// Parse a provider text payload into optional structured Noema response items.
///
/// Text without a response object is treated as one assistant text item.
///
/// # Errors
///
/// Returns [`ProviderError::MalformedResponse`] when a response-shaped object
/// is present but does not match the structured response contract.
pub fn output_items_from_text(text: String) -> Result<Vec<GenerateResponseItem>, ProviderError> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err(ProviderError::MalformedResponse {
            message: "provider produced empty output".to_string(),
        });
    }
    if let Ok(Some(response)) = noema_response_from_text(trimmed.to_string()) {
        return Ok(response.responses);
    }
    Ok(vec![GenerateResponseItem::Text { phase: None, text }])
}

/// Parse a provider text payload that must be a Noema structured response.
///
/// # Errors
///
/// Returns [`ProviderError::MalformedResponse`] when the payload is not a
/// strict Noema response object contract.
pub fn required_noema_response_from_text(
    text: String,
) -> Result<ParsedNoemaResponse, ProviderError> {
    noema_response_from_text_with_mode(text, true)
}

/// Parse required Noema text when executable tool calls arrived through a
/// provider-native channel.
///
/// Native calls satisfy the `needs_tools` requirement, so the JSON envelope
/// must not also include legacy JSON `tool_calls`.
///
/// # Errors
///
/// Returns [`ProviderError::MalformedResponse`] when the text is not a Noema
/// structured response object, includes legacy JSON tool calls, or includes
/// final-answer text alongside native tool calls.
pub fn required_noema_response_from_text_with_native_tool_calls(
    text: String,
    native_tool_calls: Vec<GenerateToolCall>,
) -> Result<ParsedNoemaResponse, ProviderError> {
    if native_tool_calls.is_empty() {
        return required_noema_response_from_text(text);
    }

    let mut parsed = noema_response_from_text_without_required_validation(text)?;
    if !parsed.tool_calls.is_empty() {
        return Err(ProviderError::MalformedResponse {
            message: "native tool response cannot include legacy JSON tool_calls".to_string(),
        });
    }
    if parsed.responses.iter().any(is_final_answer_text_response) {
        return Err(ProviderError::MalformedResponse {
            message: "native tool response cannot include final_answer text".to_string(),
        });
    }

    parsed.tool_calls = native_tool_calls;
    parsed.response_status = GenerateResponseStatus::NeedsTools;
    Ok(parsed)
}

/// Parse provider text into a Noema structured response when it contains the
/// response object contract.
///
/// # Errors
///
/// Returns [`ProviderError::MalformedResponse`] when a response-shaped object
/// is present but invalid.
pub fn noema_response_from_text(
    text: String,
) -> Result<Option<ParsedNoemaResponse>, ProviderError> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err(ProviderError::MalformedResponse {
            message: "provider produced empty output".to_string(),
        });
    }

    if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
        return noema_response_from_structured_value(value, false);
    }

    embedded_noema_response(trimmed, true)
}

#[derive(Debug, Clone, PartialEq)]
/// Parsed provider-facing Noema response object before provider metadata is attached.
pub struct ParsedNoemaResponse {
    /// User-visible response items returned by the provider.
    pub responses: Vec<GenerateResponseItem>,
    /// Tool calls requested by the provider.
    pub tool_calls: Vec<GenerateToolCall>,
    /// Memory proposals emitted by the provider.
    pub memory_proposals: Vec<ExtractorMemoryProposal>,
    /// Whether this response needs tool execution or completes the turn.
    pub response_status: GenerateResponseStatus,
}

impl ParsedNoemaResponse {
    /// Return all text response items concatenated in order.
    #[must_use]
    pub fn assistant_text(&self) -> String {
        self.responses
            .iter()
            .filter_map(|item| match item {
                GenerateResponseItem::Text { text, .. } => Some(text.as_str()),
                GenerateResponseItem::Structured { .. } => None,
            })
            .collect()
    }
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

fn noema_response_from_text_with_mode(
    text: String,
    require_noema_response: bool,
) -> Result<ParsedNoemaResponse, ProviderError> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err(ProviderError::MalformedResponse {
            message: "provider produced empty output".to_string(),
        });
    }

    if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
        if let Some(response) = noema_response_from_structured_value(value, require_noema_response)?
        {
            return Ok(response);
        }
        if require_noema_response {
            return Err(ProviderError::MalformedResponse {
                message: "provider did not return a Noema structured response object".to_string(),
            });
        }
    }

    if require_noema_response && let Some(response) = embedded_noema_response(trimmed, true)? {
        return Ok(response);
    }

    Err(ProviderError::MalformedResponse {
        message: "provider did not return a Noema structured response object".to_string(),
    })
}

fn noema_response_from_text_without_required_validation(
    text: String,
) -> Result<ParsedNoemaResponse, ProviderError> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err(ProviderError::MalformedResponse {
            message: "provider produced empty output".to_string(),
        });
    }

    if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
        if let Some(response) = noema_response_from_structured_value(value, false)? {
            return Ok(response);
        }
        return Err(ProviderError::MalformedResponse {
            message: "provider did not return a Noema structured response object".to_string(),
        });
    }

    if let Some(response) = embedded_noema_response(trimmed, false)? {
        return Ok(response);
    }

    Err(ProviderError::MalformedResponse {
        message: "provider did not return a Noema structured response object".to_string(),
    })
}

fn noema_response_from_structured_value(
    value: Value,
    require_noema_response: bool,
) -> Result<Option<ParsedNoemaResponse>, ProviderError> {
    if !looks_like_noema_response_object(&value) && !require_noema_response {
        return Ok(None);
    }

    let response_object: NoemaResponseObject =
        serde_json::from_value(value).map_err(|source| ProviderError::MalformedResponse {
            message: format!("invalid Noema structured response: {source}"),
        })?;
    let parsed = ParsedNoemaResponse {
        responses: response_object.responses,
        tool_calls: response_object.tool_calls,
        memory_proposals: Vec::new(),
        response_status: response_object.response_status,
    };
    if require_noema_response {
        validate_required_noema_response(&parsed)?;
    }
    Ok(Some(parsed))
}

fn looks_like_noema_response_object(value: &Value) -> bool {
    value.get("response_status").is_some()
        || value.get("responses").is_some()
        || value.get("tool_calls").is_some()
        || value.get("memory_proposals").is_some()
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct NoemaResponseObject {
    response_status: GenerateResponseStatus,
    responses: Vec<GenerateResponseItem>,
    tool_calls: Vec<GenerateToolCall>,
}

fn validate_required_noema_response(response: &ParsedNoemaResponse) -> Result<(), ProviderError> {
    match response.response_status {
        GenerateResponseStatus::Final => {
            if !response.tool_calls.is_empty() {
                return Err(ProviderError::MalformedResponse {
                    message: "Noema final response cannot include tool_calls".to_string(),
                });
            }
            if !has_non_empty_response_item(&response.responses) {
                return Err(ProviderError::MalformedResponse {
                    message: "Noema final response did not include any response items".to_string(),
                });
            }
            if response.responses.iter().any(is_commentary_text_response) {
                return Err(ProviderError::MalformedResponse {
                    message: "Noema final response cannot include commentary text".to_string(),
                });
            }
        }
        GenerateResponseStatus::NeedsTools => {
            if response.tool_calls.is_empty() {
                return Err(ProviderError::MalformedResponse {
                    message: "Noema needs_tools response did not include tool_calls".to_string(),
                });
            }
            if response.responses.iter().any(is_final_answer_text_response) {
                return Err(ProviderError::MalformedResponse {
                    message: "Noema needs_tools response cannot include final_answer text"
                        .to_string(),
                });
            }
        }
    }

    Ok(())
}

fn has_non_empty_response_item(responses: &[GenerateResponseItem]) -> bool {
    responses.iter().any(|item| match item {
        GenerateResponseItem::Text { text, .. } => !text.trim().is_empty(),
        GenerateResponseItem::Structured { .. } => true,
    })
}

fn is_commentary_text_response(item: &GenerateResponseItem) -> bool {
    matches!(
        item,
        GenerateResponseItem::Text {
            phase: Some(AssistantTextPhase::Commentary),
            ..
        }
    )
}

fn is_final_answer_text_response(item: &GenerateResponseItem) -> bool {
    matches!(
        item,
        GenerateResponseItem::Text {
            phase: Some(AssistantTextPhase::FinalAnswer),
            ..
        }
    )
}

fn embedded_noema_response(
    text: &str,
    require_noema_response: bool,
) -> Result<Option<ParsedNoemaResponse>, ProviderError> {
    let mut output = None;
    for candidate in balanced_json_object_candidates(text) {
        let Ok(value) = serde_json::from_str::<Value>(candidate) else {
            continue;
        };
        if !looks_like_noema_response_object(&value) {
            continue;
        }
        let Some(candidate_output) =
            noema_response_from_structured_value(value, require_noema_response)?
        else {
            continue;
        };
        if let Some(existing_output) = &output {
            if existing_output == &candidate_output {
                continue;
            }
            return Err(ProviderError::MalformedResponse {
                message: "provider returned multiple Noema structured response objects".to_string(),
            });
        }
        output = Some(candidate_output);
    }
    Ok(output)
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

    /// The provider failed after completing some durable action items.
    #[error("{provider} provider returned partial output: {message}")]
    PartialResponse {
        /// Provider name.
        provider: String,
        /// Model identifier used by the provider.
        model: String,
        /// Failure message.
        message: String,
        /// Completed action items that should still be persisted for audit.
        output: Vec<GenerateActionItem>,
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
    use serde_json::json;

    struct EchoProvider;

    impl ModelProvider for EchoProvider {
        async fn generate(
            &self,
            request: GenerateRequest,
        ) -> Result<GenerateResponse, ProviderError> {
            let text = request.input.render_for_token_count();

            Ok(GenerateResponse {
                responses: vec![GenerateResponseItem::Text { phase: None, text }],
                tool_calls: Vec::new(),
                memory_proposals: Vec::new(),
                reasoning_items: Vec::new(),
                response_status: GenerateResponseStatus::Final,
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
    fn generate_tool_call_preserves_provider_call_id_separately() {
        let call = GenerateToolCall {
            id: Some("item_1".to_string()),
            provider_call_id: Some("call_1".to_string()),
            provider_name: Some("search_memory".to_string()),
            name: "search_memory".to_string(),
            payload: json!({"query": "trains"}),
        };

        assert_eq!(call.id.as_deref(), Some("item_1"));
        assert_eq!(call.provider_call_id.as_deref(), Some("call_1"));
    }

    #[test]
    fn output_items_from_text_parses_response_object_responses() {
        let output = output_items_from_text(
            r#"{"response_status":"final","responses":[{"kind":"text","phase":"final_answer","text":"Hello"}],"tool_calls":[]}"#.to_string(),
        )
        .expect("structured output");

        assert_eq!(
            output,
            vec![GenerateResponseItem::Text {
                phase: Some(AssistantTextPhase::FinalAnswer),
                text: "Hello".to_string()
            }]
        );
    }

    #[test]
    fn output_items_from_text_treats_unrelated_json_as_plain_text() {
        let output = output_items_from_text(
            r#"{"output":[{"kind":"tool_call","id":"call_1","name":"search_memory","payload":{"query":"trains"}}]}"#
                .to_string(),
        )
        .expect("plain assistant text");

        assert_eq!(
            output,
            vec![GenerateResponseItem::Text {
                phase: None,
                text: r#"{"output":[{"kind":"tool_call","id":"call_1","name":"search_memory","payload":{"query":"trains"}}]}"#
                    .to_string()
            }]
        );
    }

    #[test]
    fn required_noema_response_rejects_old_output_array_contract() {
        let error = required_noema_response_from_text(
            r#"{"output":[{"kind":"assistant_text","text":"Hello"},{"kind":"memory_proposals","proposals":[]}]}"#
                .to_string(),
        )
        .unwrap_err();

        assert!(matches!(error, ProviderError::MalformedResponse { .. }));
    }

    #[test]
    fn required_noema_response_accepts_object_contract_final_text() {
        let response = required_noema_response_from_text(
            r#"{"response_status":"final","responses":[{"kind":"text","phase":"final_answer","text":"Done."}],"tool_calls":[]}"#
                .to_string(),
        )
        .expect("required structured response");

        assert_eq!(response.response_status, GenerateResponseStatus::Final);
        assert_eq!(
            response.responses,
            vec![GenerateResponseItem::Text {
                phase: Some(AssistantTextPhase::FinalAnswer),
                text: "Done.".to_string(),
            }]
        );
        assert!(response.tool_calls.is_empty());
        assert!(response.memory_proposals.is_empty());
    }

    #[test]
    fn noema_response_no_longer_requires_memory_proposals() {
        let parsed = required_noema_response_from_text(
            r#"{"response_status":"final","responses":[{"kind":"text","phase":"final_answer","text":"ok"}],"tool_calls":[]}"#
                .to_string(),
        )
        .expect("response parses");

        assert_eq!(parsed.responses.len(), 1);
        assert!(parsed.tool_calls.is_empty());
    }

    #[test]
    fn noema_response_rejects_memory_proposals_field() {
        let error = required_noema_response_from_text(
            r#"{"response_status":"final","responses":[],"tool_calls":[],"memory_proposals":[]}"#
                .to_string(),
        )
        .expect_err("legacy field rejected");

        assert!(error.to_string().contains("memory_proposals"));
    }

    #[test]
    fn required_noema_response_accepts_silent_tool_calls() {
        let response = required_noema_response_from_text(
            r#"{"response_status":"needs_tools","responses":[],"tool_calls":[{"id":"call_1","name":"search_memory","payload":{"scope_ids":["human:local"],"query":"","purpose":"answer_human_question","limit":8}}]}"#
                .to_string(),
        )
        .expect("silent tool call response");

        assert_eq!(response.response_status, GenerateResponseStatus::NeedsTools);
        assert!(response.responses.is_empty());
        assert_eq!(response.tool_calls.len(), 1);
        assert_eq!(response.tool_calls[0].id.as_deref(), Some("call_1"));
        assert_eq!(response.tool_calls[0].name, "search_memory");
    }

    #[test]
    fn required_noema_response_accepts_multiple_silent_tool_calls() {
        let response = required_noema_response_from_text(
            r#"{"response_status":"needs_tools","responses":[],"tool_calls":[{"id":"call_1","name":"search_memory","payload":{"query":"trains"}},{"id":"call_2","name":"mcp.docs.read","payload":{"document_id":"doc_1"}}]}"#
                .to_string(),
        )
        .expect("multiple silent tool call response");

        assert_eq!(response.response_status, GenerateResponseStatus::NeedsTools);
        assert!(response.responses.is_empty());
        assert_eq!(
            response
                .tool_calls
                .iter()
                .map(|call| call.name.as_str())
                .collect::<Vec<_>>(),
            vec!["search_memory", "mcp.docs.read"]
        );
    }

    #[test]
    fn required_noema_response_rejects_memory_proposals() {
        let error = required_noema_response_from_text(
            r#"{"response_status":"final","responses":[{"kind":"text","phase":"final_answer","text":"Hello"}],"tool_calls":[],"memory_proposals":[{"content":"Kevin is debugging Foundation Local.","memory_type":"note","title":"Foundation Local debugging","confidence":0.9,"sensitivity":"normal","subjects":[{"id":"human:local","kind":"human","name":"Kevin","role":"about"}],"retrieval_hints":{"topics":["foundation local"],"keywords":["debugging"],"summary":"Kevin is debugging Foundation Local."},"risk_flags":[]}]}"#
                .to_string(),
        )
        .expect_err("legacy proposals should be rejected");

        assert!(error.to_string().contains("memory_proposals"));
    }

    #[test]
    fn assistant_text_phase_defaults_to_final_without_runtime_tools() {
        let phase = AssistantTextPhase::effective_for_response_item(
            &GenerateResponseItem::Text {
                phase: None,
                text: "Done.".to_string(),
            },
            false,
        );
        assert_eq!(phase, AssistantTextPhase::FinalAnswer);
    }

    #[test]
    fn assistant_text_phase_defaults_to_commentary_with_runtime_tools() {
        let phase = AssistantTextPhase::effective_for_response_item(
            &GenerateResponseItem::Text {
                phase: None,
                text: "Checking that now.".to_string(),
            },
            true,
        );
        assert_eq!(phase, AssistantTextPhase::Commentary);
    }

    #[test]
    fn required_noema_response_rejects_final_without_responses() {
        let error = required_noema_response_from_text(
            r#"{"response_status":"final","responses":[],"tool_calls":[]}"#.to_string(),
        )
        .unwrap_err();

        assert!(matches!(
            error,
            ProviderError::MalformedResponse { message }
                if message == "Noema final response did not include any response items"
        ));
    }

    #[test]
    fn required_noema_response_rejects_tool_calls_in_final_response() {
        let error = required_noema_response_from_text(
            r#"{"response_status":"final","responses":[{"kind":"text","phase":"final_answer","text":"Done."}],"tool_calls":[{"id":"call_1","name":"search_memory","payload":{}}]}"#
                .to_string(),
        )
        .unwrap_err();

        assert!(matches!(
            error,
            ProviderError::MalformedResponse { message }
                if message == "Noema final response cannot include tool_calls"
        ));
    }

    #[test]
    fn required_noema_response_rejects_final_answer_in_needs_tools_response() {
        let error = required_noema_response_from_text(
            r#"{"response_status":"needs_tools","responses":[{"kind":"text","phase":"final_answer","text":"Done."}],"tool_calls":[{"id":"call_1","name":"search_memory","payload":{}}]}"#
                .to_string(),
        )
        .unwrap_err();

        assert!(matches!(
            error,
            ProviderError::MalformedResponse { message }
                if message == "Noema needs_tools response cannot include final_answer text"
        ));
    }

    #[test]
    fn required_noema_response_rejects_plain_text() {
        let error = required_noema_response_from_text(
            "No. I didn't actually call a Notion write tool.".to_string(),
        )
        .unwrap_err();

        assert!(matches!(
            error,
            ProviderError::MalformedResponse { message }
                if message == "provider did not return a Noema structured response object"
        ));
    }

    #[test]
    fn required_noema_response_recovers_object_after_leading_prose() {
        let response = required_noema_response_from_text(
            r#"Searching Dex now.{"response_status":"needs_tools","responses":[{"kind":"text","phase":"commentary","text":"Searching Dex now."}],"tool_calls":[{"id":"call_1","name":"mcp.dex.search","payload":{"query":"Gautam"}}]}"#
                .to_string(),
        )
        .expect("embedded response object");

        assert_eq!(
            response.responses[0],
            GenerateResponseItem::Text {
                phase: Some(AssistantTextPhase::Commentary),
                text: "Searching Dex now.".to_string(),
            },
        );
        assert_eq!(response.tool_calls[0].name, "mcp.dex.search");
    }

    #[test]
    fn required_noema_response_recovers_object_before_trailing_prose() {
        let response = required_noema_response_from_text(
            r#"{"response_status":"final","responses":[{"kind":"text","phase":"final_answer","text":"Hello"}],"tool_calls":[]} trailing prose"#
                .to_string(),
        )
        .expect("embedded response object");

        assert_eq!(response.assistant_text(), "Hello");
    }

    #[test]
    fn required_noema_response_accepts_exact_concatenated_stream_duplicate() {
        let duplicate = r#"{"response_status":"final","responses":[{"kind":"text","phase":"final_answer","text":"hm, I don’t have fresh recent-opening data unless I search the web. Want me to check current Seattle restaurant openings now?"}],"tool_calls":[]}"#;
        let response =
            required_noema_response_from_text(format!("{duplicate}{duplicate}")).unwrap();

        assert_eq!(response.response_status, GenerateResponseStatus::Final);
        assert_eq!(
            response.assistant_text(),
            "hm, I don’t have fresh recent-opening data unless I search the web. Want me to check current Seattle restaurant openings now?"
        );
        assert!(response.tool_calls.is_empty());
    }

    #[test]
    fn required_noema_response_rejects_conflicting_concatenated_stream_responses() {
        let first = r#"{"response_status":"final","responses":[{"kind":"text","phase":"final_answer","text":"One."}],"tool_calls":[]}"#;
        let second = r#"{"response_status":"final","responses":[{"kind":"text","phase":"final_answer","text":"Two."}],"tool_calls":[]}"#;
        let error = required_noema_response_from_text(format!("{first}{second}")).unwrap_err();

        assert!(matches!(
            error,
            ProviderError::MalformedResponse { message }
                if message == "provider returned multiple Noema structured response objects"
        ));
    }

    #[test]
    fn action_items_serialize_tool_result_shape() {
        let item = GenerateActionItem::ToolResult {
            call_id: Some("call_1".to_string()),
            provider_call_id: Some("provider_call_1".to_string()),
            provider_name: Some("search_memory".to_string()),
            name: Some("search_memory".to_string()),
            success: Some(true),
            payload: json!({"ok": true}),
        };

        assert_eq!(
            serde_json::to_value(item).expect("json"),
            json!({
                "kind": "tool_result",
                "call_id": "call_1",
                "provider_call_id": "provider_call_1",
                "provider_name": "search_memory",
                "name": "search_memory",
                "success": true,
                "payload": {"ok": true}
            })
        );
    }
}
