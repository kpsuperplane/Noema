impl RuntimeActor {
    pub(super) async fn persist_provider_tool_call_started(
        &mut self,
        turn: &ProviderActionTurn,
        index: usize,
        call: &LocalToolCall,
        item_tx: &mpsc::UnboundedSender<TurnStreamEvent>,
    ) -> Result<(), RuntimeError> {
        let GenerateActionItem::ToolCall {
            id,
            provider_call_id,
            provider_name,
            name,
            payload,
        } = tool_call_action_item(call)
        else {
            return Ok(());
        };
        let display = tool_call_display(&name, &payload);
        self.persist_provider_action_output(
            turn,
            ProviderActionOutput {
                index,
                kind: ConversationItemKind::ToolCall,
                status: ConversationItemStatus::Running,
                action_kind: "tool_call",
                title: format!("Tool call: {name}"),
                summary: display_summary(&display, "target"),
                payload: json!({
                    "id": id,
                    "provider_call_id": provider_call_id,
                    "provider_name": provider_name,
                    "name": name,
                    "payload": payload,
                }),
                display,
            },
            item_tx,
        )
        .await
    }

    pub(super) async fn persist_progress_audit_started(
        &mut self,
        turn: &ProviderActionTurn,
        index: usize,
        item_tx: &mpsc::UnboundedSender<TurnStreamEvent>,
    ) -> Result<(), RuntimeError> {
        let display = progress_audit_display("Checking progress", "running", None);
        self.persist_provider_action_output(
            turn,
            ProviderActionOutput {
                index,
                kind: ConversationItemKind::Activity,
                status: ConversationItemStatus::Running,
                action_kind: "progress_audit",
                title: "Checking progress".to_string(),
                summary: Some("Checking progress".to_string()),
                payload: json!({ "kind": "progress_audit", "status": "running" }),
                display,
            },
            item_tx,
        )
        .await
    }

    pub(super) async fn persist_progress_audit_completed(
        &mut self,
        turn: &ProviderActionTurn,
        index: usize,
        label: &str,
        summary: &str,
        item_tx: &mpsc::UnboundedSender<TurnStreamEvent>,
    ) -> Result<(), RuntimeError> {
        let display = progress_audit_display(label, "completed", Some(summary));
        self.persist_provider_action_output(
            turn,
            ProviderActionOutput {
                index,
                kind: ConversationItemKind::Activity,
                status: ConversationItemStatus::Completed,
                action_kind: "progress_audit",
                title: label.to_string(),
                summary: Some(summary.to_string()),
                payload: json!({ "kind": "progress_audit", "status": "completed", "label": label }),
                display,
            },
            item_tx,
        )
        .await
    }

    pub(super) async fn persist_agent_initiated_provider_response(
        &mut self,
        conversation_id: &str,
        turn_id: &str,
        turn_index: u64,
        response: GenerateResponse,
    ) -> Result<usize, RuntimeError> {
        let mut persisted_count = 0usize;
        for (index, output) in response.responses.into_iter().enumerate() {
            let GenerateResponseItem::Text { text, .. } = output else {
                continue;
            };
            let metadata = json!({
                "turn_index": turn_index,
                "response_index": index,
                "provider": response.provider.clone(),
                "source": "agent_onboarding",
            });
            self.store
                .append_conversation_item(NewConversationItem {
                    conversation_id: conversation_id.to_string(),
                    turn_id: Some(turn_id.to_string()),
                    parent_item_id: None,
                    kind: ConversationItemKind::AssistantText,
                    status: ConversationItemStatus::Completed,
                    author: ActorRef::agent("agent:primary")
                        .expect("static primary agent id must be valid"),
                    content_text: Some(text),
                    payload_json: json!({}),
                    metadata,
                })
                .await?;
            persisted_count += 1;
        }
        Ok(persisted_count)
    }

    pub(super) async fn persist_partial_provider_action_outputs(
        &mut self,
        turn: &ProviderActionTurn,
        output: Vec<GenerateActionItem>,
        item_tx: &mpsc::UnboundedSender<TurnStreamEvent>,
    ) -> Result<(), RuntimeError> {
        for (index, output) in output.into_iter().enumerate() {
            self.persist_provider_action_item(turn, index, output, item_tx)
                .await?;
        }
        Ok(())
    }

}
