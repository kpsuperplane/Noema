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
        ConversationItemKind::A2UICard => {
            let payload: ReplayA2UISurfacePayload = replay_payload(record)?;
            if payload.schema != "a2ui.v0.9.1" {
                return Err(RuntimeError::Protocol(format!(
                    "unsupported A2UI schema for {}",
                    record.item_id
                )));
            }
            let Some(snapshot) = payload.payload.surfaces.into_values().next() else {
                return Ok(None);
            };
            let has_actions = !snapshot.actions.is_empty();
            Ok(Some(TurnTranscriptItem::A2UISurface {
                id: payload.id,
                interaction_id: payload.payload.interaction_id,
                surface_id: snapshot.surface_id.clone(),
                version: snapshot.version.clone(),
                revision: snapshot.revision,
                interaction_revision: payload.payload.interaction_revision,
                lifecycle: payload
                    .payload
                    .lifecycle
                    .unwrap_or_else(|| "completed".to_string()),
                catalog: payload.payload.catalog,
                snapshot: serde_json::to_value(snapshot).map_err(|source| {
                    RuntimeError::Protocol(format!(
                        "invalid A2UI surface projection for {}: {source}",
                        record.item_id
                    ))
                })?,
                has_actions,
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
            Ok(Some(TurnTranscriptItem::TaskReference {
                task_id: payload.task_id,
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
struct ReplayA2UISurfacePayload {
    id: String,
    schema: String,
    payload: ReplayA2UIProjection,
}

#[derive(Debug, Deserialize)]
struct ReplayA2UIProjection {
    catalog: Value,
    surfaces: std::collections::BTreeMap<String, noema_runtime::a2ui::A2UISurface>,
    #[serde(default)]
    interaction_id: Option<String>,
    #[serde(default)]
    interaction_revision: Option<u64>,
    #[serde(default)]
    lifecycle: Option<String>,
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
}
