use std::collections::HashSet;

#[cfg(any(feature = "adapters", feature = "local-models"))]
use crate::ProviderToolTransport;
use serde::Deserialize;
use serde_json::Value;

use super::{
    AssistantTextPhase, GenerateResponseItem, GenerateResponseStatus, GenerateToolCall,
    MultipleChoiceOption, ParsedNoemaResponse, ProviderError,
};

/// Parse a provider text payload into optional structured Noema response items.
///
/// Text without a response object is treated as one assistant text item.
///
/// # Errors
///
/// Returns [`ProviderError::MalformedResponse`] when a response-shaped object
/// is present but does not match the structured response contract.
pub(crate) fn output_items_from_text(
    text: String,
) -> Result<Vec<GenerateResponseItem>, ProviderError> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err(ProviderError::MalformedResponse {
            message: "provider produced empty output".to_string(),
        });
    }
    let parsed = serde_json::from_str::<Value>(trimmed)
        .map(|value| noema_response_from_structured_value(value, false, false))
        .unwrap_or_else(|_| embedded_noema_response(trimmed, true, false));
    if let Ok(Some(response)) = parsed {
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
pub(crate) fn required_noema_response_from_text(
    text: String,
) -> Result<ParsedNoemaResponse, ProviderError> {
    noema_response_from_text(text, true)
}

/// Parse required Noema text under one explicit effective tool transport.
///
/// The transport is fixed before provider generation. Native calls satisfy the
/// `needs_tools` requirement, while the Noema envelope remains authoritative
/// for envelope requests and disabled requests cannot carry executable calls.
///
/// # Errors
///
/// Returns [`ProviderError::MalformedResponse`] when the text is invalid,
/// contains envelope tool calls, or contains final-answer text.
#[cfg(any(feature = "adapters", feature = "local-models"))]
pub(crate) fn required_noema_response_from_text_with_tool_transport(
    text: String,
    native_tool_calls: Vec<GenerateToolCall>,
    tool_transport: ProviderToolTransport,
) -> Result<ParsedNoemaResponse, ProviderError> {
    validate_native_tool_transport(&native_tool_calls, tool_transport)?;
    if tool_transport == ProviderToolTransport::NoemaEnvelope {
        return required_noema_response_from_text(text);
    }

    if tool_transport == ProviderToolTransport::None {
        let parsed = noema_response_from_text_with_options(text, true, true)?;
        if !parsed.tool_calls.is_empty() {
            return Err(ProviderError::MalformedResponse {
                message:
                    "disabled tool transport cannot include Noema response-envelope tool_calls"
                        .to_string(),
            });
        }
        return Ok(parsed);
    }

    let mut parsed = noema_response_from_text_with_options(text, false, true)?;
    if !parsed.tool_calls.is_empty() {
        return Err(ProviderError::MalformedResponse {
            message: "native tool response cannot include Noema response-envelope tool_calls"
                .to_string(),
        });
    }
    if native_tool_calls.is_empty() {
        validate_required_noema_response(&parsed)?;
        return Ok(parsed);
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

/// Reject provider-native calls when the admitted request selected another
/// transport. This check is shared by structured and plain response paths.
#[cfg(any(feature = "adapters", feature = "local-models"))]
pub(crate) fn validate_native_tool_transport(
    native_tool_calls: &[GenerateToolCall],
    tool_transport: ProviderToolTransport,
) -> Result<(), ProviderError> {
    if native_tool_calls.is_empty() || tool_transport == ProviderToolTransport::Native {
        return Ok(());
    }
    let message = match tool_transport {
        ProviderToolTransport::None => {
            "tool response returned calls while tool transport is disabled"
        }
        ProviderToolTransport::NoemaEnvelope => {
            "Noema envelope response cannot include provider-native tool calls"
        }
        ProviderToolTransport::Native => unreachable!("native transport returned early"),
    };
    Err(ProviderError::MalformedResponse {
        message: message.to_string(),
    })
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

fn noema_response_from_text(
    text: String,
    require_noema_response: bool,
) -> Result<ParsedNoemaResponse, ProviderError> {
    noema_response_from_text_with_options(text, require_noema_response, false)
}

fn noema_response_from_text_with_options(
    text: String,
    require_noema_response: bool,
    allow_missing_tool_calls: bool,
) -> Result<ParsedNoemaResponse, ProviderError> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err(ProviderError::MalformedResponse {
            message: "provider produced empty output".to_string(),
        });
    }

    if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
        if let Some(response) = noema_response_from_structured_value(
            value,
            require_noema_response,
            allow_missing_tool_calls,
        )? {
            return Ok(response);
        }
        if require_noema_response {
            return Err(ProviderError::MalformedResponse {
                message: "provider did not return a Noema structured response object".to_string(),
            });
        }
    }

    if let Some(response) =
        embedded_noema_response(trimmed, require_noema_response, allow_missing_tool_calls)?
    {
        return Ok(response);
    }

    Err(ProviderError::MalformedResponse {
        message: "provider did not return a Noema structured response object".to_string(),
    })
}

fn noema_response_from_structured_value(
    value: Value,
    require_noema_response: bool,
    allow_missing_tool_calls: bool,
) -> Result<Option<ParsedNoemaResponse>, ProviderError> {
    if !looks_like_noema_response_object(&value) && !require_noema_response {
        return Ok(None);
    }

    if !allow_missing_tool_calls && value.get("tool_calls").is_none() {
        return Err(ProviderError::MalformedResponse {
            message: "invalid Noema structured response: missing field `tool_calls`".to_string(),
        });
    }
    let response_object: NoemaResponseObject =
        serde_json::from_value(value).map_err(|source| ProviderError::MalformedResponse {
            message: format!("invalid Noema structured response: {source}"),
        })?;
    let parsed = ParsedNoemaResponse {
        responses: response_object.responses,
        tool_calls: response_object.tool_calls,
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
    #[serde(default)]
    tool_calls: Vec<GenerateToolCall>,
}

fn validate_required_noema_response(response: &ParsedNoemaResponse) -> Result<(), ProviderError> {
    validate_response_items(&response.responses)?;
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
            if response.responses.iter().any(is_multiple_choice_response) {
                return Err(ProviderError::MalformedResponse {
                    message: "Noema needs_tools response cannot include multiple_choice items"
                        .to_string(),
                });
            }
        }
    }

    Ok(())
}

