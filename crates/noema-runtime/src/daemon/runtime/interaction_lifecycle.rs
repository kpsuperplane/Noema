//! Runtime-owned interception for provider-native human presentation tools.
//!
//! Durable publication lives in `noema-store`; this module only decides when
//! a presentation call must pause, builds the transcript projections, and
//! turns invalid A2UI into an ordinary repair result for the next provider
//! round.

use noema_conversations::{
    ActorRef, ConversationItemKind, ConversationItemRecord, ConversationItemStatus,
    NewConversationItem, ReplayMode,
};
use noema_providers::ReasoningEffort;
use noema_store::{ConversationInteractionKind, NewConversationInteraction};
use serde_json::{Value, json};
use tokio::sync::mpsc;

use super::{
    actor::RuntimeActor,
    local_tool_results::{LocalToolKind, LocalToolResult},
    presentation_tools::{
        PRESENT_A2UI_TOOL, PRESENT_MULTIPLE_CHOICE_TOOL, PresentMultipleChoiceArguments,
        parse_a2ui_payload, parse_multiple_choice_payload,
    },
    tool_lifecycle::LocalToolCall,
    turn::SuccessfulProviderTurn,
};
use crate::{
    a2ui::{A2UIValidatedBatch, parse_and_reduce},
    daemon::{
        protocol::{RuntimeError, TurnActivityStatus, TurnStreamEvent, TurnTranscriptItem},
        runtime::transcript_persistence::send_conversation_item,
    },
};

/// A validated presentation that must suspend the originating turn.
#[derive(Debug, Clone)]
pub(super) enum PendingPresentation {
    MultipleChoice(PresentMultipleChoiceArguments),
    A2UI(A2UIValidatedBatch),
}

impl RuntimeActor {
    /// Return a validated presentation only when it needs a durable pause.
    /// Invalid and action-free A2UI deliberately falls through to ordinary
    /// local-tool handling so it can return a repair result or continue.
    pub(super) fn pending_presentation(
        conversation_id: &str,
        call: &LocalToolCall,
    ) -> Result<Option<PendingPresentation>, RuntimeError> {
        match call.name.as_str() {
            PRESENT_MULTIPLE_CHOICE_TOOL => Ok(parse_multiple_choice_payload(&call.payload)
                .ok()
                .map(PendingPresentation::MultipleChoice)),
            PRESENT_A2UI_TOOL => {
                let Ok(arguments) = parse_a2ui_payload(&call.payload) else {
                    return Ok(None);
                };
                let Ok(batch) = parse_and_reduce(conversation_id, &arguments.jsonl) else {
                    return Ok(None);
                };
                if batch
                    .surfaces
                    .values()
                    .any(|surface| !surface.actions.is_empty())
                {
                    Ok(Some(PendingPresentation::A2UI(batch)))
                } else {
                    Ok(None)
                }
            }
            _ => Ok(None),
        }
    }

