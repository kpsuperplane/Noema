//! Responses-compatible provider output parsing and normalization.

use super::{responses::ResponsesDiagnosticContext, responses_tools::ResponsesToolNameMap};
use crate::provider::{
    GenerateReasoningItem, GenerateResponse, GenerateResponseStatus, ParsedNoemaResponse,
    ProviderError, TokenUsage, output_items_from_text,
    required_noema_response_from_text_with_native_tool_calls,
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
        require_noema_response: bool,
        diagnostics: &ResponsesDiagnosticContext,
    ) -> Result<GenerateResponse, ProviderError> {
        let native_tool_calls = self.native_tool_calls_with_names(tool_names)?;
        let text = match self.output_text() {
            Ok(text) => text,
            Err(ProviderError::MalformedResponse { .. }) if !native_tool_calls.is_empty() => {
                let parsed = ParsedNoemaResponse {
                    responses: Vec::new(),
                    tool_calls: native_tool_calls,
                    response_status: GenerateResponseStatus::NeedsTools,
                };
                return Ok(self.generate_response(parsed, diagnostics));
            }
            Err(error @ ProviderError::MalformedResponse { .. }) => {
                diagnostics.log_malformed_error(&error, self.id.as_deref(), self.raw_payload());
                return Err(error);
            }
            Err(error) => return Err(error),
        };

        let parsed = if require_noema_response {
            required_noema_response_from_text_with_native_tool_calls(
                text.clone(),
                native_tool_calls,
            )
            .inspect_err(|error| {
                diagnostics.log_malformed_error(
                    error,
                    self.id.as_deref(),
                    serde_json::json!({ "provider_text": text }),
                );
            })?
        } else {
            let response_status = if native_tool_calls.is_empty() {
                GenerateResponseStatus::Final
            } else {
                GenerateResponseStatus::NeedsTools
            };
            ParsedNoemaResponse {
                responses: output_items_from_text(text)?,
                tool_calls: native_tool_calls,
                response_status,
            }
        };

        Ok(self.generate_response(parsed, diagnostics))
    }

    fn generate_response(
        self,
        parsed: ParsedNoemaResponse,
        diagnostics: &ResponsesDiagnosticContext,
    ) -> GenerateResponse {
        let reasoning_items = self.reasoning_items();
        GenerateResponse::from_parsed(
            parsed,
            diagnostics.provider_kind.clone(),
            self.model.unwrap_or_else(|| diagnostics.model.clone()),
            self.id,
            self.usage.map(Into::into),
        )
        .with_reasoning_items(reasoning_items)
    }

    /// Return the raw provider payload preserved for developer diagnostics.
    #[must_use]
    pub fn raw_payload(&self) -> Value {
        self.raw.clone().unwrap_or_else(|| {
            serde_json::json!({
                "id": self.id.clone(),
                "model": self.model.clone(),
                "output": self.output.clone(),
                "usage": self.usage.clone(),
            })
        })
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
                    ResponsesContent::OutputText { text } => output.push_str(text),
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
    ) -> Result<Vec<crate::provider::GenerateToolCall>, ProviderError> {
        let mut calls = Vec::new();
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
            calls.push(crate::provider::GenerateToolCall {
                id: id.clone(),
                provider_call_id: Some(call_id.clone()),
                provider_name: Some(name.clone()),
                name: canonical_name.to_string(),
                payload,
            });
        }
        Ok(calls)
    }

    /// Collect encrypted reasoning output items for stateless replay.
    #[must_use]
    pub fn reasoning_items(&self) -> Vec<GenerateReasoningItem> {
        self.output
            .iter()
            .filter_map(|item| match item {
                ResponsesOutputItem::Reasoning {
                    id,
                    encrypted_content,
                } => encrypted_content
                    .as_ref()
                    .map(|encrypted_content| GenerateReasoningItem {
                        id: id.clone(),
                        encrypted_content: Some(encrypted_content.clone()),
                    }),
                _ => None,
            })
            .collect()
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
    },
    #[serde(other)]
    Other,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type")]
enum ResponsesContent {
    #[serde(rename = "output_text")]
    OutputText { text: String },
    #[serde(rename = "refusal")]
    Refusal { refusal: String },
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
