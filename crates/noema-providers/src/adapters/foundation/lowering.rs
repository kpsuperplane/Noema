use crate::{
    GenerateInput, GenerateInputItem, GenerateMessage, GenerateMessageRole, GenerateResponse,
    GenerateResponseItem,
};

use super::bridge::{BridgeReplayToolCall, BridgeReplayToolResult, BridgeReplayTurn, BridgeRole};

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
            if items.iter().rev().take_while(|item| !matches!(
                item,
                GenerateInputItem::Message(message) if message.role == GenerateMessageRole::User
            )).any(|item| matches!(item, GenerateInputItem::ToolCall(_) | GenerateInputItem::ToolResult(_))) {
                return FoundationPrompt {
                    replay_turns: bridge_replay_input_items(items),
                    generate_input: String::new(),
                };
            }
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
        .flat_map(|item| match item {
            GenerateInputItem::Message(message) => vec![BridgeReplayTurn {
                role: bridge_role(message.role),
                text: message.content.clone(),
                tool_call: None,
                tool_result: None,
            }],
            GenerateInputItem::Reasoning(reasoning) => vec![BridgeReplayTurn {
                role: BridgeRole::Assistant,
                text: reasoning.encrypted_content.clone(),
                tool_call: None,
                tool_result: None,
            }],
            GenerateInputItem::ToolCall(call) => vec![BridgeReplayTurn {
                role: BridgeRole::Assistant,
                text: String::new(),
                tool_call: Some(BridgeReplayToolCall {
                    call_id: call.call_id.clone(),
                    tool_name: call
                        .provider_name
                        .clone()
                        .unwrap_or_else(|| call.name.clone()),
                    arguments: serde_json::to_string(&call.arguments)
                        .unwrap_or_else(|_| "{}".to_string()),
                }),
                tool_result: None,
            }],
            GenerateInputItem::ToolResult(result) => vec![BridgeReplayTurn {
                role: BridgeRole::Assistant,
                text: String::new(),
                tool_call: None,
                tool_result: Some(BridgeReplayToolResult {
                    call_id: result.call_id.clone(),
                    tool_name: result
                        .provider_name
                        .clone()
                        .unwrap_or_else(|| result.name.clone()),
                    output: serde_json::json!({
                        "success": result.success,
                        "payload": result.payload,
                    })
                    .to_string(),
                }),
            }],
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
            tool_call: None,
            tool_result: None,
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

pub(super) fn bridge_replay_response(response: &GenerateResponse) -> Vec<BridgeReplayTurn> {
    let mut turns = response
        .responses
        .iter()
        .filter_map(|item| match item {
            GenerateResponseItem::Text { text, .. } => {
                let text = text.trim();
                (!text.is_empty()).then(|| BridgeReplayTurn {
                    role: BridgeRole::Assistant,
                    text: text.to_string(),
                    tool_call: None,
                    tool_result: None,
                })
            }
        })
        .collect::<Vec<_>>();
    for call in &response.tool_calls {
        if let Some(call_id) = call.provider_call_id.clone().or_else(|| call.id.clone()) {
            turns.push(BridgeReplayTurn {
                role: BridgeRole::Assistant,
                text: String::new(),
                tool_call: Some(BridgeReplayToolCall {
                    call_id,
                    tool_name: call
                        .provider_name
                        .clone()
                        .unwrap_or_else(|| call.name.clone()),
                    arguments: serde_json::to_string(&call.payload)
                        .unwrap_or_else(|_| "{}".to_string()),
                }),
                tool_result: None,
            });
        }
    }
    turns
}
