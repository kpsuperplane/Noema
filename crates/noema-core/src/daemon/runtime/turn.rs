use crate::{
    ActorRef, ConversationItemKind, ConversationItemRecord, ConversationItemStatus,
    NewConversation, NewConversationItem, NewConversationTurn, PersistedAgentStatus, ReplayMode,
    memory_extraction::{ExtractorMemoryProposal, ValidatedMemoryProposal},
    provider::{
        GenerateInput, GenerateOptions, GenerateOutputItem, GenerateRequest, GenerateResponse,
        GenerateStreamEvent, ProviderError,
    },
};
use serde_json::json;
use tokio::sync::mpsc;

use super::{
    actor::{ActiveConversation, CodexRuntimeActor},
    local_tools::{
        agent_identity_after_local_tools, local_tool_result_continuation_input,
        local_tool_result_output_item,
    },
    transcript_persistence::{
        assistant_stream_id, handle_provider_stream_event, send_conversation_item,
    },
};
use crate::daemon::{
    agent_onboarding::{AgentPromptIdentity, agent_identity_prompt},
    memory_pipeline::{
        AssistantEvidenceItem, ConversationMemoryContext, explicit_memory_content,
        project_scope_from_cwd,
    },
    prompts::AGENT_PERSONALITY_PROMPT,
    protocol::{
        AgentStatus, DaemonError, StartedConversation, TurnStreamEvent, TurnTranscriptItem,
    },
};

impl CodexRuntimeActor {
    pub(super) async fn start_conversation(
        &mut self,
        model: Option<String>,
        cwd: Option<String>,
    ) -> Result<StartedConversation, DaemonError> {
        self.store.ensure_default_actors().await?;
        let new_conversation = NewConversation::local_chat(model.clone(), cwd.clone());
        let durable_conversation = self.store.create_conversation(new_conversation).await?;
        let conversation_id = durable_conversation.conversation_id;
        self.conversations.insert(
            conversation_id.clone(),
            ActiveConversation {
                model,
                cwd,
                next_turn_index: 1,
            },
        );

        Ok(StartedConversation { conversation_id })
    }

    pub(super) async fn start_primary_conversation(
        &mut self,
        model: Option<String>,
        cwd: Option<String>,
    ) -> Result<StartedConversation, DaemonError> {
        self.store.ensure_default_actors().await?;
        let durable_conversation = self
            .store
            .get_or_create_primary_conversation("human:local", model.clone(), cwd.clone())
            .await?;
        let conversation_id = durable_conversation.conversation_id;

        if !self.conversations.contains_key(&conversation_id) {
            let next_turn_index = self
                .store
                .next_conversation_turn_index(&conversation_id)
                .await?;
            self.conversations.insert(
                conversation_id.clone(),
                ActiveConversation {
                    model,
                    cwd,
                    next_turn_index,
                },
            );
        }

        self.ensure_initial_name_onboarding_message(&conversation_id)
            .await?;

        Ok(StartedConversation { conversation_id })
    }

