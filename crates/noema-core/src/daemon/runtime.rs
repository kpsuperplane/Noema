use std::{collections::HashMap, future::Future, pin::Pin, sync::Arc};

use crate::{
    ClaimWriteOutcome, NoemaStore,
    memory::Sensitivity,
    memory_extraction::{
        ExtractorMemoryProposal, ExtractorMemoryResponse, ValidatedMemoryProposal,
        validate_memory_extraction_response_with_assistant_items,
    },
    provider::{
        GenerateInput, GenerateOptions, GenerateOutputItem, GenerateRequest, GenerateResponse,
        GenerateStreamEvent, ModelProvider, ProviderError,
    },
    providers::codex_responses::{CodexProviderConfig, CodexResponsesProvider},
    {
        ActorRef, ConversationItemKind, ConversationItemRecord, ConversationItemStatus,
        NewConversation, NewConversationItem, NewConversationTurn, PersistedAgentStatus,
    },
};
use serde_json::{Value, json};
use tokio::sync::{mpsc, oneshot};

use super::{
    memory_pipeline::{
        AssistantEvidenceItem, ConversationMemoryContext, explicit_memory_claim_candidate,
        explicit_memory_content, memory_activity, memory_activity_failed, project_scope_from_cwd,
        provider_memory_claim_candidate, typed_memory_activity,
    },
    memory_tool::{
        MemoryToolResult, MemoryToolRuntimeContext, execute_search_memory, is_search_memory_tool,
    },
    protocol::{
        AgentStatus, DaemonError, StartedConversation, TurnActivityStatus, TurnStreamEvent,
        TurnTranscriptItem,
    },
};

pub(crate) trait RuntimeModelProvider: std::fmt::Debug + Send + Sync {
    fn generate_streaming<'a>(
        &'a self,
        request: GenerateRequest,
        on_event: &'a mut (dyn FnMut(GenerateStreamEvent) + Send),
    ) -> Pin<Box<dyn Future<Output = Result<GenerateResponse, ProviderError>> + Send + 'a>>;
}

impl<T> RuntimeModelProvider for T
where
    T: ModelProvider + std::fmt::Debug + Send + Sync,
{
    fn generate_streaming<'a>(
        &'a self,
        request: GenerateRequest,
        on_event: &'a mut (dyn FnMut(GenerateStreamEvent) + Send),
    ) -> Pin<Box<dyn Future<Output = Result<GenerateResponse, ProviderError>> + Send + 'a>> {
        Box::pin(async move { ModelProvider::generate_streaming(self, request, on_event).await })
    }
}

#[derive(Debug, Clone)]
pub(crate) struct CodexRuntimeHandle {
    sender: mpsc::Sender<CodexRuntimeCommand>,
}

impl CodexRuntimeHandle {
    pub(crate) async fn spawn(
        mut codex_config: CodexProviderConfig,
        store: NoemaStore,
    ) -> Result<Self, DaemonError> {
        let paths = crate::NoemaPaths::from_process_env()?;
        let account_home = paths.provider_account_home("codex", "default");
        crate::provider_auth::ensure_provider_account_home(&account_home)?;
        apply_provider_account_home(&mut codex_config, &account_home);

        let provider = Arc::new(CodexResponsesProvider::new(codex_config)?);
        Self::spawn_with_provider(provider, store).await
    }

    pub(crate) async fn spawn_with_provider(
        provider: Arc<dyn RuntimeModelProvider>,
        store: NoemaStore,
    ) -> Result<Self, DaemonError> {
        let (sender, receiver) = mpsc::channel(16);
        let actor = CodexRuntimeActor::new(provider, store).await?;
        tokio::spawn(actor.run(receiver));
        Ok(Self { sender })
    }

    pub(crate) async fn start_conversation(
        &self,
        model: Option<String>,
        cwd: Option<String>,
    ) -> Result<StartedConversation, DaemonError> {
        let (reply, reply_rx) = oneshot::channel();
        self.sender
            .send(CodexRuntimeCommand::StartConversation { model, cwd, reply })
            .await
            .map_err(|_| DaemonError::Protocol("daemon runtime stopped".to_string()))?;
        reply_rx
            .await
            .map_err(|_| DaemonError::Protocol("daemon runtime stopped".to_string()))?
    }

    pub(crate) async fn start_primary_conversation(
        &self,
        model: Option<String>,
        cwd: Option<String>,
    ) -> Result<StartedConversation, DaemonError> {
        let (reply, reply_rx) = oneshot::channel();
        self.sender
            .send(CodexRuntimeCommand::StartPrimaryConversation { model, cwd, reply })
            .await
            .map_err(|_| DaemonError::Protocol("daemon runtime stopped".to_string()))?;
        reply_rx
            .await
            .map_err(|_| DaemonError::Protocol("daemon runtime stopped".to_string()))?
    }

