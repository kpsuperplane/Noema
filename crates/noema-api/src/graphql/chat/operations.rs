use super::events::{
    mark_graphql_published_turn_event, publish_turn_terminal_events, turn_event_is_error_notice,
};
use super::*;

pub(in crate::graphql) async fn primary_conversation(
    state: &GraphqlState,
    human_id: &str,
) -> Result<Option<GraphqlPrimaryConversation>> {
    let store = state.store()?;
    let provider_kind = primary_agent_provider_kind(store).await?;
    let conversation = store
        .primary_conversation_for_human(human_id)
        .await
        .map_err(graphql_error)?;
    Ok(conversation.map(|conversation| GraphqlPrimaryConversation {
        conversation_id: conversation.conversation_id,
        provider: provider_kind,
    }))
}

pub(in crate::graphql) async fn ensure_primary_conversation(
    state: &GraphqlState,
    human_id: &str,
    cwd: Option<String>,
) -> Result<GraphqlPrimaryConversation> {
    require_local_human(human_id)?;
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
    human_id: &str,
    input: GraphqlConversationTranscriptPageInput,
) -> Result<GraphqlConversationTranscriptPage> {
    visible_conversation_transcript_page(
        state,
        human_id,
        &input.conversation_id,
        input.cursor.as_deref(),
        input.limit,
    )
    .await
}

pub(in crate::graphql) async fn latest_conversation_transcript_page(
    state: &GraphqlState,
    human_id: &str,
    conversation_id: &str,
    limit: Option<i32>,
) -> Result<GraphqlConversationTranscriptPage> {
    visible_conversation_transcript_page(state, human_id, conversation_id, None, limit).await
}

async fn visible_conversation_transcript_page(
    state: &GraphqlState,
    human_id: &str,
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

    let store = state.store()?;
    require_conversation_owner(store, conversation_id, human_id).await?;
    let page = store
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
    human_id: &str,
    input: GraphqlSendConversationTurnInput,
) -> Result<GraphqlTurnAccepted> {
    require_conversation_owner(state.store()?, &input.conversation_id, human_id).await?;
    let runtime = state.runtime()?.clone();
    let subscriptions = state.subscriptions().clone();
    let (item_tx, item_rx) = tokio::sync::mpsc::unbounded_channel();
    let conversation_id = input.conversation_id.clone();
    let client_message_id = input.client_message_id.clone();
    let published_client_message_id = client_message_id.clone();
    let client_time_zone = input.client_time_zone.clone();
    if let Some(time_zone) = client_time_zone.as_deref() {
        noema_tasks::recurrence_preview("0 0 * * *", time_zone, 0).map_err(|_| {
            async_graphql::Error::new("clientTimeZone must be a valid IANA timezone")
        })?;
    }
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

    let completion = async move {
        runtime
            .turn_with_client_timezone(
                completion_conversation_id,
                input_text,
                item_tx,
                published_client_message_id.clone(),
                client_time_zone,
            )
            .await
    };
    spawn_runtime_turn(
        subscriptions,
        conversation_id,
        client_message_id.clone(),
        item_rx,
        completion,
    );

    Ok(GraphqlTurnAccepted {
        conversation_id: input.conversation_id,
        client_message_id,
    })
}

pub(in crate::graphql) async fn send_multiple_choice_selection(
    state: &GraphqlState,
    human_id: &str,
    input: GraphqlSendMultipleChoiceSelectionInput,
) -> Result<GraphqlTurnAccepted> {
    require_conversation_owner(state.store()?, &input.conversation_id, human_id).await?;
    let runtime = state.runtime()?.clone();
    let subscriptions = state.subscriptions().clone();
    let (item_tx, item_rx) = tokio::sync::mpsc::unbounded_channel();
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

    let completion = async move {
        runtime
            .select_multiple_choice_with_client_message_id(
                completion_conversation_id,
                prompt_item_id,
                selected_option_ids,
                item_tx,
                published_client_message_id.clone(),
            )
            .await
    };
    spawn_runtime_turn(
        subscriptions,
        conversation_id,
        client_message_id.clone(),
        item_rx,
        completion,
    );

    Ok(GraphqlTurnAccepted {
        conversation_id: input.conversation_id,
        client_message_id,
    })
}

