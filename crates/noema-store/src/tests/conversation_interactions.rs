use noema_conversations::{
    ActorRef, ConversationItemKind, ConversationItemStatus, NewConversationItem,
    NewConversationTurn,
};
use serde_json::json;

use super::test_store;
use crate::{
    ConversationInteractionKind, ConversationInteractionStatus, NewConversationInteraction,
};

#[tokio::test]
async fn interaction_publication_resolution_and_recovery_are_atomic_one_use_cases() {
    for (case, terminal_error, reconcile_completed_turn) in [
        ("success", None, false),
        ("crash_after_persistence", None, true),
        ("failure", Some("provider_timeout"), false),
    ] {
        let store = test_store().await;
        store.ensure_default_actors().await.expect("actors");
        let conversation = store
            .create_conversation(noema_conversations::NewConversation::local_chat(
                Some("gpt-test".to_string()),
                None,
            ))
            .await
            .expect("conversation");
        let turn = store
            .create_conversation_turn(NewConversationTurn {
                conversation_id: conversation.conversation_id.clone(),
                trigger_item_id: None,
                metadata: json!({"turn_index": 1}),
            })
            .await
            .expect("turn");

        let base_interaction = NewConversationInteraction {
            interaction_id: format!("interaction:{case}"),
            conversation_id: conversation.conversation_id.clone(),
            originating_turn_id: turn.turn_id.clone(),
            kind: ConversationInteractionKind::MultipleChoice,
            provider_call_id: format!("call:{case}"),
            canonical_tool_name: "noema.present_multiple_choice".to_string(),
            provider_tool_name: "present_multiple_choice".to_string(),
            provider_kind: "codex".to_string(),
            provider_account_id: "provider_account:codex:default".to_string(),
            provider_instance_key: "provider-instance:codex:default".to_string(),
            selection_mode: "explicit_profile".to_string(),
            credential_revision: 4,
            model: "gpt-test".to_string(),
            reasoning_effort: Some("medium".to_string()),
            tool_catalog_digest: "a".repeat(64),
            request: json!({"prompt": "Choose one"}),
            projection: json!({"prompt": "Choose one", "options": [{"id": "yes"}]}),
        };
        let provider_tool_call = item(
            &conversation.conversation_id,
            &turn.turn_id,
            ConversationItemKind::ToolCall,
            ActorRef::agent("agent:primary").expect("agent"),
            json!({"provider_call_id": base_interaction.provider_call_id, "name": "present_multiple_choice"}),
        );
        let projection = item(
            &conversation.conversation_id,
            &turn.turn_id,
            ConversationItemKind::MultipleChoicePrompt,
            ActorRef::agent("agent:primary").expect("agent"),
            json!({"prompt": "Choose one", "options": [{"id": "yes"}]}),
        );

        let published = store
            .publish_conversation_interaction(
                base_interaction.clone(),
                provider_tool_call.clone(),
                projection.clone(),
            )
            .await
            .expect("publish interaction");
        assert_eq!(published.status, ConversationInteractionStatus::Pending);
        assert_eq!(published.revision, 1);
        assert_eq!(
            store
                .list_conversation_items(
                    &conversation.conversation_id,
                    noema_conversations::ReplayMode::Audit,
                )
                .await
                .expect("published items")
                .len(),
            2
        );
        assert_eq!(
            store
                .conversation_runtime_status(&conversation.conversation_id)
                .await
                .expect("runtime status")
                .expect("turn status")
                .turn_status,
            noema_conversations::ConversationTurnStatus::WaitingForTool
        );
        store
            .recover_shutdown_cancelled_work(&conversation.conversation_id)
            .await
            .expect("preserve interaction during shutdown recovery");
        let runtime = store
            .conversation_runtime_status(&conversation.conversation_id)
            .await
            .expect("runtime status")
            .expect("interaction runtime");
        assert_eq!(
            runtime.turn_status,
            noema_conversations::ConversationTurnStatus::WaitingForTool
        );
        assert_eq!(
            runtime.agent_status,
            noema_conversations::AgentStatus::ToolRunning
        );

        let human_action = item(
            &conversation.conversation_id,
            &turn.turn_id,
            ConversationItemKind::MultipleChoiceSelection,
            ActorRef::human("human:local").expect("human"),
            json!({"prompt_item_id": published.projection_item_id, "selected_option_ids": ["yes"]}),
        );
        let provider_tool_result = item(
            &conversation.conversation_id,
            &turn.turn_id,
            ConversationItemKind::ToolResult,
            ActorRef::agent("agent:primary").expect("agent"),
            json!({"provider_call_id": base_interaction.provider_call_id, "success": true, "payload": {"selected": ["yes"]}}),
        );
        let answered = store
            .resolve_conversation_interaction(
                &published.interaction_id,
                1,
                &format!("client:{case}"),
                human_action.clone(),
                provider_tool_result.clone(),
            )
            .await
            .expect("resolve interaction");
        assert_eq!(answered.status, ConversationInteractionStatus::Answered);
        assert_eq!(answered.revision, 2);
        assert_eq!(
            store
                .list_conversation_items(
                    &conversation.conversation_id,
                    noema_conversations::ReplayMode::Audit,
                )
                .await
                .expect("resolved items")
                .len(),
            4
        );

        let duplicate = store
            .resolve_conversation_interaction(
                &published.interaction_id,
                1,
                &format!("client:{case}:duplicate"),
                human_action,
                provider_tool_result,
            )
            .await;
        assert!(duplicate.is_err(), "a pending revision is one-use");
        assert_eq!(
            store
                .list_conversation_items(
                    &conversation.conversation_id,
                    noema_conversations::ReplayMode::Audit,
                )
                .await
                .expect("one-use items")
                .len(),
            4
        );

        let claimed = store
            .claim_conversation_interaction_resumption(
                &published.interaction_id,
                2,
                "worker:test",
                1,
            )
            .await
            .expect("claim interaction");
        assert_eq!(claimed.status, ConversationInteractionStatus::Resuming);
        let claim_token = claimed.resume_claim_token.clone().expect("claim token");
        store
            .heartbeat_conversation_interaction_resumption(
                &published.interaction_id,
                2,
                &claim_token,
                30,
            )
            .await
            .expect("heartbeat interaction");
        if reconcile_completed_turn {
            store
                .complete_conversation_turn(&turn.turn_id)
                .await
                .expect("simulate persisted continuation");
        }

        // Simulate a process crash. Listing resumptions reclaims the expired
        // lease and exposes the answered row for a new worker.
        if case == "failure" {
            store
                .recover_shutdown_cancelled_work(&conversation.conversation_id)
                .await
                .expect("release claim during clean shutdown");
        } else {
            store
                .with_connection(|connection| {
                    connection.execute(
                        "UPDATE conversation_interactions SET resume_claim_expires_at = '2000-01-01T00:00:00.000Z' WHERE interaction_id = ?1",
                        [&published.interaction_id],
                    )?;
                    Ok(())
                })
                .await
                .expect("expire claim");
        }
        let resumable = store
            .list_resumable_conversation_interactions()
            .await
            .expect("list resumable interactions");
        if reconcile_completed_turn {
            assert!(resumable.is_empty());
            let reconciled = store
                .get_conversation_interaction(&published.interaction_id)
                .await
                .expect("reconciled interaction")
                .expect("interaction");
            assert_eq!(reconciled.status, ConversationInteractionStatus::Completed);
            continue;
        }
        assert_eq!(resumable.len(), 1);
        assert_eq!(resumable[0].status, ConversationInteractionStatus::Answered);

        let reclaimed_claim = store
            .claim_conversation_interaction_resumption(
                &published.interaction_id,
                2,
                "worker:recovered",
                30,
            )
            .await
            .expect("reclaim interaction");
        let recovered_token = reclaimed_claim
            .resume_claim_token
            .clone()
            .expect("recovered claim token");
        assert_ne!(claim_token, recovered_token);
        let completed = store
            .finish_conversation_interaction_resumption(
                &published.interaction_id,
                2,
                &recovered_token,
                terminal_error,
            )
            .await
            .expect("finish interaction");
        assert_eq!(
            completed.status,
            if terminal_error.is_some() {
                ConversationInteractionStatus::Failed
            } else {
                ConversationInteractionStatus::Completed
            }
        );
        assert_eq!(completed.terminal_error.as_deref(), terminal_error);
    }
}