    async fn ensure_initial_name_onboarding_message(
        &mut self,
        conversation_id: &str,
    ) -> Result<(), DaemonError> {
        let agent_identity = self
            .agent_identity_for_conversation(conversation_id)
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
                DaemonError::Protocol(format!("unknown conversation id: {conversation_id}"))
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
        let response = match self
            .provider
            .generate_streaming(
                GenerateRequest {
                    model: conversation.model.clone(),
                    input: GenerateInput::Text("NOEMA_INITIAL_NAME_ONBOARDING".to_string()),
                    instructions: Some(instructions),
                    options: GenerateOptions {
                        require_noema_response: true,
                        ..GenerateOptions::default()
                    },
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
            return Err(DaemonError::Protocol(
                "initial onboarding response did not include assistant text".to_string(),
            ));
        }
        self.store.complete_conversation_turn(&turn.turn_id).await?;
        if let Some(conversation) = self.conversations.get_mut(conversation_id) {
            conversation.next_turn_index = conversation.next_turn_index.saturating_add(1);
        }
        Ok(())
    }

    pub(super) async fn turn(
        &mut self,
        conversation_id: String,
        input: String,
        item_tx: mpsc::UnboundedSender<TurnStreamEvent>,
    ) -> Result<(), DaemonError> {
        let conversation = self
            .conversations
            .get(&conversation_id)
            .cloned()
            .ok_or_else(|| {
                DaemonError::Protocol(format!("unknown conversation id: {conversation_id}"))
            })?;
        let turn_index = conversation.next_turn_index;
        let turn = self
            .store
            .create_conversation_turn(NewConversationTurn {
                conversation_id: conversation_id.clone(),
                trigger_item_id: None,
                metadata: json!({ "turn_index": turn_index }),
            })
            .await?;
        let recent_context_items = self
            .store
            .list_recent_conversation_items_for_context(&conversation_id, 24)
            .await?;
        let recent_transcript = render_recent_transcript_for_prompt(&recent_context_items);
        let agent_identity = self
            .agent_identity_for_conversation(&conversation_id)
            .await?;
        self.update_conversation_agent_status(
            &conversation_id,
            PersistedAgentStatus::InputReceived,
            &item_tx,
        )
        .await?;
        let user_metadata = json!({ "turn_index": turn_index });
        let user_item = self
            .store
            .append_conversation_item(NewConversationItem {
                conversation_id: conversation_id.clone(),
                turn_id: Some(turn.turn_id.clone()),
                parent_item_id: None,
                kind: ConversationItemKind::UserText,
                status: ConversationItemStatus::Completed,
                author: ActorRef::human("human:local"),
                content_text: Some(input.clone()),
                payload_json: json!({}),
                metadata: user_metadata.clone(),
            })
            .await?;
        let user_item_id = user_item.item_id.clone();
        send_conversation_item(
            &item_tx,
            user_item,
            user_metadata,
            TurnTranscriptItem::UserText {
                text: input.clone(),
            },
        );
        let explicit_memory_outcome =
            if let Some(explicit_content) = explicit_memory_content(&input) {
                let memory_context = ConversationMemoryContext {
                    turn_index,
                    conversation_id: conversation_id.clone(),
                    turn_id: turn.turn_id.clone(),
                    user_item_id: user_item_id.clone(),
                    assistant_item_id: None,
                    assistant_items: Vec::new(),
                    user_content: input.clone(),
                    cwd: conversation.cwd.clone(),
                };
                self.persist_explicit_memory_claim(&memory_context, &explicit_content, &item_tx)
                    .await?
            } else {
                ExplicitMemoryOutcome::None
            };
        self.update_conversation_agent_status(
            &conversation_id,
            PersistedAgentStatus::Thinking,
            &item_tx,
        )
        .await?;
        let structured_instructions = build_structured_turn_system_prompt(
            &conversation_id,
            turn_index,
            conversation.cwd.as_deref(),
            &recent_transcript,
            &agent_identity,
        );

        let initial_stream_id = assistant_stream_id(&turn.turn_id, "initial");
        let initial_event_context = ConversationMemoryContext {
            turn_index,
            conversation_id: conversation_id.clone(),
            turn_id: turn.turn_id.clone(),
            user_item_id: user_item_id.clone(),
            assistant_item_id: None,
            assistant_items: Vec::new(),
            user_content: input.clone(),
            cwd: conversation.cwd.clone(),
        };
        let mut on_initial_event = |event| {
            handle_provider_stream_event(
                event,
                &item_tx,
                &initial_event_context,
                &initial_stream_id,
                0,
            );
        };

        match self
            .provider
            .generate_streaming(
                GenerateRequest {
                    model: conversation.model.clone(),
                    input: GenerateInput::Text(input.clone()),
                    instructions: Some(structured_instructions),
                    options: GenerateOptions {
                        require_noema_response: true,
                        ..GenerateOptions::default()
                    },
                },
                &mut on_initial_event,
            )
            .await
        {
            Ok(response) => {
                let result = self
                    .persist_successful_provider_turn(
                        SuccessfulProviderTurn {
                            conversation_id: conversation_id.clone(),
                            turn_id: turn.turn_id.clone(),
                            turn_index,
                            user_item_id: user_item_id.clone(),
                            user_input: input.clone(),
                            cwd: conversation.cwd.clone(),
                            model: conversation.model.clone(),
                            initial_stream_id: initial_stream_id.clone(),
                            response,
                            explicit_memory_outcome,
                            agent_identity,
                        },
                        &item_tx,
                    )
                    .await;
                if let Err(error) = result {
                    let failure_context = ConversationMemoryContext {
                        turn_index,
                        conversation_id: conversation_id.clone(),
                        turn_id: turn.turn_id,
                        user_item_id,
                        assistant_item_id: None,
                        assistant_items: Vec::new(),
                        user_content: input,
                        cwd: conversation.cwd.clone(),
                    };
                    self.record_turn_failure(&failure_context, error.to_string(), &item_tx)
                        .await?;
                    self.conversations.remove(&conversation_id);
                    return Err(error);
                }

                Ok(())
            }
            Err(error) => {
                let partial_output = match &error {
                    ProviderError::PartialResponse {
                        provider, output, ..
                    } => Some((provider.clone(), output.clone())),
                    ProviderError::MissingCredentials { .. }
                    | ProviderError::InvalidRequest { .. }
                    | ProviderError::HttpFailure { .. }
                    | ProviderError::ApiError { .. }
                    | ProviderError::RateLimit { .. }
                    | ProviderError::AuthenticationFailure { .. }
                    | ProviderError::MalformedResponse { .. }
                    | ProviderError::ProtocolError { .. }
                    | ProviderError::Timeout { .. }
                    | ProviderError::UnsupportedFeature { .. }
                    | ProviderError::ProviderUnavailable { .. } => None,
                };
                let error_message = error.to_string();
                let error_context = ConversationMemoryContext {
                    turn_index,
                    conversation_id: conversation_id.clone(),
                    turn_id: turn.turn_id.clone(),
                    user_item_id: user_item_id.clone(),
                    assistant_item_id: None,
                    assistant_items: Vec::new(),
                    user_content: input.clone(),
                    cwd: conversation.cwd.clone(),
                };
                if let Some((provider, output)) = partial_output {
                    let action_turn = ProviderActionTurn {
                        conversation_id: conversation_id.clone(),
                        turn_id: turn.turn_id,
                        turn_index,
                        user_item_id,
                        provider,
                        stream_id: None,
                    };
                    self.persist_partial_provider_action_outputs(&action_turn, output, &item_tx)
                        .await?;
                }
                self.record_turn_failure(&error_context, error_message, &item_tx)
                    .await?;
                self.conversations.remove(&conversation_id);
                Err(error.into())
            }
        }
    }

    async fn persist_successful_provider_turn(
        &mut self,
        turn: SuccessfulProviderTurn,
        item_tx: &mpsc::UnboundedSender<TurnStreamEvent>,
    ) -> Result<(), DaemonError> {
        let initial_memory_proposals = turn.response.memory_proposals();
        let initial_output_count = turn.response.output.len();
        let action_turn = ProviderActionTurn {
            conversation_id: turn.conversation_id.clone(),
            turn_id: turn.turn_id.clone(),
            turn_index: turn.turn_index,
            user_item_id: turn.user_item_id.clone(),
            provider: turn.response.provider.clone(),
            stream_id: Some(turn.initial_stream_id.clone()),
        };
        let mut initial_assistant_response = ProviderAssistantResponse::default();
        for (index, output) in turn.response.output.iter().cloned().enumerate() {
            self.persist_provider_response_output_item(
                &action_turn,
                index,
                output,
                &mut initial_assistant_response,
                item_tx,
            )
            .await?;
        }

        let mut provider_memory_batches = Vec::new();
        if !initial_memory_proposals.is_empty() {
            provider_memory_batches.push(ProviderMemoryProposalBatch {
                context: ConversationMemoryContext {
                    turn_index: turn.turn_index,
                    conversation_id: turn.conversation_id.clone(),
                    turn_id: turn.turn_id.clone(),
                    user_item_id: turn.user_item_id.clone(),
                    assistant_item_id: initial_assistant_response.item_id.clone(),
                    assistant_items: initial_assistant_response.items.clone(),
                    user_content: turn.user_input.clone(),
                    cwd: turn.cwd.clone(),
                },
                proposals: initial_memory_proposals,
            });
        }

        let local_tool_results = self.execute_local_tools(&turn, &turn.agent_identity).await;
        let has_local_tool_results = !local_tool_results.is_empty();
        if has_local_tool_results {
            let local_action_turn = ProviderActionTurn {
                conversation_id: turn.conversation_id.clone(),
                turn_id: turn.turn_id.clone(),
                turn_index: turn.turn_index,
                user_item_id: turn.user_item_id.clone(),
                provider: "noema_local".to_string(),
                stream_id: None,
            };
            for (offset, result) in local_tool_results.iter().enumerate() {
                self.persist_provider_action_output_item(
                    &local_action_turn,
                    initial_output_count + offset,
                    local_tool_result_output_item(result),
                    item_tx,
                )
                .await?;
            }
        }

        let continuation_tool_results = local_tool_results
            .iter()
            .filter(|result| result.requires_provider_continuation())
            .collect::<Vec<_>>();
        if !continuation_tool_results.is_empty() {
            let continuation_agent_identity =
                agent_identity_after_local_tools(&turn.agent_identity, &local_tool_results);
            let continuation_input =
                local_tool_result_continuation_input(&continuation_tool_results);
            let continuation_instructions = build_local_tool_result_continuation_system_prompt(
                &turn.conversation_id,
                turn.turn_index,
                turn.cwd.as_deref(),
                &turn.user_input,
                &continuation_agent_identity,
            );
            let continuation_stream_id = assistant_stream_id(&turn.turn_id, "continuation");
            let continuation_output_base = initial_output_count + local_tool_results.len();
            let continuation_event_context = ConversationMemoryContext {
                turn_index: turn.turn_index,
                conversation_id: turn.conversation_id.clone(),
                turn_id: turn.turn_id.clone(),
                user_item_id: turn.user_item_id.clone(),
                assistant_item_id: None,
                assistant_items: Vec::new(),
                user_content: turn.user_input.clone(),
                cwd: turn.cwd.clone(),
            };
            let mut on_continuation_event = |event| {
                if !matches!(event, GenerateStreamEvent::ToolCallStarted { .. }) {
                    handle_provider_stream_event(
                        event,
                        item_tx,
                        &continuation_event_context,
                        &continuation_stream_id,
                        continuation_output_base,
                    );
                }
            };
            let continuation_response = self
                .provider
                .generate_streaming(
                    GenerateRequest {
                        model: turn.model.clone(),
                        input: GenerateInput::Text(continuation_input.to_string()),
                        instructions: Some(continuation_instructions),
                        options: GenerateOptions {
                            require_noema_response: true,
                            ..GenerateOptions::default()
                        },
                    },
                    &mut on_continuation_event,
                )
                .await?;
            let continuation_memory_proposals = continuation_response.memory_proposals();
            let mut continuation_assistant_response = ProviderAssistantResponse::default();
            let continuation_action_turn = ProviderActionTurn {
                conversation_id: turn.conversation_id.clone(),
                turn_id: turn.turn_id.clone(),
                turn_index: turn.turn_index,
                user_item_id: turn.user_item_id.clone(),
                provider: continuation_response.provider.clone(),
                stream_id: Some(continuation_stream_id.clone()),
            };
            for (offset, output) in continuation_response.output.into_iter().enumerate() {
                if matches!(
                    output,
                    GenerateOutputItem::ToolCall { .. }
                        | GenerateOutputItem::ToolResult { .. }
                        | GenerateOutputItem::ApprovalRequest { .. }
                        | GenerateOutputItem::ApprovalResult { .. }
                ) {
                    continue;
                }
                self.persist_provider_response_output_item(
                    &continuation_action_turn,
                    continuation_output_base + offset,
                    output,
                    &mut continuation_assistant_response,
                    item_tx,
                )
                .await?;
            }
            if !continuation_memory_proposals.is_empty() {
                provider_memory_batches.push(ProviderMemoryProposalBatch {
                    context: ConversationMemoryContext {
                        turn_index: turn.turn_index,
                        conversation_id: turn.conversation_id.clone(),
                        turn_id: turn.turn_id.clone(),
                        user_item_id: turn.user_item_id.clone(),
                        assistant_item_id: continuation_assistant_response.item_id.clone(),
                        assistant_items: continuation_assistant_response.items.clone(),
                        user_content: turn.user_input.clone(),
                        cwd: turn.cwd.clone(),
                    },
                    proposals: continuation_memory_proposals,
                });
            }
        }

        if !turn.explicit_memory_outcome.was_attempted() && !provider_memory_batches.is_empty() {
            self.persist_provider_memory_proposals(provider_memory_batches, item_tx)
                .await?;
        }

        self.store.complete_conversation_turn(&turn.turn_id).await?;
        self.update_conversation_agent_status(
            &turn.conversation_id,
            PersistedAgentStatus::Idle,
            item_tx,
        )
        .await?;

        if let Some(conversation) = self.conversations.get_mut(&turn.conversation_id) {
            conversation.next_turn_index = conversation.next_turn_index.saturating_add(1);
        }

        Ok(())
    }

    pub(super) async fn update_conversation_agent_status(
        &mut self,
        conversation_id: &str,
        status: PersistedAgentStatus,
        item_tx: &mpsc::UnboundedSender<TurnStreamEvent>,
    ) -> Result<(), DaemonError> {
        self.store
            .update_conversation_agent_status(conversation_id, status)
            .await?;
        let _ = item_tx.send(TurnStreamEvent::AgentStatusChanged {
            conversation_id: conversation_id.to_string(),
            status: AgentStatus::from(status),
        });
        Ok(())
    }

    async fn agent_identity_for_conversation(
        &self,
        _conversation_id: &str,
    ) -> Result<AgentPromptIdentity, DaemonError> {
        let agent_id = "agent:primary".to_string();
        let agent = self
            .store
            .get_agent(&agent_id)
            .await?
            .ok_or_else(|| DaemonError::Protocol(format!("unknown agent id: {agent_id}")))?;
        Ok(AgentPromptIdentity {
            agent_id: agent.agent_id,
            display_name: agent.display_name,
        })
    }
}

const RECENT_TRANSCRIPT_ITEM_CHAR_LIMIT: usize = 2_000;
const RECENT_TRANSCRIPT_TOTAL_CHAR_LIMIT: usize = 12_000;

fn render_recent_transcript_for_prompt(items: &[ConversationItemRecord]) -> String {
    let mut rendered = String::new();
    for item in items {
        let Some(role) = transcript_role(item.kind) else {
            continue;
        };
        let Some(content) = item.content_text.as_deref() else {
            continue;
        };
        let content = content.trim();
        if content.is_empty() {
            continue;
        }

        let line = format!(
            "{role}: {}",
            truncate_chars(content, RECENT_TRANSCRIPT_ITEM_CHAR_LIMIT)
        );
        let separator_len = usize::from(!rendered.is_empty());
        if rendered.chars().count() + separator_len + line.chars().count()
            > RECENT_TRANSCRIPT_TOTAL_CHAR_LIMIT
        {
            break;
        }
        if !rendered.is_empty() {
            rendered.push('\n');
        }
        rendered.push_str(&line);
    }

    if rendered.is_empty() {
        "none".to_string()
    } else {
        rendered
    }
}

fn transcript_role(kind: ConversationItemKind) -> Option<&'static str> {
    match kind {
        ConversationItemKind::UserText => Some("User"),
        ConversationItemKind::AssistantText => Some("Noema"),
        ConversationItemKind::Activity
        | ConversationItemKind::A2uiCard
        | ConversationItemKind::ToolCall
        | ConversationItemKind::ToolResult
        | ConversationItemKind::ApprovalRequest
        | ConversationItemKind::ApprovalResult
        | ConversationItemKind::ErrorNotice => None,
    }
}