    /// Publish a validated interaction atomically with its provider call and
    /// projection, then return a blocked local result without a tool result.
    pub(super) async fn publish_pending_presentation(
        &self,
        turn: &SuccessfulProviderTurn,
        call: &LocalToolCall,
        output_index: usize,
        display_description: Option<&str>,
        presentation: PendingPresentation,
        item_tx: &mpsc::UnboundedSender<TurnStreamEvent>,
    ) -> Result<LocalToolResult, RuntimeError> {
        let provider_call_id = provider_call_id(call, &turn.turn_id);
        let interaction_id = format!(
            "interaction:{}:{}:{}",
            turn.conversation_id, turn.turn_id, provider_call_id
        );
        let (kind, projection_kind, mut projection, result_payload) = match presentation {
            PendingPresentation::MultipleChoice(arguments) => {
                let projection = json!({
                    "prompt": arguments.prompt,
                    "selection_mode": arguments.selection_mode,
                    "options": arguments.options,
                });
                (
                    ConversationInteractionKind::MultipleChoice,
                    ConversationItemKind::MultipleChoicePrompt,
                    projection.clone(),
                    json!({"status": "pending", "interaction_id": interaction_id}),
                )
            }
            PendingPresentation::A2UI(batch) => {
                let projection = a2ui_projection_payload(&batch);
                (
                    ConversationInteractionKind::A2UI,
                    ConversationItemKind::A2UICard,
                    projection.clone(),
                    json!({"status": "pending", "interaction_id": interaction_id}),
                )
            }
        };
        projection["interaction_id"] = json!(interaction_id);
        projection["interaction_revision"] = json!(1);
        projection["lifecycle"] = json!("pending");
        let provider_tool_call = provider_call_item(
            turn,
            call,
            output_index,
            display_description,
            ConversationItemStatus::Running,
        );
        let projection_item = projection_item(turn, call, projection_kind, projection.clone());
        let selection = turn.provider_route.selection();
        let credential_revision = self
            .store
            .get_provider_account(&selection.provider_account_id)
            .await?
            .and_then(|account| {
                account
                    .metadata
                    .get("credentialRevision")
                    .and_then(Value::as_u64)
            })
            .unwrap_or(0);
        let model = selection
            .model_profile
            .clone()
            .unwrap_or_else(|| turn.response.model.clone());
        let record = self
            .store
            .publish_conversation_interaction(
                NewConversationInteraction {
                    interaction_id: interaction_id.clone(),
                    conversation_id: turn.conversation_id.clone(),
                    originating_turn_id: turn.turn_id.clone(),
                    kind,
                    provider_call_id,
                    canonical_tool_name: call.name.clone(),
                    provider_tool_name: call
                        .provider_name
                        .clone()
                        .unwrap_or_else(|| call.name.clone()),
                    provider_kind: selection.provider_kind.clone(),
                    provider_account_id: selection.provider_account_id.clone(),
                    provider_instance_key: turn.provider_route.key().as_str().to_string(),
                    selection_mode: selection.selection_mode.as_str().to_string(),
                    credential_revision,
                    model,
                    reasoning_effort: selection
                        .reasoning_effort
                        .map(ReasoningEffort::as_persistence_str)
                        .map(str::to_string),
                    tool_catalog_digest: tool_catalog_digest(turn),
                    request: call.payload.clone(),
                    projection,
                },
                provider_tool_call,
                projection_item,
            )
            .await?;
        self.emit_published_items(&turn.conversation_id, &record, item_tx)
            .await?;
        Ok(
            LocalToolResult::from_call(call, LocalToolKind::Gateway, true, result_payload, false)
                .with_pending_interaction(interaction_id),
        )
    }

    /// Persist a valid action-free A2UI batch as a normal projection. The
    /// provider call itself has already been persisted by the ordinary loop.
    pub(super) async fn persist_action_free_a2ui(
        &self,
        turn: &SuccessfulProviderTurn,
        call: &LocalToolCall,
        batch: &A2UIValidatedBatch,
    ) -> Result<Value, RuntimeError> {
        if batch.surfaces.is_empty() {
            return Ok(json!({"status": "published", "surface_count": 0}));
        }
        let projection = a2ui_projection_payload(batch);
        let record = self
            .store
            .append_conversation_item(projection_item(
                turn,
                call,
                ConversationItemKind::A2UICard,
                projection,
            ))
            .await?;
        Ok(json!({
            "status": "published",
            "projection_item_id": record.item_id,
            "surface_count": batch.surfaces.len(),
        }))
    }

    pub(super) async fn emit_projection_item(
        &self,
        conversation_id: &str,
        item_id: &str,
        item_tx: &mpsc::UnboundedSender<TurnStreamEvent>,
    ) -> Result<(), RuntimeError> {
        let record = self
            .store
            .list_conversation_items(conversation_id, ReplayMode::Visible)
            .await?
            .into_iter()
            .find(|item| item.item_id == item_id);
        if let Some(record) = record
            && let Some(item) = transcript_item(&record)
        {
            send_conversation_item(item_tx, record, json!({}), item);
        }
        Ok(())
    }

