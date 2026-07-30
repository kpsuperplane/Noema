use std::collections::BTreeMap;

use noema_conversations::{
    ActorRef, ConversationItemKind, ConversationItemStatus, NewConversationItem,
};
use noema_store::{ConversationInteractionKind, ConversationInteractionStatus};
use serde_json::{Value, json};
use tokio::sync::mpsc;

use super::actor::RuntimeActor;
use crate::{
    a2ui::A2UISurface,
    daemon::protocol::{RuntimeError, TurnStreamEvent},
};

const MAX_ACTION_JSON_BYTES: usize = 64 * 1024;

impl RuntimeActor {
    #[allow(clippy::too_many_arguments)]
    pub(in crate::daemon) async fn submit_a2ui_action(
        &mut self,
        conversation_id: String,
        interaction_id: String,
        expected_revision: u64,
        surface_id: String,
        source_component_id: String,
        action_name: String,
        context: Option<Value>,
        data_model: Option<Value>,
        item_tx: mpsc::UnboundedSender<TurnStreamEvent>,
        client_message_id: Option<String>,
    ) -> Result<(), RuntimeError> {
        let interaction = self
            .store
            .get_conversation_interaction(&interaction_id)
            .await?
            .ok_or_else(|| RuntimeError::Protocol("A2UI interaction is missing".to_string()))?;
        if interaction.kind != ConversationInteractionKind::A2UI
            || interaction.conversation_id != conversation_id
            || interaction.status != ConversationInteractionStatus::Pending
            || interaction.revision != expected_revision
        {
            return Err(RuntimeError::Protocol(
                "A2UI interaction is stale".to_string(),
            ));
        }
        bounded_object(context.as_ref(), "A2UI action context")?;
        if let Some(model) = data_model.as_ref() {
            bounded_json(model, "A2UI data model")?;
        }
        let mut surfaces: BTreeMap<String, A2UISurface> =
            serde_json::from_value(interaction.projection.get("surfaces").cloned().ok_or_else(
                || RuntimeError::Protocol("A2UI projection has no surfaces".to_string()),
            )?)
            .map_err(|error| RuntimeError::Protocol(format!("invalid A2UI projection: {error}")))?;
        let (namespaced_surface_id, surface) = surfaces
            .iter_mut()
            .find(|(_, surface)| surface.surface_id == surface_id)
            .ok_or_else(|| RuntimeError::Protocol("A2UI surface is stale".to_string()))?;
        let action = surface
            .actions
            .iter()
            .find(|action| {
                action.source_component_id == source_component_id && action.name == action_name
            })
            .ok_or_else(|| RuntimeError::Protocol("A2UI action is not available".to_string()))?;
        let declared_action_context = action.context.clone();
        if surface.send_data_model != data_model.is_some() {
            return Err(RuntimeError::Protocol(if surface.send_data_model {
                "A2UI action requires the synchronized data model".to_string()
            } else {
                "A2UI action does not accept a synchronized data model".to_string()
            }));
        }
        if let Some(model) = data_model.clone() {
            surface.data_model = model;
        }
        let action_context = declared_action_context.as_ref().map(|value| {
            Value::Object(
                value
                    .as_object()
                    .expect("validated A2UI action context is an object")
                    .iter()
                    .map(|(key, value)| (key.clone(), resolve_bindings(value, &surface.data_model)))
                    .collect(),
            )
        });
        if context != action_context {
            return Err(RuntimeError::Protocol(
                "A2UI action context does not match the declared action".to_string(),
            ));
        }
        surface.revision = surface.revision.saturating_add(1);
        let settled_snapshot = serde_json::to_value(&*surface)
            .map_err(|error| RuntimeError::Protocol(format!("invalid A2UI snapshot: {error}")))?;
        let mut projection = interaction.projection.clone();
        projection["surfaces"][namespaced_surface_id] = settled_snapshot;
        projection["interaction_revision"] = json!(expected_revision.saturating_add(1));
        projection["lifecycle"] = json!("answered");
        let settled_revision = expected_revision.saturating_add(1);
        let client_message_id = client_message_id
            .unwrap_or_else(|| format!("interaction:{interaction_id}:{settled_revision}"));
        let human_action = NewConversationItem {
            conversation_id: conversation_id.clone(),
            turn_id: Some(interaction.originating_turn_id.clone()),
            parent_item_id: Some(interaction.projection_item_id.clone()),
            kind: ConversationItemKind::A2UICard,
            status: ConversationItemStatus::Completed,
            author: ActorRef::human("human:local").expect("static local human actor id is valid"),
            content_text: None,
            payload_json: json!({
                "id": format!("a2ui:{}:{settled_revision}", interaction.originating_turn_id),
                "schema": "a2ui.v0.9.1",
                "payload": projection,
            }),
            metadata: json!({
                "source": "a2ui_action",
                "client_message_id": client_message_id,
            }),
        };
        let mut provider_payload = serde_json::Map::from_iter([
            ("status".to_string(), json!("resolved")),
            ("interaction_id".to_string(), json!(interaction_id)),
            ("surface_id".to_string(), json!(surface_id)),
            (
                "source_component_id".to_string(),
                json!(source_component_id),
            ),
            ("action_name".to_string(), json!(action_name)),
            ("context".to_string(), json!(action_context)),
        ]);
        if let Some(model) = data_model {
            provider_payload.insert("data_model".to_string(), model);
        }
        let provider_tool_result = NewConversationItem {
            conversation_id,
            turn_id: Some(interaction.originating_turn_id.clone()),
            parent_item_id: Some(interaction.provider_call_item_id.clone()),
            kind: ConversationItemKind::ToolResult,
            status: ConversationItemStatus::Completed,
            author: ActorRef::agent("agent:primary").expect("static primary agent id is valid"),
            content_text: None,
            payload_json: json!({
                "metadata": { "action": {
                    "id": interaction.provider_call_id,
                    "provider_call_id": interaction.provider_call_id,
                    "provider_name": interaction.provider_tool_name,
                    "name": interaction.canonical_tool_name,
                    "arguments": interaction.request,
                    "success": true,
                    "payload": Value::Object(provider_payload),
                }}
            }),
            metadata: json!({"source": "conversation_interaction"}),
        };
        let answered = self
            .store
            .resolve_conversation_interaction(
                &interaction_id,
                expected_revision,
                &client_message_id,
                human_action,
                provider_tool_result,
            )
            .await?;
        if let Some(item_id) = answered.resolution_item_id.as_deref() {
            self.emit_projection_item(&answered.conversation_id, item_id, &item_tx)
                .await?;
        }
        self.resume_conversation_interaction(answered, &item_tx)
            .await
    }
}

