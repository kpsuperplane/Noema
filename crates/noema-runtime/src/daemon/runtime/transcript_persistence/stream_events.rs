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
        | TurnTranscriptItem::MultipleChoicePrompt { .. }
        | TurnTranscriptItem::MultipleChoiceSelection { .. }
        | TurnTranscriptItem::ErrorNotice { .. }
        | TurnTranscriptItem::ArtifactReference { .. }
        | TurnTranscriptItem::TaskReference { .. } => format!(
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
            &assistant_response_stream_id(stream_id, output_index_base + response_index),
            response_index,
            delta,
        ),
        GenerateStreamEvent::ToolCallStarted { output_index, name } => {
            send_tool_call_started_transient(
                context,
                item_tx,
                output_index_base + output_index,
                &name,
                None,
            );
        }
        GenerateStreamEvent::HostedWebSearchStarted { output_index, id } => {
            let output_index = output_index_base + output_index;
            let correlation_id = hosted_web_search_correlation_id(context, output_index, id.as_deref());
            send_tool_call_started_transient(
                context,
                item_tx,
                output_index,
                "web.search",
                Some(&correlation_id),
            );
        }
    }
}

fn hosted_web_search_correlation_id(
    context: &ConversationMemoryContext,
    output_index: usize,
    provider_id: Option<&str>,
) -> String {
    provider_id.map_or_else(
        || format!("hosted_web_search:{}:{}:{}", context.conversation_id, context.turn_index, output_index),
        ToString::to_string,
    )
}

fn send_tool_call_started_transient(
    context: &ConversationMemoryContext,
    item_tx: &mpsc::UnboundedSender<TurnStreamEvent>,
    output_index: usize,
    name: &str,
    correlation_id: Option<&str>,
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
                "id": correlation_id,
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
