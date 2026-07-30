use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{NoemaToolChoice, ProviderTool, ProviderToolTransport};

/// Scheduling priority for generation on providers with constrained local capacity.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum GenerationPriority {
    /// User-facing work that directly gates an interactive response.
    #[default]
    Foreground,
    /// Deferred or autonomous work that may wait behind interactive responses.
    Background,
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
    pub tools: Vec<ProviderTool>,
    /// Effective tool transport selected for this request.
    ///
    /// Runtime callers derive this from the selected provider/model
    /// capabilities and the effective catalog. Adapters must use it as the
    /// response-contract authority instead of inferring transport from the
    /// number of tools or returned calls.
    pub tool_transport: ProviderToolTransport,
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
            tool_transport: ProviderToolTransport::None,
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
            Self::Reasoning(reasoning) => {
                reasoning.encrypted_content.trim().is_empty()
                    && reasoning
                        .provider_details
                        .as_ref()
                        .is_none_or(Vec::is_empty)
            }
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
            Self::Reasoning(reasoning) => {
                let mut value = serde_json::json!({
                    "type": "reasoning",
                    "id": reasoning.id,
                    "encrypted_content": reasoning.encrypted_content,
                });
                if let Some(details) = reasoning
                    .provider_details
                    .as_ref()
                    .filter(|details| !details.is_empty())
                {
                    value["reasoning_details"] = Value::Array(details.clone());
                }
                value.to_string()
            }
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
    /// Exact provider reasoning details needed for stateless replay.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_details: Option<Vec<Value>>,
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
    #[cfg(feature = "adapters")]
    pub(crate) fn output_json_string(&self) -> String {
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
    /// Runtime/system state that is neither human- nor assistant-authored.
    System,
    /// Application-authored instructions and mutable runtime context.
    Developer,
    /// Human/user message.
    User,
    /// Assistant/model message.
    Assistant,
}

impl GenerateMessageRole {
    /// Provider-independent lower-case role name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Developer => "developer",
            Self::User => "user",
            Self::Assistant => "assistant",
        }
    }
}

/// Request-wide prompt-cache breakpoint placement policy.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PromptCacheMode {
    /// Let the provider place its default implicit breakpoint in addition to
    /// any explicit breakpoints supplied by Noema.
    #[default]
    Implicit,
    /// Use only the explicit breakpoints supplied by Noema.
    Explicit,
}

/// Minimum lifetime requested for provider prompt-cache entries.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PromptCacheTtl {
    /// Keep eligible prompt prefixes cached for at least thirty minutes.
    #[default]
    #[serde(rename = "30m")]
    ThirtyMinutes,
}

/// Provider-neutral request-wide prompt-cache controls.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PromptCacheOptions {
    /// Whether implicit provider breakpoints remain enabled.
    pub mode: PromptCacheMode,
    /// Minimum lifetime for cache entries written by this request.
    pub ttl: PromptCacheTtl,
}

/// Prompt-cache retention request for providers that support configurable caching.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PromptCacheRetention {
    /// Keep eligible prompt prefixes cached for 24 hours.
    #[serde(rename = "24h")]
    TwentyFourHours,
}

/// Provider-neutral reasoning effort for providers that expose explicit controls.
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

impl ReasoningEffort {
    /// Return the stable lowercase value stored in Noema persistence.
    #[must_use]
    pub const fn as_persistence_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Minimal => "minimal",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::XHigh => "xhigh",
        }
    }

    /// Parse a stable lowercase value read from Noema persistence.
    #[must_use]
    pub fn from_persistence_str(value: &str) -> Option<Self> {
        match value {
            "none" => Some(Self::None),
            "minimal" => Some(Self::Minimal),
            "low" => Some(Self::Low),
            "medium" => Some(Self::Medium),
            "high" => Some(Self::High),
            "xhigh" => Some(Self::XHigh),
            _ => None,
        }
    }
}

/// Provider-neutral optional generation controls.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GenerateOptions {
    /// Scheduling priority for providers that serialize generation requests.
    pub generation_priority: GenerationPriority,
    /// Optional maximum number of output tokens.
    pub max_output_tokens: Option<u32>,
    /// Optional sampling temperature.
    pub temperature: Option<f32>,
    /// Optional explicit reasoning effort for reasoning-capable providers/models.
    pub reasoning_effort: Option<ReasoningEffort>,
    /// Allow the provider to execute its hosted live-web search tool.
    pub hosted_web_search: bool,
    /// Provider prompt-cache retention request when supported.
    pub prompt_cache_retention: Option<PromptCacheRetention>,
    /// Request-wide prompt-cache controls when supported.
    pub prompt_cache_options: Option<PromptCacheOptions>,
    /// Zero-based filtered message indices that carry explicit cache breakpoints.
    pub prompt_cache_breakpoints: Vec<usize>,
    /// Opaque provider response id to continue from without replaying history.
    pub previous_response_id: Option<String>,
    /// Whether the provider should retain this response for later continuation.
    pub store_response: bool,
}