fn truncate_chars(value: &str, limit: usize) -> String {
    let mut truncated: String = value.chars().take(limit).collect();
    if value.chars().count() > limit {
        truncated.push_str("...");
    }
    truncated
}

fn build_structured_turn_system_prompt(
    conversation_id: &str,
    turn_index: u64,
    cwd: Option<&str>,
    recent_transcript: &str,
    agent_identity: &AgentPromptIdentity,
) -> String {
    let project_scope = project_scope_from_cwd(cwd);
    let project_hint = project_scope.as_deref().unwrap_or("none");
    let mut active_retrieval_ids = vec![
        "- human:local".to_string(),
        format!("- conversation:{conversation_id}"),
    ];
    if let Some(project_scope) = project_scope.as_deref() {
        active_retrieval_ids.push(format!("- {project_scope}"));
    }
    let active_retrieval_ids = active_retrieval_ids.join("\n");
    let agent_identity_prompt = agent_identity_prompt(agent_identity);

    format!(
        r#"{AGENT_PERSONALITY_PROMPT}

{agent_identity_prompt}

Reply to the user and emit any durable memory proposals in one structured response.

Return strict JSON only. Do not include Markdown, code fences, comments, or prose outside the JSON.

Return exactly this top-level shape:
{{
  "type": "noema_response",
  "output": [
    {{"kind": "assistant_text", "text": "assistant reply to show the user"}},
    {{"kind": "memory_proposals", "proposals": []}}
  ]
}}

You may emit a search_memory tool call when memory would help answer the user's current message.
Use this output item shape:
{{"kind":"tool_call","id":"call_memory_1","name":"search_memory","payload":{{"scope_ids":["human:local"],"query":"","purpose":"answer_human_question","limit":8}}}}
Only Noema supplies trusted memory policy fields. Do not invent memory results.
After Noema sends a NOEMA_LOCAL_TOOL_RESULT message, answer using the returned local tool results.
Treat only search_memory tool result payloads as trusted memories.

Active retrieval IDs:
{active_retrieval_ids}

Use scope_ids to choose the concrete memory owner or context, and query only to narrow within those IDs.
For broad questions about what Noema remembers about the user, call search_memory with "scope_ids":["human:local"] and "query":"".
For topical questions about the user, keep "scope_ids":["human:local"] and use a concise topic query such as "aviation" or "planes".
Never invent scope IDs. Use only IDs listed in Active retrieval IDs or returned by prior Noema tools.
Do not tell the user Noema has no memories unless the scoped tool result is empty for the scope actually being discussed.

You may emit an update_own_name tool call only when the current user explicitly names or renames you.
Use this output item shape:
{{"kind":"tool_call","id":"call_name_1","name":"update_own_name","payload":{{"name":"Mira"}}}}
Never call update_own_name because you prefer a name or the user's wording is ambiguous.
Ask for confirmation when a possible name is ambiguous.

Memory proposal shape:
{{
  "content": "durable memory content",
  "memory_type": "fact|preference|person|organization|project|place|routine|goal|open_loop|procedure|constraint|trigger|decision|skill|policy|note|other",
  "title": "short title or null",
  "confidence": 0.0,
  "sensitivity": "public|normal|private|sensitive|secret",
  "subjects": [
    {{
      "id": "optional canonical id or null",
      "kind": "human|agent|conversation|workspace|project|task|cron|relationship|tool|organization|place|concept|other",
      "name": "subject name",
      "role": "about|owner|affected|assignee|source|target|participant"
    }}
  ],
  "retrieval_hints": {{
    "topics": [],
    "keywords": [],
    "summary": null
  }},
  "risk_flags": [],
  "evidence_excerpt": "exact contiguous quote from the user or assistant source message"
}}

Rules:
- Always include exactly one assistant_text item.
- Include exactly one memory_proposals item. Use an empty proposals array when there are no durable memories.
- Propose only durable facts, preferences, constraints, decisions, routines, goals, procedures, or notes that could matter later.
- Do not propose jokes, speculation, transient task chatter, or generic world facts.
- Do not propose memories from assistant acknowledgements, status commentary, celebratory/meta commentary, or statements that something was saved, recorded, remembered, updated, or available in memory.
- Assistant evidence may support durable assistant, conversation, project, or workspace notes, but human-subject memories require direct user evidence.
- evidence_excerpt must be an exact contiguous quote from the original turn/source message and directly support the proposal.
- For assistant-supported proposals, evidence_excerpt must exactly quote the assistant text that generated the proposal in the same provider response phase.
- subjects must be non-empty and must show a human subject or participant when the memory affects a person.
- Use id "human:local" only for the current human/user/me. Do not use it for third-party people.
- confidence must be between 0.0 and 1.0. Use at least 0.70 only when evidence directly supports the proposal.
- Use an empty risk_flags array only for low-risk direct ordinary facts and preferences.
- Add risk_flags for inferred, sensitive, secret, action-triggering, contradiction-prone, third-party, risk-bearing, temporary, or external-egress proposals.

Conversation metadata:
conversation_id: {conversation_id}
turn_index: {turn_index}
cwd_project_hint: {project_hint}

Recent durable transcript from embedded Noema store:
{recent_transcript}"#
    )
}

