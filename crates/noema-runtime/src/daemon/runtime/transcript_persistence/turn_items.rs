impl RuntimeActor {
    pub(super) async fn record_turn_failure(
        &mut self,
        context: &ConversationMemoryContext,
        message: String,
        item_tx: &mpsc::UnboundedSender<TurnStreamEvent>,
    ) -> Result<(), RuntimeError> {
        self.record_turn_failure_notice(context, message, false, item_tx)
            .await
    }

    pub(super) async fn record_turn_failure_notice(
        &mut self,
        context: &ConversationMemoryContext,
        message: String,
        recoverable: bool,
        item_tx: &mpsc::UnboundedSender<TurnStreamEvent>,
    ) -> Result<(), RuntimeError> {
        self.store.fail_conversation_turn(&context.turn_id).await?;
        self.update_conversation_agent_status(
            &context.conversation_id,
            PersistedAgentStatus::Error,
            item_tx,
        )
        .await?;
        let notice = TurnTranscriptItem::ErrorNotice {
            message,
            recoverable,
        };
        self.persist_and_send_turn_item(context, notice, item_tx)
            .await
    }

    pub(in crate::daemon) async fn persist_and_send_turn_item(
        &mut self,
        context: &ConversationMemoryContext,
        item: TurnTranscriptItem,
        item_tx: &mpsc::UnboundedSender<TurnStreamEvent>,
    ) -> Result<(), RuntimeError> {
        let (record, metadata) = self.persist_turn_item(context, &item).await?;
        send_conversation_item(item_tx, record, metadata, item);
        Ok(())
    }

    async fn persist_turn_item(
        &mut self,
        context: &ConversationMemoryContext,
        item: &TurnTranscriptItem,
    ) -> Result<(ConversationItemRecord, Value), RuntimeError> {
        let default_parent_item_id = context
            .assistant_item_id
            .clone()
            .or_else(|| Some(context.user_item_id.clone()));
        let (kind, status, author, parent_item_id, content_text, payload_json, metadata) =
            match item {
                TurnTranscriptItem::UserText { text } => (
                    ConversationItemKind::UserText,
                    ConversationItemStatus::Completed,
                    ActorRef::human("human:local")
                        .expect("static local human actor id must be valid"),
                    None,
                    Some(text.clone()),
                    json!({}),
                    json!({ "turn_index": context.turn_index }),
                ),
                TurnTranscriptItem::AssistantText { text } => (
                    ConversationItemKind::AssistantText,
                    ConversationItemStatus::Completed,
                    ActorRef::agent("agent:primary")
                        .expect("static primary agent id must be valid"),
                    default_parent_item_id.clone(),
                    Some(text.clone()),
                    json!({}),
                    json!({ "turn_index": context.turn_index }),
                ),
                TurnTranscriptItem::Activity {
                    id,
                    activity_kind,
                    status,
                    title,
                    summary,
                    metadata,
                } => (
                    ConversationItemKind::Activity,
                    conversation_item_status_for_activity(*status),
                    ActorRef::agent("agent:primary")
                        .expect("static primary agent id must be valid"),
                    default_parent_item_id.clone(),
                    Some(title.clone()),
                    json!({
                        "id": id,
                        "activity_kind": activity_kind,
                        "status": activity_status_payload(*status),
                        "title": title,
                        "summary": summary,
                        "metadata": metadata,
                    }),
                    json!({
                        "turn_index": context.turn_index,
                        "runtime_item_id": id,
                    }),
                ),
                TurnTranscriptItem::A2UISurface {
                    id,
                    interaction_id,
                    version,
                    interaction_revision,
                    lifecycle,
                    catalog,
                    snapshot,
                    ..
                } => {
                    let namespaced = snapshot["namespaced_surface_id"]
                        .as_str()
                        .unwrap_or_default();
                    (
                        ConversationItemKind::A2UICard,
                        ConversationItemStatus::Completed,
                        ActorRef::agent("agent:primary")
                            .expect("static primary agent id must be valid"),
                        default_parent_item_id.clone(),
                        None,
                        json!({
                            "id": id,
                            "schema": "a2ui.v0.9.1",
                            "payload": {
                                "protocol_version": version,
                                "catalog": catalog,
                                "surfaces": { (namespaced): snapshot },
                                "interaction_id": interaction_id,
                                "interaction_revision": interaction_revision,
                                "lifecycle": lifecycle,
                            },
                        }),
                        json!({
                            "turn_index": context.turn_index,
                            "runtime_item_id": id,
                            "schema": "a2ui.v0.9.1",
                        }),
                    )
                },
                TurnTranscriptItem::MultipleChoicePrompt {
                    prompt,
                    selection_mode,
                    options,
                } => (
                    ConversationItemKind::MultipleChoicePrompt,
                    ConversationItemStatus::Completed,
                    ActorRef::agent("agent:primary")
                        .expect("static primary agent id must be valid"),
                    default_parent_item_id.clone(),
                    Some(prompt.clone()),
                    json!({
                        "prompt": prompt,
                        "selection_mode": selection_mode,
                        "options": options,
                    }),
                    json!({ "turn_index": context.turn_index }),
                ),
                TurnTranscriptItem::MultipleChoiceSelection {
                    prompt_item_id,
                    selection_mode,
                    selected_options,
                } => (
                    ConversationItemKind::MultipleChoiceSelection,
                    ConversationItemStatus::Completed,
                    ActorRef::human("human:local")
                        .expect("static local human actor id must be valid"),
                    Some(prompt_item_id.clone()),
                    Some(
                        selected_options
                            .iter()
                            .map(|option| option.label.as_str())
                            .collect::<Vec<_>>()
                            .join(", "),
                    ),
                    json!({
                        "prompt_item_id": prompt_item_id,
                        "selection_mode": selection_mode,
                        "selected_options": selected_options,
                    }),
                    json!({ "turn_index": context.turn_index }),
                ),
                TurnTranscriptItem::ErrorNotice {
                    message,
                    recoverable,
                } => (
                    ConversationItemKind::ErrorNotice,
                    ConversationItemStatus::Failed,
                    ActorRef::agent("agent:primary")
                        .expect("static primary agent id must be valid"),
                    default_parent_item_id,
                    Some(message.clone()),
                    json!({
                        "message": message,
                        "recoverable": recoverable,
                    }),
                    json!({ "turn_index": context.turn_index }),
                ),
                TurnTranscriptItem::ArtifactReference {
                    artifact_id,
                    artifact_version_id,
                    title,
                    artifact_kind,
                    storage_kind,
                    external_url,
                    download_url,
                    media_type,
                } => (
                    ConversationItemKind::ArtifactReference,
                    ConversationItemStatus::Completed,
                    ActorRef::agent("agent:primary")
                        .expect("static primary agent id must be valid"),
                    default_parent_item_id,
                    None,
                    json!({
                        "artifact_id": artifact_id,
                        "artifact_version_id": artifact_version_id,
                        "title": title,
                        "artifact_kind": artifact_kind,
                        "storage_kind": storage_kind,
                        "external_url": external_url,
                        "download_url": download_url,
                        "media_type": media_type,
                    }),
                    json!({ "turn_index": context.turn_index }),
                ),
                TurnTranscriptItem::TaskReference { task_id } => (
                        ConversationItemKind::TaskReference,
                        ConversationItemStatus::Completed,
                        ActorRef::agent("agent:primary")
                            .expect("static primary agent id must be valid"),
                        default_parent_item_id,
                        None,
                        json!({ "task_id": task_id }),
                        json!({ "turn_index": context.turn_index }),
                    ),
            };

        let record = self
            .store
            .append_conversation_item(NewConversationItem {
                conversation_id: context.conversation_id.clone(),
                turn_id: Some(context.turn_id.clone()),
                parent_item_id,
                kind,
                status,
                author,
                content_text,
                payload_json,
                metadata: metadata.clone(),
            })
            .await?;
        Ok((record, metadata))
    }
}
