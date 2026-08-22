use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

/// Structured response returned by a model provider.
#[derive(Debug, Clone, PartialEq)]
pub struct GenerateResponse {
    /// Provider-native response items retained for continuation and projection.
    pub responses: Vec<GenerateResponseItem>,
    /// Tool calls requested by the provider.
    pub tool_calls: Vec<GenerateToolCall>,
    /// Provider reasoning items returned for replay and display metadata.
    pub reasoning_items: Vec<GenerateReasoningItem>,
    /// Provider-hosted web-search activity completed during this response.
    pub hosted_web_searches: Vec<GenerateHostedWebSearch>,
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
    /// Ordered public sources used by this hosted web action.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sources: Vec<GenerateWebSource>,
}

/// One public source used by a provider-hosted web action.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GenerateWebSource {
    /// Provider title, when supplied.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Exact HTTP(S) source URL.
    pub url: String,
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
        /// Provider-native call id used to correlate the final call and result.
        provider_call_id: String,
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
    /// A privacy-bounded provider timing milestone.
    ProviderTiming {
        /// Provider lifecycle milestone without request or response content.
        milestone: ProviderTimingMilestone,
        /// Provider output-item index for a hosted-search milestone.
        output_index: Option<usize>,
    },
}

/// Provider wait boundaries available from a streaming response.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderTimingMilestone {
    /// Successful response headers arrived.
    ResponseHeaders,
    /// The first response-body bytes arrived.
    ResponseBodyStarted,
    /// A provider-hosted web search entered its searching state.
    HostedWebSearchSearching,
    /// A provider-hosted web search completed.
    HostedWebSearchCompleted,
}

impl GenerateResponse {
    pub(crate) fn normalize_markdown_messages(&mut self) {
        self.responses = std::mem::take(&mut self.responses)
            .into_iter()
            .flat_map(split_markdown_response_item)
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
                id: None,
                phase: None,
                text: text.into(),
                citations: Vec::new(),
            }],
            tool_calls: Vec::new(),
            reasoning_items: Vec::new(),
            hosted_web_searches: Vec::new(),
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

    /// Return every non-empty assistant item in provider order.
    pub fn assistant_response_texts(&self) -> impl Iterator<Item = AssistantResponseText<'_>> + '_ {
        let has_tools = self.has_tool_calls();
        self.responses
            .iter()
            .enumerate()
            .filter_map(move |(response_index, item)| {
                let GenerateResponseItem::Text {
                    id,
                    text,
                    citations,
                    ..
                } = item;
                (!text.trim().is_empty()).then(|| AssistantResponseText {
                    response_index,
                    provider_item_id: id.as_deref(),
                    phase: AssistantTextPhase::effective_for_response_item(item, has_tools),
                    text,
                    citations,
                })
            })
    }
}

/// One phase-aware assistant item selected for durable storage.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AssistantResponseText<'a> {
    /// Position inside the provider response text collection.
    pub response_index: usize,
    /// Provider output item id, when available.
    pub provider_item_id: Option<&'a str>,
    /// Effective visible phase.
    pub phase: AssistantTextPhase,
    /// Exact provider text.
    pub text: &'a str,
    /// Citations attached to this exact item.
    pub citations: &'a [GenerateCitation],
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
        /// Provider output item id, when available.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        /// Whether the text is mid-turn commentary or the final answer.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        phase: Option<AssistantTextPhase>,
        /// Text to show in the transcript.
        text: String,
        /// Provider source citations attached to this exact text item.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        citations: Vec<GenerateCitation>,
    },
}

