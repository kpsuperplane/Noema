//! Responses-compatible provider output parsing and normalization.

use std::collections::HashSet;

use super::{ResponsesDiagnosticContext, tools::ResponsesToolNameMap};
use crate::{
    GenerateCitation, GenerateHostedWebSearch, GenerateReasoningItem, GenerateResponse,
    GenerateResponseItem, ProviderError, ProviderToolTransport, TokenUsage,
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
        let text = match self.output_text() {
            Ok(text) => text,
            Err(ProviderError::MalformedResponse { .. }) if !native_tool_calls.is_empty() => {
                String::new()
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

        let responses = (!text.is_empty())
            .then_some(GenerateResponseItem::Text { phase: None, text })
            .into_iter()
            .collect();
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
        let citations = self.citations();
        GenerateResponse {
            responses,
            tool_calls,
            reasoning_items,
            hosted_web_searches,
            citations,
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
    pub fn output_text(&self) -> Result<String, ProviderError> {
        let mut output = String::new();
        let mut refusals = Vec::new();

        for item in &self.output {
            let ResponsesOutputItem::Message { content } = item else {
                continue;
            };

            for content_item in content {
                match content_item {
                    ResponsesContent::OutputText { text, .. } => output.push_str(text),
                    ResponsesContent::Refusal { refusal } => refusals.push(refusal.as_str()),
                    ResponsesContent::Other => {}
                }
            }
        }

        if !output.is_empty() {
            return Ok(output);
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
                    let (tool_name, arguments, result) =
                        normalize_hosted_web_action(action, status);
                    Some(GenerateHostedWebSearch {
                        output_index,
                        id: id.clone(),
                        tool_name,
                        arguments,
                        result,
                        status: status.clone(),
                    })
                }
                _ => None,
            })
            .collect()
    }

    /// Collect unique safe URL citations from assistant output text.
    #[must_use]
    pub fn citations(&self) -> Vec<GenerateCitation> {
        let mut seen = std::collections::HashSet::new();
        let mut citations = Vec::new();
        for item in &self.output {
            let ResponsesOutputItem::Message { content } = item else {
                continue;
            };
            for content_item in content {
                let ResponsesContent::OutputText { annotations, .. } = content_item else {
                    continue;
                };
                for annotation in annotations {
                    let ResponsesAnnotation::UrlCitation { title, url } = annotation else {
                        continue;
                    };
                    let url = url.trim();
                    if !(url.starts_with("https://") || url.starts_with("http://"))
                        || !seen.insert(url.to_string())
                    {
                        continue;
                    }
                    let title = title.trim();
                    citations.push(GenerateCitation {
                        title: if title.is_empty() { url } else { title }.to_string(),
                        url: url.to_string(),
                    });
                }
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

fn normalize_hosted_web_action(action: &Value, status: &str) -> (String, Value, Value) {
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
    (tool_name.to_string(), arguments, result)
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type")]
enum ResponsesOutputItem {
    #[serde(rename = "message")]
    Message { content: Vec<ResponsesContent> },
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
    UrlCitation { title: String, url: String },
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
