impl RuntimeActor {
    pub(super) async fn persist_provider_reasoning_items(
        &mut self,
        conversation_id: &str,
        turn_id: &str,
        reasoning_items: &[GenerateReasoningItem],
    ) -> Result<(), RuntimeError> {
        for reasoning in reasoning_items {
            let Some(encrypted_content) = reasoning
                .encrypted_content
                .as_deref()
                .filter(|value| !value.trim().is_empty())
            else {
                continue;
            };
            self.store
                .append_conversation_item(NewConversationItem {
                    conversation_id: conversation_id.to_string(),
                    turn_id: Some(turn_id.to_string()),
                    parent_item_id: None,
                    kind: ConversationItemKind::Reasoning,
                    status: ConversationItemStatus::Completed,
                    author: ActorRef::agent("agent:primary")
                        .expect("static primary agent id must be valid"),
                    content_text: None,
                    payload_json: json!({
                        "provider_reasoning": {
                            "id": reasoning.id.clone(),
                            "encrypted_content": encrypted_content,
                        }
                    }),
                    metadata: json!({}),
                })
                .await?;
        }
        Ok(())
    }

    pub(super) async fn persist_provider_response_item(
        &mut self,
        turn: &ProviderActionTurn,
        position: ProviderResponsePosition,
        item: GenerateResponseItem,
        provider_phase_has_tools: bool,
        assistant_response: &mut ProviderAssistantResponse,
        item_tx: &mpsc::UnboundedSender<TurnStreamEvent>,
    ) -> Result<(), RuntimeError> {
        match item {
            GenerateResponseItem::Text { phase, text } => {
                assistant_response.push_text(&text);
                let effective_phase = AssistantTextPhase::effective_for_response_item(
                    &GenerateResponseItem::Text {
                        phase,
                        text: text.clone(),
                    },
                    provider_phase_has_tools,
                );
                let output_index = position.output_index.unwrap_or(position.response_index);
                let stream_id = turn
                    .stream_id
                    .as_deref()
                    .map(|stream_id| assistant_response_stream_id(stream_id, output_index));
                let mut metadata = json!({
                    "turn_index": turn.turn_index,
                    "response_index": position.response_index,
                    "output_index": position.output_index,
                    "stream_id": stream_id,
                    "phase": effective_phase.as_str(),
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
                let assistant_item = self
                    .store
                    .append_conversation_item(NewConversationItem {
                        conversation_id: turn.conversation_id.clone(),
                        turn_id: Some(turn.turn_id.clone()),
                        parent_item_id: Some(turn.user_item_id.clone()),
                        kind: ConversationItemKind::AssistantText,
                        status: ConversationItemStatus::Completed,
                        author: ActorRef::agent("agent:primary")
                            .expect("static primary agent id must be valid"),
                        content_text: Some(text.clone()),
                        payload_json: json!({}),
                        metadata: metadata.clone(),
                    })
                    .await?;
                if assistant_response.item_id.is_none() {
                    assistant_response.item_id = Some(assistant_item.item_id.clone());
                }
                send_conversation_item(
                    item_tx,
                    assistant_item,
                    metadata,
                    TurnTranscriptItem::AssistantText { text },
                );
            }
            GenerateResponseItem::Structured { schema, payload } => {
                let output_index = position.output_index.unwrap_or(position.response_index);
                let card_id = format!(
                    "provider_structured:{}:{}:{output_index}",
                    turn.conversation_id, turn.turn_index
                );
                let metadata = json!({
                    "turn_index": turn.turn_index,
                    "response_index": position.response_index,
                    "output_index": position.output_index,
                    "source": "provider_structured_output",
                });
                let structured_item = self
                    .store
                    .append_conversation_item(NewConversationItem {
                        conversation_id: turn.conversation_id.clone(),
                        turn_id: Some(turn.turn_id.clone()),
                        parent_item_id: Some(turn.user_item_id.clone()),
                        kind: ConversationItemKind::A2uiCard,
                        status: ConversationItemStatus::Completed,
                        author: ActorRef::agent("agent:primary")
                            .expect("static primary agent id must be valid"),
                        content_text: None,
                        payload_json: json!({
                            "id": card_id.clone(),
                            "schema": schema.clone(),
                            "payload": payload.clone(),
                        }),
                        metadata: metadata.clone(),
                    })
                    .await?;
                send_conversation_item(
                    item_tx,
                    structured_item,
                    metadata,
                    TurnTranscriptItem::A2uiCard {
                        id: card_id,
                        schema: schema.clone(),
                        payload: payload.clone(),
                    },
                );
            }
            GenerateResponseItem::MultipleChoice {
                phase,
                prompt,
                selection_mode,
                options,
            } => {
                let effective_phase = AssistantTextPhase::effective_for_response_item(
                    &GenerateResponseItem::MultipleChoice {
                        phase,
                        prompt: prompt.clone(),
                        selection_mode,
                        options: options.clone(),
                    },
                    provider_phase_has_tools,
                );
                let metadata = json!({
                    "turn_index": turn.turn_index,
                    "response_index": position.response_index,
                    "output_index": position.output_index,
                    "phase": effective_phase.as_str(),
                });
                let prompt_item = self
                    .store
                    .append_conversation_item(NewConversationItem {
                        conversation_id: turn.conversation_id.clone(),
                        turn_id: Some(turn.turn_id.clone()),
                        parent_item_id: Some(turn.user_item_id.clone()),
                        kind: ConversationItemKind::MultipleChoicePrompt,
                        status: ConversationItemStatus::Completed,
                        author: ActorRef::agent("agent:primary")
                            .expect("static primary agent id must be valid"),
                        content_text: Some(prompt.clone()),
                        payload_json: json!({
                            "prompt": prompt.clone(),
                            "selection_mode": selection_mode,
                            "options": options.clone(),
                        }),
                        metadata: metadata.clone(),
                    })
                    .await?;
                send_conversation_item(
                    item_tx,
                    prompt_item,
                    metadata,
                    TurnTranscriptItem::MultipleChoicePrompt {
                        prompt,
                        selection_mode,
                        options,
                    },
                );
            }
        }
        Ok(())
    }

}
