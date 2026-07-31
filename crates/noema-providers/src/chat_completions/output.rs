use std::collections::HashSet;

use crate::response_support::StructuredResponseDiagnosticContext;
use crate::{
    GenerateCitation, GenerateHostedWebSearch, GenerateReasoningItem, GenerateResponse,
    GenerateResponseItem, ProviderError, ProviderToolTransport, TokenUsage,
};
use serde::Deserialize;
use serde_json::Value;

use super::{ChatUsage, OpenAiToolNameMap};

const MAX_HOSTED_WEB_SEARCH_MARKERS: u64 = 64;

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ChatCompletionResponse {
    pub(crate) id: Option<String>,
    pub(crate) model: Option<String>,
    #[serde(default)]
    pub(crate) choices: Vec<ChatChoice>,
    pub(crate) usage: Option<ChatUsage>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ChatChoice {
    pub(crate) message: ChatOutputMessage,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ChatOutputMessage {
    pub(crate) content: Option<String>,
    #[serde(default)]
    pub(crate) tool_calls: Vec<ChatOutputToolCall>,
    #[serde(default)]
    pub(crate) reasoning_details: Vec<Value>,
    #[serde(default)]
    pub(crate) annotations: Vec<Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ChatOutputToolCall {
    #[serde(default)]
    pub(crate) id: Option<String>,
    #[serde(default)]
    pub(crate) function: ChatOutputFunction,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub(crate) struct ChatOutputFunction {
    #[serde(default)]
    pub(crate) name: Option<String>,
    #[serde(default)]
    pub(crate) arguments: Option<String>,
}

impl ChatCompletionResponse {
    pub(crate) fn finalize(
        self,
        tool_names: &OpenAiToolNameMap,
        tool_transport: ProviderToolTransport,
        diagnostics: &StructuredResponseDiagnosticContext,
    ) -> Result<GenerateResponse, ProviderError> {
        let tool_calls = self.native_tool_calls_with_names(tool_names)?;
        if !tool_calls.is_empty() && tool_transport != ProviderToolTransport::Native {
            return Err(ProviderError::MalformedResponse {
                message: "provider returned native tool calls when native tools were disabled"
                    .to_string(),
            });
        }

        let mut text = String::new();
        for choice in &self.choices {
            if let Some(content) = choice.message.content.as_deref() {
                text.push_str(content);
            }
        }
        if text.is_empty() && tool_calls.is_empty() {
            let error = ProviderError::MalformedResponse {
                message: "response did not contain assistant content or tool calls".to_string(),
            };
            return Err(error);
        }

        let responses = (!text.is_empty())
            .then_some(GenerateResponseItem::Text { phase: None, text })
            .into_iter()
            .collect();
        Ok(GenerateResponse {
            responses,
            tool_calls,
            reasoning_items: self.reasoning_items(),
            hosted_web_searches: self.hosted_web_searches(),
            citations: self.citations(),
            provider: diagnostics.provider_kind.clone(),
            model: self.model.unwrap_or_else(|| diagnostics.model.clone()),
            response_id: self.id,
            usage: self.usage.map(Into::into),
        })
    }

    fn native_tool_calls_with_names(
        &self,
        tool_names: &OpenAiToolNameMap,
    ) -> Result<Vec<crate::GenerateToolCall>, ProviderError> {
        let mut calls = Vec::new();
        let mut provider_call_ids = HashSet::new();
        for choice in &self.choices {
            for call in &choice.message.tool_calls {
                let Some(provider_call_id) = call.id.as_deref().filter(|id| !id.trim().is_empty())
                else {
                    return Err(ProviderError::MalformedResponse {
                        message: "native tool call is missing call id".to_string(),
                    });
                };
                let Some(provider_name) = call
                    .function
                    .name
                    .as_deref()
                    .filter(|name| !name.trim().is_empty())
                else {
                    return Err(ProviderError::MalformedResponse {
                        message: format!("native tool call {provider_call_id} is missing name"),
                    });
                };
                let canonical_name = tool_names.canonical_name(provider_name).ok_or_else(|| {
                    ProviderError::MalformedResponse {
                        message: "provider returned an unadvertised tool name".to_string(),
                    }
                })?;
                let arguments = call.function.arguments.as_deref().unwrap_or_default();
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
                if !provider_call_ids.insert(provider_call_id.to_string()) {
                    return Err(ProviderError::MalformedResponse {
                        message: format!(
                            "provider returned duplicate provider_call_id `{provider_call_id}` in native tool calls"
                        ),
                    });
                }
                calls.push(crate::GenerateToolCall {
                    id: None,
                    provider_call_id: Some(provider_call_id.to_string()),
                    provider_name: Some(provider_name.to_string()),
                    name: canonical_name.to_string(),
                    payload,
                });
            }
        }
        Ok(calls)
    }

    fn reasoning_items(&self) -> Vec<GenerateReasoningItem> {
        let details = self
            .choices
            .iter()
            .flat_map(|choice| choice.message.reasoning_details.iter().cloned())
            .collect::<Vec<_>>();
        if details.is_empty() {
            return Vec::new();
        }
        let mut encrypted_content = None;
        let mut summary = Vec::new();
        let mut id = None;
        for detail in &details {
            let Some(object) = detail.as_object() else {
                continue;
            };
            match object.get("type").and_then(Value::as_str) {
                Some("reasoning.encrypted") => {
                    encrypted_content = object
                        .get("data")
                        .or_else(|| object.get("encrypted_content"))
                        .and_then(Value::as_str)
                        .map(ToString::to_string)
                        .or(encrypted_content);
                    id = object
                        .get("id")
                        .and_then(Value::as_str)
                        .map(ToString::to_string)
                        .or(id);
                }
                Some("reasoning.summary") => {
                    if let Some(text) = object
                        .get("summary")
                        .or_else(|| object.get("text"))
                        .and_then(Value::as_str)
                    {
                        summary.push(text.to_string());
                    }
                }
                _ => {}
            }
        }
        vec![GenerateReasoningItem {
            id,
            encrypted_content,
            summary,
            provider_details: Some(details),
        }]
    }

    fn hosted_web_searches(&self) -> Vec<GenerateHostedWebSearch> {
        let mut searches = self
            .choices
            .iter()
            .flat_map(|choice| {
                choice
                    .message
                    .reasoning_details
                    .iter()
                    .filter_map(move |detail| {
                        let object = detail.as_object()?;
                        if object.get("type").and_then(Value::as_str)
                            != Some("reasoning.server_tool_call")
                        {
                            return None;
                        }
                        let status = object
                            .get("status")
                            .and_then(Value::as_str)
                            .unwrap_or("completed")
                            .to_string();
                        let arguments = object
                            .get("arguments")
                            .cloned()
                            .or_else(|| object.get("input").cloned())
                            .unwrap_or_else(|| serde_json::json!({}));
                        let result = object
                            .get("result")
                            .cloned()
                            .unwrap_or_else(|| serde_json::json!({"status": status}));
                        Some(GenerateHostedWebSearch {
                            output_index: 0,
                            id: object
                                .get("id")
                                .and_then(Value::as_str)
                                .map(ToString::to_string),
                            tool_name: object
                                .get("name")
                                .or_else(|| object.get("tool_name"))
                                .and_then(Value::as_str)
                                .unwrap_or("web.search")
                                .to_string(),
                            arguments,
                            result,
                            status,
                        })
                    })
            })
            .collect::<Vec<_>>();
        let citations = self.citations();
        let reported_count = self
            .usage
            .as_ref()
            .and_then(|usage| usage.server_tool_use.as_ref())
            .map_or(0, |usage| usage.web_search_requests);
        let expected_count = usize::try_from(reported_count.min(MAX_HOSTED_WEB_SEARCH_MARKERS))
            .expect("hosted web-search marker bound must fit usize")
            .max(usize::from(!citations.is_empty()));
        let source_count = citations.len();
        let summary = match source_count {
            0 => "Web search completed".to_string(),
            1 => "Found 1 cited source".to_string(),
            count => format!("Found {count} cited sources"),
        };
        searches.extend((searches.len()..expected_count).map(|index| {
            GenerateHostedWebSearch {
                output_index: index,
                id: self
                    .id
                    .as_ref()
                    .map(|response_id| format!("{response_id}:web_search:{index}")),
                tool_name: "web.search".to_string(),
                arguments: serde_json::json!({}),
                result: serde_json::json!({
                    "provider": "openrouter",
                    "source_count": source_count,
                    "summary": summary,
                }),
                status: "completed".to_string(),
            }
        }));
        for (output_index, search) in searches.iter_mut().enumerate() {
            search.output_index = output_index;
        }
        searches
    }

    fn citations(&self) -> Vec<GenerateCitation> {
        let mut seen = HashSet::new();
        let mut citations = Vec::new();
        for choice in &self.choices {
            for annotation in &choice.message.annotations {
                collect_citations(annotation, &mut seen, &mut citations);
            }
        }
        citations
    }
}

fn collect_citations(
    value: &Value,
    seen: &mut HashSet<(String, Option<usize>, Option<usize>)>,
    citations: &mut Vec<GenerateCitation>,
) {
    match value {
        Value::Object(object) => {
            let citation = object
                .get("url_citation")
                .and_then(Value::as_object)
                .unwrap_or(object);
            if (object.get("type").and_then(Value::as_str) == Some("url_citation")
                || object.contains_key("url_citation"))
                && let Some(url) = citation.get("url").and_then(Value::as_str)
            {
                let url = url.trim();
                let start_index = citation
                    .get("start_index")
                    .and_then(Value::as_u64)
                    .and_then(|index| usize::try_from(index).ok());
                let end_index = citation
                    .get("end_index")
                    .and_then(Value::as_u64)
                    .and_then(|index| usize::try_from(index).ok());
                if (url.starts_with("https://") || url.starts_with("http://"))
                    && seen.insert((url.to_string(), start_index, end_index))
                {
                    let title = citation
                        .get("title")
                        .and_then(Value::as_str)
                        .map(str::trim)
                        .filter(|title| !title.is_empty())
                        .unwrap_or(url);
                    citations.push(GenerateCitation {
                        title: title.to_string(),
                        url: url.to_string(),
                        start_index,
                        end_index,
                    });
                }
            }
            for child in object.values() {
                collect_citations(child, seen, citations);
            }
        }
        Value::Array(values) => {
            for child in values {
                collect_citations(child, seen, citations);
            }
        }
        _ => {}
    }
}

impl From<ChatUsage> for TokenUsage {
    fn from(value: ChatUsage) -> Self {
        Self {
            input_tokens: value.prompt_tokens,
            output_tokens: value.completion_tokens,
            total_tokens: value.total_tokens,
            cached_input_tokens: value
                .prompt_tokens_details
                .map(|details| details.cached_tokens),
        }
    }
}
