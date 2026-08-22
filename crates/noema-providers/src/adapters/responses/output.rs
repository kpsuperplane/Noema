//! Responses-compatible provider output parsing and normalization.

use std::collections::HashSet;

use super::{ResponsesDiagnosticContext, tools::ResponsesToolNameMap};
use crate::{
    AssistantTextPhase, GenerateCitation, GenerateHostedWebSearch, GenerateReasoningItem,
    GenerateResponse, GenerateResponseItem, GenerateWebSource, ProviderError,
    ProviderToolTransport, TokenUsage,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Parsed Responses-compatible API response.
#[derive(Debug, Deserialize, Serialize)]
pub struct ResponsesResponse {
    /// Provider response id.
    pub id: Option<String>,
    /// Model reported by the provider.
    pub model: Option<String>,
    #[serde(default)]
    output: Vec<ResponsesOutputItem>,
    /// Token usage reported by the provider.
    pub usage: Option<ResponsesUsage>,
    #[serde(default, skip)]
    pub(super) raw: Option<Value>,
}

impl ResponsesResponse {
    /// Finalize one shared Responses result into Noema's provider-neutral response.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError`] when native tool calls or assistant output are malformed.
    pub(crate) fn finalize(
        self,
        tool_names: &ResponsesToolNameMap,
        tool_transport: ProviderToolTransport,
        diagnostics: &ResponsesDiagnosticContext,
    ) -> Result<GenerateResponse, ProviderError> {
        let native_tool_calls = self.native_tool_calls_with_names(tool_names)?;
        if !native_tool_calls.is_empty() && tool_transport != ProviderToolTransport::Native {
            return Err(ProviderError::MalformedResponse {
                message: "provider returned native tool calls when native tools were disabled"
                    .to_string(),
            });
        }
        let responses = match self.output_messages() {
            Ok(responses) => responses,
            Err(ProviderError::MalformedResponse { .. }) if !native_tool_calls.is_empty() => {
                Vec::new()
            }
            Err(error @ ProviderError::MalformedResponse { .. }) => {
                let payload = self.raw.clone().unwrap_or_else(|| {
                    serde_json::json!({
                        "id": self.id.clone(),
                        "model": self.model.clone(),
                        "output": self.output.clone(),
                        "usage": self.usage.clone(),
                    })
                });
                diagnostics.log_malformed_error(&error, self.id.as_deref(), payload);
                return Err(error);
            }
            Err(error) => return Err(error),
        };

        Ok(self.generate_response(responses, native_tool_calls, diagnostics))
    }

    fn generate_response(
        self,
        responses: Vec<GenerateResponseItem>,
        tool_calls: Vec<crate::GenerateToolCall>,
        diagnostics: &ResponsesDiagnosticContext,
    ) -> GenerateResponse {
        let reasoning_items = self.reasoning_items();
        let hosted_web_searches = self.hosted_web_searches();
        GenerateResponse {
            responses,
            tool_calls,
            reasoning_items,
            hosted_web_searches,
            provider: diagnostics.provider_kind.clone(),
            model: self.model.unwrap_or_else(|| diagnostics.model.clone()),
            response_id: self.id,
            usage: self.usage.map(Into::into),
        }
    }

    /// Collect assistant output text in provider order.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError::MalformedResponse`] when the response contains
    /// no text, or [`ProviderError::ApiError`] when the only textual payload is
    /// a refusal.
    #[cfg(test)]
    pub fn output_text(&self) -> Result<String, ProviderError> {
        Ok(self
            .output_messages()?
            .into_iter()
            .map(|item| match item {
                GenerateResponseItem::Text { text, .. } => text,
            })
            .collect())
    }

    fn output_messages(&self) -> Result<Vec<GenerateResponseItem>, ProviderError> {
        let mut messages = Vec::new();
        let mut refusals = Vec::new();

        for item in &self.output {
            let ResponsesOutputItem::Message { id, phase, content } = item else {
                continue;
            };

            let mut text = String::new();
            let mut citations = Vec::new();
            let mut seen = HashSet::new();
            for content_item in content {
                match content_item {
                    ResponsesContent::OutputText {
                        text: content_text,
                        annotations,
                    } => {
                        collect_response_citations(
                            annotations,
                            text.encode_utf16().count(),
                            &mut seen,
                            &mut citations,
                        );
                        text.push_str(content_text);
                    }
                    ResponsesContent::Refusal { refusal } => refusals.push(refusal.as_str()),
                    ResponsesContent::Other => {}
                }
            }
            if !text.is_empty() {
                let phase = match phase.as_deref() {
                    Some("commentary") => Some(AssistantTextPhase::Commentary),
                    Some("final_answer") => Some(AssistantTextPhase::FinalAnswer),
                    _ => None,
                };
                messages.push(GenerateResponseItem::Text {
                    id: id.clone(),
                    phase,
                    text,
                    citations,
                });
            }
        }

        if !messages.is_empty() {
            return Ok(messages);
        }

        if !refusals.is_empty() {
            return Err(ProviderError::ApiError {
                status: 200,
                message: refusals.join("\n"),
                request_id: self.id.clone(),
            });
        }

        Err(ProviderError::MalformedResponse {
            message: "response did not contain output_text".to_string(),
        })
    }

    pub(crate) fn native_tool_calls_with_names(
        &self,
        tool_names: &ResponsesToolNameMap,
    ) -> Result<Vec<crate::GenerateToolCall>, ProviderError> {
        let mut calls = Vec::new();
        let mut provider_call_ids = HashSet::new();
        for item in &self.output {
            let ResponsesOutputItem::FunctionCall {
                id,
                call_id,
                name,
                arguments,
            } = item
            else {
                continue;
            };
            let canonical_name = tool_names.canonical_name(name).ok_or_else(|| {
                ProviderError::MalformedResponse {
                    message: "provider returned an unadvertised tool name".to_string(),
                }
            })?;
            let Some(call_id) = call_id.as_ref().filter(|value| !value.trim().is_empty()) else {
                return Err(ProviderError::MalformedResponse {
                    message: format!("native tool call {canonical_name} is missing call_id"),
                });
            };
            let payload: Value = serde_json::from_str(arguments).map_err(|source| {
                ProviderError::MalformedResponse {
                    message: format!(
                        "failed to parse native tool call arguments for {canonical_name}: {source}"
                    ),
                }
            })?;
            if !payload.is_object() {
                return Err(ProviderError::MalformedResponse {
                    message: format!(
                        "native tool call arguments for {canonical_name} must be a JSON object"
                    ),
                });
            }
            let payload = tool_names.source_form_arguments(name, payload);
            if !provider_call_ids.insert(call_id.clone()) {
                return Err(ProviderError::MalformedResponse {
                    message: format!(
                        "provider returned duplicate provider_call_id `{call_id}` in native tool calls"
                    ),
                });
            }
            calls.push(crate::GenerateToolCall {
                id: id.clone(),
                provider_call_id: Some(call_id.clone()),
                provider_name: Some(name.clone()),
                name: canonical_name.to_string(),
                payload,
            });
        }
        Ok(calls)
    }

    /// Collect reasoning output items for stateless replay and display metadata.
    #[must_use]
    pub fn reasoning_items(&self) -> Vec<GenerateReasoningItem> {
        self.output
            .iter()
            .filter_map(|item| match item {
                ResponsesOutputItem::Reasoning {
                    id,
                    encrypted_content,
                    summary,
                } => {
                    let summary = summary
                        .iter()
                        .filter_map(|part| match part {
                            ResponsesReasoningSummary::SummaryText { text } => Some(text.clone()),
                            ResponsesReasoningSummary::Other => None,
                        })
                        .collect::<Vec<_>>();
                    (encrypted_content.is_some() || !summary.is_empty()).then(|| {
                        GenerateReasoningItem {
                            id: id.clone(),
                            encrypted_content: encrypted_content.clone(),
                            summary,
                            provider_details: None,
                        }
                    })
                }
                _ => None,
            })
            .collect()
    }

    /// Collect provider-hosted web-search actions in provider output order.
    #[must_use]
    pub fn hosted_web_searches(&self) -> Vec<GenerateHostedWebSearch> {
        self.output
            .iter()
            .enumerate()
            .filter_map(|(output_index, item)| match item {
                ResponsesOutputItem::WebSearchCall { id, status, action } => {
                    let (tool_name, arguments, result, sources) =
                        normalize_hosted_web_action(action, status);
                    Some(GenerateHostedWebSearch {
                        output_index,
                        id: id.clone(),
                        tool_name,
                        arguments,
                        result,
                        status: status.clone(),
                        sources,
                    })
                }
                _ => None,
            })
            .collect()
    }

    /// Collect unique safe URL citations from assistant output text.
    #[cfg(test)]
    #[must_use]
    pub fn citations(&self) -> Vec<GenerateCitation> {
        let mut seen = std::collections::HashSet::new();
        let mut citations = Vec::new();
        for item in &self.output {
            let ResponsesOutputItem::Message { content, .. } = item else {
                continue;
            };
            let mut content_offset = 0;
            for content_item in content {
                let ResponsesContent::OutputText { text, annotations } = content_item else {
                    continue;
                };
                collect_response_citations(annotations, content_offset, &mut seen, &mut citations);
                content_offset += text.encode_utf16().count();
            }
        }
        citations
    }

    pub(super) fn from_stream_parts(
        id: Option<String>,
        model: Option<String>,
        output_values: Vec<Value>,
        usage: Option<ResponsesUsage>,
    ) -> Result<Self, ProviderError> {
        let raw_output_values = output_values.clone();
        let output = output_values
            .into_iter()
            .map(|value| {
                serde_json::from_value(value).map_err(|source| ProviderError::MalformedResponse {
                    message: format!("failed to parse SSE output item: {source}"),
                })
            })
            .collect::<Result<Vec<_>, _>>()?;

        let raw = serde_json::json!({
            "id": id.clone(),
            "model": model.clone(),
            "output": raw_output_values,
            "usage": usage.clone(),
        });
        Ok(Self {
            id,
            model,
            output,
            usage,
            raw: Some(raw),
        })
    }
}

fn collect_response_citations(
    annotations: &[ResponsesAnnotation],
    offset: usize,
    seen: &mut HashSet<(String, Option<usize>, Option<usize>)>,
    citations: &mut Vec<GenerateCitation>,
) {
    for annotation in annotations {
        let ResponsesAnnotation::UrlCitation {
            title,
            url,
            start_index,
            end_index,
        } = annotation
        else {
            continue;
        };
        let url = url.trim();
        let start_index = start_index.map(|index| offset + index);
        let end_index = end_index.map(|index| offset + index);
        if !(url.starts_with("https://") || url.starts_with("http://"))
            || !seen.insert((url.to_string(), start_index, end_index))
        {
            continue;
        }
        let title = title.trim();
        citations.push(GenerateCitation {
            title: if title.is_empty() { url } else { title }.to_string(),
            url: url.to_string(),
            start_index,
            end_index,
        });
    }
}

fn normalize_hosted_web_action(
    action: &Value,
    status: &str,
) -> (String, Value, Value, Vec<GenerateWebSource>) {
    let action_type = action.get("type").and_then(Value::as_str);
    let (tool_name, arguments) = match action_type {
        Some("open_page" | "find_in_page") => {
            let mut arguments = serde_json::Map::new();
            if let Some(url) = action.get("url").and_then(Value::as_str) {
                arguments.insert("url".to_string(), Value::String(url.to_string()));
            }
            ("web.fetch", Value::Object(arguments))
        }
        _ => {
            let query = action
                .get("query")
                .and_then(Value::as_str)
                .map(ToString::to_string)
                .or_else(|| {
                    action
                        .get("queries")
                        .and_then(Value::as_array)
                        .map(|queries| {
                            queries
                                .iter()
                                .filter_map(Value::as_str)
                                .collect::<Vec<_>>()
                                .join("; ")
                        })
                        .filter(|query| !query.is_empty())
                });
            (
                "web.search",
                query.map_or_else(
                    || serde_json::json!({}),
                    |query| serde_json::json!({"query": query}),
                ),
            )
        }
    };
    let mut result = arguments.clone();
    if let Some(result) = result.as_object_mut() {
        result.insert("status".to_string(), Value::String(status.to_string()));
        if status.eq_ignore_ascii_case("failed") {
            result.insert(
                "error".to_string(),
                Value::String("provider-hosted web action failed".to_string()),
            );
        }
    }
    let sources = match action_type {
        Some("open_page" | "find_in_page") => action
            .get("url")
            .and_then(Value::as_str)
            .into_iter()
            .filter_map(|url| web_source(url, None))
            .collect(),
        _ => action
            .get("sources")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|source| {
                web_source(
                    source.get("url").and_then(Value::as_str)?,
                    source.get("title").and_then(Value::as_str),
                )
            })
            .collect(),
    };
    (tool_name.to_string(), arguments, result, sources)
}

