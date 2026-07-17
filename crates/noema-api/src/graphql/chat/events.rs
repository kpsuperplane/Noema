use super::*;

pub(in crate::graphql) async fn conversation_events(
    state: &GraphqlState,
    human_id: &str,
    conversation_id: String,
) -> Result<impl Stream<Item = GraphqlConversationEvent>> {
    super::operations::require_conversation_owner(state.store()?, &conversation_id, human_id)
        .await?;
    let subscriptions = state.subscriptions().clone();
    let mut rx = subscriptions.subscribe_conversation(&conversation_id);

    Ok(async_stream::stream! {
        yield GraphqlConversationEvent::SubscriptionReady(
            GraphqlSubscriptionReadyEvent {
                conversation_id: conversation_id.clone(),
            },
        );

        loop {
            let event = match rx.recv().await {
                Ok(event) => event,
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                    yield GraphqlConversationEvent::SubscriptionReady(
                        GraphqlSubscriptionReadyEvent {
                            conversation_id: conversation_id.clone(),
                        },
                    );
                    continue;
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            };
            match event {
                ConversationRuntimeEvent::Turn {
                    client_message_id,
                    event,
                } => match *event {
                    noema_runtime::TurnStreamEvent::ConversationItem {
                            conversation_id,
                            item_id,
                            cursor,
                            turn_id,
                            metadata,
                            item,
                        } => {
                    yield GraphqlConversationEvent::ConversationItem(
                        Box::new(GraphqlConversationItemEvent {
                            conversation_id,
                            client_message_id,
                            item_id,
                            cursor,
                            turn_id,
                            metadata: async_graphql::Json(metadata),
                            item: (*item).into(),
                        }),
                    );
                }
                    noema_runtime::TurnStreamEvent::AgentStatusChanged {
                            conversation_id,
                            status,
                        } => {
                    yield GraphqlConversationEvent::AgentStatusChanged(
                        GraphqlAgentStatusEvent {
                            conversation_id,
                            status: status.into(),
                        },
                    );
                }
                        noema_runtime::TurnStreamEvent::AssistantTextDelta {
                            conversation_id,
                            turn_id,
                            stream_id,
                            response_index,
                            delta,
                        } => {
                    yield GraphqlConversationEvent::AssistantTextDelta(
                        GraphqlAssistantTextDeltaEvent {
                            conversation_id,
                            turn_id,
                            stream_id,
                            response_index,
                            delta,
                        },
                    );
                }
                },
                ConversationRuntimeEvent::Completed {
                    conversation_id,
                    client_message_id,
                } => {
                    yield GraphqlConversationEvent::TurnCompleted(
                        GraphqlTurnCompletedEvent {
                            conversation_id,
                            client_message_id,
                        },
                    );
                }
            }
        }
    })
}

pub(super) fn publish_turn_terminal_events(
    subscriptions: &RuntimeEventRegistry,
    conversation_id: String,
    client_message_id: Option<String>,
    published_error_notice: bool,
    result: std::result::Result<(), noema_runtime::RuntimeError>,
) {
    if let Err(ref error) = result
        && !published_error_notice
    {
        let error_item_id = client_message_id.as_ref().map_or_else(
            || format!("graphql_runtime_error:{conversation_id}:uncorrelated"),
            |client_message_id| {
                format!("graphql_runtime_error:{conversation_id}:{client_message_id}")
            },
        );
        subscriptions.publish_conversation(ConversationRuntimeEvent::Turn {
            client_message_id: client_message_id.clone(),
            event: Box::new(TurnStreamEvent::ConversationItem {
                conversation_id: conversation_id.clone(),
                item_id: error_item_id,
                cursor: None,
                turn_id: None,
                metadata: serde_json::json!({}),
                item: Box::new(noema_runtime::TurnTranscriptItem::ErrorNotice {
                    message: error.to_string(),
                    recoverable: false,
                }),
            }),
        });
    }

    mark_turn_timing_event(
        "graphql_turn_completed",
        &conversation_id,
        client_message_id.as_deref(),
        serde_json::json!({
            "success": result.is_ok(),
            "published_error_notice": published_error_notice,
        }),
    );
    subscriptions.publish_conversation(ConversationRuntimeEvent::Completed {
        conversation_id,
        client_message_id,
    });
}

pub(super) fn mark_graphql_published_turn_event(
    event: &TurnStreamEvent,
    client_message_id: Option<&str>,
) {
    match event {
        TurnStreamEvent::ConversationItem {
            conversation_id,
            item_id,
            turn_id,
            item,
            ..
        } => {
            let (item_kind, activity_kind, status) = match item.as_ref() {
                TurnTranscriptItem::UserText { .. } => ("user_text", None, None),
                TurnTranscriptItem::AssistantText { .. } => ("assistant_text", None, None),
                TurnTranscriptItem::Activity {
                    activity_kind,
                    status,
                    ..
                } => (
                    "activity",
                    Some(activity_kind.as_str()),
                    Some(activity_status_label(*status)),
                ),
                TurnTranscriptItem::A2uiCard { schema, .. } => {
                    ("a2ui_card", Some(schema.as_str()), None)
                }
                TurnTranscriptItem::MultipleChoicePrompt { .. } => {
                    ("multiple_choice_prompt", None, None)
                }
                TurnTranscriptItem::MultipleChoiceSelection { .. } => {
                    ("multiple_choice_selection", None, None)
                }
                TurnTranscriptItem::ErrorNotice { .. } => ("error_notice", None, None),
                TurnTranscriptItem::ArtifactReference { .. } => ("artifact_reference", None, None),
                TurnTranscriptItem::TaskReference { status, .. } => {
                    ("task_reference", None, Some(status.as_str()))
                }
            };
            mark_turn_timing_event(
                "graphql_publish_conversation_item",
                conversation_id,
                client_message_id,
                serde_json::json!({
                    "item_id": item_id,
                    "turn_id": turn_id,
                    "item_kind": item_kind,
                    "activity_kind": activity_kind,
                    "status": status,
                }),
            );
        }
        TurnStreamEvent::AssistantTextDelta {
            conversation_id,
            turn_id,
            stream_id,
            response_index,
            delta,
        } => mark_turn_timing_event(
            "graphql_publish_assistant_delta",
            conversation_id,
            client_message_id,
            serde_json::json!({
                "turn_id": turn_id,
                "stream_id": stream_id,
                "response_index": response_index,
                "delta_chars": delta.chars().count(),
            }),
        ),
        TurnStreamEvent::AgentStatusChanged {
            conversation_id,
            status,
        } => mark_turn_timing_event(
            "graphql_publish_agent_status",
            conversation_id,
            client_message_id,
            serde_json::json!({
                "status": format!("{status:?}"),
            }),
        ),
    }
}

fn activity_status_label(status: TurnActivityStatus) -> &'static str {
    match status {
        TurnActivityStatus::Started => "started",
        TurnActivityStatus::Completed => "completed",
        TurnActivityStatus::Failed => "failed",
    }
}

pub(super) fn turn_event_is_error_notice(event: &TurnStreamEvent) -> bool {
    matches!(
        event,
        TurnStreamEvent::ConversationItem {
            item,
            ..
        } if matches!(item.as_ref(), noema_runtime::TurnTranscriptItem::ErrorNotice { .. })
    )
}