fn validate_response_items(responses: &[GenerateResponseItem]) -> Result<(), ProviderError> {
    for item in responses {
        if let GenerateResponseItem::MultipleChoice {
            prompt, options, ..
        } = item
        {
            validate_multiple_choice_response(prompt, options)?;
        }
    }
    Ok(())
}

fn validate_multiple_choice_response(
    prompt: &str,
    options: &[MultipleChoiceOption],
) -> Result<(), ProviderError> {
    if prompt.trim().is_empty() {
        return Err(ProviderError::MalformedResponse {
            message: "Noema multiple_choice response prompt cannot be empty".to_string(),
        });
    }
    if options.len() < 2 {
        return Err(ProviderError::MalformedResponse {
            message: "Noema multiple_choice response must include at least two options".to_string(),
        });
    }

    let mut ids = HashSet::with_capacity(options.len());
    for option in options {
        if option.id.trim().is_empty() {
            return Err(ProviderError::MalformedResponse {
                message: "Noema multiple_choice response option id cannot be empty".to_string(),
            });
        }
        if !ids.insert(option.id.as_str()) {
            return Err(ProviderError::MalformedResponse {
                message: "Noema multiple_choice response option ids must be unique".to_string(),
            });
        }
        if option.label.trim().is_empty() {
            return Err(ProviderError::MalformedResponse {
                message: "Noema multiple_choice response option label cannot be empty".to_string(),
            });
        }
    }
    Ok(())
}

fn has_non_empty_response_item(responses: &[GenerateResponseItem]) -> bool {
    responses.iter().any(|item| match item {
        GenerateResponseItem::Text { text, .. } => !text.trim().is_empty(),
        GenerateResponseItem::MultipleChoice { prompt, .. } => !prompt.trim().is_empty(),
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

fn is_multiple_choice_response(item: &GenerateResponseItem) -> bool {
    matches!(item, GenerateResponseItem::MultipleChoice { .. })
}

fn embedded_noema_response(
    text: &str,
    require_noema_response: bool,
    allow_missing_tool_calls: bool,
) -> Result<Option<ParsedNoemaResponse>, ProviderError> {
    let mut output = None;
    for candidate in balanced_json_object_candidates(text) {
        let Ok(value) = serde_json::from_str::<Value>(candidate) else {
            continue;
        };
        if !looks_like_noema_response_object(&value) {
            continue;
        }
        let Some(candidate_output) = noema_response_from_structured_value(
            value,
            require_noema_response,
            allow_missing_tool_calls,
        )?
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