fn web_source(url: &str, title: Option<&str>) -> Option<GenerateWebSource> {
    let url = url.trim();
    (url.starts_with("https://") || url.starts_with("http://")).then(|| GenerateWebSource {
        title: title
            .map(str::trim)
            .filter(|title| !title.is_empty())
            .map(ToString::to_string),
        url: url.to_string(),
    })
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type")]
enum ResponsesOutputItem {
    #[serde(rename = "message")]
    Message {
        #[serde(default)]
        id: Option<String>,
        #[serde(default)]
        phase: Option<String>,
        content: Vec<ResponsesContent>,
    },
    #[serde(rename = "function_call")]
    FunctionCall {
        id: Option<String>,
        call_id: Option<String>,
        name: String,
        arguments: String,
    },
    #[serde(rename = "reasoning")]
    Reasoning {
        id: Option<String>,
        encrypted_content: Option<String>,
        #[serde(default)]
        summary: Vec<ResponsesReasoningSummary>,
    },
    #[serde(rename = "web_search_call")]
    WebSearchCall {
        id: Option<String>,
        #[serde(default)]
        status: String,
        #[serde(default)]
        action: Value,
    },
    #[serde(other)]
    Other,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type")]
enum ResponsesReasoningSummary {
    #[serde(rename = "summary_text")]
    SummaryText { text: String },
    #[serde(other)]
    Other,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type")]
enum ResponsesContent {
    #[serde(rename = "output_text")]
    OutputText {
        text: String,
        #[serde(default)]
        annotations: Vec<ResponsesAnnotation>,
    },
    #[serde(rename = "refusal")]
    Refusal { refusal: String },
    #[serde(other)]
    Other,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type")]
enum ResponsesAnnotation {
    #[serde(rename = "url_citation")]
    UrlCitation {
        #[serde(default)]
        title: String,
        url: String,
        #[serde(default)]
        start_index: Option<usize>,
        #[serde(default)]
        end_index: Option<usize>,
    },
    #[serde(other)]
    Other,
}

/// Token usage reported by a Responses-compatible API.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ResponsesUsage {
    #[serde(default, rename = "input_tokens")]
    input: u64,
    #[serde(default, rename = "output_tokens")]
    output: u64,
    #[serde(default, rename = "total_tokens")]
    total: u64,
    #[serde(default, rename = "input_tokens_details")]
    input_details: Option<ResponsesInputTokenDetails>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct ResponsesInputTokenDetails {
    #[serde(default)]
    cached_tokens: u64,
}

impl From<ResponsesUsage> for TokenUsage {
    fn from(value: ResponsesUsage) -> Self {
        Self {
            input_tokens: value.input,
            output_tokens: value.output,
            total_tokens: value.total,
            cached_input_tokens: value.input_details.map(|details| details.cached_tokens),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finalization_preserves_distinct_provider_messages() {
        let response: ResponsesResponse = serde_json::from_value(serde_json::json!({
            "id": "response:test",
            "model": "test-model",
            "output": [
                {
                    "type": "message",
                    "phase": "commentary",
                    "content": [{"type": "output_text", "text": "Nice"}]
                },
                {
                    "type": "message",
                    "phase": "final_answer",
                    "content": [{"type": "output_text", "text": "Want to go Saturday?"}]
                }
            ]
        }))
        .expect("responses payload");

        let generated = response
            .finalize(
                &ResponsesToolNameMap::default(),
                ProviderToolTransport::None,
                &ResponsesDiagnosticContext::new(None, "test", "test-model", None),
            )
            .expect("generated response");

        assert_eq!(generated.responses.len(), 2);
        assert_eq!(generated.assistant_text(), "NiceWant to go Saturday?");
    }
}
