use serde::Deserialize;
use serde_json::Value;

use crate::{
    TurnActivityStatus, TurnTranscriptItem, {ConversationItemKind, ConversationItemRecord},
};

use super::DaemonError;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ConversationReplayItem {
    pub(crate) item_id: String,
    pub(crate) cursor: String,
    pub(crate) turn_id: Option<String>,
    pub(crate) metadata: Value,
    pub(crate) item: TurnTranscriptItem,
}

impl ConversationReplayItem {
    pub(crate) fn new(
        item_id: String,
        cursor: String,
        turn_id: Option<String>,
        metadata: Value,
        item: TurnTranscriptItem,
    ) -> Self {
        Self {
            item_id,
            cursor,
            turn_id,
            metadata,
            item,
        }
    }
}

pub(crate) fn web_conversation_item_from_record(
    record: ConversationItemRecord,
) -> Result<Option<ConversationReplayItem>, DaemonError> {
    let Some(item) = turn_transcript_item_from_record(&record)? else {
        return Ok(None);
    };
    Ok(Some(ConversationReplayItem::new(
        record.item_id,
        record.cursor,
        record.turn_id,
        record.metadata,
        item,
    )))
}

fn turn_transcript_item_from_record(
    record: &ConversationItemRecord,
) -> Result<Option<TurnTranscriptItem>, DaemonError> {
    match record.kind {
        ConversationItemKind::UserText => Ok(Some(TurnTranscriptItem::UserText {
            text: required_content_text(record)?,
        })),
        ConversationItemKind::AssistantText => Ok(Some(TurnTranscriptItem::AssistantText {
            text: required_content_text(record)?,
        })),
        ConversationItemKind::Activity => {
            let payload: ReplayActivityPayload = replay_payload(record)?;
            Ok(Some(TurnTranscriptItem::Activity {
                id: payload.id,
                activity_kind: payload.activity_kind,
                status: payload.status,
                title: payload.title,
                summary: payload.summary,
                metadata: payload.metadata,
            }))
        }
        ConversationItemKind::A2uiCard => {
            let payload: ReplayA2uiCardPayload = replay_payload(record)?;
            Ok(Some(TurnTranscriptItem::A2uiCard {
                id: payload.id,
                schema: payload.schema,
                payload: payload.payload,
            }))
        }
        ConversationItemKind::ErrorNotice => {
            let payload: ReplayErrorNoticePayload = replay_payload(record)?;
            Ok(Some(TurnTranscriptItem::ErrorNotice {
                message: payload
                    .message
                    .or_else(|| record.content_text.clone())
                    .ok_or_else(|| missing_replay_field(record, "message"))?,
                recoverable: payload.recoverable,
            }))
        }
        ConversationItemKind::Reasoning => Ok(None),
        ConversationItemKind::ToolCall
        | ConversationItemKind::ToolResult
        | ConversationItemKind::ApprovalRequest
        | ConversationItemKind::ApprovalResult => {
            let payload: ReplayActivityPayload = replay_payload(record)?;
            Ok(Some(TurnTranscriptItem::Activity {
                id: payload.id,
                activity_kind: payload.activity_kind,
                status: payload.status,
                title: payload.title,
                summary: payload.summary,
                metadata: payload.metadata,
            }))
        }
    }
}

fn replay_payload<T: serde::de::DeserializeOwned>(
    record: &ConversationItemRecord,
) -> Result<T, DaemonError> {
    serde_json::from_value(record.payload_json.clone()).map_err(|source| {
        DaemonError::Protocol(format!(
            "invalid replay payload for {} {}: {source}",
            record.kind.as_str(),
            record.item_id
        ))
    })
}

fn required_content_text(record: &ConversationItemRecord) -> Result<String, DaemonError> {
    record
        .content_text
        .clone()
        .ok_or_else(|| missing_replay_field(record, "content_text"))
}

fn missing_replay_field(record: &ConversationItemRecord, field: &str) -> DaemonError {
    DaemonError::Protocol(format!(
        "missing replay field {field} for {} {}",
        record.kind.as_str(),
        record.item_id
    ))
}

#[derive(Debug, Deserialize)]
struct ReplayActivityPayload {
    id: String,
    activity_kind: String,
    status: TurnActivityStatus,
    title: String,
    #[serde(default)]
    summary: Option<String>,
    #[serde(default)]
    metadata: Value,
}

#[derive(Debug, Deserialize)]
struct ReplayA2uiCardPayload {
    id: String,
    schema: String,
    payload: Value,
}

#[derive(Debug, Deserialize)]
struct ReplayErrorNoticePayload {
    #[serde(default)]
    message: Option<String>,
    #[serde(default)]
    recoverable: bool,
}