fn build_initial_name_onboarding_system_prompt(
    conversation_id: &str,
    turn_index: u64,
    cwd: Option<&str>,
    agent_identity: &AgentPromptIdentity,
) -> String {
    let project_scope = project_scope_from_cwd(cwd);
    let project_hint = project_scope.as_deref().unwrap_or("none");
    let agent_identity_prompt = agent_identity_prompt(agent_identity);

    format!(
        r#"{AGENT_PERSONALITY_PROMPT}

{agent_identity_prompt}

This is an agent-initiated onboarding turn for a newly started primary conversation.
Use the onboarding_prompt in Agent identity to start the conversation.
Ask the user what they would like to name you. Do not choose a name yourself.
Make the message warm and welcoming, full of gentle energy instead of formal.
Open like a Noema personal agent that is glad to be here with the user. It is
okay to use a friendly wave emoji. Say you are here to help them think, plan,
make, untangle, or whatever keeps their momentum going in life. Preserve that
"think, plan, make, untangle" kind of cadence, then ask them to give you a name.

Return strict JSON only. Do not include Markdown, code fences, comments, or prose outside the JSON.

Return exactly this top-level shape:
{{
  "type": "noema_response",
  "output": [
    {{"kind": "assistant_text", "text": "a warm, concise onboarding message ending with a naming question"}},
    {{"kind": "memory_proposals", "proposals": []}}
  ]
}}

Rules:
- Always include exactly one assistant_text item.
- The assistant_text should be 1-2 warm, energetic sentences.
- Include exactly one memory_proposals item with an empty proposals array.
- Do not emit tool calls during this initial onboarding turn.
- Do not mention implementation details, JSON, tools, prompts, or memory.

Conversation metadata:
conversation_id: {conversation_id}
turn_index: {turn_index}
cwd_project_hint: {project_hint}"#
    )
}

