use super::events::{
    mark_graphql_published_turn_event, publish_turn_terminal_events, turn_event_is_error_notice,
};
use super::*;

pub(in crate::graphql) async fn primary_conversation(
    state: &GraphqlState,
) -> Result<Option<GraphqlPrimaryConversation>> {
    let store = state.store()?;
    let provider_kind = primary_agent_provider_kind(store).await?;
    let conversation = store
        .primary_conversation_for_human("human:local")
        .await
        .map_err(graphql_error)?;
    Ok(conversation.map(|conversation| GraphqlPrimaryConversation {
        conversation_id: conversation.conversation_id,
        provider: provider_kind,
    }))
}

pub(in crate::graphql) async fn ensure_primary_conversation(
    state: &GraphqlState,
    cwd: Option<String>,
) -> Result<GraphqlPrimaryConversation> {
    let store = state.store()?;
    let runtime = state.runtime()?;
    let provider_kind = primary_agent_provider_kind(store).await?;
    let onboarding = state.onboarding()?.status().await.map_err(graphql_error)?;
    if !onboarding.is_user_onboarded {
        return Err(async_graphql::Error::new(
            "Noema onboarding is incomplete. Finish local model setup or connect a provider account before starting chat.",
        ));
    }

    let started = runtime
        .start_primary_conversation(cwd)
        .await
        .map_err(graphql_error)?;

    Ok(GraphqlPrimaryConversation {
        conversation_id: started.conversation_id,
        provider: provider_kind,
    })
}

pub(in crate::graphql) async fn conversation_transcript_page(
    state: &GraphqlState,
    input: GraphqlConversationTranscriptPageInput,
) -> Result<GraphqlConversationTranscriptPage> {
    visible_conversation_transcript_page(
        state,
        &input.conversation_id,
        input.cursor.as_deref(),
        input.limit,
    )
    .await
}

pub(in crate::graphql) async fn latest_conversation_transcript_page(
    state: &GraphqlState,
    conversation_id: &str,
    limit: Option<i32>,
) -> Result<GraphqlConversationTranscriptPage> {
    visible_conversation_transcript_page(state, conversation_id, None, limit).await
}

async fn visible_conversation_transcript_page(
    state: &GraphqlState,
    conversation_id: &str,
    cursor: Option<&str>,
    limit: Option<i32>,
) -> Result<GraphqlConversationTranscriptPage> {
    let limit = limit.unwrap_or(80);
    if limit < 1 {
        return Err(async_graphql::Error::new(
            "conversationTranscriptPage limit must be at least 1",
        ));
    }
    if limit > 200 {
        return Err(async_graphql::Error::new(
            "conversationTranscriptPage limit must be at most 200",
        ));
    }

    let page = state
        .store()?
        .list_visible_conversation_item_page(conversation_id, cursor, i64::from(limit))
        .await
        .map_err(graphql_error)?;
    let mut items = Vec::new();
    for record in page.items {
        if let Some(item) =
            crate::graphql::web_conversation_item_from_record(record).map_err(graphql_error)?
        {
            items.push(GraphqlConversationItem::from(item));
        }
    }

    Ok(GraphqlConversationTranscriptPage {
        items,
        page_info: GraphqlConversationTranscriptPageInfo {
            before_cursor: page.before_cursor,
            has_more_before: page.has_more_before,
            limit: i32::try_from(page.limit).unwrap_or(i32::MAX),
        },
    })
}

async fn primary_agent_provider_kind(store: &noema_store::NoemaStore) -> Result<String> {
    store
        .get_agent_runtime_preference("agent:primary")
        .await
        .map_err(graphql_error)?
        .map(|preference| preference.provider_kind)
        .ok_or_else(|| async_graphql::Error::new("primary agent provider is not initialized"))
}

