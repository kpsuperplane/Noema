use crate::{
    ActorRef, ConversationItemKind, ConversationItemRecord, ConversationItemStatus,
    NewConversationItem, PersistedAgentStatus,
    provider::{
        AssistantTextPhase, GenerateActionItem, GenerateReasoningItem, GenerateResponse,
        GenerateResponseItem, GenerateStreamEvent,
    },
};
use serde_json::{Value, json};
use tokio::sync::mpsc;

use super::{
    actor::CodexRuntimeActor,
    tool_lifecycle::{LocalToolCall, tool_call_action_item},
    turn::{
        ProviderActionOutput, ProviderActionTurn, ProviderAssistantResponse,
        ProviderResponsePosition,
    },
};
use crate::daemon::{
    memory::context::ConversationMemoryContext,
    protocol::{DaemonError, TurnActivityStatus, TurnStreamEvent, TurnTranscriptItem},
};

pub(in crate::daemon::runtime) fn provider_usage_metadata(
    provider: &str,
    model: &str,
    phase: &'static str,
    position: ProviderResponsePosition,
    usage: Option<&crate::provider::TokenUsage>,
) -> Value {
    let Some(usage) = usage else {
        return json!({});
    };

    let mut provider_usage = serde_json::Map::new();
    provider_usage.insert("provider".to_string(), json!(provider));
    provider_usage.insert("model".to_string(), json!(model));
    provider_usage.insert("phase".to_string(), json!(phase));
    provider_usage.insert("response_index".to_string(), json!(position.response_index));
    if let Some(output_index) = position.output_index {
        provider_usage.insert("output_index".to_string(), json!(output_index));
    }
    provider_usage.insert("input_tokens".to_string(), json!(usage.input_tokens));
    provider_usage.insert("output_tokens".to_string(), json!(usage.output_tokens));
    provider_usage.insert("total_tokens".to_string(), json!(usage.total_tokens));
    if let Some(cached_input_tokens) = usage.cached_input_tokens {
        provider_usage.insert(
            "cached_input_tokens".to_string(),
            json!(cached_input_tokens),
        );
        if usage.input_tokens > 0 {
            provider_usage.insert(
                "cache_hit_ratio".to_string(),
                json!(cached_input_tokens as f64 / usage.input_tokens as f64),
            );
        }
    }

    json!({ "provider_usage": Value::Object(provider_usage) })
}

fn merge_metadata(base: &mut Value, extra: Value) {
    let Some(base) = base.as_object_mut() else {
        return;
    };
    let Value::Object(extra) = extra else {
        return;
    };
    for (key, value) in extra {
        base.insert(key, value);
    }
}

impl CodexRuntimeActor {
    pub(super) async fn persist_provider_reasoning_items(
        &mut self,
        conversation_id: &str,
        turn_id: &str,
        reasoning_items: &[GenerateReasoningItem],
    ) -> Result<(), DaemonError> {
        for reasoning in reasoning_items {
            let Some(encrypted_content) = reasoning
                .encrypted_content
                .as_deref()
                .filter(|value| !value.trim().is_empty())
            else {
                continue;
            };
            self.store
                .append_conversation_item(NewConversationItem {
                    conversation_id: conversation_id.to_string(),
                    turn_id: Some(turn_id.to_string()),
                    parent_item_id: None,
                    kind: ConversationItemKind::Reasoning,
                    status: ConversationItemStatus::Completed,
                    author: ActorRef::agent("agent:primary"),
                    content_text: None,
                    payload_json: json!({
                        "provider_reasoning": {
                            "id": reasoning.id.clone(),
                            "encrypted_content": encrypted_content,
                        }
                    }),
                    metadata: json!({}),
                })
                .await?;
        }
        Ok(())
    }

    pub(super) async fn persist_provider_response_item(
        &mut self,
        turn: &ProviderActionTurn,
        position: ProviderResponsePosition,
        item: GenerateResponseItem,
        provider_phase_has_tools: bool,
        assistant_response: &mut ProviderAssistantResponse,
        item_tx: &mpsc::UnboundedSender<TurnStreamEvent>,
    ) -> Result<(), DaemonError> {
        match item {
            GenerateResponseItem::Text { phase, text } => {
                assistant_response.push_text(&text);
                let effective_phase = AssistantTextPhase::effective_for_response_item(
                    &GenerateResponseItem::Text {
                        phase,
                        text: text.clone(),
                    },
                    provider_phase_has_tools,
                );
                let output_index = position.output_index.unwrap_or(position.response_index);
                let stream_id = turn
                    .stream_id
                    .as_deref()
                    .map(|stream_id| assistant_response_stream_id(stream_id, output_index));
                let mut metadata = json!({
                    "turn_index": turn.turn_index,
                    "response_index": position.response_index,
                    "output_index": position.output_index,
                    "stream_id": stream_id,
                    "phase": effective_phase.as_str(),
                });
                merge_metadata(
                    &mut metadata,
                    provider_usage_metadata(
                        &turn.provider,
                        &turn.model,
                        turn.response_phase,
                        position,
                        turn.usage.as_ref(),
                    ),
                );
                let assistant_item = self
                    .store
                    .append_conversation_item(NewConversationItem {
                        conversation_id: turn.conversation_id.clone(),
                        turn_id: Some(turn.turn_id.clone()),
                        parent_item_id: Some(turn.user_item_id.clone()),
                        kind: ConversationItemKind::AssistantText,
                        status: ConversationItemStatus::Completed,
                        author: ActorRef::agent("agent:primary"),
                        content_text: Some(text.clone()),
                        payload_json: json!({}),
                        metadata: metadata.clone(),
                    })
                    .await?;
                if assistant_response.item_id.is_none() {
                    assistant_response.item_id = Some(assistant_item.item_id.clone());
                }
                send_conversation_item(
                    item_tx,
                    assistant_item,
                    metadata,
                    TurnTranscriptItem::AssistantText { text },
                );
            }
            GenerateResponseItem::Structured { schema, payload } => {
                let output_index = position.output_index.unwrap_or(position.response_index);
                let card_id = format!(
                    "provider_structured:{}:{}:{output_index}",
                    turn.conversation_id, turn.turn_index
                );
                let metadata = json!({
                    "turn_index": turn.turn_index,
                    "response_index": position.response_index,
                    "output_index": position.output_index,
                    "source": "provider_structured_output",
                });
                let structured_item = self
                    .store
                    .append_conversation_item(NewConversationItem {
                        conversation_id: turn.conversation_id.clone(),
                        turn_id: Some(turn.turn_id.clone()),
                        parent_item_id: Some(turn.user_item_id.clone()),
                        kind: ConversationItemKind::A2uiCard,
                        status: ConversationItemStatus::Completed,
                        author: ActorRef::agent("agent:primary"),
                        content_text: None,
                        payload_json: json!({
                            "id": card_id.clone(),
                            "schema": schema.clone(),
                            "payload": payload.clone(),
                        }),
                        metadata: metadata.clone(),
                    })
                    .await?;
                send_conversation_item(
                    item_tx,
                    structured_item,
                    metadata,
                    TurnTranscriptItem::A2uiCard {
                        id: card_id,
                        schema: schema.clone(),
                        payload: payload.clone(),
                    },
                );
            }
        }
        Ok(())
    }

