impl RuntimeActor {
    fn log_runtime_invariant(
        &self,
        message: impl Into<String>,
        context: serde_json::Value,
        raw: serde_json::Value,
    ) {
        let message = message.into();
        self.system_errors.try_append(
            SystemErrorEvent::new(SYSTEM_ERROR_RUNTIME_INVARIANT, message.clone())
                .with_context(context)
                .with_error_chain([message])
                .with_raw(raw),
        );
    }

    #[cfg(test)]
    pub(in crate::daemon) async fn start_conversation(
        &mut self,
        cwd: Option<String>,
    ) -> Result<StartedConversation, RuntimeError> {
        self.store.ensure_default_actors().await?;
        let provider_route = Arc::new(self.resolve_primary_provider().await?);
        let selection = provider_route.selection();
        let new_conversation = noema_conversations::NewConversation::local_chat_for_provider(
            &selection.provider_kind,
            selection.model_profile.clone(),
            cwd.clone(),
        );
        let durable_conversation = self.store.create_conversation(new_conversation).await?;
        let conversation_id = durable_conversation.conversation_id;
        self.hydrate_active_conversation(&conversation_id, cwd)
            .await?;

        Ok(StartedConversation { conversation_id })
    }

    pub(in crate::daemon) async fn start_primary_conversation(
        &mut self,
        cwd: Option<String>,
    ) -> Result<StartedConversation, RuntimeError> {
        self.store.ensure_default_actors().await?;
        let provider_route = Arc::new(self.resolve_primary_provider().await?);
        let selection = provider_route.selection();
        let durable_conversation = self
            .store
            .get_or_create_primary_conversation_for_provider(
                "human:local",
                &selection.provider_kind,
                selection.model_profile.clone(),
                cwd.clone(),
            )
            .await?;
        let conversation_id = durable_conversation.conversation_id;

        self.hydrate_active_conversation(&conversation_id, cwd)
            .await?;

        self.ensure_initial_name_onboarding_message(&conversation_id, provider_route)
            .await?;

        Ok(StartedConversation { conversation_id })
    }

    async fn ensure_initial_name_onboarding_message(
        &mut self,
        conversation_id: &str,
        provider_route: Arc<ProviderRouteLease>,
    ) -> Result<(), RuntimeError> {
        let agent_identity = self
            .agent_identity_for_conversation()
            .await?;
        if agent_identity.display_name.is_some() {
            return Ok(());
        }
        let replay = self
            .store
            .list_conversation_items(conversation_id, ReplayMode::Visible)
            .await?;
        if !replay.is_empty() {
            return Ok(());
        }

        let conversation = self
            .conversations
            .get(conversation_id)
            .cloned()
            .ok_or_else(|| {
                RuntimeError::Protocol(format!("unknown conversation id: {conversation_id}"))
            })?;
        let turn_index = conversation.next_turn_index;
        let turn = self
            .store
            .create_conversation_turn(NewConversationTurn {
                conversation_id: conversation_id.to_string(),
                trigger_item_id: None,
                metadata: json!({
                    "turn_index": turn_index,
                    "source": "agent_onboarding",
                }),
            })
            .await?;
        let instructions = build_initial_name_onboarding_system_prompt(
            conversation_id,
            turn_index,
            conversation.cwd.as_deref(),
            &agent_identity,
        );
        let provider = provider_route.operations();
        let selection = provider_route.selection();
        let model_profile = selection.model_profile.as_deref();
        let tool_capabilities = provider.tool_capabilities(model_profile);
        let response = match provider
            .generate_streaming(
                GenerateRequest {
                    conversation_id: Some(conversation_id.to_string()),
                    model: selection.model_profile.clone(),
                    input: GenerateInput::Text("NOEMA_INITIAL_NAME_ONBOARDING".to_string()),
                    instructions: Some(instructions),
                    options: GenerateOptions {
                        require_noema_response: true,
                        reasoning_effort: selection.reasoning_effort,
                        prompt_cache_retention: prompt_cache_retention_for(tool_capabilities),
                        ..GenerateOptions::default()
                    },
                    tools: Vec::new(),
                    tool_transport: tool_capabilities.tool_transport,
                    tool_choice: Default::default(),
                    parallel_tool_calls: false,
                },
                &mut |_| {},
            )
            .await
        {
            Ok(response) => response,
            Err(error) => {
                self.store.fail_conversation_turn(&turn.turn_id).await?;
                self.conversations.remove(conversation_id);
                return Err(error.into());
            }
        };
        let raw_provider_response = json!({
            "provider": &response.provider,
            "model": &response.model,
            "response_id": &response.response_id,
            "usage": response.usage.as_ref().map(|usage| json!({
                "input_tokens": usage.input_tokens,
                "output_tokens": usage.output_tokens,
                "total_tokens": usage.total_tokens,
                "cached_input_tokens": usage.cached_input_tokens,
            })),
            "responses": &response.responses,
            "tool_calls": &response.tool_calls,
            "response_status": response.response_status,
        });
        let persisted_count = self
            .persist_agent_initiated_provider_response(
                conversation_id,
                &turn.turn_id,
                turn_index,
                response,
            )
            .await?;
        if persisted_count == 0 {
            self.store.fail_conversation_turn(&turn.turn_id).await?;
            self.conversations.remove(conversation_id);
            self.log_runtime_invariant(
                "initial onboarding response did not include assistant text",
                json!({
                    "conversation_id": conversation_id,
                    "turn_id": turn.turn_id,
                    "turn_index": turn_index,
                    "provider_kind": &selection.provider_kind,
                    "model": model_profile,
                }),
                json!({
                    "persisted_count": persisted_count,
                    "provider_response": raw_provider_response,
                }),
            );
            return Err(RuntimeError::Protocol(
                "initial onboarding response did not include assistant text".to_string(),
            ));
        }
        self.store.complete_conversation_turn(&turn.turn_id).await?;
        if let Some(conversation) = self.conversations.get_mut(conversation_id) {
            conversation.next_turn_index = conversation.next_turn_index.saturating_add(1);
        }
        Ok(())
    }

