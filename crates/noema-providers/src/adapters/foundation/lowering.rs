use crate::{
    GenerateInput, GenerateInputItem, GenerateMessage, GenerateMessageRole, GenerateResponseItem,
    GenerateToolCallInput, ParsedNoemaResponse,
};

use super::bridge::{BridgeReplayTurn, BridgeRole};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct FoundationPrompt {
    pub(super) replay_turns: Vec<BridgeReplayTurn>,
    pub(super) generate_input: String,
}

pub(super) fn foundation_prompt_parts(input: &GenerateInput) -> FoundationPrompt {
    match input {
        GenerateInput::Text(text) => FoundationPrompt {
            replay_turns: Vec::new(),
            generate_input: text.clone(),
        },
        GenerateInput::Messages(messages) => {
            let last_user_index = messages
                .iter()
                .rposition(|message| message.role == GenerateMessageRole::User);
            let Some(last_user_index) = last_user_index else {
                return FoundationPrompt {
                    replay_turns: bridge_replay_turns(messages),
                    generate_input: String::new(),
                };
            };
            let mut replay_turns = bridge_replay_turns(&messages[..last_user_index]);
            replay_turns.extend(
                bridge_replay_turns(&messages[last_user_index + 1..])
                    .into_iter()
                    .filter(|turn| turn.role == BridgeRole::ApplicationContext),
            );
            FoundationPrompt {
                replay_turns,
                generate_input: messages[last_user_index].content.clone(),
            }
        }
        GenerateInput::Items(items) => {
            let last_user_index = items.iter().rposition(|item| {
                matches!(
                    item,
                    GenerateInputItem::Message(message)
                        if message.role == GenerateMessageRole::User
                )
            });
            let Some(last_user_index) = last_user_index else {
                return FoundationPrompt {
                    replay_turns: bridge_replay_input_items(items),
                    generate_input: String::new(),
                };
            };
            let generate_input = match &items[last_user_index] {
                GenerateInputItem::Message(message) => message.content.clone(),
                GenerateInputItem::Reasoning(_)
                | GenerateInputItem::ToolCall(_)
                | GenerateInputItem::ToolResult(_) => String::new(),
            };
            let mut replay_turns = bridge_replay_input_items(&items[..last_user_index]);
            replay_turns.extend(
                bridge_replay_input_items(&items[last_user_index + 1..])
                    .into_iter()
                    .filter(|turn| turn.role == BridgeRole::ApplicationContext),
            );
            FoundationPrompt {
                replay_turns,
                generate_input,
            }
        }
        GenerateInput::NativeToolResults(_) => FoundationPrompt {
            replay_turns: Vec::new(),
            generate_input: input.render_for_token_count(),
        },
    }
}

fn bridge_replay_input_items(items: &[GenerateInputItem]) -> Vec<BridgeReplayTurn> {
    items
        .iter()
        .filter(|item| !item.is_empty())
        .map(|item| match item {
            GenerateInputItem::Message(message) => BridgeReplayTurn {
                role: bridge_role(message.role),
                text: message.content.clone(),
            },
            GenerateInputItem::Reasoning(_)
            | GenerateInputItem::ToolCall(_)
            | GenerateInputItem::ToolResult(_) => BridgeReplayTurn {
                role: BridgeRole::Assistant,
                text: item.render_for_token_count(),
            },
        })
        .collect()
}

fn bridge_replay_turns(messages: &[GenerateMessage]) -> Vec<BridgeReplayTurn> {
    messages
        .iter()
        .filter(|message| !message.content.trim().is_empty())
        .map(|message| BridgeReplayTurn {
            role: bridge_role(message.role),
            text: message.content.clone(),
        })
        .collect()
}

const fn bridge_role(role: GenerateMessageRole) -> BridgeRole {
    match role {
        GenerateMessageRole::System | GenerateMessageRole::Developer => {
            BridgeRole::ApplicationContext
        }
        GenerateMessageRole::User => BridgeRole::User,
        GenerateMessageRole::Assistant => BridgeRole::Assistant,
    }
}

pub(super) fn bridge_replay_parsed_response(
    response: &ParsedNoemaResponse,
) -> Vec<BridgeReplayTurn> {
    let mut turns = response
        .responses
        .iter()
        .filter_map(|item| match item {
            GenerateResponseItem::Text { text, .. } => {
                let text = text.trim();
                (!text.is_empty()).then(|| BridgeReplayTurn {
                    role: BridgeRole::Assistant,
                    text: text.to_string(),
                })
            }
            GenerateResponseItem::MultipleChoice {
                prompt, options, ..
            } => {
                let rendered_options = options
                    .iter()
                    .map(|option| format!("{}={}", option.id, option.label))
                    .collect::<Vec<_>>()
                    .join("; ");
                Some(BridgeReplayTurn {
                    role: BridgeRole::Assistant,
                    text: format!(
                        "assistant multiple_choice: {prompt}\noptions: {rendered_options}"
                    ),
                })
            }
            GenerateResponseItem::Structured { schema, payload } => Some(BridgeReplayTurn {
                role: BridgeRole::Assistant,
                text: serde_json::json!({
                    "kind": "structured",
                    "schema": schema,
                    "payload": payload,
                })
                .to_string(),
            }),
        })
        .collect::<Vec<_>>();
    for call in &response.tool_calls {
        if let Some(call_id) = call.provider_call_id.clone().or_else(|| call.id.clone()) {
            turns.extend(bridge_replay_input_items(&[GenerateInputItem::ToolCall(
                GenerateToolCallInput {
                    id: call.id.clone().filter(|id| id.starts_with("fc")),
                    call_id,
                    name: call.name.clone(),
                    provider_name: call.provider_name.clone(),
                    arguments: call.payload.clone(),
                },
            )]));
        } else {
            turns.push(BridgeReplayTurn {
                role: BridgeRole::Assistant,
                text: serde_json::json!({
                    "kind": "tool_call_without_correlation_id",
                    "name": call.name,
                    "provider_name": call.provider_name,
                    "payload": call.payload,
                })
                .to_string(),
            });
        }
    }
    turns
}