    pub(super) async fn persist_provider_tool_call_started(
        &mut self,
        turn: &ProviderActionTurn,
        index: usize,
        call: &LocalToolCall,
        item_tx: &mpsc::UnboundedSender<TurnStreamEvent>,
    ) -> Result<(), DaemonError> {
        let GenerateActionItem::ToolCall {
            id,
            provider_call_id,
            provider_name,
            name,
            payload,
        } = tool_call_action_item(call)
        else {
            return Ok(());
        };
        let display = tool_call_display(&name, &payload);
        self.persist_provider_action_output(
            turn,
            ProviderActionOutput {
                index,
                kind: ConversationItemKind::ToolCall,
                status: ConversationItemStatus::Running,
                action_kind: "tool_call",
                title: format!("Tool call: {name}"),
                summary: display_summary(&display, "target"),
                payload: json!({
                    "id": id,
                    "provider_call_id": provider_call_id,
                    "provider_name": provider_name,
                    "name": name,
                    "payload": payload,
                }),
                display,
            },
            item_tx,
        )
        .await
    }

    pub(super) async fn persist_progress_audit_started(
        &mut self,
        turn: &ProviderActionTurn,
        index: usize,
        item_tx: &mpsc::UnboundedSender<TurnStreamEvent>,
    ) -> Result<(), DaemonError> {
        let display = progress_audit_display("Checking progress", "running", None);
        self.persist_provider_action_output(
            turn,
            ProviderActionOutput {
                index,
                kind: ConversationItemKind::Activity,
                status: ConversationItemStatus::Running,
                action_kind: "progress_audit",
                title: "Checking progress".to_string(),
                summary: Some("Checking progress".to_string()),
                payload: json!({ "kind": "progress_audit", "status": "running" }),
                display,
            },
            item_tx,
        )
        .await
    }

    pub(super) async fn persist_progress_audit_completed(
        &mut self,
        turn: &ProviderActionTurn,
        index: usize,
        label: &str,
        summary: &str,
        item_tx: &mpsc::UnboundedSender<TurnStreamEvent>,
    ) -> Result<(), DaemonError> {
        let display = progress_audit_display(label, "completed", Some(summary));
        self.persist_provider_action_output(
            turn,
            ProviderActionOutput {
                index,
                kind: ConversationItemKind::Activity,
                status: ConversationItemStatus::Completed,
                action_kind: "progress_audit",
                title: label.to_string(),
                summary: Some(summary.to_string()),
                payload: json!({ "kind": "progress_audit", "status": "completed", "label": label }),
                display,
            },
            item_tx,
        )
        .await
    }

    pub(super) async fn persist_agent_initiated_provider_response(
        &mut self,
        conversation_id: &str,
        turn_id: &str,
        turn_index: u64,
        response: GenerateResponse,
    ) -> Result<usize, DaemonError> {
        let mut persisted_count = 0usize;
        for (index, output) in response.responses.into_iter().enumerate() {
            let GenerateResponseItem::Text { text, .. } = output else {
                continue;
            };
            let metadata = json!({
                "turn_index": turn_index,
                "response_index": index,
                "provider": response.provider.clone(),
                "source": "agent_onboarding",
            });
            self.store
                .append_conversation_item(NewConversationItem {
                    conversation_id: conversation_id.to_string(),
                    turn_id: Some(turn_id.to_string()),
                    parent_item_id: None,
                    kind: ConversationItemKind::AssistantText,
                    status: ConversationItemStatus::Completed,
                    author: ActorRef::agent("agent:primary"),
                    content_text: Some(text),
                    payload_json: json!({}),
                    metadata,
                })
                .await?;
            persisted_count += 1;
        }
        Ok(persisted_count)
    }

