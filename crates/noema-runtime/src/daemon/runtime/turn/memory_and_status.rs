impl RuntimeActor {
    async fn enqueue_user_message_memory_observation(
        &self,
        conversation_id: &str,
        turn_id: &str,
        user_item_id: &str,
        user_sequence_index: i64,
        user_text: &str,
        await_turn_finished: oneshot::Receiver<()>,
    ) {
        let assistant_context = self
            .memory_observation_assistant_context(
                conversation_id,
                user_item_id,
                user_sequence_index,
            )
            .await;
        let Some(request) = build_memory_observation_add_request(
            conversation_id,
            turn_id,
            user_item_id,
            user_text,
            assistant_context,
        ) else {
            return;
        };
        let Some(memory_operations) = self.memory_operations.clone() else {
            self.log_runtime_invariant(
                "memory observation could not be submitted",
                json!({
                    "conversation_id": conversation_id,
                    "turn_id": turn_id,
                    "source_item_id": user_item_id,
                }),
                json!({
                    "error_code": "service_unavailable",
                    "error": "memory service is unavailable",
                }),
            );
            return;
        };
        let system_errors = self.system_errors.clone();
        let error_context = json!({
            "conversation_id": conversation_id,
            "turn_id": turn_id,
            "source_item_id": user_item_id,
        });
        self.tasks.spawn(async move {
            let _ = await_turn_finished.await;
            if let Err(error) = memory_operations.add_memory(request).await {
                let message = "memory observation submit failed".to_string();
                system_errors.try_append(
                    SystemErrorEvent::new(SYSTEM_ERROR_RUNTIME_INVARIANT, message.clone())
                        .with_context(error_context)
                        .with_error_chain([message])
                        .with_raw(json!({
                            "error_code": error.code(),
                            "error": error.to_string(),
                        })),
                );
            }
        });
    }

    async fn memory_observation_assistant_context(
        &self,
        conversation_id: &str,
        user_item_id: &str,
        user_sequence_index: i64,
    ) -> Vec<String> {
        let Ok(items) = self
            .store
            .list_recent_conversation_items_for_context(conversation_id, 40)
            .await
        else {
            return Vec::new();
        };

        let mut messages = Vec::new();
        for item in items.iter().rev() {
            if item.item_id == user_item_id || item.sequence_index >= user_sequence_index {
                continue;
            }
            match item.kind {
                ConversationItemKind::UserText => break,
                ConversationItemKind::AssistantText => {
                    if let Some(text) = item.content_text.as_deref() {
                        messages.push(text.to_string());
                    }
                }
                _ => {}
            }
        }
        messages.reverse();
        bound_memory_observation_assistant_context(messages)
    }

    pub(in crate::daemon) async fn update_conversation_agent_status(
        &mut self,
        conversation_id: &str,
        status: PersistedAgentStatus,
        item_tx: &mpsc::UnboundedSender<TurnStreamEvent>,
    ) -> Result<(), RuntimeError> {
        self.store
            .update_conversation_agent_status(conversation_id, status)
            .await?;
        let _ = item_tx.send(TurnStreamEvent::AgentStatusChanged {
            conversation_id: conversation_id.to_string(),
            status,
        });
        Ok(())
    }
}
