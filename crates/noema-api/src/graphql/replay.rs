use noema_conversations::{ConversationItemKind, ConversationItemRecord};
use serde::Deserialize;
use serde_json::Value;

use noema_runtime::{TurnActivityStatus, TurnTranscriptItem};

use noema_runtime::RuntimeError;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ConversationReplayItem {
    pub(crate) item_id: String,
    pub(crate) cursor: String,
    pub(crate) turn_id: Option<String>,
    pub(crate) metadata: Value,
    pub(crate) item: TurnTranscriptItem,
}

pub(crate) fn web_conversation_item_from_record(
    record: ConversationItemRecord,
) -> Result<Option<ConversationReplayItem>, RuntimeError> {
    let Some(item) = turn_transcript_item_from_record(&record)? else {
        return Ok(None);
    };
    Ok(Some(ConversationReplayItem {
        item_id: record.item_id,
        cursor: record.cursor,
        turn_id: record.turn_id,
        metadata: record.metadata,
        item,
    }))
}

fn turn_transcript_item_from_record(
    record: &ConversationItemRecord,
) -> Result<Option<TurnTranscriptItem>, RuntimeError> {
    match record.kind {
        ConversationItemKind::UserText => Ok(Some(TurnTranscriptItem::UserText {
            text: required_content_text(record)?,
        })),
        ConversationItemKind::AssistantText => Ok(Some(TurnTranscriptItem::AssistantText {
            text: required_content_text(record)?,
        })),
        ConversationItemKind::Activity
        | ConversationItemKind::ToolCall
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
        ConversationItemKind::A2uiCard => {
            let payload: ReplayA2uiCardPayload = replay_payload(record)?;
            Ok(Some(TurnTranscriptItem::A2uiCard {
                id: payload.id,
                schema: payload.schema,
                payload: payload.payload,
            }))
        }
        ConversationItemKind::MultipleChoicePrompt => {
            let payload: ReplayMultipleChoicePromptPayload = replay_payload(record)?;
            Ok(Some(TurnTranscriptItem::MultipleChoicePrompt {
                prompt: payload.prompt,
                selection_mode: payload.selection_mode,
                options: payload.options,
            }))
        }
        ConversationItemKind::MultipleChoiceSelection => {
            let payload: ReplayMultipleChoiceSelectionPayload = replay_payload(record)?;
            Ok(Some(TurnTranscriptItem::MultipleChoiceSelection {
                prompt_item_id: payload.prompt_item_id,
                selection_mode: payload.selection_mode,
                selected_options: payload.selected_options,
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
        ConversationItemKind::Reasoning | ConversationItemKind::ModelContextUpdate => Ok(None),
        ConversationItemKind::ArtifactReference => {
            let payload: ReplayArtifactReferencePayload = replay_payload(record)?;
            Ok(Some(TurnTranscriptItem::ArtifactReference {
                artifact_id: payload.artifact_id,
                artifact_version_id: payload.artifact_version_id,
                title: payload.title,
                artifact_kind: payload.artifact_kind,
                storage_kind: payload.storage_kind,
                external_url: payload.external_url,
                download_url: payload.download_url,
                media_type: payload.media_type,
            }))
        }
        ConversationItemKind::TaskReference => {
            let payload: ReplayTaskReferencePayload = replay_payload(record)?;
            let status = payload
                .status
                .parse::<noema_tasks::TaskStatus>()
                .map_err(|error| {
                    RuntimeError::Protocol(format!(
                        "invalid task reference status for {}: {error}",
                        payload.task_id
                    ))
                })?
                .as_str()
                .to_string();
            Ok(Some(TurnTranscriptItem::TaskReference {
                task_id: payload.task_id,
                title: payload.title,
                status,
                revision: payload.revision,
            }))
        }
    }
}

fn replay_payload<T: serde::de::DeserializeOwned>(
    record: &ConversationItemRecord,
) -> Result<T, RuntimeError> {
    serde_json::from_value(record.payload_json.clone()).map_err(|source| {
        RuntimeError::Protocol(format!(
            "invalid replay payload for {} {}: {source}",
            record.kind.as_str(),
            record.item_id
        ))
    })
}

fn required_content_text(record: &ConversationItemRecord) -> Result<String, RuntimeError> {
    record
        .content_text
        .clone()
        .ok_or_else(|| missing_replay_field(record, "content_text"))
}

fn missing_replay_field(record: &ConversationItemRecord, field: &str) -> RuntimeError {
    RuntimeError::Protocol(format!(
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
struct ReplayMultipleChoicePromptPayload {
    prompt: String,
    selection_mode: noema_providers::MultipleChoiceSelectionMode,
    options: Vec<noema_providers::MultipleChoiceOption>,
}

#[derive(Debug, Deserialize)]
struct ReplayMultipleChoiceSelectionPayload {
    prompt_item_id: String,
    selection_mode: noema_providers::MultipleChoiceSelectionMode,
    selected_options: Vec<noema_providers::MultipleChoiceOption>,
}

#[derive(Debug, Deserialize)]
struct ReplayErrorNoticePayload {
    #[serde(default)]
    message: Option<String>,
    #[serde(default)]
    recoverable: bool,
}

#[derive(Debug, Deserialize)]
struct ReplayArtifactReferencePayload {
    artifact_id: String,
    #[serde(default)]
    artifact_version_id: Option<String>,
    title: String,
    artifact_kind: String,
    storage_kind: String,
    #[serde(default)]
    external_url: Option<String>,
    #[serde(default)]
    download_url: Option<String>,
    #[serde(default)]
    media_type: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ReplayTaskReferencePayload {
    task_id: String,
    title: String,
    status: String,
    revision: i64,
}