    pub(super) async fn persist_partial_provider_action_outputs(
        &mut self,
        turn: &ProviderActionTurn,
        output: Vec<GenerateActionItem>,
        item_tx: &mpsc::UnboundedSender<TurnStreamEvent>,
    ) -> Result<(), DaemonError> {
        for (index, output) in output.into_iter().enumerate() {
            self.persist_provider_action_item(turn, index, output, item_tx)
                .await?;
        }
        Ok(())
    }

    pub(super) async fn persist_provider_action_item(
        &mut self,
        turn: &ProviderActionTurn,
        index: usize,
        output: GenerateActionItem,
        item_tx: &mpsc::UnboundedSender<TurnStreamEvent>,
    ) -> Result<(), DaemonError> {
        match output {
            GenerateActionItem::ToolCall {
                id,
                provider_call_id,
                provider_name,
                name,
                payload,
            } => {
                let display = tool_call_display(&name, &payload);
                self.persist_provider_action_output(
                    turn,
                    ProviderActionOutput {
                        index,
                        kind: ConversationItemKind::ToolCall,
                        status: ConversationItemStatus::Completed,
                        action_kind: "tool_call",
                        title: format!("Tool call: {name}"),
                        summary: display_summary(&display, "target"),
                        payload: json!({
                            "id": id,
                            "provider_call_id": provider_call_id,
                            "provider_name": provider_name,
                            "name": name,
                            "payload": payload,
                        }),
                        display,
                    },
                    item_tx,
                )
                .await?;
            }
            GenerateActionItem::ToolResult {
                call_id,
                provider_call_id,
                provider_name,
                name,
                success,
                payload,
            } => {
                let status = if success == Some(false) {
                    ConversationItemStatus::Failed
                } else {
                    ConversationItemStatus::Completed
                };
                let display = tool_result_display(name.as_deref(), success, &payload);
                self.persist_provider_action_output(
                    turn,
                    ProviderActionOutput {
                        index,
                        kind: ConversationItemKind::ToolResult,
                        status,
                        action_kind: "tool_result",
                        title: name.as_ref().map_or_else(
                            || "Tool result".to_string(),
                            |name| format!("Tool result: {name}"),
                        ),
                        summary: display_summary(&display, "result"),
                        payload: json!({
                            "call_id": call_id,
                            "provider_call_id": provider_call_id,
                            "provider_name": provider_name,
                            "name": name,
                            "success": success,
                            "payload": payload,
                        }),
                        display,
                    },
                    item_tx,
                )
                .await?;
            }
            GenerateActionItem::ApprovalRequest {
                id,
                method,
                payload,
            } => {
                self.persist_provider_action_output(
                    turn,
                    ProviderActionOutput {
                        index,
                        kind: ConversationItemKind::ApprovalRequest,
                        status: ConversationItemStatus::Completed,
                        action_kind: "approval_request",
                        title: "Approval requested".to_string(),
                        summary: Some(method.clone()),
                        payload: json!({
                            "id": id,
                            "method": method.clone(),
                            "payload": payload,
                        }),
                        display: json!({
                            "name": "Approval requested",
                            "purpose": "Ask before taking an external action",
                            "access": "Requires approval",
                            "target": method.clone(),
                        }),
                    },
                    item_tx,
                )
                .await?;
            }
            GenerateActionItem::ApprovalResult {
                request_id,
                decision,
                payload,
            } => {
                self.persist_provider_action_output(
                    turn,
                    ProviderActionOutput {
                        index,
                        kind: ConversationItemKind::ApprovalResult,
                        status: ConversationItemStatus::Completed,
                        action_kind: "approval_result",
                        title: format!("Approval {decision}"),
                        summary: Some(decision.clone()),
                        payload: json!({
                            "request_id": request_id,
                            "decision": decision.clone(),
                            "payload": payload,
                        }),
                        display: json!({
                            "name": "Approval decision",
                            "result": decision.clone(),
                        }),
                    },
                    item_tx,
                )
                .await?;
            }
        }
        Ok(())
    }

    async fn persist_provider_action_output(
        &mut self,
        turn: &ProviderActionTurn,
        action: ProviderActionOutput,
        item_tx: &mpsc::UnboundedSender<TurnStreamEvent>,
    ) -> Result<(), DaemonError> {
        let activity_id = format!(
            "{}:{}:{}:{}",
            action.action_kind, turn.conversation_id, turn.turn_index, action.index
        );
        let activity_status = activity_status_for_conversation_item(action.status);
        let title = action.title.clone();
        let summary = action.summary.clone();
        let payload_json = json!({
            "id": activity_id,
            "activity_kind": action.action_kind,
            "status": activity_status_payload(activity_status),
            "title": title.clone(),
            "summary": summary.clone(),
            "metadata": {
                "turn_index": turn.turn_index,
                "output_index": action.index,
                "provider": turn.provider.clone(),
                "action": action.payload,
                "display": action.display,
            },
        });
        let content_text = payload_json
            .get("title")
            .and_then(serde_json::Value::as_str)
            .map(ToString::to_string);
        let metadata = json!({
            "turn_index": turn.turn_index,
            "output_index": action.index,
            "source": "provider_action",
            "provider": turn.provider.clone(),
        });
        let record = self
            .store
            .append_conversation_item(NewConversationItem {
                conversation_id: turn.conversation_id.clone(),
                turn_id: Some(turn.turn_id.clone()),
                parent_item_id: Some(turn.user_item_id.clone()),
                kind: action.kind,
                status: action.status,
                author: ActorRef::agent("agent:primary"),
                content_text,
                payload_json: payload_json.clone(),
                metadata: metadata.clone(),
            })
            .await?;

        let transcript_item = TurnTranscriptItem::Activity {
            id: activity_id,
            activity_kind: action.action_kind.to_string(),
            status: activity_status,
            title,
            summary,
            metadata: payload_json["metadata"].clone(),
        };
        send_conversation_item(item_tx, record, metadata, transcript_item);
        Ok(())
    }

