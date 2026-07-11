use serde::Deserialize;
use serde_json::Value;

use crate::{
    TurnActivityStatus, TurnTranscriptItem, {ConversationItemKind, ConversationItemRecord},
};

use crate::DaemonError;

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
        ConversationItemKind::Reasoning => Ok(None),
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
                .parse::<crate::TaskStatus>()
                .map_err(|error| {
                    DaemonError::Protocol(format!(
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
struct ReplayMultipleChoicePromptPayload {
    prompt: String,
    selection_mode: crate::provider::MultipleChoiceSelectionMode,
    options: Vec<crate::provider::MultipleChoiceOption>,
}

#[derive(Debug, Deserialize)]
struct ReplayMultipleChoiceSelectionPayload {
    prompt_item_id: String,
    selection_mode: crate::provider::MultipleChoiceSelectionMode,
    selected_options: Vec<crate::provider::MultipleChoiceOption>,
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

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::web_conversation_item_from_record;
    use crate::{
        ActorRef, ArtifactOwnerRef, ArtifactSource, ArtifactStorageKind, ArtifactVersionStorage,
        ConversationItemKind, ConversationItemStatus, NewArtifact, NewArtifactVersion,
        NewConversationItem, NewConversationTurn, TurnTranscriptItem,
    };

    #[tokio::test]
    async fn conversation_replay_maps_artifact_reference() {
        let store = crate::store::tests::test_store().await;
        let conversation = store
            .create_conversation(crate::NewConversation::local_chat(None, None))
            .await
            .expect("conversation");
        let turn = store
            .create_conversation_turn(NewConversationTurn {
                conversation_id: conversation.conversation_id.clone(),
                trigger_item_id: None,
                metadata: json!({}),
            })
            .await
            .expect("turn");
        let artifact = store
            .create_artifact_with_initial_version(
                NewArtifact {
                    artifact_id: None,
                    owner: ArtifactOwnerRef::conversation(&conversation.conversation_id),
                    title: "Noema notes".to_string(),
                    description: Some("Shared notes".to_string()),
                    artifact_kind: "document".to_string(),
                    storage_kind: ArtifactStorageKind::ExternalUrl,
                    created_by_actor_id: "agent:primary".to_string(),
                    source: ArtifactSource {
                        conversation_id: Some(conversation.conversation_id.clone()),
                        turn_id: Some(turn.turn_id.clone()),
                        item_id: None,
                    },
                    metadata: json!({}),
                },
                NewArtifactVersion {
                    artifact_version_id: None,
                    title: None,
                    storage: ArtifactVersionStorage::ExternalUrl {
                        url: "https://notion.so/noema-notes".to_string(),
                    },
                    media_type: Some("text/html".to_string()),
                    byte_size: None,
                    content_sha256: None,
                    created_by_actor_id: "agent:primary".to_string(),
                    source: ArtifactSource {
                        conversation_id: Some(conversation.conversation_id.clone()),
                        turn_id: Some(turn.turn_id.clone()),
                        item_id: None,
                    },
                    metadata: json!({}),
                },
            )
            .await
            .expect("artifact");
        let record = store
            .append_conversation_item(NewConversationItem {
                conversation_id: conversation.conversation_id.clone(),
                turn_id: Some(turn.turn_id.clone()),
                parent_item_id: None,
                kind: ConversationItemKind::ArtifactReference,
                status: ConversationItemStatus::Completed,
                author: ActorRef::agent("agent:primary"),
                content_text: None,
                payload_json: json!({
                    "artifact_id": artifact.artifact.artifact_id,
                    "artifact_version_id": artifact.current_version.artifact_version_id,
                    "title": "Noema notes",
                    "artifact_kind": "document",
                    "storage_kind": "external_url",
                    "external_url": "https://notion.so/noema-notes",
                    "download_url": null,
                    "media_type": "text/html"
                }),
                metadata: json!({}),
            })
            .await
            .expect("conversation item");

        let item = web_conversation_item_from_record(record)
            .expect("convert record")
            .expect("visible item");

        assert_eq!(
            item.item,
            TurnTranscriptItem::ArtifactReference {
                artifact_id: artifact.artifact.artifact_id,
                artifact_version_id: Some(artifact.current_version.artifact_version_id),
                title: "Noema notes".to_string(),
                artifact_kind: "document".to_string(),
                storage_kind: "external_url".to_string(),
                external_url: Some("https://notion.so/noema-notes".to_string()),
                download_url: None,
                media_type: Some("text/html".to_string()),
            }
        );
    }

    #[tokio::test]
    async fn conversation_replay_maps_task_reference_with_canonical_status() {
        let store = crate::store::tests::test_store().await;
        let conversation = store
            .create_conversation(crate::NewConversation::local_chat(None, None))
            .await
            .expect("conversation");
        let turn = store
            .create_conversation_turn(NewConversationTurn {
                conversation_id: conversation.conversation_id.clone(),
                trigger_item_id: None,
                metadata: json!({}),
            })
            .await
            .expect("turn");
        let record = store
            .append_conversation_item(NewConversationItem {
                conversation_id: conversation.conversation_id,
                turn_id: Some(turn.turn_id),
                parent_item_id: None,
                kind: ConversationItemKind::TaskReference,
                status: ConversationItemStatus::Completed,
                author: ActorRef::agent("agent:primary"),
                content_text: Some("Research providers".to_string()),
                payload_json: json!({
                    "task_id": "task_1",
                    "title": "Research providers",
                    "status": "reviewing",
                    "revision": 2
                }),
                metadata: json!({}),
            })
            .await
            .expect("conversation item");

        let item = web_conversation_item_from_record(record)
            .expect("convert record")
            .expect("visible item");

        assert_eq!(
            item.item,
            TurnTranscriptItem::TaskReference {
                task_id: "task_1".to_string(),
                title: "Research providers".to_string(),
                status: "reviewing".to_string(),
                revision: 2,
            }
        );
    }
}