    pub(crate) async fn turn(
        &self,
        conversation_id: String,
        input: String,
        item_tx: mpsc::UnboundedSender<TurnStreamEvent>,
    ) -> Result<(), DaemonError> {
        let (reply, reply_rx) = oneshot::channel();
        self.sender
            .send(CodexRuntimeCommand::Turn {
                conversation_id,
                input,
                item_tx,
                reply,
            })
            .await
            .map_err(|_| DaemonError::Protocol("daemon runtime stopped".to_string()))?;
        reply_rx
            .await
            .map_err(|_| DaemonError::Protocol("daemon runtime stopped".to_string()))?
    }

    pub(crate) async fn end_conversation(
        &self,
        conversation_id: String,
    ) -> Result<(), DaemonError> {
        let (reply, reply_rx) = oneshot::channel();
        self.sender
            .send(CodexRuntimeCommand::EndConversation {
                conversation_id,
                reply,
            })
            .await
            .map_err(|_| DaemonError::Protocol("daemon runtime stopped".to_string()))?;
        reply_rx
            .await
            .map_err(|_| DaemonError::Protocol("daemon runtime stopped".to_string()))?
    }

    pub(crate) async fn shutdown(&self) {
        let (reply, reply_rx) = oneshot::channel();
        let _ = self
            .sender
            .send(CodexRuntimeCommand::Shutdown { reply })
            .await;
        let _ = reply_rx.await;
    }
}

fn apply_provider_account_home(
    config: &mut crate::CodexProviderConfig,
    account_home: &std::path::Path,
) {
    config.account_home = Some(account_home.to_path_buf());
}

#[derive(Debug)]
enum CodexRuntimeCommand {
    StartConversation {
        model: Option<String>,
        cwd: Option<String>,
        reply: oneshot::Sender<Result<StartedConversation, DaemonError>>,
    },
    StartPrimaryConversation {
        model: Option<String>,
        cwd: Option<String>,
        reply: oneshot::Sender<Result<StartedConversation, DaemonError>>,
    },
    Turn {
        conversation_id: String,
        input: String,
        item_tx: mpsc::UnboundedSender<TurnStreamEvent>,
        reply: oneshot::Sender<Result<(), DaemonError>>,
    },
    EndConversation {
        conversation_id: String,
        reply: oneshot::Sender<Result<(), DaemonError>>,
    },
    Shutdown {
        reply: oneshot::Sender<()>,
    },
}

#[derive(Debug, Clone)]
struct MemoryExtractionWorkerHandle {
    sender: mpsc::Sender<MemoryExtractionWorkerCommand>,
}

impl MemoryExtractionWorkerHandle {
    async fn spawn(
        _provider: Arc<dyn RuntimeModelProvider>,
        _store: NoemaStore,
    ) -> Result<Self, DaemonError> {
        let (sender, receiver) = mpsc::channel(16);
        let worker = MemoryExtractionWorker;
        tokio::spawn(worker.run(receiver));
        Ok(Self { sender })
    }

    async fn shutdown(&self) {
        let (reply, reply_rx) = oneshot::channel();
        let _ = self
            .sender
            .send(MemoryExtractionWorkerCommand::Shutdown { reply })
            .await;
        let _ = reply_rx.await;
    }
}

#[derive(Debug)]
enum MemoryExtractionWorkerCommand {
    Shutdown { reply: oneshot::Sender<()> },
}

#[derive(Debug)]
struct MemoryExtractionWorker;

impl MemoryExtractionWorker {
    async fn run(self, mut receiver: mpsc::Receiver<MemoryExtractionWorkerCommand>) {
        if let Some(MemoryExtractionWorkerCommand::Shutdown { reply }) = receiver.recv().await {
            let _ = reply.send(());
        }
    }
}

#[derive(Debug)]
struct CodexRuntimeActor {
    provider: Arc<dyn RuntimeModelProvider>,
    memory_extraction_worker: MemoryExtractionWorkerHandle,
    store: NoemaStore,
    conversations: HashMap<String, ActiveConversation>,
}

impl CodexRuntimeActor {
    async fn new(
        provider: Arc<dyn RuntimeModelProvider>,
        store: NoemaStore,
    ) -> Result<Self, DaemonError> {
        Ok(Self {
            memory_extraction_worker: MemoryExtractionWorkerHandle::spawn(
                Arc::clone(&provider),
                store.clone(),
            )
            .await?,
            provider,
            store,
            conversations: HashMap::new(),
        })
    }

    async fn run(mut self, mut receiver: mpsc::Receiver<CodexRuntimeCommand>) {
        while let Some(command) = receiver.recv().await {
            match command {
                CodexRuntimeCommand::StartConversation { model, cwd, reply } => {
                    let _ = reply.send(self.start_conversation(model, cwd).await);
                }
                CodexRuntimeCommand::StartPrimaryConversation { model, cwd, reply } => {
                    let _ = reply.send(self.start_primary_conversation(model, cwd).await);
                }
                CodexRuntimeCommand::Turn {
                    conversation_id,
                    input,
                    item_tx,
                    reply,
                } => {
                    let _ = reply.send(self.turn(conversation_id, input, item_tx).await);
                }
                CodexRuntimeCommand::EndConversation {
                    conversation_id,
                    reply,
                } => {
                    self.conversations.remove(&conversation_id);
                    let _ = reply.send(Ok(()));
                }
                CodexRuntimeCommand::Shutdown { reply } => {
                    self.memory_extraction_worker.shutdown().await;
                    let _ = reply.send(());
                    break;
                }
            }
        }
    }

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

