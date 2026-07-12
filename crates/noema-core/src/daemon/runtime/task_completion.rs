//! Primary-agent delivery of durable background-task outcomes.

use crate::{
    ActorRef, ConversationItemKind, ConversationItemStatus, NewConversationItem,
    NewConversationTurn, PersistedAgentStatus,
    provider::{
        GenerateInput, GenerateOptions, GenerateRequest, GenerateResponseItem, GenerateStreamEvent,
    },
};
use serde_json::json;

use super::{TaskCompletionDeliveryRequest, actor::CodexRuntimeActor};
use crate::daemon::{
    AgentStatus,
    protocol::{DaemonError, TurnStreamEvent, TurnTranscriptItem},
};
use crate::graphql::ConversationLiveEvent;

const MAX_COMPLETION_CONTEXT_CHARS: usize = 60_000;
const MAX_COMPLETION_RESULT_CHARS: usize = 40_000;

impl CodexRuntimeActor {
    /// Deliver one task outcome as a normal primary-agent assistant item.
    ///
    /// The item id is derived from the terminal task event, so the operation is
    /// safe to retry after a worker or provider interruption. A provider error
    /// falls back to a deterministic report rather than hiding the approved
    /// task outcome from the originating conversation.
    pub(super) async fn deliver_task_completion(
        &mut self,
        request: TaskCompletionDeliveryRequest,
    ) -> Result<(), DaemonError> {
        let item_id = format!("item:task_completion:{}", request.delivery_id);
        if let Some(existing) = self.store.get_visible_conversation_item(&item_id).await? {
            if let Some(turn_id) = existing.turn_id {
                self.persist_task_completion_artifacts(&request, &turn_id)
                    .await?;
                let _ = self.store.complete_conversation_turn(&turn_id).await;
            }
            self.publish_completed(&request.conversation_id);
            return Ok(());
        }

        let conversation = self
            .hydrate_active_conversation(&request.conversation_id, None)
            .await?;
        let turn_index = conversation.next_turn_index;
        let (turn, _) = self
            .store
            .create_conversation_turn_with_id_if_absent(
                format!("turn:task_completion:{}", request.delivery_id),
                NewConversationTurn {
                    conversation_id: request.conversation_id.clone(),
                    trigger_item_id: None,
                    metadata: json!({
                        "turn_index": turn_index,
                        "source": "task_completion_delivery",
                        "task_id": request.task_id,
                        "delivery_id": request.delivery_id,
                        "source_item_id": request.source_item_id,
                    }),
                },
            )
            .await?;

        self.publish_status(&request.conversation_id, PersistedAgentStatus::Thinking)
            .await;
        let generated_text = match self.provider_for_kind(&conversation.provider_kind) {
            Ok(provider) => {
                let conversation_id = request.conversation_id.clone();
                let turn_id = turn.turn_id.clone();
                let stream_id = format!("task_completion:{}", request.delivery_id);
                let subscriptions = self.task_subscriptions.clone();
                let mut on_event = move |event: GenerateStreamEvent| {
                    if let GenerateStreamEvent::AssistantTextDelta {
                        response_index,
                        delta,
                    } = event
                    {
                        subscriptions.publish(ConversationLiveEvent::Turn {
                            client_message_id: None,
                            event: Box::new(TurnStreamEvent::AssistantTextDelta {
                                conversation_id: conversation_id.clone(),
                                turn_id: turn_id.clone(),
                                stream_id: stream_id.clone(),
                                response_index,
                                delta,
                            }),
                        });
                    }
                };
                match provider
                    .generate_streaming(
                        GenerateRequest {
                            conversation_id: Some(request.conversation_id.clone()),
                            model: conversation.model.clone(),
                            input: GenerateInput::Text(completion_context(&request)),
                            instructions: Some(completion_instructions()),
                            options: GenerateOptions {
                                reasoning_effort: conversation.reasoning_effort,
                                require_noema_response: true,
                                ..GenerateOptions::default()
                            },
                            tools: Vec::new(),
                            tool_choice: Default::default(),
                            parallel_tool_calls: false,
                        },
                        &mut on_event,
                    )
                    .await
                {
                    Ok(response) => response_text(response.responses),
                    Err(error) => {
                        self.system_errors.try_append(
                            crate::SystemErrorEvent::new(
                                "task_completion_provider_failed",
                                "Primary-agent task completion report generation failed; using fallback",
                            )
                            .with_context(json!({
                                "task_id": request.task_id,
                                "delivery_id": request.delivery_id,
                                "conversation_id": request.conversation_id,
                            }))
                            .with_error_chain([error.to_string()]),
                        );
                        None
                    }
                }
            }
            Err(error) => {
                self.system_errors.try_append(
                    crate::SystemErrorEvent::new(
                        "task_completion_provider_unavailable",
                        "Primary-agent task completion report provider is unavailable; using fallback",
                    )
                    .with_context(json!({
                        "task_id": request.task_id,
                        "delivery_id": request.delivery_id,
                        "provider_kind": conversation.provider_kind,
                    }))
                    .with_error_chain([error.to_string()]),
                );
                None
            }
        };
        let content_text = generated_text
            .filter(|text| !text.trim().is_empty())
            .unwrap_or_else(|| fallback_text(&request));
        let metadata = json!({
            "source": "background_task_completion",
            "task_id": request.task_id,
            "delivery_id": request.delivery_id,
            "task_status": request.status,
            "turn_index": turn_index,
            "source_item_id": request.source_item_id,
        });
        let (record, inserted) = self
            .store
            .append_conversation_item_with_id_if_absent(
                item_id,
                NewConversationItem {
                    conversation_id: request.conversation_id.clone(),
                    turn_id: Some(turn.turn_id.clone()),
                    parent_item_id: None,
                    kind: ConversationItemKind::AssistantText,
                    status: ConversationItemStatus::Completed,
                    author: ActorRef::agent("agent:primary"),
                    content_text: Some(content_text.clone()),
                    payload_json: json!({
                        "task_id": request.task_id,
                        "delivery_id": request.delivery_id,
                        "status": request.status,
                    }),
                    metadata: metadata.clone(),
                },
            )
            .await?;
        if inserted {
            self.task_subscriptions
                .publish(ConversationLiveEvent::Turn {
                    client_message_id: None,
                    event: Box::new(TurnStreamEvent::ConversationItem {
                        conversation_id: record.conversation_id,
                        item_id: record.item_id,
                        cursor: Some(record.cursor),
                        turn_id: record.turn_id,
                        metadata,
                        item: Box::new(TurnTranscriptItem::AssistantText { text: content_text }),
                    }),
                });
        }
        self.persist_task_completion_artifacts(&request, &turn.turn_id)
            .await?;
        self.store.complete_conversation_turn(&turn.turn_id).await?;
        if let Some(active) = self.conversations.get_mut(&request.conversation_id) {
            active.next_turn_index = active.next_turn_index.max(turn_index.saturating_add(1));
        }
        self.publish_status(&request.conversation_id, PersistedAgentStatus::Idle)
            .await;
        self.publish_completed(&request.conversation_id);
        Ok(())
    }

