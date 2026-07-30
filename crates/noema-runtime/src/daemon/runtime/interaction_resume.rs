use noema_conversations::{
    ActorRef, ConversationItemKind, ConversationItemRecord, ConversationItemStatus,
    NewConversationItem, ReplayMode,
};
use noema_providers::{
    GenerateInput, GenerateRequest, GenerateStreamEvent, GenerateToolResultInput,
    GenerationPriority, NoemaToolChoice, ProviderInstanceKey, ProviderSelectionMode,
    ProviderSelectionSnapshot, ProviderToolTransport, ReasoningEffort,
};
use noema_store::{ConversationInteractionKind, ConversationInteractionRecord};
use serde_json::Value;
use std::{sync::Arc, time::Duration};
use tokio::sync::mpsc;

use super::{
    actor::RuntimeActor,
    interaction_lifecycle::tool_catalog_digest_for,
    prompt_context::{PromptPlanRequest, plan_prompt_context},
    transcript_persistence::assistant_stream_id,
    turn::{SuccessfulProviderTurn, current_runtime_environment},
    turn_timing::TurnTiming,
};
use crate::daemon::protocol::{RuntimeError, TurnStreamEvent};

const RESUME_LEASE_SECONDS: i64 = 300;
const RESUME_HEARTBEAT_SECONDS: u64 = 60;

impl RuntimeActor {
    pub(super) async fn recover_conversation_interactions(&mut self) -> Result<(), RuntimeError> {
        let resumable = self
            .store
            .list_resumable_conversation_interactions()
            .await?;
        let (item_tx, _item_rx) = mpsc::unbounded_channel();
        let mut first_error = None;
        for interaction in resumable {
            let claimed = match self
                .store
                .claim_conversation_interaction_resumption(
                    &interaction.interaction_id,
                    interaction.revision,
                    "runtime:conversation-interactions",
                    RESUME_LEASE_SECONDS,
                )
                .await
            {
                Ok(claimed) => claimed,
                Err(error) => {
                    first_error.get_or_insert(RuntimeError::from(error));
                    continue;
                }
            };
            if let Err(error) = self
                .resume_claimed_conversation_interaction(claimed, &item_tx)
                .await
            {
                first_error.get_or_insert(error);
            }
        }
        first_error.map_or(Ok(()), Err)
    }

    pub(super) async fn resume_conversation_interaction(
        &mut self,
        interaction: ConversationInteractionRecord,
        item_tx: &mpsc::UnboundedSender<TurnStreamEvent>,
    ) -> Result<(), RuntimeError> {
        let claimed = self
            .store
            .claim_conversation_interaction_resumption(
                &interaction.interaction_id,
                interaction.revision,
                "runtime:conversation-interactions",
                RESUME_LEASE_SECONDS,
            )
            .await?;
        self.resume_claimed_conversation_interaction(claimed, item_tx)
            .await
    }

    pub(super) async fn resume_claimed_conversation_interaction(
        &mut self,
        interaction: ConversationInteractionRecord,
        item_tx: &mpsc::UnboundedSender<TurnStreamEvent>,
    ) -> Result<(), RuntimeError> {
        let claim_token = interaction.resume_claim_token.clone().ok_or_else(|| {
            RuntimeError::Protocol("interaction resume claim is missing".to_string())
        })?;
        let revision = interaction.revision;
        let store = self.store.clone();
        let interaction_id = interaction.interaction_id.clone();
        let result = {
            let execution = self.resume_interaction_provider_call(&interaction, item_tx);
            tokio::pin!(execution);
            let mut heartbeat = tokio::time::interval_at(
                tokio::time::Instant::now() + Duration::from_secs(RESUME_HEARTBEAT_SECONDS),
                Duration::from_secs(RESUME_HEARTBEAT_SECONDS),
            );
            loop {
                tokio::select! {
                    result = &mut execution => break result,
                    _ = heartbeat.tick() => {
                        if let Err(error) = store.heartbeat_conversation_interaction_resumption(
                            &interaction_id,
                            revision,
                            &claim_token,
                            RESUME_LEASE_SECONDS,
                        ).await {
                            break Err(error.into());
                        }
                    }
                }
            }
        };
        match result {
            Ok(()) => {
                self.store
                    .finish_conversation_interaction_resumption(
                        &interaction.interaction_id,
                        revision,
                        &claim_token,
                        None,
                    )
                    .await?;
                Ok(())
            }
            Err(error) => {
                let message = error.to_string();
                let finished = self
                    .store
                    .finish_conversation_interaction_resumption(
                        &interaction.interaction_id,
                        revision,
                        &claim_token,
                        Some(&message),
                    )
                    .await;
                if let Ok(finished) = finished {
                    let _ = self
                        .store
                        .fail_conversation_turn(&interaction.originating_turn_id)
                        .await;
                    let _ = self
                        .persist_failed_a2ui_projection(&finished, item_tx)
                        .await;
                }
                Err(error)
            }
        }
    }