    pub(super) async fn record_turn_failure(
        &mut self,
        context: &ConversationMemoryContext,
        message: String,
        item_tx: &mpsc::UnboundedSender<TurnStreamEvent>,
    ) -> Result<(), DaemonError> {
        self.record_turn_failure_notice(context, message, false, item_tx)
            .await
    }

    pub(super) async fn record_turn_failure_notice(
        &mut self,
        context: &ConversationMemoryContext,
        message: String,
        recoverable: bool,
        item_tx: &mpsc::UnboundedSender<TurnStreamEvent>,
    ) -> Result<(), DaemonError> {
        self.store.fail_conversation_turn(&context.turn_id).await?;
        self.update_conversation_agent_status(
            &context.conversation_id,
            PersistedAgentStatus::Error,
            item_tx,
        )
        .await?;
        let notice = TurnTranscriptItem::ErrorNotice {
            message,
            recoverable,
        };
        self.persist_and_send_turn_item(context, notice, item_tx)
            .await
    }

    pub(in crate::daemon) async fn persist_and_send_turn_item(
        &mut self,
        context: &ConversationMemoryContext,
        item: TurnTranscriptItem,
        item_tx: &mpsc::UnboundedSender<TurnStreamEvent>,
    ) -> Result<(), DaemonError> {
        let (record, metadata) = self.persist_turn_item(context, &item).await?;
        send_conversation_item(item_tx, record, metadata, item);
        Ok(())
    }

    async fn persist_turn_item(
        &mut self,
        context: &ConversationMemoryContext,
        item: &TurnTranscriptItem,
    ) -> Result<(ConversationItemRecord, Value), DaemonError> {
        let default_parent_item_id = context
            .assistant_item_id
            .clone()
            .or_else(|| Some(context.user_item_id.clone()));
        let (kind, status, author, parent_item_id, content_text, payload_json, metadata) =
            match item {
                TurnTranscriptItem::UserText { text } => (
                    ConversationItemKind::UserText,
                    ConversationItemStatus::Completed,
                    ActorRef::human("human:local"),
                    None,
                    Some(text.clone()),
                    json!({}),
                    json!({ "turn_index": context.turn_index }),
                ),
                TurnTranscriptItem::AssistantText { text } => (
                    ConversationItemKind::AssistantText,
                    ConversationItemStatus::Completed,
                    ActorRef::agent("agent:primary"),
                    default_parent_item_id.clone(),
                    Some(text.clone()),
                    json!({}),
                    json!({ "turn_index": context.turn_index }),
                ),
                TurnTranscriptItem::Activity {
                    id,
                    activity_kind,
                    status,
                    title,
                    summary,
                    metadata,
                } => (
                    ConversationItemKind::Activity,
                    conversation_item_status_for_activity(*status),
                    ActorRef::agent("agent:primary"),
                    default_parent_item_id.clone(),
                    Some(title.clone()),
                    json!({
                        "id": id,
                        "activity_kind": activity_kind,
                        "status": activity_status_payload(*status),
                        "title": title,
                        "summary": summary,
                        "metadata": metadata,
                    }),
                    json!({
                        "turn_index": context.turn_index,
                        "runtime_item_id": id,
                    }),
                ),
                TurnTranscriptItem::A2uiCard {
                    id,
                    schema,
                    payload,
                } => (
                    ConversationItemKind::A2uiCard,
                    ConversationItemStatus::Completed,
                    ActorRef::agent("agent:primary"),
                    default_parent_item_id.clone(),
                    None,
                    json!({
                        "id": id,
                        "schema": schema,
                        "payload": payload,
                    }),
                    json!({
                        "turn_index": context.turn_index,
                        "runtime_item_id": id,
                        "schema": schema,
                    }),
                ),
                TurnTranscriptItem::ErrorNotice {
                    message,
                    recoverable,
                } => (
                    ConversationItemKind::ErrorNotice,
                    ConversationItemStatus::Failed,
                    ActorRef::agent("agent:primary"),
                    default_parent_item_id,
                    Some(message.clone()),
                    json!({
                        "message": message,
                        "recoverable": recoverable,
                    }),
                    json!({ "turn_index": context.turn_index }),
                ),
            };

        let record = self
            .store
            .append_conversation_item(NewConversationItem {
                conversation_id: context.conversation_id.clone(),
                turn_id: Some(context.turn_id.clone()),
                parent_item_id,
                kind,
                status,
                author,
                content_text,
                payload_json,
                metadata: metadata.clone(),
            })
            .await?;
        Ok((record, metadata))
    }
}

pub(super) fn send_conversation_item(
    item_tx: &mpsc::UnboundedSender<TurnStreamEvent>,
    record: ConversationItemRecord,
    metadata: Value,
    item: TurnTranscriptItem,
) {
    let _ = item_tx.send(TurnStreamEvent::ConversationItem {
        conversation_id: record.conversation_id,
        item_id: record.item_id,
        cursor: Some(record.cursor),
        turn_id: record.turn_id,
        metadata,
        item: Box::new(item),
    });
}