        Ok(StartedConversation { conversation_id })
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

        let local_tool_results = self.execute_local_search_memory_tools(&turn).await;
        let has_local_tool_results = !local_tool_results.is_empty();
        let mut continuation_assistant_response = ProviderAssistantResponse::default();
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

            let continuation_input = local_tool_result_continuation_input(&local_tool_results);
            let continuation_instructions = build_local_tool_result_continuation_system_prompt(
                &turn.conversation_id,
                turn.turn_index,
                turn.cwd.as_deref(),
                &turn.user_input,
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
                handle_provider_stream_event(
                    event,
                    item_tx,
                    &continuation_event_context,
                    &continuation_stream_id,
                    continuation_output_base,
                );
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
            let continuation_action_turn = ProviderActionTurn {
                conversation_id: turn.conversation_id.clone(),
                turn_id: turn.turn_id.clone(),
                turn_index: turn.turn_index,
                user_item_id: turn.user_item_id.clone(),
                provider: continuation_response.provider.clone(),
                stream_id: Some(continuation_stream_id.clone()),
            };
            for (offset, output) in continuation_response.output.into_iter().enumerate() {
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

    async fn persist_provider_response_output_item(
        &mut self,
        turn: &ProviderActionTurn,
        index: usize,
        output: GenerateOutputItem,
        assistant_response: &mut ProviderAssistantResponse,
        item_tx: &mpsc::UnboundedSender<TurnStreamEvent>,
    ) -> Result<(), DaemonError> {
        match output {
            GenerateOutputItem::AssistantText { text } => {
                assistant_response.push_text(&text);
                let metadata = json!({
                    "turn_index": turn.turn_index,
                    "output_index": index,
                    "stream_id": turn.stream_id,
                });
                let assistant_item = self
                    .store
                    .append_conversation_item(NewConversationItem {
                        conversation_id: turn.conversation_id.clone(),
                        turn_id: Some(turn.turn_id.clone()),
                        parent_item_id: Some(turn.user_item_id.clone()),
                        kind: ConversationItemKind::AssistantText,
                        status: ConversationItemStatus::Completed,
                        author: ActorRef::agent("agent:primary"),
                        content_text: Some(text.clone()),
                        payload_json: json!({}),
                        metadata: metadata.clone(),
                    })
                    .await?;
                if assistant_response.item_id.is_none() {
                    assistant_response.item_id = Some(assistant_item.item_id.clone());
                }
                assistant_response.items.push(AssistantEvidenceItem {
                    item_id: assistant_item.item_id.clone(),
                    text: text.clone(),
                });
                send_conversation_item(
                    item_tx,
                    assistant_item,
                    metadata,
                    TurnTranscriptItem::AssistantText { text },
                );
            }
            GenerateOutputItem::MemoryProposals { .. } => {}
            output @ (GenerateOutputItem::ToolCall { .. }
            | GenerateOutputItem::ToolResult { .. }
            | GenerateOutputItem::ApprovalRequest { .. }
            | GenerateOutputItem::ApprovalResult { .. }) => {
                self.persist_provider_action_output_item(turn, index, output, item_tx)
                    .await?;
            }
            GenerateOutputItem::Structured { schema, payload } => {
                let card_id = format!(
                    "provider_structured:{}:{}:{index}",
                    turn.conversation_id, turn.turn_index
                );
                let metadata = json!({
                    "turn_index": turn.turn_index,
                    "output_index": index,
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
                        author: ActorRef::agent("agent:primary"),
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
        }
        Ok(())
    }

    async fn execute_local_search_memory_tools(
        &self,
        turn: &SuccessfulProviderTurn,
    ) -> Vec<MemoryToolResult> {
        let mut results = Vec::new();
        for (index, output) in turn.response.output.iter().enumerate() {
            let GenerateOutputItem::ToolCall { id, name, payload } = output else {
                continue;
            };
            if !is_search_memory_tool(name) {
                continue;
            }

            let context = MemoryToolRuntimeContext {
                conversation_id: turn.conversation_id.clone(),
                turn_id: turn.turn_id.clone(),
                turn_index: turn.turn_index,
                call_site_id: format!("output_{index}"),
                cwd: turn.cwd.clone(),
                user_input: turn.user_input.clone(),
            };
            results.push(execute_search_memory(&self.store, &context, id.clone(), payload).await);
        }
        results
    }

    async fn persist_partial_provider_action_outputs(
        &mut self,
        turn: &ProviderActionTurn,
        output: Vec<GenerateOutputItem>,
        item_tx: &mpsc::UnboundedSender<TurnStreamEvent>,
    ) -> Result<(), DaemonError> {
        for (index, output) in output.into_iter().enumerate() {
            self.persist_provider_action_output_item(turn, index, output, item_tx)
                .await?;
        }
        Ok(())
    }

    async fn persist_provider_action_output_item(
        &mut self,
        turn: &ProviderActionTurn,
        index: usize,
        output: GenerateOutputItem,
        item_tx: &mpsc::UnboundedSender<TurnStreamEvent>,
    ) -> Result<(), DaemonError> {
        match output {
            GenerateOutputItem::ToolCall { id, name, payload } => {
                self.persist_provider_action_output(
                    turn,
                    ProviderActionOutput {
                        index,
                        kind: ConversationItemKind::ToolCall,
                        status: ConversationItemStatus::Completed,
                        action_kind: "tool_call",
                        title: format!("Tool call: {name}"),
                        summary: id.as_deref().map(|id| format!("provider id {id}")),
                        payload: json!({
                            "id": id,
                            "name": name,
                            "payload": payload,
                        }),
                    },
                    item_tx,
                )
                .await?;
            }
            GenerateOutputItem::ToolResult {
                call_id,
                name,
                success,
                payload,
            } => {
                let status = if success == Some(false) {
                    ConversationItemStatus::Failed
                } else {
                    ConversationItemStatus::Completed
                };
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
                        summary: call_id.as_deref().map(|id| format!("call id {id}")),
                        payload: json!({
                            "call_id": call_id,
                            "name": name,
                            "success": success,
                            "payload": payload,
                        }),
                    },
                    item_tx,
                )
                .await?;
            }
            GenerateOutputItem::ApprovalRequest {
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
                            "method": method,
                            "payload": payload,
                        }),
                    },
                    item_tx,
                )
                .await?;
            }
            GenerateOutputItem::ApprovalResult {
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
                        summary: request_id.as_deref().map(|id| format!("request id {id}")),
                        payload: json!({
                            "request_id": request_id,
                            "decision": decision,
                            "payload": payload,
                        }),
                    },
                    item_tx,
                )
                .await?;
            }
            GenerateOutputItem::AssistantText { .. }
            | GenerateOutputItem::MemoryProposals { .. }
            | GenerateOutputItem::Structured { .. } => {}
        }
        Ok(())
    }

    async fn persist_provider_action_output(
        &mut self,
        turn: &ProviderActionTurn,
        action: ProviderActionOutput,
        item_tx: &mpsc::UnboundedSender<TurnStreamEvent>,
    ) -> Result<(), DaemonError> {
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
                author: ActorRef::agent("agent:primary"),
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

    async fn record_turn_failure(
        &mut self,
        context: &ConversationMemoryContext,
        message: String,
        item_tx: &mpsc::UnboundedSender<TurnStreamEvent>,
    ) -> Result<(), DaemonError> {
        self.store.fail_conversation_turn(&context.turn_id).await?;
        self.update_conversation_agent_status(
            &context.conversation_id,
            PersistedAgentStatus::Error,
            item_tx,
        )
        .await?;
        let notice = TurnTranscriptItem::ErrorNotice {
            message,
            recoverable: false,
        };
        self.persist_and_send_turn_item(context, notice, item_tx)
            .await
    }

    async fn persist_provider_memory_proposals(
        &mut self,
        batches: Vec<ProviderMemoryProposalBatch>,
        item_tx: &mpsc::UnboundedSender<TurnStreamEvent>,
    ) -> Result<(), DaemonError> {
        let Some(activity_context) = batches.first().map(|batch| batch.context.clone()) else {
            return Ok(());
        };
        let turn_index = activity_context.turn_index;
        let activity_id = format!(
            "memory_extraction:{}:{turn_index}",
            activity_context.conversation_id
        );
        let proposal_count = batches
            .iter()
            .map(|batch| batch.proposals.len())
            .sum::<usize>();
        let mut validated_proposals = Vec::with_capacity(proposal_count);
        for batch in batches {
            let assistant_item_texts = batch
                .context
                .assistant_items
                .iter()
                .map(|item| item.text.as_str())
                .collect::<Vec<_>>();
            let proposals = match validate_memory_extraction_response_with_assistant_items(
                ExtractorMemoryResponse {
                    proposals: batch.proposals,
                },
                &batch.context.user_content,
                &assistant_item_texts,
            ) {
                Ok(proposals) => proposals,
                Err(error) => {
                    let activity = memory_activity_failed(
                        &activity_id,
                        format!("memory extraction output was rejected: {error}"),
                    );
                    self.persist_and_send_turn_item(&activity_context, activity, item_tx)
                        .await?;
                    return Ok(());
                }
            };
            for proposal in proposals {
                let proposal_index = validated_proposals.len();
                validated_proposals.push(ValidatedProviderMemoryProposal {
                    context: batch.context.clone(),
                    proposal,
                    proposal_index,
                });
            }
        }

        let proposed_summary = match proposal_count {
            0 => "creating no memory candidates".to_string(),
            1 => "creating 1 memory candidate".to_string(),
            count => format!("creating {count} memory candidates"),
        };
        let proposed_activity = memory_activity(
            &activity_id,
            TurnActivityStatus::Started,
            "Memory proposed",
            Some(&proposed_summary),
            json!({
                "turn_index": turn_index,
                "proposal_count": proposal_count,
                "cwd_project_hint": project_scope_from_cwd(activity_context.cwd.as_deref()),
            }),
        );
        send_transient_turn_item(&activity_context, proposed_activity, item_tx);
        tokio::task::yield_now().await;

        let mut claim_ids = Vec::with_capacity(proposal_count);
        let mut claim_outcomes = Vec::with_capacity(proposal_count);
        let mut created_claim_count = 0usize;
        let mut reinforced_claim_count = 0usize;
        let mut failed_proposals = Vec::new();
        for proposal in validated_proposals {
            let proposal_index = proposal.proposal_index;
            let candidate = provider_memory_claim_candidate(
                &proposal.proposal,
                &proposal.context,
                proposal_index,
                "ordinary_chat",
            );
            match self.store.create_or_reinforce_claim(candidate).await {
                Ok(summary) => {
                    match summary.write_outcome {
                        ClaimWriteOutcome::Created => created_claim_count += 1,
                        ClaimWriteOutcome::Reinforced => reinforced_claim_count += 1,
                    }
                    claim_outcomes.push(claim_outcome_json(&summary));
                    claim_ids.push(summary.claim_id);
                }
                Err(error) => {
                    failed_proposals.push(json!({
                        "proposal_index": proposal_index,
                        "error": error.to_string(),
                    }));
                }
            }
        }

        let failed_proposal_count = failed_proposals.len();
        let saved_claim_count = claim_ids.len();
        let (status, title, persisted_summary) =
            provider_memory_claim_activity(saved_claim_count, failed_proposal_count);
        let activity = memory_activity(
            &activity_id,
            status,
            title,
            Some(&persisted_summary),
            json!({
                "turn_index": turn_index,
                "source": "provider_structured_output",
                "proposal_count": proposal_count,
                "claim_ids": claim_ids,
                "claim_outcomes": claim_outcomes,
                "created_claim_count": created_claim_count,
                "reinforced_claim_count": reinforced_claim_count,
                "failed_proposal_count": failed_proposal_count,
                "failed_proposals": failed_proposals,
                "cwd_project_hint": project_scope_from_cwd(activity_context.cwd.as_deref()),
            }),
        );
        self.persist_and_send_turn_item(&activity_context, activity, item_tx)
            .await
    }

    async fn persist_explicit_memory_claim(
        &mut self,
        context: &ConversationMemoryContext,
        content: &str,
        item_tx: &mpsc::UnboundedSender<TurnStreamEvent>,
    ) -> Result<ExplicitMemoryOutcome, DaemonError> {
        let candidate = explicit_memory_claim_candidate(content, context.user_item_id.clone());
        match self.store.create_or_reinforce_claim(candidate).await {
            Ok(summary) => {
                let claim_outcome = claim_outcome_json(&summary);
                let claim_id = summary.claim_id.clone();
                let activity = memory_activity(
                    &format!(
                        "explicit_memory_saved:{}:{}",
                        context.conversation_id, context.turn_index
                    ),
                    TurnActivityStatus::Completed,
                    "Explicit memory saved",
                    Some("saved graph claim"),
                    json!({
                        "turn_index": context.turn_index,
                        "trigger": "explicit_remember",
                        "claim_id": claim_id,
                        "claim_outcomes": [claim_outcome],
                        "created_claim_count": if summary.write_outcome == ClaimWriteOutcome::Created { 1 } else { 0 },
                        "reinforced_claim_count": if summary.write_outcome == ClaimWriteOutcome::Reinforced { 1 } else { 0 },
                        "failed_proposal_count": 0,
                        "predicate_id": summary.predicate_id,
                        "source_item_id": context.user_item_id,
                        "evidence_count": summary.evidence_count,
                        "sensitivity": sensitivity_label(summary.sensitivity),
                    }),
                );
                self.persist_and_send_turn_item(context, activity, item_tx)
                    .await?;
                Ok(ExplicitMemoryOutcome::Saved)
            }
            Err(error) => {
                let activity = memory_activity(
                    &format!(
                        "explicit_memory_failed:{}:{}",
                        context.conversation_id, context.turn_index
                    ),
                    TurnActivityStatus::Failed,
                    "Explicit memory save failed",
                    Some("graph claim write failed"),
                    json!({
                        "turn_index": context.turn_index,
                        "trigger": "explicit_remember",
                        "source_item_id": context.user_item_id,
                        "error": error.to_string(),
                    }),
                );
                self.persist_and_send_turn_item(context, activity, item_tx)
                    .await?;
                Ok(ExplicitMemoryOutcome::Failed)
            }
        }
    }

    async fn persist_and_send_turn_item(
        &mut self,
        context: &ConversationMemoryContext,
        item: TurnTranscriptItem,
        item_tx: &mpsc::UnboundedSender<TurnStreamEvent>,
    ) -> Result<(), DaemonError> {
        let (record, metadata) = self.persist_turn_item(context, &item).await?;
        send_conversation_item(item_tx, record, metadata, item);
        Ok(())
    }

    async fn persist_turn_item(
        &mut self,
        context: &ConversationMemoryContext,
        item: &TurnTranscriptItem,
    ) -> Result<(ConversationItemRecord, Value), DaemonError> {
        let default_parent_item_id = context
            .assistant_item_id
            .clone()
            .or_else(|| Some(context.user_item_id.clone()));
        let (kind, status, author, parent_item_id, content_text, payload_json, metadata) =
            match item {
                TurnTranscriptItem::UserText { text } => (
                    ConversationItemKind::UserText,
                    ConversationItemStatus::Completed,
                    ActorRef::human("human:local"),
                    None,
                    Some(text.clone()),
                    json!({}),
                    json!({ "turn_index": context.turn_index }),
                ),
                TurnTranscriptItem::AssistantText { text } => (
                    ConversationItemKind::AssistantText,
                    ConversationItemStatus::Completed,
                    ActorRef::agent("agent:primary"),
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
                    ActorRef::agent("agent:primary"),
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
                TurnTranscriptItem::A2uiCard {
                    id,
                    schema,
                    payload,
                } => (
                    ConversationItemKind::A2uiCard,
                    ConversationItemStatus::Completed,
                    ActorRef::agent("agent:primary"),
                    default_parent_item_id.clone(),
                    None,
                    json!({
                        "id": id,
                        "schema": schema,
                        "payload": payload,
                    }),
                    json!({
                        "turn_index": context.turn_index,
                        "runtime_item_id": id,
                        "schema": schema,
                    }),
                ),
                TurnTranscriptItem::ErrorNotice {
                    message,
                    recoverable,
                } => (
                    ConversationItemKind::ErrorNotice,
                    ConversationItemStatus::Failed,
                    ActorRef::agent("agent:primary"),
                    default_parent_item_id,
                    Some(message.clone()),
                    json!({
                        "message": message,
                        "recoverable": recoverable,
                    }),
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

    async fn update_conversation_agent_status(
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
}

fn send_conversation_item(
    item_tx: &mpsc::UnboundedSender<TurnStreamEvent>,
    record: ConversationItemRecord,
    metadata: Value,
    item: TurnTranscriptItem,
) {
    let _ = item_tx.send(TurnStreamEvent::ConversationItem {
        conversation_id: record.conversation_id,
        item_id: record.item_id,
        turn_id: record.turn_id,
        metadata,
        item: Box::new(item),
    });
}

fn send_transient_turn_item(
    context: &ConversationMemoryContext,
    item: TurnTranscriptItem,
    item_tx: &mpsc::UnboundedSender<TurnStreamEvent>,
) {
    let runtime_item_id = match &item {
        TurnTranscriptItem::Activity { id, .. } | TurnTranscriptItem::A2uiCard { id, .. } => {
            id.clone()
        }
        TurnTranscriptItem::UserText { .. }
        | TurnTranscriptItem::AssistantText { .. }
        | TurnTranscriptItem::ErrorNotice { .. } => format!(
            "transient:{}:{}",
            context.conversation_id, context.turn_index
        ),
    };
    let _ = item_tx.send(TurnStreamEvent::ConversationItem {
        conversation_id: context.conversation_id.clone(),
        item_id: format!("transient:{runtime_item_id}"),
        turn_id: Some(context.turn_id.clone()),
        metadata: json!({
            "turn_index": context.turn_index,
            "runtime_item_id": runtime_item_id,
            "transient": true,
        }),
        item: Box::new(item),
    });
}

fn handle_provider_stream_event(
    event: GenerateStreamEvent,
    item_tx: &mpsc::UnboundedSender<TurnStreamEvent>,
    context: &ConversationMemoryContext,
    stream_id: &str,
    output_index_base: usize,
) {
    match event {
        GenerateStreamEvent::AssistantTextDelta { delta } => send_assistant_text_delta(
            item_tx,
            &context.conversation_id,
            &context.turn_id,
            stream_id,
            delta,
        ),
        GenerateStreamEvent::MemoryProposalsStarted => {
            send_memory_proposed_transient(context, item_tx);
        }
        GenerateStreamEvent::ToolCallStarted { output_index, name } => {
            send_tool_call_started_transient(
                context,
                item_tx,
                output_index_base + output_index,
                &name,
            );
        }
    }
}

fn send_memory_proposed_transient(
    context: &ConversationMemoryContext,
    item_tx: &mpsc::UnboundedSender<TurnStreamEvent>,
) {
    let activity_id = format!(
        "memory_extraction:{}:{}",
        context.conversation_id, context.turn_index
    );
    let activity = memory_activity(
        &activity_id,
        TurnActivityStatus::Started,
        "Memory proposed",
        Some("memory proposal is streaming"),
        json!({
            "turn_index": context.turn_index,
            "source": "provider_structured_output",
        }),
    );
    send_transient_turn_item(context, activity, item_tx);
}

fn send_tool_call_started_transient(
    context: &ConversationMemoryContext,
    item_tx: &mpsc::UnboundedSender<TurnStreamEvent>,
    output_index: usize,
    name: &str,
) {
    let activity_id = format!(
        "tool_call:{}:{}:{}",
        context.conversation_id, context.turn_index, output_index
    );
    let activity = typed_memory_activity(
        &activity_id,
        "tool_call",
        TurnActivityStatus::Started,
        &format!("Tool call: {name}"),
        Some("tool call is streaming"),
        json!({
            "turn_index": context.turn_index,
            "output_index": output_index,
            "source": "provider_structured_output",
            "action": {
                "name": name,
            },
        }),
    );
    send_transient_turn_item(context, activity, item_tx);
}

fn send_assistant_text_delta(
    item_tx: &mpsc::UnboundedSender<TurnStreamEvent>,
    conversation_id: &str,
    turn_id: &str,
    stream_id: &str,
    delta: String,
) {
    let _ = item_tx.send(TurnStreamEvent::AssistantTextDelta {
        conversation_id: conversation_id.to_string(),
        turn_id: turn_id.to_string(),
        stream_id: stream_id.to_string(),
        delta,
    });
}

const fn conversation_item_status_for_activity(
    status: TurnActivityStatus,
) -> ConversationItemStatus {
    match status {
        TurnActivityStatus::Started => ConversationItemStatus::Running,
        TurnActivityStatus::Completed => ConversationItemStatus::Completed,
        TurnActivityStatus::Failed => ConversationItemStatus::Failed,
    }
}

const fn activity_status_for_conversation_item(
    status: ConversationItemStatus,
) -> TurnActivityStatus {
    match status {
        ConversationItemStatus::Pending | ConversationItemStatus::Running => {
            TurnActivityStatus::Started
        }
        ConversationItemStatus::Completed => TurnActivityStatus::Completed,
        ConversationItemStatus::Failed
        | ConversationItemStatus::Cancelled
        | ConversationItemStatus::Interrupted => TurnActivityStatus::Failed,
    }
}

const fn activity_status_payload(status: TurnActivityStatus) -> &'static str {
    match status {
        TurnActivityStatus::Started => "started",
        TurnActivityStatus::Completed => "completed",
        TurnActivityStatus::Failed => "failed",
    }
}

const RECENT_TRANSCRIPT_ITEM_CHAR_LIMIT: usize = 2_000;
const RECENT_TRANSCRIPT_TOTAL_CHAR_LIMIT: usize = 12_000;

fn build_structured_turn_system_prompt(
    conversation_id: &str,
    turn_index: u64,
    cwd: Option<&str>,
    recent_transcript: &str,
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

    format!(
        r#"You are Noema, a local-first personal assistant. Reply to the user and emit any durable memory proposals in one structured response.

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
After Noema sends a NOEMA_LOCAL_TOOL_RESULT message, answer using only the returned memories.

Active retrieval IDs:
{active_retrieval_ids}

Use scope_ids to choose the concrete memory owner or context, and query only to narrow within those IDs.
For broad questions about what Noema remembers about the user, call search_memory with "scope_ids":["human:local"] and "query":"".
For topical questions about the user, keep "scope_ids":["human:local"] and use a concise topic query such as "aviation" or "planes".
Never invent scope IDs. Use only IDs listed in Active retrieval IDs or returned by prior Noema tools.
Do not tell the user Noema has no memories unless the scoped tool result is empty for the scope actually being discussed.

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

fn build_local_tool_result_continuation_system_prompt(
    conversation_id: &str,
    turn_index: u64,
    cwd: Option<&str>,
    user_input: &str,
) -> String {
    let mut prompt = build_structured_turn_system_prompt(conversation_id, turn_index, cwd, "");
    prompt.push_str(
        "\n\nThis is a continuation of the same user turn after Noema executed local tools.",
    );
    prompt.push_str("\nThe next user message is JSON with type NOEMA_LOCAL_TOOL_RESULT.");
    prompt.push_str("\nUse those results to answer the original user message.");
    prompt.push_str("\n\nOriginal user message:\n");
    prompt.push_str(user_input);
    prompt
}

fn provider_memory_claim_activity(
    saved_count: usize,
    failed_count: usize,
) -> (TurnActivityStatus, &'static str, String) {
    if failed_count == 0 {
        return (
            TurnActivityStatus::Completed,
            "Memory persisted",
            provider_memory_claim_summary(saved_count, failed_count),
        );
    }

    if saved_count == 0 {
        let summary = if failed_count == 1 {
            "graph claim write failed".to_string()
        } else {
            provider_memory_claim_summary(saved_count, failed_count)
        };
        return (
            TurnActivityStatus::Failed,
            "Memory persistence failed",
            summary,
        );
    }

    (
        TurnActivityStatus::Failed,
        "Memory persistence partially failed",
        provider_memory_claim_summary(saved_count, failed_count),
    )
}

fn claim_outcome_json(summary: &crate::ClaimSummary) -> Value {
    json!({
        "claim_id": summary.claim_id,
        "outcome": claim_write_outcome_label(summary.write_outcome),
        "fact_preview": fact_preview(&summary.fact),
        "sensitivity": sensitivity_label(summary.sensitivity),
    })
}

fn claim_write_outcome_label(outcome: ClaimWriteOutcome) -> &'static str {
    match outcome {
        ClaimWriteOutcome::Created => "created",
        ClaimWriteOutcome::Reinforced => "reinforced",
    }
}

fn fact_preview(fact: &str) -> String {
    const MAX_PREVIEW_CHARS: usize = 120;
    let trimmed = fact.trim();
    if trimmed.chars().count() <= MAX_PREVIEW_CHARS {
        return trimmed.to_string();
    }
    let preview = trimmed
        .chars()
        .take(MAX_PREVIEW_CHARS - 3)
        .collect::<String>();
    format!("{preview}...")
}

fn provider_memory_claim_summary(saved_count: usize, failed_count: usize) -> String {
    match (saved_count, failed_count) {
        (1, 0) => "saved 1 graph claim".to_string(),
        (count, 0) => format!("saved {count} graph claims"),
        (0, 1) => "saved 0 graph claims; 1 proposal failed".to_string(),
        (0, failed) => format!("saved 0 graph claims; {failed} proposals failed"),
        (1, 1) => "saved 1 graph claim; 1 proposal failed".to_string(),
        (1, failed) => format!("saved 1 graph claim; {failed} proposals failed"),
        (saved, 1) => format!("saved {saved} graph claims; 1 proposal failed"),
        (saved, failed) => format!("saved {saved} graph claims; {failed} proposals failed"),
    }
}

fn local_tool_result_continuation_input(results: &[MemoryToolResult]) -> Value {
    json!({
        "type": "NOEMA_LOCAL_TOOL_RESULT",
        "results": results.iter().map(local_tool_result_payload).collect::<Vec<_>>(),
    })
}

fn local_tool_result_payload(result: &MemoryToolResult) -> Value {
    json!({
        "call_id": result.call_id,
        "name": result.name,
        "success": result.success,
        "payload": result.payload,
    })
}

fn local_tool_result_output_item(result: &MemoryToolResult) -> GenerateOutputItem {
    GenerateOutputItem::ToolResult {
        call_id: result.call_id.clone(),
        name: Some(result.name.clone()),
        success: Some(result.success),
        payload: result.payload.clone(),
    }
}

fn assistant_stream_id(turn_id: &str, segment: &str) -> String {
    format!("assistant_stream:{turn_id}:{segment}")
}

fn sensitivity_label(sensitivity: Sensitivity) -> &'static str {
    match sensitivity {
        Sensitivity::Public => "public",
        Sensitivity::Normal => "normal",
        Sensitivity::Private => "private",
        Sensitivity::Sensitive => "sensitive",
        Sensitivity::Secret => "secret",
    }
}

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

#[derive(Debug, Clone)]
struct ActiveConversation {
    model: Option<String>,
    cwd: Option<String>,
    next_turn_index: u64,
}

#[derive(Debug)]
struct SuccessfulProviderTurn {
    conversation_id: String,
    turn_id: String,
    turn_index: u64,
    user_item_id: String,
    user_input: String,
    cwd: Option<String>,
    model: Option<String>,
    initial_stream_id: String,
    response: GenerateResponse,
    explicit_memory_outcome: ExplicitMemoryOutcome,
}

#[derive(Debug)]
struct ProviderMemoryProposalBatch {
    context: ConversationMemoryContext,
    proposals: Vec<ExtractorMemoryProposal>,
}

#[derive(Debug, Default)]
struct ProviderAssistantResponse {
    item_id: Option<String>,
    text: String,
    items: Vec<AssistantEvidenceItem>,
}

impl ProviderAssistantResponse {
    fn push_text(&mut self, text: &str) {
        if !self.text.is_empty() {
            self.text.push_str("\n\n");
        }
        self.text.push_str(text);
    }
}

#[derive(Debug)]
struct ValidatedProviderMemoryProposal {
    context: ConversationMemoryContext,
    proposal: ValidatedMemoryProposal,
    proposal_index: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ExplicitMemoryOutcome {
    None,
    Saved,
    Failed,
}

impl ExplicitMemoryOutcome {
    fn was_attempted(&self) -> bool {
        !matches!(self, Self::None)
    }
}

struct ProviderActionTurn {
    conversation_id: String,
    turn_id: String,
    turn_index: u64,
    user_item_id: String,
    provider: String,
    stream_id: Option<String>,
}

struct ProviderActionOutput {
    index: usize,
    kind: ConversationItemKind,
    status: ConversationItemStatus,
    action_kind: &'static str,
    title: String,
    summary: Option<String>,
    payload: serde_json::Value,
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
        );

        assert!(prompt.contains("Active retrieval IDs:"));
        assert!(prompt.contains("- human:local"));
        assert!(prompt.contains("- conversation:conv_123"));
        assert!(prompt.contains("\"scope_ids\":[\"human:local\"],\"query\":\"\""));
        assert!(prompt.contains("Never invent scope IDs"));
    }

    #[test]
    fn codex_config_for_provider_account_uses_account_home() {
        let account_home = std::path::PathBuf::from("/noema/providers/codex/default");
        let mut config = CodexProviderConfig::default();

        apply_provider_account_home(&mut config, &account_home);

        assert_eq!(config.account_home.as_deref(), Some(account_home.as_path()));
    }
}
