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
}

impl GenerateResponse {
    /// Build a provider response from a parsed Noema response object.
    #[must_use]
    #[cfg(any(feature = "adapters", feature = "local-models"))]
    pub(crate) fn from_parsed(
        parsed: ParsedNoemaResponse,
        provider: impl Into<String>,
        model: impl Into<String>,
        response_id: Option<String>,
        usage: Option<TokenUsage>,
    ) -> Self {
        Self {
            responses: parsed.responses,
            tool_calls: parsed.tool_calls,
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
            reasoning_items: Vec::new(),
            response_status: GenerateResponseStatus::Final,
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
            .filter_map(|item| match item {
                GenerateResponseItem::Text { text, .. } => Some(text.as_str()),
                GenerateResponseItem::MultipleChoice { .. }
                | GenerateResponseItem::Structured { .. } => None,
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
            }
            | GenerateResponseItem::MultipleChoice {
                phase: Some(phase), ..
            } => *phase,
            GenerateResponseItem::Text { phase: None, .. } if provider_phase_has_tools => {
                Self::Commentary
            }
            GenerateResponseItem::Text { phase: None, .. }
            | GenerateResponseItem::MultipleChoice { phase: None, .. }
            | GenerateResponseItem::Structured { .. } => Self::FinalAnswer,
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
    /// Human-visible multiple-choice prompt.
    MultipleChoice {
        /// Whether the prompt is mid-turn commentary or a final answer.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        phase: Option<AssistantTextPhase>,
        /// Question or instruction to show above the options.
        prompt: String,
        /// Selection behavior for the options.
        selection_mode: MultipleChoiceSelectionMode,
        /// Ordered prompt options.
        options: Vec<MultipleChoiceOption>,
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

/// Parsed provider-facing Noema response object before metadata is attached.
#[derive(Debug, Clone, PartialEq)]
#[cfg(any(test, feature = "adapters", feature = "local-models"))]
pub(crate) struct ParsedNoemaResponse {
    /// User-visible response items returned by the provider.
    pub responses: Vec<GenerateResponseItem>,
    /// Tool calls requested by the provider.
    pub tool_calls: Vec<GenerateToolCall>,
    /// Whether this response needs tool execution or completes the turn.
    pub response_status: GenerateResponseStatus,
}

#[cfg(test)]
impl ParsedNoemaResponse {
    pub(crate) fn assistant_text(&self) -> String {
        self.responses
            .iter()
            .filter_map(|item| match item {
                GenerateResponseItem::Text { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect()
    }
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