pub(in crate::daemon) fn send_transient_turn_item(
    context: &ConversationMemoryContext,
    item: TurnTranscriptItem,
    item_tx: &mpsc::UnboundedSender<TurnStreamEvent>,
) {
    let runtime_item_id = match &item {
        TurnTranscriptItem::Activity { id, .. } | TurnTranscriptItem::A2uiCard { id, .. } => {
            id.clone()
        }
        TurnTranscriptItem::UserText { .. }
        | TurnTranscriptItem::AssistantText { .. }
        | TurnTranscriptItem::ErrorNotice { .. } => format!(
            "transient:{}:{}",
            context.conversation_id, context.turn_index
        ),
    };
    let _ = item_tx.send(TurnStreamEvent::ConversationItem {
        conversation_id: context.conversation_id.clone(),
        item_id: format!("transient:{runtime_item_id}"),
        cursor: None,
        turn_id: Some(context.turn_id.clone()),
        metadata: json!({
            "turn_index": context.turn_index,
            "runtime_item_id": runtime_item_id,
            "transient": true,
        }),
        item: Box::new(item),
    });
}

pub(super) fn handle_provider_stream_event(
    event: GenerateStreamEvent,
    item_tx: &mpsc::UnboundedSender<TurnStreamEvent>,
    context: &ConversationMemoryContext,
    stream_id: &str,
    output_index_base: usize,
) {
    match event {
        GenerateStreamEvent::AssistantTextDelta {
            response_index,
            delta,
        } => send_assistant_text_delta(
            item_tx,
            &context.conversation_id,
            &context.turn_id,
            &assistant_response_stream_id(stream_id, response_index),
            response_index,
            delta,
        ),
        GenerateStreamEvent::MemoryProposalsStarted => {}
        GenerateStreamEvent::ToolCallStarted { output_index, name } => {
            send_tool_call_started_transient(
                context,
                item_tx,
                output_index_base + output_index,
                &name,
            );
        }
    }
}

fn send_tool_call_started_transient(
    context: &ConversationMemoryContext,
    item_tx: &mpsc::UnboundedSender<TurnStreamEvent>,
    output_index: usize,
    name: &str,
) {
    let display = tool_call_display(name, &json!({}));
    let activity_id = format!(
        "tool_call:{}:{}:{}",
        context.conversation_id, context.turn_index, output_index
    );
    let activity = TurnTranscriptItem::Activity {
        id: activity_id,
        activity_kind: "tool_call".to_string(),
        status: TurnActivityStatus::Started,
        title: format!("Tool call: {name}"),
        summary: display_summary(&display, "target"),
        metadata: json!({
            "turn_index": context.turn_index,
            "output_index": output_index,
            "source": "provider_stream",
            "provider": "provider_stream",
            "action": {
                "name": name,
            },
            "display": display,
        }),
    };
    send_transient_turn_item(context, activity, item_tx);
}

fn send_assistant_text_delta(
    item_tx: &mpsc::UnboundedSender<TurnStreamEvent>,
    conversation_id: &str,
    turn_id: &str,
    stream_id: &str,
    response_index: usize,
    delta: String,
) {
    let _ = item_tx.send(TurnStreamEvent::AssistantTextDelta {
        conversation_id: conversation_id.to_string(),
        turn_id: turn_id.to_string(),
        stream_id: stream_id.to_string(),
        response_index,
        delta,
    });
}

const fn conversation_item_status_for_activity(
    status: TurnActivityStatus,
) -> ConversationItemStatus {
    match status {
        TurnActivityStatus::Started => ConversationItemStatus::Running,
        TurnActivityStatus::Completed => ConversationItemStatus::Completed,
        TurnActivityStatus::Failed => ConversationItemStatus::Failed,
    }
}

const fn activity_status_for_conversation_item(
    status: ConversationItemStatus,
) -> TurnActivityStatus {
    match status {
        ConversationItemStatus::Pending | ConversationItemStatus::Running => {
            TurnActivityStatus::Started
        }
        ConversationItemStatus::Completed => TurnActivityStatus::Completed,
        ConversationItemStatus::Failed
        | ConversationItemStatus::Cancelled
        | ConversationItemStatus::Interrupted => TurnActivityStatus::Failed,
    }
}

const fn activity_status_payload(status: TurnActivityStatus) -> &'static str {
    match status {
        TurnActivityStatus::Started => "started",
        TurnActivityStatus::Completed => "completed",
        TurnActivityStatus::Failed => "failed",
    }
}

fn tool_call_display(name: &str, payload: &Value) -> Value {
    let arguments = tool_arguments(payload);
    let mut display = json!({
        "name": readable_tool_name(name),
        "access": tool_access_label(name),
    });

    if name == "search_memory" {
        insert_display_value(
            &mut display,
            "purpose",
            purpose_label(arguments.get("purpose").and_then(Value::as_str)),
        );
        insert_display_value(
            &mut display,
            "scope",
            scope_label(arguments.get("scope_ids")),
        );
        let query = arguments
            .get("query")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty());
        insert_display_value(
            &mut display,
            "target",
            Some(query.map_or_else(
                || "Scoped memories".to_string(),
                |query| format!("Memory search: {query}"),
            )),
        );
    } else if name == "web.search" {
        insert_display_value(
            &mut display,
            "purpose",
            arguments
                .get("reason")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToString::to_string),
        );
        let query = arguments
            .get("query")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty());
        insert_display_value(&mut display, "target", query.map(ToString::to_string));
    } else if name == "web.fetch" {
        insert_display_value(
            &mut display,
            "purpose",
            arguments
                .get("reason")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToString::to_string),
        );
        let url = arguments
            .get("url")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(crate::web_fetch::tool::sanitized_web_fetch_display_url);
        insert_display_value(&mut display, "target", url);
    } else if name == "update_own_name" {
        insert_display_value(
            &mut display,
            "purpose",
            Some("Save the agent name you requested".to_string()),
        );
        if let Some(agent_name) = arguments.get("name").and_then(Value::as_str) {
            insert_display_value(&mut display, "target", Some(agent_name.to_string()));
        }
    } else {
        insert_display_value(
            &mut display,
            "purpose",
            Some("Use an enabled connected tool".to_string()),
        );
    }

    display
}

