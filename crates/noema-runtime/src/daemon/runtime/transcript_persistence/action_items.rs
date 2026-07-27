impl RuntimeActor {
    pub(super) async fn persist_hosted_web_searches(
        &mut self,
        turn: &ProviderActionTurn,
        output_index: usize,
        searches: &[GenerateHostedWebSearch],
        item_tx: &mpsc::UnboundedSender<TurnStreamEvent>,
    ) -> Result<(), RuntimeError> {
        for (offset, search) in searches.iter().enumerate() {
            let failed = search.status.eq_ignore_ascii_case("failed");
            self.persist_provider_action_output(
                turn,
                ProviderActionOutput {
                    index: output_index + offset,
                    kind: ConversationItemKind::Activity,
                    status: if failed {
                        ConversationItemStatus::Failed
                    } else {
                        ConversationItemStatus::Completed
                    },
                    action_kind: "hosted_web_search",
                    title: if failed {
                        "Web search failed".to_string()
                    } else {
                        "Searched the web".to_string()
                    },
                    summary: Some("Native model provider".to_string()),
                    payload: json!({
                        "id": search.id,
                        "status": search.status,
                        "action": search.action,
                    }),
                    display: json!({
                        "name": "Provider-native web search",
                        "result": search.status,
                    }),
                },
                item_tx,
            )
            .await?;
        }
        Ok(())
    }

    pub(super) async fn persist_provider_action_item(
        &mut self,
        turn: &ProviderActionTurn,
        index: usize,
        output: GenerateActionItem,
        item_tx: &mpsc::UnboundedSender<TurnStreamEvent>,
    ) -> Result<(), RuntimeError> {
        match output {
            GenerateActionItem::ToolCall {
                id,
                provider_call_id,
                provider_name,
                name,
                payload,
            } => {
                let display = tool_call_display(&name, &payload);
                self.persist_provider_action_output(
                    turn,
                    ProviderActionOutput {
                        index,
                        kind: ConversationItemKind::ToolCall,
                        status: ConversationItemStatus::Completed,
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
                .await?;
            }
            GenerateActionItem::ToolResult {
                call_id,
                provider_call_id,
                provider_name,
                name,
                success,
                payload,
            } => {
                let status = if success == Some(false) {
                    ConversationItemStatus::Failed
                } else {
                    ConversationItemStatus::Completed
                };
                let display = tool_result_display(name.as_deref(), success, &payload);
                self.persist_provider_action_output(
                    turn,
                    ProviderActionOutput {
                        index,
                        kind: ConversationItemKind::ToolResult,
                        status,
                        action_kind: "tool_result",
                        title: name.as_ref().map_or_else(
                            || "Tool result".to_string(),
                            |name| format!("Tool result: {name}"),
                        ),
                        summary: display_summary(&display, "result"),
                        payload: json!({
                            "call_id": call_id,
                            "provider_call_id": provider_call_id,
                            "provider_name": provider_name,
                            "name": name,
                            "success": success,
                            "payload": payload,
                        }),
                        display,
                    },
                    item_tx,
                )
                .await?;
            }
            GenerateActionItem::ApprovalRequest {
                id,
                method,
                payload,
            } => {
                self.persist_provider_action_output(
                    turn,
                    ProviderActionOutput {
                        index,
                        kind: ConversationItemKind::ApprovalRequest,
                        status: ConversationItemStatus::Completed,
                        action_kind: "approval_request",
                        title: "Approval requested".to_string(),
                        summary: Some(method.clone()),
                        payload: json!({
                            "id": id,
                            "method": method.clone(),
                            "payload": payload,
                        }),
                        display: json!({
                            "name": "Approval requested",
                            "purpose": "Ask before taking an external action",
                            "access": "Requires approval",
                            "target": method.clone(),
                        }),
                    },
                    item_tx,
                )
                .await?;
            }
            GenerateActionItem::AuthenticationRequest {
                id,
                method,
                payload,
            } => {
                self.persist_provider_action_output(
                    turn,
                    ProviderActionOutput {
                        index,
                        kind: ConversationItemKind::Activity,
                        status: ConversationItemStatus::Completed,
                        action_kind: "authentication_request",
                        title: "Sign-in required".to_string(),
                        summary: Some("Sign-in requested".to_string()),
                        payload: json!({
                            "id": id,
                            "method": method,
                            "payload": payload,
                        }),
                        display: json!({
                            "name": "Sign-in required",
                            "purpose": "Authenticate before continuing this tool call",
                            "access": "Waiting for you",
                            "target": method,
                        }),
                    },
                    item_tx,
                )
                .await?;
            }
            GenerateActionItem::ApprovalResult {
                request_id,
                decision,
                payload,
            } => {
                self.persist_provider_action_output(
                    turn,
                    ProviderActionOutput {
                        index,
                        kind: ConversationItemKind::ApprovalResult,
                        status: ConversationItemStatus::Completed,
                        action_kind: "approval_result",
                        title: format!("Approval {decision}"),
                        summary: Some(decision.clone()),
                        payload: json!({
                            "request_id": request_id,
                            "decision": decision.clone(),
                            "payload": payload,
                        }),
                        display: json!({
                            "name": "Approval decision",
                            "result": decision.clone(),
                        }),
                    },
                    item_tx,
                )
                .await?;
            }
        }
        Ok(())
    }

    async fn persist_provider_action_output(
        &mut self,
        turn: &ProviderActionTurn,
        action: ProviderActionOutput,
        item_tx: &mpsc::UnboundedSender<TurnStreamEvent>,
    ) -> Result<(), RuntimeError> {
        let activity_id = format!(
            "{}:{}:{}:{}",
            action.action_kind, turn.conversation_id, turn.turn_index, action.index
        );
        let activity_status = activity_status_for_conversation_item(action.status);
        let title = action.title.clone();
        let summary = action.summary.clone();
        let payload_json = json!({
            "id": activity_id,
            "activity_kind": action.action_kind,
            "status": activity_status_payload(activity_status),
            "title": title.clone(),
            "summary": summary.clone(),
            "metadata": {
                "turn_index": turn.turn_index,
                "output_index": action.index,
                "provider": turn.provider.clone(),
                "action": action.payload,
                "display": action.display,
            },
        });
        let content_text = payload_json
            .get("title")
            .and_then(serde_json::Value::as_str)
            .map(ToString::to_string);
        let metadata = json!({
            "turn_index": turn.turn_index,
            "output_index": action.index,
            "source": "provider_action",
            "provider": turn.provider.clone(),
        });
        let record = self
            .store
            .append_conversation_item(NewConversationItem {
                conversation_id: turn.conversation_id.clone(),
                turn_id: Some(turn.turn_id.clone()),
                parent_item_id: Some(turn.user_item_id.clone()),
                kind: action.kind,
                status: action.status,
                author: ActorRef::agent("agent:primary")
                    .expect("static primary agent id must be valid"),
                content_text,
                payload_json: payload_json.clone(),
                metadata: metadata.clone(),
            })
            .await?;

        let transcript_item = TurnTranscriptItem::Activity {
            id: activity_id,
            activity_kind: action.action_kind.to_string(),
            status: activity_status,
            title,
            summary,
            metadata: payload_json["metadata"].clone(),
        };
        send_conversation_item(item_tx, record, metadata, transcript_item);
        Ok(())
    }

}