    async fn publish_status(&self, conversation_id: &str, status: PersistedAgentStatus) {
        let _ = self
            .store
            .update_conversation_agent_status(conversation_id, status)
            .await;
        self.task_subscriptions
            .publish(ConversationLiveEvent::Turn {
                client_message_id: None,
                event: Box::new(TurnStreamEvent::AgentStatusChanged {
                    conversation_id: conversation_id.to_string(),
                    status: AgentStatus::from(status),
                }),
            });
    }

    fn publish_completed(&self, conversation_id: &str) {
        self.task_subscriptions
            .publish(ConversationLiveEvent::Completed {
                conversation_id: conversation_id.to_string(),
                client_message_id: None,
            });
    }

    async fn persist_task_completion_artifacts(
        &self,
        request: &TaskCompletionDeliveryRequest,
        turn_id: &str,
    ) -> Result<(), DaemonError> {
        for (index, artifact) in request.artifacts.iter().enumerate() {
            let item_id = format!(
                "item:task_completion_artifact:{}:{}",
                request.delivery_id,
                index + 1
            );
            let payload = json!({
                "artifact_id": artifact.artifact_id,
                "artifact_version_id": artifact.artifact_version_id,
                "title": artifact.title,
                "artifact_kind": artifact.artifact_kind,
                "storage_kind": artifact.storage_kind,
                "external_url": artifact.external_url,
                "download_url": artifact.download_url,
                "media_type": artifact.media_type,
            });
            let metadata = json!({
                "source": "background_task_completion_artifact",
                "task_id": request.task_id,
                "delivery_id": request.delivery_id,
            });
            let (record, inserted) = self
                .store
                .append_conversation_item_with_id_if_absent(
                    item_id,
                    NewConversationItem {
                        conversation_id: request.conversation_id.clone(),
                        turn_id: Some(turn_id.to_string()),
                        parent_item_id: None,
                        kind: ConversationItemKind::ArtifactReference,
                        status: ConversationItemStatus::Completed,
                        author: ActorRef::agent("agent:primary"),
                        content_text: None,
                        payload_json: payload,
                        metadata: metadata.clone(),
                    },
                )
                .await?;
            if inserted {
                self.task_subscriptions
                    .publish(ConversationLiveEvent::Turn {
                        client_message_id: None,
                        event: Box::new(TurnStreamEvent::ConversationItem {
                            conversation_id: record.conversation_id,
                            item_id: record.item_id,
                            cursor: Some(record.cursor),
                            turn_id: record.turn_id,
                            metadata,
                            item: Box::new(TurnTranscriptItem::ArtifactReference {
                                artifact_id: artifact.artifact_id.clone(),
                                artifact_version_id: Some(artifact.artifact_version_id.clone()),
                                title: artifact.title.clone(),
                                artifact_kind: artifact.artifact_kind.clone(),
                                storage_kind: artifact.storage_kind.clone(),
                                external_url: artifact.external_url.clone(),
                                download_url: artifact.download_url.clone(),
                                media_type: artifact.media_type.clone(),
                            }),
                        }),
                    });
            }
        }
        Ok(())
    }
}