fn tool_result_display(name: Option<&str>, success: Option<bool>, payload: &Value) -> Value {
    let name = name.unwrap_or("tool");
    let mut display = json!({
        "name": readable_tool_name(name),
        "access": tool_access_label(name),
    });

    if name == "search_memory" {
        insert_display_value(&mut display, "scope", scope_label(payload.get("scope_ids")));
        insert_display_value(
            &mut display,
            "result",
            Some(memory_search_result_label(payload)),
        );
    } else if name == "web.search" {
        insert_display_value(
            &mut display,
            "result",
            Some(web_search_result_label(success, payload)),
        );
        insert_display_value(
            &mut display,
            "provider",
            payload
                .get("provider")
                .and_then(Value::as_str)
                .map(web_search_provider_label),
        );
        insert_display_value(
            &mut display,
            "reliability",
            payload
                .get("provider_contract")
                .and_then(Value::as_str)
                .map(web_search_contract_label),
        );
        insert_display_value(
            &mut display,
            "fallbackFrom",
            payload
                .get("fallback_from")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToString::to_string),
        );
        insert_display_value(
            &mut display,
            "fallbackReason",
            payload
                .get("fallback_reason")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToString::to_string),
        );
    } else if name == "web.fetch" {
        insert_display_value(
            &mut display,
            "result",
            Some(web_fetch_result_label(success, payload)),
        );
        insert_display_value(
            &mut display,
            "model",
            payload
                .get("summary_model")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToString::to_string),
        );
        insert_display_value(
            &mut display,
            "fallbackFrom",
            payload
                .get("fallback_from")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToString::to_string),
        );
        insert_display_value(
            &mut display,
            "fallbackReason",
            payload
                .get("fallback_reason")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToString::to_string),
        );
    } else if name == "update_own_name" {
        let result = payload
            .get("display_name")
            .and_then(Value::as_str)
            .map(ToString::to_string)
            .unwrap_or_else(|| success_result_label(success, payload));
        insert_display_value(&mut display, "name", Some("Saved name".to_string()));
        insert_display_value(&mut display, "result", Some(result));
    } else {
        insert_display_value(
            &mut display,
            "result",
            Some(success_result_label(success, payload)),
        );
    }

    display
}

fn progress_audit_display(label: &str, status: &str, summary: Option<&str>) -> Value {
    let mut display = json!({
        "name": label,
        "access": "Reviews tool progress",
        "status": status,
    });
    insert_display_value(&mut display, "summary", summary.map(str::to_string));
    display
}

fn display_summary(display: &Value, key: &str) -> Option<String> {
    display
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn tool_arguments(payload: &Value) -> &Value {
    payload.get("arguments").unwrap_or(payload)
}

fn readable_tool_name(name: &str) -> String {
    match name {
        "search_memory" => "Search memory".to_string(),
        "update_own_name" => "Save name".to_string(),
        "web.search" => "Web Search".to_string(),
        "web.fetch" => "Fetched Web Page".to_string(),
        other => other
            .split('.')
            .next_back()
            .unwrap_or(other)
            .split(['_', '-'])
            .filter(|part| !part.is_empty())
            .enumerate()
            .map(|(index, part)| {
                let lower = part.to_ascii_lowercase();
                if index == 0 {
                    let mut chars = lower.chars();
                    chars
                        .next()
                        .map(|first| first.to_ascii_uppercase().to_string() + chars.as_str())
                        .unwrap_or(lower)
                } else {
                    lower
                }
            })
            .collect::<Vec<_>>()
            .join(" "),
    }
}

fn tool_access_label(name: &str) -> &'static str {
    match name {
        "search_memory" => "Reads memory",
        "update_own_name" => "Updates agent profile",
        "web.search" => "Searches public web",
        "web.fetch" => "Fetches public web pages",
        _ => "Uses a connected tool",
    }
}

fn purpose_label(purpose: Option<&str>) -> Option<String> {
    match purpose {
        Some("answer_human_question") => {
            Some("Answer the question from saved memories".to_string())
        }
        Some("personalize_response") => {
            Some("Personalize this response from saved memories".to_string())
        }
        Some("continue_task") => Some("Continue the current task with saved context".to_string()),
        Some("use_tool") => Some("Use saved context before a tool action".to_string()),
        Some(other) => Some(other.replace('_', " ")),
        None => None,
    }
}

fn scope_label(scope_ids: Option<&Value>) -> Option<String> {
    let scopes = scope_ids?.as_array()?;
    let labels = scopes
        .iter()
        .filter_map(Value::as_str)
        .filter(|scope| !scope.trim().is_empty())
        .collect::<Vec<_>>();
    if labels.is_empty() {
        None
    } else {
        Some(labels.join(", "))
    }
}

fn memory_search_result_label(payload: &Value) -> String {
    if let Some(error) = payload.get("error").and_then(Value::as_str) {
        return format!("Failed: {error}");
    }
    let memory_count = payload
        .get("memories")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    let mut result = match memory_count {
        0 => "Found no memories".to_string(),
        1 => "Found 1 memory".to_string(),
        count => format!("Found {count} memories"),
    };
    let omission_count = payload
        .get("omissions")
        .and_then(Value::as_array)
        .map(|omissions| {
            omissions
                .iter()
                .filter_map(|omission| omission.get("count").and_then(Value::as_u64))
                .sum::<u64>()
        })
        .unwrap_or(0);
    if omission_count > 0 {
        result.push_str(&format!("; {omission_count} omitted by policy"));
    }
    result
}