#[tokio::test]
async fn interaction_publication_rolls_back_items_and_turn_when_turn_fence_fails() {
    let store = test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let conversation = store
        .create_conversation(noema_conversations::NewConversation::local_chat(None, None))
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
    store
        .with_connection(|connection| {
            connection.execute(
                "UPDATE conversation_turns SET status = 'completed' WHERE turn_id = ?1",
                [&turn.turn_id],
            )?;
            Ok(())
        })
        .await
        .expect("fence turn");

    let interaction = NewConversationInteraction {
        interaction_id: "interaction:rollback".to_string(),
        conversation_id: conversation.conversation_id.clone(),
        originating_turn_id: turn.turn_id.clone(),
        kind: ConversationInteractionKind::A2UI,
        provider_call_id: "call:rollback".to_string(),
        canonical_tool_name: "noema.present_a2ui".to_string(),
        provider_tool_name: "present_a2ui".to_string(),
        provider_kind: "foundation_local".to_string(),
        provider_account_id: "provider_account:foundation:default".to_string(),
        provider_instance_key: "provider-instance:foundation:default".to_string(),
        selection_mode: "provider_default".to_string(),
        credential_revision: 1,
        model: "foundation-profile".to_string(),
        reasoning_effort: None,
        tool_catalog_digest: "b".repeat(64),
        request: json!({"jsonl": "{}"}),
        projection: json!({"surface_id": "surface:rollback"}),
    };
    let tool_call = item(
        &conversation.conversation_id,
        &turn.turn_id,
        ConversationItemKind::ToolCall,
        ActorRef::agent("agent:primary").expect("agent"),
        json!({"provider_call_id": "call:rollback"}),
    );
    let projection = item(
        &conversation.conversation_id,
        &turn.turn_id,
        ConversationItemKind::A2UICard,
        ActorRef::agent("agent:primary").expect("agent"),
        json!({"surface_id": "surface:rollback"}),
    );
    assert!(
        store
            .publish_conversation_interaction(interaction, tool_call, projection)
            .await
            .is_err()
    );
    assert_eq!(
        store
            .list_conversation_items(
                &conversation.conversation_id,
                noema_conversations::ReplayMode::Audit,
            )
            .await
            .expect("rolled back items")
            .len(),
        0
    );
    assert_eq!(
        store
            .conversation_runtime_status(&conversation.conversation_id)
            .await
            .expect("runtime status")
            .expect("turn")
            .turn_status,
        noema_conversations::ConversationTurnStatus::Completed
    );
}

fn item(
    conversation_id: &str,
    turn_id: &str,
    kind: ConversationItemKind,
    author: ActorRef,
    payload_json: serde_json::Value,
) -> NewConversationItem {
    NewConversationItem {
        conversation_id: conversation_id.to_string(),
        turn_id: Some(turn_id.to_string()),
        parent_item_id: None,
        kind,
        status: if kind == ConversationItemKind::ToolCall {
            ConversationItemStatus::Running
        } else {
            ConversationItemStatus::Completed
        },
        author,
        content_text: None,
        payload_json,
        metadata: json!({}),
    }
}