fn build_local_tool_result_continuation_system_prompt(
    conversation_id: &str,
    turn_index: u64,
    cwd: Option<&str>,
    user_input: &str,
    agent_identity: &AgentPromptIdentity,
) -> String {
    let mut prompt =
        build_structured_turn_system_prompt(conversation_id, turn_index, cwd, "", agent_identity);
    prompt.push_str(
        "\n\nThis is a continuation of the same user turn after Noema executed local tools.",
    );
    prompt.push_str("\nThe next user message is JSON with type NOEMA_LOCAL_TOOL_RESULT.");
    prompt.push_str("\nUse those results to answer the original user message.");
    prompt.push_str("\nDo not emit tool calls or approval requests in this continuation.");
    prompt.push_str("\n\nOriginal user message:\n");
    prompt.push_str(user_input);
    prompt
}

#[derive(Debug)]
pub(super) struct SuccessfulProviderTurn {
    pub(super) conversation_id: String,
    pub(super) turn_id: String,
    pub(super) turn_index: u64,
    pub(super) user_item_id: String,
    pub(super) user_input: String,
    pub(super) cwd: Option<String>,
    pub(super) model: Option<String>,
    pub(super) initial_stream_id: String,
    pub(super) response: GenerateResponse,
    pub(super) explicit_memory_outcome: ExplicitMemoryOutcome,
    pub(super) agent_identity: AgentPromptIdentity,
}