fn web_search_result_label(success: Option<bool>, payload: &Value) -> String {
    if let Some(error) = payload.get("error").and_then(Value::as_str) {
        return format!("Failed: {error}");
    }
    payload
        .get("summary")
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| success_result_label(success, payload))
}

fn web_search_provider_label(provider: &str) -> String {
    match provider {
        "duckduckgo_public" => "DuckDuckGo public search".to_string(),
        other => other.replace('_', " "),
    }
}

fn web_search_contract_label(contract: &str) -> String {
    match contract {
        "best_effort_public" => "Best effort".to_string(),
        other => other.replace('_', " "),
    }
}

fn web_fetch_result_label(success: Option<bool>, payload: &Value) -> String {
    if let Some(error) = payload.get("error").and_then(Value::as_str) {
        return format!("Failed: {error}");
    }

    let raw_chars = payload.get("raw_chars").and_then(Value::as_u64);
    let returned_chars = payload.get("returned_chars").and_then(Value::as_u64);
    if payload
        .get("content_kind")
        .and_then(Value::as_str)
        .is_some_and(|content_kind| content_kind == "summary")
        && raw_chars.is_some()
        && returned_chars.is_some()
    {
        return format!(
            "Summarized {} chars to {} chars",
            format_count(raw_chars.unwrap_or_default()),
            format_count(returned_chars.unwrap_or_default())
        );
    }

    if let Some(chars) = returned_chars.or(raw_chars) {
        return format!("Fetched {} chars", format_count(chars));
    }

    success_result_label(success, payload)
}

fn format_count(count: u64) -> String {
    let digits = count.to_string();
    let mut formatted = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, digit) in digits.chars().rev().enumerate() {
        if index > 0 && index % 3 == 0 {
            formatted.push(',');
        }
        formatted.push(digit);
    }
    formatted.chars().rev().collect()
}

fn success_result_label(success: Option<bool>, payload: &Value) -> String {
    if let Some(error) = payload.get("error").and_then(Value::as_str) {
        return format!("Failed: {error}");
    }
    match success {
        Some(false) => "Failed".to_string(),
        _ => "Completed".to_string(),
    }
}

fn insert_display_value(display: &mut Value, key: &str, value: Option<String>) {
    let Some(value) = value else {
        return;
    };
    if value.trim().is_empty() {
        return;
    }
    if let Some(object) = display.as_object_mut() {
        object.insert(key.to_string(), Value::String(value));
    }
}

pub(super) fn assistant_stream_id(turn_id: &str, segment: &str) -> String {
    format!("assistant_stream:{turn_id}:{segment}")
}