    async fn persist_failed_a2ui_projection(
        &self,
        interaction: &ConversationInteractionRecord,
        item_tx: &mpsc::UnboundedSender<TurnStreamEvent>,
    ) -> Result<(), RuntimeError> {
        if interaction.kind != ConversationInteractionKind::A2UI {
            return Ok(());
        }
        let resolution_item_id = interaction
            .resolution_item_id
            .as_deref()
            .ok_or_else(|| RuntimeError::Protocol("A2UI resolution item is missing".to_string()))?;
        let items = self
            .store
            .list_conversation_items(&interaction.conversation_id, ReplayMode::Visible)
            .await?;
        let resolution = items
            .iter()
            .find(|item| item.item_id == resolution_item_id)
            .ok_or_else(|| RuntimeError::Protocol("A2UI resolution item is missing".to_string()))?;
        let mut payload = resolution.payload_json.clone();
        payload["id"] = serde_json::json!(format!(
            "a2ui:{}:failed:{}",
            interaction.originating_turn_id, interaction.revision
        ));
        payload["payload"]["lifecycle"] = serde_json::json!("failed");
        let record = self
            .store
            .append_conversation_item(NewConversationItem {
                conversation_id: interaction.conversation_id.clone(),
                turn_id: Some(interaction.originating_turn_id.clone()),
                parent_item_id: Some(resolution_item_id.to_string()),
                kind: ConversationItemKind::A2UICard,
                status: ConversationItemStatus::Completed,
                author: ActorRef::agent("agent:primary").expect("static primary agent id is valid"),
                content_text: None,
                payload_json: payload,
                metadata: serde_json::json!({"source": "a2ui_resume_failure"}),
            })
            .await?;
        self.emit_projection_item(&interaction.conversation_id, &record.item_id, item_tx)
            .await
    }

