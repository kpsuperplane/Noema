//! Durable append-only persistence for keyed model-context updates.

use serde_json::{Value, json};

use noema_conversations::{
    ActorRef, ConversationItemKind, ConversationItemRecord, ConversationItemStatus,
    NewConversationItem,
};

use noema_store::NoemaStore;

use crate::daemon::protocol::RuntimeError;

use super::model_context::{ModelContextSnapshot, ModelContextState, ModelContextUpdate};

const UPDATE_PAYLOAD_KEY: &str = "model_context_update";

pub(super) struct ModelContextSyncRequest<'a> {
    pub(super) store: &'a NoemaStore,
    pub(super) conversation_id: &'a str,
    pub(super) turn_id: &'a str,
    pub(super) provider_kind: &'a str,
    pub(super) model_profile: Option<&'a str>,
    pub(super) state: &'a ModelContextState,
}

/// Persist the smallest ordered set of developer-context messages needed to
/// bring durable replay to `state`.
pub(super) async fn sync_model_context(
    request: ModelContextSyncRequest<'_>,
) -> Result<Vec<ModelContextUpdate>, RuntimeError> {
    let reset_sequence = request
        .store
        .latest_context_reset_sequence(request.conversation_id)
        .await?;
    let active_summary = request
        .store
        .latest_active_context_summary_after_sequence(
            request.conversation_id,
            request.provider_kind,
            request.model_profile,
            reset_sequence,
        )
        .await?;
    let after_sequence = active_summary
        .as_ref()
        .map_or(reset_sequence, |summary| summary.covered_item_end_sequence);
    let items = request
        .store
        .list_all_conversation_items_after_sequence_for_context(
            request.conversation_id,
            after_sequence,
        )
        .await?;
    let previous = snapshot_from_items(&items)?;
    let updates = request.state.diff(previous.as_ref());
    let mut resulting_snapshot = previous.unwrap_or_default();
    for update in &updates {
        resulting_snapshot.apply(update);
    }
    debug_assert_eq!(resulting_snapshot, request.state.snapshot());

    for update in &updates {
        let content = update.model_visible_content();
        let payload_json = json!({ UPDATE_PAYLOAD_KEY: update });
        request
            .store
            .append_conversation_item(NewConversationItem {
                conversation_id: request.conversation_id.to_string(),
                turn_id: Some(request.turn_id.to_string()),
                parent_item_id: None,
                kind: ConversationItemKind::ModelContextUpdate,
                status: ConversationItemStatus::Completed,
                author: ActorRef::new("agent:primary")
                    .expect("static primary agent id must be valid"),
                content_text: Some(content),
                payload_json,
                metadata: json!({
                    "source": "model_context_ledger",
                    "section_id": update.section_id.as_str(),
                }),
            })
            .await?;
    }

    Ok(updates)
}

fn snapshot_from_items(
    items: &[ConversationItemRecord],
) -> Result<Option<ModelContextSnapshot>, RuntimeError> {
    let mut snapshot = ModelContextSnapshot::default();
    let mut found = false;
    for item in items {
        if item.kind != ConversationItemKind::ModelContextUpdate {
            continue;
        }
        let update = update_from_payload(&item.payload_json).map_err(|source| {
            RuntimeError::Protocol(format!(
                "invalid model context update payload for {}: {source}",
                item.item_id
            ))
        })?;
        snapshot.apply(&update);
        found = true;
    }
    Ok(found.then_some(snapshot))
}