fn assistant_response_stream_id(stream_id: &str, response_index: usize) -> String {
    format!("{stream_id}:response:{response_index}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn web_search_tool_call_display_shows_visible_query() {
        let display = tool_call_display(
            "web.search",
            &json!({
                "query": "rust language",
                "reason": "answer current question",
                "max_results": 3
            }),
        );

        assert_eq!(display["name"], "Web Search");
        assert_eq!(display["access"], "Searches public web");
        assert_eq!(display["target"], "rust language");
        assert_eq!(display["purpose"], "answer current question");
    }

    #[test]
    fn web_search_tool_result_display_shows_provider_and_count() {
        let display = tool_result_display(
            Some("web.search"),
            Some(true),
            &json!({
                "provider": "duckduckgo_public",
                "provider_contract": "best_effort_public",
                "summary": "Found 2 web results",
                "results": [{}, {}]
            }),
        );

        assert_eq!(display["name"], "Web Search");
        assert_eq!(display["result"], "Found 2 web results");
        assert_eq!(display["provider"], "DuckDuckGo public search");
        assert_eq!(display["reliability"], "Best effort");
    }

    #[test]
    fn web_search_display_shows_provider_fallback_without_raw_payload() {
        let display = tool_result_display(
            Some("web.search"),
            Some(true),
            &json!({
                "provider": "duckduckgo_public",
                "provider_contract": "best_effort_public",
                "fallback_from": "openai",
                "fallback_reason": "provider account unauthenticated",
                "query": "rust learn",
                "results": [],
                "summary": "No web results found",
                "raw_provider_payload": "secret",
            }),
        );

        assert_eq!(display["provider"], "DuckDuckGo public search");
        assert_eq!(display["fallbackFrom"], "openai");
        assert_eq!(
            display["fallbackReason"],
            "provider account unauthenticated"
        );
        assert!(!display.to_string().contains("secret"));
    }

    #[test]
    fn web_fetch_tool_call_display_shows_visible_url() {
        let display = tool_call_display(
            "web.fetch",
            &json!({
                "url": "https://example.com/page",
                "reason": "read public documentation",
                "max_chars": 4000
            }),
        );

        assert_eq!(display["name"], "Fetched Web Page");
        assert_eq!(display["access"], "Fetches public web pages");
        assert_eq!(display["target"], "https://example.com/page");
        assert_eq!(display["purpose"], "read public documentation");
    }

    #[test]
    fn web_fetch_tool_call_display_redacts_sensitive_url_components() {
        let display = tool_call_display(
            "web.fetch",
            &json!({
                "url": "https://user:secret@example.com/page#token",
                "reason": "read public documentation"
            }),
        );

        assert_eq!(display["target"], "[redacted sensitive web.fetch URL]");
    }

    #[test]
    fn web_fetch_tool_result_display_shows_raw_and_summary_counts() {
        let raw_display = tool_result_display(
            Some("web.fetch"),
            Some(true),
            &json!({
                "provider": "direct_http",
                "fallback_from": "provider_account:codex:test",
                "fallback_reason": "bound provider account does not declare web.fetch",
                "url": "https://example.com/page",
                "final_url": "https://example.com/page",
                "title": "Example",
                "format": "markdown",
                "extraction": "readability_rs",
                "content_kind": "raw_markdown",
                "content": "secret raw fetched body",
                "raw_excerpt": "secret raw excerpt",
                "raw_chars": 183421,
                "returned_chars": 12840,
                "summary_model": null,
                "summary_strategy": "not_summarized",
                "truncated": true
            }),
        );
        assert_eq!(raw_display["name"], "Fetched Web Page");
        assert_eq!(raw_display["access"], "Fetches public web pages");
        assert_eq!(raw_display["result"], "Fetched 12,840 chars");
        assert_eq!(raw_display["fallbackFrom"], "provider_account:codex:test");
        assert_eq!(
            raw_display["fallbackReason"],
            "bound provider account does not declare web.fetch"
        );
        assert!(raw_display.get("model").is_none());
        assert!(!raw_display.to_string().contains("secret raw"));

        let saved_name_display = tool_result_display(
            Some("update_own_name"),
            Some(true),
            &json!({
                "display_name": "Momo",
            }),
        );
        assert_eq!(saved_name_display["name"], "Saved name");
        assert_eq!(saved_name_display["result"], "Momo");

        let summary_display = tool_result_display(
            Some("web.fetch"),
            Some(true),
            &json!({
                "provider": "direct_http",
                "url": "https://example.com/long",
                "final_url": "https://example.com/long",
                "title": "Long Example",
                "format": "markdown",
                "extraction": "readability_rs",
                "content_kind": "summary",
                "content": "secret summary text",
                "raw_excerpt": "secret raw excerpt",
                "raw_chars": 183421,
                "returned_chars": 4972,
                "summary_model": "gpt-5.4-mini",
                "summary_strategy": "single_pass",
                "truncated": false
            }),
        );
        assert_eq!(
            summary_display["result"],
            "Summarized 183,421 chars to 4,972 chars"
        );
        assert_eq!(summary_display["model"], "gpt-5.4-mini");
        assert!(!summary_display.to_string().contains("secret"));
    }

    #[test]
    fn progress_audit_display_labels_running_and_completed_states() {
        let running = progress_audit_display("Checking progress", "running", None);
        assert_eq!(running["name"], "Checking progress");
        assert_eq!(running["status"], "running");

        let completed = progress_audit_display(
            "Still making progress",
            "completed",
            Some("Found new sources and is preparing the write step."),
        );
        assert_eq!(completed["name"], "Still making progress");
        assert_eq!(completed["status"], "completed");
        assert_eq!(
            completed["summary"],
            "Found new sources and is preparing the write step."
        );
    }

    #[test]
    fn provider_usage_metadata_includes_cache_hit_ratio() {
        let metadata = provider_usage_metadata(
            "codex",
            "gpt-test",
            "initial",
            ProviderResponsePosition {
                response_index: 0,
                output_index: Some(0),
            },
            Some(&crate::provider::TokenUsage {
                input_tokens: 12000,
                output_tokens: 900,
                total_tokens: 12900,
                cached_input_tokens: Some(9600),
            }),
        );

        assert_eq!(metadata["provider_usage"]["provider"], "codex");
        assert_eq!(metadata["provider_usage"]["model"], "gpt-test");
        assert_eq!(metadata["provider_usage"]["phase"], "initial");
        assert_eq!(metadata["provider_usage"]["response_index"], 0);
        assert_eq!(metadata["provider_usage"]["output_index"], 0);
        assert_eq!(metadata["provider_usage"]["input_tokens"], 12000);
        assert_eq!(metadata["provider_usage"]["cached_input_tokens"], 9600);
        assert_eq!(metadata["provider_usage"]["cache_hit_ratio"], 0.8);
    }

    #[test]
    fn provider_usage_metadata_preserves_explicit_zero_cached_tokens() {
        let metadata = provider_usage_metadata(
            "codex",
            "gpt-test",
            "continuation",
            ProviderResponsePosition {
                response_index: 1,
                output_index: Some(12),
            },
            Some(&crate::provider::TokenUsage {
                input_tokens: 2048,
                output_tokens: 12,
                total_tokens: 2060,
                cached_input_tokens: Some(0),
            }),
        );

        assert_eq!(metadata["provider_usage"]["phase"], "continuation");
        assert_eq!(metadata["provider_usage"]["response_index"], 1);
        assert_eq!(metadata["provider_usage"]["output_index"], 12);
        assert_eq!(metadata["provider_usage"]["cached_input_tokens"], 0);
        assert_eq!(metadata["provider_usage"]["cache_hit_ratio"], 0.0);
    }

    #[test]
    fn provider_usage_metadata_omits_ratio_when_cached_tokens_are_unreported() {
        let metadata = provider_usage_metadata(
            "foundation_local",
            "foundation-local-default",
            "initial",
            ProviderResponsePosition {
                response_index: 0,
                output_index: None,
            },
            Some(&crate::provider::TokenUsage {
                input_tokens: 100,
                output_tokens: 5,
                total_tokens: 105,
                cached_input_tokens: None,
            }),
        );

        assert!(
            metadata["provider_usage"]
                .get("cached_input_tokens")
                .is_none()
        );
        assert!(metadata["provider_usage"].get("cache_hit_ratio").is_none());
    }
}
