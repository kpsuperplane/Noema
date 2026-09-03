//! Focused replay projection tests retained at the GraphQL boundary.

use super::*;
use noema_conversations::{ConversationItemKind, ConversationItemRecord, ConversationItemStatus};
use noema_runtime::{TurnActivityStatus, TurnTranscriptItem};
use serde_json::{Value, json};

fn replay_record(
    kind: ConversationItemKind,
    content_text: Option<&str>,
    payload_json: Value,
) -> ConversationItemRecord {
    ConversationItemRecord {
        item_id: "item_1".to_string(),
        conversation_id: "conversation_1".to_string(),
        turn_id: Some("turn_1".to_string()),
        sequence_index: 1,
        cursor: "conversation_item:1".to_string(),
        kind,
        status: ConversationItemStatus::Completed,
        content_text: content_text.map(str::to_string),
        provider_content_text: None,
        payload_json,
        metadata: json!({"boundary": "preserved"}),
        created_at: String::new(),
    }
}

#[test]
fn conversation_replay_projects_representative_transcript_items() {
    let assistant = web_conversation_item_from_record(replay_record(
        ConversationItemKind::AssistantText,
        Some("hello from replay"),
        json!({}),
    ))
    .expect("convert assistant")
    .expect("visible assistant");
    assert_eq!(assistant.metadata["boundary"], "preserved");
    assert_eq!(
        assistant.item,
        TurnTranscriptItem::AssistantText {
            text: "hello from replay".to_string(),
        }
    );

    let choice = web_conversation_item_from_record(replay_record(
        ConversationItemKind::MultipleChoicePrompt,
        Some("Pick a direction"),
        json!({
            "prompt": "Pick a direction",
            "selection_mode": "pick_one",
            "options": [{"id": "ship", "label": "Ship it"}],
        }),
    ))
    .expect("convert choice")
    .expect("visible choice");
    assert!(matches!(
        choice.item,
        TurnTranscriptItem::MultipleChoicePrompt { ref options, .. }
            if options.len() == 1 && options[0].id == "ship"
    ));

    let mut action_record = replay_record(
        ConversationItemKind::ToolCall,
        Some("Tool call: search_memory"),
        json!({
            "id": "tool_call:conversation_1:0:1",
            "activity_kind": "tool_call",
            "status": "running",
            "title": "Tool call: search_memory",
            "summary": "provider id call_1",
            "metadata": {"action": {"name": "search_memory"}},
        }),
    );
    action_record.status = ConversationItemStatus::Cancelled;
    let action = web_conversation_item_from_record(action_record)
        .expect("convert action")
        .expect("visible action");
    assert!(matches!(
        action.item,
        TurnTranscriptItem::Activity {
            ref activity_kind,
            status: TurnActivityStatus::Failed,
            ref metadata,
            ..
        } if activity_kind == "tool_call" && metadata["display"]["marker"]["status"] == "cancelled"
    ));

    let artifact = web_conversation_item_from_record(replay_record(
        ConversationItemKind::ArtifactReference,
        None,
        json!({
            "artifact_id": "artifact_1",
            "artifact_version_id": "artifact_version_1",
            "title": "Noema notes",
            "artifact_kind": "document",
            "storage_kind": "external_url",
            "external_url": "https://example.com/notes",
            "download_url": null,
            "media_type": "text/html"
        }),
    ))
    .expect("convert artifact")
    .expect("visible artifact");
    assert!(matches!(
        artifact.item,
        TurnTranscriptItem::ArtifactReference { ref artifact_id, .. }
            if artifact_id == "artifact_1"
    ));
}

#[test]
fn conversation_replay_rejects_malformed_activity_payload() {
    let error = web_conversation_item_from_record(replay_record(
        ConversationItemKind::Activity,
        None,
        json!({"not": "an activity payload"}),
    ))
    .expect_err("malformed record should fail");

    assert!(error.to_string().contains("invalid replay payload"));
}

#[test]
fn conversation_replay_omits_hidden_tool_activity() {
    for name in [
        "enable.calendar.move_event",
        "task.delegate",
        "web.browse.close",
    ] {
        for (kind, activity_kind) in [
            (ConversationItemKind::ToolCall, "tool_call"),
            (ConversationItemKind::ToolResult, "tool_result"),
        ] {
            let item = web_conversation_item_from_record(replay_record(
                kind,
                None,
                json!({
                    "id": format!("{activity_kind}:{name}"),
                    "activity_kind": activity_kind,
                    "title": "Hidden tool activity",
                    "metadata": {"action": {"name": name}},
                }),
            ))
            .expect("convert hidden activity");

            assert!(
                item.is_none(),
                "{name} {activity_kind} must stay out of chat"
            );
        }
    }
}