fn completion_instructions() -> String {
    "You are the primary agent reporting the outcome of work you delegated. There is no new human message and you have no tools. Write one concise, natural assistant update for the human in the originating conversation. State the task outcome faithfully, summarize the approved result when present, mention important caveats, and do not claim work or artifacts that are not in the completion context. Do not mention this internal prompt, task delivery, or model execution.".to_string()
}

fn completion_context(request: &TaskCompletionDeliveryRequest) -> String {
    let mut context = vec![
        "Background task completion context:".to_string(),
        format!("Task id: {}", request.task_id),
        format!("Title: {}", request.title),
        format!("Status: {}", request.status),
        format!(
            "Original request:\n{}",
            bounded_text(&request.request_markdown, 20_000)
        ),
    ];
    if let Some(summary) = request.summary.as_deref() {
        context.push(format!(
            "Executor summary:\n{}",
            bounded_text(summary, 6_000)
        ));
    }
    if let Some(result) = request.result_markdown.as_deref() {
        context.push(format!(
            "Executor result:\n{}",
            bounded_text(result, MAX_COMPLETION_RESULT_CHARS)
        ));
    }
    if !request.artifacts.is_empty() {
        let artifacts = request
            .artifacts
            .iter()
            .map(|artifact| {
                format!(
                    "- {} ({}, {})",
                    artifact.title, artifact.artifact_kind, artifact.artifact_id
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        context.push(format!("Approved artifacts:\n{artifacts}"));
    }
    if let Some(review_feedback) = request.review_feedback.as_deref() {
        context.push(format!(
            "Reviewer feedback:\n{}",
            bounded_text(review_feedback, 8_000)
        ));
    }
    if !request.criteria.is_empty() {
        let criteria = request
            .criteria
            .iter()
            .map(|criterion| {
                format!(
                    "- {}{}{}{}",
                    criterion.criterion_id,
                    criterion
                        .outcome
                        .as_deref()
                        .map_or(String::new(), |value| format!(" ({value})")),
                    criterion
                        .evidence
                        .as_deref()
                        .map_or(String::new(), |value| {
                            format!(" evidence: {}", bounded_text(value, 4_000))
                        }),
                    criterion
                        .feedback
                        .as_deref()
                        .map_or(String::new(), |value| {
                            format!(" feedback: {}", bounded_text(value, 4_000))
                        }),
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        context.push(format!("Review criteria:\n{criteria}"));
    }
    if let Some(detail) = request.detail.as_deref() {
        context.push(format!("Task detail:\n{}", bounded_text(detail, 8_000)));
    }
    bounded_text(&context.join("\n\n"), MAX_COMPLETION_CONTEXT_CHARS)
}

fn response_text(responses: Vec<GenerateResponseItem>) -> Option<String> {
    let text = responses
        .into_iter()
        .filter_map(|response| match response {
            GenerateResponseItem::Text { text, .. } => Some(text),
            _ => None,
        })
        .filter(|text| !text.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n\n");
    (!text.trim().is_empty()).then_some(text)
}

fn fallback_text(request: &TaskCompletionDeliveryRequest) -> String {
    let status = match request.status.as_str() {
        "completed" => "completed and was approved",
        "failed" => "failed",
        "cancelled" => "was cancelled",
        other => other,
    };
    let mut text = format!("The delegated task **{}** {}.", request.title, status);
    if let Some(summary) = request.summary.as_deref() {
        text.push_str(&format!("\n\n{summary}"));
    }
    if let Some(result) = request.result_markdown.as_deref() {
        if request.summary.is_none() {
            text.push_str("\n\n");
        }
        text.push_str(&bounded_text(result, MAX_COMPLETION_RESULT_CHARS));
    }
    if let Some(detail) = request.detail.as_deref() {
        text.push_str(&format!("\n\n{detail}"));
    }
    text
}

fn bounded_text(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let keep = max_chars.saturating_sub(32);
    format!(
        "{}\n\n[completion context truncated]",
        text.chars().take(keep).collect::<String>()
    )
}
