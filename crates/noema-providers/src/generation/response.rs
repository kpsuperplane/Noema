use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

/// Structured response returned by a model provider.
#[derive(Debug, Clone, PartialEq)]
pub struct GenerateResponse {
    /// User-visible response items returned by the provider.
    pub responses: Vec<GenerateResponseItem>,
    /// Tool calls requested by the provider.
    pub tool_calls: Vec<GenerateToolCall>,
    /// Provider reasoning items returned for replay and display metadata.
    pub reasoning_items: Vec<GenerateReasoningItem>,
    /// Provider-hosted web-search activity completed during this response.
    pub hosted_web_searches: Vec<GenerateHostedWebSearch>,
    /// Provider-supplied source citations for the assistant response.
    pub citations: Vec<GenerateCitation>,
    /// Provider identifier that produced the response.
    pub provider: String,
    /// Model identifier used by the provider.
    pub model: String,
    /// Provider response identifier when one is available.
    pub response_id: Option<String>,
    /// Token usage reported by the provider when available.
    pub usage: Option<TokenUsage>,
}

/// Reasoning item returned by a provider response.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GenerateReasoningItem {
    /// Provider reasoning item id, when available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Opaque provider-encrypted reasoning payload.
    pub encrypted_content: Option<String>,
    /// Provider-authored, human-readable summaries intended for display.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub summary: Vec<String>,
    /// Exact provider reasoning details needed for stateless replay.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_details: Option<Vec<Value>>,
}

/// One provider-hosted web-search action completed inside generation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GenerateHostedWebSearch {
    /// Zero-based output-item index in the provider response.
    pub output_index: usize,
    /// Provider output item id, when available.
    pub id: Option<String>,
    /// Canonical Noema web tool represented by this hosted action.
    pub tool_name: String,
    /// Provider-normalized arguments for the canonical web tool.
    pub arguments: Value,
    /// Provider-normalized result for the canonical web tool.
    pub result: Value,
    /// Provider-reported lifecycle status.
    pub status: String,
}

/// One source citation supplied by a model provider.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GenerateCitation {
    /// Human-readable source title.
    pub title: String,
    /// Exact HTTP(S) source URL.
    pub url: String,
    /// Provider character index where the supported text begins.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_index: Option<usize>,
    /// Provider character index immediately after the supported text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_index: Option<usize>,
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
    /// A provider tool call item has started streaming.
    ToolCallStarted {
        /// Zero-based index of the tool call in the provider response.
        output_index: usize,
        /// Tool name reported by the provider.
        name: String,
    },
    /// A provider-hosted web search has started.
    HostedWebSearchStarted {
        /// Zero-based output-item index in the provider response.
        output_index: usize,
        /// Provider output item id, when available.
        id: Option<String>,
    },
}

impl GenerateResponse {
    pub(crate) fn normalize_markdown_messages(&mut self) {
        self.responses = std::mem::take(&mut self.responses)
            .into_iter()
            .flat_map(|item| match item {
                GenerateResponseItem::Text { phase, text } => super::split_markdown_messages(&text)
                    .into_iter()
                    .map(move |text| GenerateResponseItem::Text { phase, text })
                    .collect::<Vec<_>>(),
            })
            .collect();
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
            reasoning_items: Vec::new(),
            hosted_web_searches: Vec::new(),
            citations: Vec::new(),
            provider: provider.into(),
            model: model.into(),
            response_id: None,
            usage: None,
        }
    }

    /// Return all text response items concatenated in order.
    #[must_use]
    pub fn assistant_text(&self) -> String {
        self.responses
            .iter()
            .map(|item| match item {
                GenerateResponseItem::Text { text, .. } => text.as_str(),
            })
            .collect()
    }

    /// Return whether the response asks Noema to execute any tools.
    #[must_use]
    pub fn has_tool_calls(&self) -> bool {
        !self.tool_calls.is_empty()
    }
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
        }
    }
}

/// Whether a multiple-choice prompt expects one option or many.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum MultipleChoiceSelectionMode {
    /// One option answers the prompt immediately.
    PickOne,
    /// Several options may be selected before submitting.
    PickMany,
}

/// One option in an assistant multiple-choice prompt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct MultipleChoiceOption {
    /// Stable semantic option id.
    pub id: String,
    /// Human-visible option label.
    pub label: String,
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
    /// Runtime request for interactive authentication before a tool can continue.
    AuthenticationRequest {
        /// Durable authentication request id.
        id: String,
        /// Tool or operation waiting for authentication.
        method: String,
        /// Payload-free request context for transcript rendering.
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
