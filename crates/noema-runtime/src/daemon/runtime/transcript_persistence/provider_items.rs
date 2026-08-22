impl RuntimeActor {
    #[allow(clippy::too_many_arguments)]
    pub(super) async fn persist_provider_assistant_text(
        &mut self,
        stable_item_id: Option<String>,
        citation_sources: &CitationSourceRegistry,
        provider_text: String,
        citations: &[GenerateCitation],
        scope_kind: &'static str,
        scope_id: &str,
        mut item: NewConversationItem,
    ) -> Result<Option<(ConversationItemRecord, bool)>, RuntimeError> {
        let normalized = self.normalize_provider_citation_text(
            citation_sources,
            &provider_text,
            citations,
            scope_kind,
            scope_id,
        );
        if normalized.text.trim().is_empty() {
            return Ok(None);
        }
        item.content_text = Some(normalized.text);
        if !normalized.citations.is_empty()
            && let Some(metadata) = item.metadata.as_object_mut()
        {
            metadata.insert("citations".to_string(), json!(normalized.citations));
        }
        let saved = if let Some(item_id) = stable_item_id {
            self.store
                .append_provider_conversation_item_with_id_if_absent(
                    item_id,
                    item,
                    provider_text,
                )
                .await?
        } else {
            (
                self.store
                    .append_provider_conversation_item(item, provider_text)
                    .await?,
                true,
            )
        };
        Ok(Some(saved))
    }

    pub(super) async fn persist_provider_reasoning_items(
        &mut self,
        conversation_id: &str,
        turn_id: &str,
        provider: &str,
        reasoning_items: &[GenerateReasoningItem],
    ) -> Result<(), RuntimeError> {
        for reasoning in reasoning_items {
            let encrypted_content = reasoning
                .encrypted_content
                .as_deref()
                .filter(|value| !value.trim().is_empty());
            if encrypted_content.is_none()
                && reasoning
                    .provider_details
                    .as_ref()
                    .is_none_or(Vec::is_empty)
            {
                continue;
            }
            let mut provider_reasoning = json!({
                "id": reasoning.id.clone(),
                "encrypted_content": encrypted_content,
            });
            if let Some(details) = reasoning
                .provider_details
                .as_ref()
                .filter(|details| !details.is_empty())
                && let Some(object) = provider_reasoning.as_object_mut()
            {
                object.insert("provider_details".to_string(), json!(details));
            }
            self.store
                .append_conversation_item(NewConversationItem {
                    conversation_id: conversation_id.to_string(),
                    turn_id: Some(turn_id.to_string()),
                    parent_item_id: None,
                    kind: ConversationItemKind::Reasoning,
                    status: ConversationItemStatus::Completed,
                    author: ActorRef::new("agent:primary")
                        .expect("static primary agent id must be valid"),
                    content_text: None,
                    payload_json: json!({"provider_reasoning": provider_reasoning}),
                    metadata: json!({"provider": provider}),
                })
                .await?;
        }
        Ok(())
    }

    pub(super) async fn persist_provider_response_item(
        &mut self,
        turn: &ProviderActionTurn,
        position: ProviderResponsePosition,
        item: noema_providers::AssistantResponseText<'_>,
        citation_sources: &CitationSourceRegistry,
        assistant_response: &mut ProviderAssistantResponse,
        item_tx: &mpsc::UnboundedSender<TurnStreamEvent>,
    ) -> Result<(), RuntimeError> {
        let provider_text = item.text.to_string();
        let output_index = position.output_index.unwrap_or(position.response_index);
        let stream_id = turn
            .stream_id
            .as_deref()
            .map(|stream_id| assistant_response_stream_id(stream_id, output_index));
        let mut metadata = json!({
            "turn_index": turn.turn_index,
            "response_index": position.response_index,
            "output_index": position.output_index,
            "provider_round": turn.provider_round,
            "stream_id": stream_id,
            "phase": item.phase.as_str(),
            "provider_item_id": item.provider_item_id,
        });
        merge_metadata(
            &mut metadata,
            provider_usage_metadata(
                &turn.provider,
                &turn.model,
                turn.response_phase,
                position,
                turn.usage.as_ref(),
            ),
        );
        let stable_item_id = format!(
            "item:assistant:{}:{output_index}",
            turn.turn_id.strip_prefix("turn:").unwrap_or(&turn.turn_id)
        );
        let Some((assistant_item, inserted)) = self
            .persist_provider_assistant_text(
                Some(stable_item_id),
                citation_sources,
                provider_text,
                item.citations,
                "conversation_turn",
                &turn.turn_id,
                NewConversationItem {
                    conversation_id: turn.conversation_id.clone(),
                    turn_id: Some(turn.turn_id.clone()),
                    parent_item_id: Some(turn.user_item_id.clone()),
                    kind: ConversationItemKind::AssistantText,
                    status: ConversationItemStatus::Completed,
                    author: ActorRef::new("agent:primary")
                        .expect("static primary agent id must be valid"),
                    content_text: None,
                    payload_json: json!({}),
                    metadata: metadata.clone(),
                },
            )
            .await?
        else {
            return Ok(());
        };
        let text = assistant_item.content_text.clone().unwrap_or_default();
        assistant_response.push_text(&text);
        if assistant_response.item_id.is_none() {
            assistant_response.item_id = Some(assistant_item.item_id.clone());
        }
        if !inserted {
            return Ok(());
        }
        let metadata = assistant_item.metadata.clone();
        send_conversation_item(
            item_tx,
            assistant_item,
            metadata,
            TurnTranscriptItem::AssistantText { text },
        );
        self.runtime_events
            .publish_memory(crate::daemon::MemoryRuntimeEvent::Changed);
        Ok(())
    }

}