    async fn resume_interaction_provider_call(
        &mut self,
        interaction: &ConversationInteractionRecord,
        item_tx: &mpsc::UnboundedSender<TurnStreamEvent>,
    ) -> Result<(), RuntimeError> {
        let selection = interaction_selection(interaction)?;
        let account = self
            .store
            .get_provider_account(&selection.provider_account_id)
            .await?
            .ok_or_else(|| {
                RuntimeError::Protocol("interaction provider account is missing".to_string())
            })?;
        if account.provider_kind != selection.provider_kind {
            return Err(RuntimeError::Protocol(
                "interaction provider account kind changed".to_string(),
            ));
        }
        let credential_revision = account
            .metadata
            .get("credentialRevision")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        if credential_revision != interaction.credential_revision {
            return Err(RuntimeError::Protocol(
                "interaction credential revision changed".to_string(),
            ));
        }

        let route = Arc::new(
            self.resolve_static_provider_route(selection.clone())
                .await?,
        );
        if route.selection() != &selection
            || route.key().as_str() != interaction.provider_instance_key
        {
            return Err(RuntimeError::Protocol(
                "interaction provider route changed".to_string(),
            ));
        }
        let model = Some(interaction.model.clone());
        let provider = route.operations();
        let capabilities = provider.tool_capabilities(selection.model_profile.as_deref());
        let model_tools = self.model_tools(true, capabilities).await?;
        if tool_catalog_digest_for(&model_tools) != interaction.tool_catalog_digest {
            return Err(RuntimeError::Protocol(
                "interaction tool catalog changed".to_string(),
            ));
        }
        let active_conversation = self
            .hydrate_active_conversation(&interaction.conversation_id, None)
            .await?;
        let memory_root_context = self.native_memory_context();
        let planned = plan_prompt_context(PromptPlanRequest {
            store: &self.store,
            provider,
            conversation_id: &interaction.conversation_id,
            provider_kind: &selection.provider_kind,
            model_profile: selection.model_profile.as_deref(),
            current_input: "",
            memory_root_context: memory_root_context.as_deref(),
        })
        .await?;
        let items = self
            .store
            .list_conversation_items(&interaction.conversation_id, ReplayMode::Visible)
            .await?;
        let provider_call = items
            .iter()
            .find(|item| item.item_id == interaction.provider_call_item_id)
            .ok_or_else(|| {
                RuntimeError::Protocol("interaction provider call item is missing".to_string())
            })?;
        let user_item_id = items
            .iter()
            .find(|item| {
                item.turn_id.as_deref() == Some(interaction.originating_turn_id.as_str())
                    && item.kind == ConversationItemKind::UserText
            })
            .map(|item| item.item_id.clone())
            .ok_or_else(|| {
                RuntimeError::Protocol("interaction originating user item is missing".to_string())
            })?;
        let result_item = interaction
            .tool_result_item_id
            .as_deref()
            .and_then(|id| items.iter().find(|item| item.item_id == id))
            .ok_or_else(|| {
                RuntimeError::Protocol("interaction tool result item is missing".to_string())
            })?;
        let result_input = interaction_tool_result_input(interaction, result_item)?;
        let mut input = planned.input.clone();
        let native_session = selection.provider_kind == "foundation_local"
            && capabilities.native_tool_results
            && provider
                .response_continuation(selection.model_profile.as_deref())
                .supports_active_session();
        let mut ignore_event = |_event: GenerateStreamEvent| {};
        let request = |input: GenerateInput| GenerateRequest {
            conversation_id: Some(interaction.conversation_id.clone()),
            model: model.clone(),
            input,
            instructions: Some(planned.instructions.clone()),
            options: noema_providers::GenerateOptions {
                generation_priority: GenerationPriority::Foreground,
                reasoning_effort: selection.reasoning_effort,
                ..noema_providers::GenerateOptions::default()
            },
            tools: model_tools.provider_tools(),
            tool_transport: model_tools.transport,
            tool_choice: NoemaToolChoice::Auto,
            parallel_tool_calls: capabilities.tool_transport == ProviderToolTransport::Native
                && model_tools.has_callable_tools()
                && capabilities.parallel_tool_calls,
        };
        if native_session {
            input = GenerateInput::NativeToolResults(vec![result_input.clone()]);
        }
        let response = match provider
            .generate_streaming(request(input), &mut ignore_event)
            .await
        {
            Ok(response) => response,
            Err(_error) if native_session => {
                provider
                    .generate_streaming(request(planned.input.clone()), &mut ignore_event)
                    .await?
            }
            Err(error) => return Err(error.into()),
        };
        let turn_index = provider_call
            .metadata
            .get("turn_index")
            .and_then(Value::as_u64)
            .unwrap_or_default();
        let user_input = items
            .iter()
            .find(|item| item.item_id == user_item_id)
            .and_then(|item| item.content_text.clone())
            .unwrap_or_default();
        let turn = SuccessfulProviderTurn {
            conversation_id: interaction.conversation_id.clone(),
            turn_id: interaction.originating_turn_id.clone(),
            turn_index,
            user_item_id,
            user_input,
            task_id: None,
            task_run_id: None,
            task_run_fence: None,
            task_terminal_contract: None,
            cwd: active_conversation.cwd.clone(),
            provider_kind: selection.provider_kind.clone(),
            model,
            reasoning_effort: selection.reasoning_effort,
            provider_route: route,
            initial_stream_id: assistant_stream_id(
                &interaction.originating_turn_id,
                "interaction-resume",
            ),
            response,
            agent_identity: self.agent_identity_for_conversation().await?,
            runtime_environment: current_runtime_environment(active_conversation.cwd.as_deref()),
            tool_capabilities: capabilities,
            initial_model_tools: model_tools.clone(),
            continuation_model_tools: model_tools,
            initial_provider_input: planned.input,
        };
        let timing = TurnTiming::new(
            interaction.conversation_id.clone(),
            interaction.originating_turn_id.clone(),
            turn_index,
            interaction.client_message_id.clone(),
        );
        self.persist_successful_provider_turn(turn, item_tx, &timing)
            .await
    }
}