    pub(in crate::daemon) async fn turn(
        &mut self,
        conversation_id: String,
        input: String,
        item_tx: mpsc::UnboundedSender<TurnStreamEvent>,
        client_message_id: Option<String>,
    ) -> Result<(), RuntimeError> {
        self.turn_with_user_input(
            conversation_id,
            UserTurnInput::Text(input),
            item_tx,
            client_message_id,
        )
        .await
    }

    pub(super) async fn continue_after_governed_action(
        &mut self,
        conversation_id: String,
        action_id: String,
        trigger_item_id: String,
        item_tx: mpsc::UnboundedSender<TurnStreamEvent>,
    ) -> Result<(), RuntimeError> {
        self.turn_with_user_input(
            conversation_id,
            UserTurnInput::GovernedActionContinuation {
                action_id,
                trigger_item_id,
            },
            item_tx,
            None,
        )
        .await
    }

    async fn turn_with_multiple_choice_selection(
        &mut self,
        conversation_id: String,
        selection: MultipleChoiceSelectionInput,
        item_tx: mpsc::UnboundedSender<TurnStreamEvent>,
        client_message_id: Option<String>,
    ) -> Result<(), RuntimeError> {
        self.turn_with_user_input(
            conversation_id,
            UserTurnInput::MultipleChoiceSelection(selection),
            item_tx,
            client_message_id,
        )
        .await
    }

    pub(in crate::daemon) async fn select_multiple_choice(
        &mut self,
        conversation_id: String,
        prompt_item_id: String,
        selected_option_ids: Vec<String>,
        item_tx: mpsc::UnboundedSender<TurnStreamEvent>,
        client_message_id: Option<String>,
    ) -> Result<(), RuntimeError> {
        let selection = self
            .validate_multiple_choice_selection(
                &conversation_id,
                prompt_item_id,
                selected_option_ids,
            )
            .await?;
        self.turn_with_multiple_choice_selection(
            conversation_id,
            selection,
            item_tx,
            client_message_id,
        )
        .await
    }

    async fn validate_multiple_choice_selection(
        &self,
        conversation_id: &str,
        prompt_item_id: String,
        selected_option_ids: Vec<String>,
    ) -> Result<MultipleChoiceSelectionInput, RuntimeError> {
        let items = self
            .store
            .list_conversation_items(conversation_id, ReplayMode::Visible)
            .await?;
        if items.iter().any(|item| {
            item.kind == ConversationItemKind::MultipleChoiceSelection
                && item
                    .payload_json
                    .get("prompt_item_id")
                    .and_then(serde_json::Value::as_str)
                    == Some(prompt_item_id.as_str())
        }) {
            return Err(RuntimeError::Protocol(
                "multiple-choice prompt already has a selection".to_string(),
            ));
        }
        let prompt_item = items
            .iter()
            .find(|item| item.item_id == prompt_item_id)
            .ok_or_else(|| {
                RuntimeError::Protocol(format!(
                    "multiple-choice prompt not found: {prompt_item_id}"
                ))
            })?;
        if prompt_item.kind != ConversationItemKind::MultipleChoicePrompt {
            return Err(RuntimeError::Protocol(format!(
                "conversation item is not a multiple-choice prompt: {prompt_item_id}"
            )));
        }
        let payload: MultipleChoicePromptPayload =
            serde_json::from_value(prompt_item.payload_json.clone()).map_err(|source| {
                RuntimeError::Protocol(format!(
                    "invalid multiple-choice prompt payload for {prompt_item_id}: {source}"
                ))
            })?;
        let _ = &payload.prompt;
        match payload.selection_mode {
            MultipleChoiceSelectionMode::PickOne if selected_option_ids.len() != 1 => {
                return Err(RuntimeError::Protocol(
                    "pick_one multiple-choice selection must include exactly one option"
                        .to_string(),
                ));
            }
            MultipleChoiceSelectionMode::PickMany if selected_option_ids.is_empty() => {
                return Err(RuntimeError::Protocol(
                    "pick_many multiple-choice selection must include at least one option"
                        .to_string(),
                ));
            }
            MultipleChoiceSelectionMode::PickOne | MultipleChoiceSelectionMode::PickMany => {}
        }

        let selected_ids = selected_option_ids
            .iter()
            .map(String::as_str)
            .collect::<HashSet<_>>();
        if selected_ids.len() != selected_option_ids.len() {
            return Err(RuntimeError::Protocol(
                "multiple-choice selected option ids must be unique".to_string(),
            ));
        }
        let option_ids = payload
            .options
            .iter()
            .map(|option| option.id.as_str())
            .collect::<HashSet<_>>();
        if let Some(invalid_id) = selected_option_ids
            .iter()
            .find(|id| !option_ids.contains(id.as_str()))
        {
            return Err(RuntimeError::Protocol(format!(
                "multiple-choice option id is not in the prompt: {invalid_id}"
            )));
        }
        let selected_options = payload
            .options
            .into_iter()
            .filter(|option| selected_ids.contains(option.id.as_str()))
            .collect::<Vec<_>>();

        Ok(MultipleChoiceSelectionInput {
            prompt_item_id,
            selection_mode: payload.selection_mode,
            selected_options,
        })
    }
}