    async fn emit_published_items(
        &self,
        conversation_id: &str,
        interaction: &noema_store::ConversationInteractionRecord,
        item_tx: &mpsc::UnboundedSender<TurnStreamEvent>,
    ) -> Result<(), RuntimeError> {
        let records = self
            .store
            .list_conversation_items(conversation_id, ReplayMode::Visible)
            .await?;
        for record in records.into_iter().filter(|record| {
            record.item_id == interaction.provider_call_item_id
                || record.item_id == interaction.projection_item_id
        }) {
            if let Some(item) = transcript_item(&record) {
                send_conversation_item(item_tx, record, json!({}), item);
            }
        }
        Ok(())
    }
}

fn provider_call_id(call: &LocalToolCall, turn_id: &str) -> String {
    call.provider_call_id
        .clone()
        .or_else(|| call.call_id.clone())
        .unwrap_or_else(|| format!("call:{turn_id}:{}", call.output_index))
}

fn tool_catalog_digest(turn: &SuccessfulProviderTurn) -> String {
    tool_catalog_digest_for(&turn.initial_model_tools)
}

pub(super) fn tool_catalog_digest_for(tools: &super::model_tools::ModelTools) -> String {
    let value = tools
        .provider_tools
        .iter()
        .map(|tool| tool.canonical_spec())
        .collect::<Vec<_>>();
    let bytes = serde_json::to_vec(&value).expect("provider tool specs serialize");
    ring::digest::digest(&ring::digest::SHA256, &bytes)
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn provider_call_item(
    turn: &SuccessfulProviderTurn,
    call: &LocalToolCall,
    output_index: usize,
    display_description: Option<&str>,
    status: ConversationItemStatus,
) -> NewConversationItem {
    let provider_call_id = provider_call_id(call, &turn.turn_id);
    let action = json!({
        "id": call.call_id,
        "provider_call_id": provider_call_id,
        "provider_name": call.provider_name,
        "name": call.name,
        "payload": call.payload,
    });
    let display = json!({
        "name": call.name,
        "access": "Waiting for you",
        "description": display_description,
    });
    let payload = json!({
        "id": format!("tool_call:{}:{}", turn.turn_id, output_index),
        "activity_kind": "tool_call",
        "status": if status == ConversationItemStatus::Running { "running" } else { "completed" },
        "title": format!("Tool call: {}", call.name),
        "summary": display.get("name"),
        "metadata": {
            "turn_index": turn.turn_index,
            "output_index": output_index,
            "source": "provider_action",
            "provider": turn.response.provider,
            "action": action,
            "display": display,
        },
    });
    NewConversationItem {
        conversation_id: turn.conversation_id.clone(),
        turn_id: Some(turn.turn_id.clone()),
        parent_item_id: Some(turn.user_item_id.clone()),
        kind: ConversationItemKind::ToolCall,
        status,
        author: ActorRef::agent("agent:primary").expect("static primary agent id is valid"),
        content_text: Some(format!("Tool call: {}", call.name)),
        payload_json: payload,
        metadata: json!({
            "turn_index": turn.turn_index,
            "output_index": output_index,
            "source": "provider_action",
            "provider": turn.response.provider,
        }),
    }
}

fn projection_item(
    turn: &SuccessfulProviderTurn,
    call: &LocalToolCall,
    kind: ConversationItemKind,
    projection: Value,
) -> NewConversationItem {
    let is_multiple_choice = kind == ConversationItemKind::MultipleChoicePrompt;
    let payload = if is_multiple_choice {
        projection
    } else {
        json!({
            "id": format!("a2ui:{}:{}", turn.turn_id, call.output_index),
            "schema": "a2ui.v0.9.1",
            "payload": projection,
        })
    };
    NewConversationItem {
        conversation_id: turn.conversation_id.clone(),
        turn_id: Some(turn.turn_id.clone()),
        parent_item_id: Some(turn.user_item_id.clone()),
        kind,
        status: ConversationItemStatus::Completed,
        author: ActorRef::agent("agent:primary").expect("static primary agent id is valid"),
        content_text: is_multiple_choice.then(|| {
            payload
                .get("prompt")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string()
        }),
        payload_json: payload,
        metadata: json!({
            "turn_index": turn.turn_index,
            "output_index": call.output_index,
            "source": "provider_presentation",
        }),
    }
}

pub(super) fn a2ui_projection_payload(batch: &A2UIValidatedBatch) -> Value {
    json!({
        "protocol_version": "v0.9.1",
        "catalog": crate::a2ui::advertised_catalog(),
        "messages": batch.messages,
        "surfaces": batch.surfaces,
        "deleted_surface_ids": batch.deleted_surface_ids,
    })
}

fn transcript_item(record: &ConversationItemRecord) -> Option<TurnTranscriptItem> {
    match record.kind {
        ConversationItemKind::ToolCall => Some(TurnTranscriptItem::Activity {
            id: record.payload_json.get("id")?.as_str()?.to_string(),
            activity_kind: "tool_call".to_string(),
            status: TurnActivityStatus::Started,
            title: record.payload_json.get("title")?.as_str()?.to_string(),
            summary: record
                .payload_json
                .get("summary")
                .and_then(Value::as_str)
                .map(str::to_string),
            metadata: record.payload_json.get("metadata")?.clone(),
        }),
        ConversationItemKind::MultipleChoicePrompt => {
            Some(TurnTranscriptItem::MultipleChoicePrompt {
                prompt: record.payload_json.get("prompt")?.as_str()?.to_string(),
                selection_mode: serde_json::from_value(
                    record.payload_json.get("selection_mode")?.clone(),
                )
                .ok()?,
                options: serde_json::from_value(record.payload_json.get("options")?.clone())
                    .ok()?,
            })
        }
        ConversationItemKind::MultipleChoiceSelection => {
            Some(TurnTranscriptItem::MultipleChoiceSelection {
                prompt_item_id: record
                    .payload_json
                    .get("prompt_item_id")?
                    .as_str()?
                    .to_string(),
                selection_mode: serde_json::from_value(
                    record.payload_json.get("selection_mode")?.clone(),
                )
                .ok()?,
                selected_options: serde_json::from_value(
                    record.payload_json.get("selected_options")?.clone(),
                )
                .ok()?,
            })
        }
        ConversationItemKind::A2UICard => a2ui_transcript_item(record),
        _ => None,
    }
}

pub(super) fn a2ui_transcript_item(record: &ConversationItemRecord) -> Option<TurnTranscriptItem> {
    let projection = record.payload_json.get("payload")?;
    let snapshot = projection
        .get("surfaces")?
        .as_object()?
        .values()
        .next()?
        .clone();
    let surface: crate::a2ui::A2UISurface = serde_json::from_value(snapshot.clone()).ok()?;
    Some(TurnTranscriptItem::A2UISurface {
        id: record.payload_json.get("id")?.as_str()?.to_string(),
        interaction_id: projection
            .get("interaction_id")
            .and_then(Value::as_str)
            .map(str::to_string),
        surface_id: surface.surface_id,
        version: surface.version,
        revision: surface.revision,
        interaction_revision: projection
            .get("interaction_revision")
            .and_then(Value::as_u64),
        lifecycle: projection
            .get("lifecycle")
            .and_then(Value::as_str)
            .unwrap_or("completed")
            .to_string(),
        catalog: projection.get("catalog")?.clone(),
        has_actions: !surface.actions.is_empty(),
        snapshot,
    })
}