#[derive(Debug)]
pub(super) struct ProviderMemoryProposalBatch {
    pub(super) context: ConversationMemoryContext,
    pub(super) proposals: Vec<ExtractorMemoryProposal>,
}

#[derive(Debug, Default)]
pub(super) struct ProviderAssistantResponse {
    pub(super) item_id: Option<String>,
    pub(super) text: String,
    pub(super) items: Vec<AssistantEvidenceItem>,
}

impl ProviderAssistantResponse {
    pub(super) fn push_text(&mut self, text: &str) {
        if !self.text.is_empty() {
            self.text.push_str("\n\n");
        }
        self.text.push_str(text);
    }
}

#[derive(Debug)]
pub(super) struct ValidatedProviderMemoryProposal {
    pub(super) context: ConversationMemoryContext,
    pub(super) proposal: ValidatedMemoryProposal,
    pub(super) proposal_index: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum ExplicitMemoryOutcome {
    None,
    Saved,
    Failed,
}

impl ExplicitMemoryOutcome {
    pub(super) fn was_attempted(&self) -> bool {
        !matches!(self, Self::None)
    }
}

pub(super) struct ProviderActionTurn {
    pub(super) conversation_id: String,
    pub(super) turn_id: String,
    pub(super) turn_index: u64,
    pub(super) user_item_id: String,
    pub(super) provider: String,
    pub(super) stream_id: Option<String>,
}

pub(super) struct ProviderActionOutput {
    pub(super) index: usize,
    pub(super) kind: ConversationItemKind,
    pub(super) status: ConversationItemStatus,
    pub(super) action_kind: &'static str,
    pub(super) title: String,
    pub(super) summary: Option<String>,
    pub(super) payload: serde_json::Value,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn structured_turn_prompt_exposes_active_retrieval_ids_and_scope_guidance() {
        let prompt = build_structured_turn_system_prompt(
            "conv_123",
            4,
            Some("/Users/kpsuperplane/Documents/Projects/Noema"),
            "",
            &test_agent_identity(),
        );

        assert!(prompt.contains("Active retrieval IDs:"));
        assert!(prompt.contains("- human:local"));
        assert!(prompt.contains("- conversation:conv_123"));
        assert!(prompt.contains("\"scope_ids\":[\"human:local\"],\"query\":\"\""));
        assert!(prompt.contains("Never invent scope IDs"));
    }

    #[test]
    fn structured_turn_prompt_includes_personality_layer_without_weakening_runtime_contract() {
        let prompt =
            build_structured_turn_system_prompt("conv_123", 4, None, "", &test_agent_identity());

        assert!(prompt.contains("Adaptive social energy:"));
        assert!(prompt.contains("Start each conversation at about 6/10 social warmth"));
        assert!(prompt.contains("Never let personality slow down the work"));
        assert!(prompt.contains("Return strict JSON only"));
        assert!(prompt.contains("Always include exactly one assistant_text item"));
        assert!(prompt.contains("Only Noema supplies trusted memory policy fields"));
        assert!(prompt.contains("Do not propose memories from assistant acknowledgements"));
        assert!(prompt.contains("statements that something was saved"));
        assert!(prompt.contains("human-subject memories require direct user evidence"));
    }

    fn test_agent_identity() -> AgentPromptIdentity {
        AgentPromptIdentity {
            agent_id: "agent:primary".to_string(),
            display_name: None,
        }
    }
}