pub(in crate::graphql) async fn send_a2ui_action(
    state: &GraphqlState,
    human_id: &str,
    input: GraphqlSendA2UIActionInput,
) -> Result<GraphqlTurnAccepted> {
    require_conversation_owner(state.store()?, &input.conversation_id, human_id).await?;
    let runtime = state.runtime()?.clone();
    let subscriptions = state.subscriptions().clone();
    let (item_tx, item_rx) = tokio::sync::mpsc::unbounded_channel();
    let conversation_id = input.conversation_id.clone();
    let client_message_id = input.client_message_id.clone();
    let completion_conversation_id = conversation_id.clone();
    let published_client_message_id = client_message_id.clone();
    mark_turn_timing_event(
        "graphql_a2ui_action_received",
        &conversation_id,
        client_message_id.as_deref(),
        serde_json::json!({
            "interaction_id": &input.interaction_id,
            "expected_revision": input.expected_revision,
            "surface_id": &input.surface_id,
            "source_component_id": &input.source_component_id,
            "action_name": &input.action_name,
        }),
    );
    let completion = async move {
        runtime
            .submit_a2ui_action_with_client_message_id(
                completion_conversation_id,
                input.interaction_id,
                input.expected_revision,
                input.surface_id,
                input.source_component_id,
                input.action_name,
                input.context.map(|value| value.0),
                input.data_model.map(|value| value.0),
                item_tx,
                published_client_message_id,
            )
            .await
    };
    spawn_runtime_turn(
        subscriptions,
        conversation_id,
        client_message_id.clone(),
        item_rx,
        completion,
    );
    Ok(GraphqlTurnAccepted {
        conversation_id: input.conversation_id,
        client_message_id,
    })
}

fn spawn_runtime_turn<F>(
    subscriptions: RuntimeEventRegistry,
    conversation_id: String,
    client_message_id: Option<String>,
    mut item_rx: tokio::sync::mpsc::UnboundedReceiver<TurnStreamEvent>,
    completion: F,
) where
    F: std::future::Future<Output = std::result::Result<(), noema_runtime::RuntimeError>>
        + Send
        + 'static,
{
    tokio::spawn(async move {
        mark_turn_timing_event(
            "graphql_runtime_task_started",
            &conversation_id,
            client_message_id.as_deref(),
            serde_json::json!({}),
        );
        tokio::pin!(completion);
        let mut published_error_notice = false;
        loop {
            tokio::select! {
                Some(event) = item_rx.recv() => {
                    if turn_event_is_error_notice(&event) {
                        published_error_notice = true;
                    }
                    mark_graphql_published_turn_event(&event, client_message_id.as_deref());
                    subscriptions.publish_conversation(ConversationRuntimeEvent::Turn {
                        client_message_id: client_message_id.clone(),
                        event: Box::new(event),
                    });
                }
                result = &mut completion => {
                    while let Ok(event) = item_rx.try_recv() {
                        if turn_event_is_error_notice(&event) {
                            published_error_notice = true;
                        }
                        mark_graphql_published_turn_event(&event, client_message_id.as_deref());
                        subscriptions.publish_conversation(ConversationRuntimeEvent::Turn {
                            client_message_id: client_message_id.clone(),
                            event: Box::new(event),
                        });
                    }
                    publish_turn_terminal_events(
                        &subscriptions,
                        conversation_id,
                        client_message_id,
                        published_error_notice,
                        result,
                    );
                    break;
                }
            }
        }
    });
}

pub(super) async fn require_conversation_owner(
    store: &noema_store::NoemaStore,
    conversation_id: &str,
    human_id: &str,
) -> Result<()> {
    if store
        .conversation_is_owned_by_human(conversation_id, human_id)
        .await
        .map_err(graphql_error)?
    {
        Ok(())
    } else {
        Err(async_graphql::Error::new("conversation is unavailable"))
    }
}

fn require_local_human(human_id: &str) -> Result<()> {
    if human_id == "human:local" {
        Ok(())
    } else {
        Err(async_graphql::Error::new("conversation is unavailable"))
    }
}