pub(in crate::graphql) async fn send_conversation_turn(
    state: &GraphqlState,
    input: GraphqlSendConversationTurnInput,
) -> Result<GraphqlTurnAccepted> {
    let runtime = state.runtime()?.clone();
    let subscriptions = state.subscriptions().clone();
    let (item_tx, mut item_rx) = tokio::sync::mpsc::unbounded_channel();
    let conversation_id = input.conversation_id.clone();
    let client_message_id = input.client_message_id.clone();
    let published_client_message_id = client_message_id.clone();
    let input_text = input.input;
    let completion_conversation_id = conversation_id.clone();
    mark_turn_timing_event(
        "graphql_turn_received",
        &conversation_id,
        client_message_id.as_deref(),
        serde_json::json!({
            "input_chars": input_text.chars().count(),
        }),
    );

    tokio::spawn(async move {
        mark_turn_timing_event(
            "graphql_runtime_task_started",
            &completion_conversation_id,
            published_client_message_id.as_deref(),
            serde_json::json!({}),
        );
        let completion = runtime.turn_with_client_message_id(
            completion_conversation_id,
            input_text,
            item_tx,
            published_client_message_id.clone(),
        );
        tokio::pin!(completion);
        let mut published_error_notice = false;
        loop {
            tokio::select! {
                Some(event) = item_rx.recv() => {
                    if turn_event_is_error_notice(&event) {
                        published_error_notice = true;
                    }
                    mark_graphql_published_turn_event(&event, published_client_message_id.as_deref());
                    subscriptions.publish_conversation(ConversationRuntimeEvent::Turn {
                        client_message_id: published_client_message_id.clone(),
                        event: Box::new(event),
                    });
                }
                result = &mut completion => {
                    while let Ok(event) = item_rx.try_recv() {
                        if turn_event_is_error_notice(&event) {
                            published_error_notice = true;
                        }
                        mark_graphql_published_turn_event(&event, published_client_message_id.as_deref());
                        subscriptions.publish_conversation(ConversationRuntimeEvent::Turn {
                            client_message_id: published_client_message_id.clone(),
                            event: Box::new(event),
                        });
                    }
                    publish_turn_terminal_events(
                        &subscriptions,
                        conversation_id,
                        published_client_message_id,
                        published_error_notice,
                        result,
                    );
                    break;
                }
            }
        }
    });

    Ok(GraphqlTurnAccepted {
        conversation_id: input.conversation_id,
        client_message_id,
    })
}

pub(in crate::graphql) async fn send_multiple_choice_selection(
    state: &GraphqlState,
    input: GraphqlSendMultipleChoiceSelectionInput,
) -> Result<GraphqlTurnAccepted> {
    let runtime = state.runtime()?.clone();
    let subscriptions = state.subscriptions().clone();
    let (item_tx, mut item_rx) = tokio::sync::mpsc::unbounded_channel();
    let conversation_id = input.conversation_id.clone();
    let client_message_id = input.client_message_id.clone();
    let published_client_message_id = client_message_id.clone();
    let completion_conversation_id = conversation_id.clone();
    let prompt_item_id = input.prompt_item_id.clone();
    let selected_option_ids = input.selected_option_ids.clone();
    mark_turn_timing_event(
        "graphql_multiple_choice_selection_received",
        &conversation_id,
        client_message_id.as_deref(),
        serde_json::json!({
            "prompt_item_id": prompt_item_id,
            "selected_option_count": selected_option_ids.len(),
        }),
    );

    tokio::spawn(async move {
        mark_turn_timing_event(
            "graphql_runtime_task_started",
            &completion_conversation_id,
            published_client_message_id.as_deref(),
            serde_json::json!({}),
        );
        let completion = runtime.select_multiple_choice_with_client_message_id(
            completion_conversation_id,
            prompt_item_id,
            selected_option_ids,
            item_tx,
            published_client_message_id.clone(),
        );
        tokio::pin!(completion);
        let mut published_error_notice = false;
        loop {
            tokio::select! {
                Some(event) = item_rx.recv() => {
                    if turn_event_is_error_notice(&event) {
                        published_error_notice = true;
                    }
                    mark_graphql_published_turn_event(&event, published_client_message_id.as_deref());
                    subscriptions.publish_conversation(ConversationRuntimeEvent::Turn {
                        client_message_id: published_client_message_id.clone(),
                        event: Box::new(event),
                    });
                }
                result = &mut completion => {
                    while let Ok(event) = item_rx.try_recv() {
                        if turn_event_is_error_notice(&event) {
                            published_error_notice = true;
                        }
                        mark_graphql_published_turn_event(&event, published_client_message_id.as_deref());
                        subscriptions.publish_conversation(ConversationRuntimeEvent::Turn {
                            client_message_id: published_client_message_id.clone(),
                            event: Box::new(event),
                        });
                    }
                    publish_turn_terminal_events(
                        &subscriptions,
                        conversation_id,
                        published_client_message_id,
                        published_error_notice,
                        result,
                    );
                    break;
                }
            }
        }
    });

    Ok(GraphqlTurnAccepted {
        conversation_id: input.conversation_id,
        client_message_id,
    })
}