fn bounded_object(value: Option<&Value>, label: &str) -> Result<(), RuntimeError> {
    if value.is_some_and(|value| !value.is_object()) {
        return Err(RuntimeError::Protocol(format!("{label} must be an object")));
    }
    value.map_or(Ok(()), |value| bounded_json(value, label))
}

fn bounded_json(value: &Value, label: &str) -> Result<(), RuntimeError> {
    if serde_json::to_vec(value).map_or(usize::MAX, |bytes| bytes.len()) > MAX_ACTION_JSON_BYTES {
        Err(RuntimeError::Protocol(format!("{label} is too large")))
    } else {
        Ok(())
    }
}

fn resolve_bindings(value: &Value, data_model: &Value) -> Value {
    match value {
        Value::Object(object)
            if object.len() == 1 && object.get("path").is_some_and(Value::is_string) =>
        {
            object
                .get("path")
                .and_then(Value::as_str)
                .and_then(|path| data_model.pointer(if path == "/" { "" } else { path }))
                .cloned()
                .unwrap_or(Value::Null)
        }
        Value::Object(object) => Value::Object(
            object
                .iter()
                .map(|(key, value)| (key.clone(), resolve_bindings(value, data_model)))
                .collect(),
        ),
        Value::Array(values) => Value::Array(
            values
                .iter()
                .map(|value| resolve_bindings(value, data_model))
                .collect(),
        ),
        _ => value.clone(),
    }
}