pub(crate) fn split_markdown_response_item(
    item: GenerateResponseItem,
) -> Vec<GenerateResponseItem> {
    let GenerateResponseItem::Text {
        id,
        phase,
        text,
        citations,
    } = item;
    let segments = super::split_markdown_message_segments(&text);
    let mut grouped = vec![Vec::new(); segments.len()];
    for mut citation in citations {
        let target = citation
            .end_index
            .and_then(|end| {
                segments
                    .iter()
                    .position(|segment| {
                        segment.source_utf16.contains(&end) || end == segment.source_utf16.end
                    })
                    .or_else(|| {
                        segments
                            .iter()
                            .rposition(|segment| segment.source_utf16.end < end)
                    })
            })
            .or_else(|| (!segments.is_empty()).then_some(segments.len() - 1));
        let Some(target) = target else { continue };
        let segment = &segments[target];
        citation.end_index = citation.end_index.map(|end| {
            end.saturating_sub(segment.source_utf16.start)
                .min(segment.text.encode_utf16().count())
        });
        citation.start_index = citation.start_index.and_then(|start| {
            (segment.source_utf16.contains(&start) || start == segment.source_utf16.end)
                .then(|| start.saturating_sub(segment.source_utf16.start))
        });
        if matches!((citation.start_index, citation.end_index), (Some(start), Some(end)) if start >= end)
        {
            citation.start_index = None;
        }
        grouped[target].push(citation);
    }
    segments
        .into_iter()
        .zip(grouped)
        .map(|(segment, citations)| GenerateResponseItem::Text {
            id: id.clone(),
            phase,
            text: segment.text,
            citations,
        })
        .collect()
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

#[cfg(test)]
mod citation_split_tests {
    use super::*;

    #[test]
    fn citations_follow_utf16_bubble_ranges_and_missing_offset_fallback() {
        let mut response = GenerateResponse {
            responses: vec![GenerateResponseItem::Text {
                id: None,
                phase: None,
                text: "😀 first\n\nsecond".to_string(),
                citations: vec![
                    citation("First", Some(9)),
                    citation("Second", Some(16)),
                    citation("Fallback", None),
                ],
            }],
            tool_calls: Vec::new(),
            reasoning_items: Vec::new(),
            hosted_web_searches: Vec::new(),
            provider: "test".to_string(),
            model: "test".to_string(),
            response_id: None,
            usage: None,
        };

        response.normalize_markdown_messages();

        let GenerateResponseItem::Text {
            text, citations, ..
        } = &response.responses[0];
        assert_eq!(text, "😀 first");
        assert_eq!(citations[0].end_index, Some(8));
        let GenerateResponseItem::Text {
            text, citations, ..
        } = &response.responses[1];
        assert_eq!(text, "second");
        assert_eq!(
            citations
                .iter()
                .map(|citation| citation.title.as_str())
                .collect::<Vec<_>>(),
            ["Second", "Fallback"]
        );
        assert_eq!(citations[0].end_index, Some(6));
        assert_eq!(citations[1].end_index, None);
    }

    #[test]
    fn assistant_response_texts_preserve_order_late_commentary_and_exact_duplicates() {
        let response = GenerateResponse {
            responses: vec![
                text_item(None, "progress"),
                text_item(Some(AssistantTextPhase::FinalAnswer), "same"),
                text_item(Some(AssistantTextPhase::Commentary), "same"),
            ],
            tool_calls: vec![GenerateToolCall {
                id: None,
                provider_call_id: Some("call_1".to_string()),
                provider_name: None,
                name: "read".to_string(),
                payload: Value::Null,
            }],
            reasoning_items: Vec::new(),
            hosted_web_searches: Vec::new(),
            provider: "test".to_string(),
            model: "test".to_string(),
            response_id: None,
            usage: None,
        };

        let response_texts = response.assistant_response_texts().collect::<Vec<_>>();
        assert_eq!(
            response_texts
                .iter()
                .map(|item| (item.text, item.phase))
                .collect::<Vec<_>>(),
            [
                ("progress", AssistantTextPhase::Commentary),
                ("same", AssistantTextPhase::FinalAnswer),
                ("same", AssistantTextPhase::Commentary),
            ]
        );
    }

    fn text_item(phase: Option<AssistantTextPhase>, text: &str) -> GenerateResponseItem {
        GenerateResponseItem::Text {
            id: None,
            phase,
            text: text.to_string(),
            citations: Vec::new(),
        }
    }

    fn citation(title: &str, end_index: Option<usize>) -> GenerateCitation {
        GenerateCitation {
            title: title.to_string(),
            url: format!("https://{}.example", title.to_lowercase()),
            start_index: None,
            end_index,
        }
    }
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