fn interaction_selection(
    interaction: &ConversationInteractionRecord,
) -> Result<ProviderSelectionSnapshot, RuntimeError> {
    let selection_mode = interaction
        .selection_mode
        .parse::<ProviderSelectionMode>()
        .map_err(|error| RuntimeError::Protocol(error.to_string()))?;
    let reasoning_effort = match interaction.reasoning_effort.as_deref() {
        None => None,
        Some(value) => Some(ReasoningEffort::from_persistence_str(value).ok_or_else(|| {
            RuntimeError::Protocol("interaction reasoning effort is invalid".to_string())
        })?),
    };
    let instance_key = ProviderInstanceKey::new(interaction.provider_instance_key.clone())
        .map_err(|error| RuntimeError::Protocol(error.to_string()))?;
    let mut selection = match selection_mode {
        ProviderSelectionMode::ExplicitProfile => ProviderSelectionSnapshot::explicit(
            interaction.provider_kind.clone(),
            interaction.provider_account_id.clone(),
            interaction.model.clone(),
            reasoning_effort,
            Some("conversation_interaction".to_string()),
        ),
        ProviderSelectionMode::ProviderDefault => ProviderSelectionSnapshot::provider_default(
            interaction.provider_kind.clone(),
            interaction.provider_account_id.clone(),
            reasoning_effort,
            Some("conversation_interaction".to_string()),
        ),
    };
    selection.provider_instance_key = Some(instance_key);
    selection
        .normalized_for_persistence()
        .map_err(|error| RuntimeError::Protocol(error.to_string()))
}

fn interaction_tool_result_input(
    interaction: &ConversationInteractionRecord,
    item: &ConversationItemRecord,
) -> Result<GenerateToolResultInput, RuntimeError> {
    let action = item
        .payload_json
        .pointer("/metadata/action")
        .unwrap_or(&item.payload_json);
    let call_id = action
        .get("provider_call_id")
        .or_else(|| action.get("call_id"))
        .and_then(Value::as_str)
        .ok_or_else(|| {
            RuntimeError::Protocol("interaction tool result call id is missing".to_string())
        })?;
    if call_id != interaction.provider_call_id {
        return Err(RuntimeError::Protocol(
            "interaction tool result call id changed".to_string(),
        ));
    }
    let name = action
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or(&interaction.canonical_tool_name)
        .to_string();
    Ok(GenerateToolResultInput {
        id: action.get("id").and_then(Value::as_str).map(str::to_string),
        call_id: call_id.to_string(),
        provider_name: action
            .get("provider_name")
            .and_then(Value::as_str)
            .map(str::to_string),
        name,
        arguments: action
            .get("arguments")
            .cloned()
            .unwrap_or_else(|| interaction.request.clone()),
        success: action
            .get("success")
            .and_then(Value::as_bool)
            .unwrap_or(item.status == ConversationItemStatus::Completed),
        payload: action.get("payload").cloned().unwrap_or(Value::Null),
    })
}