fn update_from_payload(payload: &Value) -> Result<ModelContextUpdate, serde_json::Error> {
    serde_json::from_value(
        payload
            .get(UPDATE_PAYLOAD_KEY)
            .cloned()
            .unwrap_or(Value::Null),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::daemon::runtime::model_context::{
        AgentIdentityContext, RuntimeEnvironmentContext, ToolVisibilityContext,
    };

    fn state(date: &str) -> ModelContextState {
        ModelContextState::new(
            AgentIdentityContext {
                agent_id: "agent:primary".to_string(),
                display_name: Some("Noema".to_string()),
            },
            RuntimeEnvironmentContext::new(date, "12:00:00+00:00", "UTC", Some("/workspace")),
            ToolVisibilityContext::new(
                noema_providers::ProviderToolTransport::Native,
                Vec::new(),
                vec!["- builtin\tread".to_string()],
            ),
        )
    }

    fn record(index: i64, update: &ModelContextUpdate) -> ConversationItemRecord {
        ConversationItemRecord {
            item_id: format!("item:{index}"),
            conversation_id: "conversation:1".to_string(),
            turn_id: Some("turn:1".to_string()),
            sequence_index: index,
            cursor: format!("conversation_item:{index}"),
            kind: ConversationItemKind::ModelContextUpdate,
            status: ConversationItemStatus::Completed,
            content_text: Some(update.model_visible_content()),
            payload_json: json!({ UPDATE_PAYLOAD_KEY: update }),
            metadata: json!({}),
            created_at: String::new(),
        }
    }

    #[test]
    fn reconstructs_snapshot_from_ordered_durable_updates() {
        let initial = state("2026-07-15");
        let replacement = state("2026-07-16");
        let mut items = initial
            .full_updates()
            .iter()
            .enumerate()
            .map(|(index, update)| record(i64::try_from(index).expect("small index"), update))
            .collect::<Vec<_>>();
        let offset = items.len();
        items.extend(
            replacement
                .diff(Some(&initial.snapshot()))
                .iter()
                .enumerate()
                .map(|(index, update)| {
                    record(i64::try_from(index + offset).expect("small index"), update)
                }),
        );

        let snapshot = snapshot_from_items(&items)
            .expect("valid updates")
            .expect("snapshot exists");

        assert_eq!(snapshot, replacement.snapshot());
        assert!(replacement.diff(Some(&snapshot)).is_empty());
    }

    #[tokio::test]
    async fn durable_sync_diffs_and_resets_after_compaction_checkpoint() {
        let store = crate::test_support::test_store().await;
        store.ensure_default_actors().await.expect("actors");
        let conversation = store
            .get_or_create_primary_conversation("human:local", None, None)
            .await
            .expect("conversation");
        let turn = store
            .create_conversation_turn(noema_conversations::NewConversationTurn {
                conversation_id: conversation.conversation_id.clone(),
                trigger_item_id: None,
                metadata: json!({}),
            })
            .await
            .expect("turn");
        let initial = state("2026-07-15");

        let initial_updates = sync_model_context(ModelContextSyncRequest {
            store: &store,
            conversation_id: &conversation.conversation_id,
            turn_id: &turn.turn_id,
            provider_kind: "codex",
            model_profile: None,
            state: &initial,
        })
        .await
        .expect("initial sync");
        assert_eq!(initial_updates.len(), 3);

        let unchanged_updates = sync_model_context(ModelContextSyncRequest {
            store: &store,
            conversation_id: &conversation.conversation_id,
            turn_id: &turn.turn_id,
            provider_kind: "codex",
            model_profile: None,
            state: &initial,
        })
        .await
        .expect("unchanged sync");
        assert!(unchanged_updates.is_empty());

        let replacement = state("2026-07-16");
        let replacement_updates = sync_model_context(ModelContextSyncRequest {
            store: &store,
            conversation_id: &conversation.conversation_id,
            turn_id: &turn.turn_id,
            provider_kind: "codex",
            model_profile: None,
            state: &replacement,
        })
        .await
        .expect("replacement sync");
        assert_eq!(replacement_updates.len(), 1);

        let before_summary = store
            .list_all_conversation_items_after_sequence_for_context(
                &conversation.conversation_id,
                0,
            )
            .await
            .expect("context items");
        let covered_end = before_summary
            .last()
            .expect("context items exist")
            .sequence_index;
        store
            .insert_conversation_context_summary(
                noema_conversations::NewConversationContextSummary {
                    conversation_id: conversation.conversation_id.clone(),
                    provider_kind: "codex".to_string(),
                    model_profile: None,
                    summary_text: "compacted transcript".to_string(),
                    covered_item_start_sequence: 1,
                    covered_item_end_sequence: covered_end,
                    source_item_ids: before_summary
                        .iter()
                        .map(|item| item.item_id.clone())
                        .collect(),
                    input_token_estimate: 100,
                    summary_token_estimate: 10,
                    compaction_provider_kind: "codex".to_string(),
                    compaction_model_profile: None,
                    status: noema_conversations::ConversationContextSummaryStatus::Active,
                    error_code: None,
                    error_message: None,
                },
            )
            .await
            .expect("summary");

        let reset_updates = sync_model_context(ModelContextSyncRequest {
            store: &store,
            conversation_id: &conversation.conversation_id,
            turn_id: &turn.turn_id,
            provider_kind: "codex",
            model_profile: None,
            state: &replacement,
        })
        .await
        .expect("post-compaction sync");
        assert_eq!(reset_updates.len(), 3);
        assert!(reset_updates.iter().all(|update| {
            update.operation == super::super::model_context::ModelContextUpdateOperation::Full
        }));
    }
}
